//! Stage 10: the "Schedule check" workbook and the findings CSV.
//!
//! The workbook has four sheets the owner reads (Findings, Week grid,
//! Cleaned schedule, Name and site matches) and one very hidden sheet that
//! carries the session, so reopening the file restores the schedule and
//! every answer. Every text cell is an inline string, so Excel never
//! evaluates one, whatever it starts with. In the CSV, cells that start
//! with =, +, -, @, tab or carriage return get a leading apostrophe, the
//! usual guard against spreadsheet formula injection.

use std::io::{Cursor, Write};
use std::path::Path;

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::describe;
use crate::model::{FlagKind, InfoKind, Report};
use crate::session::Session;
use crate::settings::Settings;
use crate::view::{Mark, status_label};

/// The very hidden sheet that holds the session as JSON, split over cells.
pub const STATE_SHEET: &str = "SpotterState";
/// Characters per state cell, under Excel's 32,767 limit.
const CHUNK: usize = 30_000;

/// Why saving failed. The check in the app is untouched either way.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveError {
    SaveFailed(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::SaveFailed(why) => write!(f, "{why}"),
        }
    }
}

/// `Schedule check - week of 2026-10-12.xlsx`: the Monday of the first week
/// the schedule covers.
pub fn default_name(report: &Report, settings: &Settings) -> String {
    let first = report
        .reading
        .window
        .first()
        .copied()
        .unwrap_or(settings.today);
    let monday = first - Duration::days(first.weekday().num_days_from_monday() as i64);
    format!(
        "Schedule check - week of {}.xlsx",
        monday.format("%Y-%m-%d")
    )
}

pub fn flag_label(kind: FlagKind) -> &'static str {
    match kind {
        FlagKind::DoubleBooked => "Double-booked",
        FlagKind::AssignedWhileOff => "Assigned while off",
        FlagKind::NotBooked => "Not booked",
        FlagKind::Unstaffed => "Unstaffed",
    }
}

pub fn info_label(kind: InfoKind) -> &'static str {
    match kind {
        InfoKind::DuplicateEntry => "Listed twice",
        InfoKind::SplitDay => "Split day",
        InfoKind::InternalPlusSite => "Shop and a job",
        InfoKind::SubcontractorMultiSite => "Subcontractor at two jobs",
    }
}

fn day(d: NaiveDate) -> String {
    d.format("%a %-m/%-d/%Y").to_string()
}

/// The findings as rows: Finding, Date, Person, Job, Details, Matches used.
fn finding_rows(report: &Report) -> Vec<[String; 6]> {
    let mut rows = Vec::new();
    for f in &report.flags {
        let jobs = if f.sites.is_empty() {
            f.site.clone().unwrap_or_default()
        } else {
            f.sites.join(", ")
        };
        let chips: Vec<String> = f
            .merges
            .iter()
            .filter_map(|&i| report.merges.get(i))
            .map(describe::merge)
            .collect();
        rows.push([
            flag_label(f.kind).to_string(),
            day(f.date),
            f.person.clone().unwrap_or_default(),
            jobs,
            f.message.clone(),
            chips.join("; "),
        ]);
    }
    for i in &report.info {
        rows.push([
            info_label(i.kind).to_string(),
            day(i.date),
            i.person.clone(),
            i.sites.join(", "),
            i.message.clone(),
            String::new(),
        ]);
    }
    rows
}

// ------------------------------------------------------------------ XLSX

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Plain = 0,
    Bold = 1,
    Red = 2,
    Amber = 3,
    Striped = 4,
    Wrap = 5,
    Muted = 6,
}

struct SheetXml {
    name: &'static str,
    rows: Vec<Vec<(String, Style)>>,
    widths: Vec<f32>,
    hidden: bool,
}

impl SheetXml {
    fn new(name: &'static str, widths: &[f32]) -> SheetXml {
        SheetXml {
            name,
            rows: Vec::new(),
            widths: widths.to_vec(),
            hidden: false,
        }
    }

    fn row<S: Into<String>>(&mut self, cells: impl IntoIterator<Item = (S, Style)>) {
        self.rows
            .push(cells.into_iter().map(|(t, s)| (t.into(), s)).collect());
    }

    fn header(&mut self, cells: &[&str]) {
        self.row(cells.iter().map(|c| (c.to_string(), Style::Bold)));
    }

    fn plain<S: Into<String>>(&mut self, cells: impl IntoIterator<Item = S>) {
        self.row(cells.into_iter().map(|c| (c.into(), Style::Plain)));
    }

    fn blank(&mut self) {
        self.rows.push(Vec::new());
    }

    fn xml(&self) -> String {
        let mut out = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
        );
        if !self.hidden && !self.rows.is_empty() {
            out.push_str(r#"<sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews>"#);
        }
        if !self.widths.is_empty() {
            out.push_str("<cols>");
            for (i, w) in self.widths.iter().enumerate() {
                out.push_str(&format!(
                    r#"<col min="{n}" max="{n}" width="{w}" customWidth="1"/>"#,
                    n = i + 1
                ));
            }
            out.push_str("</cols>");
        }
        out.push_str("<sheetData>");
        for (r, row) in self.rows.iter().enumerate() {
            if row.is_empty() {
                continue;
            }
            out.push_str(&format!(r#"<row r="{}">"#, r + 1));
            for (c, (text, style)) in row.iter().enumerate() {
                let reference = format!("{}{}", column_letters(c), r + 1);
                let s = *style as u8;
                if text.is_empty() {
                    if s != 0 {
                        out.push_str(&format!(r#"<c r="{reference}" s="{s}"/>"#));
                    }
                    continue;
                }
                out.push_str(&format!(
                    r#"<c r="{reference}" s="{s}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
                    xml_escape(text)
                ));
            }
            out.push_str("</row>");
        }
        out.push_str("</sheetData></worksheet>");
        out
    }
}

fn column_letters(mut c: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push(b'A' + (c % 26) as u8);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.reverse();
    String::from_utf8(s).expect("ascii")
}

/// Escapes XML and drops characters XML 1.0 can't hold.
fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(ch),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<fonts count="3"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font><font><sz val="11"/><color rgb="FF666666"/><name val="Calibri"/></font></fonts>
<fills count="5"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill><fill><patternFill patternType="solid"><fgColor rgb="FFF8C9C4"/><bgColor indexed="64"/></patternFill></fill><fill><patternFill patternType="solid"><fgColor rgb="FFFCE3A6"/><bgColor indexed="64"/></patternFill></fill><fill><patternFill patternType="lightUp"><fgColor rgb="FFB0B0B0"/><bgColor rgb="FFFFFFFF"/></patternFill></fill></fills>
<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>
<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>
<cellXfs count="7"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"><alignment vertical="top"/></xf><xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"><alignment vertical="top"/></xf><xf numFmtId="0" fontId="0" fillId="2" borderId="0" xfId="0" applyFill="1" applyAlignment="1"><alignment vertical="top" wrapText="1"/></xf><xf numFmtId="0" fontId="0" fillId="3" borderId="0" xfId="0" applyFill="1" applyAlignment="1"><alignment vertical="top" wrapText="1"/></xf><xf numFmtId="0" fontId="0" fillId="4" borderId="0" xfId="0" applyFill="1" applyAlignment="1"><alignment vertical="top" wrapText="1"/></xf><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0" applyAlignment="1"><alignment vertical="top" wrapText="1"/></xf><xf numFmtId="0" fontId="2" fillId="0" borderId="0" xfId="0" applyFont="1"><alignment vertical="top"/></xf></cellXfs>
<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>
</styleSheet>"#;

fn findings_sheet(report: &Report) -> SheetXml {
    let mut sh = SheetXml::new("Findings", &[20.0, 16.0, 22.0, 34.0, 70.0, 40.0]);
    sh.header(&[
        "Finding",
        "Date",
        "Person",
        "Job",
        "Details",
        "Matches used",
    ]);
    let rows = finding_rows(report);
    if rows.is_empty() {
        sh.plain(["No problems found", "", "", "", "", ""]);
    }
    for (i, r) in rows.into_iter().enumerate() {
        let style = match report.flags.get(i).map(|f| f.kind) {
            Some(FlagKind::DoubleBooked | FlagKind::AssignedWhileOff) => Style::Red,
            Some(FlagKind::NotBooked) => Style::Amber,
            Some(FlagKind::Unstaffed) => Style::Striped,
            None => Style::Muted,
        };
        let mut cells: Vec<(String, Style)> = Vec::new();
        for (k, text) in r.into_iter().enumerate() {
            let s = match k {
                0 => style,
                4 | 5 => Style::Wrap,
                _ => Style::Plain,
            };
            cells.push((text, s));
        }
        sh.row(cells);
    }
    if !report.questions.is_empty() {
        sh.blank();
        sh.header(&["Still open", "", "", "", "Question", "Reading used for now"]);
        for q in &report.questions {
            let v = describe::question(q);
            let current = v.current.unwrap_or_else(|| match q {
                crate::model::Question::SamePerson { .. }
                | crate::model::Question::SameSite { .. } => "kept apart".into(),
                _ => String::new(),
            });
            sh.row([
                ("Question".to_string(), Style::Plain),
                (String::new(), Style::Plain),
                (String::new(), Style::Plain),
                (String::new(), Style::Plain),
                (v.text, Style::Wrap),
                (current, Style::Wrap),
            ]);
        }
    }
    if !report.reading.notes.is_empty() {
        sh.blank();
        sh.header(&["Notes", "", "", "", "Note"]);
        for n in &report.reading.notes {
            sh.row([
                ("Note".to_string(), Style::Plain),
                (String::new(), Style::Plain),
                (String::new(), Style::Plain),
                (String::new(), Style::Plain),
                (n.clone(), Style::Wrap),
            ]);
        }
    }
    sh
}

fn week_sheet(report: &Report) -> SheetXml {
    let w = &report.week;
    let mut widths = vec![24.0];
    widths.extend(std::iter::repeat_n(22.0, w.dates.len()));
    let mut sh = SheetXml::new("Week grid", &widths);
    let mut header = vec![("Person".to_string(), Style::Bold)];
    for (d, working) in w.dates.iter().zip(&w.working) {
        let mut h = d.format("%a %-m/%-d").to_string();
        if !working {
            h.push_str(" (day off)");
        }
        header.push((h, Style::Bold));
    }
    sh.row(header);
    for p in &w.people {
        let mut row = vec![(p.name.clone(), Style::Plain)];
        for c in &p.cells {
            let style = match c.mark {
                Some(Mark::Red) => Style::Red,
                Some(Mark::Amber) => Style::Amber,
                None => Style::Wrap,
            };
            row.push((c.items.join("\n"), style));
        }
        sh.row(row);
    }
    if !w.jobs.is_empty() {
        sh.blank();
        let mut header = vec![("Job".to_string(), Style::Bold)];
        header.extend(
            w.dates
                .iter()
                .map(|d| (d.format("%a %-m/%-d").to_string(), Style::Bold)),
        );
        sh.row(header);
        for j in &w.jobs {
            let mut row = vec![(j.name.clone(), Style::Plain)];
            for c in &j.cells {
                if c.unstaffed {
                    row.push(("Nobody on it".to_string(), Style::Striped));
                } else {
                    row.push((c.people.join(", "), Style::Wrap));
                }
            }
            sh.row(row);
        }
    }
    sh
}

fn cleaned_sheet(report: &Report) -> SheetXml {
    let mut sh = SheetXml::new(
        "Cleaned schedule",
        &[
            12.0, 6.0, 22.0, 30.0, 10.0, 10.0, 8.0, 8.0, 30.0, 10.0, 22.0, 30.0,
        ],
    );
    sh.header(&[
        "Date",
        "Day",
        "Person",
        "Job",
        "Status",
        "Part of day",
        "Start",
        "End",
        "Note",
        "Unstaffed",
        "Person as written",
        "Job as written",
    ]);
    let mut rows: Vec<(usize, &crate::model::Assignment)> =
        report.reading.assignments.iter().enumerate().collect();
    rows.sort_by_key(|(_, a)| (a.date, a.source));
    for (i, a) in rows {
        let (person, site) = report
            .week
            .cleaned
            .get(i)
            .cloned()
            .unwrap_or((a.person.clone(), a.site.clone()));
        let part = a.part.map(|p| match p {
            crate::model::Part::Am => "AM",
            crate::model::Part::Pm => "PM",
            crate::model::Part::Half => "Half day",
        });
        sh.plain([
            a.date.format("%Y-%m-%d").to_string(),
            a.date.format("%a").to_string(),
            person.unwrap_or_default(),
            site.unwrap_or_default(),
            a.status.map(status_label).unwrap_or_default().to_string(),
            part.unwrap_or_default().to_string(),
            a.start.map(crate::times::fmt).unwrap_or_default(),
            a.end.map(crate::times::fmt).unwrap_or_default(),
            a.note.clone().unwrap_or_default(),
            if a.unstaffed { "Yes" } else { "" }.to_string(),
            a.person.clone().unwrap_or_default(),
            a.site.clone().unwrap_or_default(),
        ]);
    }
    sh
}

fn matches_sheet(report: &Report, session: &Session) -> SheetXml {
    let mut sh = SheetXml::new("Name and site matches", &[10.0, 30.0, 30.0, 34.0]);
    sh.header(&["Kind", "Written as", "Treated as", "Why"]);
    for m in &report.merges {
        let kind = match m.kind {
            crate::matching::MergeKind::Person => "Person",
            crate::matching::MergeKind::Site => "Job",
        };
        sh.plain([
            kind.to_string(),
            m.from.clone(),
            m.into.clone(),
            describe::reason(m.reason).to_string(),
        ]);
    }
    if report.merges.is_empty() {
        sh.plain(["", "No names or jobs were matched.", "", ""]);
    }
    if !session.answers.is_empty() {
        sh.blank();
        sh.header(&["Your answers", "Question", "Answer", ""]);
        for (q, a) in &session.answers.items {
            sh.row([
                ("".to_string(), Style::Plain),
                (describe::question(q).text, Style::Wrap),
                (describe::answer(a), Style::Plain),
                ("".to_string(), Style::Plain),
            ]);
        }
    }
    sh
}

fn state_sheet(session: &Session) -> SheetXml {
    let mut sh = SheetXml::new(STATE_SHEET, &[]);
    sh.hidden = true;
    let json = session.to_json();
    let chars: Vec<char> = json.chars().collect();
    for chunk in chars.chunks(CHUNK) {
        sh.plain([chunk.iter().collect::<String>()]);
    }
    sh
}

/// The saved workbook's bytes.
pub fn workbook(session: &Session, report: &Report) -> Vec<u8> {
    let sheets = [
        findings_sheet(report),
        week_sheet(report),
        cleaned_sheet(report),
        matches_sheet(report, session),
        state_sheet(session),
    ];
    let mut content_types = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
    );
    let mut book = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><bookViews><workbookView activeTab="0"/></bookViews><sheets>"#,
    );
    let mut rels = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for (i, sh) in sheets.iter().enumerate() {
        let n = i + 1;
        content_types.push_str(&format!(
            r#"<Override PartName="/xl/worksheets/sheet{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        ));
        let state = if sh.hidden {
            r#" state="veryHidden""#
        } else {
            ""
        };
        book.push_str(&format!(
            r#"<sheet name="{}" sheetId="{n}"{state} r:id="rId{n}"/>"#,
            xml_escape(sh.name)
        ));
        rels.push_str(&format!(
            r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{n}.xml"/>"#
        ));
    }
    let styles_id = sheets.len() + 1;
    rels.push_str(&format!(
        r#"<Relationship Id="rId{styles_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#
    ));
    content_types.push_str("</Types>");
    book.push_str("</sheets></workbook>");
    let root_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut put = |name: &str, body: &str| {
        zip.start_file(name, options).expect("zip entry");
        zip.write_all(body.as_bytes()).expect("zip write");
    };
    put("[Content_Types].xml", &content_types);
    put("_rels/.rels", root_rels);
    put("xl/workbook.xml", &book);
    put("xl/_rels/workbook.xml.rels", &rels);
    put("xl/styles.xml", STYLES);
    for (i, sh) in sheets.iter().enumerate() {
        put(&format!("xl/worksheets/sheet{}.xml", i + 1), &sh.xml());
    }
    zip.finish().expect("zip finish").into_inner()
}

/// A saved check's session, if `bytes` is one.
pub fn restore(bytes: &[u8]) -> Option<Session> {
    use calamine::{Data, Reader};
    if !bytes.starts_with(b"PK\x03\x04") {
        return None;
    }
    let mut wb = calamine::open_workbook_auto_from_rs(Cursor::new(bytes)).ok()?;
    if !wb.sheet_names().iter().any(|n| n == STATE_SHEET) {
        return None;
    }
    let range = wb.worksheet_range(STATE_SHEET).ok()?;
    let mut json = String::new();
    for row in range.rows() {
        if let Some(Data::String(s)) = row.first() {
            json.push_str(s);
        }
    }
    Session::from_json(&json)
}

/// Guards a CSV cell against formula injection.
fn csv_safe(text: &str) -> String {
    if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}")
    } else {
        text.to_string()
    }
}

/// The findings as CSV (UTF-8 with a byte-order mark, so Excel shows
/// accents right).
pub fn findings_csv(report: &Report) -> Vec<u8> {
    let mut w = csv::Writer::from_writer(Vec::from(&b"\xEF\xBB\xBF"[..]));
    w.write_record([
        "Finding",
        "Date",
        "Person",
        "Job",
        "Details",
        "Matches used",
    ])
    .expect("csv");
    for row in finding_rows(report) {
        w.write_record(row.iter().map(|c| csv_safe(c)))
            .expect("csv");
    }
    w.into_inner().expect("csv")
}

/// Writes `bytes` to `path` all at once: into a temporary file beside it,
/// then renamed over it, so a failure never leaves half a file.
pub fn save(path: &Path, bytes: &[u8]) -> Result<(), SaveError> {
    let fail = |e: std::io::Error| {
        let why = match e.kind() {
            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem => {
                "That folder can't be written to. Choose another place, such as Documents."
                    .to_string()
            }
            std::io::ErrorKind::NotFound => "That folder no longer exists.".to_string(),
            std::io::ErrorKind::StorageFull => "The disk is full.".to_string(),
            _ => format!("The file couldn't be saved ({e})."),
        };
        SaveError::SaveFailed(why)
    };
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "schedule-check".into());
    let tmp = dir.join(format!(".{file_name}.{}.saving", std::process::id()));
    let written = std::fs::File::create(&tmp).and_then(|mut f| {
        f.write_all(bytes)?;
        f.sync_all()
    });
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(fail(e));
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        fail(e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Answer, Question};

    #[test]
    fn letters() {
        assert_eq!(column_letters(0), "A");
        assert_eq!(column_letters(25), "Z");
        assert_eq!(column_letters(26), "AA");
        assert_eq!(column_letters(701), "ZZ");
        assert_eq!(column_letters(702), "AAA");
    }

    #[test]
    fn save_and_restore() {
        let mut s = Session::example(Settings::default());
        s.answer(
            Question::SameSite {
                a: "1420 Johnson Street".into(),
                b: "Johnson job".into(),
            },
            Answer::Yes,
        );
        let report = s.report_model_off();
        let bytes = workbook(&s, &report);
        let back = restore(&bytes).expect("restores");
        assert_eq!(back.answers.items, s.answers.items);
        assert_eq!(back.source, s.source);
        // The visible sheets read back too.
        use calamine::Reader;
        let mut wb = calamine::open_workbook_auto_from_rs(Cursor::new(&bytes[..])).unwrap();
        let names = wb.sheet_names();
        assert_eq!(
            &names[..4],
            [
                "Findings",
                "Week grid",
                "Cleaned schedule",
                "Name and site matches"
            ]
        );
        let findings = wb.worksheet_range("Findings").unwrap();
        assert_eq!(
            findings.get_value((1, 0)),
            Some(&calamine::Data::String(
                flag_label(report.flags[0].kind).into()
            ))
        );
    }

    #[test]
    fn big_state_splits_over_cells() {
        let big = "Name\tMon 10/12\n".to_string() + &"Ana Ruiz\tSmith remodel\n".repeat(5000);
        let s = Session::new(
            crate::session::Source::Paste { text: big },
            Settings::default(),
        );
        let bytes = workbook(&s, &s.report_model_off());
        assert_eq!(restore(&bytes).unwrap().source, s.source);
    }

    #[test]
    fn csv_guards_formulas() {
        assert_eq!(csv_safe("=SUM(A1)"), "'=SUM(A1)");
        assert_eq!(csv_safe("-5"), "'-5");
        assert_eq!(csv_safe("Mike"), "Mike");
    }
}
