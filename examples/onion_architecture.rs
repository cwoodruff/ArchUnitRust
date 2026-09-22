//! Port of `OnionArchitectureTest` against `tests/fixtures/onion_app`, by module identifiers
//! and by attributes (`#[domain_model]`, `#[domain_service]`, `#[application]`,
//! `#[adapter("cli")]`).
#[macro_use]
mod common;

use archunit::base::DescribedPredicate;
use archunit::core::domain::dependency::predicates::dependency_origin;
use archunit::core::domain::properties::AnnotationSelector;
use archunit::core::domain::properties::can_be_annotated::predicates::annotated_with;
use archunit::core::domain::rust_item::predicates::{belong_to, modules};
use archunit::core::domain::{AnnotationValue, RustAnnotation, RustItem};
use archunit::prelude::*;

fn onion_architecture_is_respected() -> impl ArchRule {
    onion_architecture()
        .domain_models(&["..domain.model.."])
        .domain_services(&["..domain.service.."])
        .application_services(&["..application.."])
        .adapter("cli", &["..adapter.cli.."])
        .adapter("persistence", &["..adapter.persistence.."])
        .adapter("rest", &["..adapter.rest.."])
}

fn onion_architecture_is_respected_with_exception() -> impl ArchRule {
    onion_architecture()
        .domain_models(&["..domain.model.."])
        .domain_services(&["..domain.service.."])
        .application_services(&["..application.."])
        .adapter("cli", &["..adapter.cli.."])
        .adapter("persistence", &["..adapter.persistence.."])
        .adapter("rest", &["..adapter.rest.."])
        .ignore_dependency(
            "onion_app::domain::model::Order",
            "onion_app::domain::service::OrderRepository",
        )
}

fn onion_architecture_defined_by_annotations() -> impl ArchRule {
    onion_architecture()
        .domain_models_with(by_annotation("domain_model"))
        .domain_services_with(by_annotation("domain_service"))
        .application_services_with(by_annotation("application"))
        .adapter_with("cli", by_annotation(adapter("cli")))
        .adapter_with("persistence", by_annotation(adapter("persistence")))
        .adapter_with("rest", by_annotation(adapter("rest")))
        // Modules carry the `use` dependencies and cannot be annotated, so they are outside of
        // every ring; ignore their imports (see docs/MAPPING.md, Phase 3 notes).
        .ignore_dependency_where(dependency_origin(modules()))
}

fn by_annotation(annotation: impl Into<AnnotationSelector>) -> DescribedPredicate<RustItem> {
    let annotated_with = annotated_with::<RustItem>(annotation);
    let description = annotated_with.description().to_owned();
    belong_to(annotated_with).as_(description)
}

fn adapter(adapter_name: &str) -> DescribedPredicate<RustAnnotation> {
    let name = adapter_name.to_owned();
    DescribedPredicate::describe(
        format!("@adapter(\"{adapter_name}\")"),
        move |a: &RustAnnotation| {
            a.matches("adapter") && a.get("value") == Some(&AnnotationValue::Str(name.clone()))
        },
    )
}

archunit_example! {
    example = "onion_architecture",
    fixture = "onion_app",
    rules = [
        onion_architecture_is_respected,
        onion_architecture_is_respected_with_exception,
        onion_architecture_defined_by_annotations,
    ],
}
