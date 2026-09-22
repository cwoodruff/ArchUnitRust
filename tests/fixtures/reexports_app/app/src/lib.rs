//! Fixture exercising re-exports, glob imports, renames, nested and `#[path]` modules,
//! `cfg(test)` code, binary and integration-test targets, and a workspace dependency.
pub mod domain;
pub mod facade;
mod hidden;

pub use domain::model::Order;
pub use facade::*;

pub(crate) use crate::hidden::hidden_helper as exported_helper;

pub fn use_exported_helper() -> u32 {
    exported_helper()
}
