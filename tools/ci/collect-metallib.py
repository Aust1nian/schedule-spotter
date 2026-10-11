"""Copies llama.cpp's precompiled Metal library (default.metallib, built when
GGML_METAL_EMBED_LIBRARY=OFF) out of the cargo build, so it can sit beside a
binary or go into the app bundle's Resources. llama.cpp looks for it as a
bundle resource first, then next to the running binary.

  --to DIR   where to copy (default app/src-tauri/macos)
Run by `tauri build` as the macOS beforeBundleCommand."""
import argparse
import pathlib
import shutil
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--to", default=str(ROOT / "app" / "src-tauri" / "macos"))
    args = ap.parse_args()
    # Apple Silicon builds only (the Intel slice is built without Metal); the
    # newest one wins.
    found = [p for p in (ROOT / "target").glob("**/build/llama-cpp-sys-2-*/out/**/default.metallib")]
    found.sort(key=lambda p: p.stat().st_mtime)
    if not found:
        sys.exit("collect-metallib: no default.metallib under target/ (is GGML_METAL_EMBED_LIBRARY=OFF?)")
    out = pathlib.Path(args.to).resolve()
    out.mkdir(parents=True, exist_ok=True)
    shutil.copy2(found[-1], out / "default.metallib")
    print(f"collect-metallib: {found[-1].relative_to(ROOT)} -> {out / 'default.metallib'}")


if __name__ == "__main__":
    main()
