//! macOS: llama.cpp's Metal code uses `@available`, which clang compiles to a
//! call to `__isPlatformVersionAtLeast` in its own runtime library. rustc
//! links with `-nodefaultlibs`, so that library has to be named. A fresh
//! release build failed to link without it (DECISIONS.md, Phase 6).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let out = std::process::Command::new("xcrun")
        .args(["clang", "--print-runtime-dir"])
        .output()
        .expect("xcrun clang --print-runtime-dir");
    let dir = String::from_utf8(out.stdout).expect("UTF-8 path");
    let dir = dir.trim();
    assert!(
        std::path::Path::new(dir).join("libclang_rt.osx.a").is_file(),
        "no libclang_rt.osx.a in {dir}"
    );
    println!("cargo:rustc-link-search=native={dir}");
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}
