use std::sync::Arc;

use crate::base::DescribedPredicate;
use crate::lang::condition::ArchCondition;
use crate::lang::events::AsCorrespondingObject;

/// Transforms the condition of a rule before it is used (`no_classes()` negates it).
pub(crate) type PrepareCondition<T> =
    Arc<dyn Fn(ArchCondition<T>) -> ArchCondition<T> + Send + Sync>;

pub(crate) fn identity<T: 'static>() -> PrepareCondition<T> {
    Arc::new(|condition| condition)
}

/// `ArchRuleDefinition.negateCondition()`: `never(condition)` keeping the original description.
pub(crate) fn negate<T: Send + 'static>() -> PrepareCondition<T> {
    Arc::new(|condition| {
        let description = condition.description().to_owned();
        ArchCondition::never(condition).as_(description)
    })
}

/// `ObjectsShouldInternal.prependDescription("should")`.
pub(crate) fn prepend_should<T: 'static>() -> PrepareCondition<T> {
    Arc::new(|condition| {
        let description = format!("should {}", condition.description());
        condition.as_(description)
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AddMode {
    And,
    Or,
}

/// Joins the predicates of `that()`/`and()`/`or()` (`PredicateAggregator`).
#[derive(Clone)]
pub(crate) struct PredicateAggregator<T: ?Sized + 'static> {
    predicate: Option<DescribedPredicate<T>>,
    mode: AddMode,
}

impl<T: ?Sized + 'static> Default for PredicateAggregator<T> {
    fn default() -> Self {
        Self {
            predicate: None,
            mode: AddMode::And,
        }
    }
}

impl<T: ?Sized + 'static> PredicateAggregator<T> {
    pub(crate) fn add(&self, other: DescribedPredicate<T>) -> Self {
        let predicate = match (&self.predicate, self.mode) {
            (None, _) => other,
            (Some(first), AddMode::And) => first.clone().and(other),
            (Some(first), AddMode::Or) => first.clone().or(other),
        };
        Self {
            predicate: Some(predicate),
            mode: self.mode,
        }
    }

    pub(crate) fn that_ands(&self) -> Self {
        Self {
            predicate: self.predicate.clone(),
            mode: AddMode::And,
        }
    }

    pub(crate) fn that_ors(&self) -> Self {
        Self {
            predicate: self.predicate.clone(),
            mode: AddMode::Or,
        }
    }

    pub(crate) fn get(&self) -> Option<&DescribedPredicate<T>> {
        self.predicate.as_ref()
    }
}

/// Joins the conditions of `should()`/`and_should()`/`or_should()` (`ConditionAggregator`).
#[derive(Clone)]
pub(crate) struct ConditionAggregator<T: 'static> {
    condition: Option<ArchCondition<T>>,
    mode: AddMode,
    prepare: PrepareCondition<T>,
}

impl<T: AsCorrespondingObject + Send + Sync + 'static> ConditionAggregator<T> {
    pub(crate) fn new() -> Self {
        Self {
            condition: None,
            mode: AddMode::And,
            prepare: identity(),
        }
    }

    pub(crate) fn with(condition: ArchCondition<T>) -> Self {
        Self {
            condition: Some(condition),
            mode: AddMode::And,
            prepare: identity(),
        }
    }

    /// The condition combined so far.
    ///
    /// # Panics
    /// If no condition was added, which would be a bug in the syntax.
    pub(crate) fn get(&self) -> ArchCondition<T> {
        self.condition.clone().expect(
            "No condition was added to this rule, this is most likely a bug within the syntax",
        )
    }

    pub(crate) fn add(&self, other: ArchCondition<T>) -> ArchCondition<T> {
        let second = (self.prepare)(other);
        match (&self.condition, self.mode) {
            (None, _) => second,
            (Some(first), AddMode::And) => first.clone().and(second),
            (Some(first), AddMode::Or) => first.clone().or(second),
        }
    }

    pub(crate) fn that_ands_with(&self, prepare: PrepareCondition<T>) -> Self {
        Self {
            condition: self.condition.clone(),
            mode: AddMode::And,
            prepare,
        }
    }

    pub(crate) fn that_ors_with(&self, prepare: PrepareCondition<T>) -> Self {
        Self {
            condition: self.condition.clone(),
            mode: AddMode::Or,
            prepare,
        }
    }
}
