"""H18: painting contractor, job x day grid exported as CSV (CRLF), M/D/YYYY headers, Saturday column."""
from common import *

FID, SLUG = "H18", "painter-job-grid-crlf-csv"
README = ("Painting contractor's job x day grid saved as Excel CSV with CRLF: M/D/YYYY date headers with a "
          "Saturday column under a Mon-Fri week, quoted multi-name cells split by commas, '&' and '/', "
          "'no work' and 'done' cells, TBD, a blank job-day, Shop and OFF rows, a floater with gaps, "
          "16 painters on 7 jobs, title rows and footer notes.")


def build():
    dates = days("2026-10-26", 6)
    Mo, Tu, We, Th, Fr, Sa = dates
    WA, GM, SH, MA, FB, TC, GS = ("Windsor Apts bldg 4", "Grandview Medical 2nd fl", "Shelby res. ext.",
                                  "Marlowe res. int.", "First Baptist fellowship hall", "Tinsley condo 5C",
                                  "Gas station #14 canopy")
    jobs = [
        (WA, ["Bo, Lamar, Wendell", "Bo, Lamar, Wendell", "Bo, Lamar, Wendell", "Bo, Lamar", "Bo, Lamar, Wendell",
              "Bo, Lamar"]),
        (GM, ["Ricky & Sergio", "Ricky & Sergio", "Ricky & Sergio & Ana", "Ricky & Sergio", "Ricky", ""]),
        (SH, ["Kat / Dev", "Kat / Dev", "no work", "Kat / Dev", "Kat / Dev / Zeke", ""]),
        (MA, ["Jorge, Nikolai", "Jorge, Nikolai", "Jorge, Nikolai", "Jorge, Nikolai, Ana", "Jorge", ""]),
        (FB, ["Reggie, Tasha", "Reggie, Tasha", "Reggie, Tasha", "", "Reggie, Tasha, Vince", "Reggie, Tasha"]),
        (TC, ["Vince, Yolanda", "Vince, Yolanda", "Vince", "Vince, Yolanda, Reggie, Tasha", "Yolanda", ""]),
        (GS, ["TBD", "Zeke", "Zeke", "Zeke, Dev", "done", ""]),
        ("Shop", ["", "", "Kat, Dev", "", "", ""]),
    ]
    off_row = ["Ana", "Nikolai", "", "Wendell", "Sergio", ""]
    recs, inactive = [], []
    for job, cells in jobs:
        for d, c in zip(dates, cells):
            if c and norm(c).lower() in INACTIVE:
                inactive.append([job, d])
            recs += names_cell(job, d, c)
    for d, c in zip(dates, off_row):
        recs += names_cell("OFF", d, c, status="off")

    sheet = [
        ["BIG SKY PAINTING CO."],
        ["Schedule wk of 10/26"],
        [""],
        ["Job"] + [md(d, year=True) for d in dates],
    ]
    for job, cells in jobs:
        sheet.append([job] + cells)
    sheet.append(["OFF"] + off_row)
    sheet += [[""], ["Ana floats - call her the night before."], ["Sat = OT, sign up with Bo. Rain plan: Shelby crew to shop."]]
    text = csv_text(pad(sheet, 7), lineterminator="\r\n")
    d = fixture_dir(FID, SLUG)
    write_bytes(os.path.join(d, "input.csv"), text.encode("utf-8"))

    flags = [
        flag_db("Dev", Th, [SH, GS]),
        flag_awo("Nikolai", Tu, [MA], "off"),
        flag_nb("Ana", Tu),
        flag_nb("Ana", Fr),
        flag_nb("Zeke", Mo),
        flag_nb("Nikolai", Fr),
        flag_nb("Yolanda", We),
        flag_un(GS, Mo),
        flag_un(FB, Th),
    ]
    expected = dict(workdays=MON_FRI, layout="job_by_day", assignments=recs, flags=flags, info=[])
    meta = dict(window=dates, roster=None, person_canon={}, internal=["Shop"], placeholders=[], subs=[],
                jobs=[j for j, _ in jobs], inactive=inactive, pseudo_rows=["OFF"])
    return README, "input.csv", expected, meta
