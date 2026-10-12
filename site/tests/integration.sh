#!/usr/bin/env bash
# The download site end to end: the real nginx config (TLS swapped for plain
# HTTP on port 8443) in front of the Python service, with release files on
# disk and mail written to a folder instead of sent.
# Usage: site/tests/integration.sh   (needs docker, python3, curl)
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'docker rm -f spotter-site-nginx >/dev/null 2>&1 || true; kill "${py:-0}" 2>/dev/null || true; rm -rf "$work"' EXIT

mkdir -p "$work/releases" "$work/data" "$work/mail"
head -c 3000000 /dev/urandom >"$work/releases/Schedule-Spotter-0.1.0-C-mac.dmg"
head -c 2000000 /dev/urandom >"$work/releases/Schedule-Spotter-0.1.0-C-windows-UNSIGNED-setup.exe"
python3 - "$work/releases" <<'PY'
import hashlib, json, pathlib, sys
d = pathlib.Path(sys.argv[1])
files = []
for f in sorted(d.iterdir()):
    files.append({"file": f.name, "platform": "mac" if f.suffix == ".dmg" else "windows",
                  "bytes": f.stat().st_size, "sha256": hashlib.sha256(f.read_bytes()).hexdigest(), "signed": False})
(d / "current.json").write_text(json.dumps({"version": "0.1.0", "files": files}))
PY

# The production config, minus TLS and the port-80 redirect.
python3 - "$here/deploy/nginx.conf" "$work/site.conf" "$work" "$here" <<'PY'
import re, sys
src, out, work, here = sys.argv[1:]
t = open(src).read()
t = t[t.index("server {", t.index("server {") + 1):]                # drop the redirect server
t = re.sub(r"listen \[::\]:443 ssl http2;\n", "", t)
t = t.replace("listen 443 ssl http2;", "listen 8443;")
t = re.sub(r"\s*ssl_certificate(_key)? [^;]+;", "", t)
t = t.replace("/srv/schedule-spotter/releases/", "/srv/releases/")
t = t.replace("/opt/schedule-spotter-site/static/", "/srv/static/")
t = t.replace("include /etc/nginx/snippets/schedule-spotter-proxy.conf;", open(here + "/deploy/schedule-spotter-proxy.conf").read().split("\n", 1)[1])
head = open(src).read()
zone = re.search(r"limit_req_zone[^;]+;", head).group(0)
open(out, "w").write(zone + "\n" + t)
PY

SECRET="$(python3 -c 'import secrets; print(secrets.token_urlsafe(48))')" \
SITE_URL="http://127.0.0.1:8443" DATA_DIR="$work/data" RELEASES_DIR="$work/releases" MAIL_DIR="$work/mail" \
  python3 "$here/server/spotter_site.py" &
py=$!
docker run -d --name spotter-site-nginx --network host \
  -v "$work/site.conf:/etc/nginx/conf.d/default.conf:ro" \
  -v "$work/releases:/srv/releases:ro" -v "$here/static:/srv/static:ro" \
  nginx:1.22-alpine >/dev/null
for _ in $(seq 50); do curl -fs http://127.0.0.1:8443/healthz >/dev/null 2>&1 && break; sleep 0.2; done

fail=0
expect() { # expect <name> <want> <got>
  if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: want $2, got $3"; fail=1; fi
}
code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }
mac=Schedule-Spotter-0.1.0-C-mac.dmg

expect "landing page" 200 "$(code http://127.0.0.1:8443/)"
expect "stylesheet from nginx" 200 "$(code http://127.0.0.1:8443/static/style.css)"
expect "request a link" 200 "$(code -d 'email=owner@acme-roofing.com&website=' http://127.0.0.1:8443/request)"
link="$(grep -ho 'http://127.0.0.1:8443/get/[A-Za-z0-9._-]*' "$work"/mail/*.eml | head -1)"
token="${link##*/get/}"
expect "link emailed" yes "$([ -n "$token" ] && echo yes || echo no)"
expect "download page" 200 "$(code -A 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)' "$link")"
curl -s -o "$work/got.dmg" -D "$work/headers" "http://127.0.0.1:8443/files/$token/$mac"
expect "file downloads whole" "$(sha256sum <"$work/releases/$mac")" "$(sha256sum <"$work/got.dmg")"
expect "served as an attachment" yes "$(grep -qi '^content-disposition: attachment' "$work/headers" && echo yes || echo no)"
expect "resume (Range)" 206 "$(code -H 'Range: bytes=1000-' "http://127.0.0.1:8443/files/$token/$mac")"
expect "other files refused" 403 "$(code "http://127.0.0.1:8443/files/$token/current.json")"
expect "bad token refused" 403 "$(code "http://127.0.0.1:8443/files/x.1.y/$mac")"
expect "path tricks refused" 404 "$(code --path-as-is "http://127.0.0.1:8443/files/$token/../current.json")"
for _ in 1 2 3 4; do curl -s -o /dev/null "http://127.0.0.1:8443/files/$token/$mac"; done
expect "link used up after 5" 403 "$(code "http://127.0.0.1:8443/files/$token/$mac")"
for _ in $(seq 8); do curl -s -o /dev/null -d 'email=x@acme-roofing.com' http://127.0.0.1:8443/request; done
expect "form rate limit (nginx)" 429 "$(code -d 'email=y@acme-roofing.com' http://127.0.0.1:8443/request)"
exit $fail
