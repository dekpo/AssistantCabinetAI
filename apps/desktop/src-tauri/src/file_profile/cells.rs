//! The words of a workbook's cells, for finding a workbook by what it says (`docs/LEXICAL-RETRIEVAL-DESIGN.md`
//! 4.5, owner decision D2).
//!
//! **Read only.** This reads the typed workbook the Analyse already stored (`tabular_workbooks`), the
//! inventory it built (`tabular_inventories`) and the way the knowledge base read the columns
//! (`kb_column_semantics`), and returns plain values for the profile's own tables. Nothing here writes to
//! the spreadsheet tables, and nothing here changes how a column is typed, read or queried.
//!
//! Rules (the owner's answers on names, organisations and identifiers):
//!
//! - **Text values are searchable by word**: names of people, companies, organisations and places, free
//!   remarks, codes. A column the knowledge base reads as an entity column keeps producing entities as it
//!   does today; its values are also searchable as words.
//! - **Identifiers are kept as exact tokens**, compact and grouped (`identifiers.rs`), wherever a cell holds
//!   one.
//! - **Numbers, amounts and dates are excluded** from the words: a numeric column is the table engine's
//!   data, and a date column contributes only its **date range**.
//! - **Size rule.** A free-text column keeps at most `MAX_FREE_TEXT_DISTINCT` distinct values; past that the
//!   column is `truncated` and every answer that rests on it says so. Entity columns and identifiers are not
//!   capped: they are what the search is for.

use std::collections::{BTreeSet, HashSet};

use chrono::NaiveDate;

use super::identifiers::{identifiers_in, IdentifierForms};
use super::name::words_of;
use super::vocabulary::ProfileVocabulary;
use crate::knowledge::packs::PackSet;
use crate::tabular::inventory::{
    data_rows, parse_date_value, text_value, ColumnType, TabularInventory,
};
use crate::tabular::{CellValue, Workbook};

/// Distinct values kept per free-text column. Measured on the fixtures, to be revisited on real data.
pub const MAX_FREE_TEXT_DISTINCT: usize = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnRole {
    /// A column the knowledge base read as people, organisations, places...
    Entity,
    /// Any other text column.
    FreeText,
}

impl ColumnRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::FreeText => "free_text",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnCells {
    pub sheet: String,
    pub column: String,
    pub role: ColumnRole,
    /// Distinct non-empty values seen, before any cap.
    pub distinct: usize,
    pub truncated: bool,
    /// The distinct values kept, in order of first appearance.
    pub values: Vec<String>,
}

impl ColumnCells {
    /// `Sheet!Column`, how a place in a workbook is written everywhere in the profile.
    pub fn locator(&self) -> String {
        format!("{}!{}", self.sheet, self.column)
    }

    /// The folded words of the kept values, each once, sorted.
    pub fn words(&self) -> BTreeSet<String> {
        self.values
            .iter()
            .flat_map(|value| words_of(value))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateRange {
    pub sheet: String,
    pub column: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CellsReading {
    pub columns: Vec<ColumnCells>,
    pub date_ranges: Vec<DateRange>,
    pub identifiers: Vec<IdentifierForms>,
}

/// Read the cells of `workbook`. `entity_columns` is the `(sheet, column)` pairs the knowledge base read.
pub fn read_cells(
    workbook: &Workbook,
    inventory: &TabularInventory,
    entity_columns: &HashSet<(String, String)>,
    packs: &PackSet,
    vocabulary: &ProfileVocabulary,
) -> CellsReading {
    let mut reading = CellsReading::default();
    let mut identifier_keys: HashSet<(String, String)> = HashSet::new();
    for (sheet_inventory, sheet_data) in inventory.sheets.iter().zip(&workbook.sheets) {
        let rows = data_rows(sheet_inventory, sheet_data);
        for column in &sheet_inventory.columns {
            match column.inferred_type {
                ColumnType::Numeric => {}
                ColumnType::Date => {
                    let mut dates = rows
                        .iter()
                        .filter_map(|row| row.get(column.index))
                        .filter_map(date_of);
                    if let Some(first) = dates.next() {
                        let (from, to) = dates.fold((first, first), |(from, to), date| {
                            (from.min(date), to.max(date))
                        });
                        reading.date_ranges.push(DateRange {
                            sheet: sheet_inventory.name.clone(),
                            column: column.name.clone(),
                            from,
                            to,
                        });
                    }
                }
                ColumnType::Categorical => {
                    let role = if entity_columns
                        .contains(&(sheet_inventory.name.clone(), column.name.clone()))
                    {
                        ColumnRole::Entity
                    } else {
                        ColumnRole::FreeText
                    };
                    let mut seen: HashSet<String> = HashSet::new();
                    let mut cells = ColumnCells {
                        sheet: sheet_inventory.name.clone(),
                        column: column.name.clone(),
                        role,
                        distinct: 0,
                        truncated: false,
                        values: Vec::new(),
                    };
                    for row in rows {
                        let Some(cell) = row.get(column.index) else {
                            continue;
                        };
                        let text = text_value(cell);
                        let text = text.trim();
                        if text.is_empty() {
                            continue;
                        }
                        // An identifier is kept whatever the column, and does not count against the cap.
                        if text.len() >= 6 && text.chars().any(|c| c.is_ascii_digit() || c == '@') {
                            for identifier in identifiers_in(text, packs, vocabulary) {
                                if identifier_keys.insert((
                                    identifier.scheme_id.clone(),
                                    identifier.compact.clone(),
                                )) {
                                    reading.identifiers.push(identifier);
                                }
                            }
                        }
                        // A cell with no letter is a number, an amount or a date: not a word.
                        if !text.chars().any(char::is_alphabetic) {
                            continue;
                        }
                        let key = words_of(text).join(" ");
                        if key.is_empty() || !seen.insert(key) {
                            continue;
                        }
                        cells.distinct += 1;
                        if role == ColumnRole::FreeText
                            && cells.values.len() >= MAX_FREE_TEXT_DISTINCT
                        {
                            cells.truncated = true;
                            continue;
                        }
                        cells.values.push(text.to_string());
                    }
                    if cells.distinct > 0 {
                        reading.columns.push(cells);
                    }
                }
            }
        }
    }
    reading
}

fn date_of(cell: &CellValue) -> Option<NaiveDate> {
    match cell {
        CellValue::Date(date) => Some(*date),
        CellValue::Text(text) => parse_date_value(text.trim()),
        CellValue::Formula {
            cached_value: Some(inner),
            ..
        } => date_of(inner),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabular::inventory::TabularFormat;
    use crate::tabular::SheetData;

    fn text(value: &str) -> CellValue {
        if value.is_empty() {
            CellValue::Empty
        } else {
            CellValue::Text(value.to_string())
        }
    }

    fn workbook(rows: &[&[&str]]) -> (Workbook, TabularInventory) {
        let sheet = SheetData {
            name: "Visits".to_string(),
            rows: rows
                .iter()
                .map(|row| row.iter().map(|cell| text(cell)).collect())
                .collect(),
        };
        let workbook = Workbook {
            sheets: vec![sheet],
        };
        let inventory =
            TabularInventory::build("visits.csv", "id", TabularFormat::Csv, &workbook, "fr-FR");
        (workbook, inventory)
    }

    fn read(rows: &[&[&str]], entity: &[(&str, &str)]) -> CellsReading {
        let (workbook, inventory) = workbook(rows);
        let entity_columns = entity
            .iter()
            .map(|(s, c)| (s.to_string(), c.to_string()))
            .collect();
        read_cells(
            &workbook,
            &inventory,
            &entity_columns,
            &PackSet::load("fr-FR", &["health-fr", "health-ch"]).expect("loads"),
            &ProfileVocabulary::load(&["health-fr", "health-ch"]).expect("loads"),
        )
    }

    const HEADER: &[&str] = &["Patient", "Remarks", "Date", "Amount"];

    #[test]
    fn text_values_are_words_and_numbers_and_dates_are_not() {
        let reading = read(
            &[
                HEADER,
                &[
                    "Marie Martin",
                    "allergy to penicillin",
                    "2026-03-02",
                    "45.50",
                ],
                &["Jean Dupont", "moved to Lyon", "2026-05-20", "60"],
                &["Marie Martin", "allergy to penicillin", "2026-04-11", "30"],
            ],
            &[("Visits", "Patient")],
        );
        let by_column = |name: &str| reading.columns.iter().find(|c| c.column == name);
        let patient = by_column("Patient").expect("a column of names");
        assert_eq!((patient.role, patient.distinct), (ColumnRole::Entity, 2));
        assert!(patient.words().contains("martin") && patient.words().contains("dupont"));
        let remarks = by_column("Remarks").expect("a free-text column");
        assert_eq!(
            (remarks.role, remarks.distinct, remarks.truncated),
            (ColumnRole::FreeText, 2, false)
        );
        assert!(remarks.words().contains("penicillin") && remarks.words().contains("lyon"));
        assert!(
            by_column("Date").is_none() && by_column("Amount").is_none(),
            "no words from dates or amounts"
        );
        let range = reading
            .date_ranges
            .first()
            .expect("the date column gives its range");
        assert_eq!(
            (
                range.column.as_str(),
                range.from.to_string(),
                range.to.to_string()
            ),
            ("Date", "2026-03-02".to_string(), "2026-05-20".to_string())
        );
    }

    #[test]
    fn an_identifier_in_a_cell_is_kept_in_every_spelling_and_a_bare_number_is_not_a_word() {
        let reading = read(
            &[
                &["Patient", "Insurance number", "Note"],
                &["Marie Martin", "756.3052.5778.43", "12.50"],
                &["Jean Dupont", "7563052577843", "n/a"],
            ],
            &[],
        );
        let compacts: Vec<&str> = reading
            .identifiers
            .iter()
            .map(|i| i.compact.as_str())
            .collect();
        assert_eq!(
            compacts,
            vec!["7563052577843"],
            "two spellings, one identifier"
        );
        assert_eq!(
            reading.identifiers[0].grouped.as_deref(),
            Some("756 3052 5778 43")
        );
        let numbers = reading
            .columns
            .iter()
            .find(|c| c.column == "Insurance number");
        assert!(numbers.is_none(), "a column of numbers has no words");
        let note = reading
            .columns
            .iter()
            .find(|c| c.column == "Note")
            .expect("one word");
        assert_eq!(note.values, vec!["n/a"]);
    }

    #[test]
    fn a_free_text_column_is_capped_and_says_so_while_an_entity_column_is_not() {
        let mut rows: Vec<Vec<String>> = vec![vec!["Patient".to_string(), "Remarks".to_string()]];
        for index in 0..MAX_FREE_TEXT_DISTINCT + 25 {
            rows.push(vec![
                format!("Person Number{index}x"),
                format!("remark number{index}x"),
            ]);
        }
        let borrowed: Vec<Vec<&str>> = rows
            .iter()
            .map(|row| row.iter().map(String::as_str).collect())
            .collect();
        let slices: Vec<&[&str]> = borrowed.iter().map(Vec::as_slice).collect();
        let reading = read(&slices, &[("Visits", "Patient")]);
        let remarks = reading
            .columns
            .iter()
            .find(|c| c.column == "Remarks")
            .expect("free text");
        assert_eq!(
            (remarks.values.len(), remarks.distinct, remarks.truncated),
            (MAX_FREE_TEXT_DISTINCT, MAX_FREE_TEXT_DISTINCT + 25, true)
        );
        let patient = reading
            .columns
            .iter()
            .find(|c| c.column == "Patient")
            .expect("entities");
        assert_eq!(
            (patient.values.len(), patient.truncated),
            (MAX_FREE_TEXT_DISTINCT + 25, false),
            "an entity column is not capped"
        );
    }

    #[test]
    fn an_empty_workbook_or_empty_cells_give_nothing() {
        assert_eq!(read(&[], &[]), CellsReading::default());
        let blanks = read(
            &[
                HEADER,
                &["Marie", "ok", "2026-01-02", "5"],
                &["", "", "", ""],
            ],
            &[],
        );
        assert!(
            blanks.columns.iter().all(|c| c.distinct == 1),
            "a blank cell is not a value"
        );
        let blanks = read(&[&["", ""], &["", ""]], &[]);
        assert!(blanks.columns.is_empty() && blanks.identifiers.is_empty());
    }

    #[test]
    fn a_date_column_that_holds_typed_dates_gives_its_range() {
        let sheet = SheetData {
            name: "Plan".to_string(),
            rows: vec![
                vec![text("Day"), text("Task")],
                vec![
                    CellValue::Date(NaiveDate::from_ymd_opt(2026, 1, 5).expect("a date")),
                    text("inventory"),
                ],
                vec![
                    CellValue::Date(NaiveDate::from_ymd_opt(2026, 9, 30).expect("a date")),
                    text("audit"),
                ],
            ],
        };
        let workbook = Workbook {
            sheets: vec![sheet],
        };
        let inventory =
            TabularInventory::build("plan.xlsx", "id", TabularFormat::Xlsx, &workbook, "fr-FR");
        let reading = read_cells(
            &workbook,
            &inventory,
            &HashSet::new(),
            &PackSet::load("fr-FR", &[]).expect("loads"),
            &ProfileVocabulary::load(&[]).expect("loads"),
        );
        assert_eq!(reading.date_ranges.len(), 1);
        assert_eq!(reading.date_ranges[0].from.to_string(), "2026-01-05");
        assert_eq!(reading.date_ranges[0].to.to_string(), "2026-09-30");
    }
}
