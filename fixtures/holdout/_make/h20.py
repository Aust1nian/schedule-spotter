"""H20: heating & air company's long list in xlsx with real date/time cells and a TECHS roster block."""
from common import *
import datetime as dt
from openpyxl import Workbook
from openpyxl.styles import Font, PatternFill

FID, SLUG = "H20", "hvac-xlsx-list-start-end-roster-block"
README = ("Heating and air company's xlsx long list: real Excel date cells shown as 'Mon 10/26' and time "
          "cells for Start/End, blank rows between days, a TECHS roster block beside the list that names "
          "'Robert Dunn' while every row says 'Bob Dunn', overlapping times (flag) vs split day vs office "
          "shop time, PTO/sick rows, a duplicated row left beside a sick row, a Temp, and TBD for a 2nd tech.")


def tm(s):
    h, m = s.split(":")
    return dt.time(int(h), int(m))


def build():
    Mo, Tu, We, Th, Fr = days("2026-10-26", 5)
    NF, CU, HR, AD, PW = ("Northfield Middle School RTU", "Cascade Credit Union", "Harlow res. furnace",
                          "Ames Family Dental", "Pinewood Sr. Living")
    full, early = ("07:00", "15:30"), ("06:00", "14:30")
    none = ("", "")
    days_rows = [
        [(Mo, "Bob Dunn", NF, early), (Mo, "Kelly Marsh", NF, early), (Mo, "Victor Han", CU, full),
         (Mo, "Sean Doherty", HR, ("08:00", "12:00")), (Mo, "Sean Doherty", AD, ("13:00", "16:30")),
         (Mo, "Amir Nasser", PW, full), (Mo, "Paul Lindqvist", "Shop", full), (Mo, "Greta Olson", PW, full),
         (Mo, "Hugo Brandt", "PTO", none)],
        [(Tu, "Bob Dunn", NF, early), (Tu, "Kelly Marsh", NF, early), (Tu, "Victor Han", CU, full),
         (Tu, "Sean Doherty", AD, full), (Tu, "Amir Nasser", PW, full), (Tu, "Greta Olson", PW, full),
         (Tu, "Paul Lindqvist", CU, ("07:00", "11:00")), (Tu, "Paul Lindqvist", "Shop", ("12:00", "15:30")),
         (Tu, "Hugo Brandt", "PTO", none)],
        [(We, "Bob Dunn", NF, early), (We, "Bob Dunn", HR, ("13:00", "16:00")), (We, "Kelly Marsh", NF, early),
         (We, "Victor Han", CU, full), (We, "Sean Doherty", AD, full), (We, "Amir Nasser", PW, full),
         (We, "Greta Olson", "sick", none), (We, "Paul Lindqvist", "Shop", none), (We, "Hugo Brandt", CU, full),
         (We, "Temp", PW, none), (We, "Greta Olson", PW, full)],
        [(Th, "Bob Dunn", NF, early), (Th, "Kelly Marsh", NF, early), (Th, "Sean Doherty", AD, none),
         (Th, "Amir Nasser", PW, none), (Th, "Greta Olson", PW, none), (Th, "Paul Lindqvist", "Shop", none),
         (Th, "Hugo Brandt", CU, none)],
        [(Fr, "Bob Dunn", NF, none), (Fr, "Kelly Marsh", NF, none), (Fr, "TBD", NF, none),
         (Fr, "Victor Han", CU, none), (Fr, "Sean Doherty", AD, none), (Fr, "Amir Nasser", PW, none),
         (Fr, "Greta Olson", PW, none), (Fr, "Paul Lindqvist", "Shop", none), (Fr, "Hugo Brandt", CU, none)],
    ]
    techs = [("Robert Dunn", "Van 3"), ("Kelly Marsh", "Van 1"), ("Victor Han", "Van 4"), ("Sean Doherty", "Van 2"),
             ("Amir Nasser", "Van 5"), ("Paul Lindqvist", "shop truck"), ("Greta Olson", "Van 6"),
             ("Hugo Brandt", "Van 7")]
    recs = []
    for group in days_rows:
        for d, tech, job, (s, e) in group:
            if tech.lower() in UNSTAFFED:
                recs.append(U(d, job))
            elif job.lower() in STATUS:
                recs.append(S(tech, d, STATUS[job.lower()]))
            else:
                recs.append(W(tech, d, job, start=s or None, end=e or None))

    wb = Workbook()
    ws = wb.active
    ws.title = "Board"
    ws.cell(1, 1, "FJORD HEATING & AIR — INSTALL & SERVICE BOARD").font = Font(bold=True, size=15)
    ws.merge_cells("A1:E1")
    ws.cell(2, 1, "Week of 10/26/2026").font = Font(italic=True)
    ws.merge_cells("A2:E2")
    fill = PatternFill("solid", fgColor="D9E1F2")
    for j, h in enumerate(["Date", "Tech", "Job", "Start", "End"], start=1):
        c = ws.cell(4, j, h)
        c.font = Font(bold=True)
        c.fill = fill
    r = 5
    for group in days_rows:
        for d, tech, job, (s, e) in group:
            c = ws.cell(r, 1, D(d))
            c.number_format = "ddd m/d"
            ws.cell(r, 2, tech)
            ws.cell(r, 3, job)
            if s:
                ws.cell(r, 4, tm(s)).number_format = "h:mm AM/PM"
                ws.cell(r, 5, tm(e)).number_format = "h:mm AM/PM"
            r += 1
        r += 1  # blank row between days
    ws.cell(r, 1, "Northfield needs a 2nd tech Fri for the crane lift - call Kelly if you can cover.")
    ws.cell(4, 8, "TECHS").font = Font(bold=True)
    ws.cell(4, 8).fill = fill
    ws.cell(4, 9, "Truck").font = Font(bold=True)
    ws.cell(4, 9).fill = fill
    for i, (n, v) in enumerate(techs, start=5):
        ws.cell(i, 8, n)
        ws.cell(i, 9, v)
    for col, w in zip("ABCDEFGHI", [12, 16, 30, 11, 11, 3, 3, 18, 12]):
        ws.column_dimensions[col].width = w
    d = fixture_dir(FID, SLUG)
    wb.save(os.path.join(d, "input.xlsx"))

    flags = [
        flag_db("Robert Dunn", We, [NF, HR]),
        flag_awo("Greta Olson", We, [PW], "sick"),
        flag_nb("Victor Han", Th),
        flag_un(NF, Fr),
        flag_un(HR, Tu),
    ]
    infos = [
        info("split_day", "Sean Doherty", Mo, [HR, AD]),
        info("internal_plus_site", "Paul Lindqvist", Tu, [CU, "Shop"]),
    ]
    expected = dict(workdays=MON_FRI, layout="long_list", assignments=recs, flags=flags, info=infos)
    meta = dict(window=None, roster=[n for n, _ in techs], person_canon={"Bob Dunn": "Robert Dunn"},
                internal=["Shop"], placeholders=["Temp"], subs=[])
    return README, "input.xlsx", expected, meta
