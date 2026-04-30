use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    // On Windows, rename the output cdylib to glu32.dll for DLL proxy hijacking.
    if target_os == "windows" {
        let project_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let profile = env::var("PROFILE").unwrap();
        println!(
            "cargo:rustc-cdylib-link-arg=/OUT:{}\\target\\{}\\glu32.dll",
            project_dir, profile
        );
    }

    // On macOS, no special linker args needed.
    // The cdylib is injected via DYLD_INSERT_LIBRARIES.
    // On Linux, no special linker args needed.
    // The cdylib is injected via LD_PRELOAD.
}
