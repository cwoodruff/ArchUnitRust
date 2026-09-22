use crate::fieldaccesscycle::slice2::SliceTwoAccessingFieldInSliceOne;

pub struct SliceOneAccessingFieldInSliceTwo {
    pub field_one: u32,
}

impl SliceOneAccessingFieldInSliceTwo {
    pub fn access(&self, two: &mut SliceTwoAccessingFieldInSliceOne) {
        two.field_two = self.field_one;
    }
}
