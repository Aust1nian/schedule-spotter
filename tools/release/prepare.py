"""Gets one variant ready for `tauri build` (spec: "Variant is a build flag").

Usage: prepare.py --variant <off|A|B-E2B|...> --platform <macos|windows> [--out DIR]

- Checks the model file in models/ against the manifest (fetch it first
  with models/fetch.py).
- Builds the Licenses folder for the variant (licenses.py).
- Writes a Tauri config overlay, passed as `tauri build --config <file>`,
  that adds Licenses/ to the app and, on macOS, puts the model in
  Contents/Resources/model/. The Windows installer gets its model appended
  afterwards (append-model.py), since NSIS can't hold more than about 2 GB.
  With --sign-command (Windows, signing secrets only), Tauri signs the app
  and the installer with it.

Prints the overlay's path. The build itself needs SPOTTER_VARIANT set to the
same variant, which bakes the model's name and SHA-256 into the app.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
TARGETS = {"macos": "universal-apple-darwin", "windows": "x86_64-pc-windows-msvc"}


def sha256_of(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        while block := f.read(8 << 20):
            h.update(block)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--variant", required=True)
    ap.add_argument("--platform", required=True, choices=TARGETS)
    ap.add_argument("--out", type=pathlib.Path, default=ROOT / "target" / "release-prep")
    ap.add_argument("--sign-command", default="",
                    help="Windows: Tauri's signCommand (\"%%1\" is the file); only with signing secrets")
    a = ap.parse_args()
    out = (a.out / f"{a.variant}-{a.platform}").resolve()
    out.mkdir(parents=True, exist_ok=True)

    model = None
    if a.variant != "off":
        manifest = json.loads((ROOT / "models" / "manifest.json").read_text(encoding="utf-8"))["models"]
        if a.variant not in manifest:
            sys.exit(f"prepare: {a.variant} is not in models/manifest.json")
        m = manifest[a.variant]
        model = ROOT / "models" / m["file"]
        if not model.is_file() or model.stat().st_size != m["bytes"]:
            sys.exit(f"prepare: {model} is missing or the wrong size; run models/fetch.py {a.variant}")
        if sha256_of(model) != m["sha256"]:
            sys.exit(f"prepare: {model} doesn't match its SHA-256 in the manifest")

    # The universal macOS build takes its crates from both Apple targets;
    # their license lists are the same.
    target = "aarch64-apple-darwin" if a.platform == "macos" else TARGETS[a.platform]
    licenses = out / "Licenses"
    subprocess.run(
        [sys.executable, str(ROOT / "tools" / "release" / "licenses.py"),
         "--variant", a.variant, "--target", target, "--out", str(licenses)],
        check=True,
    )

    bundle = {"resources": {f"{licenses.as_posix()}/": "Licenses/"}}
    if a.sign_command and a.platform == "windows":
        bundle["windows"] = {"signCommand": a.sign_command}
    if model and a.platform == "macos":
        bundle["macOS"] = {"files": {f"Resources/model/{model.name}": model.as_posix()}}
    overlay = out / "tauri.variant.json"
    overlay.write_text(json.dumps({"bundle": bundle}, indent=2) + "\n", encoding="utf-8")
    print(overlay)


if __name__ == "__main__":
    main()
