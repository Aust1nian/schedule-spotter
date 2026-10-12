"""Gathers a release run's results: releases.json and SHA256SUMS for the
download site, and a Markdown summary (stdout) for the run page.

Usage: summary.py RESULTS --version X.Y.Z --out DIR
  RESULTS/describe/*.json   one per release file (describe.py)
  RESULTS/install/*.json    one per install test (install-test-*.{sh,ps1}),
                            named install-<variant>-<runner>.json
"""
import argparse
import json
import pathlib
import re

RUNNERS = {"mac": ["macos-15", "macos-26", "macos-15-intel"], "windows": ["windows-2025", "windows-11-arm"]}


def load(path):
    try:
        return json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError):
        return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("results", type=pathlib.Path)
    ap.add_argument("--version", required=True)
    ap.add_argument("--out", type=pathlib.Path, required=True)
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)

    files = [d for p in sorted((a.results / "describe").glob("*.json")) if (d := load(p))]
    installs = {}
    for p in (a.results / "install").glob("install-*.json"):
        m = re.match(r"install-(.+)-((?:macos|windows)-[\w.-]+)\.json$", p.name)
        if m:
            installs[(m.group(1), m.group(2))] = load(p)

    lines = [f"## Release {a.version}", "",
             "| File | Size | Signed | " + " | ".join(r for rs in RUNNERS.values() for r in rs) + " |",
             "| --- | --- | --- | " + " | ".join("---" for rs in RUNNERS.values() for _ in rs) + " |"]
    for f in files:
        f["install_tests"] = {}
        cells = []
        for plat, runners in RUNNERS.items():
            for r in runners:
                if plat != f["platform"]:
                    cells.append("")
                    continue
                res = installs.get((f["variant"], r))
                status = res["install_test"] if res else "missing"
                f["install_tests"][r] = status
                st = (res or {}).get("self_test") or {}
                mc = st.get("model_check") or {}
                extra = f" (load {mc['load_s']:.1f} s)" if mc.get("load_s") else ""
                cells.append(f"{status}{extra}")
        size = f"{f['bytes'] / 1e9:.2f} GB" if f["bytes"] > 1e9 else f"{f['bytes'] / 1e6:.0f} MB"
        lines.append(f"| {f['file']} | {size} | {'yes' if f['signed'] else 'UNSIGNED'} | " + " | ".join(cells) + " |")

    failed = [(v, r, c["check"], c["detail"]) for (v, r), res in sorted(installs.items()) if res
              for c in res.get("checks", []) if c["status"] == "fail"]
    if failed:
        lines += ["", "### Failed checks", ""]
        lines += [f"- {v} on {r}: **{c}**: {d}" for v, r, c, d in failed]

    (a.out / "releases.json").write_text(
        json.dumps({"version": a.version, "files": files}, indent=1) + "\n", encoding="utf-8")
    (a.out / "SHA256SUMS").write_text(
        "".join(f"{f['sha256']}  {f['file']}\n" for f in files), encoding="utf-8")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
