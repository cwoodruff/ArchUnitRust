use super::one::UseCaseOneTwoController;

pub struct UseCaseThreeController;

impl UseCaseThreeController {
    pub fn use_case_three(&self, other: &UseCaseOneTwoController) {
        other.use_case_one();
    }
}
