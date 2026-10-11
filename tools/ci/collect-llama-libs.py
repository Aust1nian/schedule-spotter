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
    libs = [p for p in profile.glob("*.dll") if p.name.startswith(("llama", "ggml"))]
    # The CPU variants install to the sys crate's own backends folder; take
    # the most recently built one.
    backends = sorted(profile.glob("build/llama-cpp-sys-2-*/out/backends"), key=lambda p: p.stat().st_mtime)
    if backends:
        libs += list(backends[-1].glob("ggml-cpu-*.dll"))
    names = sorted({p.name for p in libs})
    if not any(n.startswith("ggml-cpu-") for n in names) or "llama.dll" not in names:
        sys.exit(f"collect-llama-libs: missing DLLs in {profile} (found: {names})")
    shutil.rmtree(OUT, ignore_errors=True)
    OUT.mkdir(parents=True)
    for p in libs:
        shutil.copy2(p, OUT / p.name)
    print(f"collect-llama-libs: {len(names)} DLLs -> {OUT.relative_to(ROOT)}: {', '.join(names)}")


if __name__ == "__main__":
    main()
