fn main() {
    let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows_msvc {
        let manifest = std::path::PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory is available"),
        )
        .join("windows-app.manifest");
        println!("cargo:rerun-if-changed={}", manifest.display());

        // Tauri's resource library is linked only into bins. Let MSVC embed the
        // same manifest into every executable, including library test harnesses.
        // Keep Tauri's icon/version resources, without a duplicate manifest #1.
        let attributes = tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        tauri_build::try_build(attributes).expect("Tauri resources must compile");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    } else {
        tauri_build::build()
    }
}
