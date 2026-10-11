"""Blocks and bands: the 2-D structure every layout renders into, before it becomes a file."""

from __future__ import annotations

import random
from dataclasses import dataclass, field


@dataclass
class Block:
    rows: list[list]
    merges: list[tuple[int, int, int, int]] = field(default_factory=list)  # r1, c1, r2, c2 inclusive
    role: str = "table"  # title | table | footer | roster | crews
    breakable: list[int] = field(default_factory=list)  # a blank row may be inserted before row i

    @property
    def height(self) -> int:
        return len(self.rows)

    @property
    def width(self) -> int:
        return max((len(r) for r in self.rows), default=0)

    def transpose(self) -> "Block":
        h, w = self.height, self.width
        grid = [[(self.rows[r][c] if c < len(self.rows[r]) else None) for r in range(h)] for c in range(w)]
        return Block(grid, [(c1, r1, c2, r2) for (r1, c1, r2, c2) in self.merges], self.role, [])

    def insert_blank_row(self, i: int) -> None:
        self.rows.insert(i, [])
        self.merges = [(r1 + (r1 >= i), c1, r2 + (r2 >= i), c2) for (r1, c1, r2, c2) in self.merges]
        self.breakable = [b + (b > i) for b in self.breakable if b != i] + [i, i + 1]
        self.breakable = sorted(set(self.breakable))


@dataclass
class Band:
    blocks: list[Block]
    gap_before: int = 0
    col_gaps: list[int] = field(default_factory=list)

    def __post_init__(self):
        if not self.col_gaps:
            self.col_gaps = [0] + [1] * (len(self.blocks) - 1)


def text_band(lines: list[str], role: str, gap: int = 0) -> Band:
    return Band([Block([[ln] for ln in lines], role=role)], gap_before=gap)


@dataclass
class Canvas:
    cells: dict  # (r, c) -> value
    merges: list[tuple[int, int, int, int]]
    n_rows: int
    n_cols: int


def compose(bands: list[Band]) -> Canvas:
    cells: dict = {}
    merges: list = []
    r = 0
    ncols = 0
    for band in bands:
        r += band.gap_before
        c = 0
        h = 0
        for k, blk in enumerate(band.blocks):
            c += band.col_gaps[k]
            for i, row in enumerate(blk.rows):
                for j, v in enumerate(row):
                    if v is not None and v != "":
                        cells[(r + i, c + j)] = v
            merges += [(r + r1, c + c1, r + r2, c + c2) for (r1, c1, r2, c2) in blk.merges]
            c += blk.width
            h = max(h, blk.height)
        ncols = max(ncols, c)
        r += h
    return Canvas(cells, merges, r, ncols)


def insert_blanks(bands: list[Band], rng: random.Random) -> list[dict]:
    """Variant "blank": add blank rows between bands or at breakable rows inside table blocks,
    and blank columns at the left margin or between side-by-side blocks. Never inside a row."""
    log: list[dict] = []
    n_rows = rng.randint(1, 3)
    for _ in range(n_rows):
        spots = [("band", bi, None) for bi in range(len(bands) + 1)]
        for bi, band in enumerate(bands):
            if len(band.blocks) == 1 and band.blocks[0].role == "table":
                spots += [("inner", bi, i) for i in band.blocks[0].breakable]
        kind, bi, i = spots[rng.randrange(len(spots))]
        if kind == "band":
            if bi == len(bands):
                bands.append(Band([Block([[]], role="blank")]))
            else:
                bands[bi].gap_before += 1
            log.append({"blank_row": "between_bands", "band": bi})
        else:
            bands[bi].blocks[0].insert_blank_row(i)
            log.append({"blank_row": "inside_table", "band": bi, "row": i})
    n_cols = rng.randint(1, 2)
    for _ in range(n_cols):
        spots = [(bi, k) for bi, band in enumerate(bands) for k in range(len(band.blocks)) if k > 0]
        if spots and rng.random() < 0.5:
            bi, k = spots[rng.randrange(len(spots))]
            bands[bi].col_gaps[k] += 1
            log.append({"blank_col": "between_blocks", "band": bi})
        else:
            for band in bands:
                band.col_gaps[0] += 1
            log.append({"blank_col": "left_margin"})
    return log
