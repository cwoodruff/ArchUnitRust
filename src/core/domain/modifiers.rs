use std::fmt;

/// The visibility of an item or member as written in the source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Visibility {
    /// `pub`
    Pub,
    /// `pub(crate)`
    PubCrate,
    /// `pub(super)`
    PubSuper,
    /// `pub(in path)`
    PubIn(String),
    /// No visibility keyword.
    Private,
}

impl Visibility {
    /// The modifiers that describe this visibility (see [`RustModifier`]).
    pub fn modifiers(&self) -> Vec<RustModifier> {
        match self {
            Visibility::Pub => vec![RustModifier::Pub],
            Visibility::PubCrate => vec![RustModifier::PubRestricted, RustModifier::PubCrate],
            Visibility::PubSuper => vec![RustModifier::PubRestricted, RustModifier::PubSuper],
            Visibility::PubIn(path) => vec![
                RustModifier::PubRestricted,
                RustModifier::PubIn(path.clone()),
            ],
            Visibility::Private => vec![RustModifier::Private],
        }
    }
}

/// Modifiers of items and members (`JavaModifier`).
///
/// Java's `PUBLIC`, `PROTECTED`, `PRIVATE`, `STATIC`, `FINAL`, `ABSTRACT` map to
/// `Pub`, `PubRestricted`, `Private`, `Static`, `Final`, `Abstract`; the rest are Rust-only.
/// See `docs/MAPPING.md` §2.3 for the exact semantics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RustModifier {
    /// `pub` without restriction (Java `PUBLIC`).
    Pub,
    /// Any restricted visibility: `pub(crate)`, `pub(super)`, `pub(in ..)` (Java `PROTECTED`).
    PubRestricted,
    /// Inherited visibility, i.e. no `pub` (Java `PRIVATE` and package-private).
    Private,
    /// `pub(crate)`.
    PubCrate,
    /// `pub(super)`.
    PubSuper,
    /// `pub(in path)`.
    PubIn(String),
    /// A function without a `self` receiver, an associated const, or a `static`/`const` item
    /// (Java `STATIC`).
    Static,
    /// An inherent method, which cannot be overridden (Java `FINAL` on methods).
    Final,
    /// A trait, or a trait method without a default body (Java `ABSTRACT`).
    Abstract,
    /// A trait item with a default body.
    Default,
    /// `unsafe`.
    Unsafe,
    /// `async`.
    Async,
    /// `const fn`.
    Const,
    /// `extern` function or block.
    Extern,
    /// `static mut`.
    Mut,
    /// `#[non_exhaustive]`.
    NonExhaustive,
}

impl fmt::Display for RustModifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            RustModifier::Pub => "pub",
            RustModifier::PubRestricted => "pub(restricted)",
            RustModifier::Private => "private",
            RustModifier::PubCrate => "pub(crate)",
            RustModifier::PubSuper => "pub(super)",
            RustModifier::PubIn(path) => return write!(f, "pub(in {path})"),
            RustModifier::Static => "static",
            RustModifier::Final => "final",
            RustModifier::Abstract => "abstract",
            RustModifier::Default => "default",
            RustModifier::Unsafe => "unsafe",
            RustModifier::Async => "async",
            RustModifier::Const => "const",
            RustModifier::Extern => "extern",
            RustModifier::Mut => "mut",
            RustModifier::NonExhaustive => "non_exhaustive",
        };
        f.write_str(text)
    }
}
