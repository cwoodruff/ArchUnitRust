//! Port of `InterfaceRulesTest`: traits are ArchUnit's interfaces.
#[macro_use]
mod common;

use archunit::prelude::*;

fn interfaces_should_not_have_names_ending_with_the_word_interface() -> impl ArchRule {
    no_classes()
        .that()
        .are_interfaces()
        .should()
        .have_name_matching(".*Interface")
}

fn interfaces_should_not_have_simple_class_names_containing_the_word_interface() -> impl ArchRule {
    no_classes()
        .that()
        .are_interfaces()
        .should()
        .have_simple_name_containing("Interface")
}

fn interfaces_must_not_be_placed_in_implementation_packages() -> impl ArchRule {
    no_classes()
        .that()
        .reside_in_a_package("..internal..")
        .should()
        .be_interfaces()
}

archunit_example! {
    example = "interface_rules",
    fixture = "layered_app",
    rules = [
        interfaces_should_not_have_names_ending_with_the_word_interface,
        interfaces_should_not_have_simple_class_names_containing_the_word_interface,
        interfaces_must_not_be_placed_in_implementation_packages,
    ],
}
