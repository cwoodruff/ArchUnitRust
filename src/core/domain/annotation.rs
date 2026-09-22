use std::fmt;

/// How an annotation was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnotationKind {
    /// An outer or inner attribute such as `#[secured]` or `#![allow(dead_code)]`.
    Attribute,
    /// One entry of a `#[derive(..)]` list.
    Derive,
}

/// A parsed attribute argument value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationValue {
    /// A string literal.
    Str(String),
    /// An integer literal.
    Int(i128),
    /// A boolean literal.
    Bool(bool),
    /// A bare path such as `Debug` in `#[derive(Debug)]` or `dead_code` in `#[allow(dead_code)]`.
    Path(String),
    /// Anything else, kept as source text.
    Tokens(String),
}

impl fmt::Display for AnnotationValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnnotationValue::Str(s) => write!(f, "\"{s}\""),
            AnnotationValue::Int(i) => write!(f, "{i}"),
            AnnotationValue::Bool(b) => write!(f, "{b}"),
            AnnotationValue::Path(p) | AnnotationValue::Tokens(p) => f.write_str(p),
        }
    }
}

/// An attribute or derive on an item, member or parameter (`JavaAnnotation`).
///
/// `#[derive(Debug, Clone)]` produces two annotations of kind [`AnnotationKind::Derive`], one per
/// derived trait, so `is_annotated_with("Debug")` works like Java's `isAnnotatedWith(..)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustAnnotation {
    pub(crate) path: String,
    pub(crate) resolved_path: Option<String>,
    pub(crate) kind: AnnotationKind,
    pub(crate) properties: Vec<(String, AnnotationValue)>,
    pub(crate) tokens: String,
}

impl RustAnnotation {
    /// The path as written, e.g. `secured`, `serde::Serialize`, `derive` is never reported.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The fully qualified name of the attribute or derive macro if it could be resolved,
    /// e.g. `std::fmt::Debug`, else the path as written.
    pub fn raw_type_name(&self) -> &str {
        self.resolved_path.as_deref().unwrap_or(&self.path)
    }

    /// The last path segment, e.g. `Serialize`.
    pub fn simple_name(&self) -> &str {
        self.path.rsplit("::").next().unwrap_or(&self.path)
    }

    /// Whether this was an attribute or a derive.
    pub fn kind(&self) -> AnnotationKind {
        self.kind
    }

    /// `name = value` pairs and bare entries of the attribute's arguments (`JavaAnnotation.getProperties()`).
    ///
    /// Bare entries (`#[allow(dead_code)]`) are reported with an empty key; a single unnamed
    /// literal (`#[adapter("cli")]`) is reported under the key `value` like Java's annotation
    /// `value()` element.
    pub fn properties(&self) -> &[(String, AnnotationValue)] {
        &self.properties
    }

    /// Looks up an argument by name (`JavaAnnotation.get(..)`).
    pub fn get(&self, key: &str) -> Option<&AnnotationValue> {
        self.properties
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// The argument tokens as written, e.g. `("cli")` or `(note = "use x")`.
    pub fn tokens(&self) -> &str {
        &self.tokens
    }

    /// Whether `name` matches this annotation by simple name or by (resolved) path.
    pub fn matches(&self, name: &str) -> bool {
        let name = name.trim_start_matches('@');
        self.path == name
            || self.simple_name() == name
            || self.resolved_path.as_deref() == Some(name)
    }
}

impl fmt::Display for RustAnnotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "@{}", self.path)
    }
}
