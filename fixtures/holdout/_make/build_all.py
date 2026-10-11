"""Build every holdout fixture. Run from anywhere:

    uv run --with openpyxl --with xlwt --with odfpy --with python-docx \
        python3 fixtures/holdout/_make/build_all.py [H01 H02 ...]

Then: python3 fixtures/holdout/_make/check_flags.py
"""
import importlib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import write_fixture  # noqa: E402


def main(ids):
    for n in range(1, 21):
        fid = f"H{n:02d}"
        if ids and fid not in ids:
            continue
        try:
            mod = importlib.import_module(f"h{n:02d}")
        except ModuleNotFoundError:
            print(f"{fid}: no module yet")
            continue
        readme, input_name, expected, meta = mod.build()
        d = write_fixture(mod.FID, mod.SLUG, readme, input_name, expected, meta)
        print(f"{fid}: wrote {os.path.basename(d)} ({len(expected['assignments'])} assignments)")


if __name__ == "__main__":
    main(set(sys.argv[1:]))
