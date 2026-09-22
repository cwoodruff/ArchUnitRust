use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::graph::{AccessId, Graph};
use super::properties::{HasName, HasOwner, HasSourceCodeLocation, ItemSelector};
use super::rust_item::RustItem;
use super::rust_member::{RustCodeUnit, RustMember};
use super::source_code_location::SourceCodeLocation;
use crate::base::{DescribedPredicate, HasDescription};

/// Whether a field is read or written (`JavaFieldAccess.AccessType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessType {
    /// A read.
    Get,
    /// A write: assignment target or `&mut` borrow.
    Set,
}

impl fmt::Display for AccessType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AccessType::Get => "GET",
            AccessType::Set => "SET",
        })
    }
}

/// The kind of an access (`JavaFieldAccess`, `JavaMethodCall`, `JavaConstructorCall`,
/// `JavaCodeUnitReference`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessKind {
    /// Reading or writing a field.
    FieldAccess(AccessType),
    /// Calling a method, associated function, or free function.
    MethodCall,
    /// Calling a constructor, or constructing with a struct literal / tuple struct / variant.
    ConstructorCall,
    /// Using a function as a value.
    FunctionReference,
}

impl AccessKind {
    /// Method or constructor call.
    pub fn is_call(self) -> bool {
        matches!(self, AccessKind::MethodCall | AccessKind::ConstructorCall)
    }
}

/// How well the access target could be resolved from source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolutionKind {
    /// The owner and member are known.
    Resolved,
    /// The owner is known but the member is not part of the import (e.g. `Vec::push`).
    OwnerOnly,
    /// The receiver type was unknown; the member was found by its unique name.
    ByNameOnly,
    /// Neither owner nor member could be determined.
    Unresolved,
}

/// A field access, method call, constructor call, or function reference from a code unit to a
/// member (`JavaAccess`).
#[derive(Clone)]
pub struct RustAccess {
    graph: Arc<Graph>,
    id: AccessId,
}

impl PartialEq for RustAccess {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && Arc::ptr_eq(&self.graph, &other.graph)
    }
}

impl Eq for RustAccess {}

impl Hash for RustAccess {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl fmt::Debug for RustAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RustAccess({})", self.description())
    }
}

impl fmt::Display for RustAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.description())
    }
}

impl HasDescription for RustAccess {
    fn description(&self) -> String {
        format!(
            "{} {} <{}> in {}",
            self.origin().description(),
            self.description_verb(),
            self.data().target_full_name,
            self.data().location
        )
    }
}

impl HasName for RustAccess {
    fn name(&self) -> String {
        self.data().target_name.clone()
    }
}

impl HasOwner<RustCodeUnit> for RustAccess {
    fn owner(&self) -> RustCodeUnit {
        self.origin()
    }
}

impl HasSourceCodeLocation for RustAccess {
    fn source_code_location(&self) -> SourceCodeLocation {
        self.data().location.clone()
    }
}

impl RustAccess {
    pub(crate) fn new(graph: Arc<Graph>, id: AccessId) -> Self {
        Self { graph, id }
    }

    fn data(&self) -> &super::graph::AccessData {
        &self.graph.accesses[self.id]
    }

    /// The kind of access.
    pub fn kind(&self) -> AccessKind {
        self.data().kind
    }

    /// For field accesses: read or write.
    pub fn access_type(&self) -> Option<AccessType> {
        match self.kind() {
            AccessKind::FieldAccess(t) => Some(t),
            _ => None,
        }
    }

    /// The code unit containing the access (`JavaAccess.getOrigin()`).
    pub fn origin(&self) -> RustCodeUnit {
        RustCodeUnit(RustMember::new(Arc::clone(&self.graph), self.data().origin))
    }

    /// The item declaring the origin code unit (`JavaAccess.getOriginOwner()`).
    pub fn origin_owner(&self) -> RustItem {
        self.origin().owner()
    }

    /// The target (`JavaAccess.getTarget()`).
    pub fn target(&self) -> AccessTarget {
        AccessTarget {
            graph: Arc::clone(&self.graph),
            access: self.id,
        }
    }

    /// The item declaring the target (`JavaAccess.getTargetOwner()`).
    pub fn target_owner(&self) -> RustItem {
        RustItem::new(Arc::clone(&self.graph), self.data().target_owner)
    }

    /// The target member name.
    pub fn name(&self) -> String {
        HasName::name(self)
    }

    /// `JavaAccess.getLineNumber()`.
    pub fn line_number(&self) -> usize {
        self.data().location.line_number()
    }

    /// `JavaAccess.getSourceCodeLocation()`.
    pub fn source_code_location(&self) -> SourceCodeLocation {
        self.data().location.clone()
    }

    /// Whether the access is inside a closure (`JavaAccess.isDeclaredInLambda()`).
    pub fn is_declared_in_closure(&self) -> bool {
        self.data().in_closure
    }

    /// How the target was resolved.
    pub fn resolution(&self) -> ResolutionKind {
        self.data().resolution
    }

    /// `<origin> <verb> <target> in <location>`, e.g.
    /// `Method <a::B::c()> calls method <d::E::f()> in (src/a.rs:12)`.
    pub fn description(&self) -> String {
        HasDescription::description(self)
    }

    pub(crate) fn description_verb(&self) -> &'static str {
        match self.kind() {
            AccessKind::FieldAccess(AccessType::Get) => "gets field",
            AccessKind::FieldAccess(AccessType::Set) => "sets field",
            AccessKind::MethodCall => "calls method",
            AccessKind::ConstructorCall => "calls constructor",
            AccessKind::FunctionReference => {
                if self
                    .target()
                    .resolve_member()
                    .is_some_and(|m| m.is_constructor())
                {
                    "references constructor"
                } else {
                    "references method"
                }
            }
        }
    }
}

/// The target of an access (`AccessTarget`).
///
/// A target is what the source names, not necessarily an imported member: it may be a method
/// of a stub type, or a member that could not be resolved. [`resolve`](Self::resolve) yields
/// the matching imported members.
#[derive(Clone)]
pub struct AccessTarget {
    graph: Arc<Graph>,
    access: AccessId,
}

impl PartialEq for AccessTarget {
    fn eq(&self, other: &Self) -> bool {
        self.full_name() == other.full_name()
    }
}

impl Eq for AccessTarget {}

impl Hash for AccessTarget {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.full_name().hash(state);
    }
}

impl fmt::Debug for AccessTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccessTarget({})", self.full_name())
    }
}

impl fmt::Display for AccessTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.full_name())
    }
}

impl HasDescription for AccessTarget {
    fn description(&self) -> String {
        format!("<{}>", self.full_name())
    }
}

impl HasName for AccessTarget {
    fn name(&self) -> String {
        self.data().target_name.clone()
    }
}

impl HasOwner<RustItem> for AccessTarget {
    fn owner(&self) -> RustItem {
        RustItem::new(Arc::clone(&self.graph), self.data().target_owner)
    }
}

impl AccessTarget {
    fn data(&self) -> &super::graph::AccessData {
        &self.graph.accesses[self.access]
    }

    /// The declaring item (`AccessTarget.getOwner()`).
    pub fn owner(&self) -> RustItem {
        HasOwner::owner(self)
    }

    /// The member name (`AccessTarget.getName()`).
    pub fn name(&self) -> String {
        HasName::name(self)
    }

    /// The full name, e.g. `my_app::Order::total()` (`AccessTarget.getFullName()`).
    pub fn full_name(&self) -> String {
        self.data().target_full_name.clone()
    }

    /// All imported members this target may refer to (`AccessTarget.resolve()`).
    pub fn resolve(&self) -> Vec<RustMember> {
        self.data()
            .resolved
            .iter()
            .map(|&id| RustMember::new(Arc::clone(&self.graph), id))
            .collect()
    }

    /// The single imported member this target refers to, if unambiguous
    /// (`AccessTarget.resolveMember()`).
    pub fn resolve_member(&self) -> Option<RustMember> {
        let resolved = self.resolve();
        (resolved.len() == 1).then(|| resolved.into_iter().next().expect("one element"))
    }

    /// Whether the target is a constructor.
    pub fn is_constructor(&self) -> bool {
        self.data().kind == AccessKind::ConstructorCall
            || self.resolve_member().is_some_and(|m| m.is_constructor())
    }
}

/// `JavaAccess.Predicates`.
pub mod predicates {
    use super::*;

    /// Described as `origin <predicate>`.
    pub fn origin(predicate: DescribedPredicate<RustCodeUnit>) -> DescribedPredicate<RustAccess> {
        let description = format!("origin {}", predicate.description());
        DescribedPredicate::describe(description, move |a: &RustAccess| {
            predicate.test(&a.origin())
        })
    }

    /// Described as `origin owner <predicate>`.
    pub fn origin_owner(predicate: DescribedPredicate<RustItem>) -> DescribedPredicate<RustAccess> {
        let description = format!("origin owner {}", predicate.description());
        DescribedPredicate::describe(description, move |a: &RustAccess| {
            predicate.test(&a.origin_owner())
        })
    }

    /// Described as `target <predicate>`.
    pub fn target(predicate: DescribedPredicate<AccessTarget>) -> DescribedPredicate<RustAccess> {
        let description = format!("target {}", predicate.description());
        DescribedPredicate::describe(description, move |a: &RustAccess| {
            predicate.test(&a.target())
        })
    }

    /// Described as `target owner <predicate>`.
    pub fn target_owner(predicate: DescribedPredicate<RustItem>) -> DescribedPredicate<RustAccess> {
        let description = format!("target owner {}", predicate.description());
        DescribedPredicate::describe(description, move |a: &RustAccess| {
            predicate.test(&a.target_owner())
        })
    }

    /// Described as `origin owner equals target owner`.
    pub fn origin_owner_equals_target_owner() -> DescribedPredicate<RustAccess> {
        DescribedPredicate::describe("origin owner equals target owner", |a: &RustAccess| {
            a.origin_owner() == a.target_owner()
        })
    }

    /// `JavaFieldAccess.Predicates.accessType(..)`; described as `access type GET`/`SET`.
    pub fn access_type(access_type: AccessType) -> DescribedPredicate<RustAccess> {
        DescribedPredicate::describe(
            format!("access type {access_type}"),
            move |a: &RustAccess| a.access_type() == Some(access_type),
        )
    }
}

/// `AccessTarget.Predicates`.
pub mod target_predicates {
    use super::*;

    /// Described as `declared in <name or predicate>`.
    pub fn declared_in(selector: impl Into<ItemSelector>) -> DescribedPredicate<AccessTarget> {
        let selector = selector.into();
        let description = format!("declared in {}", selector.description());
        DescribedPredicate::describe(description, move |t: &AccessTarget| {
            selector.matches(&t.owner())
        })
    }

    /// Described as `constructor`.
    pub fn constructor() -> DescribedPredicate<AccessTarget> {
        DescribedPredicate::describe("constructor", |t: &AccessTarget| t.is_constructor())
    }

    /// Described as `name '<name>'`.
    pub fn name(name: &str) -> DescribedPredicate<AccessTarget> {
        super::super::properties::has_name::predicates::name(name)
    }
}
