"""Same seed -> byte-identical files, in-process and across processes with different hash seeds."""

import os
import subprocess
import sys
from pathlib import Path

from spotter_gen.gen import generate
from spotter_gen.plan import draw_plan
import random


def test_same_seed_same_bytes_in_process():
    seen = set()
    for s in list(range(0, 120)) + [1_000_000, 1_000_001]:
        a, b = generate(s), generate(s)
        assert a.files == b.files, s
        seen.add(a.truth["render"]["format"])
    assert {"csv", "xlsx", "docx", "xls", "ods", "txt", "tsv"} <= seen


def _run(out: Path, hashseed: str) -> None:
    env = dict(os.environ, PYTHONHASHSEED=hashseed)
    subprocess.run([sys.executable, "-m", "spotter_gen.cli", "--seeds", "0-59", "--out", str(out), "--quiet"],
                   check=True, env=env)


def test_cli_bytes_identical_across_processes(tmp_path):
    a, b = tmp_path / "a", tmp_path / "b"
    _run(a, "0")
    _run(b, "4242")
    fa = sorted(p.relative_to(a) for p in a.rglob("*") if p.is_file())
    fb = sorted(p.relative_to(b) for p in b.rglob("*") if p.is_file())
    assert fa == fb and len(fa) == 180
    for rel in fa:
        assert (a / rel).read_bytes() == (b / rel).read_bytes(), rel


def test_holdout_category():
    assert generate(1_000_000).expected["category"] == "H"
    assert generate(5).expected["category"] == "C"


def test_format_shares_over_dev_seeds():
    from collections import Counter

    c = Counter(draw_plan(random.Random(s)).fmt for s in range(2000))
    assert c["xlsx"] <= 0.25 * 2000
    assert 0.38 * 2000 <= c["csv"] <= 0.52 * 2000
