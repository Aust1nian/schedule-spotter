"""Word lists (package data files) and small text helpers shared by the generator."""

from __future__ import annotations

import unicodedata
from functools import lru_cache
from pathlib import Path

DATA = Path(__file__).parent / "data"

# Spec stage 7 nickname table, formal name -> nicknames. Only these pairs are used
# for "nickname" corruptions, so a script that knows the spec's table can resolve them.
NICKNAMES: dict[str, tuple[str, ...]] = {
    "Michael": ("Mike",),
    "Robert": ("Bob",),
    "William": ("Bill",),
    "James": ("Jim",),
    "Anthony": ("Tony",),
    "José": ("Pepe",),
    "Francisco": ("Paco",),
    "Jesús": ("Chuy",),
    "Ignacio": ("Nacho",),
    "Eduardo": ("Lalo",),
    "Guillermo": ("Memo",),
    "Alberto": ("Beto",),
    "Roberto": ("Beto",),
}
NICK_HISP = ("José", "Francisco", "Jesús", "Ignacio", "Eduardo", "Guillermo", "Alberto", "Roberto")
NICK_ANGLO = ("Michael", "Robert", "William", "James", "Anthony")

PLACEHOLDERS = ("Helper 1", "Helper 2", "Laborer", "Temp", "New guy", "Foreman")

# Words a person alias must never equal (statuses, markers, header words, internal sites).
RESERVED = {
    "off", "pto", "vac", "vacation", "sick", "holiday", "out", "x", "n/a", "day off",
    "descanso", "vacaciones", "enfermo", "libre", "tbd", "open", "?", "need guy", "need 1",
    "shop", "yard", "office", "warehouse", "total", "name", "employee", "tech", "crew",
    "worker", "job", "site", "project", "address", "customer", "date", "day", "time",
    "status", "roster", "team", "closed", "done", "no work", "am", "pm", "half day",
}


@lru_cache(maxsize=65536)
def fold(s: str) -> str:
    """Case-fold and strip accents (for collision / compatibility checks)."""
    nfkd = unicodedata.normalize("NFKD", s)
    return "".join(ch for ch in nfkd if not unicodedata.combining(ch)).casefold()


@lru_cache(maxsize=65536)
def strip_accents(s: str) -> str:
    nfkd = unicodedata.normalize("NFKD", s)
    return unicodedata.normalize("NFC", "".join(ch for ch in nfkd if not unicodedata.combining(ch)))


def has_accent(s: str) -> bool:
    return strip_accents(s) != s


@lru_cache(maxsize=65536)
def norm_key(s: str) -> str:
    """Normalized identity key used for collision checks: fold, drop periods, collapse spaces."""
    return " ".join(fold(s).replace(".", " ").split())


def _load(name: str) -> list[str]:
    out: list[str] = []
    seen: set[str] = set()
    for line in (DATA / name).read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        line = unicodedata.normalize("NFC", line)
        k = fold(line)
        if k in seen:
            continue
        seen.add(k)
        out.append(line)
    return out


@lru_cache(maxsize=None)
def lists() -> dict[str, tuple[str, ...]]:
    first = {o: _load(f"first_{o}.txt") for o in ("hispanic", "anglo", "other")}
    nick_vals = {fold(n) for ns in NICKNAMES.values() for n in ns}
    all_first: set[str] = set()
    for o in first:
        first[o] = [f for f in first[o] if fold(f) not in nick_vals and fold(f) not in all_first]
        all_first.update(fold(f) for f in first[o])
    sur = {o: _load(f"surnames_{o}.txt") for o in ("hispanic", "anglo", "other")}
    customers = _load("customers.txt")
    streets = _load("streets.txt")
    stems = _load("sub_stems.txt")
    banned = all_first | {fold(c) for c in customers} | {fold(s) for s in streets} | {fold(s) for s in stems}
    seen_s: set[str] = set()
    for o in sur:
        keep = []
        for s in sur[o]:
            k = fold(s)
            if k in banned or k in seen_s:
                continue
            seen_s.add(k)
            keep.append(s)
        sur[o] = keep
    return {
        "first_h": tuple(first["hispanic"]),
        "first_a": tuple(first["anglo"]),
        "first_o": tuple(first["other"]),
        "sur_h": tuple(sur["hispanic"]),
        "sur_a": tuple(sur["anglo"]),
        "sur_o": tuple(sur["other"]),
        "customers": tuple(customers),
        "streets": tuple(streets),
        "places": tuple(_load("places.txt")),
        "commercial": tuple(_load("commercial.txt")),
        "stems": tuple(stems),
        "trades": tuple(_load("sub_trades.txt")),
    }
