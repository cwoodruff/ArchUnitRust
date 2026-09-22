//! The fluent rule syntax (`com.tngtech.archunit.lang.syntax`).
//!
//! Entry points mirror `ArchRuleDefinition`: [`classes`], [`no_classes`], [`the_class`],
//! [`members`], [`fields`], [`code_units`], [`methods`], [`constructors`] and their `no_`
//! counterparts, plus [`all`] / [`no`] for custom transformers and [`priority`].
//!
//! ```no_run
//! use archunit::prelude::*;
//!
//! let items = CrateImporter::new().import_workspace();
//! classes()
//!     .that().reside_in_a_package("..service..")
//!     .should().only_be_accessed().by_any_package(&["..controller..", "..service.."])
//!     .check(&items);
//! ```

mod aggregators;
mod classes;
mod members;
mod objects;

pub use classes::{
    ClassesShould, ClassesShouldConjunction, ClassesThat, GivenClass, GivenClasses,
    GivenClassesConjunction, OnlyBeAccessedSpecification,
};
pub use members::{
    CodeUnitsShould, CodeUnitsShouldConjunction, CodeUnitsThat, ConstructorsShould,
    ConstructorsShouldConjunction, ConstructorsThat, FieldsShould, FieldsShouldConjunction,
    FieldsThat, GivenCodeUnits, GivenCodeUnitsConjunction, GivenConstructors,
    GivenConstructorsConjunction, GivenFields, GivenFieldsConjunction, GivenMembers,
    GivenMembersConjunction, GivenMethods, GivenMethodsConjunction, MembersShould,
    MembersShouldConjunction, MembersThat, MethodsShould, MethodsShouldConjunction, MethodsThat,
    OnlyBeCalledSpecification,
};
pub use objects::{GivenConjunction, GivenObjects};

use crate::core::domain::{
    MemberLike, RustCodeUnit, RustConstructor, RustField, RustItem, RustItems, RustMember,
    RustMethod,
};
use crate::lang::events::AsCorrespondingObject;
use crate::lang::rule::{ClassesTransformer, Priority};

/// The entry point of the rule syntax (`ArchRuleDefinition`); every associated function is
/// also available as a free function in this module.
#[derive(Debug, Clone, Copy)]
pub struct ArchRuleDefinition;

/// `ArchRuleDefinition.Creator`: entry points with a chosen [`Priority`].
#[derive(Debug, Clone, Copy)]
pub struct Creator {
    priority: Priority,
}

fn classes_transformer() -> ClassesTransformer<RustItem> {
    ClassesTransformer::new("classes", |items: &RustItems| items.to_vec())
}

fn the_class_transformer(class_name: &str) -> ClassesTransformer<RustItem> {
    let name = class_name.to_owned();
    ClassesTransformer::new(
        format!("the class {class_name}"),
        move |items: &RustItems| vec![items.get(&name)],
    )
}

impl Creator {
    /// `classes()`.
    pub fn classes(self) -> GivenClasses {
        GivenClasses::new(
            self.priority,
            classes_transformer(),
            aggregators::identity(),
        )
    }

    /// `noClasses()`.
    pub fn no_classes(self) -> GivenClasses {
        let transformer = classes_transformer();
        let transformer = transformer.as_(format!("no {}", transformer.description()));
        GivenClasses::new(self.priority, transformer, aggregators::negate())
    }

    /// `theClass(name)`.
    pub fn the_class(self, class_name: &str) -> GivenClass {
        GivenClass::new(
            self.priority,
            the_class_transformer(class_name),
            aggregators::identity(),
        )
    }

    /// `noClass(name)`.
    pub fn no_class(self, class_name: &str) -> GivenClass {
        let transformer = the_class_transformer(class_name).as_(format!("no class {class_name}"));
        GivenClass::new(self.priority, transformer, aggregators::negate())
    }

    fn given_members<M: MemberLike>(self) -> GivenMembers<M> {
        GivenMembers::new(
            self.priority,
            members::members_transformer::<M>(),
            aggregators::identity(),
        )
    }

    fn given_no_members<M: MemberLike>(self) -> GivenMembers<M> {
        let transformer = members::members_transformer::<M>();
        let transformer = transformer.as_(format!("no {}", transformer.description()));
        GivenMembers::new(self.priority, transformer, aggregators::negate())
    }

    /// `members()`.
    pub fn members(self) -> GivenMembers<RustMember> {
        self.given_members()
    }

    /// `noMembers()`.
    pub fn no_members(self) -> GivenMembers<RustMember> {
        self.given_no_members()
    }

    /// `fields()`.
    pub fn fields(self) -> GivenFields {
        self.given_members()
    }

    /// `noFields()`.
    pub fn no_fields(self) -> GivenFields {
        self.given_no_members()
    }

    /// `codeUnits()`.
    pub fn code_units(self) -> GivenCodeUnits {
        self.given_members()
    }

    /// `noCodeUnits()`.
    pub fn no_code_units(self) -> GivenCodeUnits {
        self.given_no_members()
    }

    /// `constructors()`.
    pub fn constructors(self) -> GivenConstructors {
        self.given_members()
    }

    /// `noConstructors()`.
    pub fn no_constructors(self) -> GivenConstructors {
        self.given_no_members()
    }

    /// `methods()`.
    pub fn methods(self) -> GivenMethods {
        self.given_members()
    }

    /// `noMethods()`.
    pub fn no_methods(self) -> GivenMethods {
        self.given_no_members()
    }

    /// `all(transformer)`.
    pub fn all<T: AsCorrespondingObject + Send + Sync + 'static>(
        self,
        transformer: ClassesTransformer<T>,
    ) -> GivenObjects<T> {
        GivenObjects::new(self.priority, transformer, aggregators::identity())
    }

    /// `no(transformer)`.
    pub fn no<T: AsCorrespondingObject + Send + Sync + 'static>(
        self,
        transformer: ClassesTransformer<T>,
    ) -> GivenObjects<T> {
        let transformer = transformer.as_(format!("no {}", transformer.description()));
        GivenObjects::new(self.priority, transformer, aggregators::negate())
    }
}

impl ArchRuleDefinition {
    /// `ArchRuleDefinition.priority(priority)`.
    pub fn priority(priority: Priority) -> Creator {
        Creator { priority }
    }

    /// `ArchRuleDefinition.classes()`.
    pub fn classes() -> GivenClasses {
        Self::priority(Priority::Medium).classes()
    }

    /// `ArchRuleDefinition.noClasses()`.
    pub fn no_classes() -> GivenClasses {
        Self::priority(Priority::Medium).no_classes()
    }

    /// `ArchRuleDefinition.theClass(name)`.
    pub fn the_class(class_name: &str) -> GivenClass {
        Self::priority(Priority::Medium).the_class(class_name)
    }

    /// `ArchRuleDefinition.noClass(name)`.
    pub fn no_class(class_name: &str) -> GivenClass {
        Self::priority(Priority::Medium).no_class(class_name)
    }

    /// `ArchRuleDefinition.members()`.
    pub fn members() -> GivenMembers<RustMember> {
        Self::priority(Priority::Medium).members()
    }

    /// `ArchRuleDefinition.noMembers()`.
    pub fn no_members() -> GivenMembers<RustMember> {
        Self::priority(Priority::Medium).no_members()
    }

    /// `ArchRuleDefinition.fields()`.
    pub fn fields() -> GivenFields {
        Self::priority(Priority::Medium).fields()
    }

    /// `ArchRuleDefinition.noFields()`.
    pub fn no_fields() -> GivenFields {
        Self::priority(Priority::Medium).no_fields()
    }

    /// `ArchRuleDefinition.codeUnits()`.
    pub fn code_units() -> GivenCodeUnits {
        Self::priority(Priority::Medium).code_units()
    }

    /// `ArchRuleDefinition.noCodeUnits()`.
    pub fn no_code_units() -> GivenCodeUnits {
        Self::priority(Priority::Medium).no_code_units()
    }

    /// `ArchRuleDefinition.constructors()`.
    pub fn constructors() -> GivenConstructors {
        Self::priority(Priority::Medium).constructors()
    }

    /// `ArchRuleDefinition.noConstructors()`.
    pub fn no_constructors() -> GivenConstructors {
        Self::priority(Priority::Medium).no_constructors()
    }

    /// `ArchRuleDefinition.methods()`.
    pub fn methods() -> GivenMethods {
        Self::priority(Priority::Medium).methods()
    }

    /// `ArchRuleDefinition.noMethods()`.
    pub fn no_methods() -> GivenMethods {
        Self::priority(Priority::Medium).no_methods()
    }

    /// `ArchRuleDefinition.all(transformer)`.
    pub fn all<T: AsCorrespondingObject + Send + Sync + 'static>(
        transformer: ClassesTransformer<T>,
    ) -> GivenObjects<T> {
        Self::priority(Priority::Medium).all(transformer)
    }

    /// `ArchRuleDefinition.no(transformer)`.
    pub fn no<T: AsCorrespondingObject + Send + Sync + 'static>(
        transformer: ClassesTransformer<T>,
    ) -> GivenObjects<T> {
        Self::priority(Priority::Medium).no(transformer)
    }
}

/// `priority(priority)`.
pub fn priority(priority: Priority) -> Creator {
    ArchRuleDefinition::priority(priority)
}

/// `classes()`: a rule about all imported items.
pub fn classes() -> GivenClasses {
    ArchRuleDefinition::classes()
}

/// `noClasses()`: a rule that no imported item may satisfy.
pub fn no_classes() -> GivenClasses {
    ArchRuleDefinition::no_classes()
}

/// `theClass(name)`.
pub fn the_class(class_name: &str) -> GivenClass {
    ArchRuleDefinition::the_class(class_name)
}

/// `noClass(name)`.
pub fn no_class(class_name: &str) -> GivenClass {
    ArchRuleDefinition::no_class(class_name)
}

/// `members()`.
pub fn members() -> GivenMembers<RustMember> {
    ArchRuleDefinition::members()
}

/// `noMembers()`.
pub fn no_members() -> GivenMembers<RustMember> {
    ArchRuleDefinition::no_members()
}

/// `fields()`.
pub fn fields() -> GivenFields {
    ArchRuleDefinition::fields()
}

/// `noFields()`.
pub fn no_fields() -> GivenFields {
    ArchRuleDefinition::no_fields()
}

/// `codeUnits()`.
pub fn code_units() -> GivenCodeUnits {
    ArchRuleDefinition::code_units()
}

/// `noCodeUnits()`.
pub fn no_code_units() -> GivenCodeUnits {
    ArchRuleDefinition::no_code_units()
}

/// `constructors()`.
pub fn constructors() -> GivenConstructors {
    ArchRuleDefinition::constructors()
}

/// `noConstructors()`.
pub fn no_constructors() -> GivenConstructors {
    ArchRuleDefinition::no_constructors()
}

/// `methods()`.
pub fn methods() -> GivenMethods {
    ArchRuleDefinition::methods()
}

/// `noMethods()`.
pub fn no_methods() -> GivenMethods {
    ArchRuleDefinition::no_methods()
}

/// `all(transformer)`: a rule about custom objects.
pub fn all<T: AsCorrespondingObject + Send + Sync + 'static>(
    transformer: ClassesTransformer<T>,
) -> GivenObjects<T> {
    ArchRuleDefinition::all(transformer)
}

/// `no(transformer)`: a rule no custom object may satisfy.
pub fn no<T: AsCorrespondingObject + Send + Sync + 'static>(
    transformer: ClassesTransformer<T>,
) -> GivenObjects<T> {
    ArchRuleDefinition::no(transformer)
}

#[allow(dead_code)]
fn _member_kinds(_: RustField, _: RustCodeUnit, _: RustMethod, _: RustConstructor) {}
