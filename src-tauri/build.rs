fn main() {
    tauri_build::build();

    #[cfg(target_os = "macos")]
    {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let plist = std::path::Path::new(&manifest_dir).join("Info.plist");

        println!("cargo:rustc-link-arg-bins=-sectcreate");
        println!("cargo:rustc-link-arg-bins=__TEXT");
        println!("cargo:rustc-link-arg-bins=__info_plist");
        println!("cargo:rustc-link-arg-bins={}", plist.display());
    }
}
