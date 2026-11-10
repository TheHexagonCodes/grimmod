fn main() {
    println!("cargo:rerun-if-env-changed=DLL_VERSION");

    let version =
        std::env::var("DLL_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());

    let parts: Vec<u64> = version
        .split('.')
        .map(|p| p.parse().expect("Invalid version component"))
        .collect();

    assert!(
        parts.len() == 3 && parts.iter().all(|&v| v <= u16::MAX as u64),
        "Version must be MAJOR.MINOR.PATCH with components <= 65535"
    );

    println!("cargo:rustc-env=DLL_VERSION={version}");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();

        res.set("FileDescription", "Grimmod DLL");
        res.set("ProductName", "Grimmod");
        res.set("CompanyName", "TheHexagonCodes");
        res.set("OriginalFilename", "glu32.dll");
        res.set("FileVersion", &format!("{version}.0"));
        res.set("ProductVersion", &version);

        let quad = parts
            .iter()
            .zip([48u64, 32, 16])
            .fold(0u64, |acc, (&v, shift)| acc | (v << shift));

        res.set_version_info(winres::VersionInfo::FILEVERSION, quad);
        res.set_version_info(winres::VersionInfo::PRODUCTVERSION, quad);
        res.set_version_info(winres::VersionInfo::FILETYPE, 2);

        res.compile().expect("Failed to compile Windows resources");
    }
}
