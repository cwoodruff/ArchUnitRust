//! No-op attribute and derive macros that give the fixture crates "annotations".
use proc_macro::TokenStream;

macro_rules! noop_attributes {
    ($($name:ident),* $(,)?) => {
        $(
            #[proc_macro_attribute]
            pub fn $name(_attr: TokenStream, item: TokenStream) -> TokenStream {
                item
            }
        )*
    };
}

noop_attributes!(
    secured,
    my_service,
    my_controller,
    entity,
    high_security,
    domain_model,
    domain_service,
    application,
    adapter,
    transactional,
);

#[proc_macro_derive(Entity)]
pub fn derive_entity(_item: TokenStream) -> TokenStream {
    TokenStream::new()
}
