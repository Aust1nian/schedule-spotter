"""H08: plumbing company's New Year week, Excel 'CSV UTF-8' (BOM, CRLF), person x day. Clean."""
from common import *

FID, SLUG = "H08", "plumber-new-year-week-bom-csv-clean"
README = ("Clean schedule (no double-bookings, no one assigned while off) across the Dec 28 2026 - Jan 3 "
          "2027 week: Excel 'CSV UTF-8' with BOM and CRLF, person x day with 'Mon 12/28/2026' headers "
          "spanning the year change, HOLIDAY column, Sat/Sun columns under Mon-Fri, an AM-only cell, "
          "vacation/sick/OFF, an idle helper and a job left empty mid-week.")


def build():
    dates = days("2026-12-28", 7)
    Mo, Tu, We, Th, Fr, Sa, Su = dates
    FH, EC, LM, RW, SC = ("Fairview Hosp. boiler", "1907 Elm Ct repipe", "Lakeview Mall food court",
                          "Ridgeway Elem. restrooms", "Service calls")
    H = "HOLIDAY"
    rows = [
        ("Gil Navarro", [FH, FH, FH, FH + " AM", H, "", ""]),
        ("Josh Pruett", [EC, EC, EC, "vacation", H, "", ""]),
        ("Kendra Bly", ["vacation"] * 4 + [H, "", ""]),
        ("Arturo Vela", [EC, EC, SC, SC, H, SC, ""]),
        ("Dom Pirelli", [RW, FH, FH, FH, H, "", ""]),
        ("Sean McCabe", [LM, LM, LM, "OFF", H, "", ""]),
        ("Hung Tran", [LM, LM, "sick", LM, H, "", ""]),
        ("Pete Garza", [LM, LM, RW, LM, H, "", ""]),
        ("Marcus Hill", ["", "Shop", "Shop", "", H, "", ""]),
    ]
    recs = []
    for name, cells in rows:
        for d, c in zip(dates, cells):
            if c.endswith(" AM"):
                recs.append(W(name, d, c[:-3], part="am"))
            else:
                recs += person_cell(name, d, c)

    names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
    sheet = [
        ["PINECREST PLUMBING & DRAIN"],
        ["Schedule 12/28/2026 - 1/3/2027      Happy New Year!"],
        [""],
        ["Employee"] + [f"{n} {md(d, year=True)}" for n, d in zip(names, dates)],
    ]
    for name, cells in rows:
        sheet.append([name] + cells)
    sheet += [
        [""],
        ["Office closed Fri 1/1. Emergency line forwards to Arturo this week."],
        ["Thu 12/31 Fairview crew knocks off at noon."],
    ]
    text = csv_text(pad(sheet, 8), lineterminator="\r\n")
    d = fixture_dir(FID, SLUG)
    write_bytes(os.path.join(d, "input.csv"), b"\xef\xbb\xbf" + text.encode("utf-8"))

    flags = [
        flag_nb("Marcus Hill", Mo),
        flag_nb("Marcus Hill", Th),
        flag_un(RW, Tu),
    ]
    expected = dict(workdays=MON_FRI, layout="person_by_day", assignments=recs, flags=flags, info=[],
                    must_not_ask=[{"type": "weekday_mismatch"}])
    meta = dict(window=dates, roster=None, person_canon={}, internal=["Shop"], placeholders=[], subs=[])
    return README, "input.csv", expected, meta
