use fixture_macros::domain_model;

#[domain_model]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProductId(pub u64);

#[domain_model]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductName(pub String);

#[domain_model]
#[derive(Debug, Clone)]
pub struct Product {
    pub id: ProductId,
    pub name: ProductName,
}
