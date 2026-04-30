use std::env;

fn main() {
    // On Windows, rename the output cdylib to glu32.dll for DLL proxy hijacking.
    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let project_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let profile = env::var("PROFILE").unwrap();
        println!(
            "cargo:rustc-cdylib-link-arg=/OUT:{}\\target\\{}\\glu32.dll",
            project_dir, profile
        );
    }
}
