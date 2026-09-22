pub mod domain;
pub mod jpa;

use self::domain::PersistentObject;
use crate::thirdparty::{EntityManager, SqlError};

#[derive(Default)]
pub struct SomeDao {
    entity_manager: EntityManager,
}

impl SomeDao {
    pub fn find_all(&self) -> Vec<PersistentObject> {
        self.entity_manager.find_all()
    }

    pub fn store(&self, object: PersistentObject) -> Result<(), SqlError> {
        self.entity_manager.persist(object)
    }
}
