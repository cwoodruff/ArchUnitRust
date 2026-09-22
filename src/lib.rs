//! ArchUnit for Rust.
//!
//! A port of [ArchUnit](https://www.archunit.org) that checks the architecture of Rust
//! crates. The library is split into the same three layers as ArchUnit:
//!
//! * [`core`]: the importer ([`core::importer::CrateImporter`]) and the domain model
//!   ([`core::domain::RustItems`], [`core::domain::RustItem`], accesses and dependencies).
//! * `lang`: the fluent rule API (`classes().that()...should()...`), conditions and reports.
//! * `library`: predefined rules such as layered and onion architectures, slices and cycles.
//!
//! See `docs/MAPPING.md` in the repository for the mapping of every ArchUnit API to this crate.

pub mod base;
pub mod config;
pub mod core;
pub mod lang;
pub mod library;

/// Everything a typical architecture test needs.
pub mod prelude {
    pub use crate::base::{DescribedPredicate, HasDescription};
    pub use crate::core::domain::{
        Dependency, PackageMatcher, RustAccess, RustItem, RustItems, RustMember, RustModifier,
        RustModule,
    };
    pub use crate::core::importer::{CrateImporter, ImportOption, Location};
    pub use crate::lang::syntax::{
        ArchRuleDefinition, all, classes, code_units, constructors, fields, members, methods, no,
        no_class, no_classes, no_code_units, no_constructors, no_fields, no_members, no_methods,
        priority, the_class,
    };
    pub use crate::lang::{
        ArchCondition, ArchRule, ConditionEvents, EvaluationResult, Priority, SimpleConditionEvent,
    };
}
