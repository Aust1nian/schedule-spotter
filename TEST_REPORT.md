# Test report

Pass/fail by fixture category, generator stats across seeds, holdout results (reported separately, Phase 7 only), every changed expectation with its reason, and known failures.

## Phase 0

| Check | Result |
|---|---|
| `cargo test -p spotter-core -p spotter-llm` | pass (1 test) |
| `tools/depcheck.py` | pass: 271 crates across 4 shipping targets, none network-capable |
| `--self-test` on Linux (Docker, Xvfb, release build) | pass: page ready 689 ms, peak RSS 67.8 MB, model off |
| `--self-test` on CI runners | pass on all six (run 38121971582, 2026-10-11), table below |

**Phase 0 gate: PASSED 2026-10-11.** The empty app opens a window and passes `--self-test` (model off) on every runner. The app was installed the way a client installs it: the macOS app copied from the disk image, Windows through the current-user NSIS installer, Linux from the .deb. The no-socket check passed everywhere.

| Runner | Result | Page ready | Peak RSS (main process) | Binary |
|---|---|---|---|---|
| macos-15 (gating; M1, 3 cores, 7 GB) | pass | 2,586 ms | 69.5 MB | universal, arm64 slice |
| macos-26 (M1, 3 cores, 7 GB) | pass | 3,795 ms | 92.9 MB | universal, arm64 slice |
| macos-15-intel | pass | 6,283 ms | 45.0 MB | universal, x86_64 slice |
| windows-2025 | pass | 3,024 ms | 29.9 MB | x64 |
| windows-11-arm (x64 under emulation; non-blocking) | pass | 3,671 ms | 47.2 MB | x64 |
| ubuntu-24.04 (dev only) | pass | 1,343 ms | 145.8 MB | x64 |

Peak RSS here is the app's own process. The webview runs in helper processes (WebKit's WebContent, msedgewebview2), which are measured separately by the Phase 4 benchmark harness.

## Phase 1 — reading and layout (gate passed 2026-10-11)

### Handmade fixtures, script tier, model off

| Category | Pass | Notes |
|---|---|---|
| A Encoding and delimiters | 10/10 | |
| B Headers | 10/10 | |
| C Layouts and file formats | 19/19 | C15–C19 cover .numbers, .xls, .xlsm and .xlsb |
| D Dates and days | 14/14 | |
| I Junk and noise | 9/9 | |
| M Paste input | 5/5 | M04, M05 and M07 are free text needing the model (T5), so they are skipped until Phase 4. M08 checks that free text with the model off asks for a table. |

### Generator, seeds 0–1,999, model off

Assignment recall **99.553%** (gate ≥ 99%), precision 99.465%. Perfect cases: 1,919 of 2,000. Per case: p95 14.7 ms. Generation: 16 s.

| Layout | Cases | Perfect | Records | Recall | Extra records |
|---|---|---|---|---|---|
| long_list | 670 | 615 | 114,246 | 99.97% | 813 |
| person_by_day | 393 | 374 | 72,202 | 99.93% | 26 |
| job_by_day | 343 | 338 | 62,526 | 99.97% | 19 |
| crew_based | 244 | 207 | 41,517 | 98.25% | 265 |
| calendar_month | 120 | 101 | 46,190 | 99.34% | 250 |
| day_by_person | 116 | 110 | 19,762 | 99.92% | 8 |
| day_by_job | 114 | 112 | 22,908 | 97.62% | 651 |

Weakest spots, to revisit as Phase 2 needs them:
- **Crew-based:** 37 cases are imperfect, from crew lists in unusual shapes.
- **Day × job:** two large cases misread.
- **Calendar months:** 19 cases imperfect, mostly from entry lines without a separator.

### Holdout

Written by an independent author and sealed (`fixtures/holdout/SEALED.sha256`, 104 files). CI verifies the seal on every push. It has not been run.

## Changed expectations

None.

## Known failures

None.
