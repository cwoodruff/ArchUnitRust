//! Port of `ControllerRulesTest`: controllers may only call code that is either declared in
//! the controller layer (or `std`) or marked `#[secured]`.
#[macro_use]
mod common;

use archunit::base::DescribedPredicate;
use archunit::core::domain::properties::HasOwner;
use archunit::core::domain::properties::can_be_annotated::predicates::annotated_with;
use archunit::core::domain::rust_item::functions::get_package_name;
use archunit::core::domain::rust_member::predicates::declared_in;
use archunit::core::domain::{PackageMatchers, RustItem};
use archunit::lang::conditions::predicates::are;
use archunit::prelude::*;

fn are_declared_in_controller<T>() -> DescribedPredicate<T>
where
    T: HasOwner<RustItem> + 'static,
{
    let a_package_controller = get_package_name()
        .is(PackageMatchers::of(&["..controller..", "std..", "core.."]))
        .as_("a package '..controller..'");
    are(declared_in::<T>(a_package_controller))
}

fn controllers_should_only_call_secured_methods() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..controller..")
        .should()
        .only_call_methods_that(are_declared_in_controller().or(are(annotated_with("secured"))))
}

fn controllers_should_only_call_secured_constructors() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..controller..")
        .should()
        .only_call_constructors_that(
            are_declared_in_controller().or(are(annotated_with("secured"))),
        )
}

fn controllers_should_only_call_secured_code_units() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..controller..")
        .should()
        .only_call_code_units_that(are_declared_in_controller().or(are(annotated_with("secured"))))
}

fn controllers_should_only_access_secured_fields() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..controller..")
        .should()
        .only_access_fields_that(are_declared_in_controller().or(are(annotated_with("secured"))))
}

fn controllers_should_only_access_secured_members() -> impl ArchRule {
    classes()
        .that()
        .reside_in_a_package("..controller..")
        .should()
        .only_access_members_that(are_declared_in_controller().or(are(annotated_with("secured"))))
}

archunit_example! {
    example = "controller_rules",
    fixture = "layered_app",
    rules = [
        controllers_should_only_call_secured_methods,
        controllers_should_only_call_secured_constructors,
        controllers_should_only_call_secured_code_units,
        controllers_should_only_access_secured_fields,
        controllers_should_only_access_secured_members,
    ],
}
