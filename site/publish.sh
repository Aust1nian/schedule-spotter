#!/usr/bin/env bash
# Publishes one variant of a tested release run on tightline.systems/schedule.
# The download page and file hosting belong to the Tight Line website
# (a separate project): its scripts/release.mjs copies the files to
# get.tightline.systems/schedule/<version>/ and records them for the page.
# There is no email gate (Austin, 2026-10-14; the site's /privacy promises
# no email is needed to download).
#
# Usage: site/publish.sh --run <release run id> --variant <C|A|B-E2B|off>
#          [--website /mnt/disk1/Burnes_Hink_&_Co/Tight_Line_Website] [--dry-run]
#
# 1. Downloads the run's release-manifest and the variant's two files (gh).
# 2. Refuses files that failed an install test on a gating runner, and a Mac
#    build that isn't signed and notarized (the website says it is). Windows
#    builds are unsigned until Austin decides on Windows signing.
# 3. Checks each file's SHA-256 against releases.json, gives it the name
#    clients see (no variant letter, no UNSIGNED), and runs release.mjs.
# The website's own next steps (deploy, check, commit) are printed at the end.
set -euo pipefail
run="" variant="" dry=()
website="/mnt/disk1/Burnes_Hink_&_Co/Tight_Line_Website"
while [ $# -gt 0 ]; do
  case "$1" in
    --run) run="$2"; shift ;;
    --variant) variant="$2"; shift ;;
    --website) website="$2"; shift ;;
    --dry-run) dry=(--dry-run) ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done
[ -n "$run" ] && [ -n "$variant" ] || { sed -n '9,10p' "$0"; exit 2; }
[ -f "$website/scripts/release.mjs" ] || { echo "publish: no scripts/release.mjs under $website" >&2; exit 2; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
repo="${SPOTTER_REPO:-Aust1nian/schedule-spotter}"
gh run download "$run" -R "$repo" -n release-manifest -D "$work/manifest"
# The checks first, so a refusal doesn't download gigabytes.
python3 - "$work/manifest/releases.json" "$variant" <<'PY'
import json, sys
releases, variant = sys.argv[1], sys.argv[2]
data = json.load(open(releases))
gating = {"mac": ["macos-15", "macos-26", "macos-15-intel"], "windows": ["windows-2025"]}
chosen = {f["platform"]: f for f in data["files"] if f["variant"] == variant}
problems = [f"no {p} file for variant {variant}" for p in ("mac", "windows") if p not in chosen]
for f in chosen.values():
    for runner in gating[f["platform"]]:
        got = f.get("install_tests", {}).get(runner, "missing")
        if got != "pass":
            problems.append(f"{f['file']}: install test on {runner} is {got}")
if "mac" in chosen and not chosen["mac"]["signed"]:
    problems.append(f"{chosen['mac']['file']} isn't signed and notarized")
if problems:
    sys.exit("publish: not publishing:\n  " + "\n  ".join(problems))
PY
gh run download "$run" -R "$repo" -n "release-mac-$variant" -n "release-windows-$variant" -D "$work/files"
python3 - "$work/manifest/releases.json" "$variant" "$work/files" "$work/out" >"$work/args" <<'PY'
import hashlib, json, pathlib, shutil, sys
releases, variant, folder, out = sys.argv[1], sys.argv[2], pathlib.Path(sys.argv[3]), pathlib.Path(sys.argv[4])
data = json.load(open(releases))
chosen = {f["platform"]: f for f in data["files"] if f["variant"] == variant}
out.mkdir()
version = data["version"]
public = {"mac": f"Schedule-Spotter-{version}-mac.dmg", "windows": f"Schedule-Spotter-{version}-windows-setup.exe"}
args = ["--version", version]
for plat, f in chosen.items():
    src = next(folder.rglob(f["file"]))
    h = hashlib.sha256()
    with src.open("rb") as fh:
        while block := fh.read(8 << 20):
            h.update(block)
    if h.hexdigest() != f["sha256"]:
        sys.exit(f"publish: {f['file']} doesn't match its SHA-256")
    dest = out / public[plat]
    shutil.move(src, dest)
    args += [f"--{plat}", str(dest)]
    signed = "signed" if f["signed"] else "unsigned"
    print(f"publish: {f['file']} -> {dest.name} ({f['bytes'] / 1e9:.2f} GB, {signed}, checksum ok)", file=sys.stderr)
print("\n".join(args))
PY
mapargs=()
while IFS= read -r a; do mapargs+=("$a"); done <"$work/args"
node "$website/scripts/release.mjs" --tool schedule "${mapargs[@]}" ${dry[@]+"${dry[@]}"}
