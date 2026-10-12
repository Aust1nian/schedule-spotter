#!/usr/bin/env bash
# Install test (spec: "Install tests, every release candidate"): marks the
# disk image as downloaded the way a browser does (quarantine), mounts it,
# copies the app to Applications, checks its signature and Gatekeeper's
# verdict, runs --self-test while watching for network sockets, and checks
# the app left nothing behind.
#
# Usage: install-test-macos.sh <path.dmg> <out.json> [--signed] [--variant V]
#   --signed   the build must be Developer ID signed, notarized and stapled,
#              and launches still quarantined. Without it (no signing secrets
#              yet) those checks are reported as skipped, and the quarantine
#              is removed before launch, as the owner's "Open Anyway" would.
set -euo pipefail
dmg="$1"
out="$2"
shift 2
signed=0
variant=""
while [ $# -gt 0 ]; do
  case "$1" in
    --signed) signed=1 ;;
    --variant) variant="$2"; shift ;;
  esac
  shift
done

app_name="Schedule Spotter.app"
bundle_id="com.tightline.schedulespotter"
apps="$HOME/Applications"
app="$apps/$app_name"
work="$(mktemp -d)"
checks="$work/checks.tsv"
: >"$checks"
failed=0

# check <name> <pass|fail|skip|info> <detail>
check() {
  printf '%s\t%s\t%s\n' "$1" "$2" "$3" >>"$checks"
  echo "[$2] $1: $3"
  if [ "$2" = fail ]; then failed=1; fi
}

# 1. Downloaded the way a browser does.
xattr -w com.apple.quarantine "0083;$(printf %x "$(date +%s)");Safari;$(uuidgen)" "$dmg"
check dmg-quarantined pass "$(xattr -p com.apple.quarantine "$dmg")"
check dmg-size info "$(stat -f %z "$dmg") bytes"

if [ $signed = 1 ]; then
  if codesign --verify --strict -v "$dmg" 2>"$work/dmg-sig"; then
    check dmg-signature pass "$(tr '\n' ' ' <"$work/dmg-sig")"
  else
    check dmg-signature fail "$(tr '\n' ' ' <"$work/dmg-sig")"
  fi
  if xcrun stapler validate "$dmg" >"$work/dmg-staple" 2>&1; then
    check dmg-stapled pass "$(tail -1 "$work/dmg-staple")"
  else
    check dmg-stapled fail "$(tr '\n' ' ' <"$work/dmg-staple")"
  fi
else
  check dmg-signature skip "unsigned build (no Developer ID secrets yet)"
fi

# 2. Mount and drag to Applications (ditto keeps extended attributes, as
#    the Finder's copy does).
mnt="$work/mnt"
mkdir -p "$mnt" "$apps"
hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "$dmg" >/dev/null
rm -rf "$app"
ditto "$mnt/$app_name" "$app"
hdiutil detach "$mnt" >/dev/null
if xattr -p com.apple.quarantine "$app" >/dev/null 2>&1; then
  check app-quarantined pass "the copied app carries the quarantine from the disk image"
else
  # Some macOS versions don't carry it onto the mounted image's files; a
  # browser download always ends up quarantined, so the test adds it.
  xattr -w com.apple.quarantine "0083;$(printf %x "$(date +%s)");Safari;$(uuidgen)" "$app"
  check app-quarantined info "not carried from the image; added by the test"
fi

# 3. What's inside.
check app-size info "$(du -sk "$app" | cut -f1) KB"
check universal "$(lipo -archs "$app/Contents/MacOS/"* | grep -q 'x86_64 arm64\|arm64 x86_64' && echo pass || echo fail)" \
  "$(lipo -archs "$app/Contents/MacOS/"*)"
if [ -f "$app/Contents/Resources/Licenses/README.txt" ]; then
  check licenses pass "Contents/Resources/Licenses"
else
  check licenses fail "no Licenses folder in the bundle"
fi
if [ -n "$variant" ] && [ "$variant" != off ]; then
  model="$(find "$app/Contents/Resources/model" -name '*.gguf' 2>/dev/null | head -1)"
  if [ -n "$model" ]; then
    check model-bundled pass "$(basename "$model"), $(stat -f %z "$model") bytes"
  else
    check model-bundled fail "no model in Contents/Resources/model"
  fi
fi

# 4. Signature, sandbox and Gatekeeper.
if codesign --verify --deep --strict -vv "$app" 2>"$work/sig"; then
  check codesign-verify pass "$(tail -1 "$work/sig")"
else
  check codesign-verify fail "$(tr '\n' ' ' <"$work/sig")"
fi
check signed-by info "$(codesign -dv --verbose=2 "$app" 2>&1 | grep -E '^(Authority|Signature|TeamIdentifier|Runtime Version)' | tr '\n' ';')"
ents="$(codesign -d --entitlements - --xml "$app" 2>/dev/null || true)"
if grep -q "com.apple.security.app-sandbox" <<<"$ents" && ! grep -q "network" <<<"$ents"; then
  check sandbox pass "App Sandbox on, no network entitlement"
else
  check sandbox fail "entitlements: $ents"
fi
if codesign -dv "$app" 2>&1 | grep -q "flags=.*runtime"; then
  check hardened-runtime pass "on"
else
  check hardened-runtime fail "off"
fi
verdict="$(spctl --assess --type execute -vv "$app" 2>&1 || true)"
if [ $signed = 1 ]; then
  if grep -q "accepted" <<<"$verdict" && grep -q "Notarized Developer ID" <<<"$verdict"; then
    check gatekeeper pass "$(tr '\n' ' ' <<<"$verdict")"
  else
    check gatekeeper fail "$(tr '\n' ' ' <<<"$verdict")"
  fi
  if xcrun stapler validate "$app" >"$work/staple" 2>&1; then
    check app-stapled pass "$(tail -1 "$work/staple")"
  else
    check app-stapled fail "$(tr '\n' ' ' <"$work/staple")"
  fi
else
  check gatekeeper skip "unsigned build; Gatekeeper says: $(tr '\n' ' ' <<<"$verdict")"
  xattr -dr com.apple.quarantine "$app"
fi

# 5. Self-test from the installed location, watching for sockets. The
#    sandbox keeps the app from writing outside its container, so the JSON
#    comes from stdout.
bin="$(find "$app/Contents/MacOS" -type f -perm -u+x | head -1)"
sockets="$work/sockets"
: >"$sockets"
"$bin" --self-test --linger 60 >"$work/stdout" 2>"$work/stderr" &
pid=$!
while kill -0 "$pid" 2>/dev/null; do
  lsof -nP -a -p "$pid" -i >>"$sockets" 2>/dev/null || true
  sleep 0.2
done
set +e
wait "$pid"
code=$?
set -e
grep '^{' "$work/stdout" | tail -1 >"$work/selftest.json" || true
if [ $code = 0 ]; then
  check self-test pass "exit 0"
else
  check self-test fail "exit $code; stderr: $(tail -5 "$work/stderr" | tr '\n' ' ')"
fi
if [ -s "$sockets" ]; then
  check no-sockets fail "$(sort -u "$sockets" | tr '\n' ' ')"
else
  check no-sockets pass "no socket opened during the self-test"
fi

# 6. Nothing left behind outside the app's sandbox container, and the
#    session scratch gone from it.
container="$HOME/Library/Containers/$bundle_id"
leftovers="$(find "$HOME/Library/Application Support" "$HOME/Library/Caches" "$HOME/Library/Preferences" \
  "$HOME/Library/WebKit" "$HOME/Library/HTTPStorages" "$HOME/Library/Saved Application State" \
  -maxdepth 1 -iname "*$bundle_id*" 2>/dev/null || true)"
if [ -z "$leftovers" ]; then
  check nothing-left pass "nothing under ~/Library outside the sandbox container"
else
  check nothing-left fail "$(tr '\n' ' ' <<<"$leftovers")"
fi
scratch="$(find "$container/Data/tmp" "${TMPDIR:-/tmp}" -maxdepth 1 -name 'schedule-spotter-session-*' 2>/dev/null || true)"
if [ -z "$scratch" ]; then
  check scratch-removed pass "session scratch deleted on exit"
else
  check scratch-removed fail "$(tr '\n' ' ' <<<"$scratch")"
fi
check container info "$(find "$container/Data" -type f 2>/dev/null | sed "s|$container/Data/||" | head -20 | tr '\n' ' ')"

python3 - "$checks" "$work/selftest.json" "$out" "$failed" <<'EOF'
import json, sys
checks_tsv, selftest, out, failed = sys.argv[1:]
checks = []
for line in open(checks_tsv, encoding="utf-8"):
    name, status, detail = line.rstrip("\n").split("\t", 2)
    checks.append({"check": name, "status": status, "detail": detail})
try:
    st = json.load(open(selftest, encoding="utf-8"))
except Exception:
    st = None
json.dump({"install_test": "fail" if failed == "1" else "pass", "checks": checks, "self_test": st},
          open(out, "w", encoding="utf-8"), indent=1)
print(json.dumps(st))
EOF
exit $failed
