fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-cfg=use_proc_macro");
    println!("cargo:rustc-cfg=wrap_proc_macro");
    println!("cargo:rustc-cfg=hygiene");
    println!("cargo:rustc-cfg=proc_macro_span");
    println!("cargo:rustc-cfg=super_unstable");
    println!("cargo:rustc-cfg=span_locations");
    println!("cargo:rustc-cfg=procmacro2_semver_exempt");
    println!("cargo:rustc-cfg=lexerror_display");
}
