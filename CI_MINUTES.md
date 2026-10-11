# CI minutes estimate (Phase 0)

The repo is **public**, and standard GitHub-hosted runners are free for public repositories, macOS included, so **the expected bill is $0**. The numbers below are for wall-clock and concurrency planning. They also show what the same matrix would cost in a private repo, at GitHub's per-minute prices as of 2026-10: Linux $0.006, Windows $0.010, macOS $0.062.

## Every push (`ci.yml`)

| Job | Runner | Est. minutes | macOS share |
|---|---|---|---|
| core tests, fixtures, generator seeds 0–1,999, depcheck | ubuntu-24.04 | 8 | |
| build universal disk image | macos-15 | 25 | 25 |
| build Windows installer | windows-2025 | 20 | |
| build Linux package | ubuntu-24.04 | 12 | |
| self-test ×3 | macos-15, macos-26, macos-15-intel | 9 | 9 |
| self-test ×2 | windows-2025, windows-11-arm | 8 | |
| self-test | ubuntu-24.04 | 2 | |
| **Total** | | **≈ 84 runner-min, ≈ 35 min wall clock** | **34** |

## Nightly model tier (`bench.yml`, from Phase 4)

There are 5 model files (A, B-E4B, B-E2B, plus Gemma QAT E2B, plus C; E4B QAT once). Each runs on 3 benchmark runners (macos-15, macos-15-intel, windows-2025) with threads capped at 2. Every run covers 460 benchmark questions plus 300 generator seeds plus fixtures, about 45 min each.

That comes to **≈ 675 runner-min per night**, of which ≈ 450 are macOS. The nightly schedule turns on only after Austin approves it (spec, CI cost). On a public repo it costs nothing.

## Monthly, at ~10 pushes per working day plus nightly benches

| | Public repo (this repo) | Same matrix in a private repo |
|---|---|---|
| Push CI | $0 | ≈ 220 pushes × (34 × $0.062 + 28 × $0.010 + 22 × $0.006) ≈ $555 |
| Nightly bench | $0 | ≈ 30 × (450 × $0.062 + 225 × $0.010) ≈ $905 |
| **Total** | **$0** | **≈ $1,460 / month** |

Limits that do apply to a public repo: 5 concurrent macOS jobs (Free plan) and 20 concurrent jobs overall, plus artifact storage. Artifacts are kept for 7 days.
