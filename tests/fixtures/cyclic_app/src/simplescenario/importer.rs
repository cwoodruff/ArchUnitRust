use crate::simplescenario::report::ReportService;

pub struct ImportService {
    report: Box<ReportService>,
}

impl ImportService {
    pub fn process(&self) {
        self.report.report("imported");
    }
}
