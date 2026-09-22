//! Scopes and path resolution across modules and crates.

use std::collections::{HashMap, HashSet};

use super::build::Builder;
use super::prelude::{
    EXTERN_PRELUDE_CRATES, PRELUDE_MACROS, PRELUDE_TYPES, PRELUDE_VALUES, PRIMITIVES,
};
use crate::core::domain::ItemKind;
use crate::core::domain::graph::ItemId;

pub(crate) type ScopeId = usize;

/// The result of resolving a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Resolved {
    /// An item (possibly a stub).
    Item(ItemId),
    /// An enum variant.
    Variant { enum_: ItemId, name: String },
    /// An associated item `Base::name`.
    Assoc { base: ItemId, name: String },
    /// A generic parameter in scope.
    Generic(String),
}

impl Resolved {
    /// The item a dependency on this resolution targets.
    pub(crate) fn item(&self) -> Option<ItemId> {
        match self {
            Resolved::Item(id) => Some(*id),
            Resolved::Variant { enum_, .. } => Some(*enum_),
            Resolved::Assoc { base, .. } => Some(*base),
            Resolved::Generic(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum GlobSource {
    Scope(ScopeId),
    Enum(ItemId),
    Stub(String),
}

#[derive(Debug, Clone)]
pub(crate) struct UseEntry {
    pub local: String,
    pub segments: Vec<String>,
    pub glob: bool,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Ns {
    Type,
    Value,
    Macro,
    Any,
}

#[derive(Debug, Default)]
pub(crate) struct Scope {
    pub module: ItemId,
    pub crate_root: ItemId,
    pub parent: Option<ScopeId>,
    pub types: HashMap<String, Resolved>,
    pub values: HashMap<String, Resolved>,
    pub macros: HashMap<String, Resolved>,
    pub uses: Vec<UseEntry>,
    pub globs: Vec<GlobSource>,
}

/// Context needed to resolve `Self` and generic parameters.
#[derive(Debug, Clone, Default)]
pub(crate) struct ResolveCtx {
    pub self_type: Option<ItemId>,
    pub generics: Vec<String>,
}

pub(crate) fn segments_of(path: &syn::Path) -> Vec<String> {
    let mut segments = Vec::new();
    if path.leading_colon.is_some() {
        segments.push(String::new());
    }
    segments.extend(path.segments.iter().map(|s| s.ident.to_string()));
    segments
}

pub(crate) fn path_text(path: &syn::Path) -> String {
    let mut text = String::new();
    if path.leading_colon.is_some() {
        text.push_str("::");
    }
    for (i, seg) in path.segments.iter().enumerate() {
        if i > 0 {
            text.push_str("::");
        }
        text.push_str(&seg.ident.to_string());
    }
    text
}

impl Builder<'_> {
    pub(crate) fn new_scope(
        &mut self,
        module: ItemId,
        crate_root: ItemId,
        parent: Option<ScopeId>,
    ) -> ScopeId {
        let id = self.scopes.len();
        self.scopes.push(Scope {
            module,
            crate_root,
            parent,
            ..Scope::default()
        });
        id
    }

    /// Resolves a path in `scope`. Returns `None` when a segment cannot be found in an imported
    /// module, which may be a transient state during import resolution.
    pub(crate) fn resolve_path(
        &mut self,
        scope: ScopeId,
        segments: &[String],
        ns: Ns,
        ctx: &ResolveCtx,
    ) -> Option<Resolved> {
        if segments.is_empty() {
            return None;
        }
        let first = segments[0].as_str();
        let (mut current, rest): (Resolved, &[String]) = match first {
            "crate" | "$crate" => (
                Resolved::Item(self.scopes[scope].crate_root),
                &segments[1..],
            ),
            "self" => (Resolved::Item(self.scopes[scope].module), &segments[1..]),
            "super" => {
                let mut module = self.scopes[scope].module;
                let mut i = 0;
                while i < segments.len() && segments[i] == "super" {
                    module = self.graph.items[module].package?;
                    i += 1;
                }
                (Resolved::Item(module), &segments[i..])
            }
            "Self" => (Resolved::Item(ctx.self_type?), &segments[1..]),
            "" => {
                let name = segments.get(1)?;
                (self.extern_crate(scope, name)?, &segments[2..])
            }
            _ => {
                let want = if segments.len() == 1 { ns } else { Ns::Type };
                if matches!(want, Ns::Type | Ns::Any) && ctx.generics.iter().any(|g| g == first) {
                    (Resolved::Generic(first.to_owned()), &segments[1..])
                } else {
                    (self.lookup_name(scope, first, want)?, &segments[1..])
                }
            }
        };
        for (i, seg) in rest.iter().enumerate() {
            let want = if i + 1 == rest.len() { ns } else { Ns::Type };
            current = self.step(current, seg, want)?;
        }
        Some(current)
    }

    /// Looks up a single name visible in `scope`, with all fallbacks (extern prelude, std
    /// prelude, primitives, stub globs).
    pub(crate) fn lookup_name(&mut self, scope: ScopeId, name: &str, ns: Ns) -> Option<Resolved> {
        let mut current = Some(scope);
        while let Some(id) = current {
            let mut visited = HashSet::new();
            if let Some(found) = self.lookup_declared(id, name, ns, &mut visited) {
                return Some(found);
            }
            current = self.scopes[id].parent;
        }
        if let Some(found) = self.extern_crate(scope, name) {
            return Some(found);
        }
        if let Some(found) = self.std_prelude(name, ns) {
            return Some(found);
        }
        if matches!(ns, Ns::Type | Ns::Any) {
            if let Some(&id) = self.primitives.get(name) {
                return Some(Resolved::Item(id));
            }
        }
        let mut current = Some(scope);
        while let Some(id) = current {
            let stub_globs: Vec<String> = self.scopes[id]
                .globs
                .iter()
                .filter_map(|g| match g {
                    GlobSource::Stub(path) => Some(path.clone()),
                    _ => None,
                })
                .collect();
            if let Some(path) = stub_globs.first() {
                let stub = self
                    .graph
                    .intern_stub(&format!("{path}::{name}"), ItemKind::Unknown);
                return Some(Resolved::Item(stub));
            }
            current = self.scopes[id].parent;
        }
        None
    }

    /// Names declared or imported in `scope` (and its glob imports), without fallbacks.
    fn lookup_declared(
        &self,
        scope: ScopeId,
        name: &str,
        ns: Ns,
        visited: &mut HashSet<ScopeId>,
    ) -> Option<Resolved> {
        if !visited.insert(scope) {
            return None;
        }
        let s = &self.scopes[scope];
        let direct = match ns {
            Ns::Type => s.types.get(name),
            Ns::Value => s.values.get(name),
            Ns::Macro => s.macros.get(name),
            Ns::Any => s
                .types
                .get(name)
                .or_else(|| s.values.get(name))
                .or_else(|| s.macros.get(name)),
        };
        if let Some(found) = direct {
            return Some(found.clone());
        }
        for glob in &s.globs {
            match glob {
                GlobSource::Scope(other) => {
                    if let Some(found) = self.lookup_declared(*other, name, ns, visited) {
                        return Some(found);
                    }
                }
                GlobSource::Enum(enum_) => {
                    if matches!(ns, Ns::Type | Ns::Value | Ns::Any)
                        && self.has_variant(*enum_, name)
                    {
                        return Some(Resolved::Variant {
                            enum_: *enum_,
                            name: name.to_owned(),
                        });
                    }
                }
                GlobSource::Stub(_) => {}
            }
        }
        None
    }

    fn has_variant(&self, enum_: ItemId, name: &str) -> bool {
        self.graph.items[enum_].members.iter().any(|&m| {
            let member = &self.graph.members[m];
            member.kind == crate::core::domain::MemberKind::Variant && member.name == name
        })
    }

    /// Resolves one more path segment from an already resolved base.
    pub(crate) fn step(&mut self, current: Resolved, seg: &str, ns: Ns) -> Option<Resolved> {
        let Resolved::Item(id) = current else {
            return None;
        };
        let data = &self.graph.items[id];
        if !data.fully_imported {
            let path = format!("{}::{seg}", data.name);
            return Some(Resolved::Item(
                self.graph.intern_stub(&path, ItemKind::Unknown),
            ));
        }
        match data.kind {
            ItemKind::Module => {
                let scope = *self.module_scopes.get(&id)?;
                let mut visited = HashSet::new();
                self.lookup_declared(scope, seg, ns, &mut visited)
            }
            ItemKind::Enum => {
                if self.has_variant(id, seg) {
                    Some(Resolved::Variant {
                        enum_: id,
                        name: seg.to_owned(),
                    })
                } else {
                    Some(Resolved::Assoc {
                        base: id,
                        name: seg.to_owned(),
                    })
                }
            }
            ItemKind::Struct
            | ItemKind::Union
            | ItemKind::Trait
            | ItemKind::TypeAlias
            | ItemKind::Primitive
            | ItemKind::Unknown => Some(Resolved::Assoc {
                base: id,
                name: seg.to_owned(),
            }),
            _ => None,
        }
    }

    /// Resolves a crate name through the extern prelude of the crate owning `scope`.
    pub(crate) fn extern_crate(&mut self, scope: ScopeId, name: &str) -> Option<Resolved> {
        let root = self.scopes[scope].crate_root;
        let lib = self
            .extern_prelude
            .get(&root)
            .and_then(|m| m.get(name))
            .cloned();
        if let Some(lib) = lib {
            if let Some(&id) = self.lib_roots.get(&lib) {
                return Some(Resolved::Item(id));
            }
            return Some(Resolved::Item(
                self.graph.intern_stub(&lib, ItemKind::Module),
            ));
        }
        if EXTERN_PRELUDE_CRATES.contains(&name) {
            return Some(Resolved::Item(
                self.graph.intern_stub(name, ItemKind::Module),
            ));
        }
        if self.graph.items[root].name == name {
            return Some(Resolved::Item(root));
        }
        None
    }

    fn std_prelude(&mut self, name: &str, ns: Ns) -> Option<Resolved> {
        if matches!(ns, Ns::Type | Ns::Any) {
            if let Some((_, path)) = PRELUDE_TYPES.iter().find(|(n, _)| *n == name) {
                return Some(Resolved::Item(
                    self.graph.intern_stub(path, ItemKind::Unknown),
                ));
            }
        }
        if matches!(ns, Ns::Value | Ns::Any) {
            if let Some((_, owner, member)) = PRELUDE_VALUES.iter().find(|(n, _, _)| *n == name) {
                let owner_id = self.graph.intern_stub(owner, ItemKind::Unknown);
                return Some(if name == "drop" {
                    Resolved::Item(
                        self.graph
                            .intern_stub(&format!("{owner}::{member}"), ItemKind::Function),
                    )
                } else {
                    Resolved::Variant {
                        enum_: owner_id,
                        name: (*member).to_owned(),
                    }
                });
            }
        }
        if matches!(ns, Ns::Macro | Ns::Any) && PRELUDE_MACROS.contains(&name) {
            return Some(Resolved::Item(
                self.graph
                    .intern_stub(&format!("std::{name}"), ItemKind::Macro),
            ));
        }
        None
    }

    pub(crate) fn intern_primitives(&mut self) {
        for name in PRIMITIVES {
            let id = self.graph.intern_stub(name, ItemKind::Primitive);
            self.graph.items[id].is_primitive = true;
            self.primitives.insert((*name).to_owned(), id);
        }
    }

    /// Binds a resolved name into the namespaces it belongs to.
    pub(crate) fn bind(&mut self, scope: ScopeId, local: &str, resolved: &Resolved) {
        let (types, values, macros) = match resolved {
            Resolved::Item(id) => match self.graph.items[*id].kind {
                ItemKind::Module
                | ItemKind::Enum
                | ItemKind::Union
                | ItemKind::Trait
                | ItemKind::TypeAlias
                | ItemKind::Primitive
                | ItemKind::ExternCrate => (true, false, false),
                ItemKind::Struct => (true, self.tuple_or_unit_structs.contains(id), false),
                ItemKind::Function | ItemKind::Const | ItemKind::Static => (false, true, false),
                ItemKind::Macro => (false, false, true),
                ItemKind::Impl => (false, false, false),
                ItemKind::Unknown => (true, true, true),
            },
            Resolved::Variant { .. } | Resolved::Assoc { .. } => (true, true, false),
            Resolved::Generic(_) => (false, false, false),
        };
        if self.scopes[scope].parent.is_none() && local != "*" {
            if let Some(target) = resolved.item() {
                let alias = format!(
                    "{}::{local}",
                    self.graph.items[self.scopes[scope].module].name
                );
                if self.graph.items[target].name != alias
                    && !self.graph.by_name.contains_key(&alias)
                {
                    self.graph.aliases.entry(alias.clone()).or_insert(target);
                    self.graph.items[target].aliases.push(alias);
                }
            }
        }
        let s = &mut self.scopes[scope];
        if types {
            s.types.insert(local.to_owned(), resolved.clone());
        }
        if values {
            s.values.insert(local.to_owned(), resolved.clone());
        }
        if macros {
            s.macros.insert(local.to_owned(), resolved.clone());
        }
    }

    /// Resolves all `use` declarations to a fixpoint, then stubs whatever is left.
    pub(crate) fn resolve_uses(&mut self) {
        let ctx = ResolveCtx::default();
        loop {
            let mut progress = false;
            for scope in 0..self.scopes.len() {
                let entries = std::mem::take(&mut self.scopes[scope].uses);
                let mut remaining = Vec::new();
                for entry in entries {
                    if self.try_resolve_use(scope, &entry, &ctx) {
                        progress = true;
                    } else {
                        remaining.push(entry);
                    }
                }
                self.scopes[scope].uses = remaining;
            }
            if !progress {
                break;
            }
        }
        for scope in 0..self.scopes.len() {
            let entries = std::mem::take(&mut self.scopes[scope].uses);
            for entry in entries {
                let path = entry
                    .segments
                    .iter()
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("::");
                let stub_path = format!("<unresolved>::{path}");
                if entry.glob {
                    self.scopes[scope].globs.push(GlobSource::Stub(stub_path));
                } else {
                    let stub = self.graph.intern_stub(&stub_path, ItemKind::Unknown);
                    let resolved = Resolved::Item(stub);
                    self.bind(scope, &entry.local, &resolved);
                    self.record_import(scope, stub, entry.line);
                }
            }
        }
    }

    fn try_resolve_use(&mut self, scope: ScopeId, entry: &UseEntry, ctx: &ResolveCtx) -> bool {
        if entry.glob {
            let Some(resolved) = self.resolve_path(scope, &entry.segments, Ns::Type, ctx) else {
                return false;
            };
            let source = match resolved {
                Resolved::Item(id) => {
                    let data = &self.graph.items[id];
                    if !data.fully_imported {
                        GlobSource::Stub(data.name.clone())
                    } else if data.kind == ItemKind::Enum {
                        GlobSource::Enum(id)
                    } else if let Some(&s) = self.module_scopes.get(&id) {
                        GlobSource::Scope(s)
                    } else {
                        return true;
                    }
                }
                _ => return true,
            };
            if let Some(id) = resolved.item() {
                self.record_import(scope, id, entry.line);
            }
            self.scopes[scope].globs.push(source);
            return true;
        }
        let Some(resolved) = self.resolve_path(scope, &entry.segments, Ns::Any, ctx) else {
            return false;
        };
        self.bind(scope, &entry.local, &resolved);
        if let Some(id) = resolved.item() {
            self.record_import(scope, id, entry.line);
        }
        true
    }

    fn record_import(&mut self, scope: ScopeId, target: ItemId, line: usize) {
        let module = self.scopes[scope].module;
        self.imports.push((module, target, line));
    }

    /// Flattens a `use` tree into entries relative to the tree root.
    pub(crate) fn collect_use_entries(
        tree: &syn::UseTree,
        prefix: &mut Vec<String>,
        line: usize,
        out: &mut Vec<UseEntry>,
    ) {
        match tree {
            syn::UseTree::Path(p) => {
                prefix.push(p.ident.to_string());
                Self::collect_use_entries(&p.tree, prefix, line, out);
                prefix.pop();
            }
            syn::UseTree::Name(n) => {
                let name = n.ident.to_string();
                if name == "self" {
                    if let Some(last) = prefix.last().cloned() {
                        out.push(UseEntry {
                            local: last,
                            segments: prefix.clone(),
                            glob: false,
                            line,
                        });
                    }
                } else {
                    let mut segments = prefix.clone();
                    segments.push(name.clone());
                    out.push(UseEntry {
                        local: name,
                        segments,
                        glob: false,
                        line,
                    });
                }
            }
            syn::UseTree::Rename(r) => {
                let name = r.ident.to_string();
                let local = r.rename.to_string();
                let mut segments = prefix.clone();
                if name != "self" {
                    segments.push(name);
                }
                if local != "_" {
                    out.push(UseEntry {
                        local,
                        segments,
                        glob: false,
                        line,
                    });
                } else {
                    // `use Trait as _;` still imports (for trait methods); keep as a glob-less
                    // entry under an unusable name so the dependency is recorded.
                    out.push(UseEntry {
                        local: format!("_{}", out.len()),
                        segments,
                        glob: false,
                        line,
                    });
                }
            }
            syn::UseTree::Glob(_) => out.push(UseEntry {
                local: "*".to_owned(),
                segments: prefix.clone(),
                glob: true,
                line,
            }),
            syn::UseTree::Group(g) => {
                for item in &g.items {
                    Self::collect_use_entries(item, prefix, line, out);
                }
            }
        }
    }
}
