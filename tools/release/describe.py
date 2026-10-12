"""Describes one release file as JSON: name, size, SHA-256, variant, model.

Usage: describe.py <file> --variant V --platform <mac|windows> --signed <true|false>

release.yml stores this beside each file; summary.py gathers them into
releases.json, which the download site reads (site/upload.sh).
"""
import argparse
import hashlib
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("file", type=pathlib.Path)
    ap.add_argument("--variant", required=True)
    ap.add_argument("--platform", required=True, choices=["mac", "windows"])
    ap.add_argument("--signed", required=True)
    a = ap.parse_args()
    h = hashlib.sha256()
    with a.file.open("rb") as f:
        while block := f.read(8 << 20):
            h.update(block)
    out = {
        "file": a.file.name,
        "platform": a.platform,
        "variant": a.variant,
        "bytes": a.file.stat().st_size,
        "sha256": h.hexdigest(),
        "signed": a.signed == "true",
    }
    if a.variant != "off":
        m = json.loads((ROOT / "models" / "manifest.json").read_text(encoding="utf-8"))["models"][a.variant]
        out["model"] = {"name": m["name"], "license": m["license"], "file": m["file"], "bytes": m["bytes"]}
    print(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
