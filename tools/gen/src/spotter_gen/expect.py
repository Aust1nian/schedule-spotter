"""Step 4, expect: expected.json from the truth plus the rendering decisions (never from a
pipeline). Flags and info follow fixtures/FORMAT.md "Flag semantics" exactly, computed on
truth identities, displayed as the longest rendered spelling."""

from __future__ import annotations

import datetime as dt

from .model import DAY_KEYS, TODAY, Rec, Truth

GRID_LAYOUTS = ("person_by_day", "day_by_person", "job_by_day", "day_by_job")
JOB_GRIDS = ("job_by_day", "day_by_job")


def _mins(t: str) -> int:
    return int(t[:2]) * 60 + int(t[3:])


def _overlap(a: Rec, b: Rec) -> bool:
    return _mins(a.start) < _mins(b.end) and _mins(b.start) < _mins(a.end)


def window(T: Truth, layout: str) -> list[dt.date]:
    if layout in GRID_LAYOUTS:
        return list(T.dates)
    ds = sorted({r.date for r in T.recs})
    out, d = [], ds[0]
    while d <= ds[-1]:
        out.append(d)
        d += dt.timedelta(days=1)
    return out


def analyze(T: Truth, layout: str, roster: set[str], inactive_cells: set, job_rows: list[str]):
    """Return (flags, info) as identity-keyed dicts (person/site ids, ISO dates)."""
    wd = set(T.workdays)
    win = window(T, layout)
    working = [d for d in win if DAY_KEYS[d.weekday()] in wd]
    internal = {s.id for s in T.sites if s.internal}
    kind = {p.id: p.kind for p in T.people}
    by_pd: dict = {}
    by_sd: dict = {}
    for r in T.recs:
        if r.pid:
            by_pd.setdefault((r.pid, r.date), []).append(r)
        if r.kind == "work":
            by_sd.setdefault((r.sid, r.date), []).append(r)
    flags: list[dict] = []
    info: list[dict] = []
    for p in T.people:
        for d in win:
            rs = by_pd.get((p.id, d), [])
            works = [r for r in rs if r.kind == "work"]
            stats = [r for r in rs if r.kind == "status"]
            sites = list(dict.fromkeys(r.sid for r in works))
            jobs = [s for s in sites if s not in internal]
            ints = [s for s in sites if s in internal]
            if kind[p.id] == "subcontractor":
                if len(jobs) >= 2:
                    info.append({"type": "subcontractor_multi_site", "person": p.id, "date": d, "sites": jobs})
                continue
            if kind[p.id] != "employee":
                continue
            if len(jobs) >= 2:
                jw = [r for r in works if r.sid in jobs]
                missing = any(r.start is None for r in jw)
                overl = any(_overlap(a, b) for i, a in enumerate(jw) for b in jw[i + 1:]
                            if a.sid != b.sid and a.start and b.start)
                if missing or overl:
                    flags.append({"type": "double_booked", "person": p.id, "date": d, "sites": jobs})
                else:
                    info.append({"type": "split_day", "person": p.id, "date": d, "sites": jobs})
            elif ints and (len(jobs) == 1 or len(ints) >= 2):
                info.append({"type": "internal_plus_site", "person": p.id, "date": d, "sites": ints + jobs})
            for s in sites:
                if sum(1 for r in works if r.sid == s) >= 2:
                    info.append({"type": "duplicate_entry", "person": p.id, "date": d, "sites": [s]})
            if stats and works:
                flags.append({"type": "assigned_while_off", "person": p.id, "date": d, "sites": sites,
                              "status": stats[0].status})
            if not rs and d in working and p.id in roster:
                flags.append({"type": "not_booked", "person": p.id, "date": d})
    # unstaffed
    un: set = set()
    for r in T.recs:
        if r.kind == "unstaffed":
            un.add((r.sid, r.date))
    jobs_all = [s.id for s in T.sites if not s.internal]
    if layout in JOB_GRIDS:
        for sid in job_rows:
            for d in working:
                if (sid, d) not in inactive_cells and not by_sd.get((sid, d)):
                    un.add((sid, d))
    else:
        wset = set(working)
        for sid in jobs_all:
            weeks: dict = {}
            for d in win:
                if d in wset and by_sd.get((sid, d)):
                    monday = d - dt.timedelta(days=d.weekday())
                    weeks.setdefault(monday, []).append(d)
            for monday, ds in weeks.items():
                for d in working:
                    if ds[0] <= d <= ds[-1] and not by_sd.get((sid, d)):
                        un.add((sid, d))
    for sid, d in sorted(un, key=lambda x: (x[1], x[0])):
        flags.append({"type": "unstaffed", "site": sid, "date": d})
    return flags, info


def flag_key(f: dict) -> tuple:
    return (f["type"], f.get("person") or f.get("site"), f["date"])


TYPE_ORDER = {"double_booked": 0, "assigned_while_off": 1, "not_booked": 2, "unstaffed": 3,
              "duplicate_entry": 4, "split_day": 5, "internal_plus_site": 6, "subcontractor_multi_site": 7}


def display_items(items: list[dict], disp_p, disp_s) -> list[dict]:
    out = []
    for f in items:
        g = {"type": f["type"]}
        if "person" in f:
            g["person"] = disp_p(f["person"])
        if "site" in f:
            g["site"] = disp_s(f["site"])
        g["date"] = f["date"].isoformat()
        if "sites" in f:
            g["sites"] = sorted(disp_s(s) for s in f["sites"])
        if "status" in f:
            g["status"] = f["status"]
        out.append(g)
    out.sort(key=lambda g: (TYPE_ORDER[g["type"]], g["date"], g.get("person", ""), g.get("site", "")))
    return out


def assignments(T: Truth, text) -> list[dict]:
    out = []
    for r in T.recs:
        if r.kind == "unstaffed":
            a = {"date": r.date.isoformat(), "site": text(r.sref), "unstaffed": True}
        elif r.kind == "status":
            a = {"person": text(r.pref), "date": r.date.isoformat(), "status": r.status}
        else:
            a = {"person": text(r.pref), "date": r.date.isoformat(), "site": text(r.sref)}
            if r.start:
                a["start"], a["end"] = r.start, r.end
            if r.note:
                a["note"] = r.note
        out.append(a)
    out.sort(key=lambda a: (a["date"], a.get("person", "").casefold(), a.get("site", "").casefold(),
                            a.get("status", ""), a.get("start", ""), a.get("note", "")))
    return out


def expected_json(T: Truth, layout: str, arrangement: str, tier: str, input_name: str, paste: bool,
                  assigns: list[dict], flags: list[dict], info: list[dict], category: str) -> dict:
    return {
        "category": category,
        "tier": tier,
        "input": input_name,
        "paste": paste,
        "settings": {"today": TODAY, "workdays": T.workdays, "days_off": []},
        "layout": layout,
        "arrangement": arrangement,
        "assignments": assigns,
        "assignments_exact": True,
        "flags": flags,
        "flags_exact": True,
        "info": info,
        "info_exact": True,
        "error": None,
        "notes": [],
    }
