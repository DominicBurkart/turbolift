use std::process::Command;
use std::fs;
use std::path::Path;

use toml;

pub fn make_executable(proj_path: &Path, dest: Option<&Path>) -> std::io::Result<()> {
    let status = Command::new("cargo")
        .current_dir(proj_path)
        .arg("build")
        .arg("--release")
        .status()?;

    if !status.success() {
        panic!("[make_executable]: cargo build failed")
    }
    let executable_path = {
        let toml_contents = fs::read_to_string(proj_path.join("Cargo.toml"))?;
        let parsed_toml: toml::Value = toml::from_str(&toml_contents)?;
        let project_name = parsed_toml["name"]
            .as_str()
            .unwrap();
        let local_path = "target/release/".to_string() + project_name;
        proj_path.join(&local_path)
    };
    if let Some(destination) = dest {
        fs::rename(&executable_path, destination)?;
    }
    Ok(())
}