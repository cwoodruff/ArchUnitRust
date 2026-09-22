//! A modular monolith: business modules with a public `api` submodule each, a shared
//! kernel, no cycles and no reaching into another module's internals
//! (`Architectures::modular_monolith()`, a rust-only addition; see docs/MAPPING.md §5.1).
#[macro_use]
mod common;

use archunit::prelude::*;

fn modules_respect_their_boundaries() -> impl ArchRule {
    modular_monolith()
        .module("orders")
        .defined_by(&["..orders.."])
        .module("billing")
        .defined_by(&["..billing.."])
        .module("inventory")
        .defined_by(&["..inventory.."])
        .module("shared")
        .defined_by(&["..shared.."])
        .where_module("shared")
        .may_not_depend_on_any_module()
        .where_module("billing")
        .may_only_depend_on_modules(&["shared"])
        .where_module("inventory")
        .may_only_depend_on_modules(&["shared"])
        .modules_may_only_depend_on_each_other_through_packages(&["..api.."])
        .modules_should_be_free_of_cycles()
}

fn modules_derived_from_the_package_layout_are_free_of_cycles() -> impl ArchRule {
    modular_monolith()
        .modules_defined_by_packages("..modular_app.(*)..")
        .naming_modules("$1")
        .modules_should_be_free_of_cycles()
}

archunit_example! {
    example = "modular_monolith",
    fixture = "modular_app",
    rules = [
        modules_respect_their_boundaries,
        modules_derived_from_the_package_layout_are_free_of_cycles,
    ],
}
