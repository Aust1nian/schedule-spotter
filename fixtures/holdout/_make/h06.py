"""H06: landscaping crew, Spanish headers, semicolon CSV, person x day, Mon-Sat."""
from common import *

FID, SLUG = "H06", "landscaper-spanish-semicolon-person-grid"
README = ("Landscaping company's semicolon CSV in Spanish (Empleado, Lunes 10/12 ... Sabado), Mon-Sat: "
          "'Semana del' title line, accented names, Spanish statuses (descanso, enfermo, vacaciones, libre) "
          "plus x, two sites in one cell joined by '/' and by ' y ', a note in a cell, blank days, a totals "
          "row and a signature line.")


def build():
    dates = days("2026-10-12", 6)
    Mo, Tu, We, Th, Fr, Sa = dates
    Wh, Li, MV, ER = "Whitman patio", "Linden HOA", "1820 Monte Vista", "Escuela Roosevelt"
    rows = [
        ("Ramón Herrera", [Wh, Wh, Li, Li, MV, "descanso"]),
        ("Chuy Medina", [Wh] * 6),
        ("Pepe Lozano", [Wh, Wh, Wh, "enfermo", Wh, "descanso"]),
        ("Nacho Ibarra", [Li] * 6),
        ("Memo Cárdenas", [Li, Li, Li, f"{Li} / {ER}", ER, "libre"]),
        ("Lalo Fuentes", [MV, MV, "", MV, MV, MV]),
        ("Toño Ruiz", ["vacaciones"] * 6),
        ("Gerardo Paz", [MV, f"{MV} y {Wh}", ER, ER, ER, ""]),
        ("Adán Solís", ["Shop", f"{ER} - riego", ER, ER, ER, "x"]),
        ("Wilmer Ortega", [Wh] * 6),
    ]
    recs = []
    for name, cells in rows:
        for d, c in zip(dates, cells):
            recs += person_cell(name, d, c)
    totals = [sum(1 for r in recs if r["date"] == d and "site" in r) for d in dates]

    hdr_days = ["Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado"]
    sheet = [
        ["JARDINES HERRERA - PROGRAMA SEMANAL"] + [""] * 6,
        ["Semana del 10/12/2026"] + [""] * 6,
        [""] * 7,
        ["Empleado"] + [f"{n} {md(d)}" for n, d in zip(hdr_days, dates)],
    ]
    for name, cells in rows:
        sheet.append([name] + cells)
    sheet.append([""] * 7)
    sheet.append(["Total en obra"] + [str(t) for t in totals])
    sheet.append([""] * 7)
    sheet.append(["Nota: Pepe trae la mezcladora el lunes a Whitman."] + [""] * 6)
    sheet.append(["Firma del supervisor: ______________"] + [""] * 6)
    d = fixture_dir(FID, SLUG)
    write_bytes(os.path.join(d, "input.csv"), csv_text(sheet, delimiter=";").encode("utf-8"))

    flags = [
        flag_db("Memo Cárdenas", Th, [Li, ER]),
        flag_db("Gerardo Paz", Tu, [MV, Wh]),
        flag_nb("Lalo Fuentes", We),
        flag_nb("Gerardo Paz", Sa),
        flag_un(MV, We),
    ]
    expected = dict(workdays=MON_SAT, layout="person_by_day", assignments=recs, flags=flags, info=[],
                    must_not_ask=[{"type": "weekday_mismatch"}])
    meta = dict(window=dates, roster=None, person_canon={}, internal=["Shop"], placeholders=[], subs=[])
    return README, "input.csv", expected, meta
