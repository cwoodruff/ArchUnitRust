use fixture_macros::high_security;

pub trait CoreSatellite {}

#[high_security]
pub struct VeryCentralCore;

impl VeryCentralCore {
    pub fn do_core_stuff(&self) {}
}
