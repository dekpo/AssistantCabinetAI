//! The usage trace: what an answer did with each workbook the question named or concerned
//! (TABULAR plan 4.3). Data only, machine codes only: the interface turns them into a line in the
//! user's language, and a later step completes the reasons and renders them.

use serde::Serialize;

use crate::tabular::card::WorkbookCard;
use crate::tabular::sample::WorkbookSampleUse;
use crate::tabular_answer::{ImpliedWorkbook, TabularAnswer};

/// What was done with a workbook for this answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbookUseHow {
    /// Its structure (the card) was sent to the model.
    CardSent,
    /// A figure was computed from it by the engine.
    Computed,
}

/// One workbook an answer was built from, by index key (`file_key`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookUse {
    /// The index key; the interface shows the plain path.
    pub file: String,
    pub how: WorkbookUseHow,
    /// Why the workbook concerns the question when it was not named: an `ImpliedReason` code
    /// (`implied_single`, `implied_by_word`...). `None` for a workbook the question names.
    pub reason: Option<String>,
    /// The rows of the workbook the model was shown in the sample (`tabular::sample`), with the data
    /// rows of the sheets they come from. `None` when only the structure was sent (the setting is
    /// `off`, or the workbook has no data table to sample) and for a computed figure.
    pub rows_sent: Option<usize>,
    pub rows_total: Option<usize>,
}

/// The usage trace of a mixed answer: one `card_sent` entry per card, only when the model wrote an
/// answer (`model_wrote`, a turn that never reached it sent nothing), then a `computed` entry for
/// the workbook a figure was computed from. `implied` gives the reason of a workbook the question
/// did not name. `samples` says, for each workbook whose rows were sampled, how many were sent.
pub fn of_mixed_answer(
    cards: &[WorkbookCard],
    implied: &[ImpliedWorkbook],
    table: &Option<TabularAnswer>,
    model_wrote: bool,
    samples: &[WorkbookSampleUse],
) -> Vec<WorkbookUse> {
    let reason_of = |file: &str| {
        implied
            .iter()
            .find(|each| each.file == file)
            .map(|each| each.reason.as_code().to_string())
    };
    let mut trace = Vec::new();
    if model_wrote {
        for card in cards {
            let sample = samples.iter().find(|each| each.file == card.file);
            trace.push(WorkbookUse {
                file: card.file.clone(),
                how: WorkbookUseHow::CardSent,
                reason: reason_of(&card.file),
                rows_sent: sample.map(|each| each.rows_sent),
                rows_total: sample.map(|each| each.rows_total),
            });
        }
    }
    if let Some(TabularAnswer::Value { file, .. }) = table {
        trace.push(WorkbookUse {
            file: file.clone(),
            how: WorkbookUseHow::Computed,
            reason: reason_of(file),
            rows_sent: None,
            rows_total: None,
        });
    }
    trace
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabular_answer::ImpliedReason;

    fn card(file: &str) -> WorkbookCard {
        WorkbookCard {
            file: file.to_string(),
            text: String::new(),
            truncated: false,
        }
    }

    #[test]
    fn a_card_is_traced_only_when_the_model_wrote_an_answer() {
        let cards = [card("named.xlsx"), card("implied.xlsx")];
        let implied = [ImpliedWorkbook {
            file: "implied.xlsx".to_string(),
            reason: ImpliedReason::ImpliedByWord,
        }];
        let trace = of_mixed_answer(&cards, &implied, &None, true, &[]);
        assert_eq!(
            trace,
            vec![
                WorkbookUse {
                    file: "named.xlsx".to_string(),
                    how: WorkbookUseHow::CardSent,
                    reason: None,
                    rows_sent: None,
                    rows_total: None,
                },
                WorkbookUse {
                    file: "implied.xlsx".to_string(),
                    how: WorkbookUseHow::CardSent,
                    reason: Some("implied_by_word".to_string()),
                    rows_sent: None,
                    rows_total: None,
                },
            ]
        );
        assert!(of_mixed_answer(&cards, &implied, &None, false, &[]).is_empty());
    }

    #[test]
    fn the_rows_sent_are_traced_beside_the_card_of_their_workbook() {
        let cards = [card("named.xlsx"), card("other.xlsx")];
        let samples = [WorkbookSampleUse {
            file: "named.xlsx".to_string(),
            rows_sent: 12,
            rows_total: 480,
        }];
        let trace = of_mixed_answer(&cards, &[], &None, true, &samples);
        assert_eq!(
            (trace[0].rows_sent, trace[0].rows_total),
            (Some(12), Some(480))
        );
        assert_eq!((trace[1].rows_sent, trace[1].rows_total), (None, None));
        // A turn the model never read sent no rows either.
        assert!(of_mixed_answer(&cards, &[], &None, false, &samples).is_empty());
    }

    #[test]
    fn an_entry_serialises_as_machine_codes_in_camel_case() {
        let entry = WorkbookUse {
            file: "factures.xlsx".to_string(),
            how: WorkbookUseHow::CardSent,
            reason: Some("implied_single".to_string()),
            rows_sent: Some(9),
            rows_total: Some(9),
        };
        let json = serde_json::to_value(entry).expect("serialises");
        assert_eq!(json["file"], "factures.xlsx");
        assert_eq!(json["how"], "card_sent");
        assert_eq!(json["reason"], "implied_single");
        assert_eq!(json["rowsSent"], 9);
        assert_eq!(json["rowsTotal"], 9);
    }
}
