//! Port of `MethodsTest`.
#[macro_use]
mod common;

use archunit::prelude::*;

fn all_public_methods_in_the_anticorruption_layer_should_return_wrapped_results() -> impl ArchRule {
    methods()
        .that()
        .are_declared_in_classes_that()
        .reside_in_a_package("..anticorruption..")
        .and()
        .are_public()
        .should()
        .have_raw_return_type("layered_app::anticorruption::internal::WrappedResult")
        .because("we do not want to couple the client code directly to the return types of the encapsulated module")
}

fn code_units_in_dao_layer_should_not_be_secured() -> impl ArchRule {
    no_code_units()
        .that()
        .are_declared_in_classes_that()
        .reside_in_a_package("..persistence..")
        .should()
        .be_annotated_with("secured")
}

archunit_example! {
    example = "methods",
    fixture = "layered_app",
    rules = [
        all_public_methods_in_the_anticorruption_layer_should_return_wrapped_results,
        code_units_in_dao_layer_should_not_be_secured,
    ],
}
