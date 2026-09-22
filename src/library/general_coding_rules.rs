//! `GeneralCodingRules`: ready-made conditions and rules for common coding pitfalls.
//!
//! The Java constants are `LazyLock` statics here; clone a condition to pass it to
//! `should_with(..)`, or `check` a rule directly:
//!
//! ```no_run
//! use archunit::library::general_coding_rules::{
//!     ACCESS_STANDARD_STREAMS, NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS,
//! };
//! use archunit::prelude::*;
//! # let items = archunit::core::importer::CrateImporter::new().import_path("src");
//!
//! NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS.check(&items);
//! no_classes().that().reside_in_a_package("..domain..").should_with(ACCESS_STANDARD_STREAMS.clone()).check(&items);
//! ```
//!
//! Binary targets are excluded from [`ACCESS_STANDARD_STREAMS`] and [`CALL_PROCESS_EXIT`]
//! unless `import.exclude_binaries_from_coding_rules = false` is configured, because printing
//! and exiting is what a `main` is for.

use std::sync::{Arc, LazyLock};

use crate::base::DescribedPredicate;
use crate::config::ArchConfiguration;
use crate::core::domain::properties::can_be_annotated::predicates::annotated_with;
use crate::core::domain::rust_access::predicates::target;
use crate::core::domain::rust_member::MemberLike;
use crate::core::domain::{
    AccessTarget, Dependency, DependencyKind, RustAccess, RustItem, RustMember, RustType,
};
use crate::lang::conditions::{access_target_where, depend_on_classes_that};
use crate::lang::syntax::no_classes;
use crate::lang::{ArchCondition, ConditionEvents, SimpleArchRule, SimpleConditionEvent};

type ItemGetter<A> = Arc<dyn Fn(&RustItem) -> Vec<A> + Send + Sync>;

fn exclude_binaries() -> bool {
    ArchConfiguration::get().exclude_binaries_from_coding_rules()
}

/// A condition over the dependencies of an item: every dependency returned by `get` yields a
/// satisfied or violated event depending on `matches` (like Java's `accessTargetWhere`).
fn dependencies_where(
    description: &str,
    get: ItemGetter<Dependency>,
    matches: impl Fn(&Dependency) -> bool + Send + Sync + 'static,
) -> ArchCondition<RustItem> {
    ArchCondition::new(
        description,
        move |item: &RustItem, events: &mut ConditionEvents| {
            for dependency in get(item) {
                let message = dependency.description();
                events.add(SimpleConditionEvent::new(
                    &dependency,
                    matches(&dependency),
                    message,
                ));
            }
        },
    )
}

fn accesses_where(
    description: &str,
    get: ItemGetter<RustAccess>,
    matches: impl Fn(&RustAccess) -> bool + Send + Sync + 'static,
) -> ArchCondition<RustItem> {
    ArchCondition::new(
        description,
        move |item: &RustItem, events: &mut ConditionEvents| {
            for access in get(item) {
                let message = access.description();
                events.add(SimpleConditionEvent::new(
                    &access,
                    matches(&access),
                    message,
                ));
            }
        },
    )
}

fn macro_invocations(item: &RustItem) -> Vec<Dependency> {
    item.direct_dependencies_from_self()
        .into_iter()
        .filter(|d| d.kind() == DependencyKind::MacroInvocation)
        .collect()
}

fn macro_invocations_outside_binaries() -> ItemGetter<Dependency> {
    Arc::new(|item: &RustItem| {
        if exclude_binaries() && item.cargo_target().is_binary() {
            return Vec::new();
        }
        macro_invocations(item)
    })
}

fn method_calls_outside_binaries() -> ItemGetter<RustAccess> {
    Arc::new(|item: &RustItem| {
        if exclude_binaries() && item.cargo_target().is_binary() {
            return Vec::new();
        }
        item.method_calls_from_self()
    })
}

fn macro_named(dependency: &Dependency, names: &[&str]) -> bool {
    let name = dependency.target_item().name();
    names
        .iter()
        .any(|n| name == format!("std::{n}") || name == format!("core::{n}"))
}

fn calls_function_of(access: &RustAccess, module: &str, names: &[&str]) -> bool {
    let owner = access.target_owner().name();
    (owner == module || owner == module.replacen("std::", "core::", 1))
        && names.contains(&access.target().name().as_str())
}

// ---- standard streams ----------------------------------------------------------------------

/// `ACCESS_STANDARD_STREAMS`: invocations of `print!`, `println!`, `eprint!`, `eprintln!`
/// and `dbg!`, and calls of `std::io::stdout()`, `stderr()` or `stdin()`.
pub static ACCESS_STANDARD_STREAMS: LazyLock<ArchCondition<RustItem>> =
    LazyLock::new(access_standard_streams);

fn access_standard_streams() -> ArchCondition<RustItem> {
    let printing = dependencies_where(
        "invoke printing macros",
        macro_invocations_outside_binaries(),
        |d| macro_named(d, &["print", "println", "eprint", "eprintln", "dbg"]),
    );
    let streams = accesses_where(
        "call standard stream functions",
        method_calls_outside_binaries(),
        |a| calls_function_of(a, "std::io", &["stdout", "stderr", "stdin"]),
    );
    printing.or(streams).as_("access standard streams")
}

/// `NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS`.
pub static NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS: LazyLock<SimpleArchRule<RustItem>> =
    LazyLock::new(|| {
        no_classes()
            .should_with(ACCESS_STANDARD_STREAMS.clone())
            .into_rule()
    });

// ---- generic errors ------------------------------------------------------------------------

/// `THROW_GENERIC_EXCEPTIONS`: a "generic exception" is a generic error type in a `Result`:
/// `Box<dyn Error>` (with any auto-trait bounds), `String`, `&str`, `()`, `anyhow::Error`
/// or `eyre::Report`.
pub static THROW_GENERIC_EXCEPTIONS: LazyLock<ArchCondition<RustItem>> =
    LazyLock::new(throw_generic_exceptions);

fn is_std_type(member: &RustMember, ty: &RustType, suffixes: &[&str]) -> bool {
    let Some(item) = member.resolve_type(ty) else {
        return false;
    };
    let name = item.name();
    suffixes.iter().any(|s| {
        name == *s
            || name == format!("std::{s}")
            || name == format!("core::{s}")
            || name == format!("alloc::{s}")
    })
}

fn is_generic_error_type(member: &RustMember, ty: &RustType) -> bool {
    match ty {
        RustType::Tuple(elements) => elements.is_empty(),
        RustType::Reference { inner, .. } => is_std_type(member, inner, &["str"]),
        RustType::Path { args, .. } => {
            if is_std_type(
                member,
                ty,
                &["string::String", "String", "anyhow::Error", "eyre::Report"],
            ) {
                return true;
            }
            is_std_type(member, ty, &["boxed::Box", "Box"])
                && args.first().is_some_and(|arg| match arg {
                    RustType::TraitObject(bounds) => bounds
                        .iter()
                        .any(|b| is_std_type(member, b, &["error::Error", "Error"])),
                    _ => false,
                })
        }
        _ => false,
    }
}

fn throw_generic_exceptions() -> ArchCondition<RustItem> {
    ArchCondition::new(
        "throw generic exceptions",
        |item: &RustItem, events: &mut ConditionEvents| {
            for code_unit in item.code_units() {
                let member: &RustMember = code_unit.as_member();
                let Some(error_type) = member.error_type() else {
                    continue;
                };
                let generic = is_generic_error_type(member, &error_type);
                let message = format!(
                    "{} throws generic type <{error_type}> in {}",
                    member.description(),
                    member.source_code_location()
                );
                events.add(SimpleConditionEvent::new(member, generic, message));
            }
        },
    )
}

/// `NO_CLASSES_SHOULD_THROW_GENERIC_EXCEPTIONS`.
pub static NO_CLASSES_SHOULD_THROW_GENERIC_EXCEPTIONS: LazyLock<SimpleArchRule<RustItem>> =
    LazyLock::new(|| {
        no_classes()
            .should_with(THROW_GENERIC_EXCEPTIONS.clone())
            .into_rule()
    });

// ---- assertions ----------------------------------------------------------------------------

/// `invoke assertion macros without a detail message`: `assert!(cond)`, `debug_assert!(cond)`,
/// `assert_eq!(a, b)`, `assert_ne!(a, b)` and their `debug_` variants without a message.
pub static INVOKE_ASSERTIONS_WITHOUT_DETAIL_MESSAGE: LazyLock<ArchCondition<RustItem>> =
    LazyLock::new(|| {
        dependencies_where(
            "invoke assertion macros without a detail message",
            Arc::new(macro_invocations),
            |d| {
                let Some(count) = d.macro_argument_count() else {
                    return false;
                };
                (macro_named(d, &["assert", "debug_assert"]) && count < 2)
                    || (macro_named(
                        d,
                        &[
                            "assert_eq",
                            "assert_ne",
                            "debug_assert_eq",
                            "debug_assert_ne",
                        ],
                    ) && count < 3)
            },
        )
    });

/// `ASSERTIONS_SHOULD_HAVE_DETAIL_MESSAGE`.
pub static ASSERTIONS_SHOULD_HAVE_DETAIL_MESSAGE: LazyLock<SimpleArchRule<RustItem>> =
    LazyLock::new(|| {
        no_classes()
            .should_with(INVOKE_ASSERTIONS_WITHOUT_DETAIL_MESSAGE.clone())
            .because("assertions should have a detail message")
    });

// ---- deprecation ---------------------------------------------------------------------------

/// `DEPRECATED_API_SHOULD_NOT_BE_USED`: accesses to `#[deprecated]` members and dependencies
/// on `#[deprecated]` items.
pub static DEPRECATED_API_SHOULD_NOT_BE_USED: LazyLock<SimpleArchRule<RustItem>> =
    LazyLock::new(|| {
        no_classes()
            .should_with(
                access_target_where(target(annotated_with::<AccessTarget>("deprecated")))
                    .as_("access @deprecated members"),
            )
            .or_should_with(
                depend_on_classes_that(annotated_with::<RustItem>("deprecated"))
                    .as_("depend on @deprecated classes"),
            )
            .because("there should be a better alternative")
    });

// ---- rust-only rules -----------------------------------------------------------------------

/// `[rust-only]` `call unwrap or expect`: `.unwrap()` / `.expect(..)` calls outside test code.
pub static CALL_UNWRAP: LazyLock<ArchCondition<RustItem>> = LazyLock::new(|| {
    accesses_where(
        "call unwrap or expect outside test code",
        Arc::new(|item: &RustItem| item.method_calls_from_self()),
        |a| {
            matches!(a.target().name().as_str(), "unwrap" | "expect")
                && !a.origin().as_member().is_test_code()
        },
    )
});

/// `[rust-only]` `NO_CLASSES_SHOULD_CALL_UNWRAP`.
pub static NO_CLASSES_SHOULD_CALL_UNWRAP: LazyLock<SimpleArchRule<RustItem>> =
    LazyLock::new(|| {
        no_classes()
            .should_with(CALL_UNWRAP.clone())
            .because("errors should be propagated or handled, not crash the program")
    });

/// `[rust-only]` `panic`: invocations of `panic!`, `unreachable!`, `todo!` and
/// `unimplemented!` outside test code.
pub static PANIC: LazyLock<ArchCondition<RustItem>> = LazyLock::new(|| {
    dependencies_where(
        "panic",
        Arc::new(|item: &RustItem| {
            macro_invocations(item)
                .into_iter()
                .filter(|d| !d.origin_item().is_test_code())
                .collect()
        }),
        |d| macro_named(d, &["panic", "unreachable", "todo", "unimplemented"]),
    )
});

/// `[rust-only]` `NO_CLASSES_SHOULD_PANIC`.
pub static NO_CLASSES_SHOULD_PANIC: LazyLock<SimpleArchRule<RustItem>> =
    LazyLock::new(|| no_classes().should_with(PANIC.clone()).into_rule());

/// `[rust-only]` `call std::process::exit`: calls of `std::process::exit` or `abort`.
pub static CALL_PROCESS_EXIT: LazyLock<ArchCondition<RustItem>> = LazyLock::new(|| {
    accesses_where(
        "call std::process::exit",
        method_calls_outside_binaries(),
        |a| calls_function_of(a, "std::process", &["exit", "abort"]),
    )
});

/// `[rust-only]` `NO_LIBRARY_CODE_SHOULD_CALL_PROCESS_EXIT`.
pub static NO_LIBRARY_CODE_SHOULD_CALL_PROCESS_EXIT: LazyLock<SimpleArchRule<RustItem>> =
    LazyLock::new(|| {
        no_classes()
            .should_with(CALL_PROCESS_EXIT.clone())
            .as_("no library code should call std::process::exit")
            .because("only a binary's main may decide to end the process")
    });

/// `[rust-only]` `use unsafe`: `unsafe` items, `unsafe fn`s and `unsafe { .. }` blocks.
///
/// Like Java's access conditions, every code unit yields an event that is satisfied when it
/// uses `unsafe`, so `no_classes().should_with(USE_UNSAFE.clone())` reports each unsafe use.
pub static USE_UNSAFE: LazyLock<ArchCondition<RustItem>> = LazyLock::new(|| {
    ArchCondition::new(
        "use unsafe",
        |item: &RustItem, events: &mut ConditionEvents| {
            if item
                .modifiers()
                .contains(&crate::core::domain::RustModifier::Unsafe)
            {
                events.add(SimpleConditionEvent::satisfied(
                    item,
                    format!(
                        "{} is declared unsafe in {}",
                        item.description(),
                        item.source_code_location()
                    ),
                ));
            }
            for code_unit in item.code_units() {
                let member: &RustMember = code_unit.as_member();
                let mut uses_unsafe = false;
                if member
                    .modifiers()
                    .contains(&crate::core::domain::RustModifier::Unsafe)
                {
                    uses_unsafe = true;
                    events.add(SimpleConditionEvent::satisfied(
                        member,
                        format!(
                            "{} is declared unsafe in {}",
                            member.description(),
                            member.source_code_location()
                        ),
                    ));
                }
                let file = member.source_code_location().source_file_name().to_owned();
                for line in member.unsafe_block_lines() {
                    uses_unsafe = true;
                    events.add(SimpleConditionEvent::satisfied(
                        member,
                        format!(
                            "{} contains an unsafe block in ({file}:{line})",
                            member.description()
                        ),
                    ));
                }
                if !uses_unsafe {
                    events.add(SimpleConditionEvent::violated(
                        member,
                        format!(
                            "{} does not use unsafe in {}",
                            member.description(),
                            member.source_code_location()
                        ),
                    ));
                }
            }
        },
    )
});

/// `[rust-only]` `NO_CLASSES_SHOULD_USE_UNSAFE`.
pub static NO_CLASSES_SHOULD_USE_UNSAFE: LazyLock<SimpleArchRule<RustItem>> =
    LazyLock::new(|| no_classes().should_with(USE_UNSAFE.clone()).into_rule());

/// `[rust-only]` Described as `have a generic error type`: code units returning
/// `Result<_, E>` with a generic `E` (see [`THROW_GENERIC_EXCEPTIONS`]).
pub fn have_generic_error_type() -> DescribedPredicate<RustMember> {
    DescribedPredicate::describe("have a generic error type", |member: &RustMember| {
        member
            .error_type()
            .is_some_and(|ty| is_generic_error_type(member, &ty))
    })
}
