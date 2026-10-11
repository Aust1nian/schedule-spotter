"""H15: person x day range copied from Excel and pasted (tab-separated, CRLF), dates without a year."""
from common import *

FID, SLUG = "H15", "paste-excel-person-grid-no-year"
README = ("Paste: a person x day range copied straight out of Excel (tabs, CRLF, a quoted cell holding two "
          "sites on two lines), 'Mon 10/19' headers with no year anywhere (year from today), off/sick/PTO, "
          "Shop, blank days, a job nobody covers mid-week and an 'On jobs' count row at the bottom.")


def build():
    dates = days("2026-10-19", 5)
    Mo, Tu, We, Th, Fr = dates
    RR, GB, MK, OM = "Ridley Rd", "Garvey bath", "Mercer kitchen", "Old Mill Inn"
    rows = [
        ("Brandon Ash", [RR] * 5),
        ("Corey Mills", [RR, RR, f"{RR}\n{GB}", RR, "off"]),
        ("Danny Kim", [GB, GB, "", GB, GB]),
        ("Ezra Cohen", [MK, MK, MK, "sick", MK]),
        ("Freddy Soto", [MK] * 5),
        ("Glen Harper", [OM, OM, "Shop", OM, OM]),
        ("Hank Doyle", [OM, "PTO", "PTO", OM, ""]),
        ("Isaac Long", ["Shop", OM, "", MK, OM]),
    ]
    recs = []
    for name, cells in rows:
        for d, c in zip(dates, cells):
            recs += person_cell(name, d, c)
    counts = [sum(1 for r in recs if r["date"] == d and "site" in r and r["site"] != "Shop") for d in dates]
    sheet = [["Name"] + [f"{n} {md(d)}" for n, d in zip(["Mon", "Tue", "Wed", "Thu", "Fri"], dates)]]
    for name, cells in rows:
        sheet.append([name] + cells)
    sheet.append([""] * 6)
    sheet.append(["On jobs"] + [str(c) for c in counts])
    text = csv_text(sheet, delimiter="\t", lineterminator="\r\n")
    d = fixture_dir(FID, SLUG)
    write_bytes(os.path.join(d, "paste.txt"), text.encode("utf-8"))

    flags = [
        flag_db("Corey Mills", We, [RR, GB]),
        flag_nb("Danny Kim", We),
        flag_nb("Hank Doyle", Fr),
        flag_nb("Isaac Long", We),
        flag_un(OM, We),
    ]
    expected = dict(workdays=MON_FRI, layout="person_by_day", paste=True, assignments=recs, flags=flags, info=[],
                    must_not_ask=[{"type": "paste_as_table"}, {"type": "weekday_mismatch"}])
    meta = dict(window=dates, roster=None, person_canon={}, internal=["Shop"], placeholders=[], subs=[])
    return README, "paste.txt", expected, meta
