//! Domain objects representing Rust code (`com.tngtech.archunit.core.domain`).
//!
//! The model is a graph owned by [`RustItems`]. Every other type ([`RustItem`], [`RustMember`],
//! [`RustAccess`], [`Dependency`], ...) is a cheap handle into that graph, so it can be cloned
//! and passed around freely, exactly like ArchUnit's `JavaClass` objects.

pub(crate) mod graph;

mod annotation;
pub mod dependency;
pub mod formatters;
mod modifiers;
mod package_matcher;
pub mod properties;
pub mod rust_access;
pub mod rust_item;
mod rust_items;
pub mod rust_member;
pub mod rust_module;
mod source_code_location;
mod types;

pub use annotation::{AnnotationKind, AnnotationValue, RustAnnotation};
pub use dependency::{Dependency, DependencyKind};
pub use rust_access::{AccessKind, AccessTarget, AccessType, ResolutionKind, RustAccess};
pub use rust_member::{
    MemberKind, Receiver, RustCodeUnit, RustConstructor, RustField, RustMember, RustMethod,
    RustParameter, RustVariant,
};

/// `JavaCodeUnit.Predicates`.
pub mod rust_code_unit {
    pub use super::rust_member::code_unit_predicates as predicates;
}

/// `AccessTarget.Predicates`.
pub mod access_target {
    pub use super::rust_access::target_predicates as predicates;
}
pub use modifiers::{RustModifier, Visibility};
pub use package_matcher::{MatchResult, PackageMatcher, PackageMatchers};
pub use rust_item::{ItemKind, RustItem, TargetKind};
pub use rust_items::RustItems;
pub use rust_module::RustModule;
pub use source_code_location::SourceCodeLocation;
pub use types::{RustType, TypeParameter};
