use fixture_macros::secured;

pub struct Marshaller;

impl Marshaller {
    #[secured]
    pub fn marshal(&self, input: &str) -> String {
        input.to_uppercase()
    }
}
