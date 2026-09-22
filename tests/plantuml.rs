//! PlantUML diagrams as rules: the parser (port of `PlantUmlParserTest`) and the condition
//! (port of `PlantUmlArchConditionTest`) against `tests/fixtures/layered_app`.

mod common;

use archunit::core::domain::RustItems;
use archunit::core::domain::rust_item::predicates::{
    reside_in_a_package, reside_outside_of_packages,
};
use archunit::lang::ArchRule;
use archunit::library::plantuml::{
    Configuration, PlantUmlDiagram, PlantUmlError, adhere_to_plant_uml_diagram,
    adhere_to_plant_uml_diagram_from_str, try_adhere_to_plant_uml_diagram,
};
use archunit::prelude::*;
use common::{fixture_path, import_fixture};

fn diagram_path() -> std::path::PathBuf {
    fixture_path("plantuml").join("layered_app.puml")
}

fn layered() -> RustItems {
    import_fixture("layered_app")
}

fn expected(name: &str) -> String {
    let path = format!("{}/tests/expected/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .trim_end()
        .to_owned()
}

fn assert_report(rule: &dyn ArchRule, items: &RustItems, golden: &str) {
    let actual = rule.evaluate(items).failure_report().to_string();
    if std::env::var("UPDATE_EXPECTED").is_ok() {
        let path = format!("{}/tests/expected/{golden}.txt", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(&path, format!("{actual}\n")).unwrap();
    }
    assert_eq!(actual, expected(golden), "report for {golden}");
}

fn names(diagram: &PlantUmlDiagram, component: &str) -> Vec<String> {
    let component = diagram
        .components()
        .iter()
        .find(|c| c.name() == component)
        .unwrap();
    let mut names: Vec<String> = diagram
        .dependencies_of(component)
        .iter()
        .map(|c| c.name().to_owned())
        .collect();
    names.sort();
    names
}

// ---- parser --------------------------------------------------------------------------------

#[test]
fn parses_components_stereotypes_aliases_and_dependencies() {
    let diagram = PlantUmlDiagram::parse_file(diagram_path()).unwrap();
    let mut components: Vec<&str> = diagram.components().iter().map(|c| c.name()).collect();
    components.sort_unstable();
    assert_eq!(
        components,
        [
            "Anticorruption",
            "Controller",
            "Core",
            "Persistence",
            "Security",
            "Service"
        ]
    );
    let security = diagram
        .components()
        .iter()
        .find(|c| c.name() == "Security")
        .unwrap();
    assert_eq!(security.stereotypes(), ["..security..", "..thirdparty.."]);
    assert_eq!(security.alias(), Some("security"));
    let mut aliases: Vec<&str> = diagram
        .components_with_alias()
        .iter()
        .map(|c| c.alias().unwrap())
        .collect();
    aliases.sort_unstable();
    assert_eq!(aliases, ["persistence", "security", "service"]);

    assert_eq!(names(&diagram, "Controller"), ["Core", "Service"]);
    assert_eq!(names(&diagram, "Service"), ["Core", "Persistence"]);
    assert_eq!(names(&diagram, "Anticorruption"), ["Core"]);
    assert_eq!(names(&diagram, "Security"), ["Controller"]);
    assert!(names(&diagram, "Core").is_empty());
    assert!(names(&diagram, "Persistence").is_empty());
}

#[test]
fn parser_grammar_cases() {
    let diagram = PlantUmlDiagram::parse(
        "@startuml\n\
         ' [Commented] <<..commented..>> as commentedAlias\n\
         [SomeOrigin] <<..origin..>>\n\
         component [Some Target] <<..target..>> as target #F0808080\n\
         [Third] <<..third..>> <<..also..>> as \"third\"\n\
         [SomeOrigin] --> target : this part should be ignored, no matter the comment tick ' \n\
         [Third] <-[#green]- [SomeOrigin]\n\
         target -up-> [Third]\n\
         [Third] ..> [SomeOrigin]\n\
         [NotDefined] --> [SomeOrigin]\n\
         @enduml",
    )
    .unwrap();
    assert_eq!(diagram.components().len(), 3);
    assert_eq!(names(&diagram, "SomeOrigin"), ["Some Target", "Third"]);
    assert_eq!(names(&diagram, "Some Target"), ["Third"]);
    assert!(
        names(&diagram, "Third").is_empty(),
        "dotted arrows are not dependencies"
    );

    assert_eq!(
        PlantUmlDiagram::parse("[NoStereotype]").unwrap_err(),
        PlantUmlError::IllegalDiagram(
            "Components must include at least one stereotype specifying the package identifier(<<..>>), but component 'NoStereotype' does not".to_owned()
        )
    );
    assert!(PlantUmlDiagram::parse_file("/nowhere/missing.puml").is_err());
}

// ---- condition descriptions ----------------------------------------------------------------

#[test]
fn condition_descriptions_match_archunit() {
    let all = adhere_to_plant_uml_diagram(
        diagram_path(),
        Configuration::considering_all_dependencies(),
    );
    assert_eq!(
        all.description(),
        "adhere to PlantUML diagram <layered_app.puml>"
    );
    let in_diagram = adhere_to_plant_uml_diagram(
        diagram_path(),
        Configuration::considering_only_dependencies_in_diagram(),
    );
    assert_eq!(
        in_diagram.description(),
        "adhere to PlantUML diagram <layered_app.puml> while ignoring dependencies not contained in the diagram"
    );
    let in_packages = adhere_to_plant_uml_diagram(
        diagram_path(),
        Configuration::considering_only_dependencies_in_any_package(&[
            "..layered_app..",
            "..other..",
        ]),
    );
    assert_eq!(
        in_packages.description(),
        "adhere to PlantUML diagram <layered_app.puml> while ignoring dependencies outside of packages ['..layered_app..', '..other..']"
    );
    let with_ignores = all
        .ignore_dependencies_with_origin(reside_in_a_package("..security.."))
        .ignore_dependencies_with_target(reside_in_a_package("..core.."))
        .ignore_dependencies("layered_app::a::A", "layered_app::b::B");
    assert_eq!(
        with_ignores.description(),
        "adhere to PlantUML diagram <layered_app.puml>, ignoring dependencies with origin reside in a package '..security..', ignoring dependencies with target reside in a package '..core..', ignoring dependencies from layered_app::a::A to layered_app::b::B"
    );
    assert_eq!(
        classes().should_with(with_ignores).description(),
        "classes should adhere to PlantUML diagram <layered_app.puml>, ignoring dependencies with origin reside in a package '..security..', ignoring dependencies with target reside in a package '..core..', ignoring dependencies from layered_app::a::A to layered_app::b::B"
    );
}

#[test]
fn rejects_illegal_diagrams() {
    let err = adhere_to_plant_uml_diagram_from_str(
        "empty.puml",
        "@startuml\n@enduml",
        Configuration::considering_all_dependencies(),
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "No components defined in diagram <empty.puml>"
    );
    let err = adhere_to_plant_uml_diagram_from_str(
        "dup.puml",
        "[A] <<..a..>>\n[B] <<..a..>>",
        Configuration::considering_all_dependencies(),
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "Stereotype '..a..' should be unique");
    let err = adhere_to_plant_uml_diagram_from_str(
        "alias.puml",
        "[A] <<..a..>> as ill[]egal",
        Configuration::considering_all_dependencies(),
    );
    assert!(err.is_err());
    assert!(
        try_adhere_to_plant_uml_diagram(
            "/nowhere.puml",
            Configuration::considering_all_dependencies()
        )
        .is_err()
    );
}

// ---- evaluation ----------------------------------------------------------------------------

#[test]
fn reports_dependencies_not_allowed_by_the_diagram() {
    let items = layered();
    let rule = classes().should_with(adhere_to_plant_uml_diagram(
        diagram_path(),
        Configuration::considering_only_dependencies_in_diagram(),
    ));
    assert_report(&rule, &items, "plantuml_layered_app");

    let all_dependencies = classes().should_with(adhere_to_plant_uml_diagram(
        diagram_path(),
        Configuration::considering_all_dependencies(),
    ));
    let details = all_dependencies.evaluate(&items).failure_report().details();
    assert!(
        details.iter().any(|d| d.contains("<std::")),
        "std dependencies count: {details:#?}"
    );

    let in_packages = classes().should_with(adhere_to_plant_uml_diagram(
        diagram_path(),
        Configuration::considering_only_dependencies_in_any_package(&["..layered_app.."]),
    ));
    let in_packages_details = in_packages.evaluate(&items).failure_report().details();
    assert_eq!(
        in_packages_details,
        rule.evaluate(&items).failure_report().details()
    );
}

#[test]
fn reports_items_outside_of_every_component_and_in_several_components() {
    let items = layered();
    let partial = adhere_to_plant_uml_diagram_from_str(
        "partial.puml",
        "[Controller] <<..controller..>>\n[Service] <<..service..>>\n[Controller] --> [Service]",
        Configuration::considering_only_dependencies_in_diagram(),
    )
    .unwrap();
    let details = classes()
        .should_with(partial)
        .evaluate(&items)
        .failure_report()
        .details();
    assert!(
        details.contains(&"Item layered_app::persistence::layerviolation::DaoCallingService is not contained in any component".to_owned()),
        "{details:#?}"
    );
    // Items whose dependencies are all ignored are not checked at all.
    assert!(
        !details.iter().any(|d| d.contains("VeryCentralCore")),
        "{details:#?}"
    );

    let overlapping = adhere_to_plant_uml_diagram_from_str(
        "overlap.puml",
        "[First] <<..controller..>>\n[Second] <<..controller::one..>>\n[First] --> [Second]",
        Configuration::considering_only_dependencies_in_diagram(),
    )
    .unwrap();
    let details = classes()
        .that()
        .reside_in_a_package("..controller::one..")
        .should_with(overlapping)
        .evaluate(&items)
        .failure_report()
        .details();
    assert_eq!(
        details,
        [
            "Item layered_app::controller::one::UseCaseOneTwoController may not be contained in more than one component, but is contained in [First, Second]"
        ]
    );
}

#[test]
fn ignored_dependencies_are_not_reported() {
    let items = layered();
    let condition = adhere_to_plant_uml_diagram(
        diagram_path(),
        Configuration::considering_only_dependencies_in_diagram(),
    )
    .ignore_dependencies_with_origin(reside_in_a_package("..persistence.."))
    .ignore_dependencies_with_target(reside_in_a_package("..persistence.."))
    .ignore_dependencies(
        "layered_app::service::ServiceViolatingLayerRules",
        "layered_app::controller::SomeGuiController",
    )
    .ignore_dependencies_with(
        archunit::core::domain::dependency::predicates::dependency_origin(
            reside_outside_of_packages(&["..controller..", "..service..", "..persistence.."]),
        ),
    );
    let details = classes()
        .should_with(condition)
        .evaluate(&items)
        .failure_report()
        .details();
    assert!(
        details
            .iter()
            .all(|d| d
                .starts_with("Module <layered_app::service> imports <layered_app::controller::")),
        "{details:#?}"
    );
}
