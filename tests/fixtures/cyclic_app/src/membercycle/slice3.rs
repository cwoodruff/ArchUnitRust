use crate::membercycle::slice4::SliceFourWithErrorTypeOfSliceOne;

pub struct SliceThreeWithMethodParameterTypeOfSliceFour;

impl SliceThreeWithMethodParameterTypeOfSliceFour {
    pub fn four(&self, _four: SliceFourWithErrorTypeOfSliceOne) {}
}
