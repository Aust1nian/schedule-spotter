"""H07: HVAC install schedule, Excel 'Text (Tab delimited)' .txt, two weeks stacked."""
from common import *

FID, SLUG = "H07", "hvac-two-weeks-stacked-tab-txt"
README = ("HVAC installer's Excel 'Text (Tab delimited)' export (.txt, Windows-1252, CRLF): two weeks "
          "stacked, each a 'Week of' line over weekday-only headers (Mon..Fri), header repeated mid-file, "
          "Warehouse as an internal site, PTO/sick/vac, two sites in one cell, a Temp placeholder row, "
          "blank days and a footer note.")


def build():
    w1 = days("2026-10-19", 5)
    w2 = days("2026-10-26", 5)
    E, K, A, B, WH = "Elmwood Apts bldg 2", "Kroll residence", "St. Agnes rectory", "Bayview Plaza unit 4", "Warehouse"
    week1 = [
        ("Dennis Kuhl", [E] * 5),
        ("Omar Haddad", [E, E, K, K, E]),
        ("Ricky Banks", [A, A, WH, A, "PTO"]),
        ("Stan Wolfe", [A, A, WH, A, A]),
        ("Trinh Nguyen", [K, K, K, K, ""]),
        ("Jeff Ames", [B, B, B, "sick", B]),
        ("Carlos Mejia", [B] * 5),
        ("Brad Sowers", [WH, E, E, E, E]),
        ("Temp", ["", "", "", B, ""]),
    ]
    week2 = [
        ("Dennis Kuhl", [E] * 5),
        ("Omar Haddad", [E, f"{K} / {B}", K, K, K]),
        ("Ricky Banks", [A] * 5),
        ("Stan Wolfe", [A, A, "vac", "vac", "vac"]),
        ("Trinh Nguyen", [K] * 5),
        ("Jeff Ames", [B] * 5),
        ("Carlos Mejia", [B, B, B, "", WH]),
        ("Brad Sowers", [E, E, A, E, E]),
        ("Temp", [""] * 5),
    ]
    recs = []
    for dates, rows in ((w1, week1), (w2, week2)):
        for name, cells in rows:
            for d, c in zip(dates, cells):
                recs += person_cell(name, d, c)

    n = 6
    sheet = [["NORTHGATE MECHANICAL"], ["INSTALL SCHEDULE"], [""]]
    for label, rows in (("Week of 10/19/2026", week1), ("Week of 10/26/2026", week2)):
        sheet.append([label])
        sheet.append(["Tech", "Mon", "Tue", "Wed", "Thu", "Fri"])
        for name, cells in rows:
            sheet.append([name] + cells)
        sheet.append([""])
    sheet.append(["NOTE: Elmwood bldg 2 startup Fri 10/30 - Dennis meets the inspector at 8."])
    sheet.append(["Warehouse = pull equipment for the next job, back on site after lunch if done."])
    text = csv_text(pad(sheet, n), delimiter="\t", lineterminator="\r\n")
    d = fixture_dir(FID, SLUG)
    write_bytes(os.path.join(d, "input.txt"), text.encode("cp1252"))

    flags = [
        flag_db("Omar Haddad", w2[1], [K, B]),
        flag_nb("Trinh Nguyen", w1[4]),
        flag_nb("Carlos Mejia", w2[3]),
        flag_un(A, w1[2]),
    ]
    expected = dict(workdays=MON_FRI, layout="person_by_day", arrangement="stacked", assignments=recs,
                    flags=flags, info=[])
    meta = dict(window=w1 + w2, roster=None, person_canon={}, internal=[WH], placeholders=["Temp"], subs=[])
    return README, "input.txt", expected, meta
