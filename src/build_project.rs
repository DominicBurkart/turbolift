use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_toml2;
use toml;

use crate::DistributionResult;

pub fn edit_cargo_file(cargo_path: &Path, function_name: &str) -> DistributionResult<()> {
    let mut parsed_toml: cargo_toml2::CargoToml = cargo_toml2::from_path(cargo_path)?;

    // change name
    parsed_toml.package.name = function_name.to_string() + "_turbolift";

    // add deps
    let new_deps = vec![
        ("actix", "0.9"),
        ("serde_json", "1")
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

    // mutate any paths so that they work when the toml is at ./{CACHE DIR}/{FUNC DIR}/Cargo.toml
    deps
        .iter_mut()
        // only full dependency descriptions (not simple version number)
        .filter_map(
            |(_name, dep)| match dep {
                cargo_toml2::Dependency::Simple(_) => None,
                cargo_toml2::Dependency::Full(detail) => Some(detail),
            }
        )
        // only descriptions with a path
        .for_each(
            |detail| match detail.path {
                Some(ref mut buf) => {
                    let new = PathBuf::from("../..").join(&buf);
                    *buf = new;
                },
                None => (),
            }
        );

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
    parsed_toml.dependencies = Some(deps);
    cargo_toml2::to_path(cargo_path, parsed_toml)?;
    Ok(())
}

pub fn make_executable(proj_path: &Path, dest: Option<&Path>) -> DistributionResult<()> {
    let status = Command::new("cargo")
        .current_dir(proj_path)
        .arg("build")
        .arg("--release")
        .status()?;

    if !status.success() {
        panic!("[make_executable]: cargo build failed")
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