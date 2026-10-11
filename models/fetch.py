#!/usr/bin/env python3
"""Downloads pinned model files into models/ and verifies size and SHA-256.

Dev and CI only; the app itself never downloads anything.
Usage: python3 models/fetch.py A [B-E2B ...]   |   python3 models/fetch.py --verify-remote
"""

import hashlib
import json
import sys
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
MANIFEST = json.loads((HERE / "manifest.json").read_text())["models"]
CHUNK = 8 * 1024 * 1024


def sha256_of(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        while block := f.read(CHUNK):
            digest.update(block)
    return digest.hexdigest()


def fetch(key: str) -> Path:
    entry = MANIFEST[key]
    dest = HERE / entry["file"]
    if dest.exists() and dest.stat().st_size == entry["bytes"] and sha256_of(dest) == entry["sha256"]:
        print(f"{key}: {dest.name} already present and verified")
        return dest
    url = f"https://huggingface.co/{entry['repo']}/resolve/{entry['revision']}/{entry['file']}"
    part = dest.with_suffix(dest.suffix + ".part")
    print(f"{key}: downloading {url}")
    with urllib.request.urlopen(url) as response, part.open("wb") as out:
        while block := response.read(CHUNK):
            out.write(block)
    size, digest = part.stat().st_size, sha256_of(part)
    if size != entry["bytes"] or digest != entry["sha256"]:
        part.unlink()
        sys.exit(f"{key}: verification failed (size {size}, sha256 {digest})")
    part.rename(dest)
    print(f"{key}: verified {dest.name} ({size:,} bytes)")
    return dest


def verify_remote() -> int:
    """Checks that each pinned revision still serves the pinned file."""
    bad = 0
    for key, entry in MANIFEST.items():
        url = f"https://huggingface.co/api/models/{entry['repo']}/tree/{entry['revision']}"
        with urllib.request.urlopen(url) as response:
            files = {f["path"]: f for f in json.load(response)}
        found = files.get(entry["file"], {})
        lfs = found.get("lfs", {})
        ok = lfs.get("oid") == entry["sha256"] and found.get("size") == entry["bytes"]
        bad += not ok
        print(f"{key}: {'ok' if ok else 'MISMATCH'} {entry['repo']}@{entry['revision'][:10]} {entry['file']}")
    return 1 if bad else 0


if __name__ == "__main__":
    args = sys.argv[1:]
    if args == ["--verify-remote"]:
        sys.exit(verify_remote())
    if not args or any(a not in MANIFEST for a in args):
        sys.exit(f"usage: fetch.py <{'|'.join(MANIFEST)}>... | --verify-remote")
    for key in args:
        fetch(key)
