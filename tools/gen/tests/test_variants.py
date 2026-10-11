"""Variants: flags never change (names change under rename)."""

from spotter_gen.gen import generate

SEEDS = range(0, 150)


def id_items(case):
    t = case.truth
    pm = {p["display"]: p["id"] for p in t["people"] if p["display"]}
    sm = {s["display"]: s["id"] for s in t["sites"] if s["display"]}
    out = []
    for f in case.expected["flags"] + case.expected["info"]:
        out.append((f["type"], pm.get(f.get("person")), sm.get(f.get("site")), f["date"],
                    tuple(sorted(sm[s] for s in f.get("sites", []))), f.get("status")))
    return sorted(out, key=repr)


def test_variants_keep_flags():
    applied = {"shuffle": 0, "blank": 0, "transpose": 0, "rename": 0}
    for s in SEEDS:
        base = generate(s)
        for v in applied:
            c = generate(s, v)
            applied[v] += c.truth["variant_applied"]
            if v in ("shuffle", "blank"):
                assert c.expected == base.expected, (s, v)
            elif v == "transpose":
                e0, e1 = dict(base.expected), dict(c.expected)
                l0, l1 = e0.pop("layout"), e1.pop("layout")
                assert e0 == e1, s
                if c.truth["variant_applied"]:
                    assert {l0, l1} in ({"person_by_day", "day_by_person"}, {"job_by_day", "day_by_job"})
            else:
                assert id_items(c) == id_items(base), s
                old = {p["name"] for p in base.truth["people"] if p["kind"] == "employee"}
                new = {p["name"] for p in c.truth["people"] if p["kind"] == "employee"}
                assert not old & new, s
            if c.truth["variant_applied"] and v != "rename":
                assert c.files[c.expected["input"]] != base.files[base.expected["input"]], (s, v)
    assert all(n > 20 for n in applied.values()), applied
