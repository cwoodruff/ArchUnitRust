use std::sync::atomic::AtomicU64;

pub type OrderId = u64;
pub const MAX_ITEMS: usize = 3;
pub static ORDER_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq)]
pub struct LineItem(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Open,
    Closed { at: u64 },
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: OrderId,
    items: Vec<LineItem>,
    pub(crate) status: Status,
}

impl Order {
    pub fn new(id: OrderId) -> Self {
        Self {
            id,
            items: Vec::new(),
            status: Status::Open,
        }
    }

    pub fn try_new(id: OrderId) -> Result<Self, OrderError> {
        if id == 0 {
            Err(OrderError::InvalidId)
        } else {
            Ok(Self::new(id))
        }
    }

    pub fn add(&mut self, item: LineItem) {
        self.items.push(item);
    }

    pub fn total(&self) -> u32 {
        self.items.iter().map(|item| item.0).sum()
    }

    pub fn close(&mut self, at: u64) {
        self.status = Status::Closed { at };
    }

    fn is_open(&self) -> bool {
        self.status == Status::Open
    }

    pub fn checked_add(&mut self, item: LineItem) -> Result<(), OrderError> {
        if !self.is_open() {
            return Err(OrderError::Closed);
        }
        self.add(item);
        Ok(())
    }
}

#[derive(Debug)]
pub enum OrderError {
    InvalidId,
    Closed,
}

impl std::fmt::Display for Order {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "order {} ({} items)", self.id, self.items.len())
    }
}

impl From<Order> for String {
    fn from(order: Order) -> Self {
        order.to_string()
    }
}
