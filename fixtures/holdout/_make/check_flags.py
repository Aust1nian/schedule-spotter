"""Recompute flags and info items for every holdout fixture from its
expected.json assignments, using fixtures/FORMAT.md "Flag semantics", and
compare with the hand-written flags/info in expected.json.

Fixture facts the rules need but the assignments alone cannot carry (the day
axis of a grid, a roster block, name merges, which sites are internal, which
names are placeholders or subcontractors, job-grid rows and inactive cells)
come from _make/meta/<ID>.json.

The checker refuses (raises) on combinations whose treatment FORMAT.md leaves
open, so the fixtures stay clear of them:
  * an internal site whose times overlap a job's times,
  * an internal site plus two or more job sites on one date,
  * three or more job sites for one person on one date,
  * an entry with a time mixed with an entry with a `part`,
  * a status plus work at an internal site,
  * inferred unstaffed results that change depending on whether explicit
    TBD/open job-days count toward a non-grid job's active span.

Usage: python3 check_flags.py            (all fixtures)
       python3 check_flags.py H03 H07     (some)
Exit status 0 only if every fixture agrees.
"""
import datetime as dt
import glob
import json
import os
import sys
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
HOLDOUT = os.path.dirname(HERE)
WD = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
JOB_GRIDS = {"job_by_day", "day_by_job"}
SUB_TOKENS = ["llc", "inc", "co.", "electric", "plumbing", "hvac", "roofing", "concrete", "drywall",
              "painting", "construction", "services", "contractors"]
PLACEHOLDER_WORDS = ["helper", "foreman", "laborer", "new guy", "temp"]
INTERNAL_WORDS = ["shop", "yard", "office", "warehouse"]


def D(s):
    return dt.date.fromisoformat(s)


def daterange(a, b):
    d = D(a)
    out = []
    while d <= D(b):
        out.append(d.isoformat())
        d += dt.timedelta(1)
    return out


def minutes(hhmm):
    h, m = hhmm.split(":")
    return int(h) * 60 + int(m)


def interval(r):
    if "start" in r:
        s, e = minutes(r["start"]), minutes(r["end"])
        if e < s:
            e += 24 * 60
        return (s, e)
    if r.get("part") == "am":
        return (0, 12 * 60)
    if r.get("part") == "pm":
        return (12 * 60, 24 * 60)
    return None


def overlap(a, b):
    return a[0] < b[1] and b[0] < a[1]


class Refuse(Exception):
    pass


def compute(exp, meta):
    A = exp["assignments"]
    st = exp["settings"]
    workdays = set(st["workdays"])
    days_off = set(st.get("days_off", []))
    layout = exp["layout"]

    pmap = {k.lower(): v for k, v in meta.get("person_canon", {}).items()}
    smap = {k.lower(): v for k, v in meta.get("site_canon", {}).items()}

    def pc(p):
        return pmap.get(p.lower(), p)

    def sc(s):
        return smap.get(s.lower(), s)

    internal = {s.lower() for s in meta.get("internal", [])}
    placeholders = {p.lower() for p in meta.get("placeholders", [])}
    subs = {p.lower() for p in meta.get("subs", [])}
    pseudo = {s.lower() for s in meta.get("pseudo_rows", [])}

    # sanity: the meta classification must agree with the FORMAT.md token lists
    def toks(s):
        return {w.strip(".,") for w in s.lower().split()}

    for r in A:
        if "person" in r:
            low = r["person"].lower()
            looks_sub = bool(toks(low) & {t.strip(".") for t in SUB_TOKENS})
            if looks_sub != (low in subs):
                raise Refuse(f"subcontractor classification mismatch for {r['person']!r}")
            looks_ph = any(low == w or low.startswith(w + " ") for w in PLACEHOLDER_WORDS)
            if looks_ph != (low in placeholders):
                raise Refuse(f"placeholder classification mismatch for {r['person']!r}")
        if "site" in r and not r.get("unstaffed"):
            low = r["site"].lower()
            looks_int = bool(toks(low) & set(INTERNAL_WORDS)) or low.startswith("drive to") or low == "travel"
            if looks_int != (low in internal):
                raise Refuse(f"internal-site classification mismatch for {r['site']!r}")

    def kind(p):
        low = p.lower()
        if low in subs:
            return "sub"
        if low in placeholders:
            return "placeholder"
        return "employee"

    # ---- window and working dates
    if meta.get("window"):
        window = list(meta["window"])
    else:
        ds = sorted(r["date"] for r in A)
        window = daterange(ds[0], ds[-1])
    working = [d for d in window if WD[D(d).weekday()] in workdays and d not in days_off]

    # ---- roster
    if meta.get("roster") is not None:
        roster = {}
        for p in meta["roster"]:
            roster[pc(p).lower()] = pc(p)
    else:
        roster = {}
        for r in A:
            if "person" in r and kind(r["person"]) == "employee":
                roster[pc(r["person"]).lower()] = pc(r["person"])

    work = defaultdict(list)     # (canon person lower, date) -> records
    status = defaultdict(list)
    display = {}
    for r in A:
        if r.get("unstaffed"):
            continue
        p = pc(r["person"])
        display[p.lower()] = p
        key = (p.lower(), r["date"])
        if "status" in r:
            status[key].append(r)
        else:
            work[key].append(dict(r, site=sc(r["site"])))

    flags, infos = [], []

    # ---- double_booked / info, assigned_while_off
    for (pl, d), recs in sorted(work.items()):
        p = display[pl]
        k = kind(p)
        by_site = defaultdict(list)
        for r in recs:
            by_site[r["site"].lower()].append(r)
        site_disp = {s.lower(): r["site"] for s in by_site for r in by_site[s]}
        if k == "sub":
            jobs = [s for s in by_site if s not in internal]
            if len(jobs) >= 2:
                infos.append({"type": "subcontractor_multi_site", "person": p, "date": d,
                              "sites": sorted(site_disp[s] for s in jobs)})
            continue
        if k == "placeholder":
            continue
        for s, rs in by_site.items():
            if len(rs) >= 2:
                infos.append({"type": "duplicate_entry", "person": p, "date": d, "sites": [site_disp[s]]})
        if len(by_site) >= 2:
            jobs = [s for s in by_site if s not in internal]
            ints = [s for s in by_site if s in internal]
            has_time = any("start" in r for r in recs)
            has_part = any("part" in r for r in recs)
            if has_time and has_part:
                raise Refuse(f"{p} {d}: time mixed with part")
            if len(jobs) >= 3:
                raise Refuse(f"{p} {d}: three or more job sites")
            if len(jobs) == 2:
                if ints:
                    raise Refuse(f"{p} {d}: internal plus two job sites")
                ents = [r for s in jobs for r in by_site[s]]
                untimed = any(interval(r) is None for r in ents)
                ov = False
                for i in range(len(ents)):
                    for j in range(i + 1, len(ents)):
                        a, b = ents[i], ents[j]
                        if a["site"].lower() == b["site"].lower():
                            continue
                        ia, ib = interval(a), interval(b)
                        if ia and ib and overlap(ia, ib):
                            ov = True
                if untimed or ov:
                    flags.append({"type": "double_booked", "person": p, "date": d,
                                  "sites": sorted(site_disp[s] for s in jobs)})
                else:
                    infos.append({"type": "split_day", "person": p, "date": d,
                                  "sites": sorted(site_disp[s] for s in by_site)})
            else:
                # one job + internal, or internal only
                if len(jobs) == 1:
                    ja = [interval(r) for r in by_site[jobs[0]]]
                    ia = [interval(r) for s in ints for r in by_site[s]]
                    if any(x and y and overlap(x, y) for x in ja for y in ia):
                        raise Refuse(f"{p} {d}: internal site overlapping a job's times")
                infos.append({"type": "internal_plus_site", "person": p, "date": d,
                              "sites": sorted(site_disp[s] for s in by_site)})

    for (pl, d), srecs in sorted(status.items()):
        p = display[pl]
        if kind(p) != "employee":
            raise Refuse(f"status for non-employee {p}")
        wrecs = work.get((pl, d), [])
        if not wrecs:
            continue
        vals = {r["status"] for r in srecs}
        if len(vals) != 1:
            raise Refuse(f"{p} {d}: several different statuses")
        conflict_sites = []
        for w in wrecs:
            clash = False
            for s in srecs:
                if "part" not in s:
                    clash = True
                else:
                    half = (0, 720) if s["part"] == "am" else (720, 1440) if s["part"] == "pm" else None
                    iw = interval(w)
                    if half is None or iw is None or overlap(half, iw):
                        clash = True
            if clash:
                conflict_sites.append(w["site"])
        if conflict_sites:
            if any(s.lower() in internal for s in conflict_sites):
                raise Refuse(f"{p} {d}: status plus work at an internal site")
            flags.append({"type": "assigned_while_off", "person": p, "date": d,
                          "sites": sorted(set(conflict_sites), key=str.lower), "status": vals.pop()})

    # ---- not_booked
    for pl, p in sorted(roster.items()):
        for d in working:
            if not work.get((pl, d)) and not status.get((pl, d)):
                flags.append({"type": "not_booked", "person": p, "date": d})

    # ---- unstaffed
    explicit = set()
    for r in A:
        if r.get("unstaffed"):
            explicit.add((sc(r["site"]), r["date"]))
    staffed = defaultdict(set)  # site lower -> dates with any work record
    site_name = {}
    for (pl, d), recs in work.items():
        for r in recs:
            staffed[r["site"].lower()].add(d)
            site_name[r["site"].lower()] = r["site"]
    for s, d in explicit:
        site_name.setdefault(s.lower(), s)

    inferred = set()
    if layout in JOB_GRIDS:
        jobs = [sc(j) for j in meta["jobs"]]
        inactive = {(sc(s).lower(), d) for s, d in meta.get("inactive", [])}
        for j in jobs:
            jl = j.lower()
            if jl in internal or jl in pseudo:
                continue
            for d in working:
                if (jl, d) in inactive:
                    continue
                if d not in staffed[jl]:
                    inferred.add((j, d))
    else:
        jobs = {s for s in site_name if s not in internal and s not in pseudo}
        working_set = set(working)

        def spans(include_explicit):
            out = set()
            for jl in jobs:
                dates = {d for d in staffed[jl] if d in working_set}
                if include_explicit:
                    dates |= {d for (s, d) in explicit if s.lower() == jl and d in working_set}
                byweek = defaultdict(list)
                for d in dates:
                    iso = D(d).isocalendar()
                    byweek[(iso[0], iso[1])].append(d)
                for wk, ds in byweek.items():
                    for d in daterange(min(ds), max(ds)):
                        if d in working_set and d not in staffed[jl]:
                            out.add((site_name[jl], d))
            return out

        a, b = spans(False), spans(True)
        # explicit job-days are flagged anyway; the two readings must agree on everything else
        ex = {(s.lower(), d) for s, d in explicit}
        if {(s.lower(), d) for s, d in a} - ex != {(s.lower(), d) for s, d in b} - ex:
            raise Refuse("inferred unstaffed depends on whether explicit TBD days extend the active span")
        inferred = a

    un = {}
    for s, d in list(explicit) + list(inferred):
        un[(s.lower(), d)] = (s, d)
    for s, d in sorted(un.values()):
        flags.append({"type": "unstaffed", "site": s, "date": d})
    return flags, infos


def fkey(f):
    who = (f.get("person") or f.get("site") or "").lower()
    sites = frozenset(s.lower() for s in f.get("sites", []))
    return (f["type"], who, f["date"], sites, f.get("status"))


def compare(label, want, got):
    wk = [fkey(f) for f in want]
    gk = [fkey(f) for f in got]
    ok = True
    if len(set(wk)) != len(wk):
        print(f"  {label}: duplicate entries in expected.json")
        ok = False
    missing = set(wk) - set(gk)
    extra = set(gk) - set(wk)
    for m in sorted(missing, key=str):
        print(f"  {label}: in expected.json but not recomputed: {m}")
    for e in sorted(extra, key=str):
        print(f"  {label}: recomputed but missing from expected.json: {e}")
    return ok and not missing and not extra


def check_assignment_shape(exp):
    problems = []
    for r in exp["assignments"]:
        if r.get("unstaffed"):
            if "person" in r or "site" not in r:
                problems.append(f"bad unstaffed record {r}")
        elif "status" in r:
            if "site" in r:
                problems.append(f"status record with a site {r}")
        elif "site" not in r or "person" not in r:
            problems.append(f"bad work record {r}")
        D(r["date"])
        for k in ("start", "end"):
            if k in r and (len(r[k]) != 5 or r[k][2] != ":"):
                problems.append(f"bad time {r}")
    return problems


def main(ids):
    metas = sorted(glob.glob(os.path.join(HERE, "meta", "H*.json")))
    allok = True
    n = 0
    for mp in metas:
        fid = os.path.basename(mp)[:3]
        if ids and fid not in ids:
            continue
        meta = json.load(open(mp, encoding="utf-8"))
        exp = json.load(open(os.path.join(HOLDOUT, meta["fixture_dir"], "expected.json"), encoding="utf-8"))
        n += 1
        probs = check_assignment_shape(exp)
        try:
            flags, infos = compute(exp, meta)
        except Refuse as e:
            print(f"{fid}: REFUSED ({e})")
            allok = False
            continue
        ok = not probs
        for p in probs:
            print(f"  {fid}: {p}")
        ok &= compare(f"{fid} flags", exp["flags"], flags)
        ok &= compare(f"{fid} info", exp["info"], infos)
        if "after_answers" in exp and "flags" in exp["after_answers"]:
            ok &= compare(f"{fid} after_answers.flags", exp["after_answers"]["flags"], flags)
        print(f"{fid}: {'OK' if ok else 'MISMATCH'}  ({len(exp['assignments'])} assignments, "
              f"{len(exp['flags'])} flags, {len(exp['info'])} info)")
        allok &= ok
    print(f"checked {n} fixture(s): {'ALL AGREE' if allok and n else 'DISAGREEMENT'}")
    return 0 if allok and n else 1


if __name__ == "__main__":
    sys.exit(main(set(sys.argv[1:])))
