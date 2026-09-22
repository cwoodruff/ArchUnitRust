//! Internal storage of the imported code graph. Public handle types wrap indexes into it.

use std::collections::HashMap;

use super::annotation::RustAnnotation;
use super::dependency::DependencyKind;
use super::modifiers::{RustModifier, Visibility};
use super::rust_access::{AccessKind, ResolutionKind};
use super::rust_item::{ItemKind, TargetKind};
use super::rust_member::{MemberKind, Receiver};
use super::source_code_location::SourceCodeLocation;
use super::types::{RustType, TypeParameter};

pub(crate) type ItemId = usize;
pub(crate) type MemberId = usize;
pub(crate) type AccessId = usize;
pub(crate) type DependencyId = usize;

#[derive(Debug)]
pub(crate) struct ItemData {
    pub name: String,
    pub simple_name: String,
    pub kind: ItemKind,
    pub package: Option<ItemId>,
    pub crate_name: String,
    pub target: TargetKind,
    pub visibility: Visibility,
    pub modifiers: Vec<RustModifier>,
    pub annotations: Vec<RustAnnotation>,
    pub location: SourceCodeLocation,
    pub fully_imported: bool,
    pub is_primitive: bool,
    pub is_test_code: bool,
    pub enclosing_item: Option<ItemId>,
    pub enclosing_code_unit: Option<MemberId>,
    pub aliases: Vec<String>,
    pub members: Vec<MemberId>,
    pub type_parameters: Vec<TypeParameter>,
    pub supertraits: Vec<ItemId>,
    pub implemented_traits: Vec<ItemId>,
    pub impl_info: Option<ImplData>,
    pub alias_target: Option<RustType>,
    pub item_type: Option<RustType>,
}

impl ItemData {
    pub(crate) fn stub(name: &str, kind: ItemKind, package: Option<ItemId>) -> Self {
        let simple_name = name.rsplit("::").next().unwrap_or(name).to_owned();
        let crate_name = name.split("::").next().unwrap_or(name).to_owned();
        Self {
            name: name.to_owned(),
            simple_name,
            kind,
            package,
            crate_name,
            target: TargetKind::Lib,
            visibility: Visibility::Pub,
            modifiers: vec![RustModifier::Pub],
            annotations: Vec::new(),
            location: SourceCodeLocation::unknown(),
            fully_imported: false,
            is_primitive: false,
            is_test_code: false,
            enclosing_item: None,
            enclosing_code_unit: None,
            aliases: Vec::new(),
            members: Vec::new(),
            type_parameters: Vec::new(),
            supertraits: Vec::new(),
            implemented_traits: Vec::new(),
            impl_info: None,
            alias_target: None,
            item_type: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ImplData {
    pub self_type: RustType,
    pub trait_: Option<ItemId>,
}

#[derive(Debug)]
pub(crate) struct MemberData {
    pub owner: ItemId,
    pub kind: MemberKind,
    pub name: String,
    pub full_name: String,
    pub visibility: Visibility,
    pub modifiers: Vec<RustModifier>,
    pub annotations: Vec<RustAnnotation>,
    pub location: SourceCodeLocation,
    pub is_test_code: bool,
    pub type_: Option<RustType>,
    pub parameters: Vec<ParameterData>,
    pub return_type: Option<RustType>,
    pub error_type: Option<RustType>,
    pub receiver: Option<Receiver>,
    pub is_constructor: bool,
    pub is_trait_declaration: bool,
    pub has_body: bool,
    pub declaring_impl: Option<ItemId>,
    pub implemented_trait: Option<ItemId>,
    pub type_parameters: Vec<TypeParameter>,
    /// For free functions: the function item this member stands for.
    pub item: Option<ItemId>,
}

#[derive(Debug, Clone)]
pub(crate) struct ParameterData {
    pub name: String,
    pub type_: RustType,
    pub annotations: Vec<RustAnnotation>,
}

#[derive(Debug)]
pub(crate) struct AccessData {
    pub origin: MemberId,
    pub kind: AccessKind,
    pub target_owner: ItemId,
    pub target_name: String,
    pub target_full_name: String,
    pub resolved: Vec<MemberId>,
    pub resolution: ResolutionKind,
    pub location: SourceCodeLocation,
    pub in_closure: bool,
}

#[derive(Debug)]
pub(crate) struct DependencyData {
    pub origin: ItemId,
    pub target: ItemId,
    pub kind: DependencyKind,
    pub description: String,
    pub location: SourceCodeLocation,
}

#[derive(Debug, Default)]
pub(crate) struct Graph {
    pub items: Vec<ItemData>,
    pub members: Vec<MemberData>,
    pub accesses: Vec<AccessData>,
    pub dependencies: Vec<DependencyData>,
    pub by_name: HashMap<String, ItemId>,
    /// Public re-export paths (`my_app::Order` for `pub use domain::Order`) to items.
    pub aliases: HashMap<String, ItemId>,
    pub crate_roots: Vec<ItemId>,
    pub children: Vec<Vec<ItemId>>,
    pub deps_from: Vec<Vec<DependencyId>>,
    pub deps_to: Vec<Vec<DependencyId>>,
    pub accesses_from_member: Vec<Vec<AccessId>>,
    pub accesses_to_item: Vec<Vec<AccessId>>,
    pub accesses_to_member: Vec<Vec<AccessId>>,
    pub implementors: Vec<Vec<ItemId>>,
    pub subtraits: Vec<Vec<ItemId>>,
}

impl Graph {
    pub(crate) fn add_item(&mut self, data: ItemData) -> ItemId {
        let id = self.items.len();
        self.by_name.entry(data.name.clone()).or_insert(id);
        if let Some(parent) = data.package {
            self.children[parent].push(id);
        }
        self.items.push(data);
        self.children.push(Vec::new());
        id
    }

    pub(crate) fn add_member(&mut self, data: MemberData) -> MemberId {
        let id = self.members.len();
        self.items[data.owner].members.push(id);
        self.members.push(data);
        id
    }

    /// Finds or creates a stub item for a fully qualified path outside of the import.
    pub(crate) fn intern_stub(&mut self, path: &str, kind: ItemKind) -> ItemId {
        if let Some(&id) = self.by_name.get(path) {
            return id;
        }
        let package = path
            .rsplit_once("::")
            .map(|(parent, _)| self.intern_stub(parent, ItemKind::Module));
        self.add_item(ItemData::stub(path, kind, package))
    }

    /// Looks an item up by canonical name or public re-export path.
    pub(crate) fn find(&self, name: &str) -> Option<ItemId> {
        self.by_name
            .get(name)
            .or_else(|| self.aliases.get(name))
            .copied()
    }

    /// Whether an item counts as a "class": everything but modules and impl blocks whose self
    /// type is itself an imported item.
    pub(crate) fn is_class_like(&self, id: ItemId) -> bool {
        let data = &self.items[id];
        match data.kind {
            ItemKind::Module => false,
            ItemKind::Impl => data
                .impl_info
                .as_ref()
                .and_then(|i| i.self_type.raw_item())
                .is_none_or(|self_type| !self.items[self_type].fully_imported),
            _ => true,
        }
    }

    /// Follows type aliases to the item they stand for (`JavaType.toErasure()` for aliases).
    pub(crate) fn erase(&self, mut id: ItemId) -> ItemId {
        for _ in 0..8 {
            let data = &self.items[id];
            if data.kind != ItemKind::TypeAlias {
                return id;
            }
            match data.alias_target.as_ref().and_then(RustType::raw_item) {
                Some(target) if target != id => id = target,
                _ => return id,
            }
        }
        id
    }

    /// The erased item of a type, following aliases.
    pub(crate) fn erased(&self, ty: &RustType) -> Option<ItemId> {
        ty.raw_item().map(|id| self.erase(id))
    }

    /// Builds all reverse indexes. Called once after the import is complete.
    pub(crate) fn build_indexes(&mut self) {
        let n = self.items.len();
        let m = self.members.len();
        self.deps_from = vec![Vec::new(); n];
        self.deps_to = vec![Vec::new(); n];
        self.accesses_to_item = vec![Vec::new(); n];
        self.implementors = vec![Vec::new(); n];
        self.subtraits = vec![Vec::new(); n];
        self.accesses_from_member = vec![Vec::new(); m];
        self.accesses_to_member = vec![Vec::new(); m];
        for (id, dep) in self.dependencies.iter().enumerate() {
            self.deps_from[dep.origin].push(id);
            self.deps_to[dep.target].push(id);
        }
        for (id, access) in self.accesses.iter().enumerate() {
            self.accesses_from_member[access.origin].push(id);
            self.accesses_to_item[access.target_owner].push(id);
            for &member in &access.resolved {
                self.accesses_to_member[member].push(id);
            }
        }
        for id in 0..n {
            for &trait_ in &self.items[id].implemented_traits {
                self.implementors[trait_].push(id);
            }
            for &supertrait in &self.items[id].supertraits {
                self.subtraits[supertrait].push(id);
            }
        }
        for list in self.deps_from.iter_mut().chain(self.deps_to.iter_mut()) {
            list.sort_by(|a, b| {
                self.dependencies[*a]
                    .description
                    .cmp(&self.dependencies[*b].description)
            });
        }
    }
}
