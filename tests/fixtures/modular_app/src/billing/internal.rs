use crate::orders::api::OrderService;

#[derive(Default)]
pub struct Ledger {
    balance: u64,
}

impl Ledger {
    pub fn balance(&self) -> u64 {
        self.balance
    }

    /// Violation: billing depends back on orders, which closes a cycle.
    pub fn settle(&mut self, service: &OrderService) {
        let _ = service;
        self.balance += 1;
    }
}
