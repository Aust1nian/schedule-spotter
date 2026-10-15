# Test report

Pass/fail by fixture category, generator stats across seeds, holdout results (reported separately, Phase 7 only), every changed expectation with its reason, and known failures.

## Summary at the end of Phase 7 (2026-10-15)

### Handmade fixtures (`fixtures/handmade`, 151)

Results with the model off; the model-tier fixtures are reported in MODEL_REPORT.md.

| Category | What it covers | Pass |
| --- | --- | --- |
| A | Encoding and delimiters | 10/10 |
| B | Headers | 10/10 |
| C | Layouts (C20–C26 added in Phase 7) | 26/26 |
| D | Dates and days | 14/14 |
| E | People and names (E15–E16 added in Phase 7) | 16/16 |
| F | Jobs and sites | 10/10 |
| G | Status and unstaffed | 8/8 |
| H | Times | 8/8 |
| I | Junk and noise | 9/9 |
| J | Scale | 5/5 |
| K | Flag logic | 12/12 |
| L | Adversarial and safety | 8/8, plus 1 model-tier |
| M | Paste input | 5/5, plus 3 model-tier |
| N | Save, reopen, UI | 6/6 (also through the window: UI tests on Windows and Linux) |
| **All** | | **147/147 with the model off**, plus 4 model-tier |

### Generator, seeds 0–1,999 (model off)

| Measure | Result |
| --- | --- |
| Assignment recall | 99.555% (gate: at least 99%) |
| Precision | 99.681% |
| Schedules exactly right | 1,921 of 2,000 |
| Merges | 3,152, **none false** |
| Questions per file | 1.21 on average; 1,276 of 2,000 files ask nothing |
| Clean schedules given a red flag | 0 |

### Holdout (reported separately, below)

- **Blind, run once:** 13 of 20 handmade fixtures pass. The generated seeds reach 99.667% recall with 0 false merges, model off.
- **After the Phase 7 fixes (not blind):** 20 of 20 in every configuration.

### Known failures

None in the fixture library (`fixtures/KNOWN_FAILING.txt` is empty). The known limits are listed under "Known failures" at the end of this report.

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

Written by an independent author and sealed (`fixtures/holdout/SEALED.sha256`, 104 files). CI verifies the seal on every push. It was run once, in Phase 7; the results are under "Phase 7: holdout".

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

## Phase 4: model layer and benchmark (gate passed 2026-10-11)

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

Results are in MODEL_REPORT.md, from CI benchmark run 38184895853.

**Model-tier fixtures in CI** (`macos-15`, CPU, 2 threads): E2B-QAT 3/4, E2B and E4B-QAT 2/4, E4B 1/4, A and C 0/4. The free-text pastes that pass on the development machine come back with no rows on the runner. This fits T5 calls hitting the spec's 5 s limit; it is logged as a Phase 7 follow-up in DECISIONS.md.

**No variant passes on any runner.** On the gating runner, accuracy on answered questions is 48–79% against the 95% threshold. False merges run from 0.9% to 15.3% against the 1% limit; the 0.9% is C, which leaves over half the questions to the owner. The selection rule gives **model off**; Austin makes the final pick.

### Gate

| Gate item | Result |
| --- | --- |
| All three variants run on every runner | Pass. All six builds of A, B and C ran the benchmark on `macos-15`, `macos-15-intel`, `windows-2025`, `macos-26` and `windows-11-arm` (run 38184895853). On the CI Macs' paravirtual GPU, forced Metal can't fit Gemma E4B, but the app doesn't use Metal there. |
| MODEL_REPORT.md draft has every metric | Pass. Per variant and runner: load time, peak RSS, latency p50/p95, accuracy, false-merge rate and unsure rate. It also has questions per file against model off (1.61), and the recommendation under the selection rule. |

## Phase 5: app (gate passed 2026-10-11)

### Handmade fixtures

There are 142 fixtures; N01–N06 are new. Each category N fixture lists the owner's `steps` (fixtures/FORMAT.md). `spotter-check` runs them through the app's session, and the UI tests run them through the window.

| Fixture | What it checks |
| --- | --- |
| N01 | Save, then reopen: every answer (a merged job, an undone person chip) comes back, and nothing is asked again |
| N02 | Undo on a "St" / "Street" chip splits the job; it is then unstaffed on Tuesday |
| N03 | Helpers written in a Notes column: set to Person in What I read, which shows a double-booking and four empty days |
| N04 | All seven days: blank Saturdays are "not booked"; Mon–Fri clears them; a day off for everyone clears Wednesday |
| N05 | Saving to `Mis Documentos/Revisión de horario – José/…` and reopening |
| N06 | Answering a question and marking a day off with the keyboard only |

**Model off:** 138 of 138 script-tier fixtures pass, including all of category N.

**Now checked as well:**
- L01 `export_check`: no formula cell in the workbook, and a guard on every risky CSV field. Removing the CSV guard makes L01 fail, so the check works.
- L08 `save_test`: a read-only folder gives `save_failed` with a message, leaves no file and changes nothing.

### Generator, seeds 0–1,999, model off

Unchanged after this phase's matching fix: 0 false merges in 3,152 merges, assignment recall 99.55%, precision 99.67%.

### Self-test

The page must show the core's findings, questions and week-grid rows for the example. In the Linux dev container: pass, 5 findings, 1 question, 7 rows, peak RSS 89 MB, page ready in 0.7 s.

### UI tests (`tools/ui/ui_tests.py`)

They cover the example, editing the table, the Paste box, and N01–N06 (N06 by keyboard alone).

| Where | Result |
| --- | --- |
| Linux dev container (WebKitGTK through `tauri-driver`) | 9 of 9 pass |
| CI `ubuntu-24.04` (run 38209824504) | 9 of 9 pass |
| CI `windows-2025` (WebView2 153.0.4234.48, Edge WebDriver attached; run 38209824504) | 9 of 9 pass |

### Self-test in CI (run 38209824504)

Every runner shows the core's 5 findings, 1 question and 7 grid rows for the example.

| Runner | Peak RSS | Page ready |
| --- | --- | --- |
| macos-15 (installed from the disk image) | 87.4 MB | 3.2 s |
| macos-26 | 93.6 MB | 4.7 s |
| macos-15-intel | 52.2 MB | 2.8 s |
| windows-2025 (installed) | 54.1 MB | 4.0 s |
| windows-11-arm (x64 under emulation) | 34.9 MB | 3.2 s |
| ubuntu-24.04 | 156.4 MB | 0.9 s |

No runner opened a network socket.

### Gate

| Gate item | Result |
| --- | --- |
| UI tests green on Windows and Linux | Pass: 9 of 9 on `windows-2025` and on `ubuntu-24.04` |
| `--self-test` green on the real macOS bundle | Pass on `macos-15`, `macos-26` and `macos-15-intel`, installed from the universal disk image |

**Not covered by automation:**
- The system open and save dialogs, and dragging a file onto the window. WebDriver can't drive either, so they get a manual check on each platform before release (Phase 6, RELEASE.md).

## Phase 6: packaging, signing, download

### Install tests on every push (model-off build, CI run 38232923185)

Each runner installs the disk image or installer the way a client does (`tools/ci/install-test-*`; DECISIONS.md, Phase 6). All checks pass. Signature and Gatekeeper checks are reported as skipped until signing secrets exist; on Windows, signing is off by Austin's decision.

| Runner | What was checked | Result |
| --- | --- | --- |
| `macos-15`, `macos-26`, `macos-15-intel` | Quarantined disk image (16 MB). The app carries the quarantine once copied. Universal binary, Licenses folder, `codesign --verify --deep --strict`, App Sandbox with no listening, hardened runtime. Self-test from Applications, with no socket opened. Nothing in `~/Library` outside the sandbox container. Scratch deleted on exit. | pass |
| `windows-2025`, `windows-11-arm` | Installer (223 MB) with a Mark of the Web, manifest `asInvoker`. Silent install as a separate standard user, not elevated (about 21 s). Per-user folder, uninstall entry, Start menu shortcut and Licenses folder present. Self-test from the install folder with no socket opened. Scratch removed after exit. Silent uninstall as that user leaves nothing of the app's in the profile or registry. | pass |

Problems the install tests found and fixed:
1. With the App Sandbox and no network entitlement, the window stayed blank on all three Macs: WebKit needs `network.client`.
2. The Windows install hung on the "publisher could not be verified" shell prompt for a file with a Mark of the Web.
3. WebView2 held the session folder past the app's exit.
4. A silent uninstall left Tauri's registry key behind.

DECISIONS.md, Phase 6 has the fix for each.

### Self-test with the model

`--self-test` now loads the bundled model and asks it 20 tiebreaker questions. Local Linux check, variant C: the model loaded in 0.5 s, all 20 questions were asked, p95 was 0.63 s and peak RSS 1.13 GB, and the self-test passed. With the model file removed, the self-test fails with "model did not load: the model file is missing". The release run's results for each variant on each runner are below.

### Release candidate: every variant on every runner (release run 38236036646)

`release.yml` built each variant as a universal macOS disk image and a Windows installer, all UNSIGNED because no signing secrets exist yet. It then ran the install test for each file on each runner. Each self-test ran from the installed app, with its bundled model.

| Variant | Mac disk image | Windows installer | Mac build time |
| --- | --- | --- | --- |
| off | 0.016 GB | 0.223 GB | 10 min |
| C (Llama 3.2 1B) | 0.823 GB | 1.031 GB | 12 min |
| A (Phi-4-mini) | 2.507 GB | 2.715 GB | 13 min |
| B-E2B (Gemma 4 E2B) | 3.430 GB | 3.651 GB | 18 min |

Install test and self-test, with peak RSS, model load time (including the SHA-256 check), latency p95, and answers out of 20:

| Variant | macos-15 | macos-26 | macos-15-intel | windows-2025 | windows-11-arm (not blocking) |
| --- | --- | --- | --- | --- | --- |
| off | pass, 88 MB | pass, 96 MB | pass, 52 MB | pass, 34 MB | pass, 57 MB |
| C | pass, 1.08 GB, 1.8 s, 2.6 s, 7 | pass, 1.10 GB, 2.5 s, 2.5 s, 7 | pass, 1.12 GB, 11.1 s, 5.2 s, 7 | pass, 1.07 GB, 1.5 s, 2.3 s, 7 | pass, 0.96 GB, 3.4 s, 5.2 s, 7 |
| A | pass, 3.00 GB, 4.1 s, 5.4 s, 13 | pass, 3.05 GB, 5.3 s, 5.3 s, 11 | pass, 3.23 GB, 15.4 s, 12.7 s, 6 | pass, 3.03 GB, 3.1 s, 5.1 s, 12 | **fail**: model still loading after 240 s |
| B-E2B | pass, 2.95 GB, 11.7 s, 5.1 s, 17 | pass, 2.72 GB, 10.1 s, 5.0 s, 17 | pass, 3.65 GB, 21.1 s, 6.7 s, 9 | pass, 1.75 GB, 4.4 s, 5.1 s, 16 | **fail**: model still loading after 240 s |

Every file passed every other check on every runner:
- **Both platforms:** the model is in place and its checksum verified; no socket opens; nothing is left behind.
- **Mac:** quarantine, signature verification, the sandbox and the hardened runtime.
- **Windows:** Mark of the Web, a silent install as a standard user, and a clean silent uninstall.

The two `windows-11-arm` failures are the known Windows-on-Arm limit (DECISIONS.md, Phase 6). That runner is logged, not blocking (spec).

Latency p95 sits near 5 s because each call stops at the 5 s limit and becomes an owner question. The benchmark judges latency and accuracy (MODEL_REPORT.md); the self-test only proves that the installed model loads and answers.

### Gate

| Gate item | Result |
| --- | --- |
| Install tests pass on every runner (unsigned allowed until secrets exist) | **Pass.** All four variants pass on `macos-15`, `macos-26`, `macos-15-intel` and `windows-2025`. On the non-blocking `windows-11-arm`, off and C pass, and A and B-E2B time out loading the model (known limit, logged). |

**Not yet covered:**
- **Signed builds.** The Developer ID signing, notarization and stapling steps, and the install test's `--signed` checks, run once Austin adds the Apple secrets. Notarization of the 2.5–3.4 GB bundles gets timed on that first run.
- **Manual checks of each release candidate:** first-launch prompts, the system dialogs, drag-and-drop, and a real download over office internet (RELEASE.md).

### Download site

The email-gated download service and its tests were removed on 2026-10-14, when Austin dropped the email gate (DECISIONS.md, Phase 6). Before removal, its 15 unit tests and the 13-check nginx end-to-end test all passed. Downloads now come from the Tight Line website. `site/publish.sh` refuses a Mac build that isn't signed: in a dry run against the unsigned release run 38236036646, it stopped before downloading anything.

### Other unit tests added

- `payload.rs`: extracting the model from the installer, finding it before a signature, refusing a damaged model, and an installer with no model. 4 tests, all passing.
- `scratch.rs`: removing only our own session folders. 2 tests, all passing.
- `export.rs`: saving when only the picked file is writable, as under the macOS sandbox. Passes.

## Phase 7: holdout (run once, 2026-10-15)

**This is the blind result and is not to be changed.** The holdout was sealed in Phase 1 (commit af6014f). Its seal was verified, and it was run exactly once:
- **Code:** commit 934878c.
- **Run:** 04:56–05:56 UTC on Tight Line's workstation, in the Linux dev container.
- **Configurations:** model off, C, A and B-E2B, each with 2 llama threads and the 5 s call limit, as on the CI runners.
- **Raw outputs, script and commit:** `bench/results/holdout-2026-10-15/`.

Any later run against the holdout is labeled "after fixes, not blind".

### 20 handmade holdout fixtures

**13 of 20 pass, identically in all four configurations.** So the failures are in the rule-based reading, not the models.

| Fixture | Result | What went wrong |
| --- | --- | --- |
| H01, H02, H04, H06–H09, H13–H16, H18, H19 | pass | |
| H03 remodeler, job grid, twin addresses | fail | Four of one worker's days are missing ("Dom"), so a not-booked flag is missed |
| H05 fence, crew grid with a roster block | fail | A crew-based layout was read as person-by-day; most assignments are lost (95 missing) |
| H10 mason, calendar month | fail | A free-text note line became a person, adding a false assignment and 22 false not-booked flags |
| H11 homebuilder, materials sheet then job grid | fail | A note line became a job, with 5 false unstaffed flags; the app also asked which sheet to read when it shouldn't |
| H12 exteriors, two weeks side by side | fail | Rejected as "not a schedule" |
| H17 electrician, day by person | fail | One worker's days are missing ("Mo"), so a not-booked flag and an unstaffed flag are missed |
| H20 HVAC, list with start and end, roster block | fail | The roster block's assignments were copied onto an extra day (10 false assignments) |

### Generated holdout seeds 1,000,000–1,000,999

| Configuration | Assignment recall | Precision | Perfect schedules | Questions per file | Files with no question | Merges | False merges | Clean files given a red flag |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Model off | 99.667% | 99.866% | 960 | 1.39 | 598 | 1,646 | **0** | 0 |
| C (Llama 3.2 1B) | 99.667% | 99.866% | 960 | 1.37 | 600 | 1,649 | **0** | 0 |
| A (Phi-4-mini) | 99.691% | 99.866% | 961 | 1.16 | 669 | 1,704 | **6** | 2 |
| B-E2B (Gemma 4 E2B) | 99.667% | 99.866% | 960 | 0.29 | 845 | 2,465 | **96** | 12 |

**Reading:** the generated holdout matches the development seeds (99.55% recall at the Phase 5 gate), so reading isn't tuned to its own generator seeds.

**The handmade holdout shows layouts the development fixtures never covered.** They're failures of the "never wrong silently" bar:
- A crew roster laid out beside the grid.
- Two weeks side by side.
- A month calendar.
- Free-text notes inside a grid.
- A one- or two-letter first name that is also a day abbreviation.

**Models on the holdout:** the same picture as the benchmark (MODEL_REPORT.md).
- **B-E2B** cuts owner questions by 79%, but makes 96 false merges, such as "Alfredo" with "Wilfredo" and "Jeff W." with "Jeffrey R.". It also puts 12 false red flags on schedules that had no problems.
- **A** asks 17% fewer questions, with 6 false merges.
- **C** changes almost nothing.
- **Model off** makes no false merges.

**Latency:** this ran on the workstation's faster CPU, so latency (p95 5.5–17.7 s per file, including timed-out calls) isn't comparable with the runners. MODEL_REPORT.md has the runner figures.

### After the Phase 7 fixes (not blind)

The holdout was checked again on 2026-10-15 (06:49–08:10 UTC, commit 4011a73), after seven fixes to rule-based reading. Each fix came from a new development fixture written fresh, not from the holdout files (DECISIONS.md, Phase 7). **This is not a blind result.** The holdout's failures pointed at the problems, and it shows only that the fixes carry over to the original files. Raw outputs are in `bench/results/holdout-2026-10-15-after-fixes/`.

| Configuration | Handmade | Recall | Precision | Questions per file | False merges | Clean files given a red flag |
| --- | --- | --- | --- | --- | --- | --- |
| Model off | **20/20** (was 13/20) | 99.667% | 99.869% (was 99.866%) | 1.39 | 0 | 0 |
| C | **20/20** | 99.667% | 99.869% | 1.37 | 0 | 0 |
| A | **20/20** | 99.691% | 99.869% | 1.16 | 6 | 2 |
| B-E2B | **20/20** | 99.667% | 99.869% | 0.29 | 96 | 12 |

**What the fixes added to the development library:** fixtures C20–C26, E15 and E16, nine in all, with 147 development fixtures passing.
- On the 2,000 development seeds, none got worse and 9 got better (counting assignments and flags), compared one by one with the build before the fixes.
- The model results are unchanged, since the fixes were all in reading.

## Changed expectations

| Fixture | Was | Now | Reason |
|---|---|---|---|
| E01-mike-michael-mikey | "Mikey Baker" asked as not in the nickname table | merged with a chip, like "Mike Baker" | The table includes standard diminutives (Mikey, Bobby, Jimmy …), and the spec's own row E groups "Mike / Michael / Mikey". The expectation came from reading the spec's parenthetical list as the whole table. |
| E04-last-name-only | lone first name "Ana" merged into Ana Ruiz | asked; "yes" gives the old result | On 2,000 generated schedules, merging unique lone first names caused 4 false merges (a different person with the same first name), which breaks the Phase 3 gate. Unique lone last names caused none and still merge. |

## Known failures

`fixtures/KNOWN_FAILING.txt` lists the fixtures that fail and still run on every push. It is empty at the end of Phase 7.

| Fixture | Why | Fixed |
|---|---|---|
| K07-double-booked-after-name-merge | The double-booking appears only once "Mike B" and "Michael Baker" are one person, which needs stage 7 matching (nickname + initial). | Phase 3 |

Known limits:
- **Windows on Arm:** the shipped x64 app runs under emulation and passes its self-test with no model and with C. With A or B-E2B, the model is still loading at the self-test's 240 s limit. The app works there with smart matching off. That runner is logged, not blocking.
- **Model-tier fixtures:** the free-text pastes (M04, M05, M07) depend on one model call finishing within the 5 s limit. On 2-thread runners that call sits near the limit, so these fixtures pass in some runs and not others (MODEL_REPORT.md).
- **Not tested by automation:** first-launch security prompts, the system open and save dialogs, drag-and-drop, and a real download over office internet. These are manual checks before each release (RELEASE.md).
