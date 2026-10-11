//! Question suggestions that always work (lot 9).
//!
//! A suggestion is **machine data**: a template id and the slots a sentence needs (a name, a column,
//! a month and a year). The sentence itself is written by the interface, in the user's language
//! (`suggestions.*` in the catalogues), exactly as the tabular nudge's example is. Nothing here
//! produces prose, and nothing here is sent to a model.
//!
//! # A suggestion is shown only if it can be answered
//!
//! Before a suggestion leaves this module it has passed a **dry run** on the user's own files, with
//! no model and no sentence:
//!
//! - a question about a name in documents needs that name in at least two documents of the
//!   selection, and a name that is also a file name is dropped (the folder router would read the
//!   question as being about that one file, defect KBD-18);
//! - a question about rows, a total or a month is run as the equivalent `Operation` with a
//!   `FilterSpec` through `tabular::engine::execute` on the cached typed workbook, and must give a
//!   value (an error, or no row, drops the suggestion);
//! - the total of a column by group is run the same way.
//!
//! At most [`MAX_DRY_RUNS`] operations run per refresh. The sentences the interface writes for these
//! templates are proved by tests that fill each one with sample slots and ask the real classifier
//! and router (`tests/knowledge_surfaces.rs`), in both languages.
//!
//! # Names are hidden by default
//!
//! A screen can be seen by someone waiting in the room (decision D10). With
//! `knowledge_hide_names_in_suggestions` on, only the kinds that name nobody are returned;
//! `names_available` tells the interface whether a "show suggestions with names" action would show
//! anything. The reveal is the user's own click and lasts the session: the setting is not changed.
//!
//! # Selection and memory
//!
//! Only the names of the **current selection** are used (invariant I7). Nothing is stored, nothing
//! learned: a suggestion is computed on the fly (invariant I3). The result is kept in memory by
//! [`SuggestionCache`], keyed by the selection, the state of the base, the language and the
//! visibility of names, so that a second look at the same screen costs nothing.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use chrono::Datelike;
use rusqlite::Connection;
use serde::Serialize;

use super::normalize::{fold, normalize_name};
use super::query::{QuerySelection, SelectedFile};
use super::store;
use super::surface::{known_entities, selection_source_ids, KnownEntity};
use super::targeting::QueryVocabulary;
use super::Domain;
use crate::data_folder::DataFolder;
use crate::error::AppError;
use crate::index_store::IndexStore;
use crate::tabular::engine::{
    self, Comparison, FilterSpec, Operation, TabularOutcome, TabularValue,
};
use crate::tabular::inventory::{
    data_rows, resolve_date_column, text_value, ColumnType, SheetInventory,
};
use crate::tabular::question::{example_columns, name_words};
use crate::tabular_answer::{
    column_value_words, open_single_sheet_workbook, OpenedWorkbook, WorkbookChoice,
};

/// Operations run per refresh to prove that a suggestion can be answered.
pub const MAX_DRY_RUNS: usize = 5;

/// Suggestions returned when the caller gives no smaller limit.
pub const MAX_SUGGESTIONS: usize = 6;

/// A selection with more documents than this is not offered "each document": the answer would not
/// cover them all in one request.
pub const EACH_DOCUMENT_MAX_FILES: usize = 8;

/// Names looked at, best first, when a kind needs one that qualifies.
const ENTITIES_TRIED: usize = 8;

/// The rows scanned for the cell a name was read from.
const ROWS_SCANNED: usize = 5_000;

/// The cache keeps this many screens.
const CACHE_ENTRIES: usize = 8;

/// A workbook whose cells are not cached is read for a suggestion only when it is at most this long.
/// A suggestion is looked for without anyone asking for a file, so it does not start the reading of a
/// large one: that waits for the first question about it, which fills the cache.
pub const COLD_READ_LIMIT_BYTES: u64 = 2_000_000;

/// The template ids. They are the keys of the catalogue (`suggestions.<id>`).
pub mod template {
    pub const ENTITY_DOCUMENTS: &str = "entity_documents";
    pub const ENTITY_ROWS_COUNT: &str = "entity_rows_count";
    pub const ENTITY_TOTAL: &str = "entity_total";
    pub const ENTITY_PERIOD_COUNT: &str = "entity_period_count";
    pub const ENTITY_COMPARE_DOCS_DATA: &str = "entity_compare_docs_data";
    pub const TOP_GROUP: &str = "top_group";
    pub const EACH_DOCUMENT: &str = "each_document";
    pub const FOLDER_COUNT: &str = "folder_count";

    /// Every template id.
    pub const ALL: [&str; 8] = [
        ENTITY_DOCUMENTS,
        ENTITY_ROWS_COUNT,
        ENTITY_TOTAL,
        ENTITY_PERIOD_COUNT,
        ENTITY_COMPARE_DOCS_DATA,
        TOP_GROUP,
        EACH_DOCUMENT,
        FOLDER_COUNT,
    ];

    /// Whether the template puts a person's or an organisation's name on the screen.
    pub fn names_somebody(id: &str) -> bool {
        matches!(
            id,
            ENTITY_DOCUMENTS
                | ENTITY_ROWS_COUNT
                | ENTITY_TOTAL
                | ENTITY_PERIOD_COUNT
                | ENTITY_COMPARE_DOCS_DATA
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionEntity {
    pub entity_id: i64,
    pub display: String,
}

/// One question worth asking, as data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub template_id: &'static str,
    pub entity: Option<SuggestionEntity>,
    /// The text of the cell the name was read from, exactly as the table writes it. What a sentence
    /// about rows must say for the table to find them.
    pub value: Option<String>,
    /// The column to total or count by (`entity_total`, `top_group`).
    pub column: Option<String>,
    /// The column to group by (`top_group`).
    pub group: Option<String>,
    /// The workbook the question is about, set when the selection holds several (without it the
    /// product would ask which one).
    pub file: Option<String>,
    pub month: Option<u8>,
    pub year: Option<i32>,
}

impl Suggestion {
    fn bare(template_id: &'static str) -> Self {
        Self {
            template_id,
            entity: None,
            value: None,
            column: None,
            group: None,
            file: None,
            month: None,
            year: None,
        }
    }

    /// Whether it puts a name on the screen.
    pub fn names_somebody(&self) -> bool {
        template::names_somebody(self.template_id)
    }
}

/// What a refresh gives back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionSet {
    pub suggestions: Vec<Suggestion>,
    /// Names were left out because the setting hides them, and the selection holds names: the
    /// interface may offer "show suggestions with names".
    pub names_hidden: bool,
    /// How many operations the dry runs executed. For a developer, never shown.
    pub dry_runs: usize,
}

/// Everything a refresh reads.
pub struct SuggestionInput<'a> {
    pub index: &'a IndexStore,
    pub vocabulary: &'a QueryVocabulary,
    pub data: Option<&'a DataFolder>,
    /// The analysed documents of the selection.
    pub documents: &'a [SelectedFile],
    /// The usable workbooks of the selection.
    pub workbooks: &'a [WorkbookChoice],
    pub locale: &'a str,
    /// Whether a suggestion may name somebody (the setting, or the user's reveal).
    pub names_allowed: bool,
    /// Whether the folder router would read this text as the name of a file. A name for which it
    /// would is not offered in a question about documents.
    pub names_a_file: &'a dyn Fn(&str) -> bool,
    pub limit: usize,
    pub max_dry_runs: usize,
    /// See [`COLD_READ_LIMIT_BYTES`].
    pub max_cold_read_bytes: u64,
}

fn db<T>(result: rusqlite::Result<T>) -> Result<T, AppError> {
    result.map_err(|_| AppError::KnowledgeUnavailable)
}

/// The suggestions for a selection.
pub fn suggest(input: &SuggestionInput<'_>) -> Result<SuggestionSet, AppError> {
    let workbook_files: Vec<SelectedFile> = input
        .workbooks
        .iter()
        .map(|choice| SelectedFile::new(&choice.relative_path, &choice.content_id))
        .collect();
    let selection = QuerySelection {
        documents: input.documents,
        data: &workbook_files,
    };
    let mut result = SuggestionSet::default();
    if input.documents.is_empty() && input.workbooks.is_empty() {
        return Ok(result);
    }
    let connection = input.index.connection();
    let mut budget = Budget::new(input.max_dry_runs);

    // The names of the selection, best first. Only confirmed people, organisations, places and
    // items: an identifier or a term is not something to ask "about".
    let mut ranked = known_entities(connection, Some(&selection))?;
    ranked.retain(|known| {
        !known.is_unconfirmed()
            && matches!(
                known.entity.type_id.as_str(),
                "person" | "organization" | "location" | "item"
            )
    });
    ranked.sort_by(|left, right| {
        right
            .files
            .cmp(&left.files)
            .then(right.mentions.cmp(&left.mentions))
            .then(
                left.entity
                    .normalized_name
                    .cmp(&right.entity.normalized_name),
            )
            .then(left.entity.entity_id.cmp(&right.entity.entity_id))
    });
    ranked.truncate(ENTITIES_TRIED);
    result.names_hidden = !input.names_allowed && !ranked.is_empty();

    // With documents and two or more workbooks selected, the product asks "which workbook?" before
    // anything is read, whatever the question is about (defect KBD-21), so a question about the
    // documents cannot be answered and is not offered.
    let documents_are_asked_about = input.documents.is_empty() || input.workbooks.len() < 2;

    let mut found: Vec<Suggestion> = Vec::new();
    if input.names_allowed && !ranked.is_empty() {
        let sources = selection_source_ids(connection, &selection)?;
        let ids: Vec<i64> = ranked.iter().map(|known| known.entity.entity_id).collect();
        let files = store::files_for_entities(connection, &ids, &sources)?;
        let count_in = |entity_id: i64, domain: Domain| -> usize {
            files.per_entity.get(&entity_id).map_or(0, |set| {
                set.iter().filter(|source| source.domain == domain).count()
            })
        };

        // Documents: the name in at least two documents.
        if let Some(known) = ranked.iter().find(|known| {
            documents_are_asked_about
                && count_in(known.entity.entity_id, Domain::Documents) >= 2
                && !(input.names_a_file)(&known.entity.canonical_name)
        }) {
            let mut suggestion = Suggestion::bare(template::ENTITY_DOCUMENTS);
            suggestion.entity = Some(entity_of(known));
            found.push(suggestion);
        }

        // Tables: the first name that sits in a column of a workbook with one reachable sheet.
        if let Some(data) = input.data {
            if let Some(suggestions) =
                table_suggestions(input, data, connection, &sources, &ranked, &mut budget)?
            {
                found.extend(suggestions);
            }
        }

        // Both: a name in a document and in a table.
        if let Some(known) = ranked.iter().find(|known| {
            documents_are_asked_about
                && count_in(known.entity.entity_id, Domain::Documents) >= 1
                && count_in(known.entity.entity_id, Domain::Data) >= 1
                && !(input.names_a_file)(&known.entity.canonical_name)
        }) {
            let mut suggestion = Suggestion::bare(template::ENTITY_COMPARE_DOCS_DATA);
            suggestion.entity = Some(entity_of(known));
            found.push(suggestion);
        }
    }

    // Name-free kinds.
    if let Some(data) = input.data {
        if let Some(suggestion) = top_group(input, data, &mut budget) {
            found.push(suggestion);
        }
    }
    let documents = input.documents.len();
    if documents_are_asked_about && (2..=EACH_DOCUMENT_MAX_FILES).contains(&documents) {
        found.push(Suggestion::bare(template::EACH_DOCUMENT));
    }
    // With a table selected, "how many documents" is read by the table's own classifier as a number
    // of rows, so it is offered for documents alone.
    if documents >= 1 && input.workbooks.is_empty() {
        found.push(Suggestion::bare(template::FOLDER_COUNT));
    }

    found.truncate(input.limit.clamp(1, MAX_SUGGESTIONS));
    result.suggestions = found;
    result.dry_runs = budget.used;
    Ok(result)
}

fn entity_of(known: &KnownEntity) -> SuggestionEntity {
    SuggestionEntity {
        entity_id: known.entity.entity_id,
        display: known.entity.canonical_name.clone(),
    }
}

struct Budget {
    left: usize,
    used: usize,
}

impl Budget {
    fn new(max: usize) -> Self {
        Self {
            left: max.min(MAX_DRY_RUNS),
            used: 0,
        }
    }

    /// Spend one dry run. `false` when none is left.
    fn spend(&mut self) -> bool {
        if self.left == 0 {
            return false;
        }
        self.left -= 1;
        self.used += 1;
        true
    }
}

/// Run one operation as a dry run. `Some(value)` for a value, `None` for anything else.
fn dry_run(
    opened: &OpenedWorkbook,
    choice: &WorkbookChoice,
    operation: &Operation,
    locale: &str,
) -> Option<TabularValue> {
    match engine::execute(
        &opened.workbook,
        &opened.inventory,
        Some(&opened.sheet),
        choice.sheets.as_deref(),
        operation,
        locale,
    ) {
        TabularOutcome::Value { value, .. } => Some(value),
        TabularOutcome::NotDeterministicallyAnswerable { .. } => None,
    }
}

fn sheet_of<'a>(opened: &'a OpenedWorkbook, choice: &WorkbookChoice) -> Option<&'a SheetInventory> {
    engine::resolve_sheet_inventory(
        &opened.inventory,
        Some(&opened.sheet),
        choice.sheets.as_deref(),
    )
    .ok()
}

/// Whether a table value is worth asking about: it has at least one word of three letters.
fn is_askable_value(value: &str) -> bool {
    value
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.chars().count() >= 3 && word.chars().any(char::is_alphabetic))
}

/// The cell, in the column the name was read from, that writes this name. The text as the table
/// holds it, so that an equality filter on it finds the rows.
fn cell_of_name(
    opened: &OpenedWorkbook,
    sheet: &SheetInventory,
    column: &str,
    known: &KnownEntity,
    vocabulary: &QueryVocabulary,
) -> Option<String> {
    let column_index = sheet
        .columns
        .iter()
        .find(|candidate| candidate.name == column)?
        .index;
    let data = opened
        .workbook
        .sheets
        .iter()
        .find(|candidate| candidate.name == sheet.name)?;
    let rows = data_rows(sheet, data);
    let wanted: BTreeSet<String> = known
        .aliases
        .iter()
        .map(|alias| alias.normalized.clone())
        .chain(std::iter::once(known.entity.normalized_name.clone()))
        .collect();
    for row in rows.iter().take(ROWS_SCANNED) {
        let Some(cell) = row.get(column_index) else {
            continue;
        };
        let text = text_value(cell).trim().to_string();
        if text.is_empty() {
            continue;
        }
        if wanted.contains(&fold(&text))
            || wanted.contains(&normalize_name(&text, &vocabulary.titles).joined)
        {
            return Some(text);
        }
    }
    None
}

/// The words of a cell's text that a question can carry so that the tables find exactly that cell.
///
/// The tables read a question word by word: a word found in **one** distinct value of **one**
/// column becomes an equality on that whole value, a word found in several values of the column
/// (a title such as `Dr`, a first name two people share) becomes an equality on the word itself,
/// which no cell equals, and the answer is "0 rows" (defect KBD-24). So only the words that point
/// at this cell and no other are kept: `Dr Jean Dupont` is asked as `Jean Dupont` when other
/// doctors are `Dr` too. `None` when no word is left, or when a kept word also belongs to another
/// column (the product would then ask which column).
fn askable_words(
    opened: &OpenedWorkbook,
    sheet: &SheetInventory,
    column: &str,
    cell: &str,
) -> Option<String> {
    let data = opened
        .workbook
        .sheets
        .iter()
        .find(|candidate| candidate.name == sheet.name)?;
    let columns = column_value_words(sheet, data);
    let points_here = |word: &String| {
        let owners: Vec<&(String, HashMap<String, Vec<String>>)> = columns
            .iter()
            .filter(|(_, values)| values.contains_key(word))
            .collect();
        owners.len() == 1
            && owners[0].0 == column
            && owners[0]
                .1
                .get(word)
                .is_some_and(|values| values.as_slice() == [cell.to_string()])
    };
    let mut kept: Vec<&str> = Vec::new();
    for token in cell.split_whitespace() {
        let words = name_words(token);
        // The product skips a one-letter word as a particle of the sentence: it neither helps nor
        // hurts, so it is not carried.
        if words.iter().all(|word| word.chars().count() < 2) {
            continue;
        }
        if words.iter().all(points_here) {
            kept.push(token);
        }
    }
    if kept.is_empty() {
        None
    } else {
        Some(kept.join(" "))
    }
}

/// The month and year of the first row that holds `value` in `column`, read from the date column.
fn first_month_and_year(
    opened: &OpenedWorkbook,
    sheet: &SheetInventory,
    column: &str,
    value: &str,
    date_column: &str,
) -> Option<(u8, i32)> {
    let column_index = sheet
        .columns
        .iter()
        .find(|candidate| candidate.name == column)?
        .index;
    let date_index = sheet
        .columns
        .iter()
        .find(|candidate| candidate.name == date_column)?
        .index;
    let data = opened
        .workbook
        .sheets
        .iter()
        .find(|candidate| candidate.name == sheet.name)?;
    let wanted = fold(value);
    let rows = data_rows(sheet, data);
    let scanned = &rows[..rows.len().min(ROWS_SCANNED)];
    // The engine's own reading of the column, so a day-first text and a real date cell agree.
    let dates = resolve_date_column(scanned, date_index);
    for (row, date) in scanned.iter().zip(dates) {
        let Some(cell) = row.get(column_index) else {
            continue;
        };
        if fold(text_value(cell).trim()) != wanted {
            continue;
        }
        if let Some(date) = date {
            return Some((date.month() as u8, date.year()));
        }
    }
    None
}

/// The suggestions about one name that sits in a table: rows, a total, a month. `None` when no
/// ranked name qualifies. Each spends dry runs; a name whose first dry run fails gives nothing.
fn table_suggestions(
    input: &SuggestionInput<'_>,
    data: &DataFolder,
    connection: &Connection,
    sources: &[i64],
    ranked: &[KnownEntity],
    budget: &mut Budget,
) -> Result<Option<Vec<Suggestion>>, AppError> {
    if input.workbooks.is_empty() {
        return Ok(None);
    }
    let in_sources = serde_json::to_string(sources).map_err(|_| AppError::Internal)?;
    let several = input.workbooks.len() > 1;
    let mut statement = db(connection.prepare(
        "SELECT s.relative_path, m.sheet, m.column_name
         FROM kb_effective_mentions m
         JOIN kb_sources s ON s.source_id = m.source_id
         WHERE m.effective_entity_id = ?1 AND s.domain = 'data' AND m.locator_kind = 'cells'
           AND m.sheet IS NOT NULL AND m.column_name IS NOT NULL
           AND m.source_id IN (SELECT value FROM json_each(?2))
         GROUP BY s.relative_path, m.sheet, m.column_name
         ORDER BY SUM(m.occurrence_count) DESC, s.relative_path, m.sheet, m.column_name",
    ))?;
    for known in ranked {
        if budget.left == 0 {
            break;
        }
        let rows = db(statement.query_map(
            rusqlite::params![known.entity.entity_id, in_sources],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        ))?;
        let places: Vec<(String, String, String)> = rows
            .collect::<Result<_, _>>()
            .map_err(|_| AppError::KnowledgeUnavailable)?;
        for (path, sheet_name, column) in places {
            let Some(choice) = input
                .workbooks
                .iter()
                .find(|choice| choice.relative_path == path)
            else {
                continue;
            };
            let Some(opened) = open_single_sheet_workbook(
                data,
                choice,
                input.locale,
                input.index,
                input.max_cold_read_bytes,
            ) else {
                continue;
            };
            // A sentence names no sheet: the one reachable sheet must be the one the name is in.
            let Some(sheet) = sheet_of(&opened, choice) else {
                continue;
            };
            if sheet.name != sheet_name {
                continue;
            }
            let Some(cell) = cell_of_name(&opened, sheet, &column, known, input.vocabulary) else {
                continue;
            };
            // What the sentence will say about the cell, and nothing the tables would misread.
            let Some(value) = askable_words(&opened, sheet, &column, &cell) else {
                continue;
            };
            if !is_askable_value(&value) {
                continue;
            }
            let equals = FilterSpec {
                column: column.clone(),
                comparison: Comparison::Equals(cell.clone()),
            };
            let file = several.then(|| path.clone());
            let base = |template_id: &'static str| {
                let mut suggestion = Suggestion::bare(template_id);
                suggestion.entity = Some(entity_of(known));
                suggestion.value = Some(value.clone());
                suggestion.file = file.clone();
                suggestion
            };

            // Rows.
            if !budget.spend() {
                return Ok(None);
            }
            let counted = dry_run(
                &opened,
                choice,
                &Operation::Count {
                    filters: vec![equals.clone()],
                },
                input.locale,
            );
            if !matches!(counted, Some(TabularValue::Count(rows)) if rows > 0) {
                continue;
            }
            let mut out = vec![base(template::ENTITY_ROWS_COUNT)];

            // A total of a column that is an amount, never the year.
            let (measure, _) = example_columns(&sheet.columns);
            if let Some(measure) = measure.filter(|measure| *measure != column) {
                if budget.spend() {
                    let total = dry_run(
                        &opened,
                        choice,
                        &Operation::Sum {
                            column: measure.clone(),
                            filters: vec![equals.clone()],
                        },
                        input.locale,
                    );
                    if matches!(total, Some(TabularValue::Sum(_))) {
                        let mut suggestion = base(template::ENTITY_TOTAL);
                        suggestion.column = Some(measure);
                        out.push(suggestion);
                    }
                }
            }

            // A month and a year taken from the data.
            if let Some(date_column) = sheet
                .columns
                .iter()
                .find(|candidate| candidate.inferred_type == ColumnType::Date)
            {
                if let Some((month, year)) =
                    first_month_and_year(&opened, sheet, &column, &cell, &date_column.name)
                {
                    if budget.spend() {
                        let period = dry_run(
                            &opened,
                            choice,
                            &Operation::Count {
                                filters: vec![
                                    equals.clone(),
                                    FilterSpec {
                                        column: date_column.name.clone(),
                                        comparison: Comparison::Month(month),
                                    },
                                    FilterSpec {
                                        column: date_column.name.clone(),
                                        comparison: Comparison::Year(year),
                                    },
                                ],
                            },
                            input.locale,
                        );
                        if matches!(period, Some(TabularValue::Count(rows)) if rows > 0) {
                            let mut suggestion = base(template::ENTITY_PERIOD_COUNT);
                            suggestion.month = Some(month);
                            suggestion.year = Some(year);
                            out.push(suggestion);
                        }
                    }
                }
            }
            return Ok(Some(out));
        }
    }
    Ok(None)
}

/// "The total of an amount by group": the first workbook that has both columns and answers.
fn top_group(
    input: &SuggestionInput<'_>,
    data: &DataFolder,
    budget: &mut Budget,
) -> Option<Suggestion> {
    for choice in input.workbooks {
        let Some(opened) = open_single_sheet_workbook(
            data,
            choice,
            input.locale,
            input.index,
            input.max_cold_read_bytes,
        ) else {
            continue;
        };
        let Some(sheet) = sheet_of(&opened, choice) else {
            continue;
        };
        let (Some(measure), Some(group)) = example_columns(&sheet.columns) else {
            continue;
        };
        // A column of dates the file does not settle is not something to group by.
        let group_is_dates = sheet
            .columns
            .iter()
            .any(|column| column.name == group && column.ambiguous_date);
        if measure == group || group_is_dates || !budget.spend() {
            continue;
        }
        let ran = dry_run(
            &opened,
            choice,
            &Operation::GroupSum {
                group_by: group.clone(),
                sum_column: measure.clone(),
                filters: Vec::new(),
            },
            input.locale,
        );
        if matches!(ran, Some(TabularValue::GroupSums(ref groups)) if !groups.is_empty()) {
            let mut suggestion = Suggestion::bare(template::TOP_GROUP);
            suggestion.column = Some(measure);
            suggestion.group = Some(group);
            suggestion.file = (input.workbooks.len() > 1).then(|| choice.relative_path.clone());
            return Some(suggestion);
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// The cache
// ---------------------------------------------------------------------------------------------

/// The last few screens, kept in memory. A key changes whenever the selection, the language, the
/// visibility of names or the content of the base changes, so nothing stale is ever served.
#[derive(Default)]
pub struct SuggestionCache {
    entries: Mutex<Vec<(u64, SuggestionSet)>>,
}

impl SuggestionCache {
    pub fn get(&self, key: u64) -> Option<SuggestionSet> {
        let entries = self.entries.lock().ok()?;
        entries
            .iter()
            .find(|(known, _)| *known == key)
            .map(|(_, set)| set.clone())
    }

    pub fn put(&self, key: u64, set: SuggestionSet) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        entries.retain(|(known, _)| *known != key);
        entries.push((key, set));
        if entries.len() > CACHE_ENTRIES {
            entries.remove(0);
        }
    }
}

/// The key of a screen: the selection (path and content of every file), the language, whether names
/// are shown, and a fingerprint of the base (the generation of the name list and how many rows each
/// table holds), so a new Analyse or a management write changes it.
pub fn cache_key(
    connection: &Connection,
    documents: &[SelectedFile],
    workbooks: &[WorkbookChoice],
    locale: &str,
    names_allowed: bool,
) -> Result<u64, AppError> {
    let mut hasher = DefaultHasher::new();
    locale.hash(&mut hasher);
    names_allowed.hash(&mut hasher);
    let mut files: Vec<(&str, &str, &str)> = documents
        .iter()
        .map(|file| ("d", file.relative_path.as_str(), file.content_id.as_str()))
        .chain(workbooks.iter().map(|choice| {
            (
                "w",
                choice.relative_path.as_str(),
                choice.content_id.as_str(),
            )
        }))
        .collect();
    files.sort_unstable();
    files.hash(&mut hasher);
    for choice in workbooks {
        choice.sheets.hash(&mut hasher);
    }
    let epoch = store::meta(connection, "gazetteer_epoch")?.unwrap_or_default();
    epoch.hash(&mut hasher);
    for query in [
        "SELECT COUNT(*), COALESCE(MAX(mention_id), 0) FROM kb_mentions",
        "SELECT COUNT(*), COALESCE(MAX(entity_id), 0) FROM kb_entities",
        "SELECT COUNT(*), COALESCE(SUM(kb_version), 0) FROM kb_sources",
        "SELECT COUNT(*), COALESCE(MAX(op_id), 0) FROM kb_ops_log",
        // Which workbooks are cached: a question that fills the cache makes new suggestions possible.
        "SELECT COUNT(*), COALESCE(MAX(rowid), 0) FROM tabular_workbooks",
    ] {
        let pair: (i64, i64) =
            db(connection.query_row(query, [], |row| Ok((row.get(0)?, row.get(1)?))))?;
        pair.hash(&mut hasher);
    }
    Ok(hasher.finish())
}
