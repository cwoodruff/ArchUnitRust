#[derive(Debug, Default, Clone)]
pub struct EntityManager;

#[derive(Debug)]
pub struct SqlError;

impl EntityManager {
    pub fn find_all<T>(&self) -> Vec<T> {
        Vec::new()
    }

    pub fn persist<T>(&self, _entity: T) -> Result<(), SqlError> {
        Ok(())
    }
}

pub fn free_function_in_thirdparty() -> EntityManager {
    EntityManager
}
