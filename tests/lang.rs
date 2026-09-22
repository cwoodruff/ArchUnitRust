//! The Lang API: rule texts, evaluation, reports. Ports of `ArchRuleTest`,
//! `EvaluationResultTest`, `CompositeArchRuleTest` and the shape of `ClassesShouldTest`.

mod common;

use std::sync::{Arc, Mutex};

use archunit::base::{
    DescribedPredicate, HasDescription, always_false, always_true, less_than_or_equal_to,
};
use archunit::config::ArchConfiguration;
use archunit::core::domain::properties::HasName;
use archunit::core::domain::{RustItem, RustItems, RustModule, rust_item, rust_member};
use archunit::lang::conditions as cond;
use archunit::lang::syntax::*;
use archunit::lang::{
    ArchCondition, ArchRule, ClassesTransformer, CompositeArchRule, ConditionEvents,
    ConditionLogic, EvaluationResult, Priority, SimpleArchRule, SimpleConditionEvent,
};
use common::import_fixture;
use regex::Regex;

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

fn report(rule: &dyn ArchRule, items: &RustItems) -> String {
    rule.evaluate(items).failure_report().to_string()
}

// ---- rule texts ------------------------------------------------------------------------------

#[test]
fn rule_texts_are_assembled_like_archunit() {
    assert_eq!(
        no_classes()
            .that()
            .reside_in_a_package("..service..")
            .should()
            .access_classes_that()
            .reside_in_a_package("..controller..")
            .description(),
        "no classes that reside in a package '..service..' should access classes that reside in a package '..controller..'"
    );
    assert_eq!(
        no_classes()
            .that()
            .reside_in_a_package("..service..")
            .or()
            .reside_in_a_package("..persistence..")
            .should()
            .access_classes_that()
            .reside_in_a_package("..controller..")
            .or_should()
            .access_classes_that()
            .reside_in_a_package("..ui..")
            .description(),
        "no classes that reside in a package '..service..' or reside in a package '..persistence..' should access classes that reside in a package '..controller..' or should access classes that reside in a package '..ui..'"
    );
    assert_eq!(
        classes()
            .that()
            .reside_in_a_package("..service..")
            .should()
            .only_be_accessed()
            .by_any_package(&["..controller..", "..service.."])
            .description(),
        "classes that reside in a package '..service..' should only be accessed by any package ['..controller..', '..service..']"
    );
    assert_eq!(
        methods()
            .that()
            .are_public()
            .and()
            .are_declared_in_classes_that()
            .reside_in_a_package("..controller..")
            .should()
            .be_annotated_with("Secured")
            .description(),
        "methods that are public and are declared in classes that reside in a package '..controller..' should be annotated with @Secured"
    );
    assert_eq!(
        classes()
            .that()
            .implement("Connection")
            .should()
            .have_simple_name_ending_with("Connection")
            .description(),
        "classes that implement Connection should have simple name ending with 'Connection'"
    );
    assert_eq!(
        classes()
            .that()
            .have_name_matching(".*Bar")
            .should()
            .only_have_dependent_classes_that()
            .have_simple_name("Bar")
            .description(),
        "classes that have name matching '.*Bar' should only have dependent classes that have simple name 'Bar'"
    );
    assert_eq!(
        classes()
            .that()
            .are_assignable_to("EntityManager")
            .should()
            .only_have_dependent_classes_that()
            .are_annotated_with("transactional")
            .description(),
        "classes that are assignable to EntityManager should only have dependent classes that are annotated with @transactional"
    );
    assert_eq!(
        the_class("a::B")
            .should()
            .only_be_accessed()
            .by_classes_that()
            .implement("a::C")
            .description(),
        "the class a::B should only be accessed by classes that implement a::C"
    );
    assert_eq!(
        no_class("a::B")
            .should()
            .access_classes_that()
            .reside_outside_of_packages(&["..core..", "std.."])
            .description(),
        "no class a::B should access classes that reside outside of packages ['..core..', 'std..']"
    );
    assert_eq!(
        classes()
            .that()
            .are_annotated_with("HighSecurity")
            .should()
            .be("a::Core")
            .description(),
        "classes that are annotated with @HighSecurity should be a::Core"
    );
    assert_eq!(
        no_methods()
            .that()
            .are_declared_in_classes_that()
            .have_name_matching(".*Dao")
            .should()
            .declare_throwable_of_type("SqlError")
            .description(),
        "no methods that are declared in classes that have name matching '.*Dao' should declare throwable of type SqlError"
    );
    assert_eq!(
        fields()
            .that()
            .have_raw_type("Logger")
            .should()
            .be_private()
            .and_should()
            .be_static()
            .and_should()
            .be_final()
            .description(),
        "fields that have raw type Logger should be private and should be static and should be final"
    );
    assert_eq!(
        classes()
            .that()
            .implement("X")
            .should()
            .contain_number_of_elements(less_than_or_equal_to(1))
            .description(),
        "classes that implement X should contain number of elements less than or equal to '1'"
    );
    assert_eq!(
        classes()
            .that()
            .reside_in_a_package("..controller..")
            .should()
            .only_call_methods_that(rust_member::predicates::declared_in("a::B"))
            .description(),
        "classes that reside in a package '..controller..' should only call methods that declared in a::B"
    );
    assert_eq!(
        no_classes()
            .that()
            .are_interfaces()
            .should()
            .have_name_matching(".*Interface")
            .description(),
        "no classes that are interfaces should have name matching '.*Interface'"
    );
    assert_eq!(
        classes()
            .that()
            .have_simple_name_starting_with("Foo")
            .should()
            .reside_in_a_package("com::foo")
            .description(),
        "classes that have simple name starting with 'Foo' should reside in a package 'com::foo'"
    );
    assert_eq!(
        classes()
            .should()
            .call_method("Order", "new", &["u64", "a::Item"])
            .description(),
        "classes should call method Order::new(u64, Item)"
    );
    assert_eq!(
        classes()
            .should()
            .access_field("a::Order", "items")
            .description(),
        "classes should access field Order::items"
    );
    assert_eq!(
        no_classes()
            .should()
            .depend_on_classes_that_with(rust_item::predicates::assignable_to("EntityManager"))
            .description(),
        "no classes should depend on classes that assignable to EntityManager"
    );
}

#[test]
fn because_and_as_override_the_description() {
    let always_be_violated = ArchCondition::new(
        "always be violated",
        |item: &RustItem, events: &mut ConditionEvents| {
            events.add(SimpleConditionEvent::violated(item, "violated"));
        },
    );
    let rule = classes()
        .should_with(always_be_violated.clone())
        .because("this is the way");
    assert_eq!(
        rule.description(),
        "classes should always be violated, because this is the way"
    );
    let rule = classes()
        .should()
        .access_classes_that()
        .have_fully_qualified_name("foo")
        .because("this is the way");
    assert_eq!(
        rule.description(),
        "classes should access classes that have fully qualified name 'foo', because this is the way"
    );

    let overridden = classes()
        .should_with(always_be_violated.clone())
        .as_("rule text overridden");
    assert_eq!(overridden.description(), "rule text overridden");
    let failures = overridden.evaluate(&layered()).failure_report().to_string();
    assert!(failures.contains("rule text overridden"));

    // The same operations on a trait object.
    let boxed: Box<dyn ArchRule> = Box::new(classes().should_with(always_be_violated));
    assert_eq!(
        boxed.because("reasons").description(),
        "classes should always be violated, because reasons"
    );
    assert_eq!(boxed.as_("other").description(), "other");
    assert!(boxed.as_("other").evaluate(&layered()).has_violation());
}

#[test]
fn priority_is_passed_to_the_report() {
    let always_be_violated = ArchCondition::new(
        "always be violated",
        |item: &RustItem, events: &mut ConditionEvents| {
            events.add(SimpleConditionEvent::violated(item, "violated"));
        },
    );
    let result = priority(Priority::High)
        .classes()
        .should_with(always_be_violated)
        .evaluate(&layered());
    assert_eq!(result.priority(), Priority::High);
    assert!(result.failure_report().to_string().starts_with(
        "Architecture Violation [Priority: HIGH] - Rule 'classes should always be violated'"
    ));
}

// ---- evaluation against the fixtures ---------------------------------------------------------

#[test]
fn services_should_not_access_controllers() {
    let rule = no_classes()
        .that()
        .reside_in_a_package("..service..")
        .should()
        .access_classes_that()
        .reside_in_a_package("..controller..");
    assert_eq!(
        report(&rule, &layered()),
        expected("services_should_not_access_controllers")
    );
}

#[test]
fn persistence_should_not_depend_on_services() {
    let rule = no_classes()
        .that()
        .reside_in_a_package("..persistence..")
        .should()
        .depend_on_classes_that()
        .reside_in_a_package("..service..");
    assert_eq!(
        report(&rule, &layered()),
        expected("persistence_should_not_depend_on_services")
    );
}

#[test]
fn and_should_and_or_should_join_conditions() {
    let rule = priority(Priority::High)
        .classes()
        .that()
        .reside_in_a_package("..controller..")
        .should()
        .be_public()
        .and_should()
        .have_simple_name_ending_with("Controller")
        .or_should()
        .be_interfaces();
    assert_eq!(
        report(&rule, &layered()),
        expected("controllers_and_should_or_should")
    );
}

#[test]
fn the_class_and_because_in_report() {
    let rule = the_class("layered_app::core::VeryCentralCore")
        .should()
        .only_be_accessed()
        .by_classes_that()
        .implement("CoreSatellite")
        .because("satellites are the only supported clients");
    assert_eq!(
        report(&rule, &layered()),
        expected("core_only_accessed_by_satellites")
    );
}

#[test]
fn check_panics_with_the_report_and_passes_otherwise() {
    let items = layered();
    let failing = std::panic::catch_unwind(|| {
        no_classes()
            .that()
            .reside_in_a_package("..service..")
            .should()
            .access_classes_that()
            .reside_in_a_package("..controller..")
            .check(&items);
    });
    let message = failing.unwrap_err();
    let message = message.downcast_ref::<String>().expect("panic message");
    assert_eq!(
        message.trim_end(),
        expected("services_should_not_access_controllers")
    );

    classes()
        .that()
        .have_simple_name_containing("Controller")
        .should()
        .reside_in_a_package("..controller..")
        .check(&items);
    classes()
        .that()
        .are_annotated_with("high_security")
        .should()
        .be("layered_app::core::VeryCentralCore")
        .check(&items);
    no_class("layered_app::core::VeryCentralCore")
        .should()
        .access_classes_that()
        .reside_outside_of_packages(&["..core..", "std.."])
        .check(&items);
}

#[test]
fn class_conditions_report_like_archunit() {
    let items = layered();
    let lines = |rule: &dyn ArchRule| rule.evaluate(&items).failure_report().details();

    assert_eq!(
        lines(
            &classes()
                .that()
                .reside_in_a_package("..controller..")
                .should()
                .have_simple_name_not_containing("Gui")
        ),
        [
            "Item <layered_app::controller::SomeGuiController> has simple name containing 'Gui' in (src/controller/mod.rs:44)"
        ]
    );
    assert_eq!(
        lines(
            &classes()
                .that()
                .implement("AbstractController")
                .or()
                .are_annotated_with("my_controller")
                .should()
                .have_simple_name_ending_with("Controller")
        ),
        [
            "Item <layered_app::controller::WronglyNamed> does not have simple name ending with 'Controller' in (src/controller/mod.rs:51)"
        ]
    );
    assert_eq!(
        lines(
            &classes()
                .that()
                .reside_in_a_package("..service..")
                .and()
                .are_annotated_with("my_service")
                .should()
                .have_simple_name_starting_with("Service")
        ),
        [
            "Item <layered_app::service::BadlyNamedService> does not have simple name starting with 'Service' in (src/service/mod.rs:50)"
        ]
    );
    assert_eq!(
        lines(
            &no_classes()
                .that()
                .reside_in_a_package("..internal..")
                .should()
                .be_interfaces()
        ),
        [
            "Item <layered_app::service::internal::SomeInternalInterface> is an interface in (src/service/internal.rs:4)"
        ]
    );
    assert_eq!(
        lines(
            &classes()
                .that()
                .reside_in_a_package("..service..")
                .should()
                .only_be_accessed()
                .by_any_package(&["..controller..", "..service.."])
        ),
        [
            "Method <layered_app::persistence::layerviolation::DaoCallingService::serve()> calls method <layered_app::service::ServiceOne::serve()> in (src/persistence/layerviolation.rs:10)"
        ]
    );
    assert_eq!(
        lines(
            &classes()
                .that()
                .reside_in_a_package("..controller..")
                .should()
                .only_depend_on_classes_that()
                .reside_in_any_package(&[
                    "..controller..",
                    "..service..",
                    "..core..",
                    "std..",
                    "fixture_macros.."
                ])
        ),
        [
            "Method <layered_app::controller::SomeController::bypass_layers()> calls constructor <layered_app::persistence::first::dao::SomeDao::default()> in (src/controller/mod.rs:32)",
            "Method <layered_app::controller::SomeController::bypass_layers()> calls method <layered_app::persistence::first::dao::SomeDao::find_all()> in (src/controller/mod.rs:33)",
        ]
    );
    assert_eq!(
        lines(
            &classes()
                .that()
                .implement("ServiceInterface")
                .should()
                .contain_number_of_elements(less_than_or_equal_to(1))
        ),
        [
            "there is/are 3 element(s) in [layered_app::persistence::layerviolation::DaoCallingService, layered_app::service::ServiceOne, layered_app::service::internal::ServiceImpl]"
        ]
    );
    assert_eq!(
        lines(
            &no_classes()
                .should()
                .call_method("VeryCentralCore", "do_core_stuff", &[])
        ),
        [
            "Method <layered_app::controller::CoreSatelliteController::call_core()> calls method <layered_app::core::VeryCentralCore::do_core_stuff()> in (src/controller/mod.rs:63)",
            "Method <layered_app::service::ServiceViolatingLayerRules::illegal_access_to_core()> calls method <layered_app::core::VeryCentralCore::do_core_stuff()> in (src/service/mod.rs:63)",
        ]
    );
    assert!(
        lines(
            &classes()
                .should()
                .set_field("ServiceViolatingLayerRules", "counter")
        )
        .iter()
        .all(|line| !line.contains("ServiceViolatingLayerRules"))
    );
    assert!(
        lines(
            &classes()
                .that()
                .have_simple_name("SomeController")
                .should()
                .transitively_depend_on_classes_that()
                .have_simple_name("EntityManager")
        )
        .is_empty()
    );
    assert_eq!(
        lines(
            &classes()
                .that()
                .reside_in_a_package("..core..")
                .should()
                .only_have_dependent_classes_that()
                .reside_in_any_package(&["..core..", "..controller.."])
        ),
        [
            "Method <layered_app::service::ServiceViolatingLayerRules::illegal_access_to_core()> calls constructor <layered_app::core::VeryCentralCore> in (src/service/mod.rs:63)",
            "Method <layered_app::service::ServiceViolatingLayerRules::illegal_access_to_core()> calls method <layered_app::core::VeryCentralCore::do_core_stuff()> in (src/service/mod.rs:63)",
            "Module <layered_app::service> imports <layered_app::core::VeryCentralCore> in (src/service/mod.rs:4)",
        ]
    );
}

#[test]
fn member_rules_report_like_archunit() {
    let items = layered();
    let lines = |rule: &dyn ArchRule| rule.evaluate(&items).failure_report().details();

    assert_eq!(
        lines(
            &methods()
                .that()
                .are_declared_in_classes_that()
                .reside_in_a_package("..anticorruption..")
                .and()
                .are_public()
                .should()
                .have_raw_return_type("WrappedResult")
        ),
        [
            "Method <layered_app::anticorruption::Facade::unwrapped()> does not have raw return type WrappedResult in (src/anticorruption/mod.rs:13)"
        ]
    );
    assert_eq!(
        lines(
            &no_methods()
                .that()
                .are_declared_in_classes_that()
                .have_name_matching(".*Dao")
                .should()
                .declare_throwable_of_type("SqlError")
        ),
        [
            "Method <layered_app::persistence::first::dao::SomeDao::store(PersistentObject)> does declare throwable of type SqlError in (src/persistence/first/dao/mod.rs:17)"
        ]
    );
    assert_eq!(
        lines(
            &fields()
                .that()
                .have_raw_type("EntityManager")
                .should()
                .be_private()
        ),
        [
            "Field <layered_app::persistence::first::dao::jpa::JpaDao::manager> does not have modifier private in (src/persistence/first/dao/jpa.rs:4)"
        ]
    );
    assert!(
        lines(
            &no_code_units()
                .that()
                .are_declared_in_classes_that()
                .reside_in_a_package("..persistence..")
                .should()
                .be_annotated_with("secured")
        )
        .is_empty()
    );
    assert_eq!(
        lines(
            &methods()
                .that()
                .are_annotated_with("secured")
                .should()
                .be_declared_in_classes_that()
                .reside_in_a_package("..controller..")
        ),
        [
            "Method <layered_app::service::ServiceOne::serve()> is not declared in classes that reside in a package '..controller..' in (src/service/mod.rs:25)"
        ]
    );
    assert_eq!(
        lines(
            &code_units()
                .that()
                .have_name("do_core_stuff")
                .should()
                .only_be_called()
                .by_classes_that()
                .implement("CoreSatellite")
        ),
        [
            "Method <layered_app::service::ServiceViolatingLayerRules::illegal_access_to_core()> calls method <layered_app::core::VeryCentralCore::do_core_stuff()> in (src/service/mod.rs:63)"
        ]
    );
    assert!(
        lines(
            &constructors()
                .that()
                .are_declared_in("SomeController")
                .should()
                .be_public()
        )
        .is_empty(),
        "trait impl methods are as visible as the trait"
    );
    assert_eq!(
        lines(
            &constructors()
                .that()
                .are_declared_in("SomeController")
                .should()
                .have_name("new")
        ),
        [
            "Constructor <layered_app::controller::SomeController::default()> does not have name 'new' in (src/controller/mod.rs:38)"
        ]
    );
}

// ---- custom conditions, transformers, composites -------------------------------------------

#[test]
fn custom_conditions_and_predicates() {
    let items = layered();
    let have_a_field_of_type_dao: DescribedPredicate<RustItem> =
        DescribedPredicate::describe("have a field of type SomeDao", |item: &RustItem| {
            item.fields()
                .iter()
                .any(|f| f.raw_type().is_some_and(|t| t.simple_name() == "SomeDao"))
        });
    let only_be_accessed_by_secured_methods = ArchCondition::new(
        "only be accessed by @secured methods",
        |item: &RustItem, events: &mut ConditionEvents| {
            for call in item.method_calls_to_self() {
                if !call.origin().is_annotated_with("secured") {
                    events.add(SimpleConditionEvent::violated(
                        &call,
                        format!("Method {} is not @secured", call.origin().full_name()),
                    ));
                }
            }
        },
    );
    let rule = classes()
        .that_with(have_a_field_of_type_dao)
        .should_with(only_be_accessed_by_secured_methods);
    assert_eq!(
        rule.description(),
        "classes that have a field of type SomeDao should only be accessed by @secured methods"
    );
    let details = rule.evaluate(&items).failure_report().details();
    assert_eq!(
        details,
        [
            "Method layered_app::controller::SomeController::do_something() is not @secured",
            "Method layered_app::persistence::layerviolation::DaoCallingService::serve() is not @secured",
            "Method layered_app::service::ServiceOne::serve() is not @secured",
        ]
    );
}

#[test]
fn rules_with_custom_transformers() {
    let items = layered();
    let packages: ClassesTransformer<RustModule> =
        ClassesTransformer::new("packages", |items: &RustItems| items.modules());
    let contain_a_dao = DescribedPredicate::describe("contain a DAO", |m: &RustModule| {
        m.items().iter().any(|i| i.simple_name().ends_with("Dao"))
    });
    let be_named_dao = ArchCondition::new(
        "be named 'dao'",
        |m: &RustModule, events: &mut ConditionEvents| {
            let ok = m.relative_name() == "dao";
            events.add(SimpleConditionEvent::new(
                m,
                ok,
                format!("Module <{}> is named '{}'", m.name(), m.relative_name()),
            ));
        },
    );
    let rule = all(packages).that(contain_a_dao).should(be_named_dao);
    assert_eq!(
        rule.description(),
        "packages that contain a DAO should be named 'dao'"
    );
    assert_eq!(
        rule.evaluate(&items).failure_report().details(),
        [
            "Module <layered_app::persistence::first::dao::jpa> is named 'jpa'",
            "Module <layered_app::persistence::second> is named 'second'"
        ]
    );
    let none = no(ClassesTransformer::new("packages", |items: &RustItems| {
        items.modules()
    }))
    .that(always_true())
    .should(ArchCondition::new(
        "be empty",
        |_: &RustModule, _: &mut ConditionEvents| {},
    ));
    assert_eq!(
        none.description(),
        "no packages that always true should be empty"
    );
}

#[test]
fn composite_rules_join_descriptions_and_violations() {
    let items = layered();
    let a = no_classes()
        .that()
        .reside_in_a_package("..service..")
        .should()
        .access_classes_that()
        .reside_in_a_package("..controller..");
    let b = no_classes()
        .that()
        .reside_in_a_package("..internal..")
        .should()
        .be_interfaces();
    let composite = CompositeArchRule::of(a).and(b);
    assert_eq!(
        composite.description(),
        "no classes that reside in a package '..service..' should access classes that reside in a package '..controller..' and no classes that reside in a package '..internal..' should be interfaces"
    );
    let result = composite.evaluate(&items);
    assert_eq!(result.failure_report().details().len(), 3);
    assert!(
        result
            .failure_report()
            .to_string()
            .contains("was violated (3 times)")
    );
    assert_eq!(
        composite
            .because("x")
            .description()
            .split(", because ")
            .nth(1),
        Some("x")
    );
}

#[test]
fn rule_evaluation_inits_and_finishes_conditions() {
    struct Logic {
        seen: Arc<Mutex<Vec<String>>>,
        finished: Arc<Mutex<bool>>,
    }
    impl ConditionLogic<RustItem> for Logic {
        fn init(&mut self, all: &[RustItem]) {
            self.seen
                .lock()
                .unwrap()
                .extend(all.iter().map(HasName::name));
        }
        fn check(&mut self, _item: &RustItem, _events: &mut ConditionEvents) {}
        fn finish(&mut self, events: &mut ConditionEvents) {
            *self.finished.lock().unwrap() = true;
            events.add(SimpleConditionEvent::violated("bummer", "bummer"));
        }
    }
    let seen = Arc::new(Mutex::new(Vec::new()));
    let finished = Arc::new(Mutex::new(false));
    let condition = ArchCondition::from_logic(
        "irrelevant",
        Logic {
            seen: Arc::clone(&seen),
            finished: Arc::clone(&finished),
        },
    );
    let result = classes()
        .that()
        .have_simple_name("SomeDao")
        .should_with(condition)
        .evaluate(&layered());
    assert_eq!(
        *seen.lock().unwrap(),
        ["layered_app::persistence::first::dao::SomeDao"]
    );
    assert!(*finished.lock().unwrap());
    assert_eq!(result.failure_report().details(), ["bummer"]);
}

#[test]
fn reports_number_of_violations() {
    let three_each = ArchCondition::new(
        "be violated exactly 3 times",
        |item: &RustItem, events: &mut ConditionEvents| {
            for i in 0..3 {
                events.add(SimpleConditionEvent::violated(
                    item,
                    format!("{} violation {i}", item.simple_name()),
                ));
            }
        },
    );
    let result = classes()
        .that()
        .belong_to_any_of(&[
            "layered_app::core::VeryCentralCore",
            "layered_app::core::CoreSatellite",
        ])
        .should_with(three_each)
        .evaluate(&layered());
    assert!(result.failure_report().to_string().contains("(6 times)"));

    // `only` style conditions report each violating access separately.
    let result = no_classes()
        .should()
        .access_classes_that()
        .have_simple_name("VeryCentralCore")
        .evaluate(&layered());
    assert!(
        result.failure_report().to_string().contains("(4 times)"),
        "{}",
        result.failure_report()
    );
}

#[test]
fn condition_combinators_describe_and_evaluate() {
    let items = layered();
    let public = cond::be_public::<RustItem>();
    let ends_with_controller = cond::have_simple_name_ending_with("Controller");
    assert_eq!(
        public
            .clone()
            .and(ends_with_controller.clone())
            .description(),
        "be public and have simple name ending with 'Controller'"
    );
    assert_eq!(
        public
            .clone()
            .or(ends_with_controller.clone())
            .description(),
        "be public or have simple name ending with 'Controller'"
    );
    assert_eq!(cond::never(public.clone()).description(), "never be public");
    assert_eq!(cond::not(public.clone()).description(), "not be public");
    let both = classes()
        .that()
        .have_simple_name("WronglyNamed")
        .should_with(public.and(ends_with_controller));
    assert_eq!(
        both.evaluate(&items).failure_report().details(),
        [
            "Item <layered_app::controller::WronglyNamed> does not have simple name ending with 'Controller' in (src/controller/mod.rs:51)"
        ]
    );
    let by_predicate =
        archunit::lang::ConditionByPredicate::from(rust_item::predicates::simple_name("Nope"));
    assert_eq!(by_predicate.description(), "simple name 'Nope'");
    let rule = classes()
        .that()
        .have_simple_name("WronglyNamed")
        .should_with(by_predicate);
    assert_eq!(
        rule.evaluate(&items).failure_report().details(),
        [
            "Item <layered_app::controller::WronglyNamed> does not satisfy simple name 'Nope' in (src/controller/mod.rs:51)"
        ]
    );
}

// ---- empty should, configuration, ignore patterns -------------------------------------------

#[test]
fn evaluation_fails_on_empty_should_by_default() {
    let items = layered();
    let outcome = std::panic::catch_unwind(|| {
        classes()
            .that()
            .reside_in_a_package("..nonexistent..")
            .should()
            .be_public()
            .evaluate(&items);
    });
    let message = outcome.unwrap_err();
    let message = message.downcast_ref::<String>().expect("panic message");
    assert!(message.contains("failed to check any classes"), "{message}");
    assert!(
        message.contains("arch_rule.fail_on_empty_should"),
        "{message}"
    );

    let allowed = classes()
        .that()
        .reside_in_a_package("..nonexistent..")
        .should()
        .be_public()
        .allow_empty_should(true)
        .evaluate(&items);
    assert!(!allowed.has_violation());

    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_fail_on_empty_should(false);
        let result = classes()
            .that()
            .reside_in_a_package("..nonexistent..")
            .should()
            .be_public()
            .evaluate(&items);
        assert!(!result.has_violation());
    });

    let boxed: Box<dyn ArchRule> =
        Box::new(classes().that_with(always_false()).should().be_public());
    assert!(
        !boxed
            .allow_empty_should(true)
            .evaluate(&items)
            .has_violation()
    );
}

#[test]
fn ignore_patterns_filter_violations() {
    let items = layered();
    let rule = no_classes()
        .that()
        .reside_in_a_package("..persistence..")
        .should()
        .depend_on_classes_that()
        .reside_in_a_package("..service..");
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_ignore_patterns(vec![
            Regex::new(r"^Field .*$").unwrap(),
            Regex::new(r"^.*implements trait.*$").unwrap(),
        ]);
        let result = rule.evaluate(&items);
        assert!(result.has_violation());
        assert_eq!(result.failure_report().details().len(), 1);
        assert!(result.failure_report().to_string().contains("(1 times)"));

        ArchConfiguration::set_ignore_patterns(vec![Regex::new(".*").unwrap()]);
        assert!(!rule.evaluate(&items).has_violation());
    });
    assert_eq!(
        rule.evaluate(&items).failure_report().details().len(),
        3,
        "the scope must not leak"
    );

    let dir = std::env::temp_dir().join(format!("archunit-ignore-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("archunit_ignore_patterns.txt");
    std::fs::write(&file, "# comment1\n#comment2\n.*calls method.*\n").unwrap();
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::load_ignore_patterns_from(&file);
        assert_eq!(rule.evaluate(&items).failure_report().details().len(), 2);
    });
    std::fs::remove_dir_all(&dir).ok();
}

// ---- EvaluationResult ------------------------------------------------------------------------

fn events(messages: &[&str]) -> ConditionEvents {
    let mut events = ConditionEvents::new();
    for message in messages {
        events.add(SimpleConditionEvent::violated(*message, *message));
    }
    events
}

struct Described(&'static str);

impl HasDescription for Described {
    fn description(&self) -> String {
        self.0.to_owned()
    }
}

#[test]
fn evaluation_result_reports_description_lines_sorted() {
    let result = EvaluationResult::new(
        &Described("irrelevant"),
        events(&["some event message", "another event message"]),
        Priority::Medium,
    );
    assert_eq!(
        result.failure_report().details(),
        ["another event message", "some event message"]
    );
}

#[test]
fn evaluation_result_properties_are_passed_to_the_report() {
    let result = EvaluationResult::new(
        &Described("special description"),
        events(&["first bummer", "second bummer"]),
        Priority::High,
    );
    let report = result.failure_report().to_string();
    assert_eq!(
        report,
        "Architecture Violation [Priority: HIGH] - Rule 'special description' was violated (2 times):\nfirst bummer\nsecond bummer"
    );
}

#[test]
fn evaluation_result_allows_clients_to_handle_violations() {
    let mut events = ConditionEvents::new();
    events.add(SimpleConditionEvent::new("message", false, "expected"));
    events.add(SimpleConditionEvent::new(
        "other message",
        true,
        "not expected",
    ));
    events.add(SimpleConditionEvent::new(
        "second message",
        false,
        "also expected",
    ));
    let result = EvaluationResult::new(&Described("unimportant"), events, Priority::Medium);
    let mut actual = Vec::new();
    result.handle_violations(|objects: Vec<String>, message: &str| {
        actual.push(format!("{}: {message}", objects.join(",")))
    });
    actual.sort();
    assert_eq!(
        actual,
        ["message: expected", "second message: also expected"]
    );

    // Typed handlers only see objects of their type.
    let items = layered();
    let result = no_classes()
        .should()
        .call_method("VeryCentralCore", "do_core_stuff", &[])
        .evaluate(&items);
    let mut accesses = Vec::new();
    result.handle_violations(
        |objects: Vec<archunit::core::domain::RustAccess>, _message: &str| accesses.extend(objects),
    );
    assert_eq!(accesses.len(), 2);
    let mut items_seen = 0;
    result.handle_violations(|objects: Vec<RustItem>, _message: &str| items_seen += objects.len());
    assert_eq!(items_seen, 0);
}

#[test]
fn evaluation_result_can_filter_lines() {
    let result = EvaluationResult::new(
        &Described("unimportant"),
        events(&[
            "keep first line1",
            "drop first line2",
            "drop first line3",
            "keep second line4",
        ]),
        Priority::Medium,
    );
    let filtered = result.filter_descriptions_matching(|line| line.contains("keep"));
    assert!(filtered.has_violation());
    assert_eq!(
        filtered.failure_report().details(),
        ["keep first line1", "keep second line4"]
    );
    assert!(
        !result
            .filter_descriptions_matching(|_| false)
            .has_violation()
    );
}

#[test]
fn filtering_lines_resets_information_about_number_of_violations() {
    let mut events = events(&["drop first line1", "keep second line1"]);
    events.set_information_about_number_of_violations("test number");
    let result = EvaluationResult::new(&Described("unimportant"), events, Priority::Medium);
    assert!(
        result
            .failure_report()
            .to_string()
            .contains("(test number)")
    );
    let filtered = result.filter_descriptions_matching(|line| line.contains("keep"));
    assert!(filtered.failure_report().to_string().contains("(1 times)"));
    assert!(
        !filtered
            .failure_report()
            .to_string()
            .contains("test number")
    );
}

#[test]
fn simple_condition_event_rejects_empty_violation_messages() {
    assert!(std::panic::catch_unwind(|| SimpleConditionEvent::violated("x", "  ")).is_err());
    let satisfied = SimpleConditionEvent::satisfied("x", "");
    assert!(!archunit::lang::ConditionEvent::is_violation(&satisfied));
}

#[test]
fn simple_arch_rule_factory() {
    let items = layered();
    let transformer = ClassesTransformer::new("strings", |items: &RustItems| {
        let mut names: Vec<String> = items.iter().map(|i| i.name()).collect();
        names.sort();
        names
    });
    let contain_dao = ArchCondition::new(
        "contain 'Dao'",
        |name: &String, events: &mut ConditionEvents| {
            events.add(SimpleConditionEvent::new(
                name.clone(),
                name.contains("Dao"),
                format!("{name} does not contain 'Dao'"),
            ));
        },
    );
    let rule = SimpleArchRule::create(transformer, contain_dao, Priority::Low);
    assert_eq!(rule.description(), "strings should contain 'Dao'");
    let report = rule.evaluate(&items).failure_report();
    assert!(
        report
            .to_string()
            .starts_with("Architecture Violation [Priority: LOW]")
    );
    assert!(
        report
            .details()
            .iter()
            .all(|line| line.ends_with("does not contain 'Dao'"))
    );
}
