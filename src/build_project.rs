use std::process::Command;
use std::fs;
use std::path::Path;

use toml;
use cargo_toml;

use crate::DistributionResult;

pub fn edit_cargo_file(cargo_path: &Path, function_name: &str) -> DistributionResult<()> {
    let mut parsed_toml: cargo_toml::Manifest = cargo_toml::Manifest::from_path(cargo_path)?;
    // change name
    parsed_toml.package.unwrap().name = function_name.to_string() + "_turbolift";

    // add deps
    let deps = vec![
        ("actix", "actix = 0.9"),
        ("serde_json", "serde_json = 1")
    ];
    for (dep, version) in deps.into_iter() {
        if !parsed_toml.dependencies.contains_key(dep) {
            parsed_toml
                .dependencies
                .insert(
                    dep.to_string(),
                    toml::from_str(version)?
                );
        } else {
            // todo make sure that the deps versions here are compatible
        }
    }
    // parsed_toml.dependencies.contains_key()
    unimplemented!();

    let contents: String = toml::to_string_pretty(&parsed_toml).unwrap();
    fs::write(cargo_path, contents)?;
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
        let parsed_toml: cargo_toml::Manifest = cargo_toml::Manifest::from_path(cargo_path)?;
        let project_name = parsed_toml.package.unwrap().name;
        let local_path = "target/release/".to_string() + &project_name;
        proj_path.join(&local_path)
    };
    if let Some(destination) = dest {
        fs::rename(&executable_path, destination)?;
    }
    Ok(())
}