//! Crate discovery through `cargo metadata`.

use std::path::{Path, PathBuf};

use cargo_metadata::MetadataCommand;

use crate::base::ArchUnitError;
use crate::core::domain::{PackageMatcher, TargetKind};

/// One cargo target to import: a crate root file plus what is needed to resolve names in it.
#[derive(Debug, Clone)]
pub(crate) struct CrateSource {
    /// The root module name used for items (`my_app`), always with `_` for `-`.
    pub crate_name: String,
    /// The package's library name (`my_app`), used as the key for `extern crate` resolution.
    pub lib_name: String,
    pub target: TargetKind,
    pub root_file: PathBuf,
    pub crate_dir: PathBuf,
    pub is_workspace_member: bool,
    /// `(name as used in code, lib name of the dependency package)`.
    pub extern_crates: Vec<(String, String)>,
}

fn normalize(name: &str) -> String {
    name.replace('-', "_")
}

/// Discovers all targets of the crate or workspace at `path`.
pub(crate) fn discover(
    path: &Path,
    with_dependencies: bool,
    only_dependencies: &[PackageMatcher],
) -> Result<Vec<CrateSource>, ArchUnitError> {
    let mut command = MetadataCommand::new();
    if path.is_file() {
        command.manifest_path(path);
    } else {
        command.current_dir(path);
    }
    if !with_dependencies {
        command.no_deps();
    }
    let metadata = command.exec().map_err(|e| ArchUnitError::CargoMetadata {
        path: path.to_path_buf(),
        source: Box::new(e),
    })?;

    let lib_name_of = |package: &cargo_metadata::Package| -> String {
        package
            .targets
            .iter()
            .find(|t| t.is_lib() || t.is_proc_macro())
            .map(|t| normalize(&t.name))
            .unwrap_or_else(|| normalize(&package.name))
    };

    let mut sources = Vec::new();
    for package in &metadata.packages {
        let is_member = metadata.workspace_members.contains(&package.id);
        if !is_member && !with_dependencies {
            continue;
        }
        let lib_name = lib_name_of(package);
        if !is_member
            && !only_dependencies.is_empty()
            && !only_dependencies.iter().any(|m| m.matches(&lib_name))
        {
            continue;
        }
        let crate_dir = package
            .manifest_path
            .parent()
            .map(|p| PathBuf::from(p.as_std_path()))
            .unwrap_or_default();
        let extern_crates: Vec<(String, String)> = package
            .dependencies
            .iter()
            .map(|dep| {
                let used_name = dep.rename.clone().unwrap_or_else(|| dep.name.clone());
                let dep_lib = metadata
                    .packages
                    .iter()
                    .find(|p| p.name.as_str() == dep.name.as_str())
                    .map(lib_name_of)
                    .unwrap_or_else(|| normalize(&dep.name));
                (normalize(&used_name), dep_lib)
            })
            .collect();

        for target in &package.targets {
            if target.is_custom_build() {
                continue;
            }
            let kind = if target.is_lib() {
                TargetKind::Lib
            } else if target.is_proc_macro() {
                TargetKind::ProcMacro
            } else if target.is_bin() {
                TargetKind::Bin(normalize(&target.name))
            } else if target.is_test() {
                TargetKind::Test(normalize(&target.name))
            } else if target.is_example() {
                TargetKind::Example(normalize(&target.name))
            } else if target.is_bench() {
                TargetKind::Bench(normalize(&target.name))
            } else {
                continue;
            };
            if !is_member && !matches!(kind, TargetKind::Lib | TargetKind::ProcMacro) {
                continue;
            }
            let crate_name = match &kind {
                TargetKind::Lib | TargetKind::ProcMacro => lib_name.clone(),
                TargetKind::Bin(name)
                | TargetKind::Test(name)
                | TargetKind::Example(name)
                | TargetKind::Bench(name) => name.clone(),
            };
            let mut extern_crates = extern_crates.clone();
            if !matches!(kind, TargetKind::Lib | TargetKind::ProcMacro)
                && package
                    .targets
                    .iter()
                    .any(|t| t.is_lib() || t.is_proc_macro())
            {
                extern_crates.push((lib_name.clone(), lib_name.clone()));
            }
            sources.push(CrateSource {
                crate_name,
                lib_name: lib_name.clone(),
                target: kind,
                root_file: PathBuf::from(target.src_path.as_std_path()),
                crate_dir: crate_dir.clone(),
                is_workspace_member: is_member,
                extern_crates,
            });
        }
    }
    if sources.is_empty() {
        return Err(ArchUnitError::NoCrate(format!(
            "No cargo targets found at {}",
            path.display()
        )));
    }
    // Libraries first so that their items win name collisions with binaries.
    sources.sort_by_key(|s| !matches!(s.target, TargetKind::Lib | TargetKind::ProcMacro));
    Ok(sources)
}
