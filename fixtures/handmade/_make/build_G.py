"""Category G: status words and unstaffed markers.
Run: uv run --with openpyxl==3.1.5 python3 build_G.py

Readings chosen where FORMAT.md is silent:
  - A trailing note that is exactly a part phrase ("(AM only)", " - half day")
    sets `part` and is not also kept as a `note`.
  - In a list, an explicit unstaffed row (TBD, open, ?, need guy, need 1 in the
    name column) counts as an assigned day for the job's Mon-Sun range, as
    FORMAT.md says, so a gap between a staffed day and a TBD day is unstaffed.
"""
from fxlib import *

WEEK = days("10-12", 5)
J, S, O, P, L = "1420 Johnson St", "Smith remodel", "Oak Ridge Apts", "Parker kitchen", "Lincoln School"
NO_ASK = [Q("pick_layout"), Q("column_role"), Q("cell_meaning")]


def grid_lines(rows, first="Name"):
    return [[first] + [mdyyyy(x) for x in WEEK]] + [[p] + list(c) for p, c in rows]


def g01():
    rows = [
        ("Mike Baker", ["holiday", J, J, J, J]),
        ("Dave Kim", [J, J, "PTO", J, J]),
        ("Jose Garcia", [S, "off", S, S, S]),
        ("Tom Reed", [S, S, S, "vac", "vac"]),
        ("Luis Ortiz", [O, O, "OFF", O, O]),
        ("Ana Ruiz", [O, "sick", O, O, "out"]),
    ]
    exp = expected("G", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=person_cells(rows, WEEK), flags=[], info=[], must_not_ask=NO_ASK)
    emit("G01", "status-words-person-grid",
         "Person x day grid with the status words holiday, off, OFF, PTO, vac, sick and out in cells: each is a status record with no site, counts as booked, and is never a job.",
         {"input.csv": csv_bytes(grid_lines(rows))}, exp, {"window": WEEK})


def g02():
    rows = [
        ("Mike Baker", [J, J, "x", J, J]),
        ("Dave Kim", [J, "—", J, J, J]),
        ("Jose Garcia", [S, S, S, "n/a", S]),
        ("Tom Reed", [S, S, S, S, "day off"]),
        ("Luis Ortiz", [O, O, O, O, "-"]),
        ("Ana Ruiz", [O] * 5),
    ]

    def make(path):
        wb = xlsx_workbook()
        ws = wb.active
        ws.title = "Schedule"
        ws.cell(row=1, column=1, value="Name")
        for i, x in enumerate(WEEK):
            ws.cell(row=1, column=2 + i, value=pydate(x)).number_format = "m/d/yyyy"
        for r, (person, cells) in enumerate(rows, start=2):
            ws.cell(row=r, column=1, value=person)
            for i, cell in enumerate(cells):
                ws.cell(row=r, column=2 + i, value=cell)
        wb.save(path)

    exp = expected("G", "input.xlsx", layout="person_by_day", arrangement="single",
                   assignments=person_cells(rows, WEEK), flags=[], info=[],
                   must_not_ask=NO_ASK + [Q("pick_sheet")])
    emit("G02", "status-symbols-x-dash-na",
         "Person x day grid (.xlsx) with off days written as x, — (em dash), n/a, a lone - and 'day off': in a person's row each is status off, not a site.",
         {"input.xlsx": make}, exp, {"window": WEEK})


def g03():
    plan = [  # person, job, {weekday index: status text}
        ("Mike Baker", J, {4: "holiday"}),
        ("Dave Kim", J, {3: "vac"}),
        ("Jose Garcia", S, {2: "PTO"}),
        ("Tom Reed", S, {3: "out"}),
        ("Luis Ortiz", O, {1: "Sick"}),
        ("Ana Ruiz", O, {4: "off"}),
    ]
    lines, assigns = [["Date", "Name", "Job", "Status"]], []
    for i, x in enumerate(WEEK):
        for person, job, st in plan:
            if i in st:
                lines.append([mdyyyy(x), person, "", st[i]])
                assigns.append(ST(person, x, STATUS_TEXT[st[i].lower()]))
            else:
                lines.append([mdyyyy(x), person, job, ""])
                assigns.append(W(person, x, job))
    exp = expected("G", "input.tsv", layout="long_list", arrangement="single",
                   assignments=assigns, flags=[], info=[], must_not_ask=NO_ASK)
    emit("G03", "status-column-list",
         "Long list (.tsv) with a Status column: rows with a blank Job and PTO, Sick, vac, out, holiday or off are status records, not work and not jobs.",
         {"input.tsv": csv_bytes(lines, delim="\t")}, exp)


def g04():
    rows = [
        (J, ["Mike, Dave"] * 5),
        (S, ["Jose, Tom"] * 5),
        (O, ["Luis", "Luis", "Luis, Ana", "Luis", "Luis, Ana"]),
        (P, ["TBD", "open", "?", "x", "need 1"]),
        (L, ["Ana", "Ana", "need guy", "Ana", "—"]),
    ]
    assigns, inactive = job_cells(rows, WEEK)
    assert inactive == [[P, "2026-10-15"], [L, "2026-10-16"]], inactive
    flags = [UF(P, "10-12"), UF(P, "10-13"), UF(P, "10-14"), UF(P, "10-16"), UF(L, "10-14")]
    exp = expected("G", "input.csv", layout="job_by_day", arrangement="single",
                   assignments=assigns, flags=flags, info=[], must_not_ask=NO_ASK)
    emit("G04", "unstaffed-markers-job-grid",
         "Job x day grid with TBD, open, ?, need guy and need 1 in cells (each an explicit unstaffed job-day, no person), beside x and — cells that mark the job inactive that day and are not flagged.",
         {"input.csv": csv_bytes(grid_lines(rows, first="Job"))}, exp,
         {"window": WEEK, "jobs": [j for j, _ in rows], "inactive": inactive})


def g05():
    crew = [  # person, job per weekday
        ("Mike Baker", [J] * 5),
        ("Dave Kim", [J] * 5),
        ("Jose Garcia", [S] * 5),
        ("Tom Reed", [P, S, S, S, P]),
        ("Luis Ortiz", [O] * 5),
        ("Ana Ruiz", [L, O, O, O, O]),
    ]
    markers = {(P, 2): "TBD", (P, 3): "need guy", (L, 2): "?", (L, 3): "open", (L, 4): "need 1"}
    order = [J, S, O, P, L]
    lines, assigns = [["Date", "Job", "Employee"]], []
    for i, x in enumerate(WEEK):
        for job in order:
            for person, jobs in crew:
                if jobs[i] == job:
                    lines.append([mdyyyy(x), job, person])
                    assigns.append(W(person, x, job))
            if (job, i) in markers:
                lines.append([mdyyyy(x), job, markers[(job, i)]])
                assigns.append(UN(x, job))
    flags = [UF(P, "10-13"), UF(P, "10-14"), UF(P, "10-15"),
             UF(L, "10-13"), UF(L, "10-14"), UF(L, "10-15"), UF(L, "10-16")]
    exp = expected("G", "input.csv", layout="long_list", arrangement="single",
                   assignments=assigns, flags=flags, info=[], must_not_ask=NO_ASK)
    emit("G05", "unstaffed-markers-list",
         "Long list with TBD, need guy, ?, open and need 1 in the Employee column: each is an explicit unstaffed job-day, and those days count toward the job's range, so Lincoln School's empty Tuesday is unstaffed too.",
         {"input.csv": csv_bytes(lines)}, exp)


def g06():
    rows = [
        ("Mike Baker", [J] * 5),
        ("Dave Kim", [J, J, J, J, f"{J} - AM only"]),
        ("Jose Garcia", [S, S, f"{S} (AM only)", S, S]),
        ("Tom Reed", [S, f"{S} (half day)", S, S, S]),
        ("Luis Ortiz", [O, O, O, f"{O} - half day", O]),
        ("Ana Ruiz", [O] * 5),
    ]
    assigns = person_cells(rows, WEEK)
    assert sum("part" in a for a in assigns) == 4 and not any("note" in a for a in assigns)
    exp = expected("G", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=assigns, flags=[], info=[], must_not_ask=NO_ASK)
    emit("G06", "parts-half-day-am-only",
         "Partial days written in person-grid cells ('(AM only)', '- AM only', '(half day)', '- half day') set part am or half on the record; they are not notes and not part of the site name.",
         {"input.csv": csv_bytes(grid_lines(rows))}, exp, {"window": WEEK})


def g07():
    K = "Lopez-Hart kitchen"
    rows = [
        ("Mike Baker", ["Johnson - bring ladder", "Johnson", "Johnson", "Johnson", "Johnson"]),
        ("Dave Kim", ["Johnson", "Johnson", "Johnson - pick up permit at city", "Johnson", "Johnson"]),
        ("Jose Garcia", ["Smith", "Smith (punch list)", "Smith", "Smith", "Smith"]),
        ("Tom Reed", ["Smith", "Smith", "Smith", "Smith (meet inspector)", "Smith"]),
        ("Luis Ortiz", [K, K, K, K, f"{K} - walkthrough with owner"]),
        ("Ana Ruiz", [K, f"{K} (tile delivery)", K, K, K]),
    ]
    assigns = person_cells(rows, WEEK)
    assert {a["site"] for a in assigns} == {"Johnson", "Smith", K}
    exp = expected("G", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=assigns, flags=[], info=[], must_not_ask=NO_ASK + [Q("same_site")])
    emit("G07", "notes-inside-cells",
         "Notes inside cells: 'Johnson - bring ladder' and 'Smith (punch list)' give sites Johnson and Smith with the note split off; the hyphen in 'Lopez-Hart kitchen' has no spaces around it and stays in the site.",
         {"input.csv": csv_bytes(grid_lines(rows))}, exp, {"window": WEEK})


def g08():
    rows = [
        ("Mike Baker", [J, J, "PTO (dentist)", J, J]),
        ("Dave Kim", [J] * 5),
        ("Jose Garcia", [S, "sick - flu", S, S, S]),
        ("Tom Reed", [S] * 5),
        ("Luis Ortiz", [O, O, O, "out (jury duty)", O]),
        ("Ana Ruiz", [O, O, O, O, "vac - Cabo"]),
    ]
    assigns = person_cells(rows, WEEK)
    assert sum("status" in a and "note" in a for a in assigns) == 4
    exp = expected("G", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=assigns, flags=[], info=[], must_not_ask=NO_ASK)
    emit("G08", "status-with-note",
         "Status cells with a trailing note ('PTO (dentist)', 'sick - flu', 'out (jury duty)', 'vac - Cabo') are status records that keep the note; none is read as a site.",
         {"input.csv": csv_bytes(grid_lines(rows))}, exp, {"window": WEEK})


if __name__ == "__main__":
    for f in (g01, g02, g03, g04, g05, g06, g07, g08):
        f()
    print("G done")
