"""H01: roofing company long list, comma CSV (LF), times on some rows, Saturday overtime."""
from common import *

FID, SLUG = "H01", "roofer-long-list-times-sat-overtime"
README = ("Roofing contractor's hand-kept long list (comma CSV, LF): title rows, footer notes, "
          "times on some rows (7-3:30, 7a-3p, 1p-4p), split day vs overlapping times, rows appended "
          "out of order, Saturday overtime under a Mon-Fri week, PTO/sick rows, 'need 1', a gutter "
          "subcontractor at two sites, a placeholder laborer, stray spaces in a name.")

HOURS = {"7-3:30": ("07:00", "15:30"), "7a-3p": ("07:00", "15:00"), "7-11": ("07:00", "11:00"),
         "12-3:30": ("12:00", "15:30"), "1p-4p": ("13:00", "16:00"), "7-12": ("07:00", "12:00")}


def build():
    M, T, Wd, R, F, Sa = days("2026-10-12", 6)
    rows = [
        (M, "Dale Whitaker", "2210 Birch Ln", "7-3:30"),
        (M, "Marco Reyes", "2210 Birch Ln", "7-3:30"),
        (M, "Tyler  Bunch ", "2210 Birch Ln", ""),
        (M, "Hector Salas", "Oakmont HOA bldg C", ""),
        (M, "Jordan Pike", "Oakmont HOA bldg C", ""),
        (M, "Andre Moss", "Oakmont HOA bldg C", ""),
        (M, "Sam Ferreira", "Shop - load trailer", ""),
        (M, "Cody Lind", "PTO", ""),
        (M, "Wes Hartley", "418 Kestrel Ct", ""),
        (M, "Laborer 2", "418 Kestrel Ct", ""),
        (M, "Summit Gutter Co.", "77 Arbor Way", ""),

        (T, "Dale Whitaker", "2210 Birch Ln", ""),
        (T, "Marco Reyes", "2210 Birch Ln", ""),
        (T, "Tyler Bunch", "2210 Birch Ln", ""),
        (T, "Hector Salas", "Oakmont HOA bldg C", ""),
        (T, "Jordan Pike", "Oakmont HOA bldg C", ""),
        (T, "Andre Moss", "Oakmont HOA bldg C", ""),
        (T, "Sam Ferreira", "Shop", ""),
        (T, "Sam Ferreira", "418 Kestrel Ct", ""),
        (T, "Cody Lind", "PTO", ""),
        (T, "Wes Hartley", "418 Kestrel Ct", ""),

        (Wd, "Dale Whitaker", "2210 Birch Ln", "7-11"),
        (Wd, "Dale Whitaker", "Linden Church", "12-3:30"),
        (Wd, "Marco Reyes", "2210 Birch Ln", ""),
        (Wd, "Tyler Bunch", "2210 Birch Ln", ""),
        (Wd, "Hector Salas", "Oakmont HOA bldg C", ""),
        (Wd, "Jordan Pike", "Oakmont HOA bldg C", ""),
        (Wd, "Andre Moss", "sick", ""),
        (Wd, "Sam Ferreira", "418 Kestrel Ct", ""),
        (Wd, "Wes Hartley", "418 Kestrel Ct", ""),
        (Wd, "Summit Gutter Co.", "77 Arbor Way", ""),
        (Wd, "Summit Gutter Co.", "418 Kestrel Ct", ""),

        (R, "Dale Whitaker", "Linden Church", "7a-3p"),
        (R, "Marco Reyes", "2210 Birch Ln", "7-3:30"),
        (R, "Tyler Bunch", "2210 Birch Ln", ""),
        (R, "Hector Salas", "Oakmont HOA bldg C", ""),
        (R, "Jordan Pike", "Oakmont HOA bldg C", ""),
        (R, "Andre Moss", "Oakmont HOA bldg C", ""),
        (R, "Sam Ferreira", "2210 Birch Ln", ""),
        (R, "Wes Hartley", "Shop", ""),
        (R, "Cody Lind", "Oakmont HOA bldg C", ""),
        (R, "need 1", "Linden Church", ""),
        (R, "Andre Moss", "sick", ""),

        (F, "Dale Whitaker", "Linden Church", ""),
        (F, "Marco Reyes", "Linden Church", ""),
        (F, "Tyler Bunch", "2210 Birch Ln", ""),
        (F, "Hector Salas", "Oakmont HOA bldg C", ""),
        (F, "Andre Moss", "Oakmont HOA bldg C", ""),
        (F, "Sam Ferreira", "418 Kestrel Ct", ""),
        (F, "Wes Hartley", "Shop", ""),
        (F, "Cody Lind", "Oakmont HOA bldg C", ""),
        (F, "Laborer 2", "Linden Church", ""),

        (Sa, "Tyler Bunch", "2210 Birch Ln", "7-12"),
        (Sa, "Marco Reyes", "2210 Birch Ln", "7-12"),
        (Sa, "Hector Salas", "Oakmont HOA bldg C", "7-12"),

        # added later by the office, out of order
        (R, "Marco Reyes", "Linden Church", "1p-4p"),
        (Sa, "Hector Salas", "418 Kestrel Ct", ""),
    ]
    dayname = {d: n for d, n in zip(days("2026-10-12", 6), ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat"])}

    sheet = [
        ["RIDGELINE ROOFING & GUTTERS - CREW SCHEDULE", "", "", "", ""],
        ["Week of 10/12/2026", "", "", "", ""],
        ["", "", "", "", ""],
        ["Date", "Day", "Employee", "Job / Address", "Hours"],
    ]
    for d, emp, job, hrs in rows:
        sheet.append([md(d, year=True), dayname[d], emp, job, hrs])
    sheet += [
        ["", "", "", "", ""],
        ["NOTES:", "", "", "", ""],
        ["Birch Ln dumpster swap Wed AM - keep the driveway clear", "", "", "", ""],
        ["Summit Gutter Co. contact: 509-555-0182", "", "", "", ""],
        ["Laborer 2 = whoever Dale picks up from the temp agency", "", "", "", ""],
        ["Sat is OT - sign the sheet in the truck", "", "", "", ""],
    ]
    d = fixture_dir(FID, SLUG)
    write_bytes(os.path.join(d, "input.csv"), csv_text(sheet).encode("utf-8"))

    recs = []
    for dte, emp, job, hrs in rows:
        emp_n = norm(emp)
        if emp_n.lower() in UNSTAFFED:
            recs.append(U(dte, job))
            continue
        if norm(job).lower() in STATUS:
            recs.append(S(emp_n, dte, STATUS[norm(job).lower()]))
            continue
        site, note = strip_note(job)
        st, en = HOURS[hrs] if hrs else (None, None)
        recs.append(W(emp_n, dte, site, note=note, start=st, end=en))

    flags = [
        flag_db("Marco Reyes", R, ["2210 Birch Ln", "Linden Church"]),
        flag_db("Hector Salas", Sa, ["Oakmont HOA bldg C", "418 Kestrel Ct"]),
        flag_awo("Andre Moss", R, ["Oakmont HOA bldg C"], "sick"),
        flag_nb("Jordan Pike", F),
        flag_nb("Cody Lind", Wd),
        flag_un("Linden Church", R),
        flag_un("77 Arbor Way", T),
        flag_un("418 Kestrel Ct", R),
    ]
    infos = [
        info("split_day", "Dale Whitaker", Wd, ["2210 Birch Ln", "Linden Church"]),
        info("internal_plus_site", "Sam Ferreira", T, ["Shop", "418 Kestrel Ct"]),
        info("subcontractor_multi_site", "Summit Gutter Co.", Wd, ["77 Arbor Way", "418 Kestrel Ct"]),
    ]
    expected = dict(workdays=MON_FRI, layout="long_list", assignments=recs, flags=flags, info=infos,
                    must_not_ask=[{"type": "weekday_mismatch"}, {"type": "pick_layout"}])
    meta = dict(window=None, roster=None, person_canon={}, internal=["Shop"],
                placeholders=["Laborer 2"], subs=["Summit Gutter Co."])
    return README, "input.csv", expected, meta
