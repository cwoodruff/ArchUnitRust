//! Basic building blocks shared by every layer (`com.tngtech.archunit.base`).

mod described_function;
mod described_predicate;
mod error;

pub use described_function::DescribedFunction;
pub use described_predicate::{
    DescribedPredicate, all_elements, all_of, always_false, always_true, and, any_element_that,
    any_of, describe, do_not, does_not, empty, equal_to, greater_than, greater_than_or_equal_to,
    less_than, less_than_or_equal_to, not, optional_contains, optional_empty, or,
};
pub use error::ArchUnitError;

/// Anything that carries a human readable description used in rule texts and reports.
pub trait HasDescription {
    /// The description, e.g. `reside in a package '..service..'`.
    fn description(&self) -> String;
}

impl<T: HasDescription + ?Sized> HasDescription for &T {
    fn description(&self) -> String {
        (**self).description()
    }
}

impl<T: HasDescription + ?Sized> HasDescription for Box<T> {
    fn description(&self) -> String {
        (**self).description()
    }
}

/// Formats strings the way ArchUnit's `Formatters.joinSingleQuoted` does: `'a', 'b'`.
pub fn join_single_quoted<I, S>(strings: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let joined = strings
        .into_iter()
        .map(|s| s.as_ref().to_owned())
        .collect::<Vec<_>>()
        .join("', '");
    if joined.is_empty() {
        joined
    } else {
        format!("'{joined}'")
    }
}
