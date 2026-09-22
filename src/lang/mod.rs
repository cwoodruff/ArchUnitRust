//! The Lang API: rules, conditions, events and reports (`com.tngtech.archunit.lang`).
//!
//! ```no_run
//! use archunit::prelude::*;
//! use archunit::lang::syntax::no_classes;
//!
//! let items = CrateImporter::new().import_workspace();
//! no_classes()
//!     .that().reside_in_a_package("..service..")
//!     .should().access_classes_that().reside_in_a_package("..controller..")
//!     .check(&items);
//! ```

mod condition;
pub mod conditions;
mod evaluation;
mod events;
mod rule;
pub mod syntax;

pub use condition::{
    ArchCondition, ConditionByPredicate, ConditionLogic, ConditionTarget, EventDescriber,
};
pub use evaluation::{
    DefaultFailureDisplayFormat, EvaluationResult, FailureDisplayFormat, FailureMessages,
    FailureReport, ViolationHandler,
};
pub use events::{
    AsCorrespondingObject, ConditionEvent, ConditionEvents, CorrespondingObject,
    FromCorrespondingObject, SimpleConditionEvent,
};
pub use rule::{
    ArchRule, ClassesTransformer, CompositeArchRule, Priority, SimpleArchRule, assertions,
};
