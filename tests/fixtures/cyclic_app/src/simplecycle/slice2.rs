use crate::simplecycle::slice3::SliceThreeCallingMethodOfSliceOne;

pub struct SliceTwoCallingMethodOfSliceThree;

impl SliceTwoCallingMethodOfSliceThree {
    pub fn call_slice_three(&self) {
        SliceThreeCallingMethodOfSliceOne.call_slice_one();
    }
}
