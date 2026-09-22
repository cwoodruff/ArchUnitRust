//! Reading source files and walking the module tree.

#![allow(clippy::too_many_arguments)]

use std::path::{Path, PathBuf};

use crate::base::ArchUnitError;

use super::ImportOption;
use super::cargo::CrateSource;
use super::location::Location;

/// A parsed module: a file or an inline `mod { .. }` block.
#[derive(Debug)]
pub(crate) struct ParsedModule {
    /// Module name segments below the crate root (empty for the root).
    pub path: Vec<String>,
    /// Crate-relative file path, e.g. `src/domain/order.rs`.
    pub rel_file: String,
    /// Items declared directly in this module, excluding `mod` items (see `children`).
    pub items: Vec<syn::Item>,
    /// Outer attributes of the `mod` item plus inner attributes of the module body.
    pub attrs: Vec<syn::Attribute>,
    pub vis: syn::Visibility,
    pub is_test_code: bool,
    pub line: usize,
    pub children: Vec<ParsedModule>,
}

pub(crate) fn is_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        match &attr.meta {
            syn::Meta::List(list) => list.tokens.to_string() == "test",
            _ => false,
        }
    })
}

pub(crate) fn is_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs
        .iter()
        .any(|attr| attr.path().is_ident("test") || attr.path().is_ident("bench"))
}

fn path_attr(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("path") {
            return None;
        }
        match &attr.meta {
            syn::Meta::NameValue(nv) => match &nv.value {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) => Some(s.value()),
                _ => None,
            },
            _ => None,
        }
    })
}

struct Walker<'a> {
    source: &'a CrateSource,
    options: &'a [Box<dyn ImportOption>],
}

impl Walker<'_> {
    fn location(&self, file: &Path, is_test: bool) -> Location {
        Location::new(
            file.to_path_buf(),
            self.source.crate_dir.clone(),
            self.source.crate_name.clone(),
            self.source.target.clone(),
            is_test,
            self.source.is_workspace_member,
        )
    }

    fn included(&self, location: &Location) -> bool {
        self.options.iter().all(|o| o.includes(location))
    }

    fn rel_file(&self, file: &Path) -> String {
        file.strip_prefix(&self.source.crate_dir)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn file_is_test(&self, file: &Path) -> bool {
        if matches!(
            self.source.target,
            crate::core::domain::TargetKind::Test(_) | crate::core::domain::TargetKind::Bench(_)
        ) {
            return true;
        }
        let rel = self.rel_file(file);
        rel.starts_with("tests/") || rel.starts_with("benches/")
    }

    fn parse_file(&self, file: &Path) -> Result<syn::File, ArchUnitError> {
        let text = std::fs::read_to_string(file).map_err(|e| ArchUnitError::Io {
            path: file.to_path_buf(),
            source: e,
        })?;
        syn::parse_file(&text).map_err(|e| ArchUnitError::Parse {
            path: file.to_path_buf(),
            source: e,
        })
    }

    /// Walks a file module. `child_dir` is where `mod x;` children of this file live.
    fn walk_file(
        &self,
        file: &Path,
        child_dir: &Path,
        path: Vec<String>,
        outer_attrs: Vec<syn::Attribute>,
        vis: syn::Visibility,
        parent_is_test: bool,
        line: usize,
    ) -> Result<Option<ParsedModule>, ArchUnitError> {
        let is_test = parent_is_test || self.file_is_test(file) || is_cfg_test(&outer_attrs);
        if !self.included(&self.location(file, is_test)) {
            return Ok(None);
        }
        let parsed = self.parse_file(file)?;
        let is_test = is_test || is_cfg_test(&parsed.attrs);
        if is_test && !self.included(&self.location(file, true)) {
            return Ok(None);
        }
        let mut attrs = outer_attrs;
        attrs.extend(parsed.attrs);
        self.walk_items(
            file,
            child_dir,
            path,
            parsed.items,
            attrs,
            vis,
            is_test,
            line,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_items(
        &self,
        file: &Path,
        child_dir: &Path,
        path: Vec<String>,
        items: Vec<syn::Item>,
        attrs: Vec<syn::Attribute>,
        vis: syn::Visibility,
        is_test: bool,
        line: usize,
    ) -> Result<Option<ParsedModule>, ArchUnitError> {
        let mut module = ParsedModule {
            path: path.clone(),
            rel_file: self.rel_file(file),
            items: Vec::new(),
            attrs,
            vis,
            is_test_code: is_test,
            line,
            children: Vec::new(),
        };
        for item in items {
            match item {
                syn::Item::Mod(item_mod) => {
                    let name = item_mod.ident.to_string();
                    let mut child_path = path.clone();
                    child_path.push(name.clone());
                    let mod_line = proc_macro2::Span::start(&item_mod.ident.span()).line;
                    let mod_is_test = is_test || is_cfg_test(&item_mod.attrs);
                    let child = match item_mod.content {
                        Some((_, content)) => {
                            let child_dir = child_dir.join(&name);
                            let mut attrs = item_mod.attrs.clone();
                            // Inline modules: outer attrs already contain everything.
                            attrs.retain(|a| !a.path().is_ident("path"));
                            if !self.included(&self.location(file, mod_is_test)) {
                                None
                            } else {
                                self.walk_items(
                                    file,
                                    &child_dir,
                                    child_path,
                                    content,
                                    attrs,
                                    item_mod.vis.clone(),
                                    mod_is_test,
                                    mod_line,
                                )?
                            }
                        }
                        None => {
                            let (child_file, grand_child_dir) =
                                self.resolve_module_file(file, child_dir, &name, &item_mod.attrs)?;
                            self.walk_file(
                                &child_file,
                                &grand_child_dir,
                                child_path,
                                item_mod.attrs.clone(),
                                item_mod.vis.clone(),
                                mod_is_test,
                                mod_line,
                            )?
                        }
                    };
                    if let Some(child) = child {
                        module.children.push(child);
                    }
                }
                other => module.items.push(other),
            }
        }
        Ok(Some(module))
    }

    fn resolve_module_file(
        &self,
        declared_in: &Path,
        child_dir: &Path,
        name: &str,
        attrs: &[syn::Attribute],
    ) -> Result<(PathBuf, PathBuf), ArchUnitError> {
        if let Some(custom) = path_attr(attrs) {
            let base = declared_in.parent().unwrap_or(Path::new("."));
            let file = base.join(custom);
            let dir = file.parent().unwrap_or(base).to_path_buf();
            return Ok((file, dir));
        }
        let candidates = [
            child_dir.join(format!("{name}.rs")),
            child_dir.join(name).join("mod.rs"),
        ];
        for candidate in &candidates {
            if candidate.is_file() {
                let dir = child_dir.join(name);
                return Ok((candidate.clone(), dir));
            }
        }
        Err(ArchUnitError::MissingModuleFile {
            module: name.to_owned(),
            declared_in: declared_in.to_path_buf(),
            tried: candidates.to_vec(),
        })
    }
}

/// Parses a crate root and its module tree. Returns `None` if the root was excluded.
pub(crate) fn parse_crate(
    source: &CrateSource,
    options: &[Box<dyn ImportOption>],
) -> Result<Option<ParsedModule>, ArchUnitError> {
    let walker = Walker { source, options };
    let root = &source.root_file;
    let child_dir = root.parent().unwrap_or(Path::new(".")).to_path_buf();
    walker.walk_file(
        root,
        &child_dir,
        Vec::new(),
        Vec::new(),
        syn::Visibility::Inherited,
        false,
        1,
    )
}
