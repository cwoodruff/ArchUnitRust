//! PlantUML component diagrams as architecture rules
//! (`com.tngtech.archunit.library.plantuml.rules`).
//!
//! ```no_run
//! use archunit::library::plantuml::{Configuration, adhere_to_plant_uml_diagram};
//! use archunit::prelude::*;
//! # let items = archunit::core::importer::CrateImporter::new().import_path("src");
//!
//! classes()
//!     .should_with(adhere_to_plant_uml_diagram(
//!         "docs/architecture.puml",
//!         Configuration::considering_only_dependencies_in_diagram(),
//!     ))
//!     .check(&items);
//! ```
//!
//! Components carry their module identifiers as stereotypes: `[Service] <<..service..>>`.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use regex::Regex;

use crate::base::{DescribedPredicate, join_single_quoted};
use crate::core::domain::dependency::functions::{get_origin_item, get_target_item};
use crate::core::domain::properties::has_name::predicates::name;
use crate::core::domain::{Dependency, PackageMatcher, PackageMatchers, RustItem};
use crate::lang::conditions::only_have_dependencies_in_any_package;
use crate::lang::{ArchCondition, ConditionEvents, SimpleConditionEvent};

/// Errors while reading a diagram (`PlantUmlParseException`, `IllegalDiagramException`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlantUmlError {
    /// The diagram file could not be read.
    #[error("Could not parse diagram from {0}")]
    Parse(String),
    /// The diagram is syntactically fine but not usable as an architecture.
    #[error("{0}")]
    IllegalDiagram(String),
}

// ---- model ---------------------------------------------------------------------------------

/// A component of the diagram (`PlantUmlComponent`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlantUmlComponent {
    name: String,
    stereotypes: Vec<String>,
    alias: Option<String>,
}

impl PlantUmlComponent {
    /// The name between `[` and `]`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The stereotypes (`<<..package..>>`), i.e. the module identifiers.
    pub fn stereotypes(&self) -> &[String] {
        &self.stereotypes
    }

    /// The alias (`as alias`).
    pub fn alias(&self) -> Option<&str> {
        self.alias.as_deref()
    }
}

/// A parsed diagram (`PlantUmlDiagram`).
#[derive(Debug, Clone)]
pub struct PlantUmlDiagram {
    components: Vec<PlantUmlComponent>,
    /// Dependencies as indexes into `components`.
    dependencies: Vec<(usize, usize)>,
}

impl PlantUmlDiagram {
    /// Parses a diagram file.
    pub fn parse_file(path: impl AsRef<Path>) -> Result<Self, PlantUmlError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|e| PlantUmlError::Parse(format!("{} ({e})", path.display())))?;
        Self::parse(&text)
    }

    /// Parses diagram text.
    pub fn parse(text: &str) -> Result<Self, PlantUmlError> {
        let comment = Regex::new(r"^\s*'").expect("static regex");
        let lines: Vec<&str> = text
            .lines()
            .filter(|line| !comment.is_match(line))
            .collect();
        let mut components: Vec<PlantUmlComponent> = Vec::new();
        for line in &lines {
            if let Some(component) = parse_component(line)? {
                if !components.iter().any(|c| c.name == component.name) {
                    components.push(component);
                }
            }
        }
        let mut dependencies = Vec::new();
        for line in &lines {
            for (origin, target) in parse_dependencies(line) {
                let origin = find_component(&components, &origin);
                let target = find_component(&components, &target);
                if let (Some(o), Some(t)) = (origin, target) {
                    if !dependencies.contains(&(o, t)) {
                        dependencies.push((o, t));
                    }
                }
            }
        }
        Ok(Self {
            components,
            dependencies,
        })
    }

    /// All components (`getAllComponents()`).
    pub fn components(&self) -> &[PlantUmlComponent] {
        &self.components
    }

    /// The components with an alias (`getComponentsWithAlias()`).
    pub fn components_with_alias(&self) -> Vec<&PlantUmlComponent> {
        self.components
            .iter()
            .filter(|c| c.alias.is_some())
            .collect()
    }

    /// The components `component` depends on (`PlantUmlComponent.getDependencies()`).
    pub fn dependencies_of(&self, component: &PlantUmlComponent) -> Vec<&PlantUmlComponent> {
        let Some(index) = self.components.iter().position(|c| c == component) else {
            return Vec::new();
        };
        self.dependencies
            .iter()
            .filter(|(o, _)| *o == index)
            .map(|(_, t)| &self.components[*t])
            .collect()
    }
}

fn component_regex() -> &'static Regex {
    static REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"^\s*(?:component)?\s*\[(?P<componentName>[^\[\]]+)\]\s*(?:<<[^<>]+>>\s*)*\s*(?:as "?(?P<alias>[^" ]+)"?)?\s*(?:#[\w|/\\-]+)?\s*$"#,
        )
        .expect("static regex")
    })
}

fn stereotype_regex() -> &'static Regex {
    static REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"<<([^<>]+)>>").expect("static regex"))
}

fn parse_component(line: &str) -> Result<Option<PlantUmlComponent>, PlantUmlError> {
    let Some(captures) = component_regex().captures(line) else {
        return Ok(None);
    };
    let name = captures["componentName"].to_owned();
    let alias = captures.name("alias").map(|m| m.as_str().to_owned());
    if let Some(alias) = &alias
        && (alias.contains('[') || alias.contains(']') || alias.contains('"'))
    {
        return Err(PlantUmlError::IllegalDiagram(format!(
            "Alias '{alias}' should not contain character(s): '[' or ']' or '\"'"
        )));
    }
    let mut stereotypes: Vec<String> = Vec::new();
    for capture in stereotype_regex().captures_iter(line) {
        let stereotype = capture[1].to_owned();
        if !stereotypes.contains(&stereotype) {
            stereotypes.push(stereotype);
        }
    }
    if stereotypes.is_empty() {
        return Err(PlantUmlError::IllegalDiagram(format!(
            "Components must include at least one stereotype specifying the package identifier(<<..>>), but component '{name}' does not"
        )));
    }
    Ok(Some(PlantUmlComponent {
        name,
        stereotypes,
        alias,
    }))
}

fn arrow_regexes() -> &'static (Regex, Regex) {
    static REGEX: std::sync::OnceLock<(Regex, Regex)> = std::sync::OnceLock::new();
    REGEX.get_or_init(|| {
        let center = r"(left|right|up|down|\[[^\]]+\])?";
        (
            Regex::new(&format!(r"\s-+{center}-*>\s")).expect("static regex"),
            Regex::new(&format!(r"\s<-*{center}-+\s")).expect("static regex"),
        )
    })
}

/// `(origin, target)` names or aliases of the dependencies on a line, brackets stripped.
fn parse_dependencies(line: &str) -> Vec<(String, String)> {
    let (right, left) = arrow_regexes();
    let without_description = match line.find(':') {
        Some(index) => &line[..index],
        None => line,
    };
    let mut result = Vec::new();
    let split = |pattern: &Regex| -> Option<(String, String)> {
        let m = pattern.find(without_description)?;
        let first = without_description[..m.start()].trim();
        let second = without_description[m.end()..].trim();
        Some((first.to_owned(), second.to_owned()))
    };
    if let Some((origin, target)) = split(right) {
        result.push((origin, target));
    }
    if let Some((target, origin)) = split(left) {
        result.push((origin, target));
    }
    result
}

fn find_component(components: &[PlantUmlComponent], name_or_alias: &str) -> Option<usize> {
    let key = name_or_alias
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    components
        .iter()
        .position(|c| c.alias.as_deref() == Some(key))
        .or_else(|| components.iter().position(|c| c.name == key))
}

// ---- association of items with components ---------------------------------------------------

/// Which component an item belongs to, by its module (`JavaClassDiagramAssociation`).
#[derive(Debug, Clone)]
struct DiagramAssociation {
    diagram: PlantUmlDiagram,
    matchers: Vec<Vec<PackageMatcher>>,
}

impl DiagramAssociation {
    fn new(diagram: PlantUmlDiagram) -> Result<Self, PlantUmlError> {
        let mut seen = HashSet::new();
        for component in diagram.components() {
            for stereotype in component.stereotypes() {
                if !seen.insert(stereotype.clone()) {
                    return Err(PlantUmlError::IllegalDiagram(format!(
                        "Stereotype '{stereotype}' should be unique"
                    )));
                }
            }
        }
        let matchers = diagram
            .components()
            .iter()
            .map(|c| {
                c.stereotypes()
                    .iter()
                    .map(|s| PackageMatcher::try_of(s))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| PlantUmlError::IllegalDiagram(e.to_string()))?;
        Ok(Self { diagram, matchers })
    }

    fn associated_components(&self, item: &RustItem) -> Vec<&PlantUmlComponent> {
        let package = item.package_name();
        self.diagram
            .components()
            .iter()
            .zip(&self.matchers)
            .filter(|(_, matchers)| matchers.iter().any(|m| m.matches(&package)))
            .map(|(c, _)| c)
            .collect()
    }

    fn contains(&self, item: &RustItem) -> bool {
        !self.associated_components(item).is_empty()
    }

    /// The package identifiers of the item's component and of the components it depends on.
    fn allowed_package_identifiers(&self, component: &PlantUmlComponent) -> Vec<String> {
        let mut result: Vec<String> = component.stereotypes().to_vec();
        for target in self.diagram.dependencies_of(component) {
            for stereotype in target.stereotypes() {
                if !result.contains(stereotype) {
                    result.push(stereotype.clone());
                }
            }
        }
        result
    }
}

// ---- configuration -------------------------------------------------------------------------

type IgnorePredicateFactory =
    Arc<dyn Fn(&DiagramAssociation) -> DescribedPredicate<Dependency> + Send + Sync>;

/// Which dependencies the diagram must cover (`PlantUmlArchCondition.Configuration`).
#[derive(Clone)]
pub struct Configuration {
    create_ignore_predicate: IgnorePredicateFactory,
}

impl fmt::Debug for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PlantUml Configuration")
    }
}

impl Configuration {
    /// Every dependency must be allowed by the diagram (`consideringAllDependencies()`).
    pub fn considering_all_dependencies() -> Self {
        Self {
            create_ignore_predicate: Arc::new(|_| {
                crate::base::always_false::<Dependency>().as_("")
            }),
        }
    }

    /// Only dependencies whose target is in some component count
    /// (`consideringOnlyDependenciesInDiagram()`).
    pub fn considering_only_dependencies_in_diagram() -> Self {
        Self {
            create_ignore_predicate: Arc::new(|association: &DiagramAssociation| {
                let association = association.clone();
                DescribedPredicate::describe(
                    " while ignoring dependencies not contained in the diagram",
                    move |d: &Dependency| !association.contains(&d.target_item()),
                )
            }),
        }
    }

    /// Only dependencies whose target resides in one of the packages count
    /// (`consideringOnlyDependenciesInAnyPackage(..)`).
    pub fn considering_only_dependencies_in_any_package(package_identifiers: &[&str]) -> Self {
        let identifiers: Vec<String> = package_identifiers
            .iter()
            .map(|p| (*p).to_owned())
            .collect();
        Self {
            create_ignore_predicate: Arc::new(move |_| {
                let description = format!(
                    " while ignoring dependencies outside of packages [{}]",
                    join_single_quoted(identifiers.iter().map(String::as_str).collect::<Vec<_>>())
                );
                let refs: Vec<&str> = identifiers.iter().map(String::as_str).collect();
                let matchers = PackageMatchers::of(&refs);
                DescribedPredicate::describe(description, move |d: &Dependency| {
                    !matchers.test(&d.target_item().package_name())
                })
            }),
        }
    }
}

// ---- the condition -------------------------------------------------------------------------

/// `adhereToPlantUmlDiagram(..)`: items may only depend on items of their own component or of
/// components their component has an arrow to.
///
/// # Panics
/// If the file cannot be read or the diagram is illegal; see
/// [`try_adhere_to_plant_uml_diagram`] for a `Result`.
pub fn adhere_to_plant_uml_diagram(
    path: impl AsRef<Path>,
    configuration: Configuration,
) -> PlantUmlArchCondition {
    try_adhere_to_plant_uml_diagram(path, configuration).unwrap_or_else(|e| panic!("{e}"))
}

/// The fallible form of [`adhere_to_plant_uml_diagram`].
pub fn try_adhere_to_plant_uml_diagram(
    path: impl AsRef<Path>,
    configuration: Configuration,
) -> Result<PlantUmlArchCondition, PlantUmlError> {
    let path = path.as_ref();
    let diagram = PlantUmlDiagram::parse_file(path)?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    PlantUmlArchCondition::create(&file_name, diagram, configuration)
}

/// `adhereToPlantUmlDiagram` for diagram text held in memory `[rust-only]`; `name` is shown in
/// the rule description.
pub fn adhere_to_plant_uml_diagram_from_str(
    name: &str,
    diagram: &str,
    configuration: Configuration,
) -> Result<PlantUmlArchCondition, PlantUmlError> {
    PlantUmlArchCondition::create(name, PlantUmlDiagram::parse(diagram)?, configuration)
}

/// The condition built by [`adhere_to_plant_uml_diagram`] (`PlantUmlArchCondition`).
#[derive(Clone)]
pub struct PlantUmlArchCondition {
    description: String,
    ignore: DescribedPredicate<Dependency>,
    association: Arc<DiagramAssociation>,
}

impl fmt::Debug for PlantUmlArchCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PlantUmlArchCondition({})", self.description)
    }
}

impl PlantUmlArchCondition {
    fn create(
        name: &str,
        diagram: PlantUmlDiagram,
        configuration: Configuration,
    ) -> Result<Self, PlantUmlError> {
        if diagram.components().is_empty() {
            return Err(PlantUmlError::IllegalDiagram(format!(
                "No components defined in diagram <{name}>"
            )));
        }
        let association = DiagramAssociation::new(diagram)?;
        let ignore = (configuration.create_ignore_predicate)(&association);
        Ok(Self {
            description: format!(
                "adhere to PlantUML diagram <{name}>{}",
                ignore.description()
            ),
            ignore,
            association: Arc::new(association),
        })
    }

    /// The description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// `ignoreDependenciesWithOrigin(pred)`.
    pub fn ignore_dependencies_with_origin(self, predicate: DescribedPredicate<RustItem>) -> Self {
        let description = format!(
            "ignoring dependencies with origin {}",
            predicate.description()
        );
        self.ignore_dependencies_with(get_origin_item().is(predicate).as_(description))
    }

    /// `ignoreDependenciesWithTarget(pred)`.
    pub fn ignore_dependencies_with_target(self, predicate: DescribedPredicate<RustItem>) -> Self {
        let description = format!(
            "ignoring dependencies with target {}",
            predicate.description()
        );
        self.ignore_dependencies_with(get_target_item().is(predicate).as_(description))
    }

    /// `ignoreDependencies(origin, target)` by full name.
    pub fn ignore_dependencies(self, origin: &str, target: &str) -> Self {
        let description = format!("ignoring dependencies from {origin} to {target}");
        self.ignore_dependencies_with(
            get_origin_item()
                .is(name::<RustItem>(origin))
                .and(get_target_item().is(name::<RustItem>(target)))
                .as_(description),
        )
    }

    /// `ignoreDependencies(DescribedPredicate<Dependency>)`.
    pub fn ignore_dependencies_with(self, predicate: DescribedPredicate<Dependency>) -> Self {
        Self {
            description: format!("{}, {}", self.description, predicate.description()),
            ignore: self.ignore.or(predicate),
            association: self.association,
        }
    }

    fn check(&self, item: &RustItem, events: &mut ConditionEvents) {
        let dependencies = item.direct_dependencies_from_self();
        if dependencies.iter().all(|d| self.ignore.test(d)) {
            return;
        }
        let components = self.association.associated_components(item);
        if components.is_empty() {
            events.add(SimpleConditionEvent::violated(
                item,
                format!("Item {} is not contained in any component", item.name()),
            ));
            return;
        }
        if components.len() > 1 {
            let mut names: Vec<&str> = components.iter().map(|c| c.name()).collect();
            names.sort_unstable();
            events.add(SimpleConditionEvent::violated(
                item,
                format!(
                    "Item {} may not be contained in more than one component, but is contained in [{}]",
                    item.name(),
                    names.join(", ")
                ),
            ));
            return;
        }
        let allowed = self.association.allowed_package_identifiers(components[0]);
        let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
        let delegate: ArchCondition<RustItem> = only_have_dependencies_in_any_package(&allowed)
            .ignore_dependency(self.ignore.clone())
            .into();
        delegate.check(item, events);
    }
}

impl From<PlantUmlArchCondition> for ArchCondition<RustItem> {
    fn from(condition: PlantUmlArchCondition) -> Self {
        let description = condition.description.clone();
        ArchCondition::new(
            description,
            move |item: &RustItem, events: &mut ConditionEvents| condition.check(item, events),
        )
    }
}

#[allow(dead_code)]
fn _assert_send_sync() {
    fn check<T: Send + Sync>() {}
    check::<HashMap<String, String>>();
}
