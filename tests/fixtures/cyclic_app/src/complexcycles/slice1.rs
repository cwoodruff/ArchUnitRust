use crate::complexcycles::slice2::{InstantiatedClassInSliceTwo, SliceTwoInheritingFromSliceOne};
use crate::complexcycles::slice3::ClassCallingMethodInSliceThree;

pub struct SliceOneCallingConstructorInSliceTwoAndMethodInSliceThree;

impl SliceOneCallingConstructorInSliceTwoAndMethodInSliceThree {
    pub fn call(&self) {
        let _two = InstantiatedClassInSliceTwo::new();
        ClassCallingMethodInSliceThree.call_slice_three();
    }
}

pub trait TraitBeingImplementedInSliceTwo {}

pub struct ClassOfMinimalCycleCallingSliceTwo;

impl ClassOfMinimalCycleCallingSliceTwo {
    pub fn call(&self, two: &SliceTwoInheritingFromSliceOne) {
        two.method();
    }
}
