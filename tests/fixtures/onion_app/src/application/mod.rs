use fixture_macros::application;

use crate::adapter::cli::AdministrationCli;
use crate::domain::service::{OrderRepository, ProductRepository, ShoppingService};

#[application]
pub struct ApplicationConfiguration;

impl ApplicationConfiguration {
    pub fn shopping_service<'a>(
        &self,
        products: &'a dyn ProductRepository,
        orders: &'a dyn OrderRepository,
    ) -> ShoppingService<'a> {
        ShoppingService::new(products, orders)
    }

    /// Violation: application services must not depend on adapters.
    pub fn start_cli(&self) -> AdministrationCli {
        AdministrationCli
    }
}
