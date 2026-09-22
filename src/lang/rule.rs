use std::fmt;
use std::sync::Arc;

use super::condition::ArchCondition;
use super::evaluation::EvaluationResult;
use crate::base::{DescribedPredicate, HasDescription};
use crate::config::{ArchConfiguration, FAIL_ON_EMPTY_SHOULD_PROPERTY_NAME};
use crate::core::domain::RustItems;

/// The priority reported in the failure headline (`Priority`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Priority {
    /// `HIGH`
    High,
    /// `MEDIUM`, the default.
    #[default]
    Medium,
    /// `LOW`
    Low,
}

impl Priority {
    /// `HIGH`, `MEDIUM` or `LOW` (`asString()`).
    pub fn as_string(self) -> &'static str {
        match self {
            Priority::High => "HIGH",
            Priority::Medium => "MEDIUM",
            Priority::Low => "LOW",
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_string())
    }
}

/// Turns imported items into the objects a rule is about (`ClassesTransformer<T>` /
/// `AbstractClassesTransformer<T>`).
///
/// ```
/// use archunit::lang::ClassesTransformer;
/// use archunit::core::domain::RustModule;
///
/// let packages: ClassesTransformer<RustModule> =
///     ClassesTransformer::new("packages", |items| items.modules());
/// assert_eq!(packages.description(), "packages");
/// ```
pub struct ClassesTransformer<T> {
    description: String,
    transform: TransformFn<T>,
}

type TransformFn<T> = Arc<dyn Fn(&RustItems) -> Vec<T> + Send + Sync>;

impl<T> Clone for ClassesTransformer<T> {
    fn clone(&self) -> Self {
        Self {
            description: self.description.clone(),
            transform: Arc::clone(&self.transform),
        }
    }
}

impl<T> fmt::Debug for ClassesTransformer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ClassesTransformer({})", self.description)
    }
}

impl<T> HasDescription for ClassesTransformer<T> {
    fn description(&self) -> String {
        self.description.clone()
    }
}

impl<T: 'static> ClassesTransformer<T> {
    /// Creates a transformer (`new AbstractClassesTransformer<T>(description) { doTransform.. }`).
    pub fn new(
        description: impl Into<String>,
        transform: impl Fn(&RustItems) -> Vec<T> + Send + Sync + 'static,
    ) -> Self {
        Self {
            description: description.into(),
            transform: Arc::new(transform),
        }
    }

    /// `transform(..)`.
    pub fn transform(&self, items: &RustItems) -> Vec<T> {
        (self.transform)(items)
    }

    /// The description, e.g. `classes`.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Filters the transformed objects; the description becomes `<description> that <predicate>`.
    pub fn that(&self, predicate: DescribedPredicate<T>) -> Self {
        let description = format!("{} that {}", self.description, predicate.description());
        let inner = Arc::clone(&self.transform);
        Self {
            description,
            transform: Arc::new(move |items| {
                inner(items)
                    .into_iter()
                    .filter(|t| predicate.test(t))
                    .collect()
            }),
        }
    }

    /// Overrides the description (`as(..)`).
    pub fn as_(&self, description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            transform: Arc::clone(&self.transform),
        }
    }
}

/// An architecture rule (`ArchRule`).
///
/// Concrete rule types (the results of the fluent syntax, `SimpleArchRule`,
/// `CompositeArchRule`, the library's architectures) implement this trait and additionally
/// offer `because(..)`, `as_(..)` and `allow_empty_should(..)` returning their own type.
/// The same three are available on `Box<dyn ArchRule>` / `&dyn ArchRule`.
pub trait ArchRule: HasDescription + Send + Sync {
    /// Evaluates the rule without failing (`evaluate(..)`).
    fn evaluate(&self, items: &RustItems) -> EvaluationResult;

    /// Evaluates the rule and panics with the failure report if it is violated (`check(..)`).
    fn check(&self, items: &RustItems) {
        assertions::check(self, items);
    }

    /// A boxed copy.
    fn clone_boxed(&self) -> Box<dyn ArchRule>;

    /// `allowEmptyShould(..)` for trait objects.
    fn allow_empty_should_boxed(&self, allow_empty_should: bool) -> Box<dyn ArchRule>;
}

impl dyn ArchRule {
    /// Appends `, because <reason>` to the description (`because(..)`).
    pub fn because(&self, reason: &str) -> Box<dyn ArchRule> {
        Box::new(TransformedRule {
            inner: self.clone_boxed(),
            description: create_because_description(self, reason),
        })
    }

    /// Overrides the description (`as(..)`).
    pub fn as_(&self, description: &str) -> Box<dyn ArchRule> {
        Box::new(TransformedRule {
            inner: self.clone_boxed(),
            description: description.to_owned(),
        })
    }

    /// `allowEmptyShould(..)`.
    pub fn allow_empty_should(&self, allow_empty_should: bool) -> Box<dyn ArchRule> {
        self.allow_empty_should_boxed(allow_empty_should)
    }
}

impl ArchRule for Box<dyn ArchRule> {
    fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        (**self).evaluate(items)
    }

    fn check(&self, items: &RustItems) {
        (**self).check(items);
    }

    fn clone_boxed(&self) -> Box<dyn ArchRule> {
        (**self).clone_boxed()
    }

    fn allow_empty_should_boxed(&self, allow_empty_should: bool) -> Box<dyn ArchRule> {
        (**self).allow_empty_should_boxed(allow_empty_should)
    }
}

impl fmt::Debug for dyn ArchRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArchRule({})", self.description())
    }
}

/// `ArchRule.Factory.createBecauseDescription(..)`: `<description>, because <reason>`.
pub fn create_because_description(rule: &(impl HasDescription + ?Sized), reason: &str) -> String {
    format!("{}, because {reason}", rule.description())
}

/// A rule whose description was overridden (`ArchRule.Transformation.As` / `Because`).
struct TransformedRule {
    inner: Box<dyn ArchRule>,
    description: String,
}

impl HasDescription for TransformedRule {
    fn description(&self) -> String {
        self.description.clone()
    }
}

impl ArchRule for TransformedRule {
    fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        let inner = self.inner.evaluate(items);
        let mut result = EvaluationResult::empty(self, inner.priority());
        result.add(inner);
        result
    }

    fn clone_boxed(&self) -> Box<dyn ArchRule> {
        Box::new(TransformedRule {
            inner: self.inner.clone_boxed(),
            description: self.description.clone(),
        })
    }

    fn allow_empty_should_boxed(&self, allow_empty_should: bool) -> Box<dyn ArchRule> {
        Box::new(TransformedRule {
            inner: self.inner.allow_empty_should_boxed(allow_empty_should),
            description: self.description.clone(),
        })
    }
}

/// `ArchRule.Assertions`.
pub mod assertions {
    use super::*;

    /// Evaluates `rule` and panics with the report if there are violations
    /// (`ArchRule.Assertions.check(..)`).
    pub fn check(rule: &(impl ArchRule + ?Sized), items: &RustItems) {
        let result = rule.evaluate(items);
        assert_no_violation(&result);
    }

    /// Panics with the failure report if `result` has violations (`assertNoViolation(..)`).
    pub fn assert_no_violation(result: &EvaluationResult) {
        let report = result.failure_report();
        if !report.is_empty() {
            panic!("{report}");
        }
    }
}

/// The rule created from a transformer, a condition and a priority
/// (`ArchRule.Factory.create(..)` / `SimpleArchRule`).
pub struct SimpleArchRule<T: 'static> {
    priority: Priority,
    transformer: ClassesTransformer<T>,
    condition: ArchCondition<T>,
    overridden_description: Option<String>,
    allow_empty_should: Option<bool>,
}

impl<T: 'static> Clone for SimpleArchRule<T> {
    fn clone(&self) -> Self {
        Self {
            priority: self.priority,
            transformer: self.transformer.clone(),
            condition: self.condition.clone(),
            overridden_description: self.overridden_description.clone(),
            allow_empty_should: self.allow_empty_should,
        }
    }
}

impl<T: 'static> fmt::Debug for SimpleArchRule<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SimpleArchRule({})", self.description())
    }
}

impl<T: 'static> HasDescription for SimpleArchRule<T> {
    fn description(&self) -> String {
        self.overridden_description.clone().unwrap_or_else(|| {
            format!(
                "{} should {}",
                self.transformer.description(),
                self.condition.description()
            )
        })
    }
}

impl<T: Send + Sync + 'static> SimpleArchRule<T> {
    /// `ArchRule.Factory.create(transformer, condition, priority)`.
    pub fn create(
        transformer: ClassesTransformer<T>,
        condition: ArchCondition<T>,
        priority: Priority,
    ) -> Self {
        Self {
            priority,
            transformer,
            condition,
            overridden_description: None,
            allow_empty_should: None,
        }
    }

    /// Overrides the description (`as(..)`).
    pub fn as_(mut self, description: &str) -> Self {
        self.overridden_description = Some(description.to_owned());
        self
    }

    /// Appends `, because <reason>` (`because(..)`).
    pub fn because(self, reason: &str) -> Self {
        let description = create_because_description(&self, reason);
        self.as_(&description)
    }

    /// Whether the rule may check no objects at all (`allowEmptyShould(..)`).
    pub fn allow_empty_should(mut self, allow_empty_should: bool) -> Self {
        self.allow_empty_should = Some(allow_empty_should);
        self
    }

    /// The rule's description.
    pub fn description(&self) -> String {
        HasDescription::description(self)
    }

    /// The priority.
    pub fn priority(&self) -> Priority {
        self.priority
    }

    fn verify_no_empty_should_if_enabled(&self, all_objects: &[T]) {
        let allowed = self
            .allow_empty_should
            .unwrap_or_else(|| !ArchConfiguration::get().fail_on_empty_should());
        assert!(
            !(all_objects.is_empty() && !allowed),
            "Rule '{}' failed to check any classes. This means either that no classes have been passed to the rule at all, \
             or that no classes passed to the rule matched the `that()` clause. To allow rules being evaluated without \
             checking any classes you can either use `ArchRule::allow_empty_should(true)` on a single rule or set the \
             configuration property `{FAIL_ON_EMPTY_SHOULD_PROPERTY_NAME} = false` to change the behavior globally.",
            self.description()
        );
    }
}

impl<T: Send + Sync + 'static> ArchRule for SimpleArchRule<T> {
    fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        let all_objects = self.transformer.transform(items);
        self.verify_no_empty_should_if_enabled(&all_objects);
        let events = self.condition.evaluate_all(&all_objects);
        EvaluationResult::new(self, events, self.priority)
    }

    fn clone_boxed(&self) -> Box<dyn ArchRule> {
        Box::new(self.clone())
    }

    fn allow_empty_should_boxed(&self, allow_empty_should: bool) -> Box<dyn ArchRule> {
        Box::new(self.clone().allow_empty_should(allow_empty_should))
    }
}

/// Several rules evaluated together; violations are reported under a joined description
/// (`CompositeArchRule`).
pub struct CompositeArchRule {
    priority: Priority,
    rules: Vec<Box<dyn ArchRule>>,
    description: String,
}

impl fmt::Debug for CompositeArchRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CompositeArchRule({})", self.description)
    }
}

impl Clone for CompositeArchRule {
    fn clone(&self) -> Self {
        Self {
            priority: self.priority,
            rules: self.rules.iter().map(|r| r.clone_boxed()).collect(),
            description: self.description.clone(),
        }
    }
}

impl HasDescription for CompositeArchRule {
    fn description(&self) -> String {
        self.description.clone()
    }
}

impl CompositeArchRule {
    /// `CompositeArchRule.of(rule)`.
    pub fn of(rule: impl ArchRule + 'static) -> Self {
        Self::priority(Priority::Medium).of(rule)
    }

    /// `CompositeArchRule.of(rules)`; panics on an empty iterator like the Java original.
    pub fn of_all(rules: impl IntoIterator<Item = Box<dyn ArchRule>>) -> Self {
        let mut iter = rules.into_iter();
        let first = iter.next().expect("Iterable must be non-empty");
        let mut composite = Self::of(first);
        for rule in iter {
            composite = composite.and(rule);
        }
        composite
    }

    /// `CompositeArchRule.priority(priority).of(rule)`.
    pub fn priority(priority: Priority) -> CompositeCreator {
        CompositeCreator { priority }
    }

    /// Adds a rule; the description becomes `<a> and <b>` (`and(..)`).
    pub fn and(mut self, rule: impl ArchRule + 'static) -> Self {
        self.description = format!("{} and {}", self.description, rule.description());
        self.rules.push(Box::new(rule));
        self
    }

    /// `because(..)`.
    pub fn because(mut self, reason: &str) -> Self {
        self.description = create_because_description(&self, reason);
        self
    }

    /// `as(..)`.
    pub fn as_(mut self, description: &str) -> Self {
        self.description = description.to_owned();
        self
    }

    /// `allowEmptyShould(..)` applied to every contained rule.
    pub fn allow_empty_should(mut self, allow_empty_should: bool) -> Self {
        self.rules = self
            .rules
            .iter()
            .map(|r| r.allow_empty_should_boxed(allow_empty_should))
            .collect();
        self
    }

    /// The description.
    pub fn description(&self) -> String {
        self.description.clone()
    }
}

impl ArchRule for CompositeArchRule {
    fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        let mut result = EvaluationResult::empty(self, self.priority);
        for rule in &self.rules {
            result.add(rule.evaluate(items));
        }
        result
    }

    fn clone_boxed(&self) -> Box<dyn ArchRule> {
        Box::new(self.clone())
    }

    fn allow_empty_should_boxed(&self, allow_empty_should: bool) -> Box<dyn ArchRule> {
        Box::new(self.clone().allow_empty_should(allow_empty_should))
    }
}

/// `CompositeArchRule.Creator`.
#[derive(Debug, Clone, Copy)]
pub struct CompositeCreator {
    priority: Priority,
}

impl CompositeCreator {
    /// `of(rule)`.
    pub fn of(self, rule: impl ArchRule + 'static) -> CompositeArchRule {
        CompositeArchRule {
            priority: self.priority,
            description: rule.description(),
            rules: vec![Box::new(rule)],
        }
    }
}
