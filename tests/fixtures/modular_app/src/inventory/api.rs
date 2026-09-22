use crate::shared::api::Money;

use super::internal::Warehouse;

pub struct Stock {
    warehouse: Warehouse,
}

impl Stock {
    pub fn value(&self) -> Money {
        self.warehouse.value()
    }
}
