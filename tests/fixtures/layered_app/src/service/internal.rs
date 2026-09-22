use super::ServiceInterface;

/// Violation: an interface placed in an implementation package.
pub trait SomeInternalInterface {
    fn internal(&self);
}

pub struct ServiceImpl;

impl ServiceInterface for ServiceImpl {
    fn serve(&self) {}
}
