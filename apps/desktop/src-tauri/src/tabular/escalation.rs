//! Tier 2 of the grounding priority chain (`docs/SELECTION-AND-MEMORY.md`): tabular data is
//! selected and no document is. Reserved there since 27 September 2026 and filled in here - an
//! instruction of the same shape as `retrieval::RETRIEVAL_INSTRUCTION`, held strictly to the
//! deterministic tabular result, never to raw rows and never to the model's own arithmetic
//! (`docs/ARCHITECTURE.md`, "deterministic before generative").
//!
//! **Not wired into `commands::sourced_answer` by this session.** Whether and how a document
//! selection and a tabular selection combine in one conversation is still open
//! (`docs/SELECTION-AND-MEMORY.md`, "documents and tables together"), and this session's own
//! brief asks for model escalation only "if the repository architecture requires it for a real
//! product surface" - today no chat surface can select a workbook at all, so there is nothing yet
//! to route into this tier. What is built here is the instruction and the evidence format that
//! wiring will need, so the decision above does not also have to invent the model-facing text
//! under pressure.

use super::engine::{TabularDerivation, TabularOutcome, TabularValue};

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
    let TabularDerivation::Computed {
        operation,
        row_count,
    } = derivation;
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

fn describe_value(value: &TabularValue) -> String {
    match value {
        TabularValue::Count(count) => count.to_string(),
        TabularValue::Sum(total) => total.to_string(),
        TabularValue::Min(min) => min.to_string(),
        TabularValue::Max(max) => max.to_string(),
        TabularValue::Distinct(values) => values.join(", "),
        TabularValue::GroupSums(sums) => sums
            .iter()
            .map(|group| format!("{}: {}", group.group, group.sum))
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
    use crate::tabular::engine::{CellText, NotAnswerableReason, RowValue, TabularLocator};

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
            value: TabularValue::Sum(725.5),
            locator: TabularLocator {
                sheet: "Feuille1".into(),
                header_row: Some(0),
                column: Some("montant".into()),
                row_range: Some((0, 3)),
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

    #[test]
    fn a_single_matched_row_is_not_pluralised() {
        let outcome = TabularOutcome::Value {
            value: TabularValue::Count(1),
            locator: TabularLocator {
                sheet: "Feuille1".into(),
                header_row: Some(0),
                column: None,
                row_range: Some((2, 2)),
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
