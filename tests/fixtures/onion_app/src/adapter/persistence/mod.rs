use fixture_macros::adapter;

use crate::domain::model::{Order, Product, ProductId, ProductName};
use crate::domain::service::{OrderRepository, ProductRepository};

#[adapter("persistence")]
#[derive(Default)]
pub struct InMemoryProductRepository;

impl ProductRepository for InMemoryProductRepository {
    fn find(&self, id: ProductId) -> Option<Product> {
        Some(Product {
            id,
            name: ProductName(String::from("thing")),
        })
    }
}

#[adapter("persistence")]
#[derive(Default)]
pub struct InMemoryOrderRepository;

impl OrderRepository for InMemoryOrderRepository {
    fn save(&self, _order: &Order) {}
}
