use super::*;
use super::model::Status;

pub struct OrderPlaced {
    pub order: DomainOrder,
    pub status: Status,
}

pub trait Event: std::fmt::Debug + Send {
    fn name(&self) -> &'static str;
}
