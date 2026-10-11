"""No prompt example may also be a benchmark question: a model that has
seen the answer in its prompt would score itself. Compares each example's
distinctive fields (names, sites, cell, header + samples, line) with every
question in bench/questions, after folding case and punctuation."""
import glob
import json
import re
import sys

FIELDS = ("a", "b", "cell", "header", "samples", "line")
ALIASES = {"text": "cell"}  # bench T3 calls the cell "text"


def fold(v):
    if isinstance(v, list):
        return tuple(fold(x) for x in v)
    return re.sub(r"[^a-z0-9]+", " ", str(v).lower()).strip()


def signature(q):
    q = {ALIASES.get(k, k): v for k, v in q.items()}
    return tuple((k, fold(q[k])) for k in FIELDS if k in q)


questions = {}
for path in glob.glob("bench/questions/*.jsonl"):
    for line in open(path, encoding="utf-8"):
        if line.strip():
            q = json.loads(line)
            questions.setdefault(signature(q), []).append(q["id"])

leaks = 0
for path in sorted(glob.glob("llm/prompts/*/*.json")):
    for user, _ in json.load(open(path, encoding="utf-8"))["examples"]:
        sig = signature(json.loads(user))
        if sig in questions:
            leaks += 1
            print(f"{path}: example is benchmark question {', '.join(questions[sig])}")
        line = dict(sig).get("line")
        for other, ids in questions.items():
            if line and dict(other).get("line") == line and other != sig:
                leaks += 1
                print(f"{path}: example line is benchmark question {', '.join(ids)}")
print(f"prompt leak check: {leaks} leak(s)")
sys.exit(1 if leaks else 0)
