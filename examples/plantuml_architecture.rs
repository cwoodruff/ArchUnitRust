//! Port of `PlantUmlArchitectureTest`: `tests/fixtures/plantuml/layered_app.puml` describes the
//! components of `layered_app`, and the items must respect its arrows.
#[macro_use]
mod common;

use archunit::core::domain::PackageMatchers;
use archunit::core::domain::dependency::predicates::dependency;
use archunit::core::domain::rust_item::functions::get_package_name;
use archunit::core::domain::rust_item::predicates::equivalent_to;
use archunit::library::plantuml::{Configuration, adhere_to_plant_uml_diagram};
use archunit::prelude::*;

fn diagram() -> std::path::PathBuf {
    common::fixture_dir("plantuml").join("layered_app.puml")
}

fn classes_should_adhere_to_diagram_considering_only_dependencies_in_diagram() -> impl ArchRule {
    classes().should_with(adhere_to_plant_uml_diagram(
        diagram(),
        Configuration::considering_only_dependencies_in_diagram(),
    ))
}

fn classes_should_adhere_to_diagram_considering_all_dependencies_and_ignoring_some() -> impl ArchRule
{
    classes().should_with(
        adhere_to_plant_uml_diagram(diagram(), Configuration::considering_all_dependencies())
            .ignore_dependencies_with_origin(equivalent_to("layered_app::persistence::first::dao::SomeDao"))
            .ignore_dependencies_with_target(
                get_package_name()
                    .is(PackageMatchers::of(&["std..", "core..", "alloc.."]))
                    .as_("that is part of the standard library"),
            )
            .ignore_dependencies_with(
                dependency(
                    "layered_app::persistence::layerviolation::DaoCallingService",
                    "layered_app::service::ServiceOne",
                )
                .as_("ignoring dependencies from layered_app::persistence::layerviolation::DaoCallingService to layered_app::service::ServiceOne"),
            ),
    )
}

fn classes_should_adhere_to_diagram_considering_only_dependencies_in_any_package() -> impl ArchRule
{
    classes().should_with(adhere_to_plant_uml_diagram(
        diagram(),
        Configuration::considering_only_dependencies_in_any_package(&["..persistence.."]),
    ))
}

archunit_example! {
    example = "plantuml_architecture",
    fixture = "layered_app",
    rules = [
        classes_should_adhere_to_diagram_considering_only_dependencies_in_diagram,
        classes_should_adhere_to_diagram_considering_all_dependencies_and_ignoring_some,
        classes_should_adhere_to_diagram_considering_only_dependencies_in_any_package,
    ],
}
