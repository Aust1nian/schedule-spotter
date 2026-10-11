//! Word documents: every table becomes rows of cells; paragraphs outside
//! tables become one-cell rows, so a "Week of 10/12" line above the table
//! still anchors weekday headers.

use std::collections::HashMap;
use std::io::Cursor;

use quick_xml::events::Event;

use crate::model::ReadError;
use crate::read::{Cell, Merge, Sheet, SourceKind, Workbook, zip_entry};

pub fn read(
    mut archive: zip::ZipArchive<Cursor<&[u8]>>,
    name: &str,
) -> Result<Workbook, ReadError> {
    let xml = zip_entry(&mut archive, "word/document.xml").ok_or(ReadError::Unreadable)?;
    let sheet = parse_document(&xml, name).ok_or(ReadError::Unreadable)?;
    if sheet.rows.iter().all(|r| r.iter().all(Cell::is_empty)) {
        return Err(ReadError::EmptyFile);
    }
    Ok(Workbook {
        sheets: vec![sheet],
        kind: SourceKind::Word,
        notes: Vec::new(),
    })
}

#[derive(Default)]
struct TableState {
    row: Vec<Cell>,
    cell: String,
    in_cell: bool,
    cell_paragraphs: usize,
    span: u32,
    vmerge: Option<bool>, // Some(true) = restart, Some(false) = continue
    vmerge_start: HashMap<u32, u32>,
}

fn parse_document(xml: &[u8], name: &str) -> Option<Sheet> {
    let mut reader = quick_xml::Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let mut merges: Vec<Merge> = Vec::new();
    let mut table_depth = 0usize;
    let mut table = TableState::default();
    let mut paragraph = String::new();
    let mut in_text = false;

    loop {
        let event = reader.read_event_into(&mut buf).ok()?;
        match &event {
            Event::Start(e) | Event::Empty(e) => {
                let is_empty = matches!(event, Event::Empty(_));
                match AsRef::<str>::as_ref(&e.local_name()) {
                    "tbl" if !is_empty => {
                        table_depth += 1;
                        if table_depth == 1 {
                            table = TableState::default();
                        }
                    }
                    "tr" if table_depth == 1 && !is_empty => table.row.clear(),
                    "tc" if table_depth == 1 && !is_empty => {
                        table.in_cell = true;
                        table.cell.clear();
                        table.cell_paragraphs = 0;
                        table.span = 1;
                        table.vmerge = None;
                    }
                    "gridSpan" if table_depth == 1 => {
                        table.span = attr(e, "val")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(1)
                            .max(1);
                    }
                    "vMerge" if table_depth == 1 => {
                        table.vmerge = Some(attr(e, "val").is_some_and(|v| v == "restart"));
                    }
                    "p" if !is_empty => {
                        if table_depth >= 1 && table.in_cell {
                            if table.cell_paragraphs > 0 {
                                table.cell.push('\n');
                            }
                            table.cell_paragraphs += 1;
                        } else {
                            paragraph.clear();
                        }
                    }
                    "t" if !is_empty => in_text = true,
                    "tab" => push_text(&mut table, &mut paragraph, table_depth, "\t"),
                    "br" | "cr" => push_text(&mut table, &mut paragraph, table_depth, "\n"),
                    _ => {}
                }
            }
            Event::Text(t) if in_text => {
                let text = t.xml10_content().into_owned();
                push_text(&mut table, &mut paragraph, table_depth, &text);
            }
            Event::GeneralRef(r) if in_text => {
                let named: &str = r.as_ref();
                let text = match named {
                    "amp" => "&".to_string(),
                    "lt" => "<".to_string(),
                    "gt" => ">".to_string(),
                    "quot" => "\"".to_string(),
                    "apos" => "'".to_string(),
                    _ => r
                        .resolve_char_ref()
                        .ok()
                        .flatten()
                        .map(String::from)
                        .unwrap_or_default(),
                };
                push_text(&mut table, &mut paragraph, table_depth, &text);
            }
            Event::End(e) => match AsRef::<str>::as_ref(&e.local_name()) {
                "t" => in_text = false,
                "p" if table_depth == 0 => {
                    let text = paragraph.trim();
                    if !text.is_empty() {
                        rows.push(vec![Cell::text(text)]);
                    }
                }
                "tc" if table_depth == 1 => {
                    let col = table.row.len() as u32;
                    let row_index = rows.len() as u32;
                    match table.vmerge {
                        Some(false) => {
                            table.row.push(Cell::EMPTY);
                            let start = *table.vmerge_start.get(&col).unwrap_or(&row_index);
                            extend_merge(&mut merges, start, col, row_index, col + table.span - 1);
                        }
                        Some(true) => {
                            table.vmerge_start.insert(col, row_index);
                            table.row.push(Cell::text(table.cell.trim()));
                        }
                        None => {
                            table.vmerge_start.remove(&col);
                            table.row.push(Cell::text(table.cell.trim()));
                        }
                    }
                    if table.span > 1 {
                        merges.push(Merge {
                            first_row: row_index,
                            first_col: col,
                            last_row: row_index,
                            last_col: col + table.span - 1,
                        });
                        for _ in 1..table.span {
                            table.row.push(Cell::EMPTY);
                        }
                    }
                    table.in_cell = false;
                }
                "tr" if table_depth == 1 => rows.push(std::mem::take(&mut table.row)),
                "tbl" => {
                    table_depth = table_depth.saturating_sub(1);
                    if table_depth == 0 {
                        rows.push(Vec::new());
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Some(Sheet {
        name: crate::read::sheet_name_for(name),
        rows,
        merges,
    })
}

fn push_text(table: &mut TableState, paragraph: &mut String, depth: usize, text: &str) {
    if depth >= 1 && table.in_cell {
        table.cell.push_str(text);
    } else {
        paragraph.push_str(text);
    }
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| {
            let name = a.key.local_name();
            AsRef::<str>::as_ref(&name) == key
        })
        .and_then(|a| {
            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|v| v.into_owned())
        })
}

fn extend_merge(merges: &mut Vec<Merge>, start_row: u32, col: u32, row: u32, last_col: u32) {
    if let Some(m) = merges
        .iter_mut()
        .find(|m| m.first_row == start_row && m.first_col == col && m.last_row + 1 == row)
    {
        m.last_row = row;
    } else {
        merges.push(Merge {
            first_row: start_row,
            first_col: col,
            last_row: row,
            last_col,
        });
    }
}
