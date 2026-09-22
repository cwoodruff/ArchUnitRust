//! Importing crates from source (`com.tngtech.archunit.core.importer`).
//!
//! [`CrateImporter`] plays the role of `ClassFileImporter`: it discovers crates with
//! `cargo metadata`, parses every source file with `syn`, resolves names across modules and
//! crates, and records members, accesses and dependencies into a [`RustItems`] graph.

mod build;
mod cargo;
mod location;
mod parse;
mod prelude;
mod resolve;

pub use location::{ImportOption, Location, import_option};

use std::path::{Path, PathBuf};

use crate::base::ArchUnitError;
use crate::core::domain::{PackageMatcher, RustItems};

/// Imports Rust crates into a [`RustItems`] graph (`ClassFileImporter`).
///
/// ```no_run
/// use archunit::core::importer::{CrateImporter, import_option::DoNotIncludeTests};
///
/// let items = CrateImporter::new()
///     .with_import_option(DoNotIncludeTests)
///     .import_path("path/to/my_crate");
/// ```
#[derive(Default)]
pub struct CrateImporter {
    options: Vec<Box<dyn ImportOption>>,
    resolve_dependencies_from_classpath: bool,
}

impl std::fmt::Debug for CrateImporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrateImporter")
            .field("options", &self.options.len())
            .field(
                "resolve_dependencies_from_classpath",
                &self.resolve_dependencies_from_classpath,
            )
            .finish()
    }
}

impl CrateImporter {
    /// A new importer with no options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an [`ImportOption`] (`withImportOption(..)`).
    pub fn with_import_option(mut self, option: impl ImportOption + 'static) -> Self {
        self.options.push(Box::new(option));
        self
    }

    /// Adds several [`ImportOption`]s (`withImportOptions(..)`).
    pub fn with_import_options(mut self, options: Vec<Box<dyn ImportOption>>) -> Self {
        self.options.extend(options);
        self
    }

    /// Whether to parse the source of dependency crates that are not workspace members,
    /// instead of stubbing them (`resolveMissingDependenciesFromClassPath`). Default: `false`.
    pub fn resolving_missing_dependencies_from_classpath(mut self, resolve: bool) -> Self {
        self.resolve_dependencies_from_classpath = resolve;
        self
    }

    pub(crate) fn options(&self) -> &[Box<dyn ImportOption>] {
        &self.options
    }

    /// Imports the crate or workspace at `path` (a directory or a `Cargo.toml`)
    /// (`importPath(..)`).
    ///
    /// # Panics
    /// If the import fails; use [`try_import_path`](Self::try_import_path) for a `Result`.
    pub fn import_path(&self, path: impl AsRef<Path>) -> RustItems {
        self.try_import_path(path).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Imports the crate or workspace at `path`.
    pub fn try_import_path(&self, path: impl AsRef<Path>) -> Result<RustItems, ArchUnitError> {
        self.try_import_paths(&[path.as_ref().to_path_buf()])
    }

    /// Imports several crates or workspaces into one graph (`importPaths(..)`).
    pub fn import_paths(&self, paths: &[impl AsRef<Path>]) -> RustItems {
        let paths: Vec<PathBuf> = paths.iter().map(|p| p.as_ref().to_path_buf()).collect();
        self.try_import_paths(&paths)
            .unwrap_or_else(|e| panic!("{e}"))
    }

    /// Imports several crates or workspaces into one graph.
    pub fn try_import_paths(&self, paths: &[PathBuf]) -> Result<RustItems, ArchUnitError> {
        let mut sources = Vec::new();
        for path in paths {
            sources.extend(cargo::discover(
                path,
                self.resolve_dependencies_from_classpath,
            )?);
        }
        build::build(self, sources, "classes")
    }

    /// Imports the workspace containing the current crate (`CARGO_MANIFEST_DIR` when run by
    /// cargo, else the working directory) (`importClasspath()`).
    pub fn import_workspace(&self) -> RustItems {
        self.try_import_workspace()
            .unwrap_or_else(|e| panic!("{e}"))
    }

    /// Imports the current workspace.
    pub fn try_import_workspace(&self) -> Result<RustItems, ArchUnitError> {
        self.try_import_path(current_crate_dir())
    }

    /// Imports the current workspace and keeps the items residing in the given modules
    /// (`importPackages(..)`). Identifiers follow the package identifier syntax.
    pub fn import_packages(&self, package_identifiers: &[&str]) -> RustItems {
        self.try_import_packages(package_identifiers)
            .unwrap_or_else(|e| panic!("{e}"))
    }

    /// Imports the current workspace and keeps the items residing in the given modules.
    pub fn try_import_packages(
        &self,
        package_identifiers: &[&str],
    ) -> Result<RustItems, ArchUnitError> {
        let matchers = package_identifiers
            .iter()
            .map(|id| PackageMatcher::try_of(id))
            .collect::<Result<Vec<_>, _>>()?;
        let all = self.try_import_workspace()?;
        let predicate = crate::base::DescribedPredicate::describe(
            "in requested packages",
            move |item: &crate::core::domain::RustItem| {
                let package = item.package_name();
                matchers.iter().any(|m| m.matches(&package))
            },
        );
        Ok(all.that(&predicate).as_("classes"))
    }

    /// `importPackage(..)`.
    pub fn import_package(&self, package_identifier: &str) -> RustItems {
        self.import_packages(&[package_identifier])
    }

    /// Imports the modules containing the named items (`importPackagesOf(..)`).
    pub fn import_packages_of(&self, item_names: &[&str]) -> RustItems {
        let all = self.import_workspace();
        let packages: Vec<String> = item_names
            .iter()
            .map(|name| {
                all.try_get_any(name)
                    .map(|i| i.package_name())
                    .unwrap_or_else(|| panic!("classes do not contain {name}"))
            })
            .collect();
        let predicate = crate::base::DescribedPredicate::describe(
            "in requested packages",
            move |item: &crate::core::domain::RustItem| packages.contains(&item.package_name()),
        );
        all.that(&predicate).as_("classes")
    }

    /// Imports the current workspace and keeps only the named items (`importClasses(..)`).
    pub fn import_items(&self, item_names: &[&str]) -> RustItems {
        let all = self.import_workspace();
        let names: Vec<String> = item_names.iter().map(|s| (*s).to_owned()).collect();
        let predicate = crate::base::DescribedPredicate::describe(
            "requested classes",
            move |item: &crate::core::domain::RustItem| names.contains(&item.name()),
        );
        all.that(&predicate).as_("classes")
    }

    /// `importClass(..)`.
    pub fn import_item(&self, item_name: &str) -> RustItems {
        self.import_items(&[item_name])
    }

    /// Imports the crates owning the given locations (`importLocations(..)`).
    pub fn import_locations(&self, locations: &[Location]) -> RustItems {
        let mut paths: Vec<PathBuf> = locations
            .iter()
            .map(|l| l.crate_dir().to_path_buf())
            .collect();
        paths.sort();
        paths.dedup();
        self.import_paths(&paths)
    }
}

/// The directory of the crate under test: `CARGO_MANIFEST_DIR` if set, else the working directory.
pub fn current_crate_dir() -> PathBuf {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}
