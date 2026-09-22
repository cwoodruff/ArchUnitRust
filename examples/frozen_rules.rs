//! Port of `FrozenRulesTest`: the violations of these rules were frozen into
//! `examples/frozen/` (a `stored.rules` index plus one file per rule, in the format Java
//! ArchUnit uses), so only new violations would fail.
#[macro_use]
mod common;

use archunit::config::ArchConfiguration;
use archunit::prelude::*;

fn use_the_committed_store() {
    let store = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/frozen");
    ArchConfiguration::set_property("freeze.store.default.path", store);
    // The store is part of the repository: never create or modify it from a test run.
    ArchConfiguration::set_property("freeze.store.default.allow_store_creation", "false");
    ArchConfiguration::set_property("freeze.store.default.allow_store_update", "false");
}

fn no_classes_should_depend_on_service() -> impl ArchRule {
    freeze(
        no_classes()
            .should()
            .depend_on_classes_that()
            .reside_in_a_package("..service.."),
    )
}

fn no_classes_should_use_the_entity_manager() -> impl ArchRule {
    freeze(
        no_classes()
            .should()
            .depend_on_classes_that()
            .are_assignable_to("layered_app::thirdparty::EntityManager"),
    )
}

archunit_example! {
    example = "frozen_rules",
    fixture = "layered_app",
    setup = use_the_committed_store,
    rules = [no_classes_should_depend_on_service, no_classes_should_use_the_entity_manager],
}
