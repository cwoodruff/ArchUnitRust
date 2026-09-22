//! Port of `NamingConventionTest`.
#[macro_use]
mod common;

use archunit::prelude::*;

fn services_should_be_prefixed() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..service..")
        .and()
        .are_annotated_with("my_service")
        .should()
        .have_simple_name_starting_with("Service")
}

fn controllers_should_not_have_gui_in_name() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..controller..")
        .should()
        .have_simple_name_not_containing("Gui")
}

fn controllers_should_be_suffixed() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..controller..")
        .or()
        .are_annotated_with("my_controller")
        .or()
        .are_assignable_to("layered_app::controller::AbstractController")
        .should()
        .have_simple_name_ending_with("Controller")
}

fn classes_named_controller_should_be_in_a_controller_package() -> impl ArchRule {
    classes()
        .that()
        .have_simple_name_containing("Controller")
        .should()
        .reside_in_a_package("..controller..")
}

archunit_example! {
    example = "naming_convention",
    fixture = "layered_app",
    rules = [
        services_should_be_prefixed,
        controllers_should_not_have_gui_in_name,
        controllers_should_be_suffixed,
        classes_named_controller_should_be_in_a_controller_package,
    ],
}
