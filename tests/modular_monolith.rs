//! `Architectures::modular_monolith()` `[rust-only]` against `tests/fixtures/modular_app`.

mod common;

use archunit::base::HasDescription;
use archunit::core::domain::RustItems;
use archunit::core::domain::dependency::predicates::dependency_target;
use archunit::core::domain::rust_item::predicates::{reside_in_a_package, reside_in_any_package};
use archunit::lang::ArchRule;
use archunit::library::{Architectures, ModularMonolithArchitecture, modular_monolith};
use common::import_fixture;

fn modular() -> RustItems {
    import_fixture("modular_app")
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

fn details(rule: &dyn ArchRule, items: &RustItems) -> Vec<String> {
    rule.evaluate(items).failure_report().details()
}

fn business_modules() -> ModularMonolithArchitecture {
    modular_monolith()
        .module("orders")
        .defined_by(&["..orders.."])
        .module("billing")
        .defined_by(&["..billing.."])
        .module("inventory")
        .defined_by(&["..inventory.."])
        .module("shared")
        .defined_by(&["..shared.."])
}

#[test]
fn description_lists_modules_and_constraints() {
    let architecture = business_modules()
        .where_module("shared")
        .may_not_depend_on_any_module()
        .where_module("billing")
        .may_only_depend_on_modules(&["shared"])
        .where_module("orders")
        .may_only_be_depended_on_by_modules(&["billing"])
        .where_module("inventory")
        .may_not_be_depended_on_by_any_module()
        .modules_may_only_depend_on_each_other_through_packages(&["..api.."])
        .modules_should_be_free_of_cycles();
    assert_eq!(
        architecture.description(),
        "Modular monolith consisting of\n\
         module 'orders' ('..orders..')\n\
         module 'billing' ('..billing..')\n\
         module 'inventory' ('..inventory..')\n\
         module 'shared' ('..shared..')\n\
         where module 'shared' may not depend on any module\n\
         where module 'billing' may only depend on modules ['shared']\n\
         where module 'orders' may only be depended on by modules ['billing']\n\
         where module 'inventory' may not be depended on by any module\n\
         where modules may only depend on each other through items that reside in any package ['..api..']\n\
         where modules should be free of cycles"
    );
    assert_eq!(
        Architectures::modular_monolith()
            .modules_defined_by_packages("..modular_app.(*)..")
            .naming_modules("$1 module")
            .optional_module("legacy")
            .defined_by_with(reside_in_a_package("..legacy..").as_("legacy code"))
            .with_optional_modules(true)
            .description(),
        "Modular monolith consisting of (optional)\n\
         optional module 'legacy' (legacy code)\n\
         modules defined by packages '..modular_app.(*)..' named '$1 module'"
    );
    assert_eq!(
        business_modules().as_("my monolith").description(),
        "my monolith"
    );
    assert_eq!(
        business_modules().because("modules matter").description(),
        format!(
            "{}, because modules matter",
            business_modules().description()
        )
    );
}

#[test]
#[should_panic(expected = "There is no module named 'nope'")]
fn rejects_unknown_module_names() {
    business_modules()
        .where_module("orders")
        .may_only_depend_on_modules(&["nope"]);
}

#[test]
fn reports_forbidden_module_dependencies_api_bypasses_and_cycles() {
    let items = modular();
    let architecture = business_modules()
        .where_module("shared")
        .may_not_depend_on_any_module()
        .where_module("billing")
        .may_only_depend_on_modules(&["shared"])
        .where_module("inventory")
        .may_only_depend_on_modules(&["shared"])
        .modules_may_only_depend_on_each_other_through_packages(&["..api.."])
        .modules_should_be_free_of_cycles();
    assert_report(&architecture, &items, "modular_monolith_violations");
}

#[test]
fn modules_defined_by_packages_find_the_same_violations() {
    let items = modular();
    let explicit = business_modules()
        .where_module("billing")
        .may_only_depend_on_modules(&["shared"])
        .modules_should_be_free_of_cycles();
    let by_pattern = modular_monolith()
        .modules_defined_by_packages("..modular_app.(*)..")
        .where_module("billing")
        .may_only_depend_on_modules(&["shared"])
        .modules_should_be_free_of_cycles();
    let mut explicit_details = details(&explicit, &items);
    explicit_details.sort();
    let mut pattern_details = details(&by_pattern, &items);
    pattern_details.sort();
    // `util` becomes a module of its own with the pattern, which changes nothing here.
    assert_eq!(explicit_details, pattern_details);
    assert!(!explicit_details.is_empty());

    let named = modular_monolith()
        .modules_defined_by_packages("..modular_app.(*)..")
        .naming_modules("$1 module")
        .modules_should_be_free_of_cycles();
    let report = named.evaluate(&items).failure_report().to_string();
    assert!(
        report.contains("Cycle detected: Module 'billing module' -> \n                Module 'orders module' -> \n                Module 'billing module'"),
        "{report}"
    );
}

#[test]
fn dependencies_outside_of_modules_are_irrelevant() {
    let items = modular();
    // `orders::internal` calls `util::helper`; `util` is in no module and `std` is not either,
    // so with the dependencies on the other modules ignored nothing is left to report.
    let strict = business_modules()
        .where_module("orders")
        .may_not_depend_on_any_module()
        .ignore_dependency_where(dependency_target(reside_in_any_package(&[
            "..billing..",
            "..shared..",
        ])));
    assert!(
        !strict.evaluate(&items).has_violation(),
        "{}",
        strict.evaluate(&items).failure_report()
    );

    let through_public_items = business_modules()
        .modules_may_only_depend_on_each_other_through_items_that(
            archunit::core::domain::properties::has_modifiers::predicates::modifier::<
                archunit::core::domain::RustItem,
            >(archunit::core::domain::RustModifier::Pub)
            .as_("are public"),
        );
    // Everything crossing a module boundary in the fixture is `pub`, only the package differs.
    assert!(!through_public_items.evaluate(&items).has_violation());
}

#[test]
fn ignored_dependencies_optional_modules_and_containment() {
    let items = modular();
    let ignoring = business_modules()
        .modules_should_be_free_of_cycles()
        .modules_may_only_depend_on_each_other_through_packages(&["..api.."])
        .ignore_dependency(
            "modular_app::billing::internal::Ledger",
            "modular_app::orders::api::OrderService",
        )
        .ignore_dependency_where(dependency_target(reside_in_a_package(
            "..billing::internal..",
        )));
    assert!(
        !ignoring.evaluate(&items).has_violation(),
        "{}",
        ignoring.evaluate(&items).failure_report()
    );

    let with_empty = business_modules()
        .module("reporting")
        .defined_by(&["..reporting.."]);
    assert_eq!(
        details(&with_empty, &items),
        ["Module 'reporting' is empty"]
    );
    assert!(
        !with_empty
            .clone()
            .with_optional_modules(true)
            .evaluate(&items)
            .has_violation()
    );
    assert!(
        !with_empty
            .clone()
            .allow_empty_should(true)
            .evaluate(&items)
            .has_violation()
    );
    let optional = business_modules()
        .optional_module("reporting")
        .defined_by(&["..reporting.."]);
    assert!(!optional.evaluate(&items).has_violation());

    let contained = business_modules().ensure_all_classes_are_contained_in_architecture();
    assert_eq!(
        details(&contained, &items),
        ["Item <modular_app::util::helper> is not contained in architecture"]
    );
    let ignoring_util =
        business_modules().ensure_all_classes_are_contained_in_architecture_ignoring(&["..util.."]);
    assert!(!ignoring_util.evaluate(&items).has_violation());
}

#[test]
fn works_as_a_boxed_rule() {
    let items = modular();
    let boxed: Box<dyn ArchRule> = Box::new(business_modules().modules_should_be_free_of_cycles());
    assert!(boxed.evaluate(&items).has_violation());
    let described = boxed.because("cycles couple modules");
    assert!(
        described
            .description()
            .ends_with(", because cycles couple modules")
    );
    let passing: Box<dyn ArchRule> = Box::new(business_modules());
    passing.check(&items);
}
