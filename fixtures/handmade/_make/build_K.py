"""Category K: flag logic.
Run: uv run --with openpyxl==3.1.5 python3 build_K.py

Twelve fixtures for the spec's thirteen K items: "each flag alone" uses PTO for
assigned-while-off (K02), which is also "assigned while on PTO"; "a job with one
assignment all week" is K04 (job grid: unstaffed on its other days, which is
also "only unstaffed") and K10 (list: active range is that one day).

Merges the deterministic matcher must make (spec stage 7) are passed to the
checker as hints: K07 "Mike B" = "Michael Baker" (nickname Mike/Michael plus
initial B), K08 "1420 Johnson St" = "1420 Johnson Street" (St/Street, same
house number). Flags and info use the canonical (longest) spelling.
"""
from fxlib import *

WEEK = days("10-12", 5)
WEEK6 = days("10-12", 6)
J, S, O, P = "1420 Johnson St", "Smith remodel", "Oak Ridge Apts", "Parker kitchen"
NO_ASK = [Q("pick_layout"), Q("column_role"), Q("cell_meaning")]


def exact_info(exp):
    exp = dict(exp)
    exp["info_exact"] = True
    return {k: exp[k] for k in KEY_ORDER if k in exp}


def grid_lines(rows, dates=WEEK, first="Name"):
    return [[first] + [mdyyyy(x) for x in dates]] + [[p] + list(c) for p, c in rows]


def write_grid_xlsx(path, rows, first):
    wb = xlsx_workbook()
    ws = wb.active
    ws.title = "Schedule"
    ws.cell(row=1, column=1, value=first)
    for i, x in enumerate(WEEK):
        ws.cell(row=1, column=2 + i, value=pydate(x)).number_format = "m/d/yyyy"
    for r, (label, cells) in enumerate(rows, start=2):
        ws.cell(row=r, column=1, value=label)
        for i, cell in enumerate(cells):
            if cell:
                ws.cell(row=r, column=2 + i, value=cell)
    wb.save(path)


def list_lines(plan, header=("Date", "Name", "Job"), extra=None):
    """plan: [(person, [site or '' per weekday])]; extra: {(weekday, person): [row tail...]}
    appended after that person's row. Returns (lines, assignments)."""
    lines, assigns = [list(header)], []
    for i, x in enumerate(WEEK):
        for person, sites in plan:
            if sites[i]:
                lines.append([mdyyyy(x), person, sites[i]] + [""] * (len(header) - 3))
                assigns.append(W(person, x, sites[i]))
            for tail, rec in (extra or {}).get((i, person), []):
                lines.append([mdyyyy(x), person] + tail)
                assigns.append(rec)
    return lines, assigns


def k01():
    rows = [
        ("Mike Baker", [J, f"{J} / {S}", J, J, J]),
        ("Dave Kim", [J] * 5),
        ("Jose Garcia", [S] * 5),
        ("Tom Reed", [S] * 5),
        ("Luis Ortiz", [O] * 5),
        ("Ana Ruiz", [O] * 5),
    ]
    exp = exact_info(expected("K", "input.csv", layout="person_by_day", arrangement="single",
                              assignments=person_cells(rows, WEEK), flags=[DB("Mike Baker", "10-13", J, S)],
                              info=[], must_not_ask=NO_ASK))
    emit("K01", "double-booked-only",
         "Only a double-booking: Mike Baker is at 1420 Johnson St and Smith remodel on Tuesday (no times, no parts); nothing else is flagged.",
         {"input.csv": csv_bytes(grid_lines(rows))}, exp, {"window": WEEK})


def k02():
    plan = [("Mike Baker", [J] * 5), ("Dave Kim", [J] * 5), ("Jose Garcia", [S] * 5),
            ("Tom Reed", [S] * 5), ("Luis Ortiz", [O] * 5), ("Ana Ruiz", [O] * 5)]
    extra = {(2, "Luis Ortiz"): [(["", "PTO"], ST("Luis Ortiz", "10-14", "pto"))]}
    lines, assigns = list_lines(plan, ("Date", "Name", "Job", "Status"), extra)
    exp = exact_info(expected("K", "input.csv", layout="long_list", arrangement="single",
                              assignments=assigns, flags=[AWO("Luis Ortiz", "10-14", [O], "pto")],
                              info=[], must_not_ask=NO_ASK))
    emit("K02", "assigned-while-on-pto",
         "Only assigned-while-off: Luis Ortiz has a PTO row on Wednesday and is still listed at Oak Ridge Apts that day.",
         {"input.csv": csv_bytes(lines)}, exp)


def k03():
    rows = [
        ("Mike Baker", [J] * 5),
        ("Dave Kim", [J] * 5),
        ("Jose Garcia", [S] * 5),
        ("Tom Reed", [S] * 5),
        ("Luis Ortiz", [O] * 5),
        ("Ana Ruiz", [O, O, O, "", O]),
    ]
    exp = exact_info(expected("K", "input.tsv", layout="person_by_day", arrangement="single",
                              assignments=person_cells(rows, WEEK), flags=[NB("Ana Ruiz", "10-15")],
                              info=[], must_not_ask=NO_ASK))
    emit("K03", "not-booked-only",
         "Only not-booked (.tsv): Ana Ruiz's Thursday cell is blank, with no job and no status, on a working day; Oak Ridge still has Luis that day.",
         {"input.tsv": csv_bytes(grid_lines(rows), delim="\t")}, exp, {"window": WEEK})


def k04():
    rows = [
        (J, ["Mike Baker, Dave Kim"] * 5),
        (S, ["Jose Garcia, Tom Reed"] * 5),
        (O, ["Luis Ortiz"] + ["Luis Ortiz, Ana Ruiz"] * 4),
        (P, ["Ana Ruiz", "", "", "", ""]),
    ]
    assigns, inactive = job_cells(rows, WEEK)
    assert not inactive
    exp = exact_info(expected("K", "input.csv", layout="job_by_day", arrangement="single",
                              assignments=assigns,
                              flags=[UF(P, "10-13"), UF(P, "10-14"), UF(P, "10-15"), UF(P, "10-16")],
                              info=[], must_not_ask=NO_ASK))
    emit("K04", "unstaffed-only-one-assignment-job-grid",
         "Only unstaffed: in a job x day grid, Parker kitchen has one assignment all week (Ana Ruiz, Monday), so it is unstaffed on each of its other working days.",
         {"input.csv": csv_bytes(grid_lines(rows, first="Job"))}, exp,
         {"window": WEEK, "jobs": [j for j, _ in rows]})


def k05():
    rows = [
        (J, ["Mike Baker, Jose Garcia"] * 5),
        (S, ["Luis Ortiz", "Luis Ortiz, Mike Baker", "Luis Ortiz", "Luis Ortiz", "Luis Ortiz"]),
        (O, ["Dave Kim, Tom Reed"] * 3 + ["Dave Kim, Tom Reed, Ana Ruiz", "Dave Kim"]),
        (P, ["Ana Ruiz", "Ana Ruiz", "Ana Ruiz", "", "Ana Ruiz"]),
        ("PTO", ["", "", "Luis Ortiz", "", ""]),
    ]
    assigns, _ = job_cells(rows, WEEK)
    flags = [DB("Mike Baker", "10-13", J, S), AWO("Luis Ortiz", "10-14", [S], "pto"),
             UF(P, "10-15"), NB("Tom Reed", "10-16")]
    exp = exact_info(expected("K", "input.xlsx", layout="job_by_day", arrangement="single",
                              assignments=assigns, flags=flags, info=[],
                              must_not_ask=NO_ASK + [Q("pick_sheet")]))
    emit("K05", "all-four-flags",
         "All four flags in one job x day grid (.xlsx): Mike Baker on two jobs Tuesday, Luis Ortiz on the PTO row and at Smith remodel Wednesday, Parker kitchen empty Thursday, Tom Reed on nothing Friday.",
         {"input.xlsx": lambda p: write_grid_xlsx(p, rows, "Job")}, exp,
         {"window": WEEK, "jobs": [J, S, O, P]})


def k06():
    rows = [
        ("Mike Baker", [J] * 5),
        ("Dave Kim", [J] * 5),
        ("Jose Garcia", [S] * 5),
        ("Tom Reed", [S, S, "vac", "vac", "vac"]),
        ("Luis Ortiz", [O] * 5),
        ("Ana Ruiz", ["Shop", O, O, O, O]),
    ]
    exp = exact_info(expected("K", "input.xlsx", layout="person_by_day", arrangement="single",
                              assignments=person_cells(rows, WEEK), flags=[], info=[],
                              must_not_ask=NO_ASK + [Q("pick_sheet")]))
    emit("K06", "clean-schedule",
         "A clean week (.xlsx): everyone booked or on vacation every working day, every job staffed, one shop-only day; zero red, zero amber, nothing unstaffed, no info items.",
         {"input.xlsx": lambda p: write_grid_xlsx(p, rows, "Name")}, exp, {"window": WEEK})


def k07():
    rows = [
        (J, ["Michael Baker, Jose Garcia"] * 5),
        (S, ["Luis Ortiz", "Luis Ortiz, Mike B", "Luis Ortiz", "Luis Ortiz", "Luis Ortiz"]),
        (O, ["Dave Kim, Tom Reed"] * 5),
    ]
    assigns, _ = job_cells(rows, WEEK)
    exp = exact_info(expected("K", "input.csv", layout="job_by_day", arrangement="single",
                              assignments=assigns, flags=[DB("Michael Baker", "10-13", J, S)], info=[],
                              must_not_ask=NO_ASK + [Q("same_person", a="Mike B", b="Michael Baker")]))
    emit("K07", "double-booked-after-name-merge",
         "'Mike B' at Smith remodel on Tuesday is Michael Baker (nickname Mike + initial B), who is at 1420 Johnson St that day: a double-booking that only shows once the names merge, flagged as 'Michael Baker'.",
         {"input.csv": csv_bytes(grid_lines(rows, first="Job"))}, exp,
         {"window": WEEK, "jobs": [j for j, _ in rows], "same_person": [["Mike B", "Michael Baker"]]})


def k08():
    JS = "1420 Johnson Street"
    plan = [("Mike Baker", [J] * 5), ("Dave Kim", [J] * 5), ("Jose Garcia", [S] * 5), ("Luis Ortiz", [O] * 5)]
    lines, assigns = list_lines(plan)
    lines.append([mdyyyy(d("10-14")), "Dave Kim", JS])  # a second row typed later, other spelling
    assigns.append(W("Dave Kim", "10-14", JS))
    exp = exact_info(expected("K", "input.csv", layout="long_list", arrangement="single",
                              assignments=assigns, flags=[], info=[DUP("Dave Kim", "10-14", JS)],
                              must_not_ask=NO_ASK + [Q("same_site", a=J, b=JS)]))
    emit("K08", "false-double-booking-site-merge",
         "Dave Kim is listed at '1420 Johnson St' and, in a row added at the bottom, '1420 Johnson Street' on Wednesday: St/Street with the same house number is one site, so it's a duplicate entry (info), not a double-booking.",
         {"input.csv": csv_bytes(lines)}, exp, {"same_site": [[J, JS]]})


def k09():
    plan = [("Mike Baker", [J] * 5), ("Dave Kim", [J] * 5), ("Jose Garcia", [S] * 5), ("Luis Ortiz", [O] * 5)]
    extra = {(0, "Jose Garcia"): [([S], W("Jose Garcia", "10-12", S))]}
    lines, assigns = list_lines(plan, extra=extra)
    exp = exact_info(expected("K", "input.tsv", layout="long_list", arrangement="single",
                              assignments=assigns, flags=[], info=[DUP("Jose Garcia", "10-12", S)],
                              must_not_ask=NO_ASK))
    emit("K09", "same-person-twice-one-site",
         "Jose Garcia is listed twice at Smith remodel on Monday (a copied row, .tsv): a duplicate entry (info), never a double-booking.",
         {"input.tsv": csv_bytes(lines, delim="\t")}, exp)


def k10():
    plan = [("Mike Baker", [J] * 5), ("Dave Kim", [J, J, P, J, J]), ("Jose Garcia", [S] * 5), ("Luis Ortiz", [O] * 5)]
    lines, assigns = list_lines(plan)
    exp = exact_info(expected("K", "input.csv", layout="long_list", arrangement="single",
                              assignments=assigns, flags=[], info=[], must_not_ask=NO_ASK))
    emit("K10", "one-assignment-job-list",
         "In a long list, Parker kitchen appears once all week (Dave Kim, Wednesday): its active range is that one day, so it is never unstaffed.",
         {"input.csv": csv_bytes(lines)}, exp)


def k11():
    assert wd(WEEK6[5]) == "Sat"
    rows = [
        ("Mike Baker", [J] * 5 + [f"{J} / {S}"]),
        ("Dave Kim", [J] * 5 + [""]),
        ("Jose Garcia", [S] * 6),
        ("Tom Reed", [S] * 5 + [""]),
        ("Luis Ortiz", [O] * 5 + [""]),
        ("Ana Ruiz", [O] * 5 + [""]),
    ]
    exp = exact_info(expected("K", "input.csv", settings_=settings(WD5), layout="person_by_day",
                              arrangement="single", assignments=person_cells(rows, WEEK6),
                              flags=[DB("Mike Baker", "10-17", J, S)], info=[], must_not_ask=NO_ASK))
    emit("K11", "saturday-under-mon-fri",
         "A Saturday column under a Mon-Fri workweek: Saturday work is still read and Mike's Saturday double-booking is flagged, but blank Saturday cells are not 'not booked' and Oak Ridge is not unstaffed on Saturday.",
         {"input.csv": csv_bytes(grid_lines(rows, WEEK6))}, exp, {"window": WEEK6})


def k12():
    SUB = "ABC Electric LLC"
    rows = [
        (J, ["Mike Baker, Jose Garcia", f"Mike Baker, Jose Garcia, {SUB}"] + ["Mike Baker, Jose Garcia"] * 3),
        (S, ["Luis Ortiz, Dave Kim", f"Luis Ortiz, Dave Kim, {SUB}", "Luis Ortiz, Dave Kim, Ana Ruiz",
             "Luis Ortiz, Dave Kim", "Luis Ortiz, Dave Kim"]),
        (P, ["Ana Ruiz", "Ana Ruiz", SUB, "Ana Ruiz", "Ana Ruiz"]),
    ]
    assigns, _ = job_cells(rows, WEEK)
    exp = exact_info(expected("K", "input.csv", layout="job_by_day", arrangement="single",
                              assignments=assigns, flags=[], info=[SUBMULTI(SUB, "10-13", J, S)],
                              must_not_ask=NO_ASK))
    emit("K12", "subcontractor-two-sites",
         "Subcontractor 'ABC Electric LLC' at 1420 Johnson St and Smith remodel on Tuesday is info, not a double-booking; it is never 'not booked', and alone it still staffs Parker kitchen on Wednesday.",
         {"input.csv": csv_bytes(grid_lines(rows, first="Job"))}, exp,
         {"window": WEEK, "jobs": [j for j, _ in rows]})


if __name__ == "__main__":
    for f in (k01, k02, k03, k04, k05, k06, k07, k08, k09, k10, k11, k12):
        f()
    print("K done")
