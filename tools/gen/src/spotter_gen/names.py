"""Person names: picking canonical names from a spec, alias forms, typos, compatibility.

Name *strings* are always drawn from a dedicated name RNG (``Random(f"names:{seed}")`` or
``Random(f"rename:{seed}")``), never from the main RNG. The main RNG only draws the
structural ``NameSpec`` of each person, so the rename variant changes every string and
nothing else.
"""

from __future__ import annotations

import random
from dataclasses import dataclass
from functools import lru_cache

from .words import NICK_ANGLO, NICK_HISP, NICKNAMES, fold, has_accent, lists, norm_key, strip_accents


@dataclass
class NameSpec:
    origin: str  # "h" hispanic, "a" anglo, "o" other
    want_nick: bool = False  # first name has a spec-table nickname
    want_accent: bool = False  # canonical name has an accented letter
    compound: bool = False  # two surnames ("García López")
    group: int | None = None  # people sharing a first name share a group id


def _pool_first(origin: str) -> tuple[str, ...]:
    L = lists()
    return {"h": L["first_h"], "a": L["first_a"], "o": L["first_o"]}[origin]


def _pool_sur(origin: str) -> tuple[str, ...]:
    L = lists()
    return {"h": L["sur_h"], "a": L["sur_a"], "o": L["sur_o"]}[origin]


def materialize(specs: list[NameSpec], rng: random.Random, exclude_surnames: set[str] | None = None) -> list[tuple[str, str]]:
    """Return (first, last) per spec. Constraints:

    * different groups / ungrouped people have different first names (folded);
    * people in one group share the first name and have different last initials;
    * every surname token is unique in the company (and not in ``exclude_surnames``);
    * want_accent / want_nick / compound honoured when the pools allow.
    """
    exclude = {fold(s) for s in (exclude_surnames or set())}
    used_first: set[str] = set()
    used_sur: set[str] = set(exclude)
    group_first: dict[int, str] = {}
    group_initials: dict[int, set[str]] = {}
    out: list[tuple[str, str]] = []
    for sp in specs:
        # ---- first name
        if sp.group is not None and sp.group in group_first:
            first = group_first[sp.group]
        else:
            if sp.want_nick:
                pool = list(NICK_HISP if sp.origin == "h" else NICK_ANGLO)
                if sp.want_accent and sp.origin == "h":
                    acc = [f for f in pool if has_accent(f) and fold(f) not in used_first]
                    pool = acc or pool
            else:
                pool = list(_pool_first(sp.origin))
                if sp.want_accent and sp.origin == "h":
                    acc = [f for f in pool if has_accent(f)]
                    if rng.random() < 0.6:
                        pool = acc
            cands = [f for f in pool if fold(f) not in used_first]
            if not cands:
                cands = [f for f in _pool_first(sp.origin) + _pool_first("a") if fold(f) not in used_first]
            first = rng.choice(cands)
            used_first.add(fold(first))
            if sp.group is not None:
                group_first[sp.group] = first
        # ---- surname(s)
        taken_init = group_initials.setdefault(sp.group, set()) if sp.group is not None else set()
        pool_s = list(_pool_sur(sp.origin))
        if sp.want_accent and sp.origin == "h" and not has_accent(first):
            pool_s = [s for s in pool_s if has_accent(s)] or pool_s
        def ok(s: str) -> bool:
            k = fold(s)
            return k not in used_sur and fold(strip_accents(s)[0]) not in taken_init
        cands_s = [s for s in pool_s if ok(s)]
        if not cands_s:
            cands_s = [s for o in ("h", "a", "o") for s in _pool_sur(o) if ok(s)]
        last = rng.choice(cands_s)
        used_sur.add(fold(last))
        if sp.compound and sp.origin == "h":
            c2 = [s for s in _pool_sur("h") if fold(s) not in used_sur]
            if c2:
                second = rng.choice(c2)
                used_sur.add(fold(second))
                last = f"{last} {second}"
        if sp.group is not None:
            taken_init.add(fold(strip_accents(last)[0]))
        out.append((first, last))
    return out


# ----------------------------------------------------------------------------- alias forms

FORMS = ("FULL", "FIRST", "FIRST_LI", "FI_LAST", "LAST", "NICK_FULL", "NICK", "NICK_LI", "SHORT_COMPOUND")


def nick_of(first: str) -> str | None:
    n = NICKNAMES.get(first)
    return n[0] if n else None


def _init(s: str) -> str:
    return strip_accents(s)[0].upper()


def form(first: str, last: str, kind: str, dot: bool = False) -> str | None:
    """Render one alias form, or None when the form does not apply to this name."""
    nick = nick_of(first)
    li = _init(last) + ("." if dot else "")
    if kind == "FULL":
        return f"{first} {last}"
    if kind == "FIRST":
        return first
    if kind == "FIRST_LI":
        return f"{first} {li}"
    if kind == "FI_LAST":
        return f"{_init(first)}. {last}" if dot else f"{_init(first)} {last}"
    if kind == "LAST":
        return last
    if kind == "NICK_FULL":
        return f"{nick} {last}" if nick else None
    if kind == "NICK":
        return nick
    if kind == "NICK_LI":
        return f"{nick} {li}" if nick else None
    if kind == "SHORT_COMPOUND":
        parts = last.split()
        return f"{first} {parts[0]}" if len(parts) > 1 else None
    raise ValueError(kind)


def typo(s: str, rng: random.Random) -> str | None:
    """A believable typo in the first token: swap two inner letters or drop one."""
    tok = s.split(" ")
    w = tok[0]
    if len(w) < 4 or not w.isalpha():
        return None
    for _ in range(6):
        if rng.random() < 0.6:
            i = rng.randrange(1, len(w) - 1)
            if w[i] == w[i + 1]:
                continue
            w2 = w[:i] + w[i + 1] + w[i] + w[i + 2:]
        else:
            i = rng.randrange(1, len(w))
            w2 = w[:i] + w[i + 1:]
        if fold(w2) != fold(w):
            return " ".join([w2] + tok[1:])
    return None


# ----------------------------------------------------------------------------- compatibility

@lru_cache(maxsize=65536)
def compatible(alias: str, first: str, last: str) -> bool:
    """Could a deterministic matcher (normalization + spec nickname table + initials + token
    subset) read ``alias`` as this person? Used only to decide the expected tier."""
    a = norm_key(alias).split()
    if not a:
        return False
    firsts = {fold(first)} | {fold(n) for n in NICKNAMES.get(first, ())}
    # a nickname written as a first name also matches every formal name it abbreviates
    lasts = norm_key(last).split()
    if len(a) == 1:
        t = a[0]
        return t in firsts or t == lasts[0] or t == "".join(lasts)
    t0 = a[0]
    if not (t0 in firsts or (len(t0) == 1 and t0 == fold(first)[0])):
        return False
    j = 0
    for t in a[1:]:
        while j < len(lasts) and not (t == lasts[j] or (len(t) == 1 and t == lasts[j][0])):
            j += 1
        if j >= len(lasts):
            return False
        j += 1
    return True
