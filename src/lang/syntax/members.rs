use super::aggregators::{ConditionAggregator, PrepareCondition, prepend_should};
use super::classes::ClassesThat;
use super::objects::{GivenState, ShouldState};
use crate::base::{DescribedPredicate, HasDescription, do_not, not};
use crate::core::domain::properties::{
    AnnotationSelector, ItemSelector, can_be_annotated, has_error_types, has_full_name,
    has_modifiers, has_name, has_parameter_types, has_return_type, has_type,
};
use crate::core::domain::{
    CodeUnitLike, MemberLike, RustCodeUnit, RustConstructor, RustField, RustItem, RustItems,
    RustMember, RustMethod, RustModifier, rust_member,
};
use crate::lang::condition::ArchCondition;
use crate::lang::conditions as cond;
use crate::lang::conditions::predicates::{are, have};
use crate::lang::evaluation::EvaluationResult;
use crate::lang::rule::{ArchRule, ClassesTransformer, Priority, SimpleArchRule};

// ---- given ---------------------------------------------------------------------------------

/// The start of a rule about members (`GivenMembers<MEMBER>` / `GivenMembersConjunction`).
///
/// `M` selects the kind: [`RustMember`], [`RustField`], [`RustCodeUnit`], [`RustMethod`] or
/// [`RustConstructor`]; see the type aliases [`GivenFields`], [`GivenCodeUnits`],
/// [`GivenMethods`], [`GivenConstructors`].
#[derive(Clone)]
pub struct GivenMembers<M: MemberLike> {
    state: GivenState<M>,
}

/// `GivenMembersConjunction<MEMBER>`: the same type as [`GivenMembers`].
pub type GivenMembersConjunction<M> = GivenMembers<M>;
/// `GivenFields`.
pub type GivenFields = GivenMembers<RustField>;
/// `GivenFieldsConjunction`.
pub type GivenFieldsConjunction = GivenMembers<RustField>;
/// `GivenCodeUnits<JavaCodeUnit>`.
pub type GivenCodeUnits = GivenMembers<RustCodeUnit>;
/// `GivenCodeUnitsConjunction`.
pub type GivenCodeUnitsConjunction = GivenMembers<RustCodeUnit>;
/// `GivenMethods`.
pub type GivenMethods = GivenMembers<RustMethod>;
/// `GivenMethodsConjunction`.
pub type GivenMethodsConjunction = GivenMembers<RustMethod>;
/// `GivenConstructors`.
pub type GivenConstructors = GivenMembers<RustConstructor>;
/// `GivenConstructorsConjunction`.
pub type GivenConstructorsConjunction = GivenMembers<RustConstructor>;

impl<M: MemberLike> std::fmt::Debug for GivenMembers<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GivenMembers({})", self.state.transformer.description())
    }
}

impl<M: MemberLike> GivenMembers<M> {
    pub(crate) fn new(
        priority: Priority,
        transformer: ClassesTransformer<M>,
        prepare: PrepareCondition<M>,
    ) -> Self {
        Self {
            state: GivenState::new(priority, transformer, prepare),
        }
    }

    fn with_predicate(
        self,
        predicate: DescribedPredicate<M>,
        mode: fn(
            &super::aggregators::PredicateAggregator<M>,
        ) -> super::aggregators::PredicateAggregator<M>,
    ) -> Self {
        let predicates = mode(&self.state.predicates).add(predicate);
        Self {
            state: self.state.with(predicates),
        }
    }

    /// `that()`.
    pub fn that(self) -> MembersThat<M> {
        MembersThat {
            given: self,
            mode: |p| p.clone(),
        }
    }

    /// `that(predicate)`.
    pub fn that_with(self, predicate: DescribedPredicate<M>) -> GivenMembersConjunction<M> {
        self.with_predicate(predicate, |p| p.clone())
    }

    /// `and()`.
    pub fn and(self) -> MembersThat<M> {
        MembersThat {
            given: self,
            mode: |p| p.that_ands(),
        }
    }

    /// `and(predicate)`.
    pub fn and_with(self, predicate: DescribedPredicate<M>) -> GivenMembersConjunction<M> {
        self.with_predicate(predicate, |p| p.that_ands())
    }

    /// `or()`.
    pub fn or(self) -> MembersThat<M> {
        MembersThat {
            given: self,
            mode: |p| p.that_ors(),
        }
    }

    /// `or(predicate)`.
    pub fn or_with(self, predicate: DescribedPredicate<M>) -> GivenMembersConjunction<M> {
        self.with_predicate(predicate, |p| p.that_ors())
    }

    /// `should()`.
    pub fn should(self) -> MembersShould<M> {
        MembersShould {
            state: ShouldState {
                transformer: self.state.finished_transformer(),
                priority: self.state.priority,
                aggregator: ConditionAggregator::new(),
                prepare: self.state.prepare.clone(),
            },
        }
    }

    /// `should(condition)`.
    pub fn should_with(
        self,
        condition: impl Into<ArchCondition<M>>,
    ) -> MembersShouldConjunction<M> {
        MembersShouldConjunction {
            state: ShouldState {
                transformer: self.state.finished_transformer(),
                priority: self.state.priority,
                aggregator: ConditionAggregator::with(condition.into()),
                prepare: self.state.prepare.clone(),
            },
        }
    }
}

// ---- that ----------------------------------------------------------------------------------

/// The fluent predicates on members (`MembersThat`, `FieldsThat`, `CodeUnitsThat`, `MethodsThat`).
pub struct MembersThat<M: MemberLike> {
    given: GivenMembers<M>,
    mode: fn(
        &super::aggregators::PredicateAggregator<M>,
    ) -> super::aggregators::PredicateAggregator<M>,
}

/// `FieldsThat`.
pub type FieldsThat = MembersThat<RustField>;
/// `CodeUnitsThat`.
pub type CodeUnitsThat = MembersThat<RustCodeUnit>;
/// `MethodsThat`.
pub type MethodsThat = MembersThat<RustMethod>;
/// `ConstructorsThat`.
pub type ConstructorsThat = MembersThat<RustConstructor>;

impl<M: MemberLike> std::fmt::Debug for MembersThat<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MembersThat")
    }
}

macro_rules! members_that_methods {
    ($m:ty; $($(#[$doc:meta])* fn $name:ident($($arg:ident: $ty:ty),*) => $body:expr;)*) => {
        $(
            $(#[$doc])*
            pub fn $name(self, $($arg: $ty),*) -> GivenMembersConjunction<$m> {
                self.given_with($body)
            }
        )*
    };
}

impl<M: MemberLike> MembersThat<M> {
    fn given_with(self, predicate: DescribedPredicate<M>) -> GivenMembersConjunction<M> {
        self.given.with_predicate(predicate, self.mode)
    }

    /// Adds an arbitrary predicate (the `that(predicate)` overload).
    pub fn satisfy(self, predicate: DescribedPredicate<M>) -> GivenMembersConjunction<M> {
        self.given_with(predicate)
    }

    members_that_methods! { M;
        /// `have name '<name>'`.
        fn have_name(name: &str) => have(has_name::predicates::name(name));
        /// `do not have name '<name>'`.
        fn do_not_have_name(name: &str) => do_not(have(has_name::predicates::name(name)));
        /// `have name matching '<regex>'`.
        fn have_name_matching(regex: &str) => have(has_name::predicates::name_matching(regex));
        /// `have name not matching '<regex>'`.
        fn have_name_not_matching(regex: &str) => have(not(has_name::predicates::name_matching(regex)).as_(format!("name not matching '{regex}'")));
        /// `have full name '<name>'`.
        fn have_full_name(full_name: &str) => have(has_full_name::predicates::full_name(full_name));
        /// `do not have full name '<name>'`.
        fn do_not_have_full_name(full_name: &str) => do_not(have(has_full_name::predicates::full_name(full_name)));
        /// `have full name matching '<regex>'`.
        fn have_full_name_matching(regex: &str) => have(has_full_name::predicates::full_name_matching(regex));
        /// `have full name not matching '<regex>'`.
        fn have_full_name_not_matching(regex: &str) => have(not(has_full_name::predicates::full_name_matching(regex)).as_(format!("full name not matching '{regex}'")));
        /// `have name starting with '<prefix>'`.
        fn have_name_starting_with(prefix: &str) => have(has_name::predicates::name_starting_with(prefix));
        /// `have name not starting with '<prefix>'`.
        fn have_name_not_starting_with(prefix: &str) => have(not(has_name::predicates::name_starting_with(prefix)).as_(format!("name not starting with '{prefix}'")));
        /// `have name containing '<infix>'`.
        fn have_name_containing(infix: &str) => have(has_name::predicates::name_containing(infix));
        /// `have name not containing '<infix>'`.
        fn have_name_not_containing(infix: &str) => have(not(has_name::predicates::name_containing(infix)).as_(format!("name not containing '{infix}'")));
        /// `have name ending with '<suffix>'`.
        fn have_name_ending_with(suffix: &str) => have(has_name::predicates::name_ending_with(suffix));
        /// `have name not ending with '<suffix>'`.
        fn have_name_not_ending_with(suffix: &str) => have(not(has_name::predicates::name_ending_with(suffix)).as_(format!("name not ending with '{suffix}'")));
        /// `are public`.
        fn are_public() => has_modifiers::predicates::modifier(RustModifier::Pub).as_("are public");
        /// `are not public`.
        fn are_not_public() => not(has_modifiers::predicates::modifier(RustModifier::Pub)).as_("are not public");
        /// `are protected` (restricted visibility).
        fn are_protected() => has_modifiers::predicates::modifier(RustModifier::PubRestricted).as_("are protected");
        /// `are not protected`.
        fn are_not_protected() => not(has_modifiers::predicates::modifier(RustModifier::PubRestricted)).as_("are not protected");
        /// `are package private` (inherited visibility).
        fn are_package_private() => has_modifiers::predicates::modifier(RustModifier::Private).as_("are package private");
        /// `are not package private`.
        fn are_not_package_private() => not(has_modifiers::predicates::modifier(RustModifier::Private)).as_("are not package private");
        /// `are private` (inherited visibility).
        fn are_private() => has_modifiers::predicates::modifier(RustModifier::Private).as_("are private");
        /// `are not private`.
        fn are_not_private() => not(has_modifiers::predicates::modifier(RustModifier::Private)).as_("are not private");
        /// `are static`.
        fn are_static() => has_modifiers::predicates::modifier(RustModifier::Static).as_("are static");
        /// `are not static`.
        fn are_not_static() => not(has_modifiers::predicates::modifier(RustModifier::Static)).as_("are not static");
        /// `are final` (inherent methods).
        fn are_final() => has_modifiers::predicates::modifier(RustModifier::Final).as_("are final");
        /// `are not final`.
        fn are_not_final() => not(has_modifiers::predicates::modifier(RustModifier::Final)).as_("are not final");
        /// `are unsafe` (Rust-only).
        fn are_unsafe() => has_modifiers::predicates::modifier(RustModifier::Unsafe).as_("are unsafe");
        /// `are not unsafe` (Rust-only).
        fn are_not_unsafe() => not(has_modifiers::predicates::modifier(RustModifier::Unsafe)).as_("are not unsafe");
        /// `are async` (Rust-only).
        fn are_async() => has_modifiers::predicates::modifier(RustModifier::Async).as_("are async");
        /// `are not async` (Rust-only).
        fn are_not_async() => not(has_modifiers::predicates::modifier(RustModifier::Async)).as_("are not async");
        /// `have modifier <modifier>`.
        fn have_modifier(modifier: RustModifier) => have(has_modifiers::predicates::modifier(modifier));
        /// `do not have modifier <modifier>`.
        fn do_not_have_modifier(modifier: RustModifier) => do_not(have(has_modifiers::predicates::modifier(modifier)));
        /// `are annotated with @Name`.
        fn are_annotated_with(selector: impl Into<AnnotationSelector>) => are(can_be_annotated::predicates::annotated_with(selector));
        /// `are not annotated with @Name`.
        fn are_not_annotated_with(selector: impl Into<AnnotationSelector>) => are(not(can_be_annotated::predicates::annotated_with(selector)));
        /// `are declared in <owner>`.
        fn are_declared_in(selector: impl Into<ItemSelector>) => are(rust_member::predicates::declared_in(selector));
        /// `are not declared in <owner>`.
        fn are_not_declared_in(selector: impl Into<ItemSelector>) => are(not(rust_member::predicates::declared_in(selector)));
        /// `are declared in classes that <predicate>`.
        fn are_declared_in_classes_that_with(predicate: DescribedPredicate<RustItem>) => are(declared_in_classes_that(predicate));
    }

    /// `are declared in classes that ..` continued with fluent class predicates.
    pub fn are_declared_in_classes_that(self) -> ClassesThat<GivenMembersConjunction<M>> {
        ClassesThat::new(move |predicate| self.given_with(are(declared_in_classes_that(predicate))))
    }
}

fn declared_in_classes_that<M: MemberLike>(
    predicate: DescribedPredicate<RustItem>,
) -> DescribedPredicate<M> {
    let description = format!("declared in classes that {}", predicate.description());
    rust_member::predicates::declared_in(predicate).as_(description)
}

impl MembersThat<RustField> {
    members_that_methods! { RustField;
        /// `have raw type <type>`.
        fn have_raw_type(selector: impl Into<ItemSelector>) => have(has_type::predicates::raw_type(selector));
        /// `do not have raw type <type>`.
        fn do_not_have_raw_type(selector: impl Into<ItemSelector>) => do_not(have(has_type::predicates::raw_type(selector)));
    }
}

impl<M: CodeUnitLike> MembersThat<M> {
    members_that_methods! { M;
        /// `have raw parameter types [a, b]`.
        fn have_raw_parameter_types(parameter_type_names: &[&str]) => have(has_parameter_types::predicates::raw_parameter_types(parameter_type_names));
        /// `do not have raw parameter types [a, b]`.
        fn do_not_have_raw_parameter_types(parameter_type_names: &[&str]) => do_not(have(has_parameter_types::predicates::raw_parameter_types(parameter_type_names)));
        /// `have raw parameter types <predicate>`.
        fn have_raw_parameter_types_with(predicate: DescribedPredicate<Vec<Option<RustItem>>>) => have(has_parameter_types::predicates::raw_parameter_types_with(predicate));
        /// `do not have raw parameter types <predicate>`.
        fn do_not_have_raw_parameter_types_with(predicate: DescribedPredicate<Vec<Option<RustItem>>>) => do_not(have(has_parameter_types::predicates::raw_parameter_types_with(predicate)));
        /// `have raw return type <type>`.
        fn have_raw_return_type(selector: impl Into<ItemSelector>) => have(has_return_type::predicates::raw_return_type(selector));
        /// `do not have raw return type <type>`.
        fn do_not_have_raw_return_type(selector: impl Into<ItemSelector>) => do_not(have(has_return_type::predicates::raw_return_type(selector)));
        /// `declare throwable of type <type>`: return `Result<_, E>` with a matching `E`.
        fn declare_throwable_of_type(selector: impl Into<ItemSelector>) => declare_throwable_of_type_predicate(selector);
        /// `do not declare throwable of type <type>`.
        fn do_not_declare_throwable_of_type(selector: impl Into<ItemSelector>) => do_not(declare_throwable_of_type_predicate(selector));
    }
}

fn declare_throwable_of_type_predicate<M: CodeUnitLike>(
    selector: impl Into<ItemSelector>,
) -> DescribedPredicate<M> {
    let selector = selector.into();
    let description = format!("declare throwable of type {}", selector.description());
    has_error_types::predicates::error_types_containing(selector).as_(description)
}

// ---- should --------------------------------------------------------------------------------

/// The fluent conditions on members (`MembersShould`, `FieldsShould`, `CodeUnitsShould`,
/// `MethodsShould`).
#[derive(Clone)]
pub struct MembersShould<M: MemberLike> {
    state: ShouldState<M>,
}

/// `FieldsShould`.
pub type FieldsShould = MembersShould<RustField>;
/// `CodeUnitsShould`.
pub type CodeUnitsShould = MembersShould<RustCodeUnit>;
/// `MethodsShould`.
pub type MethodsShould = MembersShould<RustMethod>;
/// `ConstructorsShould`.
pub type ConstructorsShould = MembersShould<RustConstructor>;

impl<M: MemberLike> std::fmt::Debug for MembersShould<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MembersShould({})", self.state.transformer.description())
    }
}

/// A finished rule about members that can be extended with `and_should()` / `or_should()`
/// (`MembersShouldConjunction<MEMBER>`).
#[derive(Clone)]
pub struct MembersShouldConjunction<M: MemberLike> {
    state: ShouldState<M>,
}

/// `FieldsShouldConjunction`.
pub type FieldsShouldConjunction = MembersShouldConjunction<RustField>;
/// `CodeUnitsShouldConjunction`.
pub type CodeUnitsShouldConjunction = MembersShouldConjunction<RustCodeUnit>;
/// `MethodsShouldConjunction`.
pub type MethodsShouldConjunction = MembersShouldConjunction<RustMethod>;
/// `ConstructorsShouldConjunction`.
pub type ConstructorsShouldConjunction = MembersShouldConjunction<RustConstructor>;

impl<M: MemberLike> std::fmt::Debug for MembersShouldConjunction<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MembersShouldConjunction({})", self.description())
    }
}

macro_rules! members_should_methods {
    ($m:ty; $($(#[$doc:meta])* fn $name:ident($($arg:ident: $ty:ty),*) => $body:expr;)*) => {
        $(
            $(#[$doc])*
            pub fn $name(self, $($arg: $ty),*) -> MembersShouldConjunction<$m> {
                self.add_condition($body)
            }
        )*
    };
}

impl<M: MemberLike> MembersShould<M> {
    fn add_condition(self, condition: impl Into<ArchCondition<M>>) -> MembersShouldConjunction<M> {
        MembersShouldConjunction {
            state: self.state.add_condition(condition.into()),
        }
    }

    /// Adds an arbitrary condition (the `should(condition)` overload).
    pub fn satisfy(self, condition: impl Into<ArchCondition<M>>) -> MembersShouldConjunction<M> {
        self.add_condition(condition)
    }

    members_should_methods! { M;
        /// `have name '<name>'`.
        fn have_name(name: &str) => cond::have_name::<M>(name);
        /// `not have name '<name>'`.
        fn not_have_name(name: &str) => cond::not_have_name::<M>(name);
        /// `have name matching '<regex>'`.
        fn have_name_matching(regex: &str) => cond::have_name_matching::<M>(regex);
        /// `have name not matching '<regex>'`.
        fn have_name_not_matching(regex: &str) => cond::have_name_not_matching::<M>(regex);
        /// `have full name '<name>'`.
        fn have_full_name(full_name: &str) => cond::have_full_name::<M>(full_name);
        /// `not have full name '<name>'`.
        fn not_have_full_name(full_name: &str) => cond::not_have_full_name::<M>(full_name);
        /// `have full name matching '<regex>'`.
        fn have_full_name_matching(regex: &str) => cond::have_full_name_matching::<M>(regex);
        /// `have full name not matching '<regex>'`.
        fn have_full_name_not_matching(regex: &str) => cond::have_full_name_not_matching::<M>(regex);
        /// `have name starting with '<prefix>'`.
        fn have_name_starting_with(prefix: &str) => cond::have_name_starting_with::<M>(prefix);
        /// `have name not starting with '<prefix>'`.
        fn have_name_not_starting_with(prefix: &str) => cond::have_name_not_starting_with::<M>(prefix);
        /// `have name containing '<infix>'`.
        fn have_name_containing(infix: &str) => cond::have_name_containing::<M>(infix);
        /// `have name not containing '<infix>'`.
        fn have_name_not_containing(infix: &str) => cond::have_name_not_containing::<M>(infix);
        /// `have name ending with '<suffix>'`.
        fn have_name_ending_with(suffix: &str) => cond::have_name_ending_with::<M>(suffix);
        /// `have name not ending with '<suffix>'`.
        fn have_name_not_ending_with(suffix: &str) => cond::have_name_not_ending_with::<M>(suffix);
        /// `be public`.
        fn be_public() => cond::be_public::<M>();
        /// `not be public`.
        fn not_be_public() => cond::not_be_public::<M>();
        /// `be protected` (restricted visibility).
        fn be_protected() => cond::be_protected::<M>();
        /// `not be protected`.
        fn not_be_protected() => cond::not_be_protected::<M>();
        /// `be package private` (inherited visibility).
        fn be_package_private() => cond::be_package_private::<M>();
        /// `not be package private`.
        fn not_be_package_private() => cond::not_be_package_private::<M>();
        /// `be private` (inherited visibility).
        fn be_private() => cond::be_private::<M>();
        /// `not be private`.
        fn not_be_private() => cond::not_be_private::<M>();
        /// `be static`.
        fn be_static() => cond::be_static::<M>();
        /// `not be static`.
        fn not_be_static() => cond::not_be_static::<M>();
        /// `be final` (inherent methods).
        fn be_final() => cond::be_final::<M>();
        /// `not be final`.
        fn not_be_final() => cond::not_be_final::<M>();
        /// `be unsafe` (Rust-only).
        fn be_unsafe() => cond::be_unsafe::<M>();
        /// `not be unsafe` (Rust-only).
        fn not_be_unsafe() => cond::not_be_unsafe::<M>();
        /// `be async` (Rust-only).
        fn be_async() => cond::be_async::<M>();
        /// `not be async` (Rust-only).
        fn not_be_async() => cond::not_be_async::<M>();
        /// `have modifier <modifier>`.
        fn have_modifier(modifier: RustModifier) => cond::have_modifier::<M>(modifier);
        /// `not have modifier <modifier>`.
        fn not_have_modifier(modifier: RustModifier) => cond::not_have_modifier::<M>(modifier);
        /// `be annotated with @Name`.
        fn be_annotated_with(selector: impl Into<AnnotationSelector>) => cond::be_annotated_with::<M>(selector);
        /// `not be annotated with @Name`.
        fn not_be_annotated_with(selector: impl Into<AnnotationSelector>) => cond::not_be_annotated_with::<M>(selector);
        /// `be declared in <owner>`.
        fn be_declared_in(selector: impl Into<ItemSelector>) => cond::be_declared_in::<M>(selector);
        /// `not be declared in <owner>`.
        fn not_be_declared_in(selector: impl Into<ItemSelector>) => cond::not_be_declared_in::<M>(selector);
        /// `be declared in classes that <predicate>`.
        fn be_declared_in_classes_that_with(predicate: DescribedPredicate<RustItem>) => cond::be_declared_in_classes_that::<M>(predicate);
        /// `contain number of elements <predicate>`.
        fn contain_number_of_elements(predicate: DescribedPredicate<usize>) => cond::contain_number_of_elements::<M>(predicate);
    }

    /// `be declared in classes that ..` continued with fluent class predicates.
    pub fn be_declared_in_classes_that(self) -> ClassesThat<MembersShouldConjunction<M>> {
        ClassesThat::new(move |predicate| {
            self.add_condition(cond::be_declared_in_classes_that::<M>(predicate))
        })
    }
}

impl MembersShould<RustField> {
    members_should_methods! { RustField;
        /// `have raw type <type>`.
        fn have_raw_type(selector: impl Into<ItemSelector>) => cond::have_raw_type::<RustField>(selector);
        /// `not have raw type <type>`.
        fn not_have_raw_type(selector: impl Into<ItemSelector>) => cond::not(cond::have_raw_type::<RustField>(selector));
        /// `be accessed by methods that <predicate>`.
        fn be_accessed_by_methods_that(predicate: DescribedPredicate<RustMethod>) => cond::be_accessed_by_methods_that(predicate);
        /// `not be accessed by methods that <predicate>`.
        fn not_be_accessed_by_methods_that(predicate: DescribedPredicate<RustMethod>) => cond::not(cond::be_accessed_by_methods_that(predicate));
    }
}

impl<M: CodeUnitLike> MembersShould<M> {
    members_should_methods! { M;
        /// `have raw parameter types [a, b]`.
        fn have_raw_parameter_types(parameter_type_names: &[&str]) => cond::have_raw_parameter_types::<M>(parameter_type_names);
        /// `not have raw parameter types [a, b]`.
        fn not_have_raw_parameter_types(parameter_type_names: &[&str]) => cond::not(cond::have_raw_parameter_types::<M>(parameter_type_names));
        /// `have raw parameter types <predicate>`.
        fn have_raw_parameter_types_with(predicate: DescribedPredicate<Vec<Option<RustItem>>>) => cond::have_raw_parameter_types_with::<M>(predicate);
        /// `not have raw parameter types <predicate>`.
        fn not_have_raw_parameter_types_with(predicate: DescribedPredicate<Vec<Option<RustItem>>>) => cond::not(cond::have_raw_parameter_types_with::<M>(predicate));
        /// `have raw return type <type>`.
        fn have_raw_return_type(selector: impl Into<ItemSelector>) => cond::have_raw_return_type::<M>(selector);
        /// `not have raw return type <type>`.
        fn not_have_raw_return_type(selector: impl Into<ItemSelector>) => cond::not(cond::have_raw_return_type::<M>(selector));
        /// `declare throwable of type <type>`.
        fn declare_throwable_of_type(selector: impl Into<ItemSelector>) => cond::declare_throwable_of_type::<M>(selector);
        /// `not declare throwable of type <type>`.
        fn not_declare_throwable_of_type(selector: impl Into<ItemSelector>) => cond::not(cond::declare_throwable_of_type::<M>(selector));
    }

    /// `only be called ..` (`OnlyBeCalledSpecification`).
    pub fn only_be_called(self) -> OnlyBeCalledSpecification<M> {
        OnlyBeCalledSpecification { should: self }
    }
}

/// `only be called ..` continuation (`OnlyBeCalledSpecification<CONJUNCTION>`).
#[derive(Debug, Clone)]
pub struct OnlyBeCalledSpecification<M: CodeUnitLike> {
    should: MembersShould<M>,
}

impl<M: CodeUnitLike> OnlyBeCalledSpecification<M> {
    /// `only be called by classes that <predicate>`.
    pub fn by_classes_that_with(
        self,
        predicate: DescribedPredicate<RustItem>,
    ) -> MembersShouldConjunction<M> {
        self.should
            .add_condition(cond::only_be_called_by_classes_that::<M>(predicate))
    }

    /// `only be called by classes that ..` continued with fluent class predicates.
    pub fn by_classes_that(self) -> ClassesThat<MembersShouldConjunction<M>> {
        ClassesThat::new(move |predicate| {
            self.should
                .add_condition(cond::only_be_called_by_classes_that::<M>(predicate))
        })
    }

    /// `only be called by code units that <predicate>`.
    pub fn by_code_units_that(
        self,
        predicate: DescribedPredicate<RustCodeUnit>,
    ) -> MembersShouldConjunction<M> {
        self.should
            .add_condition(cond::only_be_called_by_code_units_that::<M>(predicate))
    }

    /// `only be called by methods that <predicate>`.
    pub fn by_methods_that(
        self,
        predicate: DescribedPredicate<RustMethod>,
    ) -> MembersShouldConjunction<M> {
        self.should
            .add_condition(cond::only_be_called_by_methods_that::<M>(predicate))
    }

    /// `only be called by constructors that <predicate>`.
    pub fn by_constructors_that(
        self,
        predicate: DescribedPredicate<RustConstructor>,
    ) -> MembersShouldConjunction<M> {
        self.should
            .add_condition(cond::only_be_called_by_constructors_that::<M>(predicate))
    }
}

impl<M: MemberLike> HasDescription for MembersShouldConjunction<M> {
    fn description(&self) -> String {
        self.state.finished_rule().description()
    }
}

impl<M: MemberLike> ArchRule for MembersShouldConjunction<M> {
    fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        self.state.finished_rule().evaluate(items)
    }

    fn clone_boxed(&self) -> Box<dyn ArchRule> {
        Box::new(self.clone())
    }

    fn allow_empty_should_boxed(&self, allow_empty_should: bool) -> Box<dyn ArchRule> {
        Box::new(
            self.state
                .finished_rule()
                .allow_empty_should(allow_empty_should),
        )
    }
}

impl<M: MemberLike> MembersShouldConjunction<M> {
    /// The rule's description.
    pub fn description(&self) -> String {
        HasDescription::description(self)
    }

    /// `and_should()`.
    pub fn and_should(self) -> MembersShould<M> {
        MembersShould {
            state: self
                .state
                .with_aggregator(self.state.aggregator.that_ands_with(prepend_should())),
        }
    }

    /// `and_should(condition)`.
    pub fn and_should_with(
        self,
        condition: impl Into<ArchCondition<M>>,
    ) -> MembersShouldConjunction<M> {
        let aggregator = self.state.aggregator.that_ands_with(prepend_should());
        MembersShouldConjunction {
            state: self
                .state
                .with_aggregator(ConditionAggregator::with(aggregator.add(condition.into()))),
        }
    }

    /// `or_should()`.
    pub fn or_should(self) -> MembersShould<M> {
        MembersShould {
            state: self
                .state
                .with_aggregator(self.state.aggregator.that_ors_with(prepend_should())),
        }
    }

    /// `or_should(condition)`.
    pub fn or_should_with(
        self,
        condition: impl Into<ArchCondition<M>>,
    ) -> MembersShouldConjunction<M> {
        let aggregator = self.state.aggregator.that_ors_with(prepend_should());
        MembersShouldConjunction {
            state: self
                .state
                .with_aggregator(ConditionAggregator::with(aggregator.add(condition.into()))),
        }
    }

    /// `because(reason)`.
    pub fn because(self, reason: &str) -> SimpleArchRule<M> {
        self.state.finished_rule().because(reason)
    }

    /// `as(description)`.
    pub fn as_(self, description: &str) -> SimpleArchRule<M> {
        self.state.finished_rule().as_(description)
    }

    /// `allowEmptyShould(..)`.
    pub fn allow_empty_should(self, allow_empty_should: bool) -> SimpleArchRule<M> {
        self.state
            .finished_rule()
            .allow_empty_should(allow_empty_should)
    }

    /// The finished [`SimpleArchRule`].
    pub fn into_rule(self) -> SimpleArchRule<M> {
        self.state.finished_rule()
    }
}

/// The transformer behind `members()`, `fields()`, ...: all members of kind `M` of all classes.
pub(crate) fn members_transformer<M: MemberLike>() -> ClassesTransformer<M> {
    ClassesTransformer::new(M::plural(), |items: &RustItems| {
        items.iter().flat_map(|item| M::members_of(&item)).collect()
    })
}

#[allow(dead_code)]
fn _assert_member_types(
    _: RustMember,
    _: RustField,
    _: RustCodeUnit,
    _: RustMethod,
    _: RustConstructor,
) {
}
