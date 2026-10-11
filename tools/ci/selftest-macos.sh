#!/usr/bin/env bash
# Installs the app from the disk image the way a user does (drag to
# Applications), runs --self-test, and fails if the process opened any
# network socket while it ran.
# Usage: selftest-macos.sh <path.dmg> [out.json]
set -euo pipefail
dmg="$1"
out="${2:-$PWD/selftest.json}"
apps="$HOME/Applications"
mnt="$(mktemp -d)"

hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "$dmg" >/dev/null
mkdir -p "$apps"
rm -rf "$apps/Schedule Spotter.app"
cp -R "$mnt/Schedule Spotter.app" "$apps/"
hdiutil detach "$mnt" >/dev/null

bin="$(find "$apps/Schedule Spotter.app/Contents/MacOS" -type f -perm -u+x | head -1)"
sockets="$(mktemp)"
"$bin" --self-test --linger 60 --json-out "$out" &
pid=$!
while kill -0 "$pid" 2>/dev/null; do
  lsof -nP -a -p "$pid" -i >>"$sockets" 2>/dev/null || true
  sleep 0.2
done
set +e
wait "$pid"
code=$?
set -e

cat "$out" || echo "no self-test output"
if [ -s "$sockets" ]; then
  echo "FAIL: the app opened network sockets:"
  sort -u "$sockets"
  exit 1
fi
echo "no sockets opened"
exit "$code"
