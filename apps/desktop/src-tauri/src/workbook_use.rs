//! The usage trace: what an answer did with each workbook the question named, implied or ticked
//! (TABULAR plan 4.3). Data only, machine codes only: the interface turns them into a line in the
//! user's language (`src/lib/workbookUse.ts`).
//!
//! Principle: a file the user named never disappears without a visible line saying what was done
//! with it. `trace` is the one place every door of `commands::sourced_answer` builds its entries.

use serde::Serialize;

use crate::tabular::card::WorkbookCard;
use crate::tabular::sample::WorkbookSampleUse;
use crate::tabular::structural::StructuralAnswer;
use crate::tabular_answer::{
    ImpliedReason, ImpliedWorkbook, SelectionFacts, TabularAnswer, Unusable,
};

/// What was done with a workbook for this answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbookUseHow {
    /// Its structure (the card) was sent to the model.
    CardSent,
    /// The engine answered from it: a figure, a structural fact, rows.
    Computed,
    /// A mail-merge plan was prepared from its rows (door 1).
    PlanPrepared,
    /// It was named, ticked or implied and did not serve; `reason` says why.
    NotUsed,
}

/// Why a workbook concerns the question without being named, or why it did not serve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UseReason {
    ImpliedSingle,
    ImpliedByWord,
    ImpliedByColumn,
    ImpliedByEntity,
    /// The question names a workbook of the folder that is not ticked.
    NamedNotSelected,
    /// Ticked, and never analysed (or its analysis is gone).
    NotAnalysed,
    /// Ticked, analysed, and nothing in it looks like a table.
    Unreadable,
    /// Ticked, then changed or removed.
    ChangedSinceAnalysis,
    /// The name matches more than one ticked workbook.
    AmbiguousName,
    /// Ticked and neither named nor implied: one aggregate entry with a `count`.
    NotRelevant,
    /// Its card was built but the model wrote no answer, so it never read it.
    NotSent,
}

impl From<ImpliedReason> for UseReason {
    fn from(reason: ImpliedReason) -> Self {
        match reason {
            ImpliedReason::ImpliedSingle => Self::ImpliedSingle,
            ImpliedReason::ImpliedByWord => Self::ImpliedByWord,
            ImpliedReason::ImpliedByColumn => Self::ImpliedByColumn,
            ImpliedReason::ImpliedByEntity => Self::ImpliedByEntity,
        }
    }
}

impl From<Unusable> for UseReason {
    fn from(why: Unusable) -> Self {
        match why {
            Unusable::ChangedSinceAnalysis => Self::ChangedSinceAnalysis,
            Unusable::NotAnalysed => Self::NotAnalysed,
            Unusable::Unreadable => Self::Unreadable,
        }
    }
}

/// One workbook an answer was built from, or one that did not serve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookUse {
    /// The index key; the interface shows the plain path. For `named_not_selected` and
    /// `ambiguous_name` it is the name as the user typed it: an unticked workbook is not in the
    /// selection, and a trace line quotes what the user wrote.
    pub file: String,
    pub how: WorkbookUseHow,
    /// Why the workbook concerns the question when it was not named (`implied_*`), or why it did not
    /// serve (`how` = `not_used`). `None` for a workbook the question names and the answer used.
    pub reason: Option<UseReason>,
    /// The rows of the workbook the model was shown in the sample (`tabular::sample`), with the data
    /// rows of the sheets they come from. `None` when only the structure was sent (the setting is
    /// `off`, or the workbook has no data table to sample) and for a computed figure.
    pub rows_sent: Option<usize>,
    pub rows_total: Option<usize>,
    /// The card was shrunk to fit the room of the model (columns or flags left out).
    pub truncated: bool,
    /// How many ticked workbooks the aggregate entry (`not_relevant`) stands for.
    pub count: Option<usize>,
    /// The index keys an ambiguous name may mean.
    pub candidates: Vec<String>,
}

impl WorkbookUse {
    fn of(file: &str, how: WorkbookUseHow, reason: Option<UseReason>) -> Self {
        Self {
            file: file.to_string(),
            how,
            reason,
            rows_sent: None,
            rows_total: None,
            truncated: false,
            count: None,
            candidates: Vec::new(),
        }
    }

    fn not_used(file: &str, reason: UseReason) -> Self {
        Self::of(file, WorkbookUseHow::NotUsed, Some(reason))
    }
}

/// What the door that built the answer did, as the trace needs it.
pub enum Answered<'a> {
    /// The mixed tier (door 4). `cards` are the cards in the turn and `implied` says why each unnamed
    /// workbook got one. A card counts as sent only when the model wrote an answer (`model_wrote`);
    /// `samples` says, for each workbook whose rows were sampled, how many were sent.
    Mixed {
        cards: &'a [WorkbookCard],
        implied: &'a [ImpliedWorkbook],
        table: &'a Option<TabularAnswer>,
        model_wrote: bool,
        samples: &'a [WorkbookSampleUse],
    },
    /// The tabular engine alone (doors 3 and the tables-only tier).
    Engine(&'a TabularAnswer),
    /// A mail-merge plan prepared from this workbook (door 1).
    Plan(&'a str),
    /// A folder engine answered (door 2): no workbook was read for a figure.
    FolderSearch,
}

/// The file an answer was computed from: a value, a structural fact.
fn computed_file(answer: &TabularAnswer) -> Option<&str> {
    match answer {
        TabularAnswer::Value { file, .. } => Some(file),
        // A structural refusal is a question the engine could not answer, not a computation.
        TabularAnswer::Structural {
            answer: StructuralAnswer::NotAnswerable { .. },
            ..
        } => None,
        TabularAnswer::Structural { file, .. } => Some(file),
        _ => None,
    }
}

/// The file an answer read without computing from it: a nudge that lists its columns, a question
/// about a measure or a column.
fn read_file(answer: &TabularAnswer) -> Option<&str> {
    match answer {
        TabularAnswer::Nudge { file, .. }
        | TabularAnswer::WhichMeasure { file, .. }
        | TabularAnswer::WhichColumn { file, .. }
        | TabularAnswer::Structural { file, .. } => Some(file),
        other => computed_file(other),
    }
}

/// The usage trace of one answer: first what the answer used (cards, figures, plan), then what the
/// question named, or the user ticked, that did not serve, then one aggregate entry for the ticked
/// workbooks the question had nothing to do with.
pub fn trace(facts: &SelectionFacts, answered: Answered<'_>) -> Vec<WorkbookUse> {
    let mut entries: Vec<WorkbookUse> = Vec::new();
    // Files an entry already accounts for, so the aggregate does not count them again.
    let mut accounted: Vec<String> = Vec::new();
    let mut aggregate = true;

    match answered {
        Answered::Mixed {
            cards,
            implied,
            table,
            model_wrote,
            samples,
        } => {
            let reason_of = |file: &str| {
                implied
                    .iter()
                    .find(|each| each.file == file)
                    .map(|each| UseReason::from(each.reason))
            };
            for card in cards {
                accounted.push(card.file.clone());
                if model_wrote {
                    let sample = samples.iter().find(|each| each.file == card.file);
                    let mut entry = WorkbookUse::of(
                        &card.file,
                        WorkbookUseHow::CardSent,
                        reason_of(&card.file),
                    );
                    entry.rows_sent = sample.map(|each| each.rows_sent);
                    entry.rows_total = sample.map(|each| each.rows_total);
                    entry.truncated = card.truncated;
                    entries.push(entry);
                } else {
                    entries.push(WorkbookUse::not_used(&card.file, UseReason::NotSent));
                }
            }
            if let Some(answer) = table {
                if let Some(file) = read_file(answer) {
                    accounted.push(file.to_string());
                }
                if let Some(file) = computed_file(answer) {
                    entries.push(WorkbookUse::of(
                        file,
                        WorkbookUseHow::Computed,
                        reason_of(file),
                    ));
                }
            }
        }
        Answered::Engine(answer) => {
            if let Some(file) = read_file(answer) {
                accounted.push(file.to_string());
            }
            if let Some(file) = computed_file(answer) {
                entries.push(WorkbookUse::of(file, WorkbookUseHow::Computed, None));
            }
            // The answer is the question "which workbook?": every ticked workbook is a candidate
            // and the card already says so.
            if matches!(
                answer,
                TabularAnswer::WhichWorkbook { .. } | TabularAnswer::NoUsableTable
            ) {
                aggregate = false;
            }
        }
        Answered::Plan(file) => {
            accounted.push(file.to_string());
            entries.push(WorkbookUse::of(file, WorkbookUseHow::PlanPrepared, None));
        }
        Answered::FolderSearch => aggregate = false,
    }

    // What the question named and did not get.
    for query in &facts.named.not_selected {
        entries.push(WorkbookUse::not_used(query, UseReason::NamedNotSelected));
    }
    for (query, candidates) in &facts.named.ambiguous {
        // The answer itself read one of the workbooks this name may mean (the engine takes the file
        // whose whole path is the name): saying the name settled on none would contradict it.
        if candidates
            .iter()
            .any(|candidate| accounted.contains(candidate))
        {
            continue;
        }
        let mut entry = WorkbookUse::not_used(query, UseReason::AmbiguousName);
        entry.candidates = candidates.clone();
        entries.push(entry);
        accounted.extend(candidates.iter().cloned());
    }
    // What the user ticked and could not be used.
    for (file, why) in &facts.unusable {
        entries.push(WorkbookUse::not_used(file, UseReason::from(*why)));
        accounted.push(file.clone());
    }

    // Ticked, usable, and not used: one line for all of them, only when several are ticked.
    let ticked = facts.usable.len() + facts.unusable.len();
    let left_out = facts
        .usable
        .iter()
        .filter(|file| !accounted.contains(file))
        .count();
    if aggregate && ticked > 1 && left_out > 0 {
        let mut entry = WorkbookUse::of("", WorkbookUseHow::NotUsed, Some(UseReason::NotRelevant));
        entry.count = Some(left_out);
        entries.push(entry);
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabular_answer::NamedWorkbooks;

    fn card(file: &str, truncated: bool) -> WorkbookCard {
        WorkbookCard {
            file: file.to_string(),
            text: String::new(),
            truncated,
        }
    }

    fn facts(usable: &[&str]) -> SelectionFacts {
        SelectionFacts {
            usable: usable.iter().map(|file| file.to_string()).collect(),
            ..SelectionFacts::default()
        }
    }

    fn mixed<'a>(
        cards: &'a [WorkbookCard],
        implied: &'a [ImpliedWorkbook],
        table: &'a Option<TabularAnswer>,
        model_wrote: bool,
        samples: &'a [WorkbookSampleUse],
    ) -> Answered<'a> {
        Answered::Mixed {
            cards,
            implied,
            table,
            model_wrote,
            samples,
        }
    }

    fn structural(file: &str) -> TabularAnswer {
        TabularAnswer::Structural {
            file: file.to_string(),
            answer: crate::tabular::structural::StructuralAnswer::SheetNames { sheets: vec![] },
        }
    }

    #[test]
    fn a_card_is_traced_only_when_the_model_wrote_an_answer() {
        let cards = [card("named.xlsx", false), card("implied.xlsx", false)];
        let implied = [ImpliedWorkbook {
            file: "implied.xlsx".to_string(),
            reason: ImpliedReason::ImpliedByWord,
        }];
        let facts = facts(&["named.xlsx", "implied.xlsx"]);
        let sent = trace(&facts, mixed(&cards, &implied, &None, true, &[]));
        assert_eq!(sent.len(), 2);
        assert_eq!(
            (sent[0].file.as_str(), sent[0].how, sent[0].reason),
            ("named.xlsx", WorkbookUseHow::CardSent, None)
        );
        assert_eq!(
            (sent[1].file.as_str(), sent[1].how, sent[1].reason),
            (
                "implied.xlsx",
                WorkbookUseHow::CardSent,
                Some(UseReason::ImpliedByWord)
            )
        );
        // A turn that never reached the model sends nothing, and says so for each card.
        let unsent = trace(&facts, mixed(&cards, &implied, &None, false, &[]));
        assert!(unsent
            .iter()
            .all(|entry| entry.how == WorkbookUseHow::NotUsed
                && entry.reason == Some(UseReason::NotSent)));
        assert_eq!(unsent.len(), 2);
    }

    #[test]
    fn the_rows_sent_and_a_shrunk_card_are_traced_beside_their_workbook() {
        let cards = [card("named.xlsx", true), card("other.xlsx", false)];
        let samples = [WorkbookSampleUse {
            file: "named.xlsx".to_string(),
            rows_sent: 12,
            rows_total: 480,
        }];
        let trace = trace(
            &facts(&["named.xlsx", "other.xlsx"]),
            mixed(&cards, &[], &None, true, &samples),
        );
        assert_eq!(
            (trace[0].rows_sent, trace[0].rows_total, trace[0].truncated),
            (Some(12), Some(480), true)
        );
        assert_eq!(
            (trace[1].rows_sent, trace[1].rows_total, trace[1].truncated),
            (None, None, false)
        );
    }

    #[test]
    fn door_four_a_figure_is_traced_beside_the_card_of_the_same_workbook() {
        let cards = [card("factures.csv", false)];
        let table = Some(TabularAnswer::Structural {
            file: "factures.csv".to_string(),
            answer: crate::tabular::structural::StructuralAnswer::SheetNames { sheets: vec![] },
        });
        let trace = trace(
            &facts(&["factures.csv"]),
            mixed(&cards, &[], &table, true, &[]),
        );
        let hows: Vec<WorkbookUseHow> = trace.iter().map(|entry| entry.how).collect();
        assert_eq!(
            hows,
            vec![WorkbookUseHow::CardSent, WorkbookUseHow::Computed]
        );
    }

    #[test]
    fn door_three_and_the_tables_only_tier_trace_the_workbook_the_engine_answered_from() {
        let answer = structural("ventes.csv");
        let facts = facts(&["ventes.csv", "achats.csv", "stock.csv"]);
        let trace = trace(&facts, Answered::Engine(&answer));
        assert_eq!(trace[0].file, "ventes.csv");
        assert_eq!(trace[0].how, WorkbookUseHow::Computed);
        // The two ticked workbooks the question did not concern are counted, not listed.
        let last = trace.last().expect("an aggregate");
        assert_eq!(last.reason, Some(UseReason::NotRelevant));
        assert_eq!(last.count, Some(2));
    }

    #[test]
    fn a_nudge_that_read_a_workbook_does_not_count_it_among_the_unused() {
        let answer = TabularAnswer::Nudge {
            file: "ventes.csv".to_string(),
            reason: None,
            available_sheets: vec![],
            available_columns: vec![],
            example_column: None,
            example_group: None,
            filter_column: None,
            filter_value: None,
            close_values: vec![],
            model_attempt: None,
        };
        let trace = trace(
            &facts(&["ventes.csv", "achats.csv"]),
            Answered::Engine(&answer),
        );
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].count, Some(1));
    }

    #[test]
    fn a_question_that_asks_which_workbook_adds_no_line_of_its_own() {
        let answer = TabularAnswer::WhichWorkbook {
            candidates: vec!["a.csv".to_string(), "b.csv".to_string()],
            hints: vec![],
        };
        assert!(trace(&facts(&["a.csv", "b.csv"]), Answered::Engine(&answer)).is_empty());
    }

    #[test]
    fn door_one_traces_the_workbook_the_plan_was_prepared_from() {
        let trace = trace(&facts(&["clients.xlsx"]), Answered::Plan("clients.xlsx"));
        assert_eq!(trace.len(), 1);
        assert_eq!(
            (trace[0].file.as_str(), trace[0].how),
            ("clients.xlsx", WorkbookUseHow::PlanPrepared)
        );
    }

    #[test]
    fn door_two_traces_only_what_was_named_and_not_found() {
        let mut facts = facts(&["a.csv", "b.csv"]);
        facts.named = NamedWorkbooks {
            not_selected: vec!["ventes.csv".to_string()],
            ..NamedWorkbooks::default()
        };
        let trace = trace(&facts, Answered::FolderSearch);
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].reason, Some(UseReason::NamedNotSelected));
        assert_eq!(trace[0].file, "ventes.csv");
    }

    #[test]
    fn a_named_workbook_that_is_not_ticked_keeps_the_name_as_typed() {
        let mut facts = facts(&["a.csv", "b.csv"]);
        facts.named = NamedWorkbooks {
            not_selected: vec!["Ventes.CSV".to_string()],
            ..NamedWorkbooks::default()
        };
        let answer = TabularAnswer::FileNotSelected {
            query: "Ventes.CSV".to_string(),
        };
        let trace = trace(&facts, Answered::Engine(&answer));
        assert_eq!(trace[0].how, WorkbookUseHow::NotUsed);
        assert_eq!(trace[0].reason, Some(UseReason::NamedNotSelected));
        assert_eq!(trace[0].file, "Ventes.CSV");
    }

    #[test]
    fn an_ambiguous_name_lists_what_it_may_mean_and_those_are_not_counted_as_left_out() {
        let mut facts = facts(&["ventes-2025.csv", "ventes-2026.csv", "autre.csv"]);
        facts.named = NamedWorkbooks {
            ambiguous: vec![(
                "ventes".to_string(),
                vec!["ventes-2025.csv".to_string(), "ventes-2026.csv".to_string()],
            )],
            ..NamedWorkbooks::default()
        };
        let answer = TabularAnswer::AmbiguousReference {
            query: "ventes".to_string(),
            candidates: vec![],
        };
        let trace = trace(&facts, Answered::Engine(&answer));
        assert_eq!(trace[0].reason, Some(UseReason::AmbiguousName));
        assert_eq!(trace[0].candidates.len(), 2);
        // `autre.csv` alone was left out.
        assert_eq!(trace[1].count, Some(1));
    }

    #[test]
    fn an_ambiguous_name_is_not_reported_when_the_answer_read_one_of_its_candidates() {
        let mut facts = facts(&["donnees.csv", "autre/donnees.csv", "stock.csv"]);
        facts.named = NamedWorkbooks {
            ambiguous: vec![(
                "donnees".to_string(),
                vec!["donnees.csv".to_string(), "autre/donnees.csv".to_string()],
            )],
            ..NamedWorkbooks::default()
        };
        let answer = structural("donnees.csv");
        let trace = trace(&facts, Answered::Engine(&answer));
        let hows: Vec<WorkbookUseHow> = trace.iter().map(|entry| entry.how).collect();
        assert_eq!(
            hows,
            vec![WorkbookUseHow::Computed, WorkbookUseHow::NotUsed]
        );
        // The other candidate and `stock.csv` were left out.
        assert_eq!(trace[1].count, Some(2));
    }

    #[test]
    fn a_workbook_changed_since_it_was_ticked_is_traced_and_not_counted_again() {
        let mut facts = facts(&["a.csv"]);
        facts
            .unusable
            .push(("b.csv".to_string(), Unusable::ChangedSinceAnalysis));
        facts
            .unusable
            .push(("c.csv".to_string(), Unusable::NotAnalysed));
        facts
            .unusable
            .push(("d.csv".to_string(), Unusable::Unreadable));
        let answer = structural("a.csv");
        let trace = trace(&facts, Answered::Engine(&answer));
        let reasons: Vec<Option<UseReason>> = trace.iter().map(|entry| entry.reason).collect();
        assert_eq!(
            reasons,
            vec![
                None,
                Some(UseReason::ChangedSinceAnalysis),
                Some(UseReason::NotAnalysed),
                Some(UseReason::Unreadable),
            ]
        );
    }

    #[test]
    fn one_ticked_workbook_gets_no_aggregate_line() {
        let answer = TabularAnswer::NoMatchingFile {
            query: "x.csv".to_string(),
        };
        assert!(trace(&facts(&["a.csv"]), Answered::Engine(&answer)).is_empty());
    }

    #[test]
    fn an_entry_serialises_as_machine_codes_in_camel_case() {
        let mut entry = WorkbookUse::of(
            "factures.xlsx",
            WorkbookUseHow::CardSent,
            Some(UseReason::ImpliedSingle),
        );
        entry.rows_sent = Some(9);
        entry.rows_total = Some(9);
        entry.truncated = true;
        let json = serde_json::to_value(entry).expect("serialises");
        assert_eq!(json["file"], "factures.xlsx");
        assert_eq!(json["how"], "card_sent");
        assert_eq!(json["reason"], "implied_single");
        assert_eq!(json["rowsSent"], 9);
        assert_eq!(json["rowsTotal"], 9);
        assert_eq!(json["truncated"], true);
        assert!(json["count"].is_null());
        assert_eq!(json["candidates"].as_array().map(Vec::len), Some(0));
        let aggregate =
            serde_json::to_value(WorkbookUse::not_used("", UseReason::NotRelevant)).expect("ok");
        assert_eq!(aggregate["how"], "not_used");
        assert_eq!(aggregate["reason"], "not_relevant");
    }
}
