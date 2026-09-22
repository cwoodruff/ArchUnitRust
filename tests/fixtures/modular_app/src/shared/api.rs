#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Money(pub u64);

impl Money {
    pub fn plus(self, other: Money) -> Money {
        Money(self.0 + other.0)
    }
}
