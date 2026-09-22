//! Fixture for `Architectures::modular_monolith()`: business modules with an `api` submodule
//! each, a `shared` kernel, and a utility module outside of every business module.
#![allow(dead_code, clippy::all)]
pub mod billing;
pub mod inventory;
pub mod orders;
pub mod shared;
pub mod util;
