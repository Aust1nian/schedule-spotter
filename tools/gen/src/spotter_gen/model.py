"""Data model shared by every stage, plus date-label helpers."""

from __future__ import annotations

import datetime as dt
from dataclasses import dataclass, field

from .names import NameSpec

BASE_MONDAY = dt.date(2026, 10, 12)
TODAY = "2026-10-10"
DAY_KEYS = ("mon", "tue", "wed", "thu", "fri", "sat", "sun")
WD_SHORT = ("Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun")
WD_LONG = ("Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday")
WD_LETTER = ("M", "T", "W", "Th", "F", "Sa", "Su")
WD_ES = ("Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado", "Domingo")
MON3 = ("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec")
MONTHS = ("January", "February", "March", "April", "May", "June", "July", "August",
          "September", "October", "November", "December")
EXCEL_EPOCH = dt.date(1899, 12, 30)

FULL_DATE_FORMATS = ("iso", "mdy4", "mdy2", "md", "mon_d", "d_mon", "wd_md", "dmy4", "serial", "native")
WEEKDAY_FORMATS = ("wd_short", "wd_long", "letters", "wd_es")
STATUSES = ("off", "pto", "vacation", "sick", "holiday", "out")


@dataclass
class Person:
    id: str
    kind: str  # employee | placeholder | subcontractor
    spec: NameSpec | None = None
    first: str = ""
    last: str = ""
    name: str = ""  # canonical name
    crew: str | None = None
    idle_all: bool = False


@dataclass
class Site:
    id: str
    kind: str  # address | customer | jobno | commercial | internal
    name: str
    internal: bool = False
    parts: dict = field(default_factory=dict)


@dataclass
class Crew:
    id: str
    label: str
    members: list[str]


@dataclass
class Rec:
    kind: str  # work | status | unstaffed
    date: dt.date
    pid: str | None = None
    sid: str | None = None
    status: str | None = None
    tag: str = "base"
    crew: str | None = None
    start: str | None = None
    end: str | None = None
    # presentation (set while rendering)
    pref: object = None
    sref: object = None
    word: str | None = None
    note: str | None = None
    tstr: str | None = None


@dataclass
class Truth:
    seed: int
    weeks: int
    start: dt.date
    ndays: int
    dates: list[dt.date]
    people: list[Person]
    sites: list[Site]
    crews: list[Crew]
    recs: list[Rec]
    spans: dict  # (sid, week) -> (a, b) | None, indexes into the week's workdays
    planted: list[dict] = field(default_factory=list)
    planted_info: list[dict] = field(default_factory=list)
    clean: bool = False

    @property
    def workdays(self) -> list[str]:
        return list(DAY_KEYS[: self.ndays])

    def week_dates(self, w: int) -> list[dt.date]:
        return self.dates[w * self.ndays:(w + 1) * self.ndays]

    def person(self, pid: str) -> Person:
        return self._pmap[pid]

    def site(self, sid: str) -> Site:
        return self._smap[sid]

    def index(self) -> None:
        self._pmap = {p.id: p for p in self.people}
        self._smap = {s.id: s for s in self.sites}

    def active(self, sid: str, d: dt.date) -> bool:
        w = (d - self.start).days // 7
        i = (d - self.start).days % 7
        sp = self.spans.get((sid, w))
        return sp is not None and sp[0] <= i <= sp[1]


# ----------------------------------------------------------------------------- dates

def fmt_date(d: dt.date, f: str, pad: bool = False):
    m, dd, y = d.month, d.day, d.year
    if f == "iso":
        return d.isoformat()
    if f == "mdy4":
        return f"{m:02d}/{dd:02d}/{y}" if pad else f"{m}/{dd}/{y}"
    if f == "mdy2":
        return f"{m:02d}/{dd:02d}/{y % 100:02d}" if pad else f"{m}/{dd}/{y % 100:02d}"
    if f == "md":
        return f"{m}/{dd}"
    if f == "mon_d":
        return f"{MON3[m - 1]} {dd}"
    if f == "d_mon":
        return f"{dd}-{MON3[m - 1]}"
    if f == "wd_md":
        return f"{WD_SHORT[d.weekday()]} {m}/{dd}"
    if f == "dmy4":
        return f"{dd:02d}/{m:02d}/{y}" if pad else f"{dd}/{m}/{y}"
    if f == "serial":
        return (d - EXCEL_EPOCH).days
    if f == "native":
        return d
    if f == "wd_short":
        return WD_SHORT[d.weekday()]
    if f == "wd_long":
        return WD_LONG[d.weekday()]
    if f == "letters":
        return WD_LETTER[d.weekday()]
    if f == "wd_es":
        return WD_ES[d.weekday()]
    raise ValueError(f)


WEEK_OF_STYLES = 6


def week_of(monday: dt.date, style: int) -> str:
    m, d, y = monday.month, monday.day, monday.year
    return (
        f"Week of {m}/{d}/{y}",
        f"Week of {m}/{d}",
        f"WEEK OF {m}/{d}/{y % 100:02d}",
        f"Week of {MON3[m - 1]} {d}, {y}",
        f"Wk of {m}/{d}",
        f"Week of: {m}/{d}/{y}",
    )[style]


def hhmm(t: str) -> str:
    return t
