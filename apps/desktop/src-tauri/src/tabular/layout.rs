//! What a spreadsheet application shows and the grid of cells does not say: hidden and filtered
//! rows, merged cells, notes around a table, a total row, several tables on one sheet
//! (TABULAR step 13, `docs/test-reports/tabular-named-files/design/13-real-world-spreadsheets.md`).
//!
//! The adapters read the **facts** of a file ([`SheetFacts`]: which rows are hidden, which cells
//! are merged, what state a sheet is in, which Excel Tables it holds). They are structure only, are
//! persisted on the inventory, and are never part of the typed cell cache, which keeps the raw
//! grid. [`prepare_sheet`] is the one pure function that turns a raw sheet and its facts into the
//! sheet the engine reads, under a [`RowPolicy`]:
//!
//! - a vertical merge inside a column is filled down, so the value is on every row the user sees it on;
//! - a fully blank row inside a table is ignored, a note block after the table is left out;
//! - a total row at the foot of the table (a locale total word, no other text) is left out;
//! - a placeholder (`-`, `n/a`) in a column that is otherwise numbers or dates reads as empty;
//! - hidden and filtered rows are kept (they are in the file) or removed, by the policy;
//! - a sheet holding several tables is split into one sheet per table, named after its range, so
//!   the existing "which sheet?" question is the "which table?" question.
//!
//! Nothing here reads a file or the network, and every count it returns is a count, never a value.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::card::column_letter;
use super::inventory::{self, CellClass};
use super::{CellValue, SheetData};
use crate::file_reference::fold_text;

/// Written on every inventory this module has seen. `0` (the serde default) is an inventory
/// analysed before this module existed: its layout facts were never read, which is not the same
/// as "nothing found".
pub const LAYOUT_VERSION: u32 = 1;

/// What to do with the rows a spreadsheet hides or filters out. A figure includes them and says
/// so; rows acted on or sent (a listing, a sample, a mail merge) exclude them and say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowPolicy {
    #[default]
    Include,
    Exclude,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetState {
    #[default]
    Visible,
    Hidden,
    VeryHidden,
}

/// A merged region, 0-based and inclusive, in the sheet's own coordinates (not the grid's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergedRange {
    pub r1: u32,
    pub c1: u32,
    pub r2: u32,
    pub c2: u32,
}

/// An Excel Table (`ListObject`): the exact region of a table, header row included, in the
/// sheet's own coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExcelTable {
    pub name: String,
    pub r1: u32,
    pub c1: u32,
    pub r2: u32,
    pub c2: u32,
    /// The table ends with a totals row of its own (`totalsRowCount`).
    #[serde(default)]
    pub totals_row: bool,
}

/// Everything an adapter could read about one sheet beyond its cells. `Default` is a visible sheet
/// with nothing hidden, merged or filtered: what a CSV file is, and what an inventory analysed
/// before this module existed is read as.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SheetFacts {
    pub state: SheetState,
    /// The sheet's own row and column of the grid's first cell (0-based): the grid starts at the
    /// first used cell, not at A1.
    pub origin_row: u32,
    pub origin_col: u32,
    /// 0-based rows and columns the application hides.
    pub hidden_rows: Vec<u32>,
    pub hidden_cols: Vec<u32>,
    /// An autofilter with at least one active criterion: the hidden rows are then filtered out.
    pub filter_active: bool,
    pub merged: Vec<MergedRange>,
    pub tables: Vec<ExcelTable>,
}

/// What [`prepare_sheet`] did to one sheet, as counts. Stored on the sheet's inventory; the
/// card, the usage line and the calculation record are written from it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SheetLayout {
    /// The sheet of the file this one comes from (a table of a sheet holding several is a sheet
    /// of its own here).
    pub raw_sheet: String,
    pub state: SheetState,
    /// The range of the table on the raw sheet, written when it is not the whole sheet.
    pub range: Option<String>,
    /// How many tables the raw sheet holds (1 for an ordinary sheet).
    pub tables_on_sheet: usize,
    /// The sheet's column the grid's first column is (0 = A).
    pub origin_col: u32,
    /// The header's row as the spreadsheet numbers it (1 = first row).
    pub header_source_row: Option<u32>,
    /// Data rows hidden by hand or by a filter, whatever the policy did.
    pub hidden_rows: usize,
    pub filter_active: bool,
    /// Of those, the rows this view removed (the policy was `Exclude`).
    pub hidden_excluded: usize,
    /// The first of them as the spreadsheet numbers them (for the calculation record).
    pub hidden_row_numbers: Vec<u32>,
    pub hidden_columns: usize,
    /// Vertical merges filled down.
    pub merged_filled: usize,
    pub blank_rows_ignored: usize,
    /// Rows of notes after the table, left out.
    pub note_rows_left_out: usize,
    /// Total rows left out, and where they were.
    pub total_rows: usize,
    pub total_row_numbers: Vec<u32>,
    /// Placeholder cells read as empty.
    pub placeholders_blanked: usize,
    pub excel_table: Option<String>,
    /// The file row of each data row, written only when those rows are not consecutive after the
    /// header (rows were removed or the table is cut into parts); `None` means "the rows after the
    /// header are numbered one after the other".
    pub row_numbers: Option<Vec<u32>>,
}

impl SheetLayout {
    /// Nothing about this sheet differs from what a plain reading of its grid gives.
    pub fn is_plain(&self) -> bool {
        self.hidden_rows == 0
            && self.hidden_columns == 0
            && self.merged_filled == 0
            && self.blank_rows_ignored == 0
            && self.note_rows_left_out == 0
            && self.total_rows == 0
            && self.placeholders_blanked == 0
            && self.tables_on_sheet <= 1
            && self.state == SheetState::Visible
    }
}

/// One sheet as the engine reads it, with the row of the file each of its rows came from.
#[derive(Debug, Clone)]
pub struct PreparedSheet {
    pub data: SheetData,
    /// 1-based, one per row of `data.rows`.
    pub source_rows: Vec<u32>,
    pub layout: SheetLayout,
}

// --- Words (per-locale data, never literals) ---------------------------------------------------

/// Total words and empty-cell markers of every shipped pack, folded. A workbook's own words are in
/// the language it was exported in, whatever the interface language is (the same rule as the
/// column words of `tabular::question`), so every pack is read at once.
pub(crate) struct LayoutWords {
    pub totals: Vec<String>,
    pub empties: Vec<String>,
}

const PACKS: &[&str] = &[
    include_str!("../../resources/tabular-questions/en-US.json"),
    include_str!("../../resources/tabular-questions/fr-FR.json"),
];

pub(crate) fn layout_words() -> &'static LayoutWords {
    static WORDS: OnceLock<LayoutWords> = OnceLock::new();
    WORDS.get_or_init(|| {
        let mut totals = Vec::new();
        let mut empties = Vec::new();
        for body in PACKS {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
                continue;
            };
            for (key, target) in [
                ("total_words", &mut totals),
                ("empty_markers", &mut empties),
            ] {
                if let Some(list) = value.get(key).and_then(|list| list.as_array()) {
                    for word in list.iter().filter_map(|word| word.as_str()) {
                        let folded = fold_text(word);
                        if !folded.is_empty() && !target.contains(&folded) {
                            target.push(folded);
                        }
                    }
                }
            }
        }
        LayoutWords { totals, empties }
    })
}

// --- The preparation ----------------------------------------------------------------------------

fn is_blank(row: &[CellValue]) -> bool {
    row.iter()
        .all(|cell| matches!(inventory::classify_cell(cell), CellClass::Empty))
}

fn filled(row: &[CellValue]) -> usize {
    row.iter()
        .filter(|cell| !matches!(inventory::classify_cell(cell), CellClass::Empty))
        .count()
}

/// `A3:F49` from a region of the sheet, 0-based inclusive.
fn a1_range(r1: u32, c1: u32, r2: u32, c2: u32) -> String {
    format!(
        "{}{}:{}{}",
        column_letter(c1 as usize),
        r1 + 1,
        column_letter(c2 as usize),
        r2 + 1
    )
}

/// Fills the cells a vertical merge covers with the value of its top-left cell, in one column
/// only: the value a person reads on every row the merge spans. A merge over several columns is
/// a title or a header band, not a value, and is left alone.
fn fill_merged(rows: &mut [Vec<CellValue>], facts: &SheetFacts) -> usize {
    let mut filled_regions = 0;
    for merge in &facts.merged {
        if merge.c1 != merge.c2 || merge.r2 <= merge.r1 {
            continue;
        }
        let (Some(top), Some(col)) = (
            merge.r1.checked_sub(facts.origin_row),
            merge.c1.checked_sub(facts.origin_col),
        ) else {
            continue;
        };
        let (top, col) = (top as usize, col as usize);
        let Some(value) = rows.get(top).and_then(|row| row.get(col)).cloned() else {
            continue;
        };
        if matches!(value, CellValue::Empty) {
            continue;
        }
        let mut any = false;
        for offset in 1..=(merge.r2 - merge.r1) as usize {
            let Some(row) = rows.get_mut(top + offset) else {
                break;
            };
            if row.len() <= col {
                row.resize(col + 1, CellValue::Empty);
            }
            if matches!(row[col], CellValue::Empty) {
                row[col] = value.clone();
                any = true;
            }
        }
        if any {
            filled_regions += 1;
        }
    }
    filled_regions
}

/// A row that holds a total: a total word opens it and it carries no other text, only numbers or
/// formulas. "Total SA | Paris | 1200" is a supplier, not a total.
fn is_total_row(row: &[CellValue], words: &LayoutWords) -> bool {
    let mut label_seen = false;
    let mut numbers = 0;
    for (position, cell) in row.iter().enumerate() {
        match inventory::classify_cell(cell) {
            CellClass::Empty => {}
            CellClass::Numeric | CellClass::Date { .. } => numbers += 1,
            CellClass::Other => {
                if label_seen || position > 2 {
                    return false;
                }
                let text = match cell {
                    CellValue::Text(text) => text.clone(),
                    CellValue::Formula { .. } => return false,
                    _ => return false,
                };
                let folded = fold_text(text.trim());
                let is_total = words.totals.iter().any(|word| {
                    folded == *word
                        || folded
                            .strip_prefix(word.as_str())
                            .and_then(|rest| rest.chars().next())
                            .is_some_and(|next| !next.is_alphanumeric())
                });
                if !is_total {
                    return false;
                }
                label_seen = true;
            }
        }
    }
    label_seen && numbers > 0
}

/// A band of consecutive non-blank rows, as indices into the grid.
fn bands(rows: &[Vec<CellValue>]) -> Vec<std::ops::Range<usize>> {
    let mut out = Vec::new();
    let mut start = None;
    for (index, row) in rows.iter().enumerate() {
        match (is_blank(row), start) {
            (false, None) => start = Some(index),
            (true, Some(from)) => {
                out.push(from..index);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        out.push(from..rows.len());
    }
    out
}

/// The header's row inside a block of rows when the block reads as a table: a header-like row near
/// its top, with data below, most of it more than one cell wide.
fn table_header_in(block: &[Vec<CellValue>]) -> Option<usize> {
    let header = inventory::detect_header_row(block)?;
    if header > 3 || filled(&block[header]) < 2 {
        return None;
    }
    let data = &block[header + 1..];
    if data.is_empty() {
        return None;
    }
    let wide = data.iter().filter(|row| filled(row) >= 2).count();
    (wide * 2 >= data.len()).then_some(header)
}

fn class_of(row: &[CellValue], index: usize) -> CellClass {
    row.get(index)
        .map(inventory::classify_cell)
        .unwrap_or(CellClass::Empty)
}

/// Whether a block that is not a table of its own carries on the table above it: about as wide,
/// with most of its rows filled.
fn continues(block: &[Vec<CellValue>], width: usize) -> bool {
    if width < 2 {
        return false;
    }
    let wide = block
        .iter()
        .filter(|row| filled(row) * 2 >= width && filled(row) >= 2)
        .count();
    wide * 2 >= block.len()
}

/// Whether the block's first row reads as the header of another table rather than as one more
/// row of the table above: its width differs, or its cells are all text where the table above
/// holds numbers or dates in the same columns.
fn starts_another_table(
    block: &[Vec<CellValue>],
    header: usize,
    table_rows: &[&Vec<CellValue>],
    width: usize,
) -> bool {
    let first = &block[header];
    if filled(first).abs_diff(width) >= 2 {
        return true;
    }
    let mut contrast = 0;
    let mut compared = 0;
    for column in 0..width.max(first.len()) {
        let mut typed = 0;
        let mut seen = 0;
        for row in table_rows.iter().skip(1) {
            match class_of(row, column) {
                CellClass::Empty => {}
                CellClass::Other => seen += 1,
                _ => {
                    seen += 1;
                    typed += 1;
                }
            }
        }
        if seen == 0 {
            continue;
        }
        compared += 1;
        if typed * 2 > seen && matches!(class_of(first, column), CellClass::Other) {
            contrast += 1;
        }
    }
    compared > 0 && contrast > 0
}

struct Region {
    /// Indices into the grid, in order.
    rows: Vec<usize>,
    /// Columns kept (all of them unless an Excel Table bounds the region).
    columns: Option<std::ops::RangeInclusive<usize>>,
    width: usize,
    table_name: Option<String>,
}

struct Plan {
    /// Rows before the first table that are kept as they are (a title above a single table).
    leading: Vec<usize>,
    regions: Vec<Region>,
    notes_left_out: usize,
    blank_ignored: usize,
}

fn plan_regions(rows: &[Vec<CellValue>], facts: &SheetFacts) -> Plan {
    // Excel Tables are exact: the region is what the file says it is.
    if !facts.tables.is_empty() {
        let mut regions = Vec::new();
        let mut covered = vec![false; rows.len()];
        for table in &facts.tables {
            let (Some(first), Some(c1), Some(c2)) = (
                table.r1.checked_sub(facts.origin_row),
                table.c1.checked_sub(facts.origin_col),
                table.c2.checked_sub(facts.origin_col),
            ) else {
                continue;
            };
            let mut last = table.r2.saturating_sub(facts.origin_row) as usize;
            if table.totals_row && last > first as usize {
                last -= 1;
            }
            let last = last.min(rows.len().saturating_sub(1));
            if first as usize >= rows.len() || last < first as usize {
                continue;
            }
            let indices: Vec<usize> = (first as usize..=last)
                .filter(|index| !is_blank(&rows[*index]))
                .collect();
            for flag in covered.iter_mut().take(last + 1).skip(first as usize) {
                *flag = true;
            }
            regions.push(Region {
                rows: indices,
                columns: Some(c1 as usize..=c2 as usize),
                width: (c2 - c1 + 1) as usize,
                table_name: Some(table.name.clone()),
            });
        }
        if !regions.is_empty() {
            let notes_left_out = rows
                .iter()
                .enumerate()
                .filter(|(index, row)| !covered[*index] && !is_blank(row))
                .count();
            return Plan {
                leading: Vec::new(),
                regions,
                notes_left_out,
                blank_ignored: 0,
            };
        }
    }

    let bands = bands(rows);
    if bands.len() <= 1 {
        // An ordinary sheet: one block, kept whole (leading and trailing blank rows apart).
        let rows_kept: Vec<usize> = bands
            .first()
            .map(|band| band.clone().collect())
            .unwrap_or_default();
        let width = rows_kept.iter().map(|&i| rows[i].len()).max().unwrap_or(0);
        return Plan {
            leading: Vec::new(),
            regions: vec![Region {
                rows: rows_kept,
                columns: None,
                width,
                table_name: None,
            }],
            notes_left_out: 0,
            blank_ignored: 0,
        };
    }

    let mut regions: Vec<Region> = Vec::new();
    let mut leading_end = 0;
    let mut notes_left_out = 0;
    let mut blank_ignored = 0;
    let mut previous_end = 0;
    for band in &bands {
        let block = &rows[band.clone()];
        let header = table_header_in(block);
        let current = regions.last();
        let is_new_table = match (header, current) {
            (Some(_), None) => true,
            (Some(h), Some(region)) => {
                let table_rows: Vec<&Vec<CellValue>> =
                    region.rows.iter().map(|&i| &rows[i]).collect();
                starts_another_table(block, h, &table_rows, region.width)
            }
            (None, _) => false,
        };
        if is_new_table {
            let width = block.iter().map(|row| filled(row)).max().unwrap_or(0);
            if regions.is_empty() {
                leading_end = band.start;
            }
            regions.push(Region {
                rows: band.clone().collect(),
                columns: None,
                width,
                table_name: None,
            });
        } else if let Some(region) = regions.last_mut() {
            if header.is_none() && continues(block, region.width) {
                blank_ignored += band.start - previous_end;
                region.rows.extend(band.clone());
            } else if header.is_some() {
                // A header-like block that is not clearly another table: the table above carries
                // on, and the repeated header row stays a data row the user can see.
                blank_ignored += band.start - previous_end;
                region.rows.extend(band.clone());
            } else {
                notes_left_out += band.len();
            }
        }
        // (A block before the first table, a title or a legend, is kept above a single table as
        // it was: `leading_end` below.)
        previous_end = band.end;
    }
    if regions.is_empty() {
        // No block reads as a table: keep the sheet whole, exactly as it was read before.
        return Plan {
            leading: Vec::new(),
            regions: vec![Region {
                rows: (0..rows.len()).collect(),
                columns: None,
                width: rows.iter().map(|row| filled(row)).max().unwrap_or(0),
                table_name: None,
            }],
            notes_left_out: 0,
            blank_ignored: 0,
        };
    }
    // Rows before the first table keep their place (blank rows included) so the header keeps the
    // row number it has in the file.
    Plan {
        leading: (0..leading_end).collect(),
        regions,
        notes_left_out,
        blank_ignored,
    }
}

/// A placeholder in a column that is otherwise numbers or dates reads as empty.
fn blank_placeholders(
    rows: &mut [Vec<CellValue>],
    header: Option<usize>,
    words: &LayoutWords,
) -> usize {
    let start = header.map_or(0, |h| h + 1);
    if start >= rows.len() {
        return 0;
    }
    let width = rows[start..].iter().map(Vec::len).max().unwrap_or(0);
    let mut blanked = 0;
    // A short text that is a marker is the only thing that can need work: find the columns that
    // hold one before classifying every cell of the sheet.
    let mut candidate_columns = std::collections::BTreeSet::new();
    for row in &rows[start..] {
        for (column, cell) in row.iter().enumerate() {
            if let CellValue::Text(text) = cell {
                let trimmed = text.trim();
                if trimmed.chars().count() <= 12
                    && words
                        .empties
                        .iter()
                        .any(|marker| *marker == fold_text(trimmed))
                {
                    candidate_columns.insert(column);
                }
            }
        }
    }
    for column in 0..width {
        if !candidate_columns.contains(&column) {
            continue;
        }
        let mut typed = 0;
        let mut other = 0;
        let mut placeholders = Vec::new();
        for (offset, row) in rows[start..].iter().enumerate() {
            let Some(cell) = row.get(column) else {
                continue;
            };
            match inventory::classify_cell(cell) {
                CellClass::Empty => {}
                CellClass::Numeric | CellClass::Date { .. } => typed += 1,
                CellClass::Other => {
                    let is_marker = match cell {
                        CellValue::Text(text) => {
                            let folded = fold_text(text.trim());
                            words.empties.contains(&folded)
                        }
                        _ => false,
                    };
                    if is_marker {
                        placeholders.push(start + offset);
                    } else {
                        other += 1;
                    }
                }
            }
        }
        if typed > 0 && !placeholders.is_empty() && (other as f64) <= 0.05 * (typed + other) as f64
        {
            for row_index in placeholders {
                rows[row_index][column] = CellValue::Empty;
                blanked += 1;
            }
        }
    }
    blanked
}

/// The sheet the engine reads: see the module documentation. `raw` is the grid as the adapter
/// read it, `source_rows` the 1-based row of the file each of its rows came from.
pub(crate) fn prepare_sheet(
    raw: &SheetData,
    source_rows: Option<&[u32]>,
    facts: &SheetFacts,
    policy: RowPolicy,
) -> Vec<PreparedSheet> {
    let words = layout_words();
    let mut rows = raw.rows.clone();
    let merged_filled = fill_merged(&mut rows, facts);

    let source_of = |index: usize| -> u32 {
        source_rows
            .and_then(|rows| rows.get(index).copied())
            .unwrap_or(facts.origin_row + index as u32 + 1)
    };
    let hidden: std::collections::BTreeSet<u32> = facts.hidden_rows.iter().copied().collect();
    let is_hidden = |index: usize| hidden.contains(&(source_of(index) - 1));

    let plan = plan_regions(&rows, facts);
    let several = plan.regions.len() > 1;

    plan.regions
        .iter()
        .map(|region| {
            let mut kept: Vec<usize> = Vec::new();
            if !several {
                kept.extend(plan.leading.iter().copied());
            }
            kept.extend(region.rows.iter().copied());
            let cropped: Vec<Vec<CellValue>> = kept
                .iter()
                .map(|&index| match &region.columns {
                    Some(columns) => {
                        let row = &rows[index];
                        let from = (*columns.start()).min(row.len());
                        let to = (*columns.end() + 1).min(row.len());
                        row[from..to].to_vec()
                    }
                    None => rows[index].clone(),
                })
                .collect();
            let mut grid = cropped;
            let mut numbers: Vec<u32> = kept.iter().map(|&index| source_of(index)).collect();
            let mut layout = SheetLayout {
                raw_sheet: raw.name.clone(),
                state: facts.state,
                origin_col: facts.origin_col
                    + region.columns.as_ref().map_or(0, |c| *c.start() as u32),
                tables_on_sheet: plan.regions.len(),
                filter_active: facts.filter_active,
                merged_filled,
                note_rows_left_out: plan.notes_left_out,
                blank_rows_ignored: plan.blank_ignored,
                excel_table: region.table_name.clone(),
                hidden_columns: facts
                    .hidden_cols
                    .iter()
                    .filter(|&&col| {
                        region.columns.as_ref().is_none_or(|c| {
                            (col as usize) >= *c.start() + facts.origin_col as usize
                                && (col as usize) <= *c.end() + facts.origin_col as usize
                        })
                    })
                    .count(),
                ..SheetLayout::default()
            };

            let header = inventory::detect_header_row(&grid);

            // A total row at the foot of the table, left out always (not a data row).
            let data_from = header.map_or(0, |h| h + 1);
            while grid.len() > data_from && is_total_row(&grid[grid.len() - 1], words) {
                let last = grid.len() - 1;
                layout.total_rows += 1;
                layout.total_row_numbers.push(numbers[last]);
                grid.pop();
                numbers.pop();
            }
            layout.total_row_numbers.reverse();

            // Hidden and filtered data rows: counted, then kept or removed by the policy.
            let mut hidden_numbers = Vec::new();
            let mut keep = vec![true; grid.len()];
            for index in data_from..grid.len() {
                let original = kept[index];
                if is_hidden(original) {
                    hidden_numbers.push(numbers[index]);
                    if policy == RowPolicy::Exclude {
                        keep[index] = false;
                    }
                }
            }
            layout.hidden_rows = hidden_numbers.len();
            if policy == RowPolicy::Exclude {
                layout.hidden_excluded = hidden_numbers.len();
            }
            hidden_numbers.truncate(20);
            layout.hidden_row_numbers = hidden_numbers;
            if keep.iter().any(|keep| !keep) {
                let mut i = 0;
                grid.retain(|_| {
                    let k = keep[i];
                    i += 1;
                    k
                });
                let mut i = 0;
                numbers.retain(|_| {
                    let k = keep[i];
                    i += 1;
                    k
                });
            }

            layout.placeholders_blanked = blank_placeholders(&mut grid, header, words);
            layout.header_source_row = header.and_then(|h| numbers.get(h).copied());
            if several || region.columns.is_some() {
                let rows_of: Vec<u32> = numbers.clone();
                if let (Some(first), Some(last)) = (rows_of.first(), rows_of.last()) {
                    let c1 = layout.origin_col;
                    let c2 = c1 + grid.iter().map(Vec::len).max().unwrap_or(1).max(1) as u32 - 1;
                    layout.range = Some(a1_range(first - 1, c1, last - 1, c2));
                }
            }

            let name = if several {
                match (&region.table_name, &layout.range) {
                    (Some(table), _) => format!("{} ({})", raw.name, table),
                    (None, Some(range)) => format!("{} {}", raw.name, range),
                    (None, None) => raw.name.clone(),
                }
            } else {
                raw.name.clone()
            };

            PreparedSheet {
                data: SheetData { name, rows: grid },
                source_rows: numbers,
                layout,
            }
        })
        .collect()
}

/// The facts of every sheet of a workbook, by the sheet's name in the file.
pub type FactsBySheet = BTreeMap<String, SheetFacts>;

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> CellValue {
        CellValue::Text(value.to_string())
    }

    fn number(value: f64) -> CellValue {
        CellValue::Number(value)
    }

    fn sheet(rows: Vec<Vec<CellValue>>) -> SheetData {
        SheetData {
            name: "Data".to_string(),
            rows,
        }
    }

    fn prepare(
        rows: Vec<Vec<CellValue>>,
        facts: &SheetFacts,
        policy: RowPolicy,
    ) -> Vec<PreparedSheet> {
        prepare_sheet(&sheet(rows), None, facts, policy)
    }

    fn header() -> Vec<CellValue> {
        vec![text("lib"), text("cat"), text("montant")]
    }

    fn line(lib: &str, cat: &str, amount: f64) -> Vec<CellValue> {
        vec![text(lib), text(cat), number(amount)]
    }

    #[test]
    fn an_ordinary_sheet_is_read_exactly_as_it_was() {
        let rows = vec![header(), line("a", "x", 1.0), line("b", "y", 2.0)];
        let prepared = prepare(rows.clone(), &SheetFacts::default(), RowPolicy::Include);
        assert_eq!(prepared.len(), 1);
        assert_eq!(prepared[0].data.rows, rows);
        assert_eq!(prepared[0].source_rows, vec![1, 2, 3]);
        assert!(prepared[0].layout.is_plain());
    }

    #[test]
    fn a_title_and_a_blank_row_above_the_table_keep_their_place() {
        let rows = vec![
            vec![text("Export du 14/03/2026")],
            vec![],
            header(),
            line("a", "x", 1.0),
            line("b", "y", 2.0),
        ];
        let prepared = prepare(rows.clone(), &SheetFacts::default(), RowPolicy::Include);
        assert_eq!(prepared.len(), 1);
        assert_eq!(prepared[0].data.rows, rows);
        assert_eq!(prepared[0].layout.header_source_row, Some(3));
    }

    #[test]
    fn a_hidden_row_is_kept_or_removed_by_the_policy_and_always_counted() {
        let rows = vec![
            header(),
            line("a", "x", 10.0),
            line("b", "x", 1000.0),
            line("c", "y", 5.0),
        ];
        let facts = SheetFacts {
            hidden_rows: vec![2],
            ..SheetFacts::default()
        };
        let included = prepare(rows.clone(), &facts, RowPolicy::Include);
        assert_eq!(included[0].data.rows.len(), 4);
        assert_eq!(included[0].layout.hidden_rows, 1);
        assert_eq!(included[0].layout.hidden_excluded, 0);

        let excluded = prepare(rows, &facts, RowPolicy::Exclude);
        assert_eq!(excluded[0].data.rows.len(), 3);
        assert_eq!(excluded[0].source_rows, vec![1, 2, 4]);
        assert_eq!(excluded[0].layout.hidden_rows, 1);
        assert_eq!(excluded[0].layout.hidden_excluded, 1);
        assert_eq!(excluded[0].layout.hidden_row_numbers, vec![3]);
    }

    #[test]
    fn a_vertical_merge_in_one_column_is_filled_down() {
        let mut second = line("b", "", 20.0);
        second[1] = CellValue::Empty;
        let rows = vec![
            header(),
            line("a", "Frais", 10.0),
            second,
            line("c", "Charges", 5.0),
        ];
        let facts = SheetFacts {
            merged: vec![MergedRange {
                r1: 1,
                c1: 1,
                r2: 2,
                c2: 1,
            }],
            ..SheetFacts::default()
        };
        let prepared = prepare(rows, &facts, RowPolicy::Include);
        assert_eq!(prepared[0].data.rows[2][1], text("Frais"));
        assert_eq!(prepared[0].layout.merged_filled, 1);
    }

    #[test]
    fn a_merged_title_band_is_not_filled() {
        let rows = vec![
            vec![text("Journal"), CellValue::Empty, CellValue::Empty],
            header(),
            line("a", "x", 1.0),
        ];
        let facts = SheetFacts {
            merged: vec![MergedRange {
                r1: 0,
                c1: 0,
                r2: 0,
                c2: 2,
            }],
            ..SheetFacts::default()
        };
        let prepared = prepare(rows.clone(), &facts, RowPolicy::Include);
        assert_eq!(prepared[0].data.rows, rows);
        assert_eq!(prepared[0].layout.merged_filled, 0);
    }

    #[test]
    fn a_labelled_total_row_at_the_foot_is_left_out_and_counted() {
        let rows = vec![
            header(),
            line("a", "x", 10.0),
            line("b", "y", 25.0),
            vec![text("Total"), CellValue::Empty, number(35.0)],
        ];
        let prepared = prepare(rows, &SheetFacts::default(), RowPolicy::Include);
        assert_eq!(prepared[0].data.rows.len(), 3);
        assert_eq!(prepared[0].layout.total_rows, 1);
        assert_eq!(prepared[0].layout.total_row_numbers, vec![4]);
    }

    #[test]
    fn a_supplier_whose_name_starts_with_total_is_not_a_total_row() {
        let rows = vec![
            header(),
            line("a", "x", 10.0),
            vec![text("Total SA"), text("Paris"), number(35.0)],
        ];
        let prepared = prepare(rows, &SheetFacts::default(), RowPolicy::Include);
        assert_eq!(prepared[0].data.rows.len(), 3);
        assert_eq!(prepared[0].layout.total_rows, 0);
    }

    #[test]
    fn a_blank_row_inside_the_table_is_ignored_and_a_note_after_it_is_left_out() {
        let rows = vec![
            header(),
            line("a", "x", 1.0),
            line("b", "y", 2.0),
            vec![],
            line("c", "x", 3.0),
            line("d", "y", 4.0),
            vec![],
            vec![text("Source: the accountant")],
        ];
        let prepared = prepare(rows, &SheetFacts::default(), RowPolicy::Include);
        assert_eq!(prepared.len(), 1);
        assert_eq!(prepared[0].data.rows.len(), 5);
        assert_eq!(prepared[0].layout.blank_rows_ignored, 1);
        assert_eq!(prepared[0].layout.note_rows_left_out, 1);
        assert_eq!(prepared[0].source_rows, vec![1, 2, 3, 5, 6]);
    }

    #[test]
    fn two_tables_on_one_sheet_become_two_sheets_named_after_their_range() {
        let rows = vec![
            header(),
            line("a", "x", 1.0),
            line("b", "y", 2.0),
            vec![],
            vec![text("fournisseur"), text("statut"), text("total")],
            vec![text("X"), text("paye"), number(100.0)],
            vec![text("Y"), text("du"), number(200.0)],
        ];
        let prepared = prepare(rows, &SheetFacts::default(), RowPolicy::Include);
        assert_eq!(prepared.len(), 2);
        assert_eq!(prepared[0].data.name, "Data A1:C3");
        assert_eq!(prepared[1].data.name, "Data A5:C7");
        assert_eq!(prepared[0].layout.tables_on_sheet, 2);
        assert_eq!(prepared[1].data.rows.len(), 3);
        assert_eq!(prepared[1].layout.header_source_row, Some(5));
    }

    #[test]
    fn an_excel_table_bounds_the_region_and_the_rest_is_a_note() {
        let rows = vec![
            vec![
                text("Title"),
                CellValue::Empty,
                CellValue::Empty,
                CellValue::Empty,
            ],
            vec![CellValue::Empty, text("lib"), text("cat"), text("montant")],
            vec![CellValue::Empty, text("a"), text("x"), number(1.0)],
            vec![CellValue::Empty, text("b"), text("y"), number(2.0)],
            vec![text("footer note")],
        ];
        let facts = SheetFacts {
            tables: vec![ExcelTable {
                name: "T1".to_string(),
                r1: 1,
                c1: 1,
                r2: 3,
                c2: 3,
                totals_row: false,
            }],
            ..SheetFacts::default()
        };
        let prepared = prepare(rows, &facts, RowPolicy::Include);
        assert_eq!(prepared.len(), 1);
        assert_eq!(prepared[0].data.name, "Data");
        assert_eq!(prepared[0].data.rows.len(), 3);
        assert_eq!(
            prepared[0].data.rows[0],
            vec![text("lib"), text("cat"), text("montant")]
        );
        assert_eq!(prepared[0].layout.excel_table.as_deref(), Some("T1"));
        assert_eq!(prepared[0].layout.note_rows_left_out, 2);
        assert_eq!(prepared[0].layout.range.as_deref(), Some("B2:D4"));
    }

    #[test]
    fn a_placeholder_in_a_numeric_column_reads_as_empty_but_not_in_a_text_column() {
        let rows = vec![
            header(),
            vec![text("a"), text("n/a"), number(1.0)],
            vec![text("b"), text("x"), text("-")],
            vec![text("c"), text("y"), number(3.0)],
        ];
        let prepared = prepare(rows, &SheetFacts::default(), RowPolicy::Include);
        assert_eq!(prepared[0].data.rows[2][2], CellValue::Empty);
        assert_eq!(prepared[0].data.rows[1][1], text("n/a"));
        assert_eq!(prepared[0].layout.placeholders_blanked, 1);
    }
}
