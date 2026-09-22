//! Port of `CyclicDependencyRulesTest` against `tests/fixtures/cyclic_app`.
#[macro_use]
mod common;

use archunit::base::always_true;
use archunit::core::domain::RustItem;
use archunit::core::domain::rust_item::predicates::reside_in_a_package;
use archunit::lang::Priority;
use archunit::library::dependencies::{SliceIdentifier, slice_assignment};
use archunit::prelude::*;

fn no_cycles_by_method_calls_between_slices() -> impl ArchRule {
    slices()
        .matching("..(simplecycle).(*)..")
        .naming_slices("$2 of $1")
        .should()
        .be_free_of_cycles()
}

fn no_cycles_by_constructor_calls_between_slices() -> impl ArchRule {
    slices()
        .matching("..(constructorcycle).(*)..")
        .naming_slices("$2 of $1")
        .should()
        .be_free_of_cycles()
}

fn no_cycles_by_inheritance_between_slices() -> impl ArchRule {
    slices()
        .matching("..(inheritancecycle).(*)..")
        .naming_slices("$2 of $1")
        .should()
        .be_free_of_cycles()
}

fn no_cycles_by_field_access_between_slices() -> impl ArchRule {
    slices()
        .matching("..(fieldaccesscycle).(*)..")
        .naming_slices("$2 of $1")
        .should()
        .be_free_of_cycles()
}

fn no_cycles_by_member_dependencies_between_slices() -> impl ArchRule {
    slices()
        .matching("..(membercycle).(*)..")
        .naming_slices("$2 of $1")
        .should()
        .be_free_of_cycles()
}

fn no_cycles_in_simple_scenario() -> impl ArchRule {
    slices()
        .matching("..simplescenario.(*)..")
        .naming_slices("$1")
        .should()
        .be_free_of_cycles()
}

fn no_cycles_in_complex_scenario() -> impl ArchRule {
    slices()
        .matching("..(complexcycles).(*)..")
        .naming_slices("$2 of $1")
        .should()
        .be_free_of_cycles()
}

fn no_cycles_in_complex_scenario_with_custom_ignore() -> impl ArchRule {
    slices()
        .matching("..(complexcycles).(*)..")
        .naming_slices("$2 of $1")
        .as_("Slices of complex scenario ignoring some violations")
        .should()
        .be_free_of_cycles()
        .ignore_dependency(
            "cyclic_app::complexcycles::slice1::SliceOneCallingConstructorInSliceTwoAndMethodInSliceThree",
            "cyclic_app::complexcycles::slice5::ClassCallingConstructorInSliceFive",
        )
        .ignore_dependency(reside_in_a_package("..slice4.."), always_true::<RustItem>())
}

fn no_cycles_in_freely_customized_slices() -> impl ArchRule {
    slices()
        .assigned_from_with_priority(in_complex_slice_one_or_two(), Priority::High)
        .naming_slices("$1[$2]")
        .should()
        .be_free_of_cycles()
}

fn in_complex_slice_one_or_two() -> impl archunit::library::dependencies::SliceAssignment {
    slice_assignment("complex slice one or two", |item: &RustItem| {
        let package = item.package_name();
        if package.contains("complexcycles::slice1") {
            SliceIdentifier::of(&["Complex-Cycle", "One"])
        } else if package.contains("complexcycles::slice2") {
            SliceIdentifier::of(&["Complex-Cycle", "Two"])
        } else {
            SliceIdentifier::ignore()
        }
    })
}

archunit_example! {
    example = "cyclic_dependency_rules",
    fixture = "cyclic_app",
    rules = [
        no_cycles_by_method_calls_between_slices,
        no_cycles_by_constructor_calls_between_slices,
        no_cycles_by_inheritance_between_slices,
        no_cycles_by_field_access_between_slices,
        no_cycles_by_member_dependencies_between_slices,
        no_cycles_in_simple_scenario,
        no_cycles_in_complex_scenario,
        no_cycles_in_complex_scenario_with_custom_ignore,
        no_cycles_in_freely_customized_slices,
    ],
}
