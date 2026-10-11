"""Category D: dates and days. Run: uv run --with openpyxl python3 build_D.py"""
import datetime as dt

from fxlib import *

WEEK = days("10-12", 5)
J, S, O = "1420 Johnson St", "Smith remodel", "Oak Ridge Apts"
P, E, R, L = "Parker kitchen", "88 Elm Ave", "Riverside duplex", "Lincoln School"
NO_LAYOUT_QS = [Q("pick_layout"), Q("column_role")]
FIXED = dt.datetime(2026, 10, 9, 12, 0)


def list_csv(rows, dates, fmt, header=("Date", "Name", "Job")):
    return csv_bytes([list(header)] + grid_to_list_rows(rows, dates, fmt))


def grid_csv(rows, header_cells, first="Name", pre=()):
    return csv_bytes([list(r) for r in pre] + [[first] + list(header_cells)] + [[p] + c for p, c in rows])


def d01():
    rows = [("Mike Baker", [J] * 5), ("Jose Garcia", [S] * 5), ("Luis Ortiz", [O] * 5), ("Dave Kim", [J, J, "off", S, S])]
    exp = expected("D", "input.csv", layout="long_list", arrangement="single",
                   assignments=person_grid(rows, WEEK), flags=[], info=[], must_not_ask=NO_LAYOUT_QS)
    emit("D01", "iso-dates", "ISO 8601 dates (2026-10-12) in a long list.",
         {"input.csv": list_csv(rows, WEEK, lambda x: x)}, exp)


def d02():
    wk = days("10-05", 5)
    assert [wd(x) for x in wk] == ["Mon", "Tue", "Wed", "Thu", "Fri"]
    rows = [("Ana Ruiz", [P] * 5), ("Tom Reed", [P, P, P, E, E]), ("Carlos Mendez", [E] * 5),
            ("Ray Jones", [E, E, f"{E} / {P}", E, E])]
    exp = expected("D", "input.csv", layout="long_list", arrangement="single",
                   assignments=person_grid(rows, wk),
                   flags=[DB("Ray Jones", "10-07", E, P)], info=[], must_not_ask=NO_LAYOUT_QS)
    emit("D02", "mdyyyy-all-ambiguous-month-first",
         "M/D/YYYY dates where every value is ambiguous (10/5/2026 ... 10/9/2026, both parts <= 12): must stay month-first (Oct 5-9, not May-Sep 10).",
         {"input.csv": list_csv(rows, wk, mdyyyy)}, exp)


def d03():
    rows = [("Jose Garcia", [S] * 5), ("Luis Ortiz", [O] * 5), ("Pedro Sanchez", [O, O, O, "Shop", "Shop"]),
            ("Juan Lopez", [S, S, "", S, S])]
    exp = expected("D", "input.csv", layout="long_list", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[NB("Juan Lopez", "10-14")], info=[], must_not_ask=NO_LAYOUT_QS)
    emit("D03", "mdyy-two-digit-year", "M/D/YY dates with a two-digit year (10/12/26 = 2026-10-12).",
         {"input.csv": list_csv(rows, WEEK, mdyy)}, exp)


def d04():
    wk = days("2027-01-04", 5)
    assert [wd(x) for x in wk] == ["Mon", "Tue", "Wed", "Thu", "Fri"]
    rows = [("Mike Baker", [J] * 5), ("Dave Kim", ["Shop", J, J, J, J]), ("Tom Reed", [S, S, f"{S} / {J}", S, S])]
    exp = expected("D", "input.csv", settings_=settings(today="2026-12-20"),
                   layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, wk),
                   flags=[DB("Tom Reed", "2027-01-06", S, J)], info=[], must_not_ask=NO_LAYOUT_QS)
    emit("D04", "md-no-year-closest-to-today",
         "M/D headers with no year (1/4 ... 1/8) and today = 2026-12-20: the closest year is 2027, not the current year.",
         {"input.csv": grid_csv(rows, [md(x) for x in wk])}, exp, {"window": wk})


def d05():
    rows = [("Luis Ortiz", [O] * 5), ("Ana Ruiz", [O, O, O, O, "sick"]), ("Ray Jones", [R] * 5), ("Sam Patel", [R] * 5)]
    hdr = [mon_dd(x) for x in WEEK]
    assert hdr[0] == "Oct 12"
    exp = expected("D", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, WEEK), flags=[], info=[], must_not_ask=NO_LAYOUT_QS)
    emit("D05", "month-name-day", "Month-name dates with no year in the header ('Oct 12'); year from today.",
         {"input.csv": grid_csv(rows, hdr)}, exp, {"window": WEEK})


def d06():
    M = "305 Main St"
    rows = [("Mike Baker", [L] * 5), ("Jose Garcia", [M] * 5), ("Chris Hall", [L, f"{L} / PTO", L, L, L]),
            ("Brian Young", [M] * 5)]
    assert dd_mon(WEEK[0]) == "12-Oct"
    exp = expected("D", "input.csv", layout="long_list", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[AWO("Chris Hall", "10-13", [L], "pto")], info=[], must_not_ask=NO_LAYOUT_QS)
    emit("D06", "day-month-name-excel", "Excel d-mmm display dates exported to CSV ('12-Oct'), no year.",
         {"input.csv": list_csv(rows, WEEK, dd_mon)}, exp)


def d07():
    rows = [("Mike Baker", [J] * 5), ("Jose Garcia", [S] * 5), ("Luis Ortiz", [O] * 5), ("Dave Kim", [J] * 5),
            ("Eddie Flores", ["Shop", "Shop", O, f"{O} / {S}", O])]
    hdr = [wd_md(x) for x in WEEK]
    assert hdr[0] == "Mon 10/12" and hdr[4] == "Fri 10/16"
    exp = expected("D", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[DB("Eddie Flores", "10-15", O, S)], info=[],
                   must_not_ask=NO_LAYOUT_QS + [Q("weekday_mismatch")])
    emit("D07", "weekday-plus-date", "Weekday plus M/D header ('Mon 10/12'), no year; every weekday matches its date.",
         {"input.csv": grid_csv(rows, hdr)}, exp, {"window": WEEK})


def d08():
    rows = [("Ana Ruiz", [P] * 5), ("Tom Reed", [P, P, E, E, E]), ("Carlos Mendez", [E] * 5), ("Ray Jones", [J, J, "", J, J])]
    names = [dt.date.fromisoformat(x).strftime("%A") for x in WEEK]
    assert names == ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"]
    pre = [["Week of 10/12/2026", "", "", "", "", ""], [""] * 6]
    exp = expected("D", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[NB("Ray Jones", "10-14"), UF(J, "10-14")], info=[], must_not_ask=NO_LAYOUT_QS)
    emit("D08", "week-of-weekday-names",
         "Weekday names only (Monday ... Friday) anchored by a 'Week of 10/12/2026' line above the header.",
         {"input.csv": grid_csv(rows, names, pre=pre)}, exp, {"window": WEEK})


def d09():
    wk = days("10-12", 7)
    letters = ["M", "T", "W", "Th", "F", "Sa", "Su"]
    assert [wd(x) for x in wk] == ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
    rows = [
        ("Mike Baker", [J, J, J, J, J, "Shop", ""]),
        ("Jose Garcia", [S, S, S, S, S, "", ""]),
        ("Luis Ortiz", [O, O, O, "off", O, "", ""]),
        ("Dave Kim", [J, J, J, f"{J} / {O}", J, O, ""]),
    ]
    pre = [["Week of 10/12/2026"] + [""] * 7]
    exp = expected("D", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, wk),
                   flags=[DB("Dave Kim", "10-15", J, O)], info=[], must_not_ask=NO_LAYOUT_QS)
    emit("D09", "single-letter-days",
         "Single-letter day headers M T W Th F Sa Su under 'Week of 10/12/2026' (T is Tuesday, Th is Thursday); Saturday work is read even though Mon-Fri are the workdays.",
         {"input.csv": grid_csv(rows, letters, pre=pre)}, exp, {"window": wk})


def d10():
    rows = [("Luis Ortiz", [O] * 5), ("Dave Kim", [J] * 5), ("Tom Reed", [S] * 5), ("Ana Ruiz", [S, S, S, O, O])]
    assert serial(WEEK[0]) == 46307

    def make(path):
        import openpyxl
        wb = openpyxl.Workbook()
        wb.properties.creator = "Office"
        wb.properties.created = wb.properties.modified = FIXED
        ws = wb.active
        ws.title = "Sheet1"
        ws.append(["Date", "Name", "Job"])
        for date_serial, person, site in grid_to_list_rows(rows, WEEK, serial):
            ws.append([date_serial, person, site])  # plain numbers, General format: the date format was lost
        wb.save(path)

    exp = expected("D", "input.xlsx", layout="long_list", arrangement="single",
                   assignments=person_grid(rows, WEEK), flags=[], info=[],
                   must_not_ask=NO_LAYOUT_QS + [Q("pick_sheet")])
    emit("D10", "excel-serial-xlsx",
         "Excel serial day numbers (46307 = 2026-10-12) stored in .xlsx as plain numbers with no date format.",
         {"input.xlsx": make}, exp)


def d11():
    rows = [("Mike Baker", [J] * 5), ("Jose Garcia", [S] * 5), ("Juan Lopez", [L] * 5), ("Miguel Torres", [L, L, L, L, ""])]
    exp = expected("D", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[NB("Miguel Torres", "10-16")], info=[], must_not_ask=NO_LAYOUT_QS)
    emit("D11", "excel-serial-csv", "Excel serial day numbers as the CSV header (46307 ... 46311 = Oct 12-16 2026).",
         {"input.csv": grid_csv(rows, [str(serial(x)) for x in WEEK])}, exp, {"window": WEEK})


def d12():
    rows = [("Ray Jones", [R] * 5), ("Sam Patel", [R, R, R, f"{R} / {E}", R]), ("Kevin Walsh", [E] * 5), ("Chris Hall", [E] * 5)]
    assert ddmmyyyy(WEEK[0]) == "12/10/2026" and ddmmyyyy(WEEK[1]) == "13/10/2026"
    exp = expected("D", "input.csv", layout="long_list", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[DB("Sam Patel", "10-15", R, E)], info=[], must_not_ask=NO_LAYOUT_QS)
    emit("D12", "day-first-over-12",
         "Day-first DD/MM/YYYY dates; the first rows (12/10/2026) are ambiguous, but 13/10 later in the column forces day-first for the whole column.",
         {"input.csv": list_csv(rows, WEEK, ddmmyyyy)}, exp)


def d13():
    wk = days("2026-12-28", 7)
    hdr = [wd_md(x) for x in wk]
    assert hdr == ["Mon 12/28", "Tue 12/29", "Wed 12/30", "Thu 12/31", "Fri 1/1", "Sat 1/2", "Sun 1/3"]
    rows = [
        ("Mike Baker", [J, J, J, J, "", "Shop", ""]),
        ("Jose Garcia", [S, S, S, "off", "", "", ""]),
        ("Luis Ortiz", ["Shop", J, f"{J} / {S}", J, "", "", ""]),
    ]
    exp = expected("D", "input.csv", settings_=settings(days_off=["2027-01-01"]),
                   layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, wk),
                   flags=[DB("Luis Ortiz", "2026-12-30", J, S)], info=[],
                   must_not_ask=NO_LAYOUT_QS + [Q("weekday_mismatch")])
    emit("D13", "year-boundary-week",
         "The Dec 28 2026 - Jan 3 2027 week written with no year ('Fri 1/1'): Jan dates belong to 2027, so no weekday mismatch; Jan 1 is a day off.",
         {"input.csv": grid_csv(rows, hdr)}, exp, {"window": wk})


def d14():
    rows = [("Mike Baker", [J] * 5), ("Jose Garcia", [S] * 5), ("Luis Ortiz", [O] * 5), ("Dave Kim", [J, J, S, S, S])]
    hdr = [f"{wd(x)} {mdyyyy(x)}" for x in WEEK]
    hdr[1] = "Mon 10/13/2026"  # planted: 10/13/2026 is a Tuesday
    assert wd("2026-10-13") == "Tue"
    full = person_grid(rows, WEEK)
    exp = expected("D", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=[a for a in full if a["date"] != "2026-10-13"], assignments_exact=False,
                   ask=[Q("weekday_mismatch", text="Mon 10/13/2026")],
                   must_not_ask=NO_LAYOUT_QS,
                   answers=[{"question": Q("weekday_mismatch", text="Mon 10/13/2026"), "answer": "yes"}],
                   after_answers={"assignments": full, "flags": [], "info": []})
    emit("D14", "weekday-mismatch-ask",
         "Header 'Mon 10/13/2026' is a Tuesday: must ask weekday_mismatch; answer 'yes' keeps the written date (Tue 10/13).",
         {"input.csv": grid_csv(rows, hdr)}, exp, {"window": WEEK})


if __name__ == "__main__":
    for f in (d01, d02, d03, d04, d05, d06, d07, d08, d09, d10, d11, d12, d13, d14):
        f()
    print("D done")
