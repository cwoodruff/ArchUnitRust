//! Predefined rules for common architectures (`com.tngtech.archunit.library`).
//!
//! * [`architectures`]: [`layered_architecture`] and [`onion_architecture`];
//!   [`modular_monolith`](fn@modular_monolith) is a `[rust-only]` addition in the same style.
//! * [`dependencies`]: [`slices`] with `be_free_of_cycles()` and `not_depend_on_each_other()`.
//! * [`cycle_detection`]: the cycle detector behind the slice rules, usable on any graph.
//! * [`general_coding_rules`], [`dependency_rules`], [`proxy_rules`]: ready-made rules.
//! * [`plantuml`]: [`adhere_to_plant_uml_diagram`] from a component diagram.
//! * [`freeze`](mod@freeze): [`freeze()`](fn@freeze) a rule so only new violations fail.

pub mod architectures;
pub mod cycle_detection;
pub mod dependencies;
pub mod dependency_rules;
pub mod freeze;
pub mod general_coding_rules;
pub mod modular_monolith;
pub mod plantuml;
pub mod proxy_rules;

pub use architectures::{
    Architectures, LayeredArchitecture, OnionArchitecture, layered_architecture, onion_architecture,
};
pub use dependencies::{SlicesRuleDefinition, slices};
pub use freeze::{FreezingArchRule, freeze};
pub use modular_monolith::{ModularMonolithArchitecture, modular_monolith};
pub use plantuml::adhere_to_plant_uml_diagram;
