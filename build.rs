fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-cfg=procmacro2_semver_exempt");
}
