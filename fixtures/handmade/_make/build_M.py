"""Category M: paste input. Run: python3 build_M.py

Free-text pastes name weekdays with no week. They resolve to the week of
Mon 2026-10-12: with today = Sat 2026-10-10 that is both the nearest and
the next Monday, so any reasonable anchoring rule agrees.
"""
from fxlib import *

WEEK = days("10-12", 5)
J, S, O = "1420 Johnson St", "Smith remodel", "Oak Ridge Apts"
P, E = "Parker kitchen", "88 Elm Ave"
TABLE_QS = [Q("paste_as_table"), Q("pick_layout"), Q("column_role")]


def text(s: str, eol="\n") -> bytes:
    return s.replace("\n", eol).encode("utf-8")


def m01():
    rows = [("Mike Baker", [J] * 5), ("Jose Garcia", [S, S, S, S, f"{S} / {J}"]), ("Luis Ortiz", [O] * 5), ("Dave Kim", [J] * 5)]
    # Excel's clipboard text: tab-separated, CRLF after every row including the last, displayed values.
    data = csv_bytes([["Name"] + [wd_md(x) for x in WEEK]] + [[p] + c for p, c in rows], delim="\t", eol="\r\n")
    exp = expected("M", "paste.txt", paste=True, layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[DB("Jose Garcia", "10-16", S, J)], info=[], must_not_ask=TABLE_QS)
    emit("M01", "paste-from-excel", "Cells copied from Excel and pasted: tab-separated, CRLF after every row including the last.",
         {"paste.txt": data}, exp, {"window": WEEK})


def m02():
    rows = [(J, ["Mike, Dave"] * 5), (S, ["Jose"] * 5), (O, ["Luis, Ana"] * 4 + ["Luis"]), ("PTO", ["", "", "", "", "Ana"])]
    # Google Sheets' clipboard text: tab-separated, LF between rows, no newline after the last row.
    data = csv_bytes([["Job"] + [mdyyyy(x) for x in WEEK]] + [[j] + c for j, c in rows], delim="\t", eol="\n", final_eol=False)
    exp = expected("M", "paste.txt", paste=True, layout="job_by_day", arrangement="single",
                   assignments=job_grid(rows, WEEK), flags=[], info=[], must_not_ask=TABLE_QS)
    emit("M02", "paste-from-google-sheets",
         "Cells copied from Google Sheets: tab-separated, LF, no trailing newline; a job x day grid with a 'PTO' pseudo-row that is not a job.",
         {"paste.txt": data}, exp, {"window": WEEK, "jobs": [J, S, O]})


def m03():
    rows = [("Tom Reed", [P] * 5), ("Ana Ruiz", [P] * 5), ("Carlos Mendez", [E] * 5), ("Ray Jones", [E, E, E, f"{E} / vac", E])]
    fmt = lambda x: f"{mon_dd(x)}, {x[:4]}"  # Numbers' medium date: "Oct 12, 2026"
    assert fmt(WEEK[0]) == "Oct 12, 2026"
    data = csv_bytes([["Date", "Name", "Job"]] + grid_to_list_rows(rows, WEEK, fmt), delim="\t", eol="\n")
    exp = expected("M", "paste.txt", paste=True, layout="long_list", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[AWO("Ray Jones", "10-15", [E], "vacation")], info=[], must_not_ask=TABLE_QS)
    emit("M03", "paste-from-numbers",
         "Cells copied from Apple Numbers: tab-separated, LF, dates as Numbers displays them ('Oct 12, 2026').",
         {"paste.txt": data}, exp)


def m04():
    msg = "Mon: Mike & Jose @ Johnson. Tue: Mike - Smith job\n"
    exp = expected("M", "paste.txt", tier="model", paste=True,
                   assignments=[W("Mike", "10-12", "Johnson"), W("Jose", "10-12", "Johnson"), W("Mike", "10-13", "Smith job")],
                   flags=[NB("Jose", "10-13")], info=[],
                   ask=[Q("paste_as_table")])
    emit("M04", "paste-text-message",
         "Free text from a text message ('Mon: Mike & Jose @ Johnson. Tue: Mike - Smith job'), read line by line by the model (T5); with the model off it must ask paste_as_table instead.",
         {"paste.txt": text(msg)}, exp)


def m05():
    msg = ("Week of 10/12\n"
           "• Mike: Johnson Mon Tue Wed, Smith remodel Thu Fri\n"
           "• Jose: Smith remodel Mon thru Fri\n"
           "• Luis: Oak Ridge Mon thru Thu, Smith remodel Thu Fri\n"
           "• Dave: Johnson Mon thru Fri\n")
    a = [W("Mike", x, "Johnson") for x in WEEK[:3]] + [W("Mike", x, "Smith remodel") for x in WEEK[3:]]
    a += [W("Jose", x, "Smith remodel") for x in WEEK]
    a += [W("Luis", x, "Oak Ridge") for x in WEEK[:4]] + [W("Luis", x, "Smith remodel") for x in WEEK[3:]]
    a += [W("Dave", x, "Johnson") for x in WEEK]
    exp = expected("M", "paste.txt", tier="model", paste=True,
                   assignments=a, flags=[DB("Luis", "10-15", "Oak Ridge", "Smith remodel")], info=[],
                   ask=[Q("paste_as_table")])
    emit("M05", "paste-bullet-list",
         "A pasted bullet list from a notes app ('• Luis: Oak Ridge Mon thru Thu, Smith remodel Thu Fri') under 'Week of 10/12'; needs the model, else ask paste_as_table.",
         {"paste.txt": text(msg)}, exp)


def m06():
    wk = WEEK[:3]
    rows = [("Ana Ruiz", [P] * 3), ("Tom Reed", [P] * 3), ("Carlos Mendez", [E] * 3), ("Ray Jones", [E, E, "Shop"])]
    data = csv_bytes([["Name"] + [mdyyyy(x) for x in wk]] + [[p] + c for p, c in rows], delim="\t", eol="\r\n")
    exp = expected("M", "paste.txt", paste=True, layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, wk), flags=[], info=[], must_not_ask=TABLE_QS)
    emit("M06", "paste-partial-week",
         "A partial week pasted from Excel (only Mon-Wed columns): Thu and Fri are outside the window, so nobody is 'not booked' on them.",
         {"paste.txt": data}, exp, {"window": wk})


def m07():
    msg = ("Lunes: José y Luis en Johnson, Pedro en Smith\n"
           "Martes: José en Smith, Luis y Pedro en Oak Ridge\n"
           "Miércoles: Luis y Pedro en Oak Ridge, José y Luis en Smith\n")
    mon, tue, wed = WEEK[:3]
    a = [W("José", mon, "Johnson"), W("Luis", mon, "Johnson"), W("Pedro", mon, "Smith"),
         W("José", tue, "Smith"), W("Luis", tue, "Oak Ridge"), W("Pedro", tue, "Oak Ridge"),
         W("Luis", wed, "Oak Ridge"), W("Pedro", wed, "Oak Ridge"), W("José", wed, "Smith"), W("Luis", wed, "Smith")]
    exp = expected("M", "paste.txt", tier="model", paste=True,
                   assignments=a, flags=[DB("Luis", "10-14", "Oak Ridge", "Smith")], info=[],
                   ask=[Q("paste_as_table")])
    emit("M07", "paste-spanish-free-text",
         "Spanish free text ('Lunes: José y Luis en Johnson, Pedro en Smith'): weekday names, 'y' joining names, 'en' before the site; needs the model.",
         {"paste.txt": text(msg)}, exp)


def m08():
    msg = ("hey can you put mike and dave on the parker job tomorrow, jose stays at the shop "
           "til the trailer gets back. luis is out friday\n")
    exp = expected("M", "paste.txt", paste=True, assignments=[], flags=[], info=[],
                   ask=[Q("paste_as_table")])
    emit("M08", "paste-free-text-model-off",
         "Free-form pasted text with the model off: nothing is guessed; the app asks for a table instead (paste_as_table).",
         {"paste.txt": text(msg)}, exp)


if __name__ == "__main__":
    for f in (m01, m02, m03, m04, m05, m06, m07, m08):
        f()
    print("M done")
