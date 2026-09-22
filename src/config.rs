//! Global configuration (`ArchConfiguration` / `archunit.properties`).
//!
//! Phase 2 provides the programmatic API and the settings the lang layer needs:
//! `fail_on_empty_should`, ignore patterns from `archunit_ignore_patterns.txt`, and the
//! failure display format. Loading `archunit.toml` and environment overrides follows in
//! Phase 4.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, RwLock};

use regex::Regex;

use crate::lang::FailureDisplayFormat;

/// The file, relative to the crate root, listing regexes of violations to ignore.
pub const ARCHUNIT_IGNORE_PATTERNS_FILE_NAME: &str = "archunit_ignore_patterns.txt";

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
    fail_on_empty_should: bool,
    ignore_patterns: Option<Vec<Regex>>,
    failure_display_format: Option<Arc<dyn FailureDisplayFormat>>,
    properties: std::collections::HashMap<String, String>,
}

impl std::fmt::Debug for ArchConfiguration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchConfiguration")
            .field("fail_on_empty_should", &self.fail_on_empty_should)
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
    fn default() -> Self {
        Self {
            fail_on_empty_should: true,
            ignore_patterns: None,
            failure_display_format: None,
            properties: std::collections::HashMap::new(),
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

    /// Whether rules fail when no items reach the `should` clause (`archRule.failOnEmptyShould`).
    pub fn fail_on_empty_should(&self) -> bool {
        self.fail_on_empty_should
    }

    /// Sets [`fail_on_empty_should`](Self::fail_on_empty_should).
    pub fn set_fail_on_empty_should(value: bool) {
        Self::update(|c| c.fail_on_empty_should = value);
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

    /// A free-form property (`ArchConfiguration.getProperty(..)`), e.g. `cycles.max_number_to_detect`.
    pub fn property(&self, name: &str) -> Option<String> {
        self.properties.get(name).cloned()
    }

    /// `getPropertyOrDefault(..)`.
    pub fn property_or_default(&self, name: &str, default: &str) -> String {
        self.property(name).unwrap_or_else(|| default.to_owned())
    }

    /// `containsProperty(..)`.
    pub fn contains_property(&self, name: &str) -> bool {
        self.properties.contains_key(name)
    }

    /// `setProperty(..)`.
    pub fn set_property(name: &str, value: &str) {
        let (name, value) = (name.to_owned(), value.to_owned());
        Self::update(|c| {
            c.properties.insert(name, value);
        });
    }

    /// Properties whose keys start with `prefix.`, with the prefix removed (`getSubProperties(..)`).
    pub fn sub_properties(&self, prefix: &str) -> std::collections::HashMap<String, String> {
        let prefix = format!("{prefix}.");
        self.properties
            .iter()
            .filter_map(|(k, v)| {
                k.strip_prefix(&prefix)
                    .map(|rest| (rest.to_owned(), v.clone()))
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
}

fn load_ignore_patterns(crate_dir: &Path) -> Vec<Regex> {
    let mut dir: Option<PathBuf> = Some(crate_dir.to_path_buf());
    while let Some(current) = dir {
        let candidate = current.join(ARCHUNIT_IGNORE_PATTERNS_FILE_NAME);
        if candidate.is_file() {
            return read_ignore_patterns(&candidate);
        }
        if current.join("Cargo.toml").is_file()
            && std::fs::read_to_string(current.join("Cargo.toml"))
                .is_ok_and(|t| t.contains("[workspace]"))
        {
            break;
        }
        dir = current.parent().map(Path::to_path_buf);
    }
    Vec::new()
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
