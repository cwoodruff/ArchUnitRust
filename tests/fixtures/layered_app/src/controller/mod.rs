pub mod marshaller;
pub mod one;
pub mod three;
pub mod two;

use crate::core::{CoreSatellite, VeryCentralCore};
use crate::persistence::first::dao::SomeDao;
use crate::service::ServiceOne;
use fixture_macros::my_controller;

pub trait AbstractController {
    fn handle(&self);
}

#[my_controller]
pub struct SomeController {
    service: ServiceOne,
}

impl SomeController {
    pub fn new() -> Self {
        Self {
            service: ServiceOne::new(),
        }
    }

    pub fn do_something(&self) {
        self.service.serve();
    }

    pub fn bypass_layers(&self) {
        let dao = SomeDao::default();
        dao.find_all();
    }
}

impl Default for SomeController {
    fn default() -> Self {
        Self::new()
    }
}

/// Naming violation: contains "Gui".
pub struct SomeGuiController;

impl AbstractController for SomeGuiController {
    fn handle(&self) {}
}

/// Naming violation: implements AbstractController but is not suffixed with Controller.
pub struct WronglyNamed;

impl AbstractController for WronglyNamed {
    fn handle(&self) {}
}

pub struct CoreSatelliteController;

impl CoreSatellite for CoreSatelliteController {}

impl CoreSatelliteController {
    pub fn call_core(&self) {
        VeryCentralCore.do_core_stuff();
    }
}
