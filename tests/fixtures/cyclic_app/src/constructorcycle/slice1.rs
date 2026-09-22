use crate::constructorcycle::slice2::SliceTwoWithConstructorParameterOfSliceOne;

pub struct SliceOneCallingConstructorInSliceTwo;

impl SliceOneCallingConstructorInSliceTwo {
    pub fn call_constructor(&self) -> SliceTwoWithConstructorParameterOfSliceOne {
        SliceTwoWithConstructorParameterOfSliceOne::new(self)
    }
}
