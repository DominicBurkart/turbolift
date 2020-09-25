use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow;
use cargo_toml2;
use toml;

use crate::utils::symlink_dir;

pub fn edit_cargo_file(cargo_path: &Path, function_name: &str) -> anyhow::Result<()> {
    println!("hi hi hi");
    let mut parsed_toml: cargo_toml2::CargoToml = cargo_toml2::from_path(cargo_path)?;
    let relative_local_deps_cache = cargo_path
        .parent()
        .unwrap()
        .join(".local_deps");
    fs::create_dir_all(&relative_local_deps_cache)?;
    let local_deps_cache = relative_local_deps_cache.canonicalize()?;

    // change name
    parsed_toml.package.name = function_name.to_string() + "_turbolift";

    // add deps
    let new_deps = vec![
        ("actix-web", "2"),
        ("actix-rt", "1"),
        ("serde_json", "1"),
        ("rust-embed", "5")
    ];
    let mut deps = match parsed_toml.dependencies {
        Some(deps) => {
            deps
        },
        None => {
            Default::default()
        }
    };
    for (dep, version) in new_deps.into_iter() {
        if !deps.contains_key(dep) {
            deps
                .insert(
                    dep.to_string(),
                    cargo_toml2::Dependency::Full(
                        cargo_toml2::DependencyFull {
                            version: Some(version.to_string()),
                            ..Default::default()
                        }
                    ),
                );
        } else {
            // todo make sure that the deps versions here are compatible
        }
    }

    // symlink any local directories so they work with the new project location
    let details = deps
        .iter_mut()
        // only full dependency descriptions (not simple version number)
        .filter_map(
            |(_name, dep)| match dep {
                cargo_toml2::Dependency::Simple(_) => None,
                cargo_toml2::Dependency::Full(detail) => Some(detail),
            }
        );
    let mut completed_locations = HashSet::new();
    for detail in details {
        // only descriptions with a path
        match detail.path {
            Some(ref mut buf) => {
                // determine what the symlink for this dependency should be
                println!("buf: {:?}", &buf);
                let canonical = buf.canonicalize()?;
                let dep_location = local_deps_cache.join(
                    &canonical.file_name().unwrap()
                );
                println!("dep_location, {:?}", dep_location);

                // check that we don't have a naming error
                // todo: automatically handle naming conflicts by mangling the dep for one
                if completed_locations.contains(&dep_location) {
                    return Err(anyhow::anyhow!("two dependencies cannot share a directory name. Can the directory for one be renamed? Issue: https://github.com/DominicBurkart/turbolift/issues/1"));
                } else {
                    completed_locations.insert(dep_location.clone());
                }

                println!("hi hi");
                if dep_location.exists() {
                    // dependency already exists, does it point to the correct place?
                    if canonical == dep_location.canonicalize()? {
                        // output already points to the right place, do nothing
                         println!("kk");
                    } else {
                        // output points somewhere else; delete it; if it's non-empty, error
                        println!("hmm");
                        fs::remove_dir(&dep_location).unwrap();
                        symlink_dir(&canonical, &dep_location)?;
                    }
                } else {
                    println!("ah");
                    symlink_dir(&canonical, &dep_location)?;
                }
                *buf = dep_location;
            },
            None => (),
        }
    }
    println!("hello");

    // mutate all simple definitions to full ones to avoid toml serialization bug
    // ^ https://github.com/DianaNites/cargo-toml2/blob/89fc8e6055d5ee3e8a2ae614f656d79f38e59f9f/README.md#limitations
    deps
        .iter_mut()
        .for_each(
            |(_name, dep)| match dep {
                cargo_toml2::Dependency::Simple(simple_version) => {
                    let full = cargo_toml2::DependencyFull {
                        version: Some(simple_version.clone()),
                        ..Default::default()
                    };
                    *dep = cargo_toml2::Dependency::Full(full);
                },
                _ => ()
            }
        );
    println!("ok.... ");
    parsed_toml.dependencies = Some(deps);
    cargo_toml2::to_path(cargo_path, parsed_toml)?;
    println!("huh.... ");
    Ok(())
}

pub fn lint(proj_path: &Path) -> anyhow::Result<()> {
    let install_status = Command::new("rustup")
        .args("component add rustfmt".split(" "))
        .status()?;

    if !install_status.success() {
        return Err(anyhow::anyhow!("clippy install failed"));
    }

    let status = Command::new("cargo")
        .current_dir(proj_path)
        .args("fmt".split(" "))
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("rustfmt fix failed"));
    }
    Ok(())
}

pub fn make_executable(proj_path: &Path, dest: Option<&Path>) -> anyhow::Result<()> {
    let status = Command::new("cargo")
        .current_dir(proj_path)
        .args("build --release".split(" "))
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("cargo build failed with code: {:?}", status.code()));
    }
    let executable_path = {
        let cargo_path = proj_path.join("Cargo.toml");
        let parsed_toml: cargo_toml2::CargoToml = cargo_toml2::from_path(cargo_path)?;
        let project_name = parsed_toml.package.name;
        let local_path = "target/release/".to_string() + &project_name;
        proj_path.join(&local_path)
    };
    if let Some(destination) = dest {
        fs::rename(&executable_path, destination)?;
    }
    Ok(())
}

pub fn fix(proj_path: &Path) -> anyhow::Result<()> {
    let status = Command::new("cargo")
        .current_dir(proj_path)
        .args("fix --allow-dirty".split(" "))
        .status()?;

    if !status.success() {
        return Err(anyhow::anyhow!("cargo fix failed with code: {:?}", status.code()));
    }
    Ok(())
}