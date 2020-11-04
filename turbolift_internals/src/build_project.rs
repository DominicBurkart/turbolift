use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use crate::utils::{symlink_dir, IS_RELEASE, RELEASE_FLAG};

pub fn edit_cargo_file(
    original_project_source_dir: &Path,
    cargo_path: &Path,
    function_name: &str,
) -> anyhow::Result<()> {
    let local_deps_dir_name = ".local_deps";
    let mut parsed_toml: cargo_toml2::CargoToml = cargo_toml2::from_path(cargo_path)
        .unwrap_or_else(|_| panic!("toml at {:?} could not be read", cargo_path));
    let relative_local_deps_cache = cargo_path.parent().unwrap().join(local_deps_dir_name);
    fs::create_dir_all(&relative_local_deps_cache)?;
    let local_deps_cache = relative_local_deps_cache.canonicalize()?;

    // change name
    parsed_toml.package.name = function_name.to_string() + "_turbolift";

    // symlink any local directories so they work with the new project location
    let mut deps = match parsed_toml.dependencies {
        Some(deps) => deps,
        None => Default::default(),
    };
    let details = deps
        .iter_mut()
        // only full dependency descriptions (not simple version number)
        .filter_map(|(name, dep)| match dep {
            cargo_toml2::Dependency::Simple(_) => None,
            cargo_toml2::Dependency::Full(detail) => Some((name, detail)),
        });
    let mut completed_locations = HashSet::new();
    for (name, detail) in details {
        // only descriptions with a path
        if let Some(ref mut buf) = detail.path {
            // determine what the symlink for this dependency should be
            let canonical = original_project_source_dir.join(&buf).canonicalize()?;
            let dep_location = local_deps_cache.join(name);

            // check that we don't have a naming error
            // todo: automatically handle naming conflicts by mangling the dep for one
            if completed_locations.contains(&dep_location) {
                return Err(anyhow::anyhow!("two dependencies cannot share a directory name. Can the directory for one be renamed? Issue: https://github.com/DominicBurkart/turbolift/issues/1"));
            } else {
                completed_locations.insert(dep_location.clone());
            }

            if dep_location.exists() {
                // dependency already exists, does it point to the correct place?
                if canonical == dep_location.canonicalize()? {
                    // output already points to the right place, do nothing
                } else {
                    // output points somewhere else; delete it; if it's non-empty, error
                    fs::remove_dir(&dep_location).unwrap();
                    symlink_dir(&canonical, &dep_location)?;
                }
            } else {
                symlink_dir(&canonical, &dep_location)?;
            }

            *buf = PathBuf::from_str(".")?.join(local_deps_dir_name).join(name);
        }
    }

    // mutate all simple definitions to full ones to avoid toml serialization bug
    // ^ https://github.com/DianaNites/cargo-toml2/blob/89fc8e6055d5ee3e8a2ae614f656d79f38e59f9f/README.md#limitations
    deps.iter_mut().for_each(|(_name, dep)| {
        if let cargo_toml2::Dependency::Simple(simple_version) = dep {
            let full = cargo_toml2::DependencyFull {
                version: Some(simple_version.clone()),
                ..Default::default()
            };
            *dep = cargo_toml2::Dependency::Full(full);
        };
    });
    parsed_toml.dependencies = Some(deps);
    cargo_toml2::to_path(cargo_path, parsed_toml)?;
    Ok(())
}

pub fn lint(proj_path: &Path) -> anyhow::Result<()> {
    let install_status = Command::new("rustup")
        .args("component add rustfmt".split(' '))
        .status()?;

    if !install_status.success() {
        return Err(anyhow::anyhow!("clippy install failed"));
    }

    let status = Command::new("cargo")
        .current_dir(proj_path)
        .args("fmt".split(' '))
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("rustfmt fix failed"));
    }
    Ok(())
}

pub fn make_executable(proj_path: &Path, dest: Option<&Path>) -> anyhow::Result<()> {
    let status = Command::new("cargo")
        .current_dir(proj_path)
        .args(format!("build {}", RELEASE_FLAG).as_str().trim().split(' '))
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "cargo build failed with code: {:?}",
            status.code()
        ));
    }
    if let Some(destination) = dest {
        let executable_path = {
            let cargo_path = proj_path.join("Cargo.toml");
            let parsed_toml: cargo_toml2::CargoToml = cargo_toml2::from_path(cargo_path)?;
            let project_name = parsed_toml.package.name;
            let local_path = if IS_RELEASE {
                "target/release/".to_string() + &project_name
            } else {
                "target/debug/".to_string() + &project_name
            };
            proj_path.canonicalize().unwrap().join(&local_path)
        };
        fs::rename(&executable_path, destination)?;
    }
    Ok(())
}

pub fn check(proj_path: &Path) -> anyhow::Result<()> {
    let status = Command::new("cargo")
        .current_dir(proj_path)
        .args("check".split(' '))
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "cargo check failed with code: {:?}",
            status.code()
        ));
    }
    Ok(())
}
