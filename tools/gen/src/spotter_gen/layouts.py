"""Step 2, render: turn the truth into bands of blocks for each layout. Cells hold plain
values or Txt fragments (str | Ref); every Rec gets its person/site Refs and words here, so
the expected assignments are exactly what was written."""

from __future__ import annotations

import datetime as dt
import random

from .model import MONTHS, MON3, STATUSES, WD_ES, WD_LONG, WD_SHORT, Rec, Truth, fmt_date, week_of
from .plan import Plan
from .present import Presenter, Txt, time_point_text, time_range_text
from .sheet import Band, Block, text_band

HEAD_EN = {
    "person": ["Name", "Employee", "Tech", "Crew", "Worker", "Guy"],
    "site": ["Job", "Site", "Project", "Address", "Customer", "Jobsite", "Location"],
    "date": ["Date"],
    "day": ["Day"],
    "time": ["Time", "Hours", "Shift"],
    "start": ["Start", "Start time", "Begin"],
    "end": ["End", "End time", "Finish"],
    "status": ["Status", "Off?"],
    "unit": ["Crew", "Crew/Name", "Who"],
    "names": ["Crew", "Workers", "Who", "Guys", "Assigned"],
    "roster": ["Roster", "Employees", "Crew list", "Team"],
    "crews": ["Crews", "Crew roster", "Crew list"],
    "total": ["Total", "TOTAL", "Totals"],
}
HEAD_ES = {
    "person": ["Empleado", "Nombre", "Trabajador"],
    "site": ["Obra", "Trabajo", "Proyecto", "Sitio"],
    "date": ["Fecha"],
    "day": ["Día"],
    "time": ["Horario", "Horas"],
    "start": ["Entrada"],
    "end": ["Salida"],
    "status": ["Estado"],
    "unit": ["Cuadrilla"],
    "names": ["Trabajadores", "Personal"],
    "roster": ["Empleados", "Personal"],
    "crews": ["Cuadrillas"],
    "total": ["Total"],
}
SUPER = {"en": {"person": "Who", "site": "Where", "date": "When", "day": "When", "time": "Hours",
                "start": "Hours", "end": "Hours", "status": "Who", "unit": "Who"},
         "es": {"person": "Quién", "site": "Dónde", "date": "Cuándo", "day": "Cuándo", "time": "Horas",
                "start": "Horas", "end": "Horas", "status": "Quién", "unit": "Quién"}}
TITLES = ["{co} - Crew Schedule", "CREW SCHEDULE", "Weekly Schedule", "Job Schedule", "Schedule",
          "{co} Schedule", "Field Schedule - Rev 2", "Who's Where"]
TITLES_ES = ["Programa de trabajo", "Horario semanal", "{co} - Horario"]
COMPANIES = ["Tight Ship Builders", "Valley Pro Remodeling", "Rivera Brothers Construction", "Summit Home Services",
             "North Fork Builders", "Big Oak Contracting", "Westside Remodel Co"]
FOOTERS = ["Call the office if you can't make it.", "Questions? Call the office.", "Signed: ________",
           "Approved by: ________", "Page 1 of 1", "Updated Thursday", "* PPE required on every site",
           "Revised - see Maria for changes"]


class Ctx:
    def __init__(self, T: Truth, P: Plan, pres: Presenter, shuffle_rng: random.Random | None):
        self.T, self.P, self.pres, self.R = T, P, pres, pres.R
        self.shuffle_rng = shuffle_rng
        self.by_pd: dict = {}
        self.by_sd: dict = {}
        self.unstaffed: dict = {}
        for r in T.recs:
            if r.kind == "unstaffed":
                self.unstaffed[(r.sid, r.date)] = r
            else:
                self.by_pd.setdefault((r.pid, r.date), []).append(r)
            if r.kind == "work":
                self.by_sd.setdefault((r.sid, r.date), []).append(r)
        self.inactive_cells: set = set()
        self.job_rows: list[str] = []
        self.date_labels: dict = {}
        self.extra_reserved: list[str] = []
        self.shuffled = False

    # ---------------------------------------------------------------- helpers
    def head(self, role: str) -> str:
        pool = (HEAD_ES if self.P.lang == "es" else HEAD_EN)[role]
        k = self.R.randrange(len(pool))
        return pool[0] if self.P.header_vocab == "plain" else pool[k]

    def dl(self, d: dt.date, fmt: str | None = None):
        f = fmt or self.P.date_fmt
        v = fmt_date(d, f, self.P.date_pad)
        if isinstance(v, str):
            self.date_labels[v] = d.isoformat()
        return v

    def week_of(self, d: dt.date) -> str:
        monday = d - dt.timedelta(days=d.weekday())
        # day-first files only use the month-name style, so no Week-of line reads two ways
        t = week_of(monday, 3 if self.P.date_fmt == "dmy4" else self.P.week_of_style)
        self.date_labels[t] = monday.isoformat()
        return t

    def join(self, parts: list, joiner: str | None = None) -> Txt:
        joiner = joiner or self.P.joiner
        out = Txt()
        n = len(parts)
        for i, p in enumerate(parts):
            if i:
                if joiner == ", " and self.P.oxford and i == n - 1 and n >= 2:
                    out.append(self.P.oxford)
                else:
                    out.append(joiner)
            out.extend(p) if isinstance(p, Txt) else out.append(p)
        return out

    def site_cell(self, r: Rec, ref) -> Txt:
        note = self.pres.maybe_note()
        if note:
            r.note = note
            return Txt([ref, " - ", note]) if self.P.note_style == " - " else Txt([ref, f" ({note})"])
        return Txt([ref])

    def needs_anchor(self) -> bool:
        P = self.P
        return P.weekday_only or (P.week_titles and P.date_fmt != "dmy4")

    def block_dates(self) -> list[list[dt.date]]:
        T = self.T
        if self.P.arrangement == "single":
            return [list(T.dates)]
        return [T.week_dates(w) for w in range(T.weeks)]


# ============================================================================ lists

def render_list(cx: Ctx) -> list[Band]:
    P, T, pres = cx.P, cx.T, cx.pres
    bands: list[Band] = []
    if P.list_style == "per_record":
        cols = {"date_person_site": ["date", "person", "site"], "person_date_site": ["person", "date", "site"],
                "date_site_person": ["date", "site", "person"],
                "person_site_date": ["person", "site", "date"]}[P.list_order]
        if P.day_col and not P.weekday_only and P.date_fmt != "wd_md":
            cols.insert(cols.index("date") + 1, "day")
        if P.status_col:
            cols.append("status")
        if P.times_mode:
            cols += ["time"] if P.time_cols == "range" else ["start", "end"]
    else:
        cols = ["date", "site", "names"] if P.list_order.startswith("date") else ["site", "date", "names"]
    merged = P.merged_col
    if P.list_style == "per_site" and merged:
        merged = "site" if merged == "site" else "date"
    labels = {c: cx.head("day" if (c == "date" and P.weekday_only) else c) for c in cols}
    for b, bdates in enumerate(cx.block_dates()):
        dset = set(bdates)
        recs = [r for r in T.recs if r.date in dset]
        specs = _list_specs(cx, recs, cols, merged, b) if P.list_style == "per_record" else _site_specs(cx, recs, merged, b)
        if cx.shuffle_rng is not None:
            specs = _shuffle_specs(specs, cx.shuffle_rng, merged is not None)
            cx.shuffled = True
        rows: list[list] = []
        merges: list = []
        top = 0
        if P.header_present:
            if P.two_row:
                sup = [SUPER[P.lang].get(c, "") for c in cols]
                rows.append(sup)
                j = 0
                while j < len(cols):
                    k = j
                    while k + 1 < len(cols) and sup[k + 1] == sup[j]:
                        k += 1
                    if k > j:
                        merges.append((0, j, 0, k))
                        for x in range(j + 1, k + 1):
                            rows[0][x] = None
                    j = k + 1
            rows.append([labels[c] for c in cols])
            top = len(rows)
        breakable = []
        prev_key = object()
        group_start = None
        mcol = cols.index(merged) if merged in cols else None
        for i, sp in enumerate(specs):
            row = [sp["cells"].get(c) for c in cols]
            if mcol is not None:
                if sp["gkey"] == prev_key:
                    row[mcol] = None
                else:
                    if group_start is not None and len(rows) - 1 > group_start:
                        merges.append((group_start, mcol, len(rows) - 1, mcol))
                    group_start = len(rows)
                    if i:
                        breakable.append(len(rows))
                prev_key = sp["gkey"]
            elif i:
                breakable.append(len(rows))
            rows.append(row)
        if mcol is not None and group_start is not None and len(rows) - 1 > group_start:
            merges.append((group_start, mcol, len(rows) - 1, mcol))
        if P.corr["totals"]:
            tot = [None] * len(cols)
            tot[0] = cx.head("total")
            tot[-1] = len(specs)
            rows.append(tot)
        blk = Block(rows, merges, "table", breakable)
        anchor = cx.needs_anchor() and (P.arrangement != "single" or T.weeks == 1)
        if anchor:
            bands.append(text_band([cx.week_of(bdates[0])], "title", gap=P.stack_gap if b else 0))
            bands.append(Band([blk], gap_before=P.title_gap))
        else:
            bands.append(Band([blk], gap_before=P.stack_gap if b else 0))
    return bands


def _list_specs(cx: Ctx, recs: list[Rec], cols: list[str], merged: str | None, b: int) -> list[dict]:
    P, pres = cx.P, cx.pres
    pord = {p.id: i for i, p in enumerate(cx.T.people)}
    sord = {s.id: i for i, s in enumerate(cx.T.sites)}

    def key(r: Rec):
        po = pord.get(r.pid, 999)
        so = sord.get(r.sid, 999)
        if P.sort_key == "person":
            return (po, r.date, so, r.kind)
        if P.sort_key == "site":
            return (so, r.date, po, r.kind)
        return (r.date, so if "site" in cols[:cols.index("person")] else po, po, so, r.kind)

    out = []
    for r in sorted(recs, key=key):
        cells: dict = {}
        cells["date"] = cx.dl(r.date)
        if "day" in cols:
            cells["day"] = cx.dl(r.date, "wd_es" if P.lang == "es" else ("wd_long" if P.date_pad else "wd_short"))
        if r.kind == "unstaffed":
            r.word = pres.marker()
            cells["person"] = r.word
            r.sref = pres.site(r.sid, slot=("mgs", b, r.sid) if merged == "site" else None)
            cells["site"] = Txt([r.sref])
            gk = {"date": r.date, "person": ("u", id(r)), "site": r.sid}.get(merged)
        else:
            r.pref = pres.person(r.pid, slot=("mgp", b, r.pid) if merged == "person" else None)
            cells["person"] = Txt([r.pref])
            if r.kind == "status":
                r.word = pres.status_word(r.status, alone=True, symbols_ok=False)
                if "status" in cols:
                    cells["status"] = r.word
                else:
                    cells["site"] = r.word
                gk = {"date": r.date, "person": r.pid, "site": ("st", id(r))}.get(merged)
            else:
                r.sref = pres.site(r.sid, slot=("mgs", b, r.sid) if merged == "site" else None)
                cells["site"] = cx.site_cell(r, r.sref) if merged != "site" else Txt([r.sref])
                if r.start and P.times_mode:
                    if "time" in cols:
                        r.tstr = time_range_text(r.start, r.end, P.time_text)
                        cells["time"] = r.tstr
                    else:
                        cells["start"] = time_point_text(r.start, P.time_text)
                        cells["end"] = time_point_text(r.end, P.time_text)
                        r.tstr = f"{cells['start']}|{cells['end']}"
                gk = {"date": r.date, "person": r.pid, "site": r.sid}.get(merged)
        out.append({"cells": cells, "gkey": gk})
    return out


def _site_specs(cx: Ctx, recs: list[Rec], merged: str | None, b: int) -> list[dict]:
    P, pres = cx.P, cx.pres
    pord = {p.id: i for i, p in enumerate(cx.T.people)}
    groups: dict = {}
    order: list = []
    for r in recs:
        if r.kind == "work":
            k = ("w", r.date, r.sid)
        elif r.kind == "status":
            k = ("s", r.date, r.status)
        else:
            k = ("u", r.date, r.sid)
        if k not in groups:
            groups[k] = []
            order.append(k)
        groups[k].append(r)
    sord = {s.id: i for i, s in enumerate(cx.T.sites)}

    def gkey(k):
        so = sord.get(k[2], 900 + (STATUSES.index(k[2]) if k[2] in STATUSES else 0))
        return (so, k[1]) if merged == "site" or P.sort_key == "site" else (k[1], k[0] == "s", so)

    out = []
    for k in sorted(order, key=gkey):
        rs = sorted(groups[k], key=lambda r: pord.get(r.pid, 999))
        cells: dict = {"date": cx.dl(k[1])}
        if k[0] == "w":
            sref = pres.site(k[2], slot=("mgs", b, k[2]) if merged == "site" else None)
            refs = []
            for r in rs:
                r.sref = sref
                r.pref = pres.person(r.pid)
                refs.append(r.pref)
            cells["site"] = Txt([sref])
            cells["names"] = cx.join(refs)
            gk = {"date": k[1], "site": k[2]}.get(merged)
        elif k[0] == "s":
            word = pres.status_word(k[2], alone=True, symbols_ok=False)
            refs = []
            for r in rs:
                r.word = word
                r.pref = pres.person(r.pid)
                refs.append(r.pref)
            cells["site"] = word
            cells["names"] = cx.join(refs)
            gk = {"date": k[1], "site": ("st", k[1], k[2])}.get(merged)
        else:
            r = rs[0]
            r.sref = pres.site(k[2], slot=("mgs", b, k[2]) if merged == "site" else None)
            r.word = pres.marker()
            cells["site"] = Txt([r.sref])
            cells["names"] = r.word
            gk = {"date": k[1], "site": k[2]}.get(merged)
        out.append({"cells": cells, "gkey": gk})
    return out


def _shuffle_specs(specs: list[dict], rng: random.Random, grouped: bool) -> list[dict]:
    if not grouped:
        specs = list(specs)
        rng.shuffle(specs)
        return specs
    groups: list[list[dict]] = []
    for sp in specs:
        if groups and groups[-1][0]["gkey"] == sp["gkey"]:
            groups[-1].append(sp)
        else:
            groups.append([sp])
    rng.shuffle(groups)
    out = []
    for g in groups:
        rng.shuffle(g)
        out += g
    return out


# ============================================================================ grids

def _grid_header(cx: Ctx, bdates: list[dt.date], corner: str | None) -> tuple[list[list], list]:
    """Header row(s) for an entity x day grid (in rows orientation)."""
    P = cx.P
    rows: list[list] = []
    merges: list = []
    labels = [cx.dl(d) for d in bdates]
    if P.two_row:
        sup: list = [corner]
        start = 1
        for i, d in enumerate(bdates):
            if i == 0 or d.weekday() < bdates[i - 1].weekday():
                if i:
                    merges.append((0, start, 0, i))
                start = i + 1
                sup.append(cx.week_of(d))
            else:
                sup.append(None)
        merges.append((0, start, 0, len(bdates)))
        merges = [m for m in merges if m[3] > m[1]]
        merges.append((0, 0, 1, 0))
        rows.append(sup)
        rows.append([None] + labels)
    else:
        rows.append([corner] + labels)
    return rows, merges


def _finish_grid(cx: Ctx, rows: list[list], merges: list, top: int, n_body: int,
                 b: int, bdates: list[dt.date], bands: list[Band]) -> None:
    P, T = cx.P, cx.T
    blk = Block(rows, merges, "table", list(range(top + 1, top + n_body)))
    if P.orient == "cols":
        blk = blk.transpose()  # day columns become data rows 1..n
        blk.breakable = list(range(2, 1 + len(bdates)))
    anchor = cx.needs_anchor() and not P.two_row
    gap = P.stack_gap if b else 0
    if anchor and P.arrangement == "side_by_side":
        blk.rows.insert(0, [cx.week_of(bdates[0])])
        blk.merges = [(r1 + 1, c1, r2 + 1, c2) for (r1, c1, r2, c2) in blk.merges]
        blk.breakable = [x + 1 for x in blk.breakable]
    elif anchor:
        bands.append(text_band([cx.week_of(bdates[0])], "title", gap=gap))
        gap = 0
    if P.arrangement == "side_by_side" and bands and b:
        bands[-1].blocks.append(blk)
        bands[-1].col_gaps.append(1)
    else:
        bands.append(Band([blk], gap_before=gap))


def render_pgrid(cx: Ctx) -> list[Band]:
    P, T, pres = cx.P, cx.T, cx.pres
    bands: list[Band] = []
    corner = cx.head("person") if P.corner_label else None
    for b, bdates in enumerate(cx.block_dates()):
        rows, merges = _grid_header(cx, bdates, corner)
        top = len(rows)
        n_body = 0
        for p in T.people:
            label = pres.person(p.id, slot=("row", b))
            row: list = [Txt([label])]
            for d in bdates:
                rs = cx.by_pd.get((p.id, d), [])
                for r in rs:
                    r.pref = label
                row.append(_person_cell(cx, rs) if rs else None)
            rows.append(row)
            n_body += 1
        un = [cx.unstaffed[(s.id, d)] for d in bdates for s in T.sites if (s.id, d) in cx.unstaffed]
        if un:
            label = pres.marker()
            row = [label]
            for d in bdates:
                parts = []
                for s in T.sites:
                    r = cx.unstaffed.get((s.id, d))
                    if r is not None:
                        r.word = label
                        r.sref = pres.site(s.id)
                        parts.append(Txt([r.sref]))
                row.append(cx.join(parts, P.site_joiner) if parts else None)
            rows.append(row)
            n_body += 1
        if P.corr["totals"]:
            rows.append([cx.head("total")] + [sum(1 for p in T.people if any(
                r.kind == "work" for r in cx.by_pd.get((p.id, d), []))) for d in bdates])
        _finish_grid(cx, rows, merges, top, n_body, b, bdates, bands)
    return bands


def _person_cell(cx: Ctx, rs: list[Rec]) -> Txt:
    pres, P = cx.pres, cx.P
    stats = [r for r in rs if r.kind == "status"]
    works = [r for r in rs if r.kind == "work"]
    alone = len(rs) == 1
    sp = []
    for r in stats:
        r.word = pres.status_word(r.status, alone=alone, symbols_ok=True)
        sp.append(r.word)
    wp = []
    for r in works:
        r.sref = pres.site(r.sid)
        wp.append(cx.site_cell(r, r.sref) if alone else Txt([r.sref]))
    parts = sp + wp if P.status_first else wp + sp
    return cx.join(parts, P.site_joiner)


def render_jgrid(cx: Ctx, crew_mode: bool = False) -> list[Band]:
    P, T, pres = cx.P, cx.T, cx.pres
    bands: list[Band] = []
    corner = cx.head("site") if P.corner_label else None
    sites = [s for s in T.sites if not s.internal] + [s for s in T.sites if s.internal]
    crews = {c.id: c for c in T.crews}
    cx.job_rows = [s.id for s in sites if not s.internal]
    for b, bdates in enumerate(cx.block_dates()):
        rows, merges = _grid_header(cx, bdates, corner)
        top = len(rows)
        n_body = 0
        for s in sites:
            label = pres.site(s.id, slot=("row", b))
            row: list = [Txt([label])]
            for d in bdates:
                ws = sorted(cx.by_sd.get((s.id, d), []), key=lambda r: (r.crew is None, r.crew or "", r.pid))
                if ws:
                    parts: list = []
                    seen_crew = set()
                    for r in ws:
                        r.sref = label
                        if crew_mode and r.crew:
                            r.pref = pres.person(r.pid, slot="crew")
                            if r.crew not in seen_crew:
                                seen_crew.add(r.crew)
                                parts.append(crews[r.crew].label)
                        else:
                            r.pref = pres.person(r.pid)
                            parts.append(Txt([r.pref]))
                    row.append(cx.join(parts))
                elif (s.id, d) in cx.unstaffed:
                    r = cx.unstaffed[(s.id, d)]
                    r.sref = label
                    r.word = pres.marker()
                    row.append(r.word)
                elif s.internal or T.active(s.id, d):
                    row.append(None)
                else:
                    cx.inactive_cells.add((s.id, d))
                    row.append(pres.inactive())
            rows.append(row)
            n_body += 1
        for st in STATUSES:
            day_recs = {d: [r for p in T.people for r in cx.by_pd.get((p.id, d), [])
                            if r.kind == "status" and r.status == st] for d in bdates}
            if not any(day_recs.values()):
                continue
            label = pres.status_word(st, alone=True, symbols_ok=False)
            row = [label]
            for d in bdates:
                parts = []
                for r in day_recs[d]:
                    r.word = label
                    r.pref = pres.person(r.pid, slot="crew" if (crew_mode and T.person(r.pid).crew) else None)
                    parts.append(Txt([r.pref]))
                row.append(cx.join(parts) if parts else None)
            rows.append(row)
            n_body += 1
        if P.corr["totals"]:
            rows.append([cx.head("total")] + [len(set(r.pid for s in sites for r in cx.by_sd.get((s.id, d), [])))
                                               for d in bdates])
        _finish_grid(cx, rows, merges, top, n_body, b, bdates, bands)
    return bands


# ============================================================================ crews

def render_crew(cx: Ctx) -> list[Band]:
    P, T, pres = cx.P, cx.T, cx.pres
    if P.crew_form == "grid":
        saved = P.orient
        P.orient = "rows"
        if P.date_fmt == "serial":
            P.date_fmt = "mdy4"
        bands = render_jgrid(cx, crew_mode=True)
        P.orient = saved
    else:
        bands = _crew_list(cx)
    bands.append(_crew_roster(cx))
    cx.extra_reserved += [c.label for c in T.crews]
    return bands


def _crew_list(cx: Ctx) -> list[Band]:
    P, T, pres = cx.P, cx.T, cx.pres
    crews = {c.id: c for c in T.crews}
    sord = {s.id: i for i, s in enumerate(T.sites)}
    bands: list[Band] = []
    cols_lbl = [cx.head("day" if P.weekday_only else "date"), cx.head("unit"), cx.head("site")]
    for b, bdates in enumerate(cx.block_dates()):
        specs = []
        for d in bdates:
            done_crews = set()
            items = []
            for r in T.recs:
                if r.date != d:
                    continue
                if r.kind == "work" and r.crew:
                    if r.crew in done_crews:
                        continue
                    done_crews.add(r.crew)
                    mem = [x for x in cx.by_sd.get((r.sid, d), []) if x.crew == r.crew]
                    items.append((sord[r.sid], 0, r.crew, ("crew", r.crew, r.sid, mem)))
                elif r.kind == "work":
                    items.append((sord[r.sid], 1, r.pid, ("ind", r)))
                elif r.kind == "status":
                    items.append((900, 2, r.pid, ("st", r)))
                else:
                    items.append((sord[r.sid], 3, "", ("un", r)))
            for it in sorted(items, key=lambda x: x[:3]):
                kind = it[3][0]
                if kind == "crew":
                    _, cid, sid, mem = it[3]
                    sref = pres.site(sid)
                    for m in mem:
                        m.sref = sref
                        m.pref = pres.person(m.pid, slot="crew")
                    specs.append([cx.dl(d), crews[cid].label, Txt([sref])])
                elif kind == "ind":
                    r = it[3][1]
                    r.pref = pres.person(r.pid)
                    r.sref = pres.site(r.sid)
                    specs.append([cx.dl(d), Txt([r.pref]), Txt([r.sref])])
                elif kind == "st":
                    r = it[3][1]
                    r.pref = pres.person(r.pid, slot="crew" if T.person(r.pid).crew else None)
                    r.word = pres.status_word(r.status, alone=True)
                    specs.append([cx.dl(d), Txt([r.pref]), r.word])
                else:
                    r = it[3][1]
                    r.word = pres.marker()
                    r.sref = pres.site(r.sid)
                    specs.append([cx.dl(d), r.word, Txt([r.sref])])
        if cx.shuffle_rng is not None:
            cx.shuffle_rng.shuffle(specs)
            cx.shuffled = True
        rows = [cols_lbl] if P.header_present else []
        top = len(rows)
        rows += specs
        if P.corr["totals"]:
            rows.append([cx.head("total"), None, len(specs)])
        blk = Block(rows, [], "table", list(range(top + 1, top + len(specs))))
        if cx.needs_anchor() and (P.arrangement != "single" or T.weeks == 1):
            bands.append(text_band([cx.week_of(bdates[0])], "title", gap=P.stack_gap if b else 0))
            bands.append(Band([blk]))
        else:
            bands.append(Band([blk], gap_before=P.stack_gap if b else 0))
    return bands


def _crew_roster(cx: Ctx) -> Band:
    P, T, pres = cx.P, cx.T, cx.pres
    rows: list[list] = []
    with_head = cx.R.random() < 0.5
    if P.crew_roster_form == "columns":
        rows.append([c.label for c in T.crews])
        depth = max(len(c.members) for c in T.crews)
        for k in range(depth):
            rows.append([Txt([pres.person(c.members[k], slot="crew")]) if k < len(c.members) else None
                         for c in T.crews])
        if with_head:
            rows.insert(0, [cx.head("crews")])
    else:
        if with_head:
            rows.append([cx.head("crews")])
        for c in T.crews:
            refs = [Txt([pres.person(pid, slot="crew")]) for pid in c.members]
            if P.crew_roster_form == "colon":
                rows.append([Txt([c.label + ": "] + list(cx.join(refs, ", ")))])
            else:
                rows.append([c.label] + refs)
    return Band([Block(rows, [], "crews")], gap_before=1)


# ============================================================================ calendar

def render_calendar(cx: Ctx) -> list[Band]:
    P, T, pres = cx.P, cx.T, cx.pres
    first, last = T.dates[0], T.dates[-1]
    if first.month == last.month:
        title = f"{MONTHS[first.month - 1]} {first.year}"
    else:
        title = f"{MON3[first.month - 1]} – {MON3[last.month - 1]} {last.year}"
    order = [6, 0, 1, 2, 3, 4, 5] if P.cal_start == "sun" else [0, 1, 2, 3, 4, 5, 6]
    names = WD_ES if P.lang == "es" else (WD_LONG if P.cal_wd_long else WD_SHORT)
    rows: list[list] = [[names[i] for i in order]]
    breakable = []
    sep = P.cal_sep
    for w in range(T.weeks):
        monday = T.week_dates(w)[0]
        cdates = [monday + dt.timedelta(days=(-1 if P.cal_start == "sun" else 0) + k) for k in range(7)]
        if w:
            breakable.append(len(rows))
        nums = []
        for d in cdates:
            if P.cal_day == "md":
                nums.append(cx.dl(d, "md"))
            else:
                nums.append(d.day)
        rows.append(nums)
        entries = [_cal_entries(cx, d, sep) for d in cdates]
        depth = max((len(e) for e in entries), default=0)
        for k in range(depth):
            rows.append([e[k] if k < len(e) else None for e in entries])
    return [text_band([title], "title"), Band([Block(rows, [], "table", breakable)], gap_before=P.title_gap)]


def _cal_entries(cx: Ctx, d: dt.date, sep: str) -> list[Txt]:
    P, T, pres = cx.P, cx.T, cx.pres
    out: list[Txt] = []
    for s in T.sites:
        ws = [r for r in cx.by_sd.get((s.id, d), [])]
        if not ws:
            continue
        ws.sort(key=lambda r: (r.tag == "dup", r.pid))
        main = [r for r in ws if r.tag != "dup"]
        if P.cal_style == "per_site" and main:
            sref = pres.site(s.id)
            refs = []
            for r in main:
                r.sref = sref
                r.pref = pres.person(r.pid)
                refs.append(r.pref)
            out.append(Txt(list(cx.join(refs, ", " if P.joiner == "\n" else None)) + [sep, sref]))
        else:
            for r in main:
                r.sref = pres.site(s.id)
                r.pref = pres.person(r.pid)
                out.append(Txt([r.pref, sep, r.sref]))
        for r in ws:
            if r.tag == "dup":
                r.sref = pres.site(s.id)
                r.pref = pres.person(r.pid)
                out.append(Txt([r.pref, sep, r.sref]))
    for p in T.people:
        for r in cx.by_pd.get((p.id, d), []):
            if r.kind == "status":
                r.pref = pres.person(p.id)
                r.word = pres.status_word(r.status, alone=True)
                out.append(Txt([r.pref, sep, r.word]))
    for s in T.sites:
        r = cx.unstaffed.get((s.id, d))
        if r is not None:
            r.word = pres.marker()
            r.sref = pres.site(s.id)
            out.append(Txt([r.word, sep, r.sref]))
    return out


# ============================================================================ extras

def title_band(cx: Ctx) -> Band | None:
    P, R = cx.P, cx.R
    lines = []
    for k in range(P.title_count):
        pool = TITLES_ES if P.lang == "es" else TITLES
        t = pool[R.randrange(len(pool))].replace("{co}", COMPANIES[R.randrange(len(COMPANIES))])
        lines.append(t)
    if not lines:
        return None
    return text_band(lines, "title")


def footer_band(cx: Ctx) -> Band | None:
    P, R = cx.P, cx.R
    if not P.corr["footer"]:
        return None
    n = R.choice([1, 1, 2])
    lines = [FOOTERS[R.randrange(len(FOOTERS))] for _ in range(n)]
    return text_band(list(dict.fromkeys(lines)), "footer", gap=1)


def roster_block(cx: Ctx) -> Block:
    P, T, pres = cx.P, cx.T, cx.pres
    rows: list[list] = [[cx.head("roster")]]
    for p in T.people:
        if p.kind == "employee":
            rows.append([Txt([pres.person(p.id, slot="roster")])])
    return Block(rows, [], "roster")
