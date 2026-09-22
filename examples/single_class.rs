//! Port of `SingleClassTest`: rules about one specific item.
#[macro_use]
mod common;

use archunit::prelude::*;

fn core_should_only_be_accessed_by_satellites() -> impl ArchRule {
    the_class("layered_app::core::VeryCentralCore")
        .should()
        .only_be_accessed()
        .by_classes_that()
        .implement("layered_app::core::CoreSatellite")
}

fn core_should_only_access_classes_in_core_itself() -> impl ArchRule {
    no_class("layered_app::core::VeryCentralCore")
        .should()
        .access_classes_that()
        .reside_outside_of_packages(&["..core..", "std.."])
}

fn the_only_class_with_high_security_is_central_core() -> impl ArchRule {
    classes()
        .that()
        .are_annotated_with("high_security")
        .should()
        .be("layered_app::core::VeryCentralCore")
}

fn central_core_should_not_implement_some_business_interface() -> impl ArchRule {
    classes()
        .that()
        .implement("layered_app::service::ServiceInterface")
        .should()
        .not_be("layered_app::core::VeryCentralCore")
}

archunit_example! {
    example = "single_class",
    fixture = "layered_app",
    rules = [
        core_should_only_be_accessed_by_satellites,
        core_should_only_access_classes_in_core_itself,
        the_only_class_with_high_security_is_central_core,
        central_core_should_not_implement_some_business_interface,
    ],
}
