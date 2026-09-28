//! `TabularInventory`: what is structurally inside a workbook, built by one full pass over its
//! cells and never inferred from a retrieved passage, a partial sample, or a model
//! (`docs/WORK-FOLDER-INVENTORY.md`'s rule, applied to the tabular pipeline).
//!
//! Deliberately a separate abstraction from `WorkFolderInventory`, not merged into it:
//! `WorkFolderInventory` stays filesystem facts only (existence, path, extension, identity,
//! size, changed/missing), and this module stays workbook facts only (sheets, header, columns,
//! types, formula presence). The two meet on one value, the content SHA-256, the same identity
//! `FileRecord::id` carries.

use serde::{Deserialize, Serialize};

use super::{CellValue, SheetData, Workbook};

/// A header candidate must be within the first rows of the sheet, and have at least one row of
/// data beneath it. This does not chase a title row sitting above the real header - the first
/// all-text, non-empty row wins - which is a deliberate limitation kept simple for Sprint 2b's
/// data-grid sheets (`docs/DECISIONS.md`, "XLSX scope for the first implementation").
const HEADER_SEARCH_ROWS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnType {
    Numeric,
    Date,
    /// The default when a column's values do not unanimously agree on numeric or date: plain
    /// text, a mix of types, or no data at all.
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
}

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
    pub fn build(
        relative_path: &str,
        workbook_id: &str,
        format: TabularFormat,
        workbook: &Workbook,
    ) -> Self {
        let sheets = workbook.sheets.iter().map(build_sheet).collect();
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
}

fn build_sheet(sheet: &SheetData) -> SheetInventory {
    let header_row = detect_header_row(&sheet.rows);
    let data_rows: &[Vec<CellValue>] = match header_row {
        Some(index) => &sheet.rows[index + 1..],
        None => &sheet.rows[..],
    };
    let column_count = sheet.rows.iter().map(Vec::len).max().unwrap_or(0);

    let columns: Vec<ColumnInventory> = (0..column_count)
        .map(|index| build_column(sheet, header_row, data_rows, index))
        .collect();
    let has_formulas = columns.iter().any(|column| column.has_formulas);

    SheetInventory {
        name: sheet.name.clone(),
        header_row,
        row_count: data_rows.len(),
        column_count,
        columns,
        has_formulas,
    }
}

fn build_column(
    sheet: &SheetData,
    header_row: Option<usize>,
    data_rows: &[Vec<CellValue>],
    index: usize,
) -> ColumnInventory {
    let name = header_row
        .and_then(|row_index| sheet.rows.get(row_index))
        .and_then(|row| row.get(index))
        .and_then(cell_text)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| format!("column_{}", index + 1));

    let mut has_formulas = false;
    let mut numeric = 0usize;
    let mut date = 0usize;
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
            CellClass::Numeric => numeric += 1,
            CellClass::Date => date += 1,
            CellClass::Other => other += 1,
        }
    }

    let inferred_type = if other == 0 && numeric > 0 && date == 0 {
        ColumnType::Numeric
    } else if other == 0 && date > 0 && numeric == 0 {
        ColumnType::Date
    } else {
        ColumnType::Categorical
    };

    ColumnInventory {
        name,
        index,
        inferred_type,
        has_formulas,
    }
}

/// A header candidate is the first row, within `HEADER_SEARCH_ROWS`, whose non-empty cells all
/// read as plain text under the same locale rules a data cell is classified with - and that has
/// at least one more row beneath it to be a header *of*. A sheet whose data starts immediately,
/// or an empty sheet, reports no header rather than guessing one.
fn detect_header_row(rows: &[Vec<CellValue>]) -> Option<usize> {
    if rows.len() < 2 {
        return None;
    }
    for (index, row) in rows.iter().enumerate().take(HEADER_SEARCH_ROWS) {
        if index + 1 >= rows.len() {
            break;
        }
        let classes: Vec<CellClass> = row.iter().map(classify_cell).collect();
        let non_empty = classes
            .iter()
            .any(|class| !matches!(class, CellClass::Empty));
        let all_textual = classes
            .iter()
            .all(|class| matches!(class, CellClass::Other | CellClass::Empty));
        if non_empty && all_textual {
            return Some(index);
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellClass {
    Empty,
    Numeric,
    Date,
    /// Text that is not itself a recognisable number or date - a label, a name, free text.
    Other,
}

fn classify_cell(cell: &CellValue) -> CellClass {
    match cell {
        CellValue::Empty => CellClass::Empty,
        CellValue::Number(_) => CellClass::Numeric,
        CellValue::Date(_) => CellClass::Date,
        CellValue::Text(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return CellClass::Empty;
            }
            if parse_locale_number(trimmed).is_some() {
                CellClass::Numeric
            } else if parse_ddmmyyyy(trimmed).is_some() {
                CellClass::Date
            } else {
                CellClass::Other
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

/// A plain decimal point, or a single decimal comma with no point present - `1234,56`, never
/// `1 234,56` or `1,234,56`. Guessing a thousands-grouping convention would risk turning a
/// mis-typed value into a wrong number; reporting the column categorical instead is the safer
/// failure.
fn parse_locale_number(text: &str) -> Option<f64> {
    if text.contains(',') && !text.contains('.') {
        let candidate = text.replacen(',', ".", 1);
        if candidate.matches('.').count() == 1 {
            return candidate.parse::<f64>().ok();
        }
        return None;
    }
    text.parse::<f64>().ok()
}

/// `dd/mm/yyyy`, checked for a plausible day and month rather than a real calendar (Sprint 2b's
/// inventory only classifies columns; it does not validate dates).
fn parse_ddmmyyyy(text: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = text.split('/').collect();
    let [day_text, month_text, year_text] = parts.as_slice() else {
        return None;
    };
    if year_text.len() != 4 {
        return None;
    }
    let day: u32 = day_text.parse().ok()?;
    let month: u32 = month_text.parse().ok()?;
    let year: u32 = year_text.parse().ok()?;
    if day == 0 || day > 31 || month == 0 || month > 12 {
        return None;
    }
    Some((day, month, year))
}

fn cell_text(cell: &CellValue) -> Option<String> {
    match cell {
        CellValue::Empty => None,
        CellValue::Text(text) => Some(text.clone()),
        CellValue::Number(number) => Some(number.to_string()),
        CellValue::Date(text) => Some(text.clone()),
        CellValue::Formula { expression, .. } => Some(expression.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let inventory =
            TabularInventory::build("data.csv", "hash-1", TabularFormat::Csv, &workbook);
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
    fn a_date_column_is_recognised_from_dd_mm_yyyy_text() {
        let sheet = build_one(vec![
            text_row(&["nom", "consultation"]),
            text_row(&["Camille", "12/03/2026"]),
            text_row(&["Esaie", "05/01/2026"]),
        ]);

        assert_eq!(sheet.columns[1].inferred_type, ColumnType::Date);
    }

    #[test]
    fn a_decimal_comma_number_is_recognised_as_numeric_not_categorical() {
        let sheet = build_one(vec![
            text_row(&["nom", "montant"]),
            text_row(&["Camille", "120,50"]),
            text_row(&["Esaie", "75,00"]),
        ]);

        assert_eq!(sheet.columns[1].inferred_type, ColumnType::Numeric);
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

        let inventory =
            TabularInventory::build("dup.xlsx", "hash-dup", TabularFormat::Xlsx, &workbook);

        assert_eq!(inventory.sheets.len(), 2);
        assert_eq!(inventory.sheets_named("Feuille1").len(), 2);
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
}
