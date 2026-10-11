"""Category F: jobs and sites (spec stage 7 for sites, internal locations, travel rows).
Run: uv run --with openpyxl==3.1.5 python3 build_F.py

Site readings encoded here (the pipeline's stage 7 rules, model off):
  - SAME, merged without asking: spellings equal after normalizing case,
    punctuation, street suffixes (St/Street, Ave/Avenue) and directions
    (N/North), with the same house number; a job-number prefix on an otherwise
    identical name ("J-1042 Smith remodel" = "Smith remodel").
  - UNSURE, asked and kept separate until answered: look-alike addresses with
    different house numbers (1420 vs 1240 Johnson St); a street with no house
    number vs a full address (Johnson St vs 1420 Johnson St); a customer-style
    name vs an address sharing its distinctive word ("Johnson job",
    "Henderson" vs "4410 Henderson Rd").
  - Never matched: internal locations (shop, yard, office, warehouse) and travel
    rows ("drive to Fresno") with jobs, even when they share a word.
Questions name each side by its canonical spelling (the longest in its group
after the deterministic merges). A site merge can only remove a double-booking
(it never adds a site), so the "yes" answers here clear flags.
"""
from fxlib import *

WEEK = days("10-12", 5)
J, S, O, P = "1420 Johnson St", "Smith remodel", "Oak Ridge Apts", "Parker kitchen"
NO_ASK = [Q("pick_layout"), Q("column_role"), Q("cell_meaning")]
NO_PERSON_Q, NO_SITE_Q = Q("same_person"), Q("same_site")


def SS(a, b):
    return Q("same_site", a=a, b=b)


def exact_info(exp):
    exp = dict(exp)
    exp["info_exact"] = True
    return {k: exp[k] for k in KEY_ORDER if k in exp}


def grid_lines(rows, first):
    return [[first] + [mdyyyy(x) for x in WEEK]] + [[label] + list(cells) for label, cells in rows]


def f01():
    JS, JJ = "Johnson St", "Johnson job"
    plan = [
        ("Mike Baker", [J, J, J, [J, JJ], J]),
        ("Dave Kim", [J, J, [J, JS], J, J]),
        ("Jose Garcia", [S] * 5),
        ("Tom Reed", [S, S, S, JJ, JJ]),
        ("Luis Ortiz", [O] * 5),
    ]
    lines, assigns = list_file(plan_rows(plan, WEEK))
    q_street, q_job, q_both = SS(JS, J), SS(JJ, J), SS(JJ, JS)
    exp = exact_info(expected(
        "F", "input.csv", layout="long_list", arrangement="single", assignments=assigns,
        flags=[DB("Dave Kim", "10-14", J, JS), DB("Mike Baker", "10-15", J, JJ)], info=[],
        ask=[q_street, q_job, q_both], must_not_ask=NO_ASK + [SS(JJ, S), NO_PERSON_Q],
        answers=[ANS(q_street, "yes"), ANS(q_job, "no")],
        after_answers={"flags": [DB("Mike Baker", "10-15", J, JJ)], "info": [DUP("Dave Kim", "10-14", J)]}))
    emit("F01", "johnson-st-vs-johnson-job-asks",
         "1420 Johnson St vs 'Johnson St' vs 'Johnson job' (long list): a street with no house number and a customer-style name sharing 'Johnson' are each asked against the address and each other, never merged silently; kept separate, Dave Kim (Wed) and Mike Baker (Thu) look double-booked; answered 'Johnson St' = 1420 yes and 'Johnson job' = 1420 no, Dave's becomes a duplicate entry and Mike's stays.",
         {"input.csv": csv_bytes(lines)}, exp)


def f02():
    JS, JU = "1420 Johnson Street", "1420 JOHNSON ST."
    rows = [
        ("Mike Baker", [J, J, S, S, J]),
        ("Dave Kim", [S, S, JS, JS, S]),
        ("Jose Garcia", [S] * 5),
        ("Luis Ortiz", [O, O, JU, O, O]),
        ("Ana Ruiz", [O, O, S, O, O]),
    ]
    lines = grid_lines(rows, "Name")
    exp = exact_info(expected(
        "F", "input.xlsx", layout="person_by_day", arrangement="single", assignments=person_cells(rows, WEEK),
        flags=[UF(O, "10-14")], info=[],
        must_not_ask=NO_ASK + [Q("pick_sheet"), SS(J, JS), SS(J, JU), NO_SITE_Q, NO_PERSON_Q]))
    emit("F02", "st-vs-street-silent-merge",
         "St vs Street (person x day, .xlsx): '1420 Johnson St', '1420 Johnson Street' and '1420 JOHNSON ST.' share the house number and differ only in suffix spelling, case and punctuation, so they merge without asking (unmerged, 1420 Johnson St would look unstaffed Wednesday and Thursday); the one real flag is Oak Ridge Apts unstaffed Wednesday.",
         {"input.xlsx": lambda p: write_lines_xlsx(p, lines, "row")}, exp,
         {"window": WEEK, "same_site": [[J, JS], [J, JU]]})


def f03():
    number = {S: "J-1042", P: "J-1043", O: "J-1051"}
    plan = [
        ("Mike Baker", [S] * 5),
        ("Jose Garcia", [S] * 5),
        ("Ana Ruiz", [P, [P, S], P, P, P]),
        ("Luis Ortiz", [O] * 5),
        ("Dave Kim", [O] * 4 + [P]),
    ]
    rows = plan_rows(plan, WEEK)
    lines = [["Date", "Employee", "Job #", "Job Name"]] + [[mdyyyy(x), p, number[s], s] for x, p, s in rows]
    assigns = [W(p, x, s) for x, p, s in rows]
    exp = exact_info(expected(
        "F", "input.csv", layout="long_list", arrangement="single", assignments=assigns,
        flags=[DB("Ana Ruiz", "10-13", P, S)], info=[],
        must_not_ask=NO_ASK + [SS("J-1042", S), NO_SITE_Q, NO_PERSON_Q]))
    emit("F03", "job-number-column-list",
         "Job numbers beside names (long list): a 'Job #' column (J-1042, J-1043, J-1051) sits next to a 'Job Name' column; the site is the job name and the number never becomes a site, a note or a question. Ana Ruiz is double-booked Tuesday (Parker kitchen, Smith remodel).",
         {"input.csv": csv_bytes(lines)}, exp)


def f04():
    JN = "J-1042 Smith remodel"
    rows = [
        (JN, ["Jose Garcia, Tom Reed"] * 3 + ["", "Jose Garcia, Tom Reed"]),
        (J, ["Mike Baker, Dave Kim"] * 4 + ["Mike Baker, Dave Kim, Luis Ortiz, Ana Ruiz"]),
        (O, ["Luis Ortiz, Ana Ruiz"] * 4 + [""]),
        (S, ["", "", "", "Jose Garcia, Tom Reed", ""]),
    ]
    assigns, _ = job_cells(rows, WEEK)
    exp = exact_info(expected(
        "F", "input.csv", layout="job_by_day", arrangement="single", assignments=assigns,
        flags=[UF(O, "10-16")], info=[],
        must_not_ask=NO_ASK + [SS(JN, S), NO_SITE_Q, NO_PERSON_Q]))
    emit("F04", "job-number-prefix-grid-merge",
         "Job-number prefix in a grid (job x day): 'J-1042 Smith remodel' and a later row 'Smith remodel' are one job (the same name once the job number is set aside) and merge without asking, so Smith remodel is staffed all week (unmerged, each row would look unstaffed on the other's days); the one real flag is Oak Ridge Apts unstaffed Friday.",
         {"input.csv": csv_bytes(grid_lines(rows, "Job"))}, exp,
         {"window": WEEK, "jobs": [j for j, _ in rows], "same_site": [[JN, S]]})


def f05():
    J2 = "1240 Johnson St"
    rows = [
        ("Mike Baker", [J, J, f"{J} / {J2}", J, J]),
        ("Dave Kim", [J2] * 5),
        ("Luis Ortiz", [J] * 5),
        ("Jose Garcia", [S] * 5),
    ]
    q = SS(J, J2)
    flags = [DB("Mike Baker", "10-14", J, J2)]
    exp = exact_info(expected(
        "F", "input.csv", layout="person_by_day", arrangement="single", assignments=person_cells(rows, WEEK),
        flags=flags, info=[], ask=[q], must_not_ask=NO_ASK + [NO_PERSON_Q],
        answers=[ANS(q, "no")], after_answers={"flags": flags, "info": []}))
    emit("F05", "1420-vs-1240-johnson-never-merge",
         "Must NOT merge 1420 vs 1240 Johnson St (person x day): look-alike addresses with different house numbers are asked, never merged; kept separate, Mike Baker is double-booked Wednesday, and answered no, the flag stays.",
         {"input.csv": csv_bytes(grid_lines(rows, "Name"))}, exp, {"window": WEEK})


def f06():
    H, HC = "4410 Henderson Rd", "Henderson"
    plan = [
        ("Mike Baker", [H, H, H, S, H]),
        ("Ana Ruiz", [S, S, H, HC, S]),
        ("Jose Garcia", [S] * 5),
        ("Luis Ortiz", [O] * 5),
    ]
    rows = plan_rows(plan, WEEK) + [(d("10-14"), "Ana Ruiz", HC)]  # a row added at the bottom later
    lines, assigns = list_file(rows)
    q = SS(HC, H)
    exp = exact_info(expected(
        "F", "input.csv", layout="long_list", arrangement="single", assignments=assigns,
        flags=[DB("Ana Ruiz", "10-14", H, HC), UF(H, "10-15")], info=[],
        ask=[q], must_not_ask=NO_ASK + [NO_PERSON_Q],
        answers=[ANS(q, "yes")], after_answers={"flags": [], "info": [DUP("Ana Ruiz", "10-14", H)]}))
    emit("F06", "customer-name-vs-address-ask",
         "Customer name vs address for one job (long list): 'Henderson' and '4410 Henderson Rd' may be one job, so it is asked; kept separate, Ana Ruiz looks double-booked Wednesday and 4410 Henderson Rd unstaffed Thursday; answered yes, both flags clear and Ana's Wednesday becomes a duplicate entry.",
         {"input.csv": csv_bytes(lines)}, exp)


def f07():
    rows = [
        (J, ["Mike Baker, Dave Kim"] * 4 + ["Mike Baker, Dave Kim, Ana Ruiz"]),
        (S, ["Jose Garcia, Tom Reed"] * 3 + ["Jose Garcia, Tom Reed, Dave Kim", "Jose Garcia, Tom Reed"]),
        ("Shop", ["Luis Ortiz", "Luis Ortiz, Mike Baker", "Luis Ortiz", "Luis Ortiz, Dave Kim", "Luis Ortiz"]),
        ("Yard", ["", "", "Luis Ortiz", "", ""]),
        ("Office", ["Ana Ruiz"] * 4 + [""]),
    ]
    assigns, _ = job_cells(rows, WEEK)
    lines = grid_lines(rows, "Job")
    exp = exact_info(expected(
        "F", "input.xlsx", layout="job_by_day", arrangement="single", assignments=assigns,
        flags=[DB("Dave Kim", "10-15", J, S)],
        info=[INTERNAL("Mike Baker", "10-13", "Shop", J), INTERNAL("Luis Ortiz", "10-14", "Shop", "Yard")],
        must_not_ask=NO_ASK + [Q("pick_sheet"), NO_SITE_Q, NO_PERSON_Q]))
    emit("F07", "shop-yard-office-internal",
         "Shop, yard, office (job x day, .xlsx): internal rows are never unstaffed (Yard and Office have blank days); Shop + one job (Mike, Tue) and Shop + Yard (Luis, Wed) are internal_plus_site info; Shop + two jobs (Dave, Thu) is double-booked on the two jobs only; Luis at the shop all week is booked.",
         {"input.xlsx": lambda p: write_lines_xlsx(p, lines, "row")}, exp,
         {"window": WEEK, "jobs": [j for j, _ in rows]})


def f08():
    DR, FS = "drive to Fresno", "Fresno school"
    rows = [
        ("Mike Baker", [S, S, S, f"{DR} / {FS}", FS]),
        ("Dave Kim", [S, S, S, f"{DR} / {FS}", FS]),
        ("Tom Reed", [S, S, S, f"{DR} / {FS} / {S}", S]),
        ("Jose Garcia", [S] * 5),
        ("Luis Ortiz", [O] * 4 + ["Travel"]),
    ]
    exp = exact_info(expected(
        "F", "input.csv", layout="person_by_day", arrangement="single", assignments=person_cells(rows, WEEK),
        flags=[DB("Tom Reed", "10-15", FS, S)],
        info=[INTERNAL("Mike Baker", "10-15", DR, FS), INTERNAL("Dave Kim", "10-15", DR, FS)],
        must_not_ask=NO_ASK + [SS(DR, FS), NO_SITE_Q, NO_PERSON_Q]))
    emit("F08", "travel-drive-to-fresno",
         "Travel rows (person x day): 'drive to Fresno' and 'Travel' are internal, never a job, never unstaffed and never matched with the 'Fresno school' job; the drive plus the Fresno job on one day is internal_plus_site info (Mike, Dave), and Tom Reed, also at Smith remodel Thursday, is double-booked on the two jobs only.",
         {"input.csv": csv_bytes(grid_lines(rows, "Name"))}, exp, {"window": WEEK})


def f09():
    N1, N2, N3 = "350 N Main St", "350 North Main Street", "350 N. Main St."
    A1, A2 = "88 Oak Ridge Ave", "88 Oak Ridge Avenue"
    people = [
        ("Mike Baker", [N1] * 5),
        ("Dave Kim", [N2, N2, A2, N2, N2]),
        ("Luis Ortiz", [A1, "smith remodel", "smith remodel", "smith remodel", A1]),
        ("Ana Ruiz", [S, A2, S, A2, S]),
        ("Jose Garcia", ["Smith Remodel"] * 5),
        ("Tom Reed", [N3] * 4 + [""]),
    ]
    pairs = [[N1, N2], [N1, N3], [A1, A2]]
    exp = exact_info(expected(
        "F", "input.csv", layout="day_by_person", arrangement="single", assignments=person_cells(people, WEEK),
        flags=[NB("Tom Reed", "10-16")], info=[],
        must_not_ask=NO_ASK + [SS(a, b) for a, b in pairs] + [NO_SITE_Q, NO_PERSON_Q]))
    emit("F09", "direction-and-suffix-spellings",
         "Direction and suffix spellings (day x person): '350 N Main St' = '350 North Main Street' = '350 N. Main St.' and '88 Oak Ridge Ave' = '88 Oak Ridge Avenue' (same house numbers), plus 'smith remodel' / 'Smith Remodel', merge without asking (unmerged, 88 Oak Ridge Ave would look unstaffed Tue-Thu); the one real flag is Tom Reed not booked Friday.",
         {"input.csv": csv_bytes(day_lines(people, WEEK))}, exp,
         {"window": WEEK, "same_site": pairs})


def f10():
    JS, J2, JJ = "1420 Johnson Street", "1240 Johnson St", "Johnson job"
    plan = [
        ("Mike Baker", [J, J, J, [J, J2], J]),
        ("Dave Kim", [JS, JS, [JS, JJ], JS, JS]),
        ("Tom Reed", [J2, J2, JJ, JJ, J2]),
        ("Jose Garcia", [S] * 5),
        ("Luis Ortiz", [S] * 5),
    ]
    lines, assigns = list_file(plan_rows(plan, WEEK))
    q_num, q_job_1420, q_job_1240 = SS(J2, JS), SS(JJ, JS), SS(JJ, J2)
    exp = exact_info(expected(
        "F", "input.tsv", layout="long_list", arrangement="single", assignments=assigns,
        flags=[DB("Mike Baker", "10-15", JS, J2), DB("Dave Kim", "10-14", JS, JJ), UF(J2, "10-14")], info=[],
        ask=[q_num, q_job_1420, q_job_1240], must_not_ask=NO_ASK + [SS(J, JS), NO_PERSON_Q],
        answers=[ANS(q_num, "no"), ANS(q_job_1240, "yes")],
        after_answers={"flags": [DB("Mike Baker", "10-15", JS, J2), DB("Dave Kim", "10-14", JS, J2)], "info": []}))
    emit("F10", "johnson-addresses-mixed",
         "Look-alike Johnson addresses together (long list, .tsv): '1420 Johnson St' = '1420 Johnson Street' merges silently, while '1240 Johnson St' and 'Johnson job' are asked against it and each other; answered 1240 vs 1420 no and 'Johnson job' = 1240 yes, Mike's and Dave's double-bookings stay (Dave's now names 1240 Johnson St) and 1240's unstaffed Wednesday clears.",
         {"input.tsv": csv_bytes(lines, delim="\t")}, exp, {"same_site": [[J, JS]]})


if __name__ == "__main__":
    for f in (f01, f02, f03, f04, f05, f06, f07, f08, f09, f10):
        f()
    print("F done")
