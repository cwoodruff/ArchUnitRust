//! `ArchConfiguration` with `archunit.toml` and environment overrides, plus the importer
//! settings it controls.

mod common;

use archunit::config::{ArchConfiguration, environment_variable_name};
use archunit::core::importer::CrateImporter;
use common::fixture_path;

fn config_file() -> std::path::PathBuf {
    fixture_path("config").join("archunit.toml")
}

#[test]
fn reads_archunit_toml_with_flattened_keys() {
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::load_from(config_file());
        let config = ArchConfiguration::get();
        assert!(!config.fail_on_empty_should());
        assert!(config.resolve_missing_dependencies_from_classpath());
        assert_eq!(config.max_number_of_cycles_to_detect(), 7);
        assert_eq!(config.max_number_of_dependencies_per_edge(), 3);
        assert_eq!(
            config.class_resolver_packages(),
            ["fixture_macros", "serde.."]
        );
        assert_eq!(config.include_targets(), ["lib", "bin"]);
        assert_eq!(
            config.property("class_resolver.packages").as_deref(),
            Some("fixture_macros,serde..")
        );
        assert_eq!(
            config
                .property("extension.my_extension.threshold")
                .as_deref(),
            Some("2.5")
        );
        assert!(config.contains_property("extension.my_extension.enabled"));
        assert!(!config.contains_property("extension.other.enabled"));
        let sub = config.sub_properties("extension.my_extension");
        assert_eq!(sub.get("enabled").map(String::as_str), Some("true"));
        assert_eq!(sub.get("threshold").map(String::as_str), Some("2.5"));
        assert_eq!(
            config.property_or_default("missing", "fallback"),
            "fallback"
        );
    });
}

#[test]
fn programmatic_changes_override_and_reset_restores_the_file() {
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::load_from(config_file());
        ArchConfiguration::set_fail_on_empty_should(true);
        assert!(ArchConfiguration::get().fail_on_empty_should());
        ArchConfiguration::set_property("cycles.max_number_to_detect", "1");
        assert_eq!(ArchConfiguration::get().max_number_of_cycles_to_detect(), 1);
        ArchConfiguration::remove_property("cycles.max_number_to_detect");
        assert_eq!(
            ArchConfiguration::get().max_number_of_cycles_to_detect(),
            100
        );
        ArchConfiguration::reset();
        // No archunit.toml in this repository: the defaults apply again.
        let config = ArchConfiguration::get();
        assert!(config.fail_on_empty_should());
        assert!(!config.resolve_missing_dependencies_from_classpath());
        assert!(config.class_resolver_packages().is_empty());
    });
    assert!(ArchConfiguration::get().fail_on_empty_should());
}

#[test]
fn environment_variables_override_properties() {
    assert_eq!(
        environment_variable_name("arch_rule.fail_on_empty_should"),
        "ARCHUNIT_ARCH_RULE_FAIL_ON_EMPTY_SHOULD"
    );
    assert_eq!(
        environment_variable_name("extension.my-ext.enabled"),
        "ARCHUNIT_EXTENSION_MY_EXT_ENABLED"
    );
    // SAFETY: the key is unique to this test; nothing else in the process reads it.
    unsafe { std::env::set_var("ARCHUNIT_CONFIG_TEST_ONLY_KEY", "from env") };
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_property("config_test.only_key", "from code");
        let config = ArchConfiguration::get();
        assert_eq!(
            config.property("config_test.only_key").as_deref(),
            Some("from env")
        );
        assert!(config.contains_property("config_test.only_key"));
        assert_eq!(
            config
                .sub_properties("config_test")
                .get("only_key")
                .map(String::as_str),
            Some("from env")
        );
    });
    unsafe { std::env::remove_var("ARCHUNIT_CONFIG_TEST_ONLY_KEY") };
}

#[test]
fn include_targets_restricts_the_imported_cargo_targets() {
    let all = CrateImporter::new().import_path(fixture_path("reexports_app"));
    assert!(
        all.iter().any(|i| i.crate_name() == "reexports_cli"),
        "bin target imported by default"
    );
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_property("import.include_targets", "lib");
        let libs_only = CrateImporter::new().import_path(fixture_path("reexports_app"));
        assert!(libs_only.iter().all(|i| i.crate_name() != "reexports_cli"));
        assert!(libs_only.iter().any(|i| i.crate_name() == "reexports_app"));
    });
}

#[test]
fn class_resolver_packages_select_the_dependency_crates_to_parse() {
    let stubbed = CrateImporter::new().import_path(fixture_path("layered_app"));
    assert!(!stubbed.contain("fixture_macros::derive_entity"));

    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_resolve_missing_dependencies_from_classpath(true);
        ArchConfiguration::set_property("class_resolver.packages", "nothing_like_this..");
        let none_selected = CrateImporter::new().import_path(fixture_path("layered_app"));
        assert!(!none_selected.contain("fixture_macros::derive_entity"));

        ArchConfiguration::set_property("class_resolver.packages", "fixture_macros");
        let selected = CrateImporter::new().import_path(fixture_path("layered_app"));
        assert!(
            selected.contain("fixture_macros::derive_entity"),
            "{:?}",
            selected.description()
        );
        assert!(
            selected
                .get("fixture_macros::derive_entity")
                .is_fully_imported()
        );

        ArchConfiguration::remove_property("class_resolver.packages");
        let all_dependencies = CrateImporter::new().import_path(fixture_path("layered_app"));
        assert!(all_dependencies.contain("fixture_macros::derive_entity"));
    });
}
