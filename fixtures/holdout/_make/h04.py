"""H04: concrete flatwork crew, day x job grid in an old .xls, Mon-Sat, OFF column."""
from common import *
import xlwt

FID, SLUG = "H04", "flatwork-day-by-job-xls"
README = ("Concrete flatwork company's Excel 97-2003 .xls with days down and jobs across (Mon-Sat): "
          "merged title, 'Week of' line, 'Mon 10/19' row labels, names split by commas and '/', an OFF "
          "column, a Yard column, inactive cells (x and em dash), 'need 1', blank job-days, a placeholder "
          "laborer and a footer note.")


def build():
    dates = days("2026-10-19", 6)
    Mo, Tu, We, Th, Fr, Sa = dates
    EM = "—"
    jobs = ["Sav-Mor lot patch", "Vista Ridge sidewalks", "214 Pine driveway", "Calvary Baptist steps", "Yard"]
    grid = [
        ["Rigo, Chava, Ernesto", "Lupe, Beto, Junior", EM, EM, "Kyle", "Darnell"],
        ["Rigo, Chava, Ernesto", "Lupe, Beto, Junior, Kyle", EM, EM, "", "Darnell, Lupe"],
        ["Rigo, Chava", "Lupe, Beto, Junior", "Ernesto, Kyle, Laborer 2", EM, "", ""],
        ["x", "Lupe, Beto", "Ernesto, Kyle, Darnell", "Rigo, Chava, Junior", "", ""],
        ["Rigo / Chava", "Lupe, Beto, Junior, Rigo", "Ernesto, Kyle, Darnell", "need 1", "Junior", ""],
        ["", "Lupe, Beto", "Ernesto, Darnell", "", "", "Rigo, Chava, Junior, Kyle"],
    ]
    recs, inactive = [], []
    for d, row in zip(dates, grid):
        for job, c in zip(jobs, row[:5]):
            if c and norm(c).lower() in INACTIVE:
                inactive.append([job, d])
            recs += names_cell(job, d, c)
        recs += names_cell("OFF", d, row[5], status="off")

    wb = xlwt.Workbook(encoding="utf-8")
    ws = wb.add_sheet("Sheet1")
    title = xlwt.easyxf("font: bold on, height 320; align: horiz center")
    bold = xlwt.easyxf("font: bold on; pattern: pattern solid, fore_colour gray25; borders: bottom thin")
    wrap = xlwt.easyxf("align: wrap on, vert top")
    ws.write_merge(0, 0, 0, 6, "MARQUEZ FLATWORK", title)
    ws.write_merge(1, 1, 0, 6, "Week of 10/19/2026")
    ws.write(3, 0, "Day", bold)
    for j, job in enumerate(jobs + ["OFF"], start=1):
        ws.write(3, j, job, bold)
    for i, (d, row) in enumerate(zip(dates, grid)):
        ws.write(4 + i, 0, f"{['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'][i]} {md(d)}", bold)
        for j, c in enumerate(row, start=1):
            if c:
                ws.write(4 + i, j, c, wrap)
    ws.write(11, 0, "Pump truck Thu 7am at Calvary - confirm with Rigo the night before.")
    ws.write(12, 0, "Laborer 2 comes from the agency, Wed only so far.")
    ws.col(0).width = 256 * 14
    for j in range(1, 7):
        ws.col(j).width = 256 * 26
    d = fixture_dir(FID, SLUG)
    wb.save(os.path.join(d, "input.xls"))

    flags = [
        flag_db("Rigo", Fr, ["Sav-Mor lot patch", "Vista Ridge sidewalks"]),
        flag_awo("Lupe", Tu, ["Vista Ridge sidewalks"], "off"),
        flag_nb("Darnell", We),
        flag_un("Sav-Mor lot patch", Sa),
        flag_un("Calvary Baptist steps", Fr),
        flag_un("Calvary Baptist steps", Sa),
    ]
    infos = [info("internal_plus_site", "Junior", Fr, ["Vista Ridge sidewalks", "Yard"])]
    expected = dict(workdays=MON_SAT, layout="day_by_job", assignments=recs, flags=flags, info=infos,
                    must_not_ask=[{"type": "weekday_mismatch"}])
    meta = dict(window=dates, roster=None, person_canon={}, internal=["Yard"], placeholders=["Laborer 2"], subs=[],
                jobs=jobs, inactive=inactive, pseudo_rows=["OFF"])
    return README, "input.xls", expected, meta
