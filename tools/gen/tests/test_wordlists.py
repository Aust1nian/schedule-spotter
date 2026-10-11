"""Word-list hygiene: sizes, uncommon-name share, cp1252-safe, no separator characters."""

from spotter_gen.words import NICKNAMES, fold, lists

SEPARATORS = (",", "&", "/", "+", " - ", "(", ")", "\n")


def test_sizes_and_mix():
    L = lists()
    first = L["first_h"] + L["first_a"] + L["first_o"]
    sur = L["sur_h"] + L["sur_a"] + L["sur_o"]
    assert len(first) >= 290 and len(sur) >= 290
    assert len(L["first_h"]) >= 120  # many Hispanic first names
    assert 0.08 <= (len(L["first_o"]) + len(L["sur_o"])) / (len(first) + len(sur)) <= 0.2
    assert len({fold(x) for x in first}) == len(first)
    assert not {fold(x) for x in sur} & {fold(x) for x in first}
    assert not {fold(n) for ns in NICKNAMES.values() for n in ns} & {fold(x) for x in first}


def test_all_words_cp1252_and_separator_free():
    L = lists()
    for key, words in L.items():
        for w in words:
            w.encode("cp1252")
            assert not any(s in w for s in SEPARATORS), (key, w)
            assert " and " not in f" {w.casefold()} " and " y " not in f" {w.casefold()} ", (key, w)
