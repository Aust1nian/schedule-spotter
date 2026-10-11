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
