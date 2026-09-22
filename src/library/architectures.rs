//! Layered and onion architectures (`com.tngtech.archunit.library.Architectures`).
//!
//! ```no_run
//! use archunit::library::Architectures;
//! # let items = archunit::core::importer::CrateImporter::new().import_path("src");
//!
//! Architectures::layered_architecture()
//!     .considering_all_dependencies()
//!     .layer("Controller").defined_by(&["..controller.."])
//!     .layer("Service").defined_by(&["..service.."])
//!     .layer("Persistence").defined_by(&["..persistence.."])
//!     .where_layer("Controller").may_not_be_accessed_by_any_layer()
//!     .where_layer("Service").may_only_be_accessed_by_layers(&["Controller"])
//!     .where_layer("Persistence").may_only_be_accessed_by_layers(&["Service"])
//!     .check(&items);
//! ```

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use crate::base::{DescribedPredicate, HasDescription, always_false, join_single_quoted, not};
use crate::core::domain::dependency::functions::{get_origin_item, get_target_item};
use crate::core::domain::dependency::predicates::{
    dependency, dependency_origin, dependency_target,
};
use crate::core::domain::properties::ItemSelector;
use crate::core::domain::rust_item::predicates::{
    reside_in_any_package, reside_outside_of_packages,
};
use crate::core::domain::{Dependency, RustItem, RustItems};
use crate::lang::conditions::{only_have_dependencies_where, only_have_dependents_where};
use crate::lang::syntax::classes;
use crate::lang::{
    ArchCondition, ArchRule, ConditionEvents, ConditionLogic, EvaluationResult, Priority,
    SimpleConditionEvent,
};

/// Entry points for architecture rules (`Architectures`).
#[derive(Debug, Clone, Copy)]
pub struct Architectures;

impl Architectures {
    /// `Architectures.layeredArchitecture()`.
    pub fn layered_architecture() -> DependencySettings {
        DependencySettings
    }

    /// `Architectures.onionArchitecture()`.
    pub fn onion_architecture() -> OnionArchitecture {
        OnionArchitecture::new()
    }
}

/// `Architectures.layeredArchitecture()`.
pub fn layered_architecture() -> DependencySettings {
    Architectures::layered_architecture()
}

/// `Architectures.onionArchitecture()`.
pub fn onion_architecture() -> OnionArchitecture {
    Architectures::onion_architecture()
}

// ---- dependency settings -------------------------------------------------------------------

type IgnoreExcluded = Arc<
    dyn Fn(&LayerDefinitions, DescribedPredicate<Dependency>) -> DescribedPredicate<Dependency>
        + Send
        + Sync,
>;

/// Which dependencies a layered architecture considers (`LayeredArchitecture.DependencySettings`).
#[derive(Debug, Clone, Copy)]
pub struct DependencySettings;

#[derive(Clone)]
struct ConfiguredDependencySettings {
    description: String,
    ignore_excluded_dependencies: IgnoreExcluded,
}

fn origin_or_target_is(predicate: DescribedPredicate<RustItem>) -> DescribedPredicate<Dependency> {
    get_origin_item()
        .is(predicate.clone())
        .or(get_target_item().is(predicate))
}

impl DependencySettings {
    /// Every dependency counts (`consideringAllDependencies()`).
    pub fn considering_all_dependencies(self) -> LayeredArchitecture {
        LayeredArchitecture::new(ConfiguredDependencySettings {
            description: "considering all dependencies".to_owned(),
            ignore_excluded_dependencies: Arc::new(|_, predicate| predicate),
        })
    }

    /// Only dependencies whose origin and target reside in one of the packages count
    /// (`consideringOnlyDependenciesInAnyPackage(..)`).
    ///
    /// # Panics
    /// If `package_identifiers` is empty.
    pub fn considering_only_dependencies_in_any_package(
        self,
        package_identifiers: &[&str],
    ) -> LayeredArchitecture {
        assert!(
            !package_identifiers.is_empty(),
            "At least 1 package identifier must be provided."
        );
        let outside_of_relevant_package = reside_outside_of_packages(package_identifiers);
        LayeredArchitecture::new(ConfiguredDependencySettings {
            description: format!(
                "considering only dependencies in any package [{}]",
                join_single_quoted(package_identifiers)
            ),
            ignore_excluded_dependencies: Arc::new(move |_, predicate| {
                predicate.or(origin_or_target_is(outside_of_relevant_package.clone()))
            }),
        })
    }

    /// Only dependencies between items of the defined layers count
    /// (`consideringOnlyDependenciesInLayers()`).
    pub fn considering_only_dependencies_in_layers(self) -> LayeredArchitecture {
        LayeredArchitecture::new(ConfiguredDependencySettings {
            description: "considering only dependencies in layers".to_owned(),
            ignore_excluded_dependencies: Arc::new(|layers, predicate| {
                let not_in_layers = not(layers.contains_predicate_for_all());
                predicate.or(origin_or_target_is(not_in_layers))
            }),
        })
    }
}

// ---- layer definitions ---------------------------------------------------------------------

#[derive(Clone)]
struct LayerDefinitionData {
    name: String,
    optional: bool,
    contains: DescribedPredicate<RustItem>,
}

impl LayerDefinitionData {
    fn describe(&self) -> String {
        format!(
            "{}layer '{}' ({})",
            if self.optional { "optional " } else { "" },
            self.name,
            self.contains.description()
        )
    }
}

#[derive(Clone, Default)]
struct LayerDefinitions {
    layers: Vec<LayerDefinitionData>,
}

impl LayerDefinitions {
    fn add(&mut self, definition: LayerDefinitionData) {
        if let Some(existing) = self.layers.iter_mut().find(|l| l.name == definition.name) {
            *existing = definition;
        } else {
            self.layers.push(definition);
        }
    }

    fn contain_layer(&self, name: &str) -> bool {
        self.layers.iter().any(|l| l.name == name)
    }

    fn contains_predicate_for(&self, names: &[String]) -> DescribedPredicate<RustItem> {
        let mut result = always_false();
        for layer in self.layers.iter().filter(|l| names.contains(&l.name)) {
            result = result.or(layer.contains.clone());
        }
        result
    }

    fn contains_predicate_for_all(&self) -> DescribedPredicate<RustItem> {
        let names: Vec<String> = self.layers.iter().map(|l| l.name.clone()).collect();
        self.contains_predicate_for(&names)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LayerDependencyConstraint {
    Origin,
    Target,
}

#[derive(Clone)]
struct LayerDependencySpecificationData {
    layer_name: String,
    allowed_layers: Vec<String>,
    constraint: LayerDependencyConstraint,
    description_suffix: String,
}

impl LayerDependencySpecificationData {
    fn describe(&self) -> String {
        format!(
            "where layer '{}' {}",
            self.layer_name, self.description_suffix
        )
    }
}

/// A layered architecture rule (`Architectures.LayeredArchitecture`).
#[derive(Clone)]
pub struct LayeredArchitecture {
    layer_definitions: LayerDefinitions,
    dependency_specifications: Vec<LayerDependencySpecificationData>,
    dependency_settings: ConfiguredDependencySettings,
    irrelevant_dependencies: Option<DescribedPredicate<Dependency>>,
    overridden_description: Option<String>,
    optional_layers: bool,
    all_items_contained_check: Option<DescribedPredicate<RustItem>>,
}

impl fmt::Debug for LayeredArchitecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LayeredArchitecture({})", self.description())
    }
}

impl fmt::Display for LayeredArchitecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.description())
    }
}

impl LayeredArchitecture {
    fn new(dependency_settings: ConfiguredDependencySettings) -> Self {
        Self {
            layer_definitions: LayerDefinitions::default(),
            dependency_specifications: Vec::new(),
            dependency_settings,
            irrelevant_dependencies: None,
            overridden_description: None,
            optional_layers: false,
            all_items_contained_check: None,
        }
    }

    /// Whether empty layers are tolerated (`withOptionalLayers(..)`).
    pub fn with_optional_layers(mut self, optional_layers: bool) -> Self {
        self.optional_layers = optional_layers;
        self
    }

    /// Starts defining a layer (`layer(name)`).
    ///
    /// # Panics
    /// If `name` is empty.
    pub fn layer(self, name: &str) -> LayerDefinition {
        LayerDefinition::new(self, name, false)
    }

    /// Starts defining a layer that may be empty (`optionalLayer(name)`).
    pub fn optional_layer(self, name: &str) -> LayerDefinition {
        LayerDefinition::new(self, name, true)
    }

    /// Starts a constraint on a layer (`whereLayer(name)`).
    ///
    /// # Panics
    /// If no layer named `name` was defined.
    pub fn where_layer(self, name: &str) -> LayerDependencySpecification {
        self.check_layer_names_exist(&[name]);
        LayerDependencySpecification {
            architecture: self,
            layer_name: name.to_owned(),
        }
    }

    fn check_layer_names_exist(&self, names: &[&str]) {
        for name in names {
            assert!(
                self.layer_definitions.contain_layer(name),
                "There is no layer named '{name}'"
            );
        }
    }

    /// Ignores dependencies from `origin` to `target`, given as full names or predicates
    /// (`ignoreDependency(..)`).
    pub fn ignore_dependency(
        self,
        origin: impl Into<ItemSelector>,
        target: impl Into<ItemSelector>,
    ) -> Self {
        self.ignore_dependency_where(dependency(origin, target))
    }

    /// Ignores dependencies matching `predicate` (`ignoreDependency(..)`).
    pub fn ignore_dependency_where(mut self, predicate: DescribedPredicate<Dependency>) -> Self {
        self.irrelevant_dependencies = Some(match self.irrelevant_dependencies {
            Some(existing) => existing.or(predicate),
            None => predicate,
        });
        self
    }

    /// Fails for items that belong to no layer (`ensureAllClassesAreContainedInArchitecture()`).
    pub fn ensure_all_classes_are_contained_in_architecture(self) -> Self {
        self.ensure_all_classes_are_contained_in_architecture_ignoring_with(always_false())
    }

    /// Like [`ensure_all_classes_are_contained_in_architecture`] but tolerates items in the
    /// given packages (`ensureAllClassesAreContainedInArchitectureIgnoring(String...)`).
    ///
    /// [`ensure_all_classes_are_contained_in_architecture`]: Self::ensure_all_classes_are_contained_in_architecture
    pub fn ensure_all_classes_are_contained_in_architecture_ignoring(
        self,
        package_identifiers: &[&str],
    ) -> Self {
        self.ensure_all_classes_are_contained_in_architecture_ignoring_with(reside_in_any_package(
            package_identifiers,
        ))
    }

    /// Like [`ensure_all_classes_are_contained_in_architecture`] but tolerates items matching
    /// `predicate` (`ensureAllClassesAreContainedInArchitectureIgnoring(DescribedPredicate)`).
    ///
    /// [`ensure_all_classes_are_contained_in_architecture`]: Self::ensure_all_classes_are_contained_in_architecture
    pub fn ensure_all_classes_are_contained_in_architecture_ignoring_with(
        mut self,
        predicate: DescribedPredicate<RustItem>,
    ) -> Self {
        self.all_items_contained_check = Some(predicate);
        self
    }

    /// Overrides the description (`as(..)`).
    pub fn as_(mut self, description: &str) -> Self {
        self.overridden_description = Some(description.to_owned());
        self
    }

    /// Appends `, because <reason>` to the description (`because(..)`).
    pub fn because(self, reason: &str) -> Box<dyn ArchRule> {
        let rule: &dyn ArchRule = &self;
        rule.because(reason)
    }

    /// Same as [`with_optional_layers`](Self::with_optional_layers) (`allowEmptyShould(..)`).
    pub fn allow_empty_should(self, allow_empty_should: bool) -> Self {
        self.with_optional_layers(allow_empty_should)
    }

    /// The description, one line per layer and constraint.
    pub fn description(&self) -> String {
        if let Some(description) = &self.overridden_description {
            return description.clone();
        }
        let prefix = format!(
            "Layered architecture {}",
            self.dependency_settings.description
        );
        let mut lines = vec![format!(
            "{prefix}, consisting of{}",
            if self.optional_layers {
                " (optional)"
            } else {
                ""
            }
        )];
        lines.extend(
            self.layer_definitions
                .layers
                .iter()
                .map(LayerDefinitionData::describe),
        );
        lines.extend(
            self.dependency_specifications
                .iter()
                .map(LayerDependencySpecificationData::describe),
        );
        lines.join("\n")
    }

    fn add_layer_definition(mut self, definition: LayerDefinitionData) -> Self {
        self.layer_definitions.add(definition);
        self
    }

    fn add_dependency_specification(
        mut self,
        specification: LayerDependencySpecificationData,
    ) -> Self {
        if let Some(existing) = self.dependency_specifications.iter_mut().find(|s| {
            s.layer_name == specification.layer_name && s.constraint == specification.constraint
        }) {
            *existing = specification;
        } else {
            self.dependency_specifications.push(specification);
        }
        self
    }

    fn evaluate_layers_should_not_be_empty(
        &self,
        items: &RustItems,
        layer: &LayerDefinitionData,
    ) -> EvaluationResult {
        let layer_name = layer.name.clone();
        classes()
            .that_with(
                self.layer_definitions
                    .contains_predicate_for(std::slice::from_ref(&layer.name)),
            )
            .should_with(ArchCondition::from_logic(
                "not be empty",
                LayerShouldNotBeEmpty {
                    layer_name,
                    empty: true,
                },
            ))
            .allow_empty_should(true)
            .evaluate(items)
    }

    fn evaluate_all_items_are_contained(
        &self,
        items: &RustItems,
        ignore: &DescribedPredicate<RustItem>,
    ) -> EvaluationResult {
        let contained = self.layer_definitions.contains_predicate_for_all();
        let ignore = ignore.clone();
        classes()
            .should_with(ArchCondition::new(
                "be contained in architecture",
                move |item: &RustItem, events: &mut ConditionEvents| {
                    if !ignore.test(item) && !contained.test(item) {
                        events.add(SimpleConditionEvent::violated(
                            item,
                            format!("Item <{}> is not contained in architecture", item.name()),
                        ));
                    }
                },
            ))
            .evaluate(items)
    }

    fn evaluate_dependencies_should_be_satisfied(
        &self,
        items: &RustItems,
        spec: &LayerDependencySpecificationData,
    ) -> EvaluationResult {
        let condition = match spec.constraint {
            LayerDependencyConstraint::Origin => {
                let origin_matches = dependency_origin(
                    self.layer_definitions
                        .contains_predicate_for(&spec.allowed_layers),
                )
                .or(dependency_origin(
                    self.layer_definitions
                        .contains_predicate_for(std::slice::from_ref(&spec.layer_name)),
                ));
                only_have_dependents_where(self.if_dependency_is_relevant(origin_matches))
            }
            LayerDependencyConstraint::Target => {
                let target_matches = dependency_target(
                    self.layer_definitions
                        .contains_predicate_for(&spec.allowed_layers),
                )
                .or(dependency_target(
                    self.layer_definitions
                        .contains_predicate_for(std::slice::from_ref(&spec.layer_name)),
                ));
                only_have_dependencies_where(self.if_dependency_is_relevant(target_matches))
            }
        };
        classes()
            .that_with(
                self.layer_definitions
                    .contains_predicate_for(std::slice::from_ref(&spec.layer_name)),
            )
            .should_with(condition)
            .allow_empty_should(true)
            .evaluate(items)
    }

    fn if_dependency_is_relevant(
        &self,
        predicate: DescribedPredicate<Dependency>,
    ) -> DescribedPredicate<Dependency> {
        let configured = (self.dependency_settings.ignore_excluded_dependencies)(
            &self.layer_definitions,
            predicate,
        );
        match &self.irrelevant_dependencies {
            Some(irrelevant) => configured.or(irrelevant.clone()),
            None => configured,
        }
    }
}

struct LayerShouldNotBeEmpty {
    layer_name: String,
    empty: bool,
}

impl ConditionLogic<RustItem> for LayerShouldNotBeEmpty {
    fn init(&mut self, _all: &[RustItem]) {
        self.empty = true;
    }

    fn check(&mut self, _item: &RustItem, _events: &mut ConditionEvents) {
        self.empty = false;
    }

    fn finish(&mut self, events: &mut ConditionEvents) {
        if self.empty {
            events.add(SimpleConditionEvent::violated(
                self.layer_name.clone(),
                format!("Layer '{}' is empty", self.layer_name),
            ));
        }
    }
}

impl LayeredArchitecture {
    /// Evaluates the rule without failing (`evaluate(..)`).
    pub fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        ArchRule::evaluate(self, items)
    }

    /// Evaluates the rule and panics with the failure report if it is violated (`check(..)`).
    pub fn check(&self, items: &RustItems) {
        ArchRule::check(self, items);
    }
}

impl HasDescription for LayeredArchitecture {
    fn description(&self) -> String {
        LayeredArchitecture::description(self)
    }
}

impl ArchRule for LayeredArchitecture {
    fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        let mut result = EvaluationResult::empty(self, Priority::Medium);
        if !self.optional_layers {
            for layer in self.layer_definitions.layers.iter().filter(|l| !l.optional) {
                result.add(self.evaluate_layers_should_not_be_empty(items, layer));
            }
        }
        if let Some(ignore) = &self.all_items_contained_check {
            result.add(self.evaluate_all_items_are_contained(items, ignore));
        }
        for specification in &self.dependency_specifications {
            result.add(self.evaluate_dependencies_should_be_satisfied(items, specification));
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

/// The `defined_by` step of `layer(name)` (`LayeredArchitecture.LayerDefinition`).
#[derive(Debug)]
pub struct LayerDefinition {
    architecture: LayeredArchitecture,
    name: String,
    optional: bool,
}

impl LayerDefinition {
    fn new(architecture: LayeredArchitecture, name: &str, optional: bool) -> Self {
        assert!(!name.is_empty(), "Layer name must be present");
        Self {
            architecture,
            name: name.to_owned(),
            optional,
        }
    }

    /// The layer holds the items in any of the packages (`definedBy(String...)`).
    pub fn defined_by(self, package_identifiers: &[&str]) -> LayeredArchitecture {
        let description = join_single_quoted(package_identifiers);
        self.defined_by_with(reside_in_any_package(package_identifiers).as_(description))
    }

    /// The layer holds the items matching `predicate` (`definedBy(DescribedPredicate)`).
    pub fn defined_by_with(self, predicate: DescribedPredicate<RustItem>) -> LayeredArchitecture {
        self.architecture.add_layer_definition(LayerDefinitionData {
            name: self.name,
            optional: self.optional,
            contains: predicate,
        })
    }
}

/// The constraint step of `where_layer(name)` (`LayeredArchitecture.LayerDependencySpecification`).
#[derive(Debug)]
pub struct LayerDependencySpecification {
    architecture: LayeredArchitecture,
    layer_name: String,
}

impl LayerDependencySpecification {
    /// `mayNotBeAccessedByAnyLayer()`.
    pub fn may_not_be_accessed_by_any_layer(self) -> LayeredArchitecture {
        self.deny_layer_access(
            LayerDependencyConstraint::Origin,
            "may not be accessed by any layer",
        )
    }

    /// `mayOnlyBeAccessedByLayers(..)`.
    ///
    /// # Panics
    /// If `layer_names` is empty or names an unknown layer.
    pub fn may_only_be_accessed_by_layers(self, layer_names: &[&str]) -> LayeredArchitecture {
        self.restrict_layers(
            LayerDependencyConstraint::Origin,
            layer_names,
            "may only be accessed by layers",
        )
    }

    /// `mayNotAccessAnyLayer()`.
    pub fn may_not_access_any_layer(self) -> LayeredArchitecture {
        self.deny_layer_access(
            LayerDependencyConstraint::Target,
            "may not access any layer",
        )
    }

    /// `mayOnlyAccessLayers(..)`.
    ///
    /// # Panics
    /// If `layer_names` is empty or names an unknown layer.
    pub fn may_only_access_layers(self, layer_names: &[&str]) -> LayeredArchitecture {
        self.restrict_layers(
            LayerDependencyConstraint::Target,
            layer_names,
            "may only access layers",
        )
    }

    fn deny_layer_access(
        self,
        constraint: LayerDependencyConstraint,
        description: &str,
    ) -> LayeredArchitecture {
        self.architecture
            .add_dependency_specification(LayerDependencySpecificationData {
                layer_name: self.layer_name,
                allowed_layers: Vec::new(),
                constraint,
                description_suffix: description.to_owned(),
            })
    }

    fn restrict_layers(
        self,
        constraint: LayerDependencyConstraint,
        layer_names: &[&str],
        description: &str,
    ) -> LayeredArchitecture {
        assert!(
            !layer_names.is_empty(),
            "At least 1 layer name must be provided."
        );
        self.architecture.check_layer_names_exist(layer_names);
        let mut seen = HashSet::new();
        let allowed_layers = layer_names
            .iter()
            .filter(|n| seen.insert(**n))
            .map(|n| (*n).to_owned())
            .collect();
        self.architecture
            .add_dependency_specification(LayerDependencySpecificationData {
                layer_name: self.layer_name,
                allowed_layers,
                constraint,
                description_suffix: format!("{description} [{}]", join_single_quoted(layer_names)),
            })
    }
}

// ---- onion architecture --------------------------------------------------------------------

const DOMAIN_MODEL_LAYER: &str = "domain model";
const DOMAIN_SERVICE_LAYER: &str = "domain service";
const APPLICATION_SERVICE_LAYER: &str = "application service";
const ADAPTER_LAYER: &str = "adapter";

/// An onion architecture rule (`Architectures.OnionArchitecture`), evaluated as a
/// [`LayeredArchitecture`].
#[derive(Clone)]
pub struct OnionArchitecture {
    domain_models: Option<DescribedPredicate<RustItem>>,
    domain_services: Option<DescribedPredicate<RustItem>>,
    application_services: Option<DescribedPredicate<RustItem>>,
    adapters: Vec<(String, DescribedPredicate<RustItem>)>,
    optional_layers: bool,
    ignored_dependencies: Vec<DescribedPredicate<Dependency>>,
    overridden_description: Option<String>,
    all_items_contained_check: Option<DescribedPredicate<RustItem>>,
}

impl fmt::Debug for OnionArchitecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OnionArchitecture({})", self.description())
    }
}

impl fmt::Display for OnionArchitecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.description())
    }
}

fn by_package_predicate(package_identifiers: &[&str]) -> DescribedPredicate<RustItem> {
    reside_in_any_package(package_identifiers).as_(join_single_quoted(package_identifiers))
}

impl OnionArchitecture {
    fn new() -> Self {
        Self {
            domain_models: None,
            domain_services: None,
            application_services: None,
            adapters: Vec::new(),
            optional_layers: false,
            ignored_dependencies: Vec::new(),
            overridden_description: None,
            all_items_contained_check: None,
        }
    }

    /// `domainModels(String...)`.
    pub fn domain_models(self, package_identifiers: &[&str]) -> Self {
        self.domain_models_with(by_package_predicate(package_identifiers))
    }

    /// `domainModels(DescribedPredicate)`.
    pub fn domain_models_with(mut self, predicate: DescribedPredicate<RustItem>) -> Self {
        self.domain_models = Some(predicate);
        self
    }

    /// `domainServices(String...)`.
    pub fn domain_services(self, package_identifiers: &[&str]) -> Self {
        self.domain_services_with(by_package_predicate(package_identifiers))
    }

    /// `domainServices(DescribedPredicate)`.
    pub fn domain_services_with(mut self, predicate: DescribedPredicate<RustItem>) -> Self {
        self.domain_services = Some(predicate);
        self
    }

    /// `applicationServices(String...)`.
    pub fn application_services(self, package_identifiers: &[&str]) -> Self {
        self.application_services_with(by_package_predicate(package_identifiers))
    }

    /// `applicationServices(DescribedPredicate)`.
    pub fn application_services_with(mut self, predicate: DescribedPredicate<RustItem>) -> Self {
        self.application_services = Some(predicate);
        self
    }

    /// `adapter(name, String...)`.
    pub fn adapter(self, name: &str, package_identifiers: &[&str]) -> Self {
        self.adapter_with(name, by_package_predicate(package_identifiers))
    }

    /// `adapter(name, DescribedPredicate)`.
    pub fn adapter_with(mut self, name: &str, predicate: DescribedPredicate<RustItem>) -> Self {
        if let Some(existing) = self.adapters.iter_mut().find(|(n, _)| n == name) {
            existing.1 = predicate;
        } else {
            self.adapters.push((name.to_owned(), predicate));
        }
        self
    }

    /// Whether empty layers are tolerated (`withOptionalLayers(..)`).
    pub fn with_optional_layers(mut self, optional_layers: bool) -> Self {
        self.optional_layers = optional_layers;
        self
    }

    /// Ignores dependencies from `origin` to `target` (`ignoreDependency(..)`).
    pub fn ignore_dependency(
        self,
        origin: impl Into<ItemSelector>,
        target: impl Into<ItemSelector>,
    ) -> Self {
        self.ignore_dependency_where(dependency(origin, target))
    }

    /// Ignores dependencies matching `predicate`.
    pub fn ignore_dependency_where(mut self, predicate: DescribedPredicate<Dependency>) -> Self {
        self.ignored_dependencies.push(predicate);
        self
    }

    /// `ensureAllClassesAreContainedInArchitecture()`.
    pub fn ensure_all_classes_are_contained_in_architecture(self) -> Self {
        self.ensure_all_classes_are_contained_in_architecture_ignoring_with(always_false())
    }

    /// `ensureAllClassesAreContainedInArchitectureIgnoring(String...)`.
    pub fn ensure_all_classes_are_contained_in_architecture_ignoring(
        self,
        package_identifiers: &[&str],
    ) -> Self {
        self.ensure_all_classes_are_contained_in_architecture_ignoring_with(reside_in_any_package(
            package_identifiers,
        ))
    }

    /// `ensureAllClassesAreContainedInArchitectureIgnoring(DescribedPredicate)`.
    pub fn ensure_all_classes_are_contained_in_architecture_ignoring_with(
        mut self,
        predicate: DescribedPredicate<RustItem>,
    ) -> Self {
        self.all_items_contained_check = Some(predicate);
        self
    }

    /// Overrides the description (`as(..)`).
    pub fn as_(mut self, description: &str) -> Self {
        self.overridden_description = Some(description.to_owned());
        self
    }

    /// Appends `, because <reason>` to the description (`because(..)`).
    pub fn because(self, reason: &str) -> Box<dyn ArchRule> {
        let rule: &dyn ArchRule = &self;
        rule.because(reason)
    }

    /// Same as [`with_optional_layers`](Self::with_optional_layers) (`allowEmptyShould(..)`).
    pub fn allow_empty_should(self, allow_empty_should: bool) -> Self {
        self.with_optional_layers(allow_empty_should)
    }

    /// The description, one line per ring and adapter.
    pub fn description(&self) -> String {
        if let Some(description) = &self.overridden_description {
            return description.clone();
        }
        let mut lines = vec![format!(
            "Onion architecture consisting of{}",
            if self.optional_layers {
                " (optional)"
            } else {
                ""
            }
        )];
        if let Some(p) = &self.domain_models {
            lines.push(format!("domain models ({})", p.description()));
        }
        if let Some(p) = &self.domain_services {
            lines.push(format!("domain services ({})", p.description()));
        }
        if let Some(p) = &self.application_services {
            lines.push(format!("application services ({})", p.description()));
        }
        for (name, predicate) in &self.adapters {
            lines.push(format!("adapter '{name}' ({})", predicate.description()));
        }
        lines.join("\n")
    }

    /// The equivalent [`LayeredArchitecture`].
    pub fn layered_architecture_delegate(&self) -> LayeredArchitecture {
        let or_false =
            |p: &Option<DescribedPredicate<RustItem>>| p.clone().unwrap_or_else(always_false);
        let mut any_adapter = always_false();
        for (_, predicate) in &self.adapters {
            any_adapter = any_adapter.or(predicate.clone());
        }
        let mut delegate = layered_architecture()
            .considering_all_dependencies()
            .layer(DOMAIN_MODEL_LAYER)
            .defined_by_with(or_false(&self.domain_models))
            .layer(DOMAIN_SERVICE_LAYER)
            .defined_by_with(or_false(&self.domain_services))
            .layer(APPLICATION_SERVICE_LAYER)
            .defined_by_with(or_false(&self.application_services))
            .layer(ADAPTER_LAYER)
            .defined_by_with(any_adapter)
            .where_layer(DOMAIN_MODEL_LAYER)
            .may_only_be_accessed_by_layers(&[
                DOMAIN_SERVICE_LAYER,
                APPLICATION_SERVICE_LAYER,
                ADAPTER_LAYER,
            ])
            .where_layer(DOMAIN_SERVICE_LAYER)
            .may_only_be_accessed_by_layers(&[APPLICATION_SERVICE_LAYER, ADAPTER_LAYER])
            .where_layer(APPLICATION_SERVICE_LAYER)
            .may_only_be_accessed_by_layers(&[ADAPTER_LAYER])
            .with_optional_layers(self.optional_layers);
        for (name, predicate) in &self.adapters {
            let adapter_layer = format!("{name} {ADAPTER_LAYER}");
            delegate = delegate
                .layer(&adapter_layer)
                .defined_by_with(predicate.clone())
                .where_layer(&adapter_layer)
                .may_not_be_accessed_by_any_layer();
        }
        for ignored in &self.ignored_dependencies {
            delegate = delegate.ignore_dependency_where(ignored.clone());
        }
        if let Some(ignore) = &self.all_items_contained_check {
            delegate = delegate
                .ensure_all_classes_are_contained_in_architecture_ignoring_with(ignore.clone());
        }
        delegate.as_(&self.description())
    }
}

impl OnionArchitecture {
    /// Evaluates the rule without failing (`evaluate(..)`).
    pub fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        ArchRule::evaluate(self, items)
    }

    /// Evaluates the rule and panics with the failure report if it is violated (`check(..)`).
    pub fn check(&self, items: &RustItems) {
        ArchRule::check(self, items);
    }
}

impl HasDescription for OnionArchitecture {
    fn description(&self) -> String {
        OnionArchitecture::description(self)
    }
}

impl ArchRule for OnionArchitecture {
    fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        ArchRule::evaluate(&self.layered_architecture_delegate(), items)
    }

    fn clone_boxed(&self) -> Box<dyn ArchRule> {
        Box::new(self.clone())
    }

    fn allow_empty_should_boxed(&self, allow_empty_should: bool) -> Box<dyn ArchRule> {
        Box::new(self.clone().allow_empty_should(allow_empty_should))
    }
}
