//! The Library API: layered and onion architectures, slices and cycle detection.
//! Ports of the shape of `ArchitecturesTest`, `SlicesRuleDefinitionTest`, `SliceRuleTest`
//! and `CycleDetectorTest`.

mod common;

use archunit::base::{DescribedPredicate, HasDescription, always_true};
use archunit::config::ArchConfiguration;
use archunit::core::domain::dependency::predicates::{dependency_origin, dependency_target};
use archunit::core::domain::properties::can_be_annotated::predicates::annotated_with;
use archunit::core::domain::rust_item::predicates::reside_in_a_package;
use archunit::core::domain::{Dependency, RustItem, RustItems};
use archunit::lang::{ArchRule, EvaluationResult};
use archunit::library::cycle_detection::{CycleDetector, SimpleEdge};
use archunit::library::dependencies::{
    Slice, SliceIdentifier, Slices, SlicesRuleDefinition, slice_assignment, slices,
};
use archunit::library::{Architectures, layered_architecture, onion_architecture};
use common::import_fixture;

fn layered() -> RustItems {
    import_fixture("layered_app")
}

fn onion() -> RustItems {
    import_fixture("onion_app")
}

fn cyclic() -> RustItems {
    import_fixture("cyclic_app")
}

fn expected(name: &str) -> String {
    let path = format!("{}/tests/expected/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .trim_end()
        .to_owned()
}

fn report(rule: &dyn ArchRule, items: &RustItems) -> String {
    rule.evaluate(items).failure_report().to_string()
}

fn assert_report(rule: &dyn ArchRule, items: &RustItems, golden: &str) {
    let actual = report(rule, items);
    let update = std::env::var("UPDATE_EXPECTED").is_ok();
    if update {
        let path = format!("{}/tests/expected/{golden}.txt", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(&path, format!("{actual}\n")).unwrap();
    }
    assert_eq!(actual, expected(golden), "report for {golden}");
}

fn sorted_details(result: &EvaluationResult) -> Vec<String> {
    result.failure_report().details()
}

// ---- layered architecture: descriptions -----------------------------------------------------

fn three_layers() -> archunit::library::LayeredArchitecture {
    layered_architecture()
        .considering_all_dependencies()
        .layer("Controller")
        .defined_by(&["..controller.."])
        .layer("Service")
        .defined_by(&["..service.."])
        .layer("Persistence")
        .defined_by(&["..persistence.."])
        .where_layer("Controller")
        .may_not_be_accessed_by_any_layer()
        .where_layer("Service")
        .may_only_be_accessed_by_layers(&["Controller"])
        .where_layer("Persistence")
        .may_only_be_accessed_by_layers(&["Service"])
}

#[test]
fn layered_architecture_description_matches_archunit() {
    assert_eq!(
        three_layers().description(),
        "Layered architecture considering all dependencies, consisting of\n\
         layer 'Controller' ('..controller..')\n\
         layer 'Service' ('..service..')\n\
         layer 'Persistence' ('..persistence..')\n\
         where layer 'Controller' may not be accessed by any layer\n\
         where layer 'Service' may only be accessed by layers ['Controller']\n\
         where layer 'Persistence' may only be accessed by layers ['Service']"
    );
    assert_eq!(
        layered_architecture()
            .considering_only_dependencies_in_any_package(&["..layered_app.."])
            .layer("One")
            .defined_by(&["..one..", "..two.."])
            .optional_layer("Two")
            .defined_by_with(always_true::<RustItem>().as_("anything"))
            .where_layer("One")
            .may_not_access_any_layer()
            .where_layer("Two")
            .may_only_access_layers(&["One"])
            .with_optional_layers(true)
            .description(),
        "Layered architecture considering only dependencies in any package ['..layered_app..'], consisting of (optional)\n\
         layer 'One' ('..one..', '..two..')\n\
         optional layer 'Two' (anything)\n\
         where layer 'One' may not access any layer\n\
         where layer 'Two' may only access layers ['One']"
    );
    assert_eq!(
        layered_architecture()
            .considering_only_dependencies_in_layers()
            .layer("A")
            .defined_by(&["..a.."])
            .description(),
        "Layered architecture considering only dependencies in layers, consisting of\nlayer 'A' ('..a..')"
    );
    let overridden = three_layers().as_("my architecture");
    assert_eq!(overridden.description(), "my architecture");
    assert_eq!(
        three_layers().because("layers matter").description(),
        format!("{}, because layers matter", three_layers().description())
    );
}

#[test]
#[should_panic(expected = "There is no layer named 'Nope'")]
fn layered_architecture_rejects_unknown_layer_in_where_layer() {
    layered_architecture()
        .considering_all_dependencies()
        .layer("A")
        .defined_by(&["..a.."])
        .where_layer("Nope")
        .may_not_be_accessed_by_any_layer();
}

#[test]
#[should_panic(expected = "There is no layer named 'Nope'")]
fn layered_architecture_rejects_unknown_layer_in_allowed_layers() {
    layered_architecture()
        .considering_all_dependencies()
        .layer("A")
        .defined_by(&["..a.."])
        .where_layer("A")
        .may_only_be_accessed_by_layers(&["Nope"]);
}

// ---- layered architecture: evaluation -------------------------------------------------------

#[test]
fn layered_architecture_reports_layer_violations() {
    assert_report(
        &three_layers(),
        &layered(),
        "layered_architecture_violations",
    );
}

#[test]
fn layered_architecture_reports_empty_layers_unless_optional() {
    let items = layered();
    let with_empty = layered_architecture()
        .considering_all_dependencies()
        .layer("Controller")
        .defined_by(&["..controller.."])
        .layer("Nothing")
        .defined_by(&["..nothing_here.."])
        .where_layer("Nothing")
        .may_not_be_accessed_by_any_layer();
    let details = sorted_details(&with_empty.evaluate(&items));
    assert_eq!(details, vec!["Layer 'Nothing' is empty"]);

    let optional_layer = layered_architecture()
        .considering_all_dependencies()
        .layer("Controller")
        .defined_by(&["..controller.."])
        .optional_layer("Nothing")
        .defined_by(&["..nothing_here.."]);
    assert!(!optional_layer.evaluate(&items).has_violation());

    let optional_layers = with_empty.clone().with_optional_layers(true);
    assert!(!optional_layers.evaluate(&items).has_violation());
    let via_allow_empty_should = with_empty.clone().allow_empty_should(true);
    assert!(!via_allow_empty_should.evaluate(&items).has_violation());
    let boxed: Box<dyn ArchRule> = Box::new(with_empty);
    assert!(
        !boxed
            .allow_empty_should(true)
            .evaluate(&items)
            .has_violation()
    );
}

#[test]
fn layered_architecture_ignores_configured_dependencies() {
    let items = layered();
    let ignoring = three_layers()
        .ignore_dependency(
            "layered_app::controller::SomeController",
            "layered_app::persistence::first::dao::domain::SomeEntity",
        )
        .ignore_dependency_where(dependency_origin(reside_in_a_package("..layerviolation..")));
    let details = sorted_details(&ignoring.evaluate(&items));
    assert!(
        details
            .iter()
            .all(|d| !d.contains("SomeEntity") && !d.contains("DaoCallingService")),
        "{details:#?}"
    );
    assert!(!details.is_empty());
}

#[test]
fn layered_architecture_considering_only_dependencies_in_layers_skips_outsiders() {
    let items = layered();
    let architecture = layered_architecture()
        .considering_only_dependencies_in_layers()
        .layer("Service")
        .defined_by(&["..service.."])
        .layer("Persistence")
        .defined_by(&["..persistence.."])
        .where_layer("Persistence")
        .may_not_be_accessed_by_any_layer()
        .where_layer("Service")
        .may_not_be_accessed_by_any_layer();
    let details = sorted_details(&architecture.evaluate(&items));
    // Only accesses between the two layers count, accesses from controllers are irrelevant.
    assert!(
        details.iter().all(|d| !d.contains("controller")),
        "{details:#?}"
    );
    assert!(
        details.iter().any(|d| d.contains("DaoCallingService")),
        "{details:#?}"
    );

    let in_any_package = layered_architecture()
        .considering_only_dependencies_in_any_package(&["..persistence.."])
        .layer("Service")
        .defined_by(&["..service.."])
        .layer("Persistence")
        .defined_by(&["..persistence.."])
        .where_layer("Service")
        .may_not_be_accessed_by_any_layer();
    // Every dependency has an origin or target outside of `..persistence..` except the DAO ones.
    let details = sorted_details(&in_any_package.evaluate(&items));
    assert!(
        details.iter().all(|d| d.contains("persistence")),
        "{details:#?}"
    );
}

#[test]
fn layered_architecture_ensures_all_items_are_contained() {
    let items = layered();
    let architecture = three_layers().ensure_all_classes_are_contained_in_architecture();
    let details = sorted_details(&architecture.evaluate(&items));
    assert!(
        details.contains(
            &"Item <layered_app::core::VeryCentralCore> is not contained in architecture"
                .to_owned()
        ),
        "{details:#?}"
    );
    let ignoring = three_layers().ensure_all_classes_are_contained_in_architecture_ignoring(&[
        "..core..",
        "..security..",
        "..thirdparty..",
        "..anticorruption..",
    ]);
    let details = sorted_details(&ignoring.evaluate(&items));
    assert!(
        details.iter().all(|d| !d.contains("VeryCentralCore")),
        "{details:#?}"
    );
}

#[test]
fn layered_architecture_check_panics_with_report() {
    let items = layered();
    let result = std::panic::catch_unwind(|| three_layers().check(&items));
    let message = result.unwrap_err();
    let message = message
        .downcast_ref::<String>()
        .cloned()
        .unwrap_or_else(|| {
            message
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .unwrap()
        });
    assert!(
        message
            .starts_with("Architecture Violation [Priority: MEDIUM] - Rule 'Layered architecture"),
        "{message}"
    );
}

// ---- onion architecture ---------------------------------------------------------------------

fn onion_rule() -> archunit::library::OnionArchitecture {
    onion_architecture()
        .domain_models(&["..domain.model.."])
        .domain_services(&["..domain.service.."])
        .application_services(&["..application.."])
        .adapter("cli", &["..adapter.cli.."])
        .adapter("persistence", &["..adapter.persistence.."])
        .adapter("rest", &["..adapter.rest.."])
}

#[test]
fn onion_architecture_description_matches_archunit() {
    assert_eq!(
        onion_rule().description(),
        "Onion architecture consisting of\n\
         domain models ('..domain.model..')\n\
         domain services ('..domain.service..')\n\
         application services ('..application..')\n\
         adapter 'cli' ('..adapter.cli..')\n\
         adapter 'persistence' ('..adapter.persistence..')\n\
         adapter 'rest' ('..adapter.rest..')"
    );
    assert_eq!(
        onion_architecture()
            .with_optional_layers(true)
            .description(),
        "Onion architecture consisting of (optional)"
    );
    assert_eq!(
        Architectures::onion_architecture()
            .domain_models_with(annotated_with::<RustItem>("domain_model"))
            .description(),
        "Onion architecture consisting of\ndomain models (annotated with @domain_model)"
    );
}

#[test]
fn onion_architecture_reports_ring_violations() {
    assert_report(&onion_rule(), &onion(), "onion_architecture_violations");
}

#[test]
fn onion_architecture_by_annotations_reports_the_same_violations() {
    let items = onion();
    let by_attributes = onion_architecture()
        .domain_models_with(annotated_with::<RustItem>("domain_model"))
        .domain_services_with(annotated_with::<RustItem>("domain_service"))
        .application_services_with(annotated_with::<RustItem>("application"))
        .adapter_with(
            "cli",
            annotated_with::<RustItem>("adapter").and(reside_in_a_package("..cli..")),
        )
        .adapter_with(
            "persistence",
            annotated_with::<RustItem>("adapter").and(reside_in_a_package("..persistence..")),
        )
        .adapter_with(
            "rest",
            annotated_with::<RustItem>("adapter").and(reside_in_a_package("..rest..")),
        );
    // Module items carry the `use` dependencies and are never annotated, so they belong to no
    // ring here; apart from those lines the reports agree.
    let without_modules = |result: EvaluationResult| -> Vec<String> {
        sorted_details(&result)
            .into_iter()
            .filter(|d| !d.starts_with("Module <"))
            .collect()
    };
    let by_attributes = without_modules(by_attributes.evaluate(&items));
    assert!(!by_attributes.is_empty());
    assert_eq!(
        by_attributes,
        without_modules(onion_rule().evaluate(&items))
    );
}

#[test]
fn onion_architecture_ignores_dependencies_and_tolerates_empty_rings() {
    let items = onion();
    let ignoring = onion_rule()
        .ignore_dependency(
            "onion_app::domain::model::Order",
            "onion_app::domain::service::OrderRepository",
        )
        // the `use` in the module declaring `Order` is a dependency of the module item
        .ignore_dependency(
            "onion_app::domain::model::order",
            "onion_app::domain::service::OrderRepository",
        )
        .ignore_dependency_where(dependency_target(reside_in_a_package("..rest..")));
    let details = sorted_details(&ignoring.evaluate(&items));
    assert_eq!(details.len(), 3, "{details:#?}");
    assert!(
        details.iter().all(|d| d.contains("AdministrationCli")),
        "{details:#?}"
    );

    let empty_ring = onion_architecture()
        .domain_models(&["..domain.model.."])
        .domain_services(&["..domain.service.."])
        .application_services(&["..nowhere.."]);
    let details = sorted_details(&empty_ring.evaluate(&items));
    assert!(
        details.contains(&"Layer 'application service' is empty".to_owned()),
        "{details:#?}"
    );
    assert!(
        details.contains(&"Layer 'adapter' is empty".to_owned()),
        "{details:#?}"
    );
    let details = sorted_details(&empty_ring.with_optional_layers(true).evaluate(&items));
    assert!(
        details.iter().all(|d| !d.contains("is empty")),
        "{details:#?}"
    );
}

#[test]
fn onion_architecture_ensures_all_items_are_contained() {
    let items = onion();
    let details = sorted_details(
        &onion_rule()
            .ensure_all_classes_are_contained_in_architecture()
            .evaluate(&items),
    );
    assert!(
        details.iter().all(|d| !d.contains("not contained")),
        "{details:#?}"
    );
    let details = sorted_details(
        &onion_architecture()
            .domain_models(&["..domain.model.."])
            .with_optional_layers(true)
            .ensure_all_classes_are_contained_in_architecture_ignoring(&[
                "..adapter..",
                "..application..",
            ])
            .evaluate(&items),
    );
    let not_contained: Vec<&String> = details
        .iter()
        .filter(|d| d.contains("not contained"))
        .collect();
    assert_eq!(
        not_contained,
        vec![
            "Item <onion_app::domain::service::shopping::OrderRepository> is not contained in architecture",
            "Item <onion_app::domain::service::shopping::ProductRepository> is not contained in architecture",
            "Item <onion_app::domain::service::shopping::ShoppingService> is not contained in architecture",
        ]
    );
}

// ---- slices: descriptions and transformation ------------------------------------------------

#[test]
fn slices_rule_descriptions_match_archunit() {
    assert_eq!(
        slices()
            .matching("..cyclic_app.(*)..")
            .should()
            .be_free_of_cycles()
            .description(),
        "slices matching '..cyclic_app.(*)..' should be free of cycles"
    );
    assert_eq!(
        slices()
            .matching("..cyclic_app.(*)..")
            .should()
            .not_depend_on_each_other()
            .description(),
        "slices matching '..cyclic_app.(*)..' should not depend on each other"
    );
    let changed = DescribedPredicate::describe("changed", |_: &Slice| true);
    assert_eq!(
        SlicesRuleDefinition::slices()
            .matching("..cyclic_app.(*)..")
            .that(changed.clone())
            .and(changed.clone())
            .or(changed.clone())
            .should()
            .be_free_of_cycles()
            .description(),
        "slices matching '..cyclic_app.(*)..' that changed and changed or changed should be free of cycles"
    );
    assert_eq!(
        slices()
            .assigned_from(slice_assignment("some description", |_| {
                SliceIdentifier::ignore()
            }))
            .should()
            .be_free_of_cycles()
            .because("reasons")
            .description(),
        "slices assigned from some description should be free of cycles, because reasons"
    );
    assert_eq!(
        slices()
            .matching("..(*)..")
            .as_("my slices")
            .should()
            .be_free_of_cycles()
            .as_("custom")
            .description(),
        "custom"
    );
    assert_eq!(
        slices()
            .matching("..(*)..")
            .naming_slices("Layer $1")
            .that(changed)
            .should()
            .be_free_of_cycles()
            .description(),
        "slices matching '..(*)..' that changed should be free of cycles"
    );
}

#[test]
fn slices_are_built_from_capture_groups() {
    let items = cyclic();
    let slices = Slices::matching("..cyclic_app.(*)..").of(&items);
    let mut names: Vec<String> = slices.iter().map(|s| s.description()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "Slice complexcycles",
            "Slice constructorcycle",
            "Slice fieldaccesscycle",
            "Slice inheritancecycle",
            "Slice membercycle",
            "Slice simplecycle",
            "Slice simplescenario",
        ]
    );
    assert_eq!(slices.description(), "slices matching '..cyclic_app.(*)..'");

    let named = Slices::matching("..cyclic_app.(*).(*)")
        .naming_slices("$2 of $1")
        .of(&items);
    let mut names: Vec<String> = named.iter().map(|s| s.description()).collect();
    names.sort();
    assert!(
        names.contains(&"slice1 of simplecycle".to_owned()),
        "{names:#?}"
    );
    let slice = named
        .iter()
        .find(|s| s.description() == "slice1 of simplecycle")
        .unwrap();
    assert_eq!(slice.name_part(1), "simplecycle");
    assert_eq!(slice.name_part(2), "slice1");
    assert_eq!(
        slice.identifier(),
        &SliceIdentifier::of(&["simplecycle", "slice1"])
    );
    assert_eq!(slice.len(), 1, "modules are packages, not items of a slice");
    let deps = slice.dependencies_from_self();
    assert!(
        deps.iter()
            .all(|d| !d.target_item().name().contains("simplecycle::slice1")),
        "{deps:#?}"
    );
    assert!(
        deps.iter()
            .any(|d| d.target_item().name().contains("simplecycle::slice2")),
        "{deps:#?}"
    );
    let deps_to = slice.dependencies_to_self();
    assert!(
        deps_to
            .iter()
            .any(|d| d.origin_item().name().contains("simplecycle::slice3")),
        "{deps_to:#?}"
    );

    let filtered = Slices::matching("..cyclic_app.(*)..")
        .that(DescribedPredicate::describe(
            "start with simple",
            |s: &Slice| s.description().contains("simple"),
        ))
        .of(&items);
    assert_eq!(filtered.len(), 2);
    assert_eq!(
        filtered.description(),
        "slices matching '..cyclic_app.(*)..' that start with simple"
    );

    let custom = Slices::assigned_from(slice_assignment("crate roots", |item: &RustItem| {
        SliceIdentifier::of(&[&item.crate_name()])
    }))
    .of(&items);
    assert_eq!(custom.len(), 1);
    assert_eq!(
        custom.iter().next().unwrap().description(),
        "Slice cyclic_app"
    );
}

// ---- slices: rules --------------------------------------------------------------------------

#[test]
fn slices_should_be_free_of_cycles_reports_cycles() {
    let items = cyclic();
    assert_report(
        &slices()
            .matching("..simplecycle.(*)..")
            .should()
            .be_free_of_cycles(),
        &items,
        "simple_cycle",
    );
    assert_report(
        &slices()
            .matching("..simplescenario.(*)..")
            .should()
            .be_free_of_cycles(),
        &items,
        "simple_scenario_cycle",
    );
    let top_level = slices()
        .matching("..cyclic_app.(*)..")
        .should()
        .be_free_of_cycles();
    assert!(!top_level.evaluate(&items).has_violation());
    top_level.check(&items);
}

#[test]
fn slices_should_be_free_of_cycles_finds_every_kind_of_dependency() {
    let items = cyclic();
    for (package, cycles, verb) in [
        ("constructorcycle", 1, "calls constructor"),
        ("fieldaccesscycle", 1, "field"),
        ("inheritancecycle", 2, "implements trait"),
        ("membercycle", 1, "has return type"),
    ] {
        let result = slices()
            .matching(&format!("..{package}::(*).."))
            .should()
            .be_free_of_cycles()
            .evaluate(&items);
        let report = result.failure_report().to_string();
        assert_eq!(
            result.failure_report().details().len(),
            cycles,
            "{package}: {report}"
        );
        assert!(
            report.contains("Cycle detected: Slice slice1 -> \n                Slice slice2"),
            "{package}: {report}"
        );
        assert!(report.contains(verb), "{package}: {report}");
    }
    let complex = slices()
        .matching("..complexcycles.(*)..")
        .should()
        .be_free_of_cycles()
        .evaluate(&items);
    let report = complex.failure_report().to_string();
    assert!(
        report.contains("was violated (") && !report.contains("(1 times)"),
        "{report}"
    );
    assert!(!report.contains("slice6"), "{report}");
}

#[test]
fn slices_should_be_free_of_cycles_respects_the_configured_limits() {
    let items = cyclic();
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_property("cycles.max_number_to_detect", "1");
        ArchConfiguration::set_property("cycles.max_number_of_dependencies_per_edge", "1");
        let report = slices()
            .matching("..complexcycles.(*)..")
            .should()
            .be_free_of_cycles()
            .evaluate(&items)
            .failure_report()
            .to_string();
        assert!(
            report.contains("was violated ( >= 1 times - the maximum number of cycles to detect has been reached; this limit can be adapted using the `archunit.toml` value `cycles.max_number_to_detect = xxx`):"),
            "{report}"
        );
        assert!(
            report.contains("further dependencies have been omitted...)"),
            "{report}"
        );
    });
}

#[test]
fn slices_should_not_depend_on_each_other_reports_slice_dependencies() {
    let items = layered();
    assert_report(
        &slices()
            .matching("..controller.(*)..")
            .should()
            .not_depend_on_each_other(),
        &items,
        "controller_slices_depend_on_each_other",
    );
    let ignoring = slices()
        .matching("..controller.(*)..")
        .should()
        .not_depend_on_each_other()
        .ignore_dependency(
            "layered_app::controller::one::UseCaseOneTwoController",
            "layered_app::controller::two::UseCaseTwoController",
        );
    let details = sorted_details(&ignoring.evaluate(&items));
    assert_eq!(details.len(), 1, "{details:#?}");
    assert!(
        details[0].starts_with("Slice three depends on Slice one:"),
        "{details:#?}"
    );
    let ignoring_all = ignoring.ignore_dependency_where(always_true::<Dependency>());
    assert!(!ignoring_all.evaluate(&items).has_violation());
}

#[test]
fn slice_rule_violations_convert_to_dependencies() {
    let items = layered();
    let result = slices()
        .matching("..controller.(*)..")
        .should()
        .not_depend_on_each_other()
        .evaluate(&items);
    let mut dependencies = Vec::new();
    result.handle_violations::<Dependency>(|deps: Vec<Dependency>, _message: &str| {
        dependencies.extend(deps)
    });
    let mut described: Vec<String> = dependencies.iter().map(|d| d.description()).collect();
    described.sort();
    assert!(
        described
            .iter()
            .any(|d| d.contains("UseCaseOneTwoController::use_case_one()")),
        "{described:#?}"
    );

    let result = slices()
        .matching("..simplecycle.(*)..")
        .should()
        .be_free_of_cycles()
        .evaluate(&items_cyclic());
    let mut count = 0;
    result.handle_violations::<Dependency>(|deps: Vec<Dependency>, _: &str| count += deps.len());
    assert!(count >= 3, "{count}");
}

fn items_cyclic() -> RustItems {
    cyclic()
}

#[test]
fn slice_rule_allow_empty_should_and_custom_conditions() {
    let items = layered();
    let no_slices = slices()
        .matching("..nothing.(*)..")
        .should()
        .be_free_of_cycles();
    let panicked =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| no_slices.check(&items))).is_err();
    assert!(panicked);
    no_slices.clone().allow_empty_should(true).check(&items);
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_fail_on_empty_should(false);
        no_slices.check(&items);
    });

    let custom =
        slices()
            .matching("..controller.(*)..")
            .should_with(archunit::lang::ArchCondition::new(
                "not be empty",
                |slice: &Slice, events| {
                    events.add(archunit::lang::SimpleConditionEvent::new(
                        slice,
                        !slice.is_empty(),
                        format!("{} has {} items", slice.description(), slice.len()),
                    ));
                },
            ));
    assert_eq!(
        custom.description(),
        "slices matching '..controller.(*)..' should not be empty"
    );
    assert!(!custom.evaluate(&items).has_violation());
}

// ---- cycle detector ------------------------------------------------------------------------

#[test]
fn cycle_detector_finds_elementary_cycles_in_order() {
    let nodes = ["a", "b", "c", "d"];
    let edges = [
        SimpleEdge::new("a", "b"),
        SimpleEdge::new("b", "c"),
        SimpleEdge::new("c", "a"),
        SimpleEdge::new("b", "a"),
        SimpleEdge::new("c", "d"),
    ];
    let cycles = CycleDetector::detect_cycles(&nodes, &edges);
    assert_eq!(cycles.len(), 2);
    assert!(!cycles.max_number_of_cycles_reached());
    let mut found: Vec<Vec<&str>> = cycles
        .iter()
        .map(|c| {
            c.edges()
                .iter()
                .map(|e| *archunit::library::cycle_detection::Edge::origin(e))
                .collect()
        })
        .collect();
    found.sort();
    assert_eq!(found, vec![vec!["a", "b"], vec!["a", "b", "c"]]);

    let limited = CycleDetector::detect_cycles_with_limit(&nodes, &edges, 1);
    assert_eq!(limited.len(), 1);
    assert!(limited.max_number_of_cycles_reached());

    let acyclic = CycleDetector::detect_cycles(
        &nodes,
        &[SimpleEdge::new("a", "b"), SimpleEdge::new("b", "c")],
    );
    assert!(acyclic.is_empty());
}

#[test]
fn cycle_detector_handles_multiple_components() {
    let nodes: Vec<u32> = (0..8).collect();
    let edges = [
        SimpleEdge::new(0, 1),
        SimpleEdge::new(1, 0),
        SimpleEdge::new(2, 3),
        SimpleEdge::new(3, 4),
        SimpleEdge::new(4, 2),
        SimpleEdge::new(4, 3),
        SimpleEdge::new(5, 6),
        SimpleEdge::new(6, 7),
        SimpleEdge::new(1, 5),
    ];
    let cycles = CycleDetector::detect_cycles(&nodes, &edges);
    let mut found: Vec<Vec<u32>> = cycles
        .iter()
        .map(|c| {
            c.edges()
                .iter()
                .map(|e| *archunit::library::cycle_detection::Edge::origin(e))
                .collect()
        })
        .collect();
    found.sort();
    assert_eq!(found, vec![vec![0, 1], vec![2, 3, 4], vec![3, 4]]);
}
