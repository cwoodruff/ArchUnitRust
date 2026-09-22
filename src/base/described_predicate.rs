use std::fmt;
use std::sync::Arc;

use super::HasDescription;

/// A predicate together with a description that is used to build rule texts
/// (`DescribedPredicate<T>`).
///
/// Predicates are cheap to clone; they share the underlying closure.
///
/// ```
/// use archunit::base::DescribedPredicate;
///
/// let even: DescribedPredicate<u32> = DescribedPredicate::describe("even", |n| n % 2 == 0);
/// let small: DescribedPredicate<u32> = DescribedPredicate::describe("small", |n| *n < 10);
/// let both = even.and(small);
/// assert_eq!(both.description(), "even and small");
/// assert!(both.test(&4));
/// assert!(!both.test(&12));
/// ```
pub struct DescribedPredicate<T: ?Sized> {
    description: String,
    test: Arc<dyn Fn(&T) -> bool + Send + Sync>,
}

impl<T: ?Sized> Clone for DescribedPredicate<T> {
    fn clone(&self) -> Self {
        Self {
            description: self.description.clone(),
            test: Arc::clone(&self.test),
        }
    }
}

impl<T: ?Sized> fmt::Debug for DescribedPredicate<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DescribedPredicate")
            .field("description", &self.description)
            .finish_non_exhaustive()
    }
}

impl<T: ?Sized> fmt::Display for DescribedPredicate<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.description)
    }
}

impl<T: ?Sized> HasDescription for DescribedPredicate<T> {
    fn description(&self) -> String {
        self.description.clone()
    }
}

impl<T: ?Sized + 'static> DescribedPredicate<T> {
    /// Creates a predicate from a description and a closure (`DescribedPredicate.describe(..)`).
    pub fn describe<F>(description: impl Into<String>, test: F) -> Self
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        Self {
            description: description.into(),
            test: Arc::new(test),
        }
    }

    /// Evaluates the predicate.
    pub fn test(&self, input: &T) -> bool {
        (self.test)(input)
    }

    /// The description used in rule texts.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Overrides the description (`DescribedPredicate.as(..)`).
    pub fn as_(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Negates the predicate; the description becomes `not <description>`.
    pub fn negate(self) -> Self {
        let description = format!("not {}", self.description);
        let inner = self.test;
        Self {
            description,
            test: Arc::new(move |t| !inner(t)),
        }
    }

    /// Conjunction; the description becomes `<a> and <b>`.
    pub fn and(self, other: DescribedPredicate<T>) -> Self {
        let description = format!("{} and {}", self.description, other.description);
        let (a, b) = (self.test, other.test);
        Self {
            description,
            test: Arc::new(move |t| a(t) && b(t)),
        }
    }

    /// Disjunction; the description becomes `<a> or <b>`.
    pub fn or(self, other: DescribedPredicate<T>) -> Self {
        let description = format!("{} or {}", self.description, other.description);
        let (a, b) = (self.test, other.test);
        Self {
            description,
            test: Arc::new(move |t| a(t) || b(t)),
        }
    }

    /// Lifts the predicate to another input type by applying `function` first
    /// (`DescribedPredicate.onResultOf(..)`). The description is kept.
    pub fn on_result_of<F, G>(self, function: G) -> DescribedPredicate<F>
    where
        T: Sized,
        F: ?Sized + 'static,
        G: Fn(&F) -> T + Send + Sync + 'static,
    {
        let inner = self.test;
        DescribedPredicate {
            description: self.description,
            test: Arc::new(move |f| inner(&function(f))),
        }
    }

    /// Kept for source compatibility with ArchUnit; Rust has no subtyping, so this is a no-op.
    pub fn for_subtype(self) -> Self {
        self
    }
}

/// `DescribedPredicate.describe(description, predicate)`.
pub fn describe<T: ?Sized + 'static, F>(
    description: impl Into<String>,
    test: F,
) -> DescribedPredicate<T>
where
    F: Fn(&T) -> bool + Send + Sync + 'static,
{
    DescribedPredicate::describe(description, test)
}

/// A predicate that is always satisfied; described as `always true`.
pub fn always_true<T: ?Sized + 'static>() -> DescribedPredicate<T> {
    DescribedPredicate::describe("always true", |_| true)
}

/// A predicate that is never satisfied; described as `always false`.
pub fn always_false<T: ?Sized + 'static>() -> DescribedPredicate<T> {
    DescribedPredicate::describe("always false", |_| false)
}

/// Described as `equal to '<value>'`.
pub fn equal_to<T>(value: T) -> DescribedPredicate<T>
where
    T: PartialEq + fmt::Display + Send + Sync + 'static,
{
    let description = format!("equal to '{value}'");
    DescribedPredicate::describe(description, move |t| *t == value)
}

/// Described as `less than '<value>'`.
pub fn less_than<T>(value: T) -> DescribedPredicate<T>
where
    T: PartialOrd + fmt::Display + Send + Sync + 'static,
{
    let description = format!("less than '{value}'");
    DescribedPredicate::describe(description, move |t| *t < value)
}

/// Described as `greater than '<value>'`.
pub fn greater_than<T>(value: T) -> DescribedPredicate<T>
where
    T: PartialOrd + fmt::Display + Send + Sync + 'static,
{
    let description = format!("greater than '{value}'");
    DescribedPredicate::describe(description, move |t| *t > value)
}

/// Described as `less than or equal to '<value>'`.
pub fn less_than_or_equal_to<T>(value: T) -> DescribedPredicate<T>
where
    T: PartialOrd + fmt::Display + Send + Sync + 'static,
{
    let description = format!("less than or equal to '{value}'");
    DescribedPredicate::describe(description, move |t| *t <= value)
}

/// Described as `greater than or equal to '<value>'`.
pub fn greater_than_or_equal_to<T>(value: T) -> DescribedPredicate<T>
where
    T: PartialOrd + fmt::Display + Send + Sync + 'static,
{
    let description = format!("greater than or equal to '{value}'");
    DescribedPredicate::describe(description, move |t| *t >= value)
}

/// Negation described as `does not <description>`.
pub fn does_not<T: ?Sized + 'static>(predicate: DescribedPredicate<T>) -> DescribedPredicate<T> {
    let description = format!("does not {}", predicate.description);
    predicate.negate().as_(description)
}

/// Negation described as `do not <description>`.
pub fn do_not<T: ?Sized + 'static>(predicate: DescribedPredicate<T>) -> DescribedPredicate<T> {
    let description = format!("do not {}", predicate.description);
    predicate.negate().as_(description)
}

/// Negation described as `not <description>`.
pub fn not<T: ?Sized + 'static>(predicate: DescribedPredicate<T>) -> DescribedPredicate<T> {
    predicate.negate()
}

/// Conjunction of several predicates (`DescribedPredicate.and(Iterable)`).
///
/// Panics if `predicates` is empty, like the Java original.
pub fn and<T: ?Sized + 'static>(
    predicates: impl IntoIterator<Item = DescribedPredicate<T>>,
) -> DescribedPredicate<T> {
    let mut iter = predicates.into_iter();
    let first = iter.next().expect("and(..) needs at least one predicate");
    iter.fold(first, DescribedPredicate::and)
}

/// Alias of [`and`] for readability when combining a list.
pub fn all_of<T: ?Sized + 'static>(
    predicates: impl IntoIterator<Item = DescribedPredicate<T>>,
) -> DescribedPredicate<T> {
    and(predicates)
}

/// Disjunction of several predicates (`DescribedPredicate.or(Iterable)`).
///
/// Panics if `predicates` is empty, like the Java original.
pub fn or<T: ?Sized + 'static>(
    predicates: impl IntoIterator<Item = DescribedPredicate<T>>,
) -> DescribedPredicate<T> {
    let mut iter = predicates.into_iter();
    let first = iter.next().expect("or(..) needs at least one predicate");
    iter.fold(first, DescribedPredicate::or)
}

/// Alias of [`or`] for readability when combining a list.
pub fn any_of<T: ?Sized + 'static>(
    predicates: impl IntoIterator<Item = DescribedPredicate<T>>,
) -> DescribedPredicate<T> {
    or(predicates)
}

/// Matches empty collections; described as `empty`.
pub fn empty<T: 'static>() -> DescribedPredicate<Vec<T>> {
    DescribedPredicate::describe("empty", |v: &Vec<T>| v.is_empty())
}

/// Matches collections with at least one element satisfying `predicate`;
/// described as `any element that <description>`.
pub fn any_element_that<T: 'static>(
    predicate: DescribedPredicate<T>,
) -> DescribedPredicate<Vec<T>> {
    let description = format!("any element that {}", predicate.description);
    DescribedPredicate::describe(description, move |v: &Vec<T>| {
        v.iter().any(|t| predicate.test(t))
    })
}

/// Matches collections whose elements all satisfy `predicate`;
/// described as `all elements <description>`.
pub fn all_elements<T: 'static>(predicate: DescribedPredicate<T>) -> DescribedPredicate<Vec<T>> {
    let description = format!("all elements {}", predicate.description);
    DescribedPredicate::describe(description, move |v: &Vec<T>| {
        v.iter().all(|t| predicate.test(t))
    })
}

/// Matches `Some(value)` where `value` satisfies `predicate`;
/// described as `optional contains <description>`.
pub fn optional_contains<T: 'static>(
    predicate: DescribedPredicate<T>,
) -> DescribedPredicate<Option<T>> {
    let description = format!("optional contains {}", predicate.description);
    DescribedPredicate::describe(description, move |o: &Option<T>| {
        o.as_ref().is_some_and(|t| predicate.test(t))
    })
}

/// Matches `None`; described as `optional empty`.
pub fn optional_empty<T: 'static>() -> DescribedPredicate<Option<T>> {
    DescribedPredicate::describe("optional empty", |o: &Option<T>| o.is_none())
}
