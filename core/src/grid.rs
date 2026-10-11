//! Stage 2: a sheet seen without its empty rows and columns.
//!
//! Layout detection works on this compressed view, so inserting blank rows
//! or columns anywhere can never change a result. Structure comes from
//! headers and content, never from where the blanks were. Every cell keeps
//! its original coordinates for "What I read" and diagnostics.

use crate::dates::{self, DayTok};
use crate::model::CellRef;
use crate::read::{Cell, Sheet, Value};

#[derive(Clone, Debug)]
pub struct Grid {
    pub sheet_index: u16,
    pub name: String,
    /// Original row index of each compressed row.
    pub rows: Vec<u32>,
    /// Original column index of each compressed column.
    pub cols: Vec<u32>,
    cells: Vec<Vec<Cell>>,
    days: Vec<Vec<Option<DayTok>>>,
    /// True where a vertical merged range was filled down.
    pub filled_from_merge: Vec<Vec<bool>>,
    transposed: bool,
}

impl Grid {
    pub fn new(sheet: &Sheet, sheet_index: u16) -> Grid {
        let mut sheet = sheet.clone();
        let mut filled = vec![vec![false; sheet.width()]; sheet.rows.len()];
        // Vertical merges in a single column ("job written once over several
        // rows") are filled down; wider merges are titles and stay as they are.
        for m in &sheet.merges {
            if m.first_col == m.last_col && m.last_row > m.first_row {
                let top = sheet
                    .cell(m.first_row as usize, m.first_col as usize)
                    .clone();
                if top.is_empty() {
                    continue;
                }
                for r in m.first_row + 1..=m.last_row {
                    let (r, c) = (r as usize, m.first_col as usize);
                    if r < sheet.rows.len() {
                        let row = &mut sheet.rows[r];
                        if row.len() <= c {
                            row.resize(c + 1, Cell::EMPTY);
                        }
                        if row[c].is_empty() {
                            row[c] = top.clone();
                            if filled[r].len() <= c {
                                filled[r].resize(c + 1, false);
                            }
                            filled[r][c] = true;
                        }
                    }
                }
            }
        }
        let width = sheet.width();
        let rows: Vec<u32> = (0..sheet.rows.len())
            .filter(|&r| sheet.rows[r].iter().any(|c| !c.is_empty()))
            .map(|r| r as u32)
            .collect();
        let cols: Vec<u32> = (0..width)
            .filter(|&c| rows.iter().any(|&r| !sheet.cell(r as usize, c).is_empty()))
            .map(|c| c as u32)
            .collect();
        let cells: Vec<Vec<Cell>> = rows
            .iter()
            .map(|&r| {
                cols.iter()
                    .map(|&c| sheet.cell(r as usize, c as usize).clone())
                    .collect()
            })
            .collect();
        let days = cells
            .iter()
            .map(|row| row.iter().map(day_of).collect())
            .collect();
        let filled_from_merge = rows
            .iter()
            .map(|&r| {
                cols.iter()
                    .map(|&c| {
                        filled
                            .get(r as usize)
                            .and_then(|f| f.get(c as usize))
                            .copied()
                            .unwrap_or(false)
                    })
                    .collect()
            })
            .collect();
        Grid {
            sheet_index,
            name: sheet.name.clone(),
            rows,
            cols,
            cells,
            days,
            filled_from_merge,
            transposed: false,
        }
    }

    pub fn height(&self) -> usize {
        self.rows.len()
    }

    pub fn width(&self) -> usize {
        self.cols.len()
    }

    pub fn cell(&self, r: usize, c: usize) -> &Cell {
        &self.cells[r][c]
    }

    pub fn text(&self, r: usize, c: usize) -> &str {
        self.cells
            .get(r)
            .and_then(|row| row.get(c))
            .map(|c| c.text.as_str())
            .unwrap_or("")
    }

    pub fn is_empty(&self, r: usize, c: usize) -> bool {
        self.cells
            .get(r)
            .and_then(|row| row.get(c))
            .is_none_or(Cell::is_empty)
    }

    pub fn day(&self, r: usize, c: usize) -> Option<DayTok> {
        self.days
            .get(r)
            .and_then(|row| row.get(c))
            .copied()
            .flatten()
    }

    pub fn source(&self, r: usize, c: usize) -> CellRef {
        if self.transposed {
            CellRef {
                sheet: self.sheet_index,
                row: self.cols[c],
                col: self.rows[r],
            }
        } else {
            CellRef {
                sheet: self.sheet_index,
                row: self.rows[r],
                col: self.cols[c],
            }
        }
    }

    pub fn is_transposed(&self) -> bool {
        self.transposed
    }

    pub fn row_nonempty(&self, r: usize) -> usize {
        (0..self.width()).filter(|&c| !self.is_empty(r, c)).count()
    }

    pub fn first_nonempty(&self, r: usize) -> Option<usize> {
        (0..self.width()).find(|&c| !self.is_empty(r, c))
    }

    /// The grid with rows and columns swapped (for day × job layouts).
    pub fn transposed(&self) -> Grid {
        let h = self.height();
        let w = self.width();
        let cells = (0..w)
            .map(|c| (0..h).map(|r| self.cells[r][c].clone()).collect())
            .collect();
        let days = (0..w)
            .map(|c| (0..h).map(|r| self.days[r][c]).collect())
            .collect();
        let filled = (0..w)
            .map(|c| (0..h).map(|r| self.filled_from_merge[r][c]).collect())
            .collect();
        Grid {
            sheet_index: self.sheet_index,
            name: self.name.clone(),
            rows: self.cols.clone(),
            cols: self.rows.clone(),
            cells,
            days,
            filled_from_merge: filled,
            transposed: !self.transposed,
        }
    }
}

fn day_of(cell: &Cell) -> Option<DayTok> {
    use chrono::Datelike;
    match cell.value {
        Value::Date(d) | Value::DateTime(d, _) => Some(DayTok::Ymd {
            year: Some(d.year()),
            month: d.month(),
            day: d.day(),
            weekday: None,
        }),
        Value::Number(n) => dates::serial(n).map(DayTok::Serial),
        Value::Text => {
            let t = cell.text.trim();
            if let Ok(n) = t.parse::<f64>() {
                return dates::serial(n).map(DayTok::Serial);
            }
            dates::parse(t)
        }
        _ => None,
    }
}
