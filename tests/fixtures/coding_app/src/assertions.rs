pub struct Checks;

impl Checks {
    pub fn without_messages(&self, a: u32, b: u32) {
        assert!(a > 0);
        assert_eq!(a, b);
        assert_ne!(a, b + 1);
        debug_assert!(a < 100);
        debug_assert_eq!(a, a);
    }

    pub fn with_messages(&self, a: u32, b: u32) {
        assert!(a > 0, "a must be positive");
        assert_eq!(a, b, "a and b must match: {} != {}", a, b);
        assert_ne!(a, b + 1, "off by one");
        debug_assert!(a < 100, "too large");
        debug_assert_ne!(a, 0, "zero");
    }
}
