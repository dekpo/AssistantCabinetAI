//! The deterministic tabular engine: `count`, `distinct`, `sum`, `min`, `max`, `mean`, `median`,
//! group sum, the largest group, the least group, the top N groups, rows per group, the largest
//! single row, `filter`, `sort` - the committed set from `docs/BRIEF-SPRINT-2.5-OCR.md`'s sibling
//! brief for Sprint 2b plus decision D5 (`docs/DECISIONS.md`), and nothing else. Every operation
//! here makes zero model or gateway calls: it reads `Workbook` cells and `TabularInventory`
//! structure, computes, and
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

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use super::inventory::{
    self, data_rows, text_value, ColumnInventory, ColumnType, SheetInventory, TabularInventory,
};
use super::{CellValue, Workbook};
use crate::file_reference::fold_text;

/// One filter as it was actually applied, for the locator to carry as data
/// (`docs/DECISIONS.md`, session 11): "what was understood". Rust never writes the sentence - the
/// React catalogues read this and the interface locale to write "fournisseur = Alpha" or "date in
/// March 2026" in her language. One variant per `Comparison`, so a filter the engine actually ran
/// can always be described, whatever comparison it used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppliedFilter {
    Equals { column: String, value: String },
    Contains { column: String, value: String },
    GreaterThan { column: String, threshold: f64 },
    LessThan { column: String, threshold: f64 },
    Between { column: String, low: f64, high: f64 },
    /// ISO weekday: 1 = Monday .. 7 = Sunday.
    Weekday { column: String, weekday: u8 },
    Month { column: String, month: u8 },
    Year { column: String, year: i32 },
    /// Inclusive on both ends, ISO 8601 (`docs/DECISIONS.md`, D3).
    DateRange { column: String, start: String, end: String },
    In { column: String, values: Vec<String> },
}

fn describe_filter(filter: &FilterSpec) -> AppliedFilter {
    let column = filter.column.clone();
    match &filter.comparison {
        Comparison::Equals(value) => AppliedFilter::Equals { column, value: value.clone() },
        Comparison::Contains(value) => AppliedFilter::Contains { column, value: value.clone() },
        Comparison::GreaterThan(threshold) => AppliedFilter::GreaterThan { column, threshold: *threshold },
        Comparison::LessThan(threshold) => AppliedFilter::LessThan { column, threshold: *threshold },
        Comparison::Between(low, high) => AppliedFilter::Between { column, low: *low, high: *high },
        Comparison::Weekday(weekday) => AppliedFilter::Weekday { column, weekday: *weekday },
        Comparison::Month(month) => AppliedFilter::Month { column, month: *month },
        Comparison::Year(year) => AppliedFilter::Year { column, year: *year },
        Comparison::DateRange(start, end) => AppliedFilter::DateRange {
            column,
            start: start.format("%Y-%m-%d").to_string(),
            end: end.format("%Y-%m-%d").to_string(),
        },
        Comparison::In(values) => AppliedFilter::In { column, values: values.clone() },
    }
}

fn applied_filters(filters: &[FilterSpec]) -> Vec<AppliedFilter> {
    filters.iter().map(describe_filter).collect()
}

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
    /// Every filter this value was actually computed under, "what was understood" - data, never
    /// prose (`docs/DECISIONS.md`, session 11). Empty for an unfiltered value.
    #[serde(default)]
    pub filters: Vec<AppliedFilter>,
}

/// The tabular form of `Source.derivation` (`docs/ARCHITECTURE.md`). Never `ModelAsserted`: this
/// module itself never asks a model for a fact, and neither variant below lets a model's own text
/// carry the number - `InterpretedByModel` still names an operation `execute` ran over a full pass
/// of the matched rows, exactly as `Computed` does. The two differ only in who chose *which*
/// operation to run: the deterministic classifier (`Computed`), or a model reading the workbook's
/// schema alone, validated by Rust before `execute` ever saw it (`InterpretedByModel`,
/// `docs/SESSION-DATA-14-Query-Plan.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TabularDerivation {
    /// Calculated here, over a full pass of the matched rows - never a sample.
    Computed { operation: String, row_count: usize },
    /// The same full-pass calculation as `Computed`, but the question did not classify
    /// deterministically: a model, given only the workbook's schema - sheet and column names,
    /// types, units, never a cell value - wrote the plan that chose this operation, and Rust
    /// validated it against the real workbook before `execute` ran it. `model_alias` is shown so
    /// she can tell which model wrote the plan. `plan` is that validated plan, serialised, kept
    /// so the exact same question on the exact same file can be answered again by replaying it -
    /// no model call needed the second time (what a saved conversation will store). An opaque
    /// JSON value on purpose: this module has no reason to depend on `tabular::query_plan`'s own
    /// type, only on the fact that the plan is already validated by the time this is built.
    InterpretedByModel {
        operation: String,
        row_count: usize,
        model_alias: String,
        plan: serde_json::Value,
    },
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

/// The groups a `TopGroups` question actually got, together with what she asked for - so the
/// interface can say "top 80" was honoured as the top 50 rather than silently answering a
/// different question (`docs/SESSION-DATA-12-More-Aggregates.md`, "a larger N is capped").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopGroups {
    pub group_column: String,
    /// At most `requested`, and at most `MAX_TOP_GROUPS`; largest total first, ties keep
    /// alphabetical order.
    pub top: Vec<GroupSum>,
    pub group_count: usize,
    /// The N she actually asked for, uncapped - so "top 80" still says 80 even though `top` holds
    /// at most 50.
    pub requested: usize,
    pub capped: bool,
}

/// The hard ceiling on a `TopGroups` question's N (`docs/DECISIONS.md`, D5): a larger request is
/// honoured as this many, stated rather than refused.
pub const MAX_TOP_GROUPS: usize = 50;

/// One group's row count, largest first: "how many rows does each agency have". A **count**, never
/// a sum - `CountPerGroup` is the row-counting sibling of `GroupSum`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupCount {
    pub group: String,
    pub count: usize,
}

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
    /// The arithmetic mean, over the same formula-free numeric cells a `Sum` would read
    /// (`docs/SESSION-DATA-12-More-Aggregates.md`, D5). Never labelled a total: its own `kind`,
    /// never folded into `Sum`.
    Mean(NumericAggregate),
    /// The middle value of the full sorted column (session 12's D5) - the mean of the two middle
    /// values on an even count, never a shortcut over group means.
    Median(NumericAggregate),
    /// Sorted, so the same column always reports its distinct values in the same order.
    Distinct(Vec<String>),
    GroupSums(Vec<GroupSum>),
    LargestGroup(GroupRanking),
    /// The mirror of `LargestGroup`: smallest total first, never the smallest single row
    /// (`docs/DECISIONS.md`, D5).
    LeastGroup(GroupRanking),
    /// The top N groups by total, N capped at `MAX_TOP_GROUPS` and the cap stated
    /// (`docs/DECISIONS.md`, D5).
    TopGroups(TopGroups),
    /// Rows per group, largest first - a count, never a sum (`docs/DECISIONS.md`, D5).
    CountPerGroup(Vec<GroupCount>),
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
    /// The operation needs a real calendar column for a weekday, month, year or date-range
    /// filter, and this column is not `Date`-typed - whether it is plain text, numeric, or a
    /// day-first column no cell settled (`ColumnInventory::ambiguous_date`). Never guessed at,
    /// the same refusal `NonNumericColumn` already gives a comparison over a non-numeric column
    /// (`docs/DECISIONS.md`, session 11).
    NonDateColumn,
    /// A residual question word was found in no reachable column's real values, and looked like
    /// an attempted filter value rather than ordinary prose (`tabular_answer::detect_filters`).
    /// Not constructed by this module, for the same reason `FilterNotSupported` is carried here
    /// rather than produced.
    ValueNotFound,
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

/// `Between`, `Weekday`, `Month`, `Year` and `DateRange` are session 11's (`docs/DECISIONS.md`):
/// numeric comparisons keep reusing `inventory::resolve_numeric_column` exactly as before, and
/// the four date variants read a column through the new `inventory::resolve_date_column` - never
/// a text comparison, so a `dd/mm/yyyy` cell and an XLSX date cell agree once parsed.
#[derive(Debug, Clone, PartialEq)]
pub enum Comparison {
    Equals(String),
    Contains(String),
    GreaterThan(f64),
    LessThan(f64),
    /// Inclusive both ends.
    Between(f64, f64),
    /// ISO weekday: 1 = Monday .. 7 = Sunday.
    Weekday(u8),
    /// 1-12.
    Month(u8),
    Year(i32),
    /// Inclusive both ends.
    DateRange(NaiveDate, NaiveDate),
    In(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilterSpec {
    pub column: String,
    pub comparison: Comparison,
}

/// The committed deterministic operations, and nothing else (`docs/DECISIONS.md`, "XLSX scope
/// for the first implementation" and its siblings): no arbitrary expression language, no formula
/// evaluation, no operation this session did not name.
///
/// `filters`, session 11: every aggregate but `Filter` and `RowAt` takes them, empty meaning
/// today's unfiltered behaviour. Several filters combine with AND only - no OR, no nesting
/// (`docs/DECISIONS.md`, session 11's anti-patterns).
#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    Count { filters: Vec<FilterSpec> },
    Distinct { column: String, filters: Vec<FilterSpec> },
    Sum { column: String, filters: Vec<FilterSpec> },
    Min { column: String, filters: Vec<FilterSpec> },
    Max { column: String, filters: Vec<FilterSpec> },
    /// Session 12's D5 (`docs/DECISIONS.md`): the arithmetic mean over the matched rows' numeric
    /// cells, unparsed cells excluded and counted exactly as `Sum` excludes and counts them.
    Mean { column: String, filters: Vec<FilterSpec> },
    /// The median of the matched rows' numeric cells, computed over the full sorted list - never a
    /// shortcut such as averaging group medians (session 12's anti-patterns).
    Median { column: String, filters: Vec<FilterSpec> },
    GroupSum { group_by: String, sum_column: String, filters: Vec<FilterSpec> },
    /// The group whose total is largest, with the runners-up.
    LargestGroup { group_by: String, sum_column: String, filters: Vec<FilterSpec> },
    /// The mirror of `LargestGroup`: smallest total first.
    LeastGroup { group_by: String, sum_column: String, filters: Vec<FilterSpec> },
    /// The top `n` groups by total, `n` capped at `MAX_TOP_GROUPS` and the cap reported.
    TopGroups {
        group_by: String,
        sum_column: String,
        n: usize,
        filters: Vec<FilterSpec>,
    },
    /// Rows per group, largest first - no sum column, since it counts rather than totals.
    CountPerGroup { group_by: String, filters: Vec<FilterSpec> },
    LargestRow { by_column: String, filters: Vec<FilterSpec> },
    /// A row listing this one filter alone produces - constructed directly, never reached by
    /// `tabular::question`'s classifier (`docs/DECISIONS.md`, "the tabular engine session").
    Filter { filter: FilterSpec },
    Sort { column: String, descending: bool, filters: Vec<FilterSpec> },
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
        Operation::Count { filters } => {
            let matched = match matching_rows(rows, columns, filters, locale) {
                Ok(indices) => indices,
                Err(reason) => return fail(reason),
            };
            TabularOutcome::Value {
                value: TabularValue::Count(matched.len()),
                locator: locator(sheet_inventory, None, range_of(&matched), applied_filters(filters)),
                derivation: computed("count", matched.len()),
            }
        }
        Operation::Distinct { column, filters } => {
            let col = match resolve_value_column(columns, column) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            let matched = match matching_rows(rows, columns, filters, locale) {
                Ok(indices) => indices,
                Err(reason) => return fail(reason),
            };
            if matched.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let values: BTreeSet<String> = matched
                .iter()
                .filter_map(|&index| rows[index].get(col.index))
                .map(text_value)
                .filter(|text| !text.trim().is_empty())
                .collect();
            TabularOutcome::Value {
                value: TabularValue::Distinct(values.into_iter().collect()),
                locator: locator(
                    sheet_inventory,
                    Some(&col.name),
                    range_of(&matched),
                    applied_filters(filters),
                ),
                derivation: computed("distinct", matched.len()),
            }
        }
        Operation::Sum { column, filters }
        | Operation::Min { column, filters }
        | Operation::Max { column, filters }
        | Operation::Mean { column, filters }
        | Operation::Median { column, filters } => {
            let col = match resolve_value_column(columns, column) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            if col.inferred_type != ColumnType::Numeric {
                return fail(NotAnswerableReason::NonNumericColumn);
            }
            let matched = match matching_rows(rows, columns, filters, locale) {
                Ok(indices) => indices,
                Err(reason) => return fail(reason),
            };
            if matched.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            // Resolved over the whole column, filters or not: a separator's thousands-vs-decimal
            // role is corroborated from the column's own cells (`docs/DECISIONS.md`, D2), which a
            // filtered subset could read differently from the same cells read unfiltered.
            let resolved = inventory::resolve_numeric_column(rows, col.index, locale);
            let values: Vec<f64> = matched
                .iter()
                .filter_map(|&index| resolved.values[index])
                .collect();
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
                Operation::Mean { .. } => {
                    let mean = values.iter().sum::<f64>() / values.len() as f64;
                    (TabularValue::Mean(aggregate(mean)), "mean")
                }
                Operation::Median { .. } => {
                    let mut sorted = values.clone();
                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    let middle = sorted.len() / 2;
                    let median = if sorted.len() % 2 == 0 {
                        (sorted[middle - 1] + sorted[middle]) / 2.0
                    } else {
                        sorted[middle]
                    };
                    (TabularValue::Median(aggregate(median)), "median")
                }
                _ => unreachable!(),
            };
            TabularOutcome::Value {
                value,
                locator: locator(
                    sheet_inventory,
                    Some(&col.name),
                    range_of(&matched),
                    applied_filters(filters),
                ),
                derivation: computed(operation_name, values.len()),
            }
        }
        Operation::GroupSum {
            group_by,
            sum_column,
            filters,
        }
        | Operation::LargestGroup {
            group_by,
            sum_column,
            filters,
        }
        | Operation::LeastGroup {
            group_by,
            sum_column,
            filters,
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
            let matched = match matching_rows(rows, columns, filters, locale) {
                Ok(indices) => indices,
                Err(reason) => return fail(reason),
            };
            if matched.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let group_sums = group_sums_for(rows, &matched, group_col, sum_col, locale);
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
                Operation::LeastGroup { .. } => {
                    let group_count = group_sums.len();
                    let mut top = group_sums;
                    // Stable: equal totals keep the alphabetical order the map gave them, exactly
                    // as `LargestGroup` does for its own ties.
                    top.sort_by(|a, b| a.sum.partial_cmp(&b.sum).unwrap_or(std::cmp::Ordering::Equal));
                    top.truncate(TOP_GROUPS);
                    (
                        TabularValue::LeastGroup(GroupRanking {
                            group_column: group_col.name.clone(),
                            top,
                            group_count,
                        }),
                        "least_group",
                    )
                }
                _ => (TabularValue::GroupSums(group_sums), "group_sum"),
            };
            TabularOutcome::Value {
                value,
                locator: locator(
                    sheet_inventory,
                    Some(&sum_col.name),
                    range_of(&matched),
                    applied_filters(filters),
                ),
                derivation: computed(operation_name, matched.len()),
            }
        }
        Operation::TopGroups {
            group_by,
            sum_column,
            n,
            filters,
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
            let matched = match matching_rows(rows, columns, filters, locale) {
                Ok(indices) => indices,
                Err(reason) => return fail(reason),
            };
            if matched.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let group_sums = group_sums_for(rows, &matched, group_col, sum_col, locale);
            if group_sums.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let group_count = group_sums.len();
            let mut top = group_sums;
            top.sort_by(|a, b| b.sum.partial_cmp(&a.sum).unwrap_or(std::cmp::Ordering::Equal));
            let requested = (*n).max(1);
            let capped = requested > MAX_TOP_GROUPS;
            top.truncate(requested.min(MAX_TOP_GROUPS));
            TabularOutcome::Value {
                value: TabularValue::TopGroups(TopGroups {
                    group_column: group_col.name.clone(),
                    top,
                    group_count,
                    requested,
                    capped,
                }),
                locator: locator(
                    sheet_inventory,
                    Some(&sum_col.name),
                    range_of(&matched),
                    applied_filters(filters),
                ),
                derivation: computed("top_n", matched.len()),
            }
        }
        Operation::CountPerGroup { group_by, filters } => {
            let group_col = match resolve_value_column(columns, group_by) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            let matched = match matching_rows(rows, columns, filters, locale) {
                Ok(indices) => indices,
                Err(reason) => return fail(reason),
            };
            if matched.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let mut counts: std::collections::BTreeMap<String, usize> =
                std::collections::BTreeMap::new();
            for &index in &matched {
                let Some(cell) = rows[index].get(group_col.index) else {
                    continue;
                };
                let key = text_value(cell);
                if key.trim().is_empty() {
                    continue;
                }
                *counts.entry(key).or_insert(0) += 1;
            }
            if counts.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let mut group_counts: Vec<GroupCount> = counts
                .into_iter()
                .map(|(group, count)| GroupCount { group, count })
                .collect();
            // Stable: equal counts keep the alphabetical order the map gave them.
            group_counts.sort_by(|a, b| b.count.cmp(&a.count));
            TabularOutcome::Value {
                value: TabularValue::CountPerGroup(group_counts),
                locator: locator(
                    sheet_inventory,
                    Some(&group_col.name),
                    range_of(&matched),
                    applied_filters(filters),
                ),
                derivation: computed("count_per_group", matched.len()),
            }
        }
        Operation::LargestRow { by_column, filters } => {
            let col = match resolve_value_column(columns, by_column) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            if col.inferred_type != ColumnType::Numeric {
                return fail(NotAnswerableReason::NonNumericColumn);
            }
            let matched = match matching_rows(rows, columns, filters, locale) {
                Ok(indices) => indices,
                Err(reason) => return fail(reason),
            };
            let resolved = inventory::resolve_numeric_column(rows, col.index, locale);
            let mut best: Option<(usize, f64)> = None;
            for &index in &matched {
                let Some(value) = resolved.values[index] else {
                    continue;
                };
                if best.is_none_or(|(_, current)| value > current) {
                    best = Some((index, value));
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
                    applied_filters(filters),
                ),
                derivation: computed("largest_row", 1),
            }
        }
        Operation::Filter { filter } => {
            let col = match resolve_filter_column(columns, filter) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            let matched = matching_rows_for(rows, col, &filter.comparison, locale);
            let result_rows: Vec<RowValue> = matched
                .iter()
                .map(|&index| RowValue {
                    row_index: index,
                    cells: row_cells(columns, &rows[index]),
                })
                .collect();
            TabularOutcome::Value {
                value: TabularValue::Rows(result_rows),
                locator: locator(
                    sheet_inventory,
                    Some(&col.name),
                    range_of(&matched),
                    applied_filters(std::slice::from_ref(filter)),
                ),
                derivation: computed("filter", matched.len()),
            }
        }
        Operation::Sort {
            column,
            descending,
            filters,
        } => {
            let col = match resolve_value_column(columns, column) {
                Ok(col) => col,
                Err(reason) => return fail(reason),
            };
            let matched = match matching_rows(rows, columns, filters, locale) {
                Ok(indices) => indices,
                Err(reason) => return fail(reason),
            };
            if matched.is_empty() {
                return fail(NotAnswerableReason::EmptySheet);
            }
            let mut indices = matched.clone();
            if col.inferred_type == ColumnType::Numeric {
                let resolved = inventory::resolve_numeric_column(rows, col.index, locale);
                indices.sort_by(|&a, &b| {
                    resolved.values[a]
                        .partial_cmp(&resolved.values[b])
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            } else if col.inferred_type == ColumnType::Date {
                // A `dd/mm/yyyy` text date must never sort as text - "09/08/2026" would then
                // read before "28/01/2025" on the day alone (`docs/DECISIONS.md`, live bug B1).
                let resolved = inventory::resolve_date_column(rows, col.index);
                indices.sort_by(|&a, &b| resolved[a].cmp(&resolved[b]));
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
                locator: locator(
                    sheet_inventory,
                    Some(&col.name),
                    range_of(&matched),
                    applied_filters(filters),
                ),
                derivation: computed("sort", matched.len()),
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
                locator: locator(sheet_inventory, None, Some((*index, *index)), Vec::new()),
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
    filters: Vec<AppliedFilter>,
) -> TabularLocator {
    TabularLocator {
        sheet: sheet.name.clone(),
        header_row: sheet.header_row,
        column: column.map(str::to_string),
        row_range,
        filters,
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

/// Which type a comparison needs its column to already be - never guessed at, the same refusal a
/// numeric comparison over a text column always gave, extended to the four date comparisons
/// session 11 adds (`docs/DECISIONS.md`). `Equals`, `Contains` and `In` read any column's text, so
/// they need no type check of their own.
fn resolve_filter_column<'a>(
    columns: &'a [ColumnInventory],
    filter: &FilterSpec,
) -> Result<&'a ColumnInventory, NotAnswerableReason> {
    let column = resolve_value_column(columns, &filter.column)?;
    match filter.comparison {
        Comparison::GreaterThan(_) | Comparison::LessThan(_) | Comparison::Between(_, _) => {
            if column.inferred_type != ColumnType::Numeric {
                return Err(NotAnswerableReason::NonNumericColumn);
            }
        }
        Comparison::Weekday(_)
        | Comparison::Month(_)
        | Comparison::Year(_)
        | Comparison::DateRange(_, _) => {
            if column.inferred_type != ColumnType::Date {
                return Err(NotAnswerableReason::NonDateColumn);
            }
        }
        Comparison::Equals(_) | Comparison::Contains(_) | Comparison::In(_) => {}
    }
    Ok(column)
}

/// Every row index every one of `filters` matches, combined with AND only - no OR, no nesting
/// (`docs/DECISIONS.md`, session 11's anti-patterns). Empty `filters` matches every row, exactly
/// today's unfiltered behaviour. `Err` the moment one filter names a column the wrong type for its
/// own comparison, before any row is read.
fn matching_rows(
    rows: &[Vec<CellValue>],
    columns: &[ColumnInventory],
    filters: &[FilterSpec],
    locale: &str,
) -> Result<Vec<usize>, NotAnswerableReason> {
    if filters.is_empty() {
        return Ok((0..rows.len()).collect());
    }
    let mut matched: Option<BTreeSet<usize>> = None;
    for filter in filters {
        let column = resolve_filter_column(columns, filter)?;
        let indices: BTreeSet<usize> = matching_rows_for(rows, column, &filter.comparison, locale)
            .into_iter()
            .collect();
        matched = Some(match matched {
            None => indices,
            Some(existing) => existing.intersection(&indices).copied().collect(),
        });
    }
    Ok(matched.unwrap_or_default().into_iter().collect())
}

/// One filter's own matches, over the whole column - `matching_rows` intersects these across
/// several filters combined with AND.
fn matching_rows_for(
    rows: &[Vec<CellValue>],
    column: &ColumnInventory,
    comparison: &Comparison,
    locale: &str,
) -> Vec<usize> {
    // Resolved once for the whole column, not per cell, so a comparison agrees with the same
    // convention the inventory used to type the column in the first place.
    let numeric = matches!(
        comparison,
        Comparison::GreaterThan(_) | Comparison::LessThan(_) | Comparison::Between(_, _)
    )
    .then(|| inventory::resolve_numeric_column(rows, column.index, locale));
    let dates = matches!(
        comparison,
        Comparison::Weekday(_) | Comparison::Month(_) | Comparison::Year(_) | Comparison::DateRange(_, _)
    )
    .then(|| inventory::resolve_date_column(rows, column.index));

    rows.iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let cell = row.get(column.index)?;
            let matches = match comparison {
                Comparison::Equals(value) => fold_text(&text_value(cell)) == fold_text(value),
                Comparison::Contains(value) => {
                    fold_text(&text_value(cell)).contains(&fold_text(value))
                }
                Comparison::In(values) => {
                    let folded_cell = fold_text(&text_value(cell));
                    values.iter().any(|value| fold_text(value) == folded_cell)
                }
                Comparison::GreaterThan(threshold) => numeric
                    .as_ref()
                    .and_then(|resolved| resolved.values[index])
                    .is_some_and(|value| value > *threshold),
                Comparison::LessThan(threshold) => numeric
                    .as_ref()
                    .and_then(|resolved| resolved.values[index])
                    .is_some_and(|value| value < *threshold),
                Comparison::Between(low, high) => numeric
                    .as_ref()
                    .and_then(|resolved| resolved.values[index])
                    .is_some_and(|value| value >= *low && value <= *high),
                Comparison::Weekday(weekday) => dates
                    .as_ref()
                    .and_then(|resolved| resolved[index])
                    .is_some_and(|date| date.weekday().number_from_monday() as u8 == *weekday),
                Comparison::Month(month) => dates
                    .as_ref()
                    .and_then(|resolved| resolved[index])
                    .is_some_and(|date| date.month() as u8 == *month),
                Comparison::Year(year) => dates
                    .as_ref()
                    .and_then(|resolved| resolved[index])
                    .is_some_and(|date| date.year() == *year),
                Comparison::DateRange(start, end) => dates
                    .as_ref()
                    .and_then(|resolved| resolved[index])
                    .is_some_and(|date| date >= *start && date <= *end),
            };
            matches.then_some(index)
        })
        .collect()
}

/// Every matched row's `sum_col` value, totalled by its `group_col` text - shared by `GroupSum`,
/// `LargestGroup`, `LeastGroup` and `TopGroups`, which differ only in how they sort and truncate
/// the result.
fn group_sums_for(
    rows: &[Vec<CellValue>],
    matched: &[usize],
    group_col: &ColumnInventory,
    sum_col: &ColumnInventory,
    locale: &str,
) -> Vec<GroupSum> {
    let resolved = inventory::resolve_numeric_column(rows, sum_col.index, locale);
    let mut totals: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for &index in matched {
        let Some(amount) = resolved.values[index] else {
            continue;
        };
        let Some(group_cell) = rows[index].get(group_col.index) else {
            continue;
        };
        let key = text_value(group_cell);
        if key.trim().is_empty() {
            continue;
        }
        *totals.entry(key).or_insert(0.0) += amount;
    }
    totals
        .into_iter()
        .map(|(group, sum)| GroupSum { group, sum })
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
        let outcome = run(&workbook, &inventory, Operation::Count { filters: vec![] });

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
                filters: vec![FilterSpec {
                    column: "categorie".into(),
                    comparison: Comparison::Equals("Fournitures".into()),
                }],
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
                    filters: vec![AppliedFilter::Equals {
                        column: "categorie".into(),
                        value: "Fournitures".into(),
                    }],
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
                filters: vec![],
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
                filters: vec![],
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
                filters: vec![],
            },
        );
        let max = run(
            &workbook,
            &inventory,
            Operation::Max {
                column: "montant".into(),
                filters: vec![],
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
                filters: vec![],
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
                filters: vec![],
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
                filters: vec![],
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
                filters: vec![],
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
                filters: vec![],
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
            &Operation::Count { filters: vec![] },
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
                filters: vec![],
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
                filters: vec![],
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
            Operation::Sum {
                column: "a".into(),
                filters: vec![],
            },
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

    /// Session 11's adversarial list (`docs/DECISIONS.md`): a filter naming a formula column is
    /// refused the same way an aggregate over one already was - `resolve_filter_column` reuses
    /// `resolve_value_column`, so the two can never disagree.
    #[test]
    fn a_filter_on_a_formula_column_refuses_rather_than_reading_it() {
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

        let outcome = run(
            &workbook,
            &inventory,
            Operation::Count {
                filters: vec![FilterSpec {
                    column: "total".into(),
                    comparison: Comparison::GreaterThan(0.0),
                }],
            },
        );

        assert!(matches!(
            outcome,
            TabularOutcome::NotDeterministicallyAnswerable {
                reason: NotAnswerableReason::FormulaCannotBeVerified,
                ..
            }
        ));
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
            &Operation::Count { filters: vec![] },
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
            &Operation::Count { filters: vec![] },
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
                filters: vec![],
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
            &Operation::Count { filters: vec![] },
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
                filters: vec![],
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
            &Operation::Count { filters: vec![] },
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
                filters: vec![],
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
                filters: vec![],
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
                filters: vec![],
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

    // --- Session 12's D5: mean, median, least group, top N groups, count per group ----------

    #[test]
    fn mean_and_median_handle_even_and_odd_counts() {
        // An even count (4 values: 10, 20, 30, 40): mean 25, median the average of the two
        // middle values (20, 30) = 25. Two columns, since a one-cell header row is never one
        // (`docs/DECISIONS.md`, D4).
        let (workbook, inventory) = build_one(vec![
            text_row(&["id", "montant"]),
            text_row(&["1", "10"]),
            text_row(&["2", "20"]),
            text_row(&["3", "30"]),
            text_row(&["4", "40"]),
        ]);
        let mean = run(
            &workbook,
            &inventory,
            Operation::Mean {
                column: "montant".into(),
                filters: vec![],
            },
        );
        assert!(matches!(
            mean,
            TabularOutcome::Value { value: TabularValue::Mean(v), .. } if (v.value - 25.0).abs() < 1e-9
        ));
        let median = run(
            &workbook,
            &inventory,
            Operation::Median {
                column: "montant".into(),
                filters: vec![],
            },
        );
        assert!(matches!(
            median,
            TabularOutcome::Value { value: TabularValue::Median(v), .. } if (v.value - 25.0).abs() < 1e-9
        ));

        // An odd count (5 values: 10, 20, 30, 40, 50): mean 30, median the single middle value
        // (30) - never an average of two.
        let (workbook, inventory) = build_one(vec![
            text_row(&["id", "montant"]),
            text_row(&["1", "10"]),
            text_row(&["2", "20"]),
            text_row(&["3", "30"]),
            text_row(&["4", "40"]),
            text_row(&["5", "50"]),
        ]);
        let mean = run(
            &workbook,
            &inventory,
            Operation::Mean {
                column: "montant".into(),
                filters: vec![],
            },
        );
        assert!(matches!(
            mean,
            TabularOutcome::Value { value: TabularValue::Mean(v), .. } if (v.value - 30.0).abs() < 1e-9
        ));
        let median = run(
            &workbook,
            &inventory,
            Operation::Median {
                column: "montant".into(),
                filters: vec![],
            },
        );
        assert!(matches!(
            median,
            TabularOutcome::Value { value: TabularValue::Median(v), .. } if (v.value - 30.0).abs() < 1e-9
        ));
    }

    #[test]
    fn mean_and_median_exclude_unparsed_cells_and_count_them() {
        // 39 clean values (1 to 39) plus one cell that is not shaped like a number at all
        // ("n/a"): the column is still 97.5% numeric, well over the 95% a column needs to type
        // `Numeric` at all (`docs/DECISIONS.md`, D2), so the mean and median are computed over
        // the 39 readable cells, and the one unreadable cell is reported, never silently
        // dropped.
        let mut rows = vec![text_row(&["id", "montant"])];
        for i in 1..=39 {
            let montant = if i == 20 { "n/a".to_string() } else { i.to_string() };
            rows.push(vec![CellValue::Number(i as f64), CellValue::Text(montant)]);
        }
        let (workbook, inventory) = build_one(rows);

        let mean = run(
            &workbook,
            &inventory,
            Operation::Mean {
                column: "montant".into(),
                filters: vec![],
            },
        );
        let TabularOutcome::Value {
            value: TabularValue::Mean(v),
            derivation,
            ..
        } = mean
        else {
            panic!("expected a mean, got {mean:?}");
        };
        // 1..=39 without 20: sum = (1+..+39) - 20 = 780 - 20 = 760, over 38 values = 20.
        assert!((v.value - 20.0).abs() < 1e-9, "mean over the 38 readable cells only");
        assert_eq!(v.unparsed, 1);
        assert_eq!(
            derivation,
            TabularDerivation::Computed { operation: "mean".into(), row_count: 38 }
        );

        let median = run(
            &workbook,
            &inventory,
            Operation::Median {
                column: "montant".into(),
                filters: vec![],
            },
        );
        let TabularOutcome::Value {
            value: TabularValue::Median(v),
            ..
        } = median
        else {
            panic!("expected a median, got {median:?}");
        };
        // 38 sorted values (1..=19, 21..=39): the two middle ones are 19 and 21, average 20.
        assert!((v.value - 20.0).abs() < 1e-9);
        assert_eq!(v.unparsed, 1);
    }

    #[test]
    fn least_group_is_a_total_per_group_different_from_the_smallest_single_row() {
        let (workbook, inventory) = invoices();

        let least = run(
            &workbook,
            &inventory,
            Operation::LeastGroup {
                group_by: "fournisseur".into(),
                sum_column: "montant".into(),
                filters: vec![],
            },
        );
        let TabularOutcome::Value {
            value: TabularValue::LeastGroup(ranking),
            ..
        } = least
        else {
            panic!("expected a least-group ranking, got {least:?}");
        };
        // Beta: 75,00 (one row) is the smallest total, and also happens to be a single row here -
        // but Alpha (150,50 over two rows) must never be reported as smaller than Beta's own
        // smallest single row (30,00) would suggest: the ranking is a total per group throughout.
        assert_eq!(ranking.group_column, "fournisseur");
        assert_eq!(
            ranking.top,
            vec![
                GroupSum { group: "Beta".into(), sum: 75.0 },
                GroupSum { group: "Alpha".into(), sum: 150.5 },
                GroupSum { group: "Gamma".into(), sum: 500.0 },
            ]
        );

        // The smallest single row (Alpha, 30,00) is a different fact, reached only through
        // `Min`/`Sort`, never through `LeastGroup`.
        let min = run(
            &workbook,
            &inventory,
            Operation::Min {
                column: "montant".into(),
                filters: vec![],
            },
        );
        assert!(matches!(
            min,
            TabularOutcome::Value { value: TabularValue::Min(v), .. } if (v.value - 30.0).abs() < 1e-9
        ));
    }

    #[test]
    fn a_top_groups_request_within_fifty_is_not_capped() {
        let (workbook, inventory) = invoices();

        let outcome = run(
            &workbook,
            &inventory,
            Operation::TopGroups {
                group_by: "fournisseur".into(),
                sum_column: "montant".into(),
                n: 2,
                filters: vec![],
            },
        );
        let TabularOutcome::Value {
            value: TabularValue::TopGroups(top),
            ..
        } = outcome
        else {
            panic!("expected a top-groups ranking, got {outcome:?}");
        };
        assert_eq!(top.requested, 2);
        assert!(!top.capped);
        assert_eq!(
            top.top,
            vec![
                GroupSum { group: "Gamma".into(), sum: 500.0 },
                GroupSum { group: "Alpha".into(), sum: 150.5 },
            ]
        );
    }

    #[test]
    fn a_top_groups_request_beyond_fifty_is_capped_and_says_so() {
        // 60 distinct one-row groups, so the request for the top 100 has real data to be capped
        // against: the answer still holds only 50, with `requested` kept at 100 so the interface
        // can say the cap was applied, not merely that fewer than 100 groups exist.
        let mut rows = vec![text_row(&["groupe", "montant"])];
        for index in 0..60 {
            rows.push(vec![
                CellValue::Text(format!("G{index:02}")),
                CellValue::Number((index + 1) as f64),
            ]);
        }
        let (workbook, inventory) = build_one(rows);

        let outcome = run(
            &workbook,
            &inventory,
            Operation::TopGroups {
                group_by: "groupe".into(),
                sum_column: "montant".into(),
                n: 100,
                filters: vec![],
            },
        );
        let TabularOutcome::Value {
            value: TabularValue::TopGroups(top),
            ..
        } = outcome
        else {
            panic!("expected a top-groups ranking, got {outcome:?}");
        };
        assert_eq!(top.requested, 100);
        assert!(top.capped);
        assert_eq!(top.group_count, 60);
        assert_eq!(top.top.len(), MAX_TOP_GROUPS);
        // Largest total first: G59 (montant 60) leads.
        assert_eq!(top.top[0].group, "G59");
    }

    #[test]
    fn count_per_group_counts_rows_largest_first_with_ties_kept_alphabetical() {
        let (workbook, inventory) = invoices();

        let outcome = run(
            &workbook,
            &inventory,
            Operation::CountPerGroup {
                group_by: "fournisseur".into(),
                filters: vec![],
            },
        );
        let TabularOutcome::Value {
            value: TabularValue::CountPerGroup(counts),
            locator,
            derivation,
        } = outcome
        else {
            panic!("expected counts per group, got {outcome:?}");
        };
        // Alpha has 2 rows; Beta and Gamma have 1 each - a tie, kept alphabetical.
        assert_eq!(
            counts,
            vec![
                GroupCount { group: "Alpha".into(), count: 2 },
                GroupCount { group: "Beta".into(), count: 1 },
                GroupCount { group: "Gamma".into(), count: 1 },
            ]
        );
        assert_eq!(locator.column.as_deref(), Some("fournisseur"));
        assert_eq!(
            derivation,
            TabularDerivation::Computed { operation: "count_per_group".into(), row_count: 4 }
        );
    }

    #[test]
    fn a_formula_column_cannot_be_grouped_or_totalled_by_mean_median_least_or_top() {
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

        for operation in [
            Operation::Mean { column: "total".into(), filters: vec![] },
            Operation::Median { column: "total".into(), filters: vec![] },
            Operation::LeastGroup {
                group_by: "a".into(),
                sum_column: "total".into(),
                filters: vec![],
            },
            Operation::TopGroups {
                group_by: "a".into(),
                sum_column: "total".into(),
                n: 5,
                filters: vec![],
            },
        ] {
            let outcome = run(&workbook, &inventory, operation.clone());
            assert!(
                matches!(
                    outcome,
                    TabularOutcome::NotDeterministicallyAnswerable {
                        reason: NotAnswerableReason::FormulaCannotBeVerified,
                        ..
                    }
                ),
                "{operation:?} should refuse a formula column, got {outcome:?}"
            );
        }
    }
}
