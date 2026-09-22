use crate::constructorcycle::slice1::SliceOneCallingConstructorInSliceTwo;

pub struct SliceTwoWithConstructorParameterOfSliceOne;

impl SliceTwoWithConstructorParameterOfSliceOne {
    pub fn new(_one: &SliceOneCallingConstructorInSliceTwo) -> Self {
        Self
    }
}
