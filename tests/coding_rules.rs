//! `GeneralCodingRules`, `DependencyRules` and `ProxyRules` against `tests/fixtures/coding_app`.

mod common;

use archunit::config::ArchConfiguration;
use archunit::core::domain::{MemberLike, RustItems};
use archunit::lang::ArchRule;
use archunit::library::dependency_rules::{
    NO_CLASSES_SHOULD_DEPEND_UPPER_PACKAGES, depend_on_upper_packages,
};
use archunit::library::general_coding_rules::*;
use archunit::library::proxy_rules;
use archunit::prelude::*;
use common::import_fixture;

fn coding() -> RustItems {
    import_fixture("coding_app")
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

#[test]
fn rule_descriptions_match_archunit() {
    assert_eq!(
        NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS.description(),
        "no classes should access standard streams"
    );
    assert_eq!(
        NO_CLASSES_SHOULD_THROW_GENERIC_EXCEPTIONS.description(),
        "no classes should throw generic exceptions"
    );
    assert_eq!(
        ASSERTIONS_SHOULD_HAVE_DETAIL_MESSAGE.description(),
        "no classes should invoke assertion macros without a detail message, because assertions should have a detail message"
    );
    assert_eq!(
        DEPRECATED_API_SHOULD_NOT_BE_USED.description(),
        "no classes should access @deprecated members or should depend on @deprecated classes, because there should be a better alternative"
    );
    assert_eq!(
        NO_CLASSES_SHOULD_DEPEND_UPPER_PACKAGES.description(),
        "no classes should depend on upper packages, because that might prevent packages on that level from being split into separate artifacts in a clean way"
    );
    assert_eq!(
        proxy_rules::no_classes_should_directly_call_other_methods_declared_in_the_same_class_that_are_annotated_with("transactional").description(),
        "no classes should directly call other methods declared in the same class that are annotated with @transactional, because it bypasses the proxy mechanism"
    );
    assert_eq!(
        NO_CLASSES_SHOULD_PANIC.description(),
        "no classes should panic"
    );
    assert_eq!(
        NO_CLASSES_SHOULD_USE_UNSAFE.description(),
        "no classes should use unsafe"
    );
    assert_eq!(
        NO_CLASSES_SHOULD_CALL_UNWRAP.description(),
        "no classes should call unwrap or expect outside test code, because errors should be propagated or handled, not crash the program"
    );
    assert_eq!(
        NO_LIBRARY_CODE_SHOULD_CALL_PROCESS_EXIT.description(),
        "no library code should call std::process::exit, because only a binary's main may decide to end the process"
    );
}

#[test]
fn standard_streams() {
    let items = coding();
    assert_report(
        &*NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS,
        &items,
        "coding_standard_streams",
    );
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_property("import.exclude_binaries_from_coding_rules", "false");
        let details = NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS
            .evaluate(&items)
            .failure_report()
            .details();
        assert!(
            details.iter().any(|d| d.contains("coding_cli::main()")),
            "{details:#?}"
        );
    });
    // The condition composes like any other.
    let outside_console = no_classes()
        .that()
        .do_not_have_simple_name("Console")
        .should_with(ACCESS_STANDARD_STREAMS.clone());
    assert!(!outside_console.evaluate(&items).has_violation());
}

#[test]
fn generic_exceptions() {
    let items = coding();
    assert_report(
        &*NO_CLASSES_SHOULD_THROW_GENERIC_EXCEPTIONS,
        &items,
        "coding_generic_exceptions",
    );
    let typed_only = methods().that().have_name("typed").should_with(
        archunit::lang::ConditionByPredicate::from(archunit::base::not(
            have_generic_error_type()
                .on_result_of(|m: &archunit::core::domain::RustMethod| m.as_member().clone()),
        ))
        .into_condition(),
    );
    assert!(!typed_only.evaluate(&items).has_violation());
}

#[test]
fn assertions_without_messages() {
    assert_report(
        &*ASSERTIONS_SHOULD_HAVE_DETAIL_MESSAGE,
        &coding(),
        "coding_assertions",
    );
}

#[test]
fn deprecated_api() {
    assert_report(
        &*DEPRECATED_API_SHOULD_NOT_BE_USED,
        &coding(),
        "coding_deprecated",
    );
}

#[test]
fn unwrap_panic_exit_and_unsafe() {
    let items = coding();
    assert_report(&*NO_CLASSES_SHOULD_CALL_UNWRAP, &items, "coding_unwrap");
    assert_report(&*NO_CLASSES_SHOULD_PANIC, &items, "coding_panic");
    assert_report(
        &*NO_LIBRARY_CODE_SHOULD_CALL_PROCESS_EXIT,
        &items,
        "coding_process_exit",
    );
    assert_report(&*NO_CLASSES_SHOULD_USE_UNSAFE, &items, "coding_unsafe");
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_property("import.exclude_binaries_from_coding_rules", "false");
        let details = NO_LIBRARY_CODE_SHOULD_CALL_PROCESS_EXIT
            .evaluate(&items)
            .failure_report()
            .details();
        assert!(
            details.iter().any(|d| d.contains("coding_cli::main()")),
            "{details:#?}"
        );
    });
}

#[test]
fn upper_packages() {
    let items = coding();
    assert_report(
        &*NO_CLASSES_SHOULD_DEPEND_UPPER_PACKAGES,
        &items,
        "coding_upper_packages",
    );
    // `mod tests` depends on its parent by design, so it is excluded here.
    let outside_lower = no_classes()
        .that()
        .reside_outside_of_packages(&["..lower..", "..tests.."])
        .should_with(depend_on_upper_packages());
    assert!(!outside_lower.evaluate(&items).has_violation());
}

#[test]
fn proxy_rules() {
    let items = coding();
    let rule = proxy_rules::no_classes_should_directly_call_other_methods_declared_in_the_same_class_that_are_annotated_with("transactional");
    assert_report(&rule, &items, "coding_proxy");
    let by_predicate =
        proxy_rules::no_classes_should_directly_call_other_methods_declared_in_the_same_class_that(
            archunit::core::domain::access_target::predicates::name("helper"),
        );
    assert_eq!(
        by_predicate.description(),
        "no classes should directly call other methods declared in the same class that name 'helper', because it bypasses the proxy mechanism"
    );
    let details = by_predicate.evaluate(&items).failure_report().details();
    assert_eq!(details.len(), 1, "{details:#?}");
    assert!(
        details[0].contains("uses_helper()> calls method"),
        "{details:#?}"
    );
}
