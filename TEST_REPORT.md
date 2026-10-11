# Test report

Pass/fail by fixture category, generator stats across seeds, holdout results (reported separately, Phase 7 only), every changed expectation with its reason, and known failures.

## Phase 0

| Check | Result |
|---|---|
| `cargo test -p spotter-core -p spotter-llm` | pass (1 test) |
| `tools/depcheck.py` | pass: 271 crates across 4 shipping targets, none network-capable |
| `--self-test` on Linux (Docker, Xvfb, release build) | pass: page ready 689 ms, peak RSS 67.8 MB, model off |
| `--self-test` on CI runners | pending: needs the GitHub repo |

## Changed expectations

None.

## Known failures

None.
