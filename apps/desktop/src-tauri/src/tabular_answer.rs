//! Tier 2 of the grounding priority chain (`docs/SELECTION-AND-MEMORY.md`): tables are selected
//! and no document is. Every question on this tier is answered by `tabular::structural` or
//! `tabular::engine`, and nothing else - no gateway call, no model, no fallback to open chat.
//!
//! When the engine cannot answer exactly, the answer is still useful: a `Nudge` naming the real
//! sheets and columns of the workbook, so the interface can suggest a question the engine does
//! answer (`docs/SESSION-DATA-05-TABULAR-UI.md` section 5b). It is built from what the engine and
//! the cached inventory already hold. An unclassified question, or a
//! `NOT_DETERMINISTICALLY_ANSWERABLE` outcome, is never sent to a model "to be helpful".
//!
//! Machine codes and data only, exactly like `folder_questions::FolderAnswer`: the interface
//! writes every sentence, in her language.
//!
//! `answer` takes no gateway, no model alias and no network client: that the tabular tier cannot
//! reach a model is a property of its signature, not a promise.

use std::collections::HashMap;

use serde::Serialize;

use crate::analysis_scope::ScopeMode;
use crate::data_folder::DataFolder;
use crate::error::AppError;
use crate::file_record::{FileRecord, ProcessingStatus};
use crate::file_reference::{fold_text, FileReferenceResolver, ReferenceStatus};
use crate::inventory::WorkFolderInventory;
use crate::tabular::engine::{
    self, Comparison, FilterSpec, NotAnswerableReason, Operation, TabularDerivation,
    TabularLocator, TabularOutcome, TabularValue,
};
use crate::tabular::inventory::{
    self, data_rows, text_value, ColumnInventory, ColumnType, SheetInventory, TabularInventory,
};
use crate::tabular::question::{self, name_words, TabularRoute};
use crate::tabular::structural::{self, StructuralAnswer};
use crate::tabular::{self, TabularError, Workbook};

/// What a question about the selected tables came to. `file` is always the workbook's path
/// relative to the Data Folder, so the interface can cite it beside the sheet.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum TabularAnswer {
    /// A value the engine computed. `locator.sheet` is the citation: where a document answer says
    /// "page 3", this says which sheet.
    Value {
        file: String,
        value: TabularValue,
        locator: TabularLocator,
        derivation: TabularDerivation,
    },
    /// A structural fact read from the inventory: sheets, rows, columns, formulas.
    Structural {
        file: String,
        answer: StructuralAnswer,
    },
    /// The engine could not answer this exactly. What the workbook does hold, so the interface can
    /// say which question would work. `reason` is `None` when the question was not recognised at
    /// all, and the engine's own reason otherwise.
    Nudge {
        file: String,
        reason: Option<NotAnswerableReason>,
        available_sheets: Vec<String>,
        available_columns: Vec<String>,
        /// A column the engine would total as it stands - an amount, never a year or an
        /// identifier (`question::example_columns`) - for a concrete example ("somme de
        /// montant"). `None` when the sheet has none.
        example_column: Option<String>,
        /// A text column to group by, for a second example ("which agency has the most
        /// amount"). `None` when the sheet has none, or no column to total.
        example_group: Option<String>,
        /// Set only when `reason` is `FilterNotSupported`: the column a residual question word
        /// was found in, when one was - a bare number with no matching column leaves this
        /// `None` even though `filter_value` is set. `None` for every other nudge
        /// (`docs/DECISIONS.md`, session 7's D1).
        filter_column: Option<String>,
        /// Set when `reason` is `FilterNotSupported` or `ValueNotFound`: the word from the
        /// question that named real data, or looked like an attempt to, exactly as she typed it.
        /// `None` for every other nudge.
        filter_value: Option<String>,
        /// Set only when `reason` is `ValueNotFound`: up to five real values close to
        /// `filter_value` (folded prefix or edit distance at most two), so she can see what the
        /// column actually holds instead of being told only that nothing matched
        /// (`docs/DECISIONS.md`, session 11). Empty for every other nudge.
        #[serde(default)]
        close_values: Vec<String>,
    },
    /// A group question with several columns it could total, none named. Asked, never picked.
    WhichMeasure {
        file: String,
        group_column: String,
        candidates: Vec<String>,
    },
    /// A residual word named real data in more than one reachable column - a filter value, never
    /// picked between silently (`docs/DECISIONS.md`, session 11).
    WhichColumn {
        file: String,
        value: String,
        candidates: Vec<String>,
    },
    /// Analysed, and nothing in it looks like a table (red on the listing). The same honesty an
    /// unreadable scan gets.
    WorkbookUnreadable { file: String },
    /// Chosen, but its analysis is gone (the Data Folder was reset) or it was never analysed.
    WorkbookNotAnalysed { file: String },
    /// Several workbooks are selected and the question names none of them. Never picked between.
    WhichWorkbook { candidates: Vec<String> },
    /// The question names a file that more than one selected workbook matches.
    AmbiguousReference {
        query: String,
        candidates: Vec<String>,
    },
    /// The question names a workbook the Data Folder holds but this conversation did not select.
    FileNotSelected { query: String },
    /// The question names a file the Data Folder does not hold.
    NoMatchingFile { query: String },
    /// "Tous" was chosen, and no workbook in the folder is usable.
    NoUsableTable,
}

/// One selected workbook, with the sheets its entry was narrowed to.
struct Chosen<'a> {
    record: &'a FileRecord,
    sheets: Option<Vec<String>>,
}

/// Answer one question from the tables `selection` chose in `data`.
///
/// Refuses with `ScopeUnavailable` exactly as the document path does when every chosen file is
/// gone or has changed since it was ticked. A red or unanalysed workbook is never answered from,
/// whatever the selection says: the interface offers it no checkbox, and this refuses it again.
pub fn answer(
    question: &str,
    data: &DataFolder,
    selection: &ScopeMode,
    locale: &str,
) -> Result<TabularAnswer, AppError> {
    let chosen = chosen_workbooks(data, selection)?;
    if chosen.is_empty() {
        return Ok(TabularAnswer::NoUsableTable);
    }

    let target = match pick_target(question, data, &chosen) {
        Ok(target) => target,
        Err(answer) => return Ok(answer),
    };
    let file = target.record.relative_path.clone();
    match target.record.processing_status {
        ProcessingStatus::Indexed => {}
        ProcessingStatus::Failed => return Ok(TabularAnswer::WorkbookUnreadable { file }),
        _ => return Ok(TabularAnswer::WorkbookNotAnalysed { file }),
    }
    let Some(inventory) = data.usable_inventory(&file) else {
        return Ok(TabularAnswer::WorkbookNotAnalysed { file });
    };
    let allowed = target.sheets.as_deref();

    let (route, residual, named_column) =
        question::classify_traced(question, inventory, allowed, locale);
    match route {
        TabularRoute::Structural(structural_question) => {
            match structural::answer(inventory, allowed, &structural_question) {
                StructuralAnswer::NotAnswerable {
                    reason,
                    available_sheets,
                } => Ok(nudge(
                    file,
                    inventory,
                    allowed,
                    Some(reason),
                    available_sheets,
                    Vec::new(),
                    None,
                    None,
                    Vec::new(),
                )),
                answer => Ok(TabularAnswer::Structural { file, answer }),
            }
        }
        TabularRoute::Operation { sheet, operation } => {
            // Re-read and re-hashed at the moment of computing: a workbook that changed since
            // its analysis is refused, never computed against a stale inventory.
            let path = data
                .files()
                .absolute_path(target.record)
                .ok_or(AppError::ScopeUnavailable)?;
            let (workbook, fresh) =
                match tabular::load_current(&path, &file, &inventory.workbook_id, locale) {
                    Ok(pair) => pair,
                    Err(TabularError::WorkbookChanged) => return Err(AppError::ScopeUnavailable),
                    Err(_) => return Ok(TabularAnswer::WorkbookUnreadable { file }),
                };

            // Session 11: a residual word anchored on real data becomes the filters
            // `tabular::engine` actually runs under, rather than only naming what was seen
            // (`docs/DECISIONS.md`).
            let operation = match detect_filters(
                question,
                &residual,
                &fresh,
                &workbook,
                sheet.as_deref(),
                allowed,
                operation_column_name(&operation),
                named_column.as_deref(),
                locale,
            ) {
                FilterDetection::Filters(filters) => with_filters(operation, filters),
                FilterDetection::WhichColumn { value, candidates } => {
                    return Ok(TabularAnswer::WhichColumn {
                        file,
                        value,
                        candidates,
                    });
                }
                FilterDetection::ValueNotFound { value, close } => {
                    return Ok(nudge(
                        file,
                        &fresh,
                        allowed,
                        Some(NotAnswerableReason::ValueNotFound),
                        Vec::new(),
                        Vec::new(),
                        None,
                        Some(value),
                        close,
                    ));
                }
                FilterDetection::None => operation,
            };

            match engine::execute(&workbook, &fresh, sheet.as_deref(), allowed, &operation, locale) {
                TabularOutcome::Value {
                    value,
                    locator,
                    derivation,
                } => Ok(TabularAnswer::Value {
                    file,
                    value,
                    locator,
                    derivation,
                }),
                TabularOutcome::NotDeterministicallyAnswerable {
                    reason,
                    available_sheets,
                    available_columns,
                } => Ok(nudge(
                    file,
                    &fresh,
                    allowed,
                    Some(reason),
                    available_sheets,
                    available_columns,
                    None,
                    None,
                    Vec::new(),
                )),
            }
        }
        TabularRoute::WhichMeasure {
            group_by,
            candidates,
        } => Ok(TabularAnswer::WhichMeasure {
            file,
            group_column: group_by,
            candidates,
        }),
        TabularRoute::NotRecognised => {
            let sheets = sheet_names(engine::reachable_sheets(inventory, allowed));
            Ok(nudge(
                file, inventory, allowed, None, sheets, Vec::new(), None, None, Vec::new(),
            ))
        }
    }
}

/// The workbooks the selection keeps. "Tous" keeps every green workbook, so an empty result
/// there means the folder has nothing usable; an explicit selection keeps whatever it pinned that
/// is still there unchanged, green or not, so that `answer` can say what is wrong with it.
fn chosen_workbooks<'a>(
    data: &'a DataFolder,
    selection: &ScopeMode,
) -> Result<Vec<Chosen<'a>>, AppError> {
    match selection {
        ScopeMode::WholeFolder => Ok(data
            .files()
            .all_files()
            .iter()
            .filter(|record| record.processing_status == ProcessingStatus::Indexed)
            .map(|record| Chosen {
                record,
                sheets: None,
            })
            .collect()),
        ScopeMode::Explicit(entries) => {
            let chosen: Vec<Chosen> = entries
                .iter()
                .filter_map(|entry| {
                    let record = data.files().find_by_relative_path(&entry.relative_path)?;
                    (record.id == entry.pinned_id).then(|| Chosen {
                        record,
                        sheets: (!entry.sheet_names.is_empty()).then(|| entry.sheet_names.clone()),
                    })
                })
                .collect();
            // She chose workbooks and none of them is there as she chose it: say so rather than
            // answer from nothing, the same refusal as a document selection gets.
            if chosen.is_empty() {
                return Err(AppError::ScopeUnavailable);
            }
            Ok(chosen)
        }
    }
}

/// Which chosen workbook the question is about: the one it names, or the only usable one. Never
/// a guess between two.
fn pick_target<'c, 'a>(
    question: &str,
    data: &'a DataFolder,
    chosen: &'c [Chosen<'a>],
) -> Result<&'c Chosen<'a>, TabularAnswer> {
    let scoped = WorkFolderInventory::from_records(
        data.files().root(),
        chosen.iter().map(|each| each.record.clone()).collect(),
    );
    let find = |relative_path: &str| {
        chosen
            .iter()
            .find(|each| each.record.relative_path == relative_path)
    };

    match FileReferenceResolver::new(&scoped).resolve_in_question(question) {
        Some(resolution) => match resolution.status {
            ReferenceStatus::Exact => resolution
                .exact_match
                .as_ref()
                .and_then(|record| find(&record.relative_path))
                .ok_or(TabularAnswer::NoMatchingFile {
                    query: resolution.query.clone(),
                }),
            ReferenceStatus::MultipleMatches => Err(TabularAnswer::AmbiguousReference {
                query: resolution.query,
                candidates: resolution
                    .candidates
                    .into_iter()
                    .map(|record| record.relative_path)
                    .collect(),
            }),
            ReferenceStatus::NoMatch => {
                let elsewhere = FileReferenceResolver::new(data.files()).resolve(&resolution.query);
                Err(match elsewhere.exact_match {
                    Some(record) if record.processing_status == ProcessingStatus::Failed => {
                        TabularAnswer::WorkbookUnreadable {
                            file: record.relative_path,
                        }
                    }
                    _ if elsewhere.status != ReferenceStatus::NoMatch => {
                        TabularAnswer::FileNotSelected {
                            query: resolution.query,
                        }
                    }
                    _ => TabularAnswer::NoMatchingFile {
                        query: resolution.query,
                    },
                })
            }
        },
        None => {
            let usable: Vec<&Chosen> = chosen
                .iter()
                .filter(|each| each.record.processing_status == ProcessingStatus::Indexed)
                .collect();
            match usable.len() {
                1 => Ok(usable[0]),
                // Nothing chosen is usable: the first one tells her why.
                0 => Ok(&chosen[0]),
                _ => Err(TabularAnswer::WhichWorkbook {
                    candidates: usable
                        .iter()
                        .map(|each| each.record.relative_path.clone())
                        .collect(),
                }),
            }
        }
    }
}

/// A refusal turned into a next step. The columns come from the engine when it resolved a sheet,
/// and from the inventory when exactly one sheet is reachable - never from a sheet outside the
/// scope, and never invented.
fn nudge(
    file: String,
    inventory: &TabularInventory,
    allowed: Option<&[String]>,
    reason: Option<NotAnswerableReason>,
    available_sheets: Vec<String>,
    available_columns: Vec<String>,
    filter_column: Option<String>,
    filter_value: Option<String>,
    close_values: Vec<String>,
) -> TabularAnswer {
    let reachable = engine::reachable_sheets(inventory, allowed);
    let sheet: Option<&SheetInventory> = if available_columns.is_empty() {
        match reachable.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    } else {
        reachable
            .iter()
            .copied()
            .find(|sheet| column_names(sheet) == available_columns)
    };
    let available_columns = match (available_columns.is_empty(), sheet) {
        (true, Some(sheet)) => column_names(sheet),
        _ => available_columns,
    };
    let (example_column, example_group) = match sheet {
        Some(sheet) => question::example_columns(&sheet.columns),
        None => (None, None),
    };
    // A group example needs a column to total as well.
    let example_group = example_group.filter(|_| example_column.is_some());
    let available_sheets = if available_sheets.is_empty() {
        sheet_names(reachable)
    } else {
        available_sheets
    };
    TabularAnswer::Nudge {
        file,
        reason,
        available_sheets,
        available_columns,
        example_column,
        example_group,
        filter_column,
        filter_value,
        close_values,
    }
}

fn column_names(sheet: &SheetInventory) -> Vec<String> {
    sheet
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect()
}

fn sheet_names(sheets: Vec<&SheetInventory>) -> Vec<String> {
    sheets.into_iter().map(|sheet| sheet.name.clone()).collect()
}

/// The column an already-classified operation names to read as data, if any - `Operation::Count`
/// and `Operation::RowAt` name none. Used only to prefer that column as a numeric-comparison
/// filter's target over an implied single reachable column (`detect_filters`).
fn operation_column_name(operation: &Operation) -> Option<&str> {
    match operation {
        Operation::Sum { column, .. }
        | Operation::Min { column, .. }
        | Operation::Max { column, .. }
        | Operation::Mean { column, .. }
        | Operation::Median { column, .. }
        | Operation::Distinct { column, .. }
        | Operation::Sort { column, .. } => Some(column.as_str()),
        Operation::LargestRow { by_column, .. } => Some(by_column.as_str()),
        Operation::GroupSum { sum_column, .. }
        | Operation::LargestGroup { sum_column, .. }
        | Operation::LeastGroup { sum_column, .. }
        | Operation::TopGroups { sum_column, .. } => Some(sum_column.as_str()),
        Operation::Count { .. }
        | Operation::CountPerGroup { .. }
        | Operation::Filter { .. }
        | Operation::RowAt { .. } => None,
    }
}

/// `operation` with its own `filters` replaced by `filters` - every variant `tabular::question`
/// can classify carries one (empty until `detect_filters` fills it); `Filter` and `RowAt` are
/// never reached here and pass through unchanged.
fn with_filters(operation: Operation, filters: Vec<FilterSpec>) -> Operation {
    match operation {
        Operation::Count { .. } => Operation::Count { filters },
        Operation::Distinct { column, .. } => Operation::Distinct { column, filters },
        Operation::Sum { column, .. } => Operation::Sum { column, filters },
        Operation::Min { column, .. } => Operation::Min { column, filters },
        Operation::Max { column, .. } => Operation::Max { column, filters },
        Operation::Mean { column, .. } => Operation::Mean { column, filters },
        Operation::Median { column, .. } => Operation::Median { column, filters },
        Operation::GroupSum {
            group_by,
            sum_column,
            ..
        } => Operation::GroupSum {
            group_by,
            sum_column,
            filters,
        },
        Operation::LargestGroup {
            group_by,
            sum_column,
            ..
        } => Operation::LargestGroup {
            group_by,
            sum_column,
            filters,
        },
        Operation::LeastGroup {
            group_by,
            sum_column,
            ..
        } => Operation::LeastGroup {
            group_by,
            sum_column,
            filters,
        },
        Operation::TopGroups {
            group_by,
            sum_column,
            n,
            ..
        } => Operation::TopGroups {
            group_by,
            sum_column,
            n,
            filters,
        },
        Operation::CountPerGroup { group_by, .. } => Operation::CountPerGroup { group_by, filters },
        Operation::LargestRow { by_column, .. } => Operation::LargestRow { by_column, filters },
        Operation::Sort {
            column, descending, ..
        } => Operation::Sort {
            column,
            descending,
            filters,
        },
        other @ (Operation::Filter { .. } | Operation::RowAt { .. }) => other,
    }
}

/// What session 11's residual-word reading actually found (`docs/DECISIONS.md`). Replaces
/// session 9's `DetectedFilter`, which only named a word and a column: this builds the real
/// `FilterSpec`s `tabular::engine::execute` runs, or says precisely why it built none.
enum FilterDetection {
    /// One or more filters, combined with AND only - never OR, never nested
    /// (`docs/DECISIONS.md`, session 11's forbidden anti-patterns).
    Filters(Vec<FilterSpec>),
    /// A word named real data in more than one reachable column: asked, never picked.
    WhichColumn { value: String, candidates: Vec<String> },
    /// A word looked like an attempted filter value (capitalised, the way every fictional value
    /// in this product's fixtures is) but matched no reachable column's real data.
    ValueNotFound { value: String, close: Vec<String> },
    /// Nothing in the residual words reads as a filter at all - an ordinary unfiltered question,
    /// exactly as before this session.
    None,
}

/// Turns `residual`'s words into the filters the question's own recognised operation should run
/// under. A filter is recognised only when it is anchored on real data: a value found, whole
/// word, in exactly one reachable column (folded, so an unaccented word in the question still
/// finds an accented real value); a weekday or month-name word, or a four-digit year, resolved
/// against the single reachable `Date`-typed column; or a comparison word beside a number,
/// resolved against `operation_column` when it is numeric, or the single reachable numeric
/// column otherwise. Several recognised words combine with AND. `None` when the operation's own
/// sheet cannot be resolved at all: `engine::execute` will refuse it with the correct reason on
/// its own.
fn detect_filters(
    question: &str,
    residual: &[String],
    fresh: &TabularInventory,
    workbook: &Workbook,
    sheet_name: Option<&str>,
    allowed: Option<&[String]>,
    operation_column: Option<&str>,
    named_column: Option<&str>,
    locale: &str,
) -> FilterDetection {
    if residual.is_empty() {
        return FilterDetection::None;
    }
    let Ok(sheet_inventory) = engine::resolve_sheet_inventory(fresh, sheet_name, allowed) else {
        return FilterDetection::None;
    };
    let Some(sheet_data) = workbook
        .sheets
        .iter()
        .find(|sheet| sheet.name == sheet_inventory.name)
    else {
        return FilterDetection::None;
    };
    let rows = data_rows(sheet_inventory, sheet_data);
    let columns = &sheet_inventory.columns;

    // Folded word -> the distinct full cell values (original text) it is one word of, per
    // column - a `Date` column excluded: its own text is `text_value`'s ISO rendering
    // ("2026-03-02"), whose digit groups would otherwise spuriously "equal" the very day, month
    // or year number a weekday/month/year filter is trying to read below, over every row that
    // happens to share the same year. A multi-word real value (a two-word company name, say) is
    // found by either of its words, but the filter this builds always equals the value's *whole*
    // text, never one word of it - an `Equals` on a lone word would never match the row it was
    // found in (`docs/DECISIONS.md`, session 11's "never match a value by substring").
    let columns_words: Vec<(String, HashMap<String, Vec<String>>)> = columns
        .iter()
        .filter(|column| column.inferred_type != ColumnType::Date)
        .map(|column| {
            let mut values: HashMap<String, Vec<String>> = HashMap::new();
            for row in rows {
                let Some(cell) = row.get(column.index) else {
                    continue;
                };
                let full = text_value(cell);
                let trimmed = full.trim().to_string();
                if trimmed.is_empty() {
                    continue;
                }
                for word in name_words(&full) {
                    let bucket = values.entry(word).or_default();
                    if !bucket.contains(&trimmed) {
                        bucket.push(trimmed.clone());
                    }
                }
            }
            (column.name.clone(), values)
        })
        .collect();
    let all_values: std::collections::BTreeSet<String> = rows
        .iter()
        .flat_map(|row| {
            columns
                .iter()
                .filter(|column| column.inferred_type != ColumnType::Date)
                .filter_map(|column| row.get(column.index))
        })
        .map(text_value)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();

    let date_columns: Vec<&ColumnInventory> = columns
        .iter()
        .filter(|column| column.inferred_type == ColumnType::Date)
        .collect();
    let numeric_columns: Vec<&ColumnInventory> = columns
        .iter()
        .filter(|column| column.inferred_type == ColumnType::Numeric && !column.has_formulas)
        .collect();
    let words = question::filter_words(locale);

    let mut filters: Vec<FilterSpec> = Vec::new();
    let mut consumed = vec![false; residual.len()];

    // --- A comparison word beside a number --------------------------------------------------
    if let Some(words) = &words {
        let target = operation_column
            .and_then(|name| numeric_columns.iter().find(|column| column.name == name))
            .copied()
            .or(match numeric_columns.as_slice() {
                [only] => Some(*only),
                _ => None,
            });
        if let Some(column) = target {
            for index in 0..residual.len() {
                if consumed[index] {
                    continue;
                }
                let folded = fold_text(&residual[index]);
                let greater = words.greater_than.contains(&folded);
                let less = words.less_than.contains(&folded);
                if !greater && !less {
                    continue;
                }
                // The shape every shipped pack's phrasing uses is "comparison word, then
                // number" ("over 1000", "plus de 1000"); a number placed before it is read
                // too, so a translation that inverts the order is not silently unsupported.
                let threshold_index = ((index + 1)..residual.len())
                    .find(|&i| !consumed[i] && residual[i].parse::<f64>().is_ok())
                    .or_else(|| {
                        (0..index)
                            .rev()
                            .find(|&i| !consumed[i] && residual[i].parse::<f64>().is_ok())
                    });
                if let Some(threshold_index) = threshold_index {
                    if let Ok(threshold) = residual[threshold_index].parse::<f64>() {
                        consumed[index] = true;
                        consumed[threshold_index] = true;
                        filters.push(FilterSpec {
                            column: column.name.clone(),
                            comparison: if greater {
                                Comparison::GreaterThan(threshold)
                            } else {
                                Comparison::LessThan(threshold)
                            },
                        });
                    }
                }
            }
        }
    }

    // --- A date range: "between <date> and <date>" ------------------------------------------
    if let Some(words) = &words {
        let has_between = residual
            .iter()
            .enumerate()
            .any(|(index, word)| !consumed[index] && words.between.contains(&fold_text(word)));
        if has_between {
            if let [start, end] = extract_dates(question).as_slice() {
                let (start, end) = if start <= end {
                    (*start, *end)
                } else {
                    (*end, *start)
                };
                if let [only] = date_columns.as_slice() {
                    filters.push(FilterSpec {
                        column: only.name.clone(),
                        comparison: Comparison::DateRange(start, end),
                    });
                    for (index, word) in residual.iter().enumerate() {
                        if words.between.contains(&fold_text(word)) {
                            consumed[index] = true;
                        }
                    }
                }
            }
        }
    }

    // --- Equality, weekday, month, year - one not-yet-consumed word at a time ---------------
    let mut unmatched_value: Option<String> = None;
    for index in 0..residual.len() {
        if consumed[index] {
            continue;
        }
        let word = &residual[index];
        let folded = fold_text(word);

        let matching_columns: Vec<(&str, &Vec<String>)> = columns_words
            .iter()
            .filter_map(|(name, words)| words.get(&folded).map(|values| (name.as_str(), values)))
            .collect();
        // The question may already have named its column ("where fournisseur is Alpha", "for the
        // fournisseur Beta") - `classify_with`'s own `column_name`, threaded through here as
        // `named_column`, regardless of whether the recognised operation reads it. Preferring it
        // over an ambiguous match is not a guess: she already told us which column, in words this
        // classifier separately resolved against the workbook's real headers.
        let resolved = match matching_columns.len() {
            1 => Some(matching_columns[0]),
            n if n > 1 => named_column
                .and_then(|named| matching_columns.iter().find(|(name, _)| *name == named))
                .copied(),
            _ => None,
        };
        match resolved {
            Some((column_name, candidate_values)) => {
                // Several distinct values of this one column share the word: a best-effort
                // fallback to the word itself, rather than refusing outright - rare in practice
                // (`docs/SESSION-DATA-06-Findings.md`'s fixtures never exercise it).
                let value = match candidate_values.as_slice() {
                    [only] => only.clone(),
                    _ => word.clone(),
                };
                let spec = FilterSpec {
                    column: column_name.to_string(),
                    comparison: Comparison::Equals(value),
                };
                if !filters.contains(&spec) {
                    filters.push(spec);
                }
                consumed[index] = true;
                continue;
            }
            None if matching_columns.len() > 1 => {
                return FilterDetection::WhichColumn {
                    value: word.clone(),
                    candidates: matching_columns
                        .into_iter()
                        .map(|(name, _)| name.to_string())
                        .collect(),
                };
            }
            None => {}
        }

        if let (Some(words), [only]) = (&words, date_columns.as_slice()) {
            if let Some((weekday, _)) = words.weekdays.iter().find(|(_, forms)| forms.contains(&folded)) {
                filters.push(FilterSpec {
                    column: only.name.clone(),
                    comparison: Comparison::Weekday(*weekday),
                });
                consumed[index] = true;
                continue;
            }
            if let Some((month, _)) = words.months.iter().find(|(_, forms)| forms.contains(&folded)) {
                filters.push(FilterSpec {
                    column: only.name.clone(),
                    comparison: Comparison::Month(*month),
                });
                consumed[index] = true;
                continue;
            }
        }

        if word.chars().count() == 4 {
            if let (Ok(year @ 1000..=9999), [only]) = (word.parse::<i32>(), date_columns.as_slice()) {
                filters.push(FilterSpec {
                    column: only.name.clone(),
                    comparison: Comparison::Year(year),
                });
                consumed[index] = true;
                continue;
            }
        }

        // Every fictional value in this product's fixtures is a proper noun - a supplier, a
        // client, an agency - and so is typed capitalised; an ordinary residual word left over
        // from the pack's grammar-only filler list ("invoices", "factures") is not
        // (`docs/SESSION-DATA-06-Findings.md`). Never a number: a stray digit that matched
        // nothing is ordinary noise, not a value worth reporting missing. Never the question's
        // own first word either: English and French both capitalise a sentence's opening word
        // regardless of what it is ("Give me a count"), so that capital is not evidence of a
        // proper noun the way every other one in this product's fixtures is.
        if unmatched_value.is_none()
            && word.chars().next().is_some_and(|ch| ch.is_uppercase())
            && Some(word.as_str()) != first_word(question)
        {
            unmatched_value = Some(word.clone());
        }
    }

    if !filters.is_empty() {
        return FilterDetection::Filters(filters);
    }
    if let Some(value) = unmatched_value {
        return FilterDetection::ValueNotFound {
            close: close_values(&value, &all_values),
            value,
        };
    }
    FilterDetection::None
}

/// The question's own first alphanumeric word, exactly as typed - the one word whose leading
/// capital a sentence's grammar explains on its own, never a proper noun the question named.
fn first_word(question: &str) -> Option<&str> {
    question
        .split(|ch: char| !ch.is_alphanumeric())
        .find(|word| !word.is_empty())
}

/// Every whitespace-delimited token of `question` that parses as a calendar date. Read from the
/// question's own text rather than reconstructed from `residual`'s separated digit tokens: the
/// residual-word split breaks `09/03/2026` into three numbers on its own `/`, which a date range
/// cannot be rebuilt from without guessing which triplet paired with which.
fn extract_dates(question: &str) -> Vec<chrono::NaiveDate> {
    question
        .split(|ch: char| ch.is_whitespace())
        .filter_map(|word| {
            let trimmed = word.trim_matches(|ch: char| !ch.is_ascii_digit());
            inventory::parse_date_value(trimmed)
        })
        .collect()
}

/// Up to five of `pool`'s values close to `word` - folded prefix match first, then edit distance
/// at most two - so a `value_not_found` nudge can show what the column actually holds instead of
/// only that nothing matched (`docs/DECISIONS.md`, session 11). Ties keep `pool`'s own (sorted)
/// order.
fn close_values(word: &str, pool: &std::collections::BTreeSet<String>) -> Vec<String> {
    let folded_word = fold_text(word);
    let mut scored: Vec<(usize, &String)> = pool
        .iter()
        .filter_map(|value| {
            let folded_value = fold_text(value);
            if folded_value.starts_with(&folded_word) || folded_word.starts_with(&folded_value) {
                Some((0, value))
            } else {
                let distance = levenshtein(&folded_word, &folded_value);
                (distance <= 2).then_some((distance + 1, value))
            }
        })
        .collect();
    scored.sort_by_key(|(distance, _)| *distance);
    scored
        .into_iter()
        .take(5)
        .map(|(_, value)| value.clone())
        .collect()
}

/// The classic edit-distance table, no crate needed for five words at a time.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, &cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            current.push(
                (previous[j + 1] + 1)
                    .min(current[j] + 1)
                    .min(previous[j] + cost),
            );
        }
        previous = current;
    }
    previous[b.len()]
}

/// This module never imports `crate::gateway`, `crate::retrieval` or `crate::conversation`: every
/// test below runs with no server at all, which is the point.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_scope::ScopeEntry;
    use crate::data_folder::tests::{invoice_csv, write};
    use crate::data_folder::{self};
    use crate::index_store::IndexStore;
    use crate::inventory::FileHashCache;
    use crate::tabular::engine::NumericAggregate;

    struct Folder {
        data: tempfile::TempDir,
        _app: tempfile::TempDir,
        index: IndexStore,
    }

    impl Folder {
        fn with(files: &[(&str, &str)]) -> Self {
            let data = tempfile::tempdir().unwrap();
            let app = tempfile::tempdir().unwrap();
            let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
            for (path, content) in files {
                write(data.path(), path, content.as_bytes());
            }
            data_folder::analyse(data.path(), &mut index, "en-US", &|_| {}).unwrap();
            Self {
                data,
                _app: app,
                index,
            }
        }

        fn open(&self) -> DataFolder {
            DataFolder::discover(self.data.path(), Some(&self.index), &FileHashCache::new())
                .unwrap()
        }

        /// An explicit selection of these files, pinned to what they hold now.
        fn ticked(&self, paths: &[&str]) -> ScopeMode {
            let folder = self.open();
            ScopeMode::Explicit(
                paths
                    .iter()
                    .map(|path| ScopeEntry {
                        relative_path: path.to_string(),
                        pinned_id: folder
                            .files()
                            .find_by_relative_path(path)
                            .unwrap()
                            .id
                            .clone(),
                        added_at: 1,
                        sheet_names: Vec::new(),
                    })
                    .collect(),
            )
        }

        fn ask(&self, question: &str, selection: &ScopeMode) -> Result<TabularAnswer, AppError> {
            answer(question, &self.open(), selection, "en-US")
        }
    }

    fn invoices() -> String {
        invoice_csv(12)
    }

    #[test]
    fn a_table_only_question_is_computed_with_its_sheet_as_the_citation() {
        let folder = Folder::with(&[("factures.csv", &invoices())]);

        let result = folder
            .ask(
                "What is the total montant?",
                &folder.ticked(&["factures.csv"]),
            )
            .unwrap();

        let TabularAnswer::Value {
            file,
            value,
            locator,
            ..
        } = result
        else {
            panic!("expected a computed value, got {result:?}");
        };
        assert_eq!(file, "factures.csv");
        // 1,50 + 2,50 + ... + 12,50
        assert_eq!(
            value,
            TabularValue::Sum(NumericAggregate {
                value: (1..=12).map(|n| n as f64 + 0.5).sum(),
                unit: None,
                unparsed: 0,
            })
        );
        assert_eq!(
            locator.sheet, "factures",
            "a CSV's one sheet is named after the file"
        );
        assert_eq!(locator.column.as_deref(), Some("montant"));
    }

    #[test]
    fn a_structural_question_is_answered_from_the_inventory() {
        let folder = Folder::with(&[("factures.csv", &invoices())]);

        let result = folder
            .ask("How many rows?", &folder.ticked(&["factures.csv"]))
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::Structural {
                file: "factures.csv".into(),
                answer: StructuralAnswer::RowCount {
                    sheet: "factures".into(),
                    rows: 12
                }
            }
        );
    }

    #[test]
    fn an_unclassified_question_gets_the_real_columns_not_a_generic_refusal() {
        let folder = Folder::with(&[("factures.csv", &invoices())]);

        let result = folder
            .ask(
                "What did the specialist recommend?",
                &folder.ticked(&["factures.csv"]),
            )
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::Nudge {
                file: "factures.csv".into(),
                reason: None,
                available_sheets: vec!["factures".into()],
                available_columns: vec!["date".into(), "fournisseur".into(), "montant".into()],
                example_column: Some("montant".into()),
                example_group: Some("fournisseur".into()),
                filter_column: None,
                filter_value: None,
                close_values: Vec::new(),
            }
        );
    }

    #[test]
    fn an_engine_refusal_becomes_a_nudge_carrying_its_reason_and_the_real_columns() {
        let folder = Folder::with(&[("factures.csv", &invoices())]);

        // `fournisseur` is text: a sum over it is NOT_DETERMINISTICALLY_ANSWERABLE.
        let result = folder
            .ask(
                "What is the total fournisseur?",
                &folder.ticked(&["factures.csv"]),
            )
            .unwrap();

        let TabularAnswer::Nudge {
            reason,
            available_columns,
            example_column,
            ..
        } = result
        else {
            panic!("expected a nudge, got {result:?}");
        };
        assert_eq!(reason, Some(NotAnswerableReason::NonNumericColumn));
        assert_eq!(available_columns, vec!["date", "fournisseur", "montant"]);
        assert_eq!(example_column.as_deref(), Some("montant"));
    }

    #[test]
    fn a_red_workbook_is_never_answered_from_even_when_the_selection_holds_it() {
        // The interface offers it no checkbox; this is the second line of defence.
        let folder = Folder::with(&[("notes.csv", "a short note\nwith no table\n")]);

        let result = folder
            .ask("How many rows?", &folder.ticked(&["notes.csv"]))
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::WorkbookUnreadable {
                file: "notes.csv".into()
            }
        );
    }

    #[test]
    fn a_red_workbook_named_in_the_question_is_called_unreadable_not_missing() {
        let folder = Folder::with(&[
            ("factures.csv", &invoices()),
            ("notes.csv", "a short note\nwith no table\n"),
        ]);

        let result = folder
            .ask("How many rows in notes.csv?", &ScopeMode::WholeFolder)
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::WorkbookUnreadable {
                file: "notes.csv".into()
            }
        );
    }

    #[test]
    fn two_selected_workbooks_and_no_name_asks_which_one() {
        let folder = Folder::with(&[("achats.csv", &invoices()), ("ventes.csv", &invoices())]);

        let result = folder
            .ask("What is the total montant?", &ScopeMode::WholeFolder)
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::WhichWorkbook {
                candidates: vec!["achats.csv".into(), "ventes.csv".into()]
            }
        );
    }

    #[test]
    fn two_selected_workbooks_and_one_named_answers_from_that_one() {
        let folder = Folder::with(&[("achats.csv", &invoices()), ("ventes.csv", &invoice_csv(9))]);

        let result = folder
            .ask("How many rows in ventes?", &ScopeMode::WholeFolder)
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::Structural {
                file: "ventes.csv".into(),
                answer: StructuralAnswer::RowCount {
                    sheet: "ventes".into(),
                    rows: 9
                }
            }
        );
    }

    #[test]
    fn a_workbook_outside_the_selection_is_named_as_not_selected() {
        let folder = Folder::with(&[("achats.csv", &invoices()), ("ventes.csv", &invoices())]);

        let result = folder
            .ask(
                "How many rows in ventes.csv?",
                &folder.ticked(&["achats.csv"]),
            )
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::FileNotSelected {
                query: "ventes.csv".into()
            }
        );
    }

    #[test]
    fn every_workbook_red_under_tous_says_there_is_no_usable_table() {
        let folder = Folder::with(&[("notes.csv", "a short note\n")]);

        let result = folder
            .ask("How many rows?", &ScopeMode::WholeFolder)
            .unwrap();

        assert_eq!(result, TabularAnswer::NoUsableTable);
    }

    #[test]
    fn a_ticked_workbook_that_changed_since_is_refused_like_a_changed_document() {
        let folder = Folder::with(&[("factures.csv", &invoices())]);
        let selection = folder.ticked(&["factures.csv"]);
        write(
            folder.data.path(),
            "factures.csv",
            invoice_csv(13).as_bytes(),
        );

        let result = folder.ask("How many rows?", &selection);

        assert!(matches!(result, Err(AppError::ScopeUnavailable)));
    }

    #[test]
    fn a_ticked_workbook_whose_analysis_was_reset_asks_for_an_analysis() {
        let mut folder = Folder::with(&[("factures.csv", &invoices())]);
        let selection = folder.ticked(&["factures.csv"]);
        folder.index.clear_tabular_inventories().unwrap();

        let result = folder.ask("How many rows?", &selection).unwrap();

        assert_eq!(
            result,
            TabularAnswer::WorkbookNotAnalysed {
                file: "factures.csv".into()
            }
        );
    }

    // --- Group questions, on the layout of a real public-revenue export ----------------------

    /// `revenue_sub_agency.csv`'s columns, with made-up rows: two text columns to group by, a
    /// year, the amount, and a numeric code that must never be mistaken for the amount.
    fn revenue() -> String {
        let mut csv = String::from(
            "agency,sub_agency,calendar_year,amount,sub_type_name,rev_type,sub_type\n",
        );
        let rows = [
            ("Parks", "Parks North", 2019, "1200.50", 3),
            ("Parks", "Parks South", 2020, "800", 3),
            ("Transit", "Transit Bus", 2019, "5000", 7),
            ("Transit", "Transit Rail", 2020, "2500.25", 7),
            ("Health", "Health Clinics", 2019, "300", 1),
            ("Health", "Health Labs", 2020, "450", 1),
            ("Water", "Water Supply", 2019, "900", 5),
            ("Water", "Water Waste", 2020, "100", 5),
            ("Transit", "Transit Bus", 2021, "1000", 7),
        ];
        for (agency, sub, year, amount, code) in rows {
            csv.push_str(&format!(
                "{agency},{sub},{year},{amount},Fees,Other,{code}\n"
            ));
        }
        csv
    }

    fn ranking(result: &TabularAnswer) -> &crate::tabular::engine::GroupRanking {
        match result {
            TabularAnswer::Value {
                value: TabularValue::LargestGroup(ranking),
                ..
            } => ranking,
            other => panic!("expected a group ranking, got {other:?}"),
        }
    }

    #[test]
    fn which_agency_costs_the_most_is_answered_with_the_total_per_agency() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask(
                "Which agency costs the most? And how much?",
                &folder.ticked(&["revenue_sub_agency.csv"]),
            )
            .unwrap();

        let top = ranking(&result);
        assert_eq!(top.group_column, "agency");
        assert_eq!(top.group_count, 4);
        assert_eq!(top.top[0].group, "Transit");
        assert!((top.top[0].sum - 8500.25).abs() < 1e-9);
        let TabularAnswer::Value { locator, .. } = &result else {
            unreachable!()
        };
        assert_eq!(
            locator.column.as_deref(),
            Some("amount"),
            "the amount, never the year or the code"
        );
    }

    #[test]
    fn please_list_agencies_lists_the_agencies() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Please list agencies", &folder.ticked(&["revenue_sub_agency.csv"]))
            .unwrap();

        let TabularAnswer::Value {
            value: TabularValue::Distinct(values),
            locator,
            ..
        } = result
        else {
            panic!("expected the agencies, got {result:?}");
        };
        assert_eq!(values, vec!["Health", "Parks", "Transit", "Water"]);
        assert_eq!(locator.column.as_deref(), Some("agency"));
    }

    #[test]
    fn a_plural_two_word_column_is_listed_by_its_own_values() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Show the sub agencies", &ScopeMode::WholeFolder)
            .unwrap();

        let TabularAnswer::Value { locator, .. } = &result else {
            panic!("expected a list, got {result:?}");
        };
        assert_eq!(locator.column.as_deref(), Some("sub_agency"));
    }

    #[test]
    fn a_plural_group_is_still_ranked() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Which agencies cost the most?", &ScopeMode::WholeFolder)
            .unwrap();

        assert_eq!(ranking(&result).group_column, "agency");
    }

    #[test]
    fn the_same_question_typed_in_english_on_a_french_interface_is_still_read() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = answer(
            "Which agency costs the most?",
            &folder.open(),
            &ScopeMode::WholeFolder,
            "fr-FR",
        )
        .unwrap();

        assert_eq!(ranking(&result).top[0].group, "Transit");
    }

    #[test]
    fn a_two_word_column_is_named_by_both_words_not_by_its_last_one() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Which sub agency has the highest amount?", &ScopeMode::WholeFolder)
            .unwrap();

        let top = ranking(&result);
        assert_eq!(top.group_column, "sub_agency");
        assert_eq!(top.top[0].group, "Transit Bus");
    }

    #[test]
    fn a_year_column_can_be_grouped_by_when_the_question_names_the_year() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Which year had the highest amount?", &ScopeMode::WholeFolder)
            .unwrap();

        let top = ranking(&result);
        assert_eq!(top.group_column, "calendar_year");
        assert_eq!(top.top[0].group, "2019");
    }

    #[test]
    fn a_total_per_group_lists_every_group() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("What is the total amount per agency?", &ScopeMode::WholeFolder)
            .unwrap();

        let TabularAnswer::Value {
            value: TabularValue::GroupSums(sums),
            ..
        } = result
        else {
            panic!("expected every group's total, got {result:?}");
        };
        let groups: Vec<&str> = sums.iter().map(|group| group.group.as_str()).collect();
        assert_eq!(groups, vec!["Health", "Parks", "Transit", "Water"]);
    }

    #[test]
    fn two_columns_that_could_both_be_the_total_are_asked_about_not_picked() {
        let mut csv = String::from("agency,amount,fee\n");
        for index in 0..9 {
            csv.push_str(&format!("A{},{}0,{}\n", index % 3, index + 1, index + 2));
        }
        let folder = Folder::with(&[("costs.csv", &csv)]);

        let result = folder
            .ask("Which agency costs the most?", &ScopeMode::WholeFolder)
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::WhichMeasure {
                file: "costs.csv".into(),
                group_column: "agency".into(),
                candidates: vec!["amount".into(), "fee".into()],
            }
        );
    }

    #[test]
    fn naming_the_column_to_total_settles_it() {
        let mut csv = String::from("agency,amount,fee\n");
        for index in 0..9 {
            csv.push_str(&format!("A{},{}0,{}\n", index % 3, index + 1, index + 2));
        }
        let folder = Folder::with(&[("costs.csv", &csv)]);

        let result = folder
            .ask("Which agency has the highest fee?", &ScopeMode::WholeFolder)
            .unwrap();

        let TabularAnswer::Value { locator, .. } = &result else {
            panic!("expected a value, got {result:?}");
        };
        assert_eq!(locator.column.as_deref(), Some("fee"));
    }

    #[test]
    fn the_nudge_suggests_the_amount_never_the_year() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Tell me something interesting", &ScopeMode::WholeFolder)
            .unwrap();

        let TabularAnswer::Nudge {
            example_column,
            example_group,
            ..
        } = result
        else {
            panic!("expected a nudge, got {result:?}");
        };
        assert_eq!(example_column.as_deref(), Some("amount"));
        assert_eq!(example_group.as_deref(), Some("agency"));
    }

    /// The nudge's examples are sentences in the interface catalogues. Each one, filled with the
    /// columns the nudge would give, must be a question this engine answers - an example that
    /// earns a second nudge would be worse than none.
    #[test]
    fn every_example_the_nudge_offers_is_a_question_the_engine_answers() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);
        let catalogues = [
            ("en-US", include_str!("../../src/locales/en-US.json")),
            ("fr-FR", include_str!("../../src/locales/fr-FR.json")),
        ];
        for (locale, body) in catalogues {
            let catalogue: serde_json::Value = serde_json::from_str(body).unwrap();
            for key in ["nudgeExample", "nudgeExampleGroup"] {
                let template = catalogue["tabularAnswer"][key]
                    .as_str()
                    .unwrap_or_else(|| panic!("{locale} has no tabularAnswer.{key}"));
                let question = quoted(template)
                    .replace("{column}", "amount")
                    .replace("{group}", "agency");

                let result = answer(&question, &folder.open(), &ScopeMode::WholeFolder, locale)
                    .unwrap();

                assert!(
                    matches!(result, TabularAnswer::Value { .. }),
                    "{locale} {key}: {question:?} gave {result:?}"
                );
            }
        }
    }

    /// The part of an example sentence between its quotation marks, whichever the language uses.
    fn quoted(template: &str) -> String {
        let open = template
            .find(['\u{201c}', '\u{ab}'])
            .expect("an example is quoted");
        let rest = &template[open..];
        let body: String = rest.chars().skip(1).collect();
        let close = body
            .find(['\u{201d}', '\u{bb}'])
            .expect("an example is closed");
        body[..close].to_string()
    }

    #[test]
    fn every_computed_value_crosses_the_bridge() {
        // The interface reads these; a value that cannot be serialised would fail the whole
        // question at the last step, after the engine had already answered it.
        let row = crate::tabular::engine::RowValue {
            row_index: 0,
            cells: vec![crate::tabular::engine::CellText {
                column: "montant".into(),
                text: "10".into(),
            }],
        };
        for value in [
            TabularValue::Count(3),
            TabularValue::Sum(NumericAggregate {
                value: 1.5,
                unit: None,
                unparsed: 0,
            }),
            TabularValue::Min(NumericAggregate {
                value: 1.0,
                unit: None,
                unparsed: 0,
            }),
            TabularValue::Max(NumericAggregate {
                value: 2.0,
                unit: Some("\u{20ac}".into()),
                unparsed: 3,
            }),
            TabularValue::Distinct(vec!["a".into()]),
            TabularValue::GroupSums(vec![crate::tabular::engine::GroupSum {
                group: "a".into(),
                sum: 1.0,
            }]),
            TabularValue::LargestRow(row.clone()),
            TabularValue::Row(row.clone()),
            TabularValue::Rows(vec![row]),
        ] {
            let answer = TabularAnswer::Value {
                file: "f.csv".into(),
                value: value.clone(),
                locator: TabularLocator {
                    sheet: "Facturation".into(),
                    header_row: Some(0),
                    column: Some("montant".into()),
                    row_range: Some((0, 0)),
                    filters: Vec::new(),
                },
                derivation: TabularDerivation::Computed {
                    operation: "sum".into(),
                    row_count: 1,
                },
            };
            let json = serde_json::to_value(&answer)
                .unwrap_or_else(|error| panic!("{value:?} did not serialise: {error}"));
            assert_eq!(json["kind"], "value");
            assert_eq!(json["locator"]["sheet"], "Facturation");
        }
        let json = serde_json::to_value(TabularValue::Sum(NumericAggregate {
            value: 1.5,
            unit: None,
            unparsed: 0,
        }))
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "kind": "sum",
                "value": { "value": 1.5, "unit": null, "unparsed": 0 }
            })
        );
    }

    #[test]
    fn the_wire_form_is_tagged_with_camel_case_fields() {
        let json = serde_json::to_value(TabularAnswer::Nudge {
            file: "f.csv".into(),
            reason: Some(NotAnswerableReason::ColumnNotFound),
            available_sheets: vec!["f".into()],
            available_columns: vec!["a".into()],
            example_column: None,
            example_group: None,
            filter_column: None,
            filter_value: None,
            close_values: Vec::new(),
        })
        .unwrap();

        assert_eq!(json["kind"], "nudge");
        assert_eq!(json["reason"], "column_not_found");
        assert_eq!(json["availableColumns"], serde_json::json!(["a"]));
        assert_eq!(json["exampleColumn"], serde_json::Value::Null);
    }

    // --- Gap A: a residual word that names real data is a filter this engine cannot apply -----

    /// `date;fournisseur;montant`, 9 rows, suppliers Alpha/Beta/Gamma cycling, `montant` rising
    /// by 10 each row (`10,00` to `90,00`). Beta's rows (`i` = 1, 4, 7): 20 + 50 + 80 = 150.
    fn invoices_with_suppliers() -> String {
        let suppliers = ["Alpha", "Beta", "Gamma"];
        let mut csv = String::from("date;fournisseur;montant\n");
        for i in 0..9 {
            csv.push_str(&format!(
                "{:02}/01/2026;{};{},00\n",
                i + 1,
                suppliers[i % 3],
                (i + 1) * 10
            ));
        }
        csv
    }

    #[test]
    fn a_filter_clause_now_returns_the_filtered_count() {
        let folder = Folder::with(&[("factures.csv", &invoices_with_suppliers())]);

        let result = folder
            .ask(
                "How many invoices where fournisseur is Alpha?",
                &folder.ticked(&["factures.csv"]),
            )
            .unwrap();

        let TabularAnswer::Value {
            value: TabularValue::Count(count),
            locator,
            ..
        } = result
        else {
            panic!("expected the filtered count, got {result:?}");
        };
        // Alpha: i = 0, 3, 6.
        assert_eq!(count, 3);
        assert_eq!(
            locator.filters,
            vec![crate::tabular::engine::AppliedFilter::Equals {
                column: "fournisseur".into(),
                value: "Alpha".into(),
            }]
        );
    }

    #[test]
    fn a_filter_clause_now_returns_the_filtered_sum() {
        let folder = Folder::with(&[("factures.csv", &invoices_with_suppliers())]);

        let result = folder
            .ask("Sum of montant for Beta", &folder.ticked(&["factures.csv"]))
            .unwrap();

        let TabularAnswer::Value {
            value: TabularValue::Sum(total),
            ..
        } = result
        else {
            panic!("expected the filtered sum, got {result:?}");
        };
        // Beta: i = 1, 4, 7 -> 20 + 50 + 80.
        assert!((total.value - 150.0).abs() < 1e-9);
    }

    #[test]
    fn a_year_number_in_the_question_is_computed_as_a_filter() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask(
                "What is the total amount in 2021?",
                &folder.ticked(&["revenue_sub_agency.csv"]),
            )
            .unwrap();

        let TabularAnswer::Value {
            value: TabularValue::Sum(total),
            ..
        } = result
        else {
            panic!("expected the filtered sum, got {result:?}");
        };
        // Only the 2021 Transit Bus row: 1000.
        assert!((total.value - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn two_filters_combine_with_and() {
        let mut csv = String::from("fournisseur,annee,montant\n");
        for (supplier, year) in [
            ("Alpha", 2024),
            ("Alpha", 2025),
            ("Beta", 2025),
            ("Alpha", 2025),
            ("Gamma", 2024),
            ("Beta", 2024),
            ("Gamma", 2025),
            ("Beta", 2025),
        ] {
            csv.push_str(&format!("{supplier},{year},10\n"));
        }
        let folder = Folder::with(&[("achats.csv", &csv)]);

        let result = folder
            .ask(
                "How many for Alpha in 2025?",
                &folder.ticked(&["achats.csv"]),
            )
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::Value {
                file: "achats.csv".into(),
                value: TabularValue::Count(2),
                locator: TabularLocator {
                    sheet: "achats".into(),
                    header_row: Some(0),
                    column: None,
                    row_range: Some((1, 3)),
                    filters: vec![
                        crate::tabular::engine::AppliedFilter::Equals {
                            column: "fournisseur".into(),
                            value: "Alpha".into(),
                        },
                        crate::tabular::engine::AppliedFilter::Equals {
                            column: "annee".into(),
                            value: "2025".into(),
                        },
                    ],
                },
                derivation: TabularDerivation::Computed {
                    operation: "count".into(),
                    row_count: 2,
                },
            }
        );
    }

    #[test]
    fn a_row_count_question_is_unaffected_by_the_filter_check() {
        let folder = Folder::with(&[("factures.csv", &invoices_with_suppliers())]);

        let result = folder
            .ask("How many rows?", &folder.ticked(&["factures.csv"]))
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::Structural {
                file: "factures.csv".into(),
                answer: StructuralAnswer::RowCount {
                    sheet: "factures".into(),
                    rows: 9
                }
            }
        );
    }

    #[test]
    fn a_value_absent_from_every_column_is_a_nudge_never_a_silent_zero() {
        let folder = Folder::with(&[("factures.csv", &invoices_with_suppliers())]);

        let result = folder
            .ask(
                "What is the total montant for Omega?",
                &folder.ticked(&["factures.csv"]),
            )
            .unwrap();

        let TabularAnswer::Nudge {
            reason,
            filter_value,
            ..
        } = result
        else {
            panic!("expected a value_not_found nudge, never a value: got {result:?}");
        };
        assert_eq!(reason, Some(NotAnswerableReason::ValueNotFound));
        assert_eq!(filter_value.as_deref(), Some("Omega"));
    }

    #[test]
    fn an_accented_multi_word_value_is_matched_by_either_of_its_unaccented_words() {
        let suppliers = ["Alpha", "Soci\u{e9}t\u{e9} G\u{e9}n\u{e9}rale", "Gamma"];
        let mut csv = String::from("date;fournisseur;montant\n");
        for i in 0..9 {
            csv.push_str(&format!(
                "{:02}/01/2026;{};{},00\n",
                i + 1,
                suppliers[i % 3],
                (i + 1) * 10
            ));
        }
        let folder = Folder::with(&[("factures.csv", &csv)]);

        let result = folder
            .ask(
                "What is the total montant for Societe Generale?",
                &folder.ticked(&["factures.csv"]),
            )
            .unwrap();

        let TabularAnswer::Value {
            value: TabularValue::Sum(total),
            ..
        } = result
        else {
            panic!("expected the filtered sum, got {result:?}");
        };
        // Societe Generale: i = 1, 4, 7 -> 20 + 50 + 80. Both "Societe" and "Generale" name the
        // same whole value, never a lone-word filter that could not match the full cell.
        assert!((total.value - 150.0).abs() < 1e-9);
    }

    #[test]
    fn a_comparison_word_beside_a_number_filters_the_single_numeric_column() {
        let folder = Folder::with(&[("factures.csv", &invoices_with_suppliers())]);

        let result = folder
            .ask(
                "How many invoices with montant over 50?",
                &folder.ticked(&["factures.csv"]),
            )
            .unwrap();

        let TabularAnswer::Value {
            value: TabularValue::Count(count),
            ..
        } = result
        else {
            panic!("expected the filtered count, got {result:?}");
        };
        // montant: 10, 20, ..., 90 - strictly greater than 50: 60, 70, 80, 90 = 4 rows.
        assert_eq!(count, 4);
    }

    // --- Session 12's D5: mean, median, least group, top N groups, count per group -----------

    fn least_ranking(result: &TabularAnswer) -> &crate::tabular::engine::GroupRanking {
        match result {
            TabularAnswer::Value {
                value: TabularValue::LeastGroup(ranking),
                ..
            } => ranking,
            other => panic!("expected a least-group ranking, got {other:?}"),
        }
    }

    fn top_groups(result: &TabularAnswer) -> &crate::tabular::engine::TopGroups {
        match result {
            TabularAnswer::Value {
                value: TabularValue::TopGroups(top),
                ..
            } => top,
            other => panic!("expected a top-groups ranking, got {other:?}"),
        }
    }

    fn counts_per_group(result: &TabularAnswer) -> &[crate::tabular::engine::GroupCount] {
        match result {
            TabularAnswer::Value {
                value: TabularValue::CountPerGroup(counts),
                ..
            } => counts,
            other => panic!("expected counts per group, got {other:?}"),
        }
    }

    #[test]
    fn which_agency_costs_the_least_is_answered_with_the_total_per_agency() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask(
                "Which agency costs the least?",
                &folder.ticked(&["revenue_sub_agency.csv"]),
            )
            .unwrap();

        let least = least_ranking(&result);
        assert_eq!(least.group_column, "agency");
        assert_eq!(least.group_count, 4);
        assert_eq!(least.top[0].group, "Health");
        assert!((least.top[0].sum - 750.0).abs() < 1e-9);
        let TabularAnswer::Value { locator, .. } = &result else {
            unreachable!()
        };
        assert_eq!(locator.column.as_deref(), Some("amount"));
    }

    #[test]
    fn top_n_agencies_by_amount_is_ranked_largest_first() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Top 2 agency by amount", &ScopeMode::WholeFolder)
            .unwrap();

        let top = top_groups(&result);
        assert_eq!(top.requested, 2);
        assert!(!top.capped);
        assert_eq!(top.group_count, 4);
        assert_eq!(top.top.len(), 2);
        assert_eq!(top.top[0].group, "Transit");
        assert_eq!(top.top[1].group, "Parks");
    }

    #[test]
    fn how_many_rows_per_agency_counts_every_group_largest_first() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("How many rows per agency?", &ScopeMode::WholeFolder)
            .unwrap();

        let counts = counts_per_group(&result);
        // Transit has 3 rows; Health, Parks and Water have 2 each - a tie, kept alphabetical.
        assert_eq!(counts[0].group, "Transit");
        assert_eq!(counts[0].count, 3);
        let tied: Vec<&str> = counts[1..].iter().map(|c| c.group.as_str()).collect();
        assert_eq!(tied, vec!["Health", "Parks", "Water"]);
        assert!(counts[1..].iter().all(|c| c.count == 2));
    }

    #[test]
    fn the_average_and_median_amount_are_recognised() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let mean = folder
            .ask("What is the average amount?", &ScopeMode::WholeFolder)
            .unwrap();
        let TabularAnswer::Value {
            value: TabularValue::Mean(v),
            ..
        } = mean
        else {
            panic!("expected a mean, got {mean:?}");
        };
        assert!((v.value - 12250.75 / 9.0).abs() < 1e-6);

        let median = folder
            .ask("What is the median amount?", &ScopeMode::WholeFolder)
            .unwrap();
        let TabularAnswer::Value {
            value: TabularValue::Median(v),
            ..
        } = median
        else {
            panic!("expected a median, got {median:?}");
        };
        // Sorted: 100, 300, 450, 800, 900, 1000, 1200.50, 2500.25, 5000 - the middle of 9 is 900.
        assert!((v.value - 900.0).abs() < 1e-9);
    }

    #[test]
    fn least_group_top_groups_and_count_per_group_each_take_a_session_11_filter() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let least = folder
            .ask("Which agency has the least amount in 2020?", &ScopeMode::WholeFolder)
            .unwrap();
        let ranking = least_ranking(&least);
        // 2020: Parks 800, Transit 2 500.25, Health 450, Water 100 - Water is least.
        assert_eq!(ranking.top[0].group, "Water");
        assert!((ranking.top[0].sum - 100.0).abs() < 1e-9);

        let top = folder
            .ask("Top 2 agency by amount in 2019", &ScopeMode::WholeFolder)
            .unwrap();
        let top = top_groups(&top);
        // 2019: Parks 1 200.50, Transit 5 000, Health 300, Water 900.
        assert_eq!(top.top[0].group, "Transit");
        assert_eq!(top.top[1].group, "Parks");

        let per_group = folder
            .ask("How many rows per agency in 2020?", &ScopeMode::WholeFolder)
            .unwrap();
        let counts = counts_per_group(&per_group);
        // Every agency has exactly one 2020 row: a four-way tie, kept alphabetical.
        assert_eq!(counts.len(), 4);
        assert!(counts.iter().all(|c| c.count == 1));
        assert_eq!(
            counts.iter().map(|c| c.group.as_str()).collect::<Vec<_>>(),
            vec!["Health", "Parks", "Transit", "Water"]
        );
    }

    #[test]
    fn a_top_groups_request_beyond_fifty_is_capped_in_the_answer() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Top 80 agency by amount", &ScopeMode::WholeFolder)
            .unwrap();

        let top = top_groups(&result);
        assert_eq!(top.requested, 80);
        assert!(top.capped);
        // Only 4 agencies exist, so the cap never bites against the available data - `requested`
        // alone says what was actually asked for.
        assert_eq!(top.top.len(), 4);
    }
}
