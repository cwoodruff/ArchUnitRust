use fixture_macros::adapter;

use crate::adapter::rest::ShoppingController;
use crate::domain::model::ProductId;
use crate::domain::service::ShoppingService;

#[adapter("cli")]
pub struct AdministrationCli;

impl AdministrationCli {
    pub fn run(&self, service: &ShoppingService<'_>) {
        service.order(ProductId(1), 2);
    }

    /// Violation: one adapter must not depend on another adapter.
    pub fn refresh_ui(&self, controller: &ShoppingController) {
        controller.refresh();
    }
}
