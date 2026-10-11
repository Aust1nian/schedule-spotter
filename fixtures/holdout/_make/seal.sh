#!/usr/bin/env bash
# Rewrite fixtures/holdout/SEALED.sha256: sha256sum of every file under
# fixtures/holdout except SEALED.sha256 itself, paths relative to
# fixtures/holdout, sorted by path (C locale). Verify later with:
#   (cd fixtures/holdout && sha256sum -c SEALED.sha256)
set -euo pipefail
cd "$(dirname "$0")/.."
find . -name __pycache__ -type d -prune -exec rm -rf {} +
find . -type f ! -name SEALED.sha256 | sed 's|^\./||' | LC_ALL=C sort | xargs -d '\n' sha256sum > SEALED.sha256
wc -l SEALED.sha256
