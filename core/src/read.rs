//! Stage 1: bytes → sheets of cells.
//!
//! The file's content decides the reader, not its extension, so an Excel
//! file renamed `.csv` still opens and a picture renamed `.csv` fails
//! cleanly. Spreadsheet cells are read as their stored values, never as
//! formulas.

use std::io::{Cursor, Read};

use calamine::{Data, Reader, Sheets};
use chrono::{Duration, NaiveDate};

use crate::model::{Input, ReadError};

/// What a cell held, beyond its text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Empty,
    Text,
    Number(f64),
    Date(NaiveDate),
    /// Minutes after midnight.
    Time(u16),
    DateTime(NaiveDate, u16),
    Bool,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub text: String,
    pub value: Value,
}

impl Cell {
    pub const EMPTY: Cell = Cell {
        text: String::new(),
        value: Value::Empty,
    };

    pub fn text(text: impl Into<String>) -> Cell {
        let text = text.into();
        let value = if text.trim().is_empty() {
            Value::Empty
        } else {
            Value::Text
        };
        Cell { text, value }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self.value, Value::Empty) || self.text.trim().is_empty()
    }
}

static EMPTY_CELL: Cell = Cell::EMPTY;

/// A merged range, inclusive, in sheet coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Merge {
    pub first_row: u32,
    pub first_col: u32,
    pub last_row: u32,
    pub last_col: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<Cell>>,
    pub merges: Vec<Merge>,
}

impl Sheet {
    pub fn cell(&self, row: usize, col: usize) -> &Cell {
        self.rows
            .get(row)
            .and_then(|r| r.get(col))
            .unwrap_or(&EMPTY_CELL)
    }

    pub fn width(&self) -> usize {
        self.rows.iter().map(Vec::len).max().unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Text,
    Spreadsheet,
    Word,
    Numbers,
}

#[derive(Clone, Debug)]
pub struct Workbook {
    pub sheets: Vec<Sheet>,
    pub kind: SourceKind,
    pub notes: Vec<String>,
}

pub fn read(input: &Input) -> Result<Workbook, ReadError> {
    match input {
        Input::Paste { text } => {
            if text.trim().is_empty() {
                return Err(ReadError::EmptyFile);
            }
            let mut notes = Vec::new();
            let sheet = parse_delimited(text, "Pasted", &mut notes);
            Ok(Workbook {
                sheets: vec![sheet],
                kind: SourceKind::Text,
                notes,
            })
        }
        Input::File { name, bytes } => read_bytes(name, bytes),
    }
}

fn read_bytes(name: &str, bytes: &[u8]) -> Result<Workbook, ReadError> {
    if bytes.is_empty() {
        return Err(ReadError::EmptyFile);
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return read_zip(name, bytes);
    }
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return read_spreadsheet(bytes);
    }
    let mut notes = Vec::new();
    let text = decode(bytes, &mut notes)?;
    if text.trim().is_empty() {
        return Err(ReadError::EmptyFile);
    }
    let stem = sheet_name_for(name);
    let sheet = parse_delimited(&text, &stem, &mut notes);
    Ok(Workbook {
        sheets: vec![sheet],
        kind: SourceKind::Text,
        notes,
    })
}

pub fn sheet_name_for(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    base.rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(base)
        .to_string()
}

// ---------------------------------------------------------------- text files

/// Decodes UTF-8 (with or without BOM), UTF-16 LE/BE, or Windows-1252.
fn decode(bytes: &[u8], notes: &mut Vec<String>) -> Result<String, ReadError> {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return Ok(utf8_lossy(rest, notes));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Ok(encoding_rs::UTF_16LE
            .decode_with_bom_removal(bytes)
            .0
            .into_owned());
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Ok(encoding_rs::UTF_16BE
            .decode_with_bom_removal(bytes)
            .0
            .into_owned());
    }
    if let Some(utf16) = sniff_utf16_without_bom(bytes) {
        return Ok(utf16.decode_without_bom_handling(bytes).0.into_owned());
    }
    if looks_binary(bytes) {
        return Err(ReadError::NotASchedule);
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(text.to_string()),
        Err(_) => {
            // Mostly valid UTF-8 with a few bad bytes: keep UTF-8. Single high
            // bytes with no UTF-8 sequences at all: it's Windows-1252.
            if has_utf8_multibyte(bytes) {
                Ok(utf8_lossy(bytes, notes))
            } else {
                Ok(encoding_rs::WINDOWS_1252
                    .decode_without_bom_handling(bytes)
                    .0
                    .into_owned())
            }
        }
    }
}

fn utf8_lossy(bytes: &[u8], notes: &mut Vec<String>) -> String {
    let text = String::from_utf8_lossy(bytes);
    if text.contains('\u{FFFD}') {
        notes.push("Some characters in the file couldn't be read and show as \u{FFFD}.".into());
    }
    text.into_owned()
}

fn has_utf8_multibyte(bytes: &[u8]) -> bool {
    bytes
        .windows(2)
        .any(|w| (0xC2..=0xF4).contains(&w[0]) && (0x80..=0xBF).contains(&w[1]))
}

fn sniff_utf16_without_bom(bytes: &[u8]) -> Option<&'static encoding_rs::Encoding> {
    let sample = &bytes[..bytes.len().min(4096)];
    if sample.len() < 4 {
        return None;
    }
    let pairs = sample.len() / 2;
    let zero_odd = sample
        .iter()
        .skip(1)
        .step_by(2)
        .filter(|b| **b == 0)
        .count();
    let zero_even = sample.iter().step_by(2).filter(|b| **b == 0).count();
    if zero_odd * 10 > pairs * 4 && zero_even * 10 < pairs {
        Some(encoding_rs::UTF_16LE)
    } else if zero_even * 10 > pairs * 4 && zero_odd * 10 < pairs {
        Some(encoding_rs::UTF_16BE)
    } else {
        None
    }
}

/// NUL bytes or lots of control characters: not a text file.
fn looks_binary(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(65536)];
    if sample.contains(&0) {
        return true;
    }
    let control = sample
        .iter()
        .filter(|b| **b < 0x20 && !matches!(**b, b'\t' | b'\n' | b'\r' | 0x0C))
        .count();
    control * 100 > sample.len()
}

const DELIMITERS: [u8; 4] = *b"\t,;|";

/// Picks the delimiter whose field counts are most consistent across lines.
pub fn sniff_delimiter(text: &str) -> Option<u8> {
    let mut best: Option<(f64, u8)> = None;
    for &delim in &DELIMITERS {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delim)
            .has_headers(false)
            .flexible(true)
            .from_reader(text.as_bytes());
        let mut counts: Vec<usize> = Vec::new();
        let mut lines = 0usize;
        for record in reader.records().take(400) {
            let Ok(record) = record else { break };
            let nonempty = record.iter().filter(|f| !f.trim().is_empty()).count();
            if nonempty == 0 {
                continue;
            }
            lines += 1;
            if record.len() > 1 {
                counts.push(record.len());
            }
        }
        if lines == 0 || counts.is_empty() {
            continue;
        }
        let mut freq = std::collections::HashMap::new();
        for c in &counts {
            *freq.entry(*c).or_insert(0usize) += 1;
        }
        let (&mode, &mode_n) = freq
            .iter()
            .max_by_key(|(c, n)| (**n, **c))
            .expect("non-empty");
        let near = counts.iter().filter(|c| c.abs_diff(mode) <= 1).count();
        let coverage = counts.len() as f64 / lines as f64;
        let consistency = (mode_n as f64 + 0.5 * (near - mode_n) as f64) / counts.len() as f64;
        let width = (mode.min(8) as f64).ln();
        let score = coverage * consistency * (1.0 + width);
        if best.is_none_or(|(s, _)| score > s + 1e-9) {
            best = Some((score, delim));
        }
    }
    best.map(|(_, d)| d)
}

pub fn parse_delimited(text: &str, name: &str, notes: &mut Vec<String>) -> Sheet {
    let rows = match sniff_delimiter(text) {
        Some(delim) => {
            let mut reader = csv::ReaderBuilder::new()
                .delimiter(delim)
                .has_headers(false)
                .flexible(true)
                .from_reader(text.as_bytes());
            let mut rows = Vec::new();
            for record in reader.records() {
                match record {
                    Ok(record) => rows.push(record.iter().map(text_cell).collect::<Vec<_>>()),
                    Err(_) => {
                        notes.push(
                            "Part of the file couldn't be split into columns and was skipped."
                                .into(),
                        );
                        break;
                    }
                }
            }
            rows
        }
        None => text.lines().map(|line| vec![text_cell(line)]).collect(),
    };
    let mut sheet = Sheet {
        name: name.to_string(),
        rows,
        merges: Vec::new(),
    };
    neutralize_errors(&mut sheet, notes);
    sheet
}

fn text_cell(field: &str) -> Cell {
    Cell::text(field.trim_end_matches('\r'))
}

/// Excel error values and leftover formula text become empty, with a note.
fn neutralize_errors(sheet: &mut Sheet, notes: &mut Vec<String>) {
    let mut errors = 0usize;
    let mut formulas = 0usize;
    for row in &mut sheet.rows {
        for cell in row.iter_mut() {
            let t = cell.text.trim();
            if is_error_text(t) || cell.value == Value::Error {
                errors += 1;
                *cell = Cell::EMPTY;
            } else if is_formula_text(t) {
                // Formula text in a text file is what the owner typed: it
                // stays as data (the export writes it as a string).
                formulas += 1;
            }
        }
    }
    if errors > 0 {
        notes.push(format!(
            "{errors} cell{} with spreadsheet errors (like #REF! or #N/A) {} treated as empty.",
            if errors == 1 { "" } else { "s" },
            if errors == 1 { "was" } else { "were" }
        ));
    }
    if formulas > 0 {
        notes.push(format!(
            "{formulas} cell{} held formula text; {} read as plain text, never calculated.",
            if formulas == 1 { "" } else { "s" },
            if formulas == 1 { "it was" } else { "they were" }
        ));
    }
}

fn is_error_text(t: &str) -> bool {
    matches!(
        t,
        "#REF!"
            | "#N/A"
            | "#NAME?"
            | "#VALUE!"
            | "#DIV/0!"
            | "#NUM!"
            | "#NULL!"
            | "#DATA!"
            | "#SPILL!"
            | "#CALC!"
    )
}

fn is_formula_text(t: &str) -> bool {
    static FORMULA: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"^=\s*([A-Za-z][A-Za-z0-9.]*\s*\(|\$?[A-Za-z]{1,3}\$?\d+|'[^']+'!)")
            .expect("valid regex")
    });
    FORMULA.is_match(t)
}

// --------------------------------------------------------------- zip formats

fn read_zip(name: &str, bytes: &[u8]) -> Result<Workbook, ReadError> {
    let Ok(archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return Err(ReadError::Unreadable);
    };
    let names: Vec<String> = archive
        .file_names()
        .filter_map(Result::ok)
        .map(|n| n.into_owned())
        .collect();
    let has = |n: &str| names.iter().any(|x| x == n);
    if has("xl/workbook.xml") || has("xl/workbook.bin") || has("content.xml") {
        return read_spreadsheet(bytes);
    }
    if has("word/document.xml") {
        return crate::read_docx::read(archive, name);
    }
    if names
        .iter()
        .any(|n| n.starts_with("Index/") && n.ends_with(".iwa"))
        || has("Index.zip")
    {
        return crate::read_numbers::read(archive, name);
    }
    Err(ReadError::NotASchedule)
}

// --------------------------------------------------------------- spreadsheets

fn read_spreadsheet(bytes: &[u8]) -> Result<Workbook, ReadError> {
    let mut book = calamine::open_workbook_auto_from_rs(Cursor::new(bytes))
        .map_err(|_| ReadError::Unreadable)?;
    let mut notes = Vec::new();
    let mut sheets = Vec::new();
    for name in book.sheet_names().to_vec() {
        let Ok(range) = book.worksheet_range(&name) else {
            notes.push(format!(
                "Sheet \u{201c}{name}\u{201d} couldn't be read and was skipped."
            ));
            continue;
        };
        let (row0, col0) = range.start().unwrap_or((0, 0));
        let mut rows: Vec<Vec<Cell>> = vec![Vec::new(); row0 as usize];
        for row in range.rows() {
            let mut cells: Vec<Cell> = vec![Cell::EMPTY; col0 as usize];
            cells.extend(row.iter().map(data_cell));
            while cells.last().is_some_and(Cell::is_empty) {
                cells.pop();
            }
            rows.push(cells);
        }
        let merges = merges_for(&mut book, &name);
        let mut sheet = Sheet {
            name: name.clone(),
            rows,
            merges,
        };
        let mut sheet_notes = Vec::new();
        neutralize_errors(&mut sheet, &mut sheet_notes);
        notes.extend(sheet_notes.into_iter().map(|n| format!("{name}: {n}")));
        sheets.push(sheet);
    }
    if sheets.is_empty() {
        return Err(ReadError::Unreadable);
    }
    Ok(Workbook {
        sheets,
        kind: SourceKind::Spreadsheet,
        notes,
    })
}

fn merges_for<RS: std::io::Read + std::io::Seek>(book: &mut Sheets<RS>, name: &str) -> Vec<Merge> {
    let dims = match book {
        Sheets::Xlsx(x) => x.merge_cells_by_sheet_name(name).ok(),
        Sheets::Xls(x) => x.merge_cells_by_sheet_name(name).ok(),
        _ => None,
    };
    dims.unwrap_or_default()
        .into_iter()
        .map(|d| Merge {
            first_row: d.start.0,
            first_col: d.start.1,
            last_row: d.end.0,
            last_col: d.end.1,
        })
        .collect()
}

fn data_cell(data: &Data) -> Cell {
    match data {
        Data::Empty => Cell::EMPTY,
        Data::String(s) => Cell::text(s.clone()),
        Data::Int(i) => Cell {
            text: i.to_string(),
            value: Value::Number(*i as f64),
        },
        Data::Float(f) => {
            let text = if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            };
            Cell {
                text,
                value: Value::Number(*f),
            }
        }
        Data::Bool(b) => Cell {
            text: b.to_string(),
            value: Value::Bool,
        },
        Data::Error(e) => Cell {
            text: e.to_string(),
            value: Value::Error,
        },
        Data::DateTime(dt) => serial_cell(dt.as_f64(), dt.is_duration()),
        Data::DateTimeIso(s) => iso_cell(s),
        Data::DurationIso(s) => Cell::text(s.clone()),
    }
}

/// Excel's 1900 date system: serial 1 is 1900-01-01, with the phantom
/// 1900-02-29, so day 0 is 1899-12-30 for every date after February 1900.
pub fn serial_to_date(serial: f64) -> Option<NaiveDate> {
    if !(1.0..2_958_466.0).contains(&serial) {
        return None;
    }
    NaiveDate::from_ymd_opt(1899, 12, 30)?.checked_add_signed(Duration::days(serial.floor() as i64))
}

fn serial_cell(serial: f64, duration: bool) -> Cell {
    let minutes = ((serial.fract() * 1440.0).round() as i64).rem_euclid(1440) as u16;
    if duration || serial < 1.0 {
        let text = format!("{:02}:{:02}", minutes / 60, minutes % 60);
        return Cell {
            text,
            value: Value::Time(minutes),
        };
    }
    match serial_to_date(serial) {
        Some(date) if minutes == 0 => Cell {
            text: date.format("%Y-%m-%d").to_string(),
            value: Value::Date(date),
        },
        Some(date) => Cell {
            text: format!(
                "{} {:02}:{:02}",
                date.format("%Y-%m-%d"),
                minutes / 60,
                minutes % 60
            ),
            value: Value::DateTime(date, minutes),
        },
        None => Cell {
            text: serial.to_string(),
            value: Value::Number(serial),
        },
    }
}

fn iso_cell(s: &str) -> Cell {
    if let Ok(date) = NaiveDate::parse_from_str(&s[..s.len().min(10)], "%Y-%m-%d") {
        return Cell {
            text: date.format("%Y-%m-%d").to_string(),
            value: Value::Date(date),
        };
    }
    Cell::text(s.to_string())
}

/// Reads one zip entry fully, with a size cap so a zip bomb can't hang us.
pub fn zip_entry(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<Vec<u8>> {
    const CAP: u64 = 256 * 1024 * 1024;
    let file = archive.by_name(name).ok()?;
    let mut out = Vec::new();
    file.take(CAP).read_to_end(&mut out).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet_of(text: &str) -> Sheet {
        let mut notes = Vec::new();
        parse_delimited(text, "t", &mut notes)
    }

    #[test]
    fn delimiters() {
        assert_eq!(sniff_delimiter("a,b,c\n1,2,3\n"), Some(b','));
        assert_eq!(sniff_delimiter("a;b;c\n1;2,5;3\n"), Some(b';'));
        assert_eq!(sniff_delimiter("a\tb\tc\n1\t2\t3\n"), Some(b'\t'));
        assert_eq!(sniff_delimiter("a|b|c\n1|2|3\n"), Some(b'|'));
        assert_eq!(sniff_delimiter("just some words\nand more\n"), None);
    }

    #[test]
    fn quoted_fields_and_cr_only() {
        let s = sheet_of("Name,Job\r\"Baker, Mike\",\"1420 Johnson\nSt\"\rJose,Smith\r");
        assert_eq!(s.rows.len(), 3);
        assert_eq!(s.cell(1, 0).text, "Baker, Mike");
        assert_eq!(s.cell(1, 1).text, "1420 Johnson\nSt");
    }

    #[test]
    fn encodings() {
        let mut notes = Vec::new();
        let cp1252 = b"Jos\xe9,\x93Smith\x94\n";
        assert_eq!(decode(cp1252, &mut notes).unwrap(), "José,“Smith”\n");
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("José\tA\n".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(decode(&utf16, &mut notes).unwrap(), "José\tA\n");
        assert_eq!(decode(b"\xEF\xBB\xBFName\n", &mut notes).unwrap(), "Name\n");
        assert!(decode(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR", &mut notes).is_err());
    }

    #[test]
    fn errors_become_empty_formula_text_stays() {
        let s = sheet_of("Name,Job\nMike,#REF!\nJose,\"=VLOOKUP(A2,B:B,1)\"\n");
        assert!(s.cell(1, 1).is_empty());
        assert_eq!(s.cell(2, 1).text, "=VLOOKUP(A2,B:B,1)");
    }

    #[test]
    fn serials() {
        assert_eq!(
            serial_to_date(46307.0),
            NaiveDate::from_ymd_opt(2026, 10, 12)
        );
    }
}
