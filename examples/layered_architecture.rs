//! Port of `LayeredArchitectureTest`: the layers of `tests/fixtures/layered_app`.
#[macro_use]
mod common;

use archunit::prelude::*;

fn layer_dependencies_are_respected() -> impl ArchRule {
    layered_architecture()
        .considering_all_dependencies()
        .layer("Controllers")
        .defined_by(&["layered_app::controller.."])
        .layer("Services")
        .defined_by(&["layered_app::service.."])
        .layer("Persistence")
        .defined_by(&["layered_app::persistence.."])
        .where_layer("Controllers")
        .may_not_be_accessed_by_any_layer()
        .where_layer("Services")
        .may_only_be_accessed_by_layers(&["Controllers"])
        .where_layer("Persistence")
        .may_only_be_accessed_by_layers(&["Services"])
}

fn layer_dependencies_are_respected_with_exception() -> impl ArchRule {
    layered_architecture()
        .considering_all_dependencies()
        .layer("Controllers")
        .defined_by(&["layered_app::controller.."])
        .layer("Services")
        .defined_by(&["layered_app::service.."])
        .layer("Persistence")
        .defined_by(&["layered_app::persistence.."])
        .where_layer("Controllers")
        .may_not_be_accessed_by_any_layer()
        .where_layer("Services")
        .may_only_be_accessed_by_layers(&["Controllers"])
        .where_layer("Persistence")
        .may_only_be_accessed_by_layers(&["Services"])
        .ignore_dependency(
            "layered_app::controller::SomeController",
            "layered_app::persistence::first::dao::SomeDao",
        )
}

archunit_example! {
    example = "layered_architecture",
    fixture = "layered_app",
    rules = [layer_dependencies_are_respected, layer_dependencies_are_respected_with_exception],
}
