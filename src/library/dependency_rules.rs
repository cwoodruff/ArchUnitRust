//! `DependencyRules`: rules about the direction of dependencies between modules.

use std::sync::LazyLock;

use crate::core::domain::RustItem;
use crate::lang::syntax::no_classes;
use crate::lang::{ArchCondition, ConditionEvents, SimpleArchRule, SimpleConditionEvent};

/// `dependOnUpperPackages()`: dependencies on items of an ancestor module (`super::..`).
///
/// A `#[cfg(test)] mod tests` depends on its parent by design; import with
/// `DoNotIncludeTests` to keep unit tests out of this rule.
pub fn depend_on_upper_packages() -> ArchCondition<RustItem> {
    ArchCondition::new(
        "depend on upper packages",
        |item: &RustItem, events: &mut ConditionEvents| {
            for dependency in item.direct_dependencies_from_self() {
                let origin_package = dependency.origin_item().package_name();
                let target_prefix = format!("{}::", dependency.target_item().package_name());
                let on_upper_package = origin_package.starts_with(&target_prefix);
                let message = dependency.description();
                events.add(SimpleConditionEvent::new(
                    &dependency,
                    on_upper_package,
                    message,
                ));
            }
        },
    )
}

/// `NO_CLASSES_SHOULD_DEPEND_UPPER_PACKAGES`.
pub static NO_CLASSES_SHOULD_DEPEND_UPPER_PACKAGES: LazyLock<SimpleArchRule<RustItem>> =
    LazyLock::new(|| {
        no_classes()
            .should_with(depend_on_upper_packages())
            .because("that might prevent packages on that level from being split into separate artifacts in a clean way")
    });
