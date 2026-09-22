//! The test harness: `#[analyze_classes]`, `#[arch_test]`, `#[arch_rules]`, `#[arch_ignore]`,
//! `#[arch_tag]`, `ArchTests::in_(..)`, the plain `analyze_classes()` builder and its cache.

mod common;

use std::sync::Arc;

use archunit::core::importer::import_option::DoNotIncludeTests;
use archunit::harness::{
    ArchTests, CacheMode, LocationProvider, analyze_classes, arch_ignore, arch_rules, arch_test,
    cached_imports, clear_cache,
};
use archunit::prelude::*;
use common::fixture_path;

/// Points the tests at the `layered_app` fixture instead of the crate under test.
#[derive(Default)]
struct LayeredApp;

impl LocationProvider for LayeredApp {
    fn get(&self, test_module: &str) -> Vec<Location> {
        assert!(test_module.starts_with("harness"), "{test_module}");
        vec![Location::of(fixture_path("layered_app"))]
    }
}

/// Only the `controller` module of the fixture, through file locations.
#[derive(Default)]
struct ControllerFiles;

impl LocationProvider for ControllerFiles {
    fn get(&self, _test_module: &str) -> Vec<Location> {
        vec![Location::of(
            fixture_path("layered_app").join("src").join("controller"),
        )]
    }
}

// ---- the macros ----------------------------------------------------------------------------

#[analyze_classes(
    locations = [LayeredApp],
    import_options = [DoNotIncludeTests],
    cache_mode = PerClass
)]
mod layered_rules {
    use super::*;

    #[arch_test]
    fn persistence_should_not_access_controllers() -> impl ArchRule {
        no_classes()
            .that()
            .reside_in_a_package("..persistence..")
            .should()
            .access_classes_that()
            .reside_in_a_package("..controller..")
    }

    #[arch_test]
    fn items_are_the_fixture(items: &RustItems) {
        assert!(items.contain("layered_app::controller::SomeController"));
        assert!(!items.contain("archunit::harness::ArchTests"));
    }

    #[arch_test]
    #[arch_ignore(reason = "documented violation of the fixture")]
    fn services_should_not_access_controllers() -> impl ArchRule {
        no_classes()
            .that()
            .reside_in_a_package("..service..")
            .should()
            .access_classes_that()
            .reside_in_a_package("..controller..")
    }

    /// Runs as `layered_rules::controllers_should_reside_in_controller__tag_naming`, so
    /// `cargo test __tag_naming` selects every rule tagged `naming`.
    #[arch_test]
    #[arch_tag("naming")]
    fn controllers_should_reside_in_controller() -> impl ArchRule {
        classes()
            .that()
            .have_simple_name_ending_with("Controller")
            .should()
            .reside_in_a_package("..controller..")
    }

    #[arch_ignore]
    #[arch_test]
    fn ignored_before_arch_test(_items: &RustItems) {
        panic!("must not run");
    }

    #[arch_test]
    fn shared_rules() -> ArchTests {
        ArchTests::in_(passing_rules::arch_tests)
    }

    #[test]
    fn items_provider_is_callable_and_cached_per_module() {
        let first = __archunit_items();
        let second = __archunit_items();
        assert!(Arc::ptr_eq(&first, &second));
        assert!(first.contain("layered_app::service::ServiceOne"));
    }
}

#[analyze_classes(locations = [ControllerFiles], packages = ["..controller.."])]
mod controller_rules {
    use super::*;

    #[arch_test]
    fn only_controller_files_are_imported(items: &RustItems) {
        assert!(items.contain("layered_app::controller::SomeController"));
        assert!(!items.contain("layered_app::service::ServiceOne"));
    }
}

#[arch_rules]
mod passing_rules {
    use super::*;

    #[arch_test]
    fn daos_should_reside_in_persistence() -> impl ArchRule {
        classes()
            .that()
            .have_simple_name_ending_with("Dao")
            .should()
            .reside_in_a_package("..persistence..")
            .allow_empty_should(true)
    }

    #[arch_test]
    fn checks_directly(items: &RustItems) {
        assert!(items.contain("layered_app::core::VeryCentralCore"));
    }

    #[arch_test]
    #[arch_ignore(reason = "known violation")]
    #[arch_tag("layers")]
    fn services_should_not_access_controllers() -> impl ArchRule {
        no_classes()
            .that()
            .reside_in_a_package("..service..")
            .should()
            .access_classes_that()
            .reside_in_a_package("..controller..")
    }

    #[arch_test]
    fn nested() -> ArchTests {
        ArchTests::in_(nested_rules::arch_tests)
    }
}

#[arch_rules]
mod nested_rules {
    use super::*;

    #[arch_test]
    fn no_item_is_named_foo(items: &RustItems) {
        assert!(!items.contain("layered_app::Foo"));
    }
}

#[arch_rules]
mod failing_rules {
    use super::*;

    #[arch_test]
    fn services_should_not_access_controllers() -> impl ArchRule {
        no_classes()
            .that()
            .reside_in_a_package("..service..")
            .should()
            .access_classes_that()
            .reside_in_a_package("..controller..")
    }

    #[arch_test]
    fn passes(_items: &RustItems) {}

    #[arch_test]
    fn panics_itself(_items: &RustItems) {
        panic!("custom check failed");
    }
}

// ---- ArchTests -----------------------------------------------------------------------------

fn layered_items() -> Arc<RustItems> {
    analyze_classes()
        .crate_dir(fixture_path("layered_app"))
        .import_option(DoNotIncludeTests)
        .import()
}

#[test]
fn arch_rules_modules_expose_their_cases() {
    let tests = ArchTests::in_(passing_rules::arch_tests);
    assert_eq!(tests.definition_location(), "harness::passing_rules");
    let names: Vec<&str> = tests.cases().iter().map(|c| c.name()).collect();
    assert_eq!(
        names,
        [
            "daos_should_reside_in_persistence",
            "checks_directly",
            "services_should_not_access_controllers",
            "nested"
        ]
    );
    let ignored = &tests.cases()[2];
    assert_eq!(ignored.ignore_reason(), Some("known violation"));
    assert_eq!(ignored.tags(), ["layers"]);
    assert!(!tests.cases()[0].is_ignored());
    let items = layered_items();
    assert!(tests.evaluate(&items).is_empty());
    tests.check(&items);
}

#[test]
fn arch_tests_collect_every_failure() {
    let items = layered_items();
    let tests = ArchTests::in_(failing_rules::arch_tests);
    let failures = tests.evaluate(&items);
    let names: Vec<&str> = failures.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        ["services_should_not_access_controllers", "panics_itself"]
    );
    assert!(
        failures[0].1.starts_with("Architecture Violation [Priority: MEDIUM] - Rule 'no classes that reside in a package '..service..' should access classes that reside in a package '..controller..'' was violated (2 times):"),
        "{}",
        failures[0].1
    );
    assert_eq!(failures[1].1, "custom check failed");

    let outcome = std::panic::catch_unwind(|| tests.check(&items));
    let message = outcome.unwrap_err();
    let message = message.downcast_ref::<String>().cloned().unwrap();
    assert!(
        message.starts_with("2 of 3 arch tests in `harness::failing_rules` failed:\n"),
        "{message}"
    );
    assert!(
        message.contains("\n[services_should_not_access_controllers]\nArchitecture Violation"),
        "{message}"
    );
    assert!(
        message.contains("\n[panics_itself]\ncustom check failed"),
        "{message}"
    );
}

// ---- the plain API and the cache ---------------------------------------------------------

#[test]
fn analyze_classes_caches_forever_by_configuration() {
    clear_cache();
    let first = analyze_classes()
        .crate_dir(fixture_path("layered_app"))
        .cache_mode(CacheMode::Forever)
        .import();
    let second = analyze_classes()
        .crate_dir(fixture_path("layered_app"))
        .import();
    assert!(Arc::ptr_eq(&first, &second));
    let with_option = analyze_classes()
        .crate_dir(fixture_path("layered_app"))
        .import_option(DoNotIncludeTests)
        .import();
    assert!(!Arc::ptr_eq(&first, &with_option));
    let per_class = analyze_classes()
        .crate_dir(fixture_path("layered_app"))
        .cache_mode(CacheMode::PerClass)
        .import();
    assert!(!Arc::ptr_eq(&first, &per_class));
    assert!(cached_imports() >= 2);
    clear_cache();
    let after_clear = analyze_classes()
        .crate_dir(fixture_path("layered_app"))
        .import();
    assert!(!Arc::ptr_eq(&first, &after_clear));
}

#[test]
fn analyze_classes_restricts_by_packages_items_and_locations() {
    let controllers = analyze_classes()
        .crate_dir(fixture_path("layered_app"))
        .packages(&["..controller.."])
        .import();
    assert!(controllers.contain("layered_app::controller::SomeController"));
    assert!(!controllers.contain("layered_app::service::ServiceOne"));
    assert_eq!(controllers.description(), "classes");

    let of_service = analyze_classes()
        .crate_dir(fixture_path("layered_app"))
        .packages_of(&["layered_app::service::ServiceOne"])
        .import();
    assert!(of_service.contain("layered_app::service::ServiceViolatingLayerRules"));
    assert!(!of_service.contain("layered_app::service::internal::ServiceImpl"));
    assert!(!of_service.contain("layered_app::controller::SomeController"));

    let single = analyze_classes()
        .crate_dir(fixture_path("layered_app"))
        .items(&["layered_app::core::VeryCentralCore"])
        .import();
    assert_eq!(single.len(), 1);

    let from_provider = analyze_classes()
        .for_test_module("harness::plain")
        .locations(|_module: &str| vec![Location::of(fixture_path("onion_app"))])
        .import();
    assert!(from_provider.contain("onion_app::domain::model::Order"));
    assert!(!from_provider.contain("layered_app::core::VeryCentralCore"));

    let whole = analyze_classes()
        .crate_dir(fixture_path("reexports_app").join("app"))
        .whole_workspace(true)
        .import();
    assert!(whole.iter().any(|i| i.crate_name() == "reexports_dep"));
    let member_only = analyze_classes()
        .crate_dir(fixture_path("reexports_app").join("app"))
        .import();
    assert!(
        member_only
            .iter()
            .all(|i| i.crate_name() != "reexports_dep")
    );
    assert!(
        member_only
            .iter()
            .any(|i| i.crate_name() == "reexports_app")
    );
}

// ---- #[arch_test] outside #[analyze_classes]: the crate under test ------------------------

#[arch_test]
fn base_must_not_depend_on_the_library_layer() -> impl ArchRule {
    no_classes()
        .that()
        .reside_in_a_package("archunit::base..")
        .should()
        .depend_on_classes_that()
        .reside_in_a_package("archunit::library..")
}
