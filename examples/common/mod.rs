//! Shared plumbing for the examples: imports a fixture crate, prints or asserts reports.
//!
//! The examples mirror `archunit-example`: most rules are violated on purpose, so the
//! expected failure report of every rule is kept under `examples/expected/<example>/`.
//! `cargo run --example <name>` prints the reports; `cargo test --examples` compares them.
#![allow(dead_code, unused_macros)]

use std::path::PathBuf;
use std::sync::Arc;

use archunit::core::domain::RustItems;
use archunit::harness::analyze_classes;
use archunit::lang::ArchRule;

/// The text written for a rule without violations.
pub const NO_VIOLATIONS: &str = "<no violations>";

pub fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// Imports (and caches) a fixture crate from `tests/fixtures/<name>`.
pub fn fixture(name: &str) -> Arc<RustItems> {
    analyze_classes().crate_dir(fixture_dir(name)).import()
}

pub fn report_of(rule: &dyn ArchRule, items: &RustItems) -> String {
    let result = rule.evaluate(items);
    if result.has_violation() {
        result.failure_report().to_string()
    } else {
        NO_VIOLATIONS.to_owned()
    }
}

pub fn print(example: &str, name: &str, rule: &dyn ArchRule, items: &RustItems) {
    println!("=== {example}::{name}\n{}\n", report_of(rule, items));
}

pub fn assert_report(example: &str, name: &str, rule: &dyn ArchRule, items: &RustItems) {
    let actual = report_of(rule, items);
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("expected")
        .join(example);
    let path = dir.join(format!("{name}.txt"));
    if std::env::var("UPDATE_EXPECTED").is_ok() {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, format!("{actual}\n")).unwrap();
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .trim_end()
        .to_owned();
    assert_eq!(actual, expected, "report of {example}::{name}");
}

/// Declares `main()` and one test per rule function.
///
/// ```ignore
/// archunit_example! {
///     example = "layered_architecture",
///     fixture = "layered_app",
///     rules = [layer_dependencies_are_respected, layer_dependencies_are_respected_with_exception],
/// }
/// ```
macro_rules! archunit_example {
    (example = $example:literal, fixture = $fixture:literal, rules = [$($rule:ident),* $(,)?] $(,)?) => {
        archunit_example!(example = $example, fixture = $fixture, setup = || {}, rules = [$($rule),*]);
    };
    (example = $example:literal, fixture = $fixture:literal, setup = $setup:expr, rules = [$($rule:ident),* $(,)?] $(,)?) => {
        fn main() {
            ($setup)();
            let items = common::fixture($fixture);
            $( common::print($example, stringify!($rule), &$rule(), &items); )*
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            $(
                #[test]
                fn $rule() {
                    ($setup)();
                    let items = common::fixture($fixture);
                    common::assert_report($example, stringify!($rule), &super::$rule(), &items);
                }
            )*
        }
    };
}
