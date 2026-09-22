use crate::domain::events::OrderPlaced;

pub fn describe(event: &OrderPlaced) -> String {
    format!("{}", event.order)
}
