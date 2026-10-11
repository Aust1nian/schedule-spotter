"""Step 3 (corrupt) at the string level: alias palettes for people and sites, status words,
markers, notes and time text. Alias *choices* use the main RNG with a fixed number of draws
per slot; alias *strings* come from names (name RNG), so renaming changes only strings."""

from __future__ import annotations

import random

from .model import Truth
from .names import compatible, form, materialize, typo
from .plan import Plan
from .truth import SUFFIXES
from .words import PLACEHOLDERS, RESERVED, has_accent, norm_key, strip_accents


class Ref:
    """A reference to one alias of one identity; resolved to text after finalize()."""

    __slots__ = ("kind", "id", "idx")

    def __init__(self, kind: str, id: str, idx: int):
        self.kind, self.id, self.idx = kind, id, idx


class Txt(list):
    """Cell text made of fragments (str | Ref)."""


STATUS_WORDS_EN = {
    "off": ["off", "OFF", "Off", "day off"],
    "pto": ["PTO"],
    "vacation": ["vac", "vacation", "Vacation", "VAC"],
    "sick": ["sick", "Sick"],
    "holiday": ["holiday", "Holiday"],
    "out": ["out", "OUT", "Out"],
}
STATUS_WORDS_ES = {"off": ["descanso", "libre", "Descanso"], "vacation": ["vacaciones"], "sick": ["enfermo"]}
OFF_SYMBOLS = ["x", "X", "—", "-", "n/a"]
MARKERS = ["TBD", "open", "?", "need guy", "need 1", "Open", "OPEN", "Need guy"]
INACTIVE = ["x", "X", "—", "n/a", "closed", "done", "no work"]
NOTES = ["bring ladder", "call first", "gate code 4471", "pour day", "meet inspector", "finish trim",
         "punch list", "pick up materials", "dumpster swap", "final walkthrough", "bring the trailer",
         "lockbox on side door", "park on street", "bring generator", "drywall delivery"]

NICK_COUNTER = {"FULL": "NICK_FULL", "FIRST": "NICK", "FIRST_LI": "NICK_LI", "NICK": "FIRST",
                "NICK_LI": "FIRST_LI", "FI_LAST": "NICK_FULL", "LAST": "NICK_FULL"}


def _case(word: str, u: float) -> str:
    if u < 0.34:
        return word.lower()
    if u < 0.67:
        return word.upper()
    return word[:1].upper() + word[1:].lower()


def fmt_time12(t: str, style: str) -> str:
    h, m = int(t[:2]), int(t[3:])
    h12 = h % 12 or 12
    mm = f":{m:02d}" if m else ""
    ap = "a" if h < 12 else "p"
    if style == "plain":
        return f"{h12}{mm}"
    if style == "plain_full":
        return f"{h12}:{m:02d}"
    if style == "ap":
        return f"{h12}{mm}{ap}"
    if style == "ampm":
        return f"{h12}{mm}{ap}m"
    if style == "AMPM":
        return f"{h12}:{m:02d} {ap.upper()}M"
    raise ValueError(style)


def time_range_text(start: str, end: str, k: int) -> str:
    if k == 0:
        return f"{fmt_time12(start, 'plain')}-{fmt_time12(end, 'plain')}"
    if k == 1:
        return f"{fmt_time12(start, 'ap')}-{fmt_time12(end, 'ap')}"
    if k == 2:
        return f"{start}–{end}"
    if k == 3:
        return f"{fmt_time12(start, 'plain_full')}-{fmt_time12(end, 'plain_full')}"
    if k == 4:
        return f"{fmt_time12(start, 'ampm')}-{fmt_time12(end, 'ampm')}"
    return f"{start}-{end}"


def time_point_text(t: str, k: int) -> str:
    return fmt_time12(t, "AMPM") if k % 2 == 0 else t


class Presenter:
    def __init__(self, R: random.Random, T: Truth, P: Plan, name_rng: random.Random,
                 exclude_surnames: set[str] | None = None):
        self.R, self.T, self.P = R, T, P
        self.corruptions: list[dict] = []
        emps = [p for p in T.people if p.kind == "employee"]
        pairs = materialize([p.spec for p in emps], name_rng, exclude_surnames)
        for p, (f, l) in zip(emps, pairs):
            p.first, p.last, p.name = f, l, f"{f} {l}"
        self.emps = emps
        self.ppal: dict[str, list[tuple[str, str]]] = {}
        self._build_people(name_rng)
        self.spal: dict[str, list[tuple[str, str]]] = {}
        self._build_sites()
        self.memo: dict = {}
        self.used: dict = {}
        # file-level vocabularies
        c = P.corr
        es = P.lang == "es"
        self.status_base = {}
        for st, words in STATUS_WORDS_EN.items():
            pool = list(STATUS_WORDS_ES.get(st, [])) if es and st in STATUS_WORDS_ES else list(words)
            self.status_base[st] = pool[R.randrange(len(pool))]
        self.off_symbol = OFF_SYMBOLS[R.randrange(len(OFF_SYMBOLS))] if R.random() < 0.3 else None
        self.marker_base = [MARKERS[R.randrange(len(MARKERS))] for _ in range(2)]
        self.inactive_base = INACTIVE[R.randrange(len(INACTIVE))]
        self.status_case = c["status_case"]
        self.notes_on = c["notes"]

    # ------------------------------------------------------------------ palettes
    def _build_people(self, nrng: random.Random) -> None:
        P, R, c = self.P, self.R, self.P.corr
        style = P.name_style
        for p in self.T.people:
            if p.kind != "employee":
                self.ppal[p.id] = [("FIXED", p.name)]
                continue
            grp = p.spec.group is not None
            nickable = form(p.first, p.last, "NICK") is not None
            prim = style
            if style in ("FIRST", "NICK") and grp:
                prim = "NICK_LI" if (style == "NICK" and nickable) else "FIRST_LI"
            elif style == "NICK":
                prim = "NICK" if nickable else "FIRST"
            kinds = [prim]
            u = [R.random() for _ in range(8)]
            if c["nickname"] and p.spec.want_nick and u[0] < 0.6:
                kinds.append(NICK_COUNTER[prim])
            if c["initials"] and u[1] < 0.35:
                kinds.append({"FULL": "FIRST_LI" if u[2] < 0.6 else "FI_LAST", "FIRST": "FIRST_LI",
                              "NICK": "NICK_LI", "FIRST_LI": "FULL", "NICK_LI": "NICK_FULL",
                              "FI_LAST": "FULL", "LAST": "FI_LAST"}[prim])
            if c["partial_name"] and u[3] < 0.3:
                kinds.append({"FULL": "FIRST" if u[2] < 0.6 else "LAST", "FIRST": "FULL", "FIRST_LI": "FULL",
                              "NICK": "NICK_FULL", "NICK_LI": "NICK_FULL", "FI_LAST": "LAST", "LAST": "FULL"}[prim])
            if c["name_case"] and u[4] < 0.3:
                kinds.append("UPPER" if u[2] < 0.5 else "LOWER")
            if c["drop_accents"] and p.spec.want_accent and u[5] < 0.6:
                kinds.append("NOACCENT")
            if c["typo"] and u[6] < 0.25:
                kinds.append("TYPO")
            if p.spec.compound and u[7] < 0.5:
                kinds.append("SHORT_COMPOUND")
            prim_s = form(p.first, p.last, prim, P.li_dot)
            pal = [(prim, prim_s)]
            for k in kinds[1:]:
                pal.append((k, self._alias_string(p, k, prim, prim_s, nrng) or prim_s))
            self.ppal[p.id] = pal

    def _alias_string(self, p, k, prim, prim_s, nrng):
        f, l, dot = p.first, p.last, self.P.li_dot
        if k == "UPPER":
            return prim_s.upper()
        if k == "LOWER":
            return prim_s.lower()
        if k == "NOACCENT":
            for base in ("FULL", "FIRST_LI", "FIRST", "FI_LAST", "LAST"):
                s = form(f, l, base, dot)
                if s and has_accent(s) and len(s) != len(prim_s):
                    return strip_accents(s)
            return None
        if k == "TYPO":
            if len(prim_s.split()) >= 2:
                return typo(f, nrng)
            return _deletion_typo(prim_s, nrng)
        return form(f, l, k, dot)

    def _build_sites(self) -> None:
        R, c = self.R, self.P.corr
        for s in self.T.sites:
            kinds = ["CANON"]
            u = [R.random() for _ in range(5)]
            if c["site_variant"] and s.kind == "address" and u[0] < 0.6:
                kinds.append("SUFFIX_SWAP" if u[1] < 0.6 else "SUFFIX_DOT")
            if c["site_case"] and u[2] < 0.4:
                kinds.append("SUPPER" if u[4] < 0.5 else "SLOWER")
            if c["site_hard"] and u[3] < 0.3 and not s.internal:
                kinds.append("HARD")
            pal = [("CANON", s.name)]
            for k in kinds[1:]:
                pal.append((k, _site_variant(s, k, u[4]) or s.name))
            self.spal[s.id] = pal

    # ------------------------------------------------------------------ slot choices
    def _pick(self, n: int, keep: float) -> int:
        u = self.R.random()
        if n == 1 or u < keep:
            return 0
        return 1 + min(n - 2, int((u - keep) / (1 - keep) * (n - 1)))

    def person(self, pid: str, slot=None) -> Ref:
        if slot is not None and ("p", pid, slot) in self.memo:
            return self.memo[("p", pid, slot)]
        ref = Ref("p", pid, self._pick(len(self.ppal[pid]), 0.72))
        self.used.setdefault(("p", pid), set()).add(ref.idx)
        if slot is not None:
            self.memo[("p", pid, slot)] = ref
        return ref

    def site(self, sid: str, slot=None) -> Ref:
        if slot is not None and ("s", sid, slot) in self.memo:
            return self.memo[("s", sid, slot)]
        ref = Ref("s", sid, self._pick(len(self.spal[sid]), 0.75))
        self.used.setdefault(("s", sid), set()).add(ref.idx)
        if slot is not None:
            self.memo[("s", sid, slot)] = ref
        return ref

    def status_word(self, status: str, alone: bool = True, symbols_ok: bool = False) -> str:
        u = self.R.random()
        if status == "off" and symbols_ok and alone and self.off_symbol:
            return self.off_symbol
        w = self.status_base[status]
        return _case(w, u) if self.status_case else w

    def marker(self) -> str:
        u = self.R.random()
        w = self.marker_base[0 if u < 0.7 else 1]
        if self.status_case and w.isalpha():
            return _case(w, self.R.random())
        return w

    def inactive(self) -> str:
        return self.inactive_base

    def maybe_note(self) -> str | None:
        u = self.R.random()
        if self.notes_on and u < 0.2:
            return NOTES[int(u / 0.2 * len(NOTES)) % len(NOTES)]
        return None

    # ------------------------------------------------------------------ finalize
    def finalize(self, extra_reserved: list[str]) -> None:
        T = self.T
        self.final: dict = {}
        reserved = set(RESERVED) | {norm_key(x) for x in extra_reserved} | {norm_key(x) for x in PLACEHOLDERS}
        site_keys = {norm_key(s) for pal in self.spal.values() for _, s in pal}
        owner: dict = {}
        # people: primaries first, then secondaries
        for pass_no in (0, 1):
            for p in T.people:
                used = sorted(self.used.get(("p", p.id), ()))
                for idx in used:
                    if (idx == 0) != (pass_no == 0):
                        continue
                    s = self.ppal[p.id][idx][1]
                    if p.kind == "employee":
                        for cand in (s, self.ppal[p.id][0][1], p.name):
                            k = norm_key(cand)
                            if owner.get(k, p.id) == p.id and k not in reserved and k not in site_keys:
                                s = cand
                                break
                        else:
                            s = p.name
                    owner[norm_key(s)] = p.id
                    self.final[("p", p.id, idx)] = s
        sowner: dict = {}
        for s in T.sites:
            for idx in sorted(self.used.get(("s", s.id), ())):
                txt = self.spal[s.id][idx][1]
                k = norm_key(txt)
                if sowner.get(k, s.id) != s.id or k in owner:
                    txt = s.name
                sowner[norm_key(txt)] = s.id
                self.final[("s", s.id, idx)] = txt
        # ties at the longest length -> fold onto one spelling
        self.display: dict = {}
        self.rendered: dict = {}
        for kind, ents in (("p", T.people), ("s", T.sites)):
            for e in ents:
                idxs = sorted(self.used.get((kind, e.id), ()))
                if not idxs:
                    continue
                groups: dict = {}
                for i in idxs:
                    groups.setdefault(self.final[(kind, e.id, i)].casefold(), []).append(i)
                L = max(len(k) for k in groups)
                tied = [k for k in groups if len(k) == L]
                if len(tied) > 1:
                    prim_key = self.final[(kind, e.id, idxs[0])].casefold()
                    win = prim_key if prim_key in tied else min(tied)
                    rep = self.final[(kind, e.id, groups[win][0])]
                    for k in tied:
                        if k != win:
                            for i in groups.pop(k):
                                self.final[(kind, e.id, i)] = rep
                                groups[win].append(i)
                    self.corruptions.append({"type": "tie_fix", "kind": kind, "id": e.id, "kept": rep})
                L = max(len(k) for k in groups)
                win = next(k for k in groups if len(k) == L)
                self.display[(kind, e.id)] = self.final[(kind, e.id, min(groups[win]))]
                seen = []
                for i in idxs:
                    t = self.final[(kind, e.id, i)]
                    if t not in seen:
                        seen.append(t)
                self.rendered[(kind, e.id)] = seen
        self._tier()

    def text(self, ref: Ref) -> str:
        return self.final[(ref.kind, ref.id, ref.idx)]

    def _tier(self) -> None:
        reasons: list[str] = []
        emps = self.emps
        for p in self.T.people:
            if p.kind != "employee":
                continue
            prim = self.ppal[p.id][0][1]
            for idx in sorted(self.used.get(("p", p.id), ())):
                kind = self.ppal[p.id][idx][0]
                txt = self.final[("p", p.id, idx)]
                if kind == "TYPO" and txt != prim and norm_key(txt) != norm_key(p.name):
                    reasons.append(f"typo:{txt}")
                n = sum(1 for e in emps if compatible(txt, e.first, e.last))
                if n >= 2:
                    reasons.append(f"ambiguous:{txt}")
        for s in self.T.sites:
            for idx in sorted(self.used.get(("s", s.id), ())):
                if self.spal[s.id][idx][0] == "HARD" and self.final[("s", s.id, idx)] != s.name:
                    reasons.append(f"site_hard:{self.final[('s', s.id, idx)]}")
            if s.parts.get("lookalike_of") and ("s", s.parts["lookalike_of"]) in self.used and ("s", s.id) in self.used:
                reasons.append(f"lookalike:{s.name}")
        self.tier_reasons = sorted(set(reasons))
        self.tier = "model" if reasons else "script"


def _deletion_typo(s: str, rng: random.Random) -> str | None:
    w = s.split(" ")[0]
    if len(w) < 4:
        return None
    i = rng.randrange(1, len(w))
    return " ".join([w[:i] + w[i + 1:]] + s.split(" ")[1:])


def _site_variant(s, k: str, u: float) -> str | None:
    if k == "SUPPER":
        return s.name.upper()
    if k == "SLOWER":
        return s.name.lower()
    if s.kind == "address":
        p = s.parts
        head = f"{p['num']} {p['dir'] + ' ' if p['dir'] else ''}{p['street']}"
        if k == "SUFFIX_SWAP":
            if p["suffix"] == "Way":
                return None
            return f"{head} {p['suffix'] if p['full'] else SUFFIXES[p['suffix']]}"
        if k == "SUFFIX_DOT":
            if p["suffix"] == "Way":
                return None
            return f"{head} {p['suffix']}."
        if k == "HARD":
            return f"{p['street']} {p['suffix']}" if u < 0.5 else f"{p['street']} job"
    if k == "HARD":
        if s.kind == "customer":
            c = s.parts["customer"]
            return f"{c} job" if not s.name.endswith(" job") else c
        if s.kind == "jobno":
            return s.parts["place"] if u < 0.5 else s.parts["num"]
        if s.kind == "commercial" and s.parts.get("place"):
            return s.parts["place"]
    return None
