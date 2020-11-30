use std::collections::{HashSet, VecDeque};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use crate::utils::{symlink_dir, IS_RELEASE, RELEASE_FLAG};

#[tracing::instrument]
pub fn edit_cargo_file(
    original_project_source_dir: &Path,
    cargo_path: &Path,
    function_name: &str,
) -> anyhow::Result<()> {
    let project_canonical = original_project_source_dir.canonicalize()?;

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
            let dep_canonical = original_project_source_dir.join(&buf).canonicalize()?;
            let dep_location = local_deps_cache.join(name);

            // check if the dependency is an ancestor of the project
            let is_ancestor = project_canonical
                .ancestors()
                .any(|p| p == dep_canonical.as_path());

            // check that we don't have a naming error
            // todo: automatically handle naming conflicts by mangling the dep for one
            if completed_locations.contains(&dep_location) {
                return Err(anyhow::anyhow!("two dependencies cannot share a directory name. Can the directory for one be renamed? Issue: https://github.com/DominicBurkart/turbolift/issues/1"));
            } else {
                completed_locations.insert(dep_location.clone());
            }

            if dep_location.exists() {
                if dep_canonical == dep_location.canonicalize()? {
                    // output already points to the right place, presumably because a previous
                    // turbolift build already created a symlink. No need to alter the
                    // dependency cache, just point to the cache in the manifest and move on.
                    *buf = PathBuf::from_str(".")?.join(local_deps_dir_name).join(name);
                    continue;
                }

                // the dependency cache is not correct. We should delete what's currently there
                // (note: symlinks will be removed, but the original files they link to will
                // not be altered).
                fs::remove_dir_all(&dep_location)?;
            }

            if !is_ancestor {
                symlink_dir(&dep_canonical, &dep_location)?;
            } else {
                // copy instead of symlinking here to avoid a symlink loop that will confuse and
                // break  the tar packer / unpacker.
                exclusive_recursive_copy(
                    dep_canonical.as_path(),
                    dep_location.as_path(),
                    vec![project_canonical.clone()].into_iter().collect(),
                    vec![OsStr::new("target")].into_iter().collect(),
                )?;
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

/// recursively copy a directory to a target, excluding a path and its
/// children if it exists as a descendant of the source_dir.
fn exclusive_recursive_copy(
    source_dir: &Path,
    target_dir: &Path,
    exclude_paths: HashSet<PathBuf>,
    exclude_file_names: HashSet<&OsStr>,
) -> anyhow::Result<()> {
    fs::create_dir_all(target_dir)?;
    let source_dir_canonical = source_dir.to_path_buf().canonicalize()?;
    let mut to_check = fs::read_dir(source_dir_canonical.as_path())?.collect::<VecDeque<_>>();
    while !to_check.is_empty() {
        let entry = to_check.pop_front().unwrap()?;
        let entry_path = entry.path();
        if exclude_paths.contains(&entry_path)
            || entry_path
                .file_name()
                .map_or(false, |f| exclude_file_names.contains(f))
        {
            // skip the excluded path (and, if it has any, all of its children)
        } else {
            let relative_entry_path = entry_path.strip_prefix(source_dir_canonical.as_path())?;
            let output = target_dir.join(relative_entry_path);

            if entry.metadata()?.is_dir() {
                to_check.append(&mut fs::read_dir(entry.path())?.collect::<VecDeque<_>>());
                fs::create_dir_all(output)?;
            } else {
                fs::copy(entry_path, output)?;
            }
        }
    }
    Ok(())
}

#[tracing::instrument]
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

#[tracing::instrument]
pub fn make_executable(proj_path: &Path, dest: Option<&Path>) -> anyhow::Result<()> {
    let status = Command::new("cargo")
        .current_dir(proj_path)
        .args(format!("build{}", RELEASE_FLAG).as_str().trim().split(' '))
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

#[tracing::instrument]
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
