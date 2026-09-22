use crate::simplescenario::administration::AdministrationService;

pub struct ReportService {
    admin: Box<AdministrationService>,
}

impl ReportService {
    pub fn report(&self, text: &str) {
        if self.admin.is_admin(text) {
            self.admin.run_import();
        }
    }
}
