use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use super::annotation::RustAnnotation;
use super::dependency::Dependency;
use super::graph::{Graph, ItemId};
use super::modifiers::{RustModifier, Visibility};
use super::properties::{
    CanBeAnnotated, HasErrorTypes, HasFullName, HasModifiers, HasName, HasSourceCodeLocation,
    HasTypeParameters, ItemSelector,
};
use super::rust_access::{AccessKind, RustAccess};
use super::rust_items::RustItems;
use super::rust_member::{
    MemberKind, RustCodeUnit, RustConstructor, RustField, RustMember, RustMethod, RustVariant,
};
use super::rust_module::RustModule;
use super::source_code_location::SourceCodeLocation;
use super::types::{RustType, TypeParameter};
use crate::base::HasDescription;

/// The kind of a [`RustItem`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    /// `struct`
    Struct,
    /// `enum`
    Enum,
    /// `union`
    Union,
    /// `trait`
    Trait,
    /// A free `fn`.
    Function,
    /// An `impl` block.
    Impl,
    /// A `mod` (including crate roots).
    Module,
    /// `type X = ..;`
    TypeAlias,
    /// `const`
    Const,
    /// `static`
    Static,
    /// `macro_rules!` or a procedural macro function.
    Macro,
    /// `extern crate`
    ExternCrate,
    /// A primitive type such as `u32`.
    Primitive,
    /// An item outside of the import whose kind is unknown (a stub).
    Unknown,
}

impl fmt::Display for ItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            ItemKind::Struct => "Struct",
            ItemKind::Enum => "Enum",
            ItemKind::Union => "Union",
            ItemKind::Trait => "Trait",
            ItemKind::Function => "Function",
            ItemKind::Impl => "Impl",
            ItemKind::Module => "Module",
            ItemKind::TypeAlias => "TypeAlias",
            ItemKind::Const => "Const",
            ItemKind::Static => "Static",
            ItemKind::Macro => "Macro",
            ItemKind::ExternCrate => "ExternCrate",
            ItemKind::Primitive => "Primitive",
            ItemKind::Unknown => "Item",
        };
        f.write_str(text)
    }
}

/// The cargo target an item was imported from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TargetKind {
    /// A library target.
    Lib,
    /// A procedural macro library.
    ProcMacro,
    /// A binary target with the given name.
    Bin(String),
    /// An integration test target.
    Test(String),
    /// An example target.
    Example(String),
    /// A benchmark target.
    Bench(String),
}

impl TargetKind {
    /// Whether this is a binary target.
    pub fn is_binary(&self) -> bool {
        matches!(self, TargetKind::Bin(_))
    }

    /// The cargo kind name: `lib`, `proc-macro`, `bin`, `test`, `example` or `bench`.
    pub fn kind_name(&self) -> &'static str {
        match self {
            TargetKind::Lib => "lib",
            TargetKind::ProcMacro => "proc-macro",
            TargetKind::Bin(_) => "bin",
            TargetKind::Test(_) => "test",
            TargetKind::Example(_) => "example",
            TargetKind::Bench(_) => "bench",
        }
    }
}

/// An item of Rust code: a struct, enum, union, trait, function, impl block, module, type
/// alias, const, static, macro, or a stub for something outside the import (`JavaClass`).
#[derive(Clone)]
pub struct RustItem {
    pub(crate) graph: Arc<Graph>,
    pub(crate) id: ItemId,
}

impl PartialEq for RustItem {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && Arc::ptr_eq(&self.graph, &other.graph)
    }
}

impl Eq for RustItem {}

impl Hash for RustItem {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialOrd for RustItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RustItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.data()
            .name
            .cmp(&other.data().name)
            .then(self.id.cmp(&other.id))
    }
}

impl fmt::Debug for RustItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RustItem({} {})", self.data().kind, self.data().name)
    }
}

impl fmt::Display for RustItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.data().name)
    }
}

impl HasDescription for RustItem {
    fn description(&self) -> String {
        let prefix = match self.kind() {
            ItemKind::Module => "Module",
            _ => "Item",
        };
        format!("{prefix} <{}>", self.name())
    }
}

impl HasName for RustItem {
    fn name(&self) -> String {
        self.data().name.clone()
    }
}

impl HasFullName for RustItem {
    fn full_name(&self) -> String {
        self.data().name.clone()
    }
}

impl HasModifiers for RustItem {
    fn modifiers(&self) -> Vec<RustModifier> {
        self.data().modifiers.clone()
    }
}

impl CanBeAnnotated for RustItem {
    fn annotations(&self) -> Vec<RustAnnotation> {
        self.data().annotations.clone()
    }
}

impl HasSourceCodeLocation for RustItem {
    fn source_code_location(&self) -> SourceCodeLocation {
        self.data().location.clone()
    }
}

impl HasTypeParameters for RustItem {
    fn type_parameters(&self) -> Vec<TypeParameter> {
        self.data().type_parameters.clone()
    }
}

impl HasErrorTypes for RustItem {
    fn error_types(&self) -> Vec<RustItem> {
        self.code_units()
            .iter()
            .flat_map(|c| c.error_types())
            .collect()
    }
}

impl RustItem {
    pub(crate) fn new(graph: Arc<Graph>, id: ItemId) -> Self {
        Self { graph, id }
    }

    pub(crate) fn data(&self) -> &super::graph::ItemData {
        &self.graph.items[self.id]
    }

    pub(crate) fn item(&self, id: ItemId) -> RustItem {
        RustItem::new(Arc::clone(&self.graph), id)
    }

    fn member(&self, id: usize) -> RustMember {
        RustMember::new(Arc::clone(&self.graph), id)
    }

    fn access(&self, id: usize) -> RustAccess {
        RustAccess::new(Arc::clone(&self.graph), id)
    }

    // ---- naming ---------------------------------------------------------------------------

    /// The fully qualified, crate-rooted name, e.g. `my_app::domain::order::Order`
    /// (`JavaClass.getName()`).
    pub fn name(&self) -> String {
        self.data().name.clone()
    }

    /// The last path segment (`JavaClass.getSimpleName()`).
    pub fn simple_name(&self) -> String {
        self.data().simple_name.clone()
    }

    /// Same as [`name`](Self::name) (`JavaClass.getFullName()`).
    pub fn full_name(&self) -> String {
        self.name()
    }

    /// The path of the enclosing module (`JavaClass.getPackageName()`).
    ///
    /// A module item resides in itself: the `use` declarations it owns belong to that package.
    pub fn package_name(&self) -> String {
        self.package().name()
    }

    /// The enclosing module (`JavaClass.getPackage()`); a module item returns itself.
    pub fn package(&self) -> RustModule {
        match self.data().package {
            Some(package) if self.kind() != ItemKind::Module => {
                RustModule::new(Arc::clone(&self.graph), package)
            }
            _ => RustModule::new(Arc::clone(&self.graph), self.id),
        }
    }

    /// The name of the crate this item belongs to.
    pub fn crate_name(&self) -> String {
        self.data().crate_name.clone()
    }

    /// The cargo target this item was imported from.
    pub fn cargo_target(&self) -> TargetKind {
        self.data().target.clone()
    }

    /// `Item <name>` (or `Module <name>` for modules); used in failure reports
    /// (`JavaClass.getDescription()`).
    pub fn description(&self) -> String {
        HasDescription::description(self)
    }

    // ---- kind ---------------------------------------------------------------------------

    /// The kind of item.
    pub fn kind(&self) -> ItemKind {
        self.data().kind
    }

    /// `struct`
    pub fn is_struct(&self) -> bool {
        self.kind() == ItemKind::Struct
    }

    /// `enum` (`JavaClass.isEnum()`).
    pub fn is_enum(&self) -> bool {
        self.kind() == ItemKind::Enum
    }

    /// `union`
    pub fn is_union(&self) -> bool {
        self.kind() == ItemKind::Union
    }

    /// `trait` (`JavaClass.isInterface()`).
    pub fn is_interface(&self) -> bool {
        self.kind() == ItemKind::Trait
    }

    /// `trait`; alias of [`is_interface`](Self::is_interface).
    pub fn is_trait(&self) -> bool {
        self.is_interface()
    }

    /// A free function.
    pub fn is_function(&self) -> bool {
        self.kind() == ItemKind::Function
    }

    /// A module.
    pub fn is_module(&self) -> bool {
        self.kind() == ItemKind::Module
    }

    /// An `impl` block.
    pub fn is_impl(&self) -> bool {
        self.kind() == ItemKind::Impl
    }

    /// A type alias.
    pub fn is_type_alias(&self) -> bool {
        self.kind() == ItemKind::TypeAlias
    }

    /// A `const` item.
    pub fn is_const(&self) -> bool {
        self.kind() == ItemKind::Const
    }

    /// A `static` item.
    pub fn is_static(&self) -> bool {
        self.kind() == ItemKind::Static
    }

    /// A macro definition.
    pub fn is_macro(&self) -> bool {
        self.kind() == ItemKind::Macro
    }

    /// A function defining an attribute or derive macro (`JavaClass.isAnnotation()`).
    pub fn is_annotation(&self) -> bool {
        self.is_macro()
            && self
                .annotations()
                .iter()
                .any(|a| matches!(a.path(), "proc_macro_attribute" | "proc_macro_derive"))
    }

    /// A primitive type such as `u32` (`JavaClass.isPrimitive()`).
    pub fn is_primitive(&self) -> bool {
        self.data().is_primitive
    }

    /// Whether the item's source was imported, as opposed to being a stub for a name referenced
    /// from outside the import (`JavaClass.isFullyImported()`).
    pub fn is_fully_imported(&self) -> bool {
        self.data().fully_imported
    }

    /// Whether the item is test code: inside `#[cfg(test)]`, a `#[test]`, under `tests/`, or in
    /// a test/bench target.
    pub fn is_test_code(&self) -> bool {
        self.data().is_test_code
    }

    /// `#[non_exhaustive]` enums and structs, or traits with a private supertrait
    /// (`JavaClass.isSealed()`).
    pub fn is_sealed(&self) -> bool {
        self.has_modifier(&RustModifier::NonExhaustive)
            || (self.is_trait()
                && self
                    .supertraits()
                    .iter()
                    .any(|t| t.visibility() == Visibility::Private))
    }

    /// Declared directly in a module body (`JavaClass.isTopLevelClass()`).
    pub fn is_top_level_item(&self) -> bool {
        self.data().enclosing_item.is_none()
    }

    /// Declared inside a function body or another item body (`JavaClass.isNestedClass()`).
    pub fn is_nested_item(&self) -> bool {
        self.data().enclosing_item.is_some()
    }

    /// Declared inside a function body (`JavaClass.isLocalClass()`).
    pub fn is_local_item(&self) -> bool {
        self.enclosing_item().is_some_and(|e| {
            matches!(
                e.kind(),
                ItemKind::Function | ItemKind::Impl | ItemKind::Trait
            )
        })
    }

    /// The item this one is declared in, for nested items (`JavaClass.getEnclosingClass()`).
    pub fn enclosing_item(&self) -> Option<RustItem> {
        self.data().enclosing_item.map(|id| self.item(id))
    }

    /// The function this item is declared in, for local items (`JavaClass.getEnclosingCodeUnit()`).
    pub fn enclosing_code_unit(&self) -> Option<RustCodeUnit> {
        self.data()
            .enclosing_code_unit
            .map(|id| RustCodeUnit(self.member(id)))
    }

    // ---- modifiers and annotations ------------------------------------------------------

    /// The visibility as written.
    pub fn visibility(&self) -> Visibility {
        self.data().visibility.clone()
    }

    /// All modifiers (`JavaClass.getModifiers()`).
    pub fn modifiers(&self) -> Vec<RustModifier> {
        self.data().modifiers.clone()
    }

    /// Attributes and derives (`JavaClass.getAnnotations()`).
    pub fn annotations(&self) -> Vec<RustAnnotation> {
        self.data().annotations.clone()
    }

    /// Whether an attribute or derive matching `selector` is present (`JavaClass.isAnnotatedWith(..)`).
    pub fn is_annotated_with(
        &self,
        selector: impl Into<super::properties::AnnotationSelector>,
    ) -> bool {
        CanBeAnnotated::is_annotated_with(self, selector)
    }

    /// The annotation with the given name (`JavaClass.getAnnotationOfType(..)`); panics if absent.
    pub fn annotation_of_type(&self, name: &str) -> RustAnnotation {
        CanBeAnnotated::annotation_of_type(self, name)
    }

    /// The annotation with the given name, if present (`JavaClass.tryGetAnnotationOfType(..)`).
    pub fn try_annotation_of_type(&self, name: &str) -> Option<RustAnnotation> {
        CanBeAnnotated::try_annotation_of_type(self, name)
    }

    /// The generic type parameters (`JavaClass.getTypeParameters()`).
    pub fn type_parameters(&self) -> Vec<TypeParameter> {
        self.data().type_parameters.clone()
    }

    /// The declaration location (`JavaClass.getSourceCodeLocation()`).
    pub fn source_code_location(&self) -> SourceCodeLocation {
        self.data().location.clone()
    }

    /// The declared type of a `const`, `static` or type alias; `None` for other kinds.
    pub fn item_type(&self) -> Option<RustType> {
        self.data()
            .item_type
            .clone()
            .or_else(|| self.data().alias_target.clone())
    }

    /// For `impl` blocks: the self type.
    pub fn impl_self_type(&self) -> Option<RustType> {
        self.data().impl_info.as_ref().map(|i| i.self_type.clone())
    }

    /// For trait `impl` blocks: the implemented trait.
    pub fn impl_trait(&self) -> Option<RustItem> {
        self.data()
            .impl_info
            .as_ref()
            .and_then(|i| i.trait_)
            .map(|id| self.item(id))
    }

    /// Identity; kept for parity with `JavaClass.toErasure()`.
    pub fn to_erasure(&self) -> RustItem {
        self.clone()
    }

    /// This item viewed as a module, if it is one.
    pub fn as_module(&self) -> Option<RustModule> {
        self.is_module()
            .then(|| RustModule::new(Arc::clone(&self.graph), self.id))
    }

    // ---- members ------------------------------------------------------------------------

    fn members_iter(&self) -> impl Iterator<Item = RustMember> + '_ {
        self.data().members.iter().map(|&id| self.member(id))
    }

    /// Directly declared fields, methods, variants, associated consts and types
    /// (`JavaClass.getMembers()`).
    pub fn members(&self) -> Vec<RustMember> {
        self.members_iter().collect()
    }

    /// Members including trait-provided default items of implemented traits
    /// (`JavaClass.getAllMembers()`).
    pub fn all_members(&self) -> Vec<RustMember> {
        let mut result = self.members();
        let own_names: HashSet<String> = result.iter().map(|m| m.name()).collect();
        for trait_ in self.all_implemented_traits() {
            for member in trait_.members() {
                if member.has_body() && !own_names.contains(&member.name()) {
                    result.push(member);
                }
            }
        }
        result
    }

    /// Struct, variant and union fields plus associated consts (`JavaClass.getFields()`).
    pub fn fields(&self) -> Vec<RustField> {
        self.members_iter()
            .filter(|m| matches!(m.kind(), MemberKind::Field | MemberKind::AssocConst))
            .map(RustField)
            .collect()
    }

    /// Same as [`fields`](Self::fields); Rust has no inherited fields (`JavaClass.getAllFields()`).
    pub fn all_fields(&self) -> Vec<RustField> {
        self.fields()
    }

    /// The field with the given name (`JavaClass.getField(..)`).
    ///
    /// # Panics
    /// If there is no such field.
    pub fn field(&self, name: &str) -> RustField {
        self.try_field(name)
            .unwrap_or_else(|| panic!("No field '{name}' in {}", self.name()))
    }

    /// The field with the given name, if any (`JavaClass.tryGetField(..)`).
    pub fn try_field(&self, name: &str) -> Option<RustField> {
        self.fields().into_iter().find(|f| f.name() == name)
    }

    /// Associated functions and trait method declarations, and for free functions the
    /// function itself (`JavaClass.getMethods()`).
    pub fn methods(&self) -> Vec<RustMethod> {
        self.members_iter()
            .filter(|m| m.kind() == MemberKind::Method)
            .map(RustMethod)
            .collect()
    }

    /// Methods including default methods of implemented traits (`JavaClass.getAllMethods()`).
    pub fn all_methods(&self) -> Vec<RustMethod> {
        self.all_members()
            .into_iter()
            .filter(|m| m.kind() == MemberKind::Method)
            .map(RustMethod)
            .collect()
    }

    /// The method with the given name (`JavaClass.getMethod(..)`).
    ///
    /// # Panics
    /// If there is no such method.
    pub fn method(&self, name: &str) -> RustMethod {
        self.try_method(name)
            .unwrap_or_else(|| panic!("No method '{name}' in {}", self.name()))
    }

    /// The method with the given name, if any (`JavaClass.tryGetMethod(..)`).
    pub fn try_method(&self, name: &str) -> Option<RustMethod> {
        self.methods().into_iter().find(|m| m.name() == name)
    }

    /// Associated functions classified as constructors: no `self` receiver and a return type
    /// of `Self`, `Option<Self>`, `Result<Self, _>` or similar (`JavaClass.getConstructors()`).
    pub fn constructors(&self) -> Vec<RustConstructor> {
        self.members_iter()
            .filter(|m| m.is_constructor())
            .map(RustConstructor)
            .collect()
    }

    /// Same as [`constructors`](Self::constructors) (`JavaClass.getAllConstructors()`).
    pub fn all_constructors(&self) -> Vec<RustConstructor> {
        self.constructors()
    }

    /// Everything that has a body: methods and, for free functions, the function itself
    /// (`JavaClass.getCodeUnits()`).
    pub fn code_units(&self) -> Vec<RustCodeUnit> {
        self.members_iter()
            .filter(|m| m.is_code_unit())
            .map(RustCodeUnit)
            .collect()
    }

    /// Enum variants (`JavaClass.getEnumConstants()`).
    pub fn variants(&self) -> Vec<RustVariant> {
        self.members_iter()
            .filter(|m| m.kind() == MemberKind::Variant)
            .map(RustVariant)
            .collect()
    }

    /// The variant with the given name (`JavaClass.getEnumConstant(..)`).
    pub fn variant(&self, name: &str) -> Option<RustVariant> {
        self.variants().into_iter().find(|v| v.name() == name)
    }

    // ---- hierarchy ----------------------------------------------------------------------

    /// Direct supertraits of a trait (`JavaClass.getRawSuperclass()` / `getInterfaces()` for traits).
    pub fn supertraits(&self) -> Vec<RustItem> {
        self.data()
            .supertraits
            .iter()
            .map(|&id| self.item(id))
            .collect()
    }

    /// All transitive supertraits (`JavaClass.getAllRawSuperclasses()`).
    pub fn all_supertraits(&self) -> Vec<RustItem> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        let mut queue: VecDeque<ItemId> = self.data().supertraits.iter().copied().collect();
        while let Some(id) = queue.pop_front() {
            if seen.insert(id) {
                result.push(self.item(id));
                queue.extend(self.graph.items[id].supertraits.iter().copied());
            }
        }
        result
    }

    /// This trait followed by its transitive supertraits (`JavaClass.getClassHierarchy()`).
    pub fn trait_hierarchy(&self) -> Vec<RustItem> {
        let mut result = vec![self.clone()];
        result.extend(self.all_supertraits());
        result
    }

    /// Traits directly implemented by this type (`JavaClass.getRawInterfaces()`).
    pub fn implemented_traits(&self) -> Vec<RustItem> {
        self.data()
            .implemented_traits
            .iter()
            .map(|&id| self.item(id))
            .collect()
    }

    /// Traits implemented directly or through supertraits (`JavaClass.getAllRawInterfaces()`).
    pub fn all_implemented_traits(&self) -> Vec<RustItem> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        let mut queue: VecDeque<ItemId> = self.data().implemented_traits.iter().copied().collect();
        queue.extend(self.data().supertraits.iter().copied());
        while let Some(id) = queue.pop_front() {
            if seen.insert(id) {
                result.push(self.item(id));
                queue.extend(self.graph.items[id].supertraits.iter().copied());
            }
        }
        result
    }

    /// Types implementing this trait directly (`JavaClass.getSubclasses()`).
    pub fn implementors(&self) -> Vec<RustItem> {
        self.graph
            .implementors
            .get(self.id)
            .map(|ids| ids.iter().map(|&id| self.item(id)).collect())
            .unwrap_or_default()
    }

    /// Traits that have this trait as a direct supertrait.
    pub fn subtraits(&self) -> Vec<RustItem> {
        self.graph
            .subtraits
            .get(self.id)
            .map(|ids| ids.iter().map(|&id| self.item(id)).collect())
            .unwrap_or_default()
    }

    /// All types and traits assignable to this trait: implementors and subtraits, transitively
    /// (`JavaClass.getAllSubclasses()`).
    pub fn all_implementors(&self) -> Vec<RustItem> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        let mut queue: VecDeque<ItemId> = VecDeque::new();
        queue.push_back(self.id);
        while let Some(id) = queue.pop_front() {
            for &next in self.graph.implementors[id]
                .iter()
                .chain(self.graph.subtraits[id].iter())
            {
                if seen.insert(next) {
                    result.push(self.item(next));
                    queue.push_back(next);
                }
            }
        }
        result
    }

    /// This item plus all traits it is assignable to (`JavaClass.getAllClassesSelfIsAssignableTo()`).
    pub fn all_items_self_is_assignable_to(&self) -> Vec<RustItem> {
        let mut result = vec![self.clone()];
        result.extend(self.all_implemented_traits());
        result
    }

    /// Whether this item is, or implements/extends, the selected trait or type
    /// (`JavaClass.isAssignableTo(..)`).
    pub fn is_assignable_to(&self, selector: impl Into<ItemSelector>) -> bool {
        let selector = selector.into();
        self.all_items_self_is_assignable_to()
            .iter()
            .any(|item| selector.matches(item))
    }

    /// Whether the selected item is, or implements/extends, this item
    /// (`JavaClass.isAssignableFrom(..)`).
    pub fn is_assignable_from(&self, selector: impl Into<ItemSelector>) -> bool {
        let selector = selector.into();
        selector.matches(self)
            || self
                .all_implementors()
                .iter()
                .any(|item| selector.matches(item))
    }

    /// Whether this item has the given full name or public re-export path
    /// (`JavaClass.isEquivalentTo(..)`).
    pub fn is_equivalent_to(&self, name: &str) -> bool {
        self.data().name == name || self.data().aliases.iter().any(|a| a == name)
    }

    /// Public re-export paths under which this item is also reachable, e.g. `my_app::Order`
    /// for `pub use domain::model::Order` in `lib.rs`.
    pub fn aliases(&self) -> Vec<String> {
        self.data().aliases.clone()
    }

    /// Whether this item implements (directly or through supertraits) the selected trait
    /// (`JavaClass.Predicates.implement(..)`).
    pub fn implements(&self, selector: impl Into<ItemSelector>) -> bool {
        let selector = selector.into();
        self.all_implemented_traits()
            .iter()
            .any(|item| selector.matches(item))
    }

    // ---- accesses -----------------------------------------------------------------------

    fn accesses_from_members(&self, ids: &[usize]) -> Vec<RustAccess> {
        ids.iter()
            .flat_map(|&member| self.graph.accesses_from_member[member].iter())
            .map(|&id| self.access(id))
            .collect()
    }

    /// Accesses originating in this item's code units (`JavaClass.getAccessesFromSelf()`).
    pub fn accesses_from_self(&self) -> Vec<RustAccess> {
        self.accesses_from_members(&self.data().members)
    }

    /// Accesses from this item and from items declared inside it
    /// (`JavaClass.getAllAccessesFromSelf()`).
    pub fn all_accesses_from_self(&self) -> Vec<RustAccess> {
        let mut result = self.accesses_from_self();
        for (id, data) in self.graph.items.iter().enumerate() {
            if data.enclosing_item == Some(self.id) {
                result.extend(self.item(id).all_accesses_from_self());
            }
        }
        result
    }

    /// Accesses targeting members of this item (`JavaClass.getAccessesToSelf()`).
    pub fn accesses_to_self(&self) -> Vec<RustAccess> {
        self.graph
            .accesses_to_item
            .get(self.id)
            .map(|ids| ids.iter().map(|&id| self.access(id)).collect())
            .unwrap_or_default()
    }

    fn filter_kind(
        accesses: Vec<RustAccess>,
        pred: impl Fn(AccessKind) -> bool,
    ) -> Vec<RustAccess> {
        accesses.into_iter().filter(|a| pred(a.kind())).collect()
    }

    /// `JavaClass.getFieldAccessesFromSelf()`.
    pub fn field_accesses_from_self(&self) -> Vec<RustAccess> {
        Self::filter_kind(self.accesses_from_self(), |k| {
            matches!(k, AccessKind::FieldAccess(_))
        })
    }

    /// `JavaClass.getFieldAccessesToSelf()`.
    pub fn field_accesses_to_self(&self) -> Vec<RustAccess> {
        Self::filter_kind(self.accesses_to_self(), |k| {
            matches!(k, AccessKind::FieldAccess(_))
        })
    }

    /// `JavaClass.getMethodCallsFromSelf()`.
    pub fn method_calls_from_self(&self) -> Vec<RustAccess> {
        Self::filter_kind(self.accesses_from_self(), |k| k == AccessKind::MethodCall)
    }

    /// `JavaClass.getMethodCallsToSelf()`.
    pub fn method_calls_to_self(&self) -> Vec<RustAccess> {
        Self::filter_kind(self.accesses_to_self(), |k| k == AccessKind::MethodCall)
    }

    /// `JavaClass.getConstructorCallsFromSelf()`.
    pub fn constructor_calls_from_self(&self) -> Vec<RustAccess> {
        Self::filter_kind(self.accesses_from_self(), |k| {
            k == AccessKind::ConstructorCall
        })
    }

    /// `JavaClass.getConstructorCallsToSelf()`.
    pub fn constructor_calls_to_self(&self) -> Vec<RustAccess> {
        Self::filter_kind(self.accesses_to_self(), |k| {
            k == AccessKind::ConstructorCall
        })
    }

    /// Method and constructor calls (`JavaClass.getCodeUnitCallsFromSelf()`).
    pub fn code_unit_calls_from_self(&self) -> Vec<RustAccess> {
        Self::filter_kind(self.accesses_from_self(), AccessKind::is_call)
    }

    /// `JavaClass.getCodeUnitCallsToSelf()`.
    pub fn code_unit_calls_to_self(&self) -> Vec<RustAccess> {
        Self::filter_kind(self.accesses_to_self(), AccessKind::is_call)
    }

    /// Functions used as values (`JavaClass.getCodeUnitReferencesFromSelf()`).
    pub fn function_references_from_self(&self) -> Vec<RustAccess> {
        Self::filter_kind(self.accesses_from_self(), |k| {
            k == AccessKind::FunctionReference
        })
    }

    /// `JavaClass.getCodeUnitReferencesToSelf()`.
    pub fn function_references_to_self(&self) -> Vec<RustAccess> {
        Self::filter_kind(self.accesses_to_self(), |k| {
            k == AccessKind::FunctionReference
        })
    }

    /// Calls plus references (`JavaClass.getCodeUnitAccessesFromSelf()`).
    pub fn code_unit_accesses_from_self(&self) -> Vec<RustAccess> {
        Self::filter_kind(self.accesses_from_self(), |k| {
            !matches!(k, AccessKind::FieldAccess(_))
        })
    }

    /// `JavaClass.getCodeUnitAccessesToSelf()`.
    pub fn code_unit_accesses_to_self(&self) -> Vec<RustAccess> {
        Self::filter_kind(self.accesses_to_self(), |k| {
            !matches!(k, AccessKind::FieldAccess(_))
        })
    }

    // ---- dependencies -------------------------------------------------------------------

    fn dependency(&self, id: usize) -> Dependency {
        Dependency::new(Arc::clone(&self.graph), id)
    }

    /// All dependencies from this item: accesses, member types, implemented traits,
    /// annotations, generic bounds, imports (`JavaClass.getDirectDependenciesFromSelf()`).
    pub fn direct_dependencies_from_self(&self) -> Vec<Dependency> {
        self.graph
            .deps_from
            .get(self.id)
            .map(|ids| ids.iter().map(|&id| self.dependency(id)).collect())
            .unwrap_or_default()
    }

    /// All dependencies targeting this item (`JavaClass.getDirectDependenciesToSelf()`).
    pub fn direct_dependencies_to_self(&self) -> Vec<Dependency> {
        self.graph
            .deps_to
            .get(self.id)
            .map(|ids| ids.iter().map(|&id| self.dependency(id)).collect())
            .unwrap_or_default()
    }

    /// Dependencies reachable transitively (`JavaClass.getTransitiveDependenciesFromSelf()`).
    pub fn transitive_dependencies_from_self(&self) -> Vec<Dependency> {
        let mut seen = HashSet::new();
        seen.insert(self.id);
        let mut result = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(self.id);
        while let Some(id) = queue.pop_front() {
            for &dep in &self.graph.deps_from[id] {
                let target = self.graph.dependencies[dep].target;
                result.push(self.dependency(dep));
                if seen.insert(target) {
                    queue.push_back(target);
                }
            }
        }
        result
    }

    /// Fields anywhere in the import whose raw type is this item
    /// (`JavaClass.getFieldsWithTypeOfSelf()`).
    pub fn fields_with_type_of_self(&self) -> Vec<RustField> {
        self.all_members_in_graph()
            .filter(|m| matches!(m.kind(), MemberKind::Field | MemberKind::AssocConst))
            .filter(|m| m.raw_type_id() == Some(self.id))
            .map(RustField)
            .collect()
    }

    /// Methods anywhere in the import with a parameter of this type
    /// (`JavaClass.getMethodsWithParameterTypeOfSelf()`).
    pub fn methods_with_parameter_type_of_self(&self) -> Vec<RustMethod> {
        self.all_members_in_graph()
            .filter(|m| m.kind() == MemberKind::Method && m.has_parameter_of_type(self.id))
            .map(RustMethod)
            .collect()
    }

    /// Methods anywhere in the import returning this type
    /// (`JavaClass.getMethodsWithReturnTypeOfSelf()`).
    pub fn methods_with_return_type_of_self(&self) -> Vec<RustMethod> {
        self.all_members_in_graph()
            .filter(|m| m.kind() == MemberKind::Method && m.raw_return_type_id() == Some(self.id))
            .map(RustMethod)
            .collect()
    }

    /// Constructors anywhere in the import with a parameter of this type
    /// (`JavaClass.getConstructorsWithParameterTypeOfSelf()`).
    pub fn constructors_with_parameter_type_of_self(&self) -> Vec<RustConstructor> {
        self.all_members_in_graph()
            .filter(|m| m.is_constructor() && m.has_parameter_of_type(self.id))
            .map(RustConstructor)
            .collect()
    }

    /// Code units anywhere in the import returning `Result<_, Self>`
    /// (`JavaClass.getMethodThrowsDeclarationsWithTypeOfSelf()`).
    pub fn functions_with_error_type_of_self(&self) -> Vec<RustCodeUnit> {
        self.all_members_in_graph()
            .filter(|m| m.is_code_unit() && m.error_type_id() == Some(self.id))
            .map(RustCodeUnit)
            .collect()
    }

    fn all_members_in_graph(&self) -> impl Iterator<Item = RustMember> + '_ {
        (0..self.graph.members.len()).map(|id| self.member(id))
    }

    /// A [`RustItems`] view containing only this item, sharing the graph.
    pub fn as_items(&self) -> RustItems {
        RustItems::from_ids(Arc::clone(&self.graph), vec![self.id], self.name())
    }
}

/// `JavaClass.Predicates`.
pub mod predicates {
    use super::*;
    use crate::base::join_single_quoted;

    fn kind_predicate(
        description: &str,
        pred: fn(&RustItem) -> bool,
    ) -> DescribedPredicate<RustItem> {
        DescribedPredicate::describe(description, pred)
    }

    /// `INTERFACES`: traits.
    pub fn interfaces() -> DescribedPredicate<RustItem> {
        kind_predicate("interfaces", RustItem::is_trait)
    }

    /// Alias of [`interfaces`].
    pub fn traits() -> DescribedPredicate<RustItem> {
        kind_predicate("traits", RustItem::is_trait)
    }

    /// `ENUMS`.
    pub fn enums() -> DescribedPredicate<RustItem> {
        kind_predicate("enums", RustItem::is_enum)
    }

    /// Structs.
    pub fn structs() -> DescribedPredicate<RustItem> {
        kind_predicate("structs", RustItem::is_struct)
    }

    /// Unions.
    pub fn unions() -> DescribedPredicate<RustItem> {
        kind_predicate("unions", RustItem::is_union)
    }

    /// Free functions.
    pub fn functions() -> DescribedPredicate<RustItem> {
        kind_predicate("functions", RustItem::is_function)
    }

    /// Modules.
    pub fn modules() -> DescribedPredicate<RustItem> {
        kind_predicate("modules", RustItem::is_module)
    }

    /// Type aliases.
    pub fn type_aliases() -> DescribedPredicate<RustItem> {
        kind_predicate("type aliases", RustItem::is_type_alias)
    }

    /// `const` items.
    pub fn consts() -> DescribedPredicate<RustItem> {
        kind_predicate("consts", RustItem::is_const)
    }

    /// `static` items.
    pub fn statics() -> DescribedPredicate<RustItem> {
        kind_predicate("statics", RustItem::is_static)
    }

    /// Macro definitions.
    pub fn macros() -> DescribedPredicate<RustItem> {
        kind_predicate("macros", RustItem::is_macro)
    }

    /// `ANNOTATIONS`: attribute and derive macro definitions.
    pub fn annotations() -> DescribedPredicate<RustItem> {
        kind_predicate("annotations", RustItem::is_annotation)
    }

    /// `TOP_LEVEL_CLASSES`.
    pub fn top_level_classes() -> DescribedPredicate<RustItem> {
        kind_predicate("top level classes", RustItem::is_top_level_item)
    }

    /// `NESTED_CLASSES`.
    pub fn nested_classes() -> DescribedPredicate<RustItem> {
        kind_predicate("nested classes", RustItem::is_nested_item)
    }

    /// `LOCAL_CLASSES`.
    pub fn local_classes() -> DescribedPredicate<RustItem> {
        kind_predicate("local classes", RustItem::is_local_item)
    }

    /// Test code.
    pub fn test_code() -> DescribedPredicate<RustItem> {
        kind_predicate("test code", RustItem::is_test_code)
    }

    /// `type(Class)`: described as `type <name>`.
    pub fn type_(name: &str) -> DescribedPredicate<RustItem> {
        let selector = ItemSelector::from(name);
        DescribedPredicate::describe(format!("type {name}"), move |item: &RustItem| {
            selector.matches(item)
        })
    }

    /// `simpleName(..)`: described as `simple name '<name>'`.
    pub fn simple_name(name: &str) -> DescribedPredicate<RustItem> {
        let expected = name.to_owned();
        DescribedPredicate::describe(format!("simple name '{name}'"), move |item: &RustItem| {
            item.simple_name() == expected
        })
    }

    /// Described as `simple name starting with '<prefix>'`.
    pub fn simple_name_starting_with(prefix: &str) -> DescribedPredicate<RustItem> {
        let expected = prefix.to_owned();
        DescribedPredicate::describe(
            format!("simple name starting with '{prefix}'"),
            move |item: &RustItem| item.simple_name().starts_with(&expected),
        )
    }

    /// Described as `simple name containing '<infix>'`.
    pub fn simple_name_containing(infix: &str) -> DescribedPredicate<RustItem> {
        let expected = infix.to_owned();
        DescribedPredicate::describe(
            format!("simple name containing '{infix}'"),
            move |item: &RustItem| item.simple_name().contains(&expected),
        )
    }

    /// Described as `simple name ending with '<suffix>'`.
    pub fn simple_name_ending_with(suffix: &str) -> DescribedPredicate<RustItem> {
        let expected = suffix.to_owned();
        DescribedPredicate::describe(
            format!("simple name ending with '{suffix}'"),
            move |item: &RustItem| item.simple_name().ends_with(&expected),
        )
    }

    /// Described as `assignable to <name or predicate>`.
    pub fn assignable_to(selector: impl Into<ItemSelector>) -> DescribedPredicate<RustItem> {
        let selector = selector.into();
        let description = format!("assignable to {}", selector.description());
        DescribedPredicate::describe(description, move |item: &RustItem| {
            item.is_assignable_to(selector.clone())
        })
    }

    /// Described as `assignable from <name or predicate>`.
    pub fn assignable_from(selector: impl Into<ItemSelector>) -> DescribedPredicate<RustItem> {
        let selector = selector.into();
        let description = format!("assignable from {}", selector.description());
        DescribedPredicate::describe(description, move |item: &RustItem| {
            item.is_assignable_from(selector.clone())
        })
    }

    /// Described as `implement <name or predicate>`.
    pub fn implement(selector: impl Into<ItemSelector>) -> DescribedPredicate<RustItem> {
        let selector = selector.into();
        let description = format!("implement {}", selector.description());
        DescribedPredicate::describe(description, move |item: &RustItem| {
            item.implements(selector.clone())
        })
    }

    /// Described as `reside in a package '<identifier>'`.
    pub fn reside_in_a_package(package_identifier: &str) -> DescribedPredicate<RustItem> {
        let matcher = super::super::PackageMatcher::of(package_identifier);
        DescribedPredicate::describe(
            format!("reside in a package '{package_identifier}'"),
            move |item: &RustItem| matcher.matches(&item.package_name()),
        )
    }

    /// Described as `reside in any package ['a', 'b']`.
    pub fn reside_in_any_package(package_identifiers: &[&str]) -> DescribedPredicate<RustItem> {
        let matchers: Vec<_> = package_identifiers
            .iter()
            .map(|id| super::super::PackageMatcher::of(id))
            .collect();
        DescribedPredicate::describe(
            format!(
                "reside in any package [{}]",
                join_single_quoted(package_identifiers)
            ),
            move |item: &RustItem| {
                let package = item.package_name();
                matchers.iter().any(|m| m.matches(&package))
            },
        )
    }

    /// Described as `reside outside of package '<identifier>'`.
    pub fn reside_outside_of_package(package_identifier: &str) -> DescribedPredicate<RustItem> {
        let matcher = super::super::PackageMatcher::of(package_identifier);
        DescribedPredicate::describe(
            format!("reside outside of package '{package_identifier}'"),
            move |item: &RustItem| !matcher.matches(&item.package_name()),
        )
    }

    /// Described as `reside outside of packages ['a', 'b']`.
    pub fn reside_outside_of_packages(
        package_identifiers: &[&str],
    ) -> DescribedPredicate<RustItem> {
        let matchers: Vec<_> = package_identifiers
            .iter()
            .map(|id| super::super::PackageMatcher::of(id))
            .collect();
        DescribedPredicate::describe(
            format!(
                "reside outside of packages [{}]",
                join_single_quoted(package_identifiers)
            ),
            move |item: &RustItem| {
                let package = item.package_name();
                !matchers.iter().any(|m| m.matches(&package))
            },
        )
    }

    /// Described as `reside in crate '<name>'`.
    pub fn reside_in_crate(crate_name: &str) -> DescribedPredicate<RustItem> {
        let expected = crate_name.replace('-', "_");
        DescribedPredicate::describe(
            format!("reside in crate '{crate_name}'"),
            move |item: &RustItem| item.crate_name() == expected,
        )
    }

    /// Described as `equivalent to <name>`.
    pub fn equivalent_to(name: &str) -> DescribedPredicate<RustItem> {
        let expected = name.to_owned();
        DescribedPredicate::describe(format!("equivalent to {name}"), move |item: &RustItem| {
            item.is_equivalent_to(&expected)
        })
    }

    /// Matches the named items and everything declared inside them;
    /// described as `belong to any of [a, b]`.
    pub fn belong_to_any_of(names: &[&str]) -> DescribedPredicate<RustItem> {
        let selectors: Vec<ItemSelector> = names.iter().map(|n| ItemSelector::from(*n)).collect();
        DescribedPredicate::describe(
            format!("belong to any of [{}]", names.join(", ")),
            move |item: &RustItem| {
                belongs_to(item, |candidate| {
                    selectors.iter().any(|s| s.matches(candidate))
                })
            },
        )
    }

    /// Matches items that are, or are declared inside, an item satisfying `predicate`;
    /// described as `belong to <predicate>`.
    pub fn belong_to(predicate: DescribedPredicate<RustItem>) -> DescribedPredicate<RustItem> {
        let description = format!("belong to {}", predicate.description());
        DescribedPredicate::describe(description, move |item: &RustItem| {
            belongs_to(item, |candidate| predicate.test(candidate))
        })
    }

    fn belongs_to(item: &RustItem, matches: impl Fn(&RustItem) -> bool) -> bool {
        let mut current = Some(item.clone());
        while let Some(candidate) = current {
            if matches(&candidate) {
                return true;
            }
            current = candidate.enclosing_item();
        }
        false
    }

    /// Described as `contain any members that <predicate>`.
    pub fn contain_any_members_that(
        predicate: DescribedPredicate<RustMember>,
    ) -> DescribedPredicate<RustItem> {
        let description = format!("contain any members that {}", predicate.description());
        DescribedPredicate::describe(description, move |item: &RustItem| {
            item.members().iter().any(|m| predicate.test(m))
        })
    }

    /// Described as `contain any fields that <predicate>`.
    pub fn contain_any_fields_that(
        predicate: DescribedPredicate<RustField>,
    ) -> DescribedPredicate<RustItem> {
        let description = format!("contain any fields that {}", predicate.description());
        DescribedPredicate::describe(description, move |item: &RustItem| {
            item.fields().iter().any(|m| predicate.test(m))
        })
    }

    /// Described as `contain any code units that <predicate>`.
    pub fn contain_any_code_units_that(
        predicate: DescribedPredicate<RustCodeUnit>,
    ) -> DescribedPredicate<RustItem> {
        let description = format!("contain any code units that {}", predicate.description());
        DescribedPredicate::describe(description, move |item: &RustItem| {
            item.code_units().iter().any(|m| predicate.test(m))
        })
    }

    /// Described as `contain any methods that <predicate>`.
    pub fn contain_any_methods_that(
        predicate: DescribedPredicate<RustMethod>,
    ) -> DescribedPredicate<RustItem> {
        let description = format!("contain any methods that {}", predicate.description());
        DescribedPredicate::describe(description, move |item: &RustItem| {
            item.methods().iter().any(|m| predicate.test(m))
        })
    }

    /// Described as `contain any constructors that <predicate>`.
    pub fn contain_any_constructors_that(
        predicate: DescribedPredicate<RustConstructor>,
    ) -> DescribedPredicate<RustItem> {
        let description = format!("contain any constructors that {}", predicate.description());
        DescribedPredicate::describe(description, move |item: &RustItem| {
            item.constructors().iter().any(|m| predicate.test(m))
        })
    }
}

/// `JavaClass.Functions`.
pub mod functions {
    use super::*;
    use crate::base::DescribedFunction;

    /// `GET_SIMPLE_NAME`.
    pub fn get_simple_name() -> DescribedFunction<RustItem, String> {
        DescribedFunction::describe("simple name", RustItem::simple_name)
    }

    /// `GET_PACKAGE_NAME`.
    pub fn get_package_name() -> DescribedFunction<RustItem, String> {
        DescribedFunction::describe("package name", RustItem::package_name)
    }

    /// `GET_PACKAGE`.
    pub fn get_package() -> DescribedFunction<RustItem, RustModule> {
        DescribedFunction::describe("package", RustItem::package)
    }

    /// `GET_MEMBERS`.
    pub fn get_members() -> DescribedFunction<RustItem, Vec<RustMember>> {
        DescribedFunction::describe("members", RustItem::members)
    }

    /// `GET_FIELDS`.
    pub fn get_fields() -> DescribedFunction<RustItem, Vec<RustField>> {
        DescribedFunction::describe("fields", RustItem::fields)
    }

    /// `GET_CODE_UNITS`.
    pub fn get_code_units() -> DescribedFunction<RustItem, Vec<RustCodeUnit>> {
        DescribedFunction::describe("code units", RustItem::code_units)
    }

    /// `GET_METHODS`.
    pub fn get_methods() -> DescribedFunction<RustItem, Vec<RustMethod>> {
        DescribedFunction::describe("methods", RustItem::methods)
    }

    /// `GET_CONSTRUCTORS`.
    pub fn get_constructors() -> DescribedFunction<RustItem, Vec<RustConstructor>> {
        DescribedFunction::describe("constructors", RustItem::constructors)
    }

    /// `GET_ACCESSES_FROM_SELF`.
    pub fn get_accesses_from_self() -> DescribedFunction<RustItem, Vec<RustAccess>> {
        DescribedFunction::describe("accesses from self", RustItem::accesses_from_self)
    }

    /// `GET_ACCESSES_TO_SELF`.
    pub fn get_accesses_to_self() -> DescribedFunction<RustItem, Vec<RustAccess>> {
        DescribedFunction::describe("accesses to self", RustItem::accesses_to_self)
    }

    /// `GET_FIELD_ACCESSES_FROM_SELF`.
    pub fn get_field_accesses_from_self() -> DescribedFunction<RustItem, Vec<RustAccess>> {
        DescribedFunction::describe(
            "field accesses from self",
            RustItem::field_accesses_from_self,
        )
    }

    /// `GET_METHOD_CALLS_FROM_SELF`.
    pub fn get_method_calls_from_self() -> DescribedFunction<RustItem, Vec<RustAccess>> {
        DescribedFunction::describe("method calls from self", RustItem::method_calls_from_self)
    }

    /// `GET_CONSTRUCTOR_CALLS_FROM_SELF`.
    pub fn get_constructor_calls_from_self() -> DescribedFunction<RustItem, Vec<RustAccess>> {
        DescribedFunction::describe(
            "constructor calls from self",
            RustItem::constructor_calls_from_self,
        )
    }

    /// `GET_CODE_UNIT_CALLS_FROM_SELF`.
    pub fn get_code_unit_calls_from_self() -> DescribedFunction<RustItem, Vec<RustAccess>> {
        DescribedFunction::describe(
            "code unit calls from self",
            RustItem::code_unit_calls_from_self,
        )
    }

    /// `GET_DIRECT_DEPENDENCIES_FROM_SELF`.
    pub fn get_direct_dependencies_from_self() -> DescribedFunction<RustItem, Vec<Dependency>> {
        DescribedFunction::describe(
            "direct dependencies from self",
            RustItem::direct_dependencies_from_self,
        )
    }

    /// `GET_DIRECT_DEPENDENCIES_TO_SELF`.
    pub fn get_direct_dependencies_to_self() -> DescribedFunction<RustItem, Vec<Dependency>> {
        DescribedFunction::describe(
            "direct dependencies to self",
            RustItem::direct_dependencies_to_self,
        )
    }

    /// `GET_TRANSITIVE_DEPENDENCIES_FROM_SELF`.
    pub fn get_transitive_dependencies_from_self() -> DescribedFunction<RustItem, Vec<Dependency>> {
        DescribedFunction::describe(
            "transitive dependencies from self",
            RustItem::transitive_dependencies_from_self,
        )
    }
}

use crate::base::DescribedPredicate;
