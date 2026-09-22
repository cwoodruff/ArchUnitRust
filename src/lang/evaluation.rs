use std::fmt;
use std::sync::Arc;

use regex::Regex;

use super::events::{
    ConditionEvent, ConditionEvents, CorrespondingObject, FromCorrespondingObject,
};
use super::rule::Priority;
use crate::base::HasDescription;
use crate::config::ArchConfiguration;

/// Handles violations with their corresponding objects (`ViolationHandler<T>`).
pub trait ViolationHandler<T> {
    /// Called once per violation with the objects of type `T` it refers to and the message.
    fn handle(&mut self, violating_objects: Vec<T>, message: &str);
}

impl<T, F> ViolationHandler<T> for F
where
    F: FnMut(Vec<T>, &str),
{
    fn handle(&mut self, violating_objects: Vec<T>, message: &str) {
        self(violating_objects, message);
    }
}

/// The outcome of evaluating a rule (`EvaluationResult`).
#[derive(Debug, Clone)]
pub struct EvaluationResult {
    rule_description: String,
    violations: Vec<Box<dyn ConditionEvent>>,
    information_about_number_of_violations: Option<String>,
    priority: Priority,
}

impl EvaluationResult {
    /// A result without violations, to which parts can be added (`new EvaluationResult(rule, priority)`).
    pub fn empty(rule: &(impl HasDescription + ?Sized), priority: Priority) -> Self {
        Self {
            rule_description: rule.description(),
            violations: Vec::new(),
            information_about_number_of_violations: None,
            priority,
        }
    }

    /// A result from the events of a condition (`new EvaluationResult(rule, events, priority)`).
    ///
    /// Violations matching a pattern of `archunit_ignore_patterns.txt` are dropped here.
    pub fn new(
        rule: &(impl HasDescription + ?Sized),
        events: ConditionEvents,
        priority: Priority,
    ) -> Self {
        let (_, violating, information) = events.into_parts();
        Self {
            rule_description: rule.description(),
            violations: apply_ignore_patterns(violating),
            information_about_number_of_violations: information,
            priority,
        }
    }

    /// The report with all violation lines sorted (`getFailureReport()`).
    pub fn failure_report(&self) -> FailureReport {
        let mut lines: Vec<String> = self
            .violations
            .iter()
            .flat_map(|e| e.description_lines())
            .collect();
        lines.sort();
        FailureReport {
            rule_description: self.rule_description.clone(),
            priority: self.priority,
            messages: FailureMessages {
                failures: lines,
                information_about_number_of_violations: self
                    .information_about_number_of_violations
                    .clone(),
            },
        }
    }

    /// Adds the violations of another result (`add(..)`).
    pub fn add(&mut self, part: EvaluationResult) {
        self.violations.extend(part.violations);
    }

    /// Whether any violation remains (`hasViolation()`).
    pub fn has_violation(&self) -> bool {
        !self.violations.is_empty()
    }

    /// `getPriority()`.
    pub fn priority(&self) -> Priority {
        self.priority
    }

    /// The rule description this result was created for.
    pub fn rule_description(&self) -> &str {
        &self.rule_description
    }

    /// Passes each violation's corresponding objects of type `T` and its message to `handler`
    /// (`handleViolations(..)`). Violations without objects of type `T` are skipped.
    pub fn handle_violations<T: FromCorrespondingObject>(
        &self,
        mut handler: impl ViolationHandler<T>,
    ) {
        for event in &self.violations {
            event.handle_with(&mut |objects: &[CorrespondingObject], message: &str| {
                let converted: Vec<T> = objects
                    .iter()
                    .flat_map(T::from_corresponding_object)
                    .collect();
                if !converted.is_empty() {
                    handler.handle(converted, message);
                }
            });
        }
    }

    /// Passes every violation with its raw corresponding objects to `handler`.
    pub fn handle_all_violations(&self, mut handler: impl FnMut(&[CorrespondingObject], &str)) {
        for event in &self.violations {
            event.handle_with(&mut handler);
        }
    }

    /// Keeps only violation lines satisfying `line_predicate`; resets the information about the
    /// number of violations (`filterDescriptionsMatching(..)`).
    pub fn filter_descriptions_matching(
        &self,
        line_predicate: impl Fn(&str) -> bool + Send + Sync + 'static,
    ) -> Self {
        let predicate: Arc<dyn Fn(&str) -> bool + Send + Sync> = Arc::new(line_predicate);
        Self {
            rule_description: self.rule_description.clone(),
            violations: filter_events(&self.violations, &predicate),
            information_about_number_of_violations: None,
            priority: self.priority,
        }
    }
}

fn filter_events(
    violations: &[Box<dyn ConditionEvent>],
    predicate: &Arc<dyn Fn(&str) -> bool + Send + Sync>,
) -> Vec<Box<dyn ConditionEvent>> {
    violations
        .iter()
        .map(|event| FilteredEvent::new(event.clone_boxed(), Arc::clone(predicate)))
        .filter(FilteredEvent::is_violation)
        .map(|event| Box::new(event) as Box<dyn ConditionEvent>)
        .collect()
}

fn apply_ignore_patterns(violations: Vec<Box<dyn ConditionEvent>>) -> Vec<Box<dyn ConditionEvent>> {
    let patterns = ArchConfiguration::get().ignore_patterns();
    if patterns.is_empty() {
        return violations;
    }
    let predicate: Arc<dyn Fn(&str) -> bool + Send + Sync> = Arc::new(move |message: &str| {
        let normalized = message.replace("\r\n", " ").replace('\n', " ");
        !patterns.iter().any(|p: &Regex| p.is_match(&normalized))
    });
    filter_events(&violations, &predicate)
}

struct FilteredEvent {
    delegate: Box<dyn ConditionEvent>,
    predicate: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    lines: Vec<String>,
}

impl fmt::Debug for FilteredEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FilteredEvent({:?})", self.lines)
    }
}

impl FilteredEvent {
    fn new(
        delegate: Box<dyn ConditionEvent>,
        predicate: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    ) -> Self {
        let lines = delegate
            .description_lines()
            .into_iter()
            .filter(|l| predicate(l))
            .collect();
        Self {
            delegate,
            predicate,
            lines,
        }
    }
}

impl ConditionEvent for FilteredEvent {
    fn is_violation(&self) -> bool {
        self.delegate.is_violation() && !self.lines.is_empty()
    }

    fn invert(&self) -> Box<dyn ConditionEvent> {
        Box::new(FilteredEvent::new(
            self.delegate.invert(),
            Arc::clone(&self.predicate),
        ))
    }

    fn description_lines(&self) -> Vec<String> {
        self.lines.clone()
    }

    fn handle_with(&self, handler: &mut dyn FnMut(&[CorrespondingObject], &str)) {
        let predicate = Arc::clone(&self.predicate);
        self.delegate.handle_with(&mut |objects, message| {
            if predicate(message) {
                handler(objects, message);
            }
        });
    }

    fn clone_boxed(&self) -> Box<dyn ConditionEvent> {
        Box::new(FilteredEvent::new(
            self.delegate.clone_boxed(),
            Arc::clone(&self.predicate),
        ))
    }
}

/// The violation lines of a report (`FailureMessages`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureMessages {
    failures: Vec<String>,
    information_about_number_of_violations: Option<String>,
}

impl FailureMessages {
    /// The text for the `(..)` part of the headline: a custom text if set, else `N times`
    /// (`getInformationAboutNumberOfViolations()`).
    pub fn information_about_number_of_violations(&self) -> String {
        self.information_about_number_of_violations
            .clone()
            .unwrap_or_else(|| format!("{} times", self.failures.len()))
    }

    /// The lines.
    pub fn lines(&self) -> &[String] {
        &self.failures
    }

    /// Whether there are no lines.
    pub fn is_empty(&self) -> bool {
        self.failures.is_empty()
    }
}

impl std::ops::Deref for FailureMessages {
    type Target = [String];

    fn deref(&self) -> &[String] {
        &self.failures
    }
}

/// Renders a failure report (`FailureDisplayFormat`).
pub trait FailureDisplayFormat: Send + Sync {
    /// Formats the complete failure text for a violated rule.
    fn format_failure(
        &self,
        rule_description: &str,
        failure_messages: &FailureMessages,
        priority: Priority,
    ) -> String;
}

/// ArchUnit's default format:
///
/// ```text
/// Architecture Violation [Priority: MEDIUM] - Rule 'no classes should ..' was violated (2 times):
/// <line>
/// <line>
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultFailureDisplayFormat;

impl FailureDisplayFormat for DefaultFailureDisplayFormat {
    fn format_failure(
        &self,
        rule_description: &str,
        failure_messages: &FailureMessages,
        priority: Priority,
    ) -> String {
        format!(
            "Architecture Violation [Priority: {}] - Rule '{}' was violated ({}):\n{}",
            priority.as_string(),
            rule_description,
            failure_messages.information_about_number_of_violations(),
            failure_messages.lines().join("\n")
        )
    }
}

/// The report of a violated rule (`FailureReport`).
#[derive(Debug, Clone)]
pub struct FailureReport {
    rule_description: String,
    priority: Priority,
    messages: FailureMessages,
}

impl FailureReport {
    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// The violation lines (`getDetails()`).
    pub fn details(&self) -> Vec<String> {
        self.messages.failures.clone()
    }

    /// The messages.
    pub fn failure_messages(&self) -> &FailureMessages {
        &self.messages
    }

    /// The rule description.
    pub fn rule_description(&self) -> &str {
        &self.rule_description
    }

    /// The priority.
    pub fn priority(&self) -> Priority {
        self.priority
    }
}

impl fmt::Display for FailureReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match ArchConfiguration::get().failure_display_format() {
            Some(format) => {
                format.format_failure(&self.rule_description, &self.messages, self.priority)
            }
            None => DefaultFailureDisplayFormat.format_failure(
                &self.rule_description,
                &self.messages,
                self.priority,
            ),
        };
        f.write_str(&text)
    }
}
