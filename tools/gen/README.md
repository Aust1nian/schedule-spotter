# spotter-gen: seeded schedule generator (v1)

A dev-only tool for the spec's "Generator, fuzzing, holdout" section. For each seed it builds a random contractor schedule with known answers (the truth), renders it into a messy real-world file, and writes the answer the pipeline must produce. The answer is computed from the truth and the rendering decisions, never from a pipeline. It is independent of `core/`.

## Run

From the repo root (quote the path, it contains `&`):

```sh
uv run --project tools/gen spotter-gen --seeds 0-1999 --out /tmp/gen
uv run --project tools/gen spotter-gen --seeds 0-1999 --out /tmp/gen --stats       # print distributions
uv run --project tools/gen spotter-gen --seeds 5,7,100-120 --out /tmp/gen --variant shuffle
uv run --project tools/gen pytest                                                   # self-checks (~20 s)
```

| Option | Meaning |
|---|---|
| `--seeds` | `0-1999`, `5`, or a comma list of both. Holdout seeds are 1,000,000–1,000,999 (`category: "H"`); never run them in the dev loop. |
| `--out DIR` | Writes `DIR/<seed>/input.<ext>` (or `paste.txt`), `expected.json` and `truth.json`. An existing `DIR/<seed>/` is replaced. |
| `--variant` | `none` (default), `shuffle`, `rename`, `blank`, `transpose`. See Variants. |
| `--clean-rate` | Share of fully clean schedules (default 0.2). |
| `--stats` | Print layout, format, encoding, date-format, tier, planted-problem and corruption distributions. |
| `--quiet` | No timing line on stderr. |

**Speed:** 2,000 seeds take about 16 s on one core, where the budget is 90 s. **Determinism:** each seed's output is byte-identical whatever the run order, process, or `PYTHONHASHSEED`. All randomness comes from `random.Random(seed)`, plus derived streams seeded with strings, such as `Random(f"names:{seed}")`. Zip-based files (xlsx, ods, docx) are re-zipped with fixed timestamps and fixed document dates. No network is used at generation time.

## Outputs

- **`expected.json`** follows `fixtures/FORMAT.md`:
  - `assignments` are the records exactly as written in the file (crews expanded, notes split off), sorted.
  - `flags` and `info` follow the "Flag semantics" section. Display names are the longest rendered spelling of each identity.
  - `tier` is `script` or `model`.
  - `settings.today` is `2026-10-10`, and `settings.workdays` is the truth workweek.
  - `assignments_exact`, `flags_exact` and `info_exact` are all `true`.
  - `category` is `"C"` for dev seeds and `"H"` for holdout seeds.
- **`truth.json`** holds:
  - `people`: id, canonical name, kind, crew, `aliases` actually rendered, `display`, and `on_roster`.
  - `sites`: id, name, kind, internal flag, rendered `aliases`, and `display`.
  - `crews`, `workweek`, `window`, and every true `assignment` by id.
  - `planted` and `planted_info`, by id.
  - `tier` and `tier_reasons`.
  - `render`: every layout, arrangement, format, encoding, delimiter, line-ending, date-format, header, join and calendar decision.
  - `corruptions`: file-level switches, plus every rendered alias with its form.
  - `date_labels`: every date label written, with its ISO date.

  The scoring harness uses the alias groups to score by identity and to answer `same_person` / `same_site` questions correctly.

## Pipeline per seed

1. **Plan.** Every layout, format and style decision is drawn first, in a fixed order, so variants change only what they target.
2. **Truth.** Draws a company:
   - **People:** 4–60, mostly employees. 25% of companies have 1–2 placeholders (`Helper 1`, `Laborer`, `Temp`, `New guy`, `Foreman`). 22% have 1–2 subcontractors (`Delta Electric LLC`, `Apex HVAC Inc`, …).
   - **Jobs:** 2–25, styled as addresses (`1420 Johnson St`), customer jobs (`Smith remodel`), job numbers (`J-1042 Oak Ridge`) or commercial jobs (`Lowe's Plaza TI`). 38% of companies also have Shop, Yard, Office or Warehouse.
   - **Dates:** 1–4 consecutive weeks starting on a Monday within ±6 weeks of 2026-10-12. The workweek is Mon–Fri 70% of the time and Mon–Sat 30%.
   - **Statuses:** off, pto, vacation, sick, holiday, out. There are also whole vacation weeks.
   - **Times:** optional, on 35% of per-record lists, with all rows or about half the rows timed.
   - **Crews:** only in crew layouts.

   Every active job-day is staffed and every employee has work or a status every working day, unless a problem is planted. Planted problems:

   | Planted (non-clean schedules) | Rate | Count |
   |---|---|---|
   | double_booked | 50% | 1–3 (with times: overlapping or one entry without a time) |
   | assigned_while_off | 35% | 1–2 |
   | not_booked | 40% | 1–4 person-days; 30% of those also have one employee idle all window (shown as a blank grid row or in a roster block) |
   | unstaffed | 40% | 1–2, half explicit (`TBD`, `open`, `?`, `need guy`, `need 1`), half inferred (a blank job-day) |
   | fully clean | 20% (`--clean-rate`) | none of the four |

   Info items are planted in any schedule, and are never created by accident:

   | Info | Rate | Count |
   |---|---|---|
   | internal_plus_site | 20% (when an internal site exists) | 1 |
   | duplicate_entry | 10% (per-record lists and calendars) | 1 |
   | split_day | 40% of timed lists | 1 |
   | subcontractor_multi_site | 40% (when a sub exists) | 1 |

   The generator re-analyzes every case and fails loudly if the flags or info differ from what was planted.
3. **Render.**
   - **Layout:** long_list 30, person grid 24, job grid 22, crew_based 12, and calendar_month 40 when the span is 4 weeks. Grids are day-major (transposed) 25% of the time.
   - **Arrangement:** multi-week grids are single 30, stacked 40, side_by_side 30. Multi-week lists and crew layouts are single 60, stacked 40.
   - **Format:** csv 45, xlsx 22, tsv 7, txt 8, xls 5, ods 5, docx 5, paste 3.
   - **Encoding** (text formats): UTF-8, UTF-8 BOM, Windows-1252, or UTF-16 LE with BOM (half of .txt files).
   - **Delimiter and line ending:** comma, semicolon, tab or pipe, with CRLF, LF or CR.
   - **Date formats** (spec stage 5):
     - With a full date: ISO, M/D/YYYY, M/D/YY, M/D, `Oct 12`, `12-Oct`, `Mon 10/12`.
     - Day-first D/M/YYYY, only when some day is over 12.
     - Excel serials (lists only) and native date cells (spreadsheets).
     - Weekday-only (`Mon`, `Monday`, `M T W Th F Sa`, `Lunes`), always under a "Week of" line or a merged two-row header.
     - Calendars use day numbers or M/D.
   - **Headers:** none (lists, 15%), plain, synonyms (Employee/Tech/Crew/Worker, Job/Site/Project/Address/Customer, …), two-row merged headers (xlsx/xls/ods/docx, 15%), title rows above, and Spanish (Empleado/Obra/Fecha/Lunes…).
   - **Multi-name cells** are joined with `, `, ` & `, ` / `, ` and `, ` + `, a newline, or ` y ` (Spanish files), sometimes as "A, B & C".
4. **Corrupt.** Each file-level switch has its own probability and is recorded in `truth.json`.

   | Corruption | Rate |
   |---|---|
   | nickname (spec table only: Mike/Michael, Bob/Robert, Bill/William, Jim/James, Tony/Anthony, Pepe/José, Paco/Francisco, Chuy/Jesús, Nacho/Ignacio, Lalo/Eduardo, Memo/Guillermo, Beto/Alberto, Beto/Roberto) | 0.30 |
   | initials (`Mike B`, `M. Baker`) | 0.20 |
   | partial names (first or last only, mixed with full) | 0.10 |
   | typos (`Micheal`, `Pero`) | 0.08 |
   | dropped accents | 0.30 |
   | name case | 0.15 |
   | site variants (`Street`/`St`/`St.`) | 0.25 |
   | site case | 0.10 |
   | hard site variants (`Johnson St`, `Smith job`, `Oak Ridge`) | 0.03 |
   | look-alike addresses (1420 vs 1240) | 0.02 |
   | title rows | 0.40 |
   | footer lines | 0.20 |
   | totals row | 0.15 |
   | merged-cell blanks (lists only; real merges in xlsx/xls/ods) | 0.12 |
   | stray spaces | 0.20 |
   | mixed-case status words | 0.30 |
   | Spanish headers, days and status words | 0.08 |
   | notes in cells (`Johnson - bring ladder`, `Johnson (call first)`) | 0.15 |

   The company naming style is FULL 35, FIRST 25, FIRST+initial 15, NICK 8, initial+LAST 7, LAST 5. People who share a first name get last initials.
5. **Expect.** See Outputs.

## Variants

| Variant | What changes | What must not change |
|---|---|---|
| `shuffle` | Data-row order in lists (long_list, crew list). Merged-blank groups move as units. Other layouts are untouched (`variant_applied: false`). | `expected.json`, byte for byte |
| `rename` | Every employee gets a fresh name everywhere (fresh surnames), with the same nickname, accent and shared-first-name structure. Placeholders and subcontractor companies keep their names. | Flags and info by identity. `tier` can change in rare cases (12/2,000), because name collisions decide tier. |
| `blank` | 1–3 blank rows (between blocks, or between data rows of a table, never inside a merged group or a header) and 1–2 blank columns (left margin or between side-by-side blocks, never inside a row). | `expected.json`, byte for byte |
| `transpose` | person_by_day ↔ day_by_person, job_by_day ↔ day_by_job. Arrangement and everything else stay the same. | `expected.json` except `layout` |

## Tier

`tier` is `"script"` unless a rendered spelling needs more than these deterministic rules:

- normalization (case, spacing, accents, periods);
- the spec nickname table;
- initial compatibility (`Mike B`, `M. Baker`);
- first-name-only and last-name-only spellings.

Each rule applies only when the spelling is compatible with exactly one person on the roster.

The tier becomes `"model"` (reasons are in `truth.tier_reasons`) for:

- typos;
- any spelling compatible with two or more roster people (colliding initials, `Beto` with both an Alberto and a Roberto);
- dropped second surnames (`José García` for `José García López`);
- hard site variants;
- look-alike addresses.

About 15% of dev seeds (0–1999) are model tier.

## Conventions the expectations rely on

- **Times without am/pm** (`7-3:30`, `12-4`, `1-4`) use business hours. A start hour of 6–11 is am, 12 is noon, and 1–5 is pm. An end at or before its start is pm. v1 has no overnight shifts, and split days always have a gap between the two entries, never exactly touching times.
- **Calendar entries** are `person<sep>site` with sep ` - ` (60%), `: ` or ` @ `. A status entry is `person<sep>status`, and unstaffed is `TBD<sep>site`. Day numbers wrap into the next month under a `Sep – Oct 2026` title.
- **Person grids:**
  - Two sites in one cell are joined by the site joiner.
  - Assigned-while-off is a status word and a site in one cell (`PTO / Smith remodel`).
  - Explicit unstaffed is a pseudo-row labelled with the marker (`TBD`, `OPEN`, `?`, …), with the job names in its cells.
  - Off may be written `x`, `X`, `—`, `-` or `n/a`, only when it is alone in the cell.
- **Job grids:**
  - Days outside a job's span carry an inactive marker (`x`, `—`, `n/a`, `closed`, `done`, `no work`).
  - A blank job-day is a planted inferred unstaffed day.
  - Statuses are pseudo-rows labelled with the status word.
- **Crew layouts:**
  - Days outside spans also get an inactive marker.
  - Every axis date has at least one record.
  - Only crew members are ever unbooked.

  This keeps FORMAT's job-grid rule and its span rule in agreement, and keeps "roster block" versus "everyone seen" from mattering.
- **Notes** are only added to single-site cells. Site and person names never contain separators, ` - ` or parentheses (enforced by tests).

## Not generated in v1

- AM/PM parts and half days, overnight shifts, and work on non-working days.
- `days_off`.
- Free-text paste (model task T5).
- Error fixtures (empty, binary or unreadable files).
- `.xlsm`, `.xlsb` and `.numbers`.
- Two-sheet workbooks that need `pick_sheet`. Extra sheets are empty.
- `ask` / `must_not_ask` lists. The harness derives questions from the alias groups in `truth.json`.

## Self-checks (`tests/`)

- **Determinism:**
  - same seed in-process, byte for byte;
  - CLI runs in two processes, in opposite seed order with different `PYTHONHASHSEED`;
  - format shares (xlsx ≤ 25%).
- **FORMAT shape:** every expected.json passes a strict FORMAT.md checker.
- **Clean cases:** clean cases have no flags.
- **Flags and info:** flags and info equal the planted lists. Display names are the unique longest alias.
- **Dates:**
  - every date's weekday is checked with `datetime`, including every rendered label and "Week of" line;
  - day-first appears only with a day over 12.
- **Re-read:** every expected person and site string is found in the file, re-read with stdlib `csv` or openpyxl, plus odfpy and python-docx for .ods and .docx. `.xls` has no reader among the allowed deps; it was checked ad hoc with xlrd, and all files passed.
- **Variants:** variant properties over 150 seeds.
- **Timing:** 200 seeds under 9 s, the 90 s / 2,000 budget pro rata.
- **Word lists:** sizes, cp1252-safe, separator-free.
