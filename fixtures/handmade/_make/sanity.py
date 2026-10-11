#!/usr/bin/env python3
"""Cross-check fixtures against their own input files.

- Every person, site and note in `assignments` appears literally in the
  decoded input (text files decoded by BOM / UTF-8 / Windows-1252; .xlsx,
  .ods and .docx by their XML text). Catches typos between the input and
  expected.json.
- Every flag's person / sites name a person / site present in the assignments.
- Model-tier paste fixtures: each record's person and site occur together on
  one pasted line (the spec's T5 guard).
- Text inputs: report encoding, BOM and line endings so they can be eyeballed.

Usage: python3 sanity.py
"""
import html
import json
import re
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def decode(path: Path):
    b = path.read_bytes()
    if path.suffix in (".xlsx", ".ods", ".docx"):
        with zipfile.ZipFile(path) as z:
            xml = " ".join(z.read(n).decode("utf-8") for n in z.namelist() if n.endswith(".xml"))
        # paragraph/cell boundaries become spaces; entity-decode the text
        return html.unescape(re.sub(r"<[^>]+>", " ", xml)), "zip-xml", ""
    if b.startswith(b"\xff\xfe"):
        t, enc = b[2:].decode("utf-16-le"), "utf-16-le+BOM"
    elif b.startswith(b"\xef\xbb\xbf"):
        t, enc = b[3:].decode("utf-8"), "utf-8+BOM"
    else:
        try:
            t, enc = b.decode("utf-8"), "utf-8"
        except UnicodeDecodeError:
            t, enc = b.decode("cp1252"), "cp1252"
    eols = []
    if "\r\n" in t:
        eols.append("CRLF")
    if re.search(r"\r(?!\n)", t):
        eols.append("CR")
    if re.search(r"(?<!\r)\n", t):
        eols.append("LF")
    return t, enc, "+".join(eols)


def main():
    bad = 0
    for folder in sorted(p for p in ROOT.iterdir() if p.is_dir() and re.match(r"^[A-N]\d\d-", p.name)):
        exp = json.loads((folder / "expected.json").read_text(encoding="utf-8"))
        text, enc, eol = decode(folder / exp["input"])
        hay = " ".join(text.replace('""', '"').split())  # undo CSV quote doubling, collapse whitespace
        errs = []
        blocks = [exp] + ([exp["after_answers"]] if "after_answers" in exp else [])
        for blk in blocks:
            assigns = blk.get("assignments", [])
            people = {a["person"] for a in assigns if "person" in a}
            sites = {a["site"] for a in assigns if "site" in a}
            for a in assigns:
                for k in ("person", "site", "note"):
                    if k in a and a[k] not in hay:
                        errs.append(f"{k} {a[k]!r} not found in {exp['input']}")
            for f in blk.get("flags", []):
                if "person" in f and f["person"] not in people and f["person"] not in hay:
                    errs.append(f"flag person {f['person']!r} neither in assignments nor in the input (roster)")
                for s in f.get("sites", []) + ([f["site"]] if "site" in f else []):
                    if s not in sites:
                        errs.append(f"flag site {s!r} not in assignments")
        if exp.get("tier") == "model" and exp.get("paste"):
            lines = text.splitlines()
            for a in exp.get("assignments", []):
                if not any(a["person"] in ln and a["site"] in ln for ln in lines):
                    errs.append(f"T5 guard: {a['person']!r} and {a['site']!r} never share a line")
        errs = sorted(set(errs))
        status = "FAIL" if errs else "ok  "
        bad += bool(errs)
        print(f"{status} {folder.name:45s} {exp['input']:10s} {enc:14s} {eol}")
        for e in errs:
            print("     ", e)
    print(f"\n{'all fixtures consistent with their inputs' if not bad else f'{bad} fixture(s) inconsistent'}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
