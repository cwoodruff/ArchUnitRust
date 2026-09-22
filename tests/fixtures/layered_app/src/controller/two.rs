pub struct UseCaseTwoController;

impl UseCaseTwoController {
    pub fn new() -> Self {
        Self
    }

    pub fn use_case_two(&self) {}
}

impl Default for UseCaseTwoController {
    fn default() -> Self {
        Self::new()
    }
}
