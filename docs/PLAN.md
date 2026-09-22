# Implementation plan

Companion to [MAPPING.md](MAPPING.md). This file fixes the crate layout, the analysis
approach, the decisions that are not obvious from ArchUnit, and the known limits of
source-level (syn-based) analysis.

## 1. Workspace layout

```
ArchUnitRust/
├── Cargo.toml                 # package `archunit` (lib) + [workspace] members, excludes fixtures
├── src/
│   ├── lib.rs                 # re-exports: prelude, base, core, lang, library, harness
│   ├── base/                  # DescribedPredicate, DescribedFunction, HasDescription, errors
│   ├── core/
│   │   ├── domain/            # RustItems, RustItem, RustModule, members, accesses, Dependency,
│   │   │                      #   RustType, RustModifier, RustAnnotation, PackageMatcher, formatters
│   │   └── importer/          # CrateImporter, ImportOption, Location, cargo metadata, syn walk,
│   │                          #   name resolution, dependency collection
│   ├── lang/                  # ArchRule, ArchCondition, ConditionEvents, EvaluationResult,
│   │   ├── conditions/        #   ArchConditions, ArchPredicates
│   │   └── syntax/            #   ArchRuleDefinition, Given*/That*/Should* fluent types
│   ├── library/               # Architectures, dependencies (slices), cycle_detection,
│   │                          #   GeneralCodingRules, DependencyRules, ProxyRules, plantuml, freeze
│   ├── harness/               # cache, analyze_classes() builder, ArchTests, CacheMode
│   └── config.rs              # ArchConfiguration, archunit.toml, env overrides, ignore patterns
├── archunit-macros/           # proc-macro crate: #[analyze_classes], #[arch_test], #[arch_rules],
│                              #   #[arch_ignore], #[arch_tag], arch_tests!
├── tests/
│   ├── fixtures/              # standalone crates (each has its own `[workspace]` table)
│   │   ├── layered_app/       # controller / service / persistence / security / anticorruption
│   │   ├── onion_app/         # domain::model, domain::service, application, adapter::{cli,persistence,rest}
│   │   ├── cyclic_app/        # simplecycle, constructorcycle, fieldaccesscycle, inheritancecycle,
│   │   │                      #   membercycle, simplescenario, complexcycles
│   │   ├── reexports_app/     # pub use, globs, renames, nested & #[path] modules, cfg(test),
│   │   │                      #   tests/ dir, a bin target, a workspace dependency crate
│   │   ├── coding_rules_app/  # println!/unwrap/panic/process::exit/generic errors/deprecated (P5)
│   │   └── shopping_app/      # PlantUML example (P5/P6)
│   ├── importer.rs, domain.rs, lang.rs, library.rs, harness.rs, freeze.rs, plantuml.rs
│   └── expected/              # golden failure reports compared verbatim
├── examples/                  # ported archunit-example rules (P6)
├── docs/MAPPING.md, docs/PLAN.md
├── CLAUDE.md, NOTICE, LICENSE, README.md
└── archunit.toml              # (only in examples/fixtures)
```

Decisions:

* **Package name.** The root package is renamed from `ArchUnitRust` to `archunit` (crate
  names are lowercase by convention; the repository keeps its name). The proc-macro crate is
  `archunit-macros`, re-exported from `archunit::harness` so users add one dependency.
  If `archunit` is unavailable on crates.io at publish time the fallback is `archunit-rs`,
  which changes nothing in code except the `use` root.
* **Edition 2024**, stable toolchain, MSRV pinned to the current stable at Phase 1 and
  recorded in `Cargo.toml` (`rust-version`).
* **Fixtures are real crates**, excluded from the workspace (`exclude = ["tests/fixtures"]`)
  and each carrying an empty `[workspace]` table so `cargo metadata` works on them in
  isolation. They are never compiled by `cargo test`; the importer only parses them.
  One fixture (`reexports_app`) is itself a two-crate workspace to exercise cross-crate
  resolution.
* **Dependencies** (all stable, permissively licensed): `syn` (`full`, `visit`,
  `extra-traits`), `proc-macro2` (`span-locations`), `quote` (macros crate only),
  `cargo_metadata`, `regex`, `toml`, `serde`, `thiserror`, `uuid` (freeze store file names),
  `petgraph` is **not** used: the cycle detector is a small port of Johnson's algorithm on top of
  Tarjan's SCCs, matching ArchUnit's `cycle_detection` package so cycle ordering and limits behave
  the same.

## 2. Analysis approach

### 2.1 Crate discovery (`cargo_metadata`)

1. Run `cargo metadata --format-version 1` from the requested path (crate dir, `Cargo.toml`,
   or workspace root). Read packages, targets (`lib`, `bin`, `test`, `example`, `bench`,
   `proc-macro`), editions, manifest paths, and the resolved dependency graph.
2. Workspace members are **fully imported**. Non-member packages are treated like JAR
   classes: stubbed unless `resolve_missing_dependencies_from_classpath = true` (then their
   source is parsed from the registry/git checkout, bounded by the same
   `max_iterations_for_*` settings ArchUnit has). `std`, `core`, `alloc`, `proc_macro` are
   always stubs, named by path (`std::collections::HashMap`).
3. Each cargo target gets a crate root file. Targets share the package name; items are named
   `<crate_name>::…` where `<crate_name>` is the lib name (dashes → underscores). Binary,
   test, example and bench targets are named `<package>::<target>` internally but display as
   the target name only when it differs from the package (`my_app::bin::cli::main` appears as
   `cli::main`) — this keeps single-target crates readable.

### 2.2 Module tree walk (`syn`)

* Parse each crate root with `syn::parse_file`. Follow `mod foo;` to `foo.rs` or
  `foo/mod.rs` (edition-aware, `#[path = ".."]` honored, both inline and file modules).
  Nested inline modules are walked recursively.
* Every `syn::Item` becomes a `RustItem` with a stable `ItemId`, full path, kind,
  visibility, attributes, generics, source file and line (`proc_macro2::Span::start()` with
  the `span-locations` feature; the reported location is the crate-relative path of the file).
* Items inside function bodies (local items) are collected with `enclosing_code_unit` set.
* `impl` blocks: inherent and trait impls are items in their own right (`ItemKind::Impl`),
  and their associated functions/consts/types are attached as members to the **self type**
  when it resolves to an imported item, otherwise to the impl item. Trait impls also
  register an `implements` relation (`Type → Trait`) and, per method, a "implements trait
  method" link used by `all_methods()`.
* `#[cfg(test)]` on a module or item marks it (and its descendants) as test code. Items
  under `tests/`, `benches/`, `examples/` and `#[test]` functions are also test code.
  `DoNotIncludeTests` drops them; otherwise they are imported like everything else.
* Other `#[cfg(..)]` branches are all imported (no feature evaluation). Rationale: rules are
  about structure, and evaluating cfgs would require a target; documented in §4.

### 2.3 Name resolution

A per-module **scope** is built in two passes:

1. **Declaration pass.** Collect items declared in each module, `use` trees (with `as`
   renames, groups, `self`, `super`, `crate`, `::`-rooted and glob imports, `pub use`
   re-exports), `extern crate` (+ `as`), and macro definitions.
2. **Resolution pass (fixpoint).** Resolve each `use` to an `ItemId` by walking the path:
   `crate::` → crate root; `self::` → current module; `super::` → parent; `::x`/`x` → extern
   prelude (dependency crate names from cargo metadata, plus `std`/`core`/`alloc`) or a
   local declaration or an already-resolved import in the current scope, then glob imports
   (last, lower priority — as rustc does), then the standard prelude (a fixed list of std
   prelude names mapped to stub items). Re-export chains and globs are iterated until no
   new resolution appears. `use foo::*` where `foo` is a stub crate contributes nothing;
   unresolved names seen later fall back to stubs named `<glob_crate>::<name>` when exactly
   one glob import from a stub crate is in scope, else `<unresolved>::<name>`.

Path expressions inside bodies resolve against the enclosing module scope, then generic
parameters (which shadow items and are recorded as type-parameter dependencies), then
`Self` (inside impls and traits), then associated items (`Type::CONST`, `Type::func`,
`Trait::func`, `<T as Trait>::func`). Enum variants resolve through their enum.

### 2.4 Dependency and access collection

A `syn::visit::Visit` over every item records `Dependency` records (see MAPPING §2.6) and
`RustAccess` records for code units:

| Construct | Recorded as |
|---|---|
| `use path` | `imports` dependency from the module (owner = the module item) |
| types in fields, signatures, generics, `where`, aliases, consts/statics, `dyn`/`impl` | `has type`/`has parameter of type`/`has return type`/`references trait`/generic dependencies |
| `impl Trait for Type` | `implements` (Type→Trait), plus `has type` for generic args |
| supertraits | `extends` |
| `#[attr]`, `#[derive(A)]` | `is annotated with`; derives that resolve to a trait also add `implements` |
| `Path::func(..)`, `func(..)` | `MethodCall` access (or `ConstructorCall` if the target is constructor-classified) |
| `Type { .. }`, `Type(..)`, `Enum::Variant { .. }` | `ConstructorCall` |
| `recv.method(..)` | `MethodCall` with receiver-type resolution: `self.m()` inside an impl resolves against the self type's inherent and trait methods; a local whose type is known from a `let x: T`, a parameter, or a direct constructor call resolves; otherwise the call is resolved **by unique name** across all imported methods and flagged `ResolutionKind::ByNameOnly`; if ambiguous it becomes an unresolved target `<unresolved>::method` |
| `recv.field`, `recv.field = v`, `&mut recv.field` | `FieldAccess` (`Get`/`Set`) with the same receiver-resolution rules |
| function path used as a value | `FunctionReference` |
| `Foo::<T>`, `TypeId::of::<T>()`, `size_of::<T>()` | `references class object` |
| closures, `async` blocks | visited inline; accesses are attributed to the enclosing code unit with `is_declared_in_closure = true` |
| macro invocations | see §4 |

Reverse indexes (`accesses_to_self`, `direct_dependencies_to_self`, `implementors`) are
built once after import, exactly like ArchUnit's `ReverseDependencies`.

### 2.5 Full names and display

* Canonical `name()` is crate-rooted: `my_app::domain::order::Order`. This is unambiguous
  in a workspace import and matches `std::any::type_name`.
* The literal pattern segment `crate` matches any crate root, so `crate::domain..` works in
  every rule and in `archunit.toml` regardless of the crate name.
* Public re-export paths are recorded as aliases: `items.get("my_app::Order")` finds
  `my_app::domain::model::Order` when `lib.rs` has `pub use domain::model::Order`, and
  `ItemSelector` names match aliases too. Reports always print the canonical name.
* Failure lines therefore read
  `Item <my_app::domain::order::Order> depends on <my_app::infrastructure::db::Pool> in (src/domain/order.rs:14)`.
  **Open question for review:** the brief's example shows `<crate::domain::order::Order>`.
  If a literal `crate::` prefix is preferred for the crate under test, that is a one-line
  change in `formatters` (`[report] root_alias = "crate"`); the plan defaults to the real
  crate name because a workspace import would otherwise print identical names for items in
  different crates.
* Item descriptions use the generic `Item <..>` prefix from the brief. A configurable
  kind prefix (`Struct <..>`, `Trait <..>`, …) is available via `[report] item_prefix = "kind"`.

### 2.6 Rule evaluation and reporting

* `SimpleArchRule` mirrors Java: transform `RustItems` → `Vec<T>`, apply the aggregated
  predicate, fail on empty should (unless allowed), `condition.init(all)`, `check` each,
  `finish`, collect **every** violating event, then `EvaluationResult`.
* `FailureReport::to_string()` produces
  `Architecture Violation [Priority: MEDIUM] - Rule '<desc>' was violated (<n> times):\n<line>\n<line>`.
  `check` panics with that string. Lines are sorted the way ArchUnit sorts them (natural
  string order of the event description lines).
* `archunit_ignore_patterns.txt` filtering happens in `EvaluationResult` exactly as in Java.
* Golden tests compare full reports against files under `tests/expected/`.

## 3. Test-first workflow per phase

* Phase 1: `tests/importer.rs` and `tests/domain.rs` assert item counts, names, modifiers,
  members, resolved imports/re-exports/globs, accesses and dependencies for each fixture,
  including negative cases (unresolvable macro-generated names, ambiguous method calls).
* Phase 2: `tests/lang.rs` ports the shape of ArchUnit's `ClassesShouldTest`,
  `ArchRuleDefinitionTest`, `DescribedPredicateTest`, `ArchConditionsTest`, and
  `FailureReportTest` where a Rust counterpart exists, plus golden reports.
* Phase 3: `tests/library.rs` ports `LayeredArchitectureTest`, `OnionArchitectureTest`,
  `SlicesRuleDefinitionTest`, `CycleDetectorTest`, and the guide's descriptions verbatim.
* Phase 4: `tests/harness.rs` compiles a fixture test module through the macros (trybuild
  is **not** used; the macro output is exercised directly in the integration test crate).
* Phase 5: `tests/freeze.rs` (store creation/update/refreeze/line matcher),
  `tests/plantuml.rs` (parser grammar cases from `PlantUmlParserTest` and
  `PlantUmlArchConditionTest`), `tests/coding_rules.rs`.
* Phase 6: `examples/*.rs` compile and run under `cargo test --examples`, each expected to
  fail with a report matching `examples/expected/*.txt`.

## 4. Known limits of syn-based resolution

These are inherent to analysing source without type information. Each is documented in
rustdoc on the affected API and, where the limit affects a rule, the rule description says
"best effort".

1. **Macros are not expanded.** `macro_rules!` and procedural macros are parsed as
   invocations. Arguments of well-known std macros and of any invocation whose tokens parse
   as `Expr, Expr, ..` are visited, so `println!("{}", order.total())` still records the
   call to `total`. Items generated by macros (`derive` impls, `lazy_static!`,
   `bitflags!`, `tokio::main` rewrites) do not exist in the model; a `#[derive(Trait)]` is
   converted into an `implements` relation as the one deliberate exception.
2. **Method calls need the receiver type.** `x.foo()` resolves only via `self`, a local with
   an annotated or constructor-derived type, or a globally unique method name. Trait
   dispatch, generics, auto-deref chains, iterator adapters and closures' parameter types
   are not modelled. Unresolved calls still produce a `MethodCall` with a stub target so
   name-based rules (`call_method_where(name("unwrap"))`) work; owner-based rules may miss
   them. `[rust-only]` `NO_CLASSES_SHOULD_CALL_UNWRAP` is name-based on purpose.
3. **Field accesses** resolve with the same rules as method calls.
4. **Glob imports and re-exports** are resolved across all imported crates. A glob from a
   stub (non-imported) crate cannot be enumerated, so names it brings in become stubs
   attributed to that crate only when the glob is the sole candidate.
5. **`cfg` is not evaluated.** All branches are imported; `#[cfg(test)]` is special-cased.
   Mutually exclusive `cfg` items with the same name coexist as separate items with the same
   path (disambiguated by span).
6. **`include!`, `include_str!` and `build.rs`-generated code** are not followed unless the
   generated file is referenced through an ordinary `mod` under `OUT_DIR` (not supported;
   documented).
7. **Type aliases** are followed one level for `Result<T, E>` detection and for raw types;
   deeper alias chains resolve to the alias item, not its target.
8. **Trait method bodies and default items** are attributed to the trait; an impl that
   inherits a default method has it in `all_methods()` but not `methods()`.
9. **Closures are not code units.** Their bodies count toward the enclosing function.
10. **Constructors are a heuristic** (associated fn without `self`, returning `Self`-like).
11. **Visibility is syntactic.** `pub` in a private module is still `Pub`; reachability
    analysis is out of scope (ArchUnit is syntactic here too).
12. **Line numbers** point at the expression span start; for dependencies without a body
    (field types, impls) at the declaration line, like ArchUnit's `:0` for class-level
    dependencies but with the real line.

## 5. Phase checklist

| Phase | Deliverables | Commit |
|---|---|---|
| 0 | MAPPING.md, PLAN.md, CLAUDE.md, NOTICE | `docs: phase 0 research, API mapping and implementation plan` |
| 1 | workspace layout, `base`, `core::domain`, `core::importer`, fixtures, importer tests | `feat(core): crate importer and domain model` |
| 2 | `lang` (rules, conditions, syntax, reporting), golden reports, ignore patterns | `feat(lang): fluent rule API, conditions and failure reports` |
| 3 | `library::{Architectures, dependencies, cycle_detection}` | `feat(library): layered, onion and slice rules with cycle detection` |
| 4 | `archunit-macros`, `harness`, `config` (`archunit.toml`) | `feat(harness): arch_test macros, plain API and archunit.toml` |
| 5 | `library::{freeze, plantuml, GeneralCodingRules, DependencyRules, ProxyRules}` | `feat(library): freezing rules, PlantUML rules and coding rules` |
| 6 | `examples/`, README with Java/Rust side-by-side | `docs: port archunit-example rules and write README` |

Each phase ends with `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`,
a MAPPING.md status update, and a summary of what changed and what is open.
