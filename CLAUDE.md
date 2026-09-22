# CLAUDE.md

Rust port of [ArchUnit](https://github.com/TNG/ArchUnit) (Java, Apache 2.0). The goal is
API-shape, rule-semantics and failure-output parity: someone who knows ArchUnit should
recognise every part of this crate, and the ArchUnit user guide should work as
documentation after a name translation.

## Build and test

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --examples          # the ported archunit-example rules (UPDATE_EXPECTED=1 regenerates examples/expected)
```

All three must pass before every commit. Stable Rust only, edition 2024.

Fixture crates under `tests/fixtures/` are excluded from the workspace and are never
compiled by `cargo test`; the importer parses them. If you add a fixture, give its
`Cargo.toml` an empty `[workspace]` table and run `cargo check` inside it once. Fixtures:
`layered_app`, `onion_app`, `cyclic_app`, `reexports_app`, `coding_app`, `modular_app`,
`fixture_macros` (the no-op attribute macros), plus `plantuml/` and `config/` data.

Integration tests live in `tests/`, one file per area (`base`, `importer`, `domain`, `lang`,
`library`, `modular_monolith`, `harness`, `config`, `coding_rules`, `plantuml`, `freeze`).
Golden reports are compared verbatim; `UPDATE_EXPECTED=1 cargo test` rewrites the ones under
`tests/expected/` that the tests generate, and `UPDATE_EXPECTED=1 cargo test --examples` the
ones under `examples/expected/`.

## Layout

See `docs/PLAN.md` §1 for the full tree. Short form:

* `src/base` – `DescribedPredicate`, `DescribedFunction`, errors.
* `src/core/domain` – `RustItems`, `RustItem`, `RustModule`, members, `RustAccess`,
  `Dependency`, `PackageMatcher`, formatters.
* `src/core/importer` – `CrateImporter` (cargo_metadata + syn), `ImportOption`, name
  resolution, dependency collection.
* `src/lang` – `ArchRule`, `ArchCondition`, `ConditionEvents`, `EvaluationResult`,
  `conditions` (ArchConditions/ArchPredicates), `syntax` (the fluent DSL).
* `src/library` – `Architectures`, slices + cycle detection, coding rules, PlantUML, freeze.
* `src/harness` – test integration and import cache; `archunit-macros/` holds the proc macros.
* `src/config.rs` – `ArchConfiguration`, `archunit.toml`, `ARCHUNIT_*` env overrides,
  `archunit_ignore_patterns.txt`.
* `examples/` – one program per ported `archunit-example` test class; `examples/common/mod.rs`
  holds the `archunit_example!` macro (a `main()` plus one test per rule), expected reports
  live in `examples/expected/<example>/<rule>.txt`, the frozen store in `examples/frozen/`.

## Conventions (do not deviate without updating docs/MAPPING.md)

* Keep ArchUnit names, converted to Rust case: `resideInAPackage` → `reside_in_a_package`,
  `ArchRuleDefinition.classes()` → `ArchRuleDefinition::classes()` and free fn `classes()`.
  Never rename a concept because a Rust name reads better.
* `Java*` domain types become `Rust*` (`JavaClass` → `RustItem`, `JavaClasses` → `RustItems`,
  `JavaPackage` → `RustModule`). Package identifiers become module identifiers with `::`.
* Java overloads: a method that exists both as zero-arg fluent (`that()`, `should()`,
  `depend_on_classes_that()`) and with a predicate/condition argument gets the suffix `_with`
  for the object form (`that_with(pred)`, `should_with(cond)`). Overloads over
  `String | Class<?> | DescribedPredicate` collapse into one method taking
  `impl Into<ItemSelector>`. Keyword clashes get a trailing underscore (`as_`, `in_`).
* Rust-only additions are allowed, named in ArchUnit style, and marked `[rust-only]` in
  MAPPING.md. Nothing from ArchUnit is dropped silently: mark it `unsupported` with a reason.
* Rule descriptions and failure reports must match ArchUnit's text format byte for byte
  (see MAPPING.md §4.9 and PLAN.md §2.6). Collect every violation before failing.
* Do not copy Java code verbatim; port concepts and API. Keep the NOTICE file.
* Tests first, against the fixtures. Port the matching ArchUnit test cases where a Rust
  counterpart exists. Golden failure reports live in `tests/expected/`.
* One commit per phase, conventional commit messages. Update the status column and the
  phase log in `docs/MAPPING.md` at the end of every phase.

## Reference material

* User guide: https://www.archunit.org/userguide/html/000_Index.html
* Source: https://github.com/TNG/ArchUnit (`archunit`, `archunit-junit`, `archunit-example`).
  A shallow clone in the session scratchpad is the fastest way to check an exact description
  string or test case: `git clone --depth 1 https://github.com/TNG/ArchUnit.git`.
