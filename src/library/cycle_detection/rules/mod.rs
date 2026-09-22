//! `CycleArchCondition`: a condition that components are free of cycles, for any kind of
//! component that maps to items and their dependencies.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::Hash;
use std::sync::Arc;

use super::{CycleDetector, Edge};
use crate::base::DescribedPredicate;
use crate::config::{ArchConfiguration, MAX_NUMBER_OF_CYCLES_TO_DETECT_PROPERTY_NAME};
use crate::core::domain::{Dependency, RustItem};
use crate::lang::{
    ArchCondition, AsCorrespondingObject, ConditionEvents, ConditionLogic, CorrespondingObject,
    SimpleConditionEvent,
};

type ComponentFn<C, R> = Arc<dyn Fn(&C) -> R + Send + Sync>;

/// A dependency between two components, carrying the class dependencies that cause it
/// (`ComponentDependency`).
#[derive(Debug, Clone)]
struct ComponentDependency {
    origin: usize,
    target: usize,
    dependencies: Vec<Dependency>,
}

impl Edge<usize> for ComponentDependency {
    fn origin(&self) -> &usize {
        &self.origin
    }

    fn target(&self) -> &usize {
        &self.target
    }
}

/// Builds a [`CycleArchCondition`] (`CycleArchCondition.builder()`).
///
/// ```
/// use archunit::library::cycle_detection::rules::CycleArchCondition;
/// use archunit::library::dependencies::Slice;
///
/// let be_free_of_cycles = CycleArchCondition::<Slice>::builder()
///     .retrieve_classes_by(|slice| slice.items())
///     .retrieve_description_by(|slice| slice.description())
///     .retrieve_outgoing_dependencies_by(|slice| slice.dependencies_from_self())
///     .build();
/// ```
pub struct CycleArchCondition<C> {
    _marker: std::marker::PhantomData<fn(&C)>,
}

impl<C> std::fmt::Debug for CycleArchCondition<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CycleArchCondition")
    }
}

/// Builder step: how to get the items of a component.
#[derive(Debug)]
pub struct NeedsRetrieveClasses<C> {
    _marker: std::marker::PhantomData<fn(&C)>,
}

/// Builder step: how to describe a component.
pub struct NeedsRetrieveDescription<C> {
    retrieve_classes: ComponentFn<C, Vec<RustItem>>,
}

/// Builder step: how to get the outgoing dependencies of a component.
pub struct NeedsRetrieveOutgoingDependencies<C> {
    retrieve_classes: ComponentFn<C, Vec<RustItem>>,
    retrieve_description: ComponentFn<C, String>,
}

/// Final builder step.
pub struct Builder<C> {
    retrieve_classes: ComponentFn<C, Vec<RustItem>>,
    retrieve_description: ComponentFn<C, String>,
    retrieve_outgoing_dependencies: ComponentFn<C, Vec<Dependency>>,
    relevant: DescribedPredicate<Dependency>,
}

impl<C> std::fmt::Debug for NeedsRetrieveDescription<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CycleArchCondition::builder()")
    }
}

impl<C> std::fmt::Debug for NeedsRetrieveOutgoingDependencies<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CycleArchCondition::builder()")
    }
}

impl<C> std::fmt::Debug for Builder<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CycleArchCondition::builder()")
    }
}

impl<C: Clone + Hash + Eq + AsCorrespondingObject + Send + Sync + 'static> CycleArchCondition<C> {
    /// Starts building the condition.
    pub fn builder() -> NeedsRetrieveClasses<C> {
        NeedsRetrieveClasses {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<C: 'static> NeedsRetrieveClasses<C> {
    /// `retrieveClassesBy(..)`.
    pub fn retrieve_classes_by(
        self,
        retrieve_classes: impl Fn(&C) -> Vec<RustItem> + Send + Sync + 'static,
    ) -> NeedsRetrieveDescription<C> {
        NeedsRetrieveDescription {
            retrieve_classes: Arc::new(retrieve_classes),
        }
    }
}

impl<C: 'static> NeedsRetrieveDescription<C> {
    /// `retrieveDescriptionBy(..)`.
    pub fn retrieve_description_by(
        self,
        retrieve_description: impl Fn(&C) -> String + Send + Sync + 'static,
    ) -> NeedsRetrieveOutgoingDependencies<C> {
        NeedsRetrieveOutgoingDependencies {
            retrieve_classes: self.retrieve_classes,
            retrieve_description: Arc::new(retrieve_description),
        }
    }
}

impl<C: 'static> NeedsRetrieveOutgoingDependencies<C> {
    /// `retrieveOutgoingDependenciesBy(..)`.
    pub fn retrieve_outgoing_dependencies_by(
        self,
        retrieve: impl Fn(&C) -> Vec<Dependency> + Send + Sync + 'static,
    ) -> Builder<C> {
        Builder {
            retrieve_classes: self.retrieve_classes,
            retrieve_description: self.retrieve_description,
            retrieve_outgoing_dependencies: Arc::new(retrieve),
            relevant: crate::base::always_true(),
        }
    }
}

impl<C: Clone + Hash + Eq + AsCorrespondingObject + Send + Sync + 'static> Builder<C> {
    /// Only considers dependencies satisfying `predicate` (`onlyConsiderDependencies(..)`).
    pub fn only_consider_dependencies(mut self, predicate: DescribedPredicate<Dependency>) -> Self {
        self.relevant = predicate;
        self
    }

    /// Builds the condition, described as `be free of cycles`.
    pub fn build(self) -> ArchCondition<C> {
        ArchCondition::from_logic(
            "be free of cycles",
            CycleLogic {
                retrieve_classes: self.retrieve_classes,
                retrieve_description: self.retrieve_description,
                retrieve_outgoing_dependencies: self.retrieve_outgoing_dependencies,
                relevant: self.relevant,
                components: Vec::new(),
                item_to_component: HashMap::new(),
                edges: BTreeMap::new(),
            },
        )
    }
}

struct CycleLogic<C> {
    retrieve_classes: ComponentFn<C, Vec<RustItem>>,
    retrieve_description: ComponentFn<C, String>,
    retrieve_outgoing_dependencies: ComponentFn<C, Vec<Dependency>>,
    relevant: DescribedPredicate<Dependency>,
    components: Vec<C>,
    item_to_component: HashMap<RustItem, usize>,
    edges: BTreeMap<(usize, usize), BTreeSet<Dependency>>,
}

impl<C: Clone + Hash + Eq + AsCorrespondingObject + Send + Sync + 'static> ConditionLogic<C>
    for CycleLogic<C>
{
    fn init(&mut self, all: &[C]) {
        self.components = all.to_vec();
        self.item_to_component.clear();
        self.edges.clear();
        for (index, component) in all.iter().enumerate() {
            for item in (self.retrieve_classes)(component) {
                self.item_to_component.insert(item, index);
            }
        }
    }

    fn check(&mut self, component: &C, _events: &mut ConditionEvents) {
        let Some(origin) = self.components.iter().position(|c| c == component) else {
            return;
        };
        for dependency in (self.retrieve_outgoing_dependencies)(component) {
            if !self.relevant.test(&dependency) {
                continue;
            }
            if let Some(&target) = self.item_to_component.get(&dependency.target_item()) {
                self.edges
                    .entry((origin, target))
                    .or_default()
                    .insert(dependency);
            }
        }
    }

    fn finish(&mut self, events: &mut ConditionEvents) {
        let nodes: Vec<usize> = (0..self.components.len()).collect();
        let edges: Vec<ComponentDependency> = self
            .edges
            .iter()
            .map(|(&(origin, target), dependencies)| ComponentDependency {
                origin,
                target,
                dependencies: dependencies.iter().cloned().collect(),
            })
            .collect();
        let cycles = CycleDetector::detect_cycles(&nodes, &edges);
        if cycles.max_number_of_cycles_reached() {
            events.set_information_about_number_of_violations(format!(
                " >= {} times - the maximum number of cycles to detect has been reached; this limit can be adapted using the `archunit.toml` value `{MAX_NUMBER_OF_CYCLES_TO_DETECT_PROPERTY_NAME} = xxx`",
                cycles.len()
            ));
        }
        let max_dependencies = ArchConfiguration::get().max_number_of_dependencies_per_edge();
        for cycle in cycles.iter() {
            let (message, dependencies) = self.describe_cycle(cycle.edges(), max_dependencies);
            events.add(SimpleConditionEvent::violated(
                CorrespondingObject::other(dependencies),
                message,
            ));
        }
        self.components.clear();
        self.item_to_component.clear();
        self.edges.clear();
    }
}

const CYCLE_DETECTED_SECTION_INTRO: &str = "Cycle detected: ";

impl<C> CycleLogic<C> {
    fn describe_cycle(
        &self,
        edges: &[ComponentDependency],
        max_dependencies: usize,
    ) -> (String, Vec<Dependency>) {
        let describe =
            |edge: &ComponentDependency| (self.retrieve_description)(&self.components[edge.origin]);
        // Rotate so that the lexicographically smallest origin description comes first.
        let start = edges
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| describe(a).cmp(&describe(b)))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let ordered: Vec<&ComponentDependency> =
            edges[start..].iter().chain(edges[..start].iter()).collect();
        let descriptions: Vec<String> = ordered.iter().map(|e| describe(e)).collect();
        let separator = format!(" -> \n{}", " ".repeat(CYCLE_DETECTED_SECTION_INTRO.len()));
        let mut headline_parts = descriptions.clone();
        headline_parts.push(descriptions[0].clone());
        let headline = headline_parts.join(&separator);
        let mut details = Vec::new();
        let mut all_dependencies = Vec::new();
        for (index, (edge, description)) in ordered.iter().zip(&descriptions).enumerate() {
            details.push(format!("  {}. Dependencies of {description}", index + 1));
            let total = edge.dependencies.len();
            for dependency in edge.dependencies.iter().take(max_dependencies) {
                details.push(format!("    - {}", dependency.description()));
            }
            if total > max_dependencies {
                details.push(format!(
                    "    ({} further dependencies have been omitted...)",
                    total - max_dependencies
                ));
            }
            all_dependencies.extend(edge.dependencies.iter().cloned());
        }
        (
            format!(
                "{CYCLE_DETECTED_SECTION_INTRO}{headline}\n{}",
                details.join("\n")
            ),
            all_dependencies,
        )
    }
}
