"""Downloads the newest artifact of each name from a workflow run.

Usage: fetch_artifacts.py --run <run id> --out DIR [--repo owner/name] PREFIX...

Re-running failed jobs adds a second artifact with the same name (an
install test that passes on the re-run, for example), and a plain download
merges both, so either could win. This takes the newest of each name (by
creation time; artifact ids aren't in order) and unpacks it into DIR/<name>/.
Needs `gh`, logged in locally or with GH_TOKEN in CI.
"""
import argparse
import json
import pathlib
import subprocess
import tempfile
import zipfile


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--out", required=True, type=pathlib.Path)
    ap.add_argument("--repo", default="Aust1nian/schedule-spotter")
    ap.add_argument("prefixes", nargs="+")
    a = ap.parse_args()
    listing = subprocess.run(
        ["gh", "api", "--paginate", f"repos/{a.repo}/actions/runs/{a.run}/artifacts?per_page=100",
         "-q", ".artifacts[] | {id, name, created_at, expired}"],
        check=True, capture_output=True, text=True,
    ).stdout
    newest = {}
    for line in listing.splitlines():
        art = json.loads(line)
        if art["expired"] or not any(art["name"].startswith(p) for p in a.prefixes):
            continue
        if art["name"] not in newest or art["created_at"] > newest[art["name"]]["created_at"]:
            newest[art["name"]] = art
    a.out.mkdir(parents=True, exist_ok=True)
    for name, art in sorted(newest.items()):
        with tempfile.NamedTemporaryFile(suffix=".zip") as tmp:
            subprocess.run(["gh", "api", f"repos/{a.repo}/actions/artifacts/{art['id']}/zip"],
                           check=True, stdout=tmp)
            tmp.flush()
            with zipfile.ZipFile(tmp.name) as z:
                z.extractall(a.out / name)
        print(f"fetched {name} (artifact {art['id']}, {art['created_at']})")


if __name__ == "__main__":
    main()
