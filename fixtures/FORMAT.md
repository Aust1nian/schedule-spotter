# Fixture format and flag semantics

The contract shared by handmade fixtures, the holdout, the generator, and the pipeline. If code and this file disagree, one of them is a bug. Fix whichever is wrong and log the change in DECISIONS.md. Fixture expectations that change must also be logged in TEST_REPORT.md.

## Folder layout

```
fixtures/handmade/<ID>-<slug>/
  README.md        one line: the failure mode this fixture isolates
  input.<ext>      the schedule (.csv .tsv .txt .xlsx .xls .xlsm .xlsb .ods .numbers .docx), or paste.txt
  expected.json    what the pipeline must produce
```

- **IDs** are the category letter plus two digits: `A01`, `C14`, `M08`. The slug is short kebab-case: `A07-utf16-le-unicode-text`.
- **Holdout fixtures** live in `fixtures/holdout/` with IDs `H01`–`H20`. The dev loop never runs them.
- **Input files** use the real format named by their extension. A file renamed on purpose (category L, "binary renamed .csv") says so in its README.

## expected.json

Every key except `category` and `tier` is optional. A missing key means "not checked".

```json
{
  "category": "C",
  "tier": "script",
  "input": "input.csv",
  "paste": false,
  "settings": { "today": "2026-10-10", "workdays": ["mon","tue","wed","thu","fri"], "days_off": [] },
  "layout": "job_by_day",
  "arrangement": "single",
  "assignments": [],
  "assignments_exact": true,
  "flags": [],
  "flags_exact": true,
  "info": [],
  "info_exact": false,
  "ask": [],
  "must_not_ask": [],
  "answers": [],
  "after_answers": { "assignments": [], "flags": [], "info": [] },
  "error": null,
  "notes": []
}
```

| Key | Meaning |
|---|---|
| `category` | `A`–`N`, or `H` for holdout |
| `tier` | `"script"`: must pass with the model off. `"model"`: needs the model. With the model off, the questions it would have settled must be asked instead. |
| `input` | File name inside the folder |
| `paste` | `true`: the input is read as pasted text (Cmd/Ctrl+V), not opened as a file |
| `settings.today` | Anchors year inference. Default `"2026-10-10"`. |
| `settings.workdays` | Working weekdays, from `mon tue wed thu fri sat sun`. Default: all seven (Austin's decision). Fixtures whose data is a Mon–Fri week should say `["mon","tue","wed","thu","fri"]`. |
| `settings.days_off` | ISO dates off for everyone |
| `layout` | `long_list`, `person_by_day`, `day_by_person`, `job_by_day`, `day_by_job`, `crew_based`, `calendar_month` |
| `arrangement` | `single`, `stacked` (weeks one under another), `side_by_side` |
| `assignments` | What was read; see Assignment below |
| `assignments_exact` | `true` (default): the read assignments must equal this list as a multiset. `false`: they must contain it. |
| `flags`, `flags_exact` | Red/amber flags, default exact. Checked from Phase 2 on. |
| `info`, `info_exact` | Information items, default not exact. Checked from Phase 2 on. |
| `ask` | Questions that must be asked (model off, before any answers) |
| `must_not_ask` | Questions that must not be asked |
| `answers` | `[{"question": {...}, "answer": "yes" \| "no" \| "<option text>"}]`, applied in order |
| `after_answers` | Expectations after `answers` are applied |
| `error` | `"empty_file"`, `"not_a_schedule"`, `"unreadable"` or `"save_failed"`. When set, nothing else is checked except `notes`. |
| `notes` | Substrings that must appear in the pipeline's notes to the user, case-insensitive |

### Assignment

One record per person per date, as written in the file:

```json
{"person": "Mike B", "date": "2026-10-12", "site": "1420 Johnson St"}
{"person": "Mike B", "date": "2026-10-12", "site": "1420 Johnson St", "start": "07:00", "end": "15:30"}
{"person": "Mike B", "date": "2026-10-12", "site": "Johnson", "note": "bring ladder"}
{"person": "Luis", "date": "2026-10-13", "status": "pto"}
{"person": "Ana", "date": "2026-10-13", "site": "Smith remodel", "part": "am"}
{"date": "2026-10-14", "site": "Smith remodel", "unstaffed": true}
```

- **`person` and `site`** are the text as written, after:
  - trimming;
  - collapsing runs of whitespace to one space;
  - moving a trailing note to `note`: text after `" - "`, or inside a trailing `(...)`.

  They are not merged or renamed; matching is checked through flags, not here. Comparison is case-insensitive and accent-sensitive, so `José` ≠ `Jose`.
- **Multi-name cells** become one record per name. `"Mike, Jose"` gives two records.
- **A grid cell with two sites** (`"Johnson / Smith"`) gives two records.
- **`status`** is one of:

  | Value | Written as |
  |---|---|
  | `off` | off, OFF, x, —, -, n/a, day off |
  | `pto` | PTO |
  | `vacation` | vac, vacation |
  | `sick` | sick |
  | `holiday` | holiday |
  | `out` | out |

  Spanish equivalents map the same way: descanso → off, vacaciones → vacation, enfermo → sick, libre → off. A status record has no site. It keeps a cell note ("PTO (dentist)" → `note: "dentist"`).
- **"x"** is `off` in a person's row and an inactive marker in a job-grid cell, where it produces no record.
- **`unstaffed: true`** marks a job-day explicitly written as TBD, open, ?, need guy or need 1. It has no person.
- **`part`** is `am`, `pm` or `half`, for "AM", "PM", "AM only" and "half day". AM and PM sub-columns under one day give one record per cell, even when both name the same site.
- **`start` and `end`** are 24-hour `HH:MM`. If `end` < `start`, the shift ends the next day.
- **Crews:** crew names in a schedule are expanded to their members from the crew roster block. Each member gets the crew's site, and the crew name never appears as a person.
- **Internal locations** (shop, yard, office, warehouse) are ordinary `site` values. Travel rows ("drive to Fresno") are `site` values too.
- **Not assignments:** totals rows, footers, signature lines, title rows and header rows produce no records.
- **Notes column:** a column headed Notes, Comments or similar fills `note` for that row's records. Notes from the cell come first, separated by "; ".
- **Emoji and symbols** are kept as written ("Dave Kim 🔧").
- **Merged-cell blanks:** a label written once over several rows, whether from real .xlsx merges or the same pattern exported to text, fills down silently when the pattern occurs at least twice. A single occurrence fills down and asks `fill_down`. Blank day cells in a grid are never filled.

### Flag

```json
{"type": "double_booked", "person": "Mike Baker", "date": "2026-10-13", "sites": ["1420 Johnson St", "Smith remodel"]}
{"type": "assigned_while_off", "person": "Luis Ortiz", "date": "2026-10-13", "sites": ["Smith remodel"], "status": "pto"}
{"type": "not_booked", "person": "Ana Ruiz", "date": "2026-10-14"}
{"type": "unstaffed", "site": "Smith remodel", "date": "2026-10-14"}
```

- **`person`** is the canonical display name after merges: the longest name among the merged spellings.
- **`sites`** are canonical site names, compared as a set. A merged site's canonical name is likewise its longest spelling.

### Info item

```json
{"type": "duplicate_entry", "person": "Jose", "date": "2026-10-12", "sites": ["Smith remodel"]}
{"type": "split_day", "person": "Jose", "date": "2026-10-12", "sites": ["Smith remodel", "Johnson"]}
{"type": "internal_plus_site", "person": "Jose", "date": "2026-10-12", "sites": ["Shop", "Johnson"]}
{"type": "subcontractor_multi_site", "person": "ABC Electric LLC", "date": "2026-10-12", "sites": ["Johnson", "Smith remodel"]}
```

### Question

Matched by `type` plus every field given. Text comparison is case-insensitive. `a`/`b` pairs are unordered.

| Type | Fields | Asked when |
|---|---|---|
| `same_person` | `a`, `b` | Two names might be one person, and neither the rules nor the model can settle it |
| `same_site` | `a`, `b` | Two site strings might be one site |
| `pick_sheet` | `options` (optional; compared as a set) | A workbook has two or more sheets that each look like a schedule |
| `pick_layout` | | The top two layout readings score too close to call |
| `weekday_mismatch` | `text`. The date is used meanwhile. Answer "yes" keeps the date; "no" uses the written weekday in that week. | A weekday contradicts its date ("Mon 10/13/2026" is a Tuesday) |
| `fill_down` | `column` (optional) | Blanks under a value might mean "same as above", but the block has no merged-cell pattern |
| `column_role` | `header` (optional) | Column-role scores tie (model task T4; asked when the model is off) |
| `cell_meaning` | `text` | A cell can't be classified (T3) |
| `paste_as_table` | | Free-form pasted text with the model off: the app asks for a table instead |

## Flag semantics (Phase 2 rules)

These definitions are normative for fixtures and the generator.

**Window.** The dates the schedule covers. `crew_based` schedules follow the rules of the grid or list they're drawn as.
- **Grids** (`person_by_day`, `day_by_person`, `job_by_day`, `day_by_job`): every date on the day axis, including a day column nobody filled in.
- **Lists:** every date from the earliest to the latest date present.
- **`calendar_month`:** from the first to the last date that has any entry.
- **Weekday-only headers** ("Mon", "Tue" under "Week of 10/12/2026"): the dates those headers name. With no "Week of" anywhere (free text, bare weekday columns), weekday names mean the current week, or the coming week when today is a Saturday or Sunday.

**Working dates.** Window dates whose weekday is in `settings.workdays` and that are not in `settings.days_off`.

**People.**
- **Employees:** everyone named in the schedule or in a roster block.
- **Placeholders:** role names such as "Helper 1", "Foreman", "Laborer", "New guy", "Temp".
- **Subcontractors:** company names, marked by tokens like LLC, Inc, Co., Electric, Plumbing, HVAC, Roofing, Concrete, Drywall, Painting, Construction, Services, Contractors.
- **Roster:** the employee roster block (titled Roster, Employees, Staff and the like) if one exists, otherwise every employee seen in the file. A crew list ("Crew A: …") defines crews and is not a roster. Placeholders and subcontractors are never on the roster.

**Sites.**
- **Internal:** shop, yard, office, warehouse, and travel rows ("drive to …", "travel").
- **Jobs:** every other site.

**`double_booked`.** An employee has work at two or more different sites (after merges) on one date, and either:
- at least one of those entries has no time and no `part`; or
- two of the entries' times overlap.

These cases are information items, not flags:

| Case | Info type |
|---|---|
| All entries have times and none overlap, or they have different parts (AM vs PM) | `split_day` |
| Exactly one of the sites is internal (shop + one job), with or without times | `internal_plus_site` |
| A shop-only combination, e.g. Shop + Yard | `internal_plus_site` |
| The same person at the same site twice on one date (an AM record plus a PM record at one site is a full day, not a duplicate) | `duplicate_entry` |

A shop plus two job sites is still `double_booked` for the two jobs. The flag's `sites` lists the conflicting job sites only, and no `internal_plus_site` item is added.

Placeholders and subcontractors never get `double_booked`, `assigned_while_off` or `not_booked`. A subcontractor at two or more job sites on one date is `subcontractor_multi_site` info.

**`assigned_while_off`.** An employee has a status record (any status) and a work record on the same date. A status with a `part` (`"off PM"`) does not conflict with work that has the other part, or with work whose time falls only in the other half of the day.

**`not_booked`.** A roster employee with no work record and no status record on a working date. Amber, not red.

**`unstaffed`.**
- **Explicit:** a job-day marked TBD, open, ?, need guy or need 1 is always flagged, working day or not.
- **Inferred:** a job active on a working date with nobody assigned.
  - **Active dates, job grids** (`job_by_day`, `day_by_job`): every working date in the window, except cells marked inactive (x, —, n/a, "no work", "closed", "done").
  - **Active dates, other layouts:** from the job's first to its last assigned working date within each Monday–Sunday week. Explicitly unstaffed days (TBD) count as assigned for this range.
- **Never unstaffed:** internal locations and status pseudo-rows (an "OFF" or "PTO" row in a job grid). A job with only placeholders or subcontractors on it is staffed.

**Red vs amber.** `double_booked` and `assigned_while_off` are red. `not_booked` is amber. `unstaffed` is shown under the grid, striped. Info items are never red.

**Clean schedule.** No `double_booked` and no `assigned_while_off`. A clean schedule may still have `not_booked` or `unstaffed` if those were planted.
