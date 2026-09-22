//! Port of `SecurityTest`: one isolated cross-cutting concern.
#[macro_use]
mod common;

use archunit::prelude::*;

fn only_security_infrastructure_should_use_the_security_guard() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..security..")
        .should()
        .only_be_accessed()
        .by_any_package(&["..security..", "..controller.."])
        .because("we want to have one isolated cross-cutting concern 'security'")
}

archunit_example! {
    example = "security",
    fixture = "layered_app",
    rules = [only_security_infrastructure_should_use_the_security_guard],
}
