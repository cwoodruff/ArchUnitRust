//! Global configuration (`ArchConfiguration` / `archunit.properties` → `archunit.toml`).
//!
//! Properties are read from `archunit.toml` next to the `Cargo.toml` of the crate under test
//! (searched upward to the workspace root) with nested tables flattened to dotted keys, e.g.
//!
//! ```toml
//! [arch_rule]
//! fail_on_empty_should = false
//!
//! [cycles]
//! max_number_to_detect = 50
//!
//! resolve_missing_dependencies_from_classpath = true
//! [class_resolver]
//! packages = ["tokio..", "serde.."]
//! ```
//!
//! gives the properties `arch_rule.fail_on_empty_should`, `cycles.max_number_to_detect`,
//! `resolve_missing_dependencies_from_classpath` and `class_resolver.packages`
//! (arrays become comma-separated values). Every property can be overridden by an
//! environment variable named `ARCHUNIT_` + the key upper-cased with `.` and `-` replaced by
//! `_` (`ARCHUNIT_ARCH_RULE_FAIL_ON_EMPTY_SHOULD=false`), the counterpart of Java's
//! `-Darchunit.archRule.failOnEmptyShould=false`. Violations to ignore live in
//! `archunit_ignore_patterns.txt`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, RwLock};

use regex::Regex;

use crate::lang::FailureDisplayFormat;
use crate::library::freeze::{ViolationLineMatcher, ViolationStore};

/// The file, relative to the crate root, listing regexes of violations to ignore.
pub const ARCHUNIT_IGNORE_PATTERNS_FILE_NAME: &str = "archunit_ignore_patterns.txt";

/// The configuration file (`archunit.properties`).
pub const ARCHUNIT_PROPERTIES_RESOURCE_NAME: &str = "archunit.toml";

/// The prefix of environment variables overriding properties.
pub const ENVIRONMENT_VARIABLE_PREFIX: &str = "ARCHUNIT_";

/// `resolveMissingDependenciesFromClassPath`: parse dependency crates instead of stubbing them.
pub const RESOLVE_MISSING_DEPENDENCIES_FROM_CLASS_PATH: &str =
    "resolve_missing_dependencies_from_classpath";

/// `classResolver.args`: the crates to parse when resolving from the classpath
/// (`class_resolver.packages = ["tokio..", "serde.."]`).
pub const CLASS_RESOLVER_PACKAGES_PROPERTY_NAME: &str = "class_resolver.packages";

/// `[rust-only]` the cargo target kinds to import
/// (`import.include_targets = ["lib", "bin", "test", "example", "bench"]`).
pub const INCLUDE_TARGETS_PROPERTY_NAME: &str = "import.include_targets";

/// `[rust-only]` whether binary targets are exempt from the standard-stream and process-exit
/// coding rules (`import.exclude_binaries_from_coding_rules`, default `true`).
pub const EXCLUDE_BINARIES_FROM_CODING_RULES_PROPERTY_NAME: &str =
    "import.exclude_binaries_from_coding_rules";

/// The configuration key controlling whether rules fail when they check nothing
/// (`archRule.failOnEmptyShould`).
pub const FAIL_ON_EMPTY_SHOULD_PROPERTY_NAME: &str = "arch_rule.fail_on_empty_should";

/// `cycles.maxNumberToDetect`: the maximum number of cycles a slice rule reports.
pub const MAX_NUMBER_OF_CYCLES_TO_DETECT_PROPERTY_NAME: &str = "cycles.max_number_to_detect";

/// `cycles.maxNumberOfDependenciesPerEdge`: the maximum number of dependencies printed per
/// edge of a cycle.
pub const MAX_NUMBER_OF_DEPENDENCIES_PER_EDGE_PROPERTY_NAME: &str =
    "cycles.max_number_of_dependencies_per_edge";

/// A snapshot of configuration values.
#[derive(Clone)]
pub struct ArchConfiguration {
    ignore_patterns: Option<Vec<Regex>>,
    failure_display_format: Option<Arc<dyn FailureDisplayFormat>>,
    violation_store_factory: Option<ViolationStoreFactory>,
    violation_line_matcher: Option<Arc<dyn ViolationLineMatcher>>,
    properties: std::collections::HashMap<String, String>,
}

/// Creates the [`ViolationStore`] used by `FreezingArchRule::freeze` (`freeze.store`).
pub type ViolationStoreFactory = Arc<dyn Fn() -> Box<dyn ViolationStore> + Send + Sync>;

impl std::fmt::Debug for ArchConfiguration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchConfiguration")
            .field(
                "ignore_patterns",
                &self
                    .ignore_patterns
                    .as_ref()
                    .map(|p| p.iter().map(Regex::as_str).collect::<Vec<_>>()),
            )
            .field(
                "failure_display_format",
                &self.failure_display_format.is_some(),
            )
            .finish()
    }
}

impl Default for ArchConfiguration {
    /// The configuration from `archunit.toml` of the crate under test, or empty defaults.
    fn default() -> Self {
        Self {
            ignore_patterns: None,
            failure_display_format: None,
            violation_store_factory: None,
            violation_line_matcher: None,
            properties: find_configuration_file(&crate::core::importer::current_crate_dir())
                .map(|file| read_properties(&file))
                .unwrap_or_default(),
        }
    }
}

static GLOBAL: LazyLock<RwLock<ArchConfiguration>> =
    LazyLock::new(|| RwLock::new(ArchConfiguration::default()));

thread_local! {
    static THREAD_LOCAL: RefCell<Option<ArchConfiguration>> = const { RefCell::new(None) };
}

impl ArchConfiguration {
    /// A copy of the effective configuration: the thread-local scope if one is active
    /// (see [`with_thread_local_scope`](Self::with_thread_local_scope)), else the global one.
    pub fn get() -> ArchConfiguration {
        THREAD_LOCAL.with(|local| {
            local
                .borrow()
                .clone()
                .unwrap_or_else(|| GLOBAL.read().expect("configuration lock").clone())
        })
    }

    fn update(f: impl FnOnce(&mut ArchConfiguration)) {
        let mut f = Some(f);
        let in_scope = THREAD_LOCAL.with(|local| {
            let mut local = local.borrow_mut();
            match local.as_mut() {
                Some(config) => {
                    if let Some(f) = f.take() {
                        f(config);
                    }
                    true
                }
                None => false,
            }
        });
        if !in_scope {
            if let Some(f) = f.take() {
                f(&mut GLOBAL.write().expect("configuration lock"));
            }
        }
    }

    /// Runs `f` with a private copy of the configuration for the current thread; changes made
    /// inside do not leak out (`ArchConfiguration.withThreadLocalScope(..)`).
    pub fn with_thread_local_scope<R>(f: impl FnOnce() -> R) -> R {
        let previous = THREAD_LOCAL.with(|local| local.replace(Some(ArchConfiguration::get())));
        struct Restore(Option<ArchConfiguration>);
        impl Drop for Restore {
            fn drop(&mut self) {
                let previous = self.0.take();
                THREAD_LOCAL.with(|local| *local.borrow_mut() = previous);
            }
        }
        let _restore = Restore(previous);
        f()
    }

    /// Resets the effective configuration to the defaults (`ArchConfiguration.reset()`).
    pub fn reset() {
        Self::update(|c| *c = ArchConfiguration::default());
    }

    /// Replaces the properties with those of the given `archunit.toml`
    /// (`ArchConfiguration.reset()` against another file) `[rust-only]`.
    pub fn load_from(file: impl AsRef<Path>) {
        let properties = read_properties(file.as_ref());
        Self::update(|c| c.properties = properties);
    }

    /// Whether rules fail when no items reach the `should` clause (`archRule.failOnEmptyShould`).
    pub fn fail_on_empty_should(&self) -> bool {
        self.bool_property(FAIL_ON_EMPTY_SHOULD_PROPERTY_NAME, true)
    }

    /// Sets [`fail_on_empty_should`](Self::fail_on_empty_should).
    pub fn set_fail_on_empty_should(value: bool) {
        Self::set_property(FAIL_ON_EMPTY_SHOULD_PROPERTY_NAME, &value.to_string());
    }

    /// `resolveMissingDependenciesFromClassPath` (default `false`): whether the importer parses
    /// the source of dependency crates instead of stubbing their items.
    pub fn resolve_missing_dependencies_from_classpath(&self) -> bool {
        self.bool_property(RESOLVE_MISSING_DEPENDENCIES_FROM_CLASS_PATH, false)
    }

    /// Sets [`resolve_missing_dependencies_from_classpath`](Self::resolve_missing_dependencies_from_classpath).
    pub fn set_resolve_missing_dependencies_from_classpath(value: bool) {
        Self::set_property(
            RESOLVE_MISSING_DEPENDENCIES_FROM_CLASS_PATH,
            &value.to_string(),
        );
    }

    /// The crate identifiers (`class_resolver.packages`) restricting which dependency crates
    /// are parsed when resolving from the classpath; empty means all of them
    /// (`classResolver.args` of `SelectedClassResolverFromClasspath`).
    pub fn class_resolver_packages(&self) -> Vec<String> {
        self.list_property(CLASS_RESOLVER_PACKAGES_PROPERTY_NAME)
    }

    /// `[rust-only]` the cargo target kinds to import (`import.include_targets`), empty for all.
    pub fn include_targets(&self) -> Vec<String> {
        self.list_property(INCLUDE_TARGETS_PROPERTY_NAME)
    }

    fn bool_property(&self, name: &str, default: bool) -> bool {
        self.property(name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    fn list_property(&self, name: &str) -> Vec<String> {
        self.property(name)
            .map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The regexes from `archunit_ignore_patterns.txt` (lines starting with `#` are comments),
    /// loaded from the crate under test on first use, or set programmatically.
    pub fn ignore_patterns(&self) -> Vec<Regex> {
        match &self.ignore_patterns {
            Some(patterns) => patterns.clone(),
            None => {
                let loaded = load_ignore_patterns(&crate::core::importer::current_crate_dir());
                Self::update(|c| {
                    if c.ignore_patterns.is_none() {
                        c.ignore_patterns = Some(loaded.clone());
                    }
                });
                loaded
            }
        }
    }

    /// Replaces the ignore patterns.
    pub fn set_ignore_patterns(patterns: Vec<Regex>) {
        Self::update(|c| c.ignore_patterns = Some(patterns));
    }

    /// Loads ignore patterns from the given file, replacing the current ones.
    pub fn load_ignore_patterns_from(file: impl AsRef<Path>) {
        let patterns = read_ignore_patterns(file.as_ref());
        Self::set_ignore_patterns(patterns);
    }

    /// A free-form property (`ArchConfiguration.getProperty(..)`), e.g.
    /// `cycles.max_number_to_detect`; an environment variable `ARCHUNIT_<KEY>` takes precedence.
    pub fn property(&self, name: &str) -> Option<String> {
        std::env::var(environment_variable_name(name))
            .ok()
            .or_else(|| self.properties.get(name).cloned())
    }

    /// Removes a property (`ArchConfiguration.removeProperty(..)`); an environment override stays.
    pub fn remove_property(name: &str) {
        let name = name.to_owned();
        Self::update(|c| {
            c.properties.remove(&name);
        });
    }

    /// `getPropertyOrDefault(..)`.
    pub fn property_or_default(&self, name: &str, default: &str) -> String {
        self.property(name).unwrap_or_else(|| default.to_owned())
    }

    /// `containsProperty(..)`.
    pub fn contains_property(&self, name: &str) -> bool {
        self.property(name).is_some()
    }

    /// `setProperty(..)`.
    pub fn set_property(name: &str, value: &str) {
        let (name, value) = (name.to_owned(), value.to_owned());
        Self::update(|c| {
            c.properties.insert(name, value);
        });
    }

    /// Properties whose keys start with `prefix.`, with the prefix removed (`getSubProperties(..)`).
    /// Environment overrides apply to the keys present in the file or set programmatically.
    pub fn sub_properties(&self, prefix: &str) -> std::collections::HashMap<String, String> {
        let prefix = format!("{prefix}.");
        self.properties
            .keys()
            .filter_map(|k| {
                k.strip_prefix(&prefix)
                    .map(|rest| (rest.to_owned(), self.property(k).unwrap_or_default()))
            })
            .collect()
    }

    fn usize_property(&self, name: &str, default: usize) -> usize {
        self.property(name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    /// `cycles.max_number_to_detect` (default 100).
    pub fn max_number_of_cycles_to_detect(&self) -> usize {
        self.usize_property(MAX_NUMBER_OF_CYCLES_TO_DETECT_PROPERTY_NAME, 100)
    }

    /// `cycles.max_number_of_dependencies_per_edge` (default 20).
    pub fn max_number_of_dependencies_per_edge(&self) -> usize {
        self.usize_property(MAX_NUMBER_OF_DEPENDENCIES_PER_EDGE_PROPERTY_NAME, 20)
    }

    /// The format used to render failure reports (`failureDisplayFormat`).
    pub fn failure_display_format(&self) -> Option<Arc<dyn FailureDisplayFormat>> {
        self.failure_display_format.clone()
    }

    /// Sets the format used to render failure reports.
    pub fn set_failure_display_format(format: Arc<dyn FailureDisplayFormat>) {
        Self::update(|c| c.failure_display_format = Some(format));
    }

    /// `[rust-only]` whether binary targets are exempt from the coding rules about standard
    /// streams and `process::exit` (default `true`).
    pub fn exclude_binaries_from_coding_rules(&self) -> bool {
        self.bool_property(EXCLUDE_BINARIES_FROM_CODING_RULES_PROPERTY_NAME, true)
    }

    /// The factory for the violation store of frozen rules, if one was configured
    /// (Java: the `freeze.store` class name).
    pub fn violation_store_factory(&self) -> Option<ViolationStoreFactory> {
        self.violation_store_factory.clone()
    }

    /// Configures the violation store used by `FreezingArchRule::freeze` (`freeze.store`).
    pub fn set_violation_store_factory(factory: ViolationStoreFactory) {
        Self::update(|c| c.violation_store_factory = Some(factory));
    }

    /// The line matcher for frozen rules, if one was configured (Java: `freeze.lineMatcher`).
    pub fn violation_line_matcher(&self) -> Option<Arc<dyn ViolationLineMatcher>> {
        self.violation_line_matcher.clone()
    }

    /// Configures the line matcher used by `FreezingArchRule::freeze` (`freeze.lineMatcher`).
    pub fn set_violation_line_matcher(matcher: Arc<dyn ViolationLineMatcher>) {
        Self::update(|c| c.violation_line_matcher = Some(matcher));
    }
}

/// The environment variable overriding `name`: `ARCHUNIT_` + upper-cased key with `.`/`-` → `_`.
pub fn environment_variable_name(name: &str) -> String {
    format!(
        "{ENVIRONMENT_VARIABLE_PREFIX}{}",
        name.to_uppercase().replace(['.', '-'], "_")
    )
}

/// Finds `file_name` in `crate_dir` or one of its parents up to the workspace root.
fn find_upward(crate_dir: &Path, file_name: &str) -> Option<PathBuf> {
    let mut dir: Option<PathBuf> = Some(crate_dir.to_path_buf());
    while let Some(current) = dir {
        let candidate = current.join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if current.join("Cargo.toml").is_file()
            && std::fs::read_to_string(current.join("Cargo.toml"))
                .is_ok_and(|t| t.contains("[workspace]"))
        {
            break;
        }
        dir = current.parent().map(Path::to_path_buf);
    }
    None
}

fn find_configuration_file(crate_dir: &Path) -> Option<PathBuf> {
    find_upward(crate_dir, ARCHUNIT_PROPERTIES_RESOURCE_NAME)
}

/// Reads `archunit.toml`, flattening tables to dotted keys and arrays to comma-separated values.
fn read_properties(file: &Path) -> std::collections::HashMap<String, String> {
    let mut properties = std::collections::HashMap::new();
    let Ok(text) = std::fs::read_to_string(file) else {
        return properties;
    };
    match text.parse::<toml::Table>() {
        Ok(table) => flatten_table("", &table, &mut properties),
        Err(e) => eprintln!("archunit: ignoring invalid {}: {e}", file.display()),
    }
    properties
}

fn flatten_table(
    prefix: &str,
    table: &toml::Table,
    into: &mut std::collections::HashMap<String, String>,
) {
    for (key, value) in table {
        let name = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            toml::Value::Table(nested) => flatten_table(&name, nested, into),
            other => {
                into.insert(name, toml_value_to_string(other));
            }
        }
    }
}

fn toml_value_to_string(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Array(items) => items
            .iter()
            .map(toml_value_to_string)
            .collect::<Vec<_>>()
            .join(","),
        toml::Value::Table(_) => String::new(),
        other => other.to_string(),
    }
}

fn load_ignore_patterns(crate_dir: &Path) -> Vec<Regex> {
    find_upward(crate_dir, ARCHUNIT_IGNORE_PATTERNS_FILE_NAME)
        .map(|file| read_ignore_patterns(&file))
        .unwrap_or_default()
}

fn read_ignore_patterns(file: &Path) -> Vec<Regex> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim_end)
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .filter_map(|line| {
            Regex::new(&format!("^(?:{line})$"))
                .map_err(|e| {
                    eprintln!(
                        "archunit: ignoring invalid pattern '{line}' in {}: {e}",
                        file.display()
                    )
                })
                .ok()
        })
        .collect()
}
