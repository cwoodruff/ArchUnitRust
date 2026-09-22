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
pub mod core;

/// Everything a typical architecture test needs.
pub mod prelude {
    pub use crate::base::{DescribedPredicate, HasDescription};
    pub use crate::core::domain::{
        Dependency, PackageMatcher, RustAccess, RustItem, RustItems, RustMember, RustModifier,
        RustModule,
    };
    pub use crate::core::importer::{CrateImporter, ImportOption, Location};
}
