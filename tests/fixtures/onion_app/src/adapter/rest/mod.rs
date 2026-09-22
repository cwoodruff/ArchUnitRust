use fixture_macros::adapter;

use crate::domain::model::ProductId;
use crate::domain::service::ShoppingService;

#[adapter("rest")]
pub struct ShoppingController;

impl ShoppingController {
    pub fn post_order(&self, service: &ShoppingService<'_>, product: u64) {
        service.order(ProductId(product), 1);
    }

    pub fn refresh(&self) {}
}
