pub struct SecurityGuard;

impl SecurityGuard {
    pub fn check(&self, _principal: &str) -> bool {
        true
    }
}
