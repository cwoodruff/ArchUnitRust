//! `ProxyRules`: rules for methods whose attribute installs a proxy (`#[transactional]`,
//! `#[cached]`, `#[tracing::instrument]`, ...) and must therefore not be called directly on
//! `self`.

use crate::base::DescribedPredicate;
use crate::core::domain::properties::AnnotationSelector;
use crate::core::domain::properties::can_be_annotated::predicates::annotated_with;
use crate::core::domain::{AccessTarget, RustItem};
use crate::lang::syntax::no_classes;
use crate::lang::{ArchCondition, ConditionEvents, SimpleArchRule, SimpleConditionEvent};

/// `no_classes_should_directly_call_other_methods_declared_in_the_same_class_that_are_annotated_with(A)`.
pub fn no_classes_should_directly_call_other_methods_declared_in_the_same_class_that_are_annotated_with(
    annotation: impl Into<AnnotationSelector>,
) -> SimpleArchRule<RustItem> {
    no_classes_should_directly_call_other_methods_declared_in_the_same_class_that(
        crate::lang::conditions::predicates::are(annotated_with::<AccessTarget>(annotation)),
    )
}

/// `no_classes_should_directly_call_other_methods_declared_in_the_same_class_that(pred)`.
pub fn no_classes_should_directly_call_other_methods_declared_in_the_same_class_that(
    predicate: DescribedPredicate<AccessTarget>,
) -> SimpleArchRule<RustItem> {
    no_classes()
        .should_with(directly_call_other_methods_declared_in_the_same_class_that(
            predicate,
        ))
        .because("it bypasses the proxy mechanism")
}

/// `directly_call_other_methods_declared_in_the_same_class_that_are_annotated_with(A)`.
pub fn directly_call_other_methods_declared_in_the_same_class_that_are_annotated_with(
    annotation: impl Into<AnnotationSelector>,
) -> ArchCondition<RustItem> {
    directly_call_other_methods_declared_in_the_same_class_that(
        crate::lang::conditions::predicates::are(annotated_with::<AccessTarget>(annotation)),
    )
}

/// `directly_call_other_methods_declared_in_the_same_class_that(pred)`: `self.method()` calls
/// to methods of the same item satisfying `predicate`.
pub fn directly_call_other_methods_declared_in_the_same_class_that(
    predicate: DescribedPredicate<AccessTarget>,
) -> ArchCondition<RustItem> {
    let description = format!(
        "directly call other methods declared in the same class that {}",
        predicate.description()
    );
    ArchCondition::new(
        description,
        move |item: &RustItem, events: &mut ConditionEvents| {
            for call in item.method_calls_from_self() {
                let satisfied =
                    call.origin_owner() == call.target_owner() && predicate.test(&call.target());
                let message = call.description();
                events.add(SimpleConditionEvent::new(&call, satisfied, message));
            }
        },
    )
}
