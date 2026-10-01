//! `TabularInventory`: what is structurally inside a workbook, built by one full pass over its
//! cells and never inferred from a retrieved passage, a partial sample, or a model
//! (`docs/WORK-FOLDER-INVENTORY.md`'s rule, applied to the tabular pipeline).
//!
//! Deliberately a separate abstraction from `WorkFolderInventory`, not merged into it:
//! `WorkFolderInventory` stays filesystem facts only (existence, path, extension, identity,
//! size, changed/missing), and this module stays workbook facts only (sheets, header, columns,
//! types, formula presence). The two meet on one value, the content SHA-256, the same identity
//! `FileRecord::id` carries.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::{CellValue, SheetData, Workbook};

/// A header candidate must be within the first rows of the sheet, and have at least one row of
/// data beneath it.
const HEADER_SEARCH_ROWS: usize = 20;

/// A column is typed `Numeric` once at least this share of its non-empty cells reads as a
/// number; the rest are reported, never silently dropped from a sum (`docs/DECISIONS.md`, D2).
const NUMERIC_PARSE_THRESHOLD: f64 = 0.95;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnType {
    Numeric,
    Date,
    /// The default when a column's values do not settle on numeric or date: plain text, a mix of
    /// types, too few numeric cells, or no data at all.
    Categorical,
}

impl ColumnType {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Numeric => "numeric",
            Self::Date => "date",
            Self::Categorical => "categorical",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TabularFormat {
    Csv,
    Xlsx,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInventory {
    /// The header cell's text, when a header row was found. A synthetic `column_N` (1-indexed)
    /// otherwise, so every column stays addressable even without one.
    pub name: String,
    /// 0-indexed position in the row, stable whether or not a header was found.
    pub index: usize,
    pub inferred_type: ColumnType,
    /// Whether any data cell in this column carries a formula rather than a plain value.
    pub has_formulas: bool,
    /// A numeric column whose every value is a whole number between 1900 and 2100: a year, not
    /// an amount. Never totalled unless a question names it (`tabular::question`).
    #[serde(default)]
    pub year_like: bool,
    /// The currency or percent sign read from this column's cells, when every cell that carried
    /// one agreed (`\u{20ac}`, `$`, `%`). `None` for a column with no unit, or with more than one.
    /// Meaningful only when `inferred_type` is `Numeric`.
    #[serde(default)]
    pub unit: Option<String>,
    /// Non-empty cells in this column that could not be read as a number, counted the same pass
    /// that typed it - never folded into a sum, and always shown beside one
    /// (`docs/DECISIONS.md`, D2). Zero for any column that is not `Numeric`.
    #[serde(default)]
    pub unparsed_count: usize,
    /// Set when this column's values parse as a day-first date (`dd/mm/yyyy` and its `-`/`.`
    /// siblings) but no cell anywhere in it settles whether the file means day-first or
    /// month-first - every day is 12 or below, so the American reading would also be valid. The
    /// column is `Categorical`, never guessed as `Date` (`docs/DECISIONS.md`, D3).
    #[serde(default)]
    pub ambiguous_date: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetInventory {
    pub name: String,
    /// 0-indexed row this sheet's header was found on. `None` when no row looks like one - an
    /// empty sheet, or one whose data starts on the first row - and the sheet is still
    /// inventoried rather than refused.
    pub header_row: Option<usize>,
    /// Data rows only: below the header when one was found, every row otherwise.
    pub row_count: usize,
    pub column_count: usize,
    pub columns: Vec<ColumnInventory>,
    /// True when at least one cell anywhere on the sheet carries a formula.
    pub has_formulas: bool,
    /// How many data cells carry a formula, counted in the same pass as `has_formulas`. Read by
    /// `has_a_usable_sheet` only. Defaults to zero for an inventory cached before the field
    /// existed; the next Analyse pass rebuilds every workbook, so the default never outlives it.
    #[serde(default)]
    pub formula_cells: usize,
}

/// "Does this sheet look like a real data table", in numbers borrowed from LocalGridMind's
/// `src/core/stats.py` (`TABULAR_MIN_ROWS`, `TABULAR_MIN_COLUMNS`, `TABULAR_MAX_FORMULA_RATIO`),
/// tuned there against real spreadsheets. Numbers only: no code is shared
/// (`docs/DECISIONS.md`, "external reference: LocalGridMind").
const USABLE_MIN_ROWS: usize = 8;
const USABLE_MIN_COLUMNS: usize = 2;
/// Formula cells, as a share of the sheet's data rows.
const USABLE_MAX_FORMULA_RATIO: f64 = 0.05;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabularInventory {
    /// Content SHA-256: the same identity `FileRecord::id` and `Source.origin.sha256` carry, so
    /// a workbook needs no second identity model.
    pub workbook_id: String,
    pub relative_path: String,
    pub format: TabularFormat,
    pub sheets: Vec<SheetInventory>,
}

impl TabularInventory {
    /// One full pass over every sheet's cells. Never a sample: a column's type is decided from
    /// everything in it, so a numeric column with one stray label far down the sheet is
    /// reported honestly rather than missed by a partial read.
    ///
    /// `locale` is read only as the last resort for a lone separator whose shape alone cannot
    /// settle whether it groups thousands or marks the decimal (`docs/DECISIONS.md`, D2); most
    /// cells settle themselves from their own shape, or from another cell of the same column.
    pub fn build(
        relative_path: &str,
        workbook_id: &str,
        format: TabularFormat,
        workbook: &Workbook,
        locale: &str,
    ) -> Self {
        let sheets = workbook
            .sheets
            .iter()
            .map(|sheet| build_sheet(sheet, locale))
            .collect();
        Self {
            workbook_id: workbook_id.to_string(),
            relative_path: relative_path.to_string(),
            format,
            sheets,
        }
    }

    /// Every sheet carrying this name. A well-formed XLSX file cannot hold two sheets with the
    /// same name, but nothing here assumes the workbook it was handed is well-formed, so a
    /// collision is returned in full rather than one entry silently winning - the same
    /// ambiguity-is-a-result rule `WorkFolderInventory::find_by_id` follows for a duplicate file
    /// identity.
    pub fn sheets_named(&self, name: &str) -> Vec<&SheetInventory> {
        self.sheets
            .iter()
            .filter(|sheet| sheet.name == name)
            .collect()
    }

    /// Whether at least one sheet looks like a table a person could actually query - the
    /// difference between green and red on the Data Folder listing. "Parsed without an error" is
    /// not enough: the CSV adapter reads almost any text as a one-column sheet, so a note renamed
    /// `.csv` parses cleanly and still holds nothing to ask about.
    ///
    /// Deliberately per sheet, never a per-file formula budget: the engine already refuses
    /// column by column when a formula is involved, so one formula-laden sheet must not hide a
    /// clean one beside it (`docs/DECISIONS.md`, "the tabular UI session").
    pub fn has_a_usable_sheet(&self) -> bool {
        self.sheets.iter().any(SheetInventory::looks_tabular)
    }
}

impl SheetInventory {
    /// Enough rows and columns to be a grid rather than a note, and few enough formulas that its
    /// values are mostly stored rather than computed by the file.
    pub fn looks_tabular(&self) -> bool {
        self.row_count >= USABLE_MIN_ROWS
            && self.column_count >= USABLE_MIN_COLUMNS
            && (self.formula_cells as f64) <= USABLE_MAX_FORMULA_RATIO * self.row_count as f64
    }
}

fn build_sheet(sheet: &SheetData, locale: &str) -> SheetInventory {
    let header_row = detect_header_row(&sheet.rows);
    let data_rows: &[Vec<CellValue>] = match header_row {
        Some(index) => &sheet.rows[index + 1..],
        None => &sheet.rows[..],
    };
    let column_count = sheet.rows.iter().map(Vec::len).max().unwrap_or(0);

    let columns: Vec<ColumnInventory> = (0..column_count)
        .map(|index| build_column(sheet, header_row, data_rows, index, locale))
        .collect();
    let has_formulas = columns.iter().any(|column| column.has_formulas);
    let formula_cells = data_rows
        .iter()
        .flatten()
        .filter(|cell| matches!(cell, CellValue::Formula { .. }))
        .count();

    SheetInventory {
        name: sheet.name.clone(),
        header_row,
        row_count: data_rows.len(),
        column_count,
        columns,
        has_formulas,
        formula_cells,
    }
}

fn build_column(
    sheet: &SheetData,
    header_row: Option<usize>,
    data_rows: &[Vec<CellValue>],
    index: usize,
    locale: &str,
) -> ColumnInventory {
    let name = header_row
        .and_then(|row_index| sheet.rows.get(row_index))
        .and_then(|row| row.get(index))
        .and_then(cell_text)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| format!("column_{}", index + 1));

    let mut has_formulas = false;
    let mut numeric_like = 0usize;
    let mut date_like = 0usize;
    let mut date_settled = false;
    let mut other = 0usize;

    for row in data_rows {
        let Some(cell) = row.get(index) else {
            continue;
        };
        if matches!(cell, CellValue::Formula { .. }) {
            has_formulas = true;
        }
        match classify_cell(cell) {
            CellClass::Empty => {}
            CellClass::Numeric => numeric_like += 1,
            CellClass::Date { settled } => {
                date_like += 1;
                date_settled |= settled;
            }
            CellClass::Other => other += 1,
        }
    }

    let total_non_empty = numeric_like + date_like + other;
    let numeric = resolve_numeric_column(data_rows, index, locale);
    let numeric_parsed = numeric.values.iter().filter(|value| value.is_some()).count();

    let is_numeric = numeric_like > 0
        && total_non_empty > 0
        && (numeric_parsed as f64 / total_non_empty as f64) >= NUMERIC_PARSE_THRESHOLD;
    let is_date = !is_numeric && numeric_like == 0 && other == 0 && date_like > 0 && date_settled;
    let ambiguous_date = !is_numeric && !is_date && numeric_like == 0 && other == 0 && date_like > 0;

    let inferred_type = if is_numeric {
        ColumnType::Numeric
    } else if is_date {
        ColumnType::Date
    } else {
        ColumnType::Categorical
    };

    let year_like = inferred_type == ColumnType::Numeric
        && numeric_parsed > 0
        && numeric
            .values
            .iter()
            .flatten()
            .all(|value| value.fract() == 0.0 && (1900.0..=2100.0).contains(value));

    ColumnInventory {
        name,
        index,
        inferred_type,
        has_formulas,
        year_like,
        unit: if inferred_type == ColumnType::Numeric {
            numeric.unit
        } else {
            None
        },
        unparsed_count: if inferred_type == ColumnType::Numeric {
            total_non_empty - numeric_parsed
        } else {
            0
        },
        ambiguous_date,
    }
}

/// A header candidate is scored, not merely "the first row of plain text" (`docs/DECISIONS.md`,
/// D4): how much of the widest row it fills, how many of its own labels are distinct, and how
/// much of the row beneath it reads as typed data rather than more text - a title row sitting
/// above the real header scores low on every count and a genuine header wins even when it is not
/// the first textual row. A row with a single filled cell is never a header outright, and a
/// candidate still needs at least one row of data beneath it to be a header *of*.
fn detect_header_row(rows: &[Vec<CellValue>]) -> Option<usize> {
    if rows.len() < 2 {
        return None;
    }
    let widest = rows.iter().map(Vec::len).max().unwrap_or(0);
    if widest == 0 {
        return None;
    }

    let mut best: Option<(usize, f64)> = None;
    for (index, row) in rows.iter().enumerate().take(HEADER_SEARCH_ROWS) {
        if index + 1 >= rows.len() {
            break;
        }
        let classes: Vec<CellClass> = row.iter().map(classify_cell).collect();
        let non_empty = classes
            .iter()
            .filter(|class| !matches!(class, CellClass::Empty))
            .count();
        if non_empty < 2 {
            continue;
        }
        let all_textual = classes
            .iter()
            .all(|class| matches!(class, CellClass::Other | CellClass::Empty));
        if !all_textual {
            continue;
        }

        let fill_ratio = non_empty as f64 / widest as f64;

        let labels: Vec<String> = row
            .iter()
            .filter_map(cell_text)
            .map(|text| text.trim().to_lowercase())
            .filter(|text| !text.is_empty())
            .collect();
        let distinct = labels
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        let distinctness = distinct as f64 / non_empty as f64;

        let below = &rows[index + 1];
        let typed_below = row
            .iter()
            .zip(below.iter())
            .filter(|(_, below_cell)| {
                !matches!(
                    classify_cell(below_cell),
                    CellClass::Empty | CellClass::Other
                )
            })
            .count();
        let typed_ratio = typed_below as f64 / non_empty as f64;

        let score = fill_ratio * 0.4 + distinctness * 0.3 + typed_ratio * 0.3;
        if best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((index, score));
        }
    }
    best.map(|(index, _)| index)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellClass {
    Empty,
    Numeric,
    /// `settled`: whether some evidence in this cell alone rules out the other day/month reading
    /// (a day above 12), or the format itself carries no such ambiguity (ISO `yyyy-mm-dd`, or a
    /// real calendar cell from the file format). `false` for a `dd/mm/yyyy`-shaped cell where
    /// every component is 12 or below.
    Date { settled: bool },
    /// Text that is not itself a recognisable number or date - a label, a name, free text.
    Other,
}

fn classify_cell(cell: &CellValue) -> CellClass {
    match cell {
        CellValue::Empty => CellClass::Empty,
        CellValue::Number(_) => CellClass::Numeric,
        CellValue::Date(_) => CellClass::Date { settled: true },
        CellValue::Text(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return CellClass::Empty;
            }
            if !matches!(number_shape(trimmed), NumberShape::Invalid) {
                return CellClass::Numeric;
            }
            match parse_date_text(trimmed) {
                Some(settled) => CellClass::Date { settled },
                None => CellClass::Other,
            }
        }
        // A formula is never evaluated; what it is classified as comes only from the value the
        // file itself cached for it, exactly the value `derivation` marks as unverified.
        CellValue::Formula { cached_value, .. } => match cached_value {
            Some(inner) => classify_cell(inner),
            None => CellClass::Other,
        },
    }
}

// --- Dates (D3) -------------------------------------------------------------------------------

/// `yyyy-mm-dd`, or a day-first `dd/mm/yyyy`, `dd-mm-yyyy`, `dd.mm.yyyy` - checked against a real
/// calendar (`NaiveDate::from_ymd_opt`), not merely plausible ranges. `Some(settled)`: `true` for
/// ISO (year-first, no day/month ambiguity possible) or a day-first value whose day exceeds 12
/// (the only evidence that rules out the month-first reading); `false` for a day-first value
/// where every component is 12 or below, genuinely ambiguous on its own.
fn parse_date_text(text: &str) -> Option<bool> {
    if parse_iso_date(text).is_some() {
        return Some(true);
    }
    for separator in ['/', '-', '.'] {
        if let Some((day, month, year)) = split_ddmmyyyy(text, separator) {
            if day == 0 || day > 31 || month == 0 || month > 12 {
                continue;
            }
            if NaiveDate::from_ymd_opt(year as i32, month, day).is_none() {
                continue;
            }
            return Some(day > 12);
        }
    }
    None
}

fn parse_iso_date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

/// The same `dd/mm/yyyy` family `parse_date_text` recognises, checked against a real calendar,
/// but returning the parsed value itself rather than only whether it settles day-first - what
/// `resolve_date_column` needs to compare a cell against a weekday, a month, a year or a range
/// (`docs/DECISIONS.md`, session 11).
pub(crate) fn parse_date_value(text: &str) -> Option<NaiveDate> {
    if let Some(date) = parse_iso_date(text) {
        return Some(date);
    }
    for separator in ['/', '-', '.'] {
        if let Some((day, month, year)) = split_ddmmyyyy(text, separator) {
            if day == 0 || day > 31 || month == 0 || month > 12 {
                continue;
            }
            if let Some(date) = NaiveDate::from_ymd_opt(year as i32, month, day) {
                return Some(date);
            }
        }
    }
    None
}

fn cell_date_value(cell: &CellValue) -> Option<NaiveDate> {
    match cell {
        CellValue::Date(date) => Some(*date),
        CellValue::Text(text) => parse_date_value(text.trim()),
        CellValue::Formula { cached_value, .. } => {
            cached_value.as_deref().and_then(cell_date_value)
        }
        CellValue::Empty | CellValue::Number(_) => None,
    }
}

/// One column's cells, resolved to calendar dates - the sibling `resolve_numeric_column` did not
/// yet have (`docs/SESSION-DATA-06-Findings.md` section 5): reads `CellValue::Date` directly and
/// parses `CellValue::Text` with the same formats `parse_date_text` already recognises, never a
/// text comparison. `None` for an empty or missing cell, or one that does not parse as a date at
/// all - a filter over a column this session did not type `Date` refuses before ever calling this
/// (`tabular::engine::resolve_filter_column`), so every `None` here is an ordinary missing value,
/// not an ambiguity to guess at.
pub(crate) fn resolve_date_column(
    rows: &[Vec<CellValue>],
    column_index: usize,
) -> Vec<Option<NaiveDate>> {
    rows.iter()
        .map(|row| row.get(column_index).and_then(cell_date_value))
        .collect()
}

fn split_ddmmyyyy(text: &str, separator: char) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = text.split(separator).collect();
    let [day_text, month_text, year_text] = parts.as_slice() else {
        return None;
    };
    if year_text.len() != 4 {
        return None;
    }
    let day: u32 = day_text.parse().ok()?;
    let month: u32 = month_text.parse().ok()?;
    let year: u32 = year_text.parse().ok()?;
    Some((day, month, year))
}

// --- Numbers (D2) -------------------------------------------------------------------------------

/// Currency and percent signs this product recognises in a cell's text, before or after the
/// number, with or without a space.
const UNIT_SYMBOLS: [&str; 3] = ["\u{20ac}", "$", "%"];

/// Every character a thousands grouping may use - plain space, the two Unicode spaces real
/// exports use, and, when it is not already playing the decimal role, a point or a comma.
const GROUPING_CHARS: [char; 5] = [' ', '\u{a0}', '\u{202f}', '.', ','];

fn is_space_like(separator: char) -> bool {
    matches!(separator, ' ' | '\u{a0}' | '\u{202f}')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeparatorRole {
    Decimal,
    Thousands,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NumberConvention {
    decimal: char,
}

impl NumberConvention {
    /// The last resort, read only when a column's own cells give no evidence either way
    /// (`docs/DECISIONS.md`, D2): decimal comma for a French interface, decimal point otherwise.
    fn locale_default(locale: &str) -> Self {
        let decimal = if locale.to_ascii_lowercase().starts_with("fr") {
            ','
        } else {
            '.'
        };
        Self { decimal }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum NumberShape {
    /// A value with nothing left to decide: no separator at all, two different separators (the
    /// grouping and the decimal mark are then both self-evident), or one separator whose
    /// grouping shape alone settles which role it plays.
    Resolved {
        value: f64,
        unit: Option<String>,
        /// Every separator character this cell's own shape settled, and the role it proved -
        /// corroboration for another cell of the same column that used the same character but
        /// could not tell on its own (`docs/DECISIONS.md`, "French and English numbers in one
        /// column").
        evidence: Vec<(char, SeparatorRole)>,
    },
    /// One separator, used exactly once, with exactly three digits on either side of it
    /// (`1,234`): genuinely ambiguous on its own - `1234` if the column reads it as a thousands
    /// grouping, `1.234` if as a decimal comma. Column evidence, or the interface locale when
    /// there is none, decides (`resolve_numeric_column`).
    Ambiguous {
        separator: char,
        whole: String,
        fraction: String,
        negative: bool,
        unit: Option<String>,
    },
    /// Not shaped like a number at all.
    Invalid,
}

/// One cell's text, classified without knowing its column's convention yet. Never guesses a
/// thousands grouping from a single value: the only case left undecided here (`Ambiguous`) is
/// exactly the one no single cell can settle.
fn number_shape(text: &str) -> NumberShape {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return NumberShape::Invalid;
    }

    let mut body = trimmed;
    let mut negative = false;
    if let Some(inner) = body.strip_prefix('(').and_then(|rest| rest.strip_suffix(')')) {
        negative = true;
        body = inner.trim();
    } else if let Some(inner) = body.strip_prefix('-') {
        negative = true;
        body = inner.trim_start();
    }

    let (body, unit) = strip_unit(body);
    if body.is_empty() {
        return NumberShape::Invalid;
    }

    if !body
        .chars()
        .all(|ch| ch.is_ascii_digit() || GROUPING_CHARS.contains(&ch))
    {
        return NumberShape::Invalid;
    }

    let mut parts: Vec<String> = Vec::new();
    let mut seps: Vec<char> = Vec::new();
    let mut current = String::new();
    for ch in body.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else {
            if current.is_empty() {
                return NumberShape::Invalid;
            }
            parts.push(std::mem::take(&mut current));
            seps.push(ch);
        }
    }
    if current.is_empty() {
        return NumberShape::Invalid;
    }
    parts.push(current);

    let sign = if negative { -1.0 } else { 1.0 };

    if seps.is_empty() {
        return match parts[0].parse::<f64>() {
            Ok(value) => NumberShape::Resolved {
                value: sign * value,
                unit,
                evidence: Vec::new(),
            },
            Err(_) => NumberShape::Invalid,
        };
    }

    let distinct: std::collections::BTreeSet<char> = seps.iter().copied().collect();
    if distinct.len() > 2 {
        return NumberShape::Invalid;
    }

    if distinct.len() == 2 {
        let decimal_sep = *seps.last().unwrap();
        let thousands_sep = seps[0];
        if is_space_like(decimal_sep) {
            return NumberShape::Invalid;
        }
        if seps[..seps.len() - 1].iter().any(|&sep| sep != thousands_sep) {
            return NumberShape::Invalid;
        }
        let thousands_groups = &parts[..parts.len() - 1];
        if thousands_groups[0].is_empty() || thousands_groups[0].len() > 3 {
            return NumberShape::Invalid;
        }
        if thousands_groups[1..].iter().any(|group| group.len() != 3) {
            return NumberShape::Invalid;
        }
        let fraction = parts.last().unwrap();
        let whole: String = thousands_groups.concat();
        return match format!("{whole}.{fraction}").parse::<f64>() {
            Ok(value) => NumberShape::Resolved {
                value: sign * value,
                unit,
                evidence: vec![
                    (thousands_sep, SeparatorRole::Thousands),
                    (decimal_sep, SeparatorRole::Decimal),
                ],
            },
            Err(_) => NumberShape::Invalid,
        };
    }

    // distinct.len() == 1: one separator character, used once or more.
    let separator = seps[0];
    if is_space_like(separator) || seps.len() >= 2 {
        return resolve_thousands_only(&parts, separator, sign, unit);
    }

    // Exactly one occurrence of `,` or `.`.
    let whole = parts[0].clone();
    let fraction = parts[1].clone();
    if fraction.len() != 3 || whole.is_empty() || whole.len() > 3 {
        // The grouping shape itself rules out a thousands reading: unambiguous decimal.
        return match format!("{whole}.{fraction}").parse::<f64>() {
            Ok(value) => NumberShape::Resolved {
                value: sign * value,
                unit,
                evidence: vec![(separator, SeparatorRole::Decimal)],
            },
            Err(_) => NumberShape::Invalid,
        };
    }

    NumberShape::Ambiguous {
        separator,
        whole,
        fraction,
        negative,
        unit,
    }
}

/// A thousands-only reading with no decimal part at all (`1,234,567`, `12 345`): the first group
/// is one to three digits, every group after it exactly three. Any other shape under a repeated,
/// or a space-like, separator is not a number this product reads.
fn resolve_thousands_only(
    parts: &[String],
    separator: char,
    sign: f64,
    unit: Option<String>,
) -> NumberShape {
    if parts[0].is_empty() || parts[0].len() > 3 {
        return NumberShape::Invalid;
    }
    if parts[1..].iter().any(|group| group.len() != 3) {
        return NumberShape::Invalid;
    }
    let whole: String = parts.concat();
    match whole.parse::<f64>() {
        Ok(value) => NumberShape::Resolved {
            value: sign * value,
            unit,
            evidence: vec![(separator, SeparatorRole::Thousands)],
        },
        Err(_) => NumberShape::Invalid,
    }
}

/// A currency or percent sign at either end, with or without a space (`docs/DECISIONS.md`, D2).
fn strip_unit(text: &str) -> (&str, Option<String>) {
    let trimmed = trim_layout_space(text);
    for symbol in UNIT_SYMBOLS {
        if let Some(rest) = trimmed.strip_prefix(symbol) {
            return (trim_layout_space(rest), Some(symbol.to_string()));
        }
        if let Some(rest) = trimmed.strip_suffix(symbol) {
            return (trim_layout_space(rest), Some(symbol.to_string()));
        }
    }
    (trimmed, None)
}

fn trim_layout_space(text: &str) -> &str {
    text.trim_matches(|ch: char| ch == ' ' || ch == '\u{a0}' || ch == '\u{202f}')
}

/// One column's cells, resolved to numbers under one locale - the same function the inventory
/// uses to type a column and the engine uses to compute over it, so the two can never disagree
/// (`docs/DECISIONS.md`, "French and English numbers in one column"). Reads a formula cell's
/// cached value exactly as `classify_cell` does, for the same reason: what a formula is typed as
/// must match what it would compute as, if an unrelated column happened to answer.
pub(crate) struct NumericColumn {
    /// One entry per row of `rows`, in the same order. `None` for an empty or missing cell, one
    /// that is not shaped like a number at all, or a lone separator whose role this column could
    /// not settle (`docs/DECISIONS.md`: mixed evidence is reported, never guessed).
    pub values: Vec<Option<f64>>,
    /// Set only when every cell that carried a unit agreed on the same one.
    pub unit: Option<String>,
}

pub(crate) fn resolve_numeric_column(
    rows: &[Vec<CellValue>],
    column_index: usize,
    locale: &str,
) -> NumericColumn {
    let shapes: Vec<Option<NumberShape>> = rows
        .iter()
        .map(|row| row.get(column_index).and_then(cell_number_shape))
        .collect();

    let mut saw_decimal: std::collections::HashSet<char> = std::collections::HashSet::new();
    let mut saw_thousands: std::collections::HashSet<char> = std::collections::HashSet::new();
    let mut ambiguous_count: std::collections::HashMap<char, usize> = std::collections::HashMap::new();
    for shape in shapes.iter().flatten() {
        match shape {
            NumberShape::Resolved { evidence, .. } => {
                for (separator, role) in evidence {
                    match role {
                        SeparatorRole::Decimal => saw_decimal.insert(*separator),
                        SeparatorRole::Thousands => saw_thousands.insert(*separator),
                    };
                }
            }
            NumberShape::Ambiguous { separator, .. } => {
                *ambiguous_count.entry(*separator).or_insert(0) += 1;
            }
            NumberShape::Invalid => {}
        }
    }
    let default = NumberConvention::locale_default(locale);
    let role_for = |separator: char| -> Option<SeparatorRole> {
        match (saw_decimal.contains(&separator), saw_thousands.contains(&separator)) {
            (true, false) => Some(SeparatorRole::Decimal),
            (false, true) => Some(SeparatorRole::Thousands),
            // Genuinely mixed evidence, from two different cells of this same column: reported,
            // never guessed.
            (true, true) => None,
            (false, false) => {
                // A separator's own exactly-three-digit shape is not corroboration on its own
                // (that shape is the definition of `Ambiguous`) - but every cell of the column
                // that carries it showing the same shape is: a real thousands grouping rarely
                // produces the same two-or-more-digit remainder by coincidence on every row
                // (`docs/DECISIONS.md`, "a comma followed by exactly three digits in every cell
                // that has one is corroborated as a thousands grouping"). A single such cell,
                // with nothing else in the column to corroborate it, has no evidence at all and
                // falls back to the interface locale.
                if ambiguous_count.get(&separator).copied().unwrap_or(0) >= 2 {
                    Some(SeparatorRole::Thousands)
                } else if separator == default.decimal {
                    Some(SeparatorRole::Decimal)
                } else {
                    Some(SeparatorRole::Thousands)
                }
            }
        }
    };

    let mut units: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut unit_conflict = false;
    let mut values = Vec::with_capacity(shapes.len());
    for shape in shapes {
        let value = match shape {
            None | Some(NumberShape::Invalid) => None,
            Some(NumberShape::Resolved { value, unit, .. }) => {
                if let Some(symbol) = unit {
                    units.insert(symbol);
                    unit_conflict |= units.len() > 1;
                }
                Some(value)
            }
            Some(NumberShape::Ambiguous {
                separator,
                whole,
                fraction,
                negative,
                unit,
            }) => {
                if let Some(symbol) = unit {
                    units.insert(symbol);
                    unit_conflict |= units.len() > 1;
                }
                let sign = if negative { -1.0 } else { 1.0 };
                match role_for(separator) {
                    Some(SeparatorRole::Decimal) => format!("{whole}.{fraction}")
                        .parse::<f64>()
                        .ok()
                        .map(|value| sign * value),
                    Some(SeparatorRole::Thousands) => format!("{whole}{fraction}")
                        .parse::<f64>()
                        .ok()
                        .map(|value| sign * value),
                    None => None,
                }
            }
        };
        values.push(value);
    }

    NumericColumn {
        values,
        unit: if unit_conflict {
            None
        } else {
            units.into_iter().next()
        },
    }
}

fn cell_number_shape(cell: &CellValue) -> Option<NumberShape> {
    match cell {
        CellValue::Number(number) => Some(NumberShape::Resolved {
            value: *number,
            unit: None,
            evidence: Vec::new(),
        }),
        CellValue::Text(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                match number_shape(trimmed) {
                    NumberShape::Invalid => None,
                    shape => Some(shape),
                }
            }
        }
        CellValue::Formula { cached_value, .. } => {
            cached_value.as_deref().and_then(cell_number_shape)
        }
        CellValue::Empty | CellValue::Date(_) => None,
    }
}

fn cell_text(cell: &CellValue) -> Option<String> {
    match cell {
        CellValue::Empty => None,
        CellValue::Text(text) => Some(text.clone()),
        CellValue::Number(number) => Some(number.to_string()),
        // ISO 8601: the recommended format for a date column this product itself produces
        // (`docs/DECISIONS.md`, D3), and what reads unambiguously regardless of locale - the
        // sentence layer localises it further only where a user actually reads a bare date.
        CellValue::Date(date) => Some(date.format("%Y-%m-%d").to_string()),
        CellValue::Formula { expression, .. } => Some(expression.clone()),
    }
}

/// The data rows one sheet's inventory was built from: below the header when one was found,
/// every row otherwise - the exact slicing `build_sheet` used, so the engine that computes over
/// these rows and the inventory that counted them can never disagree about which rows they are.
pub(crate) fn data_rows<'a>(
    sheet_inventory: &SheetInventory,
    data: &'a SheetData,
) -> &'a [Vec<CellValue>] {
    match sheet_inventory.header_row {
        Some(index) if index < data.rows.len() => &data.rows[index + 1..],
        _ => &data.rows[..],
    }
}

/// A cell's value as text, for display in a row listing (`tabular::engine::row_cells`) - what a
/// person would actually see in the cell, not the file's internal representation of it. A
/// formula cell therefore shows its last cached value when one was read, falling back to the
/// expression only when no cached value exists; `cell_text` alone would show the expression
/// always, which reads as if the sheet stored formula source text as data. This is a display
/// choice only - it never marks the value verified, and every aggregate that would have to read
/// a formula column's values refuses before reaching this function at all
/// (`docs/DECISIONS.md`, "a formula's cached result").
pub(crate) fn text_value(cell: &CellValue) -> String {
    match cell {
        CellValue::Formula {
            cached_value: Some(inner),
            ..
        } => text_value(inner),
        other => cell_text(other).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LOCALE: &str = "fr-FR";

    fn text_row(values: &[&str]) -> Vec<CellValue> {
        values
            .iter()
            .map(|value| {
                if value.is_empty() {
                    CellValue::Empty
                } else {
                    CellValue::Text(value.to_string())
                }
            })
            .collect()
    }

    fn sheet(name: &str, rows: Vec<Vec<CellValue>>) -> SheetData {
        SheetData {
            name: name.to_string(),
            rows,
        }
    }

    fn build_one(rows: Vec<Vec<CellValue>>) -> SheetInventory {
        let workbook = Workbook {
            sheets: vec![sheet("Feuille1", rows)],
        };
        let inventory = TabularInventory::build(
            "data.csv",
            "hash-1",
            TabularFormat::Csv,
            &workbook,
            TEST_LOCALE,
        );
        inventory.sheets.into_iter().next().unwrap()
    }

    #[test]
    fn a_textual_first_row_is_recognised_as_the_header() {
        let sheet = build_one(vec![
            text_row(&["nom", "age"]),
            text_row(&["Camille", "42"]),
            text_row(&["Esaie", "7"]),
        ]);

        assert_eq!(sheet.header_row, Some(0));
        assert_eq!(sheet.row_count, 2);
        assert_eq!(sheet.columns[0].name, "nom");
        assert_eq!(sheet.columns[1].name, "age");
        assert_eq!(sheet.columns[1].inferred_type, ColumnType::Numeric);
    }

    #[test]
    fn a_sheet_with_no_recognisable_header_still_inventories_its_columns() {
        let sheet = build_one(vec![text_row(&["1", "2"]), text_row(&["3", "4"])]);

        assert_eq!(sheet.header_row, None);
        assert_eq!(sheet.row_count, 2);
        assert_eq!(sheet.columns[0].name, "column_1");
        assert_eq!(sheet.columns[1].name, "column_2");
        assert!(sheet
            .columns
            .iter()
            .all(|column| column.inferred_type == ColumnType::Numeric));
    }

    #[test]
    fn an_empty_sheet_is_inventoried_without_panicking() {
        let sheet = build_one(vec![]);

        assert_eq!(sheet.header_row, None);
        assert_eq!(sheet.row_count, 0);
        assert_eq!(sheet.column_count, 0);
        assert!(sheet.columns.is_empty());
    }

    #[test]
    fn a_single_row_sheet_has_no_header_because_it_has_no_data_beneath_one() {
        let sheet = build_one(vec![text_row(&["nom", "age"])]);

        assert_eq!(sheet.header_row, None);
        assert_eq!(sheet.row_count, 1);
    }

    #[test]
    fn a_date_column_is_recognised_from_dd_mm_yyyy_text_when_a_day_settles_it() {
        let sheet = build_one(vec![
            text_row(&["nom", "consultation"]),
            text_row(&["Camille", "25/03/2026"]),
            text_row(&["Esaie", "05/01/2026"]),
        ]);

        assert_eq!(sheet.columns[1].inferred_type, ColumnType::Date);
        assert!(!sheet.columns[1].ambiguous_date);
    }

    #[test]
    fn a_day_first_column_with_every_day_at_or_below_twelve_is_ambiguous_not_guessed() {
        let sheet = build_one(vec![
            text_row(&["nom", "consultation"]),
            text_row(&["Camille", "12/03/2026"]),
            text_row(&["Esaie", "05/01/2026"]),
        ]);

        assert_eq!(sheet.columns[1].inferred_type, ColumnType::Categorical);
        assert!(sheet.columns[1].ambiguous_date);
    }

    #[test]
    fn an_iso_date_column_is_never_ambiguous() {
        let sheet = build_one(vec![
            text_row(&["nom", "consultation"]),
            text_row(&["Camille", "2026-03-05"]),
            text_row(&["Esaie", "2026-01-09"]),
        ]);

        assert_eq!(sheet.columns[1].inferred_type, ColumnType::Date);
        assert!(!sheet.columns[1].ambiguous_date);
    }

    #[test]
    fn a_decimal_comma_number_is_recognised_as_numeric_not_categorical() {
        let sheet = build_one(vec![
            text_row(&["nom", "montant"]),
            text_row(&["Camille", "120,50"]),
            text_row(&["Esaie", "75,00"]),
        ]);

        assert_eq!(sheet.columns[1].inferred_type, ColumnType::Numeric);
        assert_eq!(sheet.columns[1].unparsed_count, 0);
    }

    #[test]
    fn a_mixed_column_falls_back_to_categorical() {
        let sheet = build_one(vec![
            text_row(&["nom", "valeur"]),
            text_row(&["Camille", "120,50"]),
            text_row(&["Esaie", "en attente"]),
        ]);

        assert_eq!(sheet.columns[1].inferred_type, ColumnType::Categorical);
    }

    #[test]
    fn a_column_mostly_numeric_stays_numeric_and_reports_the_few_that_did_not_parse() {
        let mut rows = vec![text_row(&["nom", "montant"])];
        for index in 0..97 {
            rows.push(text_row(&[&format!("Personne {index}"), "10,00"]));
        }
        rows.push(text_row(&["Autre", "en attente"]));
        rows.push(text_row(&["Autre", "voir plus tard"]));
        rows.push(text_row(&["Autre", "a confirmer"]));

        let sheet = build_one(rows);

        assert_eq!(sheet.columns[1].inferred_type, ColumnType::Numeric);
        assert_eq!(sheet.columns[1].unparsed_count, 3);
    }

    #[test]
    fn an_all_empty_column_is_reported_categorical_rather_than_a_guessed_type() {
        let sheet = build_one(vec![
            text_row(&["nom", "note"]),
            text_row(&["Camille", ""]),
            text_row(&["Esaie", ""]),
        ]);

        assert_eq!(sheet.columns[1].inferred_type, ColumnType::Categorical);
    }

    #[test]
    fn a_formula_cell_marks_its_column_and_sheet_without_being_evaluated() {
        let rows = vec![
            text_row(&["a", "b", "total"]),
            vec![
                CellValue::Number(1.0),
                CellValue::Number(2.0),
                CellValue::Formula {
                    expression: "A2+B2".into(),
                    cached_value: Some(Box::new(CellValue::Number(3.0))),
                },
            ],
        ];
        let sheet = build_one(rows);

        assert!(sheet.has_formulas);
        assert!(sheet.columns[2].has_formulas);
        assert_eq!(sheet.columns[2].inferred_type, ColumnType::Numeric);
    }

    #[test]
    fn a_workbook_with_two_sheets_sharing_a_name_reports_both_rather_than_choosing() {
        let workbook = Workbook {
            sheets: vec![
                sheet("Feuille1", vec![text_row(&["a"]), text_row(&["1"])]),
                sheet("Feuille1", vec![text_row(&["b"]), text_row(&["2"])]),
            ],
        };

        let inventory = TabularInventory::build(
            "dup.xlsx",
            "hash-dup",
            TabularFormat::Xlsx,
            &workbook,
            TEST_LOCALE,
        );

        assert_eq!(inventory.sheets.len(), 2);
        assert_eq!(inventory.sheets_named("Feuille1").len(), 2);
    }

    fn grid(rows: usize, formula_rows: usize) -> Vec<Vec<CellValue>> {
        let mut grid = vec![text_row(&["fournisseur", "montant", "total"])];
        for index in 0..rows {
            let total = if index < formula_rows {
                CellValue::Formula {
                    expression: format!("B{}*2", index + 2),
                    cached_value: Some(Box::new(CellValue::Number(2.0))),
                }
            } else {
                CellValue::Number(2.0)
            };
            grid.push(vec![
                CellValue::Text(format!("Fournisseur {index}")),
                CellValue::Number(1.0),
                total,
            ]);
        }
        grid
    }

    fn workbook_of(sheets: Vec<SheetData>) -> TabularInventory {
        TabularInventory::build(
            "classeur.xlsx",
            "hash-w",
            TabularFormat::Xlsx,
            &Workbook { sheets },
            TEST_LOCALE,
        )
    }

    #[test]
    fn a_column_of_whole_years_is_marked_year_like_and_an_amount_is_not() {
        let sheet = build_one(vec![
            text_row(&["calendar_year", "amount", "code"]),
            text_row(&["2019", "2019", "12"]),
            text_row(&["2020", "1500,50", "7"]),
        ]);

        assert!(sheet.columns[0].year_like);
        assert!(!sheet.columns[1].year_like, "one amount is not a whole year");
        assert!(!sheet.columns[2].year_like);
    }

    #[test]
    fn formula_cells_are_counted_in_the_same_pass() {
        let inventory = workbook_of(vec![sheet("Calculs", grid(10, 4))]);

        assert_eq!(inventory.sheets[0].formula_cells, 4);
    }

    #[test]
    fn a_sheet_that_is_mostly_formulas_is_not_a_usable_table_on_its_own() {
        let inventory = workbook_of(vec![sheet("Calculs", grid(20, 20))]);

        assert!(!inventory.has_a_usable_sheet());
    }

    #[test]
    fn one_clean_sheet_keeps_a_workbook_usable_beside_a_formula_heavy_one() {
        // Not LocalGridMind's whole-file formula budget: the engine refuses per column, so the
        // clean sheet stays queryable however many formulas the other one carries.
        let inventory = workbook_of(vec![
            sheet("Calculs", grid(40, 40)),
            sheet("Facturation", grid(12, 0)),
        ]);

        assert!(!inventory.sheets[0].looks_tabular());
        assert!(inventory.sheets[1].looks_tabular());
        assert!(inventory.has_a_usable_sheet());
    }

    #[test]
    fn a_few_formulas_under_the_ratio_do_not_turn_a_sheet_red() {
        // One formula in twenty-four rows is under 5%.
        let inventory = workbook_of(vec![sheet("Facturation", grid(24, 1))]);

        assert!(inventory.has_a_usable_sheet());
    }

    #[test]
    fn an_inventory_cached_before_formula_cells_existed_still_reads() {
        let mut json = serde_json::to_value(workbook_of(vec![sheet("F", grid(8, 0))])).unwrap();
        json["sheets"][0]
            .as_object_mut()
            .unwrap()
            .remove("formulaCells");

        let back: TabularInventory = serde_json::from_value(json).unwrap();

        assert_eq!(back.sheets[0].formula_cells, 0);
    }

    #[test]
    fn an_inventory_cached_before_the_locale_parsing_fields_existed_still_reads() {
        let mut json = serde_json::to_value(build_one(vec![
            text_row(&["nom", "montant"]),
            text_row(&["Camille", "120,50"]),
        ]))
        .unwrap();
        let column = json["columns"][1].as_object_mut().unwrap();
        column.remove("unit");
        column.remove("unparsedCount");
        column.remove("ambiguousDate");

        let back: SheetInventory = serde_json::from_value(json).unwrap();

        assert_eq!(back.columns[1].unit, None);
        assert_eq!(back.columns[1].unparsed_count, 0);
        assert!(!back.columns[1].ambiguous_date);
    }

    #[test]
    fn the_same_workbook_bytes_produce_the_same_inventory_twice() {
        let rows = vec![
            text_row(&["nom", "montant"]),
            text_row(&["Camille", "120,50"]),
        ];
        let first = build_one(rows.clone());
        let second = build_one(rows);

        assert_eq!(first, second);
    }

    // --- Header row scoring (D4) -----------------------------------------------------------

    #[test]
    fn a_title_row_then_a_blank_row_then_the_header_is_found() {
        let sheet = build_one(vec![
            text_row(&["Export du 12/03/2026"]),
            text_row(&["", ""]),
            text_row(&["fournisseur", "montant"]),
            text_row(&["Alpha", "10,00"]),
            text_row(&["Beta", "20,00"]),
        ]);

        assert_eq!(sheet.header_row, Some(2));
        assert_eq!(sheet.columns[0].name, "fournisseur");
        assert_eq!(sheet.columns[1].name, "montant");
    }

    #[test]
    fn a_single_filled_cell_is_never_taken_as_the_header() {
        let sheet = build_one(vec![
            text_row(&["Rapport mensuel"]),
            text_row(&["fournisseur", "montant"]),
            text_row(&["Alpha", "10,00"]),
        ]);

        assert_ne!(sheet.header_row, Some(0));
    }

    // --- Numbers, adversarial (D2, docs/SESSION-DATA-10-Locale-Parsing.md section 6) --------

    fn resolved(text: &str) -> Option<f64> {
        match number_shape(text) {
            NumberShape::Resolved { value, .. } => Some(value),
            _ => None,
        }
    }

    #[test]
    fn narrow_no_break_thousands_with_a_currency_sign_parses() {
        assert_eq!(resolved("1\u{202f}234,56 \u{20ac}"), Some(1234.56));
    }

    #[test]
    fn a_point_thousands_and_comma_decimal_number_parses() {
        assert_eq!(resolved("1.234,56"), Some(1234.56));
    }

    #[test]
    fn a_comma_thousands_and_point_decimal_number_parses() {
        assert_eq!(resolved("1,234.56"), Some(1234.56));
    }

    #[test]
    fn a_parenthesised_amount_is_negative() {
        assert_eq!(resolved("(120,00)"), Some(-120.0));
    }

    #[test]
    fn a_percent_sign_parses_with_its_unit() {
        match number_shape("15 %") {
            NumberShape::Resolved { value, unit, .. } => {
                assert_eq!(value, 15.0);
                assert_eq!(unit.as_deref(), Some("%"));
            }
            other => panic!("expected a resolved percent, got {other:?}"),
        }
    }

    #[test]
    fn a_space_thousands_grouping_of_the_wrong_width_does_not_parse() {
        assert!(matches!(number_shape("12 34"), NumberShape::Invalid));
    }

    #[test]
    fn a_comma_grouping_of_the_wrong_width_does_not_parse() {
        assert!(matches!(number_shape("1,23,4"), NumberShape::Invalid));
    }

    #[test]
    fn a_double_point_grouping_of_the_wrong_width_does_not_parse() {
        assert!(matches!(number_shape("1.2.3"), NumberShape::Invalid));
    }

    #[test]
    fn a_lone_separator_with_an_exact_three_digit_group_is_ambiguous_on_its_own() {
        assert!(matches!(
            number_shape("1,234"),
            NumberShape::Ambiguous { .. }
        ));
    }

    #[test]
    fn column_evidence_resolves_an_otherwise_ambiguous_comma_as_thousands() {
        let rows = vec![text_row(&["1,234"]), text_row(&["2,345,678"])];
        let numeric = resolve_numeric_column(&rows, 0, "en-US");

        assert_eq!(numeric.values[0], Some(1234.0));
        assert_eq!(numeric.values[1], Some(2345678.0));
    }

    #[test]
    fn column_evidence_resolves_an_otherwise_ambiguous_comma_as_decimal() {
        let rows = vec![text_row(&["1,234"]), text_row(&["12,50"])];
        let numeric = resolve_numeric_column(&rows, 0, "en-US");

        assert_eq!(numeric.values[0], Some(1.234));
        assert_eq!(numeric.values[1], Some(12.50));
    }

    #[test]
    fn with_no_column_evidence_a_lone_comma_falls_back_to_the_interface_locale() {
        let rows = vec![text_row(&["1,234"])];

        assert_eq!(resolve_numeric_column(&rows, 0, "fr-FR").values[0], Some(1.234));
        assert_eq!(resolve_numeric_column(&rows, 0, "en-US").values[0], Some(1234.0));
    }

    #[test]
    fn every_cell_sharing_the_same_ambiguous_shape_is_corroborated_as_thousands_in_either_locale() {
        // depenses.csv's own rule (tests/common/tabular_fixtures.rs): a lone comma followed by
        // exactly three digits in every one of several cells, with nothing to contradict it, is
        // corroborated as a thousands grouping regardless of the interface locale.
        let rows: Vec<Vec<CellValue>> = (1..=20)
            .map(|n| text_row(&[&format!("{n},250")]))
            .collect();

        for locale in ["fr-FR", "en-US"] {
            let numeric = resolve_numeric_column(&rows, 0, locale);
            assert_eq!(numeric.values[0], Some(1250.0), "locale {locale}");
            assert_eq!(numeric.values[19], Some(20250.0), "locale {locale}");
        }
    }

    #[test]
    fn genuinely_mixed_column_evidence_is_reported_never_guessed() {
        let rows = vec![
            text_row(&["1,234"]),   // ambiguous on its own
            text_row(&["12,50"]),   // settles comma as decimal here
            text_row(&["1,234,567"]), // settles comma as thousands here
        ];
        let numeric = resolve_numeric_column(&rows, 0, "fr-FR");

        assert_eq!(numeric.values[0], None, "conflicting evidence must not be guessed");
        assert_eq!(numeric.values[1], Some(12.50));
        assert_eq!(numeric.values[2], Some(1234567.0));
    }

    #[test]
    fn a_hundred_row_column_with_three_unreadable_cells_is_numeric_and_reports_three() {
        let mut rows = vec![text_row(&["nom", "montant"])];
        for index in 0..97 {
            rows.push(text_row(&[&format!("Personne {index}"), "10,00"]));
        }
        rows.push(text_row(&["Autre", "voir facture"]));
        rows.push(text_row(&["Autre", "a confirmer"]));
        rows.push(text_row(&["Autre", "en attente"]));

        let sheet = build_one(rows);

        assert_eq!(sheet.columns[1].inferred_type, ColumnType::Numeric);
        assert_eq!(sheet.columns[1].unparsed_count, 3);
    }
}
