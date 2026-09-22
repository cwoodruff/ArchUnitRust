//! Property traits shared by items and members, with their predicates
//! (`com.tngtech.archunit.core.domain.properties`).

use std::fmt;

use regex::Regex;

use super::annotation::RustAnnotation;
use super::modifiers::RustModifier;
use super::rust_item::RustItem;
use super::source_code_location::SourceCodeLocation;
use super::types::{RustType, TypeParameter};
use crate::base::{DescribedPredicate, join_single_quoted};

/// Something with a name (`HasName`).
pub trait HasName {
    /// The name, e.g. `my_app::domain::Order` for items or `total` for members.
    fn name(&self) -> String;
}

/// Something with a name and a full name (`HasName.AndFullName`).
pub trait HasFullName: HasName {
    /// The full name, e.g. `my_app::domain::Order::total()`.
    fn full_name(&self) -> String;
}

/// Something with modifiers (`HasModifiers`).
pub trait HasModifiers {
    /// All modifiers.
    fn modifiers(&self) -> Vec<RustModifier>;

    /// Whether `modifier` is present.
    fn has_modifier(&self, modifier: &RustModifier) -> bool {
        self.modifiers().contains(modifier)
    }
}

/// Something that can carry attributes and derives (`CanBeAnnotated` / `HasAnnotations`).
pub trait CanBeAnnotated {
    /// All annotations (attributes and derives).
    fn annotations(&self) -> Vec<RustAnnotation>;

    /// Whether an annotation matching `selector` is present (`isAnnotatedWith(..)`).
    fn is_annotated_with(&self, selector: impl Into<AnnotationSelector>) -> bool {
        let selector = selector.into();
        self.annotations().iter().any(|a| selector.matches(a))
    }

    /// The annotation with the given (simple or full) name (`getAnnotationOfType(..)`).
    ///
    /// # Panics
    /// If there is no such annotation.
    fn annotation_of_type(&self, name: &str) -> RustAnnotation {
        self.try_annotation_of_type(name)
            .unwrap_or_else(|| panic!("Not annotated with @{name}"))
    }

    /// The annotation with the given (simple or full) name, if present.
    fn try_annotation_of_type(&self, name: &str) -> Option<RustAnnotation> {
        self.annotations().into_iter().find(|a| a.matches(name))
    }
}

/// Something with an owner (`HasOwner`).
pub trait HasOwner<T> {
    /// The owner.
    fn owner(&self) -> T;
}

/// Something with a source location (`HasSourceCodeLocation`).
pub trait HasSourceCodeLocation {
    /// The declaration location.
    fn source_code_location(&self) -> SourceCodeLocation;
}

/// Something with a type, such as a field (`HasType`).
pub trait HasType {
    /// The declared type.
    fn type_(&self) -> Option<RustType>;
    /// The erased type as an item.
    fn raw_type(&self) -> Option<RustItem>;
}

/// Something with a return type (`HasReturnType`).
pub trait HasReturnType {
    /// The declared return type; the unit type if omitted.
    fn return_type(&self) -> RustType;
    /// The erased return type as an item, if it is a path type.
    fn raw_return_type(&self) -> Option<RustItem>;
}

/// Something with parameters (`HasParameterTypes`).
pub trait HasParameterTypes {
    /// The declared parameter types, excluding any `self` receiver.
    fn parameter_types(&self) -> Vec<RustType>;
    /// The erased parameter types as items (`None` for type parameters and tuples).
    fn raw_parameter_types(&self) -> Vec<Option<RustItem>>;
}

/// Something that can "throw", i.e. return `Result<_, E>` (`HasThrowsClause`).
pub trait HasErrorTypes {
    /// The error type `E` of a `Result<_, E>` return type, as an item.
    fn error_types(&self) -> Vec<RustItem>;
}

/// Something with generic type parameters (`HasTypeParameters`).
pub trait HasTypeParameters {
    /// The declared type parameters.
    fn type_parameters(&self) -> Vec<TypeParameter>;
}

/// Something whose description can be overridden (`CanOverrideDescription`).
pub trait CanOverrideDescription {
    /// Returns a copy with a new description.
    fn as_(self, description: &str) -> Self;
}

/// Selects items by full name, by simple name (last segment), or by predicate.
///
/// Every ArchUnit method that is overloaded over `Class<?>`, `String` and
/// `DescribedPredicate<JavaClass>` takes an `impl Into<ItemSelector>` instead.
#[derive(Clone)]
pub enum ItemSelector {
    /// The full name (`my_app::domain::Order`), or a simple name when it contains no `::`.
    Name(String),
    /// An arbitrary predicate.
    Predicate(DescribedPredicate<RustItem>),
}

impl fmt::Debug for ItemSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ItemSelector({})", self.description())
    }
}

impl ItemSelector {
    /// Whether `item` is selected.
    pub fn matches(&self, item: &RustItem) -> bool {
        match self {
            ItemSelector::Name(name) => {
                if name.contains("::") {
                    item.is_equivalent_to(name)
                } else {
                    item.simple_name() == *name
                }
            }
            ItemSelector::Predicate(predicate) => predicate.test(item),
        }
    }

    /// The description used in rule texts: the name itself, or the predicate description.
    pub fn description(&self) -> String {
        match self {
            ItemSelector::Name(name) => name.clone(),
            ItemSelector::Predicate(predicate) => predicate.description().to_owned(),
        }
    }
}

impl From<&str> for ItemSelector {
    fn from(name: &str) -> Self {
        ItemSelector::Name(name.to_owned())
    }
}

impl From<String> for ItemSelector {
    fn from(name: String) -> Self {
        ItemSelector::Name(name)
    }
}

impl From<&String> for ItemSelector {
    fn from(name: &String) -> Self {
        ItemSelector::Name(name.clone())
    }
}

impl From<DescribedPredicate<RustItem>> for ItemSelector {
    fn from(predicate: DescribedPredicate<RustItem>) -> Self {
        ItemSelector::Predicate(predicate)
    }
}

impl From<&RustItem> for ItemSelector {
    fn from(item: &RustItem) -> Self {
        ItemSelector::Name(item.name())
    }
}

/// Selects annotations by name or by predicate.
#[derive(Clone)]
pub enum AnnotationSelector {
    /// The attribute path, simple name, or resolved macro path; a leading `@` is ignored.
    Name(String),
    /// An arbitrary predicate.
    Predicate(DescribedPredicate<RustAnnotation>),
}

impl fmt::Debug for AnnotationSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AnnotationSelector({})", self.description())
    }
}

impl AnnotationSelector {
    /// Whether `annotation` is selected.
    pub fn matches(&self, annotation: &RustAnnotation) -> bool {
        match self {
            AnnotationSelector::Name(name) => annotation.matches(name),
            AnnotationSelector::Predicate(predicate) => predicate.test(annotation),
        }
    }

    /// The description used in rule texts: `@Name` or the predicate description.
    pub fn description(&self) -> String {
        match self {
            AnnotationSelector::Name(name) => {
                let name = name.trim_start_matches('@');
                format!("@{}", name.rsplit("::").next().unwrap_or(name))
            }
            AnnotationSelector::Predicate(predicate) => predicate.description().to_owned(),
        }
    }
}

impl From<&str> for AnnotationSelector {
    fn from(name: &str) -> Self {
        AnnotationSelector::Name(name.to_owned())
    }
}

impl From<String> for AnnotationSelector {
    fn from(name: String) -> Self {
        AnnotationSelector::Name(name)
    }
}

impl From<DescribedPredicate<RustAnnotation>> for AnnotationSelector {
    fn from(predicate: DescribedPredicate<RustAnnotation>) -> Self {
        AnnotationSelector::Predicate(predicate)
    }
}

/// Predicates on [`HasName`] (`HasName.Predicates`).
pub mod has_name {
    use super::*;

    /// `HasName.Predicates`.
    pub mod predicates {
        use super::*;

        /// Described as `name '<name>'`.
        pub fn name<T: HasName + ?Sized + 'static>(name: &str) -> DescribedPredicate<T> {
            let expected = name.to_owned();
            DescribedPredicate::describe(format!("name '{name}'"), move |t: &T| {
                t.name() == expected
            })
        }

        /// Described as `name matching '<regex>'`.
        pub fn name_matching<T: HasName + ?Sized + 'static>(regex: &str) -> DescribedPredicate<T> {
            let compiled = full_match_regex(regex);
            DescribedPredicate::describe(format!("name matching '{regex}'"), move |t: &T| {
                compiled.is_match(&t.name())
            })
        }

        /// Described as `name starting with '<prefix>'`.
        pub fn name_starting_with<T: HasName + ?Sized + 'static>(
            prefix: &str,
        ) -> DescribedPredicate<T> {
            let expected = prefix.to_owned();
            DescribedPredicate::describe(format!("name starting with '{prefix}'"), move |t: &T| {
                t.name().starts_with(&expected)
            })
        }

        /// Described as `name containing '<infix>'`.
        pub fn name_containing<T: HasName + ?Sized + 'static>(
            infix: &str,
        ) -> DescribedPredicate<T> {
            let expected = infix.to_owned();
            DescribedPredicate::describe(format!("name containing '{infix}'"), move |t: &T| {
                t.name().contains(&expected)
            })
        }

        /// Described as `name ending with '<suffix>'`.
        pub fn name_ending_with<T: HasName + ?Sized + 'static>(
            suffix: &str,
        ) -> DescribedPredicate<T> {
            let expected = suffix.to_owned();
            DescribedPredicate::describe(format!("name ending with '{suffix}'"), move |t: &T| {
                t.name().ends_with(&expected)
            })
        }
    }

    /// `HasName.Functions`.
    pub mod functions {
        use super::*;
        use crate::base::DescribedFunction;

        /// `GET_NAME`.
        pub fn get_name<T: HasName + ?Sized + 'static>() -> DescribedFunction<T, String> {
            DescribedFunction::describe("name", |t: &T| t.name())
        }
    }

    /// `HasName.Utils.namesOf(..)`.
    pub fn names_of<'a, T: HasName + 'a>(
        has_names: impl IntoIterator<Item = &'a T>,
    ) -> Vec<String> {
        has_names.into_iter().map(HasName::name).collect()
    }
}

/// Predicates on [`HasFullName`] (`HasName.AndFullName.Predicates`).
pub mod has_full_name {
    use super::*;

    /// `HasName.AndFullName.Predicates`.
    pub mod predicates {
        use super::*;

        /// Described as `full name '<name>'`.
        pub fn full_name<T: HasFullName + ?Sized + 'static>(
            full_name: &str,
        ) -> DescribedPredicate<T> {
            let expected = full_name.to_owned();
            DescribedPredicate::describe(format!("full name '{full_name}'"), move |t: &T| {
                t.full_name() == expected
            })
        }

        /// Described as `full name matching '<regex>'`.
        pub fn full_name_matching<T: HasFullName + ?Sized + 'static>(
            regex: &str,
        ) -> DescribedPredicate<T> {
            let compiled = full_match_regex(regex);
            DescribedPredicate::describe(format!("full name matching '{regex}'"), move |t: &T| {
                compiled.is_match(&t.full_name())
            })
        }
    }

    /// `HasName.AndFullName.Functions`.
    pub mod functions {
        use super::*;
        use crate::base::DescribedFunction;

        /// `GET_FULL_NAME`.
        pub fn get_full_name<T: HasFullName + ?Sized + 'static>() -> DescribedFunction<T, String> {
            DescribedFunction::describe("full name", |t: &T| t.full_name())
        }
    }
}

/// Predicates on [`HasModifiers`] (`HasModifiers.Predicates`).
pub mod has_modifiers {
    use super::*;

    /// `HasModifiers.Predicates`.
    pub mod predicates {
        use super::*;

        /// Described as `modifier <modifier>`.
        pub fn modifier<T: HasModifiers + ?Sized + 'static>(
            modifier: RustModifier,
        ) -> DescribedPredicate<T> {
            let description = format!("modifier {modifier}");
            DescribedPredicate::describe(description, move |t: &T| t.has_modifier(&modifier))
        }
    }
}

/// Predicates on [`CanBeAnnotated`] (`CanBeAnnotated.Predicates`).
pub mod can_be_annotated {
    use super::*;

    /// `CanBeAnnotated.Predicates`.
    pub mod predicates {
        use super::*;

        /// Described as `annotated with @Name` or `annotated with <predicate>`.
        pub fn annotated_with<T: CanBeAnnotated + ?Sized + 'static>(
            selector: impl Into<AnnotationSelector>,
        ) -> DescribedPredicate<T> {
            let selector = selector.into();
            let description = format!("annotated with {}", selector.description());
            DescribedPredicate::describe(description, move |t: &T| {
                t.annotations().iter().any(|a| selector.matches(a))
            })
        }
    }
}

/// Predicates on [`HasOwner`] (`HasOwner.Predicates`).
pub mod has_owner {
    use super::*;

    /// `HasOwner.Predicates.With`.
    pub mod predicates {
        use super::*;

        /// Described as `owner <predicate>`.
        pub fn owner<T, O>(predicate: DescribedPredicate<O>) -> DescribedPredicate<T>
        where
            T: HasOwner<O> + ?Sized + 'static,
            O: 'static,
        {
            let description = format!("owner {}", predicate.description());
            DescribedPredicate::describe(description, move |t: &T| predicate.test(&t.owner()))
        }
    }

    /// `HasOwner.Functions.Get`.
    pub mod functions {
        use super::*;
        use crate::base::DescribedFunction;

        /// `owner()`.
        pub fn owner<T, O>() -> DescribedFunction<T, O>
        where
            T: HasOwner<O> + ?Sized + 'static,
            O: 'static,
        {
            DescribedFunction::describe("owner", |t: &T| t.owner())
        }
    }
}

/// Predicates on [`HasType`] (`HasType.Predicates`).
pub mod has_type {
    use super::*;

    /// `HasType.Predicates`.
    pub mod predicates {
        use super::*;

        /// Described as `raw type <name>` or `raw type <predicate>`.
        pub fn raw_type<T: HasType + ?Sized + 'static>(
            selector: impl Into<ItemSelector>,
        ) -> DescribedPredicate<T> {
            let selector = selector.into();
            let description = format!("raw type {}", selector.description());
            DescribedPredicate::describe(description, move |t: &T| {
                t.raw_type().is_some_and(|item| selector.matches(&item))
            })
        }
    }

    /// `HasType.Functions`.
    pub mod functions {
        use super::*;
        use crate::base::DescribedFunction;

        /// `GET_RAW_TYPE`.
        pub fn get_raw_type<T: HasType + ?Sized + 'static>()
        -> DescribedFunction<T, Option<RustItem>> {
            DescribedFunction::describe("raw type", |t: &T| t.raw_type())
        }
    }
}

/// Predicates on [`HasReturnType`] (`HasReturnType.Predicates`).
pub mod has_return_type {
    use super::*;

    /// `HasReturnType.Predicates`.
    pub mod predicates {
        use super::*;

        /// Described as `raw return type <name>` or `raw return type <predicate>`.
        pub fn raw_return_type<T: HasReturnType + ?Sized + 'static>(
            selector: impl Into<ItemSelector>,
        ) -> DescribedPredicate<T> {
            let selector = selector.into();
            let description = format!("raw return type {}", selector.description());
            DescribedPredicate::describe(description, move |t: &T| {
                t.raw_return_type()
                    .is_some_and(|item| selector.matches(&item))
            })
        }
    }

    /// `HasReturnType.Functions`.
    pub mod functions {
        use super::*;
        use crate::base::DescribedFunction;

        /// `GET_RETURN_TYPE`.
        pub fn get_return_type<T: HasReturnType + ?Sized + 'static>()
        -> DescribedFunction<T, RustType> {
            DescribedFunction::describe("return type", |t: &T| t.return_type())
        }

        /// `GET_RAW_RETURN_TYPE`.
        pub fn get_raw_return_type<T: HasReturnType + ?Sized + 'static>()
        -> DescribedFunction<T, Option<RustItem>> {
            DescribedFunction::describe("raw return type", |t: &T| t.raw_return_type())
        }
    }
}

/// Predicates on [`HasParameterTypes`] (`HasParameterTypes.Predicates`).
pub mod has_parameter_types {
    use super::*;

    /// `HasParameterTypes.Predicates`.
    pub mod predicates {
        use super::*;

        /// Matches code units whose raw parameter types are exactly `type_names` (full or simple names);
        /// described as `raw parameter types [a, b]`.
        pub fn raw_parameter_types<T: HasParameterTypes + ?Sized + 'static>(
            type_names: &[&str],
        ) -> DescribedPredicate<T> {
            let expected: Vec<ItemSelector> =
                type_names.iter().map(|n| ItemSelector::from(*n)).collect();
            let description = format!("raw parameter types [{}]", type_names.join(", "));
            DescribedPredicate::describe(description, move |t: &T| {
                let actual = t.raw_parameter_types();
                actual.len() == expected.len()
                    && actual
                        .iter()
                        .zip(&expected)
                        .all(|(a, e)| a.as_ref().is_some_and(|item| e.matches(item)))
            })
        }

        /// Matches code units whose list of raw parameter types satisfies `predicate`;
        /// described as `raw parameter types <predicate>`.
        pub fn raw_parameter_types_with<T: HasParameterTypes + ?Sized + 'static>(
            predicate: DescribedPredicate<Vec<Option<RustItem>>>,
        ) -> DescribedPredicate<T> {
            let description = format!("raw parameter types {}", predicate.description());
            DescribedPredicate::describe(description, move |t: &T| {
                predicate.test(&t.raw_parameter_types())
            })
        }
    }
}

/// Predicates on [`HasErrorTypes`] (`HasThrowsClause.Predicates`).
pub mod has_error_types {
    use super::*;

    /// `HasThrowsClause.Predicates`.
    pub mod predicates {
        use super::*;

        /// Matches code units whose error types are exactly `type_names`;
        /// described as `throws types [a, b]`.
        pub fn error_types<T: HasErrorTypes + ?Sized + 'static>(
            type_names: &[&str],
        ) -> DescribedPredicate<T> {
            let expected: Vec<ItemSelector> =
                type_names.iter().map(|n| ItemSelector::from(*n)).collect();
            let description = format!("throws types [{}]", join_single_quoted(type_names));
            DescribedPredicate::describe(description, move |t: &T| {
                let actual = t.error_types();
                actual.len() == expected.len()
                    && actual.iter().zip(&expected).all(|(a, e)| e.matches(a))
            })
        }

        /// Matches code units whose error types contain a type matching `selector`;
        /// described as `throws clause containing type <name>`.
        pub fn error_types_containing<T: HasErrorTypes + ?Sized + 'static>(
            selector: impl Into<ItemSelector>,
        ) -> DescribedPredicate<T> {
            let selector = selector.into();
            let description = format!("throws clause containing type {}", selector.description());
            DescribedPredicate::describe(description, move |t: &T| {
                t.error_types().iter().any(|item| selector.matches(item))
            })
        }
    }
}

pub(crate) fn full_match_regex(regex: &str) -> Regex {
    let anchored = format!("^(?:{regex})$");
    Regex::new(&anchored).unwrap_or_else(|e| panic!("Invalid regex '{regex}': {e}"))
}
