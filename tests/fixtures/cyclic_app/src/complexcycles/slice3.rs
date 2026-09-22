use crate::complexcycles::slice4::ClassWithAccessedFieldCallingMethodInSliceOne;
use crate::complexcycles::slice5::ClassCallingConstructorInSliceFive;

pub struct ClassCallingMethodInSliceThree;

impl ClassCallingMethodInSliceThree {
    pub fn call_slice_three(&self) {}

    pub fn call_slice_four_and_five(&self) {
        let four = ClassWithAccessedFieldCallingMethodInSliceOne { accessed_field: 1 };
        let _ = four.accessed_field;
        ClassCallingConstructorInSliceFive::new();
    }
}
