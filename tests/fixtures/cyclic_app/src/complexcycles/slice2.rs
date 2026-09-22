use crate::complexcycles::slice1::{ClassOfMinimalCycleCallingSliceTwo, TraitBeingImplementedInSliceTwo};
use crate::complexcycles::slice4::ClassWithAccessedFieldCallingMethodInSliceOne;

pub struct InstantiatedClassInSliceTwo;

impl InstantiatedClassInSliceTwo {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InstantiatedClassInSliceTwo {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SliceTwoInheritingFromSliceOne;

impl TraitBeingImplementedInSliceTwo for SliceTwoInheritingFromSliceOne {}

impl SliceTwoInheritingFromSliceOne {
    pub fn method(&self) {
        let _ = ClassOfMinimalCycleCallingSliceTwo;
    }

    pub fn access_field(&self, four: &ClassWithAccessedFieldCallingMethodInSliceOne) -> u32 {
        four.accessed_field
    }
}
