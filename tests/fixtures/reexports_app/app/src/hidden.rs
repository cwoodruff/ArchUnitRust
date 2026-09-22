use crate::domain::model::Order;

pub(crate) fn hidden_helper() -> u32 {
    Order::new(1).total()
}

pub(super) struct Hidden {
    pub(super) value: u32,
}

#[deprecated(note = "use hidden_helper")]
pub fn old() -> u32 {
    0
}

#[allow(deprecated)]
pub fn uses_old() -> u32 {
    old()
}

#[cfg(test)]
mod tests {
    use crate::domain::model::Order;

    #[test]
    fn total_starts_at_zero() {
        assert_eq!(Order::new(1).total(), 0);
    }
}
