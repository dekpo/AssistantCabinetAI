//! The deterministic tabular engine: `count`, `distinct`, `sum`, `min`, `max`, group sum, the
//! largest single row, `filter`, `sort` - the committed set from `docs/BRIEF-SPRINT-2.5-OCR.md`'s
//! sibling brief for Sprint 2b, and nothing else. Every operation here makes zero model or
//! gateway calls: it reads `Workbook` cells and `TabularInventory` structure, computes, and
//! returns a `TabularOutcome` carrying its own provenance. When it cannot establish an answer
//! deterministically it returns `NOT_DETERMINISTICALLY_ANSWERABLE` with what it does know - the
//! available sheets and columns - never a guess (`docs/ARCHITECTURE.md`, "deterministic before
//! generative").
//!
//! `TabularOutcome`'s `locator` and `derivation` are the tabular form of the repository's
//! `Source` model: `locator` is `Tabular { sheet, header_row, column, row_range }`, and
//! `derivation` keeps `Computed` apart from `FormulaStored` exactly as `docs/ARCHITECTURE.md`
//! requires. A cached formula value is never folded into a `Computed` aggregate: any operation
//! that would have to read a formula column's values refuses instead
//! (`docs/DECISIONS.md`, "a formula's cached result").
//!
//! A sheet restriction (`AnalysisScope::sheet_restrictions`) is enforced by never letting a sheet
//! outside it become reachable in the first place, so a question naming a sheet that exists in
//! the workbook but not in the scope is indistinguishable from one naming a sheet that does not
//! exist at all - the engine cannot be made to say which case it is, which is what keeps the
//! restriction a real boundary rather than a hint.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::inventory::{
    self, data_rows, text_value, ColumnInventory, ColumnType, SheetInventory, TabularInventory,
};
use super::{CellValue, Workbook};
use crate::file_reference::fold_text;

/// The tabular form of `Source.locator` (`docs/ARCHITECTURE.md`): `Tabular { sheet, header_row,
/// column, row_range }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabularLocator {
    pub sheet: String,
    pub header_row: Option<usize>,
    /// The column this value was read from. `None` for a plain row count and for a lookup that
    /// spans every column (a whole row).
    pub column: Option<String>,
    /// 0-indexed, inclusive, into the sheet's data rows below the header. The span the operation
    /// actually read from - the whole data body for an unfiltered aggregate, the min/max of the
    /// matched rows for a filter, one row for a single-row lookup. `None` when nothing matched.
    pub row_range: Option<(usize, usize)>,
}

/// The tabular form of `Source.derivation` (`docs/ARCHITECTURE.md`). Never `ModelAsserted`:
/// nothing in this module ever asks a model for a fact. A model only ever sees what this engine
/// already computed, as evidence (`tabular::escalation`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TabularDerivation {
    /// Calculated here, over a full pass of the matched rows - never a sample.
    Computed { operation: String, row_count: usize },
}

/// One cell of a row result, by column name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellText {
    pub column: String,
    pub text: String,
}

/// One whole row, exactly as read - used for a single-row lookup, the largest row, and every row
/// a filter or a sort returns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowValue {
    /// 0-indexed into the sheet's data rows below the header.
    pub row_index: usize,
    pub cells: Vec<CellText>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupSum {
    pub group: String,
    pub sum: f64,
}

/// The groups with the largest totals, largest first: "which agency costs the most". A **total
/// per group**, never the largest single row - the two are different facts (`docs/RETRIEVAL.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRanking {
    /// The column the rows were grouped by. The summed column is the locator's `column`.
    pub group_column: String,
    /// At most `TOP_GROUPS`, largest total first; ties keep alphabetical order. Two leaders with
    /// the same total are both here, and the interface says it is a tie rather than pick one.
    pub top: Vec<GroupSum>,
    /// How many distinct groups there were in all.
    pub group_count: usize,
}

/// How many leading groups a ranking reports: the answer and enough context to read it by.
pub const TOP_GROUPS: usize = 5;

/// A numeric aggregate together with what its column carries beside the number itself: the unit
/// read from its cells, when every cell that had one agreed (`\u{20ac}`, `$`, `%`), and how many
/// non-empty cells could not be read as a number - never folded into `value`, and always shown
/// beside it (`docs/DECISIONS.md`, D2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericAggregate {
    pub value: f64,
    pub unit: Option<String>,
    pub unparsed: usize,
}

/// What one deterministic operation actually produced. A group **total** and the largest
/// **single row** are different facts and are always labelled as such (`docs/RETRIEVAL.md`), so
/// they are different variants rather than one shape read two ways.
///
/// Adjacently tagged (`{"kind": "sum", "value": 12.5}`): an internally tagged enum cannot carry a
/// bare number or list, and serialising one failed at runtime until the interface first read it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TabularValue {
    Count(usize),
    Sum(NumericAggregate),
    Min(NumericAggregate),
    Max(NumericAggregate),
    /// Sorted, so the same column always reports its distinct values in the same order.
    Distinct(Vec<String>),
    GroupSums(Vec<GroupSum>),
    LargestGroup(GroupRanking),
    LargestRow(RowValue),
    /// A single row looked up directly, by position - not a maximum.
    Row(RowValue),
    /// Every row a filter or a sort produced. Never sent to the model as evidence
    /// (`tabular::escalation`): a raw row list is exactly the "complete workbook, just in case"
    /// this pipeline exists to avoid handing over.
    Rows(Vec<RowValue>),
}

/// Why the engine could not establish an answer deterministically. Every reason still returns
/// what **is** known - the reachable sheets and, once a sheet is resolved, its columns - rather
/// than nothing at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotAnswerableReason {
    /// No sheet reachable under the current scope carries this name - whether because the
    /// workbook truly has no such sheet, or because it does but the scope excludes it. The two
    /// are indistinguishable on purpose.
    SheetNotFound,
    /// Several reachable sheets fold to the same name (case, accents), or none was named and more
    /// than one sheet is reachable. Never picked between silently.
    AmbiguousSheetName,
    ColumnNotFound,
    AmbiguousColumnName,
    RowOutOfRange,
    /// The operation needs numeric values and this column's inventory says it is not uniformly
    /// numeric.
    NonNumericColumn,
    /// The operation would have to read this column's values, and at least one cell in it is a
    /// formula. A formula's cached value is never folded into a `Computed` result
    /// (`docs/DECISIONS.md`).
    FormulaCannotBeVerified,
    /// The sheet, or the column within it, has no data rows to compute over.
    EmptySheet,
    /// The question named a filter this classifier does not apply (`tabular::question`'s
    /// residual words matching real cell data): never the unfiltered value, whatever the
    /// recognised operation was (`docs/DECISIONS.md`, session 7's D1). Not constructed by this
    /// module - `execute` never sees a filter it cannot already resolve - but carried here
    /// because it is the same "why the engine did not compute a value" vocabulary every other
    /// nudge reason belongs to.
    FilterNotSupported,
}

/// The result of one deterministic question against one workbook.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TabularOutcome {
    Value {
        value: TabularValue,
        locator: TabularLocator,
        derivation: TabularDerivation,
    },
    /// The repository's `NOT_DETERMINISTICALLY_ANSWERABLE` (`docs/ARCHITECTURE.md`): what is
    /// known instead of a guess.
    NotDeterministicallyAnswerable {
        reason: NotAnswerableReason,
        /// The sheets reachable under the current scope, whether or not one was named.
        available_sheets: Vec<String>,
        /// This sheet's columns, once one was resolved; empty when sheet resolution itself
        /// failed, since no sheet's columns can honestly be offered then.
        available_columns: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Comparison {
    Equals(String),
    Contains(String),
    GreaterThan(f64),
    LessThan(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilterSpec {
    pub column: String,
    pub comparison: Comparison,
}

/// The committed deterministic operations, and nothing else (`docs/DECISIONS.md`, "XLSX scope
/// for the first implementation" and its siblings): no arbitrary expression language, no formula
/// evaluation, no operation this session did not name.
#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    Count { filter: Option<FilterSpec> },
    Distinct { column: String },
    Sum { column: String },
    Min { column: String },
    Max { column: String },
    GroupSum { group_by: String, sum_column: String },
    /// The group whose total is largest, with the runners-up.
    LargestGroup { group_by: String, sum_column: String },
    LargestRow { by_column: String },
    Filter { filter: FilterSpec },
    Sort { column: String, descending: bool },
    /// A direct row lookup by position - not one of the committed analysis operations, but the
    /// same "row outside the workbook's range" refusal every other row-touching operation needs,
    /// exercised on its own.
    RowAt { index: usize },
}

/// Run one operation against one sheet of `workbook`, restricted to `allowed_sheets` when the
/// conversation's scope narrowed it (`AnalysisScope::sheet_restrictions`). `sheet_name` is the
/// sheet the question named, if it named one; with none named, the operation runs against the
/// single reachable sheet, or refuses as ambiguous when there is more than one.
///
/// `locale` is read only where a numeric column's own cells leave a lone separator's role
/// unsettled (`inventory::resolve_numeric_column`) - the same function, and so the same result,
/// the inventory already used to type the column in the first place.
pub fn execute(
    workbook: &Workbook,
    inventory: &TabularInventory,
    sheet_name: Option<&str>,
    allowed_sheets: Option<&[String]>,
    operation: &Operation,
    locale: &str,
) -> TabularOutcome {
    let reachable_names: Vec<String> = reachable_sheets(inventory, allowed_sheets)
        .iter()
        .map(|sheet| sheet.name.clone())
        .collect();

    let (sheet_inventory, sheet_data) =
        match resolve_sheet(inventory, workbook, sheet_name, allowed_sheets) {
            Ok(pair) => pair,
            Err(reason) => return not_answerable(reason, reachable_names, Vec::new()),
        };

    let columns = &sheet_inventory.columns;
    let column_names: Vec<String> = columns.iter().map(|column| column.name.clone()).collect();
    let rows = data_rows(sheet_inventory, sheet_data);

    let fail = |reason: NotAnswerableReason| -> TabularOutcome {
        not_answerable(reason, reachable_names.clone(), column_names.clone())
    };

    match operation {
        Operation::Count { filter } => {
            let matched = match filter {
                None => (0..rows.len()).collect::<Vec<_>>(),
                Some(spec) => match resolve_filter_column(columns, spec) {
                    Ok(col) => matching_rows(rows, col, &spec.comparison, locale),
                    Err(reason) => return fail(reason),
                },
            };
            TabularOutcome::Value {
                value: TabularValue::Count(matched.len()),
                locator: locator(sheet_inventory, None, range_of(&matched)),
                derivation: computed("count", matched.len()),
            }
        }
        Operation::Distinct { column } => {
            let col = match resolve_value_column(columns, column) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            if rows.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let values: BTreeSet<String> = rows
                .iter()
                .filter_map(|row| row.get(col.index))
                .map(text_value)
                .filter(|text| !text.trim().is_empty())
                .collect();
            TabularOutcome::Value {
                value: TabularValue::Distinct(values.into_iter().collect()),
                locator: locator(sheet_inventory, Some(&col.name), Some((0, rows.len() - 1))),
                derivation: computed("distinct", rows.len()),
            }
        }
        Operation::Sum { column } | Operation::Min { column } | Operation::Max { column } => {
            let col = match resolve_value_column(columns, column) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            if col.inferred_type != ColumnType::Numeric {
                return fail(NotAnswerableReason::NonNumericColumn);
            }
            if rows.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let resolved = inventory::resolve_numeric_column(rows, col.index, locale);
            let values: Vec<f64> = resolved.values.into_iter().flatten().collect();
            if values.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let aggregate = |value: f64| NumericAggregate {
                value,
                unit: col.unit.clone(),
                unparsed: col.unparsed_count,
            };
            let (value, operation_name) = match operation {
                Operation::Sum { .. } => {
                    (TabularValue::Sum(aggregate(values.iter().sum())), "sum")
                }
                Operation::Min { .. } => (
                    TabularValue::Min(aggregate(
                        values.iter().cloned().fold(f64::INFINITY, f64::min),
                    )),
                    "min",
                ),
                Operation::Max { .. } => (
                    TabularValue::Max(aggregate(
                        values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                    )),
                    "max",
                ),
                _ => unreachable!(),
            };
            TabularOutcome::Value {
                value,
                locator: locator(sheet_inventory, Some(&col.name), Some((0, rows.len() - 1))),
                derivation: computed(operation_name, values.len()),
            }
        }
        Operation::GroupSum {
            group_by,
            sum_column,
        }
        | Operation::LargestGroup {
            group_by,
            sum_column,
        } => {
            let group_col = match resolve_value_column(columns, group_by) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            let sum_col = match resolve_value_column(columns, sum_column) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            if sum_col.inferred_type != ColumnType::Numeric {
                return fail(NotAnswerableReason::NonNumericColumn);
            }
            if rows.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let resolved = inventory::resolve_numeric_column(rows, sum_col.index, locale);
            let mut totals: std::collections::BTreeMap<String, f64> =
                std::collections::BTreeMap::new();
            for (row, amount) in rows.iter().zip(resolved.values.iter()) {
                let Some(amount) = amount else {
                    continue;
                };
                let Some(group_cell) = row.get(group_col.index) else {
                    continue;
                };
                let key = text_value(group_cell);
                if key.trim().is_empty() {
                    continue;
                }
                *totals.entry(key).or_insert(0.0) += amount;
            }
            let group_sums: Vec<GroupSum> = totals
                .into_iter()
                .map(|(group, sum)| GroupSum { group, sum })
                .collect();
            if group_sums.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let (value, operation_name) = match operation {
                Operation::LargestGroup { .. } => {
                    let group_count = group_sums.len();
                    let mut top = group_sums;
                    // Stable: equal totals keep the alphabetical order the map gave them.
                    top.sort_by(|a, b| b.sum.partial_cmp(&a.sum).unwrap_or(std::cmp::Ordering::Equal));
                    top.truncate(TOP_GROUPS);
                    (
                        TabularValue::LargestGroup(GroupRanking {
                            group_column: group_col.name.clone(),
                            top,
                            group_count,
                        }),
                        "largest_group",
                    )
                }
                _ => (TabularValue::GroupSums(group_sums), "group_sum"),
            };
            TabularOutcome::Value {
                value,
                locator: locator(sheet_inventory, Some(&sum_col.name), Some((0, rows.len() - 1))),
                derivation: computed(operation_name, rows.len()),
            }
        }
        Operation::LargestRow { by_column } => {
            let col = match resolve_value_column(columns, by_column) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            if col.inferred_type != ColumnType::Numeric {
                return fail(NotAnswerableReason::NonNumericColumn);
            }
            let resolved = inventory::resolve_numeric_column(rows, col.index, locale);
            let mut best: Option<(usize, f64)> = None;
            for (index, value) in resolved.values.iter().enumerate() {
                let Some(value) = value else {
                    continue;
                };
                if best.is_none_or(|(_, current)| *value > current) {
                    best = Some((index, *value));
                }
            }
            let Some((row_index, _)) = best else {
                return fail(NotAnswerableReason::EmptySheet);
            };
            TabularOutcome::Value {
                value: TabularValue::LargestRow(RowValue {
                    row_index,
                    cells: row_cells(columns, &rows[row_index]),
                }),
                locator: locator(
                    sheet_inventory,
                    Some(&col.name),
                    Some((row_index, row_index)),
                ),
                derivation: computed("largest_row", 1),
            }
        }
        Operation::Filter { filter } => {
            let col = match resolve_filter_column(columns, filter) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            let matched = matching_rows(rows, col, &filter.comparison, locale);
            let result_rows: Vec<RowValue> = matched
                .iter()
                .map(|&index| RowValue {
                    row_index: index,
                    cells: row_cells(columns, &rows[index]),
                })
                .collect();
            TabularOutcome::Value {
                value: TabularValue::Rows(result_rows),
                locator: locator(sheet_inventory, Some(&col.name), range_of(&matched)),
                derivation: computed("filter", matched.len()),
            }
        }
        Operation::Sort { column, descending } => {
            let col = match resolve_value_column(columns, column) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            if rows.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let mut indices: Vec<usize> = (0..rows.len()).collect();
            if col.inferred_type == ColumnType::Numeric {
                let resolved = inventory::resolve_numeric_column(rows, col.index, locale);
                indices.sort_by(|&a, &b| {
                    resolved.values[a]
                        .partial_cmp(&resolved.values[b])
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            } else {
                indices.sort_by(|&a, &b| {
                    let ta = rows[a].get(col.index).map(text_value).unwrap_or_default();
                    let tb = rows[b].get(col.index).map(text_value).unwrap_or_default();
                    ta.cmp(&tb)
                });
            }
            if *descending {
                indices.reverse();
            }
            let result_rows: Vec<RowValue> = indices
                .iter()
                .map(|&index| RowValue {
                    row_index: index,
                    cells: row_cells(columns, &rows[index]),
                })
                .collect();
            TabularOutcome::Value {
                value: TabularValue::Rows(result_rows),
                locator: locator(sheet_inventory, Some(&col.name), Some((0, rows.len() - 1))),
                derivation: computed("sort", rows.len()),
            }
        }
        Operation::RowAt { index } => {
            if *index >= rows.len() {
                return fail(NotAnswerableReason::RowOutOfRange);
            }
            TabularOutcome::Value {
                value: TabularValue::Row(RowValue {
                    row_index: *index,
                    cells: row_cells(columns, &rows[*index]),
                }),
                locator: locator(sheet_inventory, None, Some((*index, *index))),
                derivation: computed("row_at", 1),
            }
        }
    }
}

fn not_answerable(
    reason: NotAnswerableReason,
    available_sheets: Vec<String>,
    available_columns: Vec<String>,
) -> TabularOutcome {
    TabularOutcome::NotDeterministicallyAnswerable {
        reason,
        available_sheets,
        available_columns,
    }
}

fn locator(
    sheet: &SheetInventory,
    column: Option<&str>,
    row_range: Option<(usize, usize)>,
) -> TabularLocator {
    TabularLocator {
        sheet: sheet.name.clone(),
        header_row: sheet.header_row,
        column: column.map(str::to_string),
        row_range,
    }
}

fn computed(operation: &str, row_count: usize) -> TabularDerivation {
    TabularDerivation::Computed {
        operation: operation.to_string(),
        row_count,
    }
}

/// The min and max of a set of matched row indices, or `None` when nothing matched. Not
/// necessarily contiguous - a filter can match rows 2 and 9 and nothing between - but it still
/// bounds where the evidence came from, which is what a locator is for.
fn range_of(indices: &[usize]) -> Option<(usize, usize)> {
    let min = indices.iter().min().copied()?;
    let max = indices.iter().max().copied()?;
    Some((min, max))
}

/// Every sheet reachable under `allowed_sheets`: every sheet the workbook holds when it is
/// unrestricted, otherwise the intersection with it, matched case- and accent-insensitively so a
/// scope entry saved as "Facturation" still reaches a sheet the workbook itself spells
/// differently. `pub(crate)` so `tabular::structural` and `tabular::question` report the same
/// reachable set without re-deriving it.
pub(crate) fn reachable_sheets<'a>(
    inventory: &'a TabularInventory,
    allowed_sheets: Option<&[String]>,
) -> Vec<&'a SheetInventory> {
    match allowed_sheets {
        Some(allowed) if !allowed.is_empty() => {
            let folded: Vec<String> = allowed.iter().map(|name| fold_text(name)).collect();
            inventory
                .sheets
                .iter()
                .filter(|sheet| folded.contains(&fold_text(&sheet.name)))
                .collect()
        }
        _ => inventory.sheets.iter().collect(),
    }
}

/// The one sheet `requested` names, within `allowed_sheets` - structure only, no cells. Shared by
/// `resolve_sheet` below (which also needs the matching `SheetData`) and by
/// `tabular::structural`, which never needs raw cells at all.
pub(crate) fn resolve_sheet_inventory<'a>(
    inventory: &'a TabularInventory,
    requested: Option<&str>,
    allowed_sheets: Option<&[String]>,
) -> Result<&'a SheetInventory, NotAnswerableReason> {
    let reachable = reachable_sheets(inventory, allowed_sheets);
    let candidates: Vec<&SheetInventory> = match requested {
        None => reachable,
        Some(name) => {
            let folded = fold_text(name);
            reachable
                .into_iter()
                .filter(|sheet| fold_text(&sheet.name) == folded)
                .collect()
        }
    };
    match candidates.len() {
        0 => Err(NotAnswerableReason::SheetNotFound),
        1 => {
            let sheet_inventory = candidates[0];
            // Checked before the caller can touch the workbook's own sheets, so an ambiguous
            // name is never silently resolved to whichever `SheetData` happens to come first.
            if inventory.sheets_named(&sheet_inventory.name).len() > 1 {
                return Err(NotAnswerableReason::AmbiguousSheetName);
            }
            Ok(sheet_inventory)
        }
        _ => Err(NotAnswerableReason::AmbiguousSheetName),
    }
}

fn resolve_sheet<'a>(
    inventory: &'a TabularInventory,
    workbook: &'a Workbook,
    requested: Option<&str>,
    allowed_sheets: Option<&[String]>,
) -> Result<(&'a SheetInventory, &'a super::SheetData), NotAnswerableReason> {
    let sheet_inventory = resolve_sheet_inventory(inventory, requested, allowed_sheets)?;
    let data = workbook
        .sheets
        .iter()
        .find(|sheet| sheet.name == sheet_inventory.name)
        .ok_or(NotAnswerableReason::SheetNotFound)?;
    Ok((sheet_inventory, data))
}

/// A column resolved for reading its values: exact name, then a case/accent-folded match if it
/// is unique. Never checks `has_formulas` itself - callers decide whether their operation may
/// touch a formula column at all, because some (a plain row lookup) may and some (an aggregate)
/// must not.
fn resolve_column<'a>(
    columns: &'a [ColumnInventory],
    name: &str,
) -> Result<&'a ColumnInventory, NotAnswerableReason> {
    if let Some(column) = columns.iter().find(|column| column.name == name) {
        return Ok(column);
    }
    let folded = fold_text(name);
    let matches: Vec<&ColumnInventory> = columns
        .iter()
        .filter(|column| fold_text(&column.name) == folded)
        .collect();
    match matches.len() {
        1 => Ok(matches[0]),
        0 => Err(NotAnswerableReason::ColumnNotFound),
        _ => Err(NotAnswerableReason::AmbiguousColumnName),
    }
}

/// A column resolved for an operation that reads its **values** as data (an aggregate, a
/// distinct list, a sort, a row's own field): refuses a column carrying even one formula, so a
/// cached formula value can never be folded into a result labelled `Computed`
/// (`docs/DECISIONS.md`).
fn resolve_value_column<'a>(
    columns: &'a [ColumnInventory],
    name: &str,
) -> Result<&'a ColumnInventory, NotAnswerableReason> {
    let column = resolve_column(columns, name)?;
    if column.has_formulas {
        return Err(NotAnswerableReason::FormulaCannotBeVerified);
    }
    Ok(column)
}

fn resolve_filter_column<'a>(
    columns: &'a [ColumnInventory],
    filter: &FilterSpec,
) -> Result<&'a ColumnInventory, NotAnswerableReason> {
    let column = resolve_value_column(columns, &filter.column)?;
    if matches!(
        filter.comparison,
        Comparison::GreaterThan(_) | Comparison::LessThan(_)
    ) && column.inferred_type != ColumnType::Numeric
    {
        return Err(NotAnswerableReason::NonNumericColumn);
    }
    Ok(column)
}

fn matching_rows(
    rows: &[Vec<CellValue>],
    column: &ColumnInventory,
    comparison: &Comparison,
    locale: &str,
) -> Vec<usize> {
    // Resolved once for the whole column, not per cell, so a comparison agrees with the same
    // convention the inventory used to type the column in the first place.
    let numeric = matches!(comparison, Comparison::GreaterThan(_) | Comparison::LessThan(_))
        .then(|| inventory::resolve_numeric_column(rows, column.index, locale));

    rows.iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let cell = row.get(column.index)?;
            let matches = match comparison {
                Comparison::Equals(value) => fold_text(&text_value(cell)) == fold_text(value),
                Comparison::Contains(value) => {
                    fold_text(&text_value(cell)).contains(&fold_text(value))
                }
                Comparison::GreaterThan(threshold) => numeric
                    .as_ref()
                    .and_then(|resolved| resolved.values[index])
                    .is_some_and(|value| value > *threshold),
                Comparison::LessThan(threshold) => numeric
                    .as_ref()
                    .and_then(|resolved| resolved.values[index])
                    .is_some_and(|value| value < *threshold),
            };
            matches.then_some(index)
        })
        .collect()
}

fn row_cells(columns: &[ColumnInventory], row: &[CellValue]) -> Vec<CellText> {
    columns
        .iter()
        .map(|column| CellText {
            column: column.name.clone(),
            text: row.get(column.index).map(text_value).unwrap_or_default(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabular::inventory::TabularFormat;
    use crate::tabular::SheetData;

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

    fn build(sheets: Vec<SheetData>) -> (Workbook, TabularInventory) {
        let workbook = Workbook { sheets };
        let inventory = TabularInventory::build(
            "data.csv",
            "hash-1",
            TabularFormat::Csv,
            &workbook,
            TEST_LOCALE,
        );
        (workbook, inventory)
    }

    fn build_one(rows: Vec<Vec<CellValue>>) -> (Workbook, TabularInventory) {
        build(vec![sheet("Feuille1", rows)])
    }

    fn invoices() -> (Workbook, TabularInventory) {
        build_one(vec![
            text_row(&["fournisseur", "categorie", "montant"]),
            text_row(&["Alpha", "Fournitures", "120,50"]),
            text_row(&["Beta", "Materiel", "75,00"]),
            text_row(&["Alpha", "Fournitures", "30,00"]),
            text_row(&["Gamma", "Materiel", "500,00"]),
        ])
    }

    const TEST_LOCALE: &str = "fr-FR";

    fn run(
        workbook: &Workbook,
        inventory: &TabularInventory,
        operation: Operation,
    ) -> TabularOutcome {
        execute(workbook, inventory, None, None, &operation, TEST_LOCALE)
    }

    // --- Positive paths -----------------------------------------------------------------

    #[test]
    fn count_reports_every_data_row() {
        let (workbook, inventory) = invoices();
        let outcome = run(&workbook, &inventory, Operation::Count { filter: None });

        match outcome {
            TabularOutcome::Value {
                value: TabularValue::Count(count),
                locator,
                derivation,
            } => {
                assert_eq!(count, 4);
                assert_eq!(locator.row_range, Some((0, 3)));
                assert_eq!(
                    derivation,
                    TabularDerivation::Computed {
                        operation: "count".into(),
                        row_count: 4
                    }
                );
            }
            other => panic!("expected a count, got {other:?}"),
        }
    }

    #[test]
    fn count_with_a_filter_counts_only_the_matching_rows() {
        let (workbook, inventory) = invoices();
        let outcome = run(
            &workbook,
            &inventory,
            Operation::Count {
                filter: Some(FilterSpec {
                    column: "categorie".into(),
                    comparison: Comparison::Equals("Fournitures".into()),
                }),
            },
        );

        assert_eq!(
            outcome,
            TabularOutcome::Value {
                value: TabularValue::Count(2),
                locator: TabularLocator {
                    sheet: "Feuille1".into(),
                    header_row: Some(0),
                    column: None,
                    row_range: Some((0, 2)),
                },
                derivation: TabularDerivation::Computed {
                    operation: "count".into(),
                    row_count: 2
                },
            }
        );
    }

    #[test]
    fn distinct_values_are_sorted_and_deduplicated() {
        let (workbook, inventory) = invoices();
        let outcome = run(
            &workbook,
            &inventory,
            Operation::Distinct {
                column: "fournisseur".into(),
            },
        );

        let TabularOutcome::Value {
            value: TabularValue::Distinct(values),
            ..
        } = outcome
        else {
            panic!("expected a distinct list");
        };
        assert_eq!(values, vec!["Alpha", "Beta", "Gamma"]);
    }

    #[test]
    fn sum_parses_decimal_comma_amounts_over_a_full_pass() {
        let (workbook, inventory) = invoices();
        let outcome = run(
            &workbook,
            &inventory,
            Operation::Sum {
                column: "montant".into(),
            },
        );

        let TabularOutcome::Value {
            value: TabularValue::Sum(total),
            derivation,
            ..
        } = outcome
        else {
            panic!("expected a sum");
        };
        assert!((total.value - 725.50).abs() < 1e-9);
        assert_eq!(total.unparsed, 0);
        assert_eq!(
            derivation,
            TabularDerivation::Computed {
                operation: "sum".into(),
                row_count: 4
            }
        );
    }

    #[test]
    fn min_and_max_read_the_same_column_honestly() {
        let (workbook, inventory) = invoices();
        let min = run(
            &workbook,
            &inventory,
            Operation::Min {
                column: "montant".into(),
            },
        );
        let max = run(
            &workbook,
            &inventory,
            Operation::Max {
                column: "montant".into(),
            },
        );

        assert!(matches!(
            min,
            TabularOutcome::Value { value: TabularValue::Min(v), .. } if (v.value - 30.0).abs() < 1e-9
        ));
        assert!(matches!(
            max,
            TabularOutcome::Value { value: TabularValue::Max(v), .. } if (v.value - 500.0).abs() < 1e-9
        ));
    }

    #[test]
    fn group_sum_totals_per_group_and_the_largest_row_is_a_different_fact() {
        let (workbook, inventory) = invoices();
        let group_sum = run(
            &workbook,
            &inventory,
            Operation::GroupSum {
                group_by: "categorie".into(),
                sum_column: "montant".into(),
            },
        );
        let TabularOutcome::Value {
            value: TabularValue::GroupSums(sums),
            ..
        } = group_sum
        else {
            panic!("expected group sums");
        };
        let materiel = sums.iter().find(|s| s.group == "Materiel").unwrap();
        assert!((materiel.sum - 575.0).abs() < 1e-9);
        let fournitures = sums.iter().find(|s| s.group == "Fournitures").unwrap();
        assert!((fournitures.sum - 150.50).abs() < 1e-9);

        let largest = run(
            &workbook,
            &inventory,
            Operation::LargestRow {
                by_column: "montant".into(),
            },
        );
        let TabularOutcome::Value {
            value: TabularValue::LargestRow(row),
            locator,
            ..
        } = largest
        else {
            panic!("expected a largest row");
        };
        // The single row with 500,00, not a group total - a different fact from group_sum above.
        assert_eq!(row.row_index, 3);
        assert_eq!(locator.row_range, Some((3, 3)));
        let fournisseur = row
            .cells
            .iter()
            .find(|cell| cell.column == "fournisseur")
            .unwrap();
        assert_eq!(fournisseur.text, "Gamma");
    }

    #[test]
    fn filter_returns_only_the_matching_rows_with_every_column() {
        let (workbook, inventory) = invoices();
        let outcome = run(
            &workbook,
            &inventory,
            Operation::Filter {
                filter: FilterSpec {
                    column: "montant".into(),
                    comparison: Comparison::GreaterThan(100.0),
                },
            },
        );

        let TabularOutcome::Value {
            value: TabularValue::Rows(rows),
            ..
        } = outcome
        else {
            panic!("expected filtered rows");
        };
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.cells.len() == 3));
    }

    #[test]
    fn sort_orders_a_numeric_column_ascending_and_descending() {
        let (workbook, inventory) = invoices();
        let ascending = run(
            &workbook,
            &inventory,
            Operation::Sort {
                column: "montant".into(),
                descending: false,
            },
        );
        let TabularOutcome::Value {
            value: TabularValue::Rows(rows),
            ..
        } = ascending
        else {
            panic!("expected sorted rows");
        };
        let amounts: Vec<&str> = rows
            .iter()
            .map(|row| {
                row.cells
                    .iter()
                    .find(|c| c.column == "montant")
                    .unwrap()
                    .text
                    .as_str()
            })
            .collect();
        assert_eq!(amounts, vec!["30,00", "75,00", "120,50", "500,00"]);

        let descending = run(
            &workbook,
            &inventory,
            Operation::Sort {
                column: "montant".into(),
                descending: true,
            },
        );
        let TabularOutcome::Value {
            value: TabularValue::Rows(rows),
            ..
        } = descending
        else {
            panic!("expected sorted rows");
        };
        assert_eq!(
            rows[0]
                .cells
                .iter()
                .find(|c| c.column == "montant")
                .unwrap()
                .text,
            "500,00"
        );
    }

    #[test]
    fn row_at_looks_up_one_row_by_position() {
        let (workbook, inventory) = invoices();
        let outcome = run(&workbook, &inventory, Operation::RowAt { index: 1 });

        let TabularOutcome::Value {
            value: TabularValue::Row(row),
            ..
        } = outcome
        else {
            panic!("expected a row");
        };
        assert_eq!(row.row_index, 1);
        assert_eq!(
            row.cells.iter().find(|c| c.column == "fournisseur").unwrap().text,
            "Beta"
        );
    }

    // --- Adversarial cases (docs/SESSION-DATA-04-TABULAR-Engine.md section 7) ------------

    #[test]
    fn a_column_that_does_not_exist_is_refused_not_guessed() {
        let (workbook, inventory) = invoices();
        let outcome = run(
            &workbook,
            &inventory,
            Operation::Sum {
                column: "does-not-exist".into(),
            },
        );

        let TabularOutcome::NotDeterministicallyAnswerable {
            reason,
            available_columns,
            ..
        } = outcome
        else {
            panic!("expected a refusal");
        };
        assert_eq!(reason, NotAnswerableReason::ColumnNotFound);
        assert_eq!(
            available_columns,
            vec!["fournisseur", "categorie", "montant"]
        );
    }

    #[test]
    fn a_sheet_that_does_not_exist_is_refused() {
        let (workbook, inventory) = invoices();
        let outcome = execute(
            &workbook,
            &inventory,
            Some("Nope"),
            None,
            &Operation::Count { filter: None },
            TEST_LOCALE,
        );

        assert!(matches!(
            outcome,
            TabularOutcome::NotDeterministicallyAnswerable {
                reason: NotAnswerableReason::SheetNotFound,
                ..
            }
        ));
    }

    #[test]
    fn a_row_outside_the_workbooks_range_is_refused() {
        let (workbook, inventory) = invoices();
        let outcome = run(&workbook, &inventory, Operation::RowAt { index: 999 });

        assert!(matches!(
            outcome,
            TabularOutcome::NotDeterministicallyAnswerable {
                reason: NotAnswerableReason::RowOutOfRange,
                ..
            }
        ));
    }

    #[test]
    fn a_total_over_non_numeric_values_is_refused_rather_than_coerced() {
        let (workbook, inventory) = invoices();
        let outcome = run(
            &workbook,
            &inventory,
            Operation::Sum {
                column: "fournisseur".into(),
            },
        );

        assert!(matches!(
            outcome,
            TabularOutcome::NotDeterministicallyAnswerable {
                reason: NotAnswerableReason::NonNumericColumn,
                ..
            }
        ));
    }

    #[test]
    fn a_formula_whose_result_cannot_be_verified_refuses_the_aggregate() {
        let (workbook, inventory) = build_one(vec![
            text_row(&["a", "b", "total"]),
            vec![
                CellValue::Number(1.0),
                CellValue::Number(2.0),
                CellValue::Formula {
                    expression: "A2+B2".into(),
                    cached_value: Some(Box::new(CellValue::Number(3.0))),
                },
            ],
            vec![
                CellValue::Number(4.0),
                CellValue::Number(5.0),
                CellValue::Formula {
                    expression: "A3+B3".into(),
                    cached_value: Some(Box::new(CellValue::Number(9.0))),
                },
            ],
        ]);

        let sum = run(
            &workbook,
            &inventory,
            Operation::Sum {
                column: "total".into(),
            },
        );
        assert!(matches!(
            sum,
            TabularOutcome::NotDeterministicallyAnswerable {
                reason: NotAnswerableReason::FormulaCannotBeVerified,
                ..
            }
        ));

        // The engine's own "a" column has no formulas, so summing it is still answerable: the
        // refusal is about the column actually read, not the whole sheet.
        let clean = run(
            &workbook,
            &inventory,
            Operation::Sum { column: "a".into() },
        );
        assert!(matches!(
            clean,
            TabularOutcome::Value { value: TabularValue::Sum(v), .. } if (v.value - 5.0).abs() < 1e-9
        ));

        // A plain row lookup may still show the formula's last cached value - it is a row
        // listing, not an aggregate presented as verified.
        let row = run(&workbook, &inventory, Operation::RowAt { index: 0 });
        let TabularOutcome::Value {
            value: TabularValue::Row(row),
            ..
        } = row
        else {
            panic!("expected a row");
        };
        assert_eq!(
            row.cells.iter().find(|c| c.column == "total").unwrap().text,
            "3"
        );
    }

    #[test]
    fn two_sheets_with_confusable_names_are_never_chosen_between() {
        let (workbook, inventory) = build(vec![
            sheet("Facturation", vec![text_row(&["a"]), text_row(&["1"])]),
            sheet("FACTURATION", vec![text_row(&["b"]), text_row(&["2"])]),
        ]);

        let outcome = execute(
            &workbook,
            &inventory,
            Some("facturation"),
            None,
            &Operation::Count { filter: None },
            TEST_LOCALE,
        );

        assert!(matches!(
            outcome,
            TabularOutcome::NotDeterministicallyAnswerable {
                reason: NotAnswerableReason::AmbiguousSheetName,
                ..
            }
        ));
    }

    #[test]
    fn no_sheet_named_with_more_than_one_reachable_is_ambiguous_rather_than_a_guess() {
        let (workbook, inventory) = build(vec![
            sheet("Consultations", vec![text_row(&["a"]), text_row(&["1"])]),
            sheet("Facturation", vec![text_row(&["b"]), text_row(&["2"])]),
        ]);

        let outcome = execute(
            &workbook,
            &inventory,
            None,
            None,
            &Operation::Count { filter: None },
            TEST_LOCALE,
        );

        assert!(matches!(
            outcome,
            TabularOutcome::NotDeterministicallyAnswerable {
                reason: NotAnswerableReason::AmbiguousSheetName,
                ..
            }
        ));
    }

    #[test]
    fn a_csv_whose_delimiter_conflicts_with_its_decimal_separator_still_sums_correctly() {
        use crate::tabular::csv_adapter::CsvDataSource;
        use crate::tabular::TabularDataSource;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("montants.csv");
        std::fs::write(
            &path,
            "nom;montant\nCamille;120,50\nEsaie;75,00\n".as_bytes(),
        )
        .unwrap();

        let workbook = CsvDataSource.open(&path).expect("opens the fixture");
        let inventory = TabularInventory::build(
            "montants.csv",
            "hash-csv",
            TabularFormat::Csv,
            &workbook,
            TEST_LOCALE,
        );

        let outcome = execute(
            &workbook,
            &inventory,
            None,
            None,
            &Operation::Sum {
                column: "montant".into(),
            },
            TEST_LOCALE,
        );

        assert!(matches!(
            outcome,
            TabularOutcome::Value { value: TabularValue::Sum(v), .. } if (v.value - 195.50).abs() < 1e-9
        ));
    }

    // --- Scope enforcement ----------------------------------------------------------------

    fn ledger() -> (Workbook, TabularInventory) {
        build(vec![
            sheet(
                "Consultations",
                vec![text_row(&["nom"]), text_row(&["Camille"])],
            ),
            sheet(
                "Facturation",
                vec![text_row(&["montant"]), text_row(&["120,50"])],
            ),
        ])
    }

    #[test]
    fn sheet_names_restricts_analysis_to_the_named_sheets() {
        let (workbook, inventory) = ledger();
        let allowed = vec!["Consultations".to_string()];

        let outcome = execute(
            &workbook,
            &inventory,
            None,
            Some(&allowed),
            &Operation::Count { filter: None },
            TEST_LOCALE,
        );

        match outcome {
            TabularOutcome::Value { locator, .. } => assert_eq!(locator.sheet, "Consultations"),
            other => panic!("expected the single reachable sheet, got {other:?}"),
        }
    }

    #[test]
    fn the_engine_cannot_reach_a_sheet_outside_the_scope_by_naming_it_directly() {
        let (workbook, inventory) = ledger();
        let allowed = vec!["Consultations".to_string()];

        let outcome = execute(
            &workbook,
            &inventory,
            Some("Facturation"),
            Some(&allowed),
            &Operation::Sum {
                column: "montant".into(),
            },
            TEST_LOCALE,
        );

        // Indistinguishable from a sheet that plainly does not exist - the scope is a real
        // boundary, not a hint the engine can be talked past.
        assert!(matches!(
            outcome,
            TabularOutcome::NotDeterministicallyAnswerable {
                reason: NotAnswerableReason::SheetNotFound,
                ..
            }
        ));
    }

    #[test]
    fn the_available_sheets_reported_on_a_refusal_never_include_one_outside_the_scope() {
        let (workbook, inventory) = ledger();
        let allowed = vec!["Consultations".to_string()];

        let outcome = execute(
            &workbook,
            &inventory,
            Some("Nope"),
            Some(&allowed),
            &Operation::Count { filter: None },
            TEST_LOCALE,
        );

        let TabularOutcome::NotDeterministicallyAnswerable {
            available_sheets, ..
        } = outcome
        else {
            panic!("expected a refusal");
        };
        assert_eq!(available_sheets, vec!["Consultations"]);
    }

    // --- Model independence ----------------------------------------------------------------

    #[test]
    fn the_largest_group_is_a_total_per_group_with_its_runners_up() {
        let (workbook, inventory) = invoices();

        let outcome = run(
            &workbook,
            &inventory,
            Operation::LargestGroup {
                group_by: "fournisseur".into(),
                sum_column: "montant".into(),
            },
        );

        let TabularOutcome::Value {
            value: TabularValue::LargestGroup(ranking),
            locator,
            derivation,
        } = outcome
        else {
            panic!("expected a ranking, got {outcome:?}");
        };
        assert_eq!(ranking.group_column, "fournisseur");
        assert_eq!(ranking.group_count, 3);
        assert_eq!(
            ranking.top,
            vec![
                GroupSum { group: "Gamma".into(), sum: 500.0 },
                GroupSum { group: "Alpha".into(), sum: 150.5 },
                GroupSum { group: "Beta".into(), sum: 75.0 },
            ]
        );
        assert_eq!(locator.column.as_deref(), Some("montant"));
        assert_eq!(
            derivation,
            TabularDerivation::Computed {
                operation: "largest_group".into(),
                row_count: 4
            }
        );
    }

    #[test]
    fn the_largest_group_refuses_a_text_column_as_the_total() {
        let (workbook, inventory) = invoices();

        let outcome = run(
            &workbook,
            &inventory,
            Operation::LargestGroup {
                group_by: "montant".into(),
                sum_column: "fournisseur".into(),
            },
        );

        assert!(matches!(
            outcome,
            TabularOutcome::NotDeterministicallyAnswerable {
                reason: NotAnswerableReason::NonNumericColumn,
                ..
            }
        ));
    }

    /// `execute`'s signature takes no gateway, no model alias and no network client of any kind -
    /// this test module never imports `crate::gateway` at all, which is what makes "deterministic
    /// analysis needs no running model" a property of the type signature rather than a promise.
    /// Run with the server stopped, or with no server configured at all, this still passes.
    #[test]
    fn deterministic_operations_have_no_path_to_the_gateway() {
        let (workbook, inventory) = invoices();

        let outcome = run(
            &workbook,
            &inventory,
            Operation::Sum {
                column: "montant".into(),
            },
        );

        assert!(matches!(
            outcome,
            TabularOutcome::Value {
                value: TabularValue::Sum(_),
                ..
            }
        ));
    }
}
