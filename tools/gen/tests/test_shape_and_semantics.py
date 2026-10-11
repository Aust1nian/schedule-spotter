"""expected.json follows FORMAT.md, clean cases have no red flags, planted == expected."""

import datetime as dt

from format_schema import check_expected

from spotter_gen.model import DAY_KEYS

RED = {"double_booked", "assigned_while_off"}


def test_every_expected_validates(cases):
    for c in cases:
        check_expected(c.expected)


def test_clean_cases_have_no_flags(cases):
    clean = [c for c in cases if c.truth["clean"]]
    assert len(clean) > 40
    for c in clean:
        assert not [f for f in c.expected["flags"] if f["type"] in RED], c.seed
        # "fully clean": none of the four planted, so nothing at all is flagged
        assert c.expected["flags"] == [], c.seed


def test_flags_match_planted(cases):
    for c in cases:
        planted = sorted((p["type"], p["date"]) for p in c.truth["planted"])
        got = sorted((f["type"], f["date"]) for f in c.expected["flags"])
        assert planted == got, c.seed
        info = sorted((p["type"], p["date"]) for p in c.truth["planted_info"])
        assert info == sorted((i["type"], i["date"]) for i in c.expected["info"]), c.seed


def test_flag_names_are_longest_rendered_alias(cases):
    for c in cases:
        disp = {p["display"]: p for p in c.truth["people"] if p["display"]}
        for f in c.expected["flags"] + c.expected["info"]:
            if "person" in f:
                p = disp[f["person"]]
                assert len(f["person"]) == max(len(a) for a in p["aliases"]), (c.seed, f)
                longest = {a.casefold() for a in p["aliases"] if len(a) == len(f["person"])}
                assert len(longest) == 1, (c.seed, p["aliases"])


def test_dates_and_weekdays(cases):
    for c in cases:
        e, t = c.expected, c.truth
        work = set(e["settings"]["workdays"])
        start = dt.date.fromisoformat(t["window"]["start"])
        assert start.weekday() == 0
        assert abs((start - dt.date(2026, 10, 12)).days) <= 42
        for d in t["window"]["dates"]:
            assert DAY_KEYS[dt.date.fromisoformat(d).weekday()] in work
        for a in e["assignments"]:
            assert DAY_KEYS[dt.date.fromisoformat(a["date"]).weekday()] in work, (c.seed, a)
        for f in e["flags"]:
            if f["type"] == "not_booked":
                assert DAY_KEYS[dt.date.fromisoformat(f["date"]).weekday()] in work
        for label, iso in t["date_labels"]:
            check_label(label, dt.date.fromisoformat(iso), c.seed)


WD = {"mon": 0, "tue": 1, "wed": 2, "thu": 3, "fri": 4, "sat": 5, "sun": 6, "m": 0, "t": 1, "w": 2,
      "th": 3, "f": 4, "sa": 5, "su": 6, "monday": 0, "tuesday": 1, "wednesday": 2, "thursday": 3,
      "friday": 4, "saturday": 5, "sunday": 6, "lunes": 0, "martes": 1, "miércoles": 2, "jueves": 3,
      "viernes": 4, "sábado": 5, "domingo": 6}
MON = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"]


def check_label(label: str, d: dt.date, seed: int) -> None:
    low = label.casefold()
    first = low.split()[0].rstrip(":")
    if first in ("week", "wk"):
        assert d.weekday() == 0, (seed, label)
        rest = low.split("of", 1)[1].strip(" :")
        if "/" in rest:
            parts = [int(x) for x in rest.split("/")]
            assert (parts[0], parts[1]) == (d.month, d.day), (seed, label)
        else:
            mon, day = rest.replace(",", "").split()[:2]
            assert MON.index(mon) + 1 == d.month and int(day) == d.day, (seed, label)
        return
    if first in WD:
        assert WD[first] == d.weekday(), (seed, label)
        low = low[len(first):].strip()
        if not low:
            return
    if "-" in low and low[:4].isdigit():
        assert dt.date.fromisoformat(low) == d, (seed, label)
    elif "/" in low:
        parts = [int(x) for x in low.split("/")]
        if len(parts) == 3 and parts[0] == d.day and parts[1] == d.month and parts[0] > 12:
            pass  # day-first file
        else:
            assert (parts[0], parts[1]) == (d.month, d.day), (seed, label)
            if len(parts) == 3:
                assert parts[2] in (d.year, d.year % 100), (seed, label)
    elif "-" in low:
        day, mon = low.split("-")
        assert int(day) == d.day and MON.index(mon) + 1 == d.month, (seed, label)
    else:
        mon, day = low.split()
        assert MON.index(mon) + 1 == d.month and int(day) == d.day, (seed, label)


def test_day_first_only_with_a_day_over_12(cases):
    for c in cases:
        if c.truth["render"]["date_format"] == "dmy4":
            assert any(int(lbl.split("/")[0]) > 12 for lbl, _ in c.truth["date_labels"] if lbl.count("/") == 2)


def test_tier_reasons(cases):
    for c in cases:
        assert (c.expected["tier"] == "model") == bool(c.truth["tier_reasons"]), c.seed
