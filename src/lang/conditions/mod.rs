//! Predefined conditions (`ArchConditions`) and predicate wrappers (`ArchPredicates`).
//!
//! Every `ArchConditions.xyz(..)` of ArchUnit is a free function `xyz(..)` here; the
//! `ArchPredicates` wrappers `is`, `are`, `has`, `have`, `be` live in [`predicates`].

use std::sync::Arc;

use crate::base::{DescribedPredicate, join_single_quoted};
use crate::core::domain::formatters::{ensure_simple_name, format_method};
use crate::core::domain::properties::{
    AnnotationSelector, CanBeAnnotated, HasErrorTypes, HasFullName, HasModifiers, HasName,
    HasOwner, HasParameterTypes, HasReturnType, HasType, ItemSelector, can_be_annotated,
    has_error_types, has_full_name, has_modifiers, has_name, has_parameter_types, has_return_type,
    has_type,
};
use crate::core::domain::{
    AccessKind, AccessType, CodeUnitLike, Dependency, PackageMatcher, RustAccess, RustCodeUnit,
    RustConstructor, RustField, RustItem, RustMember, RustMethod, RustModifier, rust_access,
    rust_item, rust_member,
};
use crate::lang::condition::{
    ArchCondition, ConditionByPredicate, ConditionLogic, ConditionTarget,
};
use crate::lang::events::{
    AnyConditionEvent, AsCorrespondingObject, ConditionEvents, CorrespondingObject,
    OnlyConditionEvent, SimpleConditionEvent, create_message,
};

/// `ArchPredicates`: wrap a predicate to read naturally in a rule text.
pub mod predicates {
    use crate::base::DescribedPredicate;

    /// Described as `is <description>`.
    pub fn is<T: ?Sized + 'static>(predicate: DescribedPredicate<T>) -> DescribedPredicate<T> {
        let description = format!("is {}", predicate.description());
        predicate.as_(description)
    }

    /// Described as `are <description>`.
    pub fn are<T: ?Sized + 'static>(predicate: DescribedPredicate<T>) -> DescribedPredicate<T> {
        let description = format!("are {}", predicate.description());
        predicate.as_(description)
    }

    /// Described as `has <description>`.
    pub fn has<T: ?Sized + 'static>(predicate: DescribedPredicate<T>) -> DescribedPredicate<T> {
        let description = format!("has {}", predicate.description());
        predicate.as_(description)
    }

    /// Described as `have <description>`.
    pub fn have<T: ?Sized + 'static>(predicate: DescribedPredicate<T>) -> DescribedPredicate<T> {
        let description = format!("have {}", predicate.description());
        predicate.as_(description)
    }

    /// Described as `be <description>`.
    pub fn be<T: ?Sized + 'static>(predicate: DescribedPredicate<T>) -> DescribedPredicate<T> {
        let description = format!("be {}", predicate.description());
        predicate.as_(description)
    }
}

// ---- generic building blocks ---------------------------------------------------------------

/// A condition from a predicate; described as `have <predicate>`, events read
/// `has <predicate>` / `does not have <predicate>` (`ArchConditions.have(..)`).
pub fn have<T: ConditionTarget>(predicate: DescribedPredicate<T>) -> ConditionByPredicate<T> {
    let description = format!("have {}", predicate.description());
    ConditionByPredicate::from(predicate)
        .as_(description)
        .describe_events_by(|p, satisfied| {
            format!("{}{p}", if satisfied { "has " } else { "does not have " })
        })
}

/// A condition from a predicate; described as `be <predicate>`, events read
/// `is <predicate>` / `is not <predicate>` (`ArchConditions.be(..)`).
pub fn be<T: ConditionTarget>(predicate: DescribedPredicate<T>) -> ConditionByPredicate<T> {
    let description = format!("be {}", predicate.description());
    ConditionByPredicate::from(predicate)
        .as_(description)
        .describe_events_by(|p, satisfied| {
            format!("{}{p}", if satisfied { "is " } else { "is not " })
        })
}

fn does<T: ConditionTarget>(predicate: DescribedPredicate<T>) -> ConditionByPredicate<T> {
    ConditionByPredicate::from(predicate).describe_events_by(|p, satisfied| {
        format!("{}{p}", if satisfied { "does " } else { "does not " })
    })
}

/// `ArchConditions.and(a, b)`.
pub fn and<T: AsCorrespondingObject + Send + Sync + 'static>(
    first: impl Into<ArchCondition<T>>,
    second: impl Into<ArchCondition<T>>,
) -> ArchCondition<T> {
    first.into().and(second)
}

/// `ArchConditions.or(a, b)`.
pub fn or<T: AsCorrespondingObject + Send + Sync + 'static>(
    first: impl Into<ArchCondition<T>>,
    second: impl Into<ArchCondition<T>>,
) -> ArchCondition<T> {
    first.into().or(second)
}

/// `ArchConditions.never(condition)`: every event inverted; described as `never <description>`.
pub fn never<T: Send + 'static>(condition: impl Into<ArchCondition<T>>) -> ArchCondition<T> {
    ArchCondition::never(condition.into())
}

/// `ArchConditions.not(condition)`: every event inverted; described as `not <description>`.
pub fn not<T: Send + 'static>(condition: impl Into<ArchCondition<T>>) -> ArchCondition<T> {
    ArchCondition::not(condition.into())
}

type Getter<T, A> = Arc<dyn Fn(&T) -> Vec<A> + Send + Sync>;

/// `AnyAttributeMatchesCondition`: satisfied if any attribute satisfies `inner`
/// (see `ContainAnyCondition`).
fn any_attribute_matches<T, A>(
    description: impl Into<String>,
    inner: ArchCondition<A>,
    attributes: Getter<T, A>,
) -> ArchCondition<T>
where
    T: Send + Sync + 'static,
    A: AsCorrespondingObject + Send + Sync + 'static,
{
    ArchCondition::new(
        description,
        move |item: &T, events: &mut ConditionEvents| {
            let attrs = attributes(item);
            let mut sub = ConditionEvents::new();
            for attribute in &attrs {
                inner.check(attribute, &mut sub);
            }
            let (allowed, violating, _) = sub.into_parts();
            if !allowed.is_empty() || !violating.is_empty() {
                let objects: Vec<CorrespondingObject> = attrs
                    .iter()
                    .map(AsCorrespondingObject::as_corresponding_object)
                    .collect();
                events.add(AnyConditionEvent::new(objects, allowed, violating));
            }
        },
    )
}

/// `AllAttributesMatchCondition`: satisfied if every attribute satisfies `inner`
/// (see `ContainsOnlyCondition`).
fn all_attributes_match<T, A>(
    description: impl Into<String>,
    inner: ArchCondition<A>,
    attributes: Getter<T, A>,
) -> ArchCondition<T>
where
    T: Send + Sync + 'static,
    A: AsCorrespondingObject + Send + Sync + 'static,
{
    ArchCondition::new(
        description,
        move |item: &T, events: &mut ConditionEvents| {
            let attrs = attributes(item);
            let mut sub = ConditionEvents::new();
            for attribute in &attrs {
                inner.check(attribute, &mut sub);
            }
            let (allowed, violating, _) = sub.into_parts();
            if !allowed.is_empty() || !violating.is_empty() {
                let objects: Vec<CorrespondingObject> = attrs
                    .iter()
                    .map(AsCorrespondingObject::as_corresponding_object)
                    .collect();
                events.add(OnlyConditionEvent::new(objects, allowed, violating));
            }
        },
    )
}

/// `JavaAccessCondition`: one event per access with the access description.
fn access_condition(predicate: DescribedPredicate<RustAccess>) -> ArchCondition<RustAccess> {
    let description = format!("access target where {}", predicate.description());
    ArchCondition::new(
        description,
        move |access: &RustAccess, events: &mut ConditionEvents| {
            events.add(SimpleConditionEvent::new(
                access,
                predicate.test(access),
                access.description(),
            ));
        },
    )
}

/// `DependencyCondition`: one event per dependency with the dependency description.
fn dependency_condition(predicate: DescribedPredicate<Dependency>) -> ArchCondition<Dependency> {
    ArchCondition::new(
        predicate.description().to_owned(),
        move |dependency: &Dependency, events: &mut ConditionEvents| {
            events.add(SimpleConditionEvent::new(
                dependency,
                predicate.test(dependency),
                dependency.description(),
            ));
        },
    )
}

fn class_accesses(
    predicate: DescribedPredicate<RustAccess>,
    get: Getter<RustItem, RustAccess>,
) -> ArchCondition<RustItem> {
    let inner = access_condition(predicate);
    let description = inner.description().to_owned();
    any_attribute_matches(description, inner, get)
}

fn class_only_accesses(
    predicate: DescribedPredicate<RustAccess>,
    get: Getter<RustItem, RustAccess>,
) -> ArchCondition<RustItem> {
    let description = format!("only access targets where {}", predicate.description());
    all_attributes_match(description, access_condition(predicate), get)
}

fn all_accesses(
    prefix: &str,
    predicate: DescribedPredicate<RustAccess>,
    get: Getter<RustItem, RustAccess>,
) -> ArchCondition<RustItem> {
    let description = format!("{prefix} {}", predicate.description());
    all_attributes_match(description, access_condition(predicate), get)
}

fn accesses_from_self() -> Getter<RustItem, RustAccess> {
    Arc::new(|item: &RustItem| item.accesses_from_self())
}

fn accesses_to_self() -> Getter<RustItem, RustAccess> {
    Arc::new(|item: &RustItem| item.accesses_to_self())
}

fn dependencies_from_self() -> Getter<RustItem, Dependency> {
    Arc::new(|item: &RustItem| item.direct_dependencies_from_self())
}

fn dependencies_to_self() -> Getter<RustItem, Dependency> {
    Arc::new(|item: &RustItem| item.direct_dependencies_to_self())
}

/// Lifts a class predicate to the target owner of an access, keeping the description
/// (ArchUnit: `Get.target().then(Get.owner()).is(predicate)`).
fn target_owner(predicate: DescribedPredicate<RustItem>) -> DescribedPredicate<RustAccess> {
    predicate.on_result_of(|access: &RustAccess| access.target_owner())
}

/// Lifts a class predicate to the origin owner of an access, keeping the description.
fn origin_owner(predicate: DescribedPredicate<RustItem>) -> DescribedPredicate<RustAccess> {
    predicate.on_result_of(|access: &RustAccess| access.origin_owner())
}

/// Matches accesses whose resolved member satisfies `predicate`, or which could not be resolved
/// (ArchUnit: `optionalContains(predicate).or(optionalEmpty())`).
fn resolved_member_satisfies<M: Clone + 'static>(
    predicate: DescribedPredicate<M>,
    convert: impl Fn(&RustMember) -> Option<M> + Send + Sync + 'static,
) -> DescribedPredicate<RustAccess> {
    DescribedPredicate::describe(
        predicate.description().to_owned(),
        move |access: &RustAccess| match access.target().resolve_member().and_then(|m| convert(&m))
        {
            Some(member) => predicate.test(&member),
            None => true,
        },
    )
}

// ---- field accesses ------------------------------------------------------------------------

fn owner_and_name_are(owner: &str, field: &str) -> DescribedPredicate<RustAccess> {
    let owner_selector = ItemSelector::from(owner);
    let field = field.to_owned();
    let description = format!("{owner}::{field}");
    DescribedPredicate::describe(description, move |access: &RustAccess| {
        owner_selector.matches(&access.target_owner()) && access.name() == field
    })
}

fn field_access_type(access_type: AccessType) -> DescribedPredicate<RustAccess> {
    rust_access::predicates::access_type(access_type)
}

fn class_accesses_field(predicate: DescribedPredicate<RustAccess>) -> ArchCondition<RustItem> {
    let description = format!("access field where {}", predicate.description());
    let inner = ArchCondition::new(
        description.clone(),
        move |access: &RustAccess, events: &mut ConditionEvents| {
            events.add(SimpleConditionEvent::new(
                access,
                predicate.test(access),
                access.description(),
            ));
        },
    );
    any_attribute_matches(
        description,
        inner,
        Arc::new(|item: &RustItem| item.field_accesses_from_self()),
    )
}

/// Reads of `owner::field`; described as `get field Owner::field`.
pub fn get_field(owner: &str, field_name: &str) -> ArchCondition<RustItem> {
    get_field_where(owner_and_name_are(owner, field_name)).as_(format!(
        "get field {}::{field_name}",
        ensure_simple_name(owner)
    ))
}

/// Reads matching `predicate`; described as `get field where <predicate>`.
pub fn get_field_where(predicate: DescribedPredicate<RustAccess>) -> ArchCondition<RustItem> {
    let description = format!("get field where {}", predicate.description());
    class_accesses_field(predicate.and(field_access_type(AccessType::Get))).as_(description)
}

/// Writes of `owner::field`; described as `set field Owner::field`.
pub fn set_field(owner: &str, field_name: &str) -> ArchCondition<RustItem> {
    set_field_where(owner_and_name_are(owner, field_name)).as_(format!(
        "set field {}::{field_name}",
        ensure_simple_name(owner)
    ))
}

/// Writes matching `predicate`; described as `set field where <predicate>`.
pub fn set_field_where(predicate: DescribedPredicate<RustAccess>) -> ArchCondition<RustItem> {
    let description = format!("set field where {}", predicate.description());
    class_accesses_field(predicate.and(field_access_type(AccessType::Set))).as_(description)
}

/// Reads or writes of `owner::field`; described as `access field Owner::field`.
pub fn access_field(owner: &str, field_name: &str) -> ArchCondition<RustItem> {
    access_field_where(owner_and_name_are(owner, field_name)).as_(format!(
        "access field {}::{field_name}",
        ensure_simple_name(owner)
    ))
}

/// Field accesses matching `predicate`; described as `access field where <predicate>`.
pub fn access_field_where(predicate: DescribedPredicate<RustAccess>) -> ArchCondition<RustItem> {
    let description = format!("access field where {}", predicate.description());
    class_accesses_field(predicate).as_(description)
}

/// `only access fields that <predicate>`.
pub fn only_access_fields_that(
    predicate: DescribedPredicate<RustField>,
) -> ArchCondition<RustItem> {
    let description = format!("only access fields that {}", predicate.description());
    let access_predicate = resolved_member_satisfies(predicate, |m| m.as_field());
    class_only_accesses(
        access_predicate,
        Arc::new(|item: &RustItem| item.field_accesses_from_self()),
    )
    .as_(description)
}

// ---- calls ---------------------------------------------------------------------------------

/// Whether the resolved target's parameter types match `parameter_type_names` (full names,
/// simple names, or the written form). Unresolved targets match.
fn target_parameter_types(parameter_type_names: &[&str]) -> DescribedPredicate<RustAccess> {
    let expected: Vec<String> = parameter_type_names
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    let description = format!("raw parameter types [{}]", expected.join(", "));
    DescribedPredicate::describe(description, move |access: &RustAccess| {
        let Some(member) = access.target().resolve_member() else {
            return true;
        };
        let types = member.parameter_types();
        let raw = member.raw_parameter_types();
        types.len() == expected.len()
            && types
                .iter()
                .zip(raw.iter())
                .zip(&expected)
                .all(|((written, raw), expected)| {
                    written.to_string() == *expected
                        || raw
                            .as_ref()
                            .is_some_and(|item| ItemSelector::from(expected.as_str()).matches(item))
                })
    })
}

fn target_named(owner: &str, name: &str) -> DescribedPredicate<RustAccess> {
    let owner_selector = ItemSelector::from(owner);
    let name = name.to_owned();
    DescribedPredicate::describe(
        format!("target {owner}::{name}"),
        move |access: &RustAccess| {
            owner_selector.matches(&access.target_owner()) && access.name() == name
        },
    )
}

/// Calls of `owner::method(parameter types)`; described as `call method Owner::method(p1, p2)`.
pub fn call_method(
    owner: &str,
    method_name: &str,
    parameter_type_names: &[&str],
) -> ArchCondition<RustItem> {
    let params: Vec<String> = parameter_type_names
        .iter()
        .map(|p| ensure_simple_name(p))
        .collect();
    let description = format!(
        "call method {}",
        format_method(&ensure_simple_name(owner), method_name, &params)
    );
    call_method_where(
        target_named(owner, method_name).and(target_parameter_types(parameter_type_names)),
    )
    .as_(description)
}

/// Method calls matching `predicate`; described as `call method where <predicate>`.
pub fn call_method_where(predicate: DescribedPredicate<RustAccess>) -> ArchCondition<RustItem> {
    let description = format!("call method where {}", predicate.description());
    class_accesses(
        predicate,
        Arc::new(|item: &RustItem| item.method_calls_from_self()),
    )
    .as_(description)
}

/// `only call methods that <predicate>`.
pub fn only_call_methods_that(
    predicate: DescribedPredicate<RustMethod>,
) -> ArchCondition<RustItem> {
    let description = format!("only call methods that {}", predicate.description());
    let call_predicate = resolved_member_satisfies(predicate, |m| m.as_method());
    class_only_accesses(
        call_predicate,
        Arc::new(|item: &RustItem| item.method_calls_from_self()),
    )
    .as_(description)
}

/// Constructor calls of `owner` with the given parameter types; described as
/// `call constructor Owner(p1, p2)`.
pub fn call_constructor(owner: &str, parameter_type_names: &[&str]) -> ArchCondition<RustItem> {
    let params: Vec<String> = parameter_type_names
        .iter()
        .map(|p| ensure_simple_name(p))
        .collect();
    let description = format!(
        "call constructor {}({})",
        ensure_simple_name(owner),
        params.join(", ")
    );
    let owner_selector = ItemSelector::from(owner);
    let owner_predicate = DescribedPredicate::describe(
        format!("target owner {owner}"),
        move |access: &RustAccess| owner_selector.matches(&access.target_owner()),
    );
    call_constructor_where(owner_predicate.and(target_parameter_types(parameter_type_names)))
        .as_(description)
}

/// Constructor calls matching `predicate`; described as `call constructor where <predicate>`.
pub fn call_constructor_where(
    predicate: DescribedPredicate<RustAccess>,
) -> ArchCondition<RustItem> {
    let description = format!("call constructor where {}", predicate.description());
    class_accesses(
        predicate,
        Arc::new(|item: &RustItem| item.constructor_calls_from_self()),
    )
    .as_(description)
}

/// `only call constructors that <predicate>`.
pub fn only_call_constructors_that(
    predicate: DescribedPredicate<RustConstructor>,
) -> ArchCondition<RustItem> {
    let description = format!("only call constructors that {}", predicate.description());
    let call_predicate = resolved_member_satisfies(predicate, |m| m.as_constructor());
    class_only_accesses(
        call_predicate,
        Arc::new(|item: &RustItem| item.constructor_calls_from_self()),
    )
    .as_(description)
}

/// Calls of methods or constructors matching `predicate`; described as
/// `call code unit where <predicate>`.
pub fn call_code_unit_where(predicate: DescribedPredicate<RustAccess>) -> ArchCondition<RustItem> {
    let description = format!("call code unit where {}", predicate.description());
    class_accesses(
        predicate,
        Arc::new(|item: &RustItem| item.code_unit_calls_from_self()),
    )
    .as_(description)
}

/// `only call code units that <predicate>`.
pub fn only_call_code_units_that(
    predicate: DescribedPredicate<RustCodeUnit>,
) -> ArchCondition<RustItem> {
    let description = format!("only call code units that {}", predicate.description());
    let call_predicate = resolved_member_satisfies(predicate, |m| m.as_code_unit());
    class_only_accesses(
        call_predicate,
        Arc::new(|item: &RustItem| item.code_unit_calls_from_self()),
    )
    .as_(description)
}

/// `only access members that <predicate>`.
pub fn only_access_members_that(
    predicate: DescribedPredicate<RustMember>,
) -> ArchCondition<RustItem> {
    let description = format!("only access members that {}", predicate.description());
    let access_predicate = resolved_member_satisfies(predicate, |m| Some(m.clone()));
    class_only_accesses(access_predicate, accesses_from_self()).as_(description)
}

/// Any access matching `predicate`; described as `access target where <predicate>`.
pub fn access_target_where(predicate: DescribedPredicate<RustAccess>) -> ArchCondition<RustItem> {
    class_accesses(predicate, accesses_from_self())
}

// ---- classes and dependencies --------------------------------------------------------------

/// `access classes that <predicate>`.
pub fn access_classes_that(predicate: DescribedPredicate<RustItem>) -> ArchCondition<RustItem> {
    let description = format!("access classes that {}", predicate.description());
    class_accesses(target_owner(predicate), accesses_from_self()).as_(description)
}

/// `only access classes that <predicate>`.
pub fn only_access_classes_that(
    predicate: DescribedPredicate<RustItem>,
) -> ArchCondition<RustItem> {
    all_accesses(
        "only access classes that",
        target_owner(predicate),
        accesses_from_self(),
    )
}

/// A condition on any of an item's dependencies with an ignore list (`AnyDependencyCondition`).
#[derive(Clone)]
pub struct AnyDependencyCondition {
    description: String,
    predicate: DescribedPredicate<Dependency>,
    get: Getter<RustItem, Dependency>,
    ignore: DescribedPredicate<Dependency>,
}

impl std::fmt::Debug for AnyDependencyCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AnyDependencyCondition({})", self.description)
    }
}

impl AnyDependencyCondition {
    fn new(
        description: String,
        predicate: DescribedPredicate<Dependency>,
        get: Getter<RustItem, Dependency>,
    ) -> Self {
        Self {
            description,
            predicate,
            get,
            ignore: crate::base::always_false(),
        }
    }

    /// Ignores dependencies matching `ignore` (`ignoreDependency(..)`).
    pub fn ignore_dependency(mut self, ignore: DescribedPredicate<Dependency>) -> Self {
        self.ignore = self.ignore.or(ignore);
        self
    }

    /// Overrides the description.
    pub fn as_(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// The description.
    pub fn description(&self) -> &str {
        &self.description
    }
}

impl From<AnyDependencyCondition> for ArchCondition<RustItem> {
    fn from(condition: AnyDependencyCondition) -> Self {
        let AnyDependencyCondition {
            description,
            predicate,
            get,
            ignore,
        } = condition;
        let getter: Getter<RustItem, Dependency> = Arc::new(move |item: &RustItem| {
            get(item).into_iter().filter(|d| !ignore.test(d)).collect()
        });
        any_attribute_matches(description, dependency_condition(predicate), getter)
    }
}

/// A condition on all of an item's dependencies with an ignore list (`AllDependenciesCondition`).
#[derive(Clone)]
pub struct AllDependenciesCondition {
    description: String,
    predicate: DescribedPredicate<Dependency>,
    get: Getter<RustItem, Dependency>,
    ignore: DescribedPredicate<Dependency>,
}

impl std::fmt::Debug for AllDependenciesCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AllDependenciesCondition({})", self.description)
    }
}

impl AllDependenciesCondition {
    fn new(
        description: String,
        predicate: DescribedPredicate<Dependency>,
        get: Getter<RustItem, Dependency>,
    ) -> Self {
        Self {
            description,
            predicate,
            get,
            ignore: crate::base::always_false(),
        }
    }

    /// Ignores dependencies matching `ignore` (`ignoreDependency(..)`).
    pub fn ignore_dependency(mut self, ignore: DescribedPredicate<Dependency>) -> Self {
        self.ignore = self.ignore.or(ignore);
        self
    }

    /// Overrides the description.
    pub fn as_(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// The description.
    pub fn description(&self) -> &str {
        &self.description
    }
}

impl From<AllDependenciesCondition> for ArchCondition<RustItem> {
    fn from(condition: AllDependenciesCondition) -> Self {
        let AllDependenciesCondition {
            description,
            predicate,
            get,
            ignore,
        } = condition;
        let getter: Getter<RustItem, Dependency> = Arc::new(move |item: &RustItem| {
            get(item).into_iter().filter(|d| !ignore.test(d)).collect()
        });
        all_attributes_match(description, dependency_condition(predicate), getter)
    }
}

fn dependency_target(predicate: DescribedPredicate<RustItem>) -> DescribedPredicate<Dependency> {
    crate::core::domain::dependency::predicates::dependency_target(predicate)
}

fn dependency_origin(predicate: DescribedPredicate<RustItem>) -> DescribedPredicate<Dependency> {
    crate::core::domain::dependency::predicates::dependency_origin(predicate)
}

/// `depend on classes that <predicate>`.
pub fn depend_on_classes_that(predicate: DescribedPredicate<RustItem>) -> AnyDependencyCondition {
    let description = format!("depend on classes that {}", predicate.description());
    AnyDependencyCondition::new(
        description,
        dependency_target(predicate),
        dependencies_from_self(),
    )
}

/// `have any dependencies that <predicate>`.
pub fn have_any_dependencies_that(
    predicate: DescribedPredicate<Dependency>,
) -> AnyDependencyCondition {
    let description = format!("have any dependencies that {}", predicate.description());
    AnyDependencyCondition::new(description, predicate, dependencies_from_self())
}

/// `only depend on classes that <predicate>`.
pub fn only_depend_on_classes_that(
    predicate: DescribedPredicate<RustItem>,
) -> AllDependenciesCondition {
    let description = format!("only depend on classes that {}", predicate.description());
    AllDependenciesCondition::new(
        description,
        dependency_target(predicate),
        dependencies_from_self(),
    )
}

/// `only be accessed by classes that <predicate>`.
pub fn only_be_accessed_by_classes_that(
    predicate: DescribedPredicate<RustItem>,
) -> ArchCondition<RustItem> {
    all_accesses(
        "only be accessed by classes that",
        origin_owner(predicate),
        accesses_to_self(),
    )
}

fn access_package_predicate(
    package_identifiers: &[&str],
    origin: bool,
) -> DescribedPredicate<RustAccess> {
    let matchers: Vec<PackageMatcher> = package_identifiers
        .iter()
        .map(|id| PackageMatcher::of(id))
        .collect();
    let description = format!("any package [{}]", join_single_quoted(package_identifiers));
    DescribedPredicate::describe(description, move |access: &RustAccess| {
        let package = if origin {
            access.origin_owner().package_name()
        } else {
            access.target_owner().package_name()
        };
        matchers.iter().any(|m| m.matches(&package))
    })
}

/// `access classes that reside in package '<identifier>'`.
pub fn access_classes_that_reside_in(package_identifier: &str) -> ArchCondition<RustItem> {
    access_classes_that_reside_in_any_package(&[package_identifier]).as_(format!(
        "access classes that reside in package '{package_identifier}'"
    ))
}

/// `access classes that reside in any package ['a', 'b']`.
pub fn access_classes_that_reside_in_any_package(
    package_identifiers: &[&str],
) -> ArchCondition<RustItem> {
    let predicate = access_package_predicate(package_identifiers, false);
    let description = format!("access classes that reside in {}", predicate.description());
    class_accesses(predicate, accesses_from_self()).as_(description)
}

/// `only be accessed by any package ['a', 'b']`.
pub fn only_be_accessed_by_any_package(package_identifiers: &[&str]) -> ArchCondition<RustItem> {
    all_accesses(
        "only be accessed by",
        access_package_predicate(package_identifiers, true),
        accesses_to_self(),
    )
}

/// `only have dependents in any package ['a', 'b']`.
pub fn only_have_dependents_in_any_package(
    package_identifiers: &[&str],
) -> AllDependenciesCondition {
    let description = format!(
        "only have dependents in any package [{}]",
        join_single_quoted(package_identifiers)
    );
    let matchers = crate::core::domain::PackageMatchers::of(package_identifiers);
    let origin_in_package = rust_item::functions::get_package_name().is(matchers);
    only_have_dependents_where(dependency_origin(origin_in_package)).as_(description)
}

/// `only have dependent classes that <predicate>`.
pub fn only_have_dependent_classes_that(
    predicate: DescribedPredicate<RustItem>,
) -> AllDependenciesCondition {
    let description = format!(
        "only have dependent classes that {}",
        predicate.description()
    );
    only_have_dependents_where(dependency_origin(predicate)).as_(description)
}

/// `only have dependents where <predicate>`.
pub fn only_have_dependents_where(
    predicate: DescribedPredicate<Dependency>,
) -> AllDependenciesCondition {
    let description = format!("only have dependents where {}", predicate.description());
    AllDependenciesCondition::new(description, predicate, dependencies_to_self())
}

/// `only have dependencies in any package ['a', 'b']`.
pub fn only_have_dependencies_in_any_package(
    package_identifiers: &[&str],
) -> AllDependenciesCondition {
    let description = format!(
        "only have dependencies in any package [{}]",
        join_single_quoted(package_identifiers)
    );
    let matchers = crate::core::domain::PackageMatchers::of(package_identifiers);
    let target_in_package = rust_item::functions::get_package_name().is(matchers);
    only_have_dependencies_where(dependency_target(target_in_package)).as_(description)
}

/// `only have dependencies where <predicate>`.
pub fn only_have_dependencies_where(
    predicate: DescribedPredicate<Dependency>,
) -> AllDependenciesCondition {
    let description = format!("only have dependencies where {}", predicate.description());
    AllDependenciesCondition::new(description, predicate, dependencies_from_self())
}

/// `transitively depend on classes that <predicate>` (`TransitiveDependencyCondition`).
///
/// Reports, for each item, the first found path to a matching item; items whose transitive
/// dependencies never match are violations.
pub fn transitively_depend_on_classes_that(
    predicate: DescribedPredicate<RustItem>,
) -> ArchCondition<RustItem> {
    struct Logic {
        predicate: DescribedPredicate<RustItem>,
        all: std::collections::HashSet<RustItem>,
    }

    impl Logic {
        fn targets_outside(&self, item: &RustItem) -> Vec<RustItem> {
            let mut targets: Vec<RustItem> = item
                .direct_dependencies_from_self()
                .into_iter()
                .map(|d| d.target_item())
                .filter(|t| !self.all.contains(t))
                .collect();
            targets.sort();
            targets.dedup();
            targets
        }

        fn find_path(&self, start: &RustItem) -> Vec<RustItem> {
            let mut path = Vec::new();
            let mut analyzed = std::collections::HashSet::new();
            self.add_to_path(start, &mut path, &mut analyzed);
            path.reverse();
            path
        }

        fn add_to_path(
            &self,
            item: &RustItem,
            path: &mut Vec<RustItem>,
            analyzed: &mut std::collections::HashSet<RustItem>,
        ) -> bool {
            if self.predicate.test(item) {
                path.push(item.clone());
                return true;
            }
            analyzed.insert(item.clone());
            for target in self.targets_outside(item) {
                if !analyzed.contains(&target) && self.add_to_path(&target, path, analyzed) {
                    path.push(item.clone());
                    return true;
                }
            }
            false
        }
    }

    impl ConditionLogic<RustItem> for Logic {
        fn init(&mut self, all: &[RustItem]) {
            self.all = all.iter().cloned().collect();
        }

        fn check(&mut self, item: &RustItem, events: &mut ConditionEvents) {
            let mut found = false;
            for target in self.targets_outside(item) {
                let path = self.find_path(&target);
                if let Some(last) = path.last() {
                    let mut message = format!(
                        "{}depends on <{}>",
                        if path.len() > 1 { "transitively " } else { "" },
                        last.full_name()
                    );
                    if path.len() > 1 {
                        message.push_str(&format!(
                            " by [{}]",
                            path.iter()
                                .map(RustItem::name)
                                .collect::<Vec<_>>()
                                .join("->")
                        ));
                    }
                    events.add(SimpleConditionEvent::satisfied(
                        item,
                        create_message(item, &message),
                    ));
                    found = true;
                }
            }
            if !found {
                events.add(SimpleConditionEvent::violated(
                    item,
                    create_message(item, "does not transitively depend on any matching class"),
                ));
            }
        }
    }

    let description = format!(
        "transitively depend on classes that {}",
        predicate.description()
    );
    ArchCondition::from_logic(
        description,
        Logic {
            predicate,
            all: std::collections::HashSet::new(),
        },
    )
}

// ---- identity and names --------------------------------------------------------------------

/// `be <name>`: the item with the given full name (`ArchConditions.be(String)`; named
/// `be_class` because `be(predicate)` takes the generic overload).
pub fn be_class(class_name: &str) -> ArchCondition<RustItem> {
    let name = class_name.to_owned();
    ConditionByPredicate::from(fully_qualified_name::<RustItem>(class_name).as_(class_name))
        .as_(format!("be {class_name}"))
        .describe_events_by(move |_, satisfied| {
            format!("{}{name}", if satisfied { "is " } else { "is not " })
        })
        .into_condition()
}

/// `not be <name>` (`ArchConditions.notBe(String)`).
pub fn not_be_class(class_name: &str) -> ArchCondition<RustItem> {
    not(be_class(class_name))
}

/// Described as `fully qualified name '<name>'`.
pub fn fully_qualified_name<T: HasName + ?Sized + 'static>(name: &str) -> DescribedPredicate<T> {
    let predicate = has_name::predicates::name::<T>(name);
    let description = format!("fully qualified {}", predicate.description());
    predicate.as_(description)
}

/// `have name '<name>'`.
pub fn have_name<T: ConditionTarget + HasName>(name: &str) -> ArchCondition<T> {
    have(has_name::predicates::name(name)).into()
}

/// `not have name '<name>'`.
pub fn not_have_name<T: ConditionTarget + HasName>(name: &str) -> ArchCondition<T> {
    not(have_name(name))
}

/// `have full name '<name>'`.
pub fn have_full_name<T: ConditionTarget + HasFullName>(full_name: &str) -> ArchCondition<T> {
    have(has_full_name::predicates::full_name(full_name)).into()
}

/// `not have full name '<name>'`.
pub fn not_have_full_name<T: ConditionTarget + HasFullName>(full_name: &str) -> ArchCondition<T> {
    not(have_full_name(full_name))
}

/// `have fully qualified name '<name>'`.
pub fn have_fully_qualified_name(name: &str) -> ArchCondition<RustItem> {
    have(fully_qualified_name(name)).into()
}

/// `not have fully qualified name '<name>'`.
pub fn not_have_fully_qualified_name(name: &str) -> ArchCondition<RustItem> {
    not(have_fully_qualified_name(name))
}

/// `have simple name '<name>'`.
pub fn have_simple_name(name: &str) -> ArchCondition<RustItem> {
    have(rust_item::predicates::simple_name(name)).into()
}

/// `not have simple name '<name>'`.
pub fn not_have_simple_name(name: &str) -> ArchCondition<RustItem> {
    not(have_simple_name(name))
}

/// `have simple name starting with '<prefix>'`.
pub fn have_simple_name_starting_with(prefix: &str) -> ArchCondition<RustItem> {
    have(rust_item::predicates::simple_name_starting_with(prefix)).into()
}

/// `have simple name not starting with '<prefix>'`.
pub fn have_simple_name_not_starting_with(prefix: &str) -> ArchCondition<RustItem> {
    not(have_simple_name_starting_with(prefix))
        .as_(format!("have simple name not starting with '{prefix}'"))
}

/// `have simple name containing '<infix>'`.
pub fn have_simple_name_containing(infix: &str) -> ArchCondition<RustItem> {
    have(rust_item::predicates::simple_name_containing(infix)).into()
}

/// `have simple name not containing '<infix>'`.
pub fn have_simple_name_not_containing(infix: &str) -> ArchCondition<RustItem> {
    not(have_simple_name_containing(infix))
        .as_(format!("have simple name not containing '{infix}'"))
}

/// `have simple name ending with '<suffix>'`.
pub fn have_simple_name_ending_with(suffix: &str) -> ArchCondition<RustItem> {
    have(rust_item::predicates::simple_name_ending_with(suffix)).into()
}

/// `have simple name not ending with '<suffix>'`.
pub fn have_simple_name_not_ending_with(suffix: &str) -> ArchCondition<RustItem> {
    not(have_simple_name_ending_with(suffix))
        .as_(format!("have simple name not ending with '{suffix}'"))
}

/// `have name matching '<regex>'`.
pub fn have_name_matching<T: ConditionTarget + HasName>(regex: &str) -> ArchCondition<T> {
    have(has_name::predicates::name_matching(regex)).into()
}

/// `have name not matching '<regex>'`.
pub fn have_name_not_matching<T: ConditionTarget + HasName>(regex: &str) -> ArchCondition<T> {
    not(have_name_matching(regex)).as_(format!("have name not matching '{regex}'"))
}

/// `have full name matching '<regex>'`.
pub fn have_full_name_matching<T: ConditionTarget + HasFullName>(regex: &str) -> ArchCondition<T> {
    have(has_full_name::predicates::full_name_matching(regex)).into()
}

/// `have full name not matching '<regex>'`.
pub fn have_full_name_not_matching<T: ConditionTarget + HasFullName>(
    regex: &str,
) -> ArchCondition<T> {
    not(have_full_name_matching(regex)).as_(format!("have full name not matching '{regex}'"))
}

/// `have name starting with '<prefix>'`.
pub fn have_name_starting_with<T: ConditionTarget + HasName>(prefix: &str) -> ArchCondition<T> {
    have(has_name::predicates::name_starting_with(prefix)).into()
}

/// `have name not starting with '<prefix>'`.
pub fn have_name_not_starting_with<T: ConditionTarget + HasName>(prefix: &str) -> ArchCondition<T> {
    not(have_name_starting_with(prefix)).as_(format!("have name not starting with '{prefix}'"))
}

/// `have name containing '<infix>'`.
pub fn have_name_containing<T: ConditionTarget + HasName>(infix: &str) -> ArchCondition<T> {
    have(has_name::predicates::name_containing(infix)).into()
}

/// `have name not containing '<infix>'`.
pub fn have_name_not_containing<T: ConditionTarget + HasName>(infix: &str) -> ArchCondition<T> {
    not(have_name_containing(infix)).as_(format!("have name not containing '{infix}'"))
}

/// `have name ending with '<suffix>'`.
pub fn have_name_ending_with<T: ConditionTarget + HasName>(suffix: &str) -> ArchCondition<T> {
    have(has_name::predicates::name_ending_with(suffix)).into()
}

/// `have name not ending with '<suffix>'`.
pub fn have_name_not_ending_with<T: ConditionTarget + HasName>(suffix: &str) -> ArchCondition<T> {
    not(have_name_ending_with(suffix)).as_(format!("have name not ending with '{suffix}'"))
}

// ---- packages ------------------------------------------------------------------------------

/// `reside in a package '<identifier>'`.
pub fn reside_in_a_package(package_identifier: &str) -> ArchCondition<RustItem> {
    does(rust_item::predicates::reside_in_a_package(
        package_identifier,
    ))
    .into()
}

/// `reside in any package ['a', 'b']`.
pub fn reside_in_any_package(package_identifiers: &[&str]) -> ArchCondition<RustItem> {
    does(rust_item::predicates::reside_in_any_package(
        package_identifiers,
    ))
    .into()
}

/// `reside outside of package '<identifier>'`.
pub fn reside_outside_of_package(package_identifier: &str) -> ArchCondition<RustItem> {
    does(rust_item::predicates::reside_outside_of_package(
        package_identifier,
    ))
    .into()
}

/// `reside outside of packages ['a', 'b']`.
pub fn reside_outside_of_packages(package_identifiers: &[&str]) -> ArchCondition<RustItem> {
    does(rust_item::predicates::reside_outside_of_packages(
        package_identifiers,
    ))
    .into()
}

/// `reside in crate '<name>'`.
pub fn reside_in_crate(crate_name: &str) -> ArchCondition<RustItem> {
    does(rust_item::predicates::reside_in_crate(crate_name)).into()
}

// ---- modifiers -----------------------------------------------------------------------------

/// `have modifier <modifier>`.
pub fn have_modifier<T: ConditionTarget + HasModifiers>(
    modifier: RustModifier,
) -> ArchCondition<T> {
    have(has_modifiers::predicates::modifier(modifier)).into()
}

/// `not have modifier <modifier>`.
pub fn not_have_modifier<T: ConditionTarget + HasModifiers>(
    modifier: RustModifier,
) -> ArchCondition<T> {
    not(have_modifier(modifier))
}

macro_rules! modifier_conditions {
    ($($(#[$doc:meta])* $be:ident / $not_be:ident => $modifier:expr, $be_text:literal, $not_text:literal;)*) => {
        $(
            $(#[$doc])*
            pub fn $be<T: ConditionTarget + HasModifiers>() -> ArchCondition<T> {
                have_modifier($modifier).as_($be_text)
            }

            #[doc = concat!("`", $not_text, "`.")]
            pub fn $not_be<T: ConditionTarget + HasModifiers>() -> ArchCondition<T> {
                not(have_modifier($modifier)).as_($not_text)
            }
        )*
    };
}

modifier_conditions! {
    /// `be public`: `pub` without restriction.
    be_public / not_be_public => RustModifier::Pub, "be public", "not be public";
    /// `be protected`: restricted visibility (`pub(crate)`, `pub(super)`, `pub(in ..)`).
    be_protected / not_be_protected => RustModifier::PubRestricted, "be protected", "not be protected";
    /// `be package private`: inherited visibility (no `pub`).
    be_package_private / not_be_package_private => RustModifier::Private, "be package private", "not be package private";
    /// `be private`: inherited visibility (no `pub`); a synonym of `be package private` in Rust.
    be_private / not_be_private => RustModifier::Private, "be private", "not be private";
    /// `be static`: no `self` receiver, or an associated const / `static` / `const` item.
    be_static / not_be_static => RustModifier::Static, "be static", "not be static";
    /// `be final`: an inherent method, which cannot be overridden.
    be_final / not_be_final => RustModifier::Final, "be final", "not be final";
    /// `be unsafe` (Rust-only).
    be_unsafe / not_be_unsafe => RustModifier::Unsafe, "be unsafe", "not be unsafe";
    /// `be async` (Rust-only).
    be_async / not_be_async => RustModifier::Async, "be async", "not be async";
    /// `be pub(crate)` (Rust-only).
    be_pub_crate / not_be_pub_crate => RustModifier::PubCrate, "be pub(crate)", "not be pub(crate)";
}

fn have_only_modifiers<M>(
    description: &str,
    modifier: RustModifier,
    get: Getter<RustItem, M>,
) -> ArchCondition<RustItem>
where
    M: ConditionTarget + HasModifiers + Clone,
{
    let modifier_text = modifier.to_string();
    let inner: ArchCondition<M> =
        be(has_modifiers::predicates::modifier(modifier).as_(modifier_text)).into();
    all_attributes_match(format!("have only {description}"), inner, get)
}

/// `have only private fields` (Rust-only; the nearest analog of `haveOnlyFinalFields`).
pub fn have_only_private_fields() -> ArchCondition<RustItem> {
    have_only_modifiers(
        "private fields",
        RustModifier::Private,
        Arc::new(|item: &RustItem| item.fields()),
    )
}

/// `have only private constructors`: every constructor-classified function is private and the
/// type cannot be constructed with a literal from outside its module (it has a private field or
/// is `#[non_exhaustive]`).
pub fn have_only_private_constructors() -> ArchCondition<RustItem> {
    ArchCondition::new(
        "have only private constructors",
        |item: &RustItem, events: &mut ConditionEvents| {
            for constructor in item.constructors() {
                let private = constructor.has_modifier(&RustModifier::Private);
                let message = create_message(
                    &constructor,
                    if private {
                        "is private"
                    } else {
                        "is not private"
                    },
                );
                events.add(SimpleConditionEvent::new(&constructor, private, message));
            }
            if item.is_struct() {
                let sealed = item.has_modifier(&RustModifier::NonExhaustive)
                    || item
                        .fields()
                        .iter()
                        .any(|f| f.has_modifier(&RustModifier::Private));
                let message = if sealed {
                    "cannot be constructed with a struct literal outside its module"
                } else {
                    "can be constructed with a struct literal outside its module"
                };
                events.add(SimpleConditionEvent::new(
                    item,
                    sealed,
                    create_message(item, message),
                ));
            }
        },
    )
}

// ---- annotations, hierarchy, kinds ---------------------------------------------------------

/// `be annotated with @Name`.
pub fn be_annotated_with<T: ConditionTarget + CanBeAnnotated>(
    selector: impl Into<AnnotationSelector>,
) -> ArchCondition<T> {
    be(can_be_annotated::predicates::annotated_with(selector)).into()
}

/// `not be annotated with @Name`.
pub fn not_be_annotated_with<T: ConditionTarget + CanBeAnnotated>(
    selector: impl Into<AnnotationSelector>,
) -> ArchCondition<T> {
    not(be_annotated_with(selector))
}

/// `implement <trait>`.
pub fn implement(selector: impl Into<ItemSelector>) -> ArchCondition<RustItem> {
    does(rust_item::predicates::implement(selector)).into()
}

/// `not implement <trait>`.
pub fn not_implement(selector: impl Into<ItemSelector>) -> ArchCondition<RustItem> {
    not(implement(selector))
}

/// `be assignable to <type>`.
pub fn be_assignable_to(selector: impl Into<ItemSelector>) -> ArchCondition<RustItem> {
    be(rust_item::predicates::assignable_to(selector)).into()
}

/// `not be assignable to <type>`.
pub fn not_be_assignable_to(selector: impl Into<ItemSelector>) -> ArchCondition<RustItem> {
    not(be_assignable_to(selector))
}

/// `be assignable from <type>`.
pub fn be_assignable_from(selector: impl Into<ItemSelector>) -> ArchCondition<RustItem> {
    be(rust_item::predicates::assignable_from(selector)).into()
}

/// `not be assignable from <type>`.
pub fn not_be_assignable_from(selector: impl Into<ItemSelector>) -> ArchCondition<RustItem> {
    not(be_assignable_from(selector))
}

fn kind_condition(
    predicate: DescribedPredicate<RustItem>,
    positive: &'static str,
    negative: &'static str,
) -> ArchCondition<RustItem> {
    be(predicate)
        .describe_events_by(move |_, satisfied| {
            (if satisfied { positive } else { negative }).to_owned()
        })
        .into_condition()
}

/// `be interfaces` (traits).
pub fn be_interfaces() -> ArchCondition<RustItem> {
    kind_condition(
        rust_item::predicates::interfaces(),
        "is an interface",
        "is no interface",
    )
}

/// `not be interfaces`.
pub fn not_be_interfaces() -> ArchCondition<RustItem> {
    not(be_interfaces())
}

/// `be traits`; the Rust name for `be interfaces`.
pub fn be_traits() -> ArchCondition<RustItem> {
    kind_condition(rust_item::predicates::traits(), "is a trait", "is no trait")
}

/// `not be traits`.
pub fn not_be_traits() -> ArchCondition<RustItem> {
    not(be_traits())
}

/// `be enums`.
pub fn be_enums() -> ArchCondition<RustItem> {
    kind_condition(rust_item::predicates::enums(), "is an enum", "is no enum")
}

/// `not be enums`.
pub fn not_be_enums() -> ArchCondition<RustItem> {
    not(be_enums())
}

/// `be structs` (Rust-only).
pub fn be_structs() -> ArchCondition<RustItem> {
    kind_condition(
        rust_item::predicates::structs(),
        "is a struct",
        "is no struct",
    )
}

/// `not be structs`.
pub fn not_be_structs() -> ArchCondition<RustItem> {
    not(be_structs())
}

/// `be functions` (Rust-only).
pub fn be_functions() -> ArchCondition<RustItem> {
    kind_condition(
        rust_item::predicates::functions(),
        "is a function",
        "is no function",
    )
}

/// `not be functions`.
pub fn not_be_functions() -> ArchCondition<RustItem> {
    not(be_functions())
}

/// `be top level classes`.
pub fn be_top_level_classes() -> ArchCondition<RustItem> {
    kind_condition(
        rust_item::predicates::top_level_classes(),
        "is a top level class",
        "is no top level class",
    )
}

/// `not be top level classes`.
pub fn not_be_top_level_classes() -> ArchCondition<RustItem> {
    not(be_top_level_classes())
}

/// `be nested classes`.
pub fn be_nested_classes() -> ArchCondition<RustItem> {
    kind_condition(
        rust_item::predicates::nested_classes(),
        "is a nested class",
        "is no nested class",
    )
}

/// `not be nested classes`.
pub fn not_be_nested_classes() -> ArchCondition<RustItem> {
    not(be_nested_classes())
}

/// `be local classes`.
pub fn be_local_classes() -> ArchCondition<RustItem> {
    kind_condition(
        rust_item::predicates::local_classes(),
        "is a local class",
        "is no local class",
    )
}

/// `not be local classes`.
pub fn not_be_local_classes() -> ArchCondition<RustItem> {
    not(be_local_classes())
}

/// `contain number of elements <predicate>`: checked once over all objects in `finish`
/// (`NumberOfElementsCondition`).
pub fn contain_number_of_elements<T: ConditionTarget + HasFullName>(
    predicate: DescribedPredicate<usize>,
) -> ArchCondition<T> {
    struct Logic {
        predicate: DescribedPredicate<usize>,
        names: std::collections::BTreeSet<String>,
    }

    impl<T: HasFullName> ConditionLogic<T> for Logic {
        fn init(&mut self, _all: &[T]) {
            self.names.clear();
        }

        fn check(&mut self, item: &T, _events: &mut ConditionEvents) {
            self.names.insert(item.full_name());
        }

        fn finish(&mut self, events: &mut ConditionEvents) {
            let size = self.names.len();
            let satisfied = self.predicate.test(&size);
            let message = format!(
                "there is/are {size} element(s) in [{}]",
                self.names.iter().cloned().collect::<Vec<_>>().join(", ")
            );
            events.add(SimpleConditionEvent::new(size, satisfied, message));
        }
    }

    let description = format!("contain number of elements {}", predicate.description());
    ArchCondition::from_logic(
        description,
        Logic {
            predicate,
            names: std::collections::BTreeSet::new(),
        },
    )
}

// ---- members -------------------------------------------------------------------------------

/// `be declared in <owner>`.
pub fn be_declared_in<T: ConditionTarget + HasOwner<RustItem>>(
    selector: impl Into<ItemSelector>,
) -> ArchCondition<T> {
    be(rust_member::predicates::declared_in(selector)).into()
}

/// `not be declared in <owner>`.
pub fn not_be_declared_in<T: ConditionTarget + HasOwner<RustItem>>(
    selector: impl Into<ItemSelector>,
) -> ArchCondition<T> {
    not(be_declared_in(selector))
}

/// `be declared in classes that <predicate>`.
pub fn be_declared_in_classes_that<T: ConditionTarget + HasOwner<RustItem>>(
    predicate: DescribedPredicate<RustItem>,
) -> ArchCondition<T> {
    let description = format!("classes that {}", predicate.description());
    be(rust_member::predicates::declared_in(
        predicate.as_(description),
    ))
    .into()
}

/// `have raw type <type>`.
pub fn have_raw_type<T: ConditionTarget + HasType>(
    selector: impl Into<ItemSelector>,
) -> ArchCondition<T> {
    have(has_type::predicates::raw_type(selector)).into()
}

/// `have raw parameter types [a, b]`.
pub fn have_raw_parameter_types<T: ConditionTarget + HasParameterTypes>(
    parameter_type_names: &[&str],
) -> ArchCondition<T> {
    have(has_parameter_types::predicates::raw_parameter_types(
        parameter_type_names,
    ))
    .into()
}

/// `have raw parameter types <predicate>`.
pub fn have_raw_parameter_types_with<T: ConditionTarget + HasParameterTypes>(
    predicate: DescribedPredicate<Vec<Option<RustItem>>>,
) -> ArchCondition<T> {
    have(has_parameter_types::predicates::raw_parameter_types_with(
        predicate,
    ))
    .into()
}

/// `have raw return type <type>`.
pub fn have_raw_return_type<T: ConditionTarget + HasReturnType>(
    selector: impl Into<ItemSelector>,
) -> ArchCondition<T> {
    have(has_return_type::predicates::raw_return_type(selector)).into()
}

/// `declare throwable of type <type>`: returns `Result<_, E>` with a matching `E`.
pub fn declare_throwable_of_type<T: ConditionTarget + HasErrorTypes>(
    selector: impl Into<ItemSelector>,
) -> ArchCondition<T> {
    let selector = selector.into();
    let description = format!("declare throwable of type {}", selector.description());
    does(has_error_types::predicates::error_types_containing(selector).as_(description)).into()
}

fn code_unit_only_calls<T: CodeUnitLike>(
    description: String,
    predicate: DescribedPredicate<RustAccess>,
) -> ArchCondition<T> {
    all_attributes_match(
        description,
        access_condition(predicate),
        Arc::new(|unit: &T| unit.calls_of_self()),
    )
}

/// `only be called by classes that <predicate>`.
pub fn only_be_called_by_classes_that<T: CodeUnitLike>(
    predicate: DescribedPredicate<RustItem>,
) -> ArchCondition<T> {
    let description = format!("only be called by classes that {}", predicate.description());
    code_unit_only_calls(description, origin_owner(predicate))
}

/// `only be called by code units that <predicate>`.
pub fn only_be_called_by_code_units_that<T: CodeUnitLike>(
    predicate: DescribedPredicate<RustCodeUnit>,
) -> ArchCondition<T> {
    let description = format!(
        "only be called by code units that {}",
        predicate.description()
    );
    code_unit_only_calls(description, rust_access::predicates::origin(predicate))
}

/// `only be called by methods that <predicate>`.
pub fn only_be_called_by_methods_that<T: CodeUnitLike>(
    predicate: DescribedPredicate<RustMethod>,
) -> ArchCondition<T> {
    let description = format!("only be called by methods that {}", predicate.description());
    let origin_predicate = DescribedPredicate::describe(
        format!("matching RustMethod {}", predicate.description()),
        move |unit: &RustCodeUnit| {
            unit.is_method() && unit.as_method().is_some_and(|m| predicate.test(&m))
        },
    );
    code_unit_only_calls(
        description,
        rust_access::predicates::origin(origin_predicate),
    )
}

/// `only be called by constructors that <predicate>`.
pub fn only_be_called_by_constructors_that<T: CodeUnitLike>(
    predicate: DescribedPredicate<RustConstructor>,
) -> ArchCondition<T> {
    let description = format!(
        "only be called by constructors that {}",
        predicate.description()
    );
    let origin_predicate = DescribedPredicate::describe(
        format!("matching RustConstructor {}", predicate.description()),
        move |unit: &RustCodeUnit| unit.as_constructor().is_some_and(|c| predicate.test(&c)),
    );
    code_unit_only_calls(
        description,
        rust_access::predicates::origin(origin_predicate),
    )
}

/// `be accessed by methods that <predicate>`: one event per access from a method.
pub fn be_accessed_by_methods_that(
    predicate: DescribedPredicate<RustMethod>,
) -> ArchCondition<RustField> {
    let description = format!("be accessed by methods that {}", predicate.description());
    ArchCondition::new(
        description,
        move |field: &RustField, events: &mut ConditionEvents| {
            for access in field.accesses_to_self() {
                if let Some(method) = access.origin().as_method() {
                    if method.is_method() {
                        let satisfied = predicate.test(&method);
                        events.add(SimpleConditionEvent::new(
                            field,
                            satisfied,
                            access.description(),
                        ));
                    }
                }
            }
        },
    )
}

/// Which access kinds a condition considers; used by Rust-only helpers.
pub fn access_kind_is(kind: AccessKind) -> DescribedPredicate<RustAccess> {
    DescribedPredicate::describe(format!("access kind {kind:?}"), move |a: &RustAccess| {
        a.kind() == kind
    })
}
