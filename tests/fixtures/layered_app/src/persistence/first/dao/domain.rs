use fixture_macros::{entity, Entity};

#[entity]
#[derive(Debug, Clone, Entity)]
pub struct PersistentObject {
    pub id: u64,
}
