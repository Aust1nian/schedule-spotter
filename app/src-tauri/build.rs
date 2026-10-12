//! The variant is a build flag (spec: "Variant is a build flag"):
//! `SPOTTER_VARIANT=A|B-E2B|...|C` picks the model from models/manifest.json
//! and bakes its file name, SHA-256, name and license into the app. Unset or
//! "off" builds the app with no model: smart matching off.

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=SPOTTER_VARIANT");
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/manifest.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let variant = std::env::var("SPOTTER_VARIANT").unwrap_or_default();
    let (mut file, mut sha, mut name, mut license) =
        (String::new(), String::new(), String::new(), String::new());
    if !variant.is_empty() && variant != "off" {
        let text = std::fs::read_to_string(&manifest).expect("models/manifest.json");
        let json: serde_json::Value = serde_json::from_str(&text).expect("manifest is JSON");
        let m = &json["models"][&variant];
        assert!(
            m.is_object(),
            "SPOTTER_VARIANT={variant} is not in models/manifest.json"
        );
        let field = |k: &str| m[k].as_str().unwrap_or_default().to_string();
        (file, sha, name, license) = (
            field("file"),
            field("sha256"),
            field("name"),
            field("license"),
        );
    }
    let variant = if file.is_empty() {
        "off".to_string()
    } else {
        variant
    };
    println!("cargo:rustc-env=SPOTTER_VARIANT={variant}");
    println!("cargo:rustc-env=SPOTTER_MODEL_FILE={file}");
    println!("cargo:rustc-env=SPOTTER_MODEL_SHA={sha}");
    println!("cargo:rustc-env=SPOTTER_MODEL_NAME={name}");
    println!("cargo:rustc-env=SPOTTER_MODEL_LICENSE={license}");
    tauri_build::build()
}
