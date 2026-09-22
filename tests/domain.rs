//! The domain model: members, modifiers, annotations, hierarchy, accesses, dependencies.

mod common;

use archunit::base::HasDescription;
use archunit::core::domain::properties::has_name;
use archunit::core::domain::properties::{CanBeAnnotated, HasModifiers, HasName, ItemSelector};
use archunit::core::domain::{
    AccessKind, AccessType, AnnotationKind, AnnotationValue, DependencyKind, MemberKind, Receiver,
    RustModifier, Visibility, rust_item,
};
use common::{import_fixture, names};

#[test]
fn members_of_a_struct() {
    let items = import_fixture("layered_app");
    let controller = items.get("layered_app::controller::SomeController");

    assert_eq!(
        names(controller.fields()),
        ["layered_app::controller::SomeController::service"]
    );
    let field = controller.field("service");
    assert_eq!(
        field.raw_type().unwrap().name(),
        "layered_app::service::ServiceOne"
    );
    assert_eq!(field.visibility(), Visibility::Private);
    assert_eq!(
        field.description(),
        "Field <layered_app::controller::SomeController::service>"
    );

    assert_eq!(
        names(controller.methods().iter().map(|m| m.name())),
        ["bypass_layers", "default", "do_something", "new"]
    );
    assert_eq!(
        names(controller.constructors().iter().map(|m| m.name())),
        ["default", "new"]
    );
    assert_eq!(controller.code_units().len(), 4);

    let new = controller.method("new");
    assert_eq!(
        new.full_name(),
        "layered_app::controller::SomeController::new()"
    );
    assert_eq!(
        new.description(),
        "Constructor <layered_app::controller::SomeController::new()>"
    );
    assert!(new.has_modifier(&RustModifier::Pub));
    assert!(new.has_modifier(&RustModifier::Static));
    assert!(new.is_constructor());
    assert_eq!(new.receiver(), None);

    let do_something = controller.method("do_something");
    assert_eq!(do_something.receiver(), Some(Receiver::Ref));
    assert!(do_something.is_method());
    assert!(
        do_something.has_modifier(&RustModifier::Final),
        "inherent methods cannot be overridden"
    );
    assert_eq!(do_something.owner(), controller);

    let default = controller.method("default");
    assert_eq!(
        default.implemented_trait().unwrap().name(),
        "std::default::Default"
    );
    assert_eq!(
        default.declaring_impl().unwrap().simple_name(),
        "<impl Default for SomeController>"
    );
}

#[test]
fn free_functions_are_items_and_code_units() {
    let items = import_fixture("layered_app");
    let function = items.get("layered_app::thirdparty::free_function_in_thirdparty");
    assert!(function.is_function());
    let code_units = function.code_units();
    assert_eq!(code_units.len(), 1);
    let unit = &code_units[0];
    assert_eq!(
        unit.full_name(),
        "layered_app::thirdparty::free_function_in_thirdparty()"
    );
    assert_eq!(
        unit.description(),
        "Function <layered_app::thirdparty::free_function_in_thirdparty()>"
    );
    assert_eq!(unit.owner(), function);
    assert!(unit.is_free_function());
    assert_eq!(
        unit.raw_return_type().unwrap().simple_name(),
        "EntityManager"
    );
}

#[test]
fn enums_variants_aliases_consts_and_statics() {
    let items = import_fixture("reexports_app");
    let status = items.get("reexports_app::domain::model::Status");
    assert!(status.is_enum());
    assert_eq!(
        names(status.variants().iter().map(|v| v.name())),
        ["Closed", "Open"]
    );
    assert_eq!(
        status.variant("Closed").unwrap().description(),
        "Variant <reexports_app::domain::model::Status::Closed>"
    );
    assert_eq!(
        names(status.fields().iter().map(|f| f.name())),
        ["Closed::at"]
    );

    let alias = items.get("reexports_app::domain::model::OrderId");
    assert!(alias.is_type_alias());
    let target = alias.item_type().unwrap();
    assert_eq!(target.to_string(), "u64");

    let order = items.get("reexports_app::domain::model::Order");
    assert!(order.field("id").raw_type().unwrap().is_primitive());
    assert_eq!(order.field("status").visibility(), Visibility::PubCrate);
    assert_eq!(
        order.field("items").type_().unwrap().to_string(),
        "Vec<LineItem>"
    );

    let max = items.get("reexports_app::domain::model::MAX_ITEMS");
    assert!(max.is_const());
    assert!(max.has_modifier(&RustModifier::Static));
    let counter = items.get("reexports_app::domain::model::ORDER_COUNTER");
    assert!(counter.is_static());
}

#[test]
fn modifiers_and_visibility() {
    let items = import_fixture("reexports_app");
    let hidden_helper = items.get("reexports_app::hidden::hidden_helper");
    assert_eq!(hidden_helper.visibility(), Visibility::PubCrate);
    assert!(hidden_helper.has_modifier(&RustModifier::PubRestricted));
    assert!(hidden_helper.has_modifier(&RustModifier::PubCrate));
    assert!(!hidden_helper.has_modifier(&RustModifier::Pub));

    let hidden = items.get("reexports_app::hidden::Hidden");
    assert_eq!(hidden.visibility(), Visibility::PubSuper);

    assert!(
        items
            .get("reexports_app::facade::danger")
            .has_modifier(&RustModifier::Unsafe)
    );
    assert!(
        items
            .get("reexports_app::facade::later")
            .has_modifier(&RustModifier::Async)
    );
    assert!(
        items
            .get("reexports_app::facade::limit")
            .has_modifier(&RustModifier::Const)
    );

    let order = items.get("reexports_app::domain::model::Order");
    assert_eq!(order.method("is_open").visibility(), Visibility::Private);
    assert!(order.method("is_open").has_modifier(&RustModifier::Private));
    assert!(order.method("new").has_modifier(&RustModifier::Static));
    assert!(!order.method("add").has_modifier(&RustModifier::Static));

    let trait_ = items.get("reexports_app::domain::events::Event");
    assert!(trait_.has_modifier(&RustModifier::Abstract));
    assert!(trait_.method("name").has_modifier(&RustModifier::Abstract));
    assert!(trait_.method("name").is_trait_declaration());
    assert!(!trait_.method("name").has_body());
}

#[test]
fn annotations_from_attributes_and_derives() {
    let items = import_fixture("layered_app");
    let entity = items.get("layered_app::persistence::first::dao::domain::PersistentObject");
    let annotations = entity.annotations();
    assert_eq!(
        names(annotations.iter().map(|a| a.path())),
        ["Clone", "Debug", "Entity", "entity"]
    );
    assert_eq!(
        entity.annotation_of_type("entity").kind(),
        AnnotationKind::Attribute
    );
    assert_eq!(
        entity.annotation_of_type("Debug").kind(),
        AnnotationKind::Derive
    );
    assert_eq!(
        entity.annotation_of_type("Debug").raw_type_name(),
        "std::fmt::Debug"
    );
    assert_eq!(
        entity.annotation_of_type("Entity").raw_type_name(),
        "fixture_macros::Entity"
    );
    assert!(entity.is_annotated_with("entity"));
    assert!(entity.is_annotated_with("@entity"));
    assert!(entity.is_annotated_with("fixture_macros::entity"));
    assert!(!entity.is_annotated_with("secured"));
    // Derives count as implemented traits.
    assert!(entity.implements("std::fmt::Debug"));
    assert!(entity.is_assignable_to("Clone"));

    let onion = import_fixture("onion_app");
    let cli = onion.get("onion_app::adapter::cli::AdministrationCli");
    let adapter = cli.annotation_of_type("adapter");
    assert_eq!(
        adapter.get("value"),
        Some(&AnnotationValue::Str("cli".to_owned()))
    );
    assert_eq!(adapter.tokens(), "(\"cli\")");

    let reexports = import_fixture("reexports_app");
    let old = reexports.get("reexports_app::hidden::old");
    let deprecated = old.annotation_of_type("deprecated");
    assert_eq!(
        deprecated.get("note"),
        Some(&AnnotationValue::Str("use hidden_helper".to_owned()))
    );
    let secured = items
        .get("layered_app::service::ServiceOne")
        .method("serve");
    assert!(secured.is_annotated_with("secured"));
    let cfg_test = reexports.get("reexports_app::hidden::tests::total_starts_at_zero");
    assert!(cfg_test.is_annotated_with("test"));
}

#[test]
fn trait_hierarchy_and_assignability() {
    let items = import_fixture("layered_app");
    let service_one = items.get("layered_app::service::ServiceOne");
    let interface = items.get("layered_app::service::ServiceInterface");
    assert!(service_one.implemented_traits().contains(&interface));
    assert!(service_one.implements("ServiceInterface"));
    assert!(service_one.is_assignable_to("layered_app::service::ServiceInterface"));
    assert!(service_one.is_assignable_to(rust_item::predicates::simple_name("ServiceOne")));
    assert!(!service_one.is_assignable_to("AbstractController"));
    assert!(interface.is_assignable_from("ServiceOne"));
    assert_eq!(
        names(interface.implementors().iter().map(|i| i.simple_name())),
        ["DaoCallingService", "ServiceImpl", "ServiceOne"]
    );

    let cyclic = import_fixture("cyclic_app");
    let two = cyclic.get("cyclic_app::inheritancecycle::slice2::TraitInSliceTwo");
    assert_eq!(
        names(two.supertraits()),
        ["cyclic_app::inheritancecycle::slice3::TraitInSliceThree"]
    );
    assert_eq!(
        names(two.all_supertraits()),
        [
            "cyclic_app::inheritancecycle::slice1::TraitInSliceOne",
            "cyclic_app::inheritancecycle::slice3::TraitInSliceThree"
        ]
    );
    let class = cyclic
        .get("cyclic_app::inheritancecycle::slice1::ClassInSliceOneImplementingTraitOfSliceTwo");
    assert!(class.is_assignable_to("TraitInSliceOne"));
    assert!(
        cyclic
            .get("cyclic_app::inheritancecycle::slice1::TraitInSliceOne")
            .all_implementors()
            .contains(&two)
    );
}

#[test]
fn field_accesses_distinguish_get_and_set() {
    let items = import_fixture("cyclic_app");
    let one = items.get("cyclic_app::fieldaccesscycle::slice1::SliceOneAccessingFieldInSliceTwo");
    let accesses = one.method("access").field_accesses();
    let described = names(accesses.iter().map(|a| a.description()));
    assert_eq!(
        described,
        [
            "Method <cyclic_app::fieldaccesscycle::slice1::SliceOneAccessingFieldInSliceTwo::access(&mut SliceTwoAccessingFieldInSliceOne)> gets field <cyclic_app::fieldaccesscycle::slice1::SliceOneAccessingFieldInSliceTwo::field_one> in (src/fieldaccesscycle/slice1.rs:9)",
            "Method <cyclic_app::fieldaccesscycle::slice1::SliceOneAccessingFieldInSliceTwo::access(&mut SliceTwoAccessingFieldInSliceOne)> sets field <cyclic_app::fieldaccesscycle::slice2::SliceTwoAccessingFieldInSliceOne::field_two> in (src/fieldaccesscycle/slice1.rs:9)",
        ]
    );
    let set = accesses
        .iter()
        .find(|a| a.access_type() == Some(AccessType::Set))
        .unwrap();
    assert_eq!(set.kind(), AccessKind::FieldAccess(AccessType::Set));
    assert_eq!(set.target().resolve_member().unwrap().name(), "field_two");
    let two = items.get("cyclic_app::fieldaccesscycle::slice2::SliceTwoAccessingFieldInSliceOne");
    assert_eq!(two.field_accesses_to_self().len(), 1);
    assert_eq!(two.field("field_two").accesses_to_self().len(), 1);

    let layered = import_fixture("layered_app");
    let service = layered.get("layered_app::service::ServiceViolatingLayerRules");
    let sets = service.method("illegal_access_to_core").field_accesses();
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].access_type(), Some(AccessType::Set));
    assert_eq!(sets[0].target_owner(), service);
}

#[test]
fn constructor_calls_and_accesses_to_self() {
    let items = import_fixture("layered_app");
    let core = items.get("layered_app::core::VeryCentralCore");
    let to_self = core.accesses_to_self();
    let origins = names(to_self.iter().map(|a| a.origin_owner().simple_name()));
    assert_eq!(
        origins,
        [
            "CoreSatelliteController",
            "CoreSatelliteController",
            "ServiceViolatingLayerRules",
            "ServiceViolatingLayerRules"
        ]
    );
    assert_eq!(core.method_calls_to_self().len(), 2);
    assert_eq!(core.constructor_calls_to_self().len(), 2);
    assert_eq!(core.method("do_core_stuff").calls_of_self().len(), 2);

    let reexports = import_fixture("reexports_app");
    let order = reexports.get("reexports_app::domain::model::Order");
    let literal = order
        .constructor_calls_to_self()
        .into_iter()
        .find(|a| a.target().name() == "{ .. }")
        .expect("struct literal in Order::new");
    assert_eq!(
        literal.target().full_name(),
        "reexports_app::domain::model::Order { .. }"
    );
    assert_eq!(literal.origin().name(), "new");

    let line_item = reexports.get("reexports_app::domain::model::LineItem");
    let tuple = line_item.constructor_calls_to_self();
    assert!(
        tuple
            .iter()
            .any(|a| a.target().full_name() == "reexports_app::domain::model::LineItem(..)"),
        "{tuple:?}"
    );
}

#[test]
fn closures_macros_and_local_items() {
    let items = import_fixture("reexports_app");
    let with_local = items.get("reexports_app::facade::with_local_item");
    let calls = with_local.method_calls_from_self();
    let totals: Vec<_> = calls.iter().filter(|c| c.name() == "total").collect();
    assert_eq!(
        totals.len(),
        2,
        "one call inside println!, one inside the closure: {calls:?}"
    );
    assert!(totals.iter().any(|c| c.is_declared_in_closure()));
    assert!(totals.iter().any(|c| !c.is_declared_in_closure()));
    let deps = names(with_local.direct_dependencies_from_self());
    assert!(
        deps.iter()
            .any(|d| d.contains("invokes macro <std::println>")),
        "{deps:?}"
    );

    let local = items.get("reexports_app::facade::with_local_item::LocalCounter");
    assert!(local.is_local_item());
    assert!(local.is_nested_item());
    assert!(!local.is_top_level_item());
    assert_eq!(local.enclosing_item().unwrap(), with_local);
    assert!(with_local.is_top_level_item());
    assert!(with_local.all_accesses_from_self().len() >= with_local.accesses_from_self().len());
    let constructors = names(
        with_local
            .constructor_calls_from_self()
            .iter()
            .map(|a| a.target().full_name()),
    );
    assert!(
        constructors
            .contains(&"reexports_app::facade::with_local_item::LocalCounter(..)".to_owned()),
        "{constructors:?}"
    );
}

#[test]
fn error_types_from_result_return_types() {
    let items = import_fixture("layered_app");
    let store = items
        .get("layered_app::persistence::first::dao::SomeDao")
        .method("store");
    assert_eq!(
        names(store.error_types()),
        ["layered_app::thirdparty::SqlError"]
    );
    let reexports = import_fixture("reexports_app");
    let try_new = reexports
        .get("reexports_app::domain::model::Order")
        .method("try_new");
    assert_eq!(
        names(try_new.error_types()),
        ["reexports_app::domain::model::OrderError"]
    );
    assert!(
        try_new.is_constructor(),
        "Result<Self, _> counts as a constructor"
    );
    let error = reexports.get("reexports_app::domain::model::OrderError");
    assert_eq!(
        names(
            error
                .functions_with_error_type_of_self()
                .iter()
                .map(|c| c.name())
        ),
        ["checked_add", "place", "try_new"]
    );
}

#[test]
fn dependencies_from_members_and_hierarchy() {
    let items = import_fixture("layered_app");
    let dao = items.get("layered_app::persistence::layerviolation::DaoCallingService");
    let deps = names(dao.direct_dependencies_from_self());
    assert_eq!(
        deps,
        [
            "Field <layered_app::persistence::layerviolation::DaoCallingService::service> has type <layered_app::service::ServiceOne> in (src/persistence/layerviolation.rs:5)",
            "Item <layered_app::persistence::layerviolation::DaoCallingService> implements trait <layered_app::service::ServiceInterface> in (src/persistence/layerviolation.rs:4)",
            "Method <layered_app::persistence::layerviolation::DaoCallingService::serve()> calls method <layered_app::service::ServiceOne::serve()> in (src/persistence/layerviolation.rs:10)",
        ],
        "accesses to own members are not dependencies, like in ArchUnit"
    );
    let own_field_access = dao.method("serve").field_accesses();
    assert_eq!(own_field_access.len(), 1);
    assert_eq!(own_field_access[0].target_owner(), dao);
    let service_one = items.get("layered_app::service::ServiceOne");
    let to_service_one = dao
        .direct_dependencies_from_self()
        .into_iter()
        .filter(|d| d.target_item() == service_one)
        .collect::<Vec<_>>();
    assert_eq!(to_service_one.len(), 2);
    assert_eq!(to_service_one[0].origin_item(), dao);
    assert_eq!(to_service_one[0].kind(), DependencyKind::FieldType);
    assert!(
        service_one
            .direct_dependencies_to_self()
            .iter()
            .any(|d| d.origin_item() == dao)
    );

    let controller = items.get("layered_app::controller::SomeController");
    let transitive = names(
        controller
            .transitive_dependencies_from_self()
            .iter()
            .map(|d| d.target_item().simple_name()),
    );
    assert!(transitive.contains(&"SomeDao".to_owned()));
    assert!(
        transitive.contains(&"EntityManager".to_owned()),
        "{transitive:?}"
    );

    let cyclic = import_fixture("cyclic_app");
    let two = cyclic.get("cyclic_app::inheritancecycle::slice2::TraitInSliceTwo");
    assert_eq!(
        names(two.direct_dependencies_from_self()),
        [
            "Item <cyclic_app::inheritancecycle::slice2::TraitInSliceTwo> extends trait <cyclic_app::inheritancecycle::slice3::TraitInSliceThree> in (src/inheritancecycle/slice2.rs:3)"
        ]
    );
    let four = cyclic.get("cyclic_app::membercycle::slice4::SliceFourWithErrorTypeOfSliceOne");
    let error_deps = names(four.direct_dependencies_from_self());
    assert!(
        error_deps.iter().any(|d| d.contains(
            "throws type <cyclic_app::membercycle::slice1::SliceOneWithFieldTypeInSliceTwo>"
        )),
        "{error_deps:?}"
    );
    assert!(error_deps.iter().any(|d| d.contains("has generic return type <std::result::Result> with type argument depending on <cyclic_app::membercycle::slice1::SliceOneWithFieldTypeInSliceTwo>")), "{error_deps:?}");
}

#[test]
fn dependencies_from_imports_annotations_and_references() {
    let items = import_fixture("reexports_app");
    let facade = items.package("reexports_app::facade");
    let imports = names(
        facade
            .as_item()
            .direct_dependencies_from_self()
            .iter()
            .filter(|d| d.kind() == DependencyKind::Import),
    );
    assert!(imports.contains(&"Module <reexports_app::facade> imports <reexports_dep::Helper> in (src/facade/mod.rs:6)".to_owned()), "{imports:?}");

    let limit = items.get("reexports_app::facade::limit");
    let refs = names(limit.direct_dependencies_from_self());
    assert_eq!(
        refs,
        [
            "Function <reexports_app::facade::limit()> references <reexports_app::domain::model::MAX_ITEMS> in (src/facade/mod.rs:27)"
        ]
    );

    let layered = import_fixture("layered_app");
    let entity = layered.get("layered_app::persistence::first::dao::domain::PersistentObject");
    let annotation_deps = names(
        entity
            .direct_dependencies_from_self()
            .iter()
            .filter(|d| d.kind() == DependencyKind::Annotation),
    );
    assert!(
        annotation_deps
            .iter()
            .any(|d| d.contains("is annotated with <fixture_macros::entity>")),
        "{annotation_deps:?}"
    );
    assert!(
        annotation_deps
            .iter()
            .any(|d| d.contains("is annotated with <std::fmt::Debug>")),
        "{annotation_deps:?}"
    );

    let uses_old = items.get("reexports_app::hidden::uses_old");
    let call = &uses_old.method_calls_from_self()[0];
    assert!(call.target_owner().is_annotated_with("deprecated"));
}

#[test]
fn module_api() {
    let items = import_fixture("layered_app");
    let persistence = items.package("layered_app::persistence");
    assert_eq!(persistence.relative_name(), "persistence");
    assert_eq!(
        names(persistence.subpackages().iter().map(|m| m.relative_name())),
        ["first", "layerviolation", "second"]
    );
    assert!(persistence.contains_package("first::dao"));
    assert!(
        persistence
            .items_in_package_tree()
            .iter()
            .any(|i| i.simple_name() == "SomeDao")
    );
    assert!(
        persistence
            .package("first::dao")
            .contains_item_with_simple_name("SomeDao")
    );
    let package_deps = names(persistence.package_dependencies_from_this_package_tree());
    assert!(
        package_deps.contains(&"layered_app::service".to_owned()),
        "{package_deps:?}"
    );
    assert!(
        package_deps.contains(&"layered_app::thirdparty".to_owned()),
        "{package_deps:?}"
    );
    let incoming = names(persistence.package_dependencies_to_this_package_tree());
    assert!(
        incoming.contains(&"layered_app::controller".to_owned()),
        "{incoming:?}"
    );
    assert!(
        incoming.contains(&"layered_app::service".to_owned()),
        "{incoming:?}"
    );

    let mut visited = Vec::new();
    persistence.traverse_package_tree(&archunit::base::always_true(), &mut |m| {
        visited.push(m.relative_name())
    });
    assert_eq!(visited.len(), 7);
    assert_eq!(
        persistence.description(),
        "Module <layered_app::persistence>"
    );
}

#[test]
fn rust_items_that_and_predicates() {
    let items = import_fixture("layered_app");
    let controllers = items.that(&rust_item::predicates::reside_in_a_package(
        "..controller..",
    ));
    assert_eq!(
        controllers.description(),
        "classes that reside in a package '..controller..'"
    );
    assert_eq!(
        names(controllers.iter().map(|i| i.simple_name())),
        [
            "AbstractController",
            "CoreSatelliteController",
            "Marshaller",
            "SomeController",
            "SomeGuiController",
            "UseCaseOneTwoController",
            "UseCaseThreeController",
            "UseCaseTwoController",
            "WronglyNamed"
        ]
    );
    assert!(controllers.contain("layered_app::controller::SomeController"));
    assert!(!controllers.contain("layered_app::service::ServiceOne"));
    assert!(
        items
            .that(&rust_item::predicates::interfaces())
            .iter()
            .all(|i| i.is_trait())
    );
    assert_eq!(
        rust_item::predicates::simple_name_ending_with("Dao").description(),
        "simple name ending with 'Dao'"
    );
    assert_eq!(
        rust_item::predicates::reside_in_any_package(&["..a..", "..b.."]).description(),
        "reside in any package ['..a..', '..b..']"
    );
    assert_eq!(
        rust_item::predicates::assignable_to("ServiceInterface").description(),
        "assignable to ServiceInterface"
    );
    assert_eq!(
        rust_item::predicates::implement(rust_item::predicates::simple_name("X")).description(),
        "implement simple name 'X'"
    );
    let named: archunit::base::DescribedPredicate<archunit::core::domain::RustItem> =
        has_name::predicates::name_matching(".*Dao");
    assert_eq!(named.description(), "name matching '.*Dao'");
    assert_eq!(
        names(items.that(&named).iter().map(|i| i.simple_name())),
        ["InWrongPackageDao", "JpaDao", "SomeDao"]
    );
    let selector = ItemSelector::from("SomeDao");
    assert!(selector.matches(&items.get("layered_app::persistence::first::dao::SomeDao")));
    assert_eq!(
        rust_item::predicates::belong_to_any_of(&["reexports_app::facade::with_local_item"])
            .description(),
        "belong to any of [reexports_app::facade::with_local_item]"
    );
    assert!(
        items
            .get("layered_app::controller::SomeController")
            .description()
            == "Item <layered_app::controller::SomeController>"
    );
    assert_eq!(
        items.get("layered_app::controller::SomeController").name(),
        "layered_app::controller::SomeController"
    );
}

#[test]
fn impl_blocks_for_foreign_types_are_items_of_their_own() {
    let items = import_fixture("reexports_app");
    let impl_for_string = items
        .iter()
        .find(|i| i.is_impl() && i.simple_name() == "<impl From<Order> for String>")
        .expect("impl From<Order> for String is a class of its own");
    assert_eq!(
        impl_for_string.impl_trait().unwrap().name(),
        "std::convert::From"
    );
    assert_eq!(impl_for_string.methods().len(), 1);
    assert!(
        items
            .iter()
            .all(|i| !(i.is_impl() && i.simple_name() == "<impl Display for Order>")),
        "impls for imported types attach to the type"
    );
    let order = items.get("reexports_app::domain::model::Order");
    assert!(order.try_method("fmt").is_some());
    assert!(order.implements("std::fmt::Display"));
    assert_eq!(order.method("fmt").kind(), MemberKind::Method);
}
