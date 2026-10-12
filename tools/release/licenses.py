"""Builds the app's Licenses folder (spec: "Licenses ship with the app").

Usage: licenses.py --variant <off|A|B-E2B|...> --target <rust triple> --out <dir>

Contents:
  README.txt              what is here
  third-party-rust.txt    every crate compiled into the app for <target>
                          (normal dependencies only: build tools and tests
                          don't ship), with its license files
  third-party-js.txt      the JavaScript bundled into the page
  llama.cpp/              llama.cpp's license and the vendored code it links
  WebView2.txt            Windows only: the runtime the installer carries
  model/                  the variant's model license and notices

Run it after a build, so cargo has every crate's source on disk.
"""
import argparse
import json
import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
APP_PACKAGE = "schedule-spotter"
LICENSE_FILE = re.compile(r"^(licen[cs]e|copying|notice|copyright|authors)([-_.].*)?$", re.I)
MODEL_LICENSES = {"A": "phi-4-mini", "C": "llama-3.2"}  # every B-* is Gemma 4


def crates(target):
    meta = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", target],
            cwd=ROOT, check=True, capture_output=True, text=True,
        ).stdout
    )
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = next(p["id"] for p in meta["packages"] if p["name"] == APP_PACKAGE)
    seen, todo = set(), [root]
    while todo:
        pid = todo.pop()
        if pid in seen:
            continue
        seen.add(pid)
        for dep in nodes[pid]["deps"]:
            if any(k["kind"] is None for k in dep["dep_kinds"]):
                todo.append(dep["pkg"])
    workspace = set(meta["workspace_members"])
    return sorted(
        (packages[p] for p in seen if p not in workspace),
        key=lambda p: (p["name"], p["version"]),
    )


def license_texts(pkg):
    folder = pathlib.Path(pkg["manifest_path"]).parent
    files = sorted(f for f in folder.iterdir() if f.is_file() and LICENSE_FILE.match(f.name))
    if pkg.get("license_file"):
        extra = folder / pkg["license_file"]
        if extra.is_file() and extra not in files:
            files.append(extra)
    return [(f.name, f.read_text(encoding="utf-8", errors="replace")) for f in files]


TEXTS = pathlib.Path(__file__).resolve().parent / "license-texts"


def standard_text(pkg):
    """For a crate with no license file: the standard text of the first
    license its expression names that we have, with its authors as the
    copyright holders."""
    for lic in re.findall(r"[A-Za-z0-9.+-]+", pkg.get("license") or ""):
        f = TEXTS / f"{lic}.txt"
        if lic not in ("OR", "AND", "WITH") and f.is_file():
            holders = ", ".join(re.sub(r"\s*<[^>]*>", "", a) for a in pkg.get("authors", [])) or f"the {pkg['name']} authors"
            text = f.read_text(encoding="utf-8")
            text = re.sub(r"<year>\s*<(copyright holders|owner)>", holders, text)
            return lic, text
    return None, None


def rust(target, out):
    missing = []
    first_seen = {}
    parts = [
        "Rust crates compiled into Schedule Spotter",
        "=" * 42,
        "",
        f"Target: {target}. Each crate's name, version, declared license and the license",
        "files it ships. Where a crate offers a choice of licenses, Schedule Spotter",
        "uses it under the first one that its files include. The source code of every",
        "crate, including those under the MPL-2.0, is at https://crates.io/crates/<name>/<version>",
        "and at the repository listed.",
        "",
    ]
    for pkg in crates(target):
        texts = license_texts(pkg)
        if not texts:
            missing.append(f"{pkg['name']} {pkg['version']}")
        parts.append("-" * 78)
        parts.append(f"{pkg['name']} {pkg['version']}  ({pkg.get('license') or 'see files'})")
        if pkg.get("repository"):
            parts.append(pkg["repository"])
        parts.append("")
        for name, text in texts:
            text = text.strip()
            where = f"{pkg['name']} {pkg['version']}, {name}"
            if text in first_seen:
                parts += [f"--- {name}: the same text as {first_seen[text]} above ---", ""]
            else:
                first_seen[text] = where
                parts += [f"--- {name} ---", text, ""]
        if not texts:
            lic, text = standard_text(pkg)
            if text:
                parts += [f"--- the crate ships no license file; standard {lic} text ---", text.strip(), ""]
            else:
                parts += [f"(The crate ships no license file; its declared license is {pkg.get('license')}.)", ""]
    (out / "third-party-rust.txt").write_text("\n".join(parts) + "\n", encoding="utf-8")
    return missing


def llama(out, target):
    sys_crate = next((p for p in crates(target) if p["name"] == "llama-cpp-sys-2"), None)
    if sys_crate is None:
        sys.exit("llama-cpp-sys-2 is not in the dependency tree")
    src = pathlib.Path(sys_crate["manifest_path"]).parent / "llama.cpp"
    dest = out / "llama.cpp"
    dest.mkdir()
    shutil.copy(src / "LICENSE", dest / "LICENSE")
    # nlohmann/json is compiled in (llama.cpp's grammar code); its notice
    # is the header comment of json.hpp.
    header = (src / "vendor" / "nlohmann" / "json.hpp").read_text(encoding="utf-8").splitlines()
    notice = [l for l in header[:12] if l.startswith("//")]
    (dest / "nlohmann-json.txt").write_text(
        "nlohmann/json, vendored by llama.cpp and compiled into Schedule Spotter.\n\n"
        + "\n".join(notice)
        + "\n\nThe MIT License text is the same as in llama.cpp/LICENSE above, with the copyright line shown here.\n",
        encoding="utf-8",
    )


def js(out):
    pkg = ROOT / "app" / "node_modules" / "@tauri-apps" / "api"
    meta = json.loads((pkg / "package.json").read_text(encoding="utf-8"))
    parts = [
        "JavaScript bundled into Schedule Spotter's page",
        "=" * 46,
        "",
        f"@tauri-apps/api {meta['version']}  ({meta.get('license')})",
        "",
    ]
    for f in sorted(pkg.iterdir()):
        if f.is_file() and LICENSE_FILE.match(f.name):
            parts += [f"--- {f.name} ---", f.read_text(encoding="utf-8").strip(), ""]
    (out / "third-party-js.txt").write_text("\n".join(parts) + "\n", encoding="utf-8")


def model(variant, out):
    if variant == "off":
        return None
    manifest = json.loads((ROOT / "models" / "manifest.json").read_text(encoding="utf-8"))
    m = manifest["models"][variant]
    folder = MODEL_LICENSES.get(variant, "gemma-4" if variant.startswith("B") else None)
    src = ROOT / "models" / "licenses" / folder
    dest = out / "model"
    dest.mkdir()
    for f in sorted(src.iterdir()):
        shutil.copy(f, dest / f.name)
    (dest / "README.txt").write_text(
        f"{m['name']}\nLicense: {m['license']}\nSource: https://huggingface.co/{m['repo']} "
        f"(revision {m['revision']})\nFile: {m['file']}\nSHA-256: {m['sha256']}\n"
        + ("\nBuilt with Llama.\n" if variant == "C" else ""),
        encoding="utf-8",
    )
    return m


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--variant", required=True)
    ap.add_argument("--target", required=True)
    ap.add_argument("--out", required=True, type=pathlib.Path)
    a = ap.parse_args()
    if a.out.exists():
        shutil.rmtree(a.out)
    a.out.mkdir(parents=True)

    missing = rust(a.target, a.out)
    llama(a.out, a.target)
    js(a.out)
    windows = "windows" in a.target
    if windows:
        (a.out / "WebView2.txt").write_text(
            "Microsoft Edge WebView2 Runtime\n\n"
            "The Windows installer carries Microsoft's WebView2 Runtime offline installer and runs it\n"
            "only when the computer doesn't already have the runtime. The runtime is Microsoft\n"
            "software under Microsoft's own license terms, which it installs with itself:\n"
            "https://developer.microsoft.com/microsoft-edge/webview2/\n",
            encoding="utf-8",
        )
    m = model(a.variant, a.out)
    readme = [
        "Schedule Spotter: licenses",
        "",
        "Schedule Spotter is (c) 2026 Tight Line. All rights reserved.",
        "It is built from the open-source software listed in this folder, each under its own license:",
        "",
        "  third-party-rust.txt   the Rust libraries compiled into the app, including Tauri (MIT or Apache-2.0)",
        "  llama.cpp/             llama.cpp (MIT), which runs the model, and the code it includes",
        "  third-party-js.txt     the JavaScript in the app's window",
    ]
    if windows:
        readme.append("  WebView2.txt           the Microsoft Edge WebView2 Runtime")
    if m:
        readme.append(f"  model/                 {m['name']}, under the {m['license']}")
    else:
        readme.append("")
        readme.append("This copy has no model.")
    if a.variant == "C":
        readme += ["", "Built with Llama."]
    (a.out / "README.txt").write_text("\n".join(readme) + "\n", encoding="utf-8")
    if missing:
        print("crates with no license file (declared license recorded):", ", ".join(missing))
    print(f"Licenses for {a.variant} on {a.target}: {a.out}")


if __name__ == "__main__":
    main()
