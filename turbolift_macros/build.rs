use std::process::Command;
use std::{env, process};

fn main() {
    let version = match rustc_version() {
        Some(version) => version,
        None => return,
    };

    if version.minor < 45 {
        eprintln!("Turbolift requires rust 1.45 or above.");
        process::exit(1);
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-cfg=procmacro2_semver_exempt");
    println!("cargo:rustc-cfg=span_locations");
    println!("cargo:rustc-cfg=use_proc_macro");
    println!("cargo:rustc-cfg=hygiene");
    if version.nightly {
        println!("cargo:rustc-cfg=super_unstable");
        println!("cargo:rustc-cfg=proc_macro_span");
        println!("cargo:rustc-cfg=wrap_proc_macro");
    }
}

struct RustcVersion {
    minor: u32,
    nightly: bool,
}

fn rustc_version() -> Option<RustcVersion> {
    let rustc = env::var_os("RUSTC")?;
    let output = Command::new(rustc).arg("--version").output().ok()?;
    let version = std::str::from_utf8(&output.stdout).ok()?;
    let nightly = version.contains("nightly") || version.contains("dev");
    let mut pieces = version.split('.');
    if pieces.next() != Some("rustc 1") {
        return None;
    }
    let minor = pieces.next()?.parse().ok()?;
    Some(RustcVersion { minor, nightly })
}
