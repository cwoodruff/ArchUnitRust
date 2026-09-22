use super::aggregators::{ConditionAggregator, PredicateAggregator, PrepareCondition};
use crate::base::DescribedPredicate;
use crate::core::domain::RustItems;
use crate::lang::condition::ArchCondition;
use crate::lang::events::AsCorrespondingObject;
use crate::lang::rule::{ClassesTransformer, Priority, SimpleArchRule};

/// The shared state of every `given` step: how objects are produced, which predicates
/// restrict them, and how the final condition is prepared (`AbstractGivenObjects`).
#[derive(Clone)]
pub(crate) struct GivenState<T: 'static> {
    pub(crate) priority: Priority,
    pub(crate) transformer: ClassesTransformer<T>,
    pub(crate) prepare: PrepareCondition<T>,
    pub(crate) predicates: PredicateAggregator<T>,
    pub(crate) overridden_description: Option<String>,
}

impl<T: Send + Sync + 'static> GivenState<T> {
    pub(crate) fn new(
        priority: Priority,
        transformer: ClassesTransformer<T>,
        prepare: PrepareCondition<T>,
    ) -> Self {
        Self {
            priority,
            transformer,
            prepare,
            predicates: PredicateAggregator::default(),
            overridden_description: None,
        }
    }

    pub(crate) fn with(mut self, predicates: PredicateAggregator<T>) -> Self {
        self.predicates = predicates;
        self
    }

    pub(crate) fn finished_transformer(&self) -> ClassesTransformer<T> {
        let transformer = match self.predicates.get() {
            Some(predicate) => self.transformer.that(predicate.clone()),
            None => self.transformer.clone(),
        };
        match &self.overridden_description {
            Some(description) => transformer.as_(description.clone()),
            None => transformer,
        }
    }
}

/// The shared state of every `should` step (`ObjectsShouldInternal`).
#[derive(Clone)]
pub(crate) struct ShouldState<T: 'static> {
    pub(crate) transformer: ClassesTransformer<T>,
    pub(crate) priority: Priority,
    pub(crate) aggregator: ConditionAggregator<T>,
    pub(crate) prepare: PrepareCondition<T>,
}

impl<T: AsCorrespondingObject + Send + Sync + 'static> ShouldState<T> {
    pub(crate) fn finished_rule(&self) -> SimpleArchRule<T> {
        let condition = (self.prepare)(self.aggregator.get());
        SimpleArchRule::create(self.transformer.clone(), condition, self.priority)
    }

    pub(crate) fn add_condition(&self, condition: ArchCondition<T>) -> Self {
        Self {
            transformer: self.transformer.clone(),
            priority: self.priority,
            aggregator: ConditionAggregator::with(self.aggregator.add(condition)),
            prepare: self.prepare.clone(),
        }
    }

    pub(crate) fn with_aggregator(&self, aggregator: ConditionAggregator<T>) -> Self {
        Self {
            transformer: self.transformer.clone(),
            priority: self.priority,
            aggregator,
            prepare: self.prepare.clone(),
        }
    }
}

/// Rules about custom objects produced by a [`ClassesTransformer`]
/// (`GivenObjects<T>` / `GivenConjunction<T>`).
///
/// ```no_run
/// use archunit::lang::ClassesTransformer;
/// use archunit::lang::syntax::all;
/// use archunit::core::domain::RustModule;
/// # use archunit::lang::ArchCondition;
/// # let contain_a_core_item = archunit::base::always_true::<RustModule>();
/// # let be_independent: ArchCondition<RustModule> = ArchCondition::new("x", |_, _| {});
///
/// let packages = ClassesTransformer::new("packages", |items| items.modules());
/// all(packages).that(contain_a_core_item).should(be_independent);
/// ```
#[derive(Clone)]
pub struct GivenObjects<T: 'static> {
    state: GivenState<T>,
}

/// `GivenConjunction<T>`: the same type as [`GivenObjects`].
pub type GivenConjunction<T> = GivenObjects<T>;

impl<T: 'static> std::fmt::Debug for GivenObjects<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GivenObjects({})", self.state.transformer.description())
    }
}

impl<T: AsCorrespondingObject + Send + Sync + 'static> GivenObjects<T> {
    pub(crate) fn new(
        priority: Priority,
        transformer: ClassesTransformer<T>,
        prepare: PrepareCondition<T>,
    ) -> Self {
        Self {
            state: GivenState::new(priority, transformer, prepare),
        }
    }

    /// Restricts the objects (`that(predicate)`).
    pub fn that(self, predicate: DescribedPredicate<T>) -> GivenConjunction<T> {
        let predicates = self.state.predicates.add(predicate);
        Self {
            state: self.state.with(predicates),
        }
    }

    /// `and(predicate)`.
    pub fn and(self, predicate: DescribedPredicate<T>) -> GivenConjunction<T> {
        let predicates = self.state.predicates.that_ands().add(predicate);
        Self {
            state: self.state.with(predicates),
        }
    }

    /// `or(predicate)`.
    pub fn or(self, predicate: DescribedPredicate<T>) -> GivenConjunction<T> {
        let predicates = self.state.predicates.that_ors().add(predicate);
        Self {
            state: self.state.with(predicates),
        }
    }

    /// Overrides the description of the objects (`as(..)`).
    pub fn as_(mut self, description: &str) -> Self {
        self.state.overridden_description = Some(description.to_owned());
        self
    }

    /// Finishes the rule (`should(condition)`).
    pub fn should(self, condition: impl Into<ArchCondition<T>>) -> SimpleArchRule<T> {
        let condition = (self.state.prepare)(condition.into());
        SimpleArchRule::create(
            self.state.finished_transformer(),
            condition,
            self.state.priority,
        )
    }
}

impl<T: AsCorrespondingObject + Send + Sync + 'static> GivenObjects<T> {
    /// Evaluates a rule built from these objects against `items` without the fluent syntax.
    pub fn transform(&self, items: &RustItems) -> Vec<T> {
        self.state.finished_transformer().transform(items)
    }
}
