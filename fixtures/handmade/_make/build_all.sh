#!/usr/bin/env bash
# Regenerate every handmade fixture in categories A-M, then
# check them. Output is byte-identical across runs (zip timestamps are fixed).
# C15-C19 come from build_formats.py, which needs extra writers (see its docstring).
#
#   fixtures/handmade/_make/build_all.sh
#
# build_<X>.py   writes fixtures/handmade/<ID>-<slug>/ (README.md, input, expected.json)
#                and _make/hints/<ID>.json (day axis, roster block, job rows, deterministic merges)
# check_flags.py recomputes flags and info from FORMAT.md's rules and compares
# sanity.py      checks each expected.json against the text of its own input file
set -euo pipefail
cd "$(dirname "$0")"
export PYTHONDONTWRITEBYTECODE=1
UV="uv run -q --with openpyxl==3.1.5 --with odfpy==1.4.1 --with python-docx==1.2.0 python3"
for c in A B C D E F G H I J K L M; do
  $UV "build_$c.py"
done
$UV build_C24.py
python3 check_flags.py
python3 sanity.py
