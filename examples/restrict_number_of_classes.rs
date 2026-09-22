//! Port of `RestrictNumberOfClassesWithACertainPropertyTest`.
#[macro_use]
mod common;

use archunit::base::less_than_or_equal_to;
use archunit::prelude::*;

fn no_new_classes_should_implement_service_interface() -> impl ArchRule {
    classes()
        .that()
        .implement("layered_app::service::ServiceInterface")
        .should()
        .contain_number_of_elements(less_than_or_equal_to(1))
        .because("from now on new classes should implement layered_app::service::internal::SomeInternalInterface")
}

archunit_example! {
    example = "restrict_number_of_classes",
    fixture = "layered_app",
    rules = [no_new_classes_should_implement_service_interface],
}
