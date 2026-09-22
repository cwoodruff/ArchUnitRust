use crate::simplescenario::importer::ImportService;

pub struct AdministrationService {
    importer: Box<ImportService>,
}

impl AdministrationService {
    pub fn new(importer: Box<ImportService>) -> Self {
        Self { importer }
    }

    pub fn run_import(&self) {
        self.importer.process();
    }

    pub fn is_admin(&self, _name: &str) -> bool {
        true
    }
}
