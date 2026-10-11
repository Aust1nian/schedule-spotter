"""Shared helpers for building the sealed holdout fixtures (H01-H20).

Each fixture module (hNN.py) renders its input file by hand-written data and
returns its expected.json content plus a small meta record (window, roster,
merges, internal sites, placeholders, subcontractors, job-grid facts) that
check_flags.py uses to recompute flags independently of the hand-written ones.

Cell-reading helpers below implement fixtures/FORMAT.md "Assignment" rules:
trim, collapse whitespace, split multi-name cells, move trailing notes
(" - ..." or "(...)") to `note`, map status words, mark unstaffed markers.
"""
import csv
import datetime as dt
import io
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
HOLDOUT = os.path.dirname(HERE)

WD = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
MON_FRI = WD[:5]
MON_SAT = WD[:6]
TODAY = "2026-10-10"


def D(s):
    return dt.date.fromisoformat(s)


def days(start, n):
    return [(D(start) + dt.timedelta(i)).isoformat() for i in range(n)]


def wd(iso):
    return WD[D(iso).weekday()]


def md(iso, year=False, short_year=False):
    d = D(iso)
    if year:
        return f"{d.month}/{d.day}/{d.year}"
    if short_year:
        return f"{d.month}/{d.day}/{d.year % 100:02d}"
    return f"{d.month}/{d.day}"


def norm(s):
    return re.sub(r"\s+", " ", s).strip()


STATUS = {
    "off": "off", "x": "off", "—": "off", "-": "off", "n/a": "off",
    "day off": "off", "pto": "pto", "vac": "vacation", "vacation": "vacation",
    "sick": "sick", "holiday": "holiday", "out": "out",
    "descanso": "off", "vacaciones": "vacation", "enfermo": "sick", "libre": "off",
}
UNSTAFFED = {"tbd", "open", "?", "need guy", "need 1"}
INACTIVE = {"x", "—", "n/a", "no work", "closed", "done"}


def strip_note(s):
    """FORMAT.md: move a trailing note to `note` (text after " - ", or a trailing (...))."""
    s = norm(s)
    m = re.match(r"^(.*?)\s+-\s+(.+)$", s)
    if m:
        return norm(m.group(1)), norm(m.group(2))
    m = re.match(r"^(.*?)\s*\(([^()]*)\)$", s)
    if m and m.group(1):
        return norm(m.group(1)), norm(m.group(2))
    return s, None


def split_multi(s):
    """Split a multi-name / multi-site cell on , & / + line breaks ' and ' ' y '."""
    parts = re.split(r"\s*(?:,|&|/|\+|\n)\s*|\s+and\s+|\s+y\s+", s.strip())
    return [norm(p) for p in parts if norm(p)]


# ---------- record builders ----------

def W(person, date, site, note=None, part=None, start=None, end=None):
    r = {"person": norm(person), "date": date, "site": norm(site)}
    if note:
        r["note"] = note
    if part:
        r["part"] = part
    if start:
        r["start"] = start
        r["end"] = end
    return r


def S(person, date, status, note=None, part=None):
    r = {"person": norm(person), "date": date, "status": status}
    if note:
        r["note"] = note
    if part:
        r["part"] = part
    return r


def U(date, site):
    return {"date": date, "site": norm(site), "unstaffed": True}


def person_cell(person, date, text):
    """A person x day (or day x person) cell: one or more sites, or a status."""
    if text is None or norm(str(text)) == "":
        return []
    text = str(text)
    if norm(text).lower() in STATUS:
        return [S(person, date, STATUS[norm(text).lower()])]
    out = []
    for piece in split_multi(text):
        low = piece.lower()
        if low in STATUS:
            out.append(S(person, date, STATUS[low]))
            continue
        site, note = strip_note(piece)
        out.append(W(person, date, site, note=note))
    return out


def names_cell(site, date, text, status=None):
    """A job x day (or day x job) cell holding names. `status` set for an OFF/VAC pseudo-row."""
    if text is None or norm(str(text)) == "":
        return []
    text = str(text)
    if norm(text).lower() in INACTIVE and status is None:
        return []
    if norm(text).lower() in UNSTAFFED:
        return [U(date, site)]
    out = []
    for name in split_multi(text):
        if name.lower() in UNSTAFFED:
            out.append(U(date, site))
            continue
        person, note = strip_note(name)
        if status:
            out.append(S(person, date, status, note=note))
        else:
            out.append(W(person, date, site, note=note))
    return out


KEY_ORDER = ["person", "date", "site", "status", "note", "part", "start", "end", "unstaffed"]


def ordered(r):
    return {k: r[k] for k in KEY_ORDER if k in r}


def sort_recs(recs):
    return sorted((ordered(r) for r in recs),
                  key=lambda r: (r["date"], r.get("person", "~").lower(), r.get("site", ""), r.get("status", ""),
                                 r.get("start", "")))


def flag_db(person, date, sites):
    return {"type": "double_booked", "person": person, "date": date, "sites": list(sites)}


def flag_awo(person, date, sites, status):
    return {"type": "assigned_while_off", "person": person, "date": date, "sites": list(sites), "status": status}


def flag_nb(person, date):
    return {"type": "not_booked", "person": person, "date": date}


def flag_un(site, date):
    return {"type": "unstaffed", "site": site, "date": date}


def info(kind, person, date, sites):
    return {"type": kind, "person": person, "date": date, "sites": list(sites)}


# ---------- writers ----------

def fixture_dir(fid, slug):
    p = os.path.join(HOLDOUT, f"{fid}-{slug}")
    os.makedirs(p, exist_ok=True)
    return p


def csv_text(rows, delimiter=",", lineterminator="\n"):
    buf = io.StringIO()
    w = csv.writer(buf, delimiter=delimiter, lineterminator=lineterminator, quoting=csv.QUOTE_MINIMAL)
    for row in rows:
        w.writerow(["" if c is None else c for c in row])
    return buf.getvalue()


def write_bytes(path, data):
    with open(path, "wb") as f:
        f.write(data)


def pad(rows, n):
    return [list(r) + [""] * (n - len(r)) for r in rows]


def write_fixture(fid, slug, readme, input_name, expected, meta):
    d = fixture_dir(fid, slug)
    with open(os.path.join(d, "README.md"), "w", encoding="utf-8") as f:
        f.write(readme.strip() + "\n")
    exp = {
        "category": "H",
        "tier": expected.get("tier", "script"),
        "input": input_name,
        "paste": expected.get("paste", False),
        "settings": {"today": expected.get("today", TODAY), "workdays": expected["workdays"],
                     "days_off": expected.get("days_off", [])},
        "layout": expected["layout"],
        "arrangement": expected.get("arrangement", "single"),
        "assignments": sort_recs(expected["assignments"]),
        "assignments_exact": True,
        "flags": expected["flags"],
        "flags_exact": True,
        "info": expected.get("info", []),
        "info_exact": False,
        "ask": expected.get("ask", []),
        "must_not_ask": expected.get("must_not_ask", []),
    }
    if "answers" in expected:
        exp["answers"] = expected["answers"]
        exp["after_answers"] = expected["after_answers"]
    exp["error"] = None
    with open(os.path.join(d, "expected.json"), "w", encoding="utf-8") as f:
        json.dump(exp, f, ensure_ascii=False, indent=2)
        f.write("\n")
    meta = dict(meta)
    meta["fixture_dir"] = os.path.basename(d)
    with open(os.path.join(HERE, "meta", f"{fid}.json"), "w", encoding="utf-8") as f:
        json.dump(meta, f, ensure_ascii=False, indent=2)
        f.write("\n")
    return d
