use crate::service::{ServiceInterface, ServiceOne};

/// Violation: persistence depends on service.
pub struct DaoCallingService {
    pub service: ServiceOne,
}

impl ServiceInterface for DaoCallingService {
    fn serve(&self) {
        self.service.serve();
    }
}
