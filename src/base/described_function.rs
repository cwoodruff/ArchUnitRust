use std::fmt;
use std::sync::Arc;

use super::{DescribedPredicate, HasDescription};

/// A function with a description (`DescribedFunction` / `ChainableFunction`).
///
/// Used to build predicates on derived values, e.g. `get_package_name().is(matches(".."))`.
pub struct DescribedFunction<F: ?Sized, T> {
    description: String,
    apply: Arc<dyn Fn(&F) -> T + Send + Sync>,
}

impl<F: ?Sized, T> Clone for DescribedFunction<F, T> {
    fn clone(&self) -> Self {
        Self {
            description: self.description.clone(),
            apply: Arc::clone(&self.apply),
        }
    }
}

impl<F: ?Sized, T> fmt::Debug for DescribedFunction<F, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DescribedFunction")
            .field("description", &self.description)
            .finish_non_exhaustive()
    }
}

impl<F: ?Sized, T> HasDescription for DescribedFunction<F, T> {
    fn description(&self) -> String {
        self.description.clone()
    }
}

impl<F: ?Sized + 'static, T: 'static> DescribedFunction<F, T> {
    /// Creates a described function.
    pub fn describe<G>(description: impl Into<String>, apply: G) -> Self
    where
        G: Fn(&F) -> T + Send + Sync + 'static,
    {
        Self {
            description: description.into(),
            apply: Arc::new(apply),
        }
    }

    /// Applies the function.
    pub fn apply(&self, input: &F) -> T {
        (self.apply)(input)
    }

    /// The description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Overrides the description.
    pub fn as_(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Composes with another function (`ChainableFunction.then(..)`).
    pub fn then<U: 'static>(self, next: DescribedFunction<T, U>) -> DescribedFunction<F, U> {
        let description = format!("{} then {}", self.description, next.description);
        let first = self.apply;
        DescribedFunction {
            description,
            apply: Arc::new(move |f| next.apply(&first(f))),
        }
    }

    /// Builds a predicate on the result (`ChainableFunction.is(..)`).
    ///
    /// The description is `<function description> <predicate description>`.
    pub fn is(self, predicate: DescribedPredicate<T>) -> DescribedPredicate<F> {
        let description = format!("{} {}", self.description, predicate.description());
        predicate
            .on_result_of(move |f: &F| (self.apply)(f))
            .as_(description)
    }
}
