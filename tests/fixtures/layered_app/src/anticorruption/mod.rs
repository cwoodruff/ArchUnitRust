pub mod internal;

use crate::anticorruption::internal::WrappedResult;

pub struct Facade;

impl Facade {
    pub fn wrapped(&self) -> WrappedResult<u32> {
        WrappedResult(42)
    }

    /// Violation: public method in `anticorruption` not returning `WrappedResult`.
    pub fn unwrapped(&self) -> u32 {
        42
    }

    fn private_helper(&self) -> u32 {
        self.unwrapped()
    }
}
