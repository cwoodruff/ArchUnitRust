//! Port of `ThirdPartyRulesTest`: a problematic third-party type may only be instantiated by
//! a workaround factory, expressed as a custom `ArchCondition`.
#[macro_use]
mod common;

use archunit::base::{DescribedPredicate, not};
use archunit::core::domain::RustAccess;
use archunit::core::domain::access_target::predicates::constructor;
use archunit::core::domain::rust_access::predicates::{origin_owner, target, target_owner};
use archunit::core::domain::rust_item::predicates::{assignable_to, equivalent_to};
use archunit::lang::ArchCondition;
use archunit::lang::conditions::{call_code_unit_where, predicates::is};
use archunit::prelude::*;

const THIRD_PARTY_CLASS_WITH_PROBLEM: &str = "layered_app::thirdparty::EntityManager";
const WORKAROUND_FACTORY: &str = "layered_app::persistence::first::dao::SomeDao";

fn third_party_class_should_only_be_instantiated_via_workaround() -> impl ArchRule {
    classes().should_with(
        not_create_problematic_classes_outside_of_workaround_factory().as_(format!(
            "not instantiate {} and its subclasses, but instead use {}",
            "EntityManager", "SomeDao"
        )),
    )
}

fn not_create_problematic_classes_outside_of_workaround_factory() -> ArchCondition<RustItem> {
    let constructor_call_of_third_party_class: DescribedPredicate<RustAccess> =
        target(is(constructor())).and(target_owner(is(assignable_to(
            THIRD_PARTY_CLASS_WITH_PROBLEM,
        ))));
    let not_from_within_third_party_class =
        origin_owner(is(not(assignable_to(THIRD_PARTY_CLASS_WITH_PROBLEM))));
    let not_from_workaround_factory = origin_owner(is(not(equivalent_to(WORKAROUND_FACTORY))));
    let target_is_illegal_constructor_of_third_party_class = constructor_call_of_third_party_class
        .and(not_from_within_third_party_class)
        .and(not_from_workaround_factory);
    ArchCondition::never(call_code_unit_where(
        target_is_illegal_constructor_of_third_party_class,
    ))
}

archunit_example! {
    example = "third_party_rules",
    fixture = "layered_app",
    rules = [third_party_class_should_only_be_instantiated_via_workaround],
}
