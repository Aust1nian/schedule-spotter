"""200 seeds (generation + writing to disk) must fit the 90 s / 2,000-seed budget pro rata."""

import time

from spotter_gen.cli import main


def test_200_seeds_time(tmp_path):
    t0 = time.perf_counter()
    main(["--seeds", "1000-1199", "--out", str(tmp_path), "--quiet"])
    dt = time.perf_counter() - t0
    assert dt < 9.0, f"{dt:.1f} s for 200 seeds"
