use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if is_windows_msvc() {
        let manifest = PathBuf::from(
            env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must set CARGO_MANIFEST_DIR"),
        )
        .join("../../apps/desktop/src-tauri/windows-app-manifest.xml");

        embed_windows_manifest(&manifest);
    }
}

fn is_windows_msvc() -> bool {
    env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
}

fn embed_windows_manifest(manifest: &std::path::Path) {
    println!("cargo:rerun-if-changed={}", manifest.display());

    // Use the general directive because rustc-link-arg-tests does not cover a
    // library's unit-test harness, which is the executable that links Tauri.
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}
