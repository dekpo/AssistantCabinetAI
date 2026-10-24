//! Reads, from the XML of an `.xlsx` or an `.ods`, what `calamine` does not expose: the rows and
//! columns an application hides, the active autofilter, the merged cells, the Excel Tables
//! (TABULAR step 13, design note section 2).
//!
//! One streaming pass per sheet, no new crate (`zip` and `quick-xml` were already dependencies).
//! Coordinates are the sheet's own, 0-based; the caller adds the grid's origin. A file this module
//! cannot parse gives no facts rather than an error: the cells were read, and a reading without
//! layout facts is what an older inventory already is.

use std::collections::BTreeMap;
use std::io::{BufReader, Read};
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use super::layout::{ExcelTable, FactsBySheet, MergedRange, SheetFacts, SheetState};

/// The most hidden rows kept per sheet: a sheet that hides more than this keeps the count only
/// through the first rows, which is already far more than anyone checks by eye.
const MAX_HIDDEN_ROWS: usize = 1_000_000;

fn attr(tag: &BytesStart<'_>, local: &[u8]) -> Option<String> {
    tag.attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().as_ref() == local)
        .and_then(|attribute| {
            attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.into_owned())
        })
}

fn is_true(value: &str) -> bool {
    matches!(value, "1" | "true")
}

/// `B7` -> (row 6, column 1).
fn parse_cell(reference: &str) -> Option<(u32, u32)> {
    let reference = reference.trim_matches('$');
    let split = reference.find(|ch: char| ch.is_ascii_digit())?;
    let (letters, digits) = reference.split_at(split);
    if letters.is_empty() {
        return None;
    }
    let mut column = 0u32;
    for ch in letters.trim_matches('$').chars() {
        if !ch.is_ascii_alphabetic() {
            return None;
        }
        column = column * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
    }
    let row: u32 = digits.trim_matches('$').parse().ok()?;
    Some((row.checked_sub(1)?, column.checked_sub(1)?))
}

/// `A1:F10` (or one cell) -> (r1, c1, r2, c2).
fn parse_range(reference: &str) -> Option<(u32, u32, u32, u32)> {
    let mut parts = reference.split(':');
    let first = parse_cell(parts.next()?)?;
    let last = match parts.next() {
        Some(second) => parse_cell(second)?,
        None => first,
    };
    Some((
        first.0.min(last.0),
        first.1.min(last.1),
        first.0.max(last.0),
        first.1.max(last.1),
    ))
}

// --- .xlsx --------------------------------------------------------------------------------------

fn zip_text(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<Vec<u8>> {
    let mut entry = archive.by_name(name).ok()?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// `../tables/table1.xml` from a sheet at `xl/worksheets/sheet1.xml` -> `xl/tables/table1.xml`.
fn resolve(base_dir: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return absolute.to_string();
    }
    let mut parts: Vec<&str> = base_dir
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    for part in target.split('/') {
        match part {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn relationships(bytes: &[u8]) -> Vec<(String, String, String)> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(tag)) | Ok(Event::Empty(tag))
                if tag.local_name().as_ref() == b"Relationship" =>
            {
                if let (Some(id), Some(kind), Some(target)) = (
                    attr(&tag, b"Id"),
                    attr(&tag, b"Type"),
                    attr(&tag, b"Target"),
                ) {
                    out.push((id, kind, target));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

struct SheetXml {
    hidden_rows: Vec<u32>,
    hidden_cols: Vec<u32>,
    merged: Vec<MergedRange>,
    filter_active: bool,
    table_rids: Vec<String>,
}

fn read_sheet_xml<R: std::io::BufRead>(source: R) -> Option<SheetXml> {
    let mut reader = Reader::from_reader(source);
    reader.config_mut().trim_text(true);
    let mut out = SheetXml {
        hidden_rows: Vec::new(),
        hidden_cols: Vec::new(),
        merged: Vec::new(),
        filter_active: false,
        table_rids: Vec::new(),
    };
    let mut buf = Vec::new();
    let mut next_row = 0u32;
    let mut in_auto_filter = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(tag)) | Ok(Event::Empty(tag)) => match tag.local_name().as_ref() {
                b"row" => {
                    let row = attr(&tag, b"r")
                        .and_then(|value| value.parse::<u32>().ok())
                        .map(|number| number - 1)
                        .unwrap_or(next_row);
                    next_row = row + 1;
                    if attr(&tag, b"hidden").is_some_and(|value| is_true(&value))
                        && out.hidden_rows.len() < MAX_HIDDEN_ROWS
                    {
                        out.hidden_rows.push(row);
                    }
                }
                b"col" => {
                    if attr(&tag, b"hidden").is_some_and(|value| is_true(&value)) {
                        let min = attr(&tag, b"min")
                            .and_then(|v| v.parse::<u32>().ok())
                            .unwrap_or(1);
                        let max = attr(&tag, b"max")
                            .and_then(|v| v.parse::<u32>().ok())
                            .unwrap_or(min);
                        // A whole-sheet `max` (16384) means "the rest of the sheet", not 16k columns.
                        for col in min..=max.min(min + 512) {
                            out.hidden_cols.push(col - 1);
                        }
                    }
                }
                b"mergeCell" => {
                    if let Some((r1, c1, r2, c2)) = attr(&tag, b"ref").and_then(|r| parse_range(&r))
                    {
                        out.merged.push(MergedRange { r1, c1, r2, c2 });
                    }
                }
                b"autoFilter" => in_auto_filter = true,
                b"filterColumn" if in_auto_filter => out.filter_active = true,
                b"tablePart" => {
                    if let Some(id) = attr(&tag, b"id") {
                        out.table_rids.push(id);
                    }
                }
                _ => {}
            },
            Ok(Event::End(tag)) if tag.local_name().as_ref() == b"autoFilter" => {
                in_auto_filter = false
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
    Some(out)
}

fn read_table_xml(bytes: &[u8]) -> Option<(ExcelTable, bool)> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut table: Option<ExcelTable> = None;
    let mut filtered = false;
    let mut in_filter = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(tag)) | Ok(Event::Empty(tag)) => match tag.local_name().as_ref() {
                b"table" if table.is_none() => {
                    let (r1, c1, r2, c2) = attr(&tag, b"ref").and_then(|r| parse_range(&r))?;
                    let name = attr(&tag, b"displayName")
                        .or_else(|| attr(&tag, b"name"))
                        .unwrap_or_default();
                    let totals = attr(&tag, b"totalsRowCount")
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(0)
                        > 0
                        || attr(&tag, b"totalsRowShown").is_some_and(|v| is_true(&v));
                    table = Some(ExcelTable {
                        name,
                        r1,
                        c1,
                        r2,
                        c2,
                        totals_row: totals,
                    });
                }
                b"autoFilter" => in_filter = true,
                b"filterColumn" if in_filter => filtered = true,
                _ => {}
            },
            Ok(Event::End(tag)) if tag.local_name().as_ref() == b"autoFilter" => in_filter = false,
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
    table.map(|table| (table, filtered))
}

fn xlsx_facts(path: &Path) -> Option<FactsBySheet> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let workbook = zip_text(&mut archive, "xl/workbook.xml")?;
    let rels = relationships(&zip_text(&mut archive, "xl/_rels/workbook.xml.rels")?);

    let mut sheets: Vec<(String, String, SheetState)> = Vec::new();
    {
        let mut reader = Reader::from_reader(workbook.as_slice());
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(tag)) | Ok(Event::Empty(tag))
                    if tag.local_name().as_ref() == b"sheet" =>
                {
                    if let (Some(name), Some(rid)) = (attr(&tag, b"name"), attr(&tag, b"id")) {
                        let state = match attr(&tag, b"state").as_deref() {
                            Some("hidden") => SheetState::Hidden,
                            Some("veryHidden") => SheetState::VeryHidden,
                            _ => SheetState::Visible,
                        };
                        sheets.push((name, rid, state));
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
            buf.clear();
        }
    }

    let mut out = FactsBySheet::new();
    for (name, rid, state) in sheets {
        let Some((_, _, target)) = rels.iter().find(|(id, _, _)| *id == rid) else {
            continue;
        };
        let sheet_path = resolve("xl", target);
        let Ok(entry) = archive.by_name(&sheet_path) else {
            continue;
        };
        let Some(xml) = read_sheet_xml(BufReader::new(entry)) else {
            continue;
        };
        let mut facts = SheetFacts {
            state,
            hidden_rows: xml.hidden_rows,
            hidden_cols: xml.hidden_cols,
            filter_active: xml.filter_active,
            merged: xml.merged,
            ..SheetFacts::default()
        };
        if !xml.table_rids.is_empty() {
            let dir = sheet_path
                .rsplit_once('/')
                .map_or("", |(dir, _)| dir)
                .to_string();
            let file_name = sheet_path.rsplit('/').next().unwrap_or("");
            let rels_path = format!("{dir}/_rels/{file_name}.rels");
            if let Some(bytes) = zip_text(&mut archive, &rels_path) {
                for (id, _, target) in relationships(&bytes) {
                    if !xml.table_rids.contains(&id) {
                        continue;
                    }
                    let table_path = resolve(&dir, &target);
                    if let Some(table_xml) = zip_text(&mut archive, &table_path) {
                        if let Some((table, filtered)) = read_table_xml(&table_xml) {
                            facts.filter_active |= filtered;
                            facts.tables.push(table);
                        }
                    }
                }
            }
        }
        out.insert(name, facts);
    }
    Some(out)
}

// --- .ods ---------------------------------------------------------------------------------------

fn ods_facts(path: &Path) -> Option<FactsBySheet> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let entry = archive.by_name("content.xml").ok()?;
    let mut reader = Reader::from_reader(BufReader::new(entry));
    reader.config_mut().trim_text(false);

    let mut hidden_styles: BTreeMap<String, bool> = BTreeMap::new();
    let mut current_style: Option<String> = None;
    let mut out = FactsBySheet::new();
    let mut sheet: Option<(String, SheetFacts)> = None;
    // `row` is the next row to start, `row_start` the one now open.
    let mut row = 0u32;
    let mut row_start = 0u32;
    let mut col = 0u32;
    let mut column_def = 0u32;
    let mut group_hidden: Vec<bool> = Vec::new();
    let mut filter_target: Option<String> = None;
    let mut buf = Vec::new();
    // A repeated empty row is not a thousand rows of facts: a hidden repeat is capped.
    let cap = |repeat: u32| repeat.min(10_000);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(tag)) | Ok(Event::Empty(tag)) => match tag.local_name().as_ref() {
                b"style" => current_style = attr(&tag, b"name"),
                b"table-properties" => {
                    if let (Some(style), Some(display)) = (&current_style, attr(&tag, b"display")) {
                        hidden_styles.insert(style.clone(), display == "false");
                    }
                }
                b"table" => {
                    if let Some(name) = attr(&tag, b"name") {
                        let hidden = attr(&tag, b"style-name")
                            .and_then(|style| hidden_styles.get(&style).copied())
                            .unwrap_or(false);
                        let facts = SheetFacts {
                            state: if hidden {
                                SheetState::Hidden
                            } else {
                                SheetState::Visible
                            },
                            ..SheetFacts::default()
                        };
                        sheet = Some((name, facts));
                        row = 0;
                        row_start = 0;
                        column_def = 0;
                        group_hidden.clear();
                    }
                }
                b"table-row-group" => {
                    group_hidden.push(attr(&tag, b"display").is_some_and(|v| v == "false"));
                }
                b"table-column" => {
                    let repeat = attr(&tag, b"number-columns-repeated")
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(1);
                    if let Some((_, facts)) = sheet.as_mut() {
                        if attr(&tag, b"visibility")
                            .is_some_and(|v| v == "collapse" || v == "filter")
                        {
                            for offset in 0..repeat.min(512) {
                                facts.hidden_cols.push(column_def + offset);
                            }
                        }
                    }
                    column_def += repeat;
                }
                b"table-row" => {
                    let repeat = attr(&tag, b"number-rows-repeated")
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(1);
                    let hidden = group_hidden.iter().any(|hidden| *hidden)
                        || attr(&tag, b"visibility")
                            .is_some_and(|v| v == "collapse" || v == "filter");
                    if let Some((_, facts)) = sheet.as_mut() {
                        if hidden {
                            for offset in 0..cap(repeat) {
                                facts.hidden_rows.push(row + offset);
                            }
                        }
                    }
                    col = 0;
                    row_start = row;
                    row += repeat;
                }
                b"table-cell" | b"covered-table-cell" => {
                    let repeat = attr(&tag, b"number-columns-repeated")
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(1);
                    let columns = attr(&tag, b"number-columns-spanned")
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(1);
                    let rows = attr(&tag, b"number-rows-spanned")
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(1);
                    if (columns > 1 || rows > 1) && repeat == 1 {
                        if let Some((_, facts)) = sheet.as_mut() {
                            facts.merged.push(MergedRange {
                                r1: row_start,
                                c1: col,
                                r2: row_start + rows - 1,
                                c2: col + columns - 1,
                            });
                        }
                    }
                    col += repeat;
                }
                b"database-range" => {
                    filter_target = attr(&tag, b"target-range-address");
                }
                b"filter" => {
                    if let (Some(target), Some((name, facts))) = (&filter_target, sheet.as_mut()) {
                        let sheet_part = target.split('.').next().unwrap_or("");
                        let clean = sheet_part
                            .trim_matches('\'')
                            .trim_start_matches('$')
                            .to_string();
                        if clean == *name {
                            facts.filter_active = true;
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::End(tag)) => match tag.local_name().as_ref() {
                b"style" => current_style = None,
                b"table-row-group" => {
                    group_hidden.pop();
                }
                b"table" => {
                    if let Some((name, facts)) = sheet.take() {
                        out.insert(name, facts);
                    }
                }
                b"database-range" => filter_target = None,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
    Some(out)
}

/// The layout facts of a file, by sheet name. `None` for a format this module does not read
/// (`.xls`, `.csv`) or a file it could not parse.
pub fn read_facts(path: &Path) -> Option<FactsBySheet> {
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "xlsx" | "xlsm" => xlsx_facts(path),
        "ods" => ods_facts(path),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_reference_is_zero_based_row_then_column() {
        assert_eq!(parse_cell("A1"), Some((0, 0)));
        assert_eq!(parse_cell("B7"), Some((6, 1)));
        assert_eq!(parse_cell("AA3"), Some((2, 26)));
        assert_eq!(parse_cell("$C$4"), Some((3, 2)));
        assert_eq!(parse_cell("7"), None);
    }

    #[test]
    fn a_range_is_normalised_and_one_cell_is_a_range_of_one() {
        assert_eq!(parse_range("B2:D4"), Some((1, 1, 3, 3)));
        assert_eq!(parse_range("D4:B2"), Some((1, 1, 3, 3)));
        assert_eq!(parse_range("C3"), Some((2, 2, 2, 2)));
    }

    #[test]
    fn a_relative_target_is_resolved_against_the_sheet_directory() {
        assert_eq!(
            resolve("xl/worksheets", "../tables/table1.xml"),
            "xl/tables/table1.xml"
        );
        assert_eq!(
            resolve("xl", "worksheets/sheet1.xml"),
            "xl/worksheets/sheet1.xml"
        );
        assert_eq!(
            resolve("xl", "/xl/worksheets/sheet1.xml"),
            "xl/worksheets/sheet1.xml"
        );
    }

    #[test]
    fn a_sheet_xml_gives_its_hidden_rows_columns_merges_and_active_filter() {
        let xml = br#"<worksheet><cols><col min="3" max="3" hidden="1"/></cols><sheetData>
            <row r="1"/><row r="4" hidden="1"/><row r="5" hidden="true"/></sheetData>
            <autoFilter ref="A1:C6"><filterColumn colId="0"><filters/></filterColumn></autoFilter>
            <mergeCells count="1"><mergeCell ref="B2:B3"/></mergeCells></worksheet>"#;
        let facts = read_sheet_xml(&xml[..]).unwrap();
        assert_eq!(facts.hidden_rows, vec![3, 4]);
        assert_eq!(facts.hidden_cols, vec![2]);
        assert!(facts.filter_active);
        assert_eq!(
            facts.merged,
            vec![MergedRange {
                r1: 1,
                c1: 1,
                r2: 2,
                c2: 1
            }]
        );
    }

    #[test]
    fn an_autofilter_without_a_criterion_is_not_an_active_filter() {
        let xml = br#"<worksheet><sheetData><row r="2" hidden="1"/></sheetData><autoFilter ref="A1:C6"/></worksheet>"#;
        let facts = read_sheet_xml(&xml[..]).unwrap();
        assert!(!facts.filter_active);
        assert_eq!(facts.hidden_rows, vec![1]);
    }

    #[test]
    fn a_table_part_gives_its_name_range_and_totals_row() {
        let xml = br#"<table name="T1" displayName="Factures" ref="B2:D9" totalsRowCount="1"><autoFilter ref="B2:D8"/></table>"#;
        let (table, filtered) = read_table_xml(xml).unwrap();
        assert_eq!(table.name, "Factures");
        assert_eq!((table.r1, table.c1, table.r2, table.c2), (1, 1, 8, 3));
        assert!(table.totals_row);
        assert!(!filtered);
    }
}
