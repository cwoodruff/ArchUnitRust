//! Ports of `DescribedPredicateTest` and `PackageMatcherTest`.

use archunit::base::{
    DescribedFunction, DescribedPredicate, all_elements, always_false, always_true, and,
    any_element_that, describe, do_not, does_not, empty, equal_to, greater_than,
    greater_than_or_equal_to, less_than, less_than_or_equal_to, not, optional_contains,
    optional_empty, or,
};
use archunit::core::domain::{PackageMatcher, PackageMatchers};

#[test]
fn described_predicate_combinators_compose_descriptions() {
    let even: DescribedPredicate<i32> = describe("even", |n: &i32| n % 2 == 0);
    let positive: DescribedPredicate<i32> = describe("positive", |n: &i32| *n > 0);

    let both = even.clone().and(positive.clone());
    assert_eq!(both.description(), "even and positive");
    assert!(both.test(&2));
    assert!(!both.test(&-2));

    let either = even.clone().or(positive.clone());
    assert_eq!(either.description(), "even or positive");
    assert!(either.test(&-2));
    assert!(!either.test(&-3));

    assert_eq!(even.clone().negate().description(), "not even");
    assert_eq!(not(even.clone()).description(), "not even");
    assert_eq!(does_not(even.clone()).description(), "does not even");
    assert_eq!(do_not(even.clone()).description(), "do not even");
    assert_eq!(even.clone().as_("custom").description(), "custom");
    assert!(do_not(even.clone()).test(&3));

    let all = and(vec![even.clone(), positive.clone(), less_than(10)]);
    assert_eq!(all.description(), "even and positive and less than '10'");
    let any = or(vec![even, positive]);
    assert_eq!(any.description(), "even or positive");
}

#[test]
fn described_predicate_standard_predicates() {
    assert_eq!(always_true::<i32>().description(), "always true");
    assert!(always_true::<i32>().test(&1));
    assert_eq!(always_false::<i32>().description(), "always false");
    assert!(!always_false::<i32>().test(&1));
    assert_eq!(equal_to(5).description(), "equal to '5'");
    assert!(equal_to(5).test(&5));
    assert_eq!(less_than(5).description(), "less than '5'");
    assert_eq!(greater_than(5).description(), "greater than '5'");
    assert_eq!(
        less_than_or_equal_to(5).description(),
        "less than or equal to '5'"
    );
    assert_eq!(
        greater_than_or_equal_to(5).description(),
        "greater than or equal to '5'"
    );
    assert!(greater_than_or_equal_to(5).test(&5));
    assert_eq!(empty::<i32>().description(), "empty");
    assert!(empty::<i32>().test(&vec![]));
    let any = any_element_that(equal_to(1));
    assert_eq!(any.description(), "any element that equal to '1'");
    assert!(any.test(&vec![3, 1]));
    let all = all_elements(equal_to(1));
    assert_eq!(all.description(), "all elements equal to '1'");
    assert!(!all.test(&vec![3, 1]));
    let contains = optional_contains(equal_to(1));
    assert_eq!(contains.description(), "optional contains equal to '1'");
    assert!(contains.test(&Some(1)));
    assert!(optional_empty::<i32>().test(&None));
}

#[test]
fn described_predicate_on_result_of_and_functions() {
    let length: DescribedFunction<String, usize> =
        DescribedFunction::describe("length", |s: &String| s.len());
    let short = length.is(less_than(4));
    assert_eq!(short.description(), "length less than '4'");
    assert!(short.test(&"abc".to_owned()));
    assert!(!short.test(&"abcd".to_owned()));

    let lifted: DescribedPredicate<String> = equal_to(3).on_result_of(|s: &String| s.len());
    assert!(lifted.test(&"abc".to_owned()));
}

#[test]
fn package_matcher_matches_like_archunit() {
    let cases = [
        ("some..", "some", true),
        ("*..some", "some", false),
        ("some..*", "some", false),
        ("..some", "asome", false),
        ("some..", "somea", false),
        ("*::*::*", "wrong::arbitrary::pkg", true),
        ("*::*::*", "wrong::arbitrary::pkg::toomuch", false),
        ("some::arbi*::pk*..", "some::arbitrary::pkg::whatever", true),
        ("some::arbi*..", "some::brbitrary::pkg", false),
        ("some::*rary::*kg..", "some::arbitrary::pkg::whatever", true),
        ("some::*rary..", "some::arbitrarz::pkg", false),
        ("some::pkg", "someepkg", false),
        ("..pkg..", "some::random::pkg::maybe::anywhere", true),
        ("..p..", "s::r::p::m::a", true),
        ("*..pkg..*", "some::random::pkg::maybe::anywhere", true),
        ("*..p..*", "s::r::p::m::a", true),
        ("..[a|b|c]::pk*..", "some::a::pkg::whatever", true),
        ("..[b|c]::pk*..", "some::a::pkg::whatever", false),
        ("..[a|b*]::pk*..", "some::bitrary::pkg::whatever", true),
        ("..[a|b*]::pk*..", "some::a::pkg::whatever", true),
        ("..[a|b*]::pk*..", "some::arbitrary::pkg::whatever", false),
        (
            "..[*c*|*d*]::pk*..",
            "some::anydinside::pkg::whatever",
            true,
        ),
        ("..[*c*|*d*]::pk*..", "some::nofit::pkg::whatever", false),
        ("..service..", "my_app::service", true),
        ("..service..", "my_app::service::internal", true),
        ("..service", "my_app::service::internal", false),
        ("my_app::*", "my_app::service", true),
        ("my_app::*", "my_app::service::internal", false),
        ("crate::service..", "my_app::service::internal", true),
        ("crate::service..", "other::service::internal", true),
        ("crate", "my_app", true),
    ];
    for (matcher, target, expected) in cases {
        assert_eq!(
            PackageMatcher::of(matcher).matches(target),
            expected,
            "'{matcher}' matching '{target}'"
        );
    }
}

#[test]
fn package_matcher_capture_groups_like_archunit() {
    let cases: [(&str, &str, Option<&str>); 20] = [
        ("some::(*)::pkg", "some::arbitrary::pkg", Some("arbitrary")),
        ("some::arb(*)ry::pkg", "some::arbitrary::pkg", Some("itra")),
        ("some::arb(*)ry::pkg", "some::arbit::rary::pkg", None),
        (
            "some::(*)::matches::(*)::pkg",
            "some::first::matches::second::pkg",
            Some("first second"),
        ),
        (
            "(*)::matches::(*)",
            "start::matches::end",
            Some("start end"),
        ),
        ("(*)::(*)::(*)::(*)", "a::b::c::d", Some("a b c d")),
        ("(*)", "some", Some("some")),
        ("some::(*)::pkg", "some::in::between::pkg", None),
        (
            "some::(**)::pkg",
            "some::in::between::pkg",
            Some("in::between"),
        ),
        (
            "some::(**)::pkg::(*)",
            "some::in::between::pkg::addon",
            Some("in::between addon"),
        ),
        (
            "some(**)pkg",
            "somerandom::in::between::longpkg",
            Some("random::in::between::long"),
        ),
        ("some::(**)::pkg", "somer::in::between::pkg", None),
        ("some::(**)::pkg", "some::in::between::gpkg", None),
        ("so(**)me", "some", None),
        ("so(*)me", "some", None),
        ("(**)so", "awe::some::aso", Some("awe::some::a")),
        (
            "..(a|b)::pk*::(c|d)..",
            "some::a::pkg::d::whatever",
            Some("a d"),
        ),
        (
            "..[a|b|c]::pk*::(c|d)..",
            "some::c::pkg::d::whatever",
            Some("d"),
        ),
        (
            "..(a|b*|cd)::pk*::(**)::end",
            "some::bitrary::pkg::in::between::end",
            Some("bitrary in::between"),
        ),
        (
            "..[application|domain::*|infrastructure]::(*)..",
            "com::example::domain::api::a",
            Some("a"),
        ),
    ];
    for (matcher, target, groups) in cases {
        let result = PackageMatcher::of(matcher).match_(target);
        assert_eq!(
            result.is_some(),
            groups.is_some(),
            "'{matcher}' matching '{target}'"
        );
        if let (Some(result), Some(groups)) = (result, groups) {
            let expected: Vec<&str> = groups.split(' ').collect();
            assert_eq!(
                result.number_of_groups(),
                expected.len(),
                "'{matcher}' matching '{target}'"
            );
            for (i, expected) in expected.iter().enumerate() {
                assert_eq!(
                    result.group(i + 1),
                    *expected,
                    "group {} of '{matcher}' on '{target}'",
                    i + 1
                );
            }
        }
    }
}

#[test]
fn package_matcher_rejects_illegal_identifiers() {
    for illegal in [
        "some...pkg",
        "some**package",
        "some::(..)::package",
        "some::[nonalternating]::package",
        "some::pkg|other::pkg",
    ] {
        assert!(
            PackageMatcher::try_of(illegal).is_err(),
            "{illegal} should be rejected"
        );
    }
}

#[test]
fn package_matchers_of_describes_and_matches_any() {
    let predicate = PackageMatchers::of(&["..service..", "..dao.."]);
    assert_eq!(
        predicate.description(),
        "matches any of ['..service..', '..dao..']"
    );
    assert!(predicate.test(&"a::service::b".to_owned()));
    assert!(predicate.test(&"a::dao".to_owned()));
    assert!(!predicate.test(&"a::controller".to_owned()));
}
