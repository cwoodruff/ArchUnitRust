# ArchUnit for Rust

`archunit` checks the architecture of a Rust crate with rules written as ordinary tests, the
way [ArchUnit](https://www.archunit.org) does for Java. It is a port of ArchUnit's concepts,
API shape, rule semantics and failure output: if you know ArchUnit, you already know this
crate, and the [ArchUnit user guide](https://www.archunit.org/userguide/html/000_Index.html)
works as documentation after a name translation (`resideInAPackage` → `reside_in_a_package`,
`JavaClass` → `RustItem`, package → module).

```rust
use archunit::prelude::*;

#[analyze_classes(packages = ["my_app.."], import_options = [DoNotIncludeTests])]
mod architecture {
    use super::*;

    #[arch_test]
    fn services_should_not_access_controllers() -> impl ArchRule {
        no_classes().that().reside_in_a_package("..service..")
            .should().access_classes_that().reside_in_a_package("..controller..")
    }

    #[arch_test]
    fn layers_are_respected() -> impl ArchRule {
        layered_architecture().considering_all_dependencies()
            .layer("Controller").defined_by(&["..controller.."])
            .layer("Service").defined_by(&["..service.."])
            .layer("Persistence").defined_by(&["..persistence.."])
            .where_layer("Controller").may_not_be_accessed_by_any_layer()
            .where_layer("Service").may_only_be_accessed_by_layers(&["Controller"])
            .where_layer("Persistence").may_only_be_accessed_by_layers(&["Service"])
    }
}
```

A violated rule fails the test with ArchUnit's report:

```
Architecture Violation [Priority: MEDIUM] - Rule 'no classes that reside in a package '..service..' should access classes that reside in a package '..controller..'' was violated (2 times):
Method <my_app::service::ServiceViolatingLayerRules::illegal_access_to_controller()> calls constructor <my_app::controller::SomeGuiController> in (src/service/mod.rs:58)
Method <my_app::service::ServiceViolatingLayerRules::illegal_access_to_controller()> calls method <my_app::controller::SomeGuiController::handle()> in (src/service/mod.rs:59)
```

## How it works

There are no class files in Rust, so the importer parses source with `syn` and discovers
crates with `cargo metadata`. It resolves `use` declarations, re-exports, globs, `#[path]`
modules, inherent and trait impls, method calls through known receiver types, constructor
calls, field accesses, macro invocations and attributes, and builds the same kind of model
ArchUnit builds from bytecode:

| ArchUnit | archunit for Rust |
|---|---|
| `ClassFileImporter` | `CrateImporter` (`syn` + `cargo_metadata`) |
| `JavaClasses` / `JavaClass` | `RustItems` / `RustItem`: structs, enums, traits, functions, type aliases, consts, statics, macros |
| `JavaPackage` | `RustModule`: a module path such as `my_app::service` |
| package identifiers `..service..` | the same syntax on module paths (`..service..`, `my_app::domain::(*)..`, `crate::adapter::[cli|rest]..`); a single `.` is accepted for `::` |
| accesses and dependencies | method/constructor calls, field accesses, function references, field/parameter/return/error types, trait impls, supertraits, attributes, type arguments, `use` imports, macro invocations |
| inheritance | `impl Trait for Type` and supertraits (`is_assignable_to`, `implement`) |
| annotations | attributes and derives (`are_annotated_with("secured")`) |
| modifiers | visibility, `unsafe`, `async`, `const`, `static`, `mut` |
| checked exceptions | the `E` of a `Result<_, E>` return type (`declare_throwable_of_type`) |
| `DoNotIncludeTests` | excludes `#[cfg(test)]` code and `tests/`, `benches/` targets |
| `@AnalyzeClasses` / `@ArchTest` | `#[analyze_classes]` / `#[arch_test]` |
| `archunit.properties` | `archunit.toml` (+ `ARCHUNIT_*` environment overrides) |

The rule DSL, the predefined predicates and conditions, layered and onion architectures,
slices with cycle detection, PlantUML diagrams, freezing rules and the general coding rules
are all ported. Every API element and every difference is recorded in
[docs/MAPPING.md](docs/MAPPING.md), including the ArchUnit features that have no Rust
counterpart and why.

## Getting started

```toml
[dev-dependencies]
archunit = "0.1"
```

Write the rules as a test, either with the attributes or with the plain API:

```rust
// tests/architecture.rs
use archunit::prelude::*;

#[test]
fn services_should_only_be_accessed_by_controllers() {
    let items = CrateImporter::new()
        .with_import_option(DoNotIncludeTests)
        .import_workspace();

    classes().that().reside_in_a_package("..service..")
        .should().only_be_accessed().by_any_package(&["..controller..", "..service.."])
        .check(&items);
}
```

`#[analyze_classes]` on a module imports once per configuration and caches the result for the
whole test run; with no arguments it analyzes the crate the test belongs to. The attribute
takes `packages`, `packages_of`, `items`, `locations`, `whole_workspace`, `import_options` and
`cache_mode`. `#[arch_test]` accepts a function returning a rule, a function taking
`&RustItems`, or a function returning `ArchTests::in_(other_rules::arch_tests)` to include a
`#[arch_rules]` module. `#[arch_ignore(reason = "..")]` skips a rule and `#[arch_tag("slow")]`
adds `__tag_slow` to the test name so `cargo test __tag_slow` selects it.

## Java and Rust side by side

Importing:

```java
JavaClasses classes = new ClassFileImporter()
        .withImportOption(new DoNotIncludeTests())
        .importPackages("com.myapp");
```

```rust
let items = CrateImporter::new()
    .with_import_option(DoNotIncludeTests)
    .import_packages(&["my_app.."]);
```

Rules about classes and members:

```java
classes().that().resideInAPackage("..service..")
        .and().areAnnotatedWith(MyService.class)
        .should().haveSimpleNameStartingWith("Service");

noMethods().that().areDeclaredInClassesThat().haveNameMatching(".*Dao")
        .should().declareThrowableOfType(SQLException.class);
```

```rust
classes().that().reside_in_a_package("..service..")
    .and().are_annotated_with("my_service")
    .should().have_simple_name_starting_with("Service");

no_methods().that().are_declared_in_classes_that().have_name_matching(".*Dao")
    .should().declare_throwable_of_type("my_app::db::SqlError");
```

Overloads taking an object instead of continuing the sentence get a `_with` suffix; overloads
over `String | Class<?> | DescribedPredicate` collapse into one method:

```java
classes().that(annotatedWith(Secured.class)).should(beProtected());
noClasses().should().dependOnClassesThat(assignableTo(EntityManager.class));
```

```rust
classes().that_with(annotated_with("secured")).should_with(be_protected());
no_classes().should().depend_on_classes_that_with(assignable_to("my_app::db::EntityManager"));
```

Custom conditions and predicates:

```java
ArchCondition<JavaClass> haveAFieldAnnotatedWithPayload =
    new ArchCondition<JavaClass>("have a field annotated with @Payload") {
        @Override
        public void check(JavaClass javaClass, ConditionEvents events) { ... }
    };
```

```rust
let have_a_field_annotated_with_payload: ArchCondition<RustItem> =
    ArchCondition::new("have a field annotated with @payload", |item, events| {
        let satisfied = item.fields().iter().any(|f| f.is_annotated_with("payload"));
        events.add(SimpleConditionEvent::new(item, satisfied, format!("{} has no payload field", item.description())));
    });
```

Onion architecture:

```java
onionArchitecture()
        .domainModels("..domain.model..")
        .domainServices("..domain.service..")
        .applicationServices("..application..")
        .adapter("cli", "..adapter.cli..")
        .adapter("persistence", "..adapter.persistence..");
```

```rust
onion_architecture()
    .domain_models(&["..domain.model.."])
    .domain_services(&["..domain.service.."])
    .application_services(&["..application.."])
    .adapter("cli", &["..adapter.cli.."])
    .adapter("persistence", &["..adapter.persistence.."]);
```

A modular monolith (a Rust-only builder in the same style; ArchUnit's `library.modules` API
covers the same checks one rule at a time):

```rust
modular_monolith()
    .module("orders").defined_by(&["..orders.."])
    .module("billing").defined_by(&["..billing.."])
    .module("shared").defined_by(&["..shared.."])
    .where_module("shared").may_not_depend_on_any_module()
    .where_module("billing").may_only_depend_on_modules(&["shared"])
    .modules_may_only_depend_on_each_other_through_packages(&["..api.."])
    .modules_should_be_free_of_cycles();
```

Slices and cycles:

```java
slices().matching("..myapp.(*)..").should().beFreeOfCycles();
slices().matching("..controller.(*)..").namingSlices("Controller $1")
        .should().notDependOnEachOther()
        .ignoreDependency(UseCaseOneController.class, UseCaseTwoController.class);
```

```rust
slices().matching("..my_app.(*)..").should().be_free_of_cycles();
slices().matching("..controller.(*)..").naming_slices("Controller $1")
    .should().not_depend_on_each_other()
    .ignore_dependency("my_app::controller::one::UseCaseOneController", "my_app::controller::two::UseCaseTwoController");
```

Freezing, PlantUML and the coding rules:

```java
freeze(noClasses().should().dependOnClassesThat().resideInAPackage("..legacy..")).check(classes);
classes().should(adhereToPlantUmlDiagram(diagram, consideringOnlyDependenciesInDiagram())).check(classes);
NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS.check(classes);
```

```rust
freeze(no_classes().should().depend_on_classes_that().reside_in_a_package("..legacy..")).check(&items);
classes().should_with(adhere_to_plant_uml_diagram("docs/architecture.puml", Configuration::considering_only_dependencies_in_diagram())).check(&items);
NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS.check(&items);
```

Rust-only additions keep the ArchUnit style and are marked `[rust-only]` in the mapping:
`NO_CLASSES_SHOULD_CALL_UNWRAP`, `NO_CLASSES_SHOULD_PANIC`,
`NO_LIBRARY_CODE_SHOULD_CALL_PROCESS_EXIT`, `NO_CLASSES_SHOULD_USE_UNSAFE`, the
`modular_monolith()` architecture, the `imports` dependency of a module, `reside_in_crate`,
and more.

## Configuration

`archunit.toml` next to your `Cargo.toml` (searched upward to the workspace root):

```toml
[arch_rule]
fail_on_empty_should = true

[cycles]
max_number_to_detect = 100
max_number_of_dependencies_per_edge = 20

[freeze]
refreeze = false
[freeze.store.default]
path = "archunit_store"
allow_store_creation = true

resolve_missing_dependencies_from_classpath = false
[class_resolver]
packages = ["tokio..", "serde.."]
```

Every key can be overridden by an environment variable, e.g.
`ARCHUNIT_FREEZE_REFREEZE=true`. Violations to ignore go into `archunit_ignore_patterns.txt`,
one regex per line, exactly as in ArchUnit.

## Examples

`examples/` ports the rules of `archunit-example` against the fixture crates in
`tests/fixtures/`; most of them are violated on purpose, and their reports are checked against
`examples/expected/`:

```sh
cargo run --example layered_architecture      # prints the reports
cargo test --examples                          # compares them with the expected reports
cargo test --example architecture_test         # the #[analyze_classes] harness
```

## Notable differences from ArchUnit

* Modules are packages: `classes()` never yields a module, but `use` declarations are
  `imports` dependencies of the module item, and a module counts as being in its own package.
* Impl blocks of imported types are folded into the type; impls for foreign types
  (`impl From<Order> for String`) are items of their own.
* Method calls on receivers of unknown type are resolved by unique name, marked
  `ResolutionKind::ByNameOnly`; otherwise the target is an `<unresolved>` stub.
* Free functions are items and single-member code units, so `methods()` rules cover them.
* `ArchTests::in_(..)` runs all included rules as one test, because `cargo test` cannot
  register tests dynamically.
* Generic exceptions are generic error types: `Box<dyn Error>`, `String`, `&str`, `()`,
  `anyhow::Error`, `eyre::Report`.

The full list with reasons is in [docs/MAPPING.md](docs/MAPPING.md); the design and the
known limits of source-based analysis are in [docs/PLAN.md](docs/PLAN.md).

## License

Apache-2.0. ArchUnit is Copyright TNG Technology Consulting GmbH and licensed under the
Apache License 2.0; see [NOTICE](NOTICE).
