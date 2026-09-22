//! Freezing rules (`com.tngtech.archunit.library.freeze`): record the current violations of
//! a rule and fail only on new ones, so an architecture can be tightened over time.
//!
//! ```no_run
//! use archunit::library::freeze::FreezingArchRule;
//! use archunit::prelude::*;
//! # let items = archunit::core::importer::CrateImporter::new().import_path("src");
//!
//! FreezingArchRule::freeze(
//!     no_classes().that().reside_in_a_package("..service..")
//!         .should().access_classes_that().reside_in_a_package("..controller.."),
//! )
//! .check(&items);
//! ```
//!
//! The default [`TextFileBasedViolationStore`] keeps a `stored.rules` index plus one file per
//! rule under `archunit_store`, in the same format as Java ArchUnit. Configure it in
//! `archunit.toml`:
//!
//! ```toml
//! [freeze]
//! refreeze = false
//! [freeze.store.default]
//! path = "archunit_store"
//! allow_store_creation = true
//! allow_store_update = true
//! ```

mod properties;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::hash::{BuildHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use crate::base::HasDescription;
use crate::config::ArchConfiguration;
use crate::core::domain::RustItems;
use crate::lang::{ArchRule, EvaluationResult};

/// `freeze.store`: the prefix of the store properties.
pub const FREEZE_STORE_PROPERTY_NAME: &str = "freeze.store";

/// `freeze.refreeze`: when `true`, every evaluation overwrites the stored violations.
pub const FREEZE_REFREEZE_PROPERTY_NAME: &str = "freeze.refreeze";

/// Errors of the freeze infrastructure (`StoreInitializationFailedException`,
/// `StoreReadException`, `StoreUpdateFailedException`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FreezeError {
    /// The store could not be initialized.
    #[error("{0}")]
    StoreInitializationFailed(String),
    /// The store could not be read.
    #[error("{0}")]
    StoreRead(String),
    /// The store could not be updated.
    #[error("{0}")]
    StoreUpdateFailed(String),
}

// ---- ViolationStore ------------------------------------------------------------------------

/// Where frozen violations are kept (`ViolationStore`).
///
/// Errors are returned as [`FreezeError`]; [`FreezingArchRule`] panics with them, as
/// `ArchRule::evaluate` cannot fail.
pub trait ViolationStore: Send {
    /// Configures the store with the `freeze.store.*` properties, keys without the prefix
    /// (`initialize(Properties)`).
    fn initialize(&mut self, properties: &HashMap<String, String>) -> Result<(), FreezeError>;

    /// Whether violations of `rule` were stored before (`contains(rule)`).
    fn contains(&self, rule: &dyn ArchRule) -> bool;

    /// Stores the violations of `rule`, replacing older ones (`save(rule, violations)`).
    fn save(&mut self, rule: &dyn ArchRule, violations: Vec<String>) -> Result<(), FreezeError>;

    /// The stored violations of `rule` (`getViolations(rule)`).
    fn violations(&self, rule: &dyn ArchRule) -> Result<Vec<String>, FreezeError>;
}

/// A store keeping everything in memory `[rust-only]`; handy for tests.
#[derive(Debug, Default, Clone)]
pub struct InMemoryViolationStore {
    rules: BTreeMap<String, Vec<String>>,
    properties: HashMap<String, String>,
}

impl InMemoryViolationStore {
    /// The properties passed to `initialize`.
    pub fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }

    /// The stored rules with their violations.
    pub fn stored_rules(&self) -> &BTreeMap<String, Vec<String>> {
        &self.rules
    }
}

impl ViolationStore for InMemoryViolationStore {
    fn initialize(&mut self, properties: &HashMap<String, String>) -> Result<(), FreezeError> {
        self.properties = properties.clone();
        Ok(())
    }

    fn contains(&self, rule: &dyn ArchRule) -> bool {
        self.rules.contains_key(&rule.description())
    }

    fn save(&mut self, rule: &dyn ArchRule, violations: Vec<String>) -> Result<(), FreezeError> {
        self.rules.insert(rule.description(), violations);
        Ok(())
    }

    fn violations(&self, rule: &dyn ArchRule) -> Result<Vec<String>, FreezeError> {
        self.rules.get(&rule.description()).cloned().ok_or_else(|| {
            FreezeError::StoreRead(format!(
                "No rule stored with description '{}'",
                rule.description()
            ))
        })
    }
}

// ---- TextFileBasedViolationStore -----------------------------------------------------------

const STORE_PATH_PROPERTY_NAME: &str = "default.path";
const STORE_PATH_DEFAULT: &str = "archunit_store";
const STORED_RULES_FILE_NAME: &str = "stored.rules";
const ALLOW_STORE_CREATION_PROPERTY_NAME: &str = "default.allow_store_creation";
const ALLOW_STORE_UPDATE_PROPERTY_NAME: &str = "default.allow_store_update";

/// Chooses the file name for a rule's violations (`RuleViolationFileNameStrategy`).
pub type RuleViolationFileNameStrategy = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// The index of stored rules of one store folder, shared by every store using that folder.
#[derive(Debug)]
struct StoredRules {
    file: PathBuf,
    rules: BTreeMap<String, String>,
}

impl StoredRules {
    fn load(file: &Path) -> Result<Self, FreezeError> {
        let text = std::fs::read_to_string(file)
            .map_err(|e| FreezeError::StoreInitializationFailed(e.to_string()))?;
        Ok(Self {
            file: file.to_path_buf(),
            rules: properties::load(&text),
        })
    }

    fn put_if_absent(&mut self, key: &str, value: &str) -> Result<Option<String>, FreezeError> {
        if let Some(existing) = self.rules.get(key) {
            return Ok(Some(existing.clone()));
        }
        self.rules.insert(key.to_owned(), value.to_owned());
        std::fs::write(&self.file, properties::store(&self.rules))
            .map_err(|e| FreezeError::StoreUpdateFailed(e.to_string()))?;
        Ok(None)
    }
}

static STORED_RULES_BY_PATH: LazyLock<Mutex<HashMap<PathBuf, Arc<Mutex<StoredRules>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The default store: a `stored.rules` index (Java `.properties` format) mapping rule
/// descriptions to files holding one violation per line (`TextFileBasedViolationStore`).
#[derive(Clone)]
pub struct TextFileBasedViolationStore {
    file_name_strategy: RuleViolationFileNameStrategy,
    store_creation_allowed: bool,
    store_update_allowed: bool,
    store_folder: PathBuf,
    stored_rules: Option<Arc<Mutex<StoredRules>>>,
}

impl fmt::Debug for TextFileBasedViolationStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextFileBasedViolationStore")
            .field("store_folder", &self.store_folder)
            .field("store_creation_allowed", &self.store_creation_allowed)
            .field("store_update_allowed", &self.store_update_allowed)
            .finish()
    }
}

impl Default for TextFileBasedViolationStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TextFileBasedViolationStore {
    /// A store naming rule files by a random UUID.
    pub fn new() -> Self {
        Self::with_file_name_strategy(Arc::new(|_| random_uuid()))
    }

    /// A store naming rule files by `strategy(rule_description)`.
    pub fn with_file_name_strategy(strategy: RuleViolationFileNameStrategy) -> Self {
        Self {
            file_name_strategy: strategy,
            store_creation_allowed: false,
            store_update_allowed: true,
            store_folder: PathBuf::from(STORE_PATH_DEFAULT),
            stored_rules: None,
        }
    }

    /// The folder of the store.
    pub fn store_folder(&self) -> &Path {
        &self.store_folder
    }

    fn stored_rules(&self) -> Result<&Arc<Mutex<StoredRules>>, FreezeError> {
        self.stored_rules.as_ref().ok_or_else(|| {
            FreezeError::StoreInitializationFailed("Store has not been initialized".to_owned())
        })
    }

    fn ensure_rule_file_name(&self, rule: &dyn ArchRule) -> Result<String, FreezeError> {
        let description = ensure_unix_line_breaks(&rule.description());
        let candidate = (self.file_name_strategy)(&description);
        let existing = self
            .stored_rules()?
            .lock()
            .expect("stored rules lock")
            .put_if_absent(&description, &candidate)?;
        Ok(existing.unwrap_or(candidate))
    }
}

fn property<'a>(properties: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    properties.get(name).map(String::as_str).or_else(|| {
        // Java spelling of the key, for shared configuration.
        let camel = camel_case(name);
        properties.get(&camel).map(String::as_str)
    })
}

fn camel_case(name: &str) -> String {
    let mut result = String::new();
    let mut upper_next = false;
    for c in name.chars() {
        if c == '_' {
            upper_next = true;
        } else if upper_next {
            result.extend(c.to_uppercase());
            upper_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

impl ViolationStore for TextFileBasedViolationStore {
    fn initialize(&mut self, properties: &HashMap<String, String>) -> Result<(), FreezeError> {
        self.store_creation_allowed = property(properties, ALLOW_STORE_CREATION_PROPERTY_NAME)
            .is_some_and(|v| v.trim() == "true");
        self.store_update_allowed = property(properties, ALLOW_STORE_UPDATE_PROPERTY_NAME)
            .is_none_or(|v| v.trim() == "true");
        self.store_folder = PathBuf::from(
            property(properties, STORE_PATH_PROPERTY_NAME).unwrap_or(STORE_PATH_DEFAULT),
        );
        let rules_file = self.store_folder.join(STORED_RULES_FILE_NAME);
        if !rules_file.exists() && !self.store_creation_allowed {
            return Err(FreezeError::StoreInitializationFailed(format!(
                "Creating new violation store is disabled (enable by configuration {FREEZE_STORE_PROPERTY_NAME}.{ALLOW_STORE_CREATION_PROPERTY_NAME}=true)"
            )));
        }
        if !self.store_folder.is_dir() {
            std::fs::create_dir_all(&self.store_folder).map_err(|e| {
                FreezeError::StoreInitializationFailed(format!(
                    "Cannot create rule store at {} ({e})",
                    rules_file.display()
                ))
            })?;
        }
        if !rules_file.exists() {
            std::fs::write(&rules_file, "").map_err(|e| {
                FreezeError::StoreInitializationFailed(format!(
                    "Cannot create rule store at {} ({e})",
                    rules_file.display()
                ))
            })?;
        }
        let key = rules_file
            .canonicalize()
            .map_err(|e| FreezeError::StoreInitializationFailed(e.to_string()))?;
        let mut by_path = STORED_RULES_BY_PATH.lock().expect("stored rules registry");
        let stored_rules = match by_path.get(&key) {
            Some(existing) => Arc::clone(existing),
            None => {
                let loaded = Arc::new(Mutex::new(StoredRules::load(&rules_file)?));
                by_path.insert(key, Arc::clone(&loaded));
                loaded
            }
        };
        self.stored_rules = Some(stored_rules);
        Ok(())
    }

    fn contains(&self, rule: &dyn ArchRule) -> bool {
        self.stored_rules().is_ok_and(|rules| {
            rules
                .lock()
                .expect("stored rules lock")
                .rules
                .contains_key(&ensure_unix_line_breaks(&rule.description()))
        })
    }

    fn save(&mut self, rule: &dyn ArchRule, violations: Vec<String>) -> Result<(), FreezeError> {
        if !self.store_update_allowed {
            return Err(FreezeError::StoreUpdateFailed(format!(
                "Updating frozen violations is disabled (enable by configuration {FREEZE_STORE_PROPERTY_NAME}.{ALLOW_STORE_UPDATE_PROPERTY_NAME}=true)"
            )));
        }
        let file_name = self.ensure_rule_file_name(rule)?;
        let mut text = String::new();
        for violation in &violations {
            text.push_str(&violation.replace('\n', "\\\n"));
            text.push('\n');
        }
        std::fs::write(self.store_folder.join(file_name), text)
            .map_err(|e| FreezeError::StoreUpdateFailed(e.to_string()))
    }

    fn violations(&self, rule: &dyn ArchRule) -> Result<Vec<String>, FreezeError> {
        let description = ensure_unix_line_breaks(&rule.description());
        let file_name = self
            .stored_rules()?
            .lock()
            .expect("stored rules lock")
            .rules
            .get(&description)
            .cloned()
            .ok_or_else(|| {
                FreezeError::StoreRead(format!("No rule stored with description '{description}'"))
            })?;
        let text = std::fs::read_to_string(self.store_folder.join(file_name))
            .map_err(|e| FreezeError::StoreRead(e.to_string()))?;
        Ok(split_unescaped_lines(&ensure_unix_line_breaks(&text)))
    }
}

/// Splits on line breaks that are not escaped by a backslash and unescapes the rest.
fn split_unescaped_lines(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'\n') {
            chars.next();
            current.push('\n');
        } else if c == '\n' {
            if !current.is_empty() {
                result.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn ensure_unix_line_breaks(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// A random UUID (version 4 layout) without an external dependency.
fn random_uuid() -> String {
    static COUNTER: Mutex<u64> = Mutex::new(0);
    let mut counter = COUNTER.lock().expect("uuid counter");
    *counter += 1;
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    std::time::SystemTime::now().hash(&mut hasher);
    counter.hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    let a = hasher.finish();
    std::process::id().hash(&mut hasher);
    let b = hasher.finish();
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        a >> 32,
        (a >> 16) & 0xffff,
        a & 0xfff,
        ((b >> 48) & 0x3fff) | 0x8000,
        b & 0xffff_ffff_ffff
    )
}

// ---- ViolationLineMatcher ------------------------------------------------------------------

/// Decides whether a current violation line corresponds to a stored one (`ViolationLineMatcher`).
pub trait ViolationLineMatcher: Send + Sync {
    /// Whether the two violation lines describe the same violation.
    fn matches(&self, line_from_first_violation: &str, line_from_second_violation: &str) -> bool;
}

impl<F> ViolationLineMatcher for F
where
    F: Fn(&str, &str) -> bool + Send + Sync,
{
    fn matches(&self, first: &str, second: &str) -> bool {
        self(first, second)
    }
}

/// The default matcher: compares lines ignoring line numbers (`:14)`) and auto-generated
/// numbers after `$` (`FuzzyViolationLineMatcher`).
#[derive(Debug, Clone, Copy, Default)]
pub struct FuzzyViolationLineMatcher;

impl ViolationLineMatcher for FuzzyViolationLineMatcher {
    fn matches(&self, first: &str, second: &str) -> bool {
        let mut a = RelevantParts::new(first);
        let mut b = RelevantParts::new(second);
        loop {
            match (a.next(), b.next()) {
                (Some(x), Some(y)) if x == y => continue,
                (None, None) => return true,
                _ => return false,
            }
        }
    }
}

/// Iterates over the parts of a line that matter, skipping `:<digits>)` and `$<digits>`.
struct RelevantParts<'a> {
    text: &'a str,
    position: usize,
}

impl<'a> RelevantParts<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, position: 0 }
    }
}

impl<'a> Iterator for RelevantParts<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.position >= self.text.len() {
            return None;
        }
        let rest = &self.text[self.position..];
        let end = rest.find([':', '$']).map(|i| i + 1).unwrap_or(rest.len());
        let part = &rest[..end];
        let after = &rest[end..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        let skipped = if part.ends_with(':') {
            if digits > 0 && after.as_bytes().get(digits) == Some(&b')') {
                digits + 1
            } else {
                0
            }
        } else if part.ends_with('$') {
            digits
        } else {
            0
        };
        self.position += end + skipped;
        Some(part)
    }
}

// ---- FreezingArchRule ----------------------------------------------------------------------

/// A rule whose known violations are stored and tolerated (`FreezingArchRule`).
pub struct FreezingArchRule {
    delegate: Box<dyn ArchRule>,
    store: Arc<Mutex<Box<dyn ViolationStore>>>,
    matcher: Arc<dyn ViolationLineMatcher>,
}

impl Clone for FreezingArchRule {
    fn clone(&self) -> Self {
        Self {
            delegate: self.delegate.clone_boxed(),
            store: Arc::clone(&self.store),
            matcher: Arc::clone(&self.matcher),
        }
    }
}

impl fmt::Debug for FreezingArchRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FreezingArchRule{{{}}}", self.delegate.description())
    }
}

/// `FreezingArchRule.freeze(rule)`.
pub fn freeze(rule: impl ArchRule + 'static) -> FreezingArchRule {
    FreezingArchRule::freeze(rule)
}

impl FreezingArchRule {
    /// Freezes `rule` with the configured store and line matcher (`freeze(rule)`).
    pub fn freeze(rule: impl ArchRule + 'static) -> Self {
        let configuration = ArchConfiguration::get();
        let store: Box<dyn ViolationStore> = match configuration.violation_store_factory() {
            Some(factory) => factory(),
            None => Box::new(TextFileBasedViolationStore::new()),
        };
        let matcher: Arc<dyn ViolationLineMatcher> = configuration
            .violation_line_matcher()
            .unwrap_or_else(|| Arc::new(FuzzyViolationLineMatcher));
        Self {
            delegate: Box::new(rule),
            store: Arc::new(Mutex::new(store)),
            matcher,
        }
    }

    /// Uses `store` instead of the configured one (`persistIn(..)`).
    pub fn persist_in(self, store: impl ViolationStore + 'static) -> Self {
        Self {
            store: Arc::new(Mutex::new(Box::new(store))),
            ..self
        }
    }

    /// Uses `matcher` instead of the default (`associateViolationLinesVia(..)`).
    pub fn associate_violation_lines_via(
        self,
        matcher: impl ViolationLineMatcher + 'static,
    ) -> Self {
        Self {
            matcher: Arc::new(matcher),
            ..self
        }
    }

    /// `because(..)`.
    pub fn because(self, reason: &str) -> Self {
        Self {
            delegate: self.delegate.because(reason),
            ..self
        }
    }

    /// `as(..)`.
    pub fn as_(self, description: &str) -> Self {
        Self {
            delegate: self.delegate.as_(description),
            ..self
        }
    }

    /// `allowEmptyShould(..)`.
    pub fn allow_empty_should(self, allow_empty_should: bool) -> Self {
        Self {
            delegate: self.delegate.allow_empty_should_boxed(allow_empty_should),
            ..self
        }
    }

    /// The description of the frozen rule.
    pub fn description(&self) -> String {
        self.delegate.description()
    }

    /// Evaluates the rule without failing (`evaluate(..)`).
    pub fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        ArchRule::evaluate(self, items)
    }

    /// Evaluates the rule and panics with the new violations (`check(..)`).
    pub fn check(&self, items: &RustItems) {
        ArchRule::check(self, items);
    }

    fn try_evaluate(&self, items: &RustItems) -> Result<EvaluationResult, FreezeError> {
        let configuration = ArchConfiguration::get();
        let mut store = self.store.lock().expect("violation store lock");
        store.initialize(&configuration.sub_properties(FREEZE_STORE_PROPERTY_NAME))?;
        let result = self.delegate.evaluate(items);
        let actual: Vec<String> = result
            .failure_report()
            .details()
            .iter()
            .map(|d| ensure_unix_line_breaks(d))
            .collect();
        let refreeze = configuration
            .property_or_default(FREEZE_REFREEZE_PROPERTY_NAME, "false")
            .trim()
            == "true";
        if !store.contains(self.delegate.as_ref()) || refreeze {
            store.save(self.delegate.as_ref(), actual)?;
            return Ok(EvaluationResult::empty(
                self.delegate.as_ref(),
                result.priority(),
            ));
        }
        let known: Vec<String> = store
            .violations(self.delegate.as_ref())?
            .iter()
            .map(|v| ensure_unix_line_breaks(v))
            .collect();
        let categorized = CategorizedViolations::new(self.matcher.as_ref(), &actual, &known);
        if !categorized.stored_solved.is_empty() {
            store.save(self.delegate.as_ref(), categorized.stored_unsolved.clone())?;
        }
        let known_actual = categorized.known_actual;
        Ok(result.filter_descriptions_matching(move |line| {
            !known_actual.contains(&ensure_unix_line_breaks(line))
        }))
    }
}

struct CategorizedViolations {
    known_actual: HashSet<String>,
    stored_solved: Vec<String>,
    stored_unsolved: Vec<String>,
}

impl CategorizedViolations {
    fn new(matcher: &dyn ViolationLineMatcher, actual: &[String], stored: &[String]) -> Self {
        let mut stored_left: Vec<String> = stored.to_vec();
        let mut known_actual = HashSet::new();
        let mut stored_unsolved = Vec::new();
        for actual_violation in actual {
            if let Some(index) = stored_left
                .iter()
                .position(|s| matcher.matches(actual_violation, s))
            {
                let matched = stored_left.remove(index);
                known_actual.insert(actual_violation.clone());
                stored_unsolved.push(matched);
            }
        }
        let stored_solved = stored
            .iter()
            .filter(|s| !stored_unsolved.contains(s))
            .cloned()
            .collect();
        Self {
            known_actual,
            stored_solved,
            stored_unsolved,
        }
    }
}

impl HasDescription for FreezingArchRule {
    fn description(&self) -> String {
        self.delegate.description()
    }
}

impl ArchRule for FreezingArchRule {
    fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        self.try_evaluate(items).unwrap_or_else(|e| panic!("{e}"))
    }

    fn clone_boxed(&self) -> Box<dyn ArchRule> {
        Box::new(self.clone())
    }

    fn allow_empty_should_boxed(&self, allow_empty_should: bool) -> Box<dyn ArchRule> {
        Box::new(self.clone().allow_empty_should(allow_empty_should))
    }
}
