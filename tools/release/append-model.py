"""Appends a variant's model to the finished Windows setup .exe.

Usage: append-model.py --variant <A|B-E2B|...> --setup <...-setup.exe>

NSIS can't hold more than about 2 GB, and the larger models are 2.5-3.4 GB,
so the model rides after the installer's own data: model bytes, their length
as a little-endian u64, then b"SPOTMDL1". The installer's post-install hook
runs `<app> --install-model <setup.exe>`, which copies it into place and
checks its SHA-256 (app/src-tauri/src/payload.rs). Sign after this step,
never before: signing adds its certificate table at the very end.
"""
import argparse
import hashlib
import json
import pathlib
import struct
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
MAGIC = b"SPOTMDL1"
# Installer vendors report that Windows won't start an .exe of 4 GiB or
# more; the release workflow's install test proves the sizes we ship.
LIMIT = 4 * 1024**3


def has_signature(path):
    with path.open("rb") as f:
        mz = f.read(64)
        if mz[:2] != b"MZ":
            sys.exit(f"append-model: {path} isn't a Windows program")
        pe = struct.unpack_from("<I", mz, 0x3C)[0]
        f.seek(pe)
        head = f.read(26)
        magic = struct.unpack_from("<H", head, 24)[0]
        dirs = pe + 24 + (96 if magic == 0x10B else 112)
        f.seek(dirs + 4 * 8)
        offset, size = struct.unpack("<II", f.read(8))
    return offset != 0 and size != 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--variant", required=True)
    ap.add_argument("--setup", required=True, type=pathlib.Path)
    a = ap.parse_args()
    m = json.loads((ROOT / "models" / "manifest.json").read_text(encoding="utf-8"))["models"][a.variant]
    model = ROOT / "models" / m["file"]
    if has_signature(a.setup):
        sys.exit("append-model: the installer is already signed; append the model first, then sign")
    with a.setup.open("rb") as f:
        f.seek(-16, 2)
        if f.read(16)[8:] == MAGIC:
            sys.exit("append-model: the installer already carries a model")

    total = a.setup.stat().st_size + m["bytes"] + 16
    if total >= LIMIT:
        sys.exit(f"append-model: {total / 1e9:.2f} GB would pass the 4 GiB limit for a Windows program")
    h = hashlib.sha256()
    n = 0
    with model.open("rb") as src, a.setup.open("ab") as dst:
        while block := src.read(8 << 20):
            h.update(block)
            dst.write(block)
            n += len(block)
        if n != m["bytes"] or h.hexdigest() != m["sha256"]:
            sys.exit("append-model: the model file doesn't match the manifest; the installer is now damaged, rebuild it")
        dst.write(struct.pack("<Q", n) + MAGIC)
    print(f"append-model: {a.setup.name} now carries {m['file']} ({n / 1e9:.2f} GB); {total / 1e9:.2f} GB in all")


if __name__ == "__main__":
    main()
