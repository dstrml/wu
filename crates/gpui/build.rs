#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo::rustc-check-cfg=cfg(gles)");

    // Leak detection does bookkeeping on every entity handle, so `test-support`
    // does not enable it: that feature must stay cheap enough to compile into
    // every build. CI sets `GPUI_LEAK_DETECTION`; the `leak-detection` feature
    // turns it on explicitly.
    println!("cargo::rustc-check-cfg=cfg(gpui_leak_detection)");
    println!("cargo::rerun-if-env-changed=GPUI_LEAK_DETECTION");
    let requested_by_environment =
        std::env::var_os("GPUI_LEAK_DETECTION").is_some_and(|value| !value.is_empty());
    if requested_by_environment || std::env::var_os("CARGO_FEATURE_LEAK_DETECTION").is_some() {
        println!("cargo::rustc-cfg=gpui_leak_detection");
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if target_os == "windows" {
        #[cfg(feature = "windows-manifest")]
        embed_resource();
    }
}

#[cfg(feature = "windows-manifest")]
fn embed_resource() {
    let resource_dir = std::path::Path::new("resources/windows");
    let manifest = resource_dir.join("gpui.manifest.xml");
    let rc_file = resource_dir.join("gpui.rc");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rerun-if-changed={}", rc_file.display());
<<<<<<< 7a576a25c2d16d37124c400cf48068b5f26d1123
    let rc_file = if cfg!(windows) {
        rc_file.to_path_buf()
    } else {
        rc_file_with_absolute_manifest_path(rc_file, manifest)
    };
    embed_resource::compile(rc_file, embed_resource::NONE)
=======
    embed_resource::compile(rc_file, embed_resource::ParamsIncludeDirs([resource_dir]))
>>>>>>> f8381935e291dcc13a1e90480cb212972c459d31
        .manifest_required()
        .unwrap();
}

// When cross-compiling, llvm-rc resolves paths relative to the .rc file.
#[cfg(feature = "windows-manifest")]
fn rc_file_with_absolute_manifest_path(
    rc_file: &std::path::Path,
    manifest: &std::path::Path,
) -> std::path::PathBuf {
    let relative_manifest = format!("\"{}\"", manifest.display());
    let absolute_manifest = format!("\"{}\"", std::fs::canonicalize(manifest).unwrap().display());
    let contents = std::fs::read_to_string(rc_file).unwrap();
    assert!(
        contents.contains(&relative_manifest),
        "{} must reference {relative_manifest}",
        rc_file.display()
    );
    let generated = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("gpui.rc");
    std::fs::write(
        &generated,
        contents.replace(&relative_manifest, &absolute_manifest),
    )
    .unwrap();
    generated
}
