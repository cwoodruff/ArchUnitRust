use std::fmt;

/// Where something was declared: a crate-relative source file and a line
/// (`SourceCodeLocation`). Displays as `(src/domain/order.rs:14)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceCodeLocation {
    file: String,
    line: usize,
}

impl SourceCodeLocation {
    /// Creates a location from a crate-relative file path and a 1-based line number.
    pub fn of(file: impl Into<String>, line: usize) -> Self {
        Self {
            file: file.into(),
            line,
        }
    }

    pub(crate) fn unknown() -> Self {
        Self::of("<unknown>", 0)
    }

    /// The crate-relative path of the source file, e.g. `src/domain/order.rs`.
    pub fn source_file_name(&self) -> &str {
        &self.file
    }

    /// The 1-based line number, or 0 when unknown.
    pub fn line_number(&self) -> usize {
        self.line
    }

    pub(crate) fn with_line(&self, line: usize) -> Self {
        Self::of(self.file.clone(), line)
    }
}

impl fmt::Display for SourceCodeLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}:{})", self.file, self.line)
    }
}
