//! Test integration (`archunit-junit5` → `archunit::harness`): the `#[analyze_classes]`,
//! `#[arch_test]`, `#[arch_rules]`, `#[arch_ignore]` and `#[arch_tag]` attributes, the plain
//! [`analyze_classes()`] builder with the same import cache, [`ArchTests`] and [`CacheMode`].
//!
//! ```ignore
//! use archunit::harness::{analyze_classes, arch_test};
//! use archunit::prelude::*;
//!
//! #[analyze_classes(packages = ["my_app.."], import_options = [DoNotIncludeTests])]
//! mod architecture {
//!     use super::*;
//!
//!     #[arch_test]
//!     fn services_should_not_access_controllers() -> impl ArchRule {
//!         no_classes().that().reside_in_a_package("..service..")
//!             .should().access_classes_that().reside_in_a_package("..controller..")
//!     }
//! }
//! ```
//!
//! Without the macros:
//!
//! ```no_run
//! use archunit::harness::analyze_classes;
//! use archunit::prelude::*;
//!
//! let items = analyze_classes().packages(&["my_app.."]).import();
//! classes().should().be_public().check(&items);
//! ```

use std::any::type_name;
use std::collections::HashMap;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

pub use archunit_macros::{analyze_classes, arch_ignore, arch_rules, arch_tag, arch_test};

use crate::base::DescribedPredicate;
use crate::core::domain::{PackageMatcher, RustItem, RustItems};
use crate::core::importer::{CrateImporter, ImportOption, Location, current_crate_dir};
use crate::lang::ArchRule;

/// How imported items are cached between tests (`CacheMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CacheMode {
    /// Cache the import for the whole test process, shared by every test module with the same
    /// configuration (`FOREVER`, the default).
    #[default]
    Forever,
    /// Keep the import only for the tests of one module (`PER_CLASS`).
    PerClass,
}

/// Supplies locations to import for a test module (`LocationProvider`).
///
/// Named in `#[analyze_classes(locations = [MyProvider])]`; the type must implement `Default`.
pub trait LocationProvider: Send + Sync {
    /// The locations to import for the given test module (`module_path!()` of the module).
    fn get(&self, test_module: &str) -> Vec<Location>;
}

impl<F> LocationProvider for F
where
    F: Fn(&str) -> Vec<Location> + Send + Sync,
{
    fn get(&self, test_module: &str) -> Vec<Location> {
        self(test_module)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    crate_dir: PathBuf,
    packages: Vec<String>,
    packages_of: Vec<String>,
    items: Vec<String>,
    locations: Vec<PathBuf>,
    whole_workspace: bool,
    import_options: Vec<String>,
}

static CACHE: LazyLock<Mutex<HashMap<CacheKey, Arc<RustItems>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Drops every cached import (`ClassCache.clear()`) `[rust-only]`.
pub fn clear_cache() {
    CACHE.lock().expect("class cache lock").clear();
}

/// The number of cached imports `[rust-only]`.
pub fn cached_imports() -> usize {
    CACHE.lock().expect("class cache lock").len()
}

/// The plain-function form of `#[analyze_classes(..)]`: `analyze_classes().packages(..).import()`.
pub fn analyze_classes() -> AnalyzeClasses {
    AnalyzeClasses::default()
}

/// Builder for an import configuration (`@AnalyzeClasses`).
#[derive(Clone, Default)]
pub struct AnalyzeClasses {
    test_module: String,
    crate_dir: Option<PathBuf>,
    packages: Vec<String>,
    packages_of: Vec<String>,
    items: Vec<String>,
    locations: Vec<Arc<dyn LocationProvider>>,
    whole_workspace: bool,
    import_options: Vec<(String, Arc<dyn ImportOption>)>,
    cache_mode: CacheMode,
}

impl fmt::Debug for AnalyzeClasses {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnalyzeClasses")
            .field("test_module", &self.test_module)
            .field("crate_dir", &self.crate_dir)
            .field("packages", &self.packages)
            .field("packages_of", &self.packages_of)
            .field("items", &self.items)
            .field("locations", &self.locations.len())
            .field("whole_workspace", &self.whole_workspace)
            .field(
                "import_options",
                &self
                    .import_options
                    .iter()
                    .map(|(n, _)| n)
                    .collect::<Vec<_>>(),
            )
            .field("cache_mode", &self.cache_mode)
            .finish()
    }
}

impl AnalyzeClasses {
    /// The module the tests live in, passed to [`LocationProvider::get`]; the macro passes
    /// `module_path!()`.
    pub fn for_test_module(mut self, test_module: &str) -> Self {
        self.test_module = test_module.to_owned();
        self
    }

    /// The crate to analyze; defaults to `CARGO_MANIFEST_DIR` `[rust-only]`.
    pub fn crate_dir(mut self, crate_dir: impl AsRef<Path>) -> Self {
        self.crate_dir = Some(crate_dir.as_ref().to_path_buf());
        self
    }

    /// Only items in these modules (`packages`).
    pub fn packages(mut self, package_identifiers: &[&str]) -> Self {
        self.packages
            .extend(package_identifiers.iter().map(|p| (*p).to_owned()));
        self
    }

    /// Only items in the modules of the named items (`packagesOf`).
    pub fn packages_of(mut self, item_names: &[&str]) -> Self {
        self.packages_of
            .extend(item_names.iter().map(|p| (*p).to_owned()));
        self
    }

    /// Only the named items (`classes`).
    pub fn items(mut self, item_names: &[&str]) -> Self {
        self.items
            .extend(item_names.iter().map(|p| (*p).to_owned()));
        self
    }

    /// Items from the locations of a [`LocationProvider`] (`locations`).
    pub fn locations(mut self, provider: impl LocationProvider + 'static) -> Self {
        self.locations.push(Arc::new(provider));
        self
    }

    /// Every crate of the workspace, ignoring the other restrictions (`wholeClasspath`).
    pub fn whole_workspace(mut self, whole_workspace: bool) -> Self {
        self.whole_workspace = whole_workspace;
        self
    }

    /// Adds an [`ImportOption`] (`importOptions`).
    pub fn import_option<O: ImportOption + 'static>(mut self, option: O) -> Self {
        self.import_options
            .push((type_name::<O>().to_owned(), Arc::new(option)));
        self
    }

    /// `cacheMode`.
    pub fn cache_mode(mut self, cache_mode: CacheMode) -> Self {
        self.cache_mode = cache_mode;
        self
    }

    fn key(&self, crate_dir: &Path, locations: &[Location]) -> CacheKey {
        let mut location_paths: Vec<PathBuf> =
            locations.iter().map(|l| l.path().to_path_buf()).collect();
        location_paths.sort();
        location_paths.dedup();
        CacheKey {
            crate_dir: crate_dir.to_path_buf(),
            packages: self.packages.clone(),
            packages_of: self.packages_of.clone(),
            items: self.items.clone(),
            locations: location_paths,
            whole_workspace: self.whole_workspace,
            import_options: self.import_options.iter().map(|(n, _)| n.clone()).collect(),
        }
    }

    /// Imports the items, from the cache when [`CacheMode::Forever`] is set and the same
    /// configuration was imported before.
    ///
    /// # Panics
    /// If the import fails.
    pub fn import(&self) -> Arc<RustItems> {
        let crate_dir = self.crate_dir.clone().unwrap_or_else(current_crate_dir);
        let locations: Vec<Location> = self
            .locations
            .iter()
            .flat_map(|p| p.get(&self.test_module))
            .collect();
        let key = self.key(&crate_dir, &locations);
        if self.cache_mode == CacheMode::Forever
            && let Some(cached) = CACHE.lock().expect("class cache lock").get(&key)
        {
            return Arc::clone(cached);
        }
        let items = Arc::new(self.import_uncached(&crate_dir, &locations));
        if self.cache_mode == CacheMode::Forever {
            CACHE
                .lock()
                .expect("class cache lock")
                .insert(key, Arc::clone(&items));
        }
        items
    }

    fn import_uncached(&self, crate_dir: &Path, locations: &[Location]) -> RustItems {
        let mut importer = CrateImporter::new();
        for (_, option) in &self.import_options {
            let option = Arc::clone(option);
            importer = importer.with_import_option(move |l: &Location| option.includes(l));
        }
        // Declared locations replace the crate under test as the import root (Java: the union
        // of all declared locations; the classpath only when nothing is declared).
        let mut dirs: Vec<PathBuf> = if locations.is_empty() {
            vec![crate_dir.to_path_buf()]
        } else {
            locations
                .iter()
                .map(|l| l.crate_dir().to_path_buf())
                .collect()
        };
        dirs.sort();
        dirs.dedup();
        let restricted_by_name =
            !self.packages.is_empty() || !self.packages_of.is_empty() || !self.items.is_empty();
        if self.whole_workspace {
            return importer.import_paths(&dirs);
        }
        if !restricted_by_name {
            let roots: Vec<PathBuf> = if locations.is_empty() {
                dirs.clone()
            } else {
                locations.iter().map(|l| l.path().to_path_buf()).collect()
            };
            importer = importer.with_import_option(move |l: &Location| {
                roots.iter().any(|root| l.path().starts_with(root))
            });
            return importer.import_paths(&dirs);
        }
        let all = importer.import_paths(&dirs);
        let matchers: Vec<PackageMatcher> = self
            .packages
            .iter()
            .map(|p| PackageMatcher::of(p))
            .collect();
        let mut packages_of: Vec<String> = Vec::new();
        for name in &self.packages_of {
            match all.try_get(name) {
                Some(item) => packages_of.push(item.package_name()),
                None => panic!("No item named '{name}' found in {}", crate_dir.display()),
            }
        }
        let names = self.items.clone();
        let predicate =
            DescribedPredicate::describe("in analyzed scope", move |item: &RustItem| {
                let package = item.package_name();
                matchers.iter().any(|m| m.matches(&package))
                    || packages_of.contains(&package)
                    || names.iter().any(|n| item.is_equivalent_to(n))
            });
        all.that(&predicate).as_("classes")
    }
}

// ---- ArchTests -----------------------------------------------------------------------------

/// Converts the value of an `#[arch_test(items = provider)]` provider into shared items.
pub trait IntoItems {
    /// The items.
    fn into_items(self) -> Arc<RustItems>;
}

impl IntoItems for Arc<RustItems> {
    fn into_items(self) -> Arc<RustItems> {
        self
    }
}

impl IntoItems for RustItems {
    fn into_items(self) -> Arc<RustItems> {
        Arc::new(self)
    }
}

impl IntoItems for &RustItems {
    fn into_items(self) -> Arc<RustItems> {
        Arc::new(self.clone())
    }
}

/// What an `#[arch_test]` function may return: an [`ArchRule`] or [`ArchTests`].
pub trait ArchTestRunnable {
    /// Checks the rule(s) against `items`, panicking with the failure report(s).
    fn run_arch_test(&self, name: &str, items: &RustItems);
}

impl<R: ArchRule + ?Sized> ArchTestRunnable for R {
    fn run_arch_test(&self, _name: &str, items: &RustItems) {
        self.check(items);
    }
}

impl ArchTestRunnable for ArchTests {
    fn run_arch_test(&self, _name: &str, items: &RustItems) {
        self.check(items);
    }
}

type CaseFn = fn(&RustItems);

/// One rule of an `#[arch_rules]` module.
#[derive(Clone)]
pub struct ArchTestCase {
    name: String,
    run: CaseFn,
    ignored: Option<String>,
    tags: Vec<String>,
}

impl fmt::Debug for ArchTestCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArchTestCase")
            .field("name", &self.name)
            .field("ignored", &self.ignored)
            .field("tags", &self.tags)
            .finish()
    }
}

impl ArchTestCase {
    /// A case running `run` against the items.
    pub fn new(name: &str, run: CaseFn) -> Self {
        Self {
            name: name.to_owned(),
            run,
            ignored: None,
            tags: Vec::new(),
        }
    }

    /// Marks the case ignored (`@ArchIgnore`).
    pub fn ignored(mut self, reason: &str) -> Self {
        self.ignored = Some(reason.to_owned());
        self
    }

    /// Tags the case (`@ArchTag`).
    pub fn tagged(mut self, tags: &[&str]) -> Self {
        self.tags.extend(tags.iter().map(|t| (*t).to_owned()));
        self
    }

    /// The name of the rule function.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The ignore reason, if the case is ignored.
    pub fn ignore_reason(&self) -> Option<&str> {
        self.ignored.as_deref()
    }

    /// Whether the case is ignored.
    pub fn is_ignored(&self) -> bool {
        self.ignored.is_some()
    }

    /// The tags.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Runs the case, returning the panic message on failure.
    pub fn run(&self, items: &RustItems) -> Result<(), String> {
        let run = self.run;
        catch_unwind(AssertUnwindSafe(|| run(items))).map_err(|payload| {
            payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_else(|| "test panicked".to_owned())
        })
    }
}

/// A set of rules defined in an `#[arch_rules]` module, included in a test module with
/// `ArchTests::in_(other_rules::arch_tests)` (`ArchTests.in(OtherRules.class)`).
///
/// Every case is evaluated; the failure reports of all violated rules are combined into one
/// panic, since `cargo test` cannot create tests dynamically.
#[derive(Debug, Clone)]
pub struct ArchTests {
    definition_location: String,
    cases: Vec<ArchTestCase>,
}

impl ArchTests {
    /// `ArchTests.in(OtherRules.class)`: the rules of a module annotated with `#[arch_rules]`.
    pub fn in_(definition: impl FnOnce() -> ArchTests) -> ArchTests {
        definition()
    }

    /// The tests generated by `#[arch_rules]`.
    pub fn of(definition_location: &str, cases: Vec<ArchTestCase>) -> Self {
        Self {
            definition_location: definition_location.to_owned(),
            cases,
        }
    }

    /// The module path of the `#[arch_rules]` module (`getDefinitionLocation()`).
    pub fn definition_location(&self) -> &str {
        &self.definition_location
    }

    /// The cases.
    pub fn cases(&self) -> &[ArchTestCase] {
        &self.cases
    }

    /// Runs every case that is not ignored and returns the failures as `(name, message)`.
    pub fn evaluate(&self, items: &RustItems) -> Vec<(String, String)> {
        self.cases
            .iter()
            .filter(|c| !c.is_ignored())
            .filter_map(|c| c.run(items).err().map(|m| (c.name.clone(), m)))
            .collect()
    }

    /// Runs every case and panics with the combined failure messages.
    pub fn check(&self, items: &RustItems) {
        let failures = self.evaluate(items);
        if failures.is_empty() {
            return;
        }
        let total = self.cases.iter().filter(|c| !c.is_ignored()).count();
        let mut message = format!(
            "{} of {} arch tests in `{}` failed:\n",
            failures.len(),
            total,
            self.definition_location
        );
        for (name, failure) in failures {
            message.push_str(&format!("\n[{name}]\n{failure}\n"));
        }
        panic!("{message}");
    }
}
