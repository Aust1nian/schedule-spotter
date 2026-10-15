"""Lists this team's recent notarization submissions and their status, from
Apple's Notary REST API (no Mac needed). For diagnosing a release that waits
on Apple: which submissions are still "In Progress", accepted or invalid.

Needs APPLE_API_KEY_P8 (the .p8 text), APPLE_API_KEY (key ID) and
APPLE_API_ISSUER. Prints ids, file names, statuses and dates only.

Run: uv run --with pyjwt[crypto]==2.10.1 python3 tools/release/notary_status.py
"""
import json
import os
import time
import urllib.request

import jwt

now = int(time.time())
token = jwt.encode(
    {"iss": os.environ["APPLE_API_ISSUER"], "iat": now, "exp": now + 600, "aud": "appstoreconnect-v1"},
    os.environ["APPLE_API_KEY_P8"],
    algorithm="ES256",
    headers={"kid": os.environ["APPLE_API_KEY"], "typ": "JWT"},
)
req = urllib.request.Request(
    "https://appstoreconnect.apple.com/notary/v2/submissions",
    headers={"Authorization": f"Bearer {token}"},
)
with urllib.request.urlopen(req, timeout=60) as r:
    data = json.load(r)["data"]
print(f"{len(data)} submissions (newest first)")
for s in data:
    a = s["attributes"]
    print(f"{a['createdDate']}  {a['status']:<12} {a['name']:<28} {s['id']}")
