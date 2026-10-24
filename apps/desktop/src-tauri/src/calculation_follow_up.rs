//! What the user can ask under a computed figure (TABULAR step 06, owner decision 11 of 20-21 October
//! 2026): **explain the calculation**, or **analyse** the figure. Two actions, both starting from the
//! figure the engine computed and never from the model's own reading of the question.
//!
//! The interface sends the original question, the scope the answer was given under and the figure it
//! shows (`ShownCalculation`). The figure is never used: it is only compared with the one the program
//! computes again, **without any model call** (`recompute`), so a file that changed since, or a figure
//! that no longer comes out, is refused with the existing refusal rather than explained. A figure the
//! hidden interpreter had read is computed again by replaying the plan stored with it, which is
//! validated against the real workbook as the first time.
//!
//! `explain` is the one gateway call of the "Explain the calculation" button: the model is given the calculation record
//! (`tabular::escalation::format_calculation_record`) and explains the method. No document is searched,
//! nothing is embedded, nothing is recomputed by the model. "Demander a l'IA" is the writer of the mixed
//! tier with the record in its `Table results` block (`mixed_answer::ANALYSIS_ADDENDUM`); it is wired in
//! `commands`, which owns the settings and the channel.
//!
//! No `tauri::` import: the pieces the tests need are here.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::analysis_scope::ScopeMode;
use crate::data_folder::DataFolder;
use crate::error::AppError;
use crate::gateway::{ChatTurn, GatewayClient, GROUNDED_TEMPERATURE};
use crate::index_store::IndexStore;
use crate::knowledge::targeting;
use crate::number_check;
use crate::tabular::carry_over::{CarryReport, TableCarryOver};
use crate::tabular::engine::{TabularDerivation, TabularValue};
use crate::tabular_answer::{self, PendingAnswer, TabularAnswer};

/// English, profession-neutral and short, like every instruction of the product: a small model echoes a
/// long list of rules back into its answer. The record is the only source; the figure is the program's.
pub const EXPLAIN_INSTRUCTION: &str =
    "You are given the record of a calculation made by the program, not by you. Explain in plain \
     words, step by step, how the result was obtained: what was understood from the question, which \
     sheet and column, which rows were kept and which were left out, which operation was applied. \
     Never recompute, round or change a number, and add no number that is not in the record. Avoid \
     technical vocabulary. If the record shows the question may have been understood differently, \
     say so and suggest how to rephrase. Write in the language of the question, in your own words: do      not copy the lines of the record.";

/// Which of the two actions was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FollowUpKind {
    Explain,
    Analyse,
}

/// The figure on screen, as the interface keeps it. Untrusted: used to compare, and (for a figure the
/// hidden interpreter read) to take back the plan, which is validated again before it runs.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShownCalculation {
    pub file: String,
    pub value: TabularValue,
    pub derivation: TabularDerivation,
}

/// The request parameter of `ask_with_sources`: what was pressed, under which figure.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalculationFollowUp {
    pub kind: FollowUpKind,
    pub shown: ShownCalculation,
    /// The state the previous table answer had left when this figure was asked for: a figure that was
    /// a follow-up ("and for March?") does not come out of its short question alone (TABULAR step 11).
    /// Untrusted, like the rest: it is checked again exactly as at the first answer.
    #[serde(default)]
    pub table_carry_over: Option<TableCarryOver>,
}

/// The figure the program computes again for `question` under `selection` (what the answer was given
/// under, not what is ticked now), and only if it is the one on screen. The selection is narrowed to the
/// shown file first, so that a knowledge-base narrowing at the first answer cannot turn into a "which
/// workbook?" now. `document_names` is `Some` when documents were ticked as well: the figure may have
/// come from the data-only door or from the mixed tier, which reads the question beside the documents'
/// names, so both are tried. Refused with `ScopeUnavailable` when the file is gone or changed, when the
/// question no longer gives a figure, or when it gives another one.
pub fn recompute(
    question: &str,
    data: &DataFolder,
    selection: &ScopeMode,
    locale: &str,
    index: &IndexStore,
    shown: &ShownCalculation,
    document_names: Option<&[String]>,
) -> Result<TabularAnswer, AppError> {
    recompute_with_carry_over(
        question,
        data,
        selection,
        locale,
        index,
        shown,
        document_names,
        None,
    )
    .map(|(figure, _)| figure)
}

/// `recompute`, applying the carry-over the question was asked with, exactly as the tier that first
/// answered it did (TABULAR step 11).
#[allow(clippy::too_many_arguments)]
pub fn recompute_with_carry_over(
    question: &str,
    data: &DataFolder,
    selection: &ScopeMode,
    locale: &str,
    index: &IndexStore,
    shown: &ShownCalculation,
    document_names: Option<&[String]>,
    carry: Option<&TableCarryOver>,
) -> Result<(TabularAnswer, CarryReport), AppError> {
    let narrowed =
        targeting::narrowed_data_selection(data, selection, std::slice::from_ref(&shown.file));
    let mut candidates: Vec<(TabularAnswer, CarryReport)> = Vec::new();
    match &shown.derivation {
        TabularDerivation::Computed { .. } => {
            let (pending, report) = tabular_answer::prepare_with_carry_over(
                question, data, &narrowed, locale, index, false, carry,
            )?;
            candidates.push((settled(pending), report));
            if let Some(names) = document_names {
                let (pending, report) = tabular_answer::prepare_for_mixed_with_carry_over(
                    question, data, &narrowed, locale, index, names, carry,
                )?;
                candidates.push((settled(pending), report));
            }
        }
        TabularDerivation::InterpretedByModel {
            plan, model_alias, ..
        } => candidates.push((
            tabular_answer::replay_model_plan(
                question,
                data,
                &narrowed,
                locale,
                index,
                plan,
                model_alias,
            )?,
            CarryReport::default(),
        )),
    }
    candidates
        .into_iter()
        .find(|(each, _)| is_the_figure_shown(each, shown))
        .ok_or(AppError::ScopeUnavailable)
}

/// A finished answer as it is; an unfinished one (the hidden interpreter would be asked) is not a
/// figure: this path never asks it.
fn settled(pending: PendingAnswer) -> TabularAnswer {
    tabular_answer::without_the_model(pending)
}

fn is_the_figure_shown(answer: &TabularAnswer, shown: &ShownCalculation) -> bool {
    matches!(
        answer,
        TabularAnswer::Value { file, value, .. } if *file == shown.file && *value == shown.value
    )
}

/// What the "Explain the calculation" button came to. Machine data; the interface writes every sentence around it.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Explanation {
    /// The model's explanation, as written. Empty when it could not be written.
    pub text: String,
    /// The model could not be reached or wrote nothing: the figure stays and the interface says the
    /// explanation could not be written.
    pub unavailable: bool,
    /// Figures of the explanation that are in neither the record nor the question.
    pub unverified_numbers: Vec<String>,
    /// Explicit arithmetic the model wrote itself (`10 + 40 = 50`), as written.
    pub model_arithmetic: Vec<String>,
}

/// The gateway and its settings, as `explain` needs them.
pub struct ExplainContext<'a> {
    pub locale: &'a str,
    pub gateway: &'a GatewayClient,
    pub server_url: &'a str,
    pub model_alias: &'a str,
    pub idle_timeout: Duration,
}

/// The turn the model is sent: the instruction and the record, nothing else (no excerpt, no card, no
/// sample, no earlier exchange).
pub fn explain_turn(record: &str) -> String {
    format!("{EXPLAIN_INSTRUCTION}\n\n{record}")
}

/// One gateway call that explains the method of a calculation from its record. `on_delta` receives the
/// text as it is written. A gateway that cannot be reached, or an empty reply, is an `unavailable`
/// explanation, never an error: the figure on screen stays. Only the user's own stop is an error.
pub async fn explain(
    question: &str,
    record: &str,
    context: &ExplainContext<'_>,
    mut on_delta: impl FnMut(&str),
) -> Result<Explanation, AppError> {
    let turns = [ChatTurn {
        role: "user".to_string(),
        content: explain_turn(record),
    }];
    let written = context
        .gateway
        .chat_at(
            context.server_url,
            context.model_alias,
            Some(context.locale),
            &turns,
            context.idle_timeout,
            Some(GROUNDED_TEMPERATURE),
            |delta| on_delta(delta),
        )
        .await;
    let text = match written {
        Ok(text) if !text.trim().is_empty() => text,
        Err(AppError::ChatCancelled) => return Err(AppError::ChatCancelled),
        Ok(_) | Err(_) => {
            return Ok(Explanation {
                unavailable: true,
                ..Explanation::default()
            })
        }
    };
    // A figure the explanation states that the record and the question do not hold is reported beside
    // it, never edited out of it (`number_check`).
    let unverified_numbers = number_check::unsupported_numbers(&text, &[record, question], 2);
    let model_arithmetic = number_check::arithmetic_expressions(&text);
    Ok(Explanation {
        text,
        unavailable: false,
        unverified_numbers,
        model_arithmetic,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same guard every model-facing constant carries (`docs/DECISIONS.md`, "profession-neutral
    /// model-facing text").
    #[test]
    fn the_explain_instruction_stays_neutral_about_who_the_user_is() {
        let lower = EXPLAIN_INSTRUCTION.to_lowercase();
        for word in ["patient", "doctor", "practitioner", "practice", "gp"] {
            assert!(
                !lower.contains(word),
                "{word:?} found in EXPLAIN_INSTRUCTION"
            );
        }
    }

    #[test]
    fn the_explain_turn_is_the_instruction_and_the_record_and_nothing_else() {
        let turn = explain_turn("Calculation record:\nResult: 12");
        assert_eq!(
            turn,
            format!("{EXPLAIN_INSTRUCTION}\n\nCalculation record:\nResult: 12")
        );
        assert!(!turn.contains("Document excerpts"));
        assert!(!turn.contains("Workbook cards"));
    }

    #[test]
    fn the_request_reads_what_the_interface_sends() {
        let request: CalculationFollowUp = serde_json::from_str(
            r#"{"kind":"explain","shown":{"file":"commandes.xlsx",
                "value":{"kind":"sum","value":{"value":8412.5,"unit":null,"unparsed":0}},
                "derivation":{"kind":"computed","operation":"sum","row_count":41}}}"#,
        )
        .expect("parses");

        assert_eq!(request.kind, FollowUpKind::Explain);
        assert_eq!(request.shown.file, "commandes.xlsx");
        assert!(matches!(request.shown.value, TabularValue::Sum(_)));
        assert!(matches!(
            request.shown.derivation,
            TabularDerivation::Computed { row_count: 41, .. }
        ));
    }
}
