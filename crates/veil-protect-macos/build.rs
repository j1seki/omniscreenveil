// Link the Swift static library produced by `platform/macos`.
//
// The crate compiles on every host (the FFI is `cfg(target_os = "macos")`), so
// this script only does work on macOS. It accepts an explicit path through
// `VEIL_MACOS_STATIC_LIB` (used by CI and the desktop build) and otherwise
// looks in the default SwiftPM build directory.

fn main() {
    println!("cargo:rerun-if-env-changed=VEIL_MACOS_STATIC_LIB");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let explicit = std::env::var("VEIL_MACOS_STATIC_LIB").ok();
    let default = manifest.join("../../platform/macos/.build/release/libVeilMac.a");

    if let Some(path) = explicit {
        let path = std::path::PathBuf::from(path);
        if let Some(dir) = path.parent() {
            println!("cargo:rustc-link-search=native={}", dir.display());
        }
        println!("cargo:rerun-if-changed={}", path.display());
        println!("cargo:rustc-link-lib=static=VeilMac");
        return;
    }

    if default.exists() {
        if let Some(dir) = default.parent() {
            println!("cargo:rustc-link-search=native={}", dir.display());
        }
        println!("cargo:rerun-if-changed={}", default.display());
        println!("cargo:rustc-link-lib=static=VeilMac");
    } else {
        // Fail loudly at link time with a useful message rather than silently
        // shipping a backend that cannot call the helper.
        println!(
            "cargo:warning=libVeilMac.a not found; build platform/macos with `swift build -c release` or set VEIL_MACOS_STATIC_LIB"
        );
        println!("cargo:rustc-link-lib=static=VeilMac");
    }
}
