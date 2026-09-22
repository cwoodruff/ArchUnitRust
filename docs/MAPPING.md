# ArchUnit → Rust mapping

This document maps every public ArchUnit API (Java, version 1.5/1.6 line, package
`com.tngtech.archunit`) to its equivalent in this crate. Nothing is dropped silently:
every entry is either mapped, mapped with a documented approximation, or marked
unsupported with a reason.

Status legend (updated at the end of every phase):

| Status | Meaning |
|---|---|
| `P1`..`P6` | Planned for that phase, not implemented yet |
| `done` | Implemented and tested |
| `partial` | Implemented with a documented approximation or missing overloads |
| `unsupported` | Cannot be ported; reason given |
| `deferred` | Portable, but outside the phases agreed so far; needs a go-ahead |

---

## 1. Naming and translation conventions

These rules are applied mechanically. If a Java name is not listed in a table below,
translate it with these rules and it should exist.

| Java | Rust | Note |
|---|---|---|
| `camelCase` methods | `snake_case` | `resideInAPackage` → `reside_in_a_package` |
| `SCREAMING_CASE` constants | `SCREAMING_CASE` statics/consts | `NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS` stays |
| `Java*` domain types | `Rust*` domain types | `JavaClass` → `RustItem`, `JavaClasses` → `RustItems` |
| static factory classes (`ArchRuleDefinition`, `ArchConditions`, …) | a module of free functions **and** a unit struct with associated functions | `ArchRuleDefinition::classes()` and `archunit::lang::syntax::classes()` both exist |
| `Foo.Predicates.bar(..)` | `foo::predicates::bar(..)` | `JavaClass.Predicates.simpleName` → `rust_item::predicates::simple_name` |
| `Foo.Functions.GET_X` | `foo::functions::get_x()` | Chainable functions become plain `Fn` closures returning `DescribedFunction` |
| method named after a Rust keyword | trailing underscore | `as(..)` → `as_(..)`, `in(..)` → `in_(..)`, `be` is fine |
| `String...` varargs | `&[&str]` | `reside_in_any_package(&["..a..", "..b.."])` |
| `Class<?>` parameter | the item's full path as `&str` | Rust has no reflection. `std::any::type_name::<T>()` yields the same crate-rooted path this crate uses, so it can be passed where a `Class<?>` was |
| overload taking `String` **or** `Class<?>` **or** `DescribedPredicate` | one method taking `impl Into<ItemSelector>` | `&str`, `String` and `DescribedPredicate<RustItem>` all convert. `implement("my_crate::Repo")` and `implement(pred)` both work |
| zero-arg fluent method **and** an overload taking a predicate/condition object | fluent form keeps the name; object form gets suffix `_with` | `that()` / `that_with(pred)`, `should()` / `should_with(cond)`, `and()` / `and_with(pred)`, `depend_on_classes_that()` / `depend_on_classes_that_with(pred)`, `by_classes_that()` / `by_classes_that_with(pred)` |
| `Optional<T>` | `Option<T>` | |
| `Set<T>` / `List<T>` getters | `&[T]` or `impl Iterator` | Prefixes `get` are dropped: `getSimpleName()` → `simple_name()` |
| `tryGetX()` | `try_x()` returning `Option` | `getX()` (panicking) is also kept |
| `toString()` | `Display` | |
| `equals`/`hashCode` | `PartialEq`/`Eq`/`Hash` | |
| anonymous subclass of `DescribedPredicate` | `DescribedPredicate::describe("desc", \|x\| ..)` | Same as Java's `describe(..)` |
| anonymous subclass of `ArchCondition` | `ArchCondition::new("desc", \|item, events\| ..)` or `impl ArchCondition<T>` | The trait form gives `init`/`finish` |
| Java package identifier `com.myapp.service..` | module identifier `my_app::service..` | Same wildcard syntax on `::`-separated module paths. See §2.4 |
| line-number source location `(Foo.java:14)` | `(src/foo.rs:14)` | Path is relative to the crate root of the item |

### 1.1 Rust concepts that have no Java counterpart

These are additions, named in ArchUnit style, and marked `[rust-only]` in the tables.
They never replace an ArchUnit name.

---

## 2. Core API (`com.tngtech.archunit.core`) → `archunit::core`

### 2.1 Importer

| Java (`core.importer`) | Rust (`archunit::core::importer`) | Status | Note |
|---|---|---|---|
| `ClassFileImporter` | `CrateImporter` | done | Parses source with `syn`; crate graph from `cargo_metadata` |
| `new ClassFileImporter()` | `CrateImporter::new()` | done | |
| `withImportOption(ImportOption)` | `with_import_option(impl ImportOption)` | done | |
| `withImportOptions(Collection)` | `with_import_options(Vec<Box<dyn ImportOption>>)` | done | |
| `importClasspath()` | `import_workspace()` | done | Every workspace member plus (optionally) dependency crates. Name kept close to the concept: the "classpath" of a Rust build is the cargo dependency graph |
| `importPackages(String...)` | `import_packages(&[&str])` | done | Module identifiers, e.g. `"my_app::domain.."`; imports the crates that own those modules and keeps only matching modules |
| `importPackagesOf(Class...)` | `import_packages_of(&[&str])` | done | Modules containing the named items |
| `importPackage(String)` | `import_package(&str)` | done | |
| `importClasses(Class...)` | `import_items(&[&str])` | done | Named items only (plus stubs for their targets) |
| `importClass(Class)` | `import_item(&str)` | done | |
| `importPath(Path)` / `importPaths(..)` | `import_path(impl AsRef<Path>)` / `import_paths(..)` | done | Path to a `Cargo.toml`, a crate directory, or a workspace root |
| `importUrl` / `importUrls` | — | unsupported | No URL sources for Rust code |
| `importJar` / `importJars` | — | unsupported | No archive format; use `import_path` on an extracted crate |
| `importLocations(Collection<Location>)` | `import_locations(&[Location])` | done | |
| `Location` | `Location` | done | A source file path plus its crate and cargo target |
| `Location.of(Path/URL/URI/JarFile)` | `Location::of(path)` | done | Only paths |
| `Location.contains(String)` / `matches(Pattern)` | `contains(&str)` / `matches(&Regex)` | done | |
| `Location.isJar()` / `isArchive()` | — | unsupported | Always false; not provided |
| `ImportOption` (interface) | `trait ImportOption { fn includes(&self, location: &Location) -> bool }` | done | Also implemented for closures `Fn(&Location) -> bool` |
| `ImportOption.Predefined.DO_NOT_INCLUDE_TESTS` / `DoNotIncludeTests` | `import_option::DoNotIncludeTests` | done | Excludes `#[cfg(test)]` modules and items, `#[test]` functions, the `tests/` directory, and `test` cargo targets |
| `DO_NOT_INCLUDE_JARS` / `DoNotIncludeJars` | `import_option::DoNotIncludeDependencies` | done | Excludes crates that are not workspace members (registry, git, path deps outside the workspace) |
| `DO_NOT_INCLUDE_ARCHIVES` | `DoNotIncludeDependencies` | done | Same as above (no archive/jar distinction in Rust) |
| `DO_NOT_INCLUDE_PACKAGE_INFOS` | — | unsupported | No `package-info` concept |
| `ONLY_INCLUDE_TESTS` | `import_option::OnlyIncludeTests` | done | |
| `DoNotIncludeJars`-style custom `ImportOption` classes for `@AnalyzeClasses` | any `ImportOption` value in `import_options = [..]` | done | |
| `ImportOptions` (internal) | — | — | internal |
| `ClassResolver` / `SelectedClassResolverFromClasspath` | `CrateImporter::resolving_missing_dependencies_from_classpath(true)` + `class_resolver.packages` in `archunit.toml` | partial | Missing items from dependency crates are stubbed by default. The option parses the library target of every dependency, or only of the crates selected by `class_resolver.packages`; no custom resolver by class name |
| "Dealing with Missing Classes" (stubs) | `RustItem::is_fully_imported()` returns `false` for stubs | done | |

### 2.2 Domain model

| Java (`core.domain`) | Rust (`archunit::core::domain`) | Status | Note |
|---|---|---|---|
| `JavaClasses` | `RustItems` | done | Owns the graph; `IntoIterator<Item=&RustItem>` |
| `JavaClasses.get(Class/String)` | `get(&str)` / `try_get(&str)` | done | Canonical paths and public re-export paths (`my_app::Order` for `pub use domain::model::Order`) both work; `try_get_any` also finds modules and stubs |
| `contain(..)`, `containPackage(..)` | `contain(&str)`, `contain_package(&str)` | done | |
| `that(DescribedPredicate)` | `that(&DescribedPredicate<RustItem>)` | done | |
| `as(String)` / `getDescription()` | `as_(&str)` / `description()` | done | |
| `getDefaultPackage()` / `getPackage(String)` | `default_packages()` (one crate root per imported crate) / `package(&str)` | done | |
| `JavaClass` | `RustItem` | done | struct, enum, union, trait, fn, impl block, mod, type alias, const, static, macro (`macro_rules!`, proc-macro fn), extern crate |
| `JavaClass.getName()` | `name()` | done | Crate-rooted full path `my_app::domain::order::Order`. See PLAN §"Full names" for the `crate::` decision |
| `getSimpleName()` | `simple_name()` | done | Last path segment; `impl` blocks get `impl Trait for Type` / `impl Type` |
| `getFullName()` | `full_name()` | done | Same as `name()` for items |
| `getPackageName()` / `getPackage()` | `package_name()` / `package()` → `&RustModule` | done | The enclosing module path |
| `getModifiers()` | `modifiers()` → `&[RustModifier]` | done | See §2.3 |
| `isInterface()` / `isEnum()` / `isRecord()` / `isAnnotation()` | `is_trait()` / `is_enum()` / — / `is_proc_macro()` | done | `isRecord` unsupported: no record concept (see `[rust-only]` `is_struct()`). `isAnnotation` maps to items that define attribute or derive macros |
| `isTopLevelClass()` | `is_top_level_item()` | done | Declared directly in a module body |
| `isNestedClass()` / `isLocalClass()` | `is_nested_item()` / `is_local_item()` | done | Both mean "declared inside a function body or another item body" |
| `isMemberClass()` / `isInnerClass()` / `isAnonymousClass()` | — | unsupported | Rust has no member/inner/anonymous types; closures are not items |
| `isArray()` / `isPrimitive()` / `getComponentType()` / `getBaseComponentType()` | `is_primitive()` for `i32`, `bool`, `str`, … ; arrays/slices/references/pointers are unwrapped to their element type when dependencies are recorded | partial | Array-ness is not a property of an item in Rust |
| `isSealed()` / `getPermittedSubclasses()` | `is_sealed()` / — | partial | `#[non_exhaustive]` enums or traits with a private supertrait are reported sealed; permitted subclasses have no counterpart |
| `isFullyImported()` | `is_fully_imported()` | done | |
| `getSuperclass()` / `getRawSuperclass()` / `getAllRawSuperclasses()` / `getClassHierarchy()` | `supertraits()` / `all_supertraits()` / `trait_hierarchy()` | partial | Only traits have supertypes in Rust. Struct/enum items return empty |
| `getInterfaces()` / `getRawInterfaces()` / `getAllRawInterfaces()` | `implemented_traits()` / `all_implemented_traits()` | done | From `impl Trait for Type` blocks (in any imported crate) plus derives that resolve to traits |
| `getSubclasses()` / `getAllSubclasses()` | `implementors()` / `all_implementors()` on traits; `subtraits()` | done | |
| `getAllClassesSelfIsAssignableTo()` | `all_items_self_is_assignable_to()` | done | Self plus implemented traits plus their supertraits |
| `isAssignableTo(..)` / `isAssignableFrom(..)` / `isEquivalentTo(..)` | `is_assignable_to(..)` / `is_assignable_from(..)` / `is_equivalent_to(&str)` | done | `assignable_to(T)` = is `T` or implements trait `T` (transitively via supertraits) |
| `getEnclosingClass()` / `getEnclosingCodeUnit()` | `enclosing_item()` / `enclosing_code_unit()` | done | For local items |
| `getMembers()` / `getAllMembers()` | `members()` / `all_members()` | done | `all_` includes trait-provided default items |
| `getFields()` / `getAllFields()` / `getField(String)` / `tryGetField` | `fields()` / … | done | Struct/enum-variant/union fields plus associated consts (see §2.3) |
| `getMethods()` / `getAllMethods()` / `getMethod(..)` / `tryGetMethod(..)` | `methods()` / … | done | Associated functions from inherent and trait impls, and trait method declarations |
| `getConstructors()` / `getConstructor(..)` / `getAllConstructors()` | `constructors()` / … | partial | Approximation: associated functions with no `self` receiver whose return type is `Self`, `Option<Self>`, `Result<Self, _>`, or the owning type by name. Documented heuristic |
| `getCodeUnits()` / `getCodeUnitWithParameterTypes(..)` | `code_units()` / `code_unit_with_parameter_types(..)` | done | All functions of the item |
| `getStaticInitializer()` | — | unsupported | No static initializers in Rust |
| `getEnumConstants()` / `getEnumConstant(String)` | `variants()` / `variant(&str)` | done | |
| `getAnnotations()` / `getAnnotationOfType(..)` / `tryGetAnnotationOfType(..)` / `isAnnotatedWith(..)` / `isMetaAnnotatedWith(..)` | `annotations()` / `annotation_of_type(&str)` / `try_annotation_of_type(&str)` / `is_annotated_with(..)` / — | partial | Attributes and derives, see §2.5. Meta-annotation unsupported |
| `getAnnotationsWithTypeOfSelf()` / `getAnnotationsWithParameterTypeOfSelf()` | `annotations_with_type_of_self()` / — | partial | Only for items that define attribute/derive macros |
| `getAccessesFromSelf()` / `getAllAccessesFromSelf()` | `accesses_from_self()` / `all_accesses_from_self()` | done | See §2.6 |
| `getAccessesToSelf()` | `accesses_to_self()` | done | |
| `getFieldAccessesFromSelf()` / `getFieldAccessesToSelf()` | `field_accesses_from_self()` / `field_accesses_to_self()` | done | |
| `getMethodCallsFromSelf()` / `getMethodCallsToSelf()` | `method_calls_from_self()` / `method_calls_to_self()` | done | |
| `getConstructorCallsFromSelf()` / `getConstructorCallsToSelf()` | `constructor_calls_from_self()` / `constructor_calls_to_self()` | partial | Calls to functions classified as constructors, plus struct literals `Foo { .. }` and tuple-struct/variant construction `Foo(..)` |
| `getCodeUnitCallsFromSelf()` / `getCodeUnitCallsToSelf()` | `code_unit_calls_from_self()` / `code_unit_calls_to_self()` | done | |
| `getMethodReferencesFromSelf()` / `..ToSelf()` / `getConstructorReferences..` / `getCodeUnitReferences..` | `function_references_from_self()` / `function_references_to_self()` | done | A function path used as a value (`map(Foo::new)`) |
| `getCodeUnitAccessesFromSelf()` / `..ToSelf()` | `code_unit_accesses_from_self()` / `..to_self()` | done | Calls plus references |
| `getDirectDependenciesFromSelf()` / `getDirectDependenciesToSelf()` | `direct_dependencies_from_self()` / `direct_dependencies_to_self()` | done | |
| `getTransitiveDependenciesFromSelf()` | `transitive_dependencies_from_self()` | done | |
| `getFieldsWithTypeOfSelf()`, `getMethodsWithParameterTypeOfSelf()`, `getMethodsWithReturnTypeOfSelf()`, `getConstructorsWithParameterTypeOfSelf()` | same names, snake_case | done | |
| `getMethodThrowsDeclarationsWithTypeOfSelf()` / `getConstructorsWithThrowsDeclarationTypeOfSelf()` / `getThrowsDeclarations()` | `functions_with_error_type_of_self()` / — / `error_types()` | partial | "throws E" ≙ returns `Result<_, E>`. See §2.7 |
| `getInstanceofChecks()` / `getInstanceofChecksWithTypeOfSelf()` | — | unsupported | No runtime type checks; nearest is `downcast_ref::<T>()`, recorded as a plain dependency |
| `getReferencedClassObjects()` | `referenced_type_objects()` | partial | `TypeId::of::<T>()`, `size_of::<T>()`, `type_name::<T>()` turbofish uses |
| `getTryCatchBlocks()` / `getTryCatchBlocksThatCatchSelf()` | — | unsupported | No try/catch; `?` and `match` on `Result` are not blocks |
| `getTypeParameters()` | `type_parameters()` | done | Generic params with bounds |
| `getSource()` / `Source.getMd5sum()` | `source()` → `Option<&Source>` / — | partial | File path and crate; MD5 unsupported (see §7) |
| `getSourceCodeLocation()` | `source_code_location()` | done | |
| `getDescription()` | `description()` | done | `"Class <name>"` becomes `"Item <name>"` (per the agreed failure format), modules `"Module <name>"`; members: `"Method <..>"`, `"Field <..>"`, `"Constructor <..>"`, `"Function <..>"` (free functions), `"Variant <..>"` |
| `reflect()` | — | unsupported | No runtime reflection in Rust |
| `traverseSignature(SignatureVisitor)` | `traverse_signature(&mut impl SignatureVisitor)` | deferred | Generic-signature visitor; portable but low value |
| `toErasure()` | `to_erasure()` | done | Identity for items |
| `[rust-only]` | `is_struct()`, `is_union()`, `is_function()`, `is_module()`, `is_type_alias()`, `is_const()`, `is_static()`, `is_macro()`, `is_impl()`, `kind() -> ItemKind` | done | |
| `[rust-only]` | `crate_name()`, `cargo_target()`, `aliases()` (public re-export paths), `visibility()`, `impl_self_type()`, `impl_trait()`, `as_module()`, `as_items()` | done | |
| `JavaPackage` | `RustModule` | done | |
| `JavaPackage.getName()` / `getRelativeName()` | `name()` / `relative_name()` | done | |
| `getClasses()` / `getClassesInPackageTree()` | `items()` / `items_in_package_tree()` | done | |
| `getSubpackages()` / `getSubpackagesInTree()` / `getPackage(String)` / `containsPackage(..)` | `subpackages()` / `subpackages_in_tree()` / `package(&str)` / `contains_package(&str)` | done | |
| `getParent()` | `parent()` | done | |
| `getClass(..)` / `getClassWithFullyQualifiedName` / `getClassWithSimpleName` / `containsClass..` | `item(..)` / `item_with_fully_qualified_name` / `item_with_simple_name` / `contains_item..` | done | |
| `getClassDependenciesFromThisPackage()` / `..ToThisPackage()` / `..FromThisPackageTree()` / `..ToThisPackageTree()` | same names, snake_case, `class` → `item` | done | |
| `getPackageDependenciesFromThisPackage()` / … | same names, snake_case | done | |
| `getPackageInfo()` / `getAnnotations()` / `isAnnotatedWith(..)` | `module_attributes()` / `annotations()` / `is_annotated_with(..)` | partial | Inner attributes of the module (`#![allow(..)]`, `#![doc = ..]`, `#![cfg(..)]`). Custom inner attributes are unstable in Rust, so annotation-based module definitions are limited to built-in attributes |
| `traversePackageTree(predicate, visitor)` | `traverse_package_tree(pred, &mut visitor)` | done | |
| `JavaPackage.Predicates` / `Functions` | `rust_module::predicates` / `functions` | done | |
| `JavaMember` | `RustMember` | done | Field, method (associated fn), variant, associated const, associated type |
| `JavaMember.getOwner()` / `getName()` / `getFullName()` / `getModifiers()` / `getDescriptor()` | `owner()` / `name()` / `full_name()` / `modifiers()` / — | done | `getDescriptor` (JVM descriptor) unsupported |
| `getAccessesToSelf()` / `getAllInvolvedRawTypes()` | `accesses_to_self()` / `all_involved_raw_types()` | done | |
| `JavaMember.Predicates.declaredIn(..)` | `rust_member::predicates::declared_in(..)` | done | |
| `JavaField` | `RustField` | done | Named/tuple field of struct, enum variant or union; associated `const` (modeled as a static field) |
| `JavaField.getType()` / `getRawType()` | `type_()` / `raw_type()` | done | Raw = generics erased, references/arrays unwrapped |
| `getAccessesToSelf()` | `accesses_to_self()` | done | |
| `JavaCodeUnit` | `RustCodeUnit` | done | Free functions, associated functions, trait methods, closures are **not** code units (they belong to their enclosing fn) |
| `getParameters()` / `getParameterTypes()` / `getRawParameterTypes()` | `parameters()` / `parameter_types()` / `raw_parameter_types()` | done | `self` receiver excluded |
| `getReturnType()` / `getRawReturnType()` | `return_type()` / `raw_return_type()` | done | `()` for none |
| `getThrowsClause()` / `getExceptionTypes()` | `error_types()` | partial | See §2.7 |
| `getCallsFromSelf()` / `getMethodCallsFromSelf()` / `getConstructorCallsFromSelf()` / `getFieldAccesses()` / `getAccessesFromSelf()` / references | same, snake_case | done | |
| `getCallsOfSelf()` | `calls_of_self()` | done | |
| `isMethod()` / `isConstructor()` | `is_method()` / `is_constructor()` | done | |
| `getParameterAnnotations()` | `parameter_annotations()` | done | |
| `JavaCodeUnit.Predicates.method()` / `constructor()` / `anyParameterThat` / `allParameters` | `rust_code_unit::predicates::{method, constructor, any_parameter_that, all_parameters}` | done | |
| `JavaMethod` | `RustMethod` | done | Any function with an owner impl/trait; free functions are `RustItem`s **and** exposed as `RustMethod` with a module owner so `methods()` rules cover them |
| `JavaMethod.getDefaultValue()` | — | unsupported | Annotation default values do not exist |
| `JavaConstructor` | `RustConstructor` | partial | Heuristic subset of `RustMethod` (see `getConstructors`) |
| `JavaStaticInitializer` | — | unsupported | |
| `JavaParameter` | `RustParameter` | done | |
| `JavaEnumConstant` | `RustVariant` | done | |
| `JavaAnnotation<OWNER>` | `RustAnnotation` | done | See §2.5 |
| `JavaType`, `JavaParameterizedType`, `JavaTypeVariable`, `JavaWildcardType`, `JavaGenericArrayType` | `RustType` enum: `Path`, `Reference`, `Slice`, `Array`, `Tuple`, `TraitObject`, `ImplTrait`, `TypeParam`, `Never`, `Infer`, `Ptr`, `FnPointer` | done | Raw erasure via `RustType::to_erasure()` |
| `JavaModifier` | `RustModifier` | done | See §2.3 |
| `Dependency` | `Dependency` | done | `origin_item()`, `target_item()`, `source_code_location()`, `description()`, `kind() [rust-only]` |
| `Dependency.Predicates.dependency(..)` / `dependencyOrigin(..)` / `dependencyTarget(..)` | `dependency::predicates::{dependency, dependency_origin, dependency_target}` | done | |
| `Dependency.Functions.GET_ORIGIN_CLASS` / `GET_TARGET_CLASS` | `dependency::functions::{get_origin_item, get_target_item}` | done | |
| `Dependency.toTargetClasses(..)` | `Dependency::to_target_items(..)` | done | |
| `Dependency.convertTo(Class)` | — | unsupported | Reflection-based; use `kind()` |
| `JavaAccess<T>` / `JavaFieldAccess` / `JavaCall` / `JavaMethodCall` / `JavaConstructorCall` / `JavaCodeUnitReference` / `JavaMethodReference` / `JavaConstructorReference` / `JavaCodeUnitAccess` | `RustAccess` enum with variants `FieldAccess`, `MethodCall`, `ConstructorCall`, `FunctionReference` | done | See §2.6 |
| `JavaAccess.getOrigin()` / `getOriginOwner()` / `getTarget()` / `getTargetOwner()` / `getLineNumber()` / `getSourceCodeLocation()` / `getDescription()` / `isDeclaredInLambda()` / `getContainingTryBlocks()` | `origin()` / `origin_owner()` / `target()` / `target_owner()` / `line_number()` / `source_code_location()` / `description()` / `is_declared_in_closure()` / — | done | try blocks unsupported |
| `JavaAccess.Predicates.origin(..)` / `originOwner(..)` / `target(..)` / `targetOwner(..)` / `originOwnerEqualsTargetOwner()` | `rust_access::predicates::{origin, origin_owner, target, target_owner, origin_owner_equals_target_owner}` | done | |
| `JavaFieldAccess.AccessType` (`GET`/`SET`) | `AccessType::{Get, Set}` | done | `Set` = assignment target or `&mut` borrow of the field |
| `AccessTarget` and subtypes (`FieldAccessTarget`, `MethodCallTarget`, `ConstructorCallTarget`, …) | `AccessTarget` enum | done | `resolve_member()` returns `Option`/set like Java: unresolved when the target could not be found in the import |
| `AccessTarget.Predicates.declaredIn(..)` / `constructor()` | `access_target::predicates::{declared_in, constructor}` | done | |
| `SourceCodeLocation` | `SourceCodeLocation` | partial | `source_file_name()`, `line_number()`, `Display` = `(src/x.rs:14)`; `getSourceClass()` is not provided (the location is a plain value) |
| `Source` | `Source` | partial | `uri()` → file path; `md5sum()` unsupported |
| `ThrowsClause` / `ThrowsDeclaration` | `ErrorTypes` / `ErrorTypeDeclaration` | partial | See §2.7 |
| `InstanceofCheck`, `TryCatchBlock`, `ReferencedClassObject` | — / — / `ReferencedTypeObject` | see above | |
| `PackageMatcher` / `PackageMatchers` | `PackageMatcher` / `PackageMatchers` | done | Same syntax on `::` separators. See §2.4 |
| `PackageMatcher.match(String)` → `Result.getGroup(int)` / `getNumberOfGroups()` | `match_(&str)` → `Option<MatchResult>`, `group(usize)`, `number_of_groups()` | done | |
| `Formatters` | `formatters` module | done | `format_method`, `format_method_simple`, `format_named_predicate` |
| `DomainObjectCreationContext`, `ImportContext`, `DomainPlugin`, `Java14DomainPlugin`, … | — | internal | Not public API |

#### `properties` interfaces

| Java (`core.domain.properties`) | Rust | Status |
|---|---|---|
| `HasName` / `HasName.AndFullName` | `trait HasName` / `trait HasFullName` | done |
| `HasName.Predicates.name/nameMatching/nameStartingWith/nameContaining/nameEndingWith` | `has_name::predicates::{name, name_matching, name_starting_with, name_containing, name_ending_with}` | done |
| `HasName.AndFullName.Predicates.fullName/fullNameMatching` | `has_full_name::predicates::{full_name, full_name_matching}` | done |
| `HasName.Functions.GET_NAME/GET_NAMES`, `namesOf(..)` | `has_name::functions::get_name`, `names_of(..)` | done |
| `HasModifiers` / `Predicates.modifier(..)` | `trait HasModifiers` / `has_modifiers::predicates::modifier(..)` | done |
| `CanBeAnnotated` / `Predicates.annotatedWith(..)` / `metaAnnotatedWith(..)` | `trait CanBeAnnotated` / `can_be_annotated::predicates::annotated_with(..)` / — | partial (meta unsupported) |
| `HasAnnotations` | `trait HasAnnotations` | done |
| `HasOwner` / `Predicates.With.owner(..)` / `Functions.Get.owner()` | `trait HasOwner` / `has_owner::predicates::owner(..)` / `has_owner::functions::owner()` | done |
| `HasParameterTypes` / `Predicates.rawParameterTypes(..)` | `trait HasParameterTypes` / `has_parameter_types::predicates::raw_parameter_types(..)` | done |
| `HasReturnType` / `Predicates.rawReturnType(..)` / `Functions.GET_RETURN_TYPE` | `trait HasReturnType` / `has_return_type::predicates::raw_return_type(..)` / `functions::get_return_type` | done |
| `HasType` / `Predicates.rawType(..)` / `Functions.GET_RAW_TYPE` | `trait HasType` / `has_type::predicates::raw_type(..)` / `functions::get_raw_type` | done |
| `HasThrowsClause` / `Predicates.throwsClauseWithTypes/throwsClauseContainingType/throwsClause` | `trait HasErrorTypes` / `has_error_types::predicates::{error_types, error_types_containing, error_clause}` | partial |
| `HasSourceCodeLocation` | `trait HasSourceCodeLocation` | done |
| `HasDescriptor` | — | unsupported (JVM descriptor) |
| `HasTypeParameters` / `HasUpperBounds` | `trait HasTypeParameters` / `trait HasBounds` | done |
| `CanOverrideDescription` | `trait CanOverrideDescription { fn as_(..) }` | done |

### 2.3 Modifiers

| `JavaModifier` | `RustModifier` | Note |
|---|---|---|
| `PUBLIC` | `Pub` | `pub` with no restriction |
| `PROTECTED` | `PubRestricted` | `pub(crate)`, `pub(super)`, `pub(in path)`: wider than the module, narrower than public. Nearest analog of "visible to package and subclasses" |
| `PRIVATE` | `Private` | Inherited (no `pub`). Visible in the declaring module and its descendants |
| (package-private, absence of a modifier) | `Private` | Java package-private and private both map to Rust private; `are_package_private()` and `are_private()` are therefore synonyms |
| `STATIC` | `Static` | Functions without a `self` receiver; associated consts; `static`/`const` items |
| `FINAL` | — | unsupported for classes and fields (no subclassing; fields have no `final`). For methods: inherent methods (cannot be overridden) are reported as final |
| `ABSTRACT` | `Abstract` | Trait methods without a default body; traits themselves |
| `SYNCHRONIZED`, `NATIVE`, `VOLATILE`, `TRANSIENT`, `BRIDGE`, `ENUM`, `SYNTHETIC` | — | unsupported (JVM-only). `NATIVE` ≈ `Extern` below |
| `[rust-only]` | `PubCrate`, `PubSuper`, `PubIn(path)` | Finer-grained than `PubRestricted` (which matches any of them) |
| `[rust-only]` | `Unsafe`, `Async`, `Const`, `Extern`, `Mut` (for `static mut`), `Default` (trait item with default body), `NonExhaustive` | |

### 2.4 Package identifiers → module identifiers

Same grammar as `PackageMatcher`, with `::` instead of `.` as the separator. A single `.` is
accepted as an alias for `::` (it can never occur in a Rust path), so identifiers copied from the
ArchUnit user guide such as `..adapter.(*)..` work verbatim:

| Pattern | Meaning |
|---|---|
| `*` | exactly one module segment |
| `..` | any number of segments, including zero |
| `(*)` / `(**)` | capturing variants |
| `[a\|b]` | alternation inside `[..]` or `(..)` |
| `crate` (as the first segment) | matches the crate root segment of **any** imported crate, so `crate::domain..` works regardless of the crate name. `crate` is a reserved word and can never be a real crate name |

Examples: `..service..`, `my_app::domain::(*)..`, `..adapter::[cli|rest]..`, `crate::(**)`.

Patterns are matched against the **module path** of an item, not its full name, exactly
as in Java.

### 2.5 Annotations → attributes

| Java | Rust | Note |
|---|---|---|
| `@Foo` on a class/member | `#[foo]` outer attribute | `RustAnnotation { path: "foo", kind: Attribute }` |
| `@Foo` where `Foo` is a derive macro | `#[derive(Foo)]` | Each derive path becomes one `RustAnnotation { kind: Derive }`. So `are_annotated_with("Serialize")` and `are_annotated_with("serde::Serialize")` both match `#[derive(Serialize)]` |
| `annotation.get("value")` / `getProperties()` | `get("key")` / `properties()` | Best-effort parse of `#[foo(key = "v", flag, path::x)]` and `#[foo = "v"]`; raw tokens via `tokens()` |
| `annotation.getRawType()` / `getType()` | `raw_type()` → `Option<&RustItem>` | Resolved if the macro definition is in the import, else a stub |
| `annotation.as(Class)` | — | unsupported (reflection) |
| meta-annotations (`@Retention`-style annotation on an annotation) | — | unsupported. Attribute macros are functions; their attributes are not inherited |
| `@Deprecated` | `#[deprecated]` | used by `DEPRECATED_API_SHOULD_NOT_BE_USED` |
| `package-info` annotations | inner attributes `#![..]` on the module | built-in attributes only |
| `#[cfg(..)]` / `#[cfg_attr(..)]` | recorded as annotations; `#[cfg(test)]` also flags the item as test code | |
| `@AnalyzeClasses` on a meta-annotation | — | unsupported (see §6) |

### 2.6 Accesses and dependencies

ArchUnit distinguishes an **access** (a code unit calling a method, calling a constructor, or
reading/writing a field) from the broader **dependency** (any reference between two classes).
The port keeps both notions.

| Java dependency source | Rust source | `Dependency` description verb |
|---|---|---|
| method call | `Type::func(..)`, `x.method(..)`, `func(..)` | `calls method` |
| constructor call | call to a constructor-classified function, `Foo { .. }`, `Foo(..)`, `Enum::Variant(..)` | `calls constructor`; literal targets are named `Foo { .. }`, `Foo(..)`, `Enum::Variant(..)` in place of Java's `<init>` |
| field access (get/set) | `x.field`, `x.field = ..`, `&mut x.field`, `Foo { field: .. }` | `gets field` / `sets field` |
| method/constructor reference | function path as value (`iter.map(Foo::new)`) | `references method` / `references constructor` |
| field type | struct/enum/union field type | `has type` |
| parameter type / return type | fn signature | `has parameter of type` / `has return type` |
| throws declaration | `Result<_, E>` error type | `throws type` |
| extends | supertrait | `extends trait` |
| implements | `impl Trait for Type`, and derives that resolve to traits | `implements trait` |
| is annotated with | attribute or derive | `is annotated with` |
| annotation member type | attribute argument path | `has annotation member of type` |
| type parameter bounds / generic signature | generic bounds, `where` clauses, type arguments | `has type parameter 'T' depending on` / `has generic .. with type argument depending on` |
| instanceof / class object | `TypeId::of::<T>`, turbofish | `references class object` |
| — | `use` import | `[rust-only]` `imports` (an unused import still counts; ArchUnit has no import dependency because bytecode has none) |
| — | type alias target, `const`/`static` type, `impl` self type | `has type` |
| — | path expressions naming a `const`, `static` or type (`let x: T`, `as T`, turbofish, patterns) | `[rust-only]` `references` |
| — | macro invocation arguments | Best effort: arguments of well-known macros (`println!`, `format!`, `vec!`, `assert!`, `write!`, `matches!`, `dbg!`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, and any macro whose input parses as comma-separated expressions) are parsed and visited. Every invocation yields a `[rust-only]` `invokes macro` dependency on the macro item (std macros resolve to `std::<name>`) |

Description format (identical to Java, with `Item` in place of `Class`):

```
Item <my_app::domain::order::Order> depends on <my_app::infrastructure::db::Pool> in (src/domain/order.rs:14)
Method <my_app::service::OrderService::place()> calls method <my_app::controller::Api::respond()> in (src/service.rs:42)
Field <my_app::domain::Order::pool> has type <my_app::infrastructure::db::Pool> in (src/domain/order.rs:9)
```

Resolution limits are listed in PLAN.md. Unresolvable targets become stub items named by
their best-known path so rules like `have_name_matching` still work on them.

### 2.7 Exceptions → error types

`throws E` has no direct counterpart. The port treats a function returning `Result<T, E>`
(or a type alias that resolves to one, e.g. `io::Result<T>`, `anyhow::Result<T>`) as
"declaring throwable of type `E`". `declare_throwable_of_type("std::io::Error")` therefore
matches `fn f() -> io::Result<()>`. Functions that can panic are not considered to throw;
`[rust-only]` conditions cover `panic!`/`unwrap` (see §5.5).

---

## 3. Base API (`com.tngtech.archunit.base`) → `archunit::base`

| Java | Rust | Status |
|---|---|---|
| `DescribedPredicate<T>` (abstract class) | `struct DescribedPredicate<T>` holding `Arc<dyn Fn(&T) -> bool + Send + Sync>` and a description | done |
| `test(T)` | `test(&T) -> bool` (also `impl Fn(&T) -> bool`) | done |
| `getDescription()` | `description()` | done |
| `as(String, Object...)` | `as_(impl Into<String>)` (use `format!` for args) | done |
| `and(..)` / `or(..)` / `negate()` | `and(..)` / `or(..)` / `negate()` | done |
| `onResultOf(Function)` | `on_result_of(impl Fn(&F) -> T)` (returns `DescribedPredicate<F>`) | done |
| `forSubtype()` | — | unsupported: no subtyping; not needed because predicates on `RustItem` apply directly. Provided as a no-op for source compatibility |
| `alwaysTrue()` / `alwaysFalse()` / `equalTo(..)` / `lessThan` / `greaterThan` / `lessThanOrEqualTo` / `greaterThanOrEqualTo` | same, snake_case | done |
| `describe(String, Predicate)` | `describe(&str, impl Fn(&T) -> bool)` | done |
| `doesNot(..)` / `doNot(..)` / `not(..)` | `does_not(..)` / `do_not(..)` / `not(..)` | done |
| static `and(..)`/`or(..)` over iterables | `all_of(..)` / `any_of(..)` **and** `and(..)`/`or(..)` free functions | done |
| `empty()` / `anyElementThat(..)` / `allElements(..)` / `optionalContains(..)` / `optionalEmpty()` | `empty()` / `any_element_that(..)` / `all_elements(..)` / `optional_contains(..)` / `optional_empty()` | done |
| `DescribedFunction` / `ChainableFunction` (`then`, `is`, `as`) | `DescribedFunction<F, T>` with `then(..)`, `is(pred)`, `as_(..)` | done |
| `DescribedIterable` | `DescribedIterable<T>` | done |
| `HasDescription` | `trait HasDescription` | done |
| `ArchUnitException` (+ subclasses) | `ArchUnitError` enum (`thiserror`) | done |
| `ForwardingCollection/List/Set`, `ClassLoaders`, `ReflectionUtils`, `Suppliers`, `Optionals`, `Predicates`, `MayResolveTypesViaReflection`, `ResolvesTypesViaReflection` | — | internal / JVM-specific |

---

## 4. Lang API (`com.tngtech.archunit.lang`) → `archunit::lang`

### 4.1 Rules, conditions, events

| Java | Rust | Status | Note |
|---|---|---|---|
| `ArchRule` (interface) | `trait ArchRule: HasDescription + Send + Sync` | done | Object safe; `Box<dyn ArchRule>` implements it |
| `check(JavaClasses)` | `check(&RustItems)` | done | Panics with the failure report (Rust's `AssertionError`) |
| `evaluate(JavaClasses)` | `evaluate(&RustItems) -> EvaluationResult` | done | |
| `because(String)` | `because(&str) -> Self` | done | Description: `.. because 'reason'` (Java: `", because " + reason`) |
| `as(String)` | `as_(&str) -> Self` | done | |
| `allowEmptyShould(boolean)` | `allow_empty_should(bool) -> Self` | done | |
| `getDescription()` | `description()` | done | |
| `ArchRule.Assertions.check(rule, classes)` / `assertNoViolation(result)` | `assertions::check(..)` / `assert_no_violation(..)` | done | |
| `ArchRule.Factory.create(transformer, condition, priority)` / `withBecause(..)` | `ArchRule::create(..)` / `with_because(..)` | done | |
| `ArchRule.Transformation` (`As`, `Because`) | `RuleTransformation` enum | done | |
| `CompositeArchRule.of(rule).and(rule)` / `priority(..)` | `CompositeArchRule::of(rule).and(rule)` / `priority(..)` | done | |
| `ArchCondition<T>` (abstract class: `init`, `check`, `finish`, `and`, `or`, `as`, `forSubtype`, `getDescription`) | `struct ArchCondition<T>` with `ArchCondition::new(desc, \|item, events\| ..)` for the common case and `ArchCondition::from_logic(desc, impl ConditionLogic<T>)` for stateful conditions (`init(&mut self, all)`, `check(&mut self, ..)`, `finish(&mut self, ..)`); `and`, `or`, `as_`, `never`, `not`, `for_subtype` | done | Conditions are `Clone` and share their logic behind a mutex, so one instance evaluated concurrently serialises |
| `ArchCondition.ConditionByPredicate` (`describeEventsBy`) | `ConditionByPredicate<T>` with `describe_events_by(..)` | done | |
| `ConditionEvents` / `ConditionEvent` / `SimpleConditionEvent` (`violated`, `satisfied`, `invert`, `getDescriptionLines`, `handleWith`) | `ConditionEvents` / `trait ConditionEvent` / `SimpleConditionEvent::{violated, satisfied}` … | done | |
| `ConditionEvents.setInformationAboutNumberOfViolations(..)` | `set_information_about_number_of_violations(..)` | done | used by cycle detection |
| `EvaluationResult` (`hasViolation`, `getFailureReport`, `getPriority`, `add`, `handleViolations`, `filterDescriptionsMatching`) | `EvaluationResult` with `has_violation()`, `failure_report()`, `priority()`, `add(..)`, `handle_violations(..)`, `filter_descriptions_matching(..)` | done | `handleViolations` takes a `ViolationHandler<T>` closure dispatched on `RustAccess`/`Dependency`/`RustItem` via an enum instead of reified generics |
| `FailureReport` (`isEmpty`, `getDetails`, `toString`) | `FailureReport` | done | |
| `FailureMessages` (`getInformationAboutNumberOfViolations`) | `FailureMessages` | done | |
| `FailureDisplayFormat` (+ `failureDisplayFormat` property) | `trait FailureDisplayFormat`; `DefaultFailureDisplayFormat`; set programmatically via `ArchConfiguration::set_failure_display_format(..)` | partial | Cannot be instantiated from a class name in a config file |
| `Priority` (`HIGH`, `MEDIUM`, `LOW`, `asString`) | `Priority::{High, Medium, Low}`, `as_string()` | done | |
| `ClassesTransformer<T>` / `AbstractClassesTransformer` | `struct ClassesTransformer<T>` created with `ClassesTransformer::new(desc, \|items\| ..)`; `that(pred)`, `as_(desc)` | done | One struct instead of interface + abstract class |
| `ViolationHandler<T>` | `trait ViolationHandler<T>` (+ closures); `EvaluationResult::handle_violations::<T>(..)` selects objects through `FromCorrespondingObject` (implemented for `RustItem`, `RustMember`, `RustAccess`, `Dependency`, `String`) | done | Java reifies `T` from the handler's generic signature; Rust needs the type in the closure signature |
| `CanBeEvaluated` | `trait CanBeEvaluated` | done | |
| `ArchUnitExtension` / `ArchUnitExtensions` / `EvaluatedRule` (ServiceLoader plug-ins) | `trait ArchUnitExtension`; registered with `ArchConfiguration::register_extension(..)` | deferred | No ServiceLoader; programmatic registration only |
| `archunit_ignore_patterns.txt` | `archunit_ignore_patterns.txt` in the crate root (or the path in `archunit.toml`) | done | Same semantics: one regex per line, `#` comments |
| Failure message | identical: `Architecture Violation [Priority: MEDIUM] - Rule 'DESC' was violated (N times):\n<details>` | done | |
| Empty-should failure | identical text, referencing `archunit.toml` key `arch_rule.fail_on_empty_should` | done | |

### 4.2 `ArchRuleDefinition` → `archunit::lang::syntax`

| Java | Rust | Status |
|---|---|---|
| `classes()` / `noClasses()` | `classes()` / `no_classes()` | done |
| `theClass(Class/String)` / `noClass(..)` | `the_class(&str)` / `no_class(&str)` | done |
| `members()` / `noMembers()` | `members()` / `no_members()` | done |
| `fields()` / `noFields()` | `fields()` / `no_fields()` | done |
| `codeUnits()` / `noCodeUnits()` | `code_units()` / `no_code_units()` | done |
| `constructors()` / `noConstructors()` | `constructors()` / `no_constructors()` | partial | Constructors are the heuristic subset of associated functions (see §2.2) |
| `methods()` / `noMethods()` | `methods()` / `no_methods()` | done |
| `all(ClassesTransformer)` / `no(ClassesTransformer)` | `all(transformer)` / `no(transformer)` | done |
| `priority(Priority).classes()` … | `priority(Priority::Low).classes()` … | done |
| `[rust-only]` | `modules()` / `no_modules()`, `traits()` / `no_traits()`, `functions()` / `no_functions()` | done |

### 4.3 `GivenClasses` / `GivenClassesConjunction` / `GivenObjects` / `GivenConjunction`

| Java | Rust | Status |
|---|---|---|
| `that()` | `that()` → `ClassesThat<GivenClassesConjunction>` | done |
| `that(DescribedPredicate)` | `that_with(pred)`; also `that().satisfy(pred)` | done |
| `should()` | `should()` → `ClassesShould` | done |
| `should(ArchCondition)` | `should_with(cond)`; also `should().satisfy(cond)` | done |
| `and()` / `or()` | `and()` / `or()` | done |
| `and(pred)` / `or(pred)` | `and_with(pred)` / `or_with(pred)` | done |
| `GivenClass.should()` / `should(cond)` | `should()` / `should_with(cond)` | done |
| `GivenObjects<T>.that(pred)` / `should(cond)` | `that(pred)` / `should(cond)` (no zero-arg clash, so no suffix) | done |

### 4.4 `ClassesThat<CONJUNCTION>` (predicates)

All return `CONJUNCTION`. Rust: `ClassesThat<C>` returns `C`.

| Java | Rust | Status | Note |
|---|---|---|---|
| `haveFullyQualifiedName` / `doNotHaveFullyQualifiedName` | `have_fully_qualified_name` / `do_not_have_fully_qualified_name` | done | |
| `haveSimpleName` / `doNotHaveSimpleName` | `have_simple_name` / `do_not_have_simple_name` | done | |
| `haveNameMatching` / `haveNameNotMatching` | `have_name_matching` / `have_name_not_matching` | done | `regex` crate syntax |
| `haveSimpleNameStartingWith/NotStartingWith/Containing/NotContaining/EndingWith/NotEndingWith` | same, snake_case | done | |
| `resideInAPackage` / `resideInAnyPackage` / `resideOutsideOfPackage` / `resideOutsideOfPackages` | same, snake_case | done | |
| `arePublic` / `areNotPublic` | `are_public` / `are_not_public` | done | `pub` |
| `areProtected` / `areNotProtected` | `are_protected` / `are_not_protected` | done | restricted visibility |
| `arePackagePrivate` / `areNotPackagePrivate` | `are_package_private` / `are_not_package_private` | done | private |
| `arePrivate` / `areNotPrivate` | `are_private` / `are_not_private` | done | private (synonym) |
| `haveModifier` / `doNotHaveModifier` | `have_modifier(RustModifier)` / `do_not_have_modifier` | done | |
| `areAnnotatedWith(Class/String/pred)` / `areNotAnnotatedWith` | `are_annotated_with(impl Into<AnnotationSelector>)` / `are_not_annotated_with` | done | |
| `areMetaAnnotatedWith` / `areNotMetaAnnotatedWith` (3 overloads) | — | unsupported | no meta-annotations |
| `implement(Class/String/pred)` / `doNotImplement` | `implement(impl Into<ItemSelector>)` / `do_not_implement` | done | |
| `areAssignableTo` / `areNotAssignableTo` / `areAssignableFrom` / `areNotAssignableFrom` | same, snake_case | done | |
| `areInterfaces` / `areNotInterfaces` | `are_interfaces` / `are_not_interfaces` | done | traits; `are_traits` is an alias `[rust-only]` |
| `areEnums` / `areNotEnums` | `are_enums` / `are_not_enums` | done | |
| `areAnnotations` / `areNotAnnotations` | `are_annotations` / `are_not_annotations` | done | proc-macro attribute/derive definitions |
| `areRecords` / `areNotRecords` | — | unsupported | no records; use `are_structs` |
| `areTopLevelClasses` / `areNotTopLevelClasses` | `are_top_level_classes` / `are_not_top_level_classes` | done | |
| `areNestedClasses` / `areNotNestedClasses` / `areLocalClasses` / `areNotLocalClasses` | same, snake_case | done | both = declared inside a body |
| `areMemberClasses` / `areInnerClasses` / `areAnonymousClasses` (+ negations) | — | unsupported | |
| `belongToAnyOf(Class...)` / `doNotBelongToAnyOf` | `belong_to_any_of(&[&str])` / `do_not_belong_to_any_of` | done | "belong to" = is the item or is declared inside it |
| `containAnyMembersThat` / `containAnyFieldsThat` / `containAnyCodeUnitsThat` / `containAnyMethodsThat` / `containAnyConstructorsThat` | same, snake_case | done | |
| `containAnyStaticInitializersThat` | — | unsupported | |
| `[rust-only]` | `are_structs`, `are_unions`, `are_functions`, `are_modules`, `are_type_aliases`, `are_consts`, `are_statics`, `are_macros`, `are_unsafe`, `are_async`, `are_const_fns`, `are_pub_crate`, `are_test_code`, `reside_in_crate(&str)` | done | |

### 4.5 `ClassesShould` (conditions) and `ClassesShouldConjunction`

| Java | Rust | Status | Note |
|---|---|---|---|
| name conditions (`haveFullyQualifiedName`, `notHaveFullyQualifiedName`, `haveSimpleName`, `notHaveSimpleName`, `haveSimpleName{Not}StartingWith/Containing/EndingWith`, `haveName{Not}Matching`) | same, snake_case | done | |
| `resideInAPackage` / `resideInAnyPackage` / `resideOutsideOfPackage` / `resideOutsideOfPackages` | same | done | |
| `bePublic` / `notBePublic` / `beProtected` / `notBeProtected` / `bePackagePrivate` / `notBePackagePrivate` / `bePrivate` / `notBePrivate` | same | done | see §2.3 |
| `haveOnlyFinalFields()` | — | unsupported | Rust fields have no `final`; `[rust-only]` `have_only_private_fields()` is the useful analog (immutability from outside the module) |
| `haveOnlyPrivateConstructors()` | `have_only_private_constructors()` | partial | True when the type cannot be constructed outside its module: it has a private field or is `#[non_exhaustive]`, and every constructor-classified fn is private |
| `haveModifier` / `notHaveModifier` | same | done | |
| `beAnnotatedWith` / `notBeAnnotatedWith` (3 overloads) | `be_annotated_with(impl Into<AnnotationSelector>)` / `not_be_annotated_with` | done | |
| `beMetaAnnotatedWith` / `notBeMetaAnnotatedWith` | — | unsupported | |
| `implement` / `notImplement` (3 overloads) | `implement(..)` / `not_implement(..)` | done | |
| `beAssignableTo` / `notBeAssignableTo` / `beAssignableFrom` / `notBeAssignableFrom` | same | done | |
| `accessField(owner, name)` / `accessFieldWhere(pred)` / `onlyAccessFieldsThat(pred)` | `access_field(&str, &str)` / `access_field_where(pred)` / `only_access_fields_that(pred)` | done | |
| `getField` / `getFieldWhere` / `setField` / `setFieldWhere` | same | done | |
| `callMethod(owner, name, params...)` / `callMethodWhere` / `onlyCallMethodsThat` | `call_method(&str, &str, &[&str])` / `call_method_where` / `only_call_methods_that` | done | |
| `callConstructor(owner, params...)` / `callConstructorWhere` / `onlyCallConstructorsThat` | `call_constructor(&str, &[&str])` / … | partial | Described as `call constructor Owner(p1, p2)`; parameter types are only compared when the target resolves to an imported member (also for `call_method`) |
| `callCodeUnitWhere` / `onlyCallCodeUnitsThat` | same | done | |
| `accessTargetWhere` / `onlyAccessMembersThat` | same | done | |
| `accessClassesThat()` / `accessClassesThat(pred)` | `access_classes_that()` / `access_classes_that_with(pred)` | done | |
| `onlyAccessClassesThat()` / `(pred)` | `only_access_classes_that()` / `_with` | done | |
| `dependOnClassesThat()` / `(pred)` | `depend_on_classes_that()` / `_with` | done | |
| `onlyDependOnClassesThat()` / `(pred)` | `only_depend_on_classes_that()` / `_with` | done | |
| `transitivelyDependOnClassesThat()` / `(pred)` | `transitively_depend_on_classes_that()` / `_with` | done | |
| `onlyBeAccessed()` → `OnlyBeAccessedSpecification` | `only_be_accessed()` | done | |
| `OnlyBeAccessedSpecification.byAnyPackage(..)` / `byClassesThat()` / `byClassesThat(pred)` | `by_any_package(&[&str])` / `by_classes_that()` / `by_classes_that_with(pred)` | done | |
| `onlyHaveDependentClassesThat()` / `(pred)` | `only_have_dependent_classes_that()` / `_with` | done | |
| `beInterfaces` / `notBeInterfaces` / `beEnums` / `notBeEnums` | same | done | |
| `beRecords` / `notBeRecords` | — | unsupported | |
| `beTopLevelClasses` / `notBe..` / `beNestedClasses` / `notBe..` / `beLocalClasses` / `notBe..` | same | done | |
| `beMemberClasses` / `beInnerClasses` / `beAnonymousClasses` (+ negations) | — | unsupported | |
| `be(Class/String)` / `notBe(..)` | `be(&str)` / `not_be(&str)` | done | |
| `containNumberOfElements(pred)` | `contain_number_of_elements(pred)` | done | |
| `andShould()` / `andShould(cond)` / `orShould()` / `orShould(cond)` | `and_should()` / `and_should_with(cond)` / `or_should()` / `or_should_with(cond)` | done | |
| `[rust-only]` | `be_structs`, `be_traits`, `be_functions`, `be_modules`, `be_unsafe`, `not_be_unsafe`, `be_async`, `be_pub_crate`, `have_only_private_fields`, `reside_in_crate` | done | |

### 4.6 Members: `GivenMembers`, `MembersThat`, `MembersShould`, `FieldsThat/Should`, `CodeUnitsThat/Should`, `MethodsThat/Should`, `OnlyBeCalledSpecification`

| Java | Rust | Status | Note |
|---|---|---|---|
| `GivenMembers.that()` / `that(pred)` / `should()` / `should(cond)`; conjunction `and()`/`or()`/`and(pred)`/`or(pred)` | same convention as classes (`_with` for object overloads) | done | |
| `MembersThat.haveName` / `doNotHaveName` / `haveNameMatching` / `haveNameNotMatching` / `haveFullName` / `doNotHaveFullName` / `haveFullName{Not}Matching` / `haveName{Not}StartingWith/Containing/EndingWith` | same, snake_case | done | |
| `MembersThat.arePublic/areNotPublic/areProtected/…/arePrivate/areNotPrivate` | same | done | |
| `haveModifier` / `doNotHaveModifier` | same | done | |
| `areAnnotatedWith` / `areNotAnnotatedWith` (3 overloads) | `are_annotated_with(..)` / `are_not_annotated_with(..)` | done | |
| `areMetaAnnotatedWith` / `areNotMetaAnnotatedWith` | — | unsupported | |
| `areDeclaredIn(Class/String)` / `areNotDeclaredIn` | `are_declared_in(&str)` / `are_not_declared_in(&str)` | done | |
| `areDeclaredInClassesThat(pred)` / `areDeclaredInClassesThat()` | `are_declared_in_classes_that_with(pred)` / `are_declared_in_classes_that()` | done | |
| `MembersShould.haveName/notHaveName/…` (mirror of `MembersThat`) | same | done | |
| `MembersShould.bePublic/…/bePrivate` and negations | same | done | |
| `MembersShould.beAnnotatedWith/notBeAnnotatedWith` | same | done | |
| `beMetaAnnotatedWith/notBeMetaAnnotatedWith` | — | unsupported | |
| `beDeclaredIn` / `notBeDeclaredIn` / `beDeclaredInClassesThat()` / `(pred)` | `be_declared_in` / `not_be_declared_in` / `be_declared_in_classes_that()` / `_with` | done | |
| `containNumberOfElements` | same | done | |
| `MembersShouldConjunction.andShould()/(cond)/orShould()/(cond)` | `and_should()` / `and_should_with` / `or_should()` / `or_should_with` | done | |
| `FieldsThat.haveRawType(Class/String/pred)` / `doNotHaveRawType` | `have_raw_type(impl Into<ItemSelector>)` / `do_not_have_raw_type` | done | |
| `FieldsThat.areStatic/areNotStatic` | `are_static` / `are_not_static` | done | associated consts and `static`/`const` items |
| `FieldsThat.areFinal/areNotFinal` | — | unsupported | Rust fields have no `final` |
| `FieldsShould.haveRawType/notHaveRawType` | same | done | |
| `FieldsShould.beAccessedByMethodsThat(pred)` / `notBeAccessedByMethodsThat` | same | done | |
| `FieldsShould.beStatic/notBeStatic` | same | done | |
| `FieldsShould.beFinal/notBeFinal` | — | unsupported | |
| `CodeUnitsThat.haveRawParameterTypes(Class.../String.../pred)` / `doNotHaveRawParameterTypes` | `have_raw_parameter_types(&[&str])` / `have_raw_parameter_types_with(pred)` / negations | done | |
| `CodeUnitsThat.haveRawReturnType(..)` / `doNotHaveRawReturnType` | `have_raw_return_type(impl Into<ItemSelector>)` / negation | done | |
| `CodeUnitsThat.declareThrowableOfType(..)` / `doNotDeclareThrowableOfType` | `declare_throwable_of_type(..)` / negation | partial | `Result<_, E>` |
| `CodeUnitsShould.haveRawParameterTypes` / `haveRawReturnType` / `declareThrowableOfType` (+ `not` forms) | same | done | |
| `CodeUnitsShould.onlyBeCalled()` → `OnlyBeCalledSpecification.byClassesThat(pred)/byClassesThat()/byCodeUnitsThat/byMethodsThat/byConstructorsThat` | `only_be_called()` → `by_classes_that_with(pred)` / `by_classes_that()` / `by_code_units_that(pred)` / `by_methods_that(pred)` / `by_constructors_that(pred)` | done | |
| `MethodsThat.areStatic/areNotStatic` | same | done | no `self` receiver |
| `MethodsThat.areFinal/areNotFinal` | same | partial | inherent (non-overridable) methods |
| `MethodsShould.beStatic/notBeStatic/beFinal/notBeFinal` | same | partial | |
| `[rust-only]` (methods/code units) | `are_unsafe`, `are_async`, `are_const`, `have_self_receiver`, `take_self_by_value`, `take_self_by_ref`, `take_self_by_mut_ref`, `are_trait_methods`, `are_default_methods`, `be_unsafe`, `not_be_unsafe`, … | done | |

### 4.7 `ArchConditions` → `archunit::lang::conditions`

Every static factory in `ArchConditions` maps 1:1 with the naming rules of §1. The table lists
only entries that deviate.

| Java | Rust | Status | Note |
|---|---|---|---|
| all `getField/setField/accessField[Where]`, `callMethod[Where]`, `callConstructor[Where]`, `callCodeUnitWhere`, `only*`, `accessClassesThat`, `onlyAccessClassesThat`, `dependOnClassesThat`, `haveAnyDependenciesThat`, `transitivelyDependOnClassesThat`, `onlyDependOnClassesThat`, `onlyBeAccessedByClassesThat`, `accessClassesThatResideIn[AnyPackage]`, `onlyBeAccessedByAnyPackage`, `onlyHaveDependentsInAnyPackage`, `onlyHaveDependentClassesThat`, `onlyHaveDependentsWhere`, `onlyHaveDependenciesInAnyPackage`, `onlyHaveDependenciesWhere` | same, snake_case | done | |
| `and(a, b)` / `or(a, b)` / `never(c)` / `not(c)` | same | done | |
| `be(Class/String)` / `notBe(String)` | `be_class(&str)` / `not_be_class(&str)` | done | Renamed because `be(predicate)` takes the generic overload; the DSL keeps `should().be(name)` |
| name conditions / package conditions / visibility conditions / `haveModifier` / `beAnnotatedWith` / `implement` / `beAssignableTo/From` / `beInterfaces` / `beEnums` / `beTopLevelClasses` / `beNestedClasses` / `beLocalClasses` | same | done | Event texts follow ArchUnit (`is an interface` / `is no interface`, `has simple name ..` / `does not have simple name ..`) |
| `beMetaAnnotatedWith`, `beRecords`, `beMemberClasses`, `beInnerClasses`, `beAnonymousClasses`, `haveOnlyFinalFields` | — | unsupported | see §4.5 |
| `haveOnlyPrivateConstructors` | same | partial | |
| `containNumberOfElements(pred)` | same | done | |
| `beDeclaredIn` / `notBeDeclaredIn` / `beDeclaredInClassesThat` | same | done | |
| `haveRawType` / `haveRawParameterTypes` / `haveRawReturnType` / `declareThrowableOfType` | same | done | |
| `onlyBeCalledByClassesThat` / `..ByCodeUnitsThat` / `..ByMethodsThat` / `..ByConstructorsThat` / `beAccessedByMethodsThat` | same | done | |
| `have(pred)` / `be(pred)` (`ConditionByPredicate`) | `have(pred)` / `be(pred)` | done | |
| `AllDependenciesCondition.ignoreDependency(..)` (returned by `onlyHaveDependencies*`) | `AllDependenciesCondition::ignore_dependency(..)` | done | |
| `[rust-only]` | `use_unsafe_blocks`, `call_function(&str)`, `invoke_macro(&str)`, `access_standard_streams`, `panic`, `call_unwrap`, `call_process_exit`, `return_generic_errors` | partial | see §5.5 |

### 4.8 `ArchPredicates`

| Java | Rust | Status |
|---|---|---|
| `is(pred)` / `are(pred)` / `has(pred)` / `have(pred)` / `be(pred)` | `archunit::lang::conditions::predicates::{is, are, has, have, be}` | done |

### 4.9 Rule text generation

Rule descriptions are assembled exactly as ArchUnit does:

```
[no ]classes that <pred> [and|or <pred>]* should <cond> [and|or should <cond>]*[, because '<reason>'][ as '<desc>']
```

Examples the tests must reproduce verbatim:

* `classes that reside in a package '..service..' should only be accessed by any package ['..controller..', '..service..']`
* `no classes that reside in a package '..service..' should access classes that reside in a package '..controller..'`
* `methods that are public and are declared in classes that reside in a package '..controller..' should be annotated with @Secured`

---

## 5. Library API (`com.tngtech.archunit.library`) → `archunit::library`

### 5.1 `Architectures`

| Java | Rust | Status | Note |
|---|---|---|---|
| `Architectures.layeredArchitecture()` → `DependencySettings` | `Architectures::layered_architecture()` / `layered_architecture()` | done | |
| `.consideringAllDependencies()` / `.consideringOnlyDependenciesInLayers()` / `.consideringOnlyDependenciesInAnyPackage(..)` | same | done | `considering_only_dependencies_in_any_package(&[&str])` takes a slice instead of the varargs split |
| `LayeredArchitecture.layer(name)` / `optionalLayer(name)` → `LayerDefinition.definedBy(String...)` / `definedBy(pred)` | `layer(&str)` / `optional_layer(&str)` → `defined_by(&[&str])` / `defined_by_with(pred)` | done | |
| `withOptionalLayers(bool)` | same | done | |
| `whereLayer(name)` → `LayerDependencySpecification.mayNotBeAccessedByAnyLayer()` / `mayOnlyBeAccessedByLayers(..)` / `mayNotAccessAnyLayer()` / `mayOnlyAccessLayers(..)` | same | done | |
| `ignoreDependency(Class, Class)` / `(String, String)` / `(pred, pred)` | `ignore_dependency(origin, target)` with `impl Into<ItemSelector>` (names or predicates) | done | `ignore_dependency_where(DescribedPredicate<Dependency>)` is `[rust-only]` |
| `ensureAllClassesAreContainedInArchitecture()` / `..Ignoring(String...)` / `..Ignoring(pred)` | same (`_with` for pred) | done | |
| `as(..)` / `because(..)` / `allowEmptyShould(..)` / `check` / `evaluate` / `getDescription` | same | done | `because` returns `Box<dyn ArchRule>` as in Java; `as_`/`allow_empty_should` keep the concrete type |
| description text | identical: `Layered architecture considering all dependencies, consisting of\nlayer 'X' ('..x..')\nwhere layer 'X' may only be accessed by layers ['Y']` | done | `Class <x> is not contained in architecture` becomes `Item <x> is not contained in architecture` |
| `Architectures.onionArchitecture()` | `Architectures::onion_architecture()` | done | |
| `.domainModels(..)` / `.domainServices(..)` / `.applicationServices(..)` / `.adapter(name, ..)` (String... and pred overloads) | same (`_with` for pred) | done | |
| `withOptionalLayers`, `ignoreDependency` (3), `ensureAllClassesAreContainedInArchitecture[Ignoring]`, `as`, `because`, `allowEmptyShould`, `check`, `evaluate`, `getDescription` | same | done | `layered_architecture_delegate()` exposes the equivalent `LayeredArchitecture` |

### 5.2 Slices (`library.dependencies`)

| Java | Rust | Status |
|---|---|---|
| `SlicesRuleDefinition.slices()` → `Creator.matching(pattern[, priority])` / `assignedFrom(SliceAssignment[, priority])` | `SlicesRuleDefinition::slices().matching(..)` / `assigned_from(..)` (+ `_with_priority`); free fn `slices()` | done |
| `GivenSlices.namingSlices(pattern)` / `as(..)` / `that(pred)` / `should()` | `naming_slices(&str)` / `as_(..)` / `that(pred)` / `should()` | done | `should_with(cond)` for a custom `ArchCondition<Slice>` (Java `should(condition)`) |
| `GivenSlicesConjunction.and(pred)` / `or(pred)` / `as` / `should()` | same | done |
| `SlicesShould.beFreeOfCycles()` / `notDependOnEachOther()` | `be_free_of_cycles()` / `not_depend_on_each_other()` | done |
| `SliceRule.ignoreDependency(Class,Class)/(String,String)/(pred,pred)` / `as` / `because` / `allowEmptyShould` / `check` / `evaluate` | `ignore_dependency(origin, target)` with `impl Into<ItemSelector>`; rest same | done | `ignore_dependency_where(DescribedPredicate<Dependency>)` is `[rust-only]` |
| `Slices`, `Slice` (`getNamePart(i)`, `getDependenciesFromSelf/ToSelf`, `as`), `SliceDependency`, `SliceAssignment`, `SliceIdentifier.of(..)/ignore()` | same; `SliceAssignment` is a trait, `slice_assignment(desc, closure)` builds one; `Slices::matching(..)`/`assigned_from(..)` return `SlicesTransformer` (`Slices.Transformer`) | done |
| cycle report text (`Cycle detected: Slice a -> \n                Slice b -> ...` + numbered dependency details) | identical | done |
| `cycles.maxNumberToDetect` / `cycles.maxNumberOfDependenciesPerEdge` | `archunit.toml` `[cycles] max_number_to_detect`, `max_number_of_dependencies_per_edge`; `ArchConfiguration::set_property("cycles.max_number_to_detect", ..)` | done | Same defaults (100 / 20) and the same `>= N times - the maximum number of cycles to detect has been reached ...` hint |
| `CycleDetector.detectCycles(nodes, edges)`, `Cycle`, `Cycles`, `Edge` | `cycle_detection::{CycleDetector, Cycle, Cycles, Edge, SimpleEdge}`; `detect_cycles_with_limit(..)` for an explicit maximum | done |

### 5.3 Modules (`library.modules`)

| Java | Rust | Status | Note |
|---|---|---|---|
| `ModuleRuleDefinition.modules().definedByPackages(pattern)` / `.derivingNameFromPattern(..)` | `modules().defined_by_packages(..)` / `deriving_name_from_pattern(..)` | deferred | Shares the slices infrastructure (`SlicesTransformer`, `CycleArchCondition`) |
| `.definedBy(identifierFunction)` / `.derivingModule(descriptorFunction)` / `derivingModuleFromRootClassBy(..)` | `defined_by(fn)` / `deriving_module(fn)` / `deriving_module_from_root_item_by(fn)` | deferred | |
| `.definedByAnnotation(A)` / `definedByAnnotation(A, nameFunction)` and `GivenModulesByAnnotation` | — | unsupported | Custom inner attributes on modules are unstable in Rust, so a module cannot carry `#![app_module(..)]`. Workaround documented: `defined_by_root_item` with a marker `const`/`struct` carrying an outer attribute |
| `ModulesShould.beFreeOfCycles()` / `respectTheirAllowedDependencies(..)` / `respectTheirAllowedDependenciesDeclaredIn(..)` / `onlyDependOnEachOtherThroughClassesThat(..)` / `onlyDependOnEachOtherThroughPackagesDeclaredIn(..)` / `notDependOnEachOther()` | same, snake_case (the `DeclaredIn` annotation-based ones unsupported) | deferred | |
| `ArchModules.defineByPackages/defineByRootClasses/defineBy/modularize`, `ArchModule`, `ModuleDependency`, `ModuleDependencyScope` | same | deferred | |

### 5.4 `GeneralCodingRules`, `DependencyRules`, `ProxyRules`

| Java | Rust | Status | Note |
|---|---|---|---|
| `ACCESS_STANDARD_STREAMS` / `NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS` | `ACCESS_STANDARD_STREAMS` / `NO_CLASSES_SHOULD_ACCESS_STANDARD_STREAMS` | P5 | Invocations of `print!`, `println!`, `eprint!`, `eprintln!`, `dbg!`, and calls to `std::io::stdout/stderr/stdin`. Applies to library and test code; binary targets are excluded by default because printing is their job (configurable) |
| `THROW_GENERIC_EXCEPTIONS` / `NO_CLASSES_SHOULD_THROW_GENERIC_EXCEPTIONS` | `THROW_GENERIC_EXCEPTIONS` / `NO_CLASSES_SHOULD_THROW_GENERIC_EXCEPTIONS` | P5 | "Generic exception" ≙ generic error type in a `Result`: `Box<dyn Error>`, `anyhow::Error`/`eyre::Report`, `String`, `&str`, `()`. Name kept; doc explains |
| `USE_JAVA_UTIL_LOGGING` / `NO_CLASSES_SHOULD_USE_JAVA_UTIL_LOGGING` | — | unsupported | Rust's standard library has no logging facade to discourage. Nearest custom rule: `no_classes().should().depend_on_classes_that().reside_in_a_package("log..")` |
| `USE_JODATIME` / `NO_CLASSES_SHOULD_USE_JODATIME` | — | unsupported | No legacy date/time crate to steer away from |
| `BE_ANNOTATED_WITH_AN_INJECTION_ANNOTATION` / `NO_CLASSES_SHOULD_USE_FIELD_INJECTION` | — | unsupported | No field-injection annotations in mainstream Rust DI |
| `ASSERTIONS_SHOULD_HAVE_DETAIL_MESSAGE` | `ASSERTIONS_SHOULD_HAVE_DETAIL_MESSAGE` | P5 | `assert!`/`assert_eq!`/`assert_ne!`/`debug_assert*!` invoked without a message argument |
| `DEPRECATED_API_SHOULD_NOT_BE_USED` | `DEPRECATED_API_SHOULD_NOT_BE_USED` | P5 | `#[deprecated]` targets within the import |
| `OLD_DATE_AND_TIME_CLASSES_SHOULD_NOT_BE_USED` | — | unsupported | No analog |
| `testClassesShouldResideInTheSamePackageAsImplementation([suffix])` | — | unsupported | Rust unit tests already live in the implementation module; integration tests in `tests/` are separate by design |
| `[rust-only]` | `NO_CLASSES_SHOULD_CALL_UNWRAP` (`.unwrap()`/`.expect()` outside test code), `NO_CLASSES_SHOULD_PANIC` (`panic!`, `unreachable!`, `todo!`, `unimplemented!`), `NO_LIBRARY_CODE_SHOULD_CALL_PROCESS_EXIT` (`std::process::exit` outside binary targets), `NO_CLASSES_SHOULD_USE_UNSAFE`, `NO_CLASSES_SHOULD_USE_PRINTLN_OUTSIDE_BINARIES` (alias of the standard-streams rule scoped as requested) | P5 | Each has the matching `ArchCondition` constant (`CALL_UNWRAP`, `PANIC`, `CALL_PROCESS_EXIT`, `USE_UNSAFE`) |
| `DependencyRules.NO_CLASSES_SHOULD_DEPEND_UPPER_PACKAGES` / `dependOnUpperPackages()` | `NO_CLASSES_SHOULD_DEPEND_UPPER_PACKAGES` / `depend_on_upper_packages()` | P5 | `super::` and ancestor-module dependencies |
| `ProxyRules.no_classes_should_directly_call_other_methods_declared_in_the_same_class_that_are_annotated_with(A)` / `..that(pred)` / `directly_call_other_methods_declared_in_the_same_class_that[_are_annotated_with](..)` | same names | P5 | `self.other()` calls to methods carrying an attribute (`#[tracing::instrument]`, `#[cached]`, …) |

### 5.5 PlantUML (`library.plantuml.rules`)

| Java | Rust | Status |
|---|---|---|
| `PlantUmlArchCondition.adhereToPlantUmlDiagram(URL/File/Path/String, Configuration)` | `adhere_to_plant_uml_diagram(impl AsRef<Path>, Configuration)` (+ `_from_str` for in-memory diagrams) | P5 |
| `Configuration.consideringAllDependencies()` / `consideringOnlyDependenciesInDiagram()` / `consideringOnlyDependenciesInAnyPackage(..)` | same | P5 |
| `ignoreDependenciesWithOrigin(pred)` / `ignoreDependenciesWithTarget(pred)` / `ignoreDependencies(Class,Class)/(String,String)/(pred)` | same (`_with` for the pred overload of `ignore_dependencies`) | P5 |
| Diagram grammar: `[Component] <<..stereo..>> as alias #color`, arrows `-->`, `<--`, `-[#c]->`, `-up->`, any dash count, labels `: text`, comments `'`, `note` lines ignored | identical, stereotypes are module identifiers (`<<..adapter::rest..>>`) | P5 |
| `PlantUmlParseException`, `IllegalDiagramException` | `PlantUmlError::{Parse, IllegalDiagram}` | P5 |

### 5.6 Freezing (`library.freeze`)

| Java | Rust | Status | Note |
|---|---|---|---|
| `FreezingArchRule.freeze(rule)` | `FreezingArchRule::freeze(rule)` / `freeze(rule)` | P5 | |
| `persistIn(ViolationStore)` / `associateViolationLinesVia(ViolationLineMatcher)` | `persist_in(..)` / `associate_violation_lines_via(..)` | P5 | |
| `as` / `because` / `allowEmptyShould` / `check` / `evaluate` / `getDescription` | same | P5 | |
| `ViolationStore` (`initialize(Properties)`, `contains(rule)`, `save(rule, violations)`, `getViolations(rule)`) | `trait ViolationStore` with `initialize(&Properties)`, `contains`, `save`, `violations` | P5 | |
| `TextFileBasedViolationStore` (`stored.rules` index in Java `.properties` format + one UUID-named file per rule) | `TextFileBasedViolationStore`, byte-compatible file format so stores can be shared with a Java project's conventions | P5 | |
| `ViolationStoreFactory` (`freeze.store` class name) | `ArchConfiguration::set_violation_store(..)` | P5 partial | no class-name instantiation |
| `ViolationLineMatcher` + default fuzzy matcher (ignores line numbers and `$N` suffixes) | `trait ViolationLineMatcher` + `FuzzyViolationLineMatcher` (ignores `:N)` line numbers and closure/`{{closure}}` numbering) | P5 | |
| `freeze.store.default.path` / `allowStoreCreation` / `allowStoreUpdate` / `freeze.refreeze` / `freeze.lineMatcher` | `archunit.toml` `[freeze] store.default.path`, `store.default.allow_store_creation`, `store.default.allow_store_update`, `refreeze`; `line_matcher` unsupported by name | P5 | |
| `StoreInitializationFailedException`, `StoreReadException`, `StoreUpdateFailedException`, `ViolationLineMatcherInitializationFailedException` | `FreezeError` variants | P5 | |

### 5.7 Metrics (`library.metrics`)

| Java | Rust | Status |
|---|---|---|
| `MetricsComponent`, `MetricsComponents.from(..)/fromPackages(..)/fromClasses(..)`, `MetricsComponentDependencyGraph` | same | deferred |
| `ArchitectureMetrics.lakosMetrics(..)` → `LakosMetrics` (`CCD`, `ACD`, `RACD`, `NCCD`) | same | deferred |
| `ArchitectureMetrics.componentDependencyMetrics(..)` → `ComponentDependencyMetrics` (`Ce`, `Ca`, `I`, `A`, `D`) | same; abstractness counts `pub` traits over `pub` items | deferred |
| `ArchitectureMetrics.visibilityMetrics(..)` → `VisibilityMetrics` (`RV`, `ARV`, `GRV`) | same; visible = `pub` | deferred |

---

## 6. JUnit support (`archunit-junit5`) → `archunit::harness` + `archunit-macros`

The concept is "test-framework integration". JUnit is Java-specific, so the module is
named `harness`; every annotation keeps its ArchUnit name.

| Java | Rust | Status | Note |
|---|---|---|---|
| `@AnalyzeClasses(packages, packagesOf, classes, locations, wholeClasspath, importOptions, cacheMode)` on a test class | `#[analyze_classes(packages = [..], packages_of = [..], items = [..], locations = [ProviderType, ..], whole_workspace = true, import_options = [DoNotIncludeTests, ..], cache_mode = Forever \| PerClass)]` on an inline `mod` | done | `wholeClasspath` → `whole_workspace`, `classes` → `items` (`classes` is accepted too). Without arguments the crate containing the test module is analyzed; declared `locations` replace it as the import root (Java: the union of declared locations). `import_options` entries are expressions (unit structs like `DoNotIncludeTests`, or any `ImportOption` value) |
| `@AnalyzeClasses` as a meta-annotation | — | unsupported | Attribute macros cannot be aliased; use a `macro_rules!` wrapper |
| `@ArchTest` on a `static ArchRule` field | `#[arch_test] fn rule() -> impl ArchRule` | done | Rust statics cannot hold runtime-built trait objects; the attribute reports a compile error on anything but a function. `#[arch_test(items = provider_fn)]` names an explicit items provider `[rust-only]`; outside an `#[analyze_classes]` module the crate under test is imported with the defaults `[rust-only]` |
| `@ArchTest` on a `static void method(JavaClasses)` | `#[arch_test] fn rule(items: &RustItems)` | done | |
| `@ArchTest ArchTests.in(OtherRules.class)` | `#[arch_test] fn shared() -> ArchTests { ArchTests::in_(other_rules::arch_tests) }` | done | `other_rules` is an inline module annotated with `#[arch_rules]` (no import config) that gains `pub fn arch_tests() -> ArchTests`; its rules are evaluated against the enclosing `#[analyze_classes]` import. `cargo test` cannot create tests dynamically, so one `#[test]` runs every included rule and panics with the combined reports (`N of M arch tests in \`module\` failed:` + `[name]` sections); `ArchTests::evaluate(&items)` returns them as `(name, message)` pairs |
| `@ArchIgnore(reason)` | `#[arch_ignore(reason = "..")]` / `#[arch_ignore("..")]` / `#[arch_ignore]` → `#[ignore = ".."]` | done | Works before or after `#[arch_test]`; inside `#[arch_rules]` the case is skipped and reported by `ArchTestCase::ignore_reason()` |
| `@ArchTag("x")` | `#[arch_tag("x", ..)]` | partial | Appended to the generated test name (`rule__tag_x`) so `cargo test __tag_x` selects every tagged rule; `cargo test` has no native tagging. Inside `#[arch_rules]` tags are kept on `ArchTestCase::tags()` |
| `CacheMode.FOREVER` / `PER_CLASS` | `CacheMode::Forever` / `PerClass` | done | `Forever` (default) = process-wide cache keyed by crate dir, packages, packages_of, items, location paths, `whole_workspace` and the import option types; `PerClass` = the module's `OnceLock` only. No soft references; `harness::clear_cache()` / `cached_imports()` are `[rust-only]` |
| `LocationProvider` | `trait LocationProvider { fn get(&self, test_module: &str) -> Vec<Location> }` (closures implement it) | done | The macro instantiates the named type with `Default::default()` (Java: public default constructor); `Location::of(path)` accepts a crate directory, a source directory or a file |
| `junit.testFilter` | — | unsupported | Use `cargo test <name>` |
| `junit.displayName.replaceUnderscoresBySpaces` | — | unsupported | `cargo test` names are identifiers |
| JUnit 4 `ArchUnitRunner`, JUnit 6 engine | — | unsupported | Not applicable |
| plain-function API (no macros) | `archunit::harness::analyze_classes().packages(&[..]).packages_of(..).items(..).locations(provider).whole_workspace(b).import_option(opt).cache_mode(..).import() -> Arc<RustItems>` with the same cache | done | `crate_dir(path)` and `for_test_module(module_path!())` are `[rust-only]` |

---

## 7. Configuration (`ArchConfiguration`, `archunit.properties`) → `archunit.toml`

| Java (`archunit.properties`) | `archunit.toml` | Status | Note |
|---|---|---|---|
| file at classpath root | `archunit.toml` next to the `Cargo.toml` of the crate under test (`CARGO_MANIFEST_DIR`), searched upward to the workspace root | done | Nested tables flatten to dotted keys, arrays to comma-separated values; `ArchConfiguration::load_from(path)` loads another file `[rust-only]` |
| `-Darchunit.key=value` override | `ARCHUNIT_KEY=value` environment variable (`.` and `-` → `_`, upper-cased; `config::environment_variable_name(key)`) | done | Checked on every `property(..)` lookup, so it also overrides values set programmatically; `sub_properties` applies it to known keys only |
| `ArchConfiguration.get()` / `getProperty` / `setProperty` / `removeProperty` / `containsProperty` / `getPropertyOrDefault` / `getSubProperties` / `reset` / `withThreadLocalScope` | `ArchConfiguration::get()` (global, `RwLock`) / `property` / `set_property` / `remove_property` / `contains_property` / `property_or_default` / `sub_properties` / `reset` / `with_thread_local_scope` | done | `get()` returns a snapshot; setters are associated functions acting on the effective scope |
| `resolveMissingDependenciesFromClassPath` | `resolve_missing_dependencies_from_classpath` (default `false`); `CrateImporter::new()` reads it | partial | When `true`, dependency crates (path, git and registry checkouts) are parsed. Default differs from Java (`true`) because parsing large crates is slow; `std`/`core`/`alloc` are always stubs |
| `classResolver` / `classResolver.args` (`SelectedClassResolverFromClasspath`) | `[class_resolver] packages = ["tokio..", "serde.."]` | done | Only the "selected crates" resolver: with `resolve_missing_dependencies_from_classpath = true`, dependency crates whose lib name matches one of the identifiers are parsed, the others stubbed. No custom resolver by class name |
| `import.dependencyResolutionProcess.maxIterationsFor{MemberTypes,AccessesToTypes,Supertypes,PermittedSubclasses,EnclosingTypes,AnnotationTypes,GenericSignatureTypes}` | — | unsupported | The source importer resolves every name in one pass over the parsed crates; there is no iterative class-file resolution to bound. The keys are accepted and ignored |
| `enableMd5InClassSources` | — | unsupported | No class files |
| `archRule.failOnEmptyShould` | `[arch_rule] fail_on_empty_should` (default `true`); `ArchConfiguration::set_fail_on_empty_should(..)` | done | |
| `failureDisplayFormat` | — (programmatic only) | partial | |
| `extension.<id>.enabled` / `extension.<id>.<prop>` | `[extension.<id>] enabled`, … | deferred | |
| `cycles.maxNumberToDetect` / `cycles.maxNumberOfDependenciesPerEdge` | `[cycles] max_number_to_detect`, `max_number_of_dependencies_per_edge` | done | Read via `ArchConfiguration::max_number_of_cycles_to_detect()` / `max_number_of_dependencies_per_edge()` |
| `freeze.*` | `[freeze] …` (see §5.6) | P5 | |
| `junit.*` | — | unsupported | see §6 |
| `[rust-only]` | `[import] include_targets = ["lib", "proc-macro", "bin", "test", "example", "bench"]` (default: all) | done | Applied by `CrateImporter::new()` as an import option |
| `[rust-only]` | `[import] exclude_binaries_from_coding_rules = true`, `[report] item_prefix = "Item"` (allows `Struct`/`Trait`/… kind prefixes instead of the generic `Item`) | P5 | |

---

## 8. Examples (`archunit-example`) → `examples/` and `tests/fixtures/`

| Java test class | Rust port | Status |
|---|---|---|
| `LayeredArchitectureTest`, `LayerDependencyRulesTest`, `NamingConventionTest`, `ControllerRulesTest`, `DaoRulesTest`, `MethodsTest`, `InterfaceRulesTest`, `SingleClassTest`, `RestrictNumberOfClassesWithACertainPropertyTest`, `SlicesIsolationTest` | rules against `tests/fixtures/layered_app` | P6 |
| `OnionArchitectureTest` (package and annotation variants) | `tests/fixtures/onion_app` (attribute variant via outer attributes on items) | P6 |
| `CyclicDependencyRulesTest` (simple, constructor, inheritance, field access, member, simple scenario, complex, custom ignore, custom assignment) | `tests/fixtures/cyclic_app` | P6 |
| `CodingRulesTest`, `DependencyRulesTest`, `ProxyRulesTest` | `tests/fixtures/coding_rules_app` | P6 |
| `FrozenRulesTest` | frozen store under `examples/frozen/` | P6 |
| `PlantUmlArchitectureTest` (`shopping_example.puml`) | `tests/fixtures/shopping_app` + `examples/shopping_example.puml` | P6 |
| `ModulesTest` | — | deferred with §5.3 |
| `SecurityTest`, `SessionBeanRulesTest`, `ThirdPartyRulesTest` | ported where the concept exists (third-party access restriction yes; EJB session beans no) | P6 partial |
| `ArchUnitExampleJUnit5ArchitectureTest` (`@AnalyzeClasses` + `ArchTests.in`) | `examples/architecture_test.rs` using `#[analyze_classes]` and `arch_tests!` | P6 |
| `extension` example (`ArchUnitExtension`) | — | deferred |

---

## 9. Phase notes

### Phase 1

Approximations and limits established in Phase 1 (all documented in rustdoc as well):

* **Modules are packages.** `RustItems` iteration and `classes()` exclude modules; they are
  reachable through `package(..)`, `modules()` and `RustModule`. `use` declarations become
  `imports` dependencies of the enclosing module item.
* **Impl blocks** for imported types are folded into the type (members, `implements trait`
  dependencies). Impl blocks for foreign types (`impl From<Order> for String`) stay items of
  their own, named `<impl From<Order> for String>`, and appear in `classes()`.
* **Free functions** are items *and* single-member code units, so `methods()` rules cover them
  and their accesses have the function item as origin owner.
* **Method and field resolution** follows `self`, typed locals, constructor calls, and
  unique names; unresolved targets are stubs under `<unresolved>` (see PLAN.md §4).
  Accesses to a type's own members are recorded but, as in ArchUnit, produce no dependency.
* **Raw types** follow type aliases (`OrderId` → `u64`) one chain deep; dependency records keep
  the alias as target.
* **Constructors** are associated functions without `self` returning `Self`, `Option<Self>`,
  `Result<Self, _>`, `Box/Rc/Arc<Self>`; unresolved `T::new()`/`T::default()` calls count as
  constructor calls.
* **`const`/`static` initializers** are not analysed (no static-initializer code unit).
* **Binary targets** sharing the package name keep the lib's root name; colliding paths keep the
  library item in the index (documented, rare).

### Phase 2

* **Overload naming** as decided in Phase 0: fluent `that()` / `should()` / `and_should()` /
  `access_classes_that()` keep the name; the object-taking overloads are `that_with(pred)`,
  `should_with(cond)`, `and_should_with(cond)`, `access_classes_that_with(pred)`, and so on.
  `ClassesThat::satisfy(pred)` / `ClassesShould::satisfy(cond)` are Rust-only escape hatches.
* **Rule objects.** Every fluent chain ends in a concrete type (`ClassesShouldConjunction`,
  `MembersShouldConjunction<M>`, `SimpleArchRule<T>`, `CompositeArchRule`) that implements
  `ArchRule`; `because`, `as_` and `allow_empty_should` return `SimpleArchRule<T>` (no further
  chaining, like Java's `ArchRule`). `Box<dyn ArchRule>` has the same three methods.
* **`ArchCondition<T>` is a struct**, not a trait; custom logic goes through a closure or a
  `ConditionLogic<T>` implementation. Java's `ArchConditions`/`ArchPredicates` static classes
  are modules of free functions (`archunit::lang::conditions`, `..::conditions::predicates`).
* **Trait-impl members are `pub`**: `impl Trait for Type { fn f() }` items are as visible as the
  trait, so `be_public()` accepts them.
* **Modules reside in themselves.** A module item's `package_name()` is its own path, so a `use`
  in `mod service` counts as a dependency from package `..service..`.
* **`ArchConfiguration::with_thread_local_scope(..)`** gives tests a private configuration
  (fail-on-empty-should, ignore patterns, display format) without touching other threads.
* **Empty `should`** panics from `evaluate` with ArchUnit's message, naming
  `ArchRule::allow_empty_should(true)` and `arch_rule.fail_on_empty_should = false`.

### Phase 3

* **Architectures** are plain `ArchRule` implementations (`LayeredArchitecture`,
  `OnionArchitecture`) with the Java builder shape; the onion architecture is evaluated through
  its `LayeredArchitecture` delegate exactly as in Java, so both share one report format.
  Layer names in `where_layer(..)`/`may_only_be_accessed_by_layers(..)` are validated eagerly
  and panic with `There is no layer named '<x>'` (Java's `IllegalArgumentException`).
* **Module items and layers.** Because `use` declarations are `imports` dependencies of the
  module item (Phase 1), a module whose path matches a layer's package identifier is *in* that
  layer, and its imports are checked like any other dependency. Layers defined by attributes
  (`annotated_with(..)`) do not contain modules, so a module importing an annotated item from
  a protected layer is reported as a violation from outside the architecture. This is the
  faithful reading of ArchUnit's semantics; use `ignore_dependency_where(dependency_origin(modules()))`
  to opt out.
* **Folded impl blocks.** Phase 3 fixed a leak from Phase 1: impl blocks of imported types are
  now folded completely, i.e. their `implements trait`, type-parameter and self-type
  dependencies are attributed to the type, and the hidden impl item never appears as an origin
  in `only_have_dependents_where(..)`-style reports.
* **Slices** follow Java's `Slices.Transformer`: `that`/`and`/`or` extend the description
  (`slices matching '..a.(*)..' that <pred>`), `as_` replaces it, `naming_slices("$1 layer")`
  renames every slice. `Slice` equality is identifier equality; `dependencies_from_self()`
  excludes targets assigned to the same slice (via the assignment, not the item set). Slices are
  built from `RustItems` iteration, so modules are never slice members.
* **Cycle detection** ports Johnson's algorithm on Tarjan components (`cycle_detection`), with
  `Cycles::max_number_of_cycles_reached()`, the `cycles.max_number_to_detect` limit (default 100)
  and `cycles.max_number_of_dependencies_per_edge` (default 20). The hint text names
  `archunit.toml` and the Rust key instead of `archunit.properties`/`cycles.maxNumberToDetect`.
  `CycleArchCondition::builder()` is public so custom components (modules, crates) can reuse it.
* **Custom event objects.** `CorrespondingObject::Other` now holds a `CorrespondingValue`
  (`Slice`, `SliceDependency`, `Vec<Dependency>`) whose `dependencies()` feeds
  `EvaluationResult::handle_violations::<Dependency>` — the port of Java's `Convertible`, so
  cycle and slice violations can be post-processed as class dependencies (needed by
  `FreezingArchRule` in Phase 5).
* **Package identifiers** accept a single `.` as an alias for `::`, so `..app.(*)..` from the
  brief and the user guide works without translation.

### Phase 4

* **Macros rewrite, the harness runs.** `#[analyze_classes]` on an inline module adds a hidden
  `__archunit_items()` provider (a `OnceLock` per module) and turns every inner `#[arch_test]`
  into `#[::archunit::harness::arch_test(items = __archunit_items)]`, so the expansion of a
  rule never depends on how the user imported the attribute. `#[arch_test]` keeps the rule
  function under a hidden name (`__archunit_rule_<name>`) and generates `#[test] fn <name>()`;
  the return value is dispatched through `harness::ArchTestRunnable`, implemented for every
  `ArchRule` and for `ArchTests`, which is how one attribute serves `-> impl ArchRule`,
  `-> ArchTests` and `(items: &RustItems)`.
* **`ArchTests` are one test.** JUnit registers each included rule as a test of its own;
  `cargo test` cannot, so `ArchTests::check` runs all cases and panics with every failure.
  Ignored cases are skipped silently (Java reports them as skipped tests).
* **`#[arch_tag]` renames.** Tags become the `__tag_<name>` suffix of the test function, the
  only hook `cargo test <filter>` offers. The rule function keeps its name only inside
  `#[arch_rules]` modules, where nothing is generated per rule.
* **Import scope.** With no arguments the crate of `CARGO_MANIFEST_DIR` is imported (Java:
  the whole classpath), sibling workspace members become stubs. `packages`/`packages_of`/`items`
  import the workspace and filter by name; `locations` replace the crate as import roots and
  are matched by path prefix (a crate directory, a source directory, or a file);
  `whole_workspace = true` wins over everything, like `wholeClasspath`.
* **`archunit.toml` replaces `archunit.properties`.** Keys are the Java keys in snake case
  with tables for the dotted prefixes; every lookup consults `ARCHUNIT_<KEY>` first, the
  counterpart of `-Darchunit.<key>`. The importer reads
  `resolve_missing_dependencies_from_classpath`, `class_resolver.packages` and the rust-only
  `import.include_targets` when it is constructed, so a thread-local scope covers it.
* The `import.dependency_resolution_process.*` keys are `unsupported` (accepted, ignored):
  there is nothing iterative to bound in a source import.

## 10. Phase status log

| Phase | Status | Summary |
|---|---|---|
| 0 | done | Research, this mapping, PLAN.md, CLAUDE.md, NOTICE |
| 1 | done | Importer (`cargo_metadata` + `syn`), name resolution across modules/crates incl. re-exports and globs, domain model with members, accesses, dependencies; fixtures `layered_app`, `onion_app`, `cyclic_app`, `reexports_app`; 35 tests |
| 2 | done | `ArchRule`, `ArchCondition`/`ConditionLogic`, events, `EvaluationResult`/`FailureReport` in ArchUnit's format, ignore patterns, `ArchConfiguration` (programmatic), all `ArchConditions`, the full `classes()`/`no_classes()`/`the_class()`/members/`all()` syntax; 25 lang tests incl. golden reports |
| 3 | done | `library::architectures` (layered + onion), `library::dependencies` (slices, `be_free_of_cycles`, `not_depend_on_each_other`, `ignore_dependency`), `library::cycle_detection` (Johnson/Tarjan port, `CycleArchCondition`), cycle configuration properties; 24 library tests incl. golden reports for layered, onion, simple cycle, simple scenario and controller slices |
| 4 | done | `archunit-macros` (`#[analyze_classes]`, `#[arch_test]`, `#[arch_rules]`, `#[arch_ignore]`, `#[arch_tag]`), `archunit::harness` (`analyze_classes()` builder, `CacheMode`, `LocationProvider`, `ArchTests`, cache), `archunit.toml` + `ARCHUNIT_*` overrides with importer settings; 11 harness tests (macro-generated) + 5 config tests |
| 5 | pending | |
| 6 | pending | |
