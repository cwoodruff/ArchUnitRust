pub struct RawBuffer {
    ptr: *const u8,
    len: usize,
}

unsafe impl Send for RawBuffer {}

pub unsafe trait Zeroable {}

impl RawBuffer {
    pub fn first(&self) -> u8 {
        unsafe { *self.ptr }
    }

    /// # Safety
    /// `index` must be in bounds.
    pub unsafe fn get_unchecked(&self, index: usize) -> u8 {
        unsafe { *self.ptr.add(index) }
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

pub struct SafeWrapper;

impl SafeWrapper {
    pub fn safe(&self) -> u32 {
        1
    }
}
