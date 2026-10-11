#!/usr/bin/env bash
# Installs the .deb, runs --self-test under Xvfb, and fails if the process
# opened any network socket while it ran. Linux is a development target only.
# Usage: selftest-linux.sh <path.deb> [out.json]
set -euo pipefail
deb="$1"
out="${2:-$PWD/selftest.json}"
sudo apt-get install -y --no-install-recommends "$(realpath "$deb")" >/dev/null
bin="$(dpkg -L schedule-spotter | grep -E '^/usr/bin/' | head -1)"
sockets="$(mktemp)"
export WEBKIT_DISABLE_DMABUF_RENDERER=1
xvfb-run -a "$bin" --self-test --linger 60 --json-out "$out" &
runner=$!
pid=""
for _ in $(seq 1 50); do
  pid="$(pgrep -n -f -- "$bin --self-test" || true)"
  [ -n "$pid" ] && break
  sleep 0.1
done
while [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; do
  lsof -nP -a -p "$pid" -i >>"$sockets" 2>/dev/null || true
  sleep 0.2
done
set +e
wait "$runner"
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
