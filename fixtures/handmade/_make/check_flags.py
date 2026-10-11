#!/usr/bin/env python3
"""Recompute flags and info items for every handmade fixture and compare them
with the hand-written `flags` / `info` in its expected.json.

The rules are FORMAT.md's "Flag semantics (Phase 2 rules)", implemented here
from scratch. Nothing is imported from core/ or from the build helpers.

Inputs per fixture:
  expected.json         assignments, settings, layout (and after_answers)
  _make/hints/<ID>.json facts about the input that assignments alone can't
                        carry: the grid's day axis ("window"), a roster block
                        ("roster"), the job rows of a job grid ("jobs"),
                        job-grid cells marked inactive ("inactive"), merges the
                        deterministic matcher (spec stage 7) must make
                        ("same_person", "same_site": lists of [a, b] pairs), and,
                        for a fixture whose expected assignments are only a
                        sample, the full truth the flags come from
                        ("assignments").

Readings chosen where FORMAT.md is silent (fixtures avoid the cases that the
other plausible reading would decide differently):
  - Times are half-open [start, end): 7-12 and 12-4 touch but do not overlap.
  - end < start ends the next day; end == start is a 24-hour shift.
  - double_booked compares records that share a date. An overnight shift is
    not compared with the next date's records (fixtures keep the next morning's
    work after the shift ends, so the other reading agrees).
  - An AM or PM part is the half day 00:00-12:00 / 12:00-24:00. "half" (which
    half unknown) behaves like no part: it conflicts with anything that day.
  - duplicate_entry: two records at one site whose times or parts overlap
    (no time and no part counts as overlapping). AM + PM at one site is a full
    day, not a duplicate.
  - Unstaffed, non-job-grid layouts: explicit unstaffed days on working dates
    count as assigned when finding a job's first and last day in a week.

Matching (spec stage 7, categories E and F):
  - Before any answers, names and sites are joined only where they normalize
    alike (case, whitespace) or a hint pair joins them: the deterministic
    merges. A pair the matcher is unsure of stays separate and is asked.
  - `after_answers` applies the hint merges plus every same_person / same_site
    answered "yes"; a "no" keeps the pair apart. Those answers never change
    what was read, so an `after_answers` block without `assignments` uses the
    top-level ones.
  - The canonical display name is the longest spelling in a group. FORMAT.md
    names no tie-break, so a flag or info item may not name a group whose
    longest spellings tie and differ beyond case ("Jose Garcia" / "José García",
    "Micheal Baker" / "Michael Baker"); fixtures keep flags off such names.
  - Question bookkeeping: an asked same_person / same_site pair names each side
    by its canonical spelling before answers, the two sides are not already
    merged, and no hint merges it; must_not_ask and ask never overlap; each
    answered same_person / same_site question is one of the asked ones; a "no"
    is never undone by other merges.

Also validates every expected.json against FORMAT.md's shape.

Usage: python3 check_flags.py [FIXTURE_ID ...]
Exit code 1 on any mismatch or validation error.
"""
from __future__ import annotations

import datetime as dt
import itertools
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
HINTS = HERE / "hints"

WEEKDAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
GRIDS = {"person_by_day", "day_by_person", "job_by_day", "day_by_job"}
JOB_GRIDS = {"job_by_day", "day_by_job"}
LAYOUTS = GRIDS | {"long_list", "crew_based", "calendar_month"}
ARRANGEMENTS = {"single", "stacked", "side_by_side"}
STATUSES = {"off", "pto", "vacation", "sick", "holiday", "out"}
PARTS = {"am", "pm", "half"}
FLAG_TYPES = {"double_booked", "assigned_while_off", "not_booked", "unstaffed"}
INFO_TYPES = {"duplicate_entry", "split_day", "internal_plus_site", "subcontractor_multi_site"}
QUESTION_TYPES = {"same_person", "same_site", "pick_sheet", "pick_layout", "weekday_mismatch",
                  "fill_down", "column_role", "cell_meaning", "paste_as_table"}
KEYS = {"category", "tier", "input", "paste", "settings", "layout", "arrangement", "assignments",
        "assignments_exact", "flags", "flags_exact", "info", "info_exact", "ask", "must_not_ask",
        "answers", "after_answers", "error", "notes"}

INTERNAL = re.compile(r"\b(shop|yard|office|warehouse)\b|^\s*(drive to|travel)\b", re.I)
PLACEHOLDER = re.compile(r"^(helper|foreman|laborer|labourer|new guy|temp|apprentice)(\s*#?\d+)?$", re.I)
SUBCONTRACTOR = re.compile(
    r"\b(llc|inc|co\.|electric|plumbing|hvac|roofing|concrete|drywall|painting|construction|services|contractors)\b"
    r"|\bco\.?$", re.I)


def norm(s: str) -> str:
    """Case-insensitive, accent-sensitive, whitespace-collapsed (FORMAT.md)."""
    return " ".join(str(s).split()).casefold()


def kind(name: str) -> str:
    n = " ".join(name.split())
    if PLACEHOLDER.match(n):
        return "placeholder"
    if SUBCONTRACTOR.search(n):
        return "subcontractor"
    return "employee"


def is_internal(site: str) -> bool:
    return bool(INTERNAL.search(site))


def minutes(hhmm: str) -> int:
    h, m = hhmm.split(":")
    return int(h) * 60 + int(m)


def interval(rec):
    """Time interval in minutes, or None. AM/PM parts map to half days."""
    if rec.get("start") and rec.get("end"):
        s, e = minutes(rec["start"]), minutes(rec["end"])
        if e <= s:
            e += 1440  # ends the next day
        return (s, e)
    part = rec.get("part")
    if part == "am":
        return (0, 720)
    if part == "pm":
        return (720, 1440)
    return None  # no time, no part, or "half" (which half is unknown)


def overlaps(a, b) -> bool:
    ia, ib = interval(a), interval(b)
    if ia is None or ib is None:
        return True  # an entry with no time and no part conflicts with anything that day
    return ia[0] < ib[1] and ib[0] < ia[1]


def daterange(a: str, b: str):
    x, y = dt.date.fromisoformat(a), dt.date.fromisoformat(b)
    while x <= y:
        yield x.isoformat()
        x += dt.timedelta(1)


def weekday(iso: str) -> str:
    return WEEKDAYS[dt.date.fromisoformat(iso).weekday()]


def week_start(iso: str) -> str:
    x = dt.date.fromisoformat(iso)
    return (x - dt.timedelta(x.weekday())).isoformat()


class Canon:
    """Canonical display names: the longest spelling among those that normalize
    alike or that a merge hint joins (FORMAT.md: "the longest name among the
    merged spellings")."""

    def __init__(self, names, merges=()):
        parent = {}

        def find(k):
            parent.setdefault(k, k)
            while parent[k] != k:
                parent[k] = parent[parent[k]]
                k = parent[k]
            return k

        spellings = [" ".join(n.split()) for n in names] + [" ".join(n.split()) for pair in merges for n in pair]
        self.seen = {norm(n) for n in names}  # spellings in the data (not only in merge pairs)
        for n in spellings:
            find(norm(n))
        for pair in merges:
            ks = [find(norm(n)) for n in pair]
            for k in ks[1:]:
                parent[k] = ks[0]
        best = {}
        for n in spellings:
            r = find(norm(n))
            if r not in best or len(n) > len(best[r]):
                best[r] = n
        self.best = {k: best[find(k)] for k in parent}
        self.group = {k: find(k) for k in parent}
        # canonical names whose group has another spelling of the same length
        # that differs beyond case and spacing: "longest" picks neither
        self.tied = {norm(best[find(norm(n))]) for n in spellings
                     if len(n) == len(best[find(norm(n))]) and norm(n) != norm(best[find(norm(n))])}

    def __call__(self, n):
        return self.best.get(norm(n), " ".join(n.split()))

    def same(self, a, b) -> bool:
        return self.group.get(norm(a), norm(a)) == self.group.get(norm(b), norm(b))


def compute(assigns, settings, layout, hints):
    settings = settings or {}
    workdays = set(settings.get("workdays") or WEEKDAYS)
    days_off = set(settings.get("days_off") or [])

    persons = Canon([a["person"] for a in assigns if a.get("person")] + list(hints.get("roster", [])),
                    hints.get("same_person", []))
    sites = Canon([a["site"] for a in assigns if a.get("site")] + list(hints.get("jobs", [])),
                  hints.get("same_site", []))

    # Window
    if hints.get("window"):
        window = sorted(set(hints["window"]))
    elif layout in GRIDS:
        raise ValueError(f"grid layout {layout} needs a window hint")
    else:
        ds = sorted({a["date"] for a in assigns})
        window = list(daterange(ds[0], ds[-1])) if ds else []
    working = [x for x in window if weekday(x) in workdays and x not in days_off]

    work = defaultdict(list)    # (person key, date) -> work records
    status = defaultdict(list)  # (person key, date) -> status records
    explicit = []
    for a in assigns:
        if a.get("unstaffed"):
            explicit.append((sites(a["site"]), a["date"]))
        elif "status" in a:
            status[(norm(persons(a["person"])), a["date"])].append(a)
        else:
            work[(norm(persons(a["person"])), a["date"])].append(a)

    flags, info = [], []

    # double_booked / split_day / internal_plus_site / duplicate_entry / subcontractor_multi_site
    for (pk, date), recs in sorted(work.items()):
        person = persons(recs[0]["person"])
        k = kind(person)
        by_site = defaultdict(list)
        for r in recs:
            by_site[sites(r["site"])].append(r)
        jobs = [s for s in by_site if not is_internal(s)]
        internal = [s for s in by_site if is_internal(s)]
        for s, rs in by_site.items():
            if len(rs) > 1 and any(overlaps(a, b) for a, b in itertools.combinations(rs, 2)):
                info.append({"type": "duplicate_entry", "person": person, "date": date, "sites": [s]})
        if k == "subcontractor":
            if len(jobs) >= 2:
                info.append({"type": "subcontractor_multi_site", "person": person, "date": date, "sites": jobs})
            continue
        if k == "placeholder":
            continue
        if len(jobs) >= 2:
            conflicting = set()
            for s1, s2 in itertools.combinations(jobs, 2):
                if any(overlaps(a, b) for a in by_site[s1] for b in by_site[s2]):
                    conflicting |= {s1, s2}
            if conflicting:
                flags.append({"type": "double_booked", "person": person, "date": date,
                              "sites": [s for s in jobs if s in conflicting]})
            else:
                info.append({"type": "split_day", "person": person, "date": date, "sites": list(by_site)})
        elif len(jobs) == 1 and internal:
            info.append({"type": "internal_plus_site", "person": person, "date": date, "sites": list(by_site)})
        elif not jobs and len(internal) >= 2:
            info.append({"type": "internal_plus_site", "person": person, "date": date, "sites": list(by_site)})

    # assigned_while_off
    for (pk, date), sts in sorted(status.items()):
        person = persons(sts[0]["person"])
        if kind(person) != "employee":
            continue
        ws = work.get((pk, date), [])
        for s in sts:
            hit = []
            for w in ws:
                if s.get("part") in ("am", "pm"):
                    if not overlaps(s, w):
                        continue  # "off PM" vs AM work: no conflict
                hit.append(sites(w["site"]))
            if hit:
                flags.append({"type": "assigned_while_off", "person": person, "date": date,
                              "sites": sorted(set(hit), key=hit.index), "status": s["status"]})

    # not_booked
    if hints.get("roster"):
        roster = [persons(n) for n in hints["roster"] if kind(n) == "employee"]
    else:
        roster = []
        for a in assigns:
            if a.get("person") and kind(persons(a["person"])) == "employee" and persons(a["person"]) not in roster:
                roster.append(persons(a["person"]))
    for p in roster:
        for date in working:
            if not work.get((norm(p), date)) and not status.get((norm(p), date)):
                flags.append({"type": "not_booked", "person": p, "date": date})

    # unstaffed
    un = []
    for site, date in explicit:
        if (site, date) not in un:
            un.append((site, date))
    staffed = {(sites(a["site"]), a["date"]) for a in assigns
               if a.get("site") and a.get("person") and "status" not in a}
    if layout in JOB_GRIDS:
        job_list = []
        for j in list(hints.get("jobs", [])) + [a["site"] for a in assigns if a.get("site") and a.get("person")]:
            j = sites(j)
            if j not in job_list and not is_internal(j):
                job_list.append(j)
        inactive = {(sites(s), x) for s, x in hints.get("inactive", [])}
        for j in job_list:
            for date in working:
                if (j, date) not in inactive and (j, date) not in staffed and (j, date) not in un:
                    un.append((j, date))
    else:
        assigned = defaultdict(set)
        for (j, date) in list(staffed) + explicit:
            if not is_internal(j) and date in working:
                assigned[j].add(date)
        for j, ds in assigned.items():
            weeks = defaultdict(list)
            for x in ds:
                weeks[week_start(x)].append(x)
            for wk, xs in weeks.items():
                for date in working:
                    if min(xs) <= date <= max(xs) and (j, date) not in staffed and (j, date) not in un:
                        un.append((j, date))
    for site, date in un:
        flags.append({"type": "unstaffed", "site": site, "date": date})
    return flags, info, persons, sites


def key(item):
    return (
        item["type"],
        norm(item.get("person", "")),
        norm(item.get("site", "")),
        item.get("date"),
        frozenset(norm(s) for s in item.get("sites", [])),
        item.get("status"),
    )


def show(item):
    return json.dumps(item, ensure_ascii=False)


def diff(label, want, got):
    w, g = Counter(map(key, want)), Counter(map(key, got))
    out = []
    if w != g:
        byk = {key(x): x for x in list(want) + list(got)}
        for k in (w - g):
            out.append(f"  {label} expected but not derived: {show(byk[k])}")
        for k in (g - w):
            out.append(f"  {label} derived but not expected:  {show(byk[k])}")
    return out


ISO = re.compile(r"^\d{4}-\d{2}-\d{2}$")
HHMM = re.compile(r"^([01]\d|2[0-3]):[0-5]\d$")


def validate_assignments(assigns, where):
    errs = []
    for a in assigns:
        if not isinstance(a, dict):
            errs.append(f"  {where}: assignment is not an object: {a!r}")
            continue
        extra = set(a) - {"person", "date", "site", "status", "unstaffed", "part", "start", "end", "note"}
        if extra:
            errs.append(f"  {where}: unknown assignment keys {sorted(extra)} in {show(a)}")
        if not ISO.match(a.get("date", "")):
            errs.append(f"  {where}: bad date in {show(a)}")
        else:
            dt.date.fromisoformat(a["date"])
        if a.get("unstaffed"):
            if "person" in a or not a.get("site"):
                errs.append(f"  {where}: unstaffed record must have a site and no person: {show(a)}")
        else:
            if not a.get("person"):
                errs.append(f"  {where}: record without person: {show(a)}")
            if ("status" in a) == ("site" in a):
                errs.append(f"  {where}: record needs exactly one of site/status: {show(a)}")
        if "status" in a and a["status"] not in STATUSES:
            errs.append(f"  {where}: unknown status in {show(a)}")
        if "part" in a and a["part"] not in PARTS:
            errs.append(f"  {where}: unknown part in {show(a)}")
        for t in ("start", "end"):
            if t in a and not HHMM.match(a[t]):
                errs.append(f"  {where}: bad {t} in {show(a)}")
        for t in ("person", "site", "note"):
            if t in a and a[t] != " ".join(a[t].split()):
                errs.append(f"  {where}: {t} not trimmed/collapsed in {show(a)}")
    return errs


MATCH_QS = ("same_person", "same_site")


def validate_questions(qs, where):
    errs = []
    for q in qs:
        if q.get("type") not in QUESTION_TYPES:
            errs.append(f"  {where}: unknown question type {q!r}")
        if q.get("type") in MATCH_QS and ("a" in q or "b" in q):
            if not (isinstance(q.get("a"), str) and isinstance(q.get("b"), str)) or norm(q["a"]) == norm(q["b"]):
                errs.append(f"  {where}: {q['type']} needs two different names a and b: {q!r}")
    return errs


def pair(q):
    """Unordered, case-insensitive key of a same_person / same_site question (None: type only)."""
    if "a" not in q:
        return (q["type"], None)
    return (q["type"], frozenset((norm(q["a"]), norm(q["b"]))))


def merges_after(hints, answers):
    """Hint merges plus same_person / same_site pairs answered "yes"."""
    m = {t: [list(p) for p in hints.get(t, [])] for t in MATCH_QS}
    for a in answers:
        q = a.get("question", {})
        if q.get("type") in MATCH_QS and str(a.get("answer", "")).lower() == "yes":
            m[q["type"]].append([q["a"], q["b"]])
    return {**hints, **m}


def check_questions(exp, hints, canon):
    """Bookkeeping between hint merges, ask, must_not_ask and answers (see docstring)."""
    errs = []
    ask = [q for q in exp.get("ask", []) if q.get("type") in MATCH_QS]
    nots = [q for q in exp.get("must_not_ask", []) if q.get("type") in MATCH_QS]
    ask_keys = {pair(q) for q in ask}
    for q in ask:
        if "a" not in q:
            errs.append(f"  ask: {q['type']} without a and b can't be checked")
            continue
        c = canon[q["type"]]
        for n in (q["a"], q["b"]):
            if norm(n) not in c.seen:
                errs.append(f"  ask: {n!r} is not a spelling in the data ({q['type']})")
            elif norm(c(n)) != norm(n):
                errs.append(f"  ask: {n!r} should be named by its canonical spelling {c(n)!r}")
        if c.same(q["a"], q["b"]):
            errs.append(f"  ask: {q!r} is already merged by the hints (deterministic), so it can't be asked")
    for q in nots:
        if pair(q) in ask_keys or ("a" not in q and any(t == q["type"] for t, _ in ask_keys)):
            errs.append(f"  must_not_ask {q!r} contradicts ask")
    for t in MATCH_QS:
        for a, b in hints.get(t, []):
            for n in (a, b):
                if norm(n) not in canon[t].seen:
                    errs.append(f"  hint {t}: {n!r} is not a spelling in the data")
            if (t, frozenset((norm(a), norm(b)))) in ask_keys:
                errs.append(f"  hint {t} [{a!r}, {b!r}] is a deterministic merge but is also asked")
    for a in exp.get("answers", []):
        q = a.get("question", {})
        if q.get("type") not in MATCH_QS:
            continue
        if "a" not in q or pair(q) not in ask_keys:
            errs.append(f"  answers: {q!r} is not one of the asked questions")
        if str(a.get("answer", "")).lower() not in ("yes", "no"):
            errs.append(f"  answers: {q.get('type')} takes yes or no, got {a.get('answer')!r}")
    return errs


def check(folder: Path) -> tuple[list[str], dict]:
    errs = []
    stats = {"flags": 0, "red": 0}
    fid = folder.name.split("-")[0]
    try:
        exp = json.loads((folder / "expected.json").read_text(encoding="utf-8"))
    except Exception as e:  # noqa: BLE001
        return [f"  expected.json does not parse: {e}"], stats
    readme = (folder / "README.md").read_text(encoding="utf-8") if (folder / "README.md").exists() else ""
    if len(readme.strip().splitlines()) != 1:
        errs.append("  README.md must be exactly one line")
    if set(exp) - KEYS:
        errs.append(f"  unknown keys {sorted(set(exp) - KEYS)}")
    for k in ("category", "tier"):
        if k not in exp:
            errs.append(f"  missing {k}")
    if exp.get("category") != fid[0]:
        errs.append(f"  category {exp.get('category')} does not match folder {fid}")
    if exp.get("tier") not in ("script", "model"):
        errs.append("  tier must be script or model")
    if not (folder / exp.get("input", "")).is_file():
        errs.append(f"  input file {exp.get('input')} missing")
    if exp.get("paste") and exp.get("input") != "paste.txt":
        errs.append("  paste fixtures use paste.txt")
    if exp.get("layout") and exp["layout"] not in LAYOUTS:
        errs.append(f"  unknown layout {exp['layout']}")
    if exp.get("arrangement") and exp["arrangement"] not in ARRANGEMENTS:
        errs.append(f"  unknown arrangement {exp['arrangement']}")
    st = exp.get("settings", {})
    if any(w not in WEEKDAYS for w in st.get("workdays", [])):
        errs.append("  bad workdays")
    errs += validate_questions(exp.get("ask", []), "ask")
    errs += validate_questions(exp.get("must_not_ask", []), "must_not_ask")
    for a in exp.get("answers", []):
        errs += validate_questions([a.get("question", {})], "answers")

    hints_path = HINTS / f"{fid}.json"
    hints = json.loads(hints_path.read_text(encoding="utf-8")) if hints_path.exists() else {}

    answers = exp.get("answers", [])
    only_match_answers = all(a.get("question", {}).get("type") in MATCH_QS for a in answers)
    blocks = [("top", exp)]
    if "after_answers" in exp:
        blocks.append(("after_answers", exp["after_answers"]))
    top_truth = None
    for where, blk in blocks:
        blk_hints = hints if where == "top" else merges_after(hints, answers)
        if "assignments" not in blk:
            if where == "after_answers" and top_truth is not None and only_match_answers:
                truth = top_truth  # same_person / same_site answers never change what was read
            else:
                if "flags" in blk:
                    errs.append(f"  {where}: flags without assignments can't be checked")
                continue
        else:
            errs += validate_assignments(blk["assignments"], where)
            truth = blk["assignments"]
        partial = where == "top" and exp.get("assignments_exact") is False
        if partial:
            if "assignments" not in hints:
                if "flags" in blk:
                    errs.append(f"  {where}: flags given but assignments are partial and no truth hint")
                continue
            truth = hints["assignments"]
            errs += validate_assignments(truth, "hint truth")
            have = Counter(json.dumps(a, sort_keys=True, ensure_ascii=False) for a in truth)
            for a in blk["assignments"]:
                k = json.dumps(a, sort_keys=True, ensure_ascii=False)
                if have[k] <= 0:
                    errs.append(f"  {where}: sample assignment not in the truth hint: {show(a)}")
                have[k] -= 1
        if where == "top":
            top_truth = truth
        try:
            flags, info, persons, sites = compute(truth, st, exp.get("layout"), blk_hints)
        except Exception as e:  # noqa: BLE001
            errs.append(f"  {where}: could not compute: {e}")
            continue
        if where == "top":
            errs += check_questions(exp, hints, {"same_person": persons, "same_site": sites})
        else:
            for a in answers:
                q = a.get("question", {})
                if (q.get("type") in MATCH_QS and "a" in q and str(a.get("answer", "")).lower() == "no"
                        and {"same_person": persons, "same_site": sites}[q["type"]].same(q["a"], q["b"])):
                    errs.append(f"  after_answers: {q!r} was answered no but other merges join it")
        for item in flags + info:
            if "person" in item and norm(item["person"]) in persons.tied:
                errs.append(f"  {where}: {item['type']} names {item['person']!r}, whose merged spellings tie "
                            f"for longest (no defined canonical name)")
            for s in item.get("sites", []) + ([item["site"]] if "site" in item else []):
                if norm(s) in sites.tied:
                    errs.append(f"  {where}: {item['type']} names site {s!r}, whose merged spellings tie "
                                f"for longest (no defined canonical name)")
        if "flags" in blk:
            for f in blk["flags"]:
                if f.get("type") not in FLAG_TYPES:
                    errs.append(f"  {where}: unknown flag type {f!r}")
            errs += [e.replace("  ", f"  [{where}] ", 1) for e in diff("flag", blk["flags"], flags)]
            stats["flags"] += len(blk["flags"])
            stats["red"] += sum(f["type"] in ("double_booked", "assigned_while_off") for f in blk["flags"])
        else:
            errs.append(f"  {where}: no flags block")
        if "info" in blk:
            for f in blk["info"]:
                if f.get("type") not in INFO_TYPES:
                    errs.append(f"  {where}: unknown info type {f!r}")
            errs += [e.replace("  ", f"  [{where}] ", 1) for e in diff("info", blk["info"], info)]
    return errs, stats


def main(argv):
    folders = sorted(p for p in ROOT.iterdir() if p.is_dir() and re.match(r"^[A-N]\d\d-", p.name))
    if argv:
        folders = [p for p in folders if p.name.split("-")[0] in argv]
    bad = 0
    red = 0
    by_cat = Counter()
    for f in folders:
        errs, stats = check(f)
        by_cat[f.name[0]] += 1
        red += stats["red"] > 0
        if errs:
            bad += 1
            print(f"FAIL {f.name}")
            print("\n".join(errs))
        else:
            print(f"ok   {f.name}  ({stats['flags']} flags)")
    print(f"\n{len(folders) - bad}/{len(folders)} fixtures agree with the rules; "
          f"{red} have a red flag; per category: {dict(sorted(by_cat.items()))}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
