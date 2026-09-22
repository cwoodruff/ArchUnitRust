use crate::simplecycle::slice2::SliceTwoCallingMethodOfSliceThree;

pub struct SliceOneCallingMethodInSliceTwo;

impl SliceOneCallingMethodInSliceTwo {
    pub fn call_slice_two(&self, other: &SliceTwoCallingMethodOfSliceThree) {
        other.call_slice_three();
    }
}
