use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::annotation::RustAnnotation;
use super::dependency::Dependency;
use super::graph::{Graph, ItemId};
use super::properties::{CanBeAnnotated, HasName};
use super::rust_item::{ItemKind, RustItem};
use crate::base::{DescribedPredicate, HasDescription};

/// A module, playing the role of ArchUnit's `JavaPackage`.
///
/// Modules are also [`RustItem`]s of kind [`ItemKind::Module`]; this view adds the package
/// style API (`items()`, `subpackages()`, dependencies between packages, ...).
#[derive(Clone)]
pub struct RustModule {
    graph: Arc<Graph>,
    id: ItemId,
}

impl PartialEq for RustModule {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && Arc::ptr_eq(&self.graph, &other.graph)
    }
}

impl Eq for RustModule {}

impl Hash for RustModule {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl fmt::Debug for RustModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RustModule({})", self.name())
    }
}

impl fmt::Display for RustModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

impl HasDescription for RustModule {
    fn description(&self) -> String {
        format!("Module <{}>", self.name())
    }
}

impl HasName for RustModule {
    fn name(&self) -> String {
        self.graph.items[self.id].name.clone()
    }
}

impl CanBeAnnotated for RustModule {
    fn annotations(&self) -> Vec<RustAnnotation> {
        self.graph.items[self.id].annotations.clone()
    }
}

impl RustModule {
    pub(crate) fn new(graph: Arc<Graph>, id: ItemId) -> Self {
        Self { graph, id }
    }

    fn item(&self, id: ItemId) -> RustItem {
        RustItem::new(Arc::clone(&self.graph), id)
    }

    fn module(&self, id: ItemId) -> RustModule {
        RustModule::new(Arc::clone(&self.graph), id)
    }

    /// The full module path, e.g. `my_app::domain` (`JavaPackage.getName()`).
    pub fn name(&self) -> String {
        HasName::name(self)
    }

    /// The last segment (`JavaPackage.getRelativeName()`).
    pub fn relative_name(&self) -> String {
        self.graph.items[self.id].simple_name.clone()
    }

    /// `Module <name>`.
    pub fn description(&self) -> String {
        HasDescription::description(self)
    }

    /// This module as an item.
    pub fn as_item(&self) -> RustItem {
        self.item(self.id)
    }

    /// The crate this module belongs to.
    pub fn crate_name(&self) -> String {
        self.graph.items[self.id].crate_name.clone()
    }

    /// The parent module (`JavaPackage.getParent()`); `None` for crate roots.
    pub fn parent(&self) -> Option<RustModule> {
        self.graph.items[self.id].package.map(|id| self.module(id))
    }

    fn child_ids(&self) -> &[ItemId] {
        &self.graph.children[self.id]
    }

    /// Items declared directly in this module, excluding submodules (`JavaPackage.getClasses()`).
    pub fn items(&self) -> Vec<RustItem> {
        self.child_ids()
            .iter()
            .filter(|&&id| self.graph.is_class_like(id))
            .map(|&id| self.item(id))
            .collect()
    }

    /// Items in this module and all submodules (`JavaPackage.getClassesInPackageTree()`).
    pub fn items_in_package_tree(&self) -> Vec<RustItem> {
        let mut result = self.items();
        for sub in self.subpackages() {
            result.extend(sub.items_in_package_tree());
        }
        result
    }

    /// Direct submodules (`JavaPackage.getSubpackages()`).
    pub fn subpackages(&self) -> Vec<RustModule> {
        self.child_ids()
            .iter()
            .filter(|&&id| self.graph.items[id].kind == ItemKind::Module)
            .map(|&id| self.module(id))
            .collect()
    }

    /// All transitive submodules (`JavaPackage.getSubpackagesInTree()`).
    pub fn subpackages_in_tree(&self) -> Vec<RustModule> {
        let mut result = Vec::new();
        for sub in self.subpackages() {
            result.push(sub.clone());
            result.extend(sub.subpackages_in_tree());
        }
        result
    }

    /// The submodule with the given relative or full name (`JavaPackage.getPackage(..)`).
    ///
    /// # Panics
    /// If there is no such module.
    pub fn package(&self, name: &str) -> RustModule {
        self.try_package(name)
            .unwrap_or_else(|| panic!("Module {} does not contain module {name}", self.name()))
    }

    /// The submodule with the given relative or full name, if any.
    pub fn try_package(&self, name: &str) -> Option<RustModule> {
        let full = if name.starts_with(&format!("{}::", self.name())) {
            name.to_owned()
        } else {
            format!("{}::{name}", self.name())
        };
        self.subpackages_in_tree()
            .into_iter()
            .find(|m| m.name() == full)
    }

    /// `JavaPackage.containsPackage(..)`.
    pub fn contains_package(&self, name: &str) -> bool {
        self.try_package(name).is_some()
    }

    /// The item with the given simple name declared in this module
    /// (`JavaPackage.getClassWithSimpleName(..)`).
    pub fn item_with_simple_name(&self, simple_name: &str) -> Option<RustItem> {
        self.items()
            .into_iter()
            .find(|i| i.simple_name() == simple_name)
    }

    /// The item with the given full name declared in this module
    /// (`JavaPackage.getClassWithFullyQualifiedName(..)`).
    pub fn item_with_fully_qualified_name(&self, name: &str) -> Option<RustItem> {
        self.items().into_iter().find(|i| i.name() == name)
    }

    /// `JavaPackage.containsClassWithSimpleName(..)`.
    pub fn contains_item_with_simple_name(&self, simple_name: &str) -> bool {
        self.item_with_simple_name(simple_name).is_some()
    }

    /// `JavaPackage.containsClassWithFullyQualifiedName(..)`.
    pub fn contains_item_with_fully_qualified_name(&self, name: &str) -> bool {
        self.item_with_fully_qualified_name(name).is_some()
    }

    /// `JavaPackage.containsClass(..)`.
    pub fn contains_item(&self, item: &RustItem) -> bool {
        self.child_ids().contains(&item.id)
    }

    fn in_tree(&self, id: ItemId) -> bool {
        let mut current = Some(id);
        while let Some(c) = current {
            if c == self.id {
                return true;
            }
            current = self.graph.items[c].package;
        }
        false
    }

    /// Dependencies from items of this module (`JavaPackage.getClassDependenciesFromThisPackage()`).
    pub fn item_dependencies_from_this_package(&self) -> Vec<Dependency> {
        self.items()
            .iter()
            .flat_map(RustItem::direct_dependencies_from_self)
            .filter(|d| !self.contains_item(&d.target_item()))
            .collect()
    }

    /// Dependencies to items of this module (`JavaPackage.getClassDependenciesToThisPackage()`).
    pub fn item_dependencies_to_this_package(&self) -> Vec<Dependency> {
        self.items()
            .iter()
            .flat_map(RustItem::direct_dependencies_to_self)
            .filter(|d| !self.contains_item(&d.origin_item()))
            .collect()
    }

    /// Dependencies from the module tree to outside of it
    /// (`JavaPackage.getClassDependenciesFromThisPackageTree()`).
    pub fn item_dependencies_from_this_package_tree(&self) -> Vec<Dependency> {
        self.items_in_package_tree()
            .iter()
            .flat_map(RustItem::direct_dependencies_from_self)
            .filter(|d| !self.in_tree(d.target_item().id))
            .collect()
    }

    /// Dependencies from outside into the module tree
    /// (`JavaPackage.getClassDependenciesToThisPackageTree()`).
    pub fn item_dependencies_to_this_package_tree(&self) -> Vec<Dependency> {
        self.items_in_package_tree()
            .iter()
            .flat_map(RustItem::direct_dependencies_to_self)
            .filter(|d| !self.in_tree(d.origin_item().id))
            .collect()
    }

    fn packages_of<'a>(&self, items: impl Iterator<Item = &'a RustItem>) -> Vec<RustModule> {
        let mut seen = HashSet::new();
        items
            .map(RustItem::package)
            .filter(|m| seen.insert(m.id))
            .collect()
    }

    /// `JavaPackage.getPackageDependenciesFromThisPackage()`.
    pub fn package_dependencies_from_this_package(&self) -> Vec<RustModule> {
        let targets: Vec<RustItem> = self
            .item_dependencies_from_this_package()
            .iter()
            .map(Dependency::target_item)
            .collect();
        self.packages_of(targets.iter())
    }

    /// `JavaPackage.getPackageDependenciesToThisPackage()`.
    pub fn package_dependencies_to_this_package(&self) -> Vec<RustModule> {
        let origins: Vec<RustItem> = self
            .item_dependencies_to_this_package()
            .iter()
            .map(Dependency::origin_item)
            .collect();
        self.packages_of(origins.iter())
    }

    /// `JavaPackage.getPackageDependenciesFromThisPackageTree()`.
    pub fn package_dependencies_from_this_package_tree(&self) -> Vec<RustModule> {
        let targets: Vec<RustItem> = self
            .item_dependencies_from_this_package_tree()
            .iter()
            .map(Dependency::target_item)
            .collect();
        self.packages_of(targets.iter())
    }

    /// `JavaPackage.getPackageDependenciesToThisPackageTree()`.
    pub fn package_dependencies_to_this_package_tree(&self) -> Vec<RustModule> {
        let origins: Vec<RustItem> = self
            .item_dependencies_to_this_package_tree()
            .iter()
            .map(Dependency::origin_item)
            .collect();
        self.packages_of(origins.iter())
    }

    /// Inner attributes of the module (`JavaPackage.getAnnotations()` via `package-info`).
    pub fn annotations(&self) -> Vec<RustAnnotation> {
        CanBeAnnotated::annotations(self)
    }

    /// Visits this module and all submodules satisfying `predicate`
    /// (`JavaPackage.traversePackageTree(..)` with a `PackageVisitor`).
    pub fn traverse_package_tree(
        &self,
        predicate: &DescribedPredicate<RustModule>,
        visitor: &mut dyn FnMut(&RustModule),
    ) {
        if predicate.test(self) {
            visitor(self);
        }
        for sub in self.subpackages() {
            sub.traverse_package_tree(predicate, visitor);
        }
    }

    /// Visits every item in the module tree satisfying `predicate`
    /// (`JavaPackage.traversePackageTree(..)` with a `ClassVisitor`).
    pub fn traverse_items_in_package_tree(
        &self,
        predicate: &DescribedPredicate<RustItem>,
        visitor: &mut dyn FnMut(&RustItem),
    ) {
        for item in self.items_in_package_tree() {
            if predicate.test(&item) {
                visitor(&item);
            }
        }
    }
}

/// `JavaPackage.Predicates` and `Functions`.
pub mod predicates {
    use super::*;

    /// Described as `name '<name>'`.
    pub fn name(name: &str) -> DescribedPredicate<RustModule> {
        super::super::properties::has_name::predicates::name(name)
    }

    /// Described as `matches '<identifier>'`.
    pub fn matches(package_identifier: &str) -> DescribedPredicate<RustModule> {
        let matcher = super::super::PackageMatcher::of(package_identifier);
        DescribedPredicate::describe(
            format!("matches '{package_identifier}'"),
            move |m: &RustModule| matcher.matches(&m.name()),
        )
    }
}
