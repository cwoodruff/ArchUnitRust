use crate::shared::api::Money;

#[derive(Default)]
pub struct Warehouse {
    items: u64,
}

impl Warehouse {
    pub fn value(&self) -> Money {
        Money(self.items)
    }
}
