use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use super::annotation::RustAnnotation;
use super::graph::{Graph, ItemId, MemberId};
use super::modifiers::{RustModifier, Visibility};
use super::properties::{
    CanBeAnnotated, HasErrorTypes, HasFullName, HasModifiers, HasName, HasOwner, HasParameterTypes,
    HasReturnType, HasSourceCodeLocation, HasType, HasTypeParameters, ItemSelector,
};
use super::rust_access::{AccessKind, RustAccess};
use super::rust_item::RustItem;
use super::source_code_location::SourceCodeLocation;
use super::types::{RustType, TypeParameter};
use crate::base::{DescribedPredicate, HasDescription};

/// The kind of a [`RustMember`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemberKind {
    /// A struct, variant or union field.
    Field,
    /// An associated function, trait method, or free function.
    Method,
    /// An enum variant.
    Variant,
    /// An associated `const`.
    AssocConst,
    /// An associated `type`.
    AssocType,
}

/// The `self` receiver of a method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Receiver {
    /// `self`
    Value,
    /// `&self`
    Ref,
    /// `&mut self`
    MutRef,
}

/// A member of an item: field, method, variant, associated const or type (`JavaMember`).
#[derive(Clone)]
pub struct RustMember {
    pub(crate) graph: Arc<Graph>,
    pub(crate) id: MemberId,
}

impl PartialEq for RustMember {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && Arc::ptr_eq(&self.graph, &other.graph)
    }
}

impl Eq for RustMember {}

impl Hash for RustMember {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl fmt::Debug for RustMember {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RustMember({:?} {})", self.kind(), self.full_name())
    }
}

impl fmt::Display for RustMember {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.full_name())
    }
}

impl HasDescription for RustMember {
    fn description(&self) -> String {
        let prefix = match self.kind() {
            MemberKind::Field | MemberKind::AssocConst => "Field",
            MemberKind::Variant => "Variant",
            MemberKind::AssocType => "Type",
            MemberKind::Method if self.is_constructor() => "Constructor",
            MemberKind::Method if self.is_free_function() => "Function",
            MemberKind::Method => "Method",
        };
        format!("{prefix} <{}>", self.full_name())
    }
}

impl HasName for RustMember {
    fn name(&self) -> String {
        self.data().name.clone()
    }
}

impl HasFullName for RustMember {
    fn full_name(&self) -> String {
        self.data().full_name.clone()
    }
}

impl HasModifiers for RustMember {
    fn modifiers(&self) -> Vec<RustModifier> {
        self.data().modifiers.clone()
    }
}

impl CanBeAnnotated for RustMember {
    fn annotations(&self) -> Vec<RustAnnotation> {
        self.data().annotations.clone()
    }
}

impl HasOwner<RustItem> for RustMember {
    fn owner(&self) -> RustItem {
        self.item(self.data().owner)
    }
}

impl HasSourceCodeLocation for RustMember {
    fn source_code_location(&self) -> SourceCodeLocation {
        self.data().location.clone()
    }
}

impl HasType for RustMember {
    fn type_(&self) -> Option<RustType> {
        self.data().type_.clone()
    }

    fn raw_type(&self) -> Option<RustItem> {
        self.raw_type_id().map(|id| self.item(id))
    }
}

impl HasReturnType for RustMember {
    fn return_type(&self) -> RustType {
        self.data()
            .return_type
            .clone()
            .unwrap_or_else(RustType::unit)
    }

    fn raw_return_type(&self) -> Option<RustItem> {
        self.raw_return_type_id().map(|id| self.item(id))
    }
}

impl HasParameterTypes for RustMember {
    fn parameter_types(&self) -> Vec<RustType> {
        self.data()
            .parameters
            .iter()
            .map(|p| p.type_.clone())
            .collect()
    }

    fn raw_parameter_types(&self) -> Vec<Option<RustItem>> {
        self.data()
            .parameters
            .iter()
            .map(|p| self.graph.erased(&p.type_).map(|id| self.item(id)))
            .collect()
    }
}

impl HasErrorTypes for RustMember {
    fn error_types(&self) -> Vec<RustItem> {
        self.error_type_id()
            .map(|id| self.item(id))
            .into_iter()
            .collect()
    }
}

impl HasTypeParameters for RustMember {
    fn type_parameters(&self) -> Vec<TypeParameter> {
        self.data().type_parameters.clone()
    }
}

impl RustMember {
    pub(crate) fn new(graph: Arc<Graph>, id: MemberId) -> Self {
        Self { graph, id }
    }

    pub(crate) fn data(&self) -> &super::graph::MemberData {
        &self.graph.members[self.id]
    }

    fn item(&self, id: ItemId) -> RustItem {
        RustItem::new(Arc::clone(&self.graph), id)
    }

    pub(crate) fn raw_type_id(&self) -> Option<ItemId> {
        self.data()
            .type_
            .as_ref()
            .and_then(|t| self.graph.erased(t))
    }

    pub(crate) fn raw_return_type_id(&self) -> Option<ItemId> {
        self.data()
            .return_type
            .as_ref()
            .and_then(|t| self.graph.erased(t))
    }

    pub(crate) fn error_type_id(&self) -> Option<ItemId> {
        self.data()
            .error_type
            .as_ref()
            .and_then(|t| self.graph.erased(t))
    }

    pub(crate) fn has_parameter_of_type(&self, id: ItemId) -> bool {
        self.data()
            .parameters
            .iter()
            .any(|p| self.graph.erased(&p.type_) == Some(id))
    }

    /// The kind of member.
    pub fn kind(&self) -> MemberKind {
        self.data().kind
    }

    /// The declaring item (`JavaMember.getOwner()`); for free functions the function item itself.
    pub fn owner(&self) -> RustItem {
        HasOwner::owner(self)
    }

    /// The simple name, e.g. `total` (`JavaMember.getName()`).
    pub fn name(&self) -> String {
        HasName::name(self)
    }

    /// The full name, e.g. `my_app::Order::total()` for methods (with parameter types) or
    /// `my_app::Order::items` for fields (`JavaMember.getFullName()`).
    pub fn full_name(&self) -> String {
        HasFullName::full_name(self)
    }

    /// `Method <..>`, `Field <..>`, `Constructor <..>`, `Function <..>` or `Variant <..>`.
    pub fn description(&self) -> String {
        HasDescription::description(self)
    }

    /// The visibility as written.
    pub fn visibility(&self) -> Visibility {
        self.data().visibility.clone()
    }

    /// `JavaMember.getModifiers()`.
    pub fn modifiers(&self) -> Vec<RustModifier> {
        HasModifiers::modifiers(self)
    }

    /// `JavaMember.getAnnotations()`.
    pub fn annotations(&self) -> Vec<RustAnnotation> {
        CanBeAnnotated::annotations(self)
    }

    /// `JavaMember.isAnnotatedWith(..)`.
    pub fn is_annotated_with(
        &self,
        selector: impl Into<super::properties::AnnotationSelector>,
    ) -> bool {
        CanBeAnnotated::is_annotated_with(self, selector)
    }

    /// `JavaMember.getAnnotationOfType(..)`; panics if absent.
    pub fn annotation_of_type(&self, name: &str) -> RustAnnotation {
        CanBeAnnotated::annotation_of_type(self, name)
    }

    /// `JavaMember.tryGetAnnotationOfType(..)`.
    pub fn try_annotation_of_type(&self, name: &str) -> Option<RustAnnotation> {
        CanBeAnnotated::try_annotation_of_type(self, name)
    }

    /// `JavaMember.getSourceCodeLocation()`.
    pub fn source_code_location(&self) -> SourceCodeLocation {
        HasSourceCodeLocation::source_code_location(self)
    }

    /// Whether the member is test code.
    pub fn is_test_code(&self) -> bool {
        self.data().is_test_code
    }

    /// The lines of `unsafe { .. }` blocks in the body, for code units `[rust-only]`.
    pub fn unsafe_block_lines(&self) -> Vec<usize> {
        self.data().unsafe_block_lines.clone()
    }

    /// Whether this is a method, associated function, or free function.
    pub fn is_code_unit(&self) -> bool {
        self.kind() == MemberKind::Method
    }

    /// Whether this is a free function standing for its item.
    pub fn is_free_function(&self) -> bool {
        self.data().item.is_some()
    }

    /// Whether this code unit is classified as a constructor.
    pub fn is_constructor(&self) -> bool {
        self.data().is_constructor
    }

    /// Whether this is a method (a code unit that is not a constructor) (`JavaCodeUnit.isMethod()`).
    pub fn is_method(&self) -> bool {
        self.is_code_unit() && !self.is_constructor()
    }

    /// Whether this is a trait method declaration (as opposed to an impl).
    pub fn is_trait_declaration(&self) -> bool {
        self.data().is_trait_declaration
    }

    /// Whether the code unit has a body (trait declarations may not).
    pub fn has_body(&self) -> bool {
        self.data().has_body
    }

    /// The `self` receiver of a method, if any.
    pub fn receiver(&self) -> Option<Receiver> {
        self.data().receiver
    }

    /// The `impl` block declaring this member, if any.
    pub fn declaring_impl(&self) -> Option<RustItem> {
        self.data().declaring_impl.map(|id| self.item(id))
    }

    /// The trait this member implements, when declared in a trait impl.
    pub fn implemented_trait(&self) -> Option<RustItem> {
        self.data().implemented_trait.map(|id| self.item(id))
    }

    /// The declared type of a field or associated const (`JavaField.getType()`).
    pub fn type_(&self) -> Option<RustType> {
        HasType::type_(self)
    }

    /// The erased type as an item (`JavaField.getRawType()`).
    pub fn raw_type(&self) -> Option<RustItem> {
        HasType::raw_type(self)
    }

    /// Parameters of a code unit (`JavaCodeUnit.getParameters()`).
    pub fn parameters(&self) -> Vec<RustParameter> {
        (0..self.data().parameters.len())
            .map(|index| RustParameter {
                member: self.clone(),
                index,
            })
            .collect()
    }

    /// `JavaCodeUnit.getParameterTypes()`.
    pub fn parameter_types(&self) -> Vec<RustType> {
        HasParameterTypes::parameter_types(self)
    }

    /// `JavaCodeUnit.getRawParameterTypes()`.
    pub fn raw_parameter_types(&self) -> Vec<Option<RustItem>> {
        HasParameterTypes::raw_parameter_types(self)
    }

    /// `JavaCodeUnit.getReturnType()`; the unit type when omitted.
    pub fn return_type(&self) -> RustType {
        HasReturnType::return_type(self)
    }

    /// The error type `E` of a `Result<_, E>` return type, as written `[rust-only]`.
    pub fn error_type(&self) -> Option<RustType> {
        self.data().error_type.clone()
    }

    /// The item a type written in this member's signature refers to, e.g. `std::boxed::Box`
    /// for `Box<dyn Error>` `[rust-only]`.
    pub fn resolve_type(&self, type_: &RustType) -> Option<RustItem> {
        type_.raw_item().map(|id| self.item(id))
    }

    /// `JavaCodeUnit.getRawReturnType()`.
    pub fn raw_return_type(&self) -> Option<RustItem> {
        HasReturnType::raw_return_type(self)
    }

    /// The error type `E` of a `Result<_, E>` return type (`JavaCodeUnit.getExceptionTypes()`).
    pub fn error_types(&self) -> Vec<RustItem> {
        HasErrorTypes::error_types(self)
    }

    /// Generic type parameters of the member.
    pub fn type_parameters(&self) -> Vec<TypeParameter> {
        HasTypeParameters::type_parameters(self)
    }

    /// Accesses targeting this member (`JavaMember.getAccessesToSelf()`).
    pub fn accesses_to_self(&self) -> Vec<RustAccess> {
        self.graph
            .accesses_to_member
            .get(self.id)
            .map(|ids| {
                ids.iter()
                    .map(|&id| RustAccess::new(Arc::clone(&self.graph), id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Accesses from this code unit (`JavaCodeUnit.getAccessesFromSelf()`).
    pub fn accesses_from_self(&self) -> Vec<RustAccess> {
        self.graph
            .accesses_from_member
            .get(self.id)
            .map(|ids| {
                ids.iter()
                    .map(|&id| RustAccess::new(Arc::clone(&self.graph), id))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn accesses_of_kind(&self, pred: impl Fn(AccessKind) -> bool) -> Vec<RustAccess> {
        self.accesses_from_self()
            .into_iter()
            .filter(|a| pred(a.kind()))
            .collect()
    }

    /// `JavaCodeUnit.getCallsFromSelf()`.
    pub fn calls_from_self(&self) -> Vec<RustAccess> {
        self.accesses_of_kind(AccessKind::is_call)
    }

    /// `JavaCodeUnit.getMethodCallsFromSelf()`.
    pub fn method_calls_from_self(&self) -> Vec<RustAccess> {
        self.accesses_of_kind(|k| k == AccessKind::MethodCall)
    }

    /// `JavaCodeUnit.getConstructorCallsFromSelf()`.
    pub fn constructor_calls_from_self(&self) -> Vec<RustAccess> {
        self.accesses_of_kind(|k| k == AccessKind::ConstructorCall)
    }

    /// `JavaCodeUnit.getFieldAccesses()`.
    pub fn field_accesses(&self) -> Vec<RustAccess> {
        self.accesses_of_kind(|k| matches!(k, AccessKind::FieldAccess(_)))
    }

    /// `JavaCodeUnit.getCodeUnitReferencesFromSelf()`.
    pub fn function_references_from_self(&self) -> Vec<RustAccess> {
        self.accesses_of_kind(|k| k == AccessKind::FunctionReference)
    }

    /// Calls targeting this code unit (`JavaCodeUnit.getCallsOfSelf()`).
    pub fn calls_of_self(&self) -> Vec<RustAccess> {
        self.accesses_to_self()
            .into_iter()
            .filter(|a| a.kind().is_call())
            .collect()
    }

    /// Every item mentioned in the signature (`JavaMember.getAllInvolvedRawTypes()`).
    pub fn all_involved_raw_types(&self) -> Vec<RustItem> {
        let data = self.data();
        let mut ids: Vec<ItemId> = Vec::new();
        if let Some(t) = &data.type_ {
            ids.extend(t.all_items());
        }
        for p in &data.parameters {
            ids.extend(p.type_.all_items());
        }
        if let Some(t) = &data.return_type {
            ids.extend(t.all_items());
        }
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter().map(|id| self.item(id)).collect()
    }

    /// This member as a field, if it is one.
    pub fn as_field(&self) -> Option<RustField> {
        matches!(self.kind(), MemberKind::Field | MemberKind::AssocConst)
            .then(|| RustField(self.clone()))
    }

    /// This member as a code unit, if it is one.
    pub fn as_code_unit(&self) -> Option<RustCodeUnit> {
        self.is_code_unit().then(|| RustCodeUnit(self.clone()))
    }

    /// This member as a method, if it is one.
    pub fn as_method(&self) -> Option<RustMethod> {
        self.is_code_unit().then(|| RustMethod(self.clone()))
    }

    /// This member as a constructor, if it is one.
    pub fn as_constructor(&self) -> Option<RustConstructor> {
        self.is_constructor().then(|| RustConstructor(self.clone()))
    }
}

macro_rules! member_view {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub(crate) RustMember);

        impl Deref for $name {
            type Target = RustMember;
            fn deref(&self) -> &RustMember {
                &self.0
            }
        }

        impl From<$name> for RustMember {
            fn from(view: $name) -> RustMember {
                view.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0.full_name())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0.full_name())
            }
        }

        impl HasDescription for $name {
            fn description(&self) -> String {
                HasDescription::description(&self.0)
            }
        }

        impl HasName for $name {
            fn name(&self) -> String {
                HasName::name(&self.0)
            }
        }

        impl HasFullName for $name {
            fn full_name(&self) -> String {
                HasFullName::full_name(&self.0)
            }
        }

        impl HasModifiers for $name {
            fn modifiers(&self) -> Vec<RustModifier> {
                HasModifiers::modifiers(&self.0)
            }
        }

        impl CanBeAnnotated for $name {
            fn annotations(&self) -> Vec<RustAnnotation> {
                CanBeAnnotated::annotations(&self.0)
            }
        }

        impl HasOwner<RustItem> for $name {
            fn owner(&self) -> RustItem {
                HasOwner::owner(&self.0)
            }
        }

        impl HasSourceCodeLocation for $name {
            fn source_code_location(&self) -> SourceCodeLocation {
                HasSourceCodeLocation::source_code_location(&self.0)
            }
        }

        impl HasType for $name {
            fn type_(&self) -> Option<RustType> {
                HasType::type_(&self.0)
            }
            fn raw_type(&self) -> Option<RustItem> {
                HasType::raw_type(&self.0)
            }
        }

        impl HasReturnType for $name {
            fn return_type(&self) -> RustType {
                HasReturnType::return_type(&self.0)
            }
            fn raw_return_type(&self) -> Option<RustItem> {
                HasReturnType::raw_return_type(&self.0)
            }
        }

        impl HasParameterTypes for $name {
            fn parameter_types(&self) -> Vec<RustType> {
                HasParameterTypes::parameter_types(&self.0)
            }
            fn raw_parameter_types(&self) -> Vec<Option<RustItem>> {
                HasParameterTypes::raw_parameter_types(&self.0)
            }
        }

        impl HasErrorTypes for $name {
            fn error_types(&self) -> Vec<RustItem> {
                HasErrorTypes::error_types(&self.0)
            }
        }

        impl HasTypeParameters for $name {
            fn type_parameters(&self) -> Vec<TypeParameter> {
                HasTypeParameters::type_parameters(&self.0)
            }
        }
    };
}

member_view!(
    /// A struct, variant or union field, or an associated const (`JavaField`).
    RustField
);
member_view!(
    /// Anything with a body: a method, associated function, or free function (`JavaCodeUnit`).
    RustCodeUnit
);
member_view!(
    /// A method, associated function, or free function (`JavaMethod`).
    RustMethod
);
member_view!(
    /// An associated function classified as a constructor (`JavaConstructor`).
    RustConstructor
);
member_view!(
    /// An enum variant (`JavaEnumConstant`).
    RustVariant
);

/// Member kinds the rule syntax can be about: `members()`, `fields()`, `code_units()`,
/// `methods()`, `constructors()`.
pub trait MemberLike:
    Clone
    + fmt::Debug
    + HasDescription
    + HasName
    + HasFullName
    + HasModifiers
    + CanBeAnnotated
    + HasOwner<RustItem>
    + HasSourceCodeLocation
    + HasType
    + HasReturnType
    + HasParameterTypes
    + HasErrorTypes
    + crate::lang::AsCorrespondingObject
    + Send
    + Sync
    + 'static
{
    /// The plural used in rule texts, e.g. `fields`.
    fn plural() -> &'static str;
    /// The members of this kind declared by `item`.
    fn members_of(item: &RustItem) -> Vec<Self>;
    /// The underlying member.
    fn as_member(&self) -> &RustMember;
}

/// Member kinds that have a body and can be called: `code_units()`, `methods()`, `constructors()`.
pub trait CodeUnitLike: MemberLike {
    /// Calls targeting this code unit.
    fn calls_of_self(&self) -> Vec<RustAccess> {
        self.as_member().calls_of_self()
    }
}

impl MemberLike for RustMember {
    fn plural() -> &'static str {
        "members"
    }
    fn members_of(item: &RustItem) -> Vec<Self> {
        item.members()
    }
    fn as_member(&self) -> &RustMember {
        self
    }
}

impl MemberLike for RustField {
    fn plural() -> &'static str {
        "fields"
    }
    fn members_of(item: &RustItem) -> Vec<Self> {
        item.fields()
    }
    fn as_member(&self) -> &RustMember {
        &self.0
    }
}

impl MemberLike for RustCodeUnit {
    fn plural() -> &'static str {
        "code units"
    }
    fn members_of(item: &RustItem) -> Vec<Self> {
        item.code_units()
    }
    fn as_member(&self) -> &RustMember {
        &self.0
    }
}

impl CodeUnitLike for RustCodeUnit {}

impl MemberLike for RustMethod {
    fn plural() -> &'static str {
        "methods"
    }
    fn members_of(item: &RustItem) -> Vec<Self> {
        item.methods()
    }
    fn as_member(&self) -> &RustMember {
        &self.0
    }
}

impl CodeUnitLike for RustMethod {}

impl MemberLike for RustConstructor {
    fn plural() -> &'static str {
        "constructors"
    }
    fn members_of(item: &RustItem) -> Vec<Self> {
        item.constructors()
    }
    fn as_member(&self) -> &RustMember {
        &self.0
    }
}

impl CodeUnitLike for RustConstructor {}

/// A parameter of a code unit (`JavaParameter`).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RustParameter {
    member: RustMember,
    index: usize,
}

impl fmt::Debug for RustParameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RustParameter({}: {})", self.name(), self.type_())
    }
}

impl RustParameter {
    fn data(&self) -> &super::graph::ParameterData {
        &self.member.data().parameters[self.index]
    }

    /// The parameter name (pattern) as written.
    pub fn name(&self) -> String {
        self.data().name.clone()
    }

    /// The zero-based index, excluding `self`.
    pub fn index(&self) -> usize {
        self.index
    }

    /// The declared type.
    pub fn type_(&self) -> RustType {
        self.data().type_.clone()
    }

    /// The erased type as an item.
    pub fn raw_type(&self) -> Option<RustItem> {
        self.member
            .graph
            .erased(&self.data().type_)
            .map(|id| RustItem::new(Arc::clone(&self.member.graph), id))
    }

    /// The owning code unit.
    pub fn owner(&self) -> RustCodeUnit {
        RustCodeUnit(self.member.clone())
    }
}

impl CanBeAnnotated for RustParameter {
    fn annotations(&self) -> Vec<RustAnnotation> {
        self.data().annotations.clone()
    }
}

impl HasType for RustParameter {
    fn type_(&self) -> Option<RustType> {
        Some(self.data().type_.clone())
    }

    fn raw_type(&self) -> Option<RustItem> {
        RustParameter::raw_type(self)
    }
}

/// `JavaMember.Predicates`.
pub mod predicates {
    use super::*;

    /// Described as `declared in <name or predicate>`.
    pub fn declared_in<T>(selector: impl Into<ItemSelector>) -> DescribedPredicate<T>
    where
        T: HasOwner<RustItem> + ?Sized + 'static,
    {
        let selector = selector.into();
        let description = format!("declared in {}", selector.description());
        DescribedPredicate::describe(description, move |m: &T| selector.matches(&m.owner()))
    }
}

/// `JavaCodeUnit.Predicates`.
pub mod code_unit_predicates {
    use super::*;

    /// Described as `method`.
    pub fn method() -> DescribedPredicate<RustCodeUnit> {
        DescribedPredicate::describe("method", |c: &RustCodeUnit| c.is_method())
    }

    /// Described as `constructor`.
    pub fn constructor() -> DescribedPredicate<RustCodeUnit> {
        DescribedPredicate::describe("constructor", |c: &RustCodeUnit| c.is_constructor())
    }

    /// Described as `any parameter that <predicate>`.
    pub fn any_parameter_that(
        predicate: DescribedPredicate<RustParameter>,
    ) -> DescribedPredicate<RustCodeUnit> {
        let description = format!("any parameter that {}", predicate.description());
        DescribedPredicate::describe(description, move |c: &RustCodeUnit| {
            c.parameters().iter().any(|p| predicate.test(p))
        })
    }

    /// Described as `all parameters <predicate>`.
    pub fn all_parameters(
        predicate: DescribedPredicate<RustParameter>,
    ) -> DescribedPredicate<RustCodeUnit> {
        let description = format!("all parameters {}", predicate.description());
        DescribedPredicate::describe(description, move |c: &RustCodeUnit| {
            c.parameters().iter().all(|p| predicate.test(p))
        })
    }
}
