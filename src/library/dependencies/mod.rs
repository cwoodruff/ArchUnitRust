//! Slices: partitions of the items by a package pattern or a custom assignment, and rules
//! about the dependencies between them (`com.tngtech.archunit.library.dependencies`).
//!
//! ```no_run
//! use archunit::library::dependencies::SlicesRuleDefinition;
//! # let items = archunit::core::importer::CrateImporter::new().import_path("src");
//!
//! SlicesRuleDefinition::slices().matching("..my_app.(*)..").should().be_free_of_cycles().check(&items);
//! SlicesRuleDefinition::slices().matching("..my_app.(*)..").should().not_depend_on_each_other().check(&items);
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::base::{DescribedPredicate, HasDescription, not};
use crate::core::domain::dependency::predicates::dependency;
use crate::core::domain::properties::ItemSelector;
use crate::core::domain::{Dependency, PackageMatcher, RustItem, RustItems};
use crate::lang::syntax::aggregators::PredicateAggregator;
use crate::lang::syntax::priority;
use crate::lang::{
    ArchCondition, ArchRule, AsCorrespondingObject, ClassesTransformer, ConditionEvents,
    EvaluationResult, Priority, SimpleConditionEvent,
};
use crate::lang::{CorrespondingObject, CorrespondingValue};
use crate::library::cycle_detection::rules::CycleArchCondition;

// ---- SliceIdentifier / SliceAssignment ------------------------------------------------------

/// The identifier of the slice an item belongs to (`SliceIdentifier`).
///
/// [`SliceIdentifier::ignore`] marks items that belong to no slice.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SliceIdentifier {
    parts: Vec<String>,
}

impl SliceIdentifier {
    /// An identifier made of `parts` (`SliceIdentifier.of(..)`).
    ///
    /// # Panics
    /// If `parts` is empty; use [`ignore`](Self::ignore) instead.
    pub fn of(parts: &[&str]) -> Self {
        assert!(
            !parts.is_empty(),
            "Parts of a SliceIdentifier must not be empty. Use SliceIdentifier::ignore() to ignore a RustItem"
        );
        Self {
            parts: parts.iter().map(|p| (*p).to_owned()).collect(),
        }
    }

    /// An identifier from owned parts.
    pub fn of_parts(parts: Vec<String>) -> Self {
        assert!(
            !parts.is_empty(),
            "Parts of a SliceIdentifier must not be empty. Use SliceIdentifier::ignore() to ignore a RustItem"
        );
        Self { parts }
    }

    /// The identifier of items that belong to no slice (`SliceIdentifier.ignore()`).
    pub fn ignore() -> Self {
        Self { parts: Vec::new() }
    }

    /// The parts.
    pub fn parts(&self) -> &[String] {
        &self.parts
    }

    /// Whether this is [`ignore`](Self::ignore).
    pub fn is_ignore(&self) -> bool {
        self.parts.is_empty()
    }
}

/// Assigns items to slices (`SliceAssignment`).
pub trait SliceAssignment: Send + Sync {
    /// The slice `item` belongs to, or [`SliceIdentifier::ignore`].
    fn identifier_of(&self, item: &RustItem) -> SliceIdentifier;

    /// A description used in rule texts (`slices assigned from <description>`).
    fn description(&self) -> String;
}

/// A [`SliceAssignment`] from a description and a closure.
pub fn slice_assignment(
    description: impl Into<String>,
    identifier_of: impl Fn(&RustItem) -> SliceIdentifier + Send + Sync + 'static,
) -> impl SliceAssignment {
    struct Closure<F> {
        description: String,
        identifier_of: F,
    }
    impl<F: Fn(&RustItem) -> SliceIdentifier + Send + Sync> SliceAssignment for Closure<F> {
        fn identifier_of(&self, item: &RustItem) -> SliceIdentifier {
            (self.identifier_of)(item)
        }

        fn description(&self) -> String {
            self.description.clone()
        }
    }
    Closure {
        description: description.into(),
        identifier_of,
    }
}

/// `PackageMatchingSliceIdentifier`: the capture groups of a package identifier.
struct PackageMatchingSliceIdentifier {
    package_identifier: String,
    matcher: PackageMatcher,
}

impl SliceAssignment for PackageMatchingSliceIdentifier {
    fn identifier_of(&self, item: &RustItem) -> SliceIdentifier {
        match self.matcher.match_(&item.package_name()) {
            Some(result) if result.number_of_groups() > 0 => {
                SliceIdentifier::of_parts(result.groups().to_vec())
            }
            _ => SliceIdentifier::ignore(),
        }
    }

    fn description(&self) -> String {
        format!("'{}'", self.package_identifier)
    }
}

// ---- Slice ---------------------------------------------------------------------------------

/// A group of items that belong together, e.g. all items in packages matching one capture
/// group of `..my_app.(*)..` (`Slice`).
///
/// Slices are equal when their identifiers are equal.
#[derive(Clone)]
pub struct Slice {
    assignment: Arc<dyn SliceAssignment>,
    identifier: SliceIdentifier,
    description_pattern: String,
    items: Vec<RustItem>,
}

impl PartialEq for Slice {
    fn eq(&self, other: &Self) -> bool {
        self.identifier == other.identifier
    }
}

impl Eq for Slice {}

impl Hash for Slice {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identifier.hash(state);
    }
}

impl fmt::Debug for Slice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Slice({})", self.description())
    }
}

impl fmt::Display for Slice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.description())
    }
}

impl HasDescription for Slice {
    fn description(&self) -> String {
        Slice::description(self)
    }
}

impl CorrespondingValue for Slice {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl AsCorrespondingObject for Slice {
    fn as_corresponding_object(&self) -> CorrespondingObject {
        CorrespondingObject::other(self.clone())
    }
}

impl From<&Slice> for CorrespondingObject {
    fn from(slice: &Slice) -> Self {
        slice.as_corresponding_object()
    }
}

impl Slice {
    fn new(
        assignment: Arc<dyn SliceAssignment>,
        identifier: SliceIdentifier,
        items: Vec<RustItem>,
    ) -> Self {
        let captures: Vec<String> = (1..=identifier.parts.len())
            .map(|i| format!("${i}"))
            .collect();
        Self {
            assignment,
            identifier,
            description_pattern: format!("Slice {}", captures.join(" - ")),
            items,
        }
    }

    /// The description, e.g. `Slice one` (`getDescription()`).
    pub fn description(&self) -> String {
        let mut result = self.description_pattern.clone();
        for (index, part) in self.identifier.parts.iter().enumerate() {
            result = result.replace(&format!("${}", index + 1), part);
        }
        result
    }

    /// The identifier.
    pub fn identifier(&self) -> &SliceIdentifier {
        &self.identifier
    }

    /// The items in this slice.
    pub fn items(&self) -> Vec<RustItem> {
        self.items.clone()
    }

    /// Iterates over the items.
    pub fn iter(&self) -> impl Iterator<Item = &RustItem> {
        self.items.iter()
    }

    /// The number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the slice holds no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Whether `item` is part of this slice (`contains(..)`).
    pub fn contains(&self, item: &RustItem) -> bool {
        self.items.contains(item)
    }

    /// The `index`-th (1-based) part of the identifier (`getNamePart(index)`).
    ///
    /// # Panics
    /// If there is no such part.
    pub fn name_part(&self, index: usize) -> &str {
        assert!(
            index > 0 && index <= self.identifier.parts.len(),
            "Found no name part with index {index}"
        );
        &self.identifier.parts[index - 1]
    }

    /// Renames the slice; `$1`, `$2`, ... in `pattern` are replaced by the identifier parts
    /// (`as(pattern)`).
    pub fn as_(&self, pattern: &str) -> Slice {
        Slice {
            description_pattern: pattern.to_owned(),
            ..self.clone()
        }
    }

    /// Dependencies from items of this slice to items outside of it (`getDependenciesFromSelf()`).
    pub fn dependencies_from_self(&self) -> Vec<Dependency> {
        let mut result: Vec<Dependency> = self
            .items
            .iter()
            .flat_map(|item| item.direct_dependencies_from_self())
            .filter(|d| self.is_not_assigned_to_own_slice(&d.target_item()))
            .collect();
        result.sort();
        result.dedup();
        result
    }

    /// Dependencies from items outside of this slice to items in it (`getDependenciesToSelf()`).
    pub fn dependencies_to_self(&self) -> Vec<Dependency> {
        let mut result: Vec<Dependency> = self
            .items
            .iter()
            .flat_map(|item| item.direct_dependencies_to_self())
            .filter(|d| self.is_not_assigned_to_own_slice(&d.origin_item()))
            .collect();
        result.sort();
        result.dedup();
        result
    }

    fn is_not_assigned_to_own_slice(&self, item: &RustItem) -> bool {
        self.assignment.identifier_of(item) != self.identifier
    }
}

impl<'a> IntoIterator for &'a Slice {
    type Item = &'a RustItem;
    type IntoIter = std::slice::Iter<'a, RustItem>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

// ---- Slices --------------------------------------------------------------------------------

/// A described collection of [`Slice`]s (`Slices`).
#[derive(Debug, Clone)]
pub struct Slices {
    slices: Vec<Slice>,
    description: String,
}

impl HasDescription for Slices {
    fn description(&self) -> String {
        self.description.clone()
    }
}

impl Slices {
    fn new(slices: Vec<Slice>) -> Self {
        Self {
            slices,
            description: "Slices".to_owned(),
        }
    }

    /// Slices by the capture groups of a package identifier, e.g. `..my_app.(*)..`
    /// (`Slices.matching(..)`).
    pub fn matching(package_identifier: &str) -> SlicesTransformer {
        let assignment = PackageMatchingSliceIdentifier {
            package_identifier: package_identifier.to_owned(),
            matcher: PackageMatcher::of(package_identifier),
        };
        let description = format!("slices matching {}", assignment.description());
        SlicesTransformer::new(Arc::new(assignment), description)
    }

    /// Slices by a custom [`SliceAssignment`] (`Slices.assignedFrom(..)`).
    pub fn assigned_from(assignment: impl SliceAssignment + 'static) -> SlicesTransformer {
        let description = format!("slices assigned from {}", assignment.description());
        SlicesTransformer::new(Arc::new(assignment), description)
    }

    /// Iterates over the slices.
    pub fn iter(&self) -> impl Iterator<Item = &Slice> {
        self.slices.iter()
    }

    /// The number of slices.
    pub fn len(&self) -> usize {
        self.slices.len()
    }

    /// Whether there are no slices.
    pub fn is_empty(&self) -> bool {
        self.slices.is_empty()
    }

    /// The slices as a vector.
    pub fn to_vec(&self) -> Vec<Slice> {
        self.slices.clone()
    }

    /// Overrides the description (`as(..)`).
    pub fn as_(&self, description: &str) -> Slices {
        Slices {
            slices: self.slices.clone(),
            description: description.to_owned(),
        }
    }

    /// Renames every slice by `pattern` (`namingSlices(..)`), see [`Slice::as_`].
    pub fn naming_slices(&self, pattern: &str) -> Slices {
        Slices {
            slices: self.slices.iter().map(|s| s.as_(pattern)).collect(),
            description: self.description.clone(),
        }
    }
}

impl<'a> IntoIterator for &'a Slices {
    type Item = &'a Slice;
    type IntoIter = std::slice::Iter<'a, Slice>;

    fn into_iter(self) -> Self::IntoIter {
        self.slices.iter()
    }
}

impl IntoIterator for Slices {
    type Item = Slice;
    type IntoIter = std::vec::IntoIter<Slice>;

    fn into_iter(self) -> Self::IntoIter {
        self.slices.into_iter()
    }
}

/// Turns items into [`Slices`] (`Slices.Transformer`); converts into a
/// [`ClassesTransformer<Slice>`] for `all(..)`.
#[derive(Clone)]
pub struct SlicesTransformer {
    assignment: Arc<dyn SliceAssignment>,
    description: String,
    naming_pattern: Option<String>,
    predicates: PredicateAggregator<Slice>,
    join_word: &'static str,
}

impl fmt::Debug for SlicesTransformer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SlicesTransformer({})", self.description)
    }
}

impl HasDescription for SlicesTransformer {
    fn description(&self) -> String {
        self.description.clone()
    }
}

impl SlicesTransformer {
    fn new(assignment: Arc<dyn SliceAssignment>, description: String) -> Self {
        Self {
            assignment,
            description,
            naming_pattern: None,
            predicates: PredicateAggregator::default(),
            join_word: "that",
        }
    }

    /// Names the slices by `pattern` (`namingSlices(..)`), see [`Slice::as_`].
    pub fn naming_slices(mut self, pattern: &str) -> Self {
        self.naming_pattern = Some(pattern.to_owned());
        self
    }

    /// Restricts the slices; the description becomes `<description> that <predicate>`
    /// (`that(..)`).
    pub fn that(mut self, predicate: DescribedPredicate<Slice>) -> Self {
        self.description = format!(
            "{} {} {}",
            self.description,
            self.join_word,
            predicate.description()
        );
        self.predicates = self.predicates.add(predicate);
        self
    }

    pub(crate) fn that_ands(mut self, predicate: DescribedPredicate<Slice>) -> Self {
        self.predicates = self.predicates.that_ands();
        self.join_word = "and";
        self.that(predicate)
    }

    pub(crate) fn that_ors(mut self, predicate: DescribedPredicate<Slice>) -> Self {
        self.predicates = self.predicates.that_ors();
        self.join_word = "or";
        self.that(predicate)
    }

    /// Overrides the description (`as(..)`); restrictions added so far stay in effect.
    pub fn as_(mut self, description: &str) -> Self {
        self.description = description.to_owned();
        self
    }

    /// The slices of `items` (`of(..)` / `transform(..)`).
    pub fn of(&self, items: &RustItems) -> Slices {
        self.transform(items.iter())
    }

    /// The slices the targets of `dependencies` belong to (`transform(dependencies)`).
    pub fn of_dependencies(&self, dependencies: &[Dependency]) -> Slices {
        let mut targets: Vec<RustItem> = dependencies.iter().map(|d| d.target_item()).collect();
        targets.sort();
        targets.dedup();
        self.transform(targets)
    }

    fn transform(&self, items: impl IntoIterator<Item = RustItem>) -> Slices {
        let mut builders: BTreeMap<SliceIdentifier, Vec<RustItem>> = BTreeMap::new();
        for item in items {
            let identifier = self.assignment.identifier_of(&item);
            if identifier.is_ignore() {
                continue;
            }
            builders.entry(identifier).or_default().push(item);
        }
        let mut slices = Slices::new(
            builders
                .into_iter()
                .map(|(identifier, items)| {
                    Slice::new(Arc::clone(&self.assignment), identifier, items)
                })
                .collect(),
        );
        if let Some(pattern) = &self.naming_pattern {
            slices = slices.naming_slices(pattern);
        }
        if let Some(predicate) = self.predicates.get() {
            slices.slices.retain(|s| predicate.test(s));
        }
        slices.as_(&self.description)
    }
}

impl From<SlicesTransformer> for ClassesTransformer<Slice> {
    fn from(transformer: SlicesTransformer) -> Self {
        let description = transformer.description.clone();
        ClassesTransformer::new(description, move |items: &RustItems| {
            transformer.of(items).slices
        })
    }
}

// ---- SliceDependency -----------------------------------------------------------------------

/// A dependency between two slices with the class dependencies causing it (`SliceDependency`).
#[derive(Clone)]
pub struct SliceDependency {
    origin: Slice,
    target: Slice,
    relevant_dependencies: Vec<Dependency>,
}

impl fmt::Debug for SliceDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SliceDependency{{origin={}, target={}}}",
            self.origin.description(),
            self.target.description()
        )
    }
}

impl PartialEq for SliceDependency {
    fn eq(&self, other: &Self) -> bool {
        self.origin == other.origin && self.target == other.target
    }
}

impl Eq for SliceDependency {}

impl Hash for SliceDependency {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.origin.hash(state);
        self.target.hash(state);
    }
}

impl HasDescription for SliceDependency {
    /// `<origin> depends on <target>:` followed by one line per class dependency.
    fn description(&self) -> String {
        let parts: Vec<String> = self
            .relevant_dependencies
            .iter()
            .map(|d| d.description())
            .collect();
        format!(
            "{} depends on {}:\n{}",
            self.origin.description(),
            self.target.description(),
            parts.join("\n")
        )
    }
}

impl CorrespondingValue for SliceDependency {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn dependencies(&self) -> Vec<Dependency> {
        self.relevant_dependencies.clone()
    }
}

impl SliceDependency {
    fn of(origin: &Slice, dependencies_to_consider: &[Dependency], target: &Slice) -> Self {
        let mut relevant: Vec<Dependency> = dependencies_to_consider
            .iter()
            .filter(|d| target.contains(&d.target_item()))
            .cloned()
            .collect();
        relevant.sort();
        relevant.dedup();
        Self {
            origin: origin.clone(),
            target: target.clone(),
            relevant_dependencies: relevant,
        }
    }

    /// The origin slice.
    pub fn origin(&self) -> &Slice {
        &self.origin
    }

    /// The target slice.
    pub fn target(&self) -> &Slice {
        &self.target
    }

    /// The class dependencies from origin to target.
    pub fn dependencies(&self) -> &[Dependency] {
        &self.relevant_dependencies
    }
}

// ---- SlicesRuleDefinition / GivenSlices / SlicesShould / SliceRule -------------------------

/// Entry point for slice rules (`SlicesRuleDefinition`).
#[derive(Debug, Clone, Copy)]
pub struct SlicesRuleDefinition;

impl SlicesRuleDefinition {
    /// `SlicesRuleDefinition.slices()`.
    pub fn slices() -> SlicesCreator {
        SlicesCreator
    }
}

/// `SlicesRuleDefinition.slices()`.
pub fn slices() -> SlicesCreator {
    SlicesRuleDefinition::slices()
}

/// `SlicesRuleDefinition.Creator`.
#[derive(Debug, Clone, Copy)]
pub struct SlicesCreator;

impl SlicesCreator {
    /// Slices by the capture groups of a package identifier (`matching(..)`).
    pub fn matching(self, package_identifier: &str) -> GivenSlices {
        self.matching_with_priority(package_identifier, Priority::Medium)
    }

    /// `matching(packageIdentifier, priority)`.
    pub fn matching_with_priority(
        self,
        package_identifier: &str,
        priority: Priority,
    ) -> GivenSlices {
        GivenSlices {
            priority,
            transformer: Slices::matching(package_identifier),
        }
    }

    /// Slices by a custom assignment (`assignedFrom(..)`).
    pub fn assigned_from(self, assignment: impl SliceAssignment + 'static) -> GivenSlices {
        self.assigned_from_with_priority(assignment, Priority::Medium)
    }

    /// `assignedFrom(assignment, priority)`.
    pub fn assigned_from_with_priority(
        self,
        assignment: impl SliceAssignment + 'static,
        priority: Priority,
    ) -> GivenSlices {
        GivenSlices {
            priority,
            transformer: Slices::assigned_from(assignment),
        }
    }
}

/// `GivenSlices` / `GivenNamedSlices` / `GivenSlicesConjunction`.
#[derive(Debug, Clone)]
pub struct GivenSlices {
    priority: Priority,
    transformer: SlicesTransformer,
}

/// `GivenSlicesConjunction`: the same type as [`GivenSlices`].
pub type GivenSlicesConjunction = GivenSlices;

/// `GivenNamedSlices`: the same type as [`GivenSlices`].
pub type GivenNamedSlices = GivenSlices;

impl GivenSlices {
    /// Names the slices, e.g. `"Layer $1"` (`namingSlices(..)`).
    pub fn naming_slices(self, pattern: &str) -> GivenNamedSlices {
        Self {
            priority: self.priority,
            transformer: self.transformer.naming_slices(pattern),
        }
    }

    /// `that(predicate)`.
    pub fn that(self, predicate: DescribedPredicate<Slice>) -> GivenSlicesConjunction {
        Self {
            priority: self.priority,
            transformer: self.transformer.that(predicate),
        }
    }

    /// `and(predicate)`.
    pub fn and(self, predicate: DescribedPredicate<Slice>) -> GivenSlicesConjunction {
        Self {
            priority: self.priority,
            transformer: self.transformer.that_ands(predicate),
        }
    }

    /// `or(predicate)`.
    pub fn or(self, predicate: DescribedPredicate<Slice>) -> GivenSlicesConjunction {
        Self {
            priority: self.priority,
            transformer: self.transformer.that_ors(predicate),
        }
    }

    /// Overrides the description of the slices (`as(..)`).
    pub fn as_(self, description: &str) -> GivenSlicesConjunction {
        Self {
            priority: self.priority,
            transformer: self.transformer.as_(description),
        }
    }

    /// `should()`.
    pub fn should(self) -> SlicesShould {
        SlicesShould {
            priority: self.priority,
            transformer: self.transformer,
        }
    }

    /// `should(condition)` with a custom condition.
    pub fn should_with(
        self,
        condition: impl Into<ArchCondition<Slice>>,
    ) -> crate::lang::SimpleArchRule<Slice> {
        priority(self.priority)
            .all(ClassesTransformer::from(self.transformer))
            .should(condition)
    }

    /// The slices of `items`, for custom checks.
    pub fn transform(&self, items: &RustItems) -> Slices {
        self.transformer.of(items)
    }
}

/// `SlicesShould`.
#[derive(Debug, Clone)]
pub struct SlicesShould {
    priority: Priority,
    transformer: SlicesTransformer,
}

impl SlicesShould {
    /// `be free of cycles` (`beFreeOfCycles()`).
    pub fn be_free_of_cycles(self) -> SliceRule {
        SliceRule::new(
            self.transformer,
            self.priority,
            Arc::new(|_transformer, predicate| {
                CycleArchCondition::<Slice>::builder()
                    .retrieve_classes_by(|slice: &Slice| slice.items())
                    .retrieve_description_by(|slice: &Slice| slice.description())
                    .retrieve_outgoing_dependencies_by(|slice: &Slice| {
                        slice.dependencies_from_self()
                    })
                    .only_consider_dependencies(predicate)
                    .build()
            }),
        )
    }

    /// `not depend on each other` (`notDependOnEachOther()`).
    pub fn not_depend_on_each_other(self) -> SliceRule {
        SliceRule::new(
            self.transformer,
            self.priority,
            Arc::new(|transformer, predicate| {
                not_depend_on_each_other_condition(transformer, predicate)
            }),
        )
    }
}

fn not_depend_on_each_other_condition(
    transformer: SlicesTransformer,
    predicate: DescribedPredicate<Dependency>,
) -> ArchCondition<Slice> {
    ArchCondition::new(
        "not depend on each other",
        move |slice: &Slice, events: &mut ConditionEvents| {
            let relevant: Vec<Dependency> = slice
                .dependencies_from_self()
                .into_iter()
                .filter(|d| predicate.test(d))
                .collect();
            for dependency_slice in transformer.of_dependencies(&relevant).iter() {
                let dependency = SliceDependency::of(slice, &relevant, dependency_slice);
                let message = dependency.description();
                events.add(SimpleConditionEvent::violated(
                    CorrespondingObject::other(dependency),
                    message,
                ));
            }
        },
    )
}

type ConditionFactory = Arc<
    dyn Fn(SlicesTransformer, DescribedPredicate<Dependency>) -> ArchCondition<Slice> + Send + Sync,
>;

#[derive(Clone)]
enum Transformation {
    As(String),
    Because(String),
}

/// A rule about slices (`SliceRule`): supports `ignore_dependency(..)` on top of the usual
/// `as_`/`because`/`allow_empty_should`.
#[derive(Clone)]
pub struct SliceRule {
    transformer: SlicesTransformer,
    priority: Priority,
    transformations: Vec<Transformation>,
    ignore_dependency: DescribedPredicate<Dependency>,
    condition_factory: ConditionFactory,
    allow_empty_should: Option<bool>,
}

impl fmt::Debug for SliceRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SliceRule({})", self.description())
    }
}

impl SliceRule {
    fn new(
        transformer: SlicesTransformer,
        priority: Priority,
        condition_factory: ConditionFactory,
    ) -> Self {
        Self {
            transformer,
            priority,
            transformations: Vec::new(),
            ignore_dependency: crate::base::always_false(),
            condition_factory,
            allow_empty_should: None,
        }
    }

    /// Overrides the description (`as(..)`).
    pub fn as_(mut self, description: &str) -> Self {
        self.transformations
            .push(Transformation::As(description.to_owned()));
        self
    }

    /// Appends `, because <reason>` (`because(..)`).
    pub fn because(mut self, reason: &str) -> Self {
        self.transformations
            .push(Transformation::Because(reason.to_owned()));
        self
    }

    /// `allowEmptyShould(..)`.
    pub fn allow_empty_should(mut self, allow_empty_should: bool) -> Self {
        self.allow_empty_should = Some(allow_empty_should);
        self
    }

    /// Ignores dependencies from `origin` to `target`, both given as full names or
    /// predicates (`ignoreDependency(..)`).
    pub fn ignore_dependency(
        mut self,
        origin: impl Into<ItemSelector>,
        target: impl Into<ItemSelector>,
    ) -> Self {
        self.ignore_dependency = self.ignore_dependency.or(dependency(origin, target));
        self
    }

    /// Ignores dependencies matching `predicate` `[rust-only]`.
    pub fn ignore_dependency_where(mut self, predicate: DescribedPredicate<Dependency>) -> Self {
        self.ignore_dependency = self.ignore_dependency.or(predicate);
        self
    }

    /// The description.
    pub fn description(&self) -> String {
        HasDescription::description(&self.arch_rule())
    }

    /// Evaluates the rule without failing (`evaluate(..)`).
    pub fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        ArchRule::evaluate(self, items)
    }

    /// Evaluates the rule and panics with the failure report if it is violated (`check(..)`).
    pub fn check(&self, items: &RustItems) {
        ArchRule::check(self, items);
    }

    fn arch_rule(&self) -> Box<dyn ArchRule> {
        let condition = (self.condition_factory)(
            self.transformer.clone(),
            not(self.ignore_dependency.clone()),
        );
        let mut rule: Box<dyn ArchRule> = Box::new(
            priority(self.priority)
                .all(ClassesTransformer::from(self.transformer.clone()))
                .should(condition),
        );
        if let Some(allow_empty_should) = self.allow_empty_should {
            rule = rule.allow_empty_should_boxed(allow_empty_should);
        }
        for transformation in &self.transformations {
            rule = match transformation {
                Transformation::As(description) => rule.as_(description),
                Transformation::Because(reason) => rule.because(reason),
            };
        }
        rule
    }
}

impl HasDescription for SliceRule {
    fn description(&self) -> String {
        SliceRule::description(self)
    }
}

impl ArchRule for SliceRule {
    fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        ArchRule::evaluate(&self.arch_rule(), items)
    }

    fn clone_boxed(&self) -> Box<dyn ArchRule> {
        Box::new(self.clone())
    }

    fn allow_empty_should_boxed(&self, allow_empty_should: bool) -> Box<dyn ArchRule> {
        Box::new(self.clone().allow_empty_should(allow_empty_should))
    }
}
