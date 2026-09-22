#![allow(dead_code)]

use std::path::PathBuf;

use archunit::core::domain::RustItems;
use archunit::core::importer::CrateImporter;

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

pub fn import_fixture(name: &str) -> RustItems {
    CrateImporter::new().import_path(fixture_path(name))
}

pub fn names<I, T>(items: I) -> Vec<String>
where
    I: IntoIterator<Item = T>,
    T: std::fmt::Display,
{
    let mut names: Vec<String> = items.into_iter().map(|i| i.to_string()).collect();
    names.sort();
    names
}
