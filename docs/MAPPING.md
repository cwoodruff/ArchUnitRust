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
| `ClassFileImporter` | `CrateImporter` | P1 | Parses source with `syn`; crate graph from `cargo_metadata` |
| `new ClassFileImporter()` | `CrateImporter::new()` | P1 | |
| `withImportOption(ImportOption)` | `with_import_option(impl ImportOption)` | P1 | |
| `withImportOptions(Collection)` | `with_import_options(Vec<Box<dyn ImportOption>>)` | P1 | |
| `importClasspath()` | `import_workspace()` | P1 | Every workspace member plus (optionally) dependency crates. Name kept close to the concept: the "classpath" of a Rust build is the cargo dependency graph |
| `importPackages(String...)` | `import_packages(&[&str])` | P1 | Module identifiers, e.g. `"my_app::domain.."`; imports the crates that own those modules and keeps only matching modules |
| `importPackagesOf(Class...)` | `import_packages_of(&[&str])` | P1 | Modules containing the named items |
| `importPackage(String)` | `import_package(&str)` | P1 | |
| `importClasses(Class...)` | `import_items(&[&str])` | P1 | Named items only (plus stubs for their targets) |
| `importClass(Class)` | `import_item(&str)` | P1 | |
| `importPath(Path)` / `importPaths(..)` | `import_path(impl AsRef<Path>)` / `import_paths(..)` | P1 | Path to a `Cargo.toml`, a crate directory, or a workspace root |
| `importUrl` / `importUrls` | — | unsupported | No URL sources for Rust code |
| `importJar` / `importJars` | — | unsupported | No archive format; use `import_path` on an extracted crate |
| `importLocations(Collection<Location>)` | `import_locations(&[Location])` | P1 | |
| `Location` | `Location` | P1 | A source file path plus its crate and cargo target |
| `Location.of(Path/URL/URI/JarFile)` | `Location::of(path)` | P1 | Only paths |
| `Location.contains(String)` / `matches(Pattern)` | `contains(&str)` / `matches(&Regex)` | P1 | |
| `Location.isJar()` / `isArchive()` | — | unsupported | Always false; not provided |
| `ImportOption` (interface) | `trait ImportOption { fn includes(&self, location: &Location) -> bool }` | P1 | Also implemented for closures `Fn(&Location) -> bool` |
| `ImportOption.Predefined.DO_NOT_INCLUDE_TESTS` / `DoNotIncludeTests` | `import_option::DoNotIncludeTests` | P1 | Excludes `#[cfg(test)]` modules and items, `#[test]` functions, the `tests/` directory, and `test` cargo targets |
| `DO_NOT_INCLUDE_JARS` / `DoNotIncludeJars` | `import_option::DoNotIncludeDependencies` | P1 | Excludes crates that are not workspace members (registry, git, path deps outside the workspace) |
| `DO_NOT_INCLUDE_ARCHIVES` | `DoNotIncludeDependencies` | P1 | Same as above (no archive/jar distinction in Rust) |
| `DO_NOT_INCLUDE_PACKAGE_INFOS` | — | unsupported | No `package-info` concept |
| `ONLY_INCLUDE_TESTS` | `import_option::OnlyIncludeTests` | P1 | |
| `DoNotIncludeJars`-style custom `ImportOption` classes for `@AnalyzeClasses` | any type implementing `ImportOption` | P4 | |
| `ImportOptions` (internal) | — | — | internal |
| `ClassResolver` / `SelectedClassResolverFromClasspath` | `DependencyResolution` config (see §7) | P1 partial | Missing items from dependency crates are stubbed by default; optionally the dependency's source is parsed from the cargo registry checkout |
| "Dealing with Missing Classes" (stubs) | `RustItem::is_fully_imported()` returns `false` for stubs | P1 | |

### 2.2 Domain model

| Java (`core.domain`) | Rust (`archunit::core::domain`) | Status | Note |
|---|---|---|---|
| `JavaClasses` | `RustItems` | P1 | Owns the graph; `IntoIterator<Item=&RustItem>` |
| `JavaClasses.get(Class/String)` | `get(&str)` / `try_get(&str)` | P1 | |
| `contain(..)`, `containPackage(..)` | `contain(&str)`, `contain_package(&str)` | P1 | |
| `that(DescribedPredicate)` | `that(&DescribedPredicate<RustItem>)` | P1 | |
| `as(String)` / `getDescription()` | `as_(&str)` / `description()` | P1 | |
| `getDefaultPackage()` / `getPackage(String)` | `default_package()` (the crate roots) / `package(&str)` | P1 | |
| `JavaClass` | `RustItem` | P1 | struct, enum, union, trait, fn, impl block, mod, type alias, const, static, macro (`macro_rules!`, proc-macro fn), extern crate |
| `JavaClass.getName()` | `name()` | P1 | Crate-rooted full path `my_app::domain::order::Order`. See PLAN §"Full names" for the `crate::` decision |
| `getSimpleName()` | `simple_name()` | P1 | Last path segment; `impl` blocks get `impl Trait for Type` / `impl Type` |
| `getFullName()` | `full_name()` | P1 | Same as `name()` for items |
| `getPackageName()` / `getPackage()` | `package_name()` / `package()` → `&RustModule` | P1 | The enclosing module path |
| `getModifiers()` | `modifiers()` → `&[RustModifier]` | P1 | See §2.3 |
| `isInterface()` / `isEnum()` / `isRecord()` / `isAnnotation()` | `is_trait()` / `is_enum()` / — / `is_proc_macro()` | P1 | `isRecord` unsupported: no record concept (see `[rust-only]` `is_struct()`). `isAnnotation` maps to items that define attribute or derive macros |
| `isTopLevelClass()` | `is_top_level_item()` | P1 | Declared directly in a module body |
| `isNestedClass()` / `isLocalClass()` | `is_nested_item()` / `is_local_item()` | P1 | Both mean "declared inside a function body or another item body" |
| `isMemberClass()` / `isInnerClass()` / `isAnonymousClass()` | — | unsupported | Rust has no member/inner/anonymous types; closures are not items |
| `isArray()` / `isPrimitive()` / `getComponentType()` / `getBaseComponentType()` | `is_primitive()` for `i32`, `bool`, `str`, … ; arrays/slices/references/pointers are unwrapped to their element type when dependencies are recorded | P1 partial | Array-ness is not a property of an item in Rust |
| `isSealed()` / `getPermittedSubclasses()` | `is_sealed()` / — | P1 partial | `#[non_exhaustive]` enums or traits with a private supertrait are reported sealed; permitted subclasses have no counterpart |
| `isFullyImported()` | `is_fully_imported()` | P1 | |
| `getSuperclass()` / `getRawSuperclass()` / `getAllRawSuperclasses()` / `getClassHierarchy()` | `supertraits()` / `all_supertraits()` / `trait_hierarchy()` | P1 partial | Only traits have supertypes in Rust. Struct/enum items return empty |
| `getInterfaces()` / `getRawInterfaces()` / `getAllRawInterfaces()` | `implemented_traits()` / `all_implemented_traits()` | P1 | From `impl Trait for Type` blocks (in any imported crate) plus derives that resolve to traits |
| `getSubclasses()` / `getAllSubclasses()` | `implementors()` / `all_implementors()` on traits; `subtraits()` | P1 | |
| `getAllClassesSelfIsAssignableTo()` | `all_items_self_is_assignable_to()` | P1 | Self plus implemented traits plus their supertraits |
| `isAssignableTo(..)` / `isAssignableFrom(..)` / `isEquivalentTo(..)` | `is_assignable_to(..)` / `is_assignable_from(..)` / `is_equivalent_to(&str)` | P1 | `assignable_to(T)` = is `T` or implements trait `T` (transitively via supertraits) |
| `getEnclosingClass()` / `getEnclosingCodeUnit()` | `enclosing_item()` / `enclosing_code_unit()` | P1 | For local items |
| `getMembers()` / `getAllMembers()` | `members()` / `all_members()` | P1 | `all_` includes trait-provided default items |
| `getFields()` / `getAllFields()` / `getField(String)` / `tryGetField` | `fields()` / … | P1 | Struct/enum-variant/union fields plus associated consts (see §2.3) |
| `getMethods()` / `getAllMethods()` / `getMethod(..)` / `tryGetMethod(..)` | `methods()` / … | P1 | Associated functions from inherent and trait impls, and trait method declarations |
| `getConstructors()` / `getConstructor(..)` / `getAllConstructors()` | `constructors()` / … | P1 partial | Approximation: associated functions with no `self` receiver whose return type is `Self`, `Option<Self>`, `Result<Self, _>`, or the owning type by name. Documented heuristic |
| `getCodeUnits()` / `getCodeUnitWithParameterTypes(..)` | `code_units()` / `code_unit_with_parameter_types(..)` | P1 | All functions of the item |
| `getStaticInitializer()` | — | unsupported | No static initializers in Rust |
| `getEnumConstants()` / `getEnumConstant(String)` | `variants()` / `variant(&str)` | P1 | |
| `getAnnotations()` / `getAnnotationOfType(..)` / `tryGetAnnotationOfType(..)` / `isAnnotatedWith(..)` / `isMetaAnnotatedWith(..)` | `annotations()` / `annotation_of_type(&str)` / `try_annotation_of_type(&str)` / `is_annotated_with(..)` / — | P1 partial | Attributes and derives, see §2.5. Meta-annotation unsupported |
| `getAnnotationsWithTypeOfSelf()` / `getAnnotationsWithParameterTypeOfSelf()` | `annotations_with_type_of_self()` / — | P1 partial | Only for items that define attribute/derive macros |
| `getAccessesFromSelf()` / `getAllAccessesFromSelf()` | `accesses_from_self()` / `all_accesses_from_self()` | P1 | See §2.6 |
| `getAccessesToSelf()` | `accesses_to_self()` | P1 | |
| `getFieldAccessesFromSelf()` / `getFieldAccessesToSelf()` | `field_accesses_from_self()` / `field_accesses_to_self()` | P1 | |
| `getMethodCallsFromSelf()` / `getMethodCallsToSelf()` | `method_calls_from_self()` / `method_calls_to_self()` | P1 | |
| `getConstructorCallsFromSelf()` / `getConstructorCallsToSelf()` | `constructor_calls_from_self()` / `constructor_calls_to_self()` | P1 partial | Calls to functions classified as constructors, plus struct literals `Foo { .. }` and tuple-struct/variant construction `Foo(..)` |
| `getCodeUnitCallsFromSelf()` / `getCodeUnitCallsToSelf()` | `code_unit_calls_from_self()` / `code_unit_calls_to_self()` | P1 | |
| `getMethodReferencesFromSelf()` / `..ToSelf()` / `getConstructorReferences..` / `getCodeUnitReferences..` | `function_references_from_self()` / `function_references_to_self()` | P1 | A function path used as a value (`map(Foo::new)`) |
| `getCodeUnitAccessesFromSelf()` / `..ToSelf()` | `code_unit_accesses_from_self()` / `..to_self()` | P1 | Calls plus references |
| `getDirectDependenciesFromSelf()` / `getDirectDependenciesToSelf()` | `direct_dependencies_from_self()` / `direct_dependencies_to_self()` | P1 | |
| `getTransitiveDependenciesFromSelf()` | `transitive_dependencies_from_self()` | P1 | |
| `getFieldsWithTypeOfSelf()`, `getMethodsWithParameterTypeOfSelf()`, `getMethodsWithReturnTypeOfSelf()`, `getConstructorsWithParameterTypeOfSelf()` | same names, snake_case | P1 | |
| `getMethodThrowsDeclarationsWithTypeOfSelf()` / `getConstructorsWithThrowsDeclarationTypeOfSelf()` / `getThrowsDeclarations()` | `functions_with_error_type_of_self()` / — / `error_types()` | P1 partial | "throws E" ≙ returns `Result<_, E>`. See §2.7 |
| `getInstanceofChecks()` / `getInstanceofChecksWithTypeOfSelf()` | — | unsupported | No runtime type checks; nearest is `downcast_ref::<T>()`, recorded as a plain dependency |
| `getReferencedClassObjects()` | `referenced_type_objects()` | P1 partial | `TypeId::of::<T>()`, `size_of::<T>()`, `type_name::<T>()` turbofish uses |
| `getTryCatchBlocks()` / `getTryCatchBlocksThatCatchSelf()` | — | unsupported | No try/catch; `?` and `match` on `Result` are not blocks |
| `getTypeParameters()` | `type_parameters()` | P1 | Generic params with bounds |
| `getSource()` / `Source.getMd5sum()` | `source()` → `Option<&Source>` / — | P1 partial | File path and crate; MD5 unsupported (see §7) |
| `getSourceCodeLocation()` | `source_code_location()` | P1 | |
| `getDescription()` | `description()` | P1 | `"Class <name>"` becomes `"Item <name>"` (per the agreed failure format); members: `"Method <..>"`, `"Field <..>"`, `"Constructor <..>"` |
| `reflect()` | — | unsupported | No runtime reflection in Rust |
| `traverseSignature(SignatureVisitor)` | `traverse_signature(&mut impl SignatureVisitor)` | deferred | Generic-signature visitor; portable but low value |
| `toErasure()` | `to_erasure()` | P1 | Identity for items |
| `[rust-only]` | `is_struct()`, `is_union()`, `is_function()`, `is_module()`, `is_type_alias()`, `is_const()`, `is_static()`, `is_macro()`, `is_impl()`, `kind() -> ItemKind` | P1 | |
| `[rust-only]` | `crate_name()`, `cargo_target()` | P1 | |
| `JavaPackage` | `RustModule` | P1 | |
| `JavaPackage.getName()` / `getRelativeName()` | `name()` / `relative_name()` | P1 | |
| `getClasses()` / `getClassesInPackageTree()` | `items()` / `items_in_package_tree()` | P1 | |
| `getSubpackages()` / `getSubpackagesInTree()` / `getPackage(String)` / `containsPackage(..)` | `subpackages()` / `subpackages_in_tree()` / `package(&str)` / `contains_package(&str)` | P1 | |
| `getParent()` | `parent()` | P1 | |
| `getClass(..)` / `getClassWithFullyQualifiedName` / `getClassWithSimpleName` / `containsClass..` | `item(..)` / `item_with_fully_qualified_name` / `item_with_simple_name` / `contains_item..` | P1 | |
| `getClassDependenciesFromThisPackage()` / `..ToThisPackage()` / `..FromThisPackageTree()` / `..ToThisPackageTree()` | same names, snake_case, `class` → `item` | P1 | |
| `getPackageDependenciesFromThisPackage()` / … | same names, snake_case | P1 | |
| `getPackageInfo()` / `getAnnotations()` / `isAnnotatedWith(..)` | `module_attributes()` / `annotations()` / `is_annotated_with(..)` | P1 partial | Inner attributes of the module (`#![allow(..)]`, `#![doc = ..]`, `#![cfg(..)]`). Custom inner attributes are unstable in Rust, so annotation-based module definitions are limited to built-in attributes |
| `traversePackageTree(predicate, visitor)` | `traverse_package_tree(pred, &mut visitor)` | P1 | |
| `JavaPackage.Predicates` / `Functions` | `rust_module::predicates` / `functions` | P1 | |
| `JavaMember` | `RustMember` | P1 | Field, method (associated fn), variant, associated const, associated type |
| `JavaMember.getOwner()` / `getName()` / `getFullName()` / `getModifiers()` / `getDescriptor()` | `owner()` / `name()` / `full_name()` / `modifiers()` / — | P1 | `getDescriptor` (JVM descriptor) unsupported |
| `getAccessesToSelf()` / `getAllInvolvedRawTypes()` | `accesses_to_self()` / `all_involved_raw_types()` | P1 | |
| `JavaMember.Predicates.declaredIn(..)` | `rust_member::predicates::declared_in(..)` | P1 | |
| `JavaField` | `RustField` | P1 | Named/tuple field of struct, enum variant or union; associated `const` (modeled as a static field) |
| `JavaField.getType()` / `getRawType()` | `type_()` / `raw_type()` | P1 | Raw = generics erased, references/arrays unwrapped |
| `getAccessesToSelf()` | `accesses_to_self()` | P1 | |
| `JavaCodeUnit` | `RustCodeUnit` | P1 | Free functions, associated functions, trait methods, closures are **not** code units (they belong to their enclosing fn) |
| `getParameters()` / `getParameterTypes()` / `getRawParameterTypes()` | `parameters()` / `parameter_types()` / `raw_parameter_types()` | P1 | `self` receiver excluded |
| `getReturnType()` / `getRawReturnType()` | `return_type()` / `raw_return_type()` | P1 | `()` for none |
| `getThrowsClause()` / `getExceptionTypes()` | `error_types()` | P1 partial | See §2.7 |
| `getCallsFromSelf()` / `getMethodCallsFromSelf()` / `getConstructorCallsFromSelf()` / `getFieldAccesses()` / `getAccessesFromSelf()` / references | same, snake_case | P1 | |
| `getCallsOfSelf()` | `calls_of_self()` | P1 | |
| `isMethod()` / `isConstructor()` | `is_method()` / `is_constructor()` | P1 | |
| `getParameterAnnotations()` | `parameter_annotations()` | P1 | |
| `JavaCodeUnit.Predicates.method()` / `constructor()` / `anyParameterThat` / `allParameters` | `rust_code_unit::predicates::{method, constructor, any_parameter_that, all_parameters}` | P1 | |
| `JavaMethod` | `RustMethod` | P1 | Any function with an owner impl/trait; free functions are `RustItem`s **and** exposed as `RustMethod` with a module owner so `methods()` rules cover them |
| `JavaMethod.getDefaultValue()` | — | unsupported | Annotation default values do not exist |
| `JavaConstructor` | `RustConstructor` | P1 partial | Heuristic subset of `RustMethod` (see `getConstructors`) |
| `JavaStaticInitializer` | — | unsupported | |
| `JavaParameter` | `RustParameter` | P1 | |
| `JavaEnumConstant` | `RustVariant` | P1 | |
| `JavaAnnotation<OWNER>` | `RustAnnotation` | P1 | See §2.5 |
| `JavaType`, `JavaParameterizedType`, `JavaTypeVariable`, `JavaWildcardType`, `JavaGenericArrayType` | `RustType` enum: `Path`, `Reference`, `Slice`, `Array`, `Tuple`, `TraitObject`, `ImplTrait`, `TypeParam`, `Never`, `Infer`, `Ptr`, `FnPointer` | P1 | Raw erasure via `RustType::to_erasure()` |
| `JavaModifier` | `RustModifier` | P1 | See §2.3 |
| `Dependency` | `Dependency` | P1 | `origin_item()`, `target_item()`, `source_code_location()`, `description()`, `kind() [rust-only]` |
| `Dependency.Predicates.dependency(..)` / `dependencyOrigin(..)` / `dependencyTarget(..)` | `dependency::predicates::{dependency, dependency_origin, dependency_target}` | P1 | |
| `Dependency.Functions.GET_ORIGIN_CLASS` / `GET_TARGET_CLASS` | `dependency::functions::{get_origin_item, get_target_item}` | P1 | |
| `Dependency.toTargetClasses(..)` | `Dependency::to_target_items(..)` | P1 | |
| `Dependency.convertTo(Class)` | — | unsupported | Reflection-based; use `kind()` |
| `JavaAccess<T>` / `JavaFieldAccess` / `JavaCall` / `JavaMethodCall` / `JavaConstructorCall` / `JavaCodeUnitReference` / `JavaMethodReference` / `JavaConstructorReference` / `JavaCodeUnitAccess` | `RustAccess` enum with variants `FieldAccess`, `MethodCall`, `ConstructorCall`, `FunctionReference` | P1 | See §2.6 |
| `JavaAccess.getOrigin()` / `getOriginOwner()` / `getTarget()` / `getTargetOwner()` / `getLineNumber()` / `getSourceCodeLocation()` / `getDescription()` / `isDeclaredInLambda()` / `getContainingTryBlocks()` | `origin()` / `origin_owner()` / `target()` / `target_owner()` / `line_number()` / `source_code_location()` / `description()` / `is_declared_in_closure()` / — | P1 | try blocks unsupported |
| `JavaAccess.Predicates.origin(..)` / `originOwner(..)` / `target(..)` / `targetOwner(..)` / `originOwnerEqualsTargetOwner()` | `rust_access::predicates::{origin, origin_owner, target, target_owner, origin_owner_equals_target_owner}` | P1 | |
| `JavaFieldAccess.AccessType` (`GET`/`SET`) | `AccessType::{Get, Set}` | P1 | `Set` = assignment target or `&mut` borrow of the field |
| `AccessTarget` and subtypes (`FieldAccessTarget`, `MethodCallTarget`, `ConstructorCallTarget`, …) | `AccessTarget` enum | P1 | `resolve_member()` returns `Option`/set like Java: unresolved when the target could not be found in the import |
| `AccessTarget.Predicates.declaredIn(..)` / `constructor()` | `access_target::predicates::{declared_in, constructor}` | P1 | |
| `SourceCodeLocation` | `SourceCodeLocation` | P1 | `source_item()`, `source_file_name()`, `line_number()`, `Display` = `(src/x.rs:14)` |
| `Source` | `Source` | P1 partial | `uri()` → file path; `md5sum()` unsupported |
| `ThrowsClause` / `ThrowsDeclaration` | `ErrorTypes` / `ErrorTypeDeclaration` | P1 partial | See §2.7 |
| `InstanceofCheck`, `TryCatchBlock`, `ReferencedClassObject` | — / — / `ReferencedTypeObject` | see above | |
| `PackageMatcher` / `PackageMatchers` | `PackageMatcher` / `PackageMatchers` | P1 | Same syntax on `::` separators. See §2.4 |
| `PackageMatcher.match(String)` → `Result.getGroup(int)` / `getNumberOfGroups()` | `match_(&str)` → `Option<MatchResult>`, `group(usize)`, `number_of_groups()` | P1 | |
| `Formatters` | `formatters` module | P1 | `format_method`, `format_method_simple`, `format_named_predicate` |
| `DomainObjectCreationContext`, `ImportContext`, `DomainPlugin`, `Java14DomainPlugin`, … | — | internal | Not public API |

#### `properties` interfaces

| Java (`core.domain.properties`) | Rust | Status |
|---|---|---|
| `HasName` / `HasName.AndFullName` | `trait HasName` / `trait HasFullName` | P1 |
| `HasName.Predicates.name/nameMatching/nameStartingWith/nameContaining/nameEndingWith` | `has_name::predicates::{name, name_matching, name_starting_with, name_containing, name_ending_with}` | P1 |
| `HasName.AndFullName.Predicates.fullName/fullNameMatching` | `has_full_name::predicates::{full_name, full_name_matching}` | P1 |
| `HasName.Functions.GET_NAME/GET_NAMES`, `namesOf(..)` | `has_name::functions::get_name`, `names_of(..)` | P1 |
| `HasModifiers` / `Predicates.modifier(..)` | `trait HasModifiers` / `has_modifiers::predicates::modifier(..)` | P1 |
| `CanBeAnnotated` / `Predicates.annotatedWith(..)` / `metaAnnotatedWith(..)` | `trait CanBeAnnotated` / `can_be_annotated::predicates::annotated_with(..)` / — | P1 partial (meta unsupported) |
| `HasAnnotations` | `trait HasAnnotations` | P1 |
| `HasOwner` / `Predicates.With.owner(..)` / `Functions.Get.owner()` | `trait HasOwner` / `has_owner::predicates::owner(..)` / `has_owner::functions::owner()` | P1 |
| `HasParameterTypes` / `Predicates.rawParameterTypes(..)` | `trait HasParameterTypes` / `has_parameter_types::predicates::raw_parameter_types(..)` | P1 |
| `HasReturnType` / `Predicates.rawReturnType(..)` / `Functions.GET_RETURN_TYPE` | `trait HasReturnType` / `has_return_type::predicates::raw_return_type(..)` / `functions::get_return_type` | P1 |
| `HasType` / `Predicates.rawType(..)` / `Functions.GET_RAW_TYPE` | `trait HasType` / `has_type::predicates::raw_type(..)` / `functions::get_raw_type` | P1 |
| `HasThrowsClause` / `Predicates.throwsClauseWithTypes/throwsClauseContainingType/throwsClause` | `trait HasErrorTypes` / `has_error_types::predicates::{error_types, error_types_containing, error_clause}` | P1 partial |
| `HasSourceCodeLocation` | `trait HasSourceCodeLocation` | P1 |
| `HasDescriptor` | — | unsupported (JVM descriptor) |
| `HasTypeParameters` / `HasUpperBounds` | `trait HasTypeParameters` / `trait HasBounds` | P1 |
| `CanOverrideDescription` | `trait CanOverrideDescription { fn as_(..) }` | P1 |

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

Same grammar as `PackageMatcher`, with `::` instead of `.` as the separator:

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
| constructor call | call to a constructor-classified function, `Foo { .. }`, `Foo(..)`, `Enum::Variant(..)` | `calls constructor` |
| field access (get/set) | `x.field`, `x.field = ..`, `&mut x.field`, `Foo { field: .. }` | `gets field` / `sets field` |
| method/constructor reference | function path as value (`iter.map(Foo::new)`) | `references method` / `references constructor` |
| field type | struct/enum/union field type | `has type` |
| parameter type / return type | fn signature | `has parameter of type` / `has return type` |
| throws declaration | `Result<_, E>` error type | `throws type` |
| extends | supertrait | `extends` |
| implements | `impl Trait for Type`, and derives that resolve to traits | `implements` |
| is annotated with | attribute or derive | `is annotated with` |
| annotation member type | attribute argument path | `has annotation member of type` |
| type parameter bounds / generic signature | generic bounds, `where` clauses, type arguments | `has type parameter 'T' depending on` / `has generic .. with type argument depending on` |
| instanceof / class object | `TypeId::of::<T>`, turbofish | `references class object` |
| — | `use` import | `[rust-only]` `imports` (an unused import still counts; ArchUnit has no import dependency because bytecode has none) |
| — | type alias target, `const`/`static` type, `impl` self type, trait object `dyn T`, `impl T` | `has type`, `implements` (for `dyn`/`impl` the verb is `references trait`) |
| — | macro invocation arguments | Best effort: arguments of well-known macros (`println!`, `format!`, `vec!`, `assert!`, `write!`, `matches!`, `dbg!`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, and any macro whose input parses as comma-separated expressions) are parsed and visited. Unknown macro bodies yield a single `[rust-only]` `invokes macro` dependency on the macro item |

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
| `DescribedPredicate<T>` (abstract class) | `struct DescribedPredicate<T>` holding `Arc<dyn Fn(&T) -> bool + Send + Sync>` and a description | P1 |
| `test(T)` | `test(&T) -> bool` (also `impl Fn(&T) -> bool`) | P1 |
| `getDescription()` | `description()` | P1 |
| `as(String, Object...)` | `as_(impl Into<String>)` (use `format!` for args) | P1 |
| `and(..)` / `or(..)` / `negate()` | `and(..)` / `or(..)` / `negate()` | P1 |
| `onResultOf(Function)` | `on_result_of(impl Fn(&F) -> T)` (returns `DescribedPredicate<F>`) | P1 |
| `forSubtype()` | — | unsupported: no subtyping; not needed because predicates on `RustItem` apply directly. Provided as a no-op for source compatibility |
| `alwaysTrue()` / `alwaysFalse()` / `equalTo(..)` / `lessThan` / `greaterThan` / `lessThanOrEqualTo` / `greaterThanOrEqualTo` | same, snake_case | P1 |
| `describe(String, Predicate)` | `describe(&str, impl Fn(&T) -> bool)` | P1 |
| `doesNot(..)` / `doNot(..)` / `not(..)` | `does_not(..)` / `do_not(..)` / `not(..)` | P1 |
| static `and(..)`/`or(..)` over iterables | `all_of(..)` / `any_of(..)` **and** `and(..)`/`or(..)` free functions | P1 |
| `empty()` / `anyElementThat(..)` / `allElements(..)` / `optionalContains(..)` / `optionalEmpty()` | `empty()` / `any_element_that(..)` / `all_elements(..)` / `optional_contains(..)` / `optional_empty()` | P1 |
| `DescribedFunction` / `ChainableFunction` (`then`, `is`, `as`) | `DescribedFunction<F, T>` with `then(..)`, `is(pred)`, `as_(..)` | P1 |
| `DescribedIterable` | `DescribedIterable<T>` | P1 |
| `HasDescription` | `trait HasDescription` | P1 |
| `ArchUnitException` (+ subclasses) | `ArchUnitError` enum (`thiserror`) | P1 |
| `ForwardingCollection/List/Set`, `ClassLoaders`, `ReflectionUtils`, `Suppliers`, `Optionals`, `Predicates`, `MayResolveTypesViaReflection`, `ResolvesTypesViaReflection` | — | internal / JVM-specific |

---

## 4. Lang API (`com.tngtech.archunit.lang`) → `archunit::lang`

### 4.1 Rules, conditions, events

| Java | Rust | Status | Note |
|---|---|---|---|
| `ArchRule` (interface) | `trait ArchRule: HasDescription + Send + Sync` | P2 | Object safe; `Box<dyn ArchRule>` implements it |
| `check(JavaClasses)` | `check(&RustItems)` | P2 | Panics with the failure report (Rust's `AssertionError`) |
| `evaluate(JavaClasses)` | `evaluate(&RustItems) -> EvaluationResult` | P2 | |
| `because(String)` | `because(&str) -> Self` | P2 | Description: `.. because 'reason'` (Java: `", because " + reason`) |
| `as(String)` | `as_(&str) -> Self` | P2 | |
| `allowEmptyShould(boolean)` | `allow_empty_should(bool) -> Self` | P2 | |
| `getDescription()` | `description()` | P2 | |
| `ArchRule.Assertions.check(rule, classes)` / `assertNoViolation(result)` | `assertions::check(..)` / `assert_no_violation(..)` | P2 | |
| `ArchRule.Factory.create(transformer, condition, priority)` / `withBecause(..)` | `ArchRule::create(..)` / `with_because(..)` | P2 | |
| `ArchRule.Transformation` (`As`, `Because`) | `RuleTransformation` enum | P2 | |
| `CompositeArchRule.of(rule).and(rule)` / `priority(..)` | `CompositeArchRule::of(rule).and(rule)` / `priority(..)` | P2 | |
| `ArchCondition<T>` (abstract class: `init`, `check`, `finish`, `and`, `or`, `as`, `forSubtype`, `getDescription`) | `trait ArchCondition<T>` with `init(&self, all: &[&T])`, `check(&self, item: &T, events: &mut ConditionEvents)`, `finish(&self, events)`, `description()`; combinators `and`, `or`, `as_` provided where `Self: Sized`; `ArchCondition::new(desc, closure)` convenience | P2 | Stateful conditions use interior mutability |
| `ArchCondition.ConditionByPredicate` (`describeEventsBy`) | `ConditionByPredicate<T>` with `describe_events_by(..)` | P2 | |
| `ConditionEvents` / `ConditionEvent` / `SimpleConditionEvent` (`violated`, `satisfied`, `invert`, `getDescriptionLines`, `handleWith`) | `ConditionEvents` / `trait ConditionEvent` / `SimpleConditionEvent::{violated, satisfied}` … | P2 | |
| `ConditionEvents.setInformationAboutNumberOfViolations(..)` | `set_information_about_number_of_violations(..)` | P2 | used by cycle detection |
| `EvaluationResult` (`hasViolation`, `getFailureReport`, `getPriority`, `add`, `handleViolations`, `filterDescriptionsMatching`) | `EvaluationResult` with `has_violation()`, `failure_report()`, `priority()`, `add(..)`, `handle_violations(..)`, `filter_descriptions_matching(..)` | P2 | `handleViolations` takes a `ViolationHandler<T>` closure dispatched on `RustAccess`/`Dependency`/`RustItem` via an enum instead of reified generics |
| `FailureReport` (`isEmpty`, `getDetails`, `toString`) | `FailureReport` | P2 | |
| `FailureMessages` (`getInformationAboutNumberOfViolations`) | `FailureMessages` | P2 | |
| `FailureDisplayFormat` (+ `failureDisplayFormat` property) | `trait FailureDisplayFormat`; set programmatically via `ArchConfiguration::set_failure_display_format(..)` | P2 partial | Cannot be instantiated from a class name in a config file |
| `Priority` (`HIGH`, `MEDIUM`, `LOW`, `asString`) | `Priority::{High, Medium, Low}`, `as_string()` | P2 | |
| `ClassesTransformer<T>` / `AbstractClassesTransformer` | `trait ClassesTransformer<T>` / `AbstractClassesTransformer::new(desc, fn)` | P2 | |
| `ViolationHandler<T>` | `trait ViolationHandler<T>` (+ closures) | P2 | |
| `CanBeEvaluated` | `trait CanBeEvaluated` | P2 | |
| `ArchUnitExtension` / `ArchUnitExtensions` / `EvaluatedRule` (ServiceLoader plug-ins) | `trait ArchUnitExtension`; registered with `ArchConfiguration::register_extension(..)` | deferred | No ServiceLoader; programmatic registration only |
| `archunit_ignore_patterns.txt` | `archunit_ignore_patterns.txt` in the crate root (or the path in `archunit.toml`) | P2 | Same semantics: one regex per line, `#` comments |
| Failure message | identical: `Architecture Violation [Priority: MEDIUM] - Rule 'DESC' was violated (N times):\n<details>` | P2 | |
| Empty-should failure | identical text, referencing `archunit.toml` key `arch_rule.fail_on_empty_should` | P2 | |

### 4.2 `ArchRuleDefinition` → `archunit::lang::syntax`

| Java | Rust | Status |
|---|---|---|
| `classes()` / `noClasses()` | `classes()` / `no_classes()` | P2 |
| `theClass(Class/String)` / `noClass(..)` | `the_class(&str)` / `no_class(&str)` | P2 |
| `members()` / `noMembers()` | `members()` / `no_members()` | P2 |
| `fields()` / `noFields()` | `fields()` / `no_fields()` | P2 |
| `codeUnits()` / `noCodeUnits()` | `code_units()` / `no_code_units()` | P2 |
| `constructors()` / `noConstructors()` | `constructors()` / `no_constructors()` | P2 partial (heuristic constructors) |
| `methods()` / `noMethods()` | `methods()` / `no_methods()` | P2 |
| `all(ClassesTransformer)` / `no(ClassesTransformer)` | `all(transformer)` / `no(transformer)` | P2 |
| `priority(Priority).classes()` … | `priority(Priority::Low).classes()` … | P2 |
| `[rust-only]` | `modules()` / `no_modules()`, `traits()` / `no_traits()`, `functions()` / `no_functions()` | P2 |

### 4.3 `GivenClasses` / `GivenClassesConjunction` / `GivenObjects` / `GivenConjunction`

| Java | Rust | Status |
|---|---|---|
| `that()` | `that()` → `ClassesThat<GivenClassesConjunction>` | P2 |
| `that(DescribedPredicate)` | `that_with(pred)` | P2 |
| `should()` | `should()` → `ClassesShould` | P2 |
| `should(ArchCondition)` | `should_with(cond)` | P2 |
| `and()` / `or()` | `and()` / `or()` | P2 |
| `and(pred)` / `or(pred)` | `and_with(pred)` / `or_with(pred)` | P2 |
| `GivenClass.should()` / `should(cond)` | `should()` / `should_with(cond)` | P2 |
| `GivenObjects<T>.that(pred)` / `should(cond)` | `that(pred)` / `should(cond)` (no zero-arg clash, so no suffix) | P2 |

### 4.4 `ClassesThat<CONJUNCTION>` (predicates)

All return `CONJUNCTION`. Rust: `ClassesThat<C>` returns `C`.

| Java | Rust | Status | Note |
|---|---|---|---|
| `haveFullyQualifiedName` / `doNotHaveFullyQualifiedName` | `have_fully_qualified_name` / `do_not_have_fully_qualified_name` | P2 | |
| `haveSimpleName` / `doNotHaveSimpleName` | `have_simple_name` / `do_not_have_simple_name` | P2 | |
| `haveNameMatching` / `haveNameNotMatching` | `have_name_matching` / `have_name_not_matching` | P2 | `regex` crate syntax |
| `haveSimpleNameStartingWith/NotStartingWith/Containing/NotContaining/EndingWith/NotEndingWith` | same, snake_case | P2 | |
| `resideInAPackage` / `resideInAnyPackage` / `resideOutsideOfPackage` / `resideOutsideOfPackages` | same, snake_case | P2 | |
| `arePublic` / `areNotPublic` | `are_public` / `are_not_public` | P2 | `pub` |
| `areProtected` / `areNotProtected` | `are_protected` / `are_not_protected` | P2 | restricted visibility |
| `arePackagePrivate` / `areNotPackagePrivate` | `are_package_private` / `are_not_package_private` | P2 | private |
| `arePrivate` / `areNotPrivate` | `are_private` / `are_not_private` | P2 | private (synonym) |
| `haveModifier` / `doNotHaveModifier` | `have_modifier(RustModifier)` / `do_not_have_modifier` | P2 | |
| `areAnnotatedWith(Class/String/pred)` / `areNotAnnotatedWith` | `are_annotated_with(impl Into<AnnotationSelector>)` / `are_not_annotated_with` | P2 | |
| `areMetaAnnotatedWith` / `areNotMetaAnnotatedWith` (3 overloads) | — | unsupported | no meta-annotations |
| `implement(Class/String/pred)` / `doNotImplement` | `implement(impl Into<ItemSelector>)` / `do_not_implement` | P2 | |
| `areAssignableTo` / `areNotAssignableTo` / `areAssignableFrom` / `areNotAssignableFrom` | same, snake_case | P2 | |
| `areInterfaces` / `areNotInterfaces` | `are_interfaces` / `are_not_interfaces` | P2 | traits; `are_traits` is an alias `[rust-only]` |
| `areEnums` / `areNotEnums` | `are_enums` / `are_not_enums` | P2 | |
| `areAnnotations` / `areNotAnnotations` | `are_annotations` / `are_not_annotations` | P2 | proc-macro attribute/derive definitions |
| `areRecords` / `areNotRecords` | — | unsupported | no records; use `are_structs` |
| `areTopLevelClasses` / `areNotTopLevelClasses` | `are_top_level_classes` / `are_not_top_level_classes` | P2 | |
| `areNestedClasses` / `areNotNestedClasses` / `areLocalClasses` / `areNotLocalClasses` | same, snake_case | P2 | both = declared inside a body |
| `areMemberClasses` / `areInnerClasses` / `areAnonymousClasses` (+ negations) | — | unsupported | |
| `belongToAnyOf(Class...)` / `doNotBelongToAnyOf` | `belong_to_any_of(&[&str])` / `do_not_belong_to_any_of` | P2 | "belong to" = is the item or is declared inside it |
| `containAnyMembersThat` / `containAnyFieldsThat` / `containAnyCodeUnitsThat` / `containAnyMethodsThat` / `containAnyConstructorsThat` | same, snake_case | P2 | |
| `containAnyStaticInitializersThat` | — | unsupported | |
| `[rust-only]` | `are_structs`, `are_unions`, `are_functions`, `are_modules`, `are_type_aliases`, `are_consts`, `are_statics`, `are_macros`, `are_unsafe`, `are_async`, `are_const_fns`, `are_pub_crate`, `are_test_code`, `reside_in_crate(&str)` | P2 | |

### 4.5 `ClassesShould` (conditions) and `ClassesShouldConjunction`

| Java | Rust | Status | Note |
|---|---|---|---|
| name conditions (`haveFullyQualifiedName`, `notHaveFullyQualifiedName`, `haveSimpleName`, `notHaveSimpleName`, `haveSimpleName{Not}StartingWith/Containing/EndingWith`, `haveName{Not}Matching`) | same, snake_case | P2 | |
| `resideInAPackage` / `resideInAnyPackage` / `resideOutsideOfPackage` / `resideOutsideOfPackages` | same | P2 | |
| `bePublic` / `notBePublic` / `beProtected` / `notBeProtected` / `bePackagePrivate` / `notBePackagePrivate` / `bePrivate` / `notBePrivate` | same | P2 | see §2.3 |
| `haveOnlyFinalFields()` | — | unsupported | Rust fields have no `final`; `[rust-only]` `have_only_private_fields()` is the useful analog (immutability from outside the module) |
| `haveOnlyPrivateConstructors()` | `have_only_private_constructors()` | P2 partial | True when the type cannot be constructed outside its module: it has a private field or is `#[non_exhaustive]`, and every constructor-classified fn is private |
| `haveModifier` / `notHaveModifier` | same | P2 | |
| `beAnnotatedWith` / `notBeAnnotatedWith` (3 overloads) | `be_annotated_with(impl Into<AnnotationSelector>)` / `not_be_annotated_with` | P2 | |
| `beMetaAnnotatedWith` / `notBeMetaAnnotatedWith` | — | unsupported | |
| `implement` / `notImplement` (3 overloads) | `implement(..)` / `not_implement(..)` | P2 | |
| `beAssignableTo` / `notBeAssignableTo` / `beAssignableFrom` / `notBeAssignableFrom` | same | P2 | |
| `accessField(owner, name)` / `accessFieldWhere(pred)` / `onlyAccessFieldsThat(pred)` | `access_field(&str, &str)` / `access_field_where(pred)` / `only_access_fields_that(pred)` | P2 | |
| `getField` / `getFieldWhere` / `setField` / `setFieldWhere` | same | P2 | |
| `callMethod(owner, name, params...)` / `callMethodWhere` / `onlyCallMethodsThat` | `call_method(&str, &str, &[&str])` / `call_method_where` / `only_call_methods_that` | P2 | |
| `callConstructor(owner, params...)` / `callConstructorWhere` / `onlyCallConstructorsThat` | `call_constructor(&str, &[&str])` / … | P2 partial | |
| `callCodeUnitWhere` / `onlyCallCodeUnitsThat` | same | P2 | |
| `accessTargetWhere` / `onlyAccessMembersThat` | same | P2 | |
| `accessClassesThat()` / `accessClassesThat(pred)` | `access_classes_that()` / `access_classes_that_with(pred)` | P2 | |
| `onlyAccessClassesThat()` / `(pred)` | `only_access_classes_that()` / `_with` | P2 | |
| `dependOnClassesThat()` / `(pred)` | `depend_on_classes_that()` / `_with` | P2 | |
| `onlyDependOnClassesThat()` / `(pred)` | `only_depend_on_classes_that()` / `_with` | P2 | |
| `transitivelyDependOnClassesThat()` / `(pred)` | `transitively_depend_on_classes_that()` / `_with` | P2 | |
| `onlyBeAccessed()` → `OnlyBeAccessedSpecification` | `only_be_accessed()` | P2 | |
| `OnlyBeAccessedSpecification.byAnyPackage(..)` / `byClassesThat()` / `byClassesThat(pred)` | `by_any_package(&[&str])` / `by_classes_that()` / `by_classes_that_with(pred)` | P2 | |
| `onlyHaveDependentClassesThat()` / `(pred)` | `only_have_dependent_classes_that()` / `_with` | P2 | |
| `beInterfaces` / `notBeInterfaces` / `beEnums` / `notBeEnums` | same | P2 | |
| `beRecords` / `notBeRecords` | — | unsupported | |
| `beTopLevelClasses` / `notBe..` / `beNestedClasses` / `notBe..` / `beLocalClasses` / `notBe..` | same | P2 | |
| `beMemberClasses` / `beInnerClasses` / `beAnonymousClasses` (+ negations) | — | unsupported | |
| `be(Class/String)` / `notBe(..)` | `be(&str)` / `not_be(&str)` | P2 | |
| `containNumberOfElements(pred)` | `contain_number_of_elements(pred)` | P2 | |
| `andShould()` / `andShould(cond)` / `orShould()` / `orShould(cond)` | `and_should()` / `and_should_with(cond)` / `or_should()` / `or_should_with(cond)` | P2 | |
| `[rust-only]` | `be_structs`, `be_traits`, `be_functions`, `be_modules`, `be_unsafe`, `not_be_unsafe`, `be_async`, `be_pub_crate`, `have_only_private_fields`, `reside_in_crate` | P2 | |

### 4.6 Members: `GivenMembers`, `MembersThat`, `MembersShould`, `FieldsThat/Should`, `CodeUnitsThat/Should`, `MethodsThat/Should`, `OnlyBeCalledSpecification`

| Java | Rust | Status | Note |
|---|---|---|---|
| `GivenMembers.that()` / `that(pred)` / `should()` / `should(cond)`; conjunction `and()`/`or()`/`and(pred)`/`or(pred)` | same convention as classes (`_with` for object overloads) | P2 | |
| `MembersThat.haveName` / `doNotHaveName` / `haveNameMatching` / `haveNameNotMatching` / `haveFullName` / `doNotHaveFullName` / `haveFullName{Not}Matching` / `haveName{Not}StartingWith/Containing/EndingWith` | same, snake_case | P2 | |
| `MembersThat.arePublic/areNotPublic/areProtected/…/arePrivate/areNotPrivate` | same | P2 | |
| `haveModifier` / `doNotHaveModifier` | same | P2 | |
| `areAnnotatedWith` / `areNotAnnotatedWith` (3 overloads) | `are_annotated_with(..)` / `are_not_annotated_with(..)` | P2 | |
| `areMetaAnnotatedWith` / `areNotMetaAnnotatedWith` | — | unsupported | |
| `areDeclaredIn(Class/String)` / `areNotDeclaredIn` | `are_declared_in(&str)` / `are_not_declared_in(&str)` | P2 | |
| `areDeclaredInClassesThat(pred)` / `areDeclaredInClassesThat()` | `are_declared_in_classes_that_with(pred)` / `are_declared_in_classes_that()` | P2 | |
| `MembersShould.haveName/notHaveName/…` (mirror of `MembersThat`) | same | P2 | |
| `MembersShould.bePublic/…/bePrivate` and negations | same | P2 | |
| `MembersShould.beAnnotatedWith/notBeAnnotatedWith` | same | P2 | |
| `beMetaAnnotatedWith/notBeMetaAnnotatedWith` | — | unsupported | |
| `beDeclaredIn` / `notBeDeclaredIn` / `beDeclaredInClassesThat()` / `(pred)` | `be_declared_in` / `not_be_declared_in` / `be_declared_in_classes_that()` / `_with` | P2 | |
| `containNumberOfElements` | same | P2 | |
| `MembersShouldConjunction.andShould()/(cond)/orShould()/(cond)` | `and_should()` / `and_should_with` / `or_should()` / `or_should_with` | P2 | |
| `FieldsThat.haveRawType(Class/String/pred)` / `doNotHaveRawType` | `have_raw_type(impl Into<ItemSelector>)` / `do_not_have_raw_type` | P2 | |
| `FieldsThat.areStatic/areNotStatic` | `are_static` / `are_not_static` | P2 | associated consts and `static`/`const` items |
| `FieldsThat.areFinal/areNotFinal` | — | unsupported | Rust fields have no `final` |
| `FieldsShould.haveRawType/notHaveRawType` | same | P2 | |
| `FieldsShould.beAccessedByMethodsThat(pred)` / `notBeAccessedByMethodsThat` | same | P2 | |
| `FieldsShould.beStatic/notBeStatic` | same | P2 | |
| `FieldsShould.beFinal/notBeFinal` | — | unsupported | |
| `CodeUnitsThat.haveRawParameterTypes(Class.../String.../pred)` / `doNotHaveRawParameterTypes` | `have_raw_parameter_types(&[&str])` / `have_raw_parameter_types_with(pred)` / negations | P2 | |
| `CodeUnitsThat.haveRawReturnType(..)` / `doNotHaveRawReturnType` | `have_raw_return_type(impl Into<ItemSelector>)` / negation | P2 | |
| `CodeUnitsThat.declareThrowableOfType(..)` / `doNotDeclareThrowableOfType` | `declare_throwable_of_type(..)` / negation | P2 partial | `Result<_, E>` |
| `CodeUnitsShould.haveRawParameterTypes` / `haveRawReturnType` / `declareThrowableOfType` (+ `not` forms) | same | P2 | |
| `CodeUnitsShould.onlyBeCalled()` → `OnlyBeCalledSpecification.byClassesThat(pred)/byClassesThat()/byCodeUnitsThat/byMethodsThat/byConstructorsThat` | `only_be_called()` → `by_classes_that_with(pred)` / `by_classes_that()` / `by_code_units_that(pred)` / `by_methods_that(pred)` / `by_constructors_that(pred)` | P2 | |
| `MethodsThat.areStatic/areNotStatic` | same | P2 | no `self` receiver |
| `MethodsThat.areFinal/areNotFinal` | same | P2 partial | inherent (non-overridable) methods |
| `MethodsShould.beStatic/notBeStatic/beFinal/notBeFinal` | same | P2 partial | |
| `[rust-only]` (methods/code units) | `are_unsafe`, `are_async`, `are_const`, `have_self_receiver`, `take_self_by_value`, `take_self_by_ref`, `take_self_by_mut_ref`, `are_trait_methods`, `are_default_methods`, `be_unsafe`, `not_be_unsafe`, … | P2 | |

### 4.7 `ArchConditions` → `archunit::lang::conditions`

Every static factory in `ArchConditions` maps 1:1 with the naming rules of §1. The table lists
only entries that deviate.

| Java | Rust | Status | Note |
|---|---|---|---|
| all `getField/setField/accessField[Where]`, `callMethod[Where]`, `callConstructor[Where]`, `callCodeUnitWhere`, `only*`, `accessClassesThat`, `onlyAccessClassesThat`, `dependOnClassesThat`, `haveAnyDependenciesThat`, `transitivelyDependOnClassesThat`, `onlyDependOnClassesThat`, `onlyBeAccessedByClassesThat`, `accessClassesThatResideIn[AnyPackage]`, `onlyBeAccessedByAnyPackage`, `onlyHaveDependentsInAnyPackage`, `onlyHaveDependentClassesThat`, `onlyHaveDependentsWhere`, `onlyHaveDependenciesInAnyPackage`, `onlyHaveDependenciesWhere` | same, snake_case | P2 | |
| `and(a, b)` / `or(a, b)` / `never(c)` / `not(c)` | same | P2 | |
| `be(Class/String)` / `notBe` / name conditions / package conditions / visibility conditions / `haveModifier` / `beAnnotatedWith` / `implement` / `beAssignableTo/From` / `beInterfaces` / `beEnums` / `beTopLevelClasses` / `beNestedClasses` / `beLocalClasses` | same | P2 | |
| `beMetaAnnotatedWith`, `beRecords`, `beMemberClasses`, `beInnerClasses`, `beAnonymousClasses`, `haveOnlyFinalFields` | — | unsupported | see §4.5 |
| `haveOnlyPrivateConstructors` | same | P2 partial | |
| `containNumberOfElements(pred)` | same | P2 | |
| `beDeclaredIn` / `notBeDeclaredIn` / `beDeclaredInClassesThat` | same | P2 | |
| `haveRawType` / `haveRawParameterTypes` / `haveRawReturnType` / `declareThrowableOfType` | same | P2 | |
| `onlyBeCalledByClassesThat` / `..ByCodeUnitsThat` / `..ByMethodsThat` / `..ByConstructorsThat` / `beAccessedByMethodsThat` | same | P2 | |
| `have(pred)` / `be(pred)` (`ConditionByPredicate`) | `have(pred)` / `be(pred)` | P2 | |
| `AllDependenciesCondition.ignoreDependency(..)` (returned by `onlyHaveDependencies*`) | `AllDependenciesCondition::ignore_dependency(..)` | P2 | |
| `[rust-only]` | `use_unsafe_blocks`, `call_function(&str)`, `invoke_macro(&str)`, `access_standard_streams`, `panic`, `call_unwrap`, `call_process_exit`, `return_generic_errors` | P2/P5 | see §5.5 |

### 4.8 `ArchPredicates`

| Java | Rust | Status |
|---|---|---|
| `is(pred)` / `are(pred)` / `has(pred)` / `have(pred)` / `be(pred)` | same | P2 |

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
| `Architectures.layeredArchitecture()` → `DependencySettings` | `Architectures::layered_architecture()` / `layered_architecture()` | P3 | |
| `.consideringAllDependencies()` / `.consideringOnlyDependenciesInLayers()` / `.consideringOnlyDependenciesInAnyPackage(..)` | same | P3 | |
| `LayeredArchitecture.layer(name)` / `optionalLayer(name)` → `LayerDefinition.definedBy(String...)` / `definedBy(pred)` | `layer(&str)` / `optional_layer(&str)` → `defined_by(&[&str])` / `defined_by_with(pred)` | P3 | |
| `withOptionalLayers(bool)` | same | P3 | |
| `whereLayer(name)` → `LayerDependencySpecification.mayNotBeAccessedByAnyLayer()` / `mayOnlyBeAccessedByLayers(..)` / `mayNotAccessAnyLayer()` / `mayOnlyAccessLayers(..)` | same | P3 | |
| `ignoreDependency(Class, Class)` / `(String, String)` / `(pred, pred)` | `ignore_dependency(&str, &str)` / `ignore_dependency_with(pred, pred)` | P3 | |
| `ensureAllClassesAreContainedInArchitecture()` / `..Ignoring(String...)` / `..Ignoring(pred)` | same (`_with` for pred) | P3 | |
| `as(..)` / `because(..)` / `allowEmptyShould(..)` / `check` / `evaluate` / `getDescription` | same | P3 | |
| description text | identical: `Layered architecture considering all dependencies, consisting of\nlayer 'X' ('..x..')\nwhere layer 'X' may only be accessed by layers ['Y']` | P3 | |
| `Architectures.onionArchitecture()` | `Architectures::onion_architecture()` | P3 | |
| `.domainModels(..)` / `.domainServices(..)` / `.applicationServices(..)` / `.adapter(name, ..)` (String... and pred overloads) | same (`_with` for pred) | P3 | |
| `withOptionalLayers`, `ignoreDependency` (3), `ensureAllClassesAreContainedInArchitecture[Ignoring]`, `as`, `because`, `allowEmptyShould`, `check`, `evaluate`, `getDescription` | same | P3 | |

### 5.2 Slices (`library.dependencies`)

| Java | Rust | Status |
|---|---|---|
| `SlicesRuleDefinition.slices()` → `Creator.matching(pattern[, priority])` / `assignedFrom(SliceAssignment[, priority])` | `SlicesRuleDefinition::slices().matching(..)` / `assigned_from(..)` (+ `_with_priority`) | P3 |
| `GivenSlices.namingSlices(pattern)` / `as(..)` / `that(pred)` / `should()` | `naming_slices(&str)` / `as_(..)` / `that(pred)` / `should()` | P3 |
| `GivenSlicesConjunction.and(pred)` / `or(pred)` / `as` / `should()` | same | P3 |
| `SlicesShould.beFreeOfCycles()` / `notDependOnEachOther()` | `be_free_of_cycles()` / `not_depend_on_each_other()` | P3 |
| `SliceRule.ignoreDependency(Class,Class)/(String,String)/(pred,pred)` / `as` / `because` / `allowEmptyShould` / `check` / `evaluate` | same (`_with` for pred) | P3 |
| `Slices`, `Slice` (`getNamePart(i)`, `getDependenciesFromSelf/ToSelf`, `as`), `SliceDependency`, `SliceAssignment`, `SliceIdentifier.of(..)/ignore()` | same | P3 |
| cycle report text (`Cycle detected: Slice a -> \n                Slice b -> ...` + numbered dependency details) | identical | P3 |
| `cycles.maxNumberToDetect` / `cycles.maxNumberOfDependenciesPerEdge` | `archunit.toml` `[cycles] max_number_to_detect`, `max_number_of_dependencies_per_edge` | P3 |
| `CycleDetector.detectCycles(nodes, edges)`, `Cycle`, `Cycles`, `Edge` | `cycle_detection::{CycleDetector, Cycle, Cycles, Edge}` | P3 |

### 5.3 Modules (`library.modules`)

| Java | Rust | Status | Note |
|---|---|---|---|
| `ModuleRuleDefinition.modules().definedByPackages(pattern)` / `.derivingNameFromPattern(..)` | `modules().defined_by_packages(..)` / `deriving_name_from_pattern(..)` | deferred | Shares the slices infrastructure; cheap once P3 lands |
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
| `@AnalyzeClasses(packages, packagesOf, classes, locations, wholeClasspath, importOptions, cacheMode)` on a test class | `#[analyze_classes(packages = [..], packages_of = [..], items = [..], locations = Provider, whole_workspace = true, import_options = [..], cache_mode = PerClass)]` on an inline `mod` | P4 | `wholeClasspath` → `whole_workspace`. Without arguments the crate containing the test module is analyzed |
| `@AnalyzeClasses` as a meta-annotation | — | unsupported | Attribute macros cannot be aliased; use a `macro_rules!` wrapper |
| `@ArchTest` on a `static ArchRule` field | `#[arch_test] fn rule() -> impl ArchRule` | P4 | Rust statics cannot hold runtime-built trait objects ergonomically |
| `@ArchTest` on a `static void method(JavaClasses)` | `#[arch_test] fn rule(items: &RustItems)` | P4 | |
| `@ArchTest ArchTests.in(OtherRules.class)` | `#[arch_test] arch_tests!(in other_rules)` / `ArchTests::in_(other_rules::rules)` | P4 | `other_rules` is a module annotated with `#[arch_rules]` (no import config); its rules are evaluated against the enclosing `#[analyze_classes]` import |
| `@ArchIgnore(reason)` | `#[arch_ignore(reason = "..")]` → `#[ignore = ".."]` | P4 | |
| `@ArchTag("x")` | `#[arch_tag("x")]` | P4 partial | Recorded in the generated test name suffix (`__tag_x`) so `cargo test x` selects it; no native tagging in `cargo test` |
| `CacheMode.FOREVER` / `PER_CLASS` | `CacheMode::Forever` / `PerClass` | P4 | `Forever` = process-wide cache keyed by import configuration; `PerClass` = per-module `OnceLock`. No soft references |
| `LocationProvider` | `trait LocationProvider { fn get(test_module: &str) -> Vec<Location> }` | P4 | |
| `junit.testFilter` | — | unsupported | Use `cargo test <name>` |
| `junit.displayName.replaceUnderscoresBySpaces` | — | unsupported | `cargo test` names are identifiers |
| JUnit 4 `ArchUnitRunner`, JUnit 6 engine | — | unsupported | Not applicable |
| plain-function API (no macros) | `archunit::harness::analyze_classes().packages(&[..]).import_options(..).cache_mode(..).import() -> Arc<RustItems>` with the same cache | P4 | |

---

## 7. Configuration (`ArchConfiguration`, `archunit.properties`) → `archunit.toml`

| Java (`archunit.properties`) | `archunit.toml` | Status | Note |
|---|---|---|---|
| file at classpath root | `archunit.toml` in the crate root (searched upward to the workspace root) | P4 | |
| `-Darchunit.key=value` override | `ARCHUNIT_KEY=value` environment variable (`.` and `-` → `_`, upper-cased) | P4 | |
| `ArchConfiguration.get()` / `getProperty` / `setProperty` / `containsProperty` / `getPropertyOrDefault` / `getSubProperties` / `reset` / `withThreadLocalScope` | `ArchConfiguration::get()` (global, `RwLock`) / `property` / `set_property` / `contains_property` / `property_or_default` / `sub_properties` / `reset` / `with_thread_local_scope` | P4 | |
| `resolveMissingDependenciesFromClassPath` | `resolve_missing_dependencies_from_classpath` (default `false`) | P1 | When `true`, missing items from dependency crates are parsed from the cargo registry checkout. Default differs from Java (`true`) because parsing large crates is slow; `std`/`core`/`alloc` are always stubs |
| `classResolver` / `classResolver.args` (`SelectedClassResolverFromClasspath`) | `class_resolver.packages = ["tokio..", "serde.."]` | P1 partial | Only the "selected packages" resolver; no custom resolver by class name |
| `import.dependencyResolutionProcess.maxIterationsFor{MemberTypes,AccessesToTypes,Supertypes,PermittedSubclasses,EnclosingTypes,AnnotationTypes,GenericSignatureTypes}` | `[import.dependency_resolution_process] max_iterations_for_member_types`, … | P1 partial | Same defaults; `permitted_subclasses` accepted and ignored |
| `enableMd5InClassSources` | — | unsupported | No class files |
| `archRule.failOnEmptyShould` | `[arch_rule] fail_on_empty_should` (default `true`) | P2 | |
| `failureDisplayFormat` | — (programmatic only) | P2 partial | |
| `extension.<id>.enabled` / `extension.<id>.<prop>` | `[extension.<id>] enabled`, … | deferred | |
| `cycles.maxNumberToDetect` / `cycles.maxNumberOfDependenciesPerEdge` | `[cycles] max_number_to_detect`, `max_number_of_dependencies_per_edge` | P3 | |
| `freeze.*` | `[freeze] …` (see §5.6) | P5 | |
| `junit.*` | — | unsupported | see §6 |
| `[rust-only]` | `[import] include_targets = ["lib", "bin", "test", "example", "bench"]`, `[import] exclude_binaries_from_coding_rules = true`, `[report] item_prefix = "Item"` (allows `Struct`/`Trait`/… kind prefixes instead of the generic `Item`) | P1/P5 | |

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

## 9. Phase status log

| Phase | Status | Summary |
|---|---|---|
| 0 | done | Research, this mapping, PLAN.md, CLAUDE.md, NOTICE |
| 1 | pending | |
| 2 | pending | |
| 3 | pending | |
| 4 | pending | |
| 5 | pending | |
| 6 | pending | |
