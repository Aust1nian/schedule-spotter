#!/usr/bin/env bash
# The one holdout run (spec: Phase 7). Every configuration in one session.
set -u
cd /repo
out=target/holdout/results
mkdir -p "$out"
chk=target/linux-docker/release/spotter-check
run() { # run <name> [model args...]
  local name=$1; shift
  { echo "== $name: fixtures/holdout"; $chk fixtures fixtures/holdout "$@" --json "$out/fixtures-$name.json"; echo "exit $?"
    echo "== $name: generated seeds 1000000-1000999"; $chk generated target/holdout/gen "$@" --min-recall 0 --summary "$out/gen-$name.summary.json" --report "$out/gen-$name.report.txt"; echo "exit $?"; } >"$out/$name.log" 2>&1
}
date -u +"start %FT%TZ" > "$out/when.txt"
run off &
run C --model models/Llama-3.2-1B-Instruct-Q4_K_M.gguf --variant C --threads 2 &
run A --model models/microsoft_Phi-4-mini-instruct-Q4_K_M.gguf --variant A --threads 2 &
run B-E2B --model models/gemma-4-E2B-it-Q4_K_M.gguf --variant B-E2B --threads 2 &
wait
date -u +"end %FT%TZ" >> "$out/when.txt"
