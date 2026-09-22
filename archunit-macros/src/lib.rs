//! Attribute macros for [archunit](https://docs.rs/archunit): the port of ArchUnit's JUnit
//! support (`@AnalyzeClasses`, `@ArchTest`, `@ArchIgnore`, `@ArchTag`, `ArchTests.in(..)`).
//!
//! Use them through `archunit::harness` (or `archunit::prelude`); this crate is an
//! implementation detail.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, ExprLit, FnArg, Ident, Item, ItemFn, ItemMod, Lit, Meta, Path, Token};

// ---- #[analyze_classes(..)] ------------------------------------------------------------------

/// `@AnalyzeClasses`: configures the import for every `#[arch_test]` in the annotated inline
/// module.
///
/// ```ignore
/// #[analyze_classes(packages = ["my_app.."], import_options = [DoNotIncludeTests], cache_mode = PerClass)]
/// mod architecture {
///     use archunit::prelude::*;
///
///     #[arch_test]
///     fn services_should_not_access_controllers() -> impl ArchRule {
///         no_classes().that().reside_in_a_package("..service..")
///             .should().access_classes_that().reside_in_a_package("..controller..")
///     }
///
///     #[arch_test]
///     fn checked_directly(items: &RustItems) {
///         classes().should().be_public().check(items);
///     }
/// }
/// ```
///
/// Arguments (all optional): `packages = [..]`, `packages_of = [..]`, `items = [..]`,
/// `locations = [LocationProviderType, ..]`, `whole_workspace = true`,
/// `import_options = [DoNotIncludeTests, ..]`, `cache_mode = Forever | PerClass`.
/// Without arguments the crate containing the test module is analyzed.
#[proc_macro_attribute]
pub fn analyze_classes(args: TokenStream, input: TokenStream) -> TokenStream {
    let module = syn::parse_macro_input!(input as ItemMod);
    let config = match AnalyzeClassesArgs::parse.parse(args) {
        Ok(config) => config,
        Err(e) => return e.to_compile_error().into(),
    };
    match expand_analyze_classes(config, module) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[derive(Default)]
struct AnalyzeClassesArgs {
    packages: Vec<String>,
    packages_of: Vec<String>,
    items: Vec<String>,
    locations: Vec<Expr>,
    import_options: Vec<Expr>,
    whole_workspace: Option<bool>,
    cache_mode: Option<Path>,
}

impl Parse for AnalyzeClassesArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = AnalyzeClassesArgs::default();
        let entries = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        for meta in entries {
            let Meta::NameValue(name_value) = meta else {
                return Err(syn::Error::new(
                    meta.span(),
                    "expected `name = value`, e.g. `packages = [\"my_app..\"]`",
                ));
            };
            let name = name_value
                .path
                .get_ident()
                .map(ToString::to_string)
                .unwrap_or_default();
            let value = name_value.value;
            match name.as_str() {
                "packages" => args.packages = string_array(&value)?,
                "packages_of" => args.packages_of = string_array(&value)?,
                "items" | "classes" => args.items = string_array(&value)?,
                "locations" => args.locations = expr_array(&value)?,
                "import_options" => args.import_options = expr_array(&value)?,
                "whole_workspace" | "whole_classpath" => {
                    args.whole_workspace = Some(bool_value(&value)?)
                }
                "cache_mode" => {
                    let Expr::Path(path) = value else {
                        return Err(syn::Error::new(
                            value.span(),
                            "expected `Forever` or `PerClass`",
                        ));
                    };
                    args.cache_mode = Some(path.path);
                }
                other => {
                    return Err(syn::Error::new(
                        name_value.path.span(),
                        format!(
                            "unknown argument `{other}`; expected one of packages, packages_of, items, locations, whole_workspace, import_options, cache_mode"
                        ),
                    ));
                }
            }
        }
        Ok(args)
    }
}

fn expr_array(value: &Expr) -> syn::Result<Vec<Expr>> {
    match value {
        Expr::Array(array) => Ok(array.elems.iter().cloned().collect()),
        other => Ok(vec![other.clone()]),
    }
}

fn string_array(value: &Expr) -> syn::Result<Vec<String>> {
    expr_array(value)?
        .iter()
        .map(|e| match e {
            Expr::Lit(ExprLit {
                lit: Lit::Str(s), ..
            }) => Ok(s.value()),
            other => Err(syn::Error::new(other.span(), "expected a string literal")),
        })
        .collect()
}

fn bool_value(value: &Expr) -> syn::Result<bool> {
    match value {
        Expr::Lit(ExprLit {
            lit: Lit::Bool(b), ..
        }) => Ok(b.value),
        other => Err(syn::Error::new(other.span(), "expected `true` or `false`")),
    }
}

fn expand_analyze_classes(
    config: AnalyzeClassesArgs,
    mut module: ItemMod,
) -> syn::Result<TokenStream2> {
    let Some((_, items)) = module.content.as_mut() else {
        return Err(syn::Error::new(
            module.span(),
            "#[analyze_classes] must be placed on an inline module (`mod name { .. }`)",
        ));
    };
    let items_fn = format_ident!("__archunit_items");
    for item in items.iter_mut() {
        if let Item::Fn(function) = item {
            for attr in &mut function.attrs {
                if is_named(attr, "arch_test") {
                    *attr = syn::parse_quote!(#[::archunit::harness::arch_test(items = #items_fn)]);
                }
            }
        }
    }
    let packages = &config.packages;
    let packages_of = &config.packages_of;
    let named_items = &config.items;
    let locations = config
        .locations
        .iter()
        .map(|provider| quote!(.locations(<#provider as ::core::default::Default>::default())));
    let import_options = config
        .import_options
        .iter()
        .map(|option| quote!(.import_option(#option)));
    let whole_workspace = config.whole_workspace.map(|b| quote!(.whole_workspace(#b)));
    let cache_mode = config
        .cache_mode
        .map(|path| quote!(.cache_mode(::archunit::harness::CacheMode::#path)));
    let provider: Item = syn::parse_quote! {
        #[doc(hidden)]
        #[allow(dead_code)]
        pub fn #items_fn() -> ::std::sync::Arc<::archunit::core::domain::RustItems> {
            static ITEMS: ::std::sync::OnceLock<::std::sync::Arc<::archunit::core::domain::RustItems>> =
                ::std::sync::OnceLock::new();
            ::std::sync::Arc::clone(ITEMS.get_or_init(|| {
                ::archunit::harness::analyze_classes()
                    .for_test_module(::core::module_path!())
                    .packages(&[#(#packages),*])
                    .packages_of(&[#(#packages_of),*])
                    .items(&[#(#named_items),*])
                    #(#locations)*
                    #(#import_options)*
                    #whole_workspace
                    #cache_mode
                    .import()
            }))
        }
    };
    items.push(provider);
    Ok(module.into_token_stream())
}

// ---- #[arch_test] --------------------------------------------------------------------------

/// `@ArchTest`: turns a function returning a rule (`fn name() -> impl ArchRule`), a function
/// checking items itself (`fn name(items: &RustItems)`), or a function returning
/// `ArchTests` (`fn name() -> ArchTests`) into a `#[test]`.
///
/// Inside an [`analyze_classes`] module the items come from that module's configuration;
/// elsewhere the crate under test is imported with the defaults. `#[arch_test(items = provider)]`
/// names a function returning `RustItems` or `Arc<RustItems>` explicitly `[rust-only]`.
#[proc_macro_attribute]
pub fn arch_test(args: TokenStream, input: TokenStream) -> TokenStream {
    let provider = match ItemsProvider::parse.parse(args) {
        Ok(p) => p,
        Err(e) => return e.to_compile_error().into(),
    };
    let item = syn::parse_macro_input!(input as Item);
    match expand_arch_test(provider, item) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

struct ItemsProvider(Option<Path>);

impl Parse for ItemsProvider {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(Self(None));
        }
        let name: Ident = input.parse()?;
        if name != "items" {
            return Err(syn::Error::new(
                name.span(),
                "expected `items = <provider fn>`",
            ));
        }
        input.parse::<Token![=]>()?;
        let path: Path = input.parse()?;
        Ok(Self(Some(path)))
    }
}

enum Shape {
    /// `fn name() -> impl ArchRule` or `fn name() -> ArchTests`.
    Rule,
    /// `fn name(items: &RustItems)`.
    Check,
}

fn shape_of(function: &ItemFn) -> syn::Result<Shape> {
    let inputs = &function.sig.inputs;
    match inputs.len() {
        0 => Ok(Shape::Rule),
        1 => match &inputs[0] {
            FnArg::Typed(_) => Ok(Shape::Check),
            FnArg::Receiver(r) => Err(syn::Error::new(
                r.span(),
                "#[arch_test] functions cannot take `self`",
            )),
        },
        _ => Err(syn::Error::new(
            inputs.span(),
            "#[arch_test] functions take either no argument (and return a rule) or exactly one `&RustItems`",
        )),
    }
}

struct TestMeta {
    ignore: Option<Option<String>>,
    tags: Vec<String>,
    kept: Vec<Attribute>,
    forwarded: Vec<Attribute>,
}

/// Splits `#[arch_ignore]`, `#[arch_tag]`, `#[ignore]`, `#[should_panic]` and `#[cfg]` from
/// the attributes that stay on the rule function.
fn split_attributes(attrs: Vec<Attribute>) -> syn::Result<TestMeta> {
    let mut meta = TestMeta {
        ignore: None,
        tags: Vec::new(),
        kept: Vec::new(),
        forwarded: Vec::new(),
    };
    for attr in attrs {
        if is_named(&attr, "arch_ignore") {
            meta.ignore = Some(ignore_reason(&attr)?);
        } else if is_named(&attr, "arch_tag") {
            meta.tags.extend(tag_names(&attr)?);
        } else if is_named(&attr, "ignore") {
            let reason = match &attr.meta {
                Meta::NameValue(nv) => string_literal(&nv.value),
                _ => None,
            };
            meta.ignore = Some(reason);
        } else if is_named(&attr, "should_panic") {
            meta.forwarded.push(attr);
        } else if is_named(&attr, "cfg") {
            meta.forwarded.push(attr.clone());
            meta.kept.push(attr);
        } else {
            meta.kept.push(attr);
        }
    }
    Ok(meta)
}

fn string_literal(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) => Some(s.value()),
        _ => None,
    }
}

fn ignore_reason(attr: &Attribute) -> syn::Result<Option<String>> {
    match &attr.meta {
        Meta::Path(_) => Ok(None),
        Meta::NameValue(nv) => Ok(string_literal(&nv.value)),
        Meta::List(list) => {
            let parsed = list.parse_args_with(|input: ParseStream| {
                if input.peek(Lit) {
                    let lit: Lit = input.parse()?;
                    return Ok(match lit {
                        Lit::Str(s) => Some(s.value()),
                        _ => None,
                    });
                }
                let name: Ident = input.parse()?;
                if name != "reason" {
                    return Err(syn::Error::new(name.span(), "expected `reason = \"..\"`"));
                }
                input.parse::<Token![=]>()?;
                let lit: syn::LitStr = input.parse()?;
                Ok(Some(lit.value()))
            })?;
            Ok(parsed)
        }
    }
}

fn tag_names(attr: &Attribute) -> syn::Result<Vec<String>> {
    let Meta::List(list) = &attr.meta else {
        return Err(syn::Error::new(
            attr.span(),
            "expected `#[arch_tag(\"name\")]`",
        ));
    };
    let tags = list.parse_args_with(Punctuated::<syn::LitStr, Token![,]>::parse_terminated)?;
    tags.iter()
        .map(|tag| {
            let value = tag.value();
            if value.is_empty() || !value.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(syn::Error::new(
                    tag.span(),
                    "tags must be non-empty and consist of letters, digits and `_`",
                ));
            }
            Ok(value)
        })
        .collect()
}

fn is_named(attr: &Attribute, name: &str) -> bool {
    attr.path().segments.last().is_some_and(|s| s.ident == name)
}

fn test_name(base: &Ident, tags: &[String]) -> Ident {
    let mut name = base.to_string();
    for tag in tags {
        name.push_str("__tag_");
        name.push_str(tag);
    }
    Ident::new(&name, base.span())
}

fn expand_arch_test(provider: ItemsProvider, item: Item) -> syn::Result<TokenStream2> {
    let Item::Fn(mut function) = item else {
        return Err(syn::Error::new(
            item.span(),
            "#[arch_test] is only supported on functions: write `fn rule() -> impl ArchRule { .. }` or `fn rule(items: &RustItems) { .. }` (Rust statics cannot hold rules)",
        ));
    };
    let shape = shape_of(&function)?;
    let meta = split_attributes(std::mem::take(&mut function.attrs))?;
    function.attrs = meta.kept;
    let original = function.sig.ident.clone();
    let rule_fn = format_ident!("__archunit_rule_{}", original);
    function.sig.ident = rule_fn.clone();
    let name = test_name(&original, &meta.tags);
    let name_str = original.to_string();
    let ignore = meta.ignore.map(|reason| match reason {
        Some(reason) => quote!(#[ignore = #reason]),
        None => quote!(#[ignore]),
    });
    let forwarded = meta.forwarded;
    let items = match provider.0 {
        Some(path) => quote!(::archunit::harness::IntoItems::into_items(#path())),
        None => quote!(
            ::archunit::harness::analyze_classes()
                .for_test_module(::core::module_path!())
                .import()
        ),
    };
    let body = match shape {
        Shape::Rule => quote! {
            ::archunit::harness::ArchTestRunnable::run_arch_test(&#rule_fn(), #name_str, &__archunit_items)
        },
        Shape::Check => quote!(#rule_fn(&__archunit_items)),
    };
    Ok(quote! {
        #[allow(dead_code)]
        #function

        #[test]
        #[allow(non_snake_case)]
        #ignore
        #(#forwarded)*
        fn #name() {
            let __archunit_items = #items;
            #body
        }
    })
}

// ---- #[arch_rules] -------------------------------------------------------------------------

/// A module of `#[arch_test]` rules without an import configuration, included in test
/// modules through `ArchTests::in_(rules_module::arch_tests)` (`ArchTests.in(OtherRules.class)`).
///
/// The macro keeps every function and generates `pub fn arch_tests() -> ArchTests`.
#[proc_macro_attribute]
pub fn arch_rules(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(Span::call_site(), "#[arch_rules] takes no arguments")
            .to_compile_error()
            .into();
    }
    let module = syn::parse_macro_input!(input as ItemMod);
    match expand_arch_rules(module) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand_arch_rules(mut module: ItemMod) -> syn::Result<TokenStream2> {
    let Some((_, items)) = module.content.as_mut() else {
        return Err(syn::Error::new(
            module.span(),
            "#[arch_rules] must be placed on an inline module (`mod name { .. }`)",
        ));
    };
    let mut cases = Vec::new();
    for item in items.iter_mut() {
        let Item::Fn(function) = item else { continue };
        if !function.attrs.iter().any(|a| is_named(a, "arch_test")) {
            continue;
        }
        let attrs: Vec<Attribute> = function
            .attrs
            .drain(..)
            .filter(|a| !is_named(a, "arch_test"))
            .collect();
        let meta = split_attributes(attrs)?;
        function.attrs = meta.kept;
        let shape = shape_of(function)?;
        let name = &function.sig.ident;
        let name_str = name.to_string();
        let run = match shape {
            Shape::Rule => quote! {
                |items: &::archunit::core::domain::RustItems| {
                    ::archunit::harness::ArchTestRunnable::run_arch_test(&#name(), #name_str, items)
                }
            },
            Shape::Check => quote!(|items: &::archunit::core::domain::RustItems| #name(items)),
        };
        let ignored = meta.ignore.map(|reason| {
            let reason = reason.unwrap_or_default();
            quote!(.ignored(#reason))
        });
        let tags = &meta.tags;
        cases.push(quote! {
            ::archunit::harness::ArchTestCase::new(#name_str, #run)
                #ignored
                .tagged(&[#(#tags),*])
        });
    }
    let provider: Item = syn::parse_quote! {
        /// The rules of this module, for `ArchTests::in_(..)`.
        pub fn arch_tests() -> ::archunit::harness::ArchTests {
            ::archunit::harness::ArchTests::of(::core::module_path!(), vec![#(#cases),*])
        }
    };
    items.push(provider);
    Ok(module.into_token_stream())
}

// ---- #[arch_ignore], #[arch_tag] -----------------------------------------------------------

/// `@ArchIgnore(reason = "..")`: skips the test. Also accepted after `#[arch_test]`; on its
/// own it expands to `#[ignore = "reason"]`.
#[proc_macro_attribute]
pub fn arch_ignore(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr: Attribute = if args.is_empty() {
        syn::parse_quote!(#[arch_ignore])
    } else {
        let args = TokenStream2::from(args);
        syn::parse_quote!(#[arch_ignore(#args)])
    };
    let reason = match ignore_reason(&attr) {
        Ok(r) => r,
        Err(e) => return e.to_compile_error().into(),
    };
    let ignore = match reason {
        Some(reason) => quote!(#[ignore = #reason]),
        None => quote!(#[ignore]),
    };
    let input = TokenStream2::from(input);
    quote!(#ignore #input).into()
}

/// `@ArchTag("name")`: appends `__tag_<name>` to the test name so `cargo test __tag_name`
/// selects every test carrying the tag. Also accepted after `#[arch_test]`.
#[proc_macro_attribute]
pub fn arch_tag(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = TokenStream2::from(args);
    let attr: Attribute = syn::parse_quote!(#[arch_tag(#args)]);
    let tags = match tag_names(&attr) {
        Ok(t) => t,
        Err(e) => return e.to_compile_error().into(),
    };
    let mut item = syn::parse_macro_input!(input as Item);
    match &mut item {
        Item::Fn(function) => {
            function.sig.ident = test_name(&function.sig.ident, &tags);
            quote!(#[allow(non_snake_case)] #item).into()
        }
        other => syn::Error::new(other.span(), "#[arch_tag] is only supported on functions")
            .to_compile_error()
            .into(),
    }
}
