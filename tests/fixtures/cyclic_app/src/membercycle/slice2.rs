use crate::membercycle::slice3::SliceThreeWithMethodParameterTypeOfSliceFour;

pub struct SliceTwoWithMethodReturnTypeOfSliceThree;

impl SliceTwoWithMethodReturnTypeOfSliceThree {
    pub fn three(&self) -> SliceThreeWithMethodParameterTypeOfSliceFour {
        SliceThreeWithMethodParameterTypeOfSliceFour
    }
}
