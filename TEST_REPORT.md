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

## Phase 2 — analysis (gate passed 2026-10-11, K07 deferred to Phase 3)

### Handmade fixtures, flags and info compared exactly

| Category | Pass |
|---|---|
| A | 10/10 |
| B | 10/10 |
| C | 19/19 |
| D | 14/14 |
| G Status and unstaffed | 8/8 |
| H Times | 8/8 |
| I | 9/9 |
| J Scale | 5/5 |
| K Flag logic | 11/12. K07 needs the stage 7 merge Mike B → Michael Baker (Phase 3). |
| M | 5/5, with 3 model-tier skipped |

Slowest: J05, 50,000 rows capped to the first four weeks, in 1.1 s. J04, 2,000 rows, in 73 ms. J03, 60 people × 25 jobs × 4 weeks, in 38 ms.

### Generator, seeds 0–1,999, model off

- **Clean schedules with a red flag: 0 of 392** (gate: zero).
- **Flag precision and recall** before stage 7, all types together: assigned_while_off 86/91%, double_booked 89/91%, unstaffed 100/64%.
- **not_booked precision is 7%.** Of its 40,700 extra flags, 40,300 are in the 1,151 cases whose names are spelled two ways (nicknames, initials). Each spelling is still a separate person who is idle on the other spelling's days. Stage 7 matching (Phase 3) is what fixes this.

## Phase 3 — matching and the ask-the-user path (gate passed 2026-10-11)

### Handmade fixtures

124/124 script tier, A–M. E (14) and F (10) are new, from an independent author. The three model-tier free-text pastes are left for Phase 4.

### Generated seeds 0–1,999, model off

| Measure | Value |
|---|---|
| False merges | **0** of 3,152 merges (gate: zero) |
| Clean schedules with a red flag | 0 of 392 |
| Questions per file (baseline) | 1.21 mean; 1,275 of 2,000 files ask nothing |

Flags, before any answers and after an owner answers every same-person/same-site question correctly:

| Flag | Recall before | Precision before | Recall after | Precision after |
|---|---|---|---|---|
| assigned_while_off | 92.5% | 94.5% | 95.9% | 96.7% |
| double_booked | 94.2% | 94.4% | 98.3% | 96.7% |
| not_booked | 84.1% | 15.2% | 84.5% | 67.6% |
| unstaffed | 100% | 78.8% | 100% | 87.3% |

Before answers, not_booked stays low because each spelling of a person who is asked about counts as idle on the other spelling's days. The gaps that remain after answers come from reading, not matching:
- crew lists in unusual shapes;
- calendar entries without separators;
- a few layouts misread.

Phase 4's model tier and the holdout will show whether those need more work.

## Phase 4: model layer and benchmark (in progress)

### Handmade fixtures

There are 136 fixtures across categories A–M. The spec's minimum is 135.

**New: L01–L09**, from an independent author:
- prompt injection in cells (script and model tier);
- a binary file renamed `.csv`;
- a 0-byte file;
- a file of only blank lines (L09, added for the spec's "empty and 0-byte files");
- invalid UTF-8;
- a 5 MB line;
- formula-looking cells;
- saving to a read-only target.

**Model off:** 132 of 132 script-tier fixtures pass; the 4 model-tier ones are skipped.

**Model on, model-tier fixtures (local, 2 threads):**

| Variant | Passing | Failing |
| --- | --- | --- |
| B-E2B | 4 / 4 | — |
| B-E2B-QAT | 3 / 4 | L03: answered "unsure" on a typo surrounded by injected instructions |
| A | 0 / 4 | L03: answered "different", following the injected instruction; the three free-text pastes were misread, or two of their lines timed out |

L01's formula cell exposed a reader rule the spec doesn't have: formula text in text files was dropped. It's now kept as data. `export_check` and `save_test` are checked from Phase 5, when export and save exist.

### Generator, model off (no regressions)

Seeds 0–1,999:
- 0 false merges in 3,152 merges;
- 0 clean schedules with a red flag;
- assignment recall 99.55%, precision 99.67%.

### Benchmark

There are 468 labeled questions in `bench/questions`, written by an independent author:

| Task | Questions |
| --- | --- |
| T1 | 151 |
| T2 | 100 |
| T3 | 101 |
| T4 | 54 |
| T5 | 62 |

- **Look-alikes:** 44% of the T1/T2 pairs are look-alike "different" pairs.
- **Prompt injection:** 21 questions.
- **No leaks:** `tools/check_prompt_leak.py` keeps prompt examples out of the set.

Results are in MODEL_REPORT.md. On the local machine no variant passes. Accuracy on answered questions is 53–77% against the 95% threshold, and false merges are 17–100% against the 1% limit.

## Changed expectations

| Fixture | Was | Now | Reason |
|---|---|---|---|
| E01-mike-michael-mikey | "Mikey Baker" asked as not in the nickname table | merged with a chip, like "Mike Baker" | The table includes standard diminutives (Mikey, Bobby, Jimmy …), and the spec's own row E groups "Mike / Michael / Mikey". The expectation came from reading the spec's parenthetical list as the whole table. |
| E04-last-name-only | lone first name "Ana" merged into Ana Ruiz | asked; "yes" gives the old result | On 2,000 generated schedules, merging unique lone first names caused 4 false merges (a different person with the same first name), which breaks the Phase 3 gate. Unique lone last names caused none and still merge. |

## Known failures

Listed in `fixtures/KNOWN_FAILING.txt`. They still run on every push.

| Fixture | Why | Fixed by |
|---|---|---|
| K07-double-booked-after-name-merge | The double-booking appears only once "Mike B" and "Michael Baker" are one person, which needs stage 7 matching (nickname + initial). | Phase 3 |
