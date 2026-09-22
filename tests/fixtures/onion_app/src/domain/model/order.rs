use fixture_macros::domain_model;

use super::product::Product;
use crate::domain::service::OrderRepository;

#[domain_model]
#[derive(Debug, Clone, Copy)]
pub struct OrderQuantity(pub u32);

#[domain_model]
#[derive(Debug, Clone)]
pub struct OrderItem {
    pub product: Product,
    pub quantity: OrderQuantity,
}

#[domain_model]
#[derive(Debug, Default)]
pub struct Order {
    pub items: Vec<OrderItem>,
}

impl Order {
    pub fn add(&mut self, item: OrderItem) {
        self.items.push(item);
    }

    /// Violation: the domain model must not depend on domain services.
    pub fn save_with(&self, repository: &dyn OrderRepository) {
        repository.save(self);
    }
}
