"""One seed -> (input file, expected.json, truth.json). Fully deterministic per seed."""

from __future__ import annotations

import json
import random
from dataclasses import dataclass

from . import layouts as L
from .expect import analyze, assignments, display_items, expected_json, flag_key
from .names import materialize
from .plan import draw_plan
from .present import Presenter, Ref, Txt
from .sheet import Band, compose, insert_blanks
from .truth import Wants, build_truth
from .writers import write_docx, write_ods, write_text, write_xls, write_xlsx

GENERATOR_VERSION = "1"
VARIANTS = ("none", "shuffle", "rename", "blank", "transpose")
HOLDOUT_START = 1_000_000


class GenError(AssertionError):
    pass


@dataclass
class Case:
    seed: int
    files: dict  # file name -> bytes
    expected: dict
    truth: dict


def _resolve(bands: list[Band], pres: Presenter, srng: random.Random, stray: bool) -> int:
    n = 0
    for band in bands:
        for blk in band.blocks:
            for row in blk.rows:
                for j, v in enumerate(row):
                    if not isinstance(v, Txt):
                        continue
                    s = "".join(pres.text(f) if isinstance(f, Ref) else f for f in v)
                    if stray and len(v) == 1 and isinstance(v[0], Ref):
                        u = srng.random()
                        if u < 0.04:
                            s, n = " " + s, n + 1
                        elif u < 0.08:
                            s, n = s + " ", n + 1
                        elif u < 0.12 and " " in s:
                            s, n = s.replace(" ", "  ", 1), n + 1
                    row[j] = s
    return n


def generate(seed: int, variant: str = "none", clean_rate: float = 0.2) -> Case:
    if variant not in VARIANTS:
        raise ValueError(variant)
    R = random.Random(seed)
    P = draw_plan(R)
    W = Wants(R, P, clean_rate)
    if W.idle_all and P.family in ("list", "jgrid", "calendar"):
        P.roster_block = True
    if P.merged_col == "site":
        P.status_col = False
    T = build_truth(R, P, W, seed)

    applied = variant == "none"
    if variant == "transpose" and P.family in ("pgrid", "jgrid"):
        P.orient = "cols" if P.orient == "rows" else "rows"
        applied = True
    if P.date_fmt == "dmy4" and not any(d.day > 12 for d in T.dates):
        P.date_fmt = "mdy4"
    if P.date_fmt == "serial" and P.family != "list" and not (P.family == "crew" and P.crew_form == "list"):
        P.date_fmt = "mdy4"
    emps = [p for p in T.people if p.kind == "employee"]
    nrng = random.Random(f"names:{seed}")
    exclude = None
    if variant == "rename":
        orig = materialize([p.spec for p in emps], random.Random(f"names:{seed}"))
        exclude = {tok for _, last in orig for tok in last.split()}
        nrng = random.Random(f"rename:{seed}")
        applied = True
    pres = Presenter(R, T, P, nrng, exclude)
    shuffle_rng = random.Random(f"shuffle:{seed}") if variant == "shuffle" else None
    cx = L.Ctx(T, P, pres, shuffle_rng)

    # ---------------------------------------------------------------- render
    title = L.title_band(cx)
    main = {"list": L.render_list, "pgrid": L.render_pgrid, "jgrid": L.render_jgrid,
            "crew": L.render_crew, "calendar": L.render_calendar}[P.family](cx)
    if P.roster_block:
        rb = L.roster_block(cx)
        host = next((b for b in main if len(b.blocks) == 1 and b.blocks[0].role == "table"), None)
        if P.roster_pos == "right" and host is not None and P.arrangement != "side_by_side":
            host.blocks.append(rb)
            host.col_gaps.append(1)
        else:
            main.append(Band([rb], gap_before=1))
    footer = L.footer_band(cx)
    bands = ([title] if title else []) + main + ([footer] if footer else [])
    if title and main:
        main[0].gap_before = max(main[0].gap_before, P.title_gap)
    pres.finalize(cx.extra_reserved)
    if variant == "shuffle":
        applied = cx.shuffled
    n_stray = _resolve(bands, pres, random.Random(f"spaces:{seed}"), P.corr["stray_spaces"])
    blank_log = insert_blanks(bands, random.Random(f"blank:{seed}")) if variant == "blank" else []
    if variant == "blank":
        applied = True

    # ---------------------------------------------------------------- write
    used_encoding = None
    if P.fmt == "docx":
        data = write_docx(bands, P)
    else:
        cv = compose(bands)
        if P.fmt == "xlsx":
            data = write_xlsx(cv, P)
        elif P.fmt == "xls":
            data = write_xls(cv, P)
        elif P.fmt == "ods":
            data = write_ods(cv, P)
        else:
            data, used_encoding = write_text(cv, P)
    input_name = "paste.txt" if P.fmt == "paste" else f"input.{P.fmt}"

    # ---------------------------------------------------------------- expect
    roster = {k[1] for k in pres.used if k[0] == "p" and T.person(k[1]).kind == "employee"}
    flags_id, info_id = analyze(T, P.layout, roster, cx.inactive_cells, cx.job_rows)
    want = sorted((p["type"], p.get("person") or p.get("site"), p["date"]) for p in T.planted)
    got = sorted(flag_key(f) for f in flags_id)
    if got != want:
        raise GenError(f"seed {seed}: planted flags {want} != analyzed {got}")
    want_i = sorted((i["type"], i["person"], i["date"]) for i in T.planted_info)
    got_i = sorted((i["type"], i["person"], i["date"]) for i in info_id)
    if got_i != want_i:
        raise GenError(f"seed {seed}: planted info {want_i} != analyzed {got_i}")
    if T.clean and any(f["type"] in ("double_booked", "assigned_while_off", "not_booked", "unstaffed") for f in flags_id):
        raise GenError(f"seed {seed}: clean schedule has flags")
    dp = lambda pid: pres.display[("p", pid)]  # noqa: E731
    ds = lambda sid: pres.display[("s", sid)]  # noqa: E731
    flags = display_items(flags_id, dp, ds)
    info = display_items(info_id, dp, ds)
    assigns = assignments(T, pres.text)
    category = "H" if seed >= HOLDOUT_START else "C"
    expected = expected_json(T, P.layout, P.arrangement, pres.tier, input_name, P.fmt == "paste",
                             assigns, flags, info, category)
    truth = _truth_json(seed, variant, applied, T, P, W, pres, cx, roster, used_encoding, n_stray, blank_log)
    files = {input_name: data,
             "expected.json": _dump(expected),
             "truth.json": _dump(truth)}
    return Case(seed, files, expected, truth)


def _dump(obj: dict) -> bytes:
    """Readable JSON: one top-level key per line, one list element per line (C encoder, fast)."""
    enc = json.JSONEncoder(ensure_ascii=False).encode
    out = ["{"]
    items = list(obj.items())
    for i, (k, v) in enumerate(items):
        comma = "," if i < len(items) - 1 else ""
        if isinstance(v, list) and v:
            out.append(f" {enc(k)}: [")
            out += [f"  {enc(e)}," for e in v[:-1]] + [f"  {enc(v[-1])}"]
            out.append(f" ]{comma}")
        else:
            out.append(f" {enc(k)}: {enc(v)}{comma}")
    out.append("}")
    return ("\n".join(out) + "\n").encode("utf-8")


LE_NAMES = {"\r\n": "CRLF", "\n": "LF", "\r": "CR"}
DELIM_NAMES = {",": "comma", ";": "semicolon", "\t": "tab", "|": "pipe"}


def _truth_json(seed, variant, applied, T, P, W, pres, cx, roster, used_encoding, n_stray, blank_log) -> dict:
    iso = lambda d: d.isoformat()  # noqa: E731
    people = []
    for p in T.people:
        people.append({
            "id": p.id, "name": p.name, "kind": p.kind, "crew": p.crew, "idle_all": p.idle_all,
            "on_roster": p.id in roster,
            "aliases": pres.rendered.get(("p", p.id), []),
            "display": pres.display.get(("p", p.id)),
        })
    sites = []
    for s in T.sites:
        sites.append({"id": s.id, "name": s.name, "kind": s.kind, "internal": s.internal,
                      "aliases": pres.rendered.get(("s", s.id), []),
                      "display": pres.display.get(("s", s.id))})
    asg = []
    for r in T.recs:
        a = {"kind": r.kind, "date": iso(r.date)}
        if r.pid:
            a["person"] = r.pid
        if r.sid:
            a["site"] = r.sid
        if r.status:
            a["status"] = r.status
        if r.start:
            a["start"], a["end"] = r.start, r.end
        if r.crew:
            a["crew"] = r.crew
        if r.tag != "base":
            a["tag"] = r.tag
        asg.append(a)
    asg.sort(key=lambda a: (a["date"], a.get("person", ""), a.get("site", ""), a["kind"]))
    corr = []
    for k, on in P.corr.items():
        if on:
            corr.append({"type": k, "level": "file"})
    for p in T.people:
        pal = pres.ppal[p.id]
        prim = pres.final.get(("p", p.id, 0))
        for idx in sorted(pres.used.get(("p", p.id), ())):
            if idx == 0:
                continue
            txt = pres.final[("p", p.id, idx)]
            if txt != prim:
                corr.append({"type": "alias", "form": pal[idx][0], "person": p.id, "alias": txt})
    for s in T.sites:
        for idx in sorted(pres.used.get(("s", s.id), ())):
            if idx and pres.final[("s", s.id, idx)] != s.name:
                corr.append({"type": "site_alias", "form": pres.spal[s.id][idx][0], "site": s.id,
                             "alias": pres.final[("s", s.id, idx)]})
    if n_stray:
        corr.append({"type": "stray_spaces_applied", "cells": n_stray})
    corr += pres.corruptions
    render = {
        "layout": P.layout, "family": P.family, "arrangement": P.arrangement, "format": P.fmt,
        "ext": P.ext, "encoding": used_encoding, "delimiter": DELIM_NAMES.get(P.delimiter) if used_encoding else None,
        "line_ending": LE_NAMES.get(P.line_ending) if used_encoding else None,
        "pad_rows": P.pad_rows if used_encoding else None,
        "date_format": P.date_fmt, "date_pad": P.date_pad, "week_of_style": P.week_of_style,
        "header": {"present": P.header_present, "vocab": P.header_vocab, "two_row": P.two_row,
                   "corner_label": P.corner_label, "title_rows": P.title_count},
        "lang": P.lang, "list_style": P.list_style if P.family == "list" else None,
        "list_order": P.list_order if P.family == "list" else None,
        "status_col": P.status_col if P.family == "list" else None,
        "times": {"mode": P.times_mode, "rate": P.time_rate, "cols": P.time_cols, "text": P.time_text} if P.times_mode else None,
        "merged_blank_col": P.merged_col, "crew_form": P.crew_form if P.family == "crew" else None,
        "crew_roster_form": P.crew_roster_form if P.family == "crew" else None,
        "calendar": {"start": P.cal_start, "day": P.cal_day, "sep": P.cal_sep, "style": P.cal_style} if P.family == "calendar" else None,
        "joiner": P.joiner, "oxford": P.oxford, "site_joiner": P.site_joiner, "note_style": P.note_style,
        "name_style": P.name_style, "roster_block": P.roster_block, "roster_pos": P.roster_pos if P.roster_block else None,
        "sheet_name": P.sheet_name if P.fmt in ("xlsx", "xls", "ods") else None,
        "extra_sheets": P.extra_sheets if P.fmt in ("xlsx", "xls", "ods") else 0,
        "blank_variant": blank_log or None,
    }
    return {
        "generator_version": GENERATOR_VERSION,
        "seed": seed, "variant": variant, "variant_applied": applied,
        "clean": T.clean,
        "workweek": T.workdays,
        "window": {"start": iso(T.dates[0]), "end": iso(T.dates[-1]), "weeks": T.weeks,
                   "dates": [iso(d) for d in T.dates]},
        "people": people, "sites": sites,
        "crews": [{"id": c.id, "label": c.label, "members": c.members} for c in T.crews],
        "assignments": asg,
        "planted": [{**p, "date": iso(p["date"])} for p in T.planted],
        "planted_info": [{**p, "date": iso(p["date"])} for p in T.planted_info],
        "tier": pres.tier, "tier_reasons": pres.tier_reasons,
        "render": render,
        "corruptions": corr,
        "date_labels": sorted([k, v] for k, v in cx.date_labels.items()),
    }
