"""Category A: encoding and delimiters. Run: python3 build_A.py"""
from fxlib import *

WEEK = days("10-12", 5)
J, S, O = "1420 Johnson St", "Smith remodel", "Oak Ridge Apts"
NO_LAYOUT_QS = [Q("pick_layout"), Q("column_role")]


def a01():
    rows = [
        ("Mike Baker", [J, J, J, S, S]),
        ("Jose Garcia", [S] * 5),
        ("Luis Ortiz", [O] * 5),
        ("Dave Kim", [J, J, f"{J} / {O}", O, O]),
    ]
    data = csv_bytes([["Date", "Employee", "Job"]] + grid_to_list_rows(rows, WEEK, mdyyyy), eol="\n")
    exp = expected("A", "input.csv", layout="long_list", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[DB("Dave Kim", "10-14", J, O)], info=[],
                   must_not_ask=NO_LAYOUT_QS)
    emit("A01", "comma-delimited-lf", "Comma-delimited CSV, UTF-8 without BOM, LF line endings: the baseline delimiter.",
         {"input.csv": data}, exp)


def a02():
    rows = [
        ("Ana Ruiz", [O] * 5),
        ("Carlos Mendez", [O, O, "Parker kitchen", "Parker kitchen", "Parker kitchen"]),
        ("Tom Reed", ["Parker kitchen"] * 3 + ["PTO", "Parker kitchen"]),
        ("Ray Jones", [J] * 5),
    ]
    hrs = {"Ana Ruiz": "8,0", "Carlos Mendez": "8,0", "Tom Reed": "8,0", "Ray Jones": "9,5"}
    body = []
    for date_text, person, site in grid_to_list_rows(rows, WEEK, lambda x: x):
        body.append([date_text, person, site, "" if site == "PTO" else hrs[person]])
    data = csv_bytes([["Date", "Name", "Site", "Hrs"]] + body, delim=";", eol="\r\n")
    exp = expected("A", "input.csv", layout="long_list", arrangement="single",
                   assignments=person_grid(rows, WEEK), flags=[], info=[],
                   must_not_ask=NO_LAYOUT_QS)
    emit("A02", "semicolon-delimited-crlf",
         "Semicolon-delimited CSV (European Excel export) with CRLF endings and decimal-comma hours, so commas appear but are not the delimiter.",
         {"input.csv": data}, exp)


def a03():
    rows = [
        ("Mike Baker", [J] * 5),
        ("Jose Garcia", [S] * 5),
        ("Luis Ortiz", [O, O, O, f"{O} / {S}", O]),
        ("Dave Kim", ["Shop", J, J, "off", J]),
    ]
    data = csv_bytes([["Name"] + [mdyyyy(x) for x in WEEK]] + [[p] + c for p, c in rows], delim="\t", eol="\n")
    exp = expected("A", "input.tsv", layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[DB("Luis Ortiz", "10-15", O, S)], info=[],
                   must_not_ask=NO_LAYOUT_QS)
    emit("A03", "tab-delimited-tsv", "Tab-delimited .tsv (Google Sheets download), UTF-8, LF line endings.",
         {"input.tsv": data}, exp, {"window": WEEK})


def a04():
    R, E = "Riverside duplex", "88 Elm Ave"
    rows = [
        ("Sam Patel", [R, R, "", R, R]),
        ("Kevin Walsh", [R] * 5),
        ("Miguel Torres", [E] * 5),
        ("Eddie Flores", [E, E, E, "Shop", "Shop"]),
    ]
    data = csv_bytes([["Date", "Name", "Job"]] + grid_to_list_rows(rows, WEEK, mdyyyy), delim="|", eol="\n")
    exp = expected("A", "input.txt", layout="long_list", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[NB("Sam Patel", "10-14")], info=[],
                   must_not_ask=NO_LAYOUT_QS)
    emit("A04", "pipe-delimited", "Pipe-delimited (|) text export saved as .txt.", {"input.txt": data}, exp)


def a05():
    OC, LOT = "Oak Ridge Apts, Bldg C", 'Lot 7 "the barn"'
    rows = [
        (OC, ["Mike Baker, Jose Garcia", "Mike Baker, Jose Garcia", "Mike Baker\nJose Garcia", "Jose Garcia", "Jose Garcia, Ana Ruiz"]),
        (J, ["Luis Ortiz\nDave Kim", "Luis Ortiz\nDave Kim", "Luis Ortiz", "Luis Ortiz, Mike Baker", "Mike Baker"]),
        (LOT, ["Ana Ruiz", "Ana Ruiz", "Ana Ruiz, Dave Kim", "Ana Ruiz\nDave Kim", "Dave Kim\nLuis Ortiz"]),
    ]
    # Excel writes rows with CRLF and Alt+Enter breaks inside quoted cells as bare LF.
    data = csv_bytes([["Job"] + [mdyyyy(x) for x in WEEK]] + [[j] + c for j, c in rows], eol="\r\n")
    exp = expected("A", "input.csv", layout="job_by_day", arrangement="single",
                   assignments=job_grid(rows, WEEK), flags=[], info=[],
                   must_not_ask=NO_LAYOUT_QS)
    emit("A05", "quoted-fields-commas-newlines",
         "Quoted CSV fields holding commas, doubled quotes and line breaks (Alt+Enter) inside cells; job labels with commas stay whole.",
         {"input.csv": data}, exp, {"window": WEEK, "jobs": [OC, J, LOT]})


def a06():
    rows = [
        ("José Hernández", [S] * 5),
        ("Ramón Díaz", [J, J, "Shop", J, J]),
        ("Mike Baker", [J, J, J, J, "PTO"]),
        ("Luis Ortiz", [O] * 5),
    ]
    data = csv_bytes([["Employee"] + [mdyyyy(x) for x in WEEK]] + [[p] + c for p, c in rows], eol="\r\n", bom=True)
    exp = expected("A", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, WEEK), flags=[], info=[],
                   must_not_ask=NO_LAYOUT_QS)
    emit("A06", "utf8-bom", "UTF-8 CSV with a byte-order mark (Excel 'CSV UTF-8'); the BOM must not stick to the first header cell.",
         {"input.csv": data}, exp, {"window": WEEK})


def a07():
    rows = [
        ("Jesús Peña", [O, O, O, f"{O} / {S}", O]),
        ("Mike Baker", [S] * 5),
        ("Dave Kim", [J] * 5),
        ("Ana Ruiz", [J, J, J, "sick", J]),
    ]
    data = csv_bytes([["Name"] + [mdyyyy(x) for x in WEEK]] + [[p] + c for p, c in rows],
                     delim="\t", eol="\r\n", encoding="utf-16-le", bom=True)
    exp = expected("A", "input.txt", layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[DB("Jesús Peña", "10-15", O, S)], info=[],
                   must_not_ask=NO_LAYOUT_QS)
    emit("A07", "utf16-le-unicode-text", "UTF-16 LE with BOM, tab-delimited, CRLF .txt (Excel 'Unicode Text' save).",
         {"input.txt": data}, exp, {"window": WEEK})


def a08():
    OB, LOT = "O’Brien deck", "Lot 7 “the barn”"
    rows = [
        ("José Garcia", [OB] * 5),
        ("Raúl Peña", [LOT] * 5),
        ("Mike Baker", [J, J, f"{J} / vacation", J, J]),
        ("Luis Ortiz", [J] * 5),
    ]
    data = csv_bytes([["Date", "Name", "Job"]] + grid_to_list_rows(rows, WEEK, mdyyyy), eol="\r\n", encoding="cp1252")
    exp = expected("A", "input.csv", layout="long_list", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[AWO("Mike Baker", "10-14", [J], "vacation")], info=[],
                   must_not_ask=NO_LAYOUT_QS)
    emit("A08", "windows-1252-jose-smart-quotes",
         "Windows-1252 CSV (Excel 'CSV (Comma delimited)' on Windows) with José, Raúl, Peña and smart quotes; must not be read as UTF-8.",
         {"input.csv": data}, exp)


def a09():
    rows = [
        ("Mike Baker", [J] * 5),
        ("Tom Reed", [S] * 5),
        ("Ana Ruiz", [S, S, "", S, S]),
        ("Ray Jones", [O] * 5),
    ]
    data = csv_bytes([["Name"] + [mdyyyy(x) for x in WEEK]] + [[p] + c for p, c in rows], eol="\r")
    exp = expected("A", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[NB("Ana Ruiz", "10-14")], info=[],
                   must_not_ask=NO_LAYOUT_QS + [Q("fill_down")])
    emit("A09", "cr-only-line-endings", "CR-only line endings (old Mac Excel 'CSV (Macintosh)'); the file has no LF at all.",
         {"input.csv": data}, exp, {"window": WEEK})


def a10():
    rows = [
        ("Mike Baker", [J] * 5),
        ("Jose Garcia", [S] * 5),
        ("Luis Ortiz", [O, O, O, O, ""]),
        ("Dave Kim", [J, f"{J} / {S}", J, J, J]),
    ]
    lines = [
        ["Name"] + [mdyyyy(x) for x in WEEK] + ["", "", ""],
        ["Mike Baker"] + [J] * 5 + ["", "", ""],
        ["Jose Garcia"] + [S] * 5 + ["", ""],
        ["Luis Ortiz"] + [O] * 4,                      # ragged: Friday cell missing entirely
        ["Dave Kim", J, f"{J} / {S}", J, J, J, ""],
        [""] * 9,                                       # a row of bare delimiters
    ]
    data = csv_bytes(lines, eol="\n")
    exp = expected("A", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, WEEK),
                   flags=[DB("Dave Kim", "10-13", J, S), NB("Luis Ortiz", "10-16")], info=[],
                   must_not_ask=NO_LAYOUT_QS + [Q("fill_down")])
    emit("A10", "ragged-rows-trailing-delimiters",
         "Ragged rows (a short row missing its last cells) plus trailing delimiters and an all-comma row.",
         {"input.csv": data}, exp, {"window": WEEK})


if __name__ == "__main__":
    for f in (a01, a02, a03, a04, a05, a06, a07, a08, a09, a10):
        f()
    print("A done")
