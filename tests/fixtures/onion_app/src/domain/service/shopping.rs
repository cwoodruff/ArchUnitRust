use fixture_macros::domain_service;

use crate::adapter::rest::ShoppingController;
use crate::domain::model::{Order, OrderItem, OrderQuantity, Product, ProductId};

#[domain_service]
pub trait ProductRepository {
    fn find(&self, id: ProductId) -> Option<Product>;
}

#[domain_service]
pub trait OrderRepository {
    fn save(&self, order: &Order);
}

#[domain_service]
pub struct ShoppingService<'a> {
    products: &'a dyn ProductRepository,
    orders: &'a dyn OrderRepository,
}

impl<'a> ShoppingService<'a> {
    pub fn new(products: &'a dyn ProductRepository, orders: &'a dyn OrderRepository) -> Self {
        Self { products, orders }
    }

    pub fn order(&self, id: ProductId, quantity: u32) {
        if let Some(product) = self.products.find(id) {
            let mut order = Order::default();
            order.add(OrderItem {
                product,
                quantity: OrderQuantity(quantity),
            });
            self.orders.save(&order);
        }
    }

    /// Violation: the domain must not know about a driving adapter.
    pub fn notify(&self, controller: &ShoppingController) {
        controller.refresh();
    }
}
