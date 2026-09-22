use std::any::Any;
use std::fmt;
use std::sync::Arc;

use crate::core::domain::{
    Dependency, RustAccess, RustCodeUnit, RustConstructor, RustField, RustItem, RustMember,
    RustMethod, RustModule, RustVariant,
};

/// The object a [`ConditionEvent`] is about (Java passes an untyped `Object`).
#[derive(Clone)]
pub enum CorrespondingObject {
    /// An item.
    Item(RustItem),
    /// A module.
    Module(RustModule),
    /// A member.
    Member(RustMember),
    /// An access.
    Access(RustAccess),
    /// A dependency.
    Dependency(Dependency),
    /// A plain text, e.g. a count.
    Text(String),
    /// Any other value, e.g. a slice or cycle from the library layer.
    Other(Arc<dyn Any + Send + Sync>),
}

impl fmt::Debug for CorrespondingObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CorrespondingObject::Item(i) => write!(f, "{i:?}"),
            CorrespondingObject::Module(m) => write!(f, "{m:?}"),
            CorrespondingObject::Member(m) => write!(f, "{m:?}"),
            CorrespondingObject::Access(a) => write!(f, "{a:?}"),
            CorrespondingObject::Dependency(d) => write!(f, "{d:?}"),
            CorrespondingObject::Text(t) => write!(f, "{t:?}"),
            CorrespondingObject::Other(_) => f.write_str("Other(..)"),
        }
    }
}

impl CorrespondingObject {
    /// Wraps an arbitrary value.
    pub fn other<T: Any + Send + Sync>(value: T) -> Self {
        CorrespondingObject::Other(Arc::new(value))
    }

    /// Downcasts an [`Other`](Self::Other) value.
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        match self {
            CorrespondingObject::Other(any) => any.downcast_ref::<T>(),
            _ => None,
        }
    }
}

/// Conversion into a [`CorrespondingObject`], implemented for every domain type.
///
/// Implement it for custom rule inputs (custom `ClassesTransformer` targets) so they can be
/// combined with `and`/`or` and reported through `handle_violations`.
pub trait AsCorrespondingObject {
    /// The corresponding object.
    fn as_corresponding_object(&self) -> CorrespondingObject;
}

macro_rules! corresponding {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl AsCorrespondingObject for $ty {
                fn as_corresponding_object(&self) -> CorrespondingObject {
                    CorrespondingObject::$variant(self.clone())
                }
            }
            impl From<$ty> for CorrespondingObject {
                fn from(value: $ty) -> Self {
                    CorrespondingObject::$variant(value)
                }
            }
            impl From<&$ty> for CorrespondingObject {
                fn from(value: &$ty) -> Self {
                    CorrespondingObject::$variant(value.clone())
                }
            }
        )*
    };
}

corresponding!(
    RustItem => Item,
    RustModule => Module,
    RustMember => Member,
    RustAccess => Access,
    Dependency => Dependency,
    String => Text,
);

macro_rules! corresponding_view {
    ($($ty:ty),* $(,)?) => {
        $(
            impl AsCorrespondingObject for $ty {
                fn as_corresponding_object(&self) -> CorrespondingObject {
                    CorrespondingObject::Member(self.0.clone())
                }
            }
            impl From<$ty> for CorrespondingObject {
                fn from(value: $ty) -> Self {
                    CorrespondingObject::Member(value.0)
                }
            }
            impl From<&$ty> for CorrespondingObject {
                fn from(value: &$ty) -> Self {
                    CorrespondingObject::Member(value.0.clone())
                }
            }
        )*
    };
}

corresponding_view!(
    RustField,
    RustCodeUnit,
    RustMethod,
    RustConstructor,
    RustVariant
);

impl AsCorrespondingObject for &str {
    fn as_corresponding_object(&self) -> CorrespondingObject {
        CorrespondingObject::Text((*self).to_owned())
    }
}

impl From<&str> for CorrespondingObject {
    fn from(value: &str) -> Self {
        CorrespondingObject::Text(value.to_owned())
    }
}

impl AsCorrespondingObject for usize {
    fn as_corresponding_object(&self) -> CorrespondingObject {
        CorrespondingObject::Text(self.to_string())
    }
}

impl From<usize> for CorrespondingObject {
    fn from(value: usize) -> Self {
        CorrespondingObject::Text(value.to_string())
    }
}

impl AsCorrespondingObject for CorrespondingObject {
    fn as_corresponding_object(&self) -> CorrespondingObject {
        self.clone()
    }
}

/// Extracts typed objects from [`CorrespondingObject`]s (Java's reified `ViolationHandler<T>`
/// and `Convertible`).
pub trait FromCorrespondingObject: Sized {
    /// The values of this type represented by `object`, possibly none.
    fn from_corresponding_object(object: &CorrespondingObject) -> Vec<Self>;
}

impl FromCorrespondingObject for RustItem {
    fn from_corresponding_object(object: &CorrespondingObject) -> Vec<Self> {
        match object {
            CorrespondingObject::Item(i) => vec![i.clone()],
            _ => Vec::new(),
        }
    }
}

impl FromCorrespondingObject for RustMember {
    fn from_corresponding_object(object: &CorrespondingObject) -> Vec<Self> {
        match object {
            CorrespondingObject::Member(m) => vec![m.clone()],
            _ => Vec::new(),
        }
    }
}

impl FromCorrespondingObject for RustAccess {
    fn from_corresponding_object(object: &CorrespondingObject) -> Vec<Self> {
        match object {
            CorrespondingObject::Access(a) => vec![a.clone()],
            _ => Vec::new(),
        }
    }
}

impl FromCorrespondingObject for Dependency {
    fn from_corresponding_object(object: &CorrespondingObject) -> Vec<Self> {
        match object {
            CorrespondingObject::Dependency(d) => vec![d.clone()],
            CorrespondingObject::Access(a) => a
                .origin_owner()
                .direct_dependencies_from_self()
                .into_iter()
                .filter(|d| d.description() == a.description())
                .collect(),
            _ => Vec::new(),
        }
    }
}

impl FromCorrespondingObject for String {
    fn from_corresponding_object(object: &CorrespondingObject) -> Vec<Self> {
        match object {
            CorrespondingObject::Text(t) => vec![t.clone()],
            _ => Vec::new(),
        }
    }
}

/// A list of boxed events.
pub type BoxedEvents = Vec<Box<dyn ConditionEvent>>;

/// An event produced by checking a condition against one object (`ConditionEvent`).
pub trait ConditionEvent: Send + Sync + fmt::Debug {
    /// Whether the condition was violated.
    fn is_violation(&self) -> bool;
    /// The same event with satisfied and violated swapped (used by `never(..)`).
    fn invert(&self) -> Box<dyn ConditionEvent>;
    /// The lines reported for this event.
    fn description_lines(&self) -> Vec<String>;
    /// Passes the corresponding objects and message to `handler`.
    fn handle_with(&self, handler: &mut dyn FnMut(&[CorrespondingObject], &str));
    /// A boxed copy.
    fn clone_boxed(&self) -> Box<dyn ConditionEvent>;
}

impl Clone for Box<dyn ConditionEvent> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

/// Builds the standard message `<object description> <message> in <location>`
/// (`ConditionEvent.createMessage(..)`).
pub fn create_message<T>(object: &T, message: &str) -> String
where
    T: crate::base::HasDescription
        + crate::core::domain::properties::HasSourceCodeLocation
        + ?Sized,
{
    format!(
        "{} {message} in {}",
        object.description(),
        object.source_code_location()
    )
}

/// A single satisfied or violated event with one message (`SimpleConditionEvent`).
#[derive(Clone, Debug)]
pub struct SimpleConditionEvent {
    object: CorrespondingObject,
    satisfied: bool,
    message: String,
}

impl SimpleConditionEvent {
    /// Creates an event. Violations must have a non-empty message.
    pub fn new(
        object: impl Into<CorrespondingObject>,
        satisfied: bool,
        message: impl Into<String>,
    ) -> Self {
        let message = message.into();
        assert!(
            satisfied || !message.trim().is_empty(),
            "Message may not be empty for violation"
        );
        Self {
            object: object.into(),
            satisfied,
            message,
        }
    }

    /// A violated event (`SimpleConditionEvent.violated(..)`).
    pub fn violated(object: impl Into<CorrespondingObject>, message: impl Into<String>) -> Self {
        Self::new(object, false, message)
    }

    /// A satisfied event (`SimpleConditionEvent.satisfied(..)`).
    pub fn satisfied(object: impl Into<CorrespondingObject>, message: impl Into<String>) -> Self {
        Self::new(object, true, message)
    }

    /// The message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The object the event is about.
    pub fn corresponding_object(&self) -> &CorrespondingObject {
        &self.object
    }
}

impl ConditionEvent for SimpleConditionEvent {
    fn is_violation(&self) -> bool {
        !self.satisfied
    }

    fn invert(&self) -> Box<dyn ConditionEvent> {
        Box::new(Self {
            object: self.object.clone(),
            satisfied: !self.satisfied,
            message: self.message.clone(),
        })
    }

    fn description_lines(&self) -> Vec<String> {
        vec![self.message.clone()]
    }

    fn handle_with(&self, handler: &mut dyn FnMut(&[CorrespondingObject], &str)) {
        handler(std::slice::from_ref(&self.object), &self.message);
    }

    fn clone_boxed(&self) -> Box<dyn ConditionEvent> {
        Box::new(self.clone())
    }
}

/// Collects the events of a condition check (`ConditionEvents`).
///
/// Both satisfied and violated events are kept so that composite conditions can invert them.
#[derive(Debug, Default)]
pub struct ConditionEvents {
    allowed: Vec<Box<dyn ConditionEvent>>,
    violating: Vec<Box<dyn ConditionEvent>>,
    information_about_number_of_violations: Option<String>,
}

impl ConditionEvents {
    /// An empty collection (`ConditionEvents.Factory.create()`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an event.
    pub fn add(&mut self, event: impl ConditionEvent + 'static) {
        self.add_boxed(Box::new(event));
    }

    /// Adds an already boxed event.
    pub fn add_boxed(&mut self, event: Box<dyn ConditionEvent>) {
        if event.is_violation() {
            self.violating.push(event);
        } else {
            self.allowed.push(event);
        }
    }

    /// The violated events (`getViolating()`).
    pub fn violating(&self) -> &[Box<dyn ConditionEvent>] {
        &self.violating
    }

    /// The satisfied events.
    pub fn allowed(&self) -> &[Box<dyn ConditionEvent>] {
        &self.allowed
    }

    /// `containViolation()`.
    pub fn contain_violation(&self) -> bool {
        !self.violating.is_empty()
    }

    /// Custom text for the `(N times)` part of the report, if set.
    pub fn information_about_number_of_violations(&self) -> Option<&str> {
        self.information_about_number_of_violations.as_deref()
    }

    /// Overrides the `(N times)` part of the report, e.g. for cycle counts.
    pub fn set_information_about_number_of_violations(&mut self, information: impl Into<String>) {
        self.information_about_number_of_violations = Some(information.into());
    }

    /// Moves every event out, satisfied ones first.
    pub fn into_all_events(self) -> Vec<Box<dyn ConditionEvent>> {
        let mut all = self.allowed;
        all.extend(self.violating);
        all
    }

    pub(crate) fn into_parts(self) -> (BoxedEvents, BoxedEvents, Option<String>) {
        (
            self.allowed,
            self.violating,
            self.information_about_number_of_violations,
        )
    }

    /// Adds all events of `other`, inverted (used by `never(..)`).
    pub fn add_all_inverted(&mut self, other: ConditionEvents) {
        let (allowed, violating, info) = other.into_parts();
        for event in allowed.into_iter().chain(violating) {
            self.add_boxed(event.invert());
        }
        if let Some(info) = info {
            self.information_about_number_of_violations = Some(info);
        }
    }

    /// Adds all events of `other`.
    pub fn add_all(&mut self, other: ConditionEvents) {
        let (allowed, violating, info) = other.into_parts();
        self.allowed.extend(allowed);
        self.violating.extend(violating);
        if let Some(info) = info {
            self.information_about_number_of_violations = Some(info);
        }
    }
}

fn describe(events: &[Box<dyn ConditionEvent>]) -> String {
    events
        .iter()
        .flat_map(|e| e.description_lines())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The event of `contain any element that <condition>`: violated when no element satisfied
/// the condition (`ContainAnyCondition.AnyConditionEvent`).
#[derive(Debug, Clone)]
pub(crate) struct AnyConditionEvent {
    objects: Vec<CorrespondingObject>,
    allowed: Vec<Box<dyn ConditionEvent>>,
    violating: Vec<Box<dyn ConditionEvent>>,
}

impl AnyConditionEvent {
    pub(crate) fn new(
        objects: Vec<CorrespondingObject>,
        allowed: Vec<Box<dyn ConditionEvent>>,
        violating: Vec<Box<dyn ConditionEvent>>,
    ) -> Self {
        Self {
            objects,
            allowed,
            violating,
        }
    }
}

impl ConditionEvent for AnyConditionEvent {
    fn is_violation(&self) -> bool {
        self.allowed.is_empty()
    }

    fn invert(&self) -> Box<dyn ConditionEvent> {
        Box::new(OnlyConditionEvent::new(
            self.objects.clone(),
            self.violating.clone(),
            self.allowed.clone(),
        ))
    }

    fn description_lines(&self) -> Vec<String> {
        vec![describe(&self.violating)]
    }

    fn handle_with(&self, handler: &mut dyn FnMut(&[CorrespondingObject], &str)) {
        handler(&self.objects, &describe(&self.violating));
    }

    fn clone_boxed(&self) -> Box<dyn ConditionEvent> {
        Box::new(self.clone())
    }
}

/// The event of `contain only elements that <condition>`: violated when any element violated
/// the condition (`ContainsOnlyCondition.OnlyConditionEvent`).
#[derive(Debug, Clone)]
pub(crate) struct OnlyConditionEvent {
    objects: Vec<CorrespondingObject>,
    allowed: Vec<Box<dyn ConditionEvent>>,
    violating: Vec<Box<dyn ConditionEvent>>,
}

impl OnlyConditionEvent {
    pub(crate) fn new(
        objects: Vec<CorrespondingObject>,
        allowed: Vec<Box<dyn ConditionEvent>>,
        violating: Vec<Box<dyn ConditionEvent>>,
    ) -> Self {
        Self {
            objects,
            allowed,
            violating,
        }
    }
}

impl ConditionEvent for OnlyConditionEvent {
    fn is_violation(&self) -> bool {
        !self.violating.is_empty()
    }

    fn invert(&self) -> Box<dyn ConditionEvent> {
        Box::new(AnyConditionEvent::new(
            self.objects.clone(),
            self.violating.clone(),
            self.allowed.clone(),
        ))
    }

    fn description_lines(&self) -> Vec<String> {
        self.violating
            .iter()
            .flat_map(|e| e.description_lines())
            .collect()
    }

    fn handle_with(&self, handler: &mut dyn FnMut(&[CorrespondingObject], &str)) {
        for event in &self.violating {
            event.handle_with(handler);
        }
    }

    fn clone_boxed(&self) -> Box<dyn ConditionEvent> {
        Box::new(self.clone())
    }
}

/// The evaluated events of one sub-condition of an `and`/`or` (`JoinCondition.ConditionWithEvents`).
#[derive(Debug, Clone)]
pub(crate) struct EvaluatedCondition {
    pub allowed: Vec<Box<dyn ConditionEvent>>,
    pub violating: Vec<Box<dyn ConditionEvent>>,
}

impl EvaluatedCondition {
    pub(crate) fn from_events(events: ConditionEvents) -> Self {
        let (allowed, violating, _) = events.into_parts();
        Self { allowed, violating }
    }

    fn contain_violation(&self) -> bool {
        !self.violating.is_empty()
    }

    fn invert(&self) -> Self {
        let mut events = ConditionEvents::new();
        for event in self.allowed.iter().chain(self.violating.iter()) {
            events.add_boxed(event.invert());
        }
        Self::from_events(events)
    }
}

fn unique_lines_of_violations(evaluated: &[EvaluatedCondition]) -> Vec<String> {
    let mut lines: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for evaluation in evaluated {
        for event in &evaluation.violating {
            lines.extend(event.description_lines());
        }
    }
    lines.into_iter().collect()
}

/// `AndCondition.AndConditionEvent`.
#[derive(Debug, Clone)]
pub(crate) struct AndConditionEvent {
    object: CorrespondingObject,
    evaluated: Vec<EvaluatedCondition>,
}

impl AndConditionEvent {
    pub(crate) fn new(object: CorrespondingObject, evaluated: Vec<EvaluatedCondition>) -> Self {
        Self { object, evaluated }
    }
}

impl ConditionEvent for AndConditionEvent {
    fn is_violation(&self) -> bool {
        self.evaluated
            .iter()
            .any(EvaluatedCondition::contain_violation)
    }

    fn invert(&self) -> Box<dyn ConditionEvent> {
        Box::new(OrConditionEvent::new(
            self.object.clone(),
            self.evaluated
                .iter()
                .map(EvaluatedCondition::invert)
                .collect(),
        ))
    }

    fn description_lines(&self) -> Vec<String> {
        unique_lines_of_violations(&self.evaluated)
    }

    fn handle_with(&self, handler: &mut dyn FnMut(&[CorrespondingObject], &str)) {
        for evaluation in &self.evaluated {
            for event in &evaluation.violating {
                event.handle_with(handler);
            }
        }
    }

    fn clone_boxed(&self) -> Box<dyn ConditionEvent> {
        Box::new(self.clone())
    }
}

/// `OrCondition.OrConditionEvent`.
#[derive(Debug, Clone)]
pub(crate) struct OrConditionEvent {
    object: CorrespondingObject,
    evaluated: Vec<EvaluatedCondition>,
}

impl OrConditionEvent {
    pub(crate) fn new(object: CorrespondingObject, evaluated: Vec<EvaluatedCondition>) -> Self {
        Self { object, evaluated }
    }

    fn create_message(&self) -> String {
        unique_lines_of_violations(&self.evaluated).join(" and ")
    }
}

impl ConditionEvent for OrConditionEvent {
    fn is_violation(&self) -> bool {
        self.evaluated
            .iter()
            .all(EvaluatedCondition::contain_violation)
    }

    fn invert(&self) -> Box<dyn ConditionEvent> {
        Box::new(AndConditionEvent::new(
            self.object.clone(),
            self.evaluated
                .iter()
                .map(EvaluatedCondition::invert)
                .collect(),
        ))
    }

    fn description_lines(&self) -> Vec<String> {
        vec![self.create_message()]
    }

    fn handle_with(&self, handler: &mut dyn FnMut(&[CorrespondingObject], &str)) {
        handler(std::slice::from_ref(&self.object), &self.create_message());
    }

    fn clone_boxed(&self) -> Box<dyn ConditionEvent> {
        Box::new(self.clone())
    }
}
