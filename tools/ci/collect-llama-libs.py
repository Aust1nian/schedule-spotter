"""Copies llama.cpp's Windows DLLs (llama, ggml, ggml-base, the ggml-cpu-*
variants) from the build into app/src-tauri/windows/llama-libs, where the
NSIS hook (app/src-tauri/windows/hooks.nsh) installs them beside the exe.
Run by `tauri build` as the Windows beforeBundleCommand; paths are resolved
from this file, so the working directory doesn't matter.

  --profile DIR  the cargo profile folder to read (default target/release)
  --to DIR       where to copy (default app/src-tauri/windows/llama-libs);
                 the benchmark uses it to put the DLLs beside spotter-bench.exe"""
import argparse
import pathlib
import shutil
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "app" / "src-tauri" / "windows" / "llama-libs"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--profile", default=str(ROOT / "target" / "release"))
    ap.add_argument("--to", default=str(OUT))
    args = ap.parse_args()
    profile = pathlib.Path(args.profile).resolve()
    out = pathlib.Path(args.to).resolve()
    # The sys crate's newest build output: shared libraries in out/bin, the
    # CPU variants in out/backends. Anything copied beside the target
    # folder's binaries counts too.
    outs = sorted(profile.glob("build/llama-cpp-sys-2-*/out"), key=lambda p: p.stat().st_mtime)
    found = {}
    for p in profile.glob("*.dll"):
        if p.name.startswith(("llama", "ggml")):
            found[p.name] = p
    if outs:
        for sub in ("bin", "lib", "backends"):
            for p in (outs[-1] / sub).glob("*.dll"):
                found[p.name] = p
    libs = list(found.values())
    names = sorted(found)
    if not any(n.startswith("ggml-cpu-") for n in names) or "llama.dll" not in names:
        listing = [str(p.relative_to(outs[-1])) for p in outs[-1].rglob("*.dll")] if outs else []
        sys.exit(f"collect-llama-libs: missing DLLs (found: {names}; every DLL in the build output: {listing})")
    if out == OUT.resolve():
        shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True, exist_ok=True)
    for p in libs:
        if p.parent.resolve() != out:
            shutil.copy2(p, out / p.name)
    print(f"collect-llama-libs: {len(names)} DLLs -> {out}: {', '.join(names)}")


if __name__ == "__main__":
    main()
