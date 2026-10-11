"""A strict checker for the expected.json shape described in fixtures/FORMAT.md."""

from __future__ import annotations

import datetime as dt
import re

TOP_KEYS = {"category", "tier", "input", "paste", "settings", "layout", "arrangement", "assignments",
            "assignments_exact", "flags", "flags_exact", "info", "info_exact", "ask", "must_not_ask",
            "answers", "after_answers", "error", "notes"}
CATEGORIES = set("ABCDEFGHIJKLMN")
LAYOUTS = {"long_list", "person_by_day", "day_by_person", "job_by_day", "day_by_job", "crew_based", "calendar_month"}
ARRANGEMENTS = {"single", "stacked", "side_by_side"}
DAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
STATUSES = {"off", "pto", "vacation", "sick", "holiday", "out"}
FLAG_TYPES = {"double_booked", "assigned_while_off", "not_booked", "unstaffed"}
INFO_TYPES = {"duplicate_entry", "split_day", "internal_plus_site", "subcontractor_multi_site"}
HHMM = re.compile(r"^([01]\d|2[0-3]):[0-5]\d$")
NAME_SEPARATORS = (",", "&", "/", " and ", "+", "\n", " y ")


def iso(s) -> dt.date:
    assert isinstance(s, str) and re.fullmatch(r"\d{4}-\d{2}-\d{2}", s), s
    return dt.date.fromisoformat(s)


def clean_text(s) -> None:
    assert isinstance(s, str) and s, s
    assert " ".join(s.split()) == s, f"not trimmed/collapsed: {s!r}"


def check_assignment(a: dict) -> None:
    assert set(a) <= {"person", "date", "site", "start", "end", "note", "status", "part", "unstaffed"}, a
    iso(a["date"])
    if a.get("unstaffed"):
        assert a["unstaffed"] is True and "person" not in a and "status" not in a
        clean_text(a["site"])
        return
    clean_text(a["person"])
    assert not any(sep in a["person"] for sep in NAME_SEPARATORS), a
    if "status" in a:
        assert a["status"] in STATUSES and "site" not in a, a
        return
    clean_text(a["site"])
    assert " - " not in a["site"] and not a["site"].endswith(")"), a
    assert not any(sep in a["site"] for sep in NAME_SEPARATORS), a
    if "start" in a or "end" in a:
        assert HHMM.match(a["start"]) and HHMM.match(a["end"]), a
    if "note" in a:
        clean_text(a["note"])
    if "part" in a:
        assert a["part"] in ("am", "pm", "half")


def check_flag(f: dict) -> None:
    t = f["type"]
    assert t in FLAG_TYPES, f
    iso(f["date"])
    if t == "double_booked":
        assert set(f) == {"type", "person", "date", "sites"} and len(f["sites"]) >= 2, f
    elif t == "assigned_while_off":
        assert set(f) == {"type", "person", "date", "sites", "status"} and f["status"] in STATUSES, f
        assert len(f["sites"]) >= 1
    elif t == "not_booked":
        assert set(f) == {"type", "person", "date"}, f
    else:
        assert set(f) == {"type", "site", "date"}, f
    for k in ("person", "site"):
        if k in f:
            clean_text(f[k])
    for s in f.get("sites", []):
        clean_text(s)
    assert len(set(f.get("sites", []))) == len(f.get("sites", []))


def check_info(i: dict) -> None:
    assert i["type"] in INFO_TYPES, i
    assert set(i) == {"type", "person", "date", "sites"}, i
    iso(i["date"])
    clean_text(i["person"])


def check_expected(e: dict) -> None:
    assert set(e) <= TOP_KEYS, set(e) - TOP_KEYS
    assert e["category"] in CATEGORIES
    assert e["tier"] in ("script", "model")
    assert isinstance(e["input"], str) and e["input"]
    assert isinstance(e["paste"], bool)
    assert (e["input"] == "paste.txt") == e["paste"]
    st = e["settings"]
    assert set(st) <= {"today", "workdays", "days_off"}
    iso(st["today"])
    assert st["workdays"] and all(d in DAYS for d in st["workdays"])
    assert st["workdays"] == sorted(st["workdays"], key=DAYS.index)
    for d in st["days_off"]:
        iso(d)
    assert e["layout"] in LAYOUTS
    assert e["arrangement"] in ARRANGEMENTS
    for k in ("assignments_exact", "flags_exact", "info_exact"):
        assert isinstance(e[k], bool)
    assert e["assignments"]
    for a in e["assignments"]:
        check_assignment(a)
    for f in e["flags"]:
        check_flag(f)
    for i in e["info"]:
        check_info(i)
    assert e["error"] is None
    assert isinstance(e["notes"], list)
