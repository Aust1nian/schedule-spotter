#!/usr/bin/env bash
# Runs a command inside the Linux dev image with the repo mounted at /repo.
# Usage: tools/docker/run.sh <command...>
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd)"
exec docker run --rm -i \
  -u "$(id -u):$(id -g)" \
  -e HOME=/tmp/home -e CARGO_TARGET_DIR=/repo/target/linux-docker \
  -e CARGO_HOME=/repo/target/linux-docker-cargo-home \
  -v "$repo":/repo -w /repo \
  spotter-linux-dev bash -lc "mkdir -p \$HOME && export PATH=/opt/cargo/bin:\$PATH && $*"
