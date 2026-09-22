use crate::complexcycles::slice6::ClassInSliceSix;

pub struct ClassCallingConstructorInSliceFive;

impl ClassCallingConstructorInSliceFive {
    pub fn new() -> Self {
        let _ = ClassInSliceSix::default();
        Self
    }
}

impl Default for ClassCallingConstructorInSliceFive {
    fn default() -> Self {
        Self::new()
    }
}
