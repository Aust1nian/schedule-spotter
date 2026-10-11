"""Rendering plan: every layout / format / style decision, drawn from the main RNG in a fixed
order that never depends on the variant (the variant is applied afterwards)."""

from __future__ import annotations

import datetime as dt
import random
from dataclasses import dataclass, field

from .model import BASE_MONDAY, WEEK_OF_STYLES

FORMAT_WEIGHTS = {"csv": 45, "tsv": 7, "txt": 8, "xlsx": 22, "xls": 5, "ods": 5, "docx": 5, "paste": 3}
SPREADSHEETS = ("xlsx", "xls", "ods")

# Per-file corruption probabilities (spec "Corrupt" step). Each is drawn independently.
CORRUPTION_RATES = {
    "nickname": 0.30,
    "initials": 0.20,
    "partial_name": 0.10,
    "typo": 0.08,
    "drop_accents": 0.30,
    "name_case": 0.15,
    "site_variant": 0.25,
    "site_case": 0.10,
    "site_hard": 0.03,
    "lookalike_sites": 0.02,
    "title_rows": 0.40,
    "footer": 0.20,
    "totals": 0.15,
    "merged_blanks": 0.12,  # lists only
    "stray_spaces": 0.20,
    "status_case": 0.30,
    "spanish": 0.08,
    "notes": 0.15,
}

# Planted-problem rates (non-clean schedules). Info items are planted in any schedule.
PROBLEM_RATES = {
    "double_booked": 0.50,
    "assigned_while_off": 0.35,
    "not_booked": 0.40,
    "unstaffed": 0.40,
}
INFO_RATES = {
    "internal_plus_site": 0.20,
    "duplicate_entry": 0.10,
    "split_day": 0.40,
    "subcontractor_multi_site": 0.40,
}


def wpick(R: random.Random, weights: dict):
    keys = list(weights)
    return R.choices(keys, [weights[k] for k in keys])[0]


@dataclass
class Plan:
    weeks: int = 1
    start: dt.date = BASE_MONDAY
    ndays: int = 5
    family: str = "list"  # list | pgrid | jgrid | crew | calendar
    orient: str = "rows"  # rows: person_by_day / job_by_day; cols: day_by_person / day_by_job
    fmt: str = "csv"
    arrangement: str = "single"
    date_fmt: str = "mdy4"
    date_pad: bool = False
    week_of_style: int = 0
    lang: str = "en"
    header_present: bool = True
    header_vocab: str = "plain"
    two_row: bool = False
    corner_label: bool = True
    corr: dict = field(default_factory=dict)
    # list
    list_style: str = "per_record"
    list_order: str = "date_person_site"
    day_col: bool = False
    status_col: bool = False
    times_mode: bool = False
    time_rate: float = 1.0
    time_cols: str = "range"  # range | startend
    time_text: int = 0
    sort_key: str = "date"
    merged_col: str | None = None
    # crews
    crew_form: str = "grid"
    crew_roster_form: str = "colon"
    crew_label_style: int = 0
    # calendar
    cal_start: str = "sun"
    cal_day: str = "day_number"
    cal_sep: str = " - "
    cal_style: str = "per_site"
    cal_wd_long: bool = False
    # joins / words
    joiner: str = ", "
    oxford: str | None = None
    site_joiner: str = " / "
    status_first: bool = True
    note_style: str = " - "
    # names
    name_style: str = "FULL"
    li_dot: bool = False
    # extras
    roster_block: bool = False
    roster_pos: str = "below"
    title_count: int = 0
    week_titles: bool = False
    stack_gap: int = 1
    title_gap: int = 1
    # text encoding
    encoding: str = "utf-8"
    delimiter: str = ","
    line_ending: str = "\r\n"
    pad_rows: bool = True
    sheet_name: str = "Sheet1"
    extra_sheets: int = 0
    xlsx_date_numfmt: str = "m/d/yyyy"

    @property
    def layout(self) -> str:
        if self.family == "list":
            return "long_list"
        if self.family == "pgrid":
            return "person_by_day" if self.orient == "rows" else "day_by_person"
        if self.family == "jgrid":
            return "job_by_day" if self.orient == "rows" else "day_by_job"
        if self.family == "crew":
            return "crew_based"
        return "calendar_month"

    @property
    def weekday_only(self) -> bool:
        return self.date_fmt in ("wd_short", "wd_long", "letters", "wd_es")

    @property
    def ext(self) -> str:
        return "txt" if self.fmt == "paste" else self.fmt


def draw_plan(R: random.Random) -> Plan:
    P = Plan()
    P.weeks = R.choices([1, 2, 3, 4], [45, 25, 10, 20])[0]
    P.start = BASE_MONDAY + dt.timedelta(weeks=R.randint(-6, 6))
    P.ndays = 6 if R.random() < 0.30 else 5
    fam_w = {"list": 30, "pgrid": 24, "jgrid": 22, "crew": 12, "calendar": 40 if P.weeks == 4 else 0}
    P.family = wpick(R, fam_w)
    P.orient = "rows" if R.random() < 0.75 else "cols"
    P.fmt = wpick(R, FORMAT_WEIGHTS)
    P.corr = {k: R.random() < v for k, v in CORRUPTION_RATES.items()}
    if P.family != "list":
        P.corr["merged_blanks"] = False
    P.lang = "es" if P.corr["spanish"] else "en"

    # ---- arrangement
    if P.weeks == 1 or P.family == "calendar":
        P.arrangement = "single"
    elif P.family in ("pgrid", "jgrid"):
        P.arrangement = wpick(R, {"single": 30, "stacked": 40, "side_by_side": 30})
    else:
        P.arrangement = wpick(R, {"single": 60, "stacked": 40})
    if P.fmt == "docx" and P.arrangement == "side_by_side":
        P.arrangement = "stacked"

    # ---- header style
    rich = P.fmt in ("xlsx", "xls", "ods", "docx")
    P.header_present = R.random() < 0.85
    P.header_vocab = "synonym" if R.random() < 0.45 else "plain"
    P.two_row = rich and R.random() < 0.15
    P.corner_label = R.random() < 0.7
    if P.family in ("pgrid", "jgrid", "crew", "calendar"):
        P.header_present = True

    # ---- date format
    w = {"iso": 8, "mdy4": 16, "mdy2": 8, "md": 12, "mon_d": 8, "d_mon": 6, "wd_md": 10, "dmy4": 5,
         "serial": 4, "native": 10, "wd_short": 8, "wd_long": 5, "letters": 4}
    if P.fmt not in SPREADSHEETS:
        w["native"] = 0
    if not (P.family in ("list", "crew") and P.fmt in ("csv", "xlsx", "xls", "ods")):
        w["serial"] = 0
    anchorable = P.arrangement != "single" or P.weeks == 1 or (P.two_row and P.family in ("pgrid", "jgrid"))
    if not anchorable:
        for k in ("wd_short", "wd_long", "letters"):
            w[k] = 0
    P.date_fmt = wpick(R, w)
    if P.lang == "es" and P.date_fmt in ("wd_short", "wd_long"):
        P.date_fmt = "wd_es"
    P.date_pad = R.random() < 0.25
    P.week_of_style = R.randrange(WEEK_OF_STYLES)
    P.week_titles = R.random() < 0.35
    P.xlsx_date_numfmt = R.choice(["m/d/yyyy", "mm-dd-yy", "d-mmm", "ddd m/d", "yyyy-mm-dd", "mmm d"])

    # ---- list details
    P.list_style = "per_record" if R.random() < 0.75 else "per_site"
    P.list_order = R.choice(["date_person_site", "person_date_site", "date_site_person", "person_site_date"])
    P.day_col = R.random() < 0.2
    P.status_col = R.random() < 0.25
    P.times_mode = P.family == "list" and P.list_style == "per_record" and R.random() < 0.35
    P.time_rate = R.choice([0.5, 1.0])
    P.time_cols = "range" if R.random() < 0.7 else "startend"
    P.time_text = R.randrange(6)
    P.sort_key = R.choice(["date", "date", "person", "site"])
    if P.corr["merged_blanks"]:
        P.merged_col = P.sort_key
    # ---- crews
    P.crew_form = "grid" if R.random() < 0.6 else "list"
    P.crew_roster_form = R.choice(["colon", "row", "columns"])
    P.crew_label_style = R.randrange(4)
    # ---- calendar
    P.cal_start = "sun" if R.random() < 0.6 else "mon"
    P.cal_day = "day_number" if R.random() < 0.7 else "md"
    P.cal_sep = R.choices([" - ", ": ", " @ "], [60, 20, 20])[0]
    P.cal_style = "per_site" if R.random() < 0.6 else "per_person"
    P.cal_wd_long = R.random() < 0.3
    if P.family == "calendar":
        P.date_fmt = P.cal_day
    # ---- joins
    joins = [", ", " & ", " / ", " and ", "\n", " + "]
    if P.lang == "es":
        joins.append(" y ")
    P.joiner = R.choice(joins)
    P.oxford = R.choice([None, None, " & ", " and "])
    P.site_joiner = R.choice([" / ", " / ", ", ", " & "])
    P.status_first = R.random() < 0.5
    P.note_style = R.choice([" - ", "()"])
    # ---- names
    P.name_style = wpick(R, {"FULL": 35, "FIRST": 25, "FIRST_LI": 15, "NICK": 8, "FI_LAST": 7, "LAST": 5})
    P.li_dot = R.random() < 0.4
    # ---- extras
    P.roster_block = P.family in ("list", "jgrid", "calendar") and R.random() < 0.15
    P.roster_pos = "below" if (R.random() < 0.6 or P.fmt == "docx") else "right"
    P.title_count = R.choice([1, 1, 2]) if P.corr["title_rows"] else 0
    P.stack_gap = R.choice([1, 1, 2])
    P.title_gap = R.choice([0, 1])
    # ---- text encoding
    if P.fmt == "csv":
        P.delimiter = R.choices([",", ";", "|"], [80, 15, 5])[0]
        P.encoding = R.choices(["utf-8", "utf-8-sig", "cp1252"], [45, 30, 25])[0]
        P.line_ending = R.choices(["\r\n", "\n", "\r"], [60, 35, 5])[0]
    elif P.fmt == "tsv":
        P.delimiter = "\t"
        P.encoding = R.choices(["utf-8", "utf-8-sig", "cp1252"], [60, 15, 25])[0]
        P.line_ending = R.choices(["\r\n", "\n"], [60, 40])[0]
    elif P.fmt == "txt":
        P.delimiter = R.choices(["\t", "|"], [80, 20])[0]
        P.encoding = R.choices(["utf-16", "utf-8", "cp1252"], [50, 25, 25])[0]
        P.line_ending = R.choices(["\r\n", "\n", "\r"], [70, 25, 5])[0]
    elif P.fmt == "paste":
        P.delimiter = "\t"
        P.encoding = "utf-8"
        P.line_ending = R.choices(["\n", "\r\n"], [70, 30])[0]
    P.pad_rows = R.random() < 0.6
    P.sheet_name = R.choice(["Sheet1", "Schedule", "Crew", "Week", "Hoja1", "Schedule (2)"])
    P.extra_sheets = R.choices([0, 1, 2], [75, 15, 10])[0]
    return P
