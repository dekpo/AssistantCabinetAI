//! Tier 2 of the grounding priority chain (`docs/SELECTION-AND-MEMORY.md`): tabular data is
//! selected and no document is. Reserved there since 27 September 2026 and filled in here - an
//! instruction of the same shape as `retrieval::RETRIEVAL_INSTRUCTION`, held strictly to the
//! deterministic tabular result, never to raw rows and never to the model's own arithmetic
//! (`docs/ARCHITECTURE.md`, "deterministic before generative").
//!
//! **Still not sent to any model.** Tier 2 is routed since the tabular UI session
//! (`crate::tabular_answer`), but it answers from the engine alone: a computed value, a
//! structural fact, or a nudge naming the real columns, with no gateway call at all. This
//! instruction and evidence format are kept for the session that combines a document selection
//! with a tabular one (`docs/SELECTION-AND-MEMORY.md`, "documents and tables together"), where the
//! model writes and the engines compute - so that session does not have to invent the
//! model-facing text under pressure.

use super::engine::{
    AppliedFilter, TabularDerivation, TabularLocator, TabularOutcome, TabularValue,
};

/// English, like every instruction (`docs/LANGUAGE-AND-LOCALE.md`); short, for the same reason
/// `NO_DOCUMENTS_INSTRUCTION` is short - small models echo long rule lists back into their
/// answers; and, like every model-facing string in this product, silent on who the user is or
/// what they do (`docs/DECISIONS.md`, "profession-neutral model-facing text").
pub const TABULAR_INSTRUCTION: &str =
    "You are given a result computed directly from the user's own data table, not by you. \
     Answer only from this result, citing the sheet and column it came from. Never recompute, \
     re-derive or second-guess the number: report it exactly as given. If the result says the \
     answer could not be established, say so instead of guessing.";

/// The instruction plus the outcome, as one turn ready to send - the tabular counterpart of
/// `retrieval::build_context_turn`. `None` when the outcome carries nothing safe to hand a model
/// (`format_evidence`).
pub fn build_context_turn(outcome: &TabularOutcome) -> Option<String> {
    format_evidence(outcome).map(|evidence| format!("{TABULAR_INSTRUCTION}\n\n{evidence}"))
}

/// The outcome alone, as text a model can read.
///
/// `None` for `TabularValue::Rows`: a raw row list is exactly the "whole workbook, just in case"
/// this pipeline exists to avoid handing over (`docs/ARCHITECTURE.md`), so `filter` and `sort`
/// results never reach a model through this function - they are for the application's own use
/// (a table rendered in the interface), not for evidence. `None` also for
/// `NotDeterministicallyAnswerable`: there is nothing to escalate, and the caller answers from
/// its `reason` and `available_columns`/`available_sheets` directly, with no model turn at all.
pub fn format_evidence(outcome: &TabularOutcome) -> Option<String> {
    let TabularOutcome::Value {
        value,
        locator,
        derivation,
    } = outcome
    else {
        return None;
    };
    if matches!(value, TabularValue::Rows(_)) {
        return None;
    }
    // `engine::execute` - the only place a `TabularOutcome` is built - only ever produces
    // `Computed`: `InterpretedByModel` is wrapped on afterward, by `tabular_answer`, once the
    // outcome has already left this function's caller. Read both the same way regardless, so
    // this stays correct if that ever changes rather than silently dropping evidence.
    let (operation, row_count) = match derivation {
        TabularDerivation::Computed { operation, row_count }
        | TabularDerivation::InterpretedByModel { operation, row_count, .. } => (operation, row_count),
    };
    let column = locator
        .column
        .as_deref()
        .map(|name| format!(", column {name}"))
        .unwrap_or_default();
    let plural = if *row_count == 1 { "" } else { "s" };
    let described = describe_value(value);
    Some(format!(
        "Sheet {}{column}: {operation} = {described} (computed over {row_count} row{plural})",
        locator.sheet,
    ))
}

/// What the program holds about one computed figure, the input of `format_calculation_record`
/// (TABULAR plan 4.6). Borrowed from the engine's own outcome; `data_rows` is the inventory's count of
/// data rows of the sheet the figure was read from, when it is known.
pub struct CalculationRecord<'a> {
    /// The user's question as written. Left out when the turn already carries it.
    pub question: Option<&'a str>,
    /// The workbook as the interface names it (the plain path and, for the other folder, where it
    /// lies): `tabular::card::file_label`.
    pub workbook: &'a str,
    pub data_rows: Option<usize>,
    pub value: &'a TabularValue,
    pub locator: &'a TabularLocator,
    pub derivation: &'a TabularDerivation,
}

/// The record of a calculation, as text for the model: what was understood from the question, the sheet
/// and the column, the operation, the rows kept and left out, and the result. **No cell value and no
/// row** (only the filter values the user wrote or the engine resolved, the same ones the "Understood
/// as" line shows, and the group labels `format_evidence` already sends); every number in it comes from
/// the engine. `None` for a list of rows, which never reaches a model.
pub fn format_calculation_record(record: &CalculationRecord<'_>) -> Option<String> {
    if matches!(record.value, TabularValue::Rows(_)) {
        return None;
    }
    let (operation, row_count, model_alias) = match record.derivation {
        TabularDerivation::Computed {
            operation,
            row_count,
        } => (operation, *row_count, None),
        TabularDerivation::InterpretedByModel {
            operation,
            row_count,
            model_alias,
            ..
        } => (operation, *row_count, Some(model_alias.as_str())),
    };
    let locator = record.locator;
    let mut lines = vec!["Calculation record:".to_string()];
    if let Some(question) = record.question {
        lines.push(format!("Question: {question}"));
    }
    let header = match locator.header_row {
        Some(row) => format!("header on row {}", row + 1),
        None => "no header row detected".to_string(),
    };
    lines.push(format!(
        "Workbook {}, sheet \"{}\" ({header})",
        record.workbook, locator.sheet
    ));
    let unit = numeric_of(record.value)
        .and_then(|aggregate| aggregate.unit.as_deref())
        .map(|unit| format!(" ({unit})"))
        .unwrap_or_default();
    lines.push(format!(
        "Operation: {}{unit}",
        describe_operation(operation, locator.column.as_deref())
    ));
    if let Some(alias) = model_alias {
        lines.push(format!(
            "Reading of the question: proposed by the language model {alias}, then checked and \
             computed by the program"
        ));
    }
    if !locator.filters.is_empty() {
        let filters: Vec<String> = locator
            .filters
            .iter()
            .map(describe_applied_filter)
            .collect();
        lines.push(format!("Understood as: {}", filters.join("; ")));
    }
    let plural = if row_count == 1 { "" } else { "s" };
    lines.push(match record.data_rows {
        Some(total) => format!("Rows: {row_count} kept out of {total} data rows"),
        None => format!("Rows: {row_count} row{plural} kept"),
    });
    if let Some(aggregate) = numeric_of(record.value) {
        if aggregate.unparsed > 0 {
            let column = locator
                .column
                .as_deref()
                .map(|name| format!(" \"{name}\""))
                .unwrap_or_default();
            lines.push(format!(
                "Left out: {} values in column{column} that could not be read as numbers (counted \
                 over the whole column)",
                aggregate.unparsed
            ));
        }
    }
    lines.push(format!("Result: {}", describe_value(record.value)));
    Some(lines.join("\n"))
}

fn numeric_of(value: &TabularValue) -> Option<&super::engine::NumericAggregate> {
    match value {
        TabularValue::Sum(aggregate)
        | TabularValue::Min(aggregate)
        | TabularValue::Max(aggregate)
        | TabularValue::Mean(aggregate)
        | TabularValue::Median(aggregate) => Some(aggregate),
        _ => None,
    }
}

/// The operation in words, from the engine's machine name; a name this function does not know is
/// written as it is, never dropped.
fn describe_operation(operation: &str, column: Option<&str>) -> String {
    let of_column = column
        .map(|name| format!(" of column \"{name}\""))
        .unwrap_or_default();
    match operation {
        "sum" => format!("sum{of_column}"),
        "mean" => format!("average{of_column}"),
        "median" => format!("median{of_column}"),
        "min" => format!("smallest value{of_column}"),
        "max" => format!("largest value{of_column}"),
        "count" => "count of rows".to_string(),
        "distinct" => format!("distinct values{of_column}"),
        "largest_group" => format!("group with the largest total{of_column}"),
        "least_group" => format!("group with the smallest total{of_column}"),
        "group_sum" => format!("total per group{of_column}"),
        "top_n" => format!("groups with the largest totals{of_column}"),
        "groups_beyond" => format!("groups whose total is beyond a threshold{of_column}"),
        "count_per_group" => "count of rows per group".to_string(),
        other => format!("{other}{of_column}"),
    }
}

fn describe_applied_filter(filter: &AppliedFilter) -> String {
    match filter {
        AppliedFilter::Equals { column, value } => format!("{column} = \"{value}\""),
        AppliedFilter::Contains { column, value } => format!("{column} contains \"{value}\""),
        AppliedFilter::GreaterThan { column, threshold } => format!("{column} > {threshold}"),
        AppliedFilter::LessThan { column, threshold } => format!("{column} < {threshold}"),
        AppliedFilter::Between { column, low, high } => {
            format!("{column} between {low} and {high}")
        }
        AppliedFilter::Weekday { column, weekday } => {
            format!("{column} falls on a {}", weekday_name(*weekday))
        }
        AppliedFilter::Month { column, month } => format!("{column} in month {month}"),
        AppliedFilter::Year { column, year } => format!("{column} in year {year}"),
        AppliedFilter::DateRange { column, start, end } => {
            format!("{column} between {start} and {end}")
        }
        AppliedFilter::In { column, values } => {
            let listed: Vec<String> = values.iter().map(|value| format!("\"{value}\"")).collect();
            format!("{column} is one of {}", listed.join(", "))
        }
    }
}

fn weekday_name(weekday: u8) -> &'static str {
    match weekday {
        1 => "Monday",
        2 => "Tuesday",
        3 => "Wednesday",
        4 => "Thursday",
        5 => "Friday",
        6 => "Saturday",
        _ => "Sunday",
    }
}

fn describe_value(value: &TabularValue) -> String {
    match value {
        TabularValue::Count(count) => count.to_string(),
        TabularValue::Sum(total) => total.value.to_string(),
        TabularValue::Min(min) => min.value.to_string(),
        TabularValue::Max(max) => max.value.to_string(),
        TabularValue::Mean(mean) => mean.value.to_string(),
        TabularValue::Median(median) => median.value.to_string(),
        TabularValue::Distinct(values) => values.join(", "),
        TabularValue::GroupSums(sums) => sums
            .iter()
            .map(|group| format!("{}: {}", group.group, group.sum))
            .collect::<Vec<_>>()
            .join("; "),
        TabularValue::LargestGroup(ranking) | TabularValue::LeastGroup(ranking) => ranking
            .top
            .iter()
            .map(|group| format!("{} {}: {}", ranking.group_column, group.group, group.sum))
            .collect::<Vec<_>>()
            .join("; "),
        TabularValue::TopGroups(top) => top
            .top
            .iter()
            .map(|group| format!("{} {}: {}", top.group_column, group.group, group.sum))
            .collect::<Vec<_>>()
            .join("; "),
        TabularValue::GroupsBeyond(beyond) => {
            let side = if beyond.above { "above" } else { "below" };
            let listed = beyond
                .matches
                .iter()
                .map(|group| format!("{} {}: {}", beyond.group_column, group.group, group.sum))
                .collect::<Vec<_>>()
                .join("; ");
            if listed.is_empty() {
                format!(
                    "no {} has a total {side} {} (nearest: {} {}: {})",
                    beyond.group_column, beyond.threshold, beyond.group_column, beyond.extreme.group, beyond.extreme.sum
                )
            } else {
                format!("totals {side} {}: {listed}", beyond.threshold)
            }
        }
        TabularValue::CountPerGroup(counts) => counts
            .iter()
            .map(|group| format!("{}: {}", group.group, group.count))
            .collect::<Vec<_>>()
            .join("; "),
        TabularValue::LargestRow(row) | TabularValue::Row(row) => row
            .cells
            .iter()
            .map(|cell| format!("{}: {}", cell.column, cell.text))
            .collect::<Vec<_>>()
            .join(", "),
        TabularValue::Rows(_) => {
            unreachable!("format_evidence never reaches this arm for Rows")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabular::engine::{
        CellText, NotAnswerableReason, NumericAggregate, RowValue, TabularLocator,
    };

    /// The same guard `retrieval::the_retrieval_instruction_stays_neutral_about_who_the_user_is`
    /// and `conversation::the_no_documents_instruction_stays_neutral_about_who_the_user_is`
    /// carry, for this tier's instruction (`docs/DECISIONS.md`, "profession-neutral model-facing
    /// text" - a standing rule for every new model-facing constant, not a one-time cleanup).
    #[test]
    fn the_tabular_instruction_stays_neutral_about_who_the_user_is() {
        let lower = TABULAR_INSTRUCTION.to_lowercase();
        for word in ["patient", "doctor", "practitioner", "practice", "gp"] {
            assert!(!lower.contains(word), "{word:?} found in TABULAR_INSTRUCTION");
        }
    }

    fn sum_outcome() -> TabularOutcome {
        TabularOutcome::Value {
            value: TabularValue::Sum(NumericAggregate {
                value: 725.5,
                unit: None,
                unparsed: 0,
            }),
            locator: TabularLocator {
                sheet: "Feuille1".into(),
                header_row: Some(0),
                column: Some("montant".into()),
                row_range: Some((0, 3)),
                filters: Vec::new(),
            },
            derivation: TabularDerivation::Computed {
                operation: "sum".into(),
                row_count: 4,
            },
        }
    }

    #[test]
    fn a_computed_value_becomes_a_model_readable_turn() {
        let turn = build_context_turn(&sum_outcome()).expect("a value has evidence");

        assert!(turn.starts_with(TABULAR_INSTRUCTION));
        assert!(turn.contains("Feuille1"));
        assert!(turn.contains("montant"));
        assert!(turn.contains("sum"));
        assert!(turn.contains("725.5"));
        assert!(turn.contains("4 rows"));
    }

    #[test]
    fn raw_rows_are_never_turned_into_evidence() {
        let outcome = TabularOutcome::Value {
            value: TabularValue::Rows(vec![RowValue {
                row_index: 0,
                cells: vec![CellText {
                    column: "nom".into(),
                    text: "Camille".into(),
                }],
            }]),
            locator: TabularLocator {
                sheet: "Feuille1".into(),
                header_row: Some(0),
                column: Some("nom".into()),
                row_range: Some((0, 0)),
                filters: Vec::new(),
            },
            derivation: TabularDerivation::Computed {
                operation: "filter".into(),
                row_count: 1,
            },
        };

        assert_eq!(format_evidence(&outcome), None);
        assert_eq!(build_context_turn(&outcome), None);
    }

    #[test]
    fn a_refusal_has_nothing_to_escalate() {
        let outcome = TabularOutcome::NotDeterministicallyAnswerable {
            reason: NotAnswerableReason::ColumnNotFound,
            available_sheets: vec!["Feuille1".into()],
            available_columns: vec!["nom".into()],
        };

        assert_eq!(format_evidence(&outcome), None);
        assert_eq!(build_context_turn(&outcome), None);
    }

    fn record_of<'a>(
        value: &'a TabularValue,
        locator: &'a TabularLocator,
        derivation: &'a TabularDerivation,
    ) -> CalculationRecord<'a> {
        CalculationRecord {
            question: Some("What is the total of the orders?"),
            workbook: "\"orders.xlsx\"",
            data_rows: Some(124),
            value,
            locator,
            derivation,
        }
    }

    fn filtered_sum() -> (TabularValue, TabularLocator, TabularDerivation) {
        (
            TabularValue::Sum(NumericAggregate {
                value: 8412.5,
                unit: Some("EUR".into()),
                unparsed: 2,
            }),
            TabularLocator {
                sheet: "Commandes".into(),
                header_row: Some(0),
                column: Some("Montant TTC".into()),
                row_range: Some((3, 90)),
                filters: vec![
                    AppliedFilter::Equals {
                        column: "Statut".into(),
                        value: "a payer".into(),
                    },
                    AppliedFilter::DateRange {
                        column: "Date".into(),
                        start: "2026-01-01".into(),
                        end: "2026-03-31".into(),
                    },
                ],
            },
            TabularDerivation::Computed {
                operation: "sum".into(),
                row_count: 41,
            },
        )
    }

    #[test]
    fn the_record_states_the_question_the_workbook_the_sheet_and_the_header_row() {
        let (value, locator, derivation) = filtered_sum();
        let text = format_calculation_record(&record_of(&value, &locator, &derivation)).unwrap();

        assert!(
            text.starts_with("Calculation record:\nQuestion: What is the total of the orders?\n")
        );
        assert!(text.contains("Workbook \"orders.xlsx\", sheet \"Commandes\" (header on row 1)"));
    }

    #[test]
    fn the_record_names_the_operation_the_column_and_the_unit() {
        let (value, locator, derivation) = filtered_sum();
        let text = format_calculation_record(&record_of(&value, &locator, &derivation)).unwrap();

        assert!(text.contains("Operation: sum of column \"Montant TTC\" (EUR)"));
    }

    #[test]
    fn the_record_lists_the_filters_the_value_was_computed_under() {
        let (value, locator, derivation) = filtered_sum();
        let text = format_calculation_record(&record_of(&value, &locator, &derivation)).unwrap();

        assert!(text.contains(
            "Understood as: Statut = \"a payer\"; Date between 2026-01-01 and 2026-03-31"
        ));
    }

    #[test]
    fn an_unfiltered_value_has_no_understood_as_line() {
        let (value, mut locator, derivation) = filtered_sum();
        locator.filters.clear();
        let text = format_calculation_record(&record_of(&value, &locator, &derivation)).unwrap();

        assert!(!text.contains("Understood as"));
    }

    #[test]
    fn the_record_counts_the_rows_kept_out_of_the_data_rows() {
        let (value, locator, derivation) = filtered_sum();
        let text = format_calculation_record(&record_of(&value, &locator, &derivation)).unwrap();

        assert!(text.contains("Rows: 41 kept out of 124 data rows"));

        let mut unknown = record_of(&value, &locator, &derivation);
        unknown.data_rows = None;
        let text = format_calculation_record(&unknown).unwrap();
        assert!(text.contains("Rows: 41 rows kept"));
        assert!(!text.contains("out of"));
    }

    #[test]
    fn the_record_says_what_was_left_out_as_unreadable() {
        let (value, locator, derivation) = filtered_sum();
        let text = format_calculation_record(&record_of(&value, &locator, &derivation)).unwrap();

        assert!(text.contains(
            "Left out: 2 values in column \"Montant TTC\" that could not be read as numbers"
        ));

        let clean = TabularValue::Sum(NumericAggregate {
            value: 8412.5,
            unit: None,
            unparsed: 0,
        });
        let text = format_calculation_record(&record_of(&clean, &locator, &derivation)).unwrap();
        assert!(!text.contains("Left out"));
    }

    #[test]
    fn the_record_ends_with_the_engines_result() {
        let (value, locator, derivation) = filtered_sum();
        let text = format_calculation_record(&record_of(&value, &locator, &derivation)).unwrap();

        assert!(text.ends_with("Result: 8412.5"));
    }

    #[test]
    fn a_value_computed_from_the_models_plan_says_so_and_names_the_model() {
        let (value, locator, _) = filtered_sum();
        let derivation = TabularDerivation::InterpretedByModel {
            operation: "sum".into(),
            row_count: 41,
            model_alias: "cabinet-chat".into(),
            plan: serde_json::json!({"aggregate": {"op": "sum", "column": "Montant TTC"}}),
        };
        let text = format_calculation_record(&record_of(&value, &locator, &derivation)).unwrap();

        assert!(text.contains("proposed by the language model cabinet-chat"));
        assert!(text.contains("checked and computed by the program"));
        // The plan itself is the model's JSON: it is not part of the record.
        assert!(!text.contains("aggregate"));
    }

    #[test]
    fn the_record_carries_neither_a_cell_value_nor_a_row() {
        let (_, locator, derivation) = filtered_sum();
        let row = TabularValue::Rows(vec![RowValue {
            row_index: 0,
            cells: vec![CellText {
                column: "Fournisseur".into(),
                text: "Zorglub Industries".into(),
            }],
        }]);
        assert_eq!(
            format_calculation_record(&record_of(&row, &locator, &derivation)),
            None
        );

        // A figure whose column holds a distinctive name in its cells: the record never reads cells.
        let (value, _, _) = filtered_sum();
        let text = format_calculation_record(&record_of(&value, &locator, &derivation)).unwrap();
        assert!(!text.contains("Zorglub"));
    }

    #[test]
    fn a_single_matched_row_is_not_pluralised() {
        let outcome = TabularOutcome::Value {
            value: TabularValue::Count(1),
            locator: TabularLocator {
                sheet: "Feuille1".into(),
                header_row: Some(0),
                column: None,
                row_range: Some((2, 2)),
                filters: Vec::new(),
            },
            derivation: TabularDerivation::Computed {
                operation: "count".into(),
                row_count: 1,
            },
        };

        let evidence = format_evidence(&outcome).unwrap();
        assert!(evidence.contains("1 row"));
        assert!(!evidence.contains("1 rows"));
    }
}
