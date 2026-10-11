"""H17: small electrical contractor, days down / people across, .tsv. Clean."""
from common import *

FID, SLUG = "H17", "electrician-day-by-person-tsv-clean"
README = ("Clean schedule (no double-bookings, no one assigned while off) as a .tsv with days down and "
          "people across (first names only): company title containing 'Electric', 'Week of' line, "
          "'Mon 10/12' row labels, sick/PTO, Shop plus a site in one cell, blank days, a job that sits idle "
          "two days mid-week and footer notes.")


def build():
    dates = days("2026-10-12", 5)
    Mo, Tu, We, Th, Fr = dates
    HC, PR, RP, OS = "Hillcrest Elem.", "Patel residence", "Riverbend Plaza", "Oak St duplex"
    people = ["Stan", "Wojtek", "Lisa", "Andre", "Mo", "Greg"]
    grid = [
        [HC, HC, PR, RP, OS, "Shop"],
        [HC, HC, PR, RP, RP, PR],
        [HC, "sick", PR, RP, "", PR],
        [HC, HC, "", RP, OS, PR],
        [HC, HC, PR, "PTO", RP, f"Shop / {RP}"],
    ]
    recs = []
    for d, row in zip(dates, grid):
        for p, c in zip(people, row):
            recs += person_cell(p, d, c)
    sheet = [["KOWALSKI & SONS ELECTRIC"], ["Week of 10/12/2026"], [""], ["Day"] + people]
    for n, d, row in zip(["Mon", "Tue", "Wed", "Thu", "Fri"], dates, grid):
        sheet.append([f"{n} {md(d)}"] + row)
    sheet += [[""], ["Hillcrest rough-in inspection Thu 10am."], ["Patel panel swap needs the permit card on site."]]
    text = csv_text(pad(sheet, 7), delimiter="\t", lineterminator="\n")
    d = fixture_dir(FID, SLUG)
    write_bytes(os.path.join(d, "input.tsv"), text.encode("utf-8"))

    flags = [
        flag_nb("Mo", We),
        flag_nb("Lisa", Th),
        flag_un(OS, Tu),
        flag_un(OS, We),
    ]
    infos = [info("internal_plus_site", "Greg", Fr, ["Shop", RP])]
    expected = dict(workdays=MON_FRI, layout="day_by_person", assignments=recs, flags=flags, info=infos,
                    must_not_ask=[{"type": "weekday_mismatch"}])
    meta = dict(window=dates, roster=None, person_canon={}, internal=["Shop"], placeholders=[], subs=[])
    return README, "input.tsv", expected, meta
