pub mod internal;

use crate::controller::{AbstractController, SomeGuiController};
use crate::core::VeryCentralCore;
use crate::persistence::first::dao::SomeDao;
use fixture_macros::{my_service, secured};

pub trait ServiceInterface {
    fn serve(&self);
}

#[my_service]
pub struct ServiceOne {
    dao: SomeDao,
}

impl ServiceOne {
    pub fn new() -> Self {
        Self {
            dao: SomeDao::default(),
        }
    }

    #[secured]
    pub fn serve(&self) {
        self.dao.find_all();
    }
}

impl Default for ServiceOne {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceInterface for ServiceOne {
    fn serve(&self) {
        ServiceOne::serve(self);
    }
}

pub struct ServiceHelper;

pub struct SpecialServiceHelper {
    pub helper: ServiceHelper,
}

/// Naming violation: annotated with `my_service` but not prefixed with `Service`.
#[my_service]
pub struct BadlyNamedService;

pub struct ServiceViolatingLayerRules {
    pub counter: u32,
}

impl ServiceViolatingLayerRules {
    pub fn illegal_access_to_controller(&self) {
        let controller = SomeGuiController;
        controller.handle();
    }

    pub fn illegal_access_to_core(&mut self) {
        VeryCentralCore.do_core_stuff();
        self.counter = 1;
    }
}
