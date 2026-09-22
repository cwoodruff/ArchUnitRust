use crate::billing::api::Invoice;
use crate::billing::internal::Ledger;
use crate::shared::api::Money;

use super::internal::OrderRepository;

#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub total: Money,
}

pub struct OrderService {
    repository: OrderRepository,
}

impl OrderService {
    pub fn place(&mut self, order: Order) -> Invoice {
        self.repository.save(order.clone());
        Invoice::for_order(order.id, order.total)
    }

    /// Violation: orders must talk to billing through `billing::api` only.
    pub fn peek_ledger(&self, ledger: &Ledger) -> u64 {
        ledger.balance()
    }
}
