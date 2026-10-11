#!/usr/bin/env python3
"""Validate the Schedule Spotter model-layer benchmark question set.

Checks: per-task minimum counts, schema and allowed labels, unique ids, that at least a
third of T1+T2 are "different", and the T5 guard rule (every person and site in a T5 label
appears in the pasted line after normalization). Exits non-zero on any failure.

Usage: python3 validate.py [dir]   (defaults to the directory this file lives in)
"""
import json
import os
import re
import sys
import unicodedata
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
MIN = {"t1": 150, "t2": 100, "t3": 100, "t4": 50, "t5": 60}
FILES = {
    "t1": "t1_same_person.jsonl",
    "t2": "t2_same_site.jsonl",
    "t3": "t3_cell_meaning.jsonl",
    "t4": "t4_column_role.jsonl",
    "t5": "t5_free_text.jsonl",
}
PAIR_LABELS = {"same", "different"}
T3_LABELS = {"person", "site", "status", "unstaffed", "note", "time", "empty"}
T3_ROLES = {"person", "site", "date", "day", "time", "status", "note"}
T4_LABELS = {"person", "site", "date", "day", "time", "status", "note", "ignore"}
DAYS = {"mon", "tue", "wed", "thu", "fri", "sat", "sun"}
TIME_RE = re.compile(r"^([01]\d|2[0-3]):[0-5]\d-([01]\d|2[0-3]):[0-5]\d$")
ID_RE = re.compile(r"^(t[1-5])-\d{3}$")

errors = []


def err(where, msg):
    errors.append(f"{where}: {msg}")


def norm(s):
    """Case, accents, punctuation and spacing folded away (same idea as the app's T5 guard)."""
    s = unicodedata.normalize("NFKD", s)
    s = "".join(c for c in s if not unicodedata.combining(c)).lower()
    return " ".join(re.sub(r"[^a-z0-9]+", " ", s).split())


def appears_in(value, line):
    v = norm(value)
    return bool(v) and f" {v} " in f" {norm(line)} "


def is_str_list(x, n_min=0, n_max=None):
    return isinstance(x, list) and all(isinstance(v, str) for v in x) and len(x) >= n_min and (
        n_max is None or len(x) <= n_max)


def need(item, where, keys):
    ok = True
    for k, typ in keys.items():
        if k not in item:
            err(where, f"missing key '{k}'")
            ok = False
        elif typ is not None and not isinstance(item[k], typ):
            err(where, f"key '{k}' has wrong type")
            ok = False
    return ok


def check_pair(task, item, where, list_key, list_max):
    if not need(item, where, {"a": str, "b": str, list_key: list, "label": str, "note": str}):
        return
    if item["label"] not in PAIR_LABELS:
        err(where, f"label {item['label']!r} not in {sorted(PAIR_LABELS)}")
    if not item["a"].strip() or not item["b"].strip():
        err(where, "empty a or b")
    if norm(item["a"]) == norm(item["b"]):
        err(where, "a and b are identical after normalization (rules would settle it)")
    if not is_str_list(item[list_key], 0, list_max):
        err(where, f"'{list_key}' must be a list of at most {list_max} strings")
    else:
        nab = {norm(item["a"]), norm(item["b"])}
        for r in item[list_key]:
            if norm(r) in nab:
                err(where, f"'{list_key}' repeats a or b: {r!r}")
    if not item["note"].strip():
        err(where, "empty note (labels must carry their reasoning)")


def check_t3(item, where):
    if not need(item, where, {"text": str, "column_role": str, "neighbors": list, "label": str}):
        return
    if item["column_role"] not in T3_ROLES:
        err(where, f"column_role {item['column_role']!r} not in {sorted(T3_ROLES)}")
    if not is_str_list(item["neighbors"], 3, 3):
        err(where, "neighbors must be exactly 3 strings")
    if item["label"] not in T3_LABELS:
        err(where, f"label {item['label']!r} not in {sorted(T3_LABELS)}")


def check_t4(item, where):
    if not need(item, where, {"header": str, "samples": list, "label": str}):
        return
    if not is_str_list(item["samples"], 8, 8):
        err(where, "samples must be exactly 8 strings")
    if item["label"] not in T4_LABELS:
        err(where, f"label {item['label']!r} not in {sorted(T4_LABELS)}")


def check_t5(item, where):
    if not need(item, where, {"line": str, "roster": list, "sites": list, "week_of": str}):
        return
    if "label" not in item:
        err(where, "missing key 'label'")
        return
    if not re.match(r"^\d{4}-\d{2}-\d{2}$", item["week_of"]):
        err(where, "week_of must be YYYY-MM-DD")
    if not is_str_list(item["roster"]) or not is_str_list(item["sites"]):
        err(where, "roster and sites must be lists of strings")
    lab = item["label"]
    if lab == "cannot read":
        return
    if not isinstance(lab, list) or not lab:
        err(where, "label must be 'cannot read' or a non-empty list of assignments")
        return
    seen = set()
    for j, a in enumerate(lab):
        w = f"{where} label[{j}]"
        if not isinstance(a, dict) or set(a) != {"person", "day", "site", "time"}:
            err(w, "assignment must have exactly person, day, site, time")
            continue
        if not isinstance(a["person"], str) or not isinstance(a["site"], str):
            err(w, "person and site must be strings")
            continue
        if a["day"] not in DAYS:
            err(w, f"day {a['day']!r} not in {sorted(DAYS)}")
        if a["time"] is not None and not (isinstance(a["time"], str) and TIME_RE.match(a["time"])):
            err(w, f"time {a['time']!r} must be null or HH:MM-HH:MM")
        # T5 guard: every person and site must appear in the line after normalization.
        if not appears_in(a["person"], item["line"]):
            err(w, f"person {a['person']!r} does not appear in the line (T5 guard)")
        if not appears_in(a["site"], item["line"]):
            err(w, f"site {a['site']!r} does not appear in the line (T5 guard)")
        key = (norm(a["person"]), a["day"], norm(a["site"]), a["time"])
        if key in seen:
            err(w, "duplicate assignment")
        seen.add(key)


def main():
    d = sys.argv[1] if len(sys.argv) > 1 else HERE
    data = {}
    ids = Counter()
    for task, fname in FILES.items():
        path = os.path.join(d, fname)
        if not os.path.exists(path):
            err(fname, "file missing")
            data[task] = []
            continue
        items = []
        with open(path, encoding="utf-8") as f:
            for ln, raw in enumerate(f, 1):
                if not raw.strip():
                    continue
                try:
                    items.append(json.loads(raw))
                except json.JSONDecodeError as e:
                    err(f"{fname}:{ln}", f"bad JSON: {e}")
        data[task] = items
        for k, it in enumerate(items, 1):
            where = f"{fname}:{k}"
            if not isinstance(it, dict):
                err(where, "not an object")
                continue
            iid = it.get("id")
            m = ID_RE.match(iid) if isinstance(iid, str) else None
            if not m or m.group(1) != task:
                err(where, f"id {iid!r} must look like {task}-NNN")
            ids[iid] += 1
            where = f"{fname}:{iid}"
            if task == "t1":
                check_pair(task, it, where, "roster", 10)
            elif task == "t2":
                check_pair(task, it, where, "sites", 12)
            elif task == "t3":
                check_t3(it, where)
            elif task == "t4":
                check_t4(it, where)
            else:
                check_t5(it, where)
        if len(items) < MIN[task]:
            err(fname, f"{len(items)} items, need at least {MIN[task]}")

    for iid, n in ids.items():
        if n > 1:
            err("ids", f"{iid!r} used {n} times")

    pairs = data["t1"] + data["t2"]
    diff = [p for p in pairs if p.get("label") == "different"]
    if pairs and len(diff) * 3 < len(pairs):
        err("t1+t2", f"only {len(diff)}/{len(pairs)} are 'different'; need at least a third")

    # Report
    print("counts:", ", ".join(f"{t}={len(data[t])}" for t in FILES))
    for t in ("t1", "t2"):
        c = Counter(i.get("label") for i in data[t])
        look = sum(1 for i in data[t] if i.get("label") == "different"
                   and i.get("kind") not in ("control_never_asked", "injection"))
        print(f"{t}: same={c['same']} different={c['different']} (look-alike, rules would ask: {look})")
    print(f"t1+t2 different share: {len(diff)}/{len(pairs)} = {len(diff) / max(1, len(pairs)):.1%}")
    for t in ("t3", "t4"):
        print(f"{t} labels:", dict(sorted(Counter(i.get("label") for i in data[t]).items())))
    cr = sum(1 for i in data["t5"] if i.get("label") == "cannot read")
    print(f"t5: readable={len(data['t5']) - cr} cannot_read={cr}")
    inj = Counter(t for t in FILES for i in data[t] if i.get("kind") == "injection")
    print("prompt-injection items:", dict(inj), "total", sum(inj.values()))

    if errors:
        print(f"\nFAIL: {len(errors)} problem(s)")
        for e in errors:
            print("  -", e)
        return 1
    print("\nOK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
