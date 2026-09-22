//! Port of `CodingRulesTest`, `DependencyRulesTest` and `ProxyRulesTest` against
//! `tests/fixtures/coding_app`. `NO_CLASSES_SHOULD_USE_JAVA_UTIL_LOGGING`, `..JODATIME` and
//! `..FIELD_INJECTION` have no Rust counterpart (see docs/MAPPING.md §5.4).
#[macro_use]
mod common;

use archunit::lang::CompositeArchRule;
use archunit::library::dependency_rules::NO_CLASSES_SHOULD_DEPEND_UPPER_PACKAGES;
use archunit::library::general_coding_rules::*;
use archunit::library::proxy_rules::no_classes_should_directly_call_other_methods_declared_in_the_same_class_that_are_annotated_with;
use archunit::prelude::*;

fn classes_should_not_access_standard_streams_defined_by_hand() -> impl ArchRule {
    no_classes().should_with(ACCESS_STANDARD_STREAMS.clone())
}

fn classes_should_not_access_standard_streams_from_library() -> impl ArchRule {
    NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS.clone()
}

fn classes_should_not_throw_generic_exceptions() -> impl ArchRule {
    NO_CLASSES_SHOULD_THROW_GENERIC_EXCEPTIONS.clone()
}

fn no_classes_should_access_standard_streams_or_throw_generic_exceptions() -> impl ArchRule {
    CompositeArchRule::of(NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS.clone())
        .and(NO_CLASSES_SHOULD_THROW_GENERIC_EXCEPTIONS.clone())
}

fn assertions_should_have_detail_message() -> impl ArchRule {
    ASSERTIONS_SHOULD_HAVE_DETAIL_MESSAGE.clone()
}

fn deprecated_api_should_not_be_used() -> impl ArchRule {
    DEPRECATED_API_SHOULD_NOT_BE_USED.clone()
}

fn no_accesses_to_upper_package() -> impl ArchRule {
    NO_CLASSES_SHOULD_DEPEND_UPPER_PACKAGES.clone()
}

fn no_bypass_of_proxy_logic() -> impl ArchRule {
    no_classes_should_directly_call_other_methods_declared_in_the_same_class_that_are_annotated_with(
        "transactional",
    )
}

// Rust-only rules.

fn no_classes_should_call_unwrap() -> impl ArchRule {
    NO_CLASSES_SHOULD_CALL_UNWRAP.clone()
}

fn no_classes_should_panic() -> impl ArchRule {
    NO_CLASSES_SHOULD_PANIC.clone()
}

fn no_library_code_should_call_process_exit() -> impl ArchRule {
    NO_LIBRARY_CODE_SHOULD_CALL_PROCESS_EXIT.clone()
}

fn no_classes_should_use_unsafe() -> impl ArchRule {
    NO_CLASSES_SHOULD_USE_UNSAFE.clone()
}

archunit_example! {
    example = "coding_rules",
    fixture = "coding_app",
    rules = [
        classes_should_not_access_standard_streams_defined_by_hand,
        classes_should_not_access_standard_streams_from_library,
        classes_should_not_throw_generic_exceptions,
        no_classes_should_access_standard_streams_or_throw_generic_exceptions,
        assertions_should_have_detail_message,
        deprecated_api_should_not_be_used,
        no_accesses_to_upper_package,
        no_bypass_of_proxy_logic,
        no_classes_should_call_unwrap,
        no_classes_should_panic,
        no_library_code_should_call_process_exit,
        no_classes_should_use_unsafe,
    ],
}
