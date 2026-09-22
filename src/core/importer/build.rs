//! Builds the code graph: declares items, resolves signatures, collects accesses and
//! dependencies.

#![allow(clippy::too_many_arguments)]

use std::collections::{HashMap, HashSet};

use proc_macro2::Span;
use quote::ToTokens;
use syn::visit::Visit;

use super::CrateImporter;
use super::cargo::CrateSource;
use super::parse::{ParsedModule, is_cfg_test, is_test_attr, parse_crate};
use super::prelude::{BUILTIN_ATTRIBUTES, STD_DERIVES};
use super::resolve::{Ns, ResolveCtx, Resolved, ScopeId, UseEntry, path_text, segments_of};
use crate::base::ArchUnitError;
use crate::core::domain::graph::{
    AccessData, DependencyData, Graph, ImplData, ItemData, ItemId, MemberData, MemberId,
    ParameterData,
};
use crate::core::domain::{
    AccessKind, AccessType, AnnotationKind, AnnotationValue, DependencyKind, ItemKind, MemberKind,
    Receiver, ResolutionKind, RustAnnotation, RustItems, RustModifier, RustType,
    SourceCodeLocation, TargetKind, TypeParameter, Visibility,
};

/// A declared item whose signature and body still need processing.
struct Pending {
    item: ItemId,
    scope: ScopeId,
    syn_item: syn::Item,
    file: String,
    is_test: bool,
}

/// A function body to visit for accesses.
struct BodyJob {
    member: MemberId,
    block: syn::Block,
    scope: ScopeId,
    ctx: ResolveCtx,
    params: Vec<(String, RustType)>,
    file: String,
}

/// A dependency recorded while visiting bodies, other than accesses.
/// Method names of `std` prelude traits and the most common `std` types; a call with an
/// unknown receiver is never resolved to a local method of one of these names.
const STD_TRAIT_METHOD_NAMES: &[&str] = &[
    "next",
    "clone",
    "clone_from",
    "to_string",
    "to_owned",
    "into",
    "from",
    "try_into",
    "try_from",
    "as_ref",
    "as_mut",
    "borrow",
    "borrow_mut",
    "iter",
    "iter_mut",
    "into_iter",
    "len",
    "is_empty",
    "eq",
    "ne",
    "cmp",
    "partial_cmp",
    "lt",
    "le",
    "gt",
    "ge",
    "max",
    "min",
    "hash",
    "fmt",
    "default",
    "drop",
    "deref",
    "deref_mut",
    "get",
    "get_mut",
    "insert",
    "remove",
    "push",
    "pop",
    "contains",
    "contains_key",
    "map",
    "map_err",
    "and_then",
    "or_else",
    "unwrap",
    "unwrap_or",
    "unwrap_or_else",
    "unwrap_or_default",
    "expect",
    "ok",
    "err",
    "is_some",
    "is_none",
    "is_ok",
    "is_err",
    "as_str",
    "as_slice",
    "as_bytes",
    "keys",
    "values",
    "entry",
    "extend",
    "collect",
    "filter",
    "find",
    "any",
    "all",
    "count",
    "sum",
    "fold",
    "for_each",
    "enumerate",
    "zip",
    "chain",
    "rev",
    "take",
    "skip",
    "peek",
    "sort",
    "sort_by",
    "sort_by_key",
    "dedup",
    "join",
    "split",
    "trim",
    "starts_with",
    "ends_with",
    "replace",
    "parse",
    "lock",
    "read",
    "write",
    "flush",
    "send",
    "recv",
    "await_",
    "poll",
    "call",
    "add",
    "sub",
    "mul",
    "div",
    "neg",
    "not",
    "index",
    "index_mut",
    "first",
    "last",
    "copied",
    "cloned",
    "then",
    "then_some",
    "matches",
    "chars",
    "bytes",
    "lines",
    "wrapping_add",
];

struct ReferenceDep {
    origin_member: MemberId,
    target: ItemId,
    kind: DependencyKind,
    line: usize,
    macro_argument_count: Option<usize>,
}

pub(crate) struct Builder<'a> {
    options: &'a [Box<dyn super::ImportOption>],
    pub graph: Graph,
    pub scopes: Vec<super::resolve::Scope>,
    pub module_scopes: HashMap<ItemId, ScopeId>,
    pub extern_prelude: HashMap<ItemId, HashMap<String, String>>,
    pub lib_roots: HashMap<String, ItemId>,
    pub primitives: HashMap<String, ItemId>,
    pub tuple_or_unit_structs: HashSet<ItemId>,
    pub imports: Vec<(ItemId, ItemId, usize)>,
    pending: Vec<Pending>,
    bodies: Vec<BodyJob>,
    references: Vec<ReferenceDep>,
    annotation_targets: Vec<(Origin, ItemId, usize)>,
    module_macro_invocations: Vec<(ItemId, syn::ItemMacro, ScopeId, String)>,
}

#[derive(Clone, Copy)]
enum Origin {
    Item(ItemId),
    Member(MemberId),
}

pub(crate) fn build(
    importer: &CrateImporter,
    sources: Vec<CrateSource>,
    description: &str,
) -> Result<RustItems, ArchUnitError> {
    let mut builder = Builder {
        options: importer.options(),
        graph: Graph::default(),
        scopes: Vec::new(),
        module_scopes: HashMap::new(),
        extern_prelude: HashMap::new(),
        lib_roots: HashMap::new(),
        primitives: HashMap::new(),
        tuple_or_unit_structs: HashSet::new(),
        imports: Vec::new(),
        pending: Vec::new(),
        bodies: Vec::new(),
        references: Vec::new(),
        annotation_targets: Vec::new(),
        module_macro_invocations: Vec::new(),
    };
    builder.intern_primitives();
    for source in &sources {
        if let Some(root) = parse_crate(source, importer.options())? {
            builder.declare_crate(source, root);
        }
    }
    builder.resolve_uses();
    builder.resolve_signatures();
    builder.collect_bodies();
    builder.build_dependencies();
    let mut graph = builder.graph;
    graph.build_indexes();
    Ok(RustItems::from_graph(graph, description.to_owned()))
}

fn line_of(span: Span) -> usize {
    span.start().line
}

fn visibility_of(vis: &syn::Visibility) -> Visibility {
    match vis {
        syn::Visibility::Public(_) => Visibility::Pub,
        syn::Visibility::Inherited => Visibility::Private,
        syn::Visibility::Restricted(r) => {
            let path = path_text(&r.path);
            match path.as_str() {
                "crate" => Visibility::PubCrate,
                "super" => Visibility::PubSuper,
                "self" => Visibility::Private,
                _ => Visibility::PubIn(path),
            }
        }
    }
}

fn tokens_to_string(tokens: &impl ToTokens) -> String {
    let text = tokens.to_token_stream().to_string();
    text.replace(" :: ", "::")
        .replace(":: ", "::")
        .replace(" ::", "::")
        .replace(" < ", "<")
        .replace(" <", "<")
        .replace("< ", "<")
        .replace(" >", ">")
        .replace(" ,", ",")
        .replace("& ", "&")
        .replace(" '", "'")
        .replace(" (", "(")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(" ;", ";")
        .replace("# [", "#[")
}

fn receiver_of(sig: &syn::Signature) -> Option<Receiver> {
    sig.inputs.iter().find_map(|arg| match arg {
        syn::FnArg::Receiver(r) => Some(if r.reference.is_none() {
            Receiver::Value
        } else if r.mutability.is_some() {
            Receiver::MutRef
        } else {
            Receiver::Ref
        }),
        syn::FnArg::Typed(_) => None,
    })
}

fn fn_modifiers(sig: &syn::Signature) -> Vec<RustModifier> {
    let mut mods = Vec::new();
    if sig.unsafety.is_some() {
        mods.push(RustModifier::Unsafe);
    }
    if sig.asyncness.is_some() {
        mods.push(RustModifier::Async);
    }
    if sig.constness.is_some() {
        mods.push(RustModifier::Const);
    }
    if sig.abi.is_some() {
        mods.push(RustModifier::Extern);
    }
    mods
}

fn pattern_name(pat: &syn::Pat) -> String {
    match pat {
        syn::Pat::Ident(i) => i.ident.to_string(),
        syn::Pat::Type(t) => pattern_name(&t.pat),
        syn::Pat::Reference(r) => pattern_name(&r.pat),
        other => tokens_to_string(other),
    }
}

fn is_screaming_case(name: &str) -> bool {
    name.chars().any(|c| c.is_ascii_uppercase())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

impl Builder<'_> {
    // ---- declaration pass ----------------------------------------------------------------

    fn declare_crate(&mut self, source: &CrateSource, root: ParsedModule) {
        let location = SourceCodeLocation::of(root.rel_file.clone(), root.line);
        let mut data = ItemData::stub(&source.crate_name, ItemKind::Module, None);
        data.fully_imported = true;
        data.crate_name = source.crate_name.clone();
        data.target = source.target.clone();
        data.location = location;
        data.is_test_code = root.is_test_code;
        data.visibility = Visibility::Pub;
        data.modifiers = vec![RustModifier::Pub];
        let root_id = self.graph.add_item(data);
        self.graph.crate_roots.push(root_id);
        if matches!(source.target, TargetKind::Lib | TargetKind::ProcMacro) {
            self.lib_roots
                .entry(source.lib_name.clone())
                .or_insert(root_id);
        }
        self.extern_prelude
            .insert(root_id, source.extern_crates.iter().cloned().collect());
        let scope = self.new_scope(root_id, root_id, None);
        self.module_scopes.insert(root_id, scope);
        self.declare_module_contents(root_id, root_id, scope, root, source);
    }

    fn declare_module_contents(
        &mut self,
        module: ItemId,
        crate_root: ItemId,
        scope: ScopeId,
        parsed: ParsedModule,
        source: &CrateSource,
    ) {
        self.pending.push(Pending {
            item: module,
            scope,
            syn_item: syn::Item::Verbatim(Default::default()),
            file: parsed.rel_file.clone(),
            is_test: parsed.is_test_code,
        });
        self.graph.items[module].annotations = self.plain_annotations(&parsed.attrs);
        for item in &parsed.items {
            self.declare_item(
                item,
                module,
                crate_root,
                scope,
                &parsed.rel_file,
                parsed.is_test_code,
                None,
            );
        }
        for child in parsed.children {
            let name = format!(
                "{}::{}",
                self.graph.items[module].name,
                child.path.last().cloned().unwrap_or_default()
            );
            let mut data = ItemData::stub(&name, ItemKind::Module, Some(module));
            data.fully_imported = true;
            data.crate_name = source.crate_name.clone();
            data.target = source.target.clone();
            data.location = SourceCodeLocation::of(child.rel_file.clone(), child.line);
            data.is_test_code = child.is_test_code;
            data.visibility = visibility_of(&child.vis);
            data.modifiers = data.visibility.modifiers();
            let id = self.graph.add_item(data);
            let child_scope = self.new_scope(id, crate_root, None);
            self.module_scopes.insert(id, child_scope);
            let simple = self.graph.items[id].simple_name.clone();
            self.scopes[scope].types.insert(simple, Resolved::Item(id));
            self.declare_module_contents(id, crate_root, child_scope, child, source);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn declare_item(
        &mut self,
        item: &syn::Item,
        module: ItemId,
        crate_root: ItemId,
        scope: ScopeId,
        file: &str,
        module_is_test: bool,
        enclosing: Option<(ItemId, ScopeId)>,
    ) {
        let attrs = match item {
            syn::Item::Const(i) => &i.attrs,
            syn::Item::Enum(i) => &i.attrs,
            syn::Item::ExternCrate(i) => &i.attrs,
            syn::Item::Fn(i) => &i.attrs,
            syn::Item::ForeignMod(i) => &i.attrs,
            syn::Item::Impl(i) => &i.attrs,
            syn::Item::Macro(i) => &i.attrs,
            syn::Item::Static(i) => &i.attrs,
            syn::Item::Struct(i) => &i.attrs,
            syn::Item::Trait(i) => &i.attrs,
            syn::Item::TraitAlias(i) => &i.attrs,
            syn::Item::Type(i) => &i.attrs,
            syn::Item::Union(i) => &i.attrs,
            syn::Item::Use(i) => &i.attrs,
            _ => return,
        };
        let is_test = module_is_test || is_cfg_test(attrs) || is_test_attr(attrs);
        if is_test && !module_is_test {
            let root = &self.graph.items[crate_root];
            let location = super::location::Location::new(
                std::path::PathBuf::from(file),
                std::path::PathBuf::new(),
                root.crate_name.clone(),
                root.target.clone(),
                true,
                true,
            );
            if !self.options.iter().all(|o| o.includes(&location)) {
                return;
            }
        }
        let (name, kind, vis, line) = match item {
            syn::Item::Use(u) => {
                let mut entries = Vec::new();
                let mut prefix = Vec::new();
                if u.leading_colon.is_some() {
                    prefix.push(String::new());
                }
                Builder::collect_use_entries(
                    &u.tree,
                    &mut prefix,
                    line_of(u.use_token.span),
                    &mut entries,
                );
                self.scopes[scope].uses.extend(entries);
                return;
            }
            syn::Item::ExternCrate(e) => {
                let name = e.ident.to_string();
                let local = e
                    .rename
                    .as_ref()
                    .map(|(_, r)| r.to_string())
                    .unwrap_or_else(|| name.clone());
                if local != "_" {
                    self.scopes[scope].uses.push(UseEntry {
                        local,
                        segments: vec![String::new(), name],
                        glob: false,
                        line: line_of(e.ident.span()),
                    });
                }
                return;
            }
            syn::Item::Const(c) => (
                c.ident.to_string(),
                ItemKind::Const,
                &c.vis,
                line_of(c.ident.span()),
            ),
            syn::Item::Static(s) => (
                s.ident.to_string(),
                ItemKind::Static,
                &s.vis,
                line_of(s.ident.span()),
            ),
            syn::Item::Enum(e) => (
                e.ident.to_string(),
                ItemKind::Enum,
                &e.vis,
                line_of(e.ident.span()),
            ),
            syn::Item::Struct(s) => (
                s.ident.to_string(),
                ItemKind::Struct,
                &s.vis,
                line_of(s.ident.span()),
            ),
            syn::Item::Union(u) => (
                u.ident.to_string(),
                ItemKind::Union,
                &u.vis,
                line_of(u.ident.span()),
            ),
            syn::Item::Trait(t) => (
                t.ident.to_string(),
                ItemKind::Trait,
                &t.vis,
                line_of(t.ident.span()),
            ),
            syn::Item::TraitAlias(t) => (
                t.ident.to_string(),
                ItemKind::Trait,
                &t.vis,
                line_of(t.ident.span()),
            ),
            syn::Item::Type(t) => (
                t.ident.to_string(),
                ItemKind::TypeAlias,
                &t.vis,
                line_of(t.ident.span()),
            ),
            syn::Item::Fn(f) => (
                f.sig.ident.to_string(),
                ItemKind::Function,
                &f.vis,
                line_of(f.sig.ident.span()),
            ),
            syn::Item::Macro(m) => match &m.ident {
                Some(ident) => (
                    ident.to_string(),
                    ItemKind::Macro,
                    &syn::Visibility::Inherited,
                    line_of(ident.span()),
                ),
                None => {
                    self.module_macro_invocations
                        .push((module, m.clone(), scope, file.to_owned()));
                    return;
                }
            },
            syn::Item::Impl(i) => {
                let self_text = tokens_to_string(&i.self_ty);
                let name = match &i.trait_ {
                    Some((_, path, _)) => {
                        format!("<impl {} for {self_text}>", tokens_to_string(path))
                    }
                    None => format!("<impl {self_text}>"),
                };
                (
                    name,
                    ItemKind::Impl,
                    &syn::Visibility::Inherited,
                    line_of(i.impl_token.span),
                )
            }
            syn::Item::ForeignMod(f) => {
                for foreign in &f.items {
                    if let syn::ForeignItem::Fn(func) = foreign {
                        let item = syn::Item::Fn(syn::ItemFn {
                            attrs: func.attrs.clone(),
                            vis: func.vis.clone(),
                            sig: func.sig.clone(),
                            block: Box::new(syn::Block {
                                brace_token: Default::default(),
                                stmts: Vec::new(),
                            }),
                        });
                        self.declare_item(
                            &item,
                            module,
                            crate_root,
                            scope,
                            file,
                            module_is_test,
                            enclosing,
                        );
                    }
                }
                return;
            }
            _ => return,
        };
        let parent_name = match enclosing {
            Some((enclosing_item, _)) => self.graph.items[enclosing_item].name.clone(),
            None => self.graph.items[module].name.clone(),
        };
        let full_name = format!("{parent_name}::{name}");
        let visibility = visibility_of(vis);
        let mut data = ItemData::stub(&full_name, kind, Some(module));
        data.simple_name = name.clone();
        data.fully_imported = true;
        data.crate_name = self.graph.items[crate_root].crate_name.clone();
        data.target = self.graph.items[crate_root].target.clone();
        data.location = SourceCodeLocation::of(file.to_owned(), line);
        data.is_test_code = is_test;
        data.visibility = visibility.clone();
        data.modifiers = visibility.modifiers();
        data.enclosing_item = enclosing.map(|(e, _)| e);
        let id = self.graph.add_item(data);
        if let syn::Item::Struct(s) = item {
            if !matches!(s.fields, syn::Fields::Named(_)) {
                self.tuple_or_unit_structs.insert(id);
            }
        }
        let binding = Resolved::Item(id);
        if kind != ItemKind::Impl {
            self.bind(scope, &name, &binding);
        }
        self.pending.push(Pending {
            item: id,
            scope,
            syn_item: item.clone(),
            file: file.to_owned(),
            is_test,
        });
        // Local items inside function bodies get a block scope of their own.
        let bodies: Vec<&syn::Block> = match item {
            syn::Item::Fn(f) => vec![&f.block],
            syn::Item::Impl(i) => i
                .items
                .iter()
                .filter_map(|it| match it {
                    syn::ImplItem::Fn(f) => Some(&f.block),
                    _ => None,
                })
                .collect(),
            syn::Item::Trait(t) => t
                .items
                .iter()
                .filter_map(|it| match it {
                    syn::TraitItem::Fn(f) => f.default.as_ref(),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let mut local_items = Vec::new();
        for block in bodies {
            collect_local_items(block, &mut local_items);
        }
        if !local_items.is_empty() {
            let block_scope = self.new_scope(module, crate_root, Some(scope));
            if let Some(pending) = self.pending.iter_mut().rev().find(|p| p.item == id) {
                pending.scope = block_scope;
            }
            for local in local_items {
                self.declare_item(
                    &local,
                    module,
                    crate_root,
                    block_scope,
                    file,
                    is_test,
                    Some((id, block_scope)),
                );
            }
        }
    }

    // ---- annotations -----------------------------------------------------------------------

    /// Annotations without path resolution (used before `use` resolution, e.g. for modules).
    fn plain_annotations(&self, attrs: &[syn::Attribute]) -> Vec<RustAnnotation> {
        let mut out = Vec::new();
        for attr in attrs {
            for (annotation, _) in annotations_of_attr(attr) {
                out.push(annotation);
            }
        }
        out
    }

    /// Annotations with resolved macro paths; returns the resolved target items for dependencies.
    fn resolved_annotations(
        &mut self,
        attrs: &[syn::Attribute],
        scope: ScopeId,
    ) -> (Vec<RustAnnotation>, Vec<(ItemId, usize, bool)>) {
        let mut annotations = Vec::new();
        let mut targets = Vec::new();
        let ctx = ResolveCtx::default();
        for attr in attrs {
            let line = line_of(attr.pound_token.span);
            for (mut annotation, path) in annotations_of_attr(attr) {
                let is_derive = annotation.kind == AnnotationKind::Derive;
                let simple = annotation.simple_name().to_owned();
                let std_derive = STD_DERIVES
                    .iter()
                    .find(|(n, _)| *n == simple)
                    .map(|(_, p)| *p);
                let resolved: Option<ItemId> =
                    if is_derive {
                        if let (1, Some(std_path)) = (path.len(), std_derive) {
                            Some(self.graph.intern_stub(std_path, ItemKind::Trait))
                        } else {
                            self.resolve_path(scope, &path, Ns::Macro, &ctx)
                                .and_then(|r| r.item())
                                .or_else(|| {
                                    let p = path.join("::");
                                    Some(self.graph.intern_stub(
                                        &format!("<unresolved>::{p}"),
                                        ItemKind::Unknown,
                                    ))
                                })
                        }
                    } else if (path.len() == 1 && BUILTIN_ATTRIBUTES.contains(&simple.as_str()))
                        || path
                            .first()
                            .is_some_and(|p| p == "rustfmt" || p == "clippy" || p == "rustdoc")
                    {
                        None
                    } else {
                        self.resolve_path(scope, &path, Ns::Macro, &ctx)
                            .and_then(|r| r.item())
                    };
                if let Some(target) = resolved {
                    annotation.resolved_path = Some(self.graph.items[target].name.clone());
                    targets.push((target, line, is_derive));
                }
                annotations.push(annotation);
            }
        }
        (annotations, targets)
    }

    fn apply_item_annotations(&mut self, item: ItemId, attrs: &[syn::Attribute], scope: ScopeId) {
        let (annotations, targets) = self.resolved_annotations(attrs, scope);
        if annotations.iter().any(|a| a.path() == "non_exhaustive") {
            self.graph.items[item]
                .modifiers
                .push(RustModifier::NonExhaustive);
        }
        self.graph.items[item].annotations = annotations;
        for (target, line, is_derive) in targets {
            self.annotation_targets
                .push((Origin::Item(item), target, line));
            if is_derive && self.graph.items[target].kind != ItemKind::Macro {
                let traits = &mut self.graph.items[item].implemented_traits;
                if !traits.contains(&target) {
                    traits.push(target);
                }
            }
        }
    }

    fn apply_member_annotations(
        &mut self,
        member: MemberId,
        attrs: &[syn::Attribute],
        scope: ScopeId,
    ) {
        let (annotations, targets) = self.resolved_annotations(attrs, scope);
        self.graph.members[member].annotations = annotations;
        for (target, line, _) in targets {
            self.annotation_targets
                .push((Origin::Member(member), target, line));
        }
    }

    // ---- signatures ----------------------------------------------------------------------

    fn generics_of(
        &mut self,
        generics: &syn::Generics,
        scope: ScopeId,
        ctx: &ResolveCtx,
    ) -> (Vec<TypeParameter>, Vec<String>) {
        let names: Vec<String> = generics
            .params
            .iter()
            .filter_map(|p| match p {
                syn::GenericParam::Type(t) => Some(t.ident.to_string()),
                _ => None,
            })
            .collect();
        let mut inner_ctx = ctx.clone();
        inner_ctx.generics.extend(names.iter().cloned());
        let mut params: Vec<TypeParameter> = names
            .iter()
            .map(|n| TypeParameter {
                name: n.clone(),
                bounds: Vec::new(),
            })
            .collect();
        for p in &generics.params {
            if let syn::GenericParam::Type(t) = p {
                let bounds = self.bounds_to_types(&t.bounds, scope, &inner_ctx);
                if let Some(param) = params.iter_mut().find(|x| t.ident == x.name) {
                    param.bounds.extend(bounds);
                }
            }
        }
        if let Some(where_clause) = &generics.where_clause {
            for predicate in &where_clause.predicates {
                if let syn::WherePredicate::Type(pt) = predicate {
                    let bounds = self.bounds_to_types(&pt.bounds, scope, &inner_ctx);
                    let bounded = tokens_to_string(&pt.bounded_ty);
                    if let Some(param) = params.iter_mut().find(|x| x.name == bounded) {
                        param.bounds.extend(bounds);
                    } else if let Some(first) = params.first_mut() {
                        first.bounds.extend(bounds);
                    }
                }
            }
        }
        (params, names)
    }

    fn bounds_to_types(
        &mut self,
        bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>,
        scope: ScopeId,
        ctx: &ResolveCtx,
    ) -> Vec<RustType> {
        bounds
            .iter()
            .filter_map(|b| match b {
                syn::TypeParamBound::Trait(t) => Some(self.path_to_type(&t.path, scope, ctx)),
                _ => None,
            })
            .collect()
    }

    fn path_to_type(&mut self, path: &syn::Path, scope: ScopeId, ctx: &ResolveCtx) -> RustType {
        let segments = segments_of(path);
        let written = path_text(path);
        let args = path
            .segments
            .last()
            .map(|seg| self.generic_args_to_types(&seg.arguments, scope, ctx))
            .unwrap_or_default();
        if segments.len() == 1 && ctx.generics.contains(&segments[0]) {
            return RustType::TypeParam(segments[0].clone());
        }
        match self.resolve_path(scope, &segments, Ns::Type, ctx) {
            Some(Resolved::Item(item)) => RustType::Path {
                item,
                written,
                args,
            },
            Some(Resolved::Assoc { base, .. }) => RustType::Path {
                item: base,
                written,
                args,
            },
            Some(Resolved::Generic(name)) => RustType::TypeParam(name),
            Some(Resolved::Variant { enum_, .. }) => RustType::Path {
                item: enum_,
                written,
                args,
            },
            None => {
                let joined = segments
                    .iter()
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("::");
                let item = self
                    .graph
                    .intern_stub(&format!("<unresolved>::{joined}"), ItemKind::Unknown);
                RustType::Path {
                    item,
                    written,
                    args,
                }
            }
        }
    }

    fn generic_args_to_types(
        &mut self,
        args: &syn::PathArguments,
        scope: ScopeId,
        ctx: &ResolveCtx,
    ) -> Vec<RustType> {
        match args {
            syn::PathArguments::None => Vec::new(),
            syn::PathArguments::AngleBracketed(a) => a
                .args
                .iter()
                .filter_map(|arg| match arg {
                    syn::GenericArgument::Type(t) => Some(self.convert_type(t, scope, ctx)),
                    syn::GenericArgument::AssocType(at) => {
                        Some(self.convert_type(&at.ty, scope, ctx))
                    }
                    _ => None,
                })
                .collect(),
            syn::PathArguments::Parenthesized(p) => {
                let mut types: Vec<RustType> = p
                    .inputs
                    .iter()
                    .map(|t| self.convert_type(t, scope, ctx))
                    .collect();
                if let syn::ReturnType::Type(_, t) = &p.output {
                    types.push(self.convert_type(t, scope, ctx));
                }
                types
            }
        }
    }

    pub(crate) fn convert_type(
        &mut self,
        ty: &syn::Type,
        scope: ScopeId,
        ctx: &ResolveCtx,
    ) -> RustType {
        match ty {
            syn::Type::Path(tp) => {
                if let Some(qself) = &tp.qself {
                    let base = self.convert_type(&qself.ty, scope, ctx);
                    return match base {
                        RustType::Path { item, .. } => RustType::Path {
                            item,
                            written: tokens_to_string(tp),
                            args: Vec::new(),
                        },
                        other => other,
                    };
                }
                self.path_to_type(&tp.path, scope, ctx)
            }
            syn::Type::Reference(r) => RustType::Reference {
                mutable: r.mutability.is_some(),
                inner: Box::new(self.convert_type(&r.elem, scope, ctx)),
            },
            syn::Type::Ptr(p) => RustType::Ptr {
                mutable: p.mutability.is_some(),
                inner: Box::new(self.convert_type(&p.elem, scope, ctx)),
            },
            syn::Type::Slice(s) => {
                RustType::Slice(Box::new(self.convert_type(&s.elem, scope, ctx)))
            }
            syn::Type::Array(a) => {
                RustType::Array(Box::new(self.convert_type(&a.elem, scope, ctx)))
            }
            syn::Type::Tuple(t) => RustType::Tuple(
                t.elems
                    .iter()
                    .map(|e| self.convert_type(e, scope, ctx))
                    .collect(),
            ),
            syn::Type::TraitObject(t) => {
                RustType::TraitObject(self.bounds_to_types(&t.bounds, scope, ctx))
            }
            syn::Type::ImplTrait(t) => {
                RustType::ImplTrait(self.bounds_to_types(&t.bounds, scope, ctx))
            }
            syn::Type::Never(_) => RustType::Never,
            syn::Type::Infer(_) => RustType::Infer,
            syn::Type::BareFn(f) => RustType::FnPointer {
                params: f
                    .inputs
                    .iter()
                    .map(|a| self.convert_type(&a.ty, scope, ctx))
                    .collect(),
                ret: Box::new(match &f.output {
                    syn::ReturnType::Default => RustType::unit(),
                    syn::ReturnType::Type(_, t) => self.convert_type(t, scope, ctx),
                }),
            },
            syn::Type::Paren(p) => self.convert_type(&p.elem, scope, ctx),
            syn::Type::Group(g) => self.convert_type(&g.elem, scope, ctx),
            other => RustType::Unknown(tokens_to_string(other)),
        }
    }

    fn return_type_of(
        &mut self,
        output: &syn::ReturnType,
        scope: ScopeId,
        ctx: &ResolveCtx,
    ) -> Option<RustType> {
        match output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_, t) => Some(self.convert_type(t, scope, ctx)),
        }
    }

    fn error_type_of(&mut self, ret: &RustType, depth: usize) -> Option<RustType> {
        let RustType::Path { item, args, .. } = ret else {
            return None;
        };
        if depth > 3 {
            return None;
        }
        let data = &self.graph.items[*item];
        let name = data.name.clone();
        let simple = data.simple_name.clone();
        if data.kind == ItemKind::TypeAlias {
            if let Some(target) = data.alias_target.clone() {
                return self.error_type_of(&target, depth + 1);
            }
        }
        if simple != "Result" {
            return None;
        }
        if args.len() >= 2 {
            return Some(args[1].clone());
        }
        let error_path = match name.as_str() {
            "std::io::Result" => "std::io::Error",
            "std::fmt::Result" => "std::fmt::Error",
            "anyhow::Result" => "anyhow::Error",
            "eyre::Result" => "eyre::Report",
            _ => return None,
        };
        let error = self.graph.intern_stub(error_path, ItemKind::Unknown);
        Some(RustType::Path {
            item: error,
            written: error_path.to_owned(),
            args: Vec::new(),
        })
    }

    fn is_constructor_signature(
        &self,
        owner: ItemId,
        receiver: Option<Receiver>,
        ret: Option<&RustType>,
    ) -> bool {
        if receiver.is_some() {
            return false;
        }
        let Some(ret) = ret else {
            return false;
        };
        if ret.raw_item() == Some(owner) {
            return true;
        }
        if let RustType::Path { item, args, .. } = ret {
            let wrapper = self.graph.items[*item].simple_name.as_str();
            if matches!(wrapper, "Option" | "Result" | "Box" | "Rc" | "Arc") {
                return args.first().and_then(RustType::raw_item) == Some(owner);
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn add_code_unit(
        &mut self,
        owner: ItemId,
        sig: &syn::Signature,
        attrs: &[syn::Attribute],
        vis: Visibility,
        scope: ScopeId,
        ctx: &ResolveCtx,
        file: &str,
        is_test: bool,
        body: Option<&syn::Block>,
        is_trait_declaration: bool,
        declaring_impl: Option<ItemId>,
        implemented_trait: Option<ItemId>,
        item: Option<ItemId>,
    ) -> MemberId {
        let (type_params, generic_names) = self.generics_of(&sig.generics, scope, ctx);
        let mut inner_ctx = ctx.clone();
        inner_ctx.generics.extend(generic_names);
        let receiver = receiver_of(sig);
        let parameters: Vec<ParameterData> = sig
            .inputs
            .iter()
            .filter_map(|arg| match arg {
                syn::FnArg::Typed(t) => Some(ParameterData {
                    name: pattern_name(&t.pat),
                    type_: self.convert_type(&t.ty, scope, &inner_ctx),
                    annotations: self.plain_annotations(&t.attrs),
                }),
                syn::FnArg::Receiver(_) => None,
            })
            .collect();
        let return_type = self.return_type_of(&sig.output, scope, &inner_ctx);
        let error_type = return_type.as_ref().and_then(|r| self.error_type_of(r, 0));
        let is_constructor = self.is_constructor_signature(owner, receiver, return_type.as_ref());
        let name = sig.ident.to_string();
        let owner_name = match item {
            Some(fn_item) => self.graph.items[fn_item]
                .name
                .rsplit_once("::")
                .map(|(m, _)| m.to_owned())
                .unwrap_or_default(),
            None => self.graph.items[owner].name.clone(),
        };
        let param_text: Vec<String> = parameters.iter().map(|p| p.type_.to_string()).collect();
        let full_name = format!("{owner_name}::{name}({})", param_text.join(", "));
        let mut modifiers = vis.modifiers();
        modifiers.extend(fn_modifiers(sig));
        if receiver.is_none() {
            modifiers.push(RustModifier::Static);
        }
        if is_trait_declaration {
            modifiers.push(if body.is_some() {
                RustModifier::Default
            } else {
                RustModifier::Abstract
            });
        } else if declaring_impl.is_some() && implemented_trait.is_none() {
            modifiers.push(RustModifier::Final);
        }
        let member = self.graph.add_member(MemberData {
            owner,
            kind: MemberKind::Method,
            name,
            full_name,
            visibility: vis,
            modifiers,
            annotations: Vec::new(),
            location: SourceCodeLocation::of(file.to_owned(), line_of(sig.ident.span())),
            is_test_code: is_test,
            type_: None,
            parameters: parameters.clone(),
            return_type,
            error_type,
            receiver,
            is_constructor,
            is_trait_declaration,
            has_body: body.is_some(),
            declaring_impl,
            implemented_trait,
            type_parameters: type_params,
            item,
            unsafe_block_lines: Vec::new(),
        });
        self.apply_member_annotations(member, attrs, scope);
        if let Some(block) = body {
            let self_type = inner_ctx.self_type;
            let mut params: Vec<(String, RustType)> = parameters
                .iter()
                .map(|p| (p.name.clone(), p.type_.clone()))
                .collect();
            if let (Some(self_type), Some(_)) = (self_type, receiver) {
                params.push((
                    "self".to_owned(),
                    RustType::Path {
                        item: self_type,
                        written: "Self".to_owned(),
                        args: Vec::new(),
                    },
                ));
            }
            self.bodies.push(BodyJob {
                member,
                block: block.clone(),
                scope,
                ctx: inner_ctx,
                params,
                file: file.to_owned(),
            });
        }
        member
    }

    fn add_field(
        &mut self,
        owner: ItemId,
        name: String,
        ty: &syn::Type,
        vis: Visibility,
        attrs: &[syn::Attribute],
        scope: ScopeId,
        ctx: &ResolveCtx,
        file: &str,
        line: usize,
        is_test: bool,
        kind: MemberKind,
    ) -> MemberId {
        let type_ = self.convert_type(ty, scope, ctx);
        let full_name = format!("{}::{name}", self.graph.items[owner].name);
        let mut modifiers = vis.modifiers();
        if kind == MemberKind::AssocConst {
            modifiers.push(RustModifier::Static);
        }
        let member = self.graph.add_member(MemberData {
            owner,
            kind,
            name,
            full_name,
            visibility: vis,
            modifiers,
            annotations: Vec::new(),
            location: SourceCodeLocation::of(file.to_owned(), line),
            is_test_code: is_test,
            type_: Some(type_),
            parameters: Vec::new(),
            return_type: None,
            error_type: None,
            receiver: None,
            is_constructor: false,
            is_trait_declaration: false,
            has_body: false,
            declaring_impl: None,
            implemented_trait: None,
            type_parameters: Vec::new(),
            item: None,
            unsafe_block_lines: Vec::new(),
        });
        self.apply_member_annotations(member, attrs, scope);
        member
    }

    fn add_fields(
        &mut self,
        owner: ItemId,
        fields: &syn::Fields,
        prefix: &str,
        scope: ScopeId,
        ctx: &ResolveCtx,
        file: &str,
        is_test: bool,
    ) {
        for (index, field) in fields.iter().enumerate() {
            let name = match &field.ident {
                Some(ident) => ident.to_string(),
                None => index.to_string(),
            };
            let name = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}::{name}")
            };
            let line = field
                .ident
                .as_ref()
                .map(|i| line_of(i.span()))
                .unwrap_or_else(|| line_of(field.ty.span()));
            let vis = visibility_of(&field.vis);
            self.add_field(
                owner,
                name,
                &field.ty,
                vis,
                &field.attrs,
                scope,
                ctx,
                file,
                line,
                is_test,
                MemberKind::Field,
            );
        }
    }

    fn resolve_signatures(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        for p in &pending {
            self.resolve_signature(p);
        }
        self.pending = pending;
    }

    fn resolve_signature(&mut self, p: &Pending) {
        let id = p.item;
        let scope = p.scope;
        let file = &p.file;
        let is_test = p.is_test;
        let base_ctx = ResolveCtx {
            self_type: Some(id),
            generics: Vec::new(),
        };
        match &p.syn_item {
            syn::Item::Verbatim(_) => {}
            syn::Item::Struct(s) => {
                self.apply_item_annotations(id, &s.attrs, scope);
                let (params, names) = self.generics_of(&s.generics, scope, &base_ctx);
                self.graph.items[id].type_parameters = params;
                let ctx = ResolveCtx {
                    self_type: Some(id),
                    generics: names,
                };
                self.add_fields(id, &s.fields, "", scope, &ctx, file, is_test);
            }
            syn::Item::Union(u) => {
                self.apply_item_annotations(id, &u.attrs, scope);
                let (params, names) = self.generics_of(&u.generics, scope, &base_ctx);
                self.graph.items[id].type_parameters = params;
                let ctx = ResolveCtx {
                    self_type: Some(id),
                    generics: names,
                };
                let fields = syn::Fields::Named(u.fields.clone());
                self.add_fields(id, &fields, "", scope, &ctx, file, is_test);
            }
            syn::Item::Enum(e) => {
                self.apply_item_annotations(id, &e.attrs, scope);
                let (params, names) = self.generics_of(&e.generics, scope, &base_ctx);
                self.graph.items[id].type_parameters = params;
                let ctx = ResolveCtx {
                    self_type: Some(id),
                    generics: names,
                };
                for variant in &e.variants {
                    let name = variant.ident.to_string();
                    let full_name = format!("{}::{name}", self.graph.items[id].name);
                    let member = self.graph.add_member(MemberData {
                        owner: id,
                        kind: MemberKind::Variant,
                        name: name.clone(),
                        full_name,
                        visibility: Visibility::Pub,
                        modifiers: vec![RustModifier::Pub, RustModifier::Static],
                        annotations: Vec::new(),
                        location: SourceCodeLocation::of(
                            file.clone(),
                            line_of(variant.ident.span()),
                        ),
                        is_test_code: is_test,
                        type_: None,
                        parameters: Vec::new(),
                        return_type: None,
                        error_type: None,
                        receiver: None,
                        is_constructor: false,
                        is_trait_declaration: false,
                        has_body: false,
                        declaring_impl: None,
                        implemented_trait: None,
                        type_parameters: Vec::new(),
                        item: None,
                        unsafe_block_lines: Vec::new(),
                    });
                    self.apply_member_annotations(member, &variant.attrs, scope);
                    self.add_fields(id, &variant.fields, &name, scope, &ctx, file, is_test);
                }
            }
            syn::Item::Trait(t) => {
                self.apply_item_annotations(id, &t.attrs, scope);
                self.graph.items[id].modifiers.push(RustModifier::Abstract);
                let (params, names) = self.generics_of(&t.generics, scope, &base_ctx);
                self.graph.items[id].type_parameters = params;
                let ctx = ResolveCtx {
                    self_type: Some(id),
                    generics: names,
                };
                let supertraits: Vec<ItemId> = self
                    .bounds_to_types(&t.supertraits, scope, &ctx)
                    .iter()
                    .filter_map(RustType::raw_item)
                    .collect();
                self.graph.items[id].supertraits = supertraits;
                for item in &t.items {
                    match item {
                        syn::TraitItem::Fn(f) => {
                            self.add_code_unit(
                                id,
                                &f.sig,
                                &f.attrs,
                                Visibility::Pub,
                                scope,
                                &ctx,
                                file,
                                is_test,
                                f.default.as_ref(),
                                true,
                                None,
                                None,
                                None,
                            );
                        }
                        syn::TraitItem::Const(c) => {
                            self.add_field(
                                id,
                                c.ident.to_string(),
                                &c.ty,
                                Visibility::Pub,
                                &c.attrs,
                                scope,
                                &ctx,
                                file,
                                line_of(c.ident.span()),
                                is_test,
                                MemberKind::AssocConst,
                            );
                        }
                        syn::TraitItem::Type(ty) => {
                            let full_name = format!("{}::{}", self.graph.items[id].name, ty.ident);
                            self.graph.add_member(MemberData {
                                owner: id,
                                kind: MemberKind::AssocType,
                                name: ty.ident.to_string(),
                                full_name,
                                visibility: Visibility::Pub,
                                modifiers: vec![RustModifier::Pub],
                                annotations: Vec::new(),
                                location: SourceCodeLocation::of(
                                    file.clone(),
                                    line_of(ty.ident.span()),
                                ),
                                is_test_code: is_test,
                                type_: None,
                                parameters: Vec::new(),
                                return_type: None,
                                error_type: None,
                                receiver: None,
                                is_constructor: false,
                                is_trait_declaration: true,
                                has_body: false,
                                declaring_impl: None,
                                implemented_trait: None,
                                type_parameters: Vec::new(),
                                item: None,
                                unsafe_block_lines: Vec::new(),
                            });
                        }
                        _ => {}
                    }
                }
            }
            syn::Item::TraitAlias(t) => {
                self.apply_item_annotations(id, &t.attrs, scope);
                let supertraits: Vec<ItemId> = self
                    .bounds_to_types(&t.bounds, scope, &base_ctx)
                    .iter()
                    .filter_map(RustType::raw_item)
                    .collect();
                self.graph.items[id].supertraits = supertraits;
            }
            syn::Item::Type(t) => {
                self.apply_item_annotations(id, &t.attrs, scope);
                let (params, names) = self.generics_of(&t.generics, scope, &base_ctx);
                self.graph.items[id].type_parameters = params;
                let ctx = ResolveCtx {
                    self_type: None,
                    generics: names,
                };
                let target = self.convert_type(&t.ty, scope, &ctx);
                self.graph.items[id].alias_target = Some(target);
            }
            syn::Item::Const(c) => {
                self.apply_item_annotations(id, &c.attrs, scope);
                let ty = self.convert_type(&c.ty, scope, &base_ctx);
                self.graph.items[id].item_type = Some(ty);
                self.graph.items[id].modifiers.push(RustModifier::Static);
            }
            syn::Item::Static(s) => {
                self.apply_item_annotations(id, &s.attrs, scope);
                let ty = self.convert_type(&s.ty, scope, &base_ctx);
                self.graph.items[id].item_type = Some(ty);
                self.graph.items[id].modifiers.push(RustModifier::Static);
                if matches!(s.mutability, syn::StaticMutability::Mut(_)) {
                    self.graph.items[id].modifiers.push(RustModifier::Mut);
                }
            }
            syn::Item::Fn(f) => {
                self.apply_item_annotations(id, &f.attrs, scope);
                let vis = self.graph.items[id].visibility.clone();
                let ctx = ResolveCtx {
                    self_type: None,
                    generics: Vec::new(),
                };
                let member = self.add_code_unit(
                    id,
                    &f.sig,
                    &f.attrs,
                    vis,
                    scope,
                    &ctx,
                    file,
                    is_test,
                    Some(&f.block),
                    false,
                    None,
                    None,
                    Some(id),
                );
                let mods = fn_modifiers(&f.sig);
                self.graph.items[id].modifiers.extend(mods);
                self.graph.items[id].modifiers.push(RustModifier::Static);
                self.graph.items[id].type_parameters =
                    self.graph.members[member].type_parameters.clone();
                let return_type = self.graph.members[member].return_type.clone();
                self.graph.items[id].item_type = return_type;
                if f.block.stmts.is_empty() && f.sig.abi.is_none() {
                    // still a normal function with an empty body
                }
            }
            syn::Item::Macro(m) => {
                self.apply_item_annotations(id, &m.attrs, scope);
            }
            syn::Item::Impl(i) => {
                self.apply_item_annotations(id, &i.attrs, scope);
                let (params, names) = self.generics_of(&i.generics, scope, &ResolveCtx::default());
                self.graph.items[id].type_parameters = params;
                let pre_ctx = ResolveCtx {
                    self_type: None,
                    generics: names.clone(),
                };
                let self_type = self.convert_type(&i.self_ty, scope, &pre_ctx);
                let self_item = self_type.raw_item();
                let trait_ = i.trait_.as_ref().map(|(_, path, _)| {
                    let ty = self.path_to_type(path, scope, &pre_ctx);
                    ty.raw_item().unwrap_or_else(|| {
                        self.graph.intern_stub(
                            &format!("<unresolved>::{}", path_text(path)),
                            ItemKind::Trait,
                        )
                    })
                });
                self.graph.items[id].impl_info = Some(ImplData {
                    self_type: self_type.clone(),
                    trait_,
                });
                let owner = match self_item {
                    Some(item) if self.graph.items[item].fully_imported => item,
                    _ => id,
                };
                if let (Some(trait_), Some(item)) = (trait_, self_item) {
                    let traits = &mut self.graph.items[item].implemented_traits;
                    if !traits.contains(&trait_) {
                        traits.push(trait_);
                    }
                    if item != owner {
                        let traits = &mut self.graph.items[owner].implemented_traits;
                        if !traits.contains(&trait_) {
                            traits.push(trait_);
                        }
                    }
                }
                let ctx = ResolveCtx {
                    self_type: self_item.or(Some(id)),
                    generics: names,
                };
                for impl_item in &i.items {
                    match impl_item {
                        syn::ImplItem::Fn(f) => {
                            let vis = if trait_.is_some() {
                                Visibility::Pub
                            } else {
                                visibility_of(&f.vis)
                            };
                            self.add_code_unit(
                                owner,
                                &f.sig,
                                &f.attrs,
                                vis,
                                scope,
                                &ctx,
                                file,
                                is_test,
                                Some(&f.block),
                                false,
                                Some(id),
                                trait_,
                                None,
                            );
                        }
                        syn::ImplItem::Const(c) => {
                            let vis = if trait_.is_some() {
                                Visibility::Pub
                            } else {
                                visibility_of(&c.vis)
                            };
                            let member = self.add_field(
                                owner,
                                c.ident.to_string(),
                                &c.ty,
                                vis,
                                &c.attrs,
                                scope,
                                &ctx,
                                file,
                                line_of(c.ident.span()),
                                is_test,
                                MemberKind::AssocConst,
                            );
                            self.graph.members[member].declaring_impl = Some(id);
                            self.graph.members[member].implemented_trait = trait_;
                        }
                        syn::ImplItem::Type(ty) => {
                            let target = self.convert_type(&ty.ty, scope, &ctx);
                            let full_name =
                                format!("{}::{}", self.graph.items[owner].name, ty.ident);
                            self.graph.add_member(MemberData {
                                owner,
                                kind: MemberKind::AssocType,
                                name: ty.ident.to_string(),
                                full_name,
                                visibility: Visibility::Pub,
                                modifiers: vec![RustModifier::Pub],
                                annotations: Vec::new(),
                                location: SourceCodeLocation::of(
                                    file.clone(),
                                    line_of(ty.ident.span()),
                                ),
                                is_test_code: is_test,
                                type_: Some(target),
                                parameters: Vec::new(),
                                return_type: None,
                                error_type: None,
                                receiver: None,
                                is_constructor: false,
                                is_trait_declaration: false,
                                has_body: false,
                                declaring_impl: Some(id),
                                implemented_trait: trait_,
                                type_parameters: Vec::new(),
                                item: None,
                                unsafe_block_lines: Vec::new(),
                            });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // ---- bodies ----------------------------------------------------------------------------

    fn collect_bodies(&mut self) {
        let bodies = std::mem::take(&mut self.bodies);
        for job in bodies {
            let mut visitor = BodyVisitor {
                b: self,
                member: job.member,
                scope: job.scope,
                ctx: job.ctx.clone(),
                file: job.file.clone(),
                locals: vec![job.params.iter().cloned().collect()],
                closure_depth: 0,
                assigning: false,
            };
            visitor.visit_block(&job.block);
        }
        let invocations = std::mem::take(&mut self.module_macro_invocations);
        for (module, mac, scope, file) in invocations {
            let target = self.resolve_macro_target(&mac.mac.path, scope);
            if let Some(target) = target {
                let line = line_of(
                    mac.mac
                        .path
                        .segments
                        .last()
                        .map(|s| s.ident.span())
                        .unwrap_or_else(Span::call_site),
                );
                let location = SourceCodeLocation::of(file, line);
                let description = format!(
                    "{} invokes macro <{}> in {location}",
                    item_description(&self.graph, module),
                    self.graph.items[target].name
                );
                self.graph.dependencies.push(DependencyData {
                    origin: module,
                    target,
                    kind: DependencyKind::MacroInvocation,
                    description,
                    location,
                    macro_argument_count: None,
                });
            }
        }
    }

    fn resolve_macro_target(&mut self, path: &syn::Path, scope: ScopeId) -> Option<ItemId> {
        let segments = segments_of(path);
        match self.resolve_path(scope, &segments, Ns::Macro, &ResolveCtx::default()) {
            Some(r) => r.item(),
            None => {
                let joined = segments
                    .iter()
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("::");
                Some(
                    self.graph
                        .intern_stub(&format!("<unresolved>::{joined}"), ItemKind::Macro),
                )
            }
        }
    }

    pub(crate) fn find_method(&self, type_id: ItemId, name: &str) -> Option<MemberId> {
        let mut visited = HashSet::new();
        self.find_method_in(type_id, name, &mut visited)
    }

    fn find_method_in(
        &self,
        type_id: ItemId,
        name: &str,
        visited: &mut HashSet<ItemId>,
    ) -> Option<MemberId> {
        if !visited.insert(type_id) {
            return None;
        }
        let data = &self.graph.items[type_id];
        if let Some(&m) = data.members.iter().find(|&&m| {
            let member = &self.graph.members[m];
            member.kind == MemberKind::Method && member.name == name
        }) {
            return Some(m);
        }
        for &trait_ in data
            .implemented_traits
            .iter()
            .chain(data.supertraits.iter())
        {
            if let Some(m) = self.find_method_in(trait_, name, visited) {
                return Some(m);
            }
        }
        None
    }

    pub(crate) fn find_field(&self, type_id: ItemId, name: &str) -> Option<MemberId> {
        self.graph.items[type_id]
            .members
            .iter()
            .copied()
            .find(|&m| {
                let member = &self.graph.members[m];
                matches!(member.kind, MemberKind::Field | MemberKind::AssocConst)
                    && member.name == name
            })
    }

    fn unique_member_by_name(
        &self,
        name: &str,
        kind: MemberKind,
        needs_receiver: bool,
    ) -> Option<MemberId> {
        if needs_receiver && STD_TRAIT_METHOD_NAMES.contains(&name) {
            // `iter.next()`, `x.clone()`, ...: almost always a std trait method on a type the
            // import cannot see, so a unique local method of the same name is a false match.
            return None;
        }
        let mut found = None;
        for (id, member) in self.graph.members.iter().enumerate() {
            if member.kind == kind
                && member.name == name
                && (!needs_receiver || member.receiver.is_some())
            {
                if found.is_some() {
                    return None;
                }
                found = Some(id);
            }
        }
        found
    }

    // ---- dependencies ----------------------------------------------------------------------

    fn add_dependency(
        &mut self,
        origin: ItemId,
        target: ItemId,
        kind: DependencyKind,
        description: String,
        location: SourceCodeLocation,
    ) {
        if origin == target || self.graph.items[target].is_primitive {
            return;
        }
        self.graph.dependencies.push(DependencyData {
            origin,
            target,
            kind,
            description,
            location,
            macro_argument_count: None,
        });
    }

    fn origin_item(&self, origin: Origin) -> ItemId {
        match origin {
            Origin::Item(id) => id,
            Origin::Member(m) => self.graph.members[m].owner,
        }
    }

    fn origin_description(&self, origin: Origin) -> String {
        match origin {
            Origin::Item(id) => item_description(&self.graph, id),
            Origin::Member(m) => member_description(&self.graph, m),
        }
    }

    /// Records dependencies for a type in a signature: the raw type with `verb`, nested type
    /// arguments with the generic wording.
    fn type_dependencies(
        &mut self,
        origin: Origin,
        ty: &RustType,
        verb: &str,
        generic_kind: &str,
        kind: DependencyKind,
        location: &SourceCodeLocation,
    ) {
        let origin_item = self.origin_item(origin);
        let origin_desc = self.origin_description(origin);
        let raw = ty.raw_item();
        if let Some(target) = raw {
            let description = format!(
                "{origin_desc} {verb} <{}> in {location}",
                self.graph.items[target].name
            );
            self.add_dependency(origin_item, target, kind, description, location.clone());
        }
        let outer_name = raw.map(|r| self.graph.items[r].name.clone());
        for item in ty.all_items() {
            if Some(item) == raw {
                continue;
            }
            let description = match &outer_name {
                Some(outer) => format!(
                    "{origin_desc} has generic {generic_kind} <{outer}> with type argument depending on <{}> in {location}",
                    self.graph.items[item].name
                ),
                None => format!(
                    "{origin_desc} {verb} <{}> in {location}",
                    self.graph.items[item].name
                ),
            };
            self.add_dependency(
                origin_item,
                item,
                DependencyKind::Generic,
                description,
                location.clone(),
            );
        }
    }

    fn type_parameter_dependencies(
        &mut self,
        origin: Origin,
        params: &[TypeParameter],
        location: &SourceCodeLocation,
    ) {
        let origin_item = self.origin_item(origin);
        let origin_desc = self.origin_description(origin);
        for param in params {
            for bound in &param.bounds {
                for item in bound.all_items() {
                    let description = format!(
                        "{origin_desc} has type parameter '{}' depending on <{}> in {location}",
                        param.name, self.graph.items[item].name
                    );
                    self.add_dependency(
                        origin_item,
                        item,
                        DependencyKind::Generic,
                        description,
                        location.clone(),
                    );
                }
            }
        }
    }

    fn build_dependencies(&mut self) {
        // Imports.
        let imports = std::mem::take(&mut self.imports);
        for (module, target, line) in imports {
            let location = self.graph.items[module].location.with_line(line);
            let description = format!(
                "{} imports <{}> in {location}",
                item_description(&self.graph, module),
                self.graph.items[target].name
            );
            self.add_dependency(
                module,
                target,
                DependencyKind::Import,
                description,
                location,
            );
        }
        // Annotations.
        let annotation_targets = std::mem::take(&mut self.annotation_targets);
        for (origin, target, line) in annotation_targets {
            let location = match origin {
                Origin::Item(id) => self.graph.items[id].location.with_line(line),
                Origin::Member(m) => self.graph.members[m].location.with_line(line),
            };
            let description = format!(
                "{} is annotated with <{}> in {location}",
                self.origin_description(origin),
                self.graph.items[target].name
            );
            let origin_item = self.origin_item(origin);
            self.add_dependency(
                origin_item,
                target,
                DependencyKind::Annotation,
                description,
                location,
            );
        }
        // Item level: supertraits, implemented traits, type parameters, alias targets, item types.
        for id in 0..self.graph.items.len() {
            if !self.graph.items[id].fully_imported {
                continue;
            }
            let location = self.graph.items[id].location.clone();
            let desc = item_description(&self.graph, id);
            for supertrait in self.graph.items[id].supertraits.clone() {
                let description = format!(
                    "{desc} extends trait <{}> in {location}",
                    self.graph.items[supertrait].name
                );
                self.add_dependency(
                    id,
                    supertrait,
                    DependencyKind::Extends,
                    description,
                    location.clone(),
                );
            }
            for trait_ in self.graph.items[id].implemented_traits.clone() {
                let description = format!(
                    "{desc} implements trait <{}> in {location}",
                    self.graph.items[trait_].name
                );
                self.add_dependency(
                    id,
                    trait_,
                    DependencyKind::Implements,
                    description,
                    location.clone(),
                );
            }
            // A folded impl block (of an imported type) contributes to its self type; the
            // impl item itself never appears as an origin.
            let folded_into =
                if self.graph.items[id].kind == ItemKind::Impl && !self.graph.is_class_like(id) {
                    self.graph.items[id]
                        .impl_info
                        .as_ref()
                        .and_then(|i| i.self_type.raw_item())
                } else {
                    None
                };
            let params = self.graph.items[id].type_parameters.clone();
            self.type_parameter_dependencies(
                Origin::Item(folded_into.unwrap_or(id)),
                &params,
                &location,
            );
            if let Some(alias) = self.graph.items[id].alias_target.clone() {
                self.type_dependencies(
                    Origin::Item(id),
                    &alias,
                    "has type",
                    "type",
                    DependencyKind::Alias,
                    &location,
                );
            }
            if let Some(ty) = self.graph.items[id].item_type.clone() {
                if matches!(
                    self.graph.items[id].kind,
                    ItemKind::Const | ItemKind::Static
                ) {
                    self.type_dependencies(
                        Origin::Item(id),
                        &ty,
                        "has type",
                        "type",
                        DependencyKind::FieldType,
                        &location,
                    );
                }
            }
            if let Some(impl_info) = self.graph.items[id].impl_info.clone() {
                if self.graph.items[id].kind == ItemKind::Impl && folded_into.is_none() {
                    self.type_dependencies(
                        Origin::Item(id),
                        &impl_info.self_type,
                        "has type",
                        "type",
                        DependencyKind::FieldType,
                        &location,
                    );
                    if let Some(trait_) = impl_info.trait_ {
                        let description = format!(
                            "{desc} implements trait <{}> in {location}",
                            self.graph.items[trait_].name
                        );
                        self.add_dependency(
                            id,
                            trait_,
                            DependencyKind::Implements,
                            description,
                            location.clone(),
                        );
                    }
                }
            }
        }
        // Members: field types, parameters, return and error types, type parameters.
        for m in 0..self.graph.members.len() {
            let member = &self.graph.members[m];
            let location = member.location.clone();
            let kind = member.kind;
            let type_ = member.type_.clone();
            let params: Vec<RustType> = member.parameters.iter().map(|p| p.type_.clone()).collect();
            let return_type = member.return_type.clone();
            let error_type = member.error_type.clone();
            let type_params = member.type_parameters.clone();
            match kind {
                MemberKind::Field | MemberKind::AssocConst | MemberKind::AssocType => {
                    if let Some(ty) = type_ {
                        self.type_dependencies(
                            Origin::Member(m),
                            &ty,
                            "has type",
                            "field type",
                            DependencyKind::FieldType,
                            &location,
                        );
                    }
                }
                MemberKind::Method => {
                    for p in &params {
                        self.type_dependencies(
                            Origin::Member(m),
                            p,
                            "has parameter of type",
                            "parameter type",
                            DependencyKind::ParameterType,
                            &location,
                        );
                    }
                    if let Some(ret) = &return_type {
                        self.type_dependencies(
                            Origin::Member(m),
                            ret,
                            "has return type",
                            "return type",
                            DependencyKind::ReturnType,
                            &location,
                        );
                    }
                    if let Some(err) = &error_type {
                        if let Some(target) = err.raw_item() {
                            let description = format!(
                                "{} throws type <{}> in {location}",
                                member_description(&self.graph, m),
                                self.graph.items[target].name
                            );
                            let origin = self.graph.members[m].owner;
                            self.add_dependency(
                                origin,
                                target,
                                DependencyKind::ErrorType,
                                description,
                                location.clone(),
                            );
                        }
                    }
                    self.type_parameter_dependencies(Origin::Member(m), &type_params, &location);
                }
                MemberKind::Variant => {}
            }
        }
        // Accesses.
        for a in 0..self.graph.accesses.len() {
            let access = &self.graph.accesses[a];
            let origin = self.graph.members[access.origin].owner;
            let target = access.target_owner;
            let verb = match access.kind {
                AccessKind::FieldAccess(AccessType::Get) => "gets field",
                AccessKind::FieldAccess(AccessType::Set) => "sets field",
                AccessKind::MethodCall => "calls method",
                AccessKind::ConstructorCall => "calls constructor",
                AccessKind::FunctionReference => {
                    if access.resolved.len() == 1
                        && self.graph.members[access.resolved[0]].is_constructor
                    {
                        "references constructor"
                    } else {
                        "references method"
                    }
                }
            };
            let description = format!(
                "{} {verb} <{}> in {}",
                member_description(&self.graph, access.origin),
                access.target_full_name,
                access.location
            );
            let location = access.location.clone();
            self.add_dependency(
                origin,
                target,
                DependencyKind::Access,
                description,
                location,
            );
        }
        // References from bodies.
        let references = std::mem::take(&mut self.references);
        for r in references {
            let origin = self.graph.members[r.origin_member].owner;
            let location = self.graph.members[r.origin_member]
                .location
                .with_line(r.line);
            let verb = match r.kind {
                DependencyKind::MacroInvocation => "invokes macro",
                DependencyKind::TraitReference => "references trait",
                _ => "references",
            };
            let description = format!(
                "{} {verb} <{}> in {location}",
                member_description(&self.graph, r.origin_member),
                self.graph.items[r.target].name
            );
            let before = self.graph.dependencies.len();
            self.add_dependency(origin, r.target, r.kind, description, location);
            if self.graph.dependencies.len() > before {
                if let Some(added) = self.graph.dependencies.last_mut() {
                    added.macro_argument_count = r.macro_argument_count;
                }
            }
        }
    }
}

fn collect_local_items(block: &syn::Block, out: &mut Vec<syn::Item>) {
    struct Collector<'a> {
        out: &'a mut Vec<syn::Item>,
    }
    impl<'ast> Visit<'ast> for Collector<'_> {
        fn visit_item(&mut self, item: &'ast syn::Item) {
            if !matches!(item, syn::Item::Use(_)) {
                self.out.push(item.clone());
            }
            // Do not descend: nested local items are declared when their parent is processed.
        }
    }
    Collector { out }.visit_block(block);
}

pub(crate) fn item_description(graph: &Graph, id: ItemId) -> String {
    let data = &graph.items[id];
    let prefix = match data.kind {
        ItemKind::Module => "Module",
        _ => "Item",
    };
    format!("{prefix} <{}>", data.name)
}

pub(crate) fn member_description(graph: &Graph, id: MemberId) -> String {
    let member = &graph.members[id];
    let prefix = match member.kind {
        MemberKind::Field | MemberKind::AssocConst => "Field",
        MemberKind::Variant => "Variant",
        MemberKind::AssocType => "Type",
        MemberKind::Method if member.is_constructor => "Constructor",
        MemberKind::Method if member.item.is_some() => "Function",
        MemberKind::Method => "Method",
    };
    format!("{prefix} <{}>", member.full_name)
}

/// Splits an attribute into annotations (one per derive entry) plus the path segments to
/// resolve.
fn annotations_of_attr(attr: &syn::Attribute) -> Vec<(RustAnnotation, Vec<String>)> {
    let path = path_text(attr.path());
    if attr.path().is_ident("derive") {
        let mut out = Vec::new();
        if let syn::Meta::List(list) = &attr.meta {
            let parsed = list.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            );
            if let Ok(paths) = parsed {
                for p in paths {
                    out.push((
                        RustAnnotation {
                            path: path_text(&p),
                            resolved_path: None,
                            kind: AnnotationKind::Derive,
                            properties: Vec::new(),
                            tokens: String::new(),
                        },
                        segments_of(&p),
                    ));
                }
            }
        }
        return out;
    }
    let (properties, tokens) = match &attr.meta {
        syn::Meta::Path(_) => (Vec::new(), String::new()),
        syn::Meta::List(list) => {
            let tokens = format!("({})", tokens_to_string(&list.tokens));
            let props = list
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                )
                .map(|metas| metas.iter().map(meta_property).collect::<Vec<_>>())
                .or_else(|_| {
                    list.parse_args::<syn::Lit>()
                        .map(|lit| vec![("value".to_owned(), lit_value(&lit))])
                })
                .unwrap_or_else(|_| {
                    vec![(
                        String::new(),
                        AnnotationValue::Tokens(tokens_to_string(&list.tokens)),
                    )]
                });
            (props, tokens)
        }
        syn::Meta::NameValue(nv) => {
            let value = expr_value(&nv.value);
            (
                vec![("value".to_owned(), value)],
                format!("= {}", tokens_to_string(&nv.value)),
            )
        }
    };
    vec![(
        RustAnnotation {
            path,
            resolved_path: None,
            kind: AnnotationKind::Attribute,
            properties,
            tokens,
        },
        segments_of(attr.path()),
    )]
}

fn meta_property(meta: &syn::Meta) -> (String, AnnotationValue) {
    match meta {
        syn::Meta::Path(p) => (String::new(), AnnotationValue::Path(path_text(p))),
        syn::Meta::NameValue(nv) => (path_text(&nv.path), expr_value(&nv.value)),
        syn::Meta::List(list) => (
            path_text(&list.path),
            AnnotationValue::Tokens(tokens_to_string(&list.tokens)),
        ),
    }
}

fn expr_value(expr: &syn::Expr) -> AnnotationValue {
    match expr {
        syn::Expr::Lit(lit) => lit_value(&lit.lit),
        syn::Expr::Path(p) => AnnotationValue::Path(path_text(&p.path)),
        other => AnnotationValue::Tokens(tokens_to_string(other)),
    }
}

fn lit_value(lit: &syn::Lit) -> AnnotationValue {
    match lit {
        syn::Lit::Str(s) => AnnotationValue::Str(s.value()),
        syn::Lit::Int(i) => i
            .base10_parse::<i128>()
            .map(AnnotationValue::Int)
            .unwrap_or_else(|_| AnnotationValue::Tokens(i.to_string())),
        syn::Lit::Bool(b) => AnnotationValue::Bool(b.value),
        other => AnnotationValue::Tokens(tokens_to_string(other)),
    }
}

// ---- body visitor ---------------------------------------------------------------------------

struct BodyVisitor<'b, 'a> {
    b: &'b mut Builder<'a>,
    member: MemberId,
    scope: ScopeId,
    ctx: ResolveCtx,
    file: String,
    locals: Vec<HashMap<String, RustType>>,
    closure_depth: usize,
    assigning: bool,
}

enum Callee {
    Function(ItemId),
    Struct(ItemId),
    Variant(ItemId, String),
    Assoc(ItemId, String),
    None,
}

impl BodyVisitor<'_, '_> {
    fn location(&self, span: Span) -> SourceCodeLocation {
        SourceCodeLocation::of(self.file.clone(), line_of(span))
    }

    fn local_type(&self, name: &str) -> Option<RustType> {
        self.locals
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn bind_local(&mut self, name: String, ty: Option<RustType>) {
        if let Some(ty) = ty {
            if let Some(scope) = self.locals.last_mut() {
                scope.insert(name, ty);
            }
        }
    }

    fn record_access(
        &mut self,
        kind: AccessKind,
        target_owner: ItemId,
        target_name: String,
        target_full_name: String,
        resolved: Vec<MemberId>,
        resolution: ResolutionKind,
        span: Span,
    ) {
        let location = self.location(span);
        self.b.graph.accesses.push(AccessData {
            origin: self.member,
            kind,
            target_owner,
            target_name,
            target_full_name,
            resolved,
            resolution,
            location,
            in_closure: self.closure_depth > 0,
        });
    }

    fn record_reference(&mut self, target: ItemId, kind: DependencyKind, span: Span) {
        self.b.references.push(ReferenceDep {
            origin_member: self.member,
            target,
            kind,
            line: line_of(span),
            macro_argument_count: None,
        });
    }

    fn record_member_access(
        &mut self,
        kind: AccessKind,
        owner: ItemId,
        name: &str,
        member: Option<MemberId>,
        resolution: ResolutionKind,
        span: Span,
    ) {
        let full_name = match member {
            Some(m) => self.b.graph.members[m].full_name.clone(),
            None => match kind {
                AccessKind::FieldAccess(_) => format!("{}::{name}", self.b.graph.items[owner].name),
                _ => format!("{}::{name}()", self.b.graph.items[owner].name),
            },
        };
        let kind = match (kind, member) {
            (AccessKind::MethodCall, Some(m)) if self.b.graph.members[m].is_constructor => {
                AccessKind::ConstructorCall
            }
            (k, _) => k,
        };
        self.record_access(
            kind,
            owner,
            name.to_owned(),
            full_name,
            member.into_iter().collect(),
            resolution,
            span,
        );
    }

    fn unresolved_owner(&mut self) -> ItemId {
        self.b.graph.intern_stub("<unresolved>", ItemKind::Module)
    }

    fn resolve_value_path(&mut self, path: &syn::Path) -> Callee {
        if path.segments.len() == 1 && path.leading_colon.is_none() {
            let name = path.segments[0].ident.to_string();
            if self.local_type(&name).is_some() || name == "self" {
                return Callee::None;
            }
        }
        let segments = segments_of(path);
        match self
            .b
            .resolve_path(self.scope, &segments, Ns::Value, &self.ctx)
        {
            Some(Resolved::Item(id)) => match self.b.graph.items[id].kind {
                ItemKind::Function => Callee::Function(id),
                ItemKind::Struct => Callee::Struct(id),
                ItemKind::Const | ItemKind::Static => {
                    let span = path
                        .segments
                        .last()
                        .map(|s| s.ident.span())
                        .unwrap_or_else(Span::call_site);
                    self.record_reference(id, DependencyKind::Reference, span);
                    Callee::None
                }
                ItemKind::Unknown => {
                    // A stub: treat `a::b::c` as an associated function of `a::b` when it looks like one.
                    let name = self.b.graph.items[id].simple_name.clone();
                    match self.b.graph.items[id].package {
                        Some(owner) if segments.len() > 1 => Callee::Assoc(owner, name),
                        _ => Callee::Function(id),
                    }
                }
                _ => Callee::None,
            },
            Some(Resolved::Variant { enum_, name }) => Callee::Variant(enum_, name),
            Some(Resolved::Assoc { base, name }) => Callee::Assoc(base, name),
            Some(Resolved::Generic(_)) | None => Callee::None,
        }
    }

    fn handle_call(&mut self, path: &syn::Path, span: Span) {
        match self.resolve_value_path(path) {
            Callee::Function(id) => {
                let member = self.b.graph.items[id].members.first().copied();
                self.record_member_access(
                    AccessKind::MethodCall,
                    id,
                    &self.b.graph.items[id].simple_name.clone(),
                    member,
                    ResolutionKind::Resolved,
                    span,
                );
            }
            Callee::Struct(id) => {
                let name = self.b.graph.items[id].name.clone();
                self.record_access(
                    AccessKind::ConstructorCall,
                    id,
                    "(..)".to_owned(),
                    format!("{name}(..)"),
                    Vec::new(),
                    ResolutionKind::Resolved,
                    span,
                );
            }
            Callee::Variant(enum_, variant) => {
                let name = self.b.graph.items[enum_].name.clone();
                let member = self.b.graph.items[enum_]
                    .members
                    .iter()
                    .copied()
                    .find(|&m| {
                        let md = &self.b.graph.members[m];
                        md.kind == MemberKind::Variant && md.name == variant
                    });
                self.record_access(
                    AccessKind::ConstructorCall,
                    enum_,
                    variant.clone(),
                    format!("{name}::{variant}(..)"),
                    member.into_iter().collect(),
                    ResolutionKind::Resolved,
                    span,
                );
            }
            Callee::Assoc(base, name) => {
                let member = self.b.find_method(base, &name);
                let resolution = if member.is_some() {
                    ResolutionKind::Resolved
                } else {
                    ResolutionKind::OwnerOnly
                };
                let kind = if member.is_none() && matches!(name.as_str(), "new" | "default") {
                    AccessKind::ConstructorCall
                } else {
                    AccessKind::MethodCall
                };
                self.record_member_access(kind, base, &name, member, resolution, span);
            }
            Callee::None => {}
        }
    }

    fn infer_type(&mut self, expr: &syn::Expr) -> Option<RustType> {
        match expr {
            syn::Expr::Path(p) => {
                if p.qself.is_none() && p.path.segments.len() == 1 {
                    let name = p.path.segments[0].ident.to_string();
                    if let Some(ty) = self.local_type(&name) {
                        return Some(ty);
                    }
                }
                let segments = segments_of(&p.path);
                match self
                    .b
                    .resolve_path(self.scope, &segments, Ns::Value, &self.ctx)?
                {
                    Resolved::Item(id) => match self.b.graph.items[id].kind {
                        ItemKind::Struct => Some(self.path_type(id)),
                        ItemKind::Const | ItemKind::Static => {
                            self.b.graph.items[id].item_type.clone()
                        }
                        _ => None,
                    },
                    Resolved::Variant { enum_, .. } => Some(self.path_type(enum_)),
                    Resolved::Assoc { base, name } => {
                        if let Some(m) = self.b.find_field(base, &name) {
                            self.b.graph.members[m].type_.clone()
                        } else {
                            None
                        }
                    }
                    Resolved::Generic(_) => None,
                }
            }
            syn::Expr::Call(call) => {
                let syn::Expr::Path(p) = &*call.func else {
                    return None;
                };
                match self.resolve_value_path(&p.path) {
                    Callee::Function(id) => {
                        let member = self.b.graph.items[id].members.first().copied()?;
                        self.b.graph.members[member].return_type.clone()
                    }
                    Callee::Struct(id) => Some(self.path_type(id)),
                    Callee::Variant(enum_, _) => Some(self.path_type(enum_)),
                    Callee::Assoc(base, name) => match self.b.find_method(base, &name) {
                        Some(m) => self.b.graph.members[m].return_type.clone(),
                        None if matches!(
                            name.as_str(),
                            "new" | "default" | "from" | "with_capacity"
                        ) =>
                        {
                            Some(self.path_type(base))
                        }
                        None => None,
                    },
                    Callee::None => None,
                }
            }
            syn::Expr::MethodCall(mc) => {
                let receiver = self.infer_type(&mc.receiver)?;
                let name = mc.method.to_string();
                let receiver_item = receiver.raw_item()?;
                let simple = self.b.graph.items[receiver_item].simple_name.clone();
                if matches!(simple.as_str(), "Option" | "Result" | "Box" | "Rc" | "Arc")
                    && matches!(
                        name.as_str(),
                        "unwrap" | "expect" | "unwrap_or" | "unwrap_or_default" | "unwrap_or_else"
                    )
                {
                    return receiver.type_arguments().first().cloned();
                }
                if matches!(
                    name.as_str(),
                    "clone" | "to_owned" | "as_ref" | "as_mut" | "borrow" | "borrow_mut"
                ) {
                    return Some(receiver);
                }
                let m = self.b.find_method(receiver_item, &name)?;
                self.b.graph.members[m].return_type.clone()
            }
            syn::Expr::Struct(s) => {
                let segments = segments_of(&s.path);
                match self
                    .b
                    .resolve_path(self.scope, &segments, Ns::Type, &self.ctx)?
                {
                    Resolved::Item(id) => Some(self.path_type(id)),
                    Resolved::Variant { enum_, .. } => Some(self.path_type(enum_)),
                    _ => None,
                }
            }
            syn::Expr::Reference(r) => self.infer_type(&r.expr).map(|inner| RustType::Reference {
                mutable: r.mutability.is_some(),
                inner: Box::new(inner),
            }),
            syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => {
                match self.infer_type(&u.expr)? {
                    RustType::Reference { inner, .. } | RustType::Ptr { inner, .. } => Some(*inner),
                    other => Some(other),
                }
            }
            syn::Expr::Paren(p) => self.infer_type(&p.expr),
            syn::Expr::Field(f) => {
                let base = self.infer_type(&f.base)?;
                let base_item = base.raw_item()?;
                let name = match &f.member {
                    syn::Member::Named(i) => i.to_string(),
                    syn::Member::Unnamed(i) => i.index.to_string(),
                };
                let m = self.b.find_field(base_item, &name)?;
                self.b.graph.members[m].type_.clone()
            }
            syn::Expr::Try(t) => {
                let inner = self.infer_type(&t.expr)?;
                inner.type_arguments().first().cloned()
            }
            syn::Expr::Await(a) => self.infer_type(&a.base),
            syn::Expr::Cast(c) => Some(self.b.convert_type(&c.ty, self.scope, &self.ctx)),
            syn::Expr::Block(b) => {
                let last = b.block.stmts.last()?;
                match last {
                    syn::Stmt::Expr(e, None) => self.infer_type(e),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn path_type(&self, item: ItemId) -> RustType {
        RustType::Path {
            item,
            written: self.b.graph.items[item].simple_name.clone(),
            args: Vec::new(),
        }
    }

    fn handle_macro(&mut self, mac: &syn::Macro) {
        let span = mac
            .path
            .segments
            .last()
            .map(|s| s.ident.span())
            .unwrap_or_else(Span::call_site);
        let target = self.b.resolve_macro_target(&mac.path, self.scope);
        let parsed = mac
            .parse_body_with(
                syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
            )
            .or_else(|_| {
                let rewritten: proc_macro2::TokenStream = mac
                    .tokens
                    .clone()
                    .into_iter()
                    .map(|tt| match &tt {
                        proc_macro2::TokenTree::Punct(p) if p.as_char() == ';' => {
                            proc_macro2::TokenTree::Punct(proc_macro2::Punct::new(
                                ',',
                                proc_macro2::Spacing::Alone,
                            ))
                        }
                        _ => tt,
                    })
                    .collect();
                syn::parse::Parser::parse2(
                    syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
                    rewritten,
                )
            });
        if let Some(target) = target {
            self.b.references.push(ReferenceDep {
                origin_member: self.member,
                target,
                kind: DependencyKind::MacroInvocation,
                line: line_of(span),
                macro_argument_count: parsed.as_ref().ok().map(|exprs| exprs.len()),
            });
        }
        if let Ok(exprs) = parsed {
            for expr in exprs.iter() {
                self.visit_expr(expr);
            }
        }
    }

    fn handle_field(&mut self, f: &syn::ExprField, access_type: AccessType) {
        let name = match &f.member {
            syn::Member::Named(i) => i.to_string(),
            syn::Member::Unnamed(i) => i.index.to_string(),
        };
        let span = match &f.member {
            syn::Member::Named(i) => i.span(),
            syn::Member::Unnamed(i) => i.span,
        };
        let base = self.infer_type(&f.base);
        match base.as_ref().and_then(RustType::raw_item) {
            Some(owner) => {
                let member = self.b.find_field(owner, &name);
                let resolution = if member.is_some() {
                    ResolutionKind::Resolved
                } else if self.b.graph.items[owner].fully_imported {
                    ResolutionKind::Unresolved
                } else {
                    ResolutionKind::OwnerOnly
                };
                self.record_member_access(
                    AccessKind::FieldAccess(access_type),
                    owner,
                    &name,
                    member,
                    resolution,
                    span,
                );
            }
            None => {
                if let Some(m) = self
                    .b
                    .unique_member_by_name(&name, MemberKind::Field, false)
                {
                    let owner = self.b.graph.members[m].owner;
                    self.record_member_access(
                        AccessKind::FieldAccess(access_type),
                        owner,
                        &name,
                        Some(m),
                        ResolutionKind::ByNameOnly,
                        span,
                    );
                } else if name.chars().all(|c| c.is_ascii_digit()) {
                    // Tuple index on an unknown receiver: not worth recording.
                } else {
                    let owner = self.unresolved_owner();
                    self.record_member_access(
                        AccessKind::FieldAccess(access_type),
                        owner,
                        &name,
                        None,
                        ResolutionKind::Unresolved,
                        span,
                    );
                }
            }
        }
    }

    fn handle_pattern_path(&mut self, path: &syn::Path) {
        let segments = segments_of(path);
        let span = path
            .segments
            .last()
            .map(|s| s.ident.span())
            .unwrap_or_else(Span::call_site);
        if segments.len() == 1
            && (self.local_type(&segments[0]).is_some()
                || segments[0].chars().next().is_some_and(|c| c.is_lowercase()))
        {
            return;
        }
        if let Some(target) = self
            .b
            .resolve_path(self.scope, &segments, Ns::Any, &self.ctx)
            .and_then(|r| r.item())
        {
            self.record_reference(target, DependencyKind::Reference, span);
        }
    }
}

impl<'ast> Visit<'ast> for BodyVisitor<'_, '_> {
    fn visit_item(&mut self, _item: &'ast syn::Item) {
        // Local items are declared and visited on their own.
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.locals.push(HashMap::new());
        syn::visit::visit_block(self, block);
        self.locals.pop();
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        let mut inferred = None;
        if let Some(init) = &local.init {
            self.visit_expr(&init.expr);
            inferred = self.infer_type(&init.expr);
            if let Some((_, diverge)) = &init.diverge {
                self.visit_expr(diverge);
            }
        }
        match &local.pat {
            syn::Pat::Type(pt) => {
                let ty = self.b.convert_type(&pt.ty, self.scope, &self.ctx);
                let span = pt.colon_token.span;
                for item in ty.all_items() {
                    self.record_reference(item, DependencyKind::Reference, span);
                }
                if let syn::Pat::Ident(i) = &*pt.pat {
                    self.bind_local(i.ident.to_string(), Some(ty));
                }
            }
            syn::Pat::Ident(i) => {
                self.bind_local(i.ident.to_string(), inferred);
            }
            other => self.visit_pat(other),
        }
    }

    fn visit_expr_path(&mut self, expr: &'ast syn::ExprPath) {
        if let Some(qself) = &expr.qself {
            self.visit_type(&qself.ty);
        }
        let span = expr
            .path
            .segments
            .last()
            .map(|s| s.ident.span())
            .unwrap_or_else(Span::call_site);
        match self.resolve_value_path(&expr.path) {
            Callee::Function(id) => {
                let member = self.b.graph.items[id].members.first().copied();
                let name = self.b.graph.items[id].simple_name.clone();
                self.record_member_access(
                    AccessKind::FunctionReference,
                    id,
                    &name,
                    member,
                    ResolutionKind::Resolved,
                    span,
                );
            }
            Callee::Struct(id) => {
                if self.b.tuple_or_unit_structs.contains(&id) {
                    let name = self.b.graph.items[id].name.clone();
                    self.record_access(
                        AccessKind::ConstructorCall,
                        id,
                        "".to_owned(),
                        name,
                        Vec::new(),
                        ResolutionKind::Resolved,
                        span,
                    );
                }
            }
            Callee::Variant(enum_, variant) => {
                let name = self.b.graph.items[enum_].name.clone();
                self.record_access(
                    AccessKind::ConstructorCall,
                    enum_,
                    variant.clone(),
                    format!("{name}::{variant}"),
                    Vec::new(),
                    ResolutionKind::Resolved,
                    span,
                );
            }
            Callee::Assoc(base, name) => {
                if let Some(m) = self.b.find_field(base, &name) {
                    self.record_member_access(
                        AccessKind::FieldAccess(AccessType::Get),
                        base,
                        &name,
                        Some(m),
                        ResolutionKind::Resolved,
                        span,
                    );
                } else if let Some(m) = self.b.find_method(base, &name) {
                    self.record_member_access(
                        AccessKind::FunctionReference,
                        base,
                        &name,
                        Some(m),
                        ResolutionKind::Resolved,
                        span,
                    );
                } else if is_screaming_case(&name) {
                    self.record_member_access(
                        AccessKind::FieldAccess(AccessType::Get),
                        base,
                        &name,
                        None,
                        ResolutionKind::OwnerOnly,
                        span,
                    );
                } else {
                    self.record_member_access(
                        AccessKind::FunctionReference,
                        base,
                        &name,
                        None,
                        ResolutionKind::OwnerOnly,
                        span,
                    );
                }
            }
            Callee::None => {}
        }
        for seg in &expr.path.segments {
            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                self.visit_angle_bracketed_generic_arguments(args);
            }
        }
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        match &*call.func {
            syn::Expr::Path(p) => {
                let span = p
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident.span())
                    .unwrap_or_else(Span::call_site);
                if let Some(qself) = &p.qself {
                    self.visit_type(&qself.ty);
                }
                self.handle_call(&p.path, span);
                for seg in &p.path.segments {
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        self.visit_angle_bracketed_generic_arguments(args);
                    }
                }
            }
            other => self.visit_expr(other),
        }
        for arg in &call.args {
            self.visit_expr(arg);
        }
    }

    fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
        let name = mc.method.to_string();
        let span = mc.method.span();
        let receiver_type = self.infer_type(&mc.receiver);
        match receiver_type.as_ref().and_then(RustType::raw_item) {
            Some(owner) => {
                let member = self.b.find_method(owner, &name);
                let resolution = if member.is_some() {
                    ResolutionKind::Resolved
                } else if self.b.graph.items[owner].fully_imported {
                    self.b
                        .unique_member_by_name(&name, MemberKind::Method, true)
                        .map(|_| ResolutionKind::ByNameOnly)
                        .unwrap_or(ResolutionKind::OwnerOnly)
                } else {
                    ResolutionKind::OwnerOnly
                };
                let (owner, member) = match (member, resolution) {
                    (None, ResolutionKind::ByNameOnly) => {
                        let m = self
                            .b
                            .unique_member_by_name(&name, MemberKind::Method, true)
                            .expect("checked");
                        (self.b.graph.members[m].owner, Some(m))
                    }
                    _ => (owner, member),
                };
                self.record_member_access(
                    AccessKind::MethodCall,
                    owner,
                    &name,
                    member,
                    resolution,
                    span,
                );
            }
            None => match self
                .b
                .unique_member_by_name(&name, MemberKind::Method, true)
            {
                Some(m) => {
                    let owner = self.b.graph.members[m].owner;
                    self.record_member_access(
                        AccessKind::MethodCall,
                        owner,
                        &name,
                        Some(m),
                        ResolutionKind::ByNameOnly,
                        span,
                    );
                }
                None => {
                    let owner = self.unresolved_owner();
                    self.record_member_access(
                        AccessKind::MethodCall,
                        owner,
                        &name,
                        None,
                        ResolutionKind::Unresolved,
                        span,
                    );
                }
            },
        }
        if let Some(turbofish) = &mc.turbofish {
            self.visit_angle_bracketed_generic_arguments(turbofish);
        }
        self.visit_expr(&mc.receiver);
        for arg in &mc.args {
            self.visit_expr(arg);
        }
    }

    fn visit_expr_field(&mut self, f: &'ast syn::ExprField) {
        let access_type = if self.assigning {
            AccessType::Set
        } else {
            AccessType::Get
        };
        self.assigning = false;
        self.handle_field(f, access_type);
        self.visit_expr(&f.base);
    }

    fn visit_expr_assign(&mut self, assign: &'ast syn::ExprAssign) {
        self.assigning = true;
        self.visit_expr(&assign.left);
        self.assigning = false;
        self.visit_expr(&assign.right);
    }

    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        let compound = matches!(
            binary.op,
            syn::BinOp::AddAssign(_)
                | syn::BinOp::SubAssign(_)
                | syn::BinOp::MulAssign(_)
                | syn::BinOp::DivAssign(_)
                | syn::BinOp::RemAssign(_)
                | syn::BinOp::BitXorAssign(_)
                | syn::BinOp::BitAndAssign(_)
                | syn::BinOp::BitOrAssign(_)
                | syn::BinOp::ShlAssign(_)
                | syn::BinOp::ShrAssign(_)
        );
        self.assigning = compound;
        self.visit_expr(&binary.left);
        self.assigning = false;
        self.visit_expr(&binary.right);
    }

    fn visit_expr_reference(&mut self, r: &'ast syn::ExprReference) {
        if r.mutability.is_some() && matches!(*r.expr, syn::Expr::Field(_)) {
            self.assigning = true;
        }
        self.visit_expr(&r.expr);
        self.assigning = false;
    }

    fn visit_expr_struct(&mut self, s: &'ast syn::ExprStruct) {
        let segments = segments_of(&s.path);
        let span = s
            .path
            .segments
            .last()
            .map(|seg| seg.ident.span())
            .unwrap_or_else(Span::call_site);
        match self
            .b
            .resolve_path(self.scope, &segments, Ns::Type, &self.ctx)
        {
            Some(Resolved::Item(id)) => {
                let name = self.b.graph.items[id].name.clone();
                self.record_access(
                    AccessKind::ConstructorCall,
                    id,
                    "{ .. }".to_owned(),
                    format!("{name} {{ .. }}"),
                    Vec::new(),
                    ResolutionKind::Resolved,
                    span,
                );
            }
            Some(Resolved::Variant { enum_, name }) => {
                let enum_name = self.b.graph.items[enum_].name.clone();
                self.record_access(
                    AccessKind::ConstructorCall,
                    enum_,
                    name.clone(),
                    format!("{enum_name}::{name} {{ .. }}"),
                    Vec::new(),
                    ResolutionKind::Resolved,
                    span,
                );
            }
            _ => {}
        }
        for field in &s.fields {
            self.visit_expr(&field.expr);
        }
        if let Some(rest) = &s.rest {
            self.visit_expr(rest);
        }
    }

    fn visit_expr_macro(&mut self, m: &'ast syn::ExprMacro) {
        self.handle_macro(&m.mac);
    }

    fn visit_stmt_macro(&mut self, m: &'ast syn::StmtMacro) {
        self.handle_macro(&m.mac);
    }

    fn visit_expr_unsafe(&mut self, block: &'ast syn::ExprUnsafe) {
        let line = line_of(block.unsafe_token.span);
        self.b.graph.members[self.member]
            .unsafe_block_lines
            .push(line);
        syn::visit::visit_expr_unsafe(self, block);
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        self.closure_depth += 1;
        self.locals.push(HashMap::new());
        for input in &closure.inputs {
            if let syn::Pat::Type(pt) = input {
                let ty = self.b.convert_type(&pt.ty, self.scope, &self.ctx);
                if let syn::Pat::Ident(i) = &*pt.pat {
                    self.bind_local(i.ident.to_string(), Some(ty));
                }
            }
        }
        self.visit_expr(&closure.body);
        self.locals.pop();
        self.closure_depth -= 1;
    }

    fn visit_expr_cast(&mut self, cast: &'ast syn::ExprCast) {
        self.visit_expr(&cast.expr);
        self.visit_type(&cast.ty);
    }

    fn visit_type(&mut self, ty: &'ast syn::Type) {
        let converted = self.b.convert_type(ty, self.scope, &self.ctx);
        let span = match ty {
            syn::Type::Path(p) => p
                .path
                .segments
                .last()
                .map(|s| s.ident.span())
                .unwrap_or_else(Span::call_site),
            _ => Span::call_site(),
        };
        for item in converted.all_items() {
            self.record_reference(item, DependencyKind::Reference, span);
        }
    }

    fn visit_pat(&mut self, pat: &'ast syn::Pat) {
        match pat {
            syn::Pat::Path(p) => self.handle_pattern_path(&p.path),
            other => syn::visit::visit_pat(self, other),
        }
    }

    fn visit_pat_tuple_struct(&mut self, p: &'ast syn::PatTupleStruct) {
        self.handle_pattern_path(&p.path);
        for elem in &p.elems {
            self.visit_pat(elem);
        }
    }

    fn visit_pat_struct(&mut self, p: &'ast syn::PatStruct) {
        self.handle_pattern_path(&p.path);
        for field in &p.fields {
            self.visit_pat(&field.pat);
        }
    }

    fn visit_pat_ident(&mut self, p: &'ast syn::PatIdent) {
        if let Some((_, sub)) = &p.subpat {
            self.visit_pat(sub);
        }
    }
}

trait SpanOf {
    fn span(&self) -> Span;
}

impl SpanOf for syn::Type {
    fn span(&self) -> Span {
        syn::spanned::Spanned::span(self)
    }
}
