//! Port of `SlicesIsolationTest`: the use-case slices of the controller layer must not depend
//! on each other.
#[macro_use]
mod common;

use archunit::base::{DescribedPredicate, always_true};
use archunit::core::domain::RustItem;
use archunit::core::domain::properties::has_name::predicates::name_matching;
use archunit::library::dependencies::Slice;
use archunit::prelude::*;

fn controllers_should_only_use_their_own_slice() -> impl ArchRule {
    slices()
        .matching("..controller.(*)..")
        .naming_slices("Controller $1")
        .as_("Controllers")
        .should()
        .not_depend_on_each_other()
}

fn specific_controllers_should_only_use_their_own_slice() -> impl ArchRule {
    slices()
        .matching("..controller.(*)..")
        .naming_slices("Controller $1")
        .that(contain_description("Controller one"))
        .or(contain_description("Controller two"))
        .as_("Controllers one and two")
        .should()
        .not_depend_on_each_other()
}

fn controllers_should_only_use_their_own_slice_with_custom_ignore() -> impl ArchRule {
    slices()
        .matching("..controller.(*)..")
        .naming_slices("Controller $1")
        .as_("Controllers")
        .should()
        .not_depend_on_each_other()
        .ignore_dependency(
            "layered_app::controller::one::UseCaseOneTwoController",
            "layered_app::controller::two::UseCaseTwoController",
        )
        .ignore_dependency(
            name_matching::<RustItem>(".*controller::three.*"),
            always_true::<RustItem>(),
        )
}

fn contain_description(description_part: &str) -> DescribedPredicate<Slice> {
    let part = description_part.to_owned();
    DescribedPredicate::describe(
        format!("contain description '{description_part}'"),
        move |slice: &Slice| slice.description().contains(&part),
    )
}

archunit_example! {
    example = "slices_isolation",
    fixture = "layered_app",
    rules = [
        controllers_should_only_use_their_own_slice,
        specific_controllers_should_only_use_their_own_slice,
        controllers_should_only_use_their_own_slice_with_custom_ignore,
    ],
}
