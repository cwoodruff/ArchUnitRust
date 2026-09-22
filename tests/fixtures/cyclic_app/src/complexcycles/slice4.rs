use crate::complexcycles::slice1::SliceOneCallingConstructorInSliceTwoAndMethodInSliceThree;

pub struct ClassWithAccessedFieldCallingMethodInSliceOne {
    pub accessed_field: u32,
}

impl ClassWithAccessedFieldCallingMethodInSliceOne {
    pub fn call_slice_one(&self) {
        SliceOneCallingConstructorInSliceTwoAndMethodInSliceThree.call();
    }
}
