"""C24 (Phase 7): an Orders sheet before the Crew Schedule. The other Phase 7
fixtures (C20-C23, C25, E15, E16) are hand-written CSVs.

Run: uv run -q --with openpyxl==3.1.5 python3 fixtures/handmade/_make/build_C24.py
"""
import json, datetime as dt, sys, os
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.chdir(os.path.dirname(HERE))  # fixtures/handmade
from pathlib import Path
from fxlib import normalize_zip, xlsx_workbook
out = "C24-workbook-orders-sheet-then-schedule"
wb = xlsx_workbook()
o = wb.active; o.title = "Orders"
o.append(["Item", "Qty", "For job", "Vendor", "Need by", "Ordered by"])
for item, q, job, v, d, who in [
    ("Treated 2x6x12", 36, "Pine Hollow deck", "Mason Lumber", 13, "Gil"),
    ("Hidden fasteners", 4, "Pine Hollow deck", "Mason Lumber", 13, "Gil"),
    ("Tile backer board", 20, "Reyes bath", "Coastline Tile", 14, "Tess"),
    ("Thinset (50 lb)", 6, "Reyes bath", "Coastline Tile", 14, "Tess"),
    ("Exterior paint, 5 gal", 3, "Lark St duplex", "Valley Paint", 15, "Abe"),
    ("Drop cloths", 10, "Lark St duplex", "Valley Paint", 15, "Abe"),
]:
    o.append([item, q, job, v, dt.datetime(2026, 10, d), who])
    o.cell(row=o.max_row, column=5).number_format = "m/d/yyyy"
s = wb.create_sheet("Crew Schedule")
days = [dt.date(2026, 10, 12) + dt.timedelta(i) for i in range(5)]
s.append(["Name"] + [f"{d:%a} {d.month}/{d.day}" for d in days])
grid = {"Gil Navarro": ["Pine Hollow deck"] * 5, "Tess Ruiz": ["Reyes bath"] * 4 + ["Shop"],
        "Abe Lindgren": ["Lark St duplex"] * 5, "Rosa Vega": ["Reyes bath", "Lark St duplex", "Lark St duplex", "Pine Hollow deck", "Pine Hollow deck"]}
for p, cells in grid.items():
    s.append([p] + cells)
wb.save(f"{out}/input.xlsx")
normalize_zip(Path(f"{out}/input.xlsx"))  # byte-identical across runs
asg = [{"person": p, "date": d.isoformat(), "site": c} for p, cells in grid.items() for d, c in zip(days, cells)]
e = {"category": "C", "tier": "script", "input": "input.xlsx", "paste": False,
     "settings": {"today": "2026-10-10", "workdays": ["mon", "tue", "wed", "thu", "fri"], "days_off": []},
     "layout": "person_by_day", "arrangement": "single", "assignments": asg, "assignments_exact": True,
     "flags": [], "flags_exact": True, "must_not_ask": [{"type": "pick_sheet"}, {"type": "pick_layout"}]}
json.dump(e, open(f"{out}/expected.json", "w"), indent=1)
open(f"{out}/README.md", "w").write("Workbook with an 'Orders' sheet first (Item, Qty, For job, Vendor, Need by, Ordered by: job names, dates and first names) and the 'Crew Schedule' second: the schedule must be picked without asking. Written in Phase 7 after the holdout showed the pattern.\n")
