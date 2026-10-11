"""H09: interiors contractor long list, Windows-1252 CSV with CRLF, accents and smart quotes."""
from common import *

FID, SLUG = "H09", "interiors-cp1252-long-list-start-end"
README = ("Interiors contractor's long list saved as Excel 'CSV (Comma delimited)' on Windows: Windows-1252 "
          "with José/Martín/Ávila and smart quotes inside job names, CRLF, rows grouped by job rather than "
          "date, Start/End columns on some rows (split day vs untimed second job), PTO/vac rows appended "
          "at the bottom, a drywall subcontractor at two jobs, a title with an en dash and footer lines.")

T = {"7:00 AM": "07:00", "3:30 PM": "15:30", "11:00 AM": "11:00", "12:00 PM": "12:00"}


def build():
    Mo, Tu, We, Th, Fr = days("2026-10-26", 5)
    OH = "Oak Hollow “model home”"
    HP = "Harbor Pointe bldg 7"
    SL = "St. Luke’s parish hall"
    DC = "Dunmore condo 3B"
    TR = "Tanaka residence"
    ACE = "Ace Drywall Inc."
    full = ("7:00 AM", "3:30 PM")
    rows = []

    def add(d, people, job, times=("", "")):
        for p in people:
            rows.append((d, p, job, times[0], times[1]))

    add(Mo, ["José Peña", "Martín Ibáñez", "Raúl Ortega"], OH, full)
    add(Tu, ["José Peña", "Martín Ibáñez", "Raúl Ortega"], OH)
    add(We, ["José Peña", "Martín Ibáñez"], OH)
    add(Th, ["José Peña"], OH, full)
    add(Th, ["Martín Ibáñez", "Raúl Ortega"], OH)
    add(Fr, ["José Peña", "Martín Ibáñez"], OH)

    add(Mo, ["Germán Ávila", "Joaquín Saenz", ACE], HP)
    add(Tu, ["Germán Ávila", "Joaquín Saenz", ACE], HP)
    add(We, ["Germán Ávila", "Joaquín Saenz", "Raúl Ortega"], HP)
    add(Th, ["Germán Ávila"], HP, ("7:00 AM", "11:00 AM"))
    add(Th, ["Joaquín Saenz"], HP)

    add(Mo, ["Rubén Ochoa", "Dwight Harlan"], SL)
    add(Tu, ["Rubén Ochoa", "Dwight Harlan"], SL)
    add(Th, ["Rubén Ochoa", "Dwight Harlan"], SL)
    add(Th, ["Germán Ávila"], SL, ("12:00 PM", "3:30 PM"))
    add(Fr, ["Rubén Ochoa", "Dwight Harlan"], SL)

    add(Mo, ["Kurt Neumann"], DC)
    add(Tu, ["Kurt Neumann", ACE], DC)
    add(We, ["Kurt Neumann", "Rubén Ochoa"], DC)
    add(Th, ["Kurt Neumann"], DC)
    add(Fr, ["Kurt Neumann", "Lorena Díaz"], DC)

    add(Mo, ["Lorena Díaz", "Efraín Montes"], TR)
    add(Tu, ["Lorena Díaz", "Efraín Montes"], TR)
    add(We, ["Lorena Díaz", "Efraín Montes", "Dwight Harlan"], TR)
    add(Th, ["Lorena Díaz", "Efraín Montes", "José Peña"], TR)
    add(Fr, ["Efraín Montes"], TR)

    add(Fr, ["Raúl Ortega"], "Shop")
    add(Fr, ["Martín Ibáñez"], "PTO")
    add(Fr, ["Joaquín Saenz"], "vac")

    sheet = [
        ["Castillo Interiors – Field Schedule", "", "", "", ""],
        ["Week of 10/26/2026", "", "", "", ""],
        ["", "", "", "", ""],
        ["Date", "Employee", "Job", "Start", "End"],
    ]
    recs = []
    for d, p, job, s, e in rows:
        sheet.append([md(d, year=True), p, job, s, e])
        if job.lower() in STATUS:
            recs.append(S(p, d, STATUS[job.lower()]))
        else:
            recs.append(W(p, d, job, start=T.get(s), end=T.get(e)))
    sheet += [
        ["", "", "", "", ""],
        ["“Please don’t change this sheet – call the office”", "", "", "", ""],
        ["Approved by: ______________", "", "", "", ""],
    ]
    text = csv_text(sheet, lineterminator="\r\n")
    d = fixture_dir(FID, SLUG)
    write_bytes(os.path.join(d, "input.csv"), text.encode("cp1252"))

    flags = [
        flag_db("José Peña", Th, [OH, TR]),
        flag_awo("Martín Ibáñez", Fr, [OH], "pto"),
        flag_nb("Germán Ávila", Fr),
        flag_un(SL, We),
    ]
    infos = [
        info("split_day", "Germán Ávila", Th, [HP, SL]),
        info("subcontractor_multi_site", ACE, Tu, [HP, DC]),
    ]
    expected = dict(workdays=MON_FRI, layout="long_list", assignments=recs, flags=flags, info=infos)
    meta = dict(window=None, roster=None, person_canon={}, internal=["Shop"], placeholders=[], subs=[ACE])
    return README, "input.csv", expected, meta
