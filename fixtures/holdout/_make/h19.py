"""H19: masonry company's job-day list (one row per job per day, crew in one cell), comma CSV."""
from common import *

FID, SLUG = "H19", "mason-job-day-list-multiline-crews"
README = ("Masonry company's list with one row per job per day and the whole crew in one quoted cell "
          "(commas, '&' and line breaks inside quotes), Date + Day columns, times on some rows applying to "
          "the whole crew, a split day, PTO/SICK/VAC used as the job, a placeholder Laborer, Yard, and the "
          "owner's son written 'Tony Russo' on most rows but 'Anthony Russo' on one.")

HOURS = {"7-3:30": ("07:00", "15:30"), "7-11": ("07:00", "11:00"), "11:30-3:30": ("11:30", "15:30")}


def build():
    Mo, Tu, We, Th, Fr = days("2026-10-19", 5)
    EC, WL, CS, SR = "Ellison chimney rebuild", "Westbrook library veneer", "Calder St retaining wall", "St. Rita's steps"
    rows = [
        (Mo, EC, "Tony Russo, Gabe Lutz", "7-3:30"),
        (Mo, WL, "Marty Flynn\nOwen Price\nReuben Cho", ""),
        (Mo, CS, "Sal DiNapoli & Eddie Kwan & Laborer", ""),
        (Mo, SR, "Hector Ruiz, Jim Kovac", ""),
        (Mo, "Yard", "Pablo Vega", ""),
        (Mo, "PTO", "Lance Moody", ""),
        (Tu, EC, "Tony Russo, Gabe Lutz", "7-3:30"),
        (Tu, WL, "Marty Flynn\nOwen Price\nReuben Cho\nPablo Vega", ""),
        (Tu, CS, "Sal DiNapoli & Eddie Kwan", ""),
        (Tu, SR, "Hector Ruiz, Jim Kovac", ""),
        (Tu, "PTO", "Lance Moody", ""),
        (We, EC, "Tony Russo, Gabe Lutz", ""),
        (We, WL, "Marty Flynn, Owen Price, Anthony Russo", ""),
        (We, CS, "Sal DiNapoli, Eddie Kwan, Lance Moody", ""),
        (We, SR, "Hector Ruiz", ""),
        (We, "Yard", "Jim Kovac, Pablo Vega", ""),
        (Th, EC, "Tony Russo, Gabe Lutz", "7-11"),
        (Th, SR, "Tony Russo", "11:30-3:30"),
        (Th, WL, "Marty Flynn\nOwen Price\nReuben Cho\nLance Moody", ""),
        (Th, SR, "Hector Ruiz, Jim Kovac, Pablo Vega", ""),
        (Th, "Yard", "Eddie Kwan", ""),
        (Th, "SICK", "Sal DiNapoli", ""),
        (Fr, EC, "Tony Russo, Gabe Lutz", ""),
        (Fr, WL, "Marty Flynn, Owen Price, Reuben Cho", ""),
        (Fr, CS, "Sal DiNapoli, Eddie Kwan, Lance Moody", ""),
        (Fr, SR, "Hector Ruiz, Jim Kovac, Pablo Vega", ""),
        (Fr, "VAC", "Pablo Vega", ""),
    ]
    dayname = dict(zip([Mo, Tu, We, Th, Fr], ["Mon", "Tue", "Wed", "Thu", "Fri"]))
    sheet = [["KEYSTONE STONE & BRICK"], ["Crew list - week of 10/19/2026"], [""],
             ["Date", "Day", "Job", "Crew", "Hours"]]
    recs = []
    for d, job, crew, hrs in rows:
        sheet.append([md(d, year=True), dayname[d], job, crew, hrs])
        st = STATUS.get(job.lower())
        if st:
            recs += names_cell(job, d, crew, status=st)
            continue
        s, e = HOURS[hrs] if hrs else (None, None)
        for name in split_multi(crew):
            recs.append(W(name, d, job, start=s, end=e))
    sheet += [[""], ["Questions call Tony (cell 555-0144). Mortar and sand delivered to the yard Mon AM."]]
    text = csv_text(pad(sheet, 5), lineterminator="\n")
    d = fixture_dir(FID, SLUG)
    write_bytes(os.path.join(d, "input.csv"), text.encode("utf-8"))

    flags = [
        flag_db("Anthony Russo", We, [EC, WL]),
        flag_awo("Pablo Vega", Fr, [SR], "vacation"),
        flag_nb("Reuben Cho", We),
        flag_un(CS, Th),
    ]
    infos = [info("split_day", "Anthony Russo", Th, [EC, SR])]
    expected = dict(workdays=MON_FRI, layout="long_list", assignments=recs, flags=flags, info=infos,
                    must_not_ask=[{"type": "weekday_mismatch"}])
    meta = dict(window=None, roster=None, person_canon={"Tony Russo": "Anthony Russo"}, internal=["Yard"],
                placeholders=["Laborer"], subs=[])
    return README, "input.csv", expected, meta
