use crate::fieldaccesscycle::slice1::SliceOneAccessingFieldInSliceTwo;

pub struct SliceTwoAccessingFieldInSliceOne {
    pub field_two: u32,
}

impl SliceTwoAccessingFieldInSliceOne {
    pub fn access(&self, one: &SliceOneAccessingFieldInSliceTwo) -> u32 {
        one.field_one
    }
}
