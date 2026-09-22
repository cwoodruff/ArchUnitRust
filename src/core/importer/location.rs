use std::fmt;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::core::domain::TargetKind;

/// A source location considered during import: a file (or an inline `#[cfg(test)]` region of
/// one) together with the crate and cargo target it belongs to (`Location`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Location {
    path: PathBuf,
    crate_dir: PathBuf,
    crate_name: String,
    target: TargetKind,
    is_test_code: bool,
    is_workspace_member: bool,
}

impl Location {
    pub(crate) fn new(
        path: PathBuf,
        crate_dir: PathBuf,
        crate_name: String,
        target: TargetKind,
        is_test_code: bool,
        is_workspace_member: bool,
    ) -> Self {
        Self {
            path,
            crate_dir,
            crate_name,
            target,
            is_test_code,
            is_workspace_member,
        }
    }

    /// A location for an arbitrary path (a source file or a crate directory), for use in
    /// custom import options, [`LocationProvider`](crate::harness::LocationProvider)s and tests.
    /// The crate directory is the nearest directory containing a `Cargo.toml`.
    pub fn of(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let crate_dir = path
            .ancestors()
            .find(|dir| dir.join("Cargo.toml").is_file())
            .map(Path::to_path_buf)
            .or_else(|| path.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        Self::new(path, crate_dir, String::new(), TargetKind::Lib, false, true)
    }

    /// The absolute path of the source file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The directory containing the crate's `Cargo.toml`.
    pub fn crate_dir(&self) -> &Path {
        &self.crate_dir
    }

    /// The crate name (with `_` for `-`).
    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    /// The cargo target.
    pub fn target(&self) -> &TargetKind {
        &self.target
    }

    /// Whether this is test code: a test or bench target, a file under `tests/` or `benches/`,
    /// or a `#[cfg(test)]` region.
    pub fn is_test_code(&self) -> bool {
        self.is_test_code
    }

    /// Whether the crate is a member of the imported workspace (as opposed to a dependency).
    pub fn is_workspace_member(&self) -> bool {
        self.is_workspace_member
    }

    /// Whether the path contains `part` (`Location.contains(..)`).
    pub fn contains(&self, part: &str) -> bool {
        self.path.to_string_lossy().contains(part)
    }

    /// Whether the path matches `pattern` (`Location.matches(..)`).
    pub fn matches(&self, pattern: &Regex) -> bool {
        pattern.is_match(&self.path.to_string_lossy())
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

/// Decides which locations to import (`ImportOption`).
///
/// Implemented for closures `Fn(&Location) -> bool`.
pub trait ImportOption: Send + Sync {
    /// Whether `location` should be imported.
    fn includes(&self, location: &Location) -> bool;
}

impl<F> ImportOption for F
where
    F: Fn(&Location) -> bool + Send + Sync,
{
    fn includes(&self, location: &Location) -> bool {
        self(location)
    }
}

impl ImportOption for Box<dyn ImportOption> {
    fn includes(&self, location: &Location) -> bool {
        (**self).includes(location)
    }
}

/// Predefined import options (`ImportOption.Predefined`).
pub mod import_option {
    use super::{ImportOption, Location};

    /// Excludes test code: `#[cfg(test)]` modules and items, `#[test]` functions, files under
    /// `tests/` and `benches/`, and test/bench cargo targets (`DoNotIncludeTests`).
    #[derive(Debug, Clone, Copy, Default)]
    pub struct DoNotIncludeTests;

    impl ImportOption for DoNotIncludeTests {
        fn includes(&self, location: &Location) -> bool {
            !location.is_test_code()
        }
    }

    /// Includes only test code (`OnlyIncludeTests`).
    #[derive(Debug, Clone, Copy, Default)]
    pub struct OnlyIncludeTests;

    impl ImportOption for OnlyIncludeTests {
        fn includes(&self, location: &Location) -> bool {
            location.is_test_code()
        }
    }

    /// Excludes crates that are not workspace members, i.e. dependencies whose source was
    /// resolved from the registry (`DoNotIncludeJars` / `DoNotIncludeArchives`).
    #[derive(Debug, Clone, Copy, Default)]
    pub struct DoNotIncludeDependencies;

    impl ImportOption for DoNotIncludeDependencies {
        fn includes(&self, location: &Location) -> bool {
            location.is_workspace_member()
        }
    }
}
