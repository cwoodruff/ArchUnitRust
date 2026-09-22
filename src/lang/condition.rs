use std::fmt;
use std::sync::{Arc, Mutex};

use super::events::{
    AndConditionEvent, AsCorrespondingObject, ConditionEvents, EvaluatedCondition,
    OrConditionEvent, SimpleConditionEvent, create_message,
};
use crate::base::{DescribedPredicate, HasDescription};
use crate::core::domain::properties::HasSourceCodeLocation;

/// Everything a condition can be checked against: it has a description and a source location
/// for messages and can be reported as a corresponding object.
pub trait ConditionTarget:
    HasDescription + HasSourceCodeLocation + AsCorrespondingObject + Send + Sync + 'static
{
}

impl<T> ConditionTarget for T where
    T: HasDescription + HasSourceCodeLocation + AsCorrespondingObject + Send + Sync + 'static
{
}

/// The behaviour behind an [`ArchCondition`]: `init` is called with all objects before the
/// checks, `check` once per object, `finish` after all checks.
///
/// Implement this for stateful conditions (counting, cycle detection); closures suffice for
/// the common per-object check (see [`ArchCondition::new`]).
pub trait ConditionLogic<T>: Send {
    /// Called once with all objects that will be checked.
    fn init(&mut self, _all_objects_to_test: &[T]) {}
    /// Checks one object, adding satisfied or violated events.
    fn check(&mut self, item: &T, events: &mut ConditionEvents);
    /// Called once after all objects were checked.
    fn finish(&mut self, _events: &mut ConditionEvents) {}
}

struct ClosureLogic<T, F>(F, std::marker::PhantomData<fn(&T)>);

impl<T, F> ConditionLogic<T> for ClosureLogic<T, F>
where
    F: Fn(&T, &mut ConditionEvents) + Send,
{
    fn check(&mut self, item: &T, events: &mut ConditionEvents) {
        (self.0)(item, events);
    }
}

/// A condition that objects of type `T` should satisfy (`ArchCondition<T>`).
///
/// Conditions are cheap to clone; clones share the underlying logic.
///
/// ```
/// use archunit::lang::{ArchCondition, ConditionEvents, SimpleConditionEvent};
/// use archunit::core::domain::RustItem;
///
/// let only_be_accessed_by_secured = ArchCondition::new(
///     "only be accessed by @secured methods",
///     |item: &RustItem, events: &mut ConditionEvents| {
///         for call in item.method_calls_to_self() {
///             if !call.origin().is_annotated_with("secured") {
///                 events.add(SimpleConditionEvent::violated(&call, format!("{} is not @secured", call.origin().full_name())));
///             }
///         }
///     },
/// );
/// assert_eq!(only_be_accessed_by_secured.description(), "only be accessed by @secured methods");
/// ```
pub struct ArchCondition<T: 'static> {
    description: String,
    logic: Arc<Mutex<dyn ConditionLogic<T>>>,
}

impl<T: 'static> Clone for ArchCondition<T> {
    fn clone(&self) -> Self {
        Self {
            description: self.description.clone(),
            logic: Arc::clone(&self.logic),
        }
    }
}

impl<T: 'static> fmt::Debug for ArchCondition<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArchCondition({})", self.description)
    }
}

impl<T: 'static> fmt::Display for ArchCondition<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.description)
    }
}

impl<T: 'static> HasDescription for ArchCondition<T> {
    fn description(&self) -> String {
        self.description.clone()
    }
}

impl<T: 'static> ArchCondition<T> {
    /// A condition checked by a closure (the anonymous-subclass idiom of ArchUnit).
    pub fn new<F>(description: impl Into<String>, check: F) -> Self
    where
        F: Fn(&T, &mut ConditionEvents) + Send + 'static,
    {
        Self::from_logic(description, ClosureLogic(check, std::marker::PhantomData))
    }

    /// A condition with custom `init`/`check`/`finish` logic.
    pub fn from_logic(
        description: impl Into<String>,
        logic: impl ConditionLogic<T> + 'static,
    ) -> Self {
        Self {
            description: description.into(),
            logic: Arc::new(Mutex::new(logic)),
        }
    }

    /// The description used in rule texts.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Overrides the description (`ArchCondition.as(..)`).
    pub fn as_(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Kept for source compatibility with ArchUnit; a no-op.
    pub fn for_subtype(self) -> Self {
        self
    }

    /// `init(..)`.
    pub fn init(&self, all_objects_to_test: &[T]) {
        self.logic
            .lock()
            .expect("condition lock")
            .init(all_objects_to_test);
    }

    /// `check(..)`.
    pub fn check(&self, item: &T, events: &mut ConditionEvents) {
        self.logic
            .lock()
            .expect("condition lock")
            .check(item, events);
    }

    /// `finish(..)`.
    pub fn finish(&self, events: &mut ConditionEvents) {
        self.logic.lock().expect("condition lock").finish(events);
    }

    /// Runs `init`, all `check`s and `finish` in one go.
    pub fn evaluate_all(&self, all_objects: &[T]) -> ConditionEvents {
        let mut events = ConditionEvents::new();
        self.init(all_objects);
        for object in all_objects {
            self.check(object, &mut events);
        }
        self.finish(&mut events);
        events
    }
}

impl<T: AsCorrespondingObject + Send + Sync + 'static> ArchCondition<T> {
    /// Conjunction (`ArchCondition.and(..)`); described as `<a> and <b>`.
    pub fn and(self, other: impl Into<ArchCondition<T>>) -> Self {
        JoinCondition::join("and", vec![self, other.into()], true)
    }

    /// Disjunction (`ArchCondition.or(..)`); described as `<a> or <b>`.
    pub fn or(self, other: impl Into<ArchCondition<T>>) -> Self {
        JoinCondition::join("or", vec![self, other.into()], false)
    }
}

impl<T: Send + 'static> ArchCondition<T> {
    /// Inverts every event of `condition` (`ArchConditions.never(..)`); described as
    /// `never <description>`.
    pub fn never(condition: ArchCondition<T>) -> Self {
        let description = format!("never {}", condition.description);
        Self::from_logic(description, NeverLogic(condition))
    }

    /// Inverts every event of `condition` (`ArchConditions.not(..)`); described as
    /// `not <description>`.
    #[allow(clippy::should_implement_trait)]
    pub fn not(condition: ArchCondition<T>) -> Self {
        let description = format!("not {}", condition.description);
        Self::never(condition).as_(description)
    }
}

struct NeverLogic<T: 'static>(ArchCondition<T>);

impl<T: Send + 'static> ConditionLogic<T> for NeverLogic<T> {
    fn init(&mut self, all: &[T]) {
        self.0.init(all);
    }

    fn check(&mut self, item: &T, events: &mut ConditionEvents) {
        let mut inner = ConditionEvents::new();
        self.0.check(item, &mut inner);
        events.add_all_inverted(inner);
    }

    fn finish(&mut self, events: &mut ConditionEvents) {
        let mut inner = ConditionEvents::new();
        self.0.finish(&mut inner);
        events.add_all_inverted(inner);
    }
}

struct JoinCondition<T: 'static> {
    conditions: Vec<ArchCondition<T>>,
    is_and: bool,
}

impl<T: AsCorrespondingObject + Send + Sync + 'static> JoinCondition<T> {
    fn join(infix: &str, conditions: Vec<ArchCondition<T>>, is_and: bool) -> ArchCondition<T> {
        let description = conditions
            .iter()
            .map(|c| c.description.clone())
            .collect::<Vec<_>>()
            .join(&format!(" {infix} "));
        ArchCondition::from_logic(description, JoinCondition { conditions, is_and })
    }
}

impl<T: AsCorrespondingObject + Send + Sync + 'static> ConditionLogic<T> for JoinCondition<T> {
    fn init(&mut self, all: &[T]) {
        for condition in &self.conditions {
            condition.init(all);
        }
    }

    fn check(&mut self, item: &T, events: &mut ConditionEvents) {
        let evaluated: Vec<EvaluatedCondition> = self
            .conditions
            .iter()
            .map(|condition| {
                let mut sub = ConditionEvents::new();
                condition.check(item, &mut sub);
                EvaluatedCondition::from_events(sub)
            })
            .collect();
        let object = item.as_corresponding_object();
        if self.is_and {
            events.add(AndConditionEvent::new(object, evaluated));
        } else {
            events.add(OrConditionEvent::new(object, evaluated));
        }
    }

    fn finish(&mut self, events: &mut ConditionEvents) {
        for condition in &self.conditions {
            condition.finish(events);
        }
    }
}

/// Describes the event of a [`ConditionByPredicate`]: receives the predicate description and
/// whether it was satisfied, returns the message part after the object description.
pub type EventDescriber = Arc<dyn Fn(&str, bool) -> String + Send + Sync>;

/// A condition derived from a predicate (`ArchCondition.ConditionByPredicate`).
///
/// Each object produces one event `<object> <message> in <location>`, where the message
/// comes from the [`EventDescriber`] (default: `satisfies <p>` / `does not satisfy <p>`).
#[derive(Clone)]
pub struct ConditionByPredicate<T: 'static> {
    predicate: DescribedPredicate<T>,
    description: String,
    event_describer: EventDescriber,
}

impl<T: 'static> fmt::Debug for ConditionByPredicate<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConditionByPredicate({})", self.description)
    }
}

impl<T: ConditionTarget> ConditionByPredicate<T> {
    /// `ArchCondition.from(predicate)`.
    pub fn from(predicate: DescribedPredicate<T>) -> Self {
        let description = predicate.description().to_owned();
        Self {
            predicate,
            description,
            event_describer: Arc::new(|predicate_description, satisfied| {
                format!(
                    "{}{predicate_description}",
                    if satisfied {
                        "satisfies "
                    } else {
                        "does not satisfy "
                    }
                )
            }),
        }
    }

    /// Changes how events are described (`describeEventsBy(..)`).
    pub fn describe_events_by(
        mut self,
        describer: impl Fn(&str, bool) -> String + Send + Sync + 'static,
    ) -> Self {
        self.event_describer = Arc::new(describer);
        self
    }

    /// Overrides the description.
    pub fn as_(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// The description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Kept for source compatibility; a no-op.
    pub fn for_subtype(self) -> Self {
        self
    }

    /// Converts into a plain [`ArchCondition`].
    pub fn into_condition(self) -> ArchCondition<T> {
        ArchCondition::from(self)
    }
}

impl<T: ConditionTarget> From<ConditionByPredicate<T>> for ArchCondition<T> {
    fn from(condition: ConditionByPredicate<T>) -> Self {
        let ConditionByPredicate {
            predicate,
            description,
            event_describer,
        } = condition;
        ArchCondition::new(
            description,
            move |object: &T, events: &mut ConditionEvents| {
                let satisfied = predicate.test(object);
                let message =
                    create_message(object, &event_describer(predicate.description(), satisfied));
                events.add(SimpleConditionEvent::new(
                    object.as_corresponding_object(),
                    satisfied,
                    message,
                ));
            },
        )
    }
}
