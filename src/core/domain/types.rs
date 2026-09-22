use std::fmt;

use super::graph::ItemId;

/// A type as it appears in a signature (`JavaType` and its subtypes).
///
/// `raw_item()` gives the erasure: the item behind a path type with references, slices,
/// arrays and pointers unwrapped (`JavaType.toErasure()`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RustType {
    /// A path type such as `Vec<Order>`; `item` is the resolved (or stub) item.
    Path {
        /// The resolved item.
        item: ItemId,
        /// The path as written, for display.
        written: String,
        /// Generic arguments.
        args: Vec<RustType>,
    },
    /// `&T` / `&mut T`.
    Reference {
        /// Whether it is `&mut`.
        mutable: bool,
        /// The referenced type.
        inner: Box<RustType>,
    },
    /// `*const T` / `*mut T`.
    Ptr {
        /// Whether it is `*mut`.
        mutable: bool,
        /// The pointee type.
        inner: Box<RustType>,
    },
    /// `[T]`.
    Slice(Box<RustType>),
    /// `[T; N]`.
    Array(Box<RustType>),
    /// `(A, B)`; the unit type is the empty tuple.
    Tuple(Vec<RustType>),
    /// `dyn A + B`.
    TraitObject(Vec<RustType>),
    /// `impl A + B`.
    ImplTrait(Vec<RustType>),
    /// A generic type parameter such as `T`.
    TypeParam(String),
    /// `!`.
    Never,
    /// `_`.
    Infer,
    /// `fn(A) -> B`.
    FnPointer {
        /// Parameter types.
        params: Vec<RustType>,
        /// Return type.
        ret: Box<RustType>,
    },
    /// A type that could not be interpreted (macros in type position).
    Unknown(String),
}

impl RustType {
    /// The unit type `()`.
    pub fn unit() -> Self {
        RustType::Tuple(Vec::new())
    }

    /// The erased item this type refers to, if any.
    pub(crate) fn raw_item(&self) -> Option<ItemId> {
        match self {
            RustType::Path { item, .. } => Some(*item),
            RustType::Reference { inner, .. }
            | RustType::Ptr { inner, .. }
            | RustType::Slice(inner)
            | RustType::Array(inner) => inner.raw_item(),
            RustType::TraitObject(bounds) | RustType::ImplTrait(bounds) => {
                bounds.first().and_then(RustType::raw_item)
            }
            _ => None,
        }
    }

    /// Every item mentioned anywhere in this type, outermost first.
    pub(crate) fn all_items(&self) -> Vec<ItemId> {
        let mut out = Vec::new();
        self.collect_items(&mut out);
        out
    }

    fn collect_items(&self, out: &mut Vec<ItemId>) {
        match self {
            RustType::Path { item, args, .. } => {
                out.push(*item);
                args.iter().for_each(|a| a.collect_items(out));
            }
            RustType::Reference { inner, .. }
            | RustType::Ptr { inner, .. }
            | RustType::Slice(inner)
            | RustType::Array(inner) => inner.collect_items(out),
            RustType::Tuple(types) | RustType::TraitObject(types) | RustType::ImplTrait(types) => {
                types.iter().for_each(|t| t.collect_items(out));
            }
            RustType::FnPointer { params, ret } => {
                params.iter().for_each(|t| t.collect_items(out));
                ret.collect_items(out);
            }
            RustType::TypeParam(_) | RustType::Never | RustType::Infer | RustType::Unknown(_) => {}
        }
    }

    /// Generic arguments of a path type; empty otherwise.
    pub fn type_arguments(&self) -> &[RustType] {
        match self {
            RustType::Path { args, .. } => args,
            _ => &[],
        }
    }

    /// The type as written in the source (with generics), e.g. `Vec<Order>`.
    pub fn written(&self) -> String {
        self.to_string()
    }

    /// Whether the erasure is a generic type parameter.
    pub fn is_type_parameter(&self) -> bool {
        matches!(self, RustType::TypeParam(_))
    }
}

impl fmt::Display for RustType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn list(f: &mut fmt::Formatter<'_>, types: &[RustType], sep: &str) -> fmt::Result {
            for (i, t) in types.iter().enumerate() {
                if i > 0 {
                    f.write_str(sep)?;
                }
                write!(f, "{t}")?;
            }
            Ok(())
        }
        match self {
            RustType::Path { written, args, .. } => {
                f.write_str(written)?;
                if !args.is_empty() {
                    f.write_str("<")?;
                    list(f, args, ", ")?;
                    f.write_str(">")?;
                }
                Ok(())
            }
            RustType::Reference { mutable, inner } => {
                write!(f, "&{}{inner}", if *mutable { "mut " } else { "" })
            }
            RustType::Ptr { mutable, inner } => {
                write!(f, "*{} {inner}", if *mutable { "mut" } else { "const" })
            }
            RustType::Slice(inner) => write!(f, "[{inner}]"),
            RustType::Array(inner) => write!(f, "[{inner}; _]"),
            RustType::Tuple(types) => {
                f.write_str("(")?;
                list(f, types, ", ")?;
                if types.len() == 1 {
                    f.write_str(",")?;
                }
                f.write_str(")")
            }
            RustType::TraitObject(bounds) => {
                f.write_str("dyn ")?;
                list(f, bounds, " + ")
            }
            RustType::ImplTrait(bounds) => {
                f.write_str("impl ")?;
                list(f, bounds, " + ")
            }
            RustType::TypeParam(name) => f.write_str(name),
            RustType::Never => f.write_str("!"),
            RustType::Infer => f.write_str("_"),
            RustType::FnPointer { params, ret } => {
                f.write_str("fn(")?;
                list(f, params, ", ")?;
                write!(f, ") -> {ret}")
            }
            RustType::Unknown(text) => f.write_str(text),
        }
    }
}

/// A generic type parameter with its bounds (`JavaTypeVariable`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeParameter {
    pub(crate) name: String,
    pub(crate) bounds: Vec<RustType>,
}

impl TypeParameter {
    /// The parameter name, e.g. `T`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The declared bounds (`JavaTypeVariable.getUpperBounds()`).
    pub fn upper_bounds(&self) -> &[RustType] {
        &self.bounds
    }
}
