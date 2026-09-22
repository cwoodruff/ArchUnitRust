//! Importer behaviour: crate discovery, module tree, import options, name resolution.

mod common;

use archunit::core::domain::{AccessKind, ItemKind, ResolutionKind, TargetKind};
use archunit::core::importer::import_option::{DoNotIncludeTests, OnlyIncludeTests};
use archunit::core::importer::{CrateImporter, Location};
use common::{fixture_path, import_fixture, names};

#[test]
fn imports_items_and_modules_of_a_crate() {
    let items = import_fixture("layered_app");

    let controller = items.get("layered_app::controller::SomeController");
    assert_eq!(controller.kind(), ItemKind::Struct);
    assert_eq!(controller.simple_name(), "SomeController");
    assert_eq!(controller.package_name(), "layered_app::controller");
    assert_eq!(controller.crate_name(), "layered_app");
    assert_eq!(controller.cargo_target(), TargetKind::Lib);
    assert!(controller.is_fully_imported());
    assert!(!controller.is_test_code());

    assert!(
        items
            .get("layered_app::service::ServiceInterface")
            .is_interface()
    );
    assert!(
        items
            .get("layered_app::thirdparty::free_function_in_thirdparty")
            .is_function()
    );
    assert!(
        items
            .get("layered_app::anticorruption::internal::WrappedResult")
            .is_struct()
    );

    // Modules are packages, not classes.
    assert!(!items.contain("layered_app::controller"));
    assert!(items.contain_package("layered_app::controller"));
    let dao = items.package("layered_app::persistence::first::dao");
    assert_eq!(
        names(dao.subpackages()),
        [
            "layered_app::persistence::first::dao::domain",
            "layered_app::persistence::first::dao::jpa"
        ]
    );
    assert_eq!(
        names(dao.items()),
        ["layered_app::persistence::first::dao::SomeDao"]
    );
    assert_eq!(
        dao.parent().unwrap().name(),
        "layered_app::persistence::first"
    );
    assert_eq!(items.default_packages().len(), 1);
    assert_eq!(items.default_packages()[0].name(), "layered_app");
}

#[test]
fn records_source_locations() {
    let items = import_fixture("layered_app");
    let controller = items.get("layered_app::controller::SomeController");
    assert_eq!(
        controller.source_code_location().to_string(),
        "(src/controller/mod.rs:16)"
    );
    let custom = import_fixture("reexports_app").get("reexports_app::facade::custom::describe");
    assert_eq!(
        custom.source_code_location().source_file_name(),
        "src/facade/custom_path.rs"
    );
}

#[test]
fn imports_every_cargo_target_of_a_workspace() {
    let items = import_fixture("reexports_app");

    let main = items.get("reexports_cli::main");
    assert_eq!(
        main.cargo_target(),
        TargetKind::Bin("reexports_cli".to_owned())
    );
    let integration = items.get("integration::places_an_order");
    assert_eq!(
        integration.cargo_target(),
        TargetKind::Test("integration".to_owned())
    );
    assert!(integration.is_test_code());

    // The dependency crate in the same workspace is fully imported.
    let helper = items.get("reexports_dep::Helper");
    assert!(helper.is_fully_imported());
    assert_eq!(helper.crate_name(), "reexports_dep");
}

#[test]
fn do_not_include_tests_excludes_cfg_test_test_functions_and_test_targets() {
    let all = import_fixture("reexports_app");
    assert!(all.contain("reexports_app::hidden::tests::total_starts_at_zero"));
    assert!(
        all.get("reexports_app::hidden::tests::total_starts_at_zero")
            .is_test_code()
    );
    assert!(all.contain("integration::places_an_order"));

    let production = CrateImporter::new()
        .with_import_option(DoNotIncludeTests)
        .import_path(fixture_path("reexports_app"));
    assert!(!production.contain("reexports_app::hidden::tests::total_starts_at_zero"));
    assert!(!production.contain("integration::places_an_order"));
    assert!(production.contain("reexports_app::domain::model::Order"));
    assert!(production.iter().all(|item| !item.is_test_code()));

    let tests_only = CrateImporter::new()
        .with_import_option(OnlyIncludeTests)
        .import_path(fixture_path("reexports_app"));
    assert!(tests_only.contain("integration::places_an_order"));
    assert!(!tests_only.contain("reexports_app::domain::model::Order"));
}

#[test]
fn custom_import_options_filter_by_location() {
    let without_facade = CrateImporter::new()
        .with_import_option(|location: &Location| !location.contains("/facade/"))
        .import_path(fixture_path("reexports_app"));
    assert!(!without_facade.contain("reexports_app::facade::place"));
    assert!(without_facade.contain("reexports_app::domain::model::Order"));
}

#[test]
fn resolves_re_exports_renames_and_glob_imports() {
    let items = import_fixture("reexports_app");
    let order = items.get("reexports_app::domain::model::Order");

    // `use crate::Order` goes through `pub use domain::model::Order` in lib.rs.
    let place = items.get("reexports_app::facade::place");
    let param = place.code_units()[0].raw_parameter_types()[0]
        .clone()
        .unwrap();
    assert_eq!(param, order);

    // `use super::*` in events.rs finds `DomainOrder`, itself a `pub use .. as` rename.
    let event = items.get("reexports_app::domain::events::OrderPlaced");
    assert_eq!(event.field("order").raw_type().unwrap(), order);
    assert_eq!(
        event.field("status").raw_type().unwrap().name(),
        "reexports_app::domain::model::Status"
    );

    // The binary reaches the library through its extern prelude and a glob re-export.
    let main = items.get("reexports_cli::main");
    let calls = names(
        main.method_calls_from_self()
            .iter()
            .chain(main.constructor_calls_from_self().iter())
            .map(|a| a.target().full_name()),
    );
    assert!(
        calls.contains(&"reexports_app::domain::model::Order::new(OrderId)".to_owned()),
        "{calls:?}"
    );
    assert!(
        calls.contains(&"reexports_app::facade::place(&mut Order)".to_owned()),
        "{calls:?}"
    );

    // A workspace dependency: `use reexports_dep::Helper` and `use reexports_dep::prelude::*`.
    let targets = names(
        place.code_units()[0]
            .calls_from_self()
            .iter()
            .map(|a| a.target().full_name()),
    );
    assert!(
        targets.contains(&"reexports_dep::Helper::help()".to_owned()),
        "{targets:?}"
    );
    assert!(
        targets.contains(&"reexports_dep::helper_fn()".to_owned()),
        "{targets:?}"
    );
}

#[test]
fn creates_stubs_for_items_outside_the_import() {
    let items = import_fixture("reexports_app");
    let counter = items.get("reexports_app::domain::model::ORDER_COUNTER");
    assert_eq!(counter.item_type().unwrap().to_string(), "AtomicU64");
    let stub = items.try_get_any("std::sync::atomic::AtomicU64").unwrap();
    assert!(!stub.is_fully_imported());
    assert_eq!(stub.package_name(), "std::sync::atomic");
    assert!(
        !items.contain("std::sync::atomic::AtomicU64"),
        "stubs are not part of the imported classes"
    );
    let order = items.get("reexports_app::domain::model::Order");
    assert!(
        order.is_equivalent_to("reexports_app::Order"),
        "{:?}",
        order.aliases()
    );
    assert_eq!(items.get("reexports_app::Order"), order);
    assert_eq!(items.get("reexports_app::facade::FacadeOrder"), order);

    // A path dependency outside the workspace is stubbed by crate name.
    let layered = import_fixture("layered_app");
    let entity = layered.get("layered_app::persistence::first::dao::domain::PersistentObject");
    let annotation = entity.annotation_of_type("entity");
    assert_eq!(annotation.raw_type_name(), "fixture_macros::entity");
}

#[test]
fn resolves_method_calls_through_local_variable_types_and_self() {
    let items = import_fixture("layered_app");
    let controller = items.get("layered_app::controller::SomeController");
    let bypass = controller.method("bypass_layers");
    let calls: Vec<_> = bypass.calls_from_self();
    let descriptions = names(calls.iter().map(|c| c.description()));
    assert_eq!(
        descriptions,
        [
            "Method <layered_app::controller::SomeController::bypass_layers()> calls constructor <layered_app::persistence::first::dao::SomeDao::default()> in (src/controller/mod.rs:32)",
            "Method <layered_app::controller::SomeController::bypass_layers()> calls method <layered_app::persistence::first::dao::SomeDao::find_all()> in (src/controller/mod.rs:33)",
        ]
    );
    let find_all = calls.iter().find(|c| c.name() == "find_all").unwrap();
    assert_eq!(find_all.kind(), AccessKind::MethodCall);
    assert_eq!(find_all.resolution(), ResolutionKind::Resolved);
    assert_eq!(
        find_all
            .target()
            .resolve_member()
            .unwrap()
            .owner()
            .simple_name(),
        "SomeDao"
    );

    let do_something = controller.method("do_something");
    let call = &do_something.method_calls_from_self()[0];
    assert_eq!(
        call.target().full_name(),
        "layered_app::service::ServiceOne::serve()"
    );
    assert_eq!(call.origin_owner(), controller);
    assert_eq!(call.target_owner().simple_name(), "ServiceOne");
}

#[test]
fn unresolvable_method_calls_fall_back_to_unique_names_or_stubs() {
    let items = import_fixture("reexports_app");
    let total = items
        .get("reexports_app::domain::model::Order")
        .method("total");
    let calls = names(
        total
            .method_calls_from_self()
            .iter()
            .map(|c| format!("{} {:?}", c.target().full_name(), c.resolution())),
    );
    // `self.items.iter()` is a call on a stub type: the owner is known, the member is not.
    assert!(
        calls.iter().any(|c| c == "std::vec::Vec::iter() OwnerOnly"),
        "{calls:?}"
    );
    // `.map(..)` and `.sum()` on an inferred iterator cannot be typed.
    assert!(
        calls.iter().any(|c| c.starts_with("<unresolved>::sum()")),
        "{calls:?}"
    );
}

#[test]
fn import_packages_restricts_to_matching_modules_of_the_current_workspace() {
    let items = CrateImporter::new()
        .with_import_option(DoNotIncludeTests)
        .import_packages(&["archunit::base.."]);
    assert!(items.contain("archunit::base::described_predicate::DescribedPredicate"));
    assert!(
        items.contain("archunit::base::DescribedPredicate"),
        "public re-export paths are accepted"
    );
    assert!(!items.contain("archunit::core::domain::rust_item::RustItem"));
    assert_eq!(items.description(), "classes");
}
