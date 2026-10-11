"""Copies llama.cpp's Windows DLLs (llama, ggml, ggml-base, the ggml-cpu-*
variants) from the build into app/src-tauri/windows/llama-libs, where the
NSIS hook (app/src-tauri/windows/hooks.nsh) installs them beside the exe.
Run by `tauri build` as the Windows beforeBundleCommand; paths are resolved
from this file, so the working directory doesn't matter."""
import pathlib
import shutil
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "app" / "src-tauri" / "windows" / "llama-libs"


def main():
    profile = ROOT / "target" / "release"
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
    shutil.rmtree(OUT, ignore_errors=True)
    OUT.mkdir(parents=True)
    for p in libs:
        shutil.copy2(p, OUT / p.name)
    print(f"collect-llama-libs: {len(names)} DLLs -> {OUT.relative_to(ROOT)}: {', '.join(names)}")


if __name__ == "__main__":
    main()
