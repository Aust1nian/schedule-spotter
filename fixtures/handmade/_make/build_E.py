"""Category E: people and names (spec stage 7, deterministic matching and the ask-the-user path).
Run: uv run --with openpyxl==3.1.5 python3 build_E.py

Matching readings encoded here (the pipeline's stage 7 rules, model off):
  - SAME, merged without asking: spellings equal after normalizing case,
    spacing, accents and punctuation; nickname-table pairs with the same surname
    (Mike/Mikey/Michael, Bob/Robert, Paco/Francisco, Chuy/Jesús, Nacho/Ignacio,
    Pepe/José ...); a surname initial that fits exactly one roster person
    ("Mike B", "M. Baker"); a lone last name that fits exactly one.
  - DIFFERENT, never merged and never asked: conflicting initials (Mike B / Mike
    R); Sr vs Jr; same first name with different surnames; different first names
    with one surname; placeholders and subcontractors vs anyone.
  - UNSURE, asked and kept separate until answered: typos (Micheal), a lone
    first name (Ana / Ana Ruiz: another Ana may exist), a lone first name that
    fits two people (Chris), a dropped second surname (José García López /
    José García).
Changed 2026-10-11 (TEST_REPORT.md "Changed expectations"): E01 Mikey merges
by the nickname table; E04 lone first name 'Ana' is asked.
Questions name each side by its canonical spelling (the longest in its group
after the deterministic merges). Deterministic merges go to the checker as
`same_person` hints; answers are applied by the checker for `after_answers`.
Flags never name a merged person whose longest spellings tie (Jose Garcia /
José García, Micheal / Michael Baker): FORMAT.md has no tie-break.
"""
from fxlib import *

WEEK = days("10-12", 5)
J, S, O, P = "1420 Johnson St", "Smith remodel", "Oak Ridge Apts", "Parker kitchen"
NO_ASK = [Q("pick_layout"), Q("column_role"), Q("cell_meaning")]
NO_PERSON_Q, NO_SITE_Q = Q("same_person"), Q("same_site")


def SP(a, b):
    return Q("same_person", a=a, b=b)


def exact_info(exp):
    exp = dict(exp)
    exp["info_exact"] = True
    return {k: exp[k] for k in KEY_ORDER if k in exp}


def grid_lines(rows, first):
    return [[first] + [mdyyyy(x) for x in WEEK]] + [[label] + list(cells) for label, cells in rows]


def e01():
    MB, MK = "Michael Baker", "Mikey Baker"
    rows = [
        (J, ["Michael Baker, Jose Garcia", "Mike Baker, Jose Garcia", "Michael Baker, Jose Garcia",
             "Michael Baker, Jose Garcia", "Mike Baker, Jose Garcia"]),
        (S, ["Luis Ortiz"] * 3 + ["Luis Ortiz, Mikey Baker", "Luis Ortiz"]),
        (O, ["Dave Kim, Tom Reed"] * 5),
    ]
    assigns, _ = job_cells(rows, WEEK)
    exp = exact_info(expected(
        "E", "input.csv", layout="job_by_day", arrangement="single", assignments=assigns,
        flags=[DB(MB, "10-15", J, S)], info=[],
        must_not_ask=NO_ASK + [SP("Mike Baker", MB), SP(MK, MB), NO_PERSON_Q, NO_SITE_Q]))
    emit("E01", "mike-michael-mikey",
         "Mike / Michael / Mikey (job x day): 'Mike Baker' and 'Mikey Baker' = 'Michael Baker' by the nickname table and the same surname, merged without asking (shown as chips); merged, Michael Baker is double-booked Thursday at 1420 Johnson St and Smith remodel.",
         {"input.csv": csv_bytes(grid_lines(rows, "Job"))}, exp,
         {"window": WEEK, "jobs": [j for j, _ in rows], "same_person": [["Mike Baker", MB], [MK, MB]]})


def e02():
    MB = "Michael Baker"
    plan = [
        (MB, [J, J, J, "", J]),
        ("Mike B", ["", "", S, J, ""]),
        ("Jose Garcia", [S] * 5),
        ("Luis Ortiz", [O] * 5),
        ("Tom Brown", [P] * 5),
        ("Dave Kim", [J] * 5),
    ]
    lines, assigns = list_file(plan_rows(plan, WEEK))
    exp = exact_info(expected(
        "E", "input.csv", layout="long_list", arrangement="single", assignments=assigns,
        flags=[DB(MB, "10-14", J, S)], info=[],
        must_not_ask=NO_ASK + [SP("Mike B", MB), SP("Mike B", "Tom Brown"), NO_PERSON_Q, NO_SITE_Q]))
    emit("E02", "mike-b-vs-michael-baker",
         "Mike B vs Michael Baker (long list): 'Mike B' fits exactly one roster person by nickname (Mike/Michael) plus surname initial ('Tom Brown' shares the B, not the first name), so it merges without asking; merged, Michael Baker is booked Thursday and double-booked Wednesday at 1420 Johnson St and Smith remodel.",
         {"input.csv": csv_bytes(lines)}, exp, {"same_person": [["Mike B", MB]]})


def e03():
    MB = "Michael Baker"
    cols = [
        (J, ["Michael Baker, Maria Lopez"] * 2 + ["Maria Lopez"] + ["Michael Baker, Maria Lopez"] * 2),
        (S, ["Dan Baker"] * 2 + ["Dan Baker, M. Baker"] + ["Dan Baker"] * 2),
        (O, ["Jose Garcia"] * 3 + ["Jose Garcia, M. Baker", "Jose Garcia"]),
        (P, ["Luis Ortiz"] * 5),
    ]
    assigns, _ = job_cells(cols, WEEK)
    lines = day_lines(cols, WEEK)
    exp = exact_info(expected(
        "E", "input.xlsx", layout="day_by_job", arrangement="single", assignments=assigns,
        flags=[DB(MB, "10-15", J, O)], info=[],
        must_not_ask=NO_ASK + [Q("pick_sheet"), SP("M. Baker", MB), SP("M. Baker", "Dan Baker"),
                               SP(MB, "Dan Baker"), NO_PERSON_Q, NO_SITE_Q]))
    emit("E03", "m-baker-initial",
         "M. Baker (day x job, .xlsx): 'M. Baker' fits only Michael Baker (Dan Baker's first initial conflicts, Maria Lopez's surname differs), so it merges without asking: Michael is booked Wednesday at Smith remodel and double-booked Thursday at 1420 Johnson St and Oak Ridge Apts; Dan Baker is never merged or asked about.",
         {"input.xlsx": lambda p: write_lines_xlsx(p, lines, "col")}, exp,
         {"window": WEEK, "jobs": [j for j, _ in cols], "same_person": [["M. Baker", MB]]})


def e04():
    rows = [
        (J, ["Mike Baker, Dave Kim"] * 2 + ["Mike Baker, Kim"] + ["Mike Baker, Dave Kim"] * 2),
        (S, ["Jose Garcia, Tom Reed"] * 3 + ["Jose Garcia, Tom Reed, Ortiz"] * 2),
        (O, ["Luis Ortiz, Ana Ruiz", "Luis Ortiz, Ana", "Luis Ortiz, Ana Ruiz", "Ana Ruiz", "Luis Ortiz, Ana Ruiz"]),
    ]
    assigns, _ = job_cells(rows, WEEK)
    pairs = [["Kim", "Dave Kim"], ["Ortiz", "Luis Ortiz"]]
    q = SP("Ana", "Ana Ruiz")
    exp = exact_info(expected(
        "E", "input.tsv", layout="job_by_day", arrangement="single", assignments=assigns,
        flags=[DB("Luis Ortiz", "10-16", O, S), NB("Ana", "10-12"), NB("Ana Ruiz", "10-13"),
               NB("Ana", "10-14"), NB("Ana", "10-15"), NB("Ana", "10-16")], info=[],
        ask=[q], must_not_ask=NO_ASK + [SP(a, b) for a, b in pairs] + [NO_SITE_Q],
        answers=[ANS(q, "yes")], after_answers={"flags": [DB("Luis Ortiz", "10-16", O, S)], "info": []}))
    emit("E04", "last-name-only",
         "Single-name spellings (job x day, .tsv): last name only 'Kim' and 'Ortiz' each fit exactly one roster person and merge without asking (Dave Kim booked Wednesday; Luis Ortiz booked Thursday and double-booked Friday at Oak Ridge Apts and Smith remodel). First name only 'Ana' is asked (another Ana may exist) and kept separate meanwhile; answered yes, Ana Ruiz is booked Tuesday.",
         {"input.tsv": csv_bytes(grid_lines(rows, "Job"), delim="\t")}, exp,
         {"window": WEEK, "jobs": [j for j, _ in rows], "same_person": pairs})


def e05():
    MB, TY = "Michael Baker", "Micheal Baker"
    plan = [
        (MB, [J, J, J, "", ""]),
        (TY, ["", "", "", J, J]),
        ("Dave Kim", [J] * 5),
        ("Jose Garcia", [S] * 5),
        ("Luis Ortiz", [O] * 5),
    ]
    lines, assigns = list_file(plan_rows(plan, WEEK))
    q = SP(TY, MB)
    exp = exact_info(expected(
        "E", "input.csv", layout="long_list", arrangement="single", assignments=assigns,
        flags=[NB(MB, "10-15"), NB(MB, "10-16"), NB(TY, "10-12"), NB(TY, "10-13"), NB(TY, "10-14")], info=[],
        ask=[q], must_not_ask=NO_ASK + [NO_SITE_Q],
        answers=[ANS(q, "yes")], after_answers={"flags": [], "info": []}))
    emit("E05", "typo-micheal-ask",
         "Typo 'Micheal Baker' vs 'Michael Baker' (long list): a misspelling is neither a nickname nor an initial, so with the model off it is asked and the two stay separate meanwhile (each not booked on the other's days); answered yes, all five amber flags clear.",
         {"input.csv": csv_bytes(lines)}, exp)


def e06():
    rows = []
    week = [
        [("Mike Baker", J), ("Dave Kim", J), ("Jose Garcia", S), ("Tom O'Reilly", O)],
        [("MIKE BAKER", J), ("Dave Kim", J), ("JOSE GARCIA", S), ("Tom O'Reilly", O)],
        [("mike baker", J), ("Dave Kim", J), ("Jose Garcia", S), ("Tom OReilly", O), ("Mike   Baker", S)],
        [("Mike Baker", J), ("  Dave Kim ", J), ("jose garcia", S), ("Tom OReilly", O)],
        [("Mike Baker", J), ("Dave   Kim", J), ("Jose Garcia", S), ("Tom O'Reilly", O)],
    ]
    for x, entries in zip(WEEK, week):
        rows += [(x, p, s) for p, s in entries]
    lines, assigns = list_file(rows)
    exp = exact_info(expected(
        "E", "input.tsv", layout="long_list", arrangement="single", assignments=assigns,
        flags=[DB("Mike Baker", "10-14", J, S)], info=[],
        must_not_ask=NO_ASK + [SP("Tom OReilly", "Tom O'Reilly"), NO_PERSON_Q, NO_SITE_Q]))
    emit("E06", "case-and-spacing",
         "Case, spacing and punctuation (long list, .tsv): 'MIKE BAKER', 'mike baker' and 'Mike   Baker' are one person, as are padded '  Dave Kim ' / 'Dave   Kim' and 'Tom OReilly' / 'Tom O'Reilly'; merged without asking, Mike Baker is double-booked Wednesday (1420 Johnson St, Smith remodel) and nobody is falsely not booked.",
         {"input.tsv": csv_bytes(lines, delim="\t")}, exp,
         {"same_person": [["Tom O'Reilly", "Tom OReilly"]]})


def e07():
    cols = [
        (J, ["Mike Baker, Dave Kim"] * 5),
        (S, ["José García, Tom Reed"] * 3 + ["Jose Garcia, Tom Reed"] * 2),
        (O, ["Luis Ortiz, Ana Ruiz"] * 4 + ["Ana Ruiz"]),
    ]
    assigns, _ = job_cells(cols, WEEK)
    exp = exact_info(expected(
        "E", "input.csv", layout="day_by_job", arrangement="single", assignments=assigns,
        flags=[NB("Luis Ortiz", "10-16")], info=[],
        must_not_ask=NO_ASK + [SP("José García", "Jose Garcia"), NO_PERSON_Q, NO_SITE_Q]))
    emit("E07", "jose-vs-jose-accent",
         "Jose vs José (day x job): 'José García' (Mon-Wed) and 'Jose Garcia' (Thu-Fri) differ only by accents and merge without asking, so neither spelling is falsely not booked; the one real flag is Luis Ortiz not booked Friday.",
         {"input.csv": csv_bytes(day_lines(cols, WEEK))}, exp,
         {"window": WEEK, "jobs": [j for j, _ in cols], "same_person": [["José García", "Jose Garcia"]]})


def e08():
    LONG, SHORT = "José García López", "José García"
    rows = [
        (J, ["Mike Baker, Ana García"] * 5),
        (S, [LONG] * 5),
        (O, ["Luis Ortiz"] * 2 + [f"Luis Ortiz, {SHORT}"] + ["Luis Ortiz"] * 2),
    ]
    assigns, _ = job_cells(rows, WEEK)
    lines = grid_lines(rows, "Job")
    q = SP(LONG, SHORT)
    exp = exact_info(expected(
        "E", "input.xlsx", layout="job_by_day", arrangement="single", assignments=assigns,
        flags=[NB(SHORT, "10-12"), NB(SHORT, "10-13"), NB(SHORT, "10-15"), NB(SHORT, "10-16")], info=[],
        ask=[q], must_not_ask=NO_ASK + [Q("pick_sheet"), SP(SHORT, "Ana García"), SP(LONG, "Ana García"), NO_SITE_Q],
        answers=[ANS(q, "yes")], after_answers={"flags": [DB(LONG, "10-14", S, O)], "info": []}))
    emit("E08", "garcia-lopez-second-surname-ask",
         "Dropped second surname (job x day, .xlsx): 'José García' may be 'José García López' or someone else, so it is asked (model off) and kept separate meanwhile (not booked four days); answered yes, José García López is double-booked Wednesday (Smith remodel, Oak Ridge Apts). Ana García shares only the surname and is never asked about.",
         {"input.xlsx": lambda p: write_lines_xlsx(p, lines, "row")}, exp,
         {"window": WEEK, "jobs": [j for j, _ in rows]})


def e09():
    rows = [
        ("Mike B", [J] * 5),
        ("Mike R", [S] * 5),
        ("Jose Garcia", [S] * 5),
        ("Luis Ortiz", [O] * 5),
        ("Dave Kim", [J] * 4 + [""]),
    ]
    exp = exact_info(expected(
        "E", "input.csv", layout="person_by_day", arrangement="single", assignments=person_cells(rows, WEEK),
        flags=[NB("Dave Kim", "10-16")], info=[],
        must_not_ask=NO_ASK + [SP("Mike B", "Mike R"), NO_PERSON_Q, NO_SITE_Q]))
    emit("E09", "mike-b-vs-mike-r-never-merge",
         "Must NOT merge Mike B and Mike R (person x day): one first name, conflicting surname initials, so two people, never merged and never asked about (merged, they would be double-booked all week); the only flag is Dave Kim not booked Friday.",
         {"input.csv": csv_bytes(grid_lines(rows, "Name"))}, exp, {"window": WEEK})


def e10():
    rows = [
        (J, ["Bob Smith Jr, Dave Kim"] * 5),
        (P, ["Bob Smith Sr"] * 3 + ["Bob Smith Sr, Robert Smith Jr", "Bob Smith Sr"]),
        (O, ["Luis Ortiz, Tom Reed"] * 5),
    ]
    assigns, _ = job_cells(rows, WEEK)
    exp = exact_info(expected(
        "E", "input.csv", layout="job_by_day", arrangement="single", assignments=assigns,
        flags=[DB("Robert Smith Jr", "10-15", J, P)], info=[],
        must_not_ask=NO_ASK + [SP("Bob Smith Sr", "Bob Smith Jr"), SP("Robert Smith Jr", "Bob Smith Sr"),
                               SP("Robert Smith Jr", "Bob Smith Jr"), NO_PERSON_Q, NO_SITE_Q]))
    emit("E10", "bob-smith-sr-vs-jr",
         "Bob Smith Sr vs Bob Smith Jr (job x day): Sr and Jr are two people, never merged or asked about (merged, they would be double-booked all week); 'Robert Smith Jr' is Bob Smith Jr by the nickname table and the same suffix (never Sr), merged without asking, so Robert Smith Jr is double-booked Thursday (1420 Johnson St, Parker kitchen).",
         {"input.csv": csv_bytes(grid_lines(rows, "Job"))}, exp,
         {"window": WEEK, "jobs": [j for j, _ in rows], "same_person": [["Robert Smith Jr", "Bob Smith Jr"]]})


def e11():
    plan = [
        ("Chris Lee", [J] * 5),
        ("Chris Ortiz", [S] * 5),
        ("Mike Baker", [J] * 5),
        ("Ana Ruiz", [O] * 5),
        ("Chris", ["", "", "", "", O]),
    ]
    lines, assigns = list_file(plan_rows(plan, WEEK))
    q_lee, q_ortiz = SP("Chris", "Chris Lee"), SP("Chris", "Chris Ortiz")
    exp = exact_info(expected(
        "E", "input.csv", layout="long_list", arrangement="single", assignments=assigns,
        flags=[NB("Chris", x) for x in WEEK[:4]], info=[],
        ask=[q_lee, q_ortiz], must_not_ask=NO_ASK + [SP("Chris Lee", "Chris Ortiz"), NO_SITE_Q],
        answers=[ANS(q_lee, "no"), ANS(q_ortiz, "yes")],
        after_answers={"flags": [DB("Chris Ortiz", "10-16", S, O)], "info": []}))
    emit("E11", "two-different-chrises",
         "Two different Chrises (long list): Chris Lee and Chris Ortiz share only a first name and are never merged or asked about; a bare 'Chris' fits both, so each pair is asked (model off) and 'Chris' stays separate meanwhile (not booked Mon-Thu); answered no (Lee) and yes (Ortiz), Chris Ortiz is double-booked Friday (Smith remodel, Oak Ridge Apts).",
         {"input.csv": csv_bytes(lines)}, exp)


def e12():
    rows = [
        ("Mike Baker", [J] * 5),
        ("Helper 1", [J, f"{J} / {S}", J, J, J]),
        ("Helper 2", [S, "", S, S, S]),
        ("Jose Garcia", [S] * 5),
        ("Tom Foreman", [O] * 4 + [""]),
        ("Luis Ortiz", [O] * 5),
        ("Foreman", [P] * 5),
        ("New guy", ["", "", P, "", ""]),
    ]
    exp = exact_info(expected(
        "E", "input.csv", layout="person_by_day", arrangement="single", assignments=person_cells(rows, WEEK),
        flags=[NB("Tom Foreman", "10-16")], info=[],
        must_not_ask=NO_ASK + [SP("Helper 1", "Helper 2"), SP("Foreman", "Tom Foreman"), NO_PERSON_Q, NO_SITE_Q]))
    emit("E12", "placeholders-helper-foreman",
         "Placeholders (person x day): 'Helper 1', 'Helper 2', 'Foreman' and 'New guy' are never not booked, never double-booked (Helper 1 at two jobs Tuesday) and never merged with each other or with employee Tom Foreman; Parker kitchen, staffed only by placeholders, is not unstaffed. The one flag is Tom Foreman not booked Friday.",
         {"input.csv": csv_bytes(grid_lines(rows, "Name"))}, exp, {"window": WEEK})


def e13():
    WP, ABC = "Ward Plumbing", "ABC Electric LLC"
    plan = [
        ("Bill Ward", [J] * 5),
        ("Mike Baker", [S] * 5),
        ("Jose Garcia", [P] * 4 + [S]),
        (WP, ["", J, [S, P], J, ""]),
        (ABC, [[S, P], "", "", "", P]),
    ]
    lines, assigns = list_file(plan_rows(plan, WEEK))
    exp = exact_info(expected(
        "E", "input.csv", layout="long_list", arrangement="single", assignments=assigns,
        flags=[], info=[SUBMULTI(ABC, "10-12", S, P), SUBMULTI(WP, "10-14", S, P)],
        must_not_ask=NO_ASK + [SP("Bill Ward", WP), NO_PERSON_Q, NO_SITE_Q]))
    emit("E13", "subcontractors-never-merged",
         "Subcontractors (long list): 'ABC Electric LLC' and 'Ward Plumbing' at two jobs on one day are info only, never not booked, and ABC alone staffs Parker kitchen Friday; 'Ward Plumbing' shares a word with employee Bill Ward but is never merged with him or asked about (merged, Bill would be double-booked Wednesday).",
         {"input.csv": csv_bytes(lines)}, exp)


def e14():
    plan = [
        ("Francisco Reyes", [J, J, J, "", ""]),
        ("Paco Reyes", ["", "", S, J, J]),
        ("Jesús Mendoza", [O, O, "", O, O]),
        ("Chuy Mendoza", ["", "", O, "", ""]),
        ("Ignacio Torres", [S] * 5),
        ("Nacho Torres", ["", "", "", "", S]),
        ("José García", [P] * 4 + [""]),
        ("Pepe García", [""] * 4 + [P]),
    ]
    lines, assigns = list_file(plan_rows(plan, WEEK))
    pairs = [["Paco Reyes", "Francisco Reyes"], ["Chuy Mendoza", "Jesús Mendoza"],
             ["Nacho Torres", "Ignacio Torres"], ["Pepe García", "José García"]]
    exp = exact_info(expected(
        "E", "input.xlsx", layout="long_list", arrangement="single", assignments=assigns,
        flags=[DB("Francisco Reyes", "10-14", J, S)], info=[DUP("Ignacio Torres", "10-16", S)],
        must_not_ask=NO_ASK + [Q("pick_sheet")] + [SP(a, b) for a, b in pairs] + [NO_PERSON_Q, NO_SITE_Q]))
    emit("E14", "spanish-nicknames",
         "Spanish nicknames (long list, .xlsx): Paco = Francisco, Chuy = Jesús, Nacho = Ignacio and Pepe = José, each with the same surname, merge without asking: Francisco Reyes is double-booked Wednesday (1420 Johnson St, Smith remodel), Ignacio Torres's second Friday row is a duplicate entry, and nobody is falsely not booked.",
         {"input.xlsx": lambda p: write_lines_xlsx(p, lines, "col")}, exp, {"same_person": pairs})


if __name__ == "__main__":
    for f in (e01, e02, e03, e04, e05, e06, e07, e08, e09, e10, e11, e12, e13, e14):
        f()
    print("E done")
