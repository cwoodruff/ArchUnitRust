use crate::membercycle::slice1::SliceOneWithFieldTypeInSliceTwo;

pub struct SliceFourWithErrorTypeOfSliceOne;

impl SliceFourWithErrorTypeOfSliceOne {
    pub fn fails(&self) -> Result<(), SliceOneWithFieldTypeInSliceTwo> {
        Ok(())
    }
}
