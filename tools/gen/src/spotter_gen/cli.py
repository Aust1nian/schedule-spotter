"""spotter-gen CLI: write DIR/<seed>/{input.<ext>|paste.txt, expected.json, truth.json}."""

from __future__ import annotations

import argparse
import shutil
import sys
import time
from collections import Counter
from pathlib import Path

from .gen import VARIANTS, generate


def parse_seeds(spec: str) -> list[int]:
    out: list[int] = []
    for part in spec.split(","):
        part = part.strip()
        if not part:
            continue
        if "-" in part:
            a, b = part.split("-", 1)
            out += list(range(int(a), int(b) + 1))
        else:
            out.append(int(part))
    return out


def write_case(case, out: Path) -> None:
    d = out / str(case.seed)
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    for name, data in case.files.items():
        (d / name).write_bytes(data)


def summarize(cases_meta: list[dict]) -> str:
    n = len(cases_meta)
    lines = [f"seeds: {n}"]

    def table(title: str, key) -> None:
        c = Counter()
        for m in cases_meta:
            v = key(m)
            for x in (v if isinstance(v, list) else [v]):
                c[x] += 1
        lines.append(f"\n{title}")
        for k, v in sorted(c.items(), key=lambda kv: (-kv[1], str(kv[0]))):
            lines.append(f"  {str(k):28s} {v:6d}  {100 * v / n:5.1f}%")

    table("layout", lambda m: m["render"]["layout"])
    table("arrangement", lambda m: m["render"]["arrangement"])
    table("format", lambda m: m["render"]["format"])
    table("encoding (text formats)", lambda m: m["render"]["encoding"] or "-")
    table("delimiter (text formats)", lambda m: m["render"]["delimiter"] or "-")
    table("date format", lambda m: m["render"]["date_format"])
    table("tier", lambda m: m["tier"])
    table("clean", lambda m: "clean" if m["clean"] else "planted")
    table("planted (cases with >=1)", lambda m: sorted({p["type"] for p in m["planted"]}) or ["none"])
    table("info planted (cases with >=1)", lambda m: sorted({p["type"] for p in m["planted_info"]}) or ["none"])
    table("corruptions (cases with >=1)", lambda m: sorted(
        {c["type"] if c["type"] not in ("alias", "site_alias") else f'{c["type"]}:{c["form"]}' for c in m["corruptions"]}) or ["none"])
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="spotter-gen", description=__doc__)
    ap.add_argument("--seeds", required=True, help="e.g. 0-1999 or 0-9,100,200-210")
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--variant", default="none", choices=VARIANTS)
    ap.add_argument("--clean-rate", type=float, default=0.2)
    ap.add_argument("--stats", action="store_true", help="print the distribution of layouts, formats, ...")
    ap.add_argument("--quiet", action="store_true")
    a = ap.parse_args(argv)
    seeds = parse_seeds(a.seeds)
    a.out.mkdir(parents=True, exist_ok=True)
    t0 = time.perf_counter()
    meta = []
    for s in seeds:
        case = generate(s, a.variant, a.clean_rate)
        write_case(case, a.out)
        if a.stats:
            meta.append(case.truth)
    dt_s = time.perf_counter() - t0
    if not a.quiet:
        print(f"wrote {len(seeds)} cases to {a.out} in {dt_s:.1f} s ({1000 * dt_s / max(1, len(seeds)):.1f} ms/case)",
              file=sys.stderr)
    if a.stats:
        print(summarize(meta))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
