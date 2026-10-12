#!/usr/bin/env bash
# The disk image a client downloads: the app beside a link to Applications,
# so installing is one drag. Made from the finished app (signed, notarized
# and stapled when the secrets exist) with hdiutil rather than `tauri bundle`,
# which would rebuild and re-sign the app. Moves the app into the image's
# staging folder: a multi-GB app has no room for another copy on a runner.
# Usage: make-dmg.sh <path/Schedule Spotter.app> <out.dmg>
set -euo pipefail
app="$1"
out="$2"
stage="$(mktemp -d)/Schedule Spotter"
mkdir -p "$stage"
mv "$app" "$stage/"
ln -s /Applications "$stage/Applications"
mkdir -p "$(dirname "$out")"
hdiutil create -volname "Schedule Spotter" -srcfolder "$stage" -format UDZO -ov "$out" >/dev/null
rm -rf "$(dirname "$stage")"
echo "make-dmg: $out ($(stat -f %z "$out") bytes)"
