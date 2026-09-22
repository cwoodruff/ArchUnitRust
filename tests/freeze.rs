//! `FreezingArchRule`: port of the shape of `FreezingArchRuleTest` and
//! `TextFileBasedViolationStoreTest`.

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use archunit::base::HasDescription;
use archunit::config::ArchConfiguration;
use archunit::core::domain::RustItems;
use archunit::lang::syntax::all;
use archunit::lang::{ArchCondition, ArchRule, ClassesTransformer, SimpleConditionEvent};
use archunit::library::freeze::{
    FreezeError, FreezingArchRule, FuzzyViolationLineMatcher, InMemoryViolationStore,
    TextFileBasedViolationStore, ViolationLineMatcher, ViolationStore, freeze,
};
use common::import_fixture;

/// A rule with a fixed description and configurable violations (Java's `RuleCreator`).
fn rule(description: &str, violations: &[&str]) -> impl ArchRule + Clone + 'static {
    let violations: Vec<String> = violations.iter().map(|v| (*v).to_owned()).collect();
    all(ClassesTransformer::new("subject", |_| {
        vec![String::from("subject")]
    }))
    .should(ArchCondition::new(
        "condition",
        move |subject: &String, events| {
            for violation in &violations {
                events.add(SimpleConditionEvent::violated(
                    subject.clone(),
                    violation.clone(),
                ));
            }
            if violations.is_empty() {
                events.add(SimpleConditionEvent::satisfied(subject.clone(), "fine"));
            }
        },
    ))
    .as_(description)
}

/// A store shared between the test and the frozen rule.
#[derive(Clone, Default)]
struct SharedStore(Arc<Mutex<InMemoryViolationStore>>);

impl ViolationStore for SharedStore {
    fn initialize(&mut self, properties: &HashMap<String, String>) -> Result<(), FreezeError> {
        self.0.lock().unwrap().initialize(properties)
    }

    fn contains(&self, rule: &dyn ArchRule) -> bool {
        self.0.lock().unwrap().contains(rule)
    }

    fn save(&mut self, rule: &dyn ArchRule, violations: Vec<String>) -> Result<(), FreezeError> {
        self.0.lock().unwrap().save(rule, violations)
    }

    fn violations(&self, rule: &dyn ArchRule) -> Result<Vec<String>, FreezeError> {
        self.0.lock().unwrap().violations(rule)
    }
}

impl SharedStore {
    fn stored(&self, description: &str) -> Vec<String> {
        self.0.lock().unwrap().stored_rules()[description].clone()
    }
}

fn items() -> RustItems {
    import_fixture("layered_app")
}

fn details(rule: &dyn ArchRule, items: &RustItems) -> Vec<String> {
    rule.evaluate(items).failure_report().details()
}

fn temp_store_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "archunit_freeze_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn delegates_description_and_supports_as_and_because() {
    let original = rule("some description", &[]);
    let frozen = freeze(original.clone());
    assert_eq!(frozen.description(), original.description());
    assert_eq!(format!("{frozen:?}"), "FreezingArchRule{some description}");
    let frozen = FreezingArchRule::freeze(original)
        .as_("any description")
        .because("some reason");
    assert_eq!(frozen.description(), "any description, because some reason");
}

#[test]
fn freezes_violations_on_first_call_and_passes_afterwards() {
    let store = SharedStore::default();
    let items = items();
    let frozen = freeze(rule(
        "some description",
        &["first violation", "second violation"],
    ))
    .persist_in(store.clone());
    assert!(!frozen.evaluate(&items).has_violation());
    assert_eq!(
        store.stored("some description"),
        ["first violation", "second violation"]
    );
    frozen.check(&items);
    assert!(!frozen.evaluate(&items).has_violation());
}

#[test]
fn fails_on_violations_additional_to_frozen_ones() {
    let store = SharedStore::default();
    let items = items();
    freeze(rule("some description", &["first violation"]))
        .persist_in(store.clone())
        .check(&items);
    let with_new = freeze(rule(
        "some description",
        &["first violation", "second violation"],
    ))
    .persist_in(store.clone());
    assert_eq!(details(&with_new, &items), ["second violation"]);
    let report = with_new.evaluate(&items).failure_report().to_string();
    assert_eq!(
        report,
        "Architecture Violation [Priority: MEDIUM] - Rule 'some description' was violated (1 times):\nsecond violation"
    );
    let panicked =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| with_new.check(&items)));
    assert!(panicked.is_err());
}

#[test]
fn allows_to_overwrite_frozen_violations_if_configured() {
    let store = SharedStore::default();
    let items = items();
    freeze(rule("some description", &["first violation"]))
        .persist_in(store.clone())
        .check(&items);
    let with_new = freeze(rule(
        "some description",
        &["first violation", "second violation"],
    ))
    .persist_in(store.clone());
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_property("freeze.refreeze", "true");
        assert!(!with_new.evaluate(&items).has_violation());
        assert_eq!(
            store.stored("some description"),
            ["first violation", "second violation"]
        );
    });
    assert_eq!(
        details(
            &freeze(rule("some description", &["second violation"])).persist_in(store.clone()),
            &items
        ),
        Vec::<String>::new()
    );
}

#[test]
fn automatically_reduces_allowed_violations_if_any_vanish() {
    let store = SharedStore::default();
    let items = items();
    freeze(rule(
        "some description",
        &["first violation", "second violation"],
    ))
    .persist_in(store.clone())
    .check(&items);
    freeze(rule("some description", &["second violation"]))
        .persist_in(store.clone())
        .check(&items);
    assert_eq!(store.stored("some description"), ["second violation"]);
    let back_again = freeze(rule(
        "some description",
        &["first violation", "second violation"],
    ))
    .persist_in(store.clone());
    assert_eq!(details(&back_again, &items), ["first violation"]);
}

#[test]
fn allows_a_custom_matcher_to_decide_which_violations_count_as_known() {
    let store = SharedStore::default();
    let items = items();
    freeze(rule(
        "some description",
        &["some #1 violation", "some #2 violation"],
    ))
    .persist_in(store.clone())
    .check(&items);
    let matcher = |first: &str, second: &str| {
        first.replace(char::is_numeric, "") == second.replace(char::is_numeric, "")
    };
    let frozen = freeze(rule(
        "some description",
        &["some #7 violation", "another violation"],
    ))
    .persist_in(store.clone())
    .associate_violation_lines_via(matcher);
    assert_eq!(details(&frozen, &items), ["another violation"]);
    assert_eq!(store.stored("some description"), ["some #1 violation"]);
}

#[test]
fn fails_on_an_increased_count_of_the_same_violation() {
    let store = SharedStore::default();
    let items = items();
    freeze(rule("some description", &["violation"]))
        .persist_in(store.clone())
        .check(&items);
    let frozen = freeze(rule("some description", &["violation", "equivalent one"]))
        .persist_in(store.clone())
        .associate_violation_lines_via(|_: &str, _: &str| true);
    assert_eq!(details(&frozen, &items).len(), 1);
}

#[test]
fn default_matcher_ignores_line_numbers_and_generated_numbers() {
    let store = SharedStore::default();
    let items = items();
    freeze(rule(
        "some description",
        &[
            "first violation one in (src/some.rs:12) and first violation two in (src/some.rs:13)",
            "second violation in (src/some.rs:77)",
            "third violation in (src/other.rs:123)",
        ],
    ))
    .persist_in(store.clone())
    .check(&items);
    let only_line_number_changed =
        "first violation one in (src/some.rs:98) and first violation two in (src/some.rs:99)";
    let location_does_not_match = "second violation in (src/other.rs:77)";
    let description_does_not_match = "unknown violation in (src/some.rs:77)";
    let frozen = freeze(rule(
        "some description",
        &[
            only_line_number_changed,
            location_does_not_match,
            description_does_not_match,
        ],
    ))
    .persist_in(store.clone());
    assert_eq!(
        details(&frozen, &items),
        [location_does_not_match, description_does_not_match]
    );

    let matcher = FuzzyViolationLineMatcher;
    assert!(matcher.matches(
        "Method <a::B::c()> calls method <d::E::f()> in (src/a.rs:12)",
        "Method <a::B::c()> calls method <d::E::f()> in (src/a.rs:99)"
    ));
    assert!(matcher.matches("Class <Foo$1> does x", "Class <Foo$23> does x"));
    assert!(!matcher.matches("Class <Foo$1> does x", "Class <Foo$1> does y"));
    assert!(!matcher.matches("a in (src/a.rs:12)", "a in (src/b.rs:12)"));
    assert!(
        !matcher.matches("a:12", "a:13"),
        "a number without closing parenthesis is significant"
    );
}

#[test]
fn violations_ignored_by_archunit_ignore_patterns_are_omitted_from_the_store() {
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_ignore_patterns(vec![regex::Regex::new(".*ignored.*").unwrap()]);
        let store = SharedStore::default();
        freeze(rule(
            "some description",
            &["kept violation", "ignored violation"],
        ))
        .persist_in(store.clone())
        .check(&items());
        assert_eq!(store.stored("some description"), ["kept violation"]);
    });
}

#[test]
fn works_with_multi_line_violations_and_rule_texts() {
    let store = SharedStore::default();
    let items = items();
    let description = "first line\nsecond line";
    freeze(rule(
        description,
        &["first with\nlinebreak", "second with\r\nlinebreak"],
    ))
    .persist_in(store.clone())
    .check(&items);
    assert_eq!(
        store.stored(description),
        ["first with\nlinebreak", "second with\nlinebreak"]
    );
    let frozen =
        freeze(rule(description, &["first with\r\nlinebreak", "third"])).persist_in(store.clone());
    assert_eq!(details(&frozen, &items), ["third"]);
}

// ---- the default store ---------------------------------------------------------------------

#[test]
fn default_violation_store_works() {
    let dir = temp_store_dir("default");
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_property("freeze.store.default.path", &dir.to_string_lossy());
        ArchConfiguration::set_property("freeze.store.default.allow_store_creation", "true");
        let items = items();
        let frozen = freeze(rule(
            "some description",
            &["first violation", "second violation"],
        ));
        assert!(!frozen.evaluate(&items).has_violation());
        assert!(dir.join("stored.rules").is_file());
        let index = std::fs::read_to_string(dir.join("stored.rules")).unwrap();
        assert!(index.contains("some\\ description="), "{index}");
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            2,
            "index plus one rule file"
        );

        let frozen = freeze(rule(
            "some description",
            &["first violation", "third violation"],
        ));
        assert_eq!(details(&frozen, &items), ["third violation"]);
        let frozen = freeze(rule(
            "some description",
            &["first violation", "second violation", "third violation"],
        ));
        assert_eq!(
            details(&frozen, &items),
            ["second violation", "third violation"]
        );

        // A second store instance reads what the first wrote.
        let mut other = TextFileBasedViolationStore::new();
        other
            .initialize(&ArchConfiguration::get().sub_properties("freeze.store"))
            .unwrap();
        let rule = rule("some description", &[]);
        assert!(other.contains(&rule));
        assert_eq!(other.violations(&rule).unwrap(), ["first violation"]);
        assert_eq!(
            other.violations(&self::rule("unknown", &[])).unwrap_err(),
            FreezeError::StoreRead("No rule stored with description 'unknown'".to_owned())
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn default_store_escapes_multi_line_descriptions_and_violations_like_java() {
    let dir = temp_store_dir("escaping");
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_property("freeze.store.default.path", &dir.to_string_lossy());
        ArchConfiguration::set_property("freeze.store.default.allow_store_creation", "true");
        let items = items();
        let description = "rule: with = special\nchars";
        freeze(rule(description, &["first with\nlinebreak", "second"])).check(&items);
        let index = std::fs::read_to_string(dir.join("stored.rules")).unwrap();
        assert!(
            index.contains("rule\\:\\ with\\ \\=\\ special\\nchars="),
            "{index}"
        );
        let rule_file = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.file_name().unwrap() != "stored.rules")
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&rule_file).unwrap(),
            "first with\\\nlinebreak\nsecond\n"
        );
        let frozen = freeze(rule(description, &["second", "third"]));
        assert_eq!(details(&frozen, &items), ["third"]);
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn can_prevent_default_store_from_creation_and_update() {
    let dir = temp_store_dir("disabled");
    let items = items();
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_property("freeze.store.default.path", &dir.to_string_lossy());
        let frozen = freeze(rule("some description", &["violation"]));
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| frozen.check(&items)));
        let message = outcome
            .unwrap_err()
            .downcast_ref::<String>()
            .cloned()
            .unwrap();
        assert_eq!(
            message,
            "Creating new violation store is disabled (enable by configuration freeze.store.default.allow_store_creation=true)"
        );

        ArchConfiguration::set_property("freeze.store.default.allow_store_creation", "true");
        freeze(rule("first, store must be created", &[])).check(&items);
        ArchConfiguration::set_property("freeze.store.default.allow_store_creation", "false");
        freeze(rule("second, store exists", &["first"])).check(&items);
        assert_eq!(
            details(
                &freeze(rule("second, store exists", &["first", "second"])),
                &items
            ),
            ["second"]
        );

        ArchConfiguration::set_property("freeze.store.default.allow_store_update", "false");
        let frozen = freeze(rule("third, unknown rule", &["violation"]));
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| frozen.check(&items)));
        let message = outcome
            .unwrap_err()
            .downcast_ref::<String>()
            .cloned()
            .unwrap();
        assert_eq!(
            message,
            "Updating frozen violations is disabled (enable by configuration freeze.store.default.allow_store_update=true)"
        );
        // Known rules with unchanged violations still pass without an update.
        freeze(rule("second, store exists", &["first"])).check(&items);
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn allows_to_adjust_default_store_file_names_and_configure_the_store_globally() {
    let dir = temp_store_dir("names");
    ArchConfiguration::with_thread_local_scope(|| {
        ArchConfiguration::set_property("freeze.store.default.path", &dir.to_string_lossy());
        ArchConfiguration::set_property("freeze.store.default.allowStoreCreation", "true");
        let dir_for_factory = dir.clone();
        ArchConfiguration::set_violation_store_factory(Arc::new(move || {
            let _ = &dir_for_factory;
            Box::new(TextFileBasedViolationStore::with_file_name_strategy(
                Arc::new(|description| format!("{}.txt", description.replace(' ', "_"))),
            ))
        }));
        freeze(rule("some rule", &["violation"])).check(&items());
        assert!(dir.join("some_rule.txt").is_file());
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn frozen_rules_work_with_real_rules() {
    let store = SharedStore::default();
    let items = items();
    let rule = || no_classes_in_persistence_depend_on_services();
    let frozen = freeze(rule()).persist_in(store.clone());
    assert!(!frozen.evaluate(&items).has_violation());
    assert_eq!(store.stored(&rule().description()).len(), 3);
    frozen.check(&items);
    let boxed: Box<dyn ArchRule> = Box::new(frozen);
    boxed.check(&items);
}

fn no_classes_in_persistence_depend_on_services() -> impl ArchRule + 'static {
    use archunit::prelude::*;
    no_classes()
        .that()
        .reside_in_a_package("..persistence..")
        .should()
        .depend_on_classes_that()
        .reside_in_a_package("..service..")
}
