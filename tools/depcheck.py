#!/usr/bin/env python3
"""Fails if anything that could talk to the network enters the shipping app.

Checks, for every shipping target:
  1. No crate from DENY appears in the app's normal (shipped) dependency tree.
  2. Every crate in that tree is listed in tools/deps-allowlist.txt, so a new
     dependency can't slip in without someone reading what it is.
  3. The page's runtime npm dependencies are only @tauri-apps/api.
  4. If app/dist exists, the built page contains no URL outside ALLOWED_URL_HOSTS
     and none of the browser networking APIs in DENY_JS.

Usage: python3 tools/depcheck.py [--update-allowlist]
"""

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ALLOWLIST = ROOT / "tools" / "deps-allowlist.txt"
SHIPPING_TARGETS = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
]

# HTTP clients, TLS stacks, socket layers, telemetry, and Tauri plugins that
# open URLs or reach the network. Adding one requires a DECISIONS.md entry and
# a change here, never just an allowlist line.
DENY = {
    "reqwest", "hyper", "hyper-util", "h2", "h3", "quinn", "ureq", "curl", "curl-sys",
    "isahc", "surf", "attohttpc", "minreq", "tungstenite", "tokio-tungstenite",
    "native-tls", "openssl", "openssl-sys", "rustls", "tokio-rustls", "hf-hub",
    "mio", "socket2", "sentry", "opentelemetry", "tauri-plugin-http",
    "tauri-plugin-updater", "tauri-plugin-opener", "tauri-plugin-shell",
    "tauri-plugin-websocket", "tauri-plugin-upload", "tauri-plugin-deep-link",
}

ALLOWED_NPM_RUNTIME = {"@tauri-apps/api"}
# In-process schemes Tauri uses for IPC and assets, and XML namespace strings.
ALLOWED_URL_HOSTS = {"ipc.localhost", "tauri.localhost", "asset.localhost", "www.w3.org"}
DENY_JS = ["new WebSocket", "EventSource(", "sendBeacon", "XMLHttpRequest"]


def shipped_crates(target: str) -> set[str]:
    out = subprocess.run(
        ["cargo", "tree", "-p", "schedule-spotter", "--target", target, "-e", "normal,no-proc-macro",
         "--prefix", "none", "--format", "{p}"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    return {line.split()[0] for line in out.splitlines() if line.strip()}


def main() -> int:
    update = "--update-allowlist" in sys.argv
    problems: list[str] = []

    crates: set[str] = set()
    for target in SHIPPING_TARGETS:
        found = shipped_crates(target)
        for name in sorted(found & DENY):
            problems.append(f"{target}: network-capable crate in the shipped tree: {name}")
        crates |= found

    allowed = {
        line.strip() for line in ALLOWLIST.read_text().splitlines()
        if line.strip() and not line.startswith("#")
    } if ALLOWLIST.exists() else set()
    if update:
        header = "# Crates allowed in the shipped app (all shipping targets). Review before adding.\n"
        ALLOWLIST.write_text(header + "\n".join(sorted(crates - DENY)) + "\n")
        print(f"allowlist updated: {len(crates - DENY)} crates")
    else:
        for name in sorted(crates - allowed - DENY):
            problems.append(f"crate not on tools/deps-allowlist.txt: {name}")

    package = json.loads((ROOT / "app" / "package.json").read_text())
    for name in sorted(set(package.get("dependencies", {})) - ALLOWED_NPM_RUNTIME):
        problems.append(f"npm runtime dependency not allowed: {name}")

    dist = ROOT / "app" / "dist"
    if dist.exists():
        for path in sorted(dist.rglob("*")):
            if path.suffix not in {".js", ".html", ".css"}:
                continue
            text = path.read_text(errors="replace")
            for host in re.findall(r"(?:https?|wss?)://([A-Za-z0-9.\-]+)", text):
                if host not in ALLOWED_URL_HOSTS:
                    problems.append(f"{path.relative_to(ROOT)}: URL to {host}")
            for needle in DENY_JS:
                if needle in text:
                    problems.append(f"{path.relative_to(ROOT)}: uses {needle}")

    for problem in problems:
        print(f"depcheck: {problem}")
    if not problems:
        print(f"depcheck: ok ({len(crates)} crates across {len(SHIPPING_TARGETS)} targets)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
