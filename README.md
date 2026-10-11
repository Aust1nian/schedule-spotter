# Schedule Spotter

A free, offline desktop app from Tight Line. It reads a contractor's own crew schedule and flags who's double-booked, who's not booked, and which jobs have nobody on them.

- Spec: [`Schedule Conflict Spotter — Build Spec.md`](Schedule%20Conflict%20Spotter%20%E2%80%94%20Build%20Spec.md)
- Choices and deviations: [`DECISIONS.md`](DECISIONS.md)
- Test results: [`TEST_REPORT.md`](TEST_REPORT.md)

| Folder | What's in it |
|---|---|
| `core/` | Reading, layout detection, normalization, matching, analysis, export. No UI or model code. |
| `llm/` | llama.cpp tiebreaker layer |
| `app/` | Tauri shell and TypeScript page |
| `tools/gen/` | Fixture generator (dev only) |
| `bench/` | Model benchmark harness |
| `fixtures/` | Handmade fixtures; `holdout/` stays sealed until Phase 7 |
| `models/` | Pinned manifest and fetch script; model files are gitignored |

## Develop

```sh
cargo test -p spotter-core -p spotter-llm   # core tests
python3 tools/depcheck.py                    # no network code in the shipped tree
(cd app && npm ci && npx tauri dev)          # run the app (needs Tauri's Linux packages on Linux)
tools/docker/run.sh "cd app && npx tauri build --no-bundle"   # Linux build without installing system packages
```
