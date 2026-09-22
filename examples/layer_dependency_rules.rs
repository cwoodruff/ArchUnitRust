//! Port of `LayerDependencyRulesTest`: `access` catches real accesses (field accesses, method
//! calls), `depend_on` catches every kind of dependency (field types, parameters, traits, ...).
#[macro_use]
mod common;

use archunit::prelude::*;

fn services_should_not_access_controllers() -> impl ArchRule {
    no_classes()
        .that()
        .reside_in_a_package("..service..")
        .should()
        .access_classes_that()
        .reside_in_a_package("..controller..")
}

fn persistence_should_not_access_services() -> impl ArchRule {
    no_classes()
        .that()
        .reside_in_a_package("..persistence..")
        .should()
        .access_classes_that()
        .reside_in_a_package("..service..")
}

fn services_should_only_be_accessed_by_controllers_or_other_services() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..service..")
        .should()
        .only_be_accessed()
        .by_any_package(&["..controller..", "..service.."])
}

fn services_should_only_access_persistence_or_other_services() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..service..")
        .should()
        .only_access_classes_that()
        .reside_in_any_package(&["..service..", "..persistence..", "std..", "core.."])
}

fn services_should_not_depend_on_controllers() -> impl ArchRule {
    no_classes()
        .that()
        .reside_in_a_package("..service..")
        .should()
        .depend_on_classes_that()
        .reside_in_a_package("..controller..")
}

fn persistence_should_not_depend_on_services() -> impl ArchRule {
    no_classes()
        .that()
        .reside_in_a_package("..persistence..")
        .should()
        .depend_on_classes_that()
        .reside_in_a_package("..service..")
}

fn services_should_only_be_depended_on_by_controllers_or_other_services() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..service..")
        .should()
        .only_have_dependent_classes_that()
        .reside_in_any_package(&["..controller..", "..service.."])
}

fn services_should_only_depend_on_persistence_or_other_services() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..service..")
        .should()
        .only_depend_on_classes_that()
        .reside_in_any_package(&["..service..", "..persistence..", "std..", "core.."])
}

archunit_example! {
    example = "layer_dependency_rules",
    fixture = "layered_app",
    rules = [
        services_should_not_access_controllers,
        persistence_should_not_access_services,
        services_should_only_be_accessed_by_controllers_or_other_services,
        services_should_only_access_persistence_or_other_services,
        services_should_not_depend_on_controllers,
        persistence_should_not_depend_on_services,
        services_should_only_be_depended_on_by_controllers_or_other_services,
        services_should_only_depend_on_persistence_or_other_services,
    ],
}
