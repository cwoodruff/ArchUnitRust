use crate::simplecycle::slice1::SliceOneCallingMethodInSliceTwo;

pub struct SliceThreeCallingMethodOfSliceOne;

impl SliceThreeCallingMethodOfSliceOne {
    pub fn call_slice_one(&self) {
        let one = SliceOneCallingMethodInSliceTwo;
        one.call_slice_two(&super::slice2::SliceTwoCallingMethodOfSliceThree);
    }
}
