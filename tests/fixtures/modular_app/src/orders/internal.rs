use super::api::Order;
use crate::util::helper;

#[derive(Default)]
pub struct OrderRepository {
    orders: Vec<Order>,
}

impl OrderRepository {
    pub fn save(&mut self, order: Order) {
        let _ = helper();
        self.orders.push(order);
    }
}
