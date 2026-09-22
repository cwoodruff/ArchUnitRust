use std::path::PathBuf;

/// Errors raised while importing crates or evaluating rules (`ArchUnitException`).
#[derive(Debug, thiserror::Error)]
pub enum ArchUnitError {
    /// `cargo metadata` failed or returned something unusable.
    #[error("could not read cargo metadata for {path}: {source}")]
    CargoMetadata {
        /// The path that was queried.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: Box<cargo_metadata::Error>,
    },
    /// A source file could not be read.
    #[error("could not read {path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// A source file could not be parsed.
    #[error("could not parse {path}: {source}")]
    Parse {
        /// The file.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: syn::Error,
    },
    /// A `mod foo;` declaration whose file does not exist.
    #[error("module `{module}` declared in {declared_in} has no file (tried {tried:?})")]
    MissingModuleFile {
        /// The module name.
        module: String,
        /// The file that declared it.
        declared_in: PathBuf,
        /// The candidate paths.
        tried: Vec<PathBuf>,
    },
    /// No crate could be found at the requested location.
    #[error("{0}")]
    NoCrate(String),
    /// An invalid package identifier such as `some...pkg`.
    #[error("{0}")]
    IllegalPackageIdentifier(String),
    /// A requested item or package is not contained in the import.
    #[error("{0}")]
    NotFound(String),
    /// A configuration problem.
    #[error("{0}")]
    Configuration(String),
}
