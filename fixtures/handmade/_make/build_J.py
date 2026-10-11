"""Category J: scale.
Run: uv run --with openpyxl==3.1.5 python3 build_J.py

J01-J02 are written by hand. J03-J05 are generated here, deterministically
(random.Random with a fixed seed per fixture), from a clean base:
every person works one job all week, every job has at least two people every
working day, and jobs change from week to week. Each planted problem below
spells out the flag it must cause; check_flags.py re-derives them all.

J05 (50,000 rows) exceeds the spec's 20,000-row limit, so the pipeline reads
only the first 4 weeks found and says so. The file is sorted by date and starts
on a Monday, so "first 4 weeks in file order", "the 4 earliest Mon-Sun weeks"
and "28 days from the first date" all pick the same 4 weeks. Every week after
the 4th is clean (no flag of any kind), so the expected flags hold whether or
not the later rows are read. Its expected.json lists a sample of the first 4
weeks' assignments (assignments_exact false); the full first-4-weeks truth is
in hints/J05.json for the checker.

Names: 100 first names and 100 last names, every token distinct, none in the
spec's nickname table, no placeholder or subcontractor tokens, and every pair
of full names below Jaro-Winkler 0.85, so no name pair is a merge candidate.
Sites: every token distinct, no internal-location word, no house number reused.
"""
import datetime as dt
import random

from fxlib import *

WEEK = days("10-12", 5)
J, S, O, P, L = "1420 Johnson St", "Smith remodel", "Oak Ridge Apts", "Parker kitchen", "Lincoln School"
NO_ASK = [Q("pick_layout"), Q("column_role")]

FIRST = """Aaron Adrian Andre Angel Arturo Barry Bennett Blake Bruce Caleb Calvin Cesar Chad Clay Cody
Colin Curtis Dale Damon Darius Dean Derek Diego Dominic Dustin Edgar Elias Emilio Eric Ernesto Ethan
Felix Fernando Gabriel Gary Glen Grant Hector Hugo Isaac Ivan Jared Javier Jerome Joel Jorge Julian
Kirk Kyle Lance Leon Lorenzo Lucas Malik Marcus Wendell Martin Mateo Neil Noah Omar Oscar Owen Pablo
Pedro Percy Quinn Rafael Ramon Reggie Rico Rodney Rolando Ruben Russell Salvador Samuel Saul Sergio
Shane Simon Stefan Terrence Tobias Travis Trent Troy Ulises Victor Wesley Xavier Yusuf Zane Ahmed
Boris Cyrus Desmond Ignatius Byron Garrett""".split()
LAST = """Abbott Acosta Barnes Becker Bishop Bowen Brady Burke Cabrera Carver Chandler Cisneros Cole
Conway Crane Dalton Dawson Duran Ellis Espinoza Farley Fischer Fleming Fowler Gallegos Garner Gibbs
Goodwin Graves Guerra Hahn Hardy Hayes Hendrix Hobbs Holt Huerta Ingram Jensen Keller Kemp Knox Lamb
Larson Lowe Lucero Macias Mann Marsh Medina Mercer Molina Morrow Nash Navarro Nolan Novak Ochoa Olsen
Orozco Pace Padilla Pruitt Quintero Ramos Rhodes Riggs Rojas Rowe Salas Sandoval Sexton Shaw Sloan
Snow Soto Stark Stone Tate Thorne Trujillo Underwood Valdez Vega Webb Whitaker Wiley Yates York
Zamora Zimmerman Baird Cantu Esparza Frost Gill Hurst Pittman Rasmussen Langford""".split()
SITES = ["1420 Johnson St", "88 Elm Ave", "312 Birch Rd", "77 Cedar Ln", "905 Maple Dr", "Lincoln School",
         "Riverside duplex", "Oak Ridge Apts", "Parker kitchen", "Smith remodel", "Harbor View condos",
         "Gomez addition", "Patel basement", "Nguyen deck", "Walsh bathroom", "Hillcrest church",
         "Lakeside clinic", "Mesa library", "Sunset Plaza", "Kowalski garage", "Brennan porch",
         "Delgado patio", "Valley bank", "Hartman attic", "Fairview dental"]
NICKNAMES = {"mike", "michael", "bob", "robert", "bill", "william", "jim", "james", "tony", "anthony", "pepe",
             "jose", "paco", "francisco", "chuy", "jesus", "nacho", "ignacio", "lalo", "eduardo", "memo",
             "guillermo", "beto", "alberto", "roberto"}


def jaro_winkler(a: str, b: str) -> float:
    a, b = a.casefold(), b.casefold()
    if a == b:
        return 1.0
    la, lb = len(a), len(b)
    rng = max(la, lb) // 2 - 1
    ma, mb = [False] * la, [False] * lb
    m = 0
    for i, ch in enumerate(a):
        for j in range(max(0, i - rng), min(lb, i + rng + 1)):
            if not mb[j] and b[j] == ch:
                ma[i] = mb[j] = True
                m += 1
                break
    if not m:
        return 0.0
    t, k = 0, 0
    for i in range(la):
        if ma[i]:
            while not mb[k]:
                k += 1
            t += a[i] != b[k]
            k += 1
    jaro = (m / la + m / lb + (m - t / 2) / m) / 3
    p = 0
    while p < min(4, la, lb) and a[p] == b[p]:
        p += 1
    return jaro + p * 0.1 * (1 - jaro)


def people(n):
    assert len(FIRST) >= 100 and len(LAST) >= 100, (len(FIRST), len(LAST))
    names = [f"{f} {l}" for f, l in zip(FIRST[:n], LAST[:n])]
    return names


def check_names():
    toks = [t.casefold() for t in FIRST[:100] + LAST[:100]]
    assert len(set(toks)) == len(toks), [t for t in toks if toks.count(t) > 1]
    assert not NICKNAMES & set(toks)
    names = people(100)
    for x in names:
        assert kind_ok(x), x
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            assert jaro_winkler(a, b) < 0.85, (a, b, jaro_winkler(a, b))
    stoks = [t.casefold() for s in SITES for t in s.split()]
    assert len(set(stoks)) == len(stoks), [t for t in stoks if stoks.count(t) > 1]
    assert not {"shop", "yard", "office", "warehouse", "travel"} & set(stoks)
    for i, a in enumerate(SITES):
        for b in SITES[i + 1:]:
            assert jaro_winkler(a, b) < 0.85, (a, b)


def kind_ok(name):
    bad = {"helper", "foreman", "laborer", "temp", "llc", "inc", "co", "electric", "plumbing", "hvac", "roofing",
           "concrete", "drywall", "painting", "construction", "services", "contractors"}
    return not bad & {t.casefold().strip(".") for t in name.split()}


def weeks_from(start, n):
    s = dt.date.fromisoformat(d(start))
    assert s.weekday() == 0
    return [[(s + dt.timedelta(7 * w + i)).isoformat() for i in range(5)] for w in range(n)]


def weekly_jobs(rng, names, jobs):
    """One week's crews: people shuffled and dealt round-robin over the jobs in a
    shuffled order, so every job gets at least two people (len(names) >= 2 * len(jobs))."""
    assert len(names) >= 2 * len(jobs)
    order, js = names[:], jobs[:]
    rng.shuffle(order)
    rng.shuffle(js)
    return {p: js[i % len(js)] for i, p in enumerate(order)}


def crews(assign):
    out = {}
    for p, j in assign.items():
        out.setdefault(j, []).append(p)
    return out


# ---------------------------------------------------------------- hand-written

def j01():
    rows = [("Mike Baker", [J] * 5)]
    lines = [["Name"] + [mdyyyy(x) for x in WEEK]] + [[p] + c for p, c in rows]
    exp = expected("J", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, WEEK), flags=[], info=[], must_not_ask=NO_ASK)
    emit("J01", "one-person-one-job",
         "Smallest real schedule: one person, one job, one week, all clean; nothing flagged.",
         {"input.csv": csv_bytes(lines)}, exp, {"window": WEEK})


def j02():
    pair = {J: "Mike Baker, Jose Garcia", S: "Tom Reed, Ana Ruiz", O: "Luis Ortiz, Dave Kim",
            P: "Carlos Mendez, Kevin Walsh", L: "Ray Jones, Brian Young"}
    rows = [(job, [cell] * 5) for job, cell in pair.items()]
    rows[0][1][1] = "Mike Baker, Jose Garcia, Luis Ortiz, Dave Kim"   # Tue: Oak Ridge crew helps at Johnson
    rows[2][1][1] = ""                                               # -> Oak Ridge unstaffed Tue
    rows[4][1][2] = "Ray Jones"                                      # Wed: Brian Young on nothing
    rows[4][1][3] = "Ray Jones, Brian Young, Kevin Walsh"            # Thu: Kevin also at Lincoln
    flags = [UF(O, "10-13"), NB("Brian Young", "10-14"), DB("Kevin Walsh", "10-15", P, L)]

    def make(path):
        wb = xlsx_workbook()
        ws = wb.active
        ws.title = "Week of 10-12"
        ws.cell(row=1, column=1, value="Job")
        for i, x in enumerate(WEEK):
            ws.cell(row=1, column=2 + i, value=pydate(x)).number_format = "m/d/yyyy"
        for r, (job, cells) in enumerate(rows, start=2):
            ws.cell(row=r, column=1, value=job)
            for i, cell in enumerate(cells):
                if cell:
                    ws.cell(row=r, column=2 + i, value=cell)
        wb.save(path)

    exp = expected("J", "input.xlsx", layout="job_by_day", arrangement="single",
                   assignments=job_grid(rows, WEEK), flags=flags, info=[],
                   must_not_ask=NO_ASK + [Q("pick_sheet")])
    emit("J02", "ten-people-five-jobs-one-week",
         "Ten people, five jobs, one week in a job x day grid (.xlsx), with one double-booking, one not-booked day and one unstaffed job-day planted.",
         {"input.xlsx": make}, exp, {"window": WEEK, "jobs": list(pair)})


# ---------------------------------------------------------------- generated

def j03():
    rng = random.Random(3003)
    names, jobs = people(60), SITES[:25]
    weeks = weeks_from("10-12", 4)
    dates = [x for w in weeks for x in w]
    cell = {p: {} for p in names}
    plan = []
    for w in weeks:
        a = weekly_jobs(rng, names, jobs)
        plan.append(a)
        for p, j in a.items():
            for x in w:
                cell[p][x] = j
    flags = []

    def pick(week, used):
        """People whose job that week has 3 or more, so 2 or more stay if one is taken away."""
        cr = crews(plan[week])
        return [p for j in jobs for p in sorted(cr.get(j, [])) if len(cr[j]) >= 3 and p not in used]

    used = set()
    # 1-2. double-booked: a second job in one cell
    for week, day in ((0, 2), (2, 4)):
        p = rng.choice(pick(week, used)); used.add(p)
        x = weeks[week][day]
        other = rng.choice([j for j in jobs if j != plan[week][p]])
        cell[p][x] = f"{plan[week][p]} / {other}"
        flags.append(DB(p, x, plan[week][p], other))
    # 3-4. not booked: a blank cell; the job keeps two people that day
    for week, day in ((1, 1), (3, 3)):
        p = rng.choice(pick(week, used)); used.add(p)
        x = weeks[week][day]
        cell[p][x] = ""
        flags.append(NB(p, x))
    # 5. vacation all of week 2 and 6. PTO one day of week 3: statuses, no flag
    p = rng.choice(pick(1, used)); used.add(p)
    for x in weeks[1]:
        cell[p][x] = "vac"
    p = rng.choice(pick(2, used)); used.add(p)
    cell[p][weeks[2][2]] = "PTO"
    # 7. unstaffed: the whole crew of one job sent to another job on Tuesday of week 4
    cr = crews(plan[3])
    job = rng.choice([j for j in jobs if not set(cr[j]) & used])
    dest = rng.choice([j for j in jobs if j != job])
    for p in cr[job]:
        used.add(p)
        cell[p][weeks[3][1]] = dest
    flags.append(UF(job, weeks[3][1]))

    rows = [(p, [cell[p][x] for x in dates]) for p in names]
    lines = [["Name"] + [mdyyyy(x) for x in dates]] + [[p] + c for p, c in rows]
    exp = expected("J", "input.csv", layout="person_by_day", arrangement="single",
                   assignments=person_grid(rows, dates), flags=flags, info=[], must_not_ask=NO_ASK)
    emit("J03", "sixty-people-25-jobs-4-weeks",
         "Typical-size file: 60 people x 25 jobs x 4 weeks in one person x day grid (20 weekday columns), with two double-bookings, two not-booked days and one unstaffed job-day planted; must read and analyze in under 200 ms.",
         {"input.csv": csv_bytes(lines)}, exp, {"window": dates})


def list_weeks(rng, names, jobs, weeks):
    """Clean long-list truth: {(date, person): [site]} plus each week's crews."""
    work, plan = {}, []
    for w in weeks:
        a = weekly_jobs(rng, names, jobs)
        plan.append(a)
        for p, j in a.items():
            for x in w:
                work[(x, p)] = [j]
    return work, plan


def list_rows(names, jobs, dates, work, status):
    """Rows sorted by date, then job order, then name; status rows after a date's work rows."""
    jix = {j: i for i, j in enumerate(jobs)}
    lines, assigns = [], []
    for x in dates:
        day = []
        for p in names:
            for j in work.get((x, p), []):
                day.append((jix[j], p, j))
        for _, p, j in sorted(day):
            lines.append([mdyyyy(x), p, j, ""])
            assigns.append(W(p, x, j))
        for p in names:
            if (x, p) in status:
                lines.append([mdyyyy(x), p, "", status[(x, p)]])
                assigns.append(ST(p, x, STATUS_TEXT[status[(x, p)].lower()]))
    return lines, assigns


def plant_list(rng, jobs, weeks, plan, work, status, spec):
    """spec: [(kind, week, day)]. Returns hand-spelled flags for each plant."""
    flags, used = [], set()

    def pick(week):
        """People whose job that week has 3 or more, so 2 or more stay if one is taken away."""
        cr = crews(plan[week])
        return [p for j in jobs for p in sorted(cr.get(j, [])) if len(cr[j]) >= 3 and p not in used]

    for kind_, week, day in spec:
        x = weeks[week][day]
        if kind_ == "double":
            p = rng.choice(pick(week)); used.add(p)
            other = rng.choice([j for j in jobs if j != plan[week][p]])
            work[(x, p)] = [plan[week][p], other]
            flags.append(DB(p, x, plan[week][p], other))
        elif kind_ == "pto_and_work":
            p = rng.choice(pick(week)); used.add(p)
            status[(x, p)] = "PTO"
            flags.append(AWO(p, x, [plan[week][p]], "pto"))
        elif kind_ == "missing":
            p = rng.choice(pick(week)); used.add(p)
            del work[(x, p)]
            flags.append(NB(p, x))
        elif kind_ == "sick":
            p = rng.choice(pick(week)); used.add(p)
            del work[(x, p)]
            status[(x, p)] = "sick"
        elif kind_ == "unstaffed":
            cr = crews(plan[week])
            job = rng.choice([j for j in jobs if not set(cr[j]) & used])
            dest = rng.choice([j for j in jobs if j != job])
            for p in cr[job]:
                used.add(p)
                work[(x, p)] = [dest]
            flags.append(UF(job, x))
        else:
            raise ValueError(kind_)
    return flags


def j04():
    rng = random.Random(4004)
    names, jobs = people(50), SITES[:20]
    weeks = weeks_from("10-12", 8)
    dates = [x for w in weeks for x in w]
    work, plan = list_weeks(rng, names, jobs, weeks)
    status = {}
    flags = plant_list(rng, jobs, weeks, plan, work, status, [
        ("double", 1, 1), ("double", 5, 3), ("pto_and_work", 4, 2),
        ("missing", 2, 0), ("missing", 6, 4), ("missing", 7, 2),
        ("sick", 3, 3), ("unstaffed", 5, 1)])
    lines, assigns = list_rows(names, jobs, dates, work, status)
    assert len(lines) == 2000, len(lines)
    exp = expected("J", "input.csv", layout="long_list", arrangement="single",
                   assignments=assigns, flags=flags, info=[], must_not_ask=NO_ASK)
    emit("J04", "two-thousand-row-list",
         "Long list of exactly 2,000 rows (50 people, 20 jobs, 8 weeks, Date/Name/Job/Status) with double-bookings, an assigned-while-off, not-booked days, a sick day and an unstaffed day planted.",
         {"input.csv": csv_bytes([["Date", "Name", "Job", "Status"]] + lines)}, exp)


def j05():
    rng = random.Random(5005)
    names, jobs = people(100), SITES[:25]
    weeks = weeks_from("10-12", 100)
    dates = [x for w in weeks for x in w]
    work, plan = list_weeks(rng, names, jobs, weeks)
    status = {}
    flags = plant_list(rng, jobs, weeks, plan, work, status, [
        ("double", 0, 3), ("pto_and_work", 1, 1), ("unstaffed", 1, 3),
        ("missing", 2, 2), ("missing", 3, 4)])
    lines, assigns = list_rows(names, jobs, dates, work, status)
    assert len(lines) == 50000, len(lines)
    first4 = set(x for w in weeks[:4] for x in w)
    truth = [a for a in assigns if a["date"] in first4]
    assert len(truth) == 2000 + 1 + 1 - 2
    sample = truth[:12]  # the first rows of the file
    for f in flags:  # the records behind each planted flag
        if "person" in f:
            sample += [a for a in truth if a.get("person") == f["person"] and a["date"] == f["date"] and a not in sample]
        else:
            sample += [a for a in truth if a.get("site") == f["site"] and a["date"] != f["date"]
                       and a["date"] in weeks[1] and a not in sample][:2]
    sample += [a for a in truth if a["date"] == weeks[3][4]][-6:]  # the last day of week 4
    exp = expected("J", "input.csv", layout="long_list", arrangement="single",
                   assignments=sample, assignments_exact=False, flags=flags, info=[],
                   must_not_ask=NO_ASK, notes=["first 4 weeks"])
    data = csv_bytes([["Date", "Name", "Job", "Status"]] + lines)
    assert len(data) < 8_000_000, len(data)
    emit("J05", "fifty-thousand-row-list",
         "Long list of 50,000 rows (100 people, 25 jobs, 100 weeks from 10/12/2026, sorted by date): over 20,000 rows, so only the first 4 weeks are read, the notes say so, and it never hangs; the planted flags are all in those 4 weeks.",
         {"input.csv": data}, exp, {"assignments": truth})


if __name__ == "__main__":
    check_names()
    for f in (j01, j02, j03, j04, j05):
        f()
    print("J done")
