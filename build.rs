use std::env;

fn main() {
    let project_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let profile = env::var("PROFILE").unwrap();
    println!(
        "cargo:rustc-cdylib-link-arg=/OUT:{}\\target\\{}\\glu32.dll",
        project_dir, profile
    );
}
