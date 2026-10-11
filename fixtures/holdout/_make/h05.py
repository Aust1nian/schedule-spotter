"""H05: fence & deck company, crew-based grid with a crew roster block on the same sheet."""
from common import *
from openpyxl import Workbook
from openpyxl.styles import Font, PatternFill

FID, SLUG = "H05", "fence-crew-grid-roster-block"
README = ("Fence and deck company's xlsx crew-based week: crew names in the grid, crew roster block to the "
          "right, weekday-only headers under a 'Week of' line, one man listed in two crews, individual rows "
          "for people outside crews (one written 'Mike B' while the roster says Mike Baker), a vac override "
          "row, Shop, a note in a site cell and a job left empty mid-week.")


def build():
    dates = days("2026-10-12", 5)
    Mo, Tu, We, Th, Fr = dates
    roster = {
        "CREW A": ["Hugo Saldana", "Pete Varga", "Andre Lucas"],
        "CREW B": ["Rafa Torres", "Dwayne Cole", "Sione Taufa", "Luis Ortiz"],
        "CREW C": ["Bill Wexler", "Ty Gentry", "Pete Varga"],
        "DECK CREW": ["Gus Pappas", "Mike Baker", "Jamal Reed"],
        "OTHER": ["Ken Ashby"],
    }
    crews = [
        ("Crew A", ["Haskell Farm"] * 5),
        ("Crew B", ["319 Quarry Rd", "319 Quarry Rd", "Linwood Elem.", "Linwood Elem.", "Linwood Elem."]),
        ("Crew C", ["Oakes - gate repair", "Shop", "Haskell Farm", "52 Sumac Dr", "52 Sumac Dr"]),
        ("Deck crew", ["Brennan deck", "Brennan deck", "Ferris deck", "Brennan deck", "Brennan deck"]),
    ]
    people_rows = [
        ("Mike B", ["", "", "", "Linwood Elem. (layout)", ""]),
        ("Sione Taufa", ["", "", "", "", "vac"]),
        ("Ken Ashby", ["Shop", "Shop", "", "", "Shop"]),
    ]
    recs = []
    for crew, cells in crews:
        members = roster[crew.upper()]
        for d, c in zip(dates, cells):
            site, note = strip_note(c)
            for m in members:
                recs.append(W(m, d, site, note=note))
    for name, cells in people_rows:
        for d, c in zip(dates, cells):
            recs += person_cell(name, d, c)

    wb = Workbook()
    ws = wb.active
    ws.title = "This week"
    ws.cell(1, 1, "TRI-COUNTY FENCE & DECK").font = Font(bold=True, size=16)
    ws.merge_cells("A1:F1")
    ws.cell(2, 1, "Week of 10/12/2026").font = Font(bold=True)
    ws.merge_cells("A2:F2")
    hdr_fill = PatternFill("solid", fgColor="E2EFDA")
    for j, v in enumerate(["Crew", "Mon", "Tue", "Wed", "Thu", "Fri"], start=1):
        c = ws.cell(4, j, v)
        c.font = Font(bold=True)
        c.fill = hdr_fill
    r = 5
    for crew, cells in crews:
        ws.cell(r, 1, crew).font = Font(bold=True)
        for j, c in enumerate(cells):
            ws.cell(r, 2 + j, c)
        r += 1
    r += 1
    for name, cells in people_rows:
        ws.cell(r, 1, name)
        for j, c in enumerate(cells):
            if c:
                ws.cell(r, 2 + j, c)
        r += 1
    r += 1
    ws.cell(r, 1, "Gate hardware for Oakes is on the shelf by the shop door.")
    ws.cell(r + 1, 1, "Mike B lays out the Linwood fence line Thu, back on decks after.")
    # crew roster block, columns H..L
    for j, (crew, members) in enumerate(roster.items()):
        c = ws.cell(4, 8 + j, crew)
        c.font = Font(bold=True)
        c.fill = PatternFill("solid", fgColor="FCE4D6")
        for i, m in enumerate(members):
            ws.cell(5 + i, 8 + j, m)
    ws.column_dimensions["A"].width = 14
    for col in "BCDEF":
        ws.column_dimensions[col].width = 20
    ws.column_dimensions["G"].width = 4
    for col in "HIJKL":
        ws.column_dimensions[col].width = 16
    d = fixture_dir(FID, SLUG)
    wb.save(os.path.join(d, "input.xlsx"))

    flags = [
        flag_db("Pete Varga", Mo, ["Haskell Farm", "Oakes"]),
        flag_db("Pete Varga", Th, ["Haskell Farm", "52 Sumac Dr"]),
        flag_db("Pete Varga", Fr, ["Haskell Farm", "52 Sumac Dr"]),
        flag_db("Mike Baker", Th, ["Brennan deck", "Linwood Elem."]),
        flag_awo("Sione Taufa", Fr, ["Linwood Elem."], "vacation"),
        flag_nb("Ken Ashby", We),
        flag_nb("Ken Ashby", Th),
        flag_un("Brennan deck", We),
    ]
    infos = [
        info("internal_plus_site", "Pete Varga", Tu, ["Haskell Farm", "Shop"]),
        info("duplicate_entry", "Pete Varga", We, ["Haskell Farm"]),
    ]
    expected = dict(workdays=MON_FRI, layout="crew_based", assignments=recs, flags=flags, info=infos,
                    must_not_ask=[{"type": "weekday_mismatch"}])
    all_roster = []
    for members in roster.values():
        for m in members:
            if m not in all_roster:
                all_roster.append(m)
    meta = dict(window=dates, roster=all_roster, person_canon={"Mike B": "Mike Baker"}, internal=["Shop"],
                placeholders=[], subs=[])
    return README, "input.xlsx", expected, meta
