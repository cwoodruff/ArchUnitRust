//! Port of `DaoRulesTest`.
#[macro_use]
mod common;

use archunit::prelude::*;

fn daos_must_reside_in_a_dao_package() -> impl ArchRule {
    classes()
        .that()
        .have_name_matching(".*Dao")
        .should()
        .reside_in_a_package("..dao..")
        .as_("DAOs should reside in a package '..dao..'")
}

fn entities_must_reside_in_a_domain_package() -> impl ArchRule {
    classes()
        .that()
        .are_annotated_with("entity")
        .should()
        .reside_in_a_package("..domain..")
        .as_("Entities should reside in a package '..domain..'")
}

fn only_daos_may_use_the_entity_manager() -> impl ArchRule {
    no_classes()
        .that()
        .reside_outside_of_package("..dao..")
        .should()
        .access_classes_that()
        .are_assignable_to("layered_app::thirdparty::EntityManager")
        .as_("Only DAOs may use the EntityManager")
}

fn daos_must_not_throw_sql_error() -> impl ArchRule {
    no_methods()
        .that()
        .are_declared_in_classes_that()
        .have_name_matching(".*Dao")
        .should()
        .declare_throwable_of_type("layered_app::thirdparty::SqlError")
}

archunit_example! {
    example = "dao_rules",
    fixture = "layered_app",
    rules = [
        daos_must_reside_in_a_dao_package,
        entities_must_reside_in_a_domain_package,
        only_daos_may_use_the_entity_manager,
        daos_must_not_throw_sql_error,
    ],
}
