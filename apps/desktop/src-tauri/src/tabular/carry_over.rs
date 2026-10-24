//! Follow-up questions on a table ("and for March?", "and for supplier B?", "and the count?", "sort
//! them by date"): the small structured state a table answer leaves behind, and the pure rules that
//! read the next short question against it (TABULAR step 11, plan decisions 17 and 21).
//!
//! The memory is **not** the model's context. It is a handful of names and filter values (workbook,
//! sheet, column, operation, filters, grouping) that cost a few tokens whatever the model, built
//! from the operation the engine just ran. The client keeps it on the entry of the answer and sends
//! the last one back with the next question, where it is **untrusted**: `validate` checks it against
//! the real inventory, and anything that does not hold is dropped, never repaired.
//!
//! Nothing here asks a model and nothing computes: `read` turns the cues of the new question into an
//! `Operation` (or says it cannot, or asks), and `tabular::engine::execute` runs it, unchanged. A
//! question that names a workbook, carries a complete operation of its own, points at another file
//! or is long never inherits: it resets the carry-over. When two readings are possible, the answer
//! is a question, never a guess.
//!
//! No `tauri::` import. The vocabulary is the locale packs' (`follow_up_markers` and the operation
//! lists), never a word in this file.

use serde::{Deserialize, Serialize};

use super::engine::{describe_filter, AppliedFilter, Comparison, FilterSpec, Operation};
use super::inventory::{ColumnInventory, SheetInventory, TabularInventory};
use super::question::{FollowUpCues, NamedColumn, WordOperation};
use crate::file_reference::fold_text;

/// The machine name of the operation a table answer ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Count,
    Distinct,
    Sum,
    Min,
    Max,
    Mean,
    Median,
    GroupSum,
    LargestGroup,
    LeastGroup,
    TopGroups,
    CountPerGroup,
    LargestRow,
    Filter,
    Sort,
}

/// What a table answer was about, in names and filter values only. No cell value beyond the filter
/// values already shown in "Understood as".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableCarryOver {
    /// The workbook, by its index key.
    pub workbook: String,
    /// The sheet the figure was read from.
    #[serde(default)]
    pub sheet: Option<String>,
    /// The column read (summed, averaged, sorted by...); the column totalled for a group question.
    #[serde(default)]
    pub column: Option<String>,
    pub operation: OperationKind,
    #[serde(default)]
    pub filters: Vec<AppliedFilter>,
    /// The column rows are grouped by, for a group question.
    #[serde(default)]
    pub group_by: Option<String>,
    /// The N of a "top N" question.
    #[serde(default)]
    pub top_n: Option<usize>,
    /// The order of a sort.
    #[serde(default)]
    pub descending: bool,
    /// The rows the figure left out because the spreadsheet hides or filters them out
    /// (`tabular::layout::RowPolicy::Exclude`); `None` when it counted them. A short follow-up
    /// continues the same reading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_policy: Option<crate::tabular::layout::RowPolicy>,
}

/// Why a carry-over the client sent was not used. Machine codes; the interface writes the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CarryDrop {
    /// The workbook is not ticked any more.
    WorkbookNotSelected,
    /// Ticked, but never analysed, unreadable, or its analysis is gone.
    WorkbookNotAnalysed,
    /// The sheet no longer exists (or is no longer in the scope).
    SheetGone,
    /// A column it names no longer exists.
    ColumnGone,
    /// The state itself does not hold together (a date that is not a date, a missing column).
    Malformed,
}

/// What a follow-up inherited from the previous question, for the "Understood as" line to mark.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CarriedParts {
    /// The workbook is always inherited.
    pub workbook: bool,
    pub sheet: bool,
    /// The column read (or totalled).
    pub column: Option<String>,
    /// The operation was not changed by the new question.
    pub operation: Option<OperationKind>,
    pub group_by: Option<String>,
    /// The filters of the previous question that still apply, unchanged.
    pub filters: Vec<AppliedFilter>,
}

/// What `tabular_answer` learned about the carry-over while it answered one question.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CarryReport {
    /// The state this answer leaves for the next question, when it is a figure the engine computed
    /// from an operation that can be continued.
    pub next: Option<TableCarryOver>,
    /// Set when the carry-over was applied: what it supplied.
    pub inherited: Option<CarriedParts>,
    /// Set when a follow-up was recognised and the state it relied on no longer holds.
    pub dropped: Option<(String, CarryDrop)>,
}

impl TableCarryOver {
    /// The state a computed operation leaves behind. `None` for an operation that cannot be
    /// continued (a threshold on group totals, a row at a position).
    pub fn from_operation(workbook: &str, sheet: &str, operation: &Operation) -> Option<Self> {
        let made = |kind: OperationKind,
                    column: Option<&str>,
                    group_by: Option<&str>,
                    filters: &[FilterSpec]| TableCarryOver {
            workbook: workbook.to_string(),
            sheet: Some(sheet.to_string()),
            column: column.map(str::to_string),
            operation: kind,
            filters: filters.iter().map(describe_filter).collect(),
            group_by: group_by.map(str::to_string),
            top_n: None,
            descending: false,
            row_policy: None,
        };
        Some(match operation {
            Operation::Count { filters } => made(OperationKind::Count, None, None, filters),
            Operation::Distinct { column, filters } => {
                made(OperationKind::Distinct, Some(column), None, filters)
            }
            Operation::Sum { column, filters } => {
                made(OperationKind::Sum, Some(column), None, filters)
            }
            Operation::Min { column, filters } => {
                made(OperationKind::Min, Some(column), None, filters)
            }
            Operation::Max { column, filters } => {
                made(OperationKind::Max, Some(column), None, filters)
            }
            Operation::Mean { column, filters } => {
                made(OperationKind::Mean, Some(column), None, filters)
            }
            Operation::Median { column, filters } => {
                made(OperationKind::Median, Some(column), None, filters)
            }
            Operation::GroupSum {
                group_by,
                sum_column,
                filters,
            } => made(
                OperationKind::GroupSum,
                Some(sum_column),
                Some(group_by),
                filters,
            ),
            Operation::LargestGroup {
                group_by,
                sum_column,
                filters,
            } => made(
                OperationKind::LargestGroup,
                Some(sum_column),
                Some(group_by),
                filters,
            ),
            Operation::LeastGroup {
                group_by,
                sum_column,
                filters,
            } => made(
                OperationKind::LeastGroup,
                Some(sum_column),
                Some(group_by),
                filters,
            ),
            Operation::TopGroups {
                group_by,
                sum_column,
                n,
                filters,
            } => TableCarryOver {
                top_n: Some(*n),
                ..made(
                    OperationKind::TopGroups,
                    Some(sum_column),
                    Some(group_by),
                    filters,
                )
            },
            Operation::CountPerGroup { group_by, filters } => {
                made(OperationKind::CountPerGroup, None, Some(group_by), filters)
            }
            Operation::LargestRow { by_column, filters } => {
                made(OperationKind::LargestRow, Some(by_column), None, filters)
            }
            Operation::Filter { filter } => made(
                OperationKind::Filter,
                Some(&filter.column),
                None,
                std::slice::from_ref(filter),
            ),
            Operation::Sort {
                column,
                descending,
                filters,
            } => TableCarryOver {
                descending: *descending,
                ..made(OperationKind::Sort, Some(column), None, filters)
            },
            Operation::GroupsBeyond { .. } | Operation::RowAt { .. } => return None,
        })
    }

    /// The filters as the engine runs them. `None` for a state that does not hold together (a date
    /// that is not an ISO date): it is dropped, never repaired.
    pub fn specs(&self) -> Option<Vec<FilterSpec>> {
        self.filters.iter().map(spec_of).collect()
    }

    /// The operation this state describes, under `filters`. `None` when the state lacks a name its
    /// kind needs, or a list of rows is asked under other than one filter.
    pub fn to_operation(&self, filters: Vec<FilterSpec>) -> Option<Operation> {
        let column = self.column.clone();
        let group_by = self.group_by.clone();
        Some(match self.operation {
            OperationKind::Count => Operation::Count { filters },
            OperationKind::Distinct => Operation::Distinct {
                column: column?,
                filters,
            },
            OperationKind::Sum => Operation::Sum {
                column: column?,
                filters,
            },
            OperationKind::Min => Operation::Min {
                column: column?,
                filters,
            },
            OperationKind::Max => Operation::Max {
                column: column?,
                filters,
            },
            OperationKind::Mean => Operation::Mean {
                column: column?,
                filters,
            },
            OperationKind::Median => Operation::Median {
                column: column?,
                filters,
            },
            OperationKind::GroupSum => Operation::GroupSum {
                group_by: group_by?,
                sum_column: column?,
                filters,
            },
            OperationKind::LargestGroup => Operation::LargestGroup {
                group_by: group_by?,
                sum_column: column?,
                filters,
            },
            OperationKind::LeastGroup => Operation::LeastGroup {
                group_by: group_by?,
                sum_column: column?,
                filters,
            },
            OperationKind::TopGroups => Operation::TopGroups {
                group_by: group_by?,
                sum_column: column?,
                n: self.top_n?,
                filters,
            },
            OperationKind::CountPerGroup => Operation::CountPerGroup {
                group_by: group_by?,
                filters,
            },
            OperationKind::LargestRow => Operation::LargestRow {
                by_column: column?,
                filters,
            },
            OperationKind::Filter => {
                let mut filters = filters;
                if filters.len() == 1 {
                    Operation::Filter {
                        filter: filters.remove(0),
                    }
                } else {
                    // A listing under several filters: the engine's list of rows is `Sort`, which keeps the
                    // sheet's order among equal keys.
                    Operation::Sort {
                        column: column?,
                        descending: false,
                        filters,
                    }
                }
            }
            OperationKind::Sort => Operation::Sort {
                column: column?,
                descending: self.descending,
                filters,
            },
        })
    }

    /// Checks the state against the real inventory. The workbook itself (ticked, analysed) is checked
    /// by the caller, which holds the selection. The sheet and every column it names must exist.
    pub fn validate(
        &self,
        inventory: &TabularInventory,
        allowed_sheets: Option<&[String]>,
    ) -> Result<(), CarryDrop> {
        let reachable = super::engine::reachable_sheets(inventory, allowed_sheets);
        let sheet = match &self.sheet {
            Some(name) => reachable
                .iter()
                .find(|each| fold_text(&each.name) == fold_text(name))
                .copied(),
            None => match reachable.as_slice() {
                [only] => Some(*only),
                _ => None,
            },
        }
        .ok_or(CarryDrop::SheetGone)?;
        let holds = |name: &str| {
            sheet
                .columns
                .iter()
                .any(|column| fold_text(&column.name) == fold_text(name))
        };
        let named = self
            .column
            .iter()
            .chain(self.group_by.iter())
            .map(String::as_str)
            .chain(self.filters.iter().map(filter_column));
        if named.into_iter().any(|name| !holds(name)) {
            return Err(CarryDrop::ColumnGone);
        }
        let specs = self.specs().ok_or(CarryDrop::Malformed)?;
        self.to_operation(specs).ok_or(CarryDrop::Malformed)?;
        Ok(())
    }
}

fn filter_column(filter: &AppliedFilter) -> &str {
    match filter {
        AppliedFilter::Equals { column, .. }
        | AppliedFilter::Contains { column, .. }
        | AppliedFilter::GreaterThan { column, .. }
        | AppliedFilter::LessThan { column, .. }
        | AppliedFilter::Between { column, .. }
        | AppliedFilter::Weekday { column, .. }
        | AppliedFilter::Month { column, .. }
        | AppliedFilter::Year { column, .. }
        | AppliedFilter::DateRange { column, .. }
        | AppliedFilter::In { column, .. } => column,
    }
}

/// The inverse of `describe_filter`.
fn spec_of(filter: &AppliedFilter) -> Option<FilterSpec> {
    let (column, comparison) = match filter {
        AppliedFilter::Equals { column, value } => (column, Comparison::Equals(value.clone())),
        AppliedFilter::Contains { column, value } => (column, Comparison::Contains(value.clone())),
        AppliedFilter::GreaterThan { column, threshold } => {
            (column, Comparison::GreaterThan(*threshold))
        }
        AppliedFilter::LessThan { column, threshold } => (column, Comparison::LessThan(*threshold)),
        AppliedFilter::Between { column, low, high } => (column, Comparison::Between(*low, *high)),
        AppliedFilter::Weekday { column, weekday } if (1..=7).contains(weekday) => {
            (column, Comparison::Weekday(*weekday))
        }
        AppliedFilter::Month { column, month } if (1..=12).contains(month) => {
            (column, Comparison::Month(*month))
        }
        AppliedFilter::Year { column, year } => (column, Comparison::Year(*year)),
        AppliedFilter::DateRange { column, start, end } => {
            let parse = |text: &str| chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").ok();
            (column, Comparison::DateRange(parse(start)?, parse(end)?))
        }
        AppliedFilter::In { column, values } => (column, Comparison::In(values.clone())),
        _ => return None,
    };
    Some(FilterSpec {
        column: column.clone(),
        comparison,
    })
}

/// The part of the calendar or of the data a filter speaks about: a new filter replaces the old one
/// of the same slot on the same column (a new value for a filtered column, a new month for a month),
/// and a new period replaces the old one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Value,
    Number,
    Weekday,
    Month,
    Year,
    Period,
}

fn slot_of(comparison: &Comparison) -> Slot {
    match comparison {
        Comparison::Equals(_) | Comparison::Contains(_) | Comparison::In(_) => Slot::Value,
        Comparison::GreaterThan(_) | Comparison::LessThan(_) | Comparison::Between(..) => {
            Slot::Number
        }
        Comparison::Weekday(_) => Slot::Weekday,
        Comparison::Month(_) => Slot::Month,
        Comparison::Year(_) => Slot::Year,
        Comparison::DateRange(..) => Slot::Period,
    }
}

/// Whether a filter of slot `new` makes a filter of slot `old` on the same column obsolete. A period
/// ("the second quarter") replaces every calendar filter of the column; a month or a year replaces a
/// period; a month beside a year is a month of that year and both stay.
fn replaces(new: Slot, old: Slot) -> bool {
    use Slot::{Month, Period, Weekday, Year};
    new == old
        || (new == Period && matches!(old, Month | Year | Weekday))
        || (matches!(new, Month | Year) && old == Period)
}

/// The previous question's filters under the new ones: each new filter replaces what it makes
/// obsolete (same column, see `replaces`), everything else stays. Order: what stayed, then the new
/// filters in the order they were read.
pub fn merge_filters(previous: Vec<FilterSpec>, new: Vec<FilterSpec>) -> Vec<FilterSpec> {
    let mut merged: Vec<FilterSpec> = previous
        .into_iter()
        .filter(|old| {
            !new.iter().any(|fresh| {
                fold_text(&fresh.column) == fold_text(&old.column)
                    && replaces(slot_of(&fresh.comparison), slot_of(&old.comparison))
            })
        })
        .collect();
    for fresh in new {
        if !merged.contains(&fresh) {
            merged.push(fresh);
        }
    }
    merged
}

/// What the cues of a follow-up came to.
#[derive(Debug, Clone, PartialEq)]
pub enum FollowUpReading {
    /// Not a follow-up after all (a workbook named, a complete operation, another file, too long, a
    /// state that cannot be continued): the question stands alone and the carry-over is reset.
    Alone,
    /// The operation to run under the carried filters. `values_only` is true when the new question
    /// only supplies filter values (the operation is the previous one): then at least one filter must
    /// be found, or the question reads as nothing.
    Read {
        operation: Operation,
        values_only: bool,
    },
    /// Two readings fit: the interface asks which column is meant. Never a guess.
    Ask { candidates: Vec<String> },
}

/// Reads a short follow-up against the carried state. `short` is the structural length test
/// (`conversation::FOLLOW_UP_MAX_WORDS`), made by the caller on the question as typed.
pub(crate) fn read(
    carried: &TableCarryOver,
    cues: &FollowUpCues,
    sheet: &SheetInventory,
    short: bool,
) -> FollowUpReading {
    if cues.other_workbook || !short {
        return FollowUpReading::Alone;
    }
    let Some(specs) = carried.specs() else {
        return FollowUpReading::Alone;
    };
    // A complete operation of its own: an aggregate and the column it reads, or an operation and a
    // grouping with its column. The question stands alone.
    let aggregate = matches!(
        cues.operation_word,
        Some(
            WordOperation::Sum
                | WordOperation::Mean
                | WordOperation::Median
                | WordOperation::Min
                | WordOperation::Max
                | WordOperation::Distinct
        )
    );
    if (aggregate && !cues.columns.is_empty())
        || (cues.operation_word.is_some() && cues.per && cues.columns.iter().any(|c| c.group))
    {
        return FollowUpReading::Alone;
    }

    let read = |operation: Option<Operation>, values_only: bool| match operation {
        Some(operation) => FollowUpReading::Read {
            operation,
            values_only,
        },
        None => FollowUpReading::Alone,
    };

    // Sorting: the same selection, in another order.
    if cues.sort || cues.operation_word == Some(WordOperation::Sort) {
        let column = match cues.columns.as_slice() {
            [one] => one.name.clone(),
            [] => match (&carried.operation, &carried.column) {
                (OperationKind::Sort, Some(column)) => column.clone(),
                _ => return FollowUpReading::Alone,
            },
            many => return ask(many),
        };
        return read(
            Some(Operation::Sort {
                column,
                descending: cues.descending,
                filters: specs,
            }),
            false,
        );
    }

    // "By <column>": a grouping added to the same measure, in place of any filter on that column.
    if cues.per && cues.operation_word.is_none() {
        let groups: Vec<&NamedColumn> = cues.columns.iter().filter(|c| c.group).collect();
        let group = match groups.as_slice() {
            [one] => one.name.clone(),
            [] => return FollowUpReading::Alone,
            many => return ask_named(many),
        };
        // "By supplier" after a question about one supplier breaks the figure down by supplier: the filter
        // on the column that becomes the grouping is dropped, the others stay.
        let specs: Vec<FilterSpec> = specs
            .into_iter()
            .filter(|spec| fold_text(&spec.column) != fold_text(&group))
            .collect();
        let operation = match carried.operation {
            OperationKind::Count | OperationKind::CountPerGroup => Some(Operation::CountPerGroup {
                group_by: group,
                filters: specs,
            }),
            OperationKind::Sum | OperationKind::GroupSum => {
                carried
                    .column
                    .clone()
                    .map(|sum_column| Operation::GroupSum {
                        group_by: group,
                        sum_column,
                        filters: specs,
                    })
            }
            OperationKind::LargestGroup => {
                carried
                    .column
                    .clone()
                    .map(|sum_column| Operation::LargestGroup {
                        group_by: group,
                        sum_column,
                        filters: specs,
                    })
            }
            OperationKind::LeastGroup => {
                carried
                    .column
                    .clone()
                    .map(|sum_column| Operation::LeastGroup {
                        group_by: group,
                        sum_column,
                        filters: specs,
                    })
            }
            OperationKind::TopGroups => {
                carried
                    .column
                    .clone()
                    .zip(carried.top_n)
                    .map(|(sum_column, n)| Operation::TopGroups {
                        group_by: group,
                        sum_column,
                        n,
                        filters: specs,
                    })
            }
            _ => None,
        };
        return read(operation, false);
    }

    // "And the count / the total / the average": the operation changes, the filters stay.
    if let Some(word) = cues.operation_word {
        let measure = |cues: &FollowUpCues| -> Result<String, FollowUpReading> {
            let numeric: Vec<&NamedColumn> = cues.columns.iter().filter(|c| c.numeric).collect();
            match numeric.as_slice() {
                [one] => return Ok(one.name.clone()),
                [] => {}
                many => return Err(ask_named(many)),
            }
            let carried_measure = match carried.operation {
                OperationKind::Sum
                | OperationKind::Min
                | OperationKind::Max
                | OperationKind::Mean
                | OperationKind::Median
                | OperationKind::GroupSum
                | OperationKind::LargestGroup
                | OperationKind::LeastGroup
                | OperationKind::TopGroups
                | OperationKind::LargestRow => carried.column.clone(),
                _ => None,
            };
            if let Some(column) = carried_measure {
                return Ok(column);
            }
            let candidates = measure_candidates(sheet);
            match candidates.as_slice() {
                [only] => Ok(only.clone()),
                [] => Err(FollowUpReading::Alone),
                _ => Err(FollowUpReading::Ask { candidates }),
            }
        };
        let operation = match word {
            WordOperation::Count => match &carried.group_by {
                Some(group_by) => Operation::CountPerGroup {
                    group_by: group_by.clone(),
                    filters: specs,
                },
                None => Operation::Count { filters: specs },
            },
            WordOperation::Sum => match measure(cues) {
                Ok(column) => match &carried.group_by {
                    Some(group_by) => Operation::GroupSum {
                        group_by: group_by.clone(),
                        sum_column: column,
                        filters: specs,
                    },
                    None => Operation::Sum {
                        column,
                        filters: specs,
                    },
                },
                Err(reading) => return reading,
            },
            WordOperation::Mean => match measure(cues) {
                Ok(column) => Operation::Mean {
                    column,
                    filters: specs,
                },
                Err(reading) => return reading,
            },
            WordOperation::Median => match measure(cues) {
                Ok(column) => Operation::Median {
                    column,
                    filters: specs,
                },
                Err(reading) => return reading,
            },
            WordOperation::Min => match measure(cues) {
                Ok(column) => Operation::Min {
                    column,
                    filters: specs,
                },
                Err(reading) => return reading,
            },
            WordOperation::Max => match measure(cues) {
                Ok(column) => Operation::Max {
                    column,
                    filters: specs,
                },
                Err(reading) => return reading,
            },
            WordOperation::Distinct => match (cues.columns.as_slice(), &carried.column) {
                ([one], _) => Operation::Distinct {
                    column: one.name.clone(),
                    filters: specs,
                },
                ([], Some(column)) => Operation::Distinct {
                    column: column.clone(),
                    filters: specs,
                },
                _ => return FollowUpReading::Alone,
            },
            WordOperation::Sort => return FollowUpReading::Alone,
        };
        return read(Some(operation), false);
    }

    // No operation of its own: only new values for the same operation.
    read(carried.to_operation(specs), true)
}

fn ask(columns: &[NamedColumn]) -> FollowUpReading {
    ask_named(&columns.iter().collect::<Vec<_>>())
}

fn ask_named(columns: &[&NamedColumn]) -> FollowUpReading {
    FollowUpReading::Ask {
        candidates: columns.iter().map(|column| column.name.clone()).collect(),
    }
}

/// The numeric columns a total or an average could read: not a year, not a formula column.
fn measure_candidates(sheet: &SheetInventory) -> Vec<String> {
    sheet
        .columns
        .iter()
        .filter(|column| is_measure(column))
        .map(|column| column.name.clone())
        .collect()
}

fn is_measure(column: &ColumnInventory) -> bool {
    column.inferred_type == super::inventory::ColumnType::Numeric
        && !column.has_formulas
        && !column.year_like
}

/// What the follow-up took from the previous question, compared with the state it leaves: the
/// column, the operation and the grouping when unchanged, and the previous filters that still apply.
pub fn inherited_parts(carried: &TableCarryOver, next: &TableCarryOver) -> CarriedParts {
    CarriedParts {
        workbook: true,
        sheet: carried.sheet.is_some() && carried.sheet == next.sheet,
        column: carried
            .column
            .clone()
            .filter(|column| next.column.as_ref() == Some(column)),
        operation: (carried.operation == next.operation).then_some(carried.operation),
        group_by: carried
            .group_by
            .clone()
            .filter(|group| next.group_by.as_ref() == Some(group)),
        filters: carried
            .filters
            .iter()
            .filter(|filter| next.filters.contains(filter))
            .cloned()
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn equals(column: &str, value: &str) -> FilterSpec {
        FilterSpec {
            column: column.into(),
            comparison: Comparison::Equals(value.into()),
        }
    }

    fn month(column: &str, month: u8) -> FilterSpec {
        FilterSpec {
            column: column.into(),
            comparison: Comparison::Month(month),
        }
    }

    fn year(column: &str, year: i32) -> FilterSpec {
        FilterSpec {
            column: column.into(),
            comparison: Comparison::Year(year),
        }
    }

    #[test]
    fn a_new_value_for_a_filtered_column_replaces_that_filter() {
        let merged = merge_filters(
            vec![equals("fournisseur", "Alpha")],
            vec![equals("fournisseur", "Beta")],
        );
        assert_eq!(merged, vec![equals("fournisseur", "Beta")]);
    }

    #[test]
    fn a_value_on_another_column_is_added() {
        let merged = merge_filters(
            vec![equals("fournisseur", "Alpha")],
            vec![equals("client", "Omega")],
        );
        assert_eq!(
            merged,
            vec![equals("fournisseur", "Alpha"), equals("client", "Omega")]
        );
    }

    #[test]
    fn a_new_month_replaces_the_previous_month_and_keeps_the_year() {
        let merged = merge_filters(
            vec![month("date", 2), year("date", 2026)],
            vec![month("date", 3)],
        );
        assert_eq!(merged, vec![year("date", 2026), month("date", 3)]);
    }

    #[test]
    fn a_period_replaces_every_calendar_filter_of_the_column() {
        let period = FilterSpec {
            column: "date".into(),
            comparison: Comparison::DateRange(
                chrono::NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(),
            ),
        };
        let merged = merge_filters(
            vec![
                month("date", 2),
                year("date", 2026),
                equals("fournisseur", "Alpha"),
            ],
            vec![period.clone()],
        );
        assert_eq!(merged, vec![equals("fournisseur", "Alpha"), period]);
    }

    #[test]
    fn an_applied_filter_survives_the_round_trip_to_the_engine() {
        let period = FilterSpec {
            column: "date".into(),
            comparison: Comparison::DateRange(
                chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                chrono::NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
            ),
        };
        let carried = TableCarryOver::from_operation(
            "factures.csv",
            "factures",
            &Operation::Sum {
                column: "montant".into(),
                filters: vec![
                    period.clone(),
                    equals("fournisseur", "Beta"),
                    month("date", 3),
                ],
            },
        )
        .unwrap();
        assert_eq!(
            carried.specs().unwrap(),
            vec![period, equals("fournisseur", "Beta"), month("date", 3)]
        );
    }

    #[test]
    fn a_date_that_is_not_a_date_is_malformed_not_repaired() {
        let carried = TableCarryOver {
            workbook: "factures.csv".into(),
            sheet: None,
            column: Some("montant".into()),
            operation: OperationKind::Sum,
            filters: vec![AppliedFilter::DateRange {
                column: "date".into(),
                start: "not a date".into(),
                end: "2026-03-31".into(),
            }],
            group_by: None,
            top_n: None,
            descending: false,
            row_policy: None,
        };
        assert!(carried.specs().is_none());
    }

    #[test]
    fn a_threshold_on_group_totals_and_a_row_position_leave_no_state() {
        assert!(TableCarryOver::from_operation(
            "a.csv",
            "a",
            &Operation::GroupsBeyond {
                group_by: "g".into(),
                sum_column: "m".into(),
                threshold: 1.0,
                above: true,
                filters: Vec::new(),
            }
        )
        .is_none());
        assert!(
            TableCarryOver::from_operation("a.csv", "a", &Operation::RowAt { index: 0 }).is_none()
        );
    }

    #[test]
    fn the_wire_form_carries_names_and_filter_values_only() {
        let carried = TableCarryOver::from_operation(
            "factures.csv",
            "factures",
            &Operation::Sum {
                column: "montant".into(),
                filters: vec![equals("fournisseur", "Beta")],
            },
        )
        .unwrap();
        let wire = serde_json::to_value(&carried).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({
                "workbook": "factures.csv",
                "sheet": "factures",
                "column": "montant",
                "operation": "sum",
                "filters": [{"kind": "equals", "column": "fournisseur", "value": "Beta"}],
                "groupBy": null,
                "topN": null,
                "descending": false
            })
        );
        let back: TableCarryOver = serde_json::from_value(wire).unwrap();
        assert_eq!(back, carried);
    }
}
