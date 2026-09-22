pub struct Helper;

impl Helper {
    pub fn help() {}
}

pub fn helper_fn() {}

pub mod prelude {
    pub use crate::helper_fn;
    pub use crate::Helper;
}
