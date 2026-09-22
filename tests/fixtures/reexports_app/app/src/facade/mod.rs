#[path = "custom_path.rs"]
pub mod custom;

use crate::Order;
use crate::domain::model::{LineItem, OrderError};
use reexports_dep::Helper;
use reexports_dep::prelude::*;

pub use crate::domain::model::Order as FacadeOrder;

pub fn place(order: &mut Order) -> Result<u32, OrderError> {
    order.checked_add(LineItem(1))?;
    Helper::help();
    helper_fn();
    Ok(order.total())
}

pub unsafe fn danger(order: *const Order) -> u64 {
    unsafe { (*order).id }
}

pub async fn later(order: Order) -> u32 {
    order.total()
}

pub const fn limit() -> usize {
    crate::domain::model::MAX_ITEMS
}

pub fn with_local_item() -> u32 {
    struct LocalCounter(u32);
    let counter = LocalCounter(7);
    let order = Order::new(counter.0 as u64);
    let closure = |o: &Order| o.total();
    println!("{}", order.total());
    closure(&order)
}
