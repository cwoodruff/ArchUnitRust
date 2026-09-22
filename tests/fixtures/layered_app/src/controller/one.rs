use crate::controller::two::UseCaseTwoController;

pub struct UseCaseOneTwoController;

impl UseCaseOneTwoController {
    pub fn use_case_one(&self) {
        UseCaseTwoController::new().use_case_two();
    }
}
