//! Apple Numbers documents (.numbers, the single-file zip form).
//!
//! The format is undocumented. What's read here: every `Index/*.iwa` file is
//! a series of Snappy-compressed chunks holding protobuf archives. The
//! document lists sheets; each sheet lists drawables, of which the tables
//! (type 6000) point to a table model (6001). The model's data store has
//! tiles of 256 rows, each row a buffer of cell records, and a string table
//! for text cells. Cell records are storage version 5. Values only:
//! formulas are never evaluated, and their cached results are used.

use std::collections::HashMap;
use std::io::Cursor;

use chrono::{Duration, NaiveDate, NaiveDateTime, Timelike};

use crate::model::ReadError;
use crate::read::{Cell, Sheet, SourceKind, Value, Workbook, zip_entry};

const TABLE_INFO: u32 = 6000;
const TABLE_MODEL: u32 = 6001;
const DEFAULT_TILE_SIZE: u64 = 256;

pub fn read(
    mut archive: zip::ZipArchive<Cursor<&[u8]>>,
    _name: &str,
) -> Result<Workbook, ReadError> {
    let names: Vec<String> = archive
        .file_names()
        .filter_map(Result::ok)
        .map(|n| n.into_owned())
        .filter(|n| n.starts_with("Index/") && n.ends_with(".iwa"))
        .collect();
    let mut objects: HashMap<u64, (u32, Vec<u8>)> = HashMap::new();
    for name in &names {
        let Some(bytes) = zip_entry(&mut archive, name) else {
            continue;
        };
        let Some(stream) = decompress_iwa(&bytes) else {
            return Err(ReadError::Unreadable);
        };
        parse_archives(&stream, &mut objects).ok_or(ReadError::Unreadable)?;
    }
    // The document is object 1 by convention; find it by type otherwise.
    let doc = objects
        .get(&1)
        .filter(|(t, _)| *t == 1)
        .or_else(|| objects.values().find(|(t, _)| *t == 1));
    let Some((_, doc)) = doc else {
        return Err(ReadError::Unreadable);
    };

    let mut sheets = Vec::new();
    let mut notes = Vec::new();
    for sheet_ref in refs(doc, 1) {
        let Some((_, sheet)) = objects.get(&sheet_ref) else {
            continue;
        };
        let sheet_name = string_field(sheet, 1).unwrap_or_default();
        let tables: Vec<u64> = refs(sheet, 2)
            .into_iter()
            .filter(|id| objects.get(id).is_some_and(|(t, _)| *t == TABLE_INFO))
            .collect();
        for (k, info_id) in tables.iter().enumerate() {
            let Some(model_id) = refs(&objects[info_id].1, 2).first().copied() else {
                continue;
            };
            let Some((TABLE_MODEL, model)) = objects.get(&model_id).map(|(t, b)| (*t, b)) else {
                continue;
            };
            let table_name = string_field(model, 8).unwrap_or_default();
            match read_table(model, &objects) {
                Some(rows) => {
                    let name = if tables.len() > 1 {
                        format!("{sheet_name} – {table_name}")
                    } else {
                        sheet_name.clone()
                    };
                    sheets.push(Sheet {
                        name,
                        rows,
                        merges: Vec::new(),
                    });
                }
                None => notes.push(format!(
                    "A table on sheet \u{201c}{sheet_name}\u{201d} ({}) couldn't be read.",
                    k + 1
                )),
            }
        }
    }
    if sheets.is_empty() {
        return Err(ReadError::Unreadable);
    }
    Ok(Workbook {
        sheets,
        kind: SourceKind::Numbers,
        notes,
    })
}

fn read_table(model: &[u8], objects: &HashMap<u64, (u32, Vec<u8>)>) -> Option<Vec<Vec<Cell>>> {
    let n_rows = varint_field(model, 6)? as usize;
    let n_cols = varint_field(model, 7)? as usize;
    let store = bytes_field(model, 4)?;
    let strings = refs(store, 4)
        .first()
        .and_then(|id| objects.get(id))
        .map(|(_, b)| string_table(b))
        .unwrap_or_default();
    let rich = refs(store, 17)
        .first()
        .and_then(|id| objects.get(id))
        .map(|(_, b)| rich_table(b, objects))
        .unwrap_or_default();
    let tiles = bytes_field(store, 3)?;
    let tile_size = varint_field(tiles, 2)
        .filter(|s| *s > 0)
        .unwrap_or(DEFAULT_TILE_SIZE);

    let mut rows: Vec<Vec<Cell>> = vec![Vec::new(); n_rows.min(1_000_000)];
    for tile_entry in fields(tiles)
        .filter(|(f, _)| *f == 1)
        .filter_map(|(_, v)| v.bytes())
    {
        let tile_id = varint_field(tile_entry, 1).unwrap_or(0);
        let Some(tile_ref) = refs(tile_entry, 2).first().copied() else {
            continue;
        };
        let Some((_, tile)) = objects.get(&tile_ref) else {
            continue;
        };
        for row_info in fields(tile)
            .filter(|(f, _)| *f == 5)
            .filter_map(|(_, v)| v.bytes())
        {
            let row = (tile_id * tile_size + varint_field(row_info, 1).unwrap_or(0)) as usize;
            if row >= rows.len() {
                continue;
            }
            let buffer = bytes_field(row_info, 6).unwrap_or(&[]);
            let offsets = bytes_field(row_info, 7).unwrap_or(&[]);
            let wide = varint_field(row_info, 8).unwrap_or(0) != 0;
            rows[row] = row_cells(buffer, offsets, wide, n_cols, &strings, &rich);
        }
    }
    Some(rows)
}

fn row_cells(
    buffer: &[u8],
    offsets: &[u8],
    wide: bool,
    n_cols: usize,
    strings: &HashMap<u32, String>,
    rich: &HashMap<u32, String>,
) -> Vec<Cell> {
    let offs: Vec<i32> = offsets
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as i32)
        .map(|o| if o >= 0 && wide { o * 4 } else { o })
        .collect();
    let mut cells = Vec::with_capacity(n_cols);
    for col in 0..n_cols.min(offs.len()) {
        let start = offs[col];
        if start < 0 {
            cells.push(Cell::EMPTY);
            continue;
        }
        let end = offs[col + 1..]
            .iter()
            .find(|o| **o >= 0)
            .map(|o| *o as usize)
            .unwrap_or(buffer.len());
        let record = buffer
            .get(start as usize..end.min(buffer.len()))
            .unwrap_or(&[]);
        cells.push(decode_cell(record, strings, rich).unwrap_or(Cell::EMPTY));
    }
    while cells.last().is_some_and(Cell::is_empty) {
        cells.pop();
    }
    cells
}

fn decode_cell(
    b: &[u8],
    strings: &HashMap<u32, String>,
    rich: &HashMap<u32, String>,
) -> Option<Cell> {
    if b.len() < 12 || b[0] != 5 {
        return None;
    }
    let cell_type = b[1];
    let flags = u32::from_le_bytes(b[8..12].try_into().ok()?);
    let mut at = 12usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let s = b.get(at..at + n)?;
        at += n;
        Some(s)
    };
    let mut d128 = None;
    let mut double = None;
    let mut seconds = None;
    let mut string_id = None;
    let mut rich_id = None;
    if flags & 0x1 != 0 {
        d128 = Some(decimal128(take(16)?));
    }
    if flags & 0x2 != 0 {
        double = Some(f64::from_le_bytes(take(8)?.try_into().ok()?));
    }
    if flags & 0x4 != 0 {
        seconds = Some(f64::from_le_bytes(take(8)?.try_into().ok()?));
    }
    if flags & 0x8 != 0 {
        string_id = Some(u32::from_le_bytes(take(4)?.try_into().ok()?));
    }
    if flags & 0x10 != 0 {
        rich_id = Some(u32::from_le_bytes(take(4)?.try_into().ok()?));
    }
    Some(match cell_type {
        3 => Cell::text(strings.get(&string_id?)?.clone()),
        9 => Cell::text(rich.get(&rich_id?)?.clone()),
        2 | 10 => {
            let n = d128.or(double)?;
            let text = if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", n as i64)
            } else {
                n.to_string()
            };
            Cell {
                text,
                value: Value::Number(n),
            }
        }
        5 => {
            let epoch = NaiveDate::from_ymd_opt(2001, 1, 1)?.and_hms_opt(0, 0, 0)?;
            let dt: NaiveDateTime =
                epoch + Duration::milliseconds((seconds? * 1000.0).round() as i64);
            let minutes = (dt.time().num_seconds_from_midnight() / 60) as u16;
            if minutes == 0 {
                Cell {
                    text: dt.date().format("%Y-%m-%d").to_string(),
                    value: Value::Date(dt.date()),
                }
            } else {
                Cell {
                    text: format!(
                        "{} {:02}:{:02}",
                        dt.date().format("%Y-%m-%d"),
                        minutes / 60,
                        minutes % 60
                    ),
                    value: Value::DateTime(dt.date(), minutes),
                }
            }
        }
        6 => Cell {
            text: (double? > 0.0).to_string(),
            value: Value::Bool,
        },
        7 => {
            let minutes = (double? / 60.0).round() as i64;
            Cell {
                text: format!("{:02}:{:02}", minutes / 60, minutes % 60),
                value: Value::Time((minutes.rem_euclid(1440)) as u16),
            }
        }
        8 => Cell {
            text: "#ERROR".into(),
            value: Value::Error,
        },
        _ => return None,
    })
}

/// IEEE 754 decimal128 as Numbers stores it (binary integer significand).
fn decimal128(b: &[u8]) -> f64 {
    let exp = ((((b[15] & 0x7F) as i32) << 7) | (b[14] >> 1) as i32) - 0x1820;
    let mut mantissa: u128 = (b[14] & 1) as u128;
    for i in (0..14).rev() {
        mantissa = mantissa * 256 + b[i] as u128;
    }
    let value = mantissa as f64 * 10f64.powi(exp);
    if b[15] & 0x80 != 0 { -value } else { value }
}

fn string_table(list: &[u8]) -> HashMap<u32, String> {
    fields(list)
        .filter(|(f, _)| *f == 3)
        .filter_map(|(_, v)| v.bytes())
        .filter_map(|entry| Some((varint_field(entry, 1)? as u32, string_field(entry, 3)?)))
        .collect()
}

/// Rich-text cells: entry → payload (6218) → storage (2001) → text.
fn rich_table(list: &[u8], objects: &HashMap<u64, (u32, Vec<u8>)>) -> HashMap<u32, String> {
    let mut out = HashMap::new();
    for entry in fields(list)
        .filter(|(f, _)| *f == 3)
        .filter_map(|(_, v)| v.bytes())
    {
        let Some(key) = varint_field(entry, 1) else {
            continue;
        };
        let text = refs(entry, 9)
            .first()
            .and_then(|id| objects.get(id))
            .and_then(|(_, payload)| refs(payload, 1).first().copied())
            .and_then(|id| objects.get(&id))
            .map(|(_, storage)| {
                fields(storage)
                    .filter(|(f, _)| *f == 3)
                    .filter_map(|(_, v)| v.bytes())
                    .map(|s| String::from_utf8_lossy(s).into_owned())
                    .collect::<String>()
            });
        if let Some(text) = text {
            out.insert(
                key as u32,
                text.trim_end_matches(['\n', '\u{2029}'])
                    .replace('\u{2029}', "\n"),
            );
        }
    }
    out
}

// ---------------------------------------------------------------- IWA framing

fn decompress_iwa(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut at = 0usize;
    let mut decoder = snap::raw::Decoder::new();
    while at < data.len() {
        let header = data.get(at..at + 4)?;
        if header[0] != 0 {
            return None;
        }
        let len = header[1] as usize | (header[2] as usize) << 8 | (header[3] as usize) << 16;
        let chunk = data.get(at + 4..at + 4 + len)?;
        out.extend(decoder.decompress_vec(chunk).ok()?);
        at += 4 + len;
    }
    Some(out)
}

/// Archive segments: varint length, ArchiveInfo {1: id, 2: MessageInfo{1: type, 3: length}*},
/// then each message's payload. Only the first message of each segment is kept.
fn parse_archives(stream: &[u8], objects: &mut HashMap<u64, (u32, Vec<u8>)>) -> Option<()> {
    let mut at = 0usize;
    while at < stream.len() {
        let (info_len, n) = varint(&stream[at..])?;
        at += n;
        let info = stream.get(at..at + info_len as usize)?;
        at += info_len as usize;
        let id = varint_field(info, 1)?;
        let mut first = true;
        for msg in fields(info)
            .filter(|(f, _)| *f == 2)
            .filter_map(|(_, v)| v.bytes())
        {
            let ty = varint_field(msg, 1).unwrap_or(0) as u32;
            let len = varint_field(msg, 3).unwrap_or(0) as usize;
            let payload = stream.get(at..at + len)?;
            at += len;
            if first {
                objects.insert(id, (ty, payload.to_vec()));
                first = false;
            }
        }
    }
    Some(())
}

// ---------------------------------------------------------- protobuf reading

enum Wire<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Fixed,
}

impl<'a> Wire<'a> {
    fn bytes(self) -> Option<&'a [u8]> {
        match self {
            Wire::Bytes(b) => Some(b),
            _ => None,
        }
    }
}

fn varint(b: &[u8]) -> Option<(u64, usize)> {
    let mut value = 0u64;
    for (i, byte) in b.iter().enumerate().take(10) {
        value |= ((byte & 0x7F) as u64) << (7 * i);
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
    }
    None
}

fn fields(mut b: &[u8]) -> impl Iterator<Item = (u32, Wire<'_>)> {
    std::iter::from_fn(move || {
        if b.is_empty() {
            return None;
        }
        let (tag, n) = varint(b)?;
        b = &b[n..];
        let field = (tag >> 3) as u32;
        let wire = match tag & 7 {
            0 => {
                let (v, n) = varint(b)?;
                b = &b[n..];
                Wire::Varint(v)
            }
            1 => {
                b = b.get(8..)?;
                Wire::Fixed
            }
            2 => {
                let (len, n) = varint(b)?;
                let data = b.get(n..n + len as usize)?;
                b = &b[n + len as usize..];
                Wire::Bytes(data)
            }
            5 => {
                b = b.get(4..)?;
                Wire::Fixed
            }
            _ => return None,
        };
        Some((field, wire))
    })
}

fn varint_field(b: &[u8], field: u32) -> Option<u64> {
    fields(b).find_map(|(f, w)| match (f == field, w) {
        (true, Wire::Varint(v)) => Some(v),
        _ => None,
    })
}

fn bytes_field(b: &[u8], field: u32) -> Option<&[u8]> {
    fields(b).find_map(|(f, w)| if f == field { w.bytes() } else { None })
}

fn string_field(b: &[u8], field: u32) -> Option<String> {
    bytes_field(b, field).map(|s| String::from_utf8_lossy(s).into_owned())
}

/// TSP.Reference fields (`{1: identifier}`) at `field`, in order.
fn refs(b: &[u8], field: u32) -> Vec<u64> {
    fields(b)
        .filter(|(f, _)| *f == field)
        .filter_map(|(_, w)| w.bytes())
        .filter_map(|r| varint_field(r, 1))
        .collect()
}
