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

## Changed expectations

None.

## Known failures

None.
