use super::aggregators::{ConditionAggregator, PrepareCondition, prepend_should};
use super::objects::{GivenState, ShouldState};
use crate::base::{DescribedPredicate, HasDescription, do_not, not};
use crate::core::domain::properties::{
    AnnotationSelector, ItemSelector, can_be_annotated, has_modifiers, has_name,
};
use crate::core::domain::{
    RustAccess, RustCodeUnit, RustConstructor, RustField, RustItem, RustItems, RustMember,
    RustMethod, RustModifier, rust_item::predicates as item,
};
use crate::lang::condition::ArchCondition;
use crate::lang::conditions as cond;
use crate::lang::conditions::predicates::{are, have};
use crate::lang::evaluation::EvaluationResult;
use crate::lang::rule::{ArchRule, ClassesTransformer, Priority, SimpleArchRule};

// ---- given ---------------------------------------------------------------------------------

/// The start of a rule about classes (`GivenClasses` / `GivenClassesConjunction`).
#[derive(Clone)]
pub struct GivenClasses {
    state: GivenState<RustItem>,
}

/// `GivenClassesConjunction`: the same type as [`GivenClasses`].
pub type GivenClassesConjunction = GivenClasses;

impl std::fmt::Debug for GivenClasses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GivenClasses({})", self.state.transformer.description())
    }
}

impl GivenClasses {
    pub(crate) fn new(
        priority: Priority,
        transformer: ClassesTransformer<RustItem>,
        prepare: PrepareCondition<RustItem>,
    ) -> Self {
        Self {
            state: GivenState::new(priority, transformer, prepare),
        }
    }

    /// `that()`: restrict the classes with the fluent predicates of [`ClassesThat`].
    pub fn that(self) -> ClassesThat<GivenClassesConjunction> {
        ClassesThat::new(move |predicate| {
            let predicates = self.state.predicates.add(predicate);
            Self {
                state: self.state.with(predicates),
            }
        })
    }

    /// `that(predicate)`.
    pub fn that_with(self, predicate: DescribedPredicate<RustItem>) -> GivenClassesConjunction {
        let predicates = self.state.predicates.add(predicate);
        Self {
            state: self.state.with(predicates),
        }
    }

    /// `and()`.
    pub fn and(self) -> ClassesThat<GivenClassesConjunction> {
        ClassesThat::new(move |predicate| {
            let predicates = self.state.predicates.that_ands().add(predicate);
            Self {
                state: self.state.with(predicates),
            }
        })
    }

    /// `and(predicate)`.
    pub fn and_with(self, predicate: DescribedPredicate<RustItem>) -> GivenClassesConjunction {
        let predicates = self.state.predicates.that_ands().add(predicate);
        Self {
            state: self.state.with(predicates),
        }
    }

    /// `or()`.
    pub fn or(self) -> ClassesThat<GivenClassesConjunction> {
        ClassesThat::new(move |predicate| {
            let predicates = self.state.predicates.that_ors().add(predicate);
            Self {
                state: self.state.with(predicates),
            }
        })
    }

    /// `or(predicate)`.
    pub fn or_with(self, predicate: DescribedPredicate<RustItem>) -> GivenClassesConjunction {
        let predicates = self.state.predicates.that_ors().add(predicate);
        Self {
            state: self.state.with(predicates),
        }
    }

    /// `should()`: continue with the fluent conditions of [`ClassesShould`].
    pub fn should(self) -> ClassesShould {
        ClassesShould {
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
        condition: impl Into<ArchCondition<RustItem>>,
    ) -> ClassesShouldConjunction {
        ClassesShouldConjunction {
            state: ShouldState {
                transformer: self.state.finished_transformer(),
                priority: self.state.priority,
                aggregator: ConditionAggregator::with(condition.into()),
                prepare: self.state.prepare.clone(),
            },
        }
    }
}

/// A rule about one specific class (`GivenClass`), from `the_class(..)` / `no_class(..)`.
#[derive(Clone)]
pub struct GivenClass {
    priority: Priority,
    transformer: ClassesTransformer<RustItem>,
    prepare: PrepareCondition<RustItem>,
}

impl std::fmt::Debug for GivenClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GivenClass({})", self.transformer.description())
    }
}

impl GivenClass {
    pub(crate) fn new(
        priority: Priority,
        transformer: ClassesTransformer<RustItem>,
        prepare: PrepareCondition<RustItem>,
    ) -> Self {
        Self {
            priority,
            transformer,
            prepare,
        }
    }

    /// `should()`.
    pub fn should(self) -> ClassesShould {
        ClassesShould {
            state: ShouldState {
                transformer: self.transformer,
                priority: self.priority,
                aggregator: ConditionAggregator::new(),
                prepare: self.prepare,
            },
        }
    }

    /// `should(condition)`.
    pub fn should_with(
        self,
        condition: impl Into<ArchCondition<RustItem>>,
    ) -> ClassesShouldConjunction {
        ClassesShouldConjunction {
            state: ShouldState {
                transformer: self.transformer,
                priority: self.priority,
                aggregator: ConditionAggregator::with(condition.into()),
                prepare: self.prepare,
            },
        }
    }
}

// ---- that ----------------------------------------------------------------------------------

/// The fluent predicates on classes (`ClassesThat<CONJUNCTION>`).
///
/// Each method adds a predicate and returns the continuation `C`: a
/// [`GivenClassesConjunction`] after `that()`/`and()`/`or()`, a [`ClassesShouldConjunction`]
/// after `should().access_classes_that()` and the like.
pub struct ClassesThat<C> {
    add: Box<dyn FnOnce(DescribedPredicate<RustItem>) -> C>,
}

impl<C> std::fmt::Debug for ClassesThat<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClassesThat")
    }
}

macro_rules! that_methods {
    ($($(#[$doc:meta])* fn $name:ident($($arg:ident: $ty:ty),*) => $body:expr;)*) => {
        $(
            $(#[$doc])*
            pub fn $name(self, $($arg: $ty),*) -> C {
                (self.add)($body)
            }
        )*
    };
}

impl<C> ClassesThat<C> {
    pub(crate) fn new(add: impl FnOnce(DescribedPredicate<RustItem>) -> C + 'static) -> Self {
        Self { add: Box::new(add) }
    }

    /// Adds an arbitrary predicate (the `that(predicate)` overload).
    pub fn satisfy(self, predicate: DescribedPredicate<RustItem>) -> C {
        (self.add)(predicate)
    }

    that_methods! {
        /// `have fully qualified name '<name>'`.
        fn have_fully_qualified_name(name: &str) => have(cond::fully_qualified_name(name));
        /// `do not have fully qualified name '<name>'`.
        fn do_not_have_fully_qualified_name(name: &str) => do_not(have(cond::fully_qualified_name(name)));
        /// `have simple name '<name>'`.
        fn have_simple_name(name: &str) => have(item::simple_name(name));
        /// `do not have simple name '<name>'`.
        fn do_not_have_simple_name(name: &str) => do_not(have(item::simple_name(name)));
        /// `have name matching '<regex>'`.
        fn have_name_matching(regex: &str) => have(has_name::predicates::name_matching(regex));
        /// `have name not matching '<regex>'`.
        fn have_name_not_matching(regex: &str) => have(not(has_name::predicates::name_matching(regex)).as_(format!("name not matching '{regex}'")));
        /// `have simple name starting with '<prefix>'`.
        fn have_simple_name_starting_with(prefix: &str) => have(item::simple_name_starting_with(prefix));
        /// `have simple name not starting with '<prefix>'`.
        fn have_simple_name_not_starting_with(prefix: &str) => have(not(item::simple_name_starting_with(prefix)).as_(format!("simple name not starting with '{prefix}'")));
        /// `have simple name containing '<infix>'`.
        fn have_simple_name_containing(infix: &str) => have(item::simple_name_containing(infix));
        /// `have simple name not containing '<infix>'`.
        fn have_simple_name_not_containing(infix: &str) => have(not(item::simple_name_containing(infix)).as_(format!("simple name not containing '{infix}'")));
        /// `have simple name ending with '<suffix>'`.
        fn have_simple_name_ending_with(suffix: &str) => have(item::simple_name_ending_with(suffix));
        /// `have simple name not ending with '<suffix>'`.
        fn have_simple_name_not_ending_with(suffix: &str) => have(not(item::simple_name_ending_with(suffix)).as_(format!("simple name not ending with '{suffix}'")));
        /// `reside in a package '<identifier>'`.
        fn reside_in_a_package(package_identifier: &str) => item::reside_in_a_package(package_identifier);
        /// `reside in any package ['a', 'b']`.
        fn reside_in_any_package(package_identifiers: &[&str]) => item::reside_in_any_package(package_identifiers);
        /// `reside outside of package '<identifier>'`.
        fn reside_outside_of_package(package_identifier: &str) => item::reside_outside_of_package(package_identifier);
        /// `reside outside of packages ['a', 'b']`.
        fn reside_outside_of_packages(package_identifiers: &[&str]) => item::reside_outside_of_packages(package_identifiers);
        /// `reside in crate '<name>'` (Rust-only).
        fn reside_in_crate(crate_name: &str) => item::reside_in_crate(crate_name);
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
        /// `are pub(crate)` (Rust-only).
        fn are_pub_crate() => has_modifiers::predicates::modifier(RustModifier::PubCrate).as_("are pub(crate)");
        /// `are not pub(crate)` (Rust-only).
        fn are_not_pub_crate() => not(has_modifiers::predicates::modifier(RustModifier::PubCrate)).as_("are not pub(crate)");
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
        /// `implement <trait>`.
        fn implement(selector: impl Into<ItemSelector>) => item::implement(selector);
        /// `do not implement <trait>`.
        fn do_not_implement(selector: impl Into<ItemSelector>) => do_not(item::implement(selector));
        /// `are assignable to <type>`.
        fn are_assignable_to(selector: impl Into<ItemSelector>) => are(item::assignable_to(selector));
        /// `are not assignable to <type>`.
        fn are_not_assignable_to(selector: impl Into<ItemSelector>) => are(not(item::assignable_to(selector)));
        /// `are assignable from <type>`.
        fn are_assignable_from(selector: impl Into<ItemSelector>) => are(item::assignable_from(selector));
        /// `are not assignable from <type>`.
        fn are_not_assignable_from(selector: impl Into<ItemSelector>) => are(not(item::assignable_from(selector)));
        /// `are interfaces` (traits).
        fn are_interfaces() => are(item::interfaces());
        /// `are not interfaces`.
        fn are_not_interfaces() => are(not(item::interfaces()));
        /// `are traits` (Rust-only alias).
        fn are_traits() => are(item::traits());
        /// `are not traits` (Rust-only alias).
        fn are_not_traits() => are(not(item::traits()));
        /// `are enums`.
        fn are_enums() => are(item::enums());
        /// `are not enums`.
        fn are_not_enums() => are(not(item::enums()));
        /// `are structs` (Rust-only).
        fn are_structs() => are(item::structs());
        /// `are not structs` (Rust-only).
        fn are_not_structs() => are(not(item::structs()));
        /// `are functions` (Rust-only).
        fn are_functions() => are(item::functions());
        /// `are not functions` (Rust-only).
        fn are_not_functions() => are(not(item::functions()));
        /// `are annotations` (attribute and derive macro definitions).
        fn are_annotations() => are(item::annotations());
        /// `are not annotations`.
        fn are_not_annotations() => are(not(item::annotations()));
        /// `are top level classes`.
        fn are_top_level_classes() => are(item::top_level_classes());
        /// `are not top level classes`.
        fn are_not_top_level_classes() => are(not(item::top_level_classes()));
        /// `are nested classes`.
        fn are_nested_classes() => are(item::nested_classes());
        /// `are not nested classes`.
        fn are_not_nested_classes() => are(not(item::nested_classes()));
        /// `are local classes`.
        fn are_local_classes() => are(item::local_classes());
        /// `are not local classes`.
        fn are_not_local_classes() => are(not(item::local_classes()));
        /// `are test code` (Rust-only).
        fn are_test_code() => are(item::test_code());
        /// `are not test code` (Rust-only).
        fn are_not_test_code() => are(not(item::test_code()));
        /// `belong to any of [a, b]`.
        fn belong_to_any_of(names: &[&str]) => item::belong_to_any_of(names);
        /// `do not belong to any of [a, b]`.
        fn do_not_belong_to_any_of(names: &[&str]) => do_not(item::belong_to_any_of(names));
        /// `contain any members that <predicate>`.
        fn contain_any_members_that(predicate: DescribedPredicate<RustMember>) => item::contain_any_members_that(predicate);
        /// `contain any fields that <predicate>`.
        fn contain_any_fields_that(predicate: DescribedPredicate<RustField>) => item::contain_any_fields_that(predicate);
        /// `contain any code units that <predicate>`.
        fn contain_any_code_units_that(predicate: DescribedPredicate<RustCodeUnit>) => item::contain_any_code_units_that(predicate);
        /// `contain any methods that <predicate>`.
        fn contain_any_methods_that(predicate: DescribedPredicate<RustMethod>) => item::contain_any_methods_that(predicate);
        /// `contain any constructors that <predicate>`.
        fn contain_any_constructors_that(predicate: DescribedPredicate<RustConstructor>) => item::contain_any_constructors_that(predicate);
    }
}

// ---- should --------------------------------------------------------------------------------

/// The fluent conditions on classes (`ClassesShould`).
#[derive(Clone)]
pub struct ClassesShould {
    state: ShouldState<RustItem>,
}

impl std::fmt::Debug for ClassesShould {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ClassesShould({})", self.state.transformer.description())
    }
}

/// A finished rule about classes that can be extended with `and_should()` / `or_should()`
/// (`ClassesShouldConjunction`).
#[derive(Clone)]
pub struct ClassesShouldConjunction {
    state: ShouldState<RustItem>,
}

impl std::fmt::Debug for ClassesShouldConjunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ClassesShouldConjunction({})", self.description())
    }
}

macro_rules! should_methods {
    ($($(#[$doc:meta])* fn $name:ident($($arg:ident: $ty:ty),*) => $body:expr;)*) => {
        $(
            $(#[$doc])*
            pub fn $name(self, $($arg: $ty),*) -> ClassesShouldConjunction {
                self.add_condition($body)
            }
        )*
    };
}

impl ClassesShould {
    fn add_condition(
        self,
        condition: impl Into<ArchCondition<RustItem>>,
    ) -> ClassesShouldConjunction {
        ClassesShouldConjunction {
            state: self.state.add_condition(condition.into()),
        }
    }

    /// Adds an arbitrary condition (the `should(condition)` overload).
    pub fn satisfy(
        self,
        condition: impl Into<ArchCondition<RustItem>>,
    ) -> ClassesShouldConjunction {
        self.add_condition(condition)
    }

    fn classes_that(
        self,
        build: impl Fn(DescribedPredicate<RustItem>) -> ArchCondition<RustItem> + 'static,
    ) -> ClassesThat<ClassesShouldConjunction> {
        ClassesThat::new(move |predicate| self.add_condition(build(predicate)))
    }

    should_methods! {
        /// `be <name>`.
        fn be(class_name: &str) => cond::be_class(class_name);
        /// `not be <name>`.
        fn not_be(class_name: &str) => cond::not_be_class(class_name);
        /// `have fully qualified name '<name>'`.
        fn have_fully_qualified_name(name: &str) => cond::have_fully_qualified_name(name);
        /// `not have fully qualified name '<name>'`.
        fn not_have_fully_qualified_name(name: &str) => cond::not_have_fully_qualified_name(name);
        /// `have simple name '<name>'`.
        fn have_simple_name(name: &str) => cond::have_simple_name(name);
        /// `not have simple name '<name>'`.
        fn not_have_simple_name(name: &str) => cond::not_have_simple_name(name);
        /// `have simple name starting with '<prefix>'`.
        fn have_simple_name_starting_with(prefix: &str) => cond::have_simple_name_starting_with(prefix);
        /// `have simple name not starting with '<prefix>'`.
        fn have_simple_name_not_starting_with(prefix: &str) => cond::have_simple_name_not_starting_with(prefix);
        /// `have simple name containing '<infix>'`.
        fn have_simple_name_containing(infix: &str) => cond::have_simple_name_containing(infix);
        /// `have simple name not containing '<infix>'`.
        fn have_simple_name_not_containing(infix: &str) => cond::have_simple_name_not_containing(infix);
        /// `have simple name ending with '<suffix>'`.
        fn have_simple_name_ending_with(suffix: &str) => cond::have_simple_name_ending_with(suffix);
        /// `have simple name not ending with '<suffix>'`.
        fn have_simple_name_not_ending_with(suffix: &str) => cond::have_simple_name_not_ending_with(suffix);
        /// `have name matching '<regex>'`.
        fn have_name_matching(regex: &str) => cond::have_name_matching::<RustItem>(regex);
        /// `have name not matching '<regex>'`.
        fn have_name_not_matching(regex: &str) => cond::have_name_not_matching::<RustItem>(regex);
        /// `reside in a package '<identifier>'`.
        fn reside_in_a_package(package_identifier: &str) => cond::reside_in_a_package(package_identifier);
        /// `reside in any package ['a', 'b']`.
        fn reside_in_any_package(package_identifiers: &[&str]) => cond::reside_in_any_package(package_identifiers);
        /// `reside outside of package '<identifier>'`.
        fn reside_outside_of_package(package_identifier: &str) => cond::reside_outside_of_package(package_identifier);
        /// `reside outside of packages ['a', 'b']`.
        fn reside_outside_of_packages(package_identifiers: &[&str]) => cond::reside_outside_of_packages(package_identifiers);
        /// `reside in crate '<name>'` (Rust-only).
        fn reside_in_crate(crate_name: &str) => cond::reside_in_crate(crate_name);
        /// `be public`.
        fn be_public() => cond::be_public::<RustItem>();
        /// `not be public`.
        fn not_be_public() => cond::not_be_public::<RustItem>();
        /// `be protected` (restricted visibility).
        fn be_protected() => cond::be_protected::<RustItem>();
        /// `not be protected`.
        fn not_be_protected() => cond::not_be_protected::<RustItem>();
        /// `be package private` (inherited visibility).
        fn be_package_private() => cond::be_package_private::<RustItem>();
        /// `not be package private`.
        fn not_be_package_private() => cond::not_be_package_private::<RustItem>();
        /// `be private` (inherited visibility).
        fn be_private() => cond::be_private::<RustItem>();
        /// `not be private`.
        fn not_be_private() => cond::not_be_private::<RustItem>();
        /// `be pub(crate)` (Rust-only).
        fn be_pub_crate() => cond::be_pub_crate::<RustItem>();
        /// `not be pub(crate)` (Rust-only).
        fn not_be_pub_crate() => cond::not_be_pub_crate::<RustItem>();
        /// `be unsafe` (Rust-only).
        fn be_unsafe() => cond::be_unsafe::<RustItem>();
        /// `not be unsafe` (Rust-only).
        fn not_be_unsafe() => cond::not_be_unsafe::<RustItem>();
        /// `be async` (Rust-only).
        fn be_async() => cond::be_async::<RustItem>();
        /// `not be async` (Rust-only).
        fn not_be_async() => cond::not_be_async::<RustItem>();
        /// `have only private fields` (Rust-only; nearest analog of `haveOnlyFinalFields`).
        fn have_only_private_fields() => cond::have_only_private_fields();
        /// `have only private constructors`.
        fn have_only_private_constructors() => cond::have_only_private_constructors();
        /// `have modifier <modifier>`.
        fn have_modifier(modifier: RustModifier) => cond::have_modifier::<RustItem>(modifier);
        /// `not have modifier <modifier>`.
        fn not_have_modifier(modifier: RustModifier) => cond::not_have_modifier::<RustItem>(modifier);
        /// `be annotated with @Name`.
        fn be_annotated_with(selector: impl Into<AnnotationSelector>) => cond::be_annotated_with::<RustItem>(selector);
        /// `not be annotated with @Name`.
        fn not_be_annotated_with(selector: impl Into<AnnotationSelector>) => cond::not_be_annotated_with::<RustItem>(selector);
        /// `implement <trait>`.
        fn implement(selector: impl Into<ItemSelector>) => cond::implement(selector);
        /// `not implement <trait>`.
        fn not_implement(selector: impl Into<ItemSelector>) => cond::not_implement(selector);
        /// `be assignable to <type>`.
        fn be_assignable_to(selector: impl Into<ItemSelector>) => cond::be_assignable_to(selector);
        /// `not be assignable to <type>`.
        fn not_be_assignable_to(selector: impl Into<ItemSelector>) => cond::not_be_assignable_to(selector);
        /// `be assignable from <type>`.
        fn be_assignable_from(selector: impl Into<ItemSelector>) => cond::be_assignable_from(selector);
        /// `not be assignable from <type>`.
        fn not_be_assignable_from(selector: impl Into<ItemSelector>) => cond::not_be_assignable_from(selector);
        /// `access field Owner::field`.
        fn access_field(owner: &str, field_name: &str) => cond::access_field(owner, field_name);
        /// `get field Owner::field`.
        fn get_field(owner: &str, field_name: &str) => cond::get_field(owner, field_name);
        /// `set field Owner::field`.
        fn set_field(owner: &str, field_name: &str) => cond::set_field(owner, field_name);
        /// `access field where <predicate>`.
        fn access_field_where(predicate: DescribedPredicate<RustAccess>) => cond::access_field_where(predicate);
        /// `get field where <predicate>`.
        fn get_field_where(predicate: DescribedPredicate<RustAccess>) => cond::get_field_where(predicate);
        /// `set field where <predicate>`.
        fn set_field_where(predicate: DescribedPredicate<RustAccess>) => cond::set_field_where(predicate);
        /// `only access fields that <predicate>`.
        fn only_access_fields_that(predicate: DescribedPredicate<RustField>) => cond::only_access_fields_that(predicate);
        /// `call method Owner::name(p1, p2)`.
        fn call_method(owner: &str, method_name: &str, parameter_type_names: &[&str]) => cond::call_method(owner, method_name, parameter_type_names);
        /// `call method where <predicate>`.
        fn call_method_where(predicate: DescribedPredicate<RustAccess>) => cond::call_method_where(predicate);
        /// `only call methods that <predicate>`.
        fn only_call_methods_that(predicate: DescribedPredicate<RustMethod>) => cond::only_call_methods_that(predicate);
        /// `call constructor Owner(p1, p2)`.
        fn call_constructor(owner: &str, parameter_type_names: &[&str]) => cond::call_constructor(owner, parameter_type_names);
        /// `call constructor where <predicate>`.
        fn call_constructor_where(predicate: DescribedPredicate<RustAccess>) => cond::call_constructor_where(predicate);
        /// `only call constructors that <predicate>`.
        fn only_call_constructors_that(predicate: DescribedPredicate<RustConstructor>) => cond::only_call_constructors_that(predicate);
        /// `call code unit where <predicate>`.
        fn call_code_unit_where(predicate: DescribedPredicate<RustAccess>) => cond::call_code_unit_where(predicate);
        /// `only call code units that <predicate>`.
        fn only_call_code_units_that(predicate: DescribedPredicate<RustCodeUnit>) => cond::only_call_code_units_that(predicate);
        /// `access target where <predicate>`.
        fn access_target_where(predicate: DescribedPredicate<RustAccess>) => cond::access_target_where(predicate);
        /// `only access members that <predicate>`.
        fn only_access_members_that(predicate: DescribedPredicate<RustMember>) => cond::only_access_members_that(predicate);
        /// `access classes that <predicate>`.
        fn access_classes_that_with(predicate: DescribedPredicate<RustItem>) => cond::access_classes_that(predicate);
        /// `only access classes that <predicate>`.
        fn only_access_classes_that_with(predicate: DescribedPredicate<RustItem>) => cond::only_access_classes_that(predicate);
        /// `depend on classes that <predicate>`.
        fn depend_on_classes_that_with(predicate: DescribedPredicate<RustItem>) => cond::depend_on_classes_that(predicate);
        /// `only depend on classes that <predicate>`.
        fn only_depend_on_classes_that_with(predicate: DescribedPredicate<RustItem>) => cond::only_depend_on_classes_that(predicate);
        /// `transitively depend on classes that <predicate>`.
        fn transitively_depend_on_classes_that_with(predicate: DescribedPredicate<RustItem>) => cond::transitively_depend_on_classes_that(predicate);
        /// `only have dependent classes that <predicate>`.
        fn only_have_dependent_classes_that_with(predicate: DescribedPredicate<RustItem>) => cond::only_have_dependent_classes_that(predicate);
        /// `be interfaces` (traits).
        fn be_interfaces() => cond::be_interfaces();
        /// `not be interfaces`.
        fn not_be_interfaces() => cond::not_be_interfaces();
        /// `be traits` (Rust-only alias).
        fn be_traits() => cond::be_traits();
        /// `not be traits` (Rust-only alias).
        fn not_be_traits() => cond::not_be_traits();
        /// `be enums`.
        fn be_enums() => cond::be_enums();
        /// `not be enums`.
        fn not_be_enums() => cond::not_be_enums();
        /// `be structs` (Rust-only).
        fn be_structs() => cond::be_structs();
        /// `not be structs` (Rust-only).
        fn not_be_structs() => cond::not_be_structs();
        /// `be functions` (Rust-only).
        fn be_functions() => cond::be_functions();
        /// `not be functions` (Rust-only).
        fn not_be_functions() => cond::not_be_functions();
        /// `be top level classes`.
        fn be_top_level_classes() => cond::be_top_level_classes();
        /// `not be top level classes`.
        fn not_be_top_level_classes() => cond::not_be_top_level_classes();
        /// `be nested classes`.
        fn be_nested_classes() => cond::be_nested_classes();
        /// `not be nested classes`.
        fn not_be_nested_classes() => cond::not_be_nested_classes();
        /// `be local classes`.
        fn be_local_classes() => cond::be_local_classes();
        /// `not be local classes`.
        fn not_be_local_classes() => cond::not_be_local_classes();
        /// `contain number of elements <predicate>`.
        fn contain_number_of_elements(predicate: DescribedPredicate<usize>) => cond::contain_number_of_elements::<RustItem>(predicate);
    }

    /// `access classes that ..` continued with fluent predicates.
    pub fn access_classes_that(self) -> ClassesThat<ClassesShouldConjunction> {
        self.classes_that(cond::access_classes_that)
    }

    /// `only access classes that ..` continued with fluent predicates.
    pub fn only_access_classes_that(self) -> ClassesThat<ClassesShouldConjunction> {
        self.classes_that(cond::only_access_classes_that)
    }

    /// `depend on classes that ..` continued with fluent predicates.
    pub fn depend_on_classes_that(self) -> ClassesThat<ClassesShouldConjunction> {
        self.classes_that(|p| cond::depend_on_classes_that(p).into())
    }

    /// `only depend on classes that ..` continued with fluent predicates.
    pub fn only_depend_on_classes_that(self) -> ClassesThat<ClassesShouldConjunction> {
        self.classes_that(|p| cond::only_depend_on_classes_that(p).into())
    }

    /// `transitively depend on classes that ..` continued with fluent predicates.
    pub fn transitively_depend_on_classes_that(self) -> ClassesThat<ClassesShouldConjunction> {
        self.classes_that(cond::transitively_depend_on_classes_that)
    }

    /// `only have dependent classes that ..` continued with fluent predicates.
    pub fn only_have_dependent_classes_that(self) -> ClassesThat<ClassesShouldConjunction> {
        self.classes_that(|p| cond::only_have_dependent_classes_that(p).into())
    }

    /// `only be accessed ..` (`OnlyBeAccessedSpecification`).
    pub fn only_be_accessed(self) -> OnlyBeAccessedSpecification {
        OnlyBeAccessedSpecification { should: self }
    }
}

/// `only be accessed ..` continuation (`OnlyBeAccessedSpecification<CONJUNCTION>`).
#[derive(Debug, Clone)]
pub struct OnlyBeAccessedSpecification {
    should: ClassesShould,
}

impl OnlyBeAccessedSpecification {
    /// `only be accessed by any package ['a', 'b']`.
    pub fn by_any_package(self, package_identifiers: &[&str]) -> ClassesShouldConjunction {
        self.should
            .add_condition(cond::only_be_accessed_by_any_package(package_identifiers))
    }

    /// `only be accessed by classes that ..` continued with fluent predicates.
    pub fn by_classes_that(self) -> ClassesThat<ClassesShouldConjunction> {
        self.should
            .classes_that(cond::only_be_accessed_by_classes_that)
    }

    /// `only be accessed by classes that <predicate>`.
    pub fn by_classes_that_with(
        self,
        predicate: DescribedPredicate<RustItem>,
    ) -> ClassesShouldConjunction {
        self.should
            .add_condition(cond::only_be_accessed_by_classes_that(predicate))
    }
}

impl HasDescription for ClassesShouldConjunction {
    fn description(&self) -> String {
        self.state.finished_rule().description()
    }
}

impl ArchRule for ClassesShouldConjunction {
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

impl ClassesShouldConjunction {
    /// The rule's description, e.g. `classes that .. should ..`.
    pub fn description(&self) -> String {
        HasDescription::description(self)
    }

    /// `and_should()`: add another condition with fluent syntax.
    pub fn and_should(self) -> ClassesShould {
        ClassesShould {
            state: self
                .state
                .with_aggregator(self.state.aggregator.that_ands_with(prepend_should())),
        }
    }

    /// `and_should(condition)`.
    pub fn and_should_with(
        self,
        condition: impl Into<ArchCondition<RustItem>>,
    ) -> ClassesShouldConjunction {
        let aggregator = self.state.aggregator.that_ands_with(prepend_should());
        ClassesShouldConjunction {
            state: self
                .state
                .with_aggregator(ConditionAggregator::with(aggregator.add(condition.into()))),
        }
    }

    /// `or_should()`: add an alternative condition with fluent syntax.
    pub fn or_should(self) -> ClassesShould {
        ClassesShould {
            state: self
                .state
                .with_aggregator(self.state.aggregator.that_ors_with(prepend_should())),
        }
    }

    /// `or_should(condition)`.
    pub fn or_should_with(
        self,
        condition: impl Into<ArchCondition<RustItem>>,
    ) -> ClassesShouldConjunction {
        let aggregator = self.state.aggregator.that_ors_with(prepend_should());
        ClassesShouldConjunction {
            state: self
                .state
                .with_aggregator(ConditionAggregator::with(aggregator.add(condition.into()))),
        }
    }

    /// `because(reason)`.
    pub fn because(self, reason: &str) -> SimpleArchRule<RustItem> {
        self.state.finished_rule().because(reason)
    }

    /// `as(description)`.
    pub fn as_(self, description: &str) -> SimpleArchRule<RustItem> {
        self.state.finished_rule().as_(description)
    }

    /// `allowEmptyShould(..)`.
    pub fn allow_empty_should(self, allow_empty_should: bool) -> SimpleArchRule<RustItem> {
        self.state
            .finished_rule()
            .allow_empty_should(allow_empty_should)
    }

    /// The finished [`SimpleArchRule`].
    pub fn into_rule(self) -> SimpleArchRule<RustItem> {
        self.state.finished_rule()
    }
}
