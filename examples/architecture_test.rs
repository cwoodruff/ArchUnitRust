//! Port of `ArchUnitExampleJUnit5ArchitectureTest`, `RuleSetsTest` and `RuleLibraryTest`: the
//! test harness with `#[analyze_classes]`, `#[arch_test]`, `#[arch_rules]` and
//! `ArchTests::in_(..)`. Run with `cargo test --example architecture_test`.
//!
//! In a real project the module lives in `tests/architecture.rs` and `#[analyze_classes]`
//! takes no `locations`: the crate of `CARGO_MANIFEST_DIR` is analyzed.
#![allow(dead_code)]

mod common;

use archunit::harness::LocationProvider;
use archunit::prelude::*;

/// Points the tests at the fixture instead of this crate.
#[derive(Default)]
struct LayeredApp;

impl LocationProvider for LayeredApp {
    fn get(&self, _test_module: &str) -> Vec<Location> {
        vec![Location::of(common::fixture_dir("layered_app"))]
    }
}

#[analyze_classes(locations = [LayeredApp], import_options = [DoNotIncludeTests])]
mod architecture {
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
    fn entities_are_derived(items: &RustItems) {
        classes()
            .that()
            .are_annotated_with("entity")
            .should()
            .be_annotated_with("Entity")
            .or_should()
            .reside_in_a_package("..second..")
            .check(items);
    }

    #[arch_test]
    #[arch_ignore(reason = "the fixture violates this on purpose")]
    fn services_should_not_access_controllers() -> impl ArchRule {
        no_classes()
            .that()
            .reside_in_a_package("..service..")
            .should()
            .access_classes_that()
            .reside_in_a_package("..controller..")
    }

    #[arch_test]
    #[arch_tag("library")]
    fn rule_library() -> ArchTests {
        ArchTests::in_(rule_sets::arch_tests)
    }
}

/// `RuleSetsTest`: a reusable set of rules without an import configuration.
#[arch_rules]
mod rule_sets {
    use super::*;

    #[arch_test]
    fn coding_rules() -> ArchTests {
        ArchTests::in_(coding_rules::arch_tests)
    }

    #[arch_test]
    fn daos_should_reside_in_persistence() -> impl ArchRule {
        classes()
            .that()
            .have_simple_name_ending_with("Dao")
            .should()
            .reside_in_a_package("..persistence..")
    }
}

/// `CodingRulesTest` as a rule set.
#[arch_rules]
mod coding_rules {
    use super::*;
    use archunit::library::general_coding_rules::NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS;

    #[arch_test]
    fn no_access_to_standard_streams() -> impl ArchRule {
        NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS.clone()
    }

    #[arch_test]
    fn no_panics_outside_tests(items: &RustItems) {
        archunit::library::general_coding_rules::NO_CLASSES_SHOULD_PANIC.check(items);
    }
}

fn main() {
    println!("This example is a test suite: run `cargo test --example architecture_test`.");
}
