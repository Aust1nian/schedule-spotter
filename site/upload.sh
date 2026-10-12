#!/usr/bin/env bash
# Publishes one variant of a release run to the download site on the Pi
# (replaces the spec's R2 upload script; DECISIONS.md, "Download delivery").
#
# Usage: site/upload.sh --run <release run id> --variant <C|A|B-E2B|off> --host <user@pi>
#          [--dest /srv/schedule-spotter/releases] [--allow-unsigned] [--prune]
#
# 1. Downloads the run's release-manifest and the variant's two files (gh).
# 2. Refuses files that failed an install test on a gating runner, or are
#    UNSIGNED unless --allow-unsigned (internal testing only).
# 3. Checks every file's SHA-256 against releases.json.
# 4. Copies them to the Pi with rsync (resumes if the connection drops), then
#    switches current.json over in one rename, so a visitor never sees a
#    half-uploaded file. --prune then removes files no longer offered.
set -euo pipefail
run="" variant="" host="" dest="/srv/schedule-spotter/releases" allow_unsigned=0 prune=0
while [ $# -gt 0 ]; do
  case "$1" in
    --run) run="$2"; shift ;;
    --variant) variant="$2"; shift ;;
    --host) host="$2"; shift ;;
    --dest) dest="$2"; shift ;;
    --allow-unsigned) allow_unsigned=1 ;;
    --prune) prune=1 ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done
[ -n "$run" ] && [ -n "$variant" ] && [ -n "$host" ] || { sed -n '5,7p' "$0"; exit 2; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
repo="${SPOTTER_REPO:-Aust1nian/schedule-spotter}"
gh run download "$run" -R "$repo" -n release-manifest -D "$work/manifest"
python3 - "$work/manifest/releases.json" "$variant" "$allow_unsigned" "$work/current.json" <<'PY'
import json, sys
releases, variant, allow_unsigned, out = sys.argv[1], sys.argv[2], sys.argv[3] == "1", sys.argv[4]
data = json.load(open(releases))
gating = {"mac": ["macos-15", "macos-26", "macos-15-intel"], "windows": ["windows-2025"]}
chosen = [f for f in data["files"] if f["variant"] == variant]
problems = []
for plat in ("mac", "windows"):
    if not [f for f in chosen if f["platform"] == plat]:
        problems.append(f"no {plat} file for variant {variant}")
for f in chosen:
    for runner in gating[f["platform"]]:
        if f.get("install_tests", {}).get(runner) != "pass":
            problems.append(f"{f['file']}: install test on {runner} is {f.get('install_tests', {}).get(runner, 'missing')}")
    if not f["signed"] and not allow_unsigned:
        problems.append(f"{f['file']} is UNSIGNED (use --allow-unsigned for an internal test site)")
if problems:
    sys.exit("upload: not publishing:\n  " + "\n  ".join(problems))
json.dump({"version": data["version"], "files": chosen}, open(out, "w"), indent=1)
for f in chosen:
    print(f"upload: {f['file']} ({f['bytes'] / 1e9:.2f} GB)")
PY
gh run download "$run" -R "$repo" -n "release-mac-$variant" -n "release-windows-$variant" -D "$work/files"
python3 - "$work/current.json" "$work/files" <<'PY'
import hashlib, json, pathlib, sys
cur, folder = json.load(open(sys.argv[1])), pathlib.Path(sys.argv[2])
for f in cur["files"]:
    path = next(folder.rglob(f["file"]))
    h = hashlib.sha256()
    with path.open("rb") as fh:
        while block := fh.read(8 << 20):
            h.update(block)
    if h.hexdigest() != f["sha256"]:
        sys.exit(f"upload: {f['file']} doesn't match its SHA-256")
    print(f"upload: {f['file']} checksum ok")
PY
# (A read loop rather than mapfile: macOS still ships bash 3.2.)
files=()
while IFS= read -r f; do files+=("$f"); done < <(python3 -c 'import json,sys; [print(f["file"]) for f in json.load(open(sys.argv[1]))["files"]]' "$work/current.json")
for f in "${files[@]}"; do
  rsync --partial --progress --chmod=F644 "$(find "$work/files" -name "$f" | head -1)" "$host:$dest/$f"
done
rsync --chmod=F644 "$work/current.json" "$host:$dest/current.json.new"
# shellcheck disable=SC2029  # $dest is expanded here on purpose
ssh "$host" "mv '$dest/current.json.new' '$dest/current.json'"
echo "upload: the site now offers $(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$work/current.json") ($variant)"
if [ $prune = 1 ]; then
  keep="$(printf '%s\n' current.json "${files[@]}")"
  # shellcheck disable=SC2029
  ssh "$host" "cd '$dest' && for f in *; do printf '%s\n' \"$keep\" | grep -qxF \"\$f\" || { echo \"removing \$f\"; rm -f -- \"\$f\"; }; done"
fi
