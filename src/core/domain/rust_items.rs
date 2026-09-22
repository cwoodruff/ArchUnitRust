use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use super::graph::{Graph, ItemId};
use super::rust_item::{ItemKind, RustItem};
use super::rust_module::RustModule;
use crate::base::{DescribedPredicate, HasDescription};

/// The imported items of one or more crates (`JavaClasses`).
///
/// Owns the code graph; every [`RustItem`] handed out shares it. Iteration yields the
/// "classes": every imported item except modules and `impl` blocks whose self type is itself an
/// imported item (their members belong to that type). Modules are reachable through
/// [`modules`](Self::modules) and [`package`](Self::package).
#[derive(Clone)]
pub struct RustItems {
    graph: Arc<Graph>,
    ids: Arc<Vec<ItemId>>,
    id_set: Arc<HashSet<ItemId>>,
    description: String,
}

impl fmt::Debug for RustItems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RustItems({}, {} items)",
            self.description,
            self.ids.len()
        )
    }
}

impl HasDescription for RustItems {
    fn description(&self) -> String {
        self.description.clone()
    }
}

impl RustItems {
    pub(crate) fn from_graph(graph: Graph, description: String) -> Self {
        let ids: Vec<ItemId> = (0..graph.items.len())
            .filter(|&id| graph.items[id].fully_imported && graph.is_class_like(id))
            .collect();
        Self::from_ids(Arc::new(graph), ids, description)
    }

    pub(crate) fn from_ids(graph: Arc<Graph>, ids: Vec<ItemId>, description: String) -> Self {
        let id_set = ids.iter().copied().collect();
        Self {
            graph,
            ids: Arc::new(ids),
            id_set: Arc::new(id_set),
            description,
        }
    }

    fn item(&self, id: ItemId) -> RustItem {
        RustItem::new(Arc::clone(&self.graph), id)
    }

    /// Iterates over the contained items.
    pub fn iter(&self) -> impl Iterator<Item = RustItem> + '_ {
        self.ids.iter().map(|&id| self.item(id))
    }

    /// The number of contained items.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether no items are contained.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The item with the given full name (`JavaClasses.get(..)`).
    ///
    /// # Panics
    /// If the item is not contained.
    pub fn get(&self, name: &str) -> RustItem {
        self.try_get(name)
            .unwrap_or_else(|| panic!("{} do not contain {name}", self.description))
    }

    /// The item with the given full name, if contained (`JavaClasses.get(..)` without the throw).
    ///
    /// Public re-export paths (`my_app::Order` for `pub use domain::model::Order`) are accepted too.
    pub fn try_get(&self, name: &str) -> Option<RustItem> {
        self.graph
            .find(name)
            .filter(|id| self.id_set.contains(id))
            .map(|id| self.item(id))
    }

    /// Any item of the graph by full name, including modules and stubs outside the import.
    pub fn try_get_any(&self, name: &str) -> Option<RustItem> {
        self.graph.find(name).map(|id| self.item(id))
    }

    /// `JavaClasses.contain(..)`.
    pub fn contain(&self, name: &str) -> bool {
        self.try_get(name).is_some()
    }

    /// `JavaClasses.containPackage(..)`.
    pub fn contain_package(&self, name: &str) -> bool {
        self.try_package(name).is_some()
    }

    /// Restricts to items satisfying `predicate` (`JavaClasses.that(..)`).
    ///
    /// The description becomes `<description> that <predicate description>`.
    pub fn that(&self, predicate: &DescribedPredicate<RustItem>) -> RustItems {
        let ids: Vec<ItemId> = self
            .iter()
            .filter(|i| predicate.test(i))
            .map(|i| i.id)
            .collect();
        let description = format!("{} that {}", self.description, predicate.description());
        RustItems::from_ids(Arc::clone(&self.graph), ids, description)
    }

    /// Overrides the description (`JavaClasses.as(..)`).
    pub fn as_(&self, description: &str) -> RustItems {
        let mut copy = self.clone();
        copy.description = description.to_owned();
        copy
    }

    /// The description, e.g. `classes` (`JavaClasses.getDescription()`).
    pub fn description(&self) -> String {
        self.description.clone()
    }

    /// The crate root modules (`JavaClasses.getDefaultPackage()` returns the roots of the
    /// package tree; Rust has one per crate).
    pub fn default_packages(&self) -> Vec<RustModule> {
        self.graph
            .crate_roots
            .iter()
            .map(|&id| RustModule::new(Arc::clone(&self.graph), id))
            .collect()
    }

    /// The module with the given full path (`JavaClasses.getPackage(..)`).
    ///
    /// # Panics
    /// If there is no such module.
    pub fn package(&self, name: &str) -> RustModule {
        self.try_package(name)
            .unwrap_or_else(|| panic!("{} do not contain module {name}", self.description))
    }

    /// The module with the given full path, if any.
    pub fn try_package(&self, name: &str) -> Option<RustModule> {
        self.graph
            .by_name
            .get(name)
            .filter(|&&id| {
                self.graph.items[id].kind == ItemKind::Module && self.graph.items[id].fully_imported
            })
            .map(|&id| RustModule::new(Arc::clone(&self.graph), id))
    }

    /// Every imported module.
    pub fn modules(&self) -> Vec<RustModule> {
        self.graph
            .items
            .iter()
            .enumerate()
            .filter(|(_, d)| d.kind == ItemKind::Module && d.fully_imported)
            .map(|(id, _)| RustModule::new(Arc::clone(&self.graph), id))
            .collect()
    }

    /// Every item in the graph, including modules, hidden impl blocks and stubs.
    pub fn all_items_in_graph(&self) -> Vec<RustItem> {
        (0..self.graph.items.len())
            .map(|id| self.item(id))
            .collect()
    }

    /// The contained items as a `Vec`.
    pub fn to_vec(&self) -> Vec<RustItem> {
        self.iter().collect()
    }
}

impl<'a> IntoIterator for &'a RustItems {
    type Item = RustItem;
    type IntoIter = Box<dyn Iterator<Item = RustItem> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}
