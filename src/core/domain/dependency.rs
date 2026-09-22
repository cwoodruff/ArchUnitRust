use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::graph::{DependencyId, Graph};
use super::properties::{HasSourceCodeLocation, ItemSelector};
use super::rust_item::RustItem;
use super::rust_items::RustItems;
use super::source_code_location::SourceCodeLocation;
use crate::base::{DescribedPredicate, HasDescription};

/// Why one item depends on another (Rust-only; ArchUnit keeps this in the description).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DependencyKind {
    /// A method or constructor call, field access, or function reference.
    Access,
    /// A field, const or static type.
    FieldType,
    /// A parameter type.
    ParameterType,
    /// A return type.
    ReturnType,
    /// The error type of a `Result`.
    ErrorType,
    /// A generic type argument or bound.
    Generic,
    /// `impl Trait for Type`, or a derive.
    Implements,
    /// A supertrait.
    Extends,
    /// An attribute or derive.
    Annotation,
    /// A type alias target.
    Alias,
    /// A `use` import.
    Import,
    /// A path expression naming a const, static, or item (e.g. `as` casts, turbofish).
    Reference,
    /// A macro invocation whose body could not be analysed.
    MacroInvocation,
    /// A trait object or `impl Trait` mention.
    TraitReference,
}

/// A dependency of one item on another (`Dependency`).
///
/// Displays as `<origin description> <verb> <<target>> in (<file>:<line>)`.
#[derive(Clone)]
pub struct Dependency {
    graph: Arc<Graph>,
    id: DependencyId,
}

impl PartialEq for Dependency {
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = (self.data(), other.data());
        a.origin == b.origin
            && a.target == b.target
            && a.description == b.description
            && a.location == b.location
    }
}

impl Eq for Dependency {}

impl Hash for Dependency {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data().description.hash(state);
    }
}

impl PartialOrd for Dependency {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Dependency {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.data().description.cmp(&other.data().description)
    }
}

impl fmt::Debug for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Dependency({})", self.data().description)
    }
}

impl fmt::Display for Dependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.data().description)
    }
}

impl HasDescription for Dependency {
    fn description(&self) -> String {
        self.data().description.clone()
    }
}

impl HasSourceCodeLocation for Dependency {
    fn source_code_location(&self) -> SourceCodeLocation {
        self.data().location.clone()
    }
}

impl Dependency {
    pub(crate) fn new(graph: Arc<Graph>, id: DependencyId) -> Self {
        Self { graph, id }
    }

    fn data(&self) -> &super::graph::DependencyData {
        &self.graph.dependencies[self.id]
    }

    /// The item the dependency originates from (`Dependency.getOriginClass()`).
    pub fn origin_item(&self) -> RustItem {
        RustItem::new(Arc::clone(&self.graph), self.data().origin)
    }

    /// The item depended upon (`Dependency.getTargetClass()`).
    pub fn target_item(&self) -> RustItem {
        RustItem::new(Arc::clone(&self.graph), self.data().target)
    }

    /// The full description line (`Dependency.getDescription()`).
    pub fn description(&self) -> String {
        self.data().description.clone()
    }

    /// `Dependency.getSourceCodeLocation()`.
    pub fn source_code_location(&self) -> SourceCodeLocation {
        self.data().location.clone()
    }

    /// Why the dependency exists.
    /// For a macro invocation: the number of top-level arguments passed to the macro, if the
    /// invocation could be parsed as a comma-separated expression list `[rust-only]`.
    pub fn macro_argument_count(&self) -> Option<usize> {
        self.data().macro_argument_count
    }

    pub fn kind(&self) -> DependencyKind {
        self.data().kind
    }

    /// The distinct target items of the given dependencies as [`RustItems`]
    /// (`Dependency.toTargetClasses(..)`).
    pub fn to_target_items<'a>(
        dependencies: impl IntoIterator<Item = &'a Dependency>,
    ) -> Option<RustItems> {
        let mut graph = None;
        let mut ids = Vec::new();
        for dep in dependencies {
            graph.get_or_insert_with(|| Arc::clone(&dep.graph));
            if !ids.contains(&dep.data().target) {
                ids.push(dep.data().target);
            }
        }
        graph.map(|g| RustItems::from_ids(g, ids, "target classes".to_owned()))
    }
}

/// `Dependency.Predicates`.
pub mod predicates {
    use super::*;

    /// Described as `dependency <origin> -> <target>`.
    pub fn dependency(
        origin: impl Into<ItemSelector>,
        target: impl Into<ItemSelector>,
    ) -> DescribedPredicate<Dependency> {
        let (origin, target) = (origin.into(), target.into());
        let description = format!(
            "dependency {} -> {}",
            origin.description(),
            target.description()
        );
        DescribedPredicate::describe(description, move |d: &Dependency| {
            origin.matches(&d.origin_item()) && target.matches(&d.target_item())
        })
    }

    /// Described as `origin <name or predicate>`.
    pub fn dependency_origin(selector: impl Into<ItemSelector>) -> DescribedPredicate<Dependency> {
        let selector = selector.into();
        let description = format!("origin {}", selector.description());
        DescribedPredicate::describe(description, move |d: &Dependency| {
            selector.matches(&d.origin_item())
        })
    }

    /// Described as `target <name or predicate>`.
    pub fn dependency_target(selector: impl Into<ItemSelector>) -> DescribedPredicate<Dependency> {
        let selector = selector.into();
        let description = format!("target {}", selector.description());
        DescribedPredicate::describe(description, move |d: &Dependency| {
            selector.matches(&d.target_item())
        })
    }
}

/// `Dependency.Functions`.
pub mod functions {
    use super::*;
    use crate::base::DescribedFunction;

    /// `GET_ORIGIN_CLASS`.
    pub fn get_origin_item() -> DescribedFunction<Dependency, RustItem> {
        DescribedFunction::describe("origin class", Dependency::origin_item)
    }

    /// `GET_TARGET_CLASS`.
    pub fn get_target_item() -> DescribedFunction<Dependency, RustItem> {
        DescribedFunction::describe("target class", Dependency::target_item)
    }
}
