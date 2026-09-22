//! `[rust-only]` A modular monolith: business modules that may only depend on each other
//! along declared edges, through their public API, and without cycles.
//!
//! ArchUnit has no `Architectures.modularMonolith()`; its `library.modules` API covers the
//! same checks one rule at a time. This builder bundles them in the style of
//! [`layered_architecture`](super::layered_architecture) and
//! [`onion_architecture`](super::onion_architecture):
//!
//! ```no_run
//! use archunit::core::domain::rust_item::predicates::reside_in_a_package;
//! use archunit::prelude::*;
//! # let items = archunit::core::importer::CrateImporter::new().import_path("src");
//!
//! modular_monolith()
//!     .module("orders").defined_by(&["..orders.."])
//!     .module("billing").defined_by(&["..billing.."])
//!     .module("shared").defined_by(&["..shared.."])
//!     .where_module("shared").may_not_depend_on_any_module()
//!     .where_module("billing").may_only_depend_on_modules(&["shared"])
//!     .modules_may_only_depend_on_each_other_through_items_that(reside_in_a_package("..api.."))
//!     .modules_should_be_free_of_cycles()
//!     .check(&items);
//! ```
//!
//! Only dependencies between items of different modules are checked; dependencies on the
//! standard library, on other crates and on items outside every module are irrelevant, as
//! with `considering_only_dependencies_in_layers()`.

use std::fmt;
use std::sync::Arc;

use crate::base::{DescribedPredicate, HasDescription, always_false, join_single_quoted};
use crate::core::domain::dependency::predicates::dependency;
use crate::core::domain::properties::ItemSelector;
use crate::core::domain::rust_item::predicates::reside_in_any_package;
use crate::core::domain::{Dependency, PackageMatcher, RustItem, RustItems};
use crate::lang::conditions::{only_have_dependencies_where, only_have_dependents_where};
use crate::lang::syntax::classes;
use crate::lang::{
    ArchCondition, ArchRule, ConditionEvents, ConditionLogic, EvaluationResult, Priority,
    SimpleConditionEvent,
};
use crate::library::dependencies::{SliceIdentifier, slice_assignment, slices};

/// `Architectures::modular_monolith()`.
pub fn modular_monolith() -> ModularMonolithArchitecture {
    ModularMonolithArchitecture::new()
}

#[derive(Clone)]
struct ModuleDefinitionData {
    name: String,
    optional: bool,
    contains: DescribedPredicate<RustItem>,
}

#[derive(Clone)]
struct PackagePattern {
    identifier: String,
    matcher: PackageMatcher,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Constraint {
    /// The module's outgoing dependencies (`may only depend on ..`).
    Target,
    /// The module's incoming dependencies (`may only be depended on by ..`).
    Origin,
}

#[derive(Clone)]
struct ModuleDependencySpecificationData {
    module_name: String,
    allowed: Vec<String>,
    constraint: Constraint,
    description_suffix: String,
}

/// The assignment of items to modules, shared by every check of one evaluation.
#[derive(Clone)]
struct ModuleAssignment {
    definitions: Vec<ModuleDefinitionData>,
    pattern: Option<PackagePattern>,
    naming_pattern: Option<String>,
}

impl ModuleAssignment {
    /// The module `item` belongs to: the first matching definition, else the pattern.
    fn module_of(&self, item: &RustItem) -> Option<String> {
        if let Some(definition) = self.definitions.iter().find(|d| d.contains.test(item)) {
            return Some(definition.name.clone());
        }
        let pattern = self.pattern.as_ref()?;
        let result = pattern.matcher.match_(&item.package_name())?;
        if result.number_of_groups() == 0 {
            return None;
        }
        Some(match &self.naming_pattern {
            Some(naming) => {
                let mut name = naming.clone();
                for (index, group) in result.groups().iter().enumerate() {
                    name = name.replace(&format!("${}", index + 1), group);
                }
                name
            }
            None => result.groups().join("-"),
        })
    }

    fn in_modules(&self, names: Vec<String>) -> DescribedPredicate<RustItem> {
        let assignment = self.clone();
        DescribedPredicate::describe(
            format!("belong to modules [{}]", join_single_quoted(&names)),
            move |item: &RustItem| {
                assignment
                    .module_of(item)
                    .is_some_and(|m| names.contains(&m))
            },
        )
    }

    fn in_any_module(&self) -> DescribedPredicate<RustItem> {
        let assignment = self.clone();
        DescribedPredicate::describe("belong to a module", move |item: &RustItem| {
            assignment.module_of(item).is_some()
        })
    }
}

/// A modular monolith rule (`Architectures::modular_monolith()`) `[rust-only]`.
#[derive(Clone)]
pub struct ModularMonolithArchitecture {
    assignment: ModuleAssignment,
    specifications: Vec<ModuleDependencySpecificationData>,
    through_items: Option<DescribedPredicate<RustItem>>,
    free_of_cycles: bool,
    irrelevant_dependencies: Option<DescribedPredicate<Dependency>>,
    overridden_description: Option<String>,
    optional_modules: bool,
    all_items_contained_check: Option<DescribedPredicate<RustItem>>,
}

impl fmt::Debug for ModularMonolithArchitecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ModularMonolithArchitecture({})", self.description())
    }
}

impl fmt::Display for ModularMonolithArchitecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.description())
    }
}

impl Default for ModularMonolithArchitecture {
    fn default() -> Self {
        Self::new()
    }
}

impl ModularMonolithArchitecture {
    /// An architecture without modules yet.
    pub fn new() -> Self {
        Self {
            assignment: ModuleAssignment {
                definitions: Vec::new(),
                pattern: None,
                naming_pattern: None,
            },
            specifications: Vec::new(),
            through_items: None,
            free_of_cycles: false,
            irrelevant_dependencies: None,
            overridden_description: None,
            optional_modules: false,
            all_items_contained_check: None,
        }
    }

    /// Starts defining a module (`module(name)`).
    ///
    /// # Panics
    /// If `name` is empty.
    pub fn module(self, name: &str) -> ModuleDefinition {
        ModuleDefinition::new(self, name, false)
    }

    /// Starts defining a module that may be empty (`optional_module(name)`).
    pub fn optional_module(self, name: &str) -> ModuleDefinition {
        ModuleDefinition::new(self, name, true)
    }

    /// Defines one module per capture group of a package identifier, e.g. `..my_app.(*)..`
    /// makes `my_app::orders` and `my_app::billing` the modules `orders` and `billing`
    /// (ArchUnit: `modules().definedByPackages(..)`). Items matched by an explicit
    /// [`module`](Self::module) definition keep that module.
    ///
    /// # Panics
    /// If the identifier is not a valid package identifier.
    pub fn modules_defined_by_packages(mut self, package_identifier: &str) -> Self {
        self.assignment.pattern = Some(PackagePattern {
            identifier: package_identifier.to_owned(),
            matcher: PackageMatcher::of(package_identifier),
        });
        self
    }

    /// Names the modules of [`modules_defined_by_packages`](Self::modules_defined_by_packages)
    /// with `$1`, `$2`, ... standing for the capture groups (ArchUnit:
    /// `derivingNameFromPattern(..)`); by default the groups are joined with `-`.
    pub fn naming_modules(mut self, pattern: &str) -> Self {
        self.assignment.naming_pattern = Some(pattern.to_owned());
        self
    }

    /// Starts a constraint on a module (`where_module(name)`).
    ///
    /// # Panics
    /// If `name` is neither a defined module nor can come from
    /// [`modules_defined_by_packages`](Self::modules_defined_by_packages).
    pub fn where_module(self, name: &str) -> ModuleDependencySpecification {
        self.check_module_names_exist(&[name]);
        ModuleDependencySpecification {
            architecture: self,
            module_name: name.to_owned(),
        }
    }

    fn check_module_names_exist(&self, names: &[&str]) {
        if self.assignment.pattern.is_some() {
            return;
        }
        for name in names {
            assert!(
                self.assignment.definitions.iter().any(|d| d.name == *name),
                "There is no module named '{name}'"
            );
        }
    }

    /// Modules may only depend on items of other modules that satisfy `predicate`, e.g.
    /// `reside_in_a_package("..api..")` or `are_public()` (ArchUnit:
    /// `onlyDependOnEachOtherThroughClassesThat(..)`).
    pub fn modules_may_only_depend_on_each_other_through_items_that(
        mut self,
        predicate: DescribedPredicate<RustItem>,
    ) -> Self {
        self.through_items = Some(predicate);
        self
    }

    /// Modules may only depend on items of other modules residing in the given packages
    /// (ArchUnit: `onlyDependOnEachOtherThroughPackagesDeclaredIn(..)`).
    pub fn modules_may_only_depend_on_each_other_through_packages(
        self,
        package_identifiers: &[&str],
    ) -> Self {
        let description = format!(
            "reside in any package [{}]",
            join_single_quoted(package_identifiers)
        );
        self.modules_may_only_depend_on_each_other_through_items_that(
            reside_in_any_package(package_identifiers).as_(description),
        )
    }

    /// The modules must not form dependency cycles (ArchUnit: `beFreeOfCycles()`).
    pub fn modules_should_be_free_of_cycles(mut self) -> Self {
        self.free_of_cycles = true;
        self
    }

    /// Ignores dependencies from `origin` to `target`, given as full names or predicates
    /// (`ignore_dependency(..)`).
    pub fn ignore_dependency(
        self,
        origin: impl Into<ItemSelector>,
        target: impl Into<ItemSelector>,
    ) -> Self {
        self.ignore_dependency_where(dependency(origin, target))
    }

    /// Ignores dependencies matching `predicate`.
    pub fn ignore_dependency_where(mut self, predicate: DescribedPredicate<Dependency>) -> Self {
        self.irrelevant_dependencies = Some(match self.irrelevant_dependencies {
            Some(existing) => existing.or(predicate),
            None => predicate,
        });
        self
    }

    /// Whether empty modules are tolerated (`with_optional_modules(..)`).
    pub fn with_optional_modules(mut self, optional_modules: bool) -> Self {
        self.optional_modules = optional_modules;
        self
    }

    /// Fails for items that belong to no module (`ensure_all_classes_are_contained_in_architecture()`).
    pub fn ensure_all_classes_are_contained_in_architecture(self) -> Self {
        self.ensure_all_classes_are_contained_in_architecture_ignoring_with(always_false())
    }

    /// Like [`ensure_all_classes_are_contained_in_architecture`] but tolerates items in the
    /// given packages.
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
    /// `predicate`.
    ///
    /// [`ensure_all_classes_are_contained_in_architecture`]: Self::ensure_all_classes_are_contained_in_architecture
    pub fn ensure_all_classes_are_contained_in_architecture_ignoring_with(
        mut self,
        predicate: DescribedPredicate<RustItem>,
    ) -> Self {
        self.all_items_contained_check = Some(predicate);
        self
    }

    /// Overrides the description (`as_(..)`).
    pub fn as_(mut self, description: &str) -> Self {
        self.overridden_description = Some(description.to_owned());
        self
    }

    /// Appends `, because <reason>` to the description (`because(..)`).
    pub fn because(self, reason: &str) -> Box<dyn ArchRule> {
        let rule: &dyn ArchRule = &self;
        rule.because(reason)
    }

    /// Same as [`with_optional_modules`](Self::with_optional_modules) (`allow_empty_should(..)`).
    pub fn allow_empty_should(self, allow_empty_should: bool) -> Self {
        self.with_optional_modules(allow_empty_should)
    }

    /// The description, one line per module and constraint.
    pub fn description(&self) -> String {
        if let Some(description) = &self.overridden_description {
            return description.clone();
        }
        let mut lines = vec![format!(
            "Modular monolith consisting of{}",
            if self.optional_modules {
                " (optional)"
            } else {
                ""
            }
        )];
        for definition in &self.assignment.definitions {
            lines.push(format!(
                "{}module '{}' ({})",
                if definition.optional { "optional " } else { "" },
                definition.name,
                definition.contains.description()
            ));
        }
        if let Some(pattern) = &self.assignment.pattern {
            lines.push(match &self.assignment.naming_pattern {
                Some(naming) => format!(
                    "modules defined by packages '{}' named '{naming}'",
                    pattern.identifier
                ),
                None => format!("modules defined by packages '{}'", pattern.identifier),
            });
        }
        for specification in &self.specifications {
            lines.push(format!(
                "where module '{}' {}",
                specification.module_name, specification.description_suffix
            ));
        }
        if let Some(predicate) = &self.through_items {
            lines.push(format!(
                "where modules may only depend on each other through items that {}",
                predicate.description()
            ));
        }
        if self.free_of_cycles {
            lines.push("where modules should be free of cycles".to_owned());
        }
        lines.join("\n")
    }

    /// Evaluates the rule without failing (`evaluate(..)`).
    pub fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        ArchRule::evaluate(self, items)
    }

    /// Evaluates the rule and panics with the failure report if it is violated (`check(..)`).
    pub fn check(&self, items: &RustItems) {
        ArchRule::check(self, items);
    }

    fn add_specification(mut self, specification: ModuleDependencySpecificationData) -> Self {
        if let Some(existing) = self.specifications.iter_mut().find(|s| {
            s.module_name == specification.module_name && s.constraint == specification.constraint
        }) {
            *existing = specification;
        } else {
            self.specifications.push(specification);
        }
        self
    }

    fn is_relevant(
        &self,
        predicate: DescribedPredicate<Dependency>,
    ) -> DescribedPredicate<Dependency> {
        // Dependencies whose origin or target belongs to no module never count.
        let assignment = self.assignment.clone();
        let outside_modules = DescribedPredicate::describe(
            "origin or target outside of every module",
            move |d: &Dependency| {
                assignment.module_of(&d.origin_item()).is_none()
                    || assignment.module_of(&d.target_item()).is_none()
            },
        );
        let configured = predicate.or(outside_modules);
        match &self.irrelevant_dependencies {
            Some(irrelevant) => configured.or(irrelevant.clone()),
            None => configured,
        }
    }

    fn evaluate_module_should_not_be_empty(
        &self,
        items: &RustItems,
        definition: &ModuleDefinitionData,
    ) -> EvaluationResult {
        classes()
            .that_with(self.assignment.in_modules(vec![definition.name.clone()]))
            .should_with(ArchCondition::from_logic(
                "not be empty",
                ModuleShouldNotBeEmpty {
                    module_name: definition.name.clone(),
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
        let assignment = self.assignment.clone();
        let ignore = ignore.clone();
        classes()
            .should_with(ArchCondition::new(
                "be contained in architecture",
                move |item: &RustItem, events: &mut ConditionEvents| {
                    if !ignore.test(item) && assignment.module_of(item).is_none() {
                        events.add(SimpleConditionEvent::violated(
                            item,
                            format!("Item <{}> is not contained in architecture", item.name()),
                        ));
                    }
                },
            ))
            .evaluate(items)
    }

    fn evaluate_specification(
        &self,
        items: &RustItems,
        specification: &ModuleDependencySpecificationData,
    ) -> EvaluationResult {
        let assignment = self.assignment.clone();
        let own = specification.module_name.clone();
        let mut allowed = specification.allowed.clone();
        allowed.push(own.clone());
        let condition = match specification.constraint {
            Constraint::Target => {
                let matches = DescribedPredicate::describe(
                    format!(
                        "target belongs to modules [{}]",
                        join_single_quoted(&allowed)
                    ),
                    move |d: &Dependency| {
                        assignment
                            .module_of(&d.target_item())
                            .is_some_and(|m| allowed.contains(&m))
                    },
                );
                only_have_dependencies_where(self.is_relevant(matches))
            }
            Constraint::Origin => {
                let matches = DescribedPredicate::describe(
                    format!(
                        "origin belongs to modules [{}]",
                        join_single_quoted(&allowed)
                    ),
                    move |d: &Dependency| {
                        assignment
                            .module_of(&d.origin_item())
                            .is_some_and(|m| allowed.contains(&m))
                    },
                );
                only_have_dependents_where(self.is_relevant(matches))
            }
        };
        classes()
            .that_with(self.assignment.in_modules(vec![own]))
            .should_with(condition)
            .allow_empty_should(true)
            .evaluate(items)
    }

    fn evaluate_through_items(
        &self,
        items: &RustItems,
        predicate: &DescribedPredicate<RustItem>,
    ) -> EvaluationResult {
        let assignment = self.assignment.clone();
        let through = predicate.clone();
        let same_module_or_api = DescribedPredicate::describe(
            format!(
                "target is in the same module or {}",
                predicate.description()
            ),
            move |d: &Dependency| {
                let origin = assignment.module_of(&d.origin_item());
                let target = assignment.module_of(&d.target_item());
                origin == target || through.test(&d.target_item())
            },
        );
        let condition =
            only_have_dependencies_where(self.is_relevant(same_module_or_api)).as_(format!(
                "only depend on items of other modules that {}",
                predicate.description()
            ));
        classes()
            .that_with(self.assignment.in_any_module())
            .should_with(condition)
            .allow_empty_should(true)
            .evaluate(items)
    }

    fn evaluate_free_of_cycles(&self, items: &RustItems) -> EvaluationResult {
        let assignment = self.assignment.clone();
        let mut rule = slices()
            .assigned_from(slice_assignment(
                "modules",
                move |item: &RustItem| match assignment.module_of(item) {
                    Some(module) => SliceIdentifier::of(&[&module]),
                    None => SliceIdentifier::ignore(),
                },
            ))
            .naming_slices("Module '$1'")
            .should()
            .be_free_of_cycles()
            .allow_empty_should(true);
        if let Some(irrelevant) = &self.irrelevant_dependencies {
            rule = rule.ignore_dependency_where(irrelevant.clone());
        }
        rule.evaluate(items)
    }
}

struct ModuleShouldNotBeEmpty {
    module_name: String,
    empty: bool,
}

impl ConditionLogic<RustItem> for ModuleShouldNotBeEmpty {
    fn init(&mut self, _all: &[RustItem]) {
        self.empty = true;
    }

    fn check(&mut self, _item: &RustItem, _events: &mut ConditionEvents) {
        self.empty = false;
    }

    fn finish(&mut self, events: &mut ConditionEvents) {
        if self.empty {
            events.add(SimpleConditionEvent::violated(
                self.module_name.clone(),
                format!("Module '{}' is empty", self.module_name),
            ));
        }
    }
}

impl HasDescription for ModularMonolithArchitecture {
    fn description(&self) -> String {
        ModularMonolithArchitecture::description(self)
    }
}

impl ArchRule for ModularMonolithArchitecture {
    fn evaluate(&self, items: &RustItems) -> EvaluationResult {
        let mut result = EvaluationResult::empty(self, Priority::Medium);
        if !self.optional_modules {
            for definition in self.assignment.definitions.iter().filter(|d| !d.optional) {
                result.add(self.evaluate_module_should_not_be_empty(items, definition));
            }
        }
        if let Some(ignore) = &self.all_items_contained_check {
            result.add(self.evaluate_all_items_are_contained(items, ignore));
        }
        for specification in &self.specifications {
            result.add(self.evaluate_specification(items, specification));
        }
        if let Some(predicate) = &self.through_items {
            result.add(self.evaluate_through_items(items, predicate));
        }
        if self.free_of_cycles {
            result.add(self.evaluate_free_of_cycles(items));
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

/// The `defined_by` step of `module(name)`.
#[derive(Debug)]
pub struct ModuleDefinition {
    architecture: ModularMonolithArchitecture,
    name: String,
    optional: bool,
}

impl ModuleDefinition {
    fn new(architecture: ModularMonolithArchitecture, name: &str, optional: bool) -> Self {
        assert!(!name.is_empty(), "Module name must be present");
        Self {
            architecture,
            name: name.to_owned(),
            optional,
        }
    }

    /// The module holds the items in any of the packages (`defined_by(..)`).
    pub fn defined_by(self, package_identifiers: &[&str]) -> ModularMonolithArchitecture {
        let description = join_single_quoted(package_identifiers);
        self.defined_by_with(reside_in_any_package(package_identifiers).as_(description))
    }

    /// The module holds the items matching `predicate` (`defined_by_with(..)`).
    pub fn defined_by_with(
        mut self,
        predicate: DescribedPredicate<RustItem>,
    ) -> ModularMonolithArchitecture {
        let definition = ModuleDefinitionData {
            name: self.name,
            optional: self.optional,
            contains: predicate,
        };
        let definitions = &mut self.architecture.assignment.definitions;
        if let Some(existing) = definitions.iter_mut().find(|d| d.name == definition.name) {
            *existing = definition;
        } else {
            definitions.push(definition);
        }
        self.architecture
    }
}

/// The constraint step of `where_module(name)`.
#[derive(Debug)]
pub struct ModuleDependencySpecification {
    architecture: ModularMonolithArchitecture,
    module_name: String,
}

impl ModuleDependencySpecification {
    /// `may_only_depend_on_modules(..)`.
    ///
    /// # Panics
    /// If `module_names` is empty or names an unknown module.
    pub fn may_only_depend_on_modules(self, module_names: &[&str]) -> ModularMonolithArchitecture {
        self.restrict(
            Constraint::Target,
            module_names,
            "may only depend on modules",
        )
    }

    /// `may_not_depend_on_any_module()`.
    pub fn may_not_depend_on_any_module(self) -> ModularMonolithArchitecture {
        self.deny(Constraint::Target, "may not depend on any module")
    }

    /// `may_only_be_depended_on_by_modules(..)`.
    ///
    /// # Panics
    /// If `module_names` is empty or names an unknown module.
    pub fn may_only_be_depended_on_by_modules(
        self,
        module_names: &[&str],
    ) -> ModularMonolithArchitecture {
        self.restrict(
            Constraint::Origin,
            module_names,
            "may only be depended on by modules",
        )
    }

    /// `may_not_be_depended_on_by_any_module()`.
    pub fn may_not_be_depended_on_by_any_module(self) -> ModularMonolithArchitecture {
        self.deny(Constraint::Origin, "may not be depended on by any module")
    }

    fn deny(self, constraint: Constraint, description: &str) -> ModularMonolithArchitecture {
        self.architecture
            .add_specification(ModuleDependencySpecificationData {
                module_name: self.module_name,
                allowed: Vec::new(),
                constraint,
                description_suffix: description.to_owned(),
            })
    }

    fn restrict(
        self,
        constraint: Constraint,
        module_names: &[&str],
        description: &str,
    ) -> ModularMonolithArchitecture {
        assert!(
            !module_names.is_empty(),
            "At least 1 module name must be provided."
        );
        self.architecture.check_module_names_exist(module_names);
        let mut allowed: Vec<String> = Vec::new();
        for name in module_names {
            if !allowed.iter().any(|a| a == name) {
                allowed.push((*name).to_owned());
            }
        }
        self.architecture
            .add_specification(ModuleDependencySpecificationData {
                module_name: self.module_name,
                allowed,
                constraint,
                description_suffix: format!("{description} [{}]", join_single_quoted(module_names)),
            })
    }
}

#[allow(dead_code)]
fn _assert_send_sync() {
    fn check<T: Send + Sync>() {}
    check::<Arc<ModularMonolithArchitecture>>();
}
