use crate::shared::api::Money;

#[derive(Debug, Clone)]
pub struct Invoice {
    pub order_id: u64,
    pub amount: Money,
}

impl Invoice {
    pub fn for_order(order_id: u64, amount: Money) -> Invoice {
        Invoice { order_id, amount }
    }
}
