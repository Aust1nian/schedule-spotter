"""H13: builder's job x day grid in LibreOffice .ods with an EMPLOYEES roster block beside it."""
from common import *
from odf.opendocument import OpenDocumentSpreadsheet
from odf.table import CoveredTableCell, Table, TableCell, TableColumn, TableRow
from odf.text import P

FID, SLUG = "H13", "builder-ods-job-grid-roster-block"
README = ("Builder's LibreOffice .ods job x day grid with an EMPLOYEES roster block (names and cell numbers) "
          "to the right: spanned title, 'week of October 19, 2026' line, upper-case 'MON 10/19' headers, a "
          "roster employee who is never on a job after vacation, a new hire missing from the roster, a "
          "concrete subcontractor at two jobs, open and em dash cells, Shop, OFF and VAC rows, footer notes.")


def build():
    dates = days("2026-10-19", 5)
    Mo, Tu, We, Th, Fr = dates
    EM = "—"
    BR, QS, HA, FS, LP = "Bishop Rd barn", "Quincy St 4-plex", "Hartman ADU", "Fire Stn 12 bays", "Lakeside pavilion"
    BC = "Bayside Concrete Inc."
    jobs = [
        (BR, ["Arlo, Brett, Cesar", "Arlo, Brett, Cesar", "Arlo, Brett", "Arlo, Brett, Cesar", "Arlo, Brett, Cesar"]),
        (QS, ["Darius, Emmett, Colton", "Darius, Emmett, Colton", "Darius, Emmett, Colton", "Darius, Emmett", "Darius"]),
        (HA, ["Felix, Grady", "Felix, Grady", "Felix, Grady, Cesar", "", "Felix, Grady"]),
        (FS, ["Hollis, Ivan", f"Hollis, Ivan, {BC}", "Hollis, Ivan", "Hollis, Ivan, Felix, Grady", f"Ivan, {BC}"]),
        (LP, [EM, EM, "open", f"Grady, {BC}", f"Emmett, {BC}"]),
        ("Shop", ["", "", "", "", "Hollis"]),
    ]
    pseudo = [("OFF", "off", ["", "", "", "", "Brett"]), ("VAC", "vacation", ["Wade", "Wade", "Wade", "", ""])]
    roster = [("Arlo", "541-555-0141"), ("Brett", "541-555-0187"), ("Cesar", "541-555-0119"),
              ("Darius", "541-555-0163"), ("Emmett", "541-555-0108"), ("Felix", "541-555-0172"),
              ("Grady", "541-555-0135"), ("Hollis", "541-555-0190"), ("Ivan", "541-555-0126"),
              ("Wade", "541-555-0122")]
    recs, inactive = [], []
    for job, cells in jobs:
        for d, c in zip(dates, cells):
            if c and norm(c).lower() in INACTIVE:
                inactive.append([job, d])
            recs += names_cell(job, d, c)
    for label, st, cells in pseudo:
        for d, c in zip(dates, cells):
            recs += names_cell(label, d, c, status=st)

    grid = [[job] + cells for job, cells in jobs] + [[label] + cells for label, _, cells in pseudo]
    sheet = [
        ["KESTREL VALLEY BUILDERS"],
        ["Crew schedule week of October 19, 2026"],
        [],
        ["JOB"] + [f"{n} {md(d)}" for n, d in zip(["MON", "TUE", "WED", "THU", "FRI"], dates)]
        + ["", "EMPLOYEES", "Cell"],
    ]
    for i in range(max(len(grid), len(roster))):
        left = grid[i] if i < len(grid) else [""] * 6
        right = list(roster[i]) if i < len(roster) else ["", ""]
        sheet.append(left + [""] + right)
    sheet += [[], ["Bayside Concrete Inc. pouring Fire Stn aprons Tue and Fri, Lakeside footings Thu-Fri."],
              ["Colton starts Monday - he rides with Darius to Quincy."]]

    doc = OpenDocumentSpreadsheet()
    t = Table(name="Week of 10-19")
    t.addElement(TableColumn(numbercolumnsrepeated=9))
    for i, row in enumerate(sheet):
        tr = TableRow()
        if i == 0:
            tc = TableCell(valuetype="string", numbercolumnsspanned=6)
            tc.addElement(P(text=row[0]))
            tr.addElement(tc)
            for _ in range(5):
                tr.addElement(CoveredTableCell())
        else:
            for v in row:
                if v == "":
                    tr.addElement(TableCell())
                else:
                    tc = TableCell(valuetype="string")
                    tc.addElement(P(text=v))
                    tr.addElement(tc)
        t.addElement(tr)
    doc.spreadsheet.addElement(t)
    d = fixture_dir(FID, SLUG)
    doc.save(os.path.join(d, "input.ods"))

    flags = [
        flag_db("Grady", Th, [FS, LP]),
        flag_awo("Brett", Fr, [BR], "off"),
        flag_nb("Wade", Th),
        flag_nb("Wade", Fr),
        flag_un(HA, Th),
        flag_un(LP, We),
    ]
    infos = [info("subcontractor_multi_site", BC, Fr, [FS, LP])]
    expected = dict(workdays=MON_FRI, layout="job_by_day", assignments=recs, flags=flags, info=infos,
                    must_not_ask=[{"type": "weekday_mismatch"}])
    meta = dict(window=dates, roster=[n for n, _ in roster], person_canon={}, internal=["Shop"], placeholders=[],
                subs=[BC], jobs=[j for j, _ in jobs], inactive=inactive, pseudo_rows=["OFF", "VAC"])
    return README, "input.ods", expected, meta
