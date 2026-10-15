# Decisions

Every choice the spec (`Schedule Conflict Spotter — Build Spec.md`) leaves open, and every deviation from it, with the date and the reason. Newest entries go at the bottom of each section's list.

## Austin's open decisions (spec § "Decisions still open for Austin")

**2026-10-10 · Code-signing identity: Austin Burnes, individual developer.** Same as the spec default. Signatures and the Windows SmartScreen publisher line show "Austin Burnes". Austin answered directly.

**2026-10-10 · App name and branding: "Schedule Spotter", a free tool from Tight Line.** The company is **Tight Line**, and the download page will sit on Tight Line's website. The window title and bundle name stay "Schedule Spotter". About and the download page credit Tight Line. Still pending from Austin: the website domain and the one contact line for About. Until he gives them, both are placeholders marked `TODO(Austin)` in `app/src/brand.ts` and `site/`. Bundle identifier: `com.tightline.schedulespotter`.

**2026-10-10 · Default workweek: all seven days (Mon–Sun), and the owner sets working days in the app.** This replaces the spec's Mon–Fri default and its two-way Mon–Fri / Mon–Sat toggle. Austin chose Mon–Sun and then asked that "the person needs to be able to tweak the settings in the gift's GUI, and adjust/remove working and non working days." Design:
- The toolbar has seven day chips (M T W Th F Sa Su), each on or off, plus three presets: Mon–Fri, Mon–Sat, All 7. All seven are on at launch.
- A day column header in the week grid has "Day off for everyone" for one specific date, such as a holiday.
- "Not booked" and "Unstaffed" are computed only on working days. Assignments on non-working days are still read, and double-bookings and assigned-while-off on those days are still flagged.
- The setting is not remembered between launches, because the hard constraints forbid writing anything except where the user saves and session scratch. It is saved inside the "Schedule check" workbook and restored when that file is reopened.
- Fixtures and the generator pass the working days explicitly (`settings.workdays` in `expected.json`), so each test states its own workweek.

**2026-10-10 · Spreadsheet files accepted: 10 formats.** Austin: "as many as it can accommodate, select from the most popular formats up to 10." Chosen, by how small contractors actually keep schedules:

| # | Extension | Source | Reader |
|---|---|---|---|
| 1 | `.csv` | Any spreadsheet export | own delimiter/encoding detection |
| 2 | `.tsv` | Excel/Sheets "tab-separated" | same |
| 3 | `.txt` | Excel "Text (Tab delimited)" / "Unicode Text" (UTF-16) | same |
| 4 | `.xlsx` | Excel 2007+, Google Sheets download | calamine |
| 5 | `.xls` | Excel 97–2003 | calamine |
| 6 | `.xlsm` | Excel with macros (macros never run; values only) | calamine |
| 7 | `.xlsb` | Excel binary workbook | calamine |
| 8 | `.ods` | LibreOffice / Google Sheets "OpenDocument" | calamine |
| 9 | `.numbers` | Apple Numbers (Mac-first owners) | own reader (zip + IWA) |
| 10 | `.docx` | A table typed in Word (common for small offices) | own reader (zip + XML tables) |

PDF was considered and left out, because table extraction from PDFs (often scans) is unreliable and would break the "never wrong silently" bar. If the `.numbers` reader can't reach 100% on its fixtures by the Phase 1 gate, it is dropped with an entry here, and the app tells Numbers users to use File › Export To › Excel or to paste.

**2026-10-10 · One app or two: separate, standalone products.** Austin: "Each gift is a stand alone product and may share similar frameworks but must never co mingle in the code." This repo contains only the Schedule Spotter. No crate, package or module is shared with the Document Sorter or any other gift. Patterns may be re-implemented, never imported.

**2026-10-10 · Microsoft Store listing: no for v1** (spec default). Phase 6 still writes the short report on what a Store listing would take.

## Repo and CI

**2026-10-10 · Repo: public GitHub repository.** Austin chose public. Standard GitHub-hosted runners, macOS included, are free for public repositories. Consequences the spec anticipates: Windows x64 runners are 4 cores and 16 GB instead of 2 and 8, so every Windows benchmark and self-test caps llama threads at 2 and runs inside a 4 GB job-object memory limit. The source being readable by anyone also supports the "nothing leaves this computer" claim. No license file grants reuse; the code is all rights reserved by default. Third-party licenses ship in `/Licenses` as the spec requires.

**2026-10-10 · CI-minutes estimate: $0, approval still noted.** The spec asks for a monthly-minutes estimate and Austin's approval before the nightly model tier is enabled. On a public repo, standard runners cost nothing, so the estimate in `CI_MINUTES.md` is for wall-clock and concurrency planning. It is not a cost.

**2026-10-10 · Repo location.** The spec says "a fresh repo". The repo was initialized in the folder that held the spec (`Gifts/Schedule_Conflict_Spotter/`), and the spec is committed unchanged as the source of truth.

**2026-10-10 · `models/` holds the fetch script and manifest.** Only the model files themselves (`*.gguf`) are gitignored, not the whole folder, so the pinned manifest and fetch script live next to the files they fetch.

**2026-10-10 · Reasoning-effort setting.** The spec asks for "the highest reasoning-effort setting". This session runs Claude Opus 5.5 at "xhigh", the level Austin selected, one step below "max". Recorded as a deviation, not changed.

## Stack

**2026-10-10 · Tauri 2.x, not 3.0 alpha.** crates.io's newest Tauri is a 3.0 alpha. The spec says Tauri v2, and alpha releases don't belong in a shipped gift. Pinned to the latest 2.x in `Cargo.lock` and `package-lock.json`.

**2026-10-10 · Frontend: Vite + plain TypeScript, no UI framework.** Vite is only the bundler. It is a dev dependency and none of it ships.

**2026-10-10 · File dialogs through `rfd` called from Rust, not `tauri-plugin-dialog` / `tauri-plugin-fs`.** The webview never gets file-system capabilities. The Rust side shows the system open or save dialog, reads or writes the file itself, and hands the webview only parsed results. This is fewer plugins and a smaller permission surface, and `tauri-plugin-dialog` wraps `rfd` anyway.

**2026-10-10 · `--self-test` opens the real window.** It renders the built-in example in the webview and checks the rendered flag count against the core's, which proves the webview runtime, bundled assets and CSP work on that machine. Then it prints JSON metrics and exits: nonzero on any missed threshold, or after 90 s with no result. On Windows a release build has no console, so `--json-out <path>` also writes the JSON to a file. CI uses that path.

**2026-10-10 · CSP.** The spec requires `default-src 'self'`. Tauri's IPC needs `connect-src ipc: http://ipc.localhost`. Both are in-process custom schemes, not network endpoints: Windows WebView2 intercepts the `http://ipc.localhost` origin and never resolves it. The no-socket test is the check on this.

## Phase 0 fact check (2026-10-11)

The spec's external facts were checked against primary sources: Hugging Face API, GitHub docs and runner-images issues, crates.io, Tauri, Microsoft, Apple and Cloudflare docs. Where reality differs:

**2026-10-11 · Gating runner moves from `macos-14` to `macos-15`.** `macos-14` has been deprecated since 2026-07-06. It retires on **2026-11-02**, with brownouts on Oct 12, 16, 19, 23, 26, 29 and 30 (actions/runner-images#13518). `macos-15` is the same M1, 3-core, 7 GB standard runner on the next-oldest macOS, so it keeps the spec's intent: weak Apple Silicon on an older macOS. `macos-26` (GA; now `macos-latest`) and `macos-15-intel` are unchanged. With `macos-14` gone, macOS 14 joins macOS 11–13 under "untested" in Known limits.

**2026-10-11 · `ubuntu-latest` is pinned to `ubuntu-24.04`.** `ubuntu-latest` moves to 26.04 between 2026-10-19 and 2026-11-19 (actions/runner-images#14748). Pinning keeps CI reproducible across the migration. It is a development target only.

**2026-10-11 · Gemma 4 GGUF source: lmstudio-community.** The spec's sizes (E4B 5.34 GB, E2B 3.43 GB) match lmstudio-community's Q4_K_M files exactly. bartowski's are 5.41 / 3.46 GB and unsloth's are 4.98 / 3.11 GB. Google's official QAT Q4_0 builds are also pinned: E4B 5.15 GB, E2B 3.35 GB, both ungated. Every Gemma repo ships a separate `mmproj` file, which is never fetched. All pins (repo, revision, size, SHA-256) are in `models/manifest.json`, and `models/fetch.py --verify-remote` re-checks them.

**2026-10-11 · llama.cpp support for Gemma 4** starts at build b8637, with fixes through b8751. The `llama-cpp-2` crate (0.1.159) bundles b11074, which is new enough.

**2026-10-11 · Licenses confirmed for free redistribution inside a commercial app.**
- **Phi-4-mini (MIT):** ship the copyright line and the MIT text.
- **Gemma 4 (Apache-2.0):** ship the license text. There is no NOTICE file.
- **Llama 3.2 Community License:** ship a copy of the Agreement, plus a "Notice" text file with "Llama 3.2 is licensed under the Llama 3.2 Community License, Copyright © Meta Platforms, Inc. All Rights Reserved." Show "Built with Llama" prominently in About and on the download page. The Acceptable Use Policy applies, and so does the 700M-MAU clause, which doesn't matter here.

**2026-10-11 · Plan: runtime CPU-variant selection versus static linking.** In `llama-cpp-2`, the `dynamic-backends` feature (GGML_BACKEND_DL + GGML_CPU_ALL_VARIANTS) forces dynamic linking, which conflicts with the spec's "llama.cpp linked statically" on macOS. Plan for Phase 4:
- **Windows:** dynamic backends, with the `ggml-cpu-*` variant DLLs installed next to the exe and each one signed. This meets the spec's "older CPUs without AVX2 still run".
- **macOS:** static linking, as specified. The Apple Silicon slice uses Metal plus NEON, which every M-series chip has. The Intel slice is built with AVX2/FMA/F16C. Every Intel Mac that runs macOS 11+ has AVX2 except the 2013 Mac Pro, which is logged as unsupported for smart matching. On a CPU without AVX2 the app must still run with the model off, so the check happens at runtime before the model loads.

**2026-10-11 · Risk carried to Phase 6: NSIS's ~2 GB data limit.** A standard NSIS installer can't hold more than about 2 GB of data. The Phi-4-mini file alone is 2.49 GB and Gemma E2B is 3.35–3.43 GB. Only variant C (0.81 GB) fits in the spec's one-file installer as-is. Phase 6 must prove a one-download path for the larger variants, for example an NSIS large-data build, or a stub installer that installs a model file shipped beside it in one zip. Austin will hear about it before any change to the "one download" promise.

**2026-10-11 · Risk carried to Phase 6: notarizing multi-GB bundles.** Apple publishes no size limit for notarization. It advises keeping large non-executable data out of what gets notarized, and field reports describe multi-GB uploads as slow and flaky. Phase 6 measures this with the real bundle.

**2026-10-11 · Risk carried to Phase 6: macOS App Sandbox without `network.client`.** There are open reports (tauri-docs#3171) of a sandboxed Tauri app showing a blank WKWebView without the network-client entitlement. As the spec allows, if that reproduces, the entitlement is logged and the no-socket test is the enforcement.

**2026-10-11 · Signing facts.**
- Azure's product is now "Artifact Signing". Individual developers must be in the US or Canada and verify through a Verified ID flow that takes minutes. Basic tier is $9.99/month.
- The Apple Developer Program is $99/year.
- EV certificates no longer bypass SmartScreen (confirmed).
- Both paid accounts are Austin's to open. Until then, builds are UNSIGNED, ad-hoc signed on macOS so the arm64 slice runs.

**2026-10-11 · Hosting facts.**
- GitHub Releases caps each file at 2 GiB, which every variant except C exceeds, so the spec's choice of R2 stands.
- R2 gives 10 GB-month of storage free, charges no egress fees, and accepts single objects up to 5 TiB via multipart.

**2026-10-11 · Readers.**
- `calamine` 0.36.1 reads xls/xlsx/xlsm/xlsb/ods and returns cached values, never evaluated formulas, as the spec requires.
- `.numbers` has no mature pure-Rust reader; `iwork` 0.4.0 is three weeks old. The Numbers reader is written in-repo on `zip` + `snap` + a minimal protobuf decoder.
- Its fixtures are written by Python `numbers-parser` (MIT, writes .numbers without macOS) in `tools/gen/`.

**2026-10-11 · UI tests on macOS** are now possible through WebdriverIO's `tauri-plugin-wdio-webdriver`. The spec only requires Windows and Linux. This may be added in Phase 5 if it's cheap, never as a gate.

## Austin's answers, round 3 (2026-10-11)

**2026-10-11 · Repo owner: Austin's personal GitHub account**, as a public repo named `schedule-spotter`.

**2026-10-11 · Nightly model benchmarks: approved.** `bench.yml` gets its nightly schedule when Phase 4 lands, as the spec's CI-cost step requires.

**2026-10-11 · Apple Developer Program: Austin has an account.** Phase 6 wires Developer ID signing and notarization to it. Azure Artifact Signing is on hold until Austin has read what it's for and decides.

**2026-10-11 · Windows code signing: none for now (deviation from the hard constraint "Signed on both platforms").** Austin: "We won't need an azure signing because for now we won't need to run it on the microsoft store, we simply have the download link on the website page." Consequences, as told to Austin:
- Windows builds ship unsigned.
- SmartScreen shows "Unknown publisher" rather than "Austin Burnes", and each new release starts with no reputation.
- The recovery card describes the "Unknown publisher" prompt: More info, then Run anyway.
- `release.yml` keeps an optional Azure Artifact Signing step that runs only if its secrets exist, so turning signing on later is just a matter of adding the secrets.
- macOS signing and notarization go ahead with Austin's Apple Developer account.
- Phase 6 install tests run against the unsigned Windows installer, and the `signtool verify` check is reported as "skipped: unsigned by decision" rather than as a failure.

**Superseded 2026-10-14: no email gate, downloads from tightline.systems (Phase 6).** **2026-10-11 · Download delivery: a Pi-hosted site with an email-gated link (deviation from the spec's static page with "no tracking, cookies or forms" and its Cloudflare R2 hosting).** Austin: "Pi is the website host, and the method of delivery is via qr code, where they scan, it asks for their business email, and sends them a link from the company email to download what would ordinarily be viewed as a paid tool." The flow:
1. The owner scans the QR code on the business card with a phone.
2. The page explains the tool and asks for a business email.
3. Tight Line's company email sends a download link.
4. The owner opens that email on the laptop and downloads the Mac or Windows build.

The email replaces the spec's phone "Share" button as the bridge from phone to laptop. `site/` becomes a small page plus a form handler that sends the email with a link that expires, all running on Austin's Raspberry Pi. The files are served from the Pi unless Phase 6 measurements say otherwise.

The privacy promise about the app is unchanged, since the app still never touches the network. The site now collects one email address, so the page must say plainly what it's used for.

Open items for Phase 6, to ask Austin then:
- Which mail provider sends from the company address (SMTP or API credentials, plus SPF/DKIM on the domain).
- Whether emails may be used for sales follow-up, which sets the consent wording.
- How long a link stays valid.
- Whether the Pi's upload bandwidth suits 1–3.7 GB downloads.

This page serves this gift only. Because of the no-co-mingling rule, any website-wide pieces shared across gifts belong outside this repo.

**2026-10-11 · Phase 1 drafting overlapped the Phase 0 CI run.** While the first CI matrix ran (about 30 minutes), Phase 1 groundwork was drafted on the `phase-1` branch: the fixture contract `fixtures/FORMAT.md`, the name lists, and the stage 1 reader. Independent agents also began the holdout fixtures and the generator. Nothing from Phase 1 was merged into `main` before the Phase 0 gate passed (run 38121971582). The spec's ordering applies to merging and gating, and both were respected.

## Phase 1

**2026-10-11 · `nix` is allowed, with its file-system feature only.** `zip` 9 uses `nix` with `fs` on Unix. That crate also wraps sockets behind its `socket`/`net` features. `tools/depcheck.py` now fails if any of those features is ever enabled, so the crate can stay without opening a network code path.

**2026-10-11 · Fixture rules clarified in `fixtures/FORMAT.md`.** The independent fixture author flagged nine ambiguities, all resolved without changing any expectation:
- AM/PM sub-columns give one record per cell.
- Notes columns fill `note`.
- Merged-cell patterns exported to CSV fill down silently.
- Weekday names with no anchor mean the current or coming week.
- Shop plus one job is always `internal_plus_site`.
- `pick_sheet` options compare as a set.
- `weekday_mismatch` keeps the date until answered.
- Crew-based schedules follow the window rules of the grid or list they're drawn as.
- Emoji stay in names.

**2026-10-11 · Blank rows and columns are removed before any layout decision.** Detection works on a compressed view of each sheet. Structure comes from headers and content, never from where the blanks are. That makes the spec's property "adding blank rows or columns changes nothing" true by construction. Section breaks between stacked weeks come from their own header rows and "Week of" titles.

**2026-10-11 · Reading stage 5 for header runs.** "Switch a column to day-first only if a value over 12 appears in the first position" is applied per sheet: one author, one habit. A side-by-side layout whose first week is all-ambiguous ("02/11 … 06/11") but whose second week shows "13/11" reads both weeks day-first. As a backstop, an all-ambiguous header row stays month-first unless month-first spreads consecutive day columns over more than a month while day-first makes them one week. Plain all-ambiguous files stay month-first (D02).

**2026-10-11 · Fill-down rules (spec stage 6: "only when the block shows a merged-cell pattern; otherwise ask").**
- Dates and jobs fill down silently when the pattern of a value with blanks below it occurs at least twice. A single occurrence fills down and asks `fill_down`.
- A row that carries a status ("off", "Out") never gets a job filled into it.
- Names fill down only when at least 30% of dated rows have no name in any name column, so a single blank name, which can mean nobody is on the job, is never filled.
- Real .xlsx merges (single-column, vertical) always fill.

**2026-10-11 · Header words beat content for column roles.** A column headed "Job" stays the job column even when most of its non-blank cells are OFF or vac, as in a merged-blank list where each job is written once. Content alone makes a column Status only at 90% or more status words, and Start/End only for clock-like values ("7:30", "4pm"), never bare numbers.

**2026-10-11 · How competing readings are chosen.** Every reader (day-axis grid in both orientations, list, calendar) proposes a reading, scored by: the share of data cells it understood × a size factor × how much its "names" look like people. Dates and bare numbers are never accepted as names or sites. A day-axis grid whose labels are mostly header words ("Customer", "Crew") is rejected, because that is a list with a date column. When the top two readings of different families score within 0.03, the owner is asked (`pick_layout`) and the better one is used meanwhile.

**2026-10-11 · Hours without am/pm.** A start of 5–11 is morning, 12 is noon, 1–4 is afternoon, and the end is the first matching hour after the start. A 5 o'clock start is read as 5am, the early-crew case. The generator's own convention treats 5 as afternoon, but it never emits a 5 o'clock start.

**2026-10-11 · Phase 1 gate: passed.**
- Fixture categories A–D, I and M pass 67/67 on the script tier. The three model-tier free-text pastes are left to Phase 4, as the gate's "non-model parts" allows.
- Generator seeds 0–1,999 with the model off: 99.55% assignment recall (gate ≥ 99%) and 99.47% precision.

**2026-10-11 · WebView2 reached the network; now locked down.** The CI no-socket test caught the Windows WebView2 runtime connecting to a Microsoft server (150.171.22.17:443) during `--self-test`. The first runs missed it only because of timing. Since the app never loads remote content, this was Edge's background services.

Fixes:
- The window is created in code with WebView2 arguments that switch off component updates, SmartScreen, phishing detection, sync, pings and crash upload. `--host-resolver-rules="MAP * ~NOTFOUND"` with `--dns-prefetch-disable` makes every host name unresolvable, including the app's own `tauri.localhost`. A second CI run showed Chromium pre-connecting to loopback port 80 for that origin even though Tauri answers its requests in-process.
- The window runs in private (incognito) mode, and its data directory is the session temp folder.
- `--self-test --linger 60` keeps the app open a minute on every CI runner while sockets are polled, so delayed background calls get caught too.

**2026-10-11 · Session scratch folder.** WebView data (Windows, Linux) goes to `<OS temp>/schedule-spotter-session-<pid>/`. It is deleted on exit, and any leftover from an earlier session (a crash, or files still locked at exit) is swept at the next launch. This is the spec's "session scratch in the OS temp folder, deleted on exit and again on next launch". On macOS, private mode gives WKWebView a non-persistent data store.

## Phase 2

**2026-10-11 · Identity before matching.** People are one identity when their spellings differ only in case, spacing or stray end punctuation. Accents still separate them; "José" vs "Jose" is a stage 7 merge with a chip. Sites also merge across street-suffix and direction spellings ("1420 Johnson Street" = "1420 Johnson St"). That deterministic normalization is from stage 7, done early because K08 ("a false double-booking that disappears after a site merge") is a Phase 2 category. The canonical name shown is the longest spelling.

**2026-10-11 · Rules settled where FORMAT.md was silent.** Proposed by the G–K fixture author and adopted into FORMAT.md:
- Times are start-inclusive and end-exclusive.
- Overnight shifts are compared on their start date.
- Part words in note position set `part`.
- A status in an AM/PM sub-column carries that part.
- The 50,000-row cap keeps the earliest four Monday–Sunday weeks.

Added here:
- A `half` part clashes with nothing that has its own part or time.
- A half-day status never conflicts with work.

**2026-10-11 · Phase 2 gate, measured on everything that doesn't need stage 7.**
- Handmade fixtures: 99/100 on the script tier, with flags and info compared exactly. The one exception is K07, "a double-booking visible only after a name merge" (Mike B + Michael Baker). Its merge is stage 7 nickname and initial matching, which is Phase 3 by the spec's own ordering, so it counts toward the Phase 3 gate.
- Generated seeds 0–1,999: zero red flags on all 392 clean schedules.

**2026-10-11 · Speed.**
- The 50,000-row file read in 16.9 s because a per-row count was recomputed once per cell on the transposed, 50,000-column view. Now 1.1 s.
- A typical 60 × 25 × 4-week file takes 38 ms on this machine, against the spec's 200 ms target on the gating runner. CI timing is measured in Phase 4's benchmark.

## Phase 3

**2026-10-11 · Matching rules (stage 7), tuned against the generator's truth.** The full rules are in `fixtures/FORMAT.md` under "Matching". Decisions the data forced:
- **Lone first names are asked, never merged.** Merging a unique lone first name produced 4 false merges in 2,000 generated schedules, for example "Guadalupe" was another Guadalupe, not Guadalupe Acosta. Unique lone last names produced none, so they merge with a chip.
- **The nickname table is conservative.** Cross-language guesses (Joe→José, Gary→Gerald, Rick→Eric and the like) caused false merges and were removed. The spec's Spanish pairs stay, as do standard diminutives like Mikey for Michael.
- **Two bare first names related only by nickname are asked.** "Jeff" / "Jeffrey" may be two people in a first-names-only schedule.
- **An initial fits a real first name, or any first name followed by a surname.** "S." is not "Swanson", but "R. Singh" may be "Ravi Singh". "R" also fits "Bob", through Robert.
- **"Less specific" is measured by how much of the name is spelled out**, not by string length. A pair where each side abbreviates what the other spells out is asked.
- **Trade words make a company even after a first name.** "Ward Plumbing" is a subcontractor.
- **"Job #" headers are job numbers**, recognized before the '#' is trimmed.

**2026-10-11 · Answers override every rule.** Answering "no" to a chip undoes that merge. Questions name each side by its group's canonical spelling after silent merges. Answers also apply to the reading stage:
- which sheet;
- which layout;
- a weekday that contradicts its date ("no" uses the written weekday);
- fill-down ("no" leaves the blanks);
- column roles.

**2026-10-11 · Two fixture expectations changed (spec: "unless the expectation itself was wrong").** E01 and E04 encoded matching rules that this phase replaced on evidence. See TEST_REPORT.md "Changed expectations".

**2026-10-11 · Phase 3 gate: passed.**
- Handmade fixtures: 124/124 on the script tier, plus 3 model-tier free-text pastes left for Phase 4.
- Generated seeds 0–1,999: 0 false merges in 3,152 merges, and 0 red flags on clean schedules. CI enforces both.
- Model-off baseline: 1.21 questions per file, and 64% of files ask nothing.

## Phase 4

**2026-10-11 · llama.cpp binding: `llama-cpp-2` 0.1.159, features `common` only.**
- **No OpenMP.** The shipped app has no libgomp dependency.
- **Network-free build, checked on the built binary.** The crate builds llama.cpp with `LLAMA_CURL=OFF`, and the server and tools aren't built. The benchmark binary has no curl, HTTP, SSL, DNS or socket imports (`nm`).
- **Local Linux builds** need `BINDGEN_EXTRA_CLANG_ARGS=-I/usr/lib/gcc/x86_64-linux-gnu/13/include`, because this machine has libclang but no clang headers. CI runners have full toolchains.

**2026-10-11 · Gemma 4's chat template is rendered in-repo.** llama.cpp's built-in template engine doesn't recognize Gemma 4's new `<|turn>role … <turn|>` format. When applying a model's template fails, the engine renders the verified Gemma 4 format itself: system turn, alternating user and model turns, reply opened with `<|turn>model`, thinking off.

**2026-10-11 · Formula text in text files stays as data.** Category L showed the reader was dropping `=SUM(…)`-style text from CSVs, a rule the spec doesn't have. The spec says spreadsheet cells give their stored values. In a CSV, that text is what the owner typed. It's now kept, with a note that it was read as plain text and never calculated, and the export (stage 10) writes it as a string. Error values like `#REF!` still become empty, with a note.

**2026-10-11 · Weight repacking off (`GGML_CPU_REPACK=OFF`, `.cargo/config.toml`).** llama.cpp's CPU backend copies quantized weights into a repacked layout in anonymous memory. The memory-mapped originals stay resident too, which roughly doubles weight memory. Measured peak RSS at load, 2 threads:

| Model | Repack on | Repack off |
| --- | --- | --- |
| Phi-4-mini | 4.10 GB | 2.88 GB |
| Gemma E2B | 4.55 GB | 3.68 GB |

Latency rose about 10%. Without this change, A and B-E2B both break the 4.0 GB ceiling. It sits in the cargo config so the app, the benchmark and CI all build the same way.

**2026-10-11 · CPU builds, carrying out the Phase 4 plan above.**
- **Windows x64:** `dynamic-backends`. Every `ggml-cpu-*` variant is built, from SSE4.2 to AVX-512, and the best one for the machine is loaded at start from the exe's folder. Phase 6 installs and signs those DLLs.
- **macOS Apple Silicon:** static, with Metal.
- **macOS Intel:** static AVX2/FMA/F16C. A runtime check before loading keeps the model off on CPUs without AVX2/FMA (the 2013 Mac Pro).
- **Windows on Arm:** static NEON.
- **Linux (development and CI only, not a ship target):** static AVX2. The binding's dynamic-link copy step breaks on Linux's versioned `.so` symlinks.

**2026-10-11 · Each task's fixed prompt prefix is evaluated once and restored.**
- **What.** The system text and examples (about 400–630 tokens) are evaluated once per task. Their KV state is saved with `state_seq_get` and restored with `state_seq_set` when that task comes up again.
- **Why.** On 2 CPU threads, prompt evaluation runs at about 50 tokens/s. A cold prefix alone took 8 s, over the 5 s call limit. Restored, the first T1 call takes 1.3 s.
- **Warm-up.** `Engine::warm` runs on the loading thread after `load`. The app warms tasks in the order questions come: T4, T3 (reading), then T1, T2 (matching), and T5 only for pastes. Warm-up time is reported separately from load time. The spec's "model load ≤ 20 s" is applied to load alone, and MODEL_REPORT.md shows both numbers so Austin can judge.
- **Cost.** No measurable change in peak RSS for E2B (3.68 → 3.70 GB).

**2026-10-11 · Grammar sampling checks the 40 likeliest tokens first.** Greedy decoding under a grammar picks the highest-logit token the grammar allows. Checking only the top 40 gives the identical token whenever one of them fits, and the code falls back to the full vocabulary when none does. On Gemma's 262k-token vocabulary, the full check cost about 22 ms per token, roughly 30% of generation time. Outputs are unchanged.

**2026-10-11 · Deviation: T5's raw output is grouped.**
- **What changed.** The model returns `{"groups": [{days, people, site, time}]}`, and the engine expands that to the spec's list of {person, day, site, time?}. For example, "Mon–Fri: Mike, Jose @ Smith" is one short group, not ten entries.
- **Why.** Per-entry output took 50–90 tokens, about 14 s on 2 threads, and longer lines never finished.
- **Unchanged.** An empty list still means "cannot read". The T5 guard runs on the expanded entries.

**2026-10-11 · Pasted lines are split before T5.** "Mon: Mike & Jose @ Johnson. Tue: Mike - Smith job" is split where a sentence after ". " or "; " starts with a weekday, so the model reads one day's line at a time. Other periods ("1420 Johnson St. Unit B") are untouched.

**2026-10-11 · Where the model is asked inside reading.**
- **T4:** asked when a list column's person and site scores tie and there's no header, before the owner is asked. "person" or "site" settles it. Anything else falls through to the owner's question.
- **T3:** asked for a lone value in a person-grid day cell that reads as neither a site nor a person, with column role `day`.
  - "status" makes it an absence.
  - "note" or "empty" drops it to a note.
  - Anything else leaves it as a site.
- **T5:** asked for pastes the table reader can't read.
- **Script-tier fixtures** always run with the model off: they define the rules' behavior. Model-tier fixtures run with the model under test (`spotter-check fixtures --model`).

**2026-10-11 · Prompt examples never repeat benchmark questions.**
- **What happened.** The first prompts reused the spec's own sample pairs: Micheal/Michael, Mike B/Mike R, 1420/1240 Johnson, Walgreens #3299/#1087, "lunes: Chuy y Memo…". The benchmark set uses the same pairs, so 13 benchmark items were answered inside their own prompts.
- **The fix.** The examples were rewritten with fresh names and places. `tools/check_prompt_leak.py`, run in CI, fails if any example's names, cell, header + samples, or line matches a benchmark question.
- **Prompt policy.** The prompts now state the labeling conventions written in `bench/questions/README.md`:
  - nicknames and Spanish/English forms are the same person;
  - real names a letter apart are different people;
  - a lone first name is the same person when no one else has it;
  - a surname alone in a site column is a customer;
  - Jr/Sr marks people;
  - reasons a person is off are status.
- **Earlier contradictions removed.** T1 said "different initials mean different people", which the model applied to Ted/Edward. T4 called daily hours "ignore" while its own system text said hours are "time".

**2026-10-11 · Benchmark scoring.**
- **Controls stay in.** The 14 questions the rules settle on their own in the app (Mike B/Mike R, Sr/Jr, store numbers) are scored, because the spec names them as part of the set.
- **T5 is scored strictly.** Every entry's person, day, site and time must match, and so must "cannot read".
- **Injection items get their own score.** Per-question records go in the output so MODEL_REPORT.md can report the 21 prompt-injection questions separately.

**2026-10-11 · A model "same" needs confidence ≥ 0.9 to merge (`SAME_MIN`).**
- **What.** The engine records the probability of each chosen token among the tokens the grammar allows. A T1/T2 "same" below 0.9 is treated as "unsure", which becomes the owner's Yes/No question.
- **How the value was chosen.** Swept on the odd-numbered T1/T2 questions and checked on the even ones, for both E2B builds.
- **Results:**

  | Model | Threshold | False merges | Unsure |
  | --- | --- | --- | --- |
  | E2B | 0 | 21–24% | 4–5% |
  | E2B | 0.9 | 13–16% | 10% |
  | E2B-QAT | 0 | 15–16% | — |
  | E2B-QAT | 0.9 | 5–7% | 25–31% |
  | E2B-QAT | 0.999 | 0% | 58% |

- **Why 0.9.** No threshold brings either model within both limits (false merges ≤ 1%, unsure ≤ 15%). Gemma E2B is often confidently wrong on real look-alike names (Marla/Marta, Shawn/Shane at 0.998–1.0). 0.9 is kept because a wrong merge can hide or invent a red flag, while an extra question only costs the owner a click.
- **Reporting.** MODEL_REPORT.md reports both the raw answers and the gated ones.

**2026-10-11 · Windows on Arm runs the x64 build.** llama.cpp's own Arm build refuses MSVC ("MSVC is not supported for ARM, use clang"). The spec ships Windows x64 only, and the app's self-test on `windows-11-arm` already runs the x64 installer under emulation. So the benchmark on that runner builds `x86_64-pc-windows-msvc` and runs it emulated: it measures what an Arm laptop would actually run. The runner stays non-blocking.

**2026-10-11 · Intel Macs: Metal off at build time.**
- **What happened.** llama.cpp turns Metal on for every Apple build, whatever the binding's features say. On `macos-15-intel` the benchmark spent 62 s compiling Metal shaders, then aborted in the Metal backend (`GGML_ASSERT(buf_dst)`) on the runner's paravirtual GPU.
- **The fix.** The spec says "CPU on Intel Macs". A universal build compiles both architectures in one run, so a plain environment variable can't separate them. Instead, `CMAKE_TOOLCHAIN_FILE_x86_64_apple_darwin` (in `.cargo/config.toml`) points the cmake crate at `tools/cmake/intel-mac-cpu.cmake` for the Intel slice only, and that file forces `GGML_METAL=OFF`. Apple Silicon keeps Metal.
- **CI caches.** The cmake crate doesn't declare that variable as a rebuild trigger, and cargo only tracks `GGML_*` options that were already set at the last build. So every Rust cache in CI is now keyed on a hash of `.cargo/config.toml`: any change to the build options means a fresh build.

**2026-10-11 · Metal shaders compiled at build time (`GGML_METAL_EMBED_LIBRARY=OFF`).**
- **What happened.** By default llama.cpp embeds its Metal shader sources and compiles all 20 libraries at every start. The first nightly measured model loads of 40–65 s on the Apple Silicon runners (32 s of shader compiling) against the spec's 20 s.
- **The fix.** The build now produces `default.metallib` with `xcrun metal`. The app bundle carries it in `Contents/Resources` (`tauri.macos.conf.json`, `bundle.macOS.files`), which is the first place llama.cpp looks. The benchmark puts it beside its binary.
- **CI.** CI installs Apple's Metal compiler where Xcode 26 ships it separately, and checks that the file is in the `.app`.
- **Caveat for MODEL_REPORT.md.** The CI Macs are virtual machines whose GPU ("Apple Paravirtual device") lacks the SIMD-group operations llama.cpp's fast Metal kernels use. Metal latency there overstates what a real M-series Mac does.

**2026-10-11 · Known limit: the model is impractical on Windows on Arm.** The shipped x64 build runs under emulation on `windows-11-arm`. The app itself is fine there and passes its self-test. Model calls are another matter: in the first nightly they took 7–35 s at p95, so nearly every one ran past the 5 s limit and became an owner question. On those laptops the app effectively runs with the model off. A native Arm64 Windows build would need llama.cpp built with clang instead of MSVC, and the spec ships Windows x64 only. This goes into Known limits.

**2026-10-11 · The model runs on the CPU when the only GPU is a VM's paravirtual device.**
- **What we measured.** On the CI Macs, Metal on "Apple Paravirtual device" generated about 4 tokens/s. The development machine's two CPU threads manage about 15–20. The paravirtual GPU lacks the SIMD-group operations llama.cpp's fast kernels need. The CPU's speed on the runners themselves comes from the next nightly, which runs both backends.
- **The rule.** `use_gpu()` keeps Metal on real Apple Silicon and uses the CPU when every GPU llama.cpp sees is paravirtual: no GPU layers, and no KV or op offload. That matches the spec's weak-hardware intent and helps anyone running macOS in a VM. `SPOTTER_LLM_BACKEND=cpu|gpu` overrides the choice.
- **Reporting.** The benchmark records which backend ran. On Apple Silicon runners it adds a short forced-Metal run, so MODEL_REPORT.md shows what the choice is worth. Load time is reported with the SHA-256 check's share.

**2026-10-11 · `sha2` 0.11 for the model checksum.**
- **What happened.** On the Apple Silicon runner, the SHA-256 check took 21.5 s of E4B-QAT's 28.1 s load, about 240 MB/s. `sha2` 0.10 only uses the ARM SHA instructions with its `asm` feature, so it ran in software.
- **The fix.** `sha2` 0.11.0 (published 2026-03-25) detects the instructions at run time by default.
- **Verified locally.** The output hash is unchanged, and a mismatch still turns the model off.
- **What's left.** If the runner's disk turns out to be the limit, the next step is to run the hash alongside the model load.

**2026-10-11 · Selection rule: the best scorer is chosen first, then checked.**
- **The spec.** "If none passes all, recommend the best scorer only if false merges ≤ 1% and RSS ≤ 4.0 GB; otherwise recommend model off."
- **What was wrong.** `tools/model_report.py` first dropped every variant over those limits, then took the best scorer of the rest. On the first full CI run that picked Llama 3.2 1B (C), which answers under half the questions and is right on 22.6% of all of them.
- **Now.** The report takes the best scorer of all variants that ran (share of all questions answered correctly) and recommends it only if it is within both limits; otherwise model off, with the reason. On run 38184895853 that is B-E2B at 70.5%, with 15.3% false merges, so **model off**.
- **Unchanged.** The report still gives the best scorer's end-to-end numbers against model off. Austin makes the final pick.

**2026-10-11 · Follow-up for Phase 7: free-text pastes on slow CPUs.** On the CI runners (2 llama threads, CPU) T5's p95 sits at the 5 s call limit, and the model-tier free-text fixtures (M04, M05, M07) come back with no rows, though they pass on the development machine. Options for the final run: a T5-only time limit, or splitting long pastes into smaller calls. The spec's 5 s figure is a threshold, so neither is changed without a measured reason.

## Phase 5

**2026-10-11 · Window layout.** The spec lists the window top to bottom: toolbar, week grid, unstaffed jobs, findings, questions, What I read. At 1280 px and wider, that order becomes two columns so findings are visible without scrolling: the grid, job rows and the editable schedule on the left; findings, questions and What I read on the right. Below 1,100 px it is one column in the spec's order. The toolbar is sticky, so it never scrolls away (the UI test checks this at 1280×800).

**2026-10-11 · Working days in the window** (Austin's Mon–Sun decision above):
- seven day buttons (M T W Th F Sa Su) and three presets (Mon–Fri, Mon–Sat, All 7) in the toolbar;
- a "Day off?" button under each date in the week grid for one-off days off for everyone.
- A newly opened schedule keeps the owner's working days but not the previous file's dates off.

**2026-10-11 · Paste.** The Paste button opens a box to paste into, which needs no clipboard permission. Cmd/Ctrl+V anywhere outside a text field checks the clipboard's text at once.

**2026-10-11 · Editing the table ("Edit the example table directly").**
- "Your schedule" shows the cells of the sheet being checked; results update as the owner types (400 ms after the last key).
- It is open for the example and one click away for other files.
- An edited schedule becomes a plain table of that one sheet: a workbook's other sheets are dropped, and dates are kept as their text.
- Editing in the app is offered up to 400 rows × 60 columns; bigger files are edited in their own spreadsheet program.

**2026-10-11 · The "Schedule check" workbook is written in-repo** (`core/src/export.rs`, on `zip` already in the tree) rather than with an XLSX-writer crate:
- **Sheets:** Findings (with still-open questions and notes), Week grid (red, amber and striped fills), Cleaned schedule (matched names beside what was written), and Name and site matches (with the owner's answers).
- **Text cells:** every one is an inline string, so Excel never evaluates one (stage 10).
- **Saved state:** a very hidden sheet, `SpotterState`, carries the check as JSON: the original file (base64) or pasted text, every answer, and the working days. It is split into 30,000-character cells to stay under Excel's 32,767 limit.
- **Reopening:** opening the file restores all of that.
- **Why a hidden sheet:** a custom XML part could be dropped when Excel re-saves the file; a hidden sheet survives.
- **Limit:** if the owner edits the visible sheets in Excel, reopening shows the saved check, not those edits. The visible sheets are a report.

**2026-10-11 · Saving never leaves half a file.** The bytes go to a temporary file beside the target (`.<name>.<pid>.saving`), then are renamed over it. That temporary file is inside the place the owner chose, which the "writes nothing except where the user saves" constraint allows. A failure removes it and shows a plain message ("That folder can't be written to. Choose another place, such as Documents.").

**2026-10-11 · Findings CSV.** The save dialog offers "CSV, findings only" as a second format. Excel does run formulas in CSV cells, so cells starting with =, +, -, @, tab or carriage return get a leading apostrophe (OWASP's CSV-injection guard). The file starts with a UTF-8 byte-order mark so Excel shows accents correctly.

**2026-10-11 · Undoing a chip is an answer.** "Undo" answers "no" to that pair, and "Put back" (under Your answers) withdraws the answer.
- **Bug found by N02:** spellings that share an identity key ("1420 Johnson St" / "1420 Johnson Street") showed a chip that undo couldn't split.
- **Fix:** an undo now gives that spelling a key of its own.
- **Regressions:** none. Generator seeds 0–1,999 are unchanged: 0 false merges in 3,152 merges.

**2026-10-11 · "What I read" corrections.**
- **Lists:** the owner can set any column to Person, Job, Date, Day, Time, Start time, End time, Status, Note or Ignore. Before, only Person or Job could be set.
- **Grids:** their axes are corrected by picking another layout reading; all candidate readings are offered.
- A grid's date columns are listed as one run.

**2026-10-11 · Model in the app.** The variant is a build flag:
- `SPOTTER_VARIANT=A|B-E2B|…|C` makes `build.rs` bake in the model's file name, SHA-256, name and license from `models/manifest.json`;
- unset means a model-off app;
- the app looks for the file at `<resources>/model/<file>`, where Phase 6's bundles will put it (debug builds also look in `models/`);
- it loads on a background thread after the window shows.

Each change shows the rules' result at once; the model's pass follows, and a pass for an older change is dropped. Until Phase 7's pick, CI builds are model-off.

**2026-10-11 · Today's date comes from the page.** The webview knows the owner's local date, so the page sends it; Rust's `chrono` stays without its `clock` feature and the time-zone crates it pulls in. The example shows the current week, or next week on a weekend, when owners lay it out. The self-test pins the date so the page and the core agree.

**2026-10-11 · Self-test, Phase 5.** It now checks that the page shows exactly the core's findings, questions and week-grid rows for the example, and that the example has findings and a question. It still runs with the model off and the 300 MB ceiling.

**2026-10-11 · UI tests: `tools/ui/ui_tests.py`, a standard-library WebDriver client** (no Selenium or WebdriverIO).
- **Driver:** `tauri-driver` 2.0.6 runs WebKitWebDriver on Linux, and on Windows the runner image's Edge WebDriver, which drives WebView2.
- **What's tested:**
  - every category N fixture's steps through the window, with N06 by keyboard alone;
  - the example (all four kinds of finding, a chip, a question, red, amber and striped cells, a sticky toolbar, no text under 15 px);
  - editing the table;
  - the Paste box.
- **What differs in the `ui-test` build:**
  - it adds `test_open` and `test_save`, which take a path, because WebDriver can't drive system dialogs;
  - its webview keeps the platform defaults, because:
    - WebKitGTK automation can't attach to the ephemeral context wry makes for incognito webviews;
    - Edge WebDriver sets WebView2's data folder and debugging port through environment variables, and the app's own data folder and lockdown arguments would override them.
- **Windows:** the test attaches Edge WebDriver to the running app instead of having `tauri-driver` launch it.
  - **What failed:** when msedgedriver launched the app itself, it never found WebView2's `DevToolsActivePort` file. That happened three times on `windows-2025`, with the driver and runtime both at 153.0.4234.48 and the UI-test build passing its own self-test.
  - **What works:** with `SPOTTER_CDP_PORT` set, the UI-test build opens a DevTools port on localhost. `ui_tests.py --attach` starts the app with it and attaches msedgedriver through `debuggerAddress`, a route Microsoft supports for WebView2.
- Shipped builds have none of these changes. Their network lockdown is checked by the self-test's no-socket test.

**2026-10-11 · Diagnostics.** The text is built from structured data with placeholders (Person 1, Site 2, Text 3). Then every spelling the schedule contains, plus the file name, is scrubbed as whole words, as a second guard. It adds the app's own log: what was opened (extension and size only), timings, model status and page errors. The owner sees the full text before saving it.

**2026-10-11 · About.** It shows the version, Tight Line, the privacy line and how to check it, the model and its license, "Built with Llama" for variant C, and a short license list. The full license texts go into the bundle's Licenses folder in Phase 6. The website and contact line are blank until Austin gives them (`app/src/brand.ts`, `TODO(Austin)`); both are plain text, never links.

**2026-10-11 · System dialogs through `rfd` 0.17, asynchronously.** The open and save dialogs are `rfd::AsyncFileDialog`, parented to the window, called from the app's async commands:
- **macOS:** a sheet on the window, run on the main thread by rfd.
- **Windows:** a COM file dialog on rfd's own thread.
- **Linux (development only):** the `gtk3` backend with no XDG portal (the portal would bring D-Bus networking crates). It runs GTK on a thread of its own beside Tauri's. Checked in the dev container: the dialog opens and the app keeps answering.

WebDriver can't drive any of these dialogs, so the UI tests use the `ui-test` hooks, and the real dialogs get a manual check on each platform before release (RELEASE.md, Phase 6).

## Phase 6

**2026-10-12 · Release variants: off, C, A and B-E2B.** The spec's deliverable is a build set for each of A, B and C.
- **B is Gemma 4 E2B Q4_K_M.** It was the best scorer in the Phase 4 benchmark. E4B needs more than the 4 GB ceiling on Intel Macs, and the spec allows E2B as "the in-family fallback".
- **"off" is built too,** because model off is the selection rule's current recommendation (MODEL_REPORT.md).
- `release.yml` takes the list as an input, so any manifest entry can be built.

**2026-10-12 · The Windows installer carries the model after its own data, not inside it.** NSIS can't hold more than about 2 GB (Phase 0 risk). Phi-4-mini (2.49 GB) and Gemma E2B (3.43 GB) don't fit, so neither does a one-download installer built the usual way.

How it works instead:
- `tools/release/append-model.py` appends the model to the finished `-setup.exe`: the model bytes, their length, and an 8-byte marker.
- The installer's post-install hook runs `<app> --install-model "$EXEPATH"`.
- The app (`payload.rs`) copies the bytes into `model\`, checks them against the SHA-256 compiled into it, and only then gives the file its real name.
- If the check fails, the app still installs and runs with smart matching off ("Never fails because of the model"). The owner sees a message to download again, and a silent install exits 2.
- The uninstaller removes `model\`.

Why this design:
- It keeps the spec's "one download" and "no admin rights".
- Signing still works later: Authenticode adds its certificate table after the appended model, and the app looks for the marker just before that table. The model then counts as part of the signed file.
- The `.exe` must stay under 4 GiB. Installer vendors report that Windows won't start a larger program, and `append-model.py` refuses to build one. B-E2B comes to about 3.6 GB.
- Rejected: a zip with the installer and the model side by side (owners would run the installer from inside the zip, without the model); NSISBI, an NSIS fork for large installers (unmaintained; Tauri can't use it).

The release run's install tests are the proof that Windows runs and installs the 3.6 GB file.

**2026-10-12 · The macOS disk image is made with `hdiutil` from the finished app** (`tools/release/make-dmg.sh`), not by `tauri bundle`.
- `tauri bundle --bundles dmg` rebuilds the app, so it would re-sign it and undo Tauri's notarization and stapling of the app.
- The image holds the app and a link to Applications, with no background art.
- The release workflow builds the app, deletes the build's intermediate files to make room on the runner, and then makes the image. A 3.5 GB app needs room for one more copy.
- CI uses the same script, so its install tests run on the same kind of image.

**2026-10-12 · macOS App Sandbox, with `network.client` (spec: "OS-level network block on macOS").**
- **Tried without network entitlements:** on all three Mac runners the window stayed blank and the self-test timed out. WebKit's page process won't start inside a sandbox without `com.apple.security.network.client`, even though the app loads nothing over the network (tauri-apps/tauri-docs#3171, where others reproduce it).
- **Decision, as the spec allows:** keep the sandbox and add `network.client` only.
  - The sandbox still confines files: the app reads and writes only what the owner picks in a dialog or drops on the window.
  - Nothing may listen (`network.server` stays off).
  - The no-socket check during the self-test enforces "never touches the network".
- **Save under the sandbox:** the sandbox lets the app write only the file picked in the save dialog, not a temporary file beside it. `export::save` now writes the picked file directly when it can't create the temporary one. A core test covers this with a read-only folder.
- **The container:** macOS creates `~/Library/Containers/com.tightline.schedulespotter` on first launch. The app's session scratch lives inside it and is deleted on exit. The install test checks that nothing appears in `~/Library` outside the container. Throwing the app in the Trash leaves the container behind, as with every sandboxed Mac app (RELEASE.md).

**2026-10-12 · `--self-test` loads the model** (spec: "runs the built-in example plus 20 tiebreaker questions").
- In a build with a model, once the page has reported back, the self-test loads the bundled model from where it was installed. It then asks the first 5/5/4/3/3 questions of T1–T5 from `bench/questions`.
- **Gated:**
  - the model loads, which includes its SHA-256 check;
  - all 20 questions are asked;
  - no llama.cpp call fails;
  - peak RSS is at most 4 GB (300 MB with no model).
- **Reported, not gated:** load time, latency p50/p95, and how many it answered. These depend on the runner, and the benchmark already gates them (MODEL_REPORT.md).
- The model loads only after the page check, so that check still compares the page with the core's model-off result.

**2026-10-12 · Install tests (spec: "Install tests, every release candidate").** `tools/ci/install-test-macos.sh` and `install-test-windows.ps1` replace the Phase 0 self-test scripts. They run in CI on every push (model-off build) and in `release.yml` on every variant.

macOS:
- Marks the disk image with a browser's quarantine attribute, mounts it, and copies the app with `ditto`. The quarantine carries over onto the app.
- Checks the universal binary, the Licenses folder, the model, `codesign --verify --deep --strict`, the sandbox entitlements and the hardened runtime.
- Asks for Gatekeeper's verdict.
- Runs the self-test while polling `lsof` for sockets, then checks for leftovers.
- **Signed builds** must pass Gatekeeper as "Notarized Developer ID", with stapled tickets on both the app and the image, and they launch still quarantined.
- **Unsigned builds** (until the secrets exist) report Gatekeeper's rejection as skipped and remove the quarantine before launch, as an owner's "Open Anyway" would.
- A quarantined app started from a shell skips Gatekeeper's first-launch dialog, which CI can't show anyway (Known limits). The `spctl` verdict is the check.

Windows:
- Writes a Mark of the Web (zone 3) on the installer and checks its signature (skipped while unsigned by decision). Confirms its manifest asks for `asInvoker`.
- Creates a separate standard user. It is given only the "log on as a batch job" right, so a scheduled task can run as it.
- Installs silently as that user, which also proves no admin rights are needed: a standard user can't elevate in a silent task. Checks the per-user location, the uninstall entry, the Start menu shortcut, the Licenses folder and the model.
- Runs the self-test from the installed location (as the runner's account, since a scheduled task has no desktop for a window), watching the app and its WebView2 processes for sockets.
- Uninstalls silently as the standard user, then compares that user's profile and registry with a snapshot taken before the install. Anything of the app's left behind fails the test.

**2026-10-12 · Install test findings on Windows.**
- **A file with a Mark of the Web hangs when started through the shell from a scheduled task.** The shell shows its "publisher could not be verified" prompt, and a task has no desktop to click it on: the first run waited 20 minutes. The test now starts the installer directly with `CreateProcess`, as a silent deployment would. The file keeps its mark. An owner sees SmartScreen there instead (recovery card).
- **WebView2 holds the session folder past the app's exit.** On both Windows runners its processes still had files open in the scratch folder when the app exited, so the folder outlived the session. When `scratch::remove` can't delete it, the app now starts a hidden copy of itself (`--remove-scratch <folder> <pid>`). The copy waits for the app to exit, then deletes the folder, retrying for up to a minute. It only ever deletes a `schedule-spotter-session-*` folder directly in the temp folder, and the next launch still sweeps anything left.
- **NSIS's uninstaller runs from a copy of itself in `%TEMP%\~nsu*.tmp`,** which it can't delete while running. That copy is NSIS's standard behavior and lives in the temp folder, so the leftover check reports it with the other new paths rather than failing on it.

**2026-10-12 · macOS builds link clang's runtime library** (`llm/build.rs`). The first release run, a fresh build with no cache, failed to link on Apple Silicon with `Undefined symbols: ___isPlatformVersionAtLeast`. llama.cpp's Metal code uses `@available`, which clang turns into a call into its runtime library (`libclang_rt.osx.a`). rustc links with `-nodefaultlibs`, so that library is never linked unless something names it. CI's own Mac build reused an older cached llama.cpp build and never hit this. The build script asks `xcrun clang --print-runtime-dir` where the library is and links it.

**2026-10-12 · Windows runs and installs the multi-GB installers** (release run 38236036646, `windows-2025`). This settles the Phase 0 risk.
- **A:** 2.71 GB installer. The silent standard-user install unpacked and verified the model, the model loaded in 3.1 s, and peak memory was 3.0 GB.
- **B-E2B:** 3.65 GB installer. Model load 4.4 s, peak memory 1.75 GB.
- **Both:** all 20 questions asked, no socket opened, nothing left after uninstall.

**2026-10-12 · Known limit, logged and not blocking: the model self-test times out on `windows-11-arm`.**
- **Fails:** A and B-E2B install and run correctly there, but under x64 emulation loading and warming the model takes longer than the self-test's 240 s wait.
- **Passes:** C and model off pass on the same runner.
- This fits Phase 4's finding that the model is impractical on Windows on Arm. The app answers on its own meanwhile, sending every unsure item to the owner.
- The spec makes this runner logged, not blocking. The wait stays at 240 s, because a longer one would only hide how slow it is.

**2026-10-12 · Silent uninstall now removes Tauri's registry key.** Tauri's uninstaller keeps `HKCU\Software\Tight Line\Schedule Spotter` (install folder and language) unless the owner ticks "delete app data", and a silent uninstall never does. The app keeps nothing there, so the uninstall hook deletes it, except during an update. `bundle.publisher` is now "Tight Line": it names that key and the uninstall entry's publisher, and it keeps the publisher different from the product name, which the Microsoft Store would require.

**2026-10-12 · Licenses folder** (`tools/release/licenses.py`, run by `prepare.py` for each variant and platform). It holds:
- every crate compiled into the app for the target, with the license files it ships (identical texts printed once);
- llama.cpp's license, plus the notice of nlohmann/json, which llama.cpp compiles in;
- the bundled `@tauri-apps/api` JavaScript;
- on Windows, a WebView2 note;
- the model's license and notices (`models/licenses/`, sources and SHA-256 in `SOURCES.md`):
  - Phi-4-mini: LICENSE and NOTICE.md from Microsoft's repo.
  - Gemma 4: the Apache-2.0 text, since its repos ship none.
  - Llama 3.2: the Community License and Use Policy from Meta's GitHub (the Hugging Face repo is gated), plus the `NOTICE.txt` the license requires.

A few crates ship no license file:
- the `objc2` family, `webview2-com`, `llama-cpp-2`, `selectors` (MPL-2.0) and `alloc-stdlib` (BSD-3-Clause);
- for these, the standard SPDX text of their declared license is included, with their authors as the copyright holders (`tools/release/license-texts/`, SPDX license-list-data 3.27.0);
- the file says where every crate's source is, which the MPL asks for.

**Superseded 2026-10-14 and removed (below).** **2026-10-12 · Download site: Python standard library behind nginx** (`site/`). This implements Austin's Pi-hosted, email-gated delivery.
- **The service** shows the pages, sends the email over SMTP, and checks links. It needs nothing installed beyond Python 3, which Raspberry Pi OS has.
- **nginx** serves the files, with resume support. It asks the service (`auth_request`) whether a link is still good before each one.
- **Links** are HMAC-signed, expire after `LINK_DAYS` (default 7), and allow `MAX_DOWNLOADS` fresh downloads (default 5). Resuming a download doesn't count.
- **Limits:** 3 links per address per day, 10 requests per network per hour in the service, and 6 a minute at nginx. A hidden field catches bots.
- **Privacy:** no cookies, no scripts and no nginx access log. An address is deleted a day after its link expires, unless the owner ticks a follow-up box. That box exists only once Austin sets the wording (`CONSENT_TEXT`).
- **"Business email"** is how the field is labeled, but personal domains such as gmail.com are accepted by default (`COMPANY_DOMAINS_ONLY=0`). Many small contractors run their business from one, and turning them away would lose exactly the owners this gift is for. One setting changes it.
- **The download page** follows the spec's page: one big button for the visitor's computer, the other platform as a small link, the size, a rough time at `ASSUMED_MBPS`, each file's SHA-256, and "Mac and Windows only" for anything else. The phone's "Share" button is replaced by the email.
- **`site/upload.sh` replaces the spec's R2 upload script.** It refuses files that failed an install test on a gating runner, or are unsigned, and checks each SHA-256. It copies with rsync, which resumes, and switches the site over by renaming `current.json`.
- **Tests:** unit tests, plus an end-to-end test of the real nginx config in Docker (nginx 1.22, the version in Debian 12), both in CI.
- **Still needed from Austin:** the domain, SMTP and SPF/DKIM, the follow-up wording, the link lifetime, the contact line, and the Pi's upload speed (site/README.md, "Bandwidth").

**2026-10-12 · Signing wired, untested until the secrets exist.** `release.yml`:
- **macOS:** imports the Developer ID certificate into a temporary keychain. Tauri then signs with the hardened runtime and entitlements, notarizes and staples the app. The workflow then signs, notarizes and staples the disk image, and prints how long each notarization took. Notarizing the multi-GB bundles (Phase 0 risk) gets measured on the first signed run.
- **Windows:** the optional Azure Artifact Signing path:
  - signs the app and installer through Tauri's `signCommand`, and the llama.cpp DLLs as they are collected;
  - removes the installer's signature, appends the model, and signs the installer again.
- Without secrets, file names say `UNSIGNED`.

**2026-10-14 · Website and contact line.** The domain is `tightline.systems`. About shows it as plain text, never a link. Austin: the free tools get no contact line of their own, so About has none, and the recovery card points salespeople to Austin directly.

**2026-10-14 · No email gate: downloads come from tightline.systems/schedule (Austin; supersedes the Pi-hosted, email-gated delivery of 2026-10-11).**
- The Tight Line website, a separate project served by Caddy on the workstation, already has the `/schedule` page and `get.tightline.systems` for the files. Its `/privacy` page promises that no email is needed to download.
- This repo's email service (`site/server`), its nginx and systemd files, its tests and `upload.sh` were removed before ever going live.
- `site/publish.sh` replaces them. It refuses a file that failed an install test on a gating runner, and a Mac build that isn't signed and notarized (the website says it is). It checks each SHA-256 and gives the files their client-facing names, with no variant letter and no UNSIGNED. Then it runs the website's `scripts/release.mjs --tool schedule`.
- **Kept, and still in use:** the website's own release script decides where files go and how many old versions stay. This repo only calls it, never imports from it, so the two projects stay separate.
- **Gone with the email gate:** no SMTP, Zoho, link expiry, follow-up wording or Pi. The spec's original download page (no forms, no cookies, no tracking) is back, now built by the website.

**2026-10-14 · Apple signing secrets added.**
- **Developer ID Application:** AUSTIN TYLER BURNES (J85MUL5KVY), valid until 2027-11-13. Mac signatures show Austin's legal name.
- **The App Store Connect API key** has the Developer role.
- **Where they live:** only in GitHub secrets, used by `release.yml`, and in Austin's password manager.
- Windows signing (Azure Artifact Signing, $9.99/month) is still Austin's open decision. The website hides its "publisher" step while it's pending.

**2026-10-15 · Notarizing the multi-GB bundles: the waiting moved out of the build job** (the Phase 0 risk, now measured). The first signed release run (38658533940) was the team's first-ever notarization.
- **Model off (16 MB):** Apple accepted the app after about 65 minutes and the disk image after 45.
- **C (0.82 GB):** the app took about 70 minutes, the disk image about 99.
- **A and B-E2B (2.5 and 3.4 GB):** after more than 4 hours, still waiting on the app.
- **Why that matters:** with Tauri notarizing inside the build, a job also has to build the app and then notarize the disk image, and GitHub stops any job at 6 hours.
- **The workflow now has three macOS jobs** (`tools/release/notary.sh`, `mac-signing.sh`):
  1. Build and sign the app, then submit it to Apple without waiting.
  2. Wait for Apple, staple the app, make and sign the disk image, then submit it.
  3. Wait again, then staple the disk image.
- **Each wait gets a job of its own,** so each has up to 5 h 30 min. Tauri never sees the notarization variables any more.
- **Riding out network blips:** B-E2B's app had waited 4 h 49 min when the runner briefly lost its network, and Tauri's status check gave up on that one error. `notary.sh` now checks the status once a minute until the deadline and ignores failed checks, and it retries a failed upload up to 3 times.
- **Stapling retries:** Apple's ticket server dropped the connection once, after accepting C's disk image (`stapler` error 68). Stapling now retries up to 6 times, a minute apart.
- Apple usually speeds up after a team's first submissions. The next signed run shows by how much.

## Phase 7

**2026-10-15 · The holdout was run once, then its failure patterns were fixed through new development fixtures.** The blind result is recorded in TEST_REPORT.md, Phase 7, and stays unchanged. Fixes follow two rules:
- Each failure pattern first gets a **new** handmade fixture, written fresh rather than copied from the holdout. That fixture must fail before the fix and pass after it.
- All development fixtures and generator seeds 0–1,999 must still pass.

Any later holdout check is reported as "after fixes, not blind".

**2026-10-15 · Fix 1: first names that are also day abbreviations** (holdout H03 "Dom", H17 "Mo"; new fixtures E15 and E16, with Mo, Dom and Lu across and down). `Sink::work` dropped every job cell of a person or site whose name `dates::parse` accepts as a day. The date reader accepts two-letter and Spanish short forms ("mo", "lu", "dom", "mar"), so a worker called Mo lost the whole week, silently. `dates::is_day_not_name` now rejects:
- real dates, Excel serials and single letters;
- English day words of three letters or more ("Mon", "Monday");
- full Spanish day names ("lunes", "domingo").

Short forms that are also first names stay names.

**2026-10-15 · Fix 2: day headers written as a letter plus a date, and spaced weeks** (holdout H12; new fixture C20).
- **The headers:** the date reader accepted a day **word** before a date ("Mon 10/12") but not a day **letter** ("M 10/12", "Th 10/15"). A two-week board headed that way was rejected as "not a schedule". A letter before a date now reads as that date. The letter itself isn't checked, since T can mean Tue or Thu.
- **The arrangement:** two weeks in one header row separated by a blank spacer column now count as side by side. The grid drops empty columns, so the gap is found in the file's own column numbers.

**2026-10-15 · Fix 3: a crew roster in columns beside the schedule** (holdout H05; new fixture C21). The reader already handled crew names heading columns of members, but only when the **whole row** held nothing else. In practice the roster often shares the schedule's header row after a blank column ("Crew | Mon … Fri | | Crew A | Crew B | Other"). That roster was missed, so crews weren't expanded into their members. The check now looks at the roster's own block of cells: the cells next to each other in the file, found in the file's own coordinates because the grid drops blank columns. The block may include an "Other" (or "Misc", "Extras" and so on) column, which isn't a crew.

**2026-10-15 · Fix 4: a roster block beside a list, on the list's header row** (holdout H20; new fixture C22). The "Technicians | Truck" names next to a long list, after blank columns, were read as a second person column. Each of the list's first rows then gained an extra worker who wasn't on that job.
- **The rule:** a block in the header row that is separated from the list by a blank column, has no days in it, and is headed by a roster title or a people header ("Techs", "Technicians") is a roster. Its whole block of cells is set aside, Truck column included.
- **The exclusion:** a block with days in its header is another week's schedule, as in two weeks side by side, each with its own Name column (C08).
- "Technicians" and "Installers" joined the people headers.
- **Kept narrow by the generator:** a first version also caught a "Laborer" placeholder that opened the second week of a side-by-side board, and the third week's repeated "Worker" header. That lost a week's names on 3 of the 2,000 development seeds. The rule now also needs the header row to have at least two header words to the block's left, at least one of them not a people header ("Date | Tech | Job"). Compared seed by seed with the build before the fix: none worse, one better (1713).

**2026-10-15 · Fix 5: month calendars, footer notes and short weeks** (holdout H10; new fixture C23, which also exposed the second bug).
- **Footer notes:** a note below the calendar ("Roof tear-off at Lowell moved to 11/30 - Dana knows.") was read as an entry for the last Sunday, with "Lowell moved to 11" as a person. The last week's entries now end at a blank row, whether the grid kept it as an empty row or dropped it (a gap in the file's own row numbers). They also end at a line alone in its row that reads like a sentence and whose "people" side doesn't look like names (score under 0.4). The score keeps long crew entries ("Alejandro A + Lázaro R + Salvador H - Job 2612 Hillcrest") that are alone in their row.
- **Short weeks:** a month's first or last week with only one or two days ("29 | 30" under Sun and Mon) wasn't recognized as a week, because a week needed three day numbers. Its entries went to the week above. Such a row now counts when it holds only those numbers and continues the full weeks.
- **Seed by seed:** the 2,000 development seeds have none worse and 9 calendars better.

**2026-10-15 · Fix 6: an order sheet ahead of the schedule, and a note under a job grid** (holdout H11; new fixtures C24 and C25).
- **Which sheet:** a materials or purchase list ("Item | Qty | For job | Vendor | Need by | Ordered by") reads like a schedule, with jobs, dates and first names. So the app asked which sheet to read.
  - A sheet whose header has two or more order-list words ("Item", "Qty", "Vendor", "Price" and the like) now never makes the choice a coin flip, and sorts after the other sheets.
  - Of several plausible sheets, the one named like a schedule ("Schedule", "Crew Schedule", "Calendar", "Week …") is chosen.
  - Two so named are still a question (C12: "Week 10-12" and "Week 10-19").
- **A note in the job column:** "Lot 12 walk-through with the owners Fri 9am." under a job grid became a job with nobody on it, with an unstaffed flag every day. A label with nothing across the week that reads as a sentence (five or more words, ending like one or naming a time of day) is now a note. A job left empty all week ("Garcia garage") still reads, and is flagged unstaffed.
- **Seed by seed:** none of the 2,000 development seeds got worse, counting flags as well as assignments, and 9 are better.

**2026-10-15 · Checking the Phase 7 fixtures.** The rule checker (`_make/check_flags.py`, which derives flags from FORMAT.md's rules independently of the app) agrees with all eight new fixtures (`_make/hints/` gives it their date windows, rosters and job rows). `_make/build_C24.py` makes C24's workbook byte-identically; the others are hand-written CSVs. While deriving C23's flags, `spotter-check run` turned out to use the default seven-day week, not a fixture's settings. Flags were taken from the fixtures run instead.

**2026-10-15 · Fix 7: a crew's schedule row taken for a roster row** (found by the post-fix holdout check on H05; new fixture C26).
- **What happened:** with the roster in columns beside the grid (fix 3), the schedule row "Crew A | Haskell Farm × 5" was read as a roster row ("Crew A: these members"). A farm named after a family scores as a person's name. Because the grid drops the blank column, the real roster's names sat right next to it. So that crew's whole week was set aside.
- **The fix:** a roster row now takes only the cells next to the crew name in the file, stopping at a blank column. Its members must also be distinct: the same "name" across the row is a job.
- **Checks:** all development fixtures pass, and the seeds are unchanged.

**2026-10-15 · Apple's notarization queue for a new team, measured** (`notary-status`, Apple's Notary REST API).
- **Still waiting:** the first signed run's A and B-E2B apps were still "In Progress" at Apple after about 9 hours. The second run's four apps, including the 16 MB build with no model, were still waiting after 3 h 45 min, apparently queued behind them.
- **Accepted:** the first run's small builds, about 1 hour for each app and 45–100 minutes for each disk image.
- **Waiting doesn't block a release:**
  - The waits are in jobs of their own (5 h 30 min each).
  - `notary-status` shows what Apple is doing.
  - Re-running the failed jobs once Apple accepts reuses the submission IDs and app files already in the run, without uploading again (RELEASE.md, "When notarization waits").
- Apple normally speeds up after a team's first submissions. The next release shows by how much.

**2026-10-15 · Final benchmark and recommendation (Phase 7).** CI run 38703867149 (commit 29b9363): all 30 benchmark jobs and the report passed. No variant passes the selection rule on any runner, so the rule's answer is **model off**. The best scorer, B-E2B, has 16.2% false merges against the 1% limit. The holdout agrees: 96 wrong merges per 1,000 unseen schedules for B-E2B, 6 for A, 0 for C and for model off. MODEL_REPORT.md sets out the choice for Austin: model off is recommended, with C, A and B-E2B as the alternatives and their costs. The Phase 7 gate ("reports complete; Austin picks the variant") waits only on his pick.

**2026-10-15 · Austin picked the variant: model off. The Phase 7 gate passed.**
- **What ships:** the app with no model: a 16 MB Mac disk image and a 223 MB Windows installer, most of it Microsoft's WebView2.
- **The model layer stays in the code** (`llm/`, `SPOTTER_VARIANT`), so a model that later passes the selection rule can ship as a variant without changing the app.
- **Builds:** `release.yml` builds `["off"]` by default; the other variants are one input away.
- **The words a build without a model shows** describe how it works, not that something is switched off:
  - The status line says "I'll ask you about any name or site I'm unsure of".
  - About explains that unsure names and sites come as Yes/No questions and sure matches show as a chip you can undo.
  - The free-text paste message no longer mentions smart matching.
  - Builds with a model keep their "Smart matching" wording.
- **For the website project (not changed from here):** `site/data/tools/schedule.json` needs its size (16 MB Mac, 223 MB Windows). Its `support.slow` text says smart matching loads in the background, which a build without a model never does.

