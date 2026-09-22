//! Formatting helpers used in descriptions (`Formatters`).

pub use crate::base::join_single_quoted;

/// Formats a method as `owner::name(param1, param2)`.
pub fn format_method(owner_name: &str, method_name: &str, parameters: &[String]) -> String {
    format!("{owner_name}::{method_name}({})", parameters.join(", "))
}

/// Formats a method with simple names only, e.g. `Order::new(u64)`.
pub fn format_method_simple(owner_name: &str, method_name: &str, parameters: &[String]) -> String {
    let params: Vec<String> = parameters.iter().map(|p| ensure_simple_name(p)).collect();
    format_method(&ensure_simple_name(owner_name), method_name, &params)
}

/// Reduces a fully qualified name to its last segment.
pub fn ensure_simple_name(name: &str) -> String {
    name.rsplit("::").next().unwrap_or(name).to_owned()
}

/// Formats a predicate name with an argument the way ArchUnit's `formatNamedPredicate` does,
/// e.g. `have simple name 'Foo'`.
pub fn format_named_predicate(name: &str, argument: &str) -> String {
    format!("{name} '{argument}'")
}
