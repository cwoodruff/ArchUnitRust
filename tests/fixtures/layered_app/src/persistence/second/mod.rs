use crate::thirdparty::EntityManager;
use fixture_macros::entity;

/// Violation: named like a DAO but not in a `dao` package.
pub struct InWrongPackageDao {
    manager: EntityManager,
}

impl InWrongPackageDao {
    pub fn manager(&self) -> &EntityManager {
        &self.manager
    }
}

/// Violation: an entity outside of a `domain` package.
#[entity]
pub struct WronglyPlacedEntity;
