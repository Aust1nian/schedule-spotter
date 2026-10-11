"""Step 1, truth: a random company, its jobs and week(s), the true assignments, and planted
problems. Everything here is drawn from the main RNG; no name strings are touched."""

from __future__ import annotations

import datetime as dt
import random

from .model import STATUSES, Crew, Person, Rec, Site, Truth
from .names import NameSpec
from .plan import INFO_RATES, PROBLEM_RATES, Plan, wpick
from .words import PLACEHOLDERS, fold, lists

SUFFIXES = {  # abbreviation -> full
    "St": "Street", "Ave": "Avenue", "Rd": "Road", "Dr": "Drive", "Blvd": "Boulevard",
    "Ln": "Lane", "Ct": "Court", "Pl": "Place", "Cir": "Circle", "Pkwy": "Parkway", "Way": "Way",
}
CUSTOMER_JOBS = ("remodel", "kitchen", "bath", "addition", "deck", "reroof", "basement", "job",
                 "residence", "ADU", "garage", "patio", "fence")
INTERNAL = ("Shop", "Yard", "Office", "Warehouse")
SHIFTS = (("07:00", "15:30"), ("06:30", "15:00"), ("07:00", "15:00"), ("08:00", "16:30"),
          ("06:00", "14:30"), ("07:30", "16:00"))


class Wants:
    """How many of each problem / info item to plant (drawn before the company exists)."""

    def __init__(self, R: random.Random, P: Plan, clean_rate: float):
        self.clean = R.random() < clean_rate
        self.db = R.randint(1, 3) if R.random() < PROBLEM_RATES["double_booked"] else 0
        self.awo = R.randint(1, 2) if R.random() < PROBLEM_RATES["assigned_while_off"] else 0
        self.nb = R.randint(1, 4) if R.random() < PROBLEM_RATES["not_booked"] else 0
        self.idle_all = R.random() < 0.3
        self.un = R.randint(1, 2) if R.random() < PROBLEM_RATES["unstaffed"] else 0
        self.un_explicit = [R.random() < 0.5 for _ in range(2)]
        forced = R.choice(["db", "awo", "nb", "un"])
        if self.clean:
            self.db = self.awo = self.nb = self.un = 0
            self.idle_all = False
        elif not (self.db or self.awo or self.nb or self.un):
            setattr(self, forced, 1)
        if self.nb == 0 or P.family == "crew":
            self.idle_all = False
        self.ips = R.random() < INFO_RATES["internal_plus_site"]
        self.dup = R.random() < INFO_RATES["duplicate_entry"] and (
            (P.family == "list" and P.list_style == "per_record") or P.family == "calendar")
        self.split = R.random() < INFO_RATES["split_day"] and P.times_mode
        self.submulti = R.random() < INFO_RATES["subcontractor_multi_site"]


def _clamp(x: int, lo: int, hi: int) -> int:
    return max(lo, min(hi, x))


def build_truth(R: random.Random, P: Plan, W: Wants, seed: int) -> Truth:
    L = lists()
    dates = [P.start + dt.timedelta(days=7 * w + i) for w in range(P.weeks) for i in range(P.ndays)]

    # ------------------------------------------------------------------ people
    lo, hi = R.choices([(4, 10), (11, 25), (26, 60)], [45, 35, 20])[0]
    n_total = R.randint(lo, hi)
    n_place = n_sub = 0
    if n_total >= 6:
        n_place = R.choices([0, 1, 2], [75, 15, 10])[0]
        n_sub = R.choices([0, 1, 2], [78, 17, 5])[0]
    n_emp = n_total - n_place - n_sub
    specs: list[NameSpec] = []
    for _ in range(n_emp):
        origin = R.choices(["h", "a", "o"], [45, 45, 10])[0]
        specs.append(NameSpec(origin, False, origin == "h" and R.random() < 0.55,
                              origin == "h" and R.random() < 0.06))
    k_nick = R.choices([0, 1, 2, 3, 4], [25, 30, 25, 12, 8])[0]
    elig = [i for i in range(n_emp) if specs[i].origin in "ha"]
    for i in R.sample(elig, min(k_nick, len(elig))):
        specs[i].want_nick = True
    n_groups = 0 if n_emp < 8 else (R.choices([0, 1, 2], [50, 35, 15])[0] if n_emp < 30
                                    else R.choices([1, 2, 3], [40, 40, 20])[0])
    for g in range(n_groups):
        free = [i for i in range(n_emp) if specs[i].group is None]
        a, b = R.sample(free, 2)
        specs[a].group = specs[b].group = g
        specs[b].origin = specs[a].origin
        specs[b].want_nick = specs[a].want_nick
        specs[b].compound = specs[b].compound and specs[b].origin == "h"
    people: list[Person] = []
    for i, sp in enumerate(specs):
        people.append(Person(id=f"p{i + 1:02d}", kind="employee", spec=sp))
    if W.idle_all:
        idle = people[R.randrange(len(people))]
        idle.idle_all = True
    for name in R.sample(PLACEHOLDERS, n_place):
        people.append(Person(id=f"p{len(people) + 1:02d}", kind="placeholder", name=name))
    stems = R.sample(L["stems"], n_sub)
    for st in stems:
        people.append(Person(id=f"p{len(people) + 1:02d}", kind="subcontractor",
                             name=f"{st} {R.choice(L['trades'])}"))
    emps = [p for p in people if p.kind == "employee"]
    workers = [p for p in emps if not p.idle_all]

    # ------------------------------------------------------------------ crews
    crews: list[Crew] = []
    if P.family == "crew":
        max_crews = max(1, min(4, (len(workers) - 1) // 2))
        n_crews = R.randint(1, max_crews)
        pool = list(workers)
        R.shuffle(pool)
        labels = _crew_labels(P.crew_label_style)
        for c in range(n_crews):
            size = R.randint(2, 4)
            if len(pool) - size < (1 if c == n_crews - 1 and n_crews == 1 else 0):
                size = max(2, len(pool) - 1)
            if len(pool) < 2:
                break
            mem = pool[:size]
            pool = pool[size:]
            mem.sort(key=lambda p: p.id)
            crews.append(Crew(id=f"c{c + 1}", label=labels[c], members=[p.id for p in mem]))
            for p in mem:
                p.crew = crews[-1].id
    individuals = [p for p in workers if p.crew is None]
    n_units = len(crews) + len(individuals)

    # ------------------------------------------------------------------ sites
    J = _clamp(round(len(workers) / R.uniform(1.6, 3.2)), 2, 25)
    J = _clamp(min(J, n_units), 2, 25)
    sites = _make_sites(R, J, P)
    n_int = R.choices([0, 1, 2], [62, 33, 5])[0]
    for k, nm in enumerate(R.sample(INTERNAL, n_int)):
        sites.append(Site(id=f"s{len(sites) + 1:02d}", kind="internal", name=nm, internal=True))
    jobs = [s for s in sites if not s.internal]

    # ------------------------------------------------------------------ spans
    nd = P.ndays
    spans: dict = {}
    for s in jobs:
        for w in range(P.weeks):
            r = R.random()
            if r < 0.65:
                sp = (0, nd - 1)
            elif r < 0.9 or P.weeks == 1:
                a = R.randint(0, nd - 1)
                sp = (a, R.randint(a, nd - 1))
            else:
                sp = None
            spans[(s.id, w)] = sp
        if all(spans[(s.id, w)] is None for w in range(P.weeks)):
            spans[(s.id, 0)] = (0, nd - 1)

    T = Truth(seed=seed, weeks=P.weeks, start=P.start, ndays=nd, dates=dates, people=people,
              sites=sites, crews=crews, recs=[], spans=spans, clean=W.clean)
    T.index()

    def act(d: dt.date) -> list[str]:
        return [s.id for s in jobs if T.active(s.id, d)]

    # every date needs >= 1 active job; supply (units) must cover demand (active jobs)
    for d in dates:
        w, i = (d - P.start).days // 7, (d - P.start).days % 7
        if not act(d):
            cand = [s.id for s in jobs if spans[(s.id, w)] is not None]
            sid = cand[0] if cand else jobs[0].id
            sp = spans[(sid, w)]
            spans[(sid, w)] = (min(sp[0], i), max(sp[1], i)) if sp else (i, i)
        while len(act(d)) > n_units:
            sid = act(d)[-1]
            a, b = spans[(sid, w)]
            if i == a:
                a += 1
            elif i == b:
                b -= 1
            else:
                b = i - 1
            spans[(sid, w)] = (a, b) if a <= b else None

    # ------------------------------------------------------------------ statuses
    status: dict = {}  # (pid, date) -> status
    vac_weeks = {}
    for p in individuals:
        for w in range(P.weeks):
            vac_weeks[(p.id, w)] = R.random() < 0.04
        for d in dates:
            if R.random() < 0.03:
                status[(p.id, d)] = R.choices(STATUSES, [35, 20, 15, 15, 5, 10])[0]
    for (pid, w), on in vac_weeks.items():
        if on:
            for d in T.week_dates(w):
                status[(pid, d)] = "vacation"
    # drop statuses that would leave fewer units than active jobs
    for d in dates:
        off = [p.id for p in individuals if (p.id, d) in status]
        while off and n_units - len(off) < len(act(d)):
            del status[(off.pop(), d)]

    # ------------------------------------------------------------------ staffing
    units = [("crew", c.id) for c in crews] + [("ind", p.id) for p in individuals]
    extras = [p for p in people if p.kind != "employee"]
    extra_days = {}
    for p in extras:
        rate = 0.7 if p.kind == "placeholder" else 0.5
        days = [d for d in dates if R.random() < rate]
        if not days:
            days = [dates[R.randrange(len(dates))]]
        extra_days[p.id] = set(days)
    home = {}
    for u in units:
        for w in range(P.weeks):
            wk = [s.id for s in jobs if spans[(s.id, w)] is not None]
            home[(u, w)] = R.choice(wk) if wk else None
    internals = [s.id for s in sites if s.internal]
    crew_map = {c.id: c for c in crews}
    recs: list[Rec] = []
    for d in dates:
        w = (d - P.start).days // 7
        A = act(d)
        avail = [u for u in units if not (u[0] == "ind" and (u[1], d) in status)]
        order = list(A)
        R.shuffle(order)
        load = {j: [] for j in A}
        assign = {}
        for u in avail:
            h = home[(u, w)]
            if h in load:
                assign[u] = h
                load[h].append(u)
        for u in avail:
            if u in assign:
                continue
            if internals and u[0] == "ind" and R.random() < 0.12:
                assign[u] = internals[R.randrange(len(internals))]
                continue
            j = min(order, key=lambda j: len(load[j]))
            assign[u] = j
            load[j].append(u)
        for j in order:
            if load[j]:
                continue
            donors = [k for k in order if len(load[k]) >= 2]
            if donors:
                k = max(donors, key=lambda k: len(load[k]))
                u = load[k].pop()
            else:
                cand = [u for u in avail if assign.get(u) in internals]
                if not cand:
                    raise AssertionError("cannot staff every active job")
                u = cand[0]
            assign[u] = j
            load[j].append(u)
        for u in avail:
            sid = assign[u]
            if u[0] == "crew":
                for pid in crew_map[u[1]].members:
                    recs.append(Rec("work", d, pid=pid, sid=sid, crew=u[1]))
            else:
                recs.append(Rec("work", d, pid=u[1], sid=sid))
        for p in extras:
            if d in extra_days[p.id] and A:
                recs.append(Rec("work", d, pid=p.id, sid=A[R.randrange(len(A))]))
    for (pid, d), st in sorted(status.items(), key=lambda kv: (kv[0][1], kv[0][0])):
        recs.append(Rec("status", d, pid=pid, status=st))
    if P.times_mode:
        for r in recs:
            if r.kind == "work" and R.random() < P.time_rate:
                r.start, r.end = SHIFTS[R.randrange(len(SHIFTS))]
    T.recs = recs
    _plant(R, T, P, W)
    return T


def _crew_labels(style: int) -> list[str]:
    return [
        ["Crew A", "Crew B", "Crew C", "Crew D"],
        ["Crew 1", "Crew 2", "Crew 3", "Crew 4"],
        ["Red Crew", "Blue Crew", "Green Crew", "Gold Crew"],
        ["Team 1", "Team 2", "Team 3", "Team 4"],
    ][style]


def _make_sites(R: random.Random, J: int, P: Plan) -> list[Site]:
    L = lists()
    sites: list[Site] = []
    used = set()
    used_words = set()  # customers / streets / places used, kept disjoint
    while len(sites) < J:
        kind = wpick(R, {"address": 45, "customer": 25, "jobno": 15, "commercial": 15})
        if kind == "address":
            street = R.choice(L["streets"])
            if fold(street) in used_words:
                continue
            suffix = R.choice(list(SUFFIXES))
            num = R.choice([R.randint(100, 999), R.randint(1000, 9999), R.randint(10, 99), R.randint(10000, 29999)])
            direc = R.choice(["N", "S", "E", "W"]) if R.random() < 0.1 else ""
            full = R.random() < 0.3 and suffix != "Way"
            suf_txt = SUFFIXES[suffix] if full else suffix
            name = f"{num} {direc + ' ' if direc else ''}{street} {suf_txt}"
            parts = {"num": str(num), "dir": direc, "street": street, "suffix": suffix, "full": full}
            used_words.add(fold(street))
        elif kind == "customer":
            c = R.choice(L["customers"])
            if fold(c) in used_words:
                continue
            name = f"{c} {R.choice(CUSTOMER_JOBS)}"
            parts = {"customer": c}
            used_words.add(fold(c))
        elif kind == "jobno":
            pl = R.choice(L["places"])
            if fold(pl) in used_words:
                continue
            n = R.randint(1000, 2999)
            style = R.randrange(3)
            num_txt = (f"J-{n}", f"#{n}", f"Job {n}")[style]
            name = f"{num_txt} {pl}"
            parts = {"place": pl, "num": num_txt}
            used_words.add(fold(pl))
        else:
            tpl = R.choice(L["commercial"])
            pl = R.choice(L["places"])
            if "{P}" in tpl and fold(pl) in used_words:
                continue
            name = tpl.replace("{P}", pl).replace("{N}", str(R.randint(2, 4999)))
            parts = {"place": pl if "{P}" in tpl else ""}
            if "{P}" in tpl:
                used_words.add(fold(pl))
        if fold(name) in used:
            continue
        used.add(fold(name))
        sites.append(Site(id=f"s{len(sites) + 1:02d}", kind=kind, name=name, parts=parts))
    if P.corr.get("lookalike_sites"):
        addr = [s for s in sites if s.kind == "address" and len(s.parts["num"]) >= 3]
        if len(addr) >= 2:
            a, b = addr[0], addr[1]
            n = a.parts["num"]
            swapped = n[0] + n[2] + n[1] + n[3:] if n[1] != n[2] else n[:-1] + str((int(n[-1]) + 5) % 10)
            b.parts = dict(a.parts, num=swapped)
            p = b.parts
            b.name = f"{swapped} {p['dir'] + ' ' if p['dir'] else ''}{p['street']} {SUFFIXES[p['suffix']] if p['full'] else p['suffix']}"
            b.parts["lookalike_of"] = a.id
    return sites


# ---------------------------------------------------------------------------- planting

def _plant(R: random.Random, T: Truth, P: Plan, W: Wants) -> None:
    jobs = [s.id for s in T.sites if not s.internal]
    internals = [s.id for s in T.sites if s.internal]
    emp_ids = [p.id for p in T.people if p.kind == "employee" and not p.idle_all]
    crew_member = {p.id for p in T.people if p.crew}
    busy: set = set()
    touched: set = set()  # (sid, week) with a planted unstaffed day
    planted: list[dict] = []
    info: list[dict] = []
    job_grid = P.family == "jgrid"
    jobset = set(jobs)

    BPD: dict = {}
    BSD: dict = {}
    for r in T.recs:
        if r.pid:
            BPD.setdefault((r.pid, r.date), []).append(r)
        if r.kind == "work":
            BSD.setdefault((r.sid, r.date), []).append(r)

    def add(r: Rec) -> None:
        T.recs.append(r)
        if r.pid:
            BPD.setdefault((r.pid, r.date), []).append(r)
        if r.kind == "work":
            BSD.setdefault((r.sid, r.date), []).append(r)

    def remove(r: Rec) -> None:
        T.recs.remove(r)
        BPD[(r.pid, r.date)].remove(r)
        BSD[(r.sid, r.date)].remove(r)

    def move(r: Rec, sid: str) -> None:
        BSD[(r.sid, r.date)].remove(r)
        r.sid = sid
        BSD.setdefault((sid, r.date), []).append(r)

    def by_pd(pid, d):
        return BPD.get((pid, d), [])

    def workers_at(sid, d):
        return BSD.get((sid, d), [])

    def week(d):
        return (d - T.start).days // 7

    def other_jobs(d, exclude):
        return [j for j in jobs if j != exclude and T.active(j, d) and BSD.get((j, d))
                and (j, d) not in unstaffed_days]

    def single_job(pid, d):
        rs = BPD.get((pid, d))
        if rs and len(rs) == 1 and rs[0].kind == "work" and rs[0].sid in jobset:
            return rs[0]
        return None

    unstaffed_days: set = set()
    # ---- unstaffed (moves workers away)
    for k in range(W.un):
        explicit = W.un_explicit[k]
        cands = []
        for j in jobs:
            for d in T.dates:
                if not T.active(j, d) or (j, week(d)) in touched:
                    continue
                ws = workers_at(j, d)
                if not ws or any((r.pid, d) in busy for r in ws):
                    continue
                if not explicit and not job_grid:
                    w, i = week(d), (d - T.start).days % 7
                    a, b = T.spans[(j, w)]
                    if not (a < i < b):
                        continue
                if not other_jobs(d, j):
                    continue
                cands.append((j, d))
        if not cands:
            continue
        j, d = cands[R.randrange(len(cands))]
        ws = list(workers_at(j, d))
        targets = other_jobs(d, j)
        moved_crews: dict = {}
        for r in ws:
            if r.crew:
                if r.crew not in moved_crews:
                    moved_crews[r.crew] = targets[R.randrange(len(targets))]
                move(r, moved_crews[r.crew])
            else:
                move(r, targets[R.randrange(len(targets))])
        if explicit:
            add(Rec("unstaffed", d, sid=j, tag="tbd"))
        unstaffed_days.add((j, d))
        touched.add((j, week(d)))
        planted.append({"type": "unstaffed", "site": j, "date": d,
                        "mode": "explicit" if explicit else "inferred"})

    # ---- not booked (removes a person's only record that day)
    nb_left = W.nb
    if W.idle_all:
        idle = next(p for p in T.people if p.idle_all)
        for d in T.dates:
            planted.append({"type": "not_booked", "person": idle.id, "date": d, "mode": "idle_all"})
        nb_left = max(0, nb_left - 1)
    for _ in range(nb_left):
        cands = []
        if P.family == "crew":
            # crew layouts: only crew members can go unbooked (a crew left off one day), so the
            # result is the same whether the roster is the crew roster block or everyone seen.
            for c in T.crews:
                for d in T.dates:
                    if any((m, d) in busy for m in c.members):
                        continue
                    rl = [by_pd(m, d) for m in c.members]
                    if not all(len(x) == 1 and x[0].kind == "work" and x[0].crew == c.id for x in rl):
                        continue
                    sid = rl[0][0].sid
                    if sid in jobset and len(workers_at(sid, d)) <= len(c.members):
                        continue
                    cands.append((c, d, [x[0] for x in rl]))
            if not cands:
                break
            c, d, rl = cands[R.randrange(len(cands))]
            for r in rl:
                remove(r)
                busy.add((r.pid, d))
                planted.append({"type": "not_booked", "person": r.pid, "date": d, "mode": "crew_day"})
            continue
        for pid in emp_ids:
            if pid in crew_member:
                continue
            for d in T.dates:
                if (pid, d) in busy:
                    continue
                rs = by_pd(pid, d)
                if len(rs) != 1 or rs[0].kind != "work":
                    continue
                s = rs[0].sid
                if s in jobs and len(workers_at(s, d)) < 2:
                    continue
                cands.append((pid, d, rs[0]))
        if not cands:
            break
        pid, d, r = cands[R.randrange(len(cands))]
        remove(r)
        busy.add((pid, d))
        planted.append({"type": "not_booked", "person": pid, "date": d, "mode": "day"})

    def add_plant(n, pick_ok, action):
        for _ in range(n):
            cands = []
            for pid in emp_ids:
                for d in T.dates:
                    if (pid, d) in busy:
                        continue
                    r = single_job(pid, d)
                    if r is None or not pick_ok(pid, d, r):
                        continue
                    cands.append((pid, d, r))
            if not cands:
                return
            pid, d, r = cands[R.randrange(len(cands))]
            busy.add((pid, d))
            action(pid, d, r)

    # ---- double booked
    def do_db(pid, d, r):
        others = other_jobs(d, r.sid)
        k = others[R.randrange(len(others))]
        new = Rec("work", d, pid=pid, sid=k, tag="db")
        if P.times_mode:
            if R.random() < 0.5:
                r.start, r.end = "07:00", "15:30"
                new.start, new.end = "12:00", "16:00"
            elif R.random() < 0.5:
                new.start = new.end = None
            else:
                r.start = r.end = None
        add(new)
        planted.append({"type": "double_booked", "person": pid, "date": d, "sites": sorted([r.sid, k])})

    add_plant(W.db, lambda pid, d, r: bool(other_jobs(d, r.sid)), do_db)

    # ---- assigned while off
    def do_awo(pid, d, r):
        st = R.choices(["off", "pto", "vacation", "sick", "out"], [30, 30, 15, 15, 10])[0]
        add(Rec("status", d, pid=pid, status=st, tag="awo"))
        planted.append({"type": "assigned_while_off", "person": pid, "date": d, "sites": [r.sid], "status": st})

    add_plant(W.awo, lambda pid, d, r: True, do_awo)

    # ---- info: internal + site
    if W.ips and internals:
        def do_ips(pid, d, r):
            r.start = r.end = None
            sid = internals[R.randrange(len(internals))]
            add(Rec("work", d, pid=pid, sid=sid, tag="ips"))
            info.append({"type": "internal_plus_site", "person": pid, "date": d, "sites": sorted([sid, r.sid])})
        add_plant(1, lambda pid, d, r: True, do_ips)

    # ---- info: duplicate entry
    if W.dup:
        def do_dup(pid, d, r):
            add(Rec("work", d, pid=pid, sid=r.sid, tag="dup", start=r.start, end=r.end, crew=None))
            info.append({"type": "duplicate_entry", "person": pid, "date": d, "sites": [r.sid]})
        add_plant(1, lambda pid, d, r: r.crew is None, do_dup)

    # ---- info: split day (times only)
    if W.split:
        def do_split(pid, d, r):
            others = other_jobs(d, r.sid)
            k = others[R.randrange(len(others))]
            (s1, e1), (s2, e2) = R.choice([(("07:00", "11:00"), ("12:00", "15:30")),
                                           (("06:30", "10:30"), ("11:00", "15:00")),
                                           (("07:00", "12:00"), ("13:00", "16:00"))])
            r.start, r.end = s1, e1
            add(Rec("work", d, pid=pid, sid=k, tag="split", start=s2, end=e2))
            info.append({"type": "split_day", "person": pid, "date": d, "sites": sorted([r.sid, k])})
        add_plant(1, lambda pid, d, r: r.crew is None and bool(other_jobs(d, r.sid)), do_split)

    # ---- info: subcontractor at two sites
    subs = [p.id for p in T.people if p.kind == "subcontractor"]
    if W.submulti and subs:
        cands = []
        for pid in subs:
            for d in T.dates:
                rs = by_pd(pid, d)
                if len(rs) == 1 and other_jobs(d, rs[0].sid):
                    cands.append((pid, d, rs[0]))
        if cands:
            pid, d, r = cands[R.randrange(len(cands))]
            others = other_jobs(d, r.sid)
            k = others[R.randrange(len(others))]
            r.start = r.end = None
            add(Rec("work", d, pid=pid, sid=k, tag="submulti"))
            info.append({"type": "subcontractor_multi_site", "person": pid, "date": d, "sites": sorted([r.sid, k])})

    T.planted = planted
    T.planted_info = info
