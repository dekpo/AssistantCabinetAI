//! Session 16 (`docs/SESSION-DATA-16-Mixed-Tier.md`): the mixed tier. Documents and tables are
//! both selected, and the question is neither clearly data-only (session 15's own router, still
//! the first thing tried - `commands::mixed_data_only_tier`) nor refused outright: the tabular
//! engine computes, retrieval cites, and a model is asked to write across the two, held to both by
//! `MIXED_INSTRUCTION` and checked afterward (`verify_numbers`, `reject_citations`) rather than
//! trusted blindly (`docs/SELECTION-AND-MEMORY.md`, "documents and tables together").
//!
//! No `tauri::` import, by design (`docs/SESSION-DATA-16-Mixed-Tier.md` section 4's own scope):
//! `answer` below takes everything it needs as owned or borrowed data, a `GatewayClient` and two
//! plain callbacks, the same way `tabular_answer::answer`/`ModelAssist` already avoid an
//! `AppHandle`. `commands::mixed_tier` is the thin Tauri-coupled wrapper - settings, the local
//! index, the `Channel` - that calls into this module and nowhere else does the mixed tier's own
//! reasoning live.
//!
//! The table block is built from `tabular::escalation::format_evidence`, the same function tier 2
//! keeps unsent for exactly this session, which already refuses a raw row list, and a workbook it is
//! told about reaches it as a card, which holds structure only (`tabular::card`). Since TABULAR step
//! 03b a controlled sample of its rows and the engine's column profile may follow the card, identifiers
//! masked and the cells held between random markers (`tabular::sample`, the setting
//! `tabular_rows_mode`); with that setting off the turn is exactly the turn without them. Entity linking (`entity_link`) reads retrieved
//! excerpt text, never the question's own words a second time - `tabular_answer::detect_filters`
//! (gap A, session 11) already tried those - and anchors a candidate word against real column
//! values through `tabular_answer::column_value_words`, the same anchoring rule, applied to a
//! different source of words.

use std::collections::HashSet;
use std::time::Duration;

use serde::Serialize;

use crate::conversation::{self, ContextBudget};
use crate::error::AppError;
use crate::file_reference::fold_text;
use crate::gateway::{ChatTurn, GatewayClient};
use crate::number_check;
use crate::retrieval::Evidence;
use crate::tabular::engine::{self, FilterSpec, TabularValue};
use crate::tabular::formula_check::{
    self, FormulaContext, FormulaIssue, FormulaNote, FormulaReport,
};
use crate::tabular::inventory::{ColumnType, TabularInventory};
use crate::tabular::question::{self, TabularRoute};
use crate::tabular::sample::SAMPLE_INSTRUCTION;
use crate::tabular::Workbook;
use crate::tabular_answer::{self, PendingAnswer, TabularAnswer};

/// English, profession-neutral, short - the same reasons every other model-facing constant in
/// this product is (`docs/DECISIONS.md`, "profession-neutral model-facing text"). Lists the sources a
/// turn may carry (the blocks that are absent read "none"), forbids computing or second-guessing a
/// number of the table results, and asks the model to say plainly when a part cannot be answered
/// rather than guess - mechanisms 3, 4 and 6 of `docs/SESSION-DATA-REFERENCE-report.md` section 5,
/// stated as one instruction the way `RETRIEVAL_INSTRUCTION` and `TABULAR_INSTRUCTION` each state
/// their own tier. The formula sentence is TABULAR plan 4.4: each formula on a line of its own
/// starting with `=`, so that a later step can find it and check it.
pub const MIXED_INSTRUCTION: &str =
    "You are given sources, each labelled: 'Document excerpts' from the user's own documents; \
     'Workbook cards' describing only the structure of the user's own data tables (sheet names, \
     column letters, column names, types, row counts - never the data); and 'Table results' \
     computed by the program, not by you. Use only these sources. Never invent a value, a total, \
     a date or a name that is not in them, and never compute or second-guess a number from the \
     table results: report it as given. Cite a document fact with its bracketed number, exactly \
     as supplied, and never cite a document number for a fact that came from a table. For a \
     formula, use the column letters of the cards, write it in the spreadsheet conventions of the \
     answer language, put each formula on a line of its own starting with =, and say you cannot \
     test it because you do not see the data. If the sources cannot establish part of the \
     question, say so plainly and, when a calculation would settle it, say which calculation to \
     ask for. A workbook card is a real source about structure: use it to say what a workbook \
     contains, which columns to use and how to relate or combine files, even though no cell value \
     is available, and do not refuse a question about structure for lack of data. Write in the \
     language of the question and describe the sources in your own words: do not copy their lines.";

/// The second request of the formula repair (TABULAR step 07): sent once, after the program found a
/// function name or a column letter it could look up and that is wrong, followed by one line per
/// problem (`repair_turn`). English, profession-neutral, short.
pub const FORMULA_REPAIR_INSTRUCTION: &str =
    "The program checked the formulas of your previous answer and found the problems listed below. \
     Write the answer again with every formula corrected. Use only functions that exist in the \
     spreadsheet application, write their names and the argument separator in the conventions of \
     the answer language, use only the sheets and the column letters of the workbook cards, and put \
     each formula on a line of its own starting with =. Keep the rest of the answer as it is.";

/// Appended to the instruction when the user asked for the analysis of a computed figure (TABULAR
/// step 06, plan 4.7): the figure of the table results is final, the model comments on it and goes
/// further, and never computes a new one.
pub const ANALYSIS_ADDENDUM: &str =
    "The user asked for an analysis of the table result. Treat the figure as final. Comment on what      it means, relate it to the document excerpts when there are any, point out anything that looks      unusual, and suggest next questions. Never compute a new figure; if one is needed, say which      calculation to ask for.";

/// Added to the instruction only when a document the question names is targeted and its folder
/// facts are in the turn (TABULAR step 08). English, profession-neutral, one sentence.
pub const FOLDER_FACTS_INSTRUCTION: &str =
    "The 'Folder facts' block is authoritative for filesystem facts only, such as which document the \
     user named; never use it as a substitute for what a document says, and never invent a file, a \
     name or a path.";

/// The words of the formula sentence of `MIXED_INSTRUCTION` that are true only without a sample, and
/// what a turn that carries a sample says instead (`build_turn`). Measured on 22 October 2026
/// (TABULAR step 03b): with rows in the turn, `qwen3:4b-instruct` answered "the data is not visible
/// here, so I cannot test the formula", which the sample contradicts.
const FORMULA_NO_DATA_PHRASE: &str = "because you do not see the data";
const FORMULA_WITH_SAMPLE_PHRASE: &str = "because you cannot run it on the data";

/// How much of the document share of the budget this tier sends, under the existing cap
/// (mechanism 4: "the context cap is split between the blocks, a documented share each"). Smaller
/// than retrieval's own `MAX_EVIDENCE_CHARS`, since the table block and the instruction also have
/// to fit beside it in the same turn. This is the share at the default window; a model with a
/// larger window gets it scaled the same way retrieval's limits are (`ContextBudget::
/// scaled_over_baseline`), so the table block and the instruction keep their proportion.
pub const MIXED_DOCUMENT_SHARE_CHARS: usize = 4_000;

/// Why one side of a mixed answer has nothing to show, named rather than left to the absence of a
/// field to explain (mechanism 6, partial refusal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MixedPartUnavailable {
    /// The question had nothing for this side at all - today, only a pure content question's data
    /// half: the classifier found no tabular operation in it, so no gap-G escalation is even
    /// tried.
    NotAskedAbout,
    /// This side was asked about, but nothing could be found: retrieval found no evidence, or an
    /// embedding call could not be made.
    NoEvidence,
    /// The model could not be reached to write the combined answer. The other side, when it is a
    /// computed table value, still answers on its own - the same "zero gateway calls for a
    /// deterministic answer" guarantee a mixed question now keeps too.
    GatewayUnavailable,
    /// The question points at a document ("the supplier named in this letter") and no entity of
    /// that document could be tied to a table row, so the only value the engine could give is the
    /// whole table's figure. That figure answers a different question, and is withheld rather
    /// than shown as if it were the one asked for (HAP-1, Q24; the real linking is lot D).
    NotLinked,
    /// No document is ticked: the answer is written from the workbooks alone, which the interface
    /// says in its own words (TABULAR step 05, the tables-only advisory writer). Informative, not a
    /// caution.
    NotSelected,
}

/// One number the model wrote that matched neither the table's own value nor any excerpt's text
/// verbatim (mechanism 5, numeric verification). The draft is never rewritten - this is appended
/// data, and the interface decides how to show it beside the answer it corrects.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericCorrection {
    /// The number exactly as the model wrote it - never reformatted, so the interface can quote
    /// the model's own claim.
    pub claimed: String,
    /// The table's real value. Raw: the interface formats it in her language
    /// (`docs/LANGUAGE-AND-LOCALE.md`).
    pub correct: f64,
}

/// What a mixed question came to: a model's prose across two sources it is held to, the table
/// part and the document part each carrying their own provenance and their own "could not
/// establish" reason, plus whatever the post-generation checks found.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixedAnswer {
    /// The model's prose. Empty when generation was never attempted - the table answered alone,
    /// with no gateway call, because the document side had nothing to add
    /// (`documents_unavailable`).
    pub answer: String,
    /// The tabular part's own computed value, when the question had one. `None` when
    /// `table_unavailable` is set, and also when the question concerned a workbook that reached
    /// the model as a card but asked for no calculation.
    pub table: Option<TabularAnswer>,
    pub table_unavailable: Option<MixedPartUnavailable>,
    /// The document excerpts the model was actually sent - already capped to this tier's own
    /// share of the budget (`MIXED_DOCUMENT_SHARE_CHARS`), so this is also what citation numbers
    /// in `answer` refer to.
    pub document_sources: Vec<Evidence>,
    pub documents_unavailable: Option<MixedPartUnavailable>,
    /// Appended, never silent: a number the model wrote that did not match the table or any
    /// excerpt.
    pub corrections: Vec<NumericCorrection>,
    /// A bracketed citation (`"[3]"`) the model wrote that does not resolve to any of
    /// `document_sources` - most often a table fact dressed up as a document citation.
    pub rejected_citations: Vec<String>,
    /// Figures the model stated that are in none of the excerpts and not in the question, reported
    /// only when no table figure is shown (a table value is checked by `corrections` instead).
    /// Reported beside the answer, never edited into it (`number_check`).
    pub unverified_numbers: Vec<String>,
    /// The explicit arithmetic the model wrote itself (`10 + 40 = 50`), as written: the interface
    /// says under the answer that only the engine is reliable, and the answer is never edited
    /// (`number_check::arithmetic_expressions`, TABULAR step 03b).
    pub model_arithmetic: Vec<String>,
    /// The problems the program found in the formulas the model wrote, each with its formula (TABULAR
    /// step 07). The answer is never edited: these are shown beside it.
    pub formula_notes: Vec<FormulaNote>,
    /// How many formulas had nothing the program could find wrong: the interface then says once that
    /// the names and columns exist and that the result is not tested.
    pub formulas_verified: usize,
    /// The first answer had a function or a column that does not exist and the model was asked once
    /// to correct it: this answer is the second one.
    pub formula_repaired: bool,
    /// What was wrong in the first answer, when it was corrected (the problems that triggered the
    /// request, each with the formula as first written): the interface says it beside the corrected
    /// answer, so the correction can be judged. Empty when nothing was corrected.
    pub formula_repaired_from: Vec<FormulaNote>,
}

/// Everything `answer` needs, owned or borrowed, with no `AppHandle` and - on purpose - no
/// `IndexStore` anywhere: both the tabular classification (`tabular_answer::prepare`) and
/// retrieval's own embed-then-search sequence need `&IndexStore` interleaved with a gateway
/// `.await`, which a reference cannot survive crossing (`IndexStore` holds a `RefCell` and so is
/// not `Sync` - exactly the reason `tabular_answer`'s own module doc gives for its `prepare`/
/// `resolve` split). `commands::mixed_tier` does that work itself, owning the index for the whole
/// stretch the way `sourced_answer`'s own tier 1 arm already does, and calls into this module only
/// with what came out of it: an owned `PendingAnswer` and the retrieved `Evidence`.
pub struct MixedContext<'a> {
    pub locale: &'a str,
    pub gateway: &'a GatewayClient,
    pub server_url: &'a str,
    pub model_alias: &'a str,
    pub idle_timeout: Duration,
    pub history: &'a [ChatTurn],
    pub budget: ContextBudget,
    /// The block of workbook cards (`tabular::card::CardBlock::text`, header included), already
    /// built by the caller from the workbooks the question names or concerns. Empty when there is
    /// none: the turn then reads `Workbook cards: none.`
    pub cards_text: &'a str,
    /// The block of sample and profile (`tabular::sample::SampleBlock::text`, markers included),
    /// empty when the setting is `off` or there is nothing to send. Its figures are known facts of
    /// the turn for the number checks, as the cards are.
    pub sample_text: &'a str,
    /// The folder facts of the one document the question names (`work_folder_context`), empty when no
    /// document is targeted. Known facts of the turn for the number checks, as the cards are.
    pub folder_facts: &'a str,
    /// What the calculation record and the analysis need beyond the figure (TABULAR step 06).
    pub calculation: CalculationSide<'a>,
    /// What the formulas of the answer are checked against (TABULAR step 07): the dictionaries of the
    /// spreadsheet application and the structure of the workbooks that were sent. `None` checks nothing.
    pub formulas: Option<&'a FormulaContext>,
}

/// The part of a turn that exists only for a computed figure: the data rows of the sheets in play (the
/// record says "41 kept out of 124 data rows") and whether the user asked for the analysis, which adds
/// `ANALYSIS_ADDENDUM` to the instruction. `Default` is the ordinary turn.
#[derive(Debug, Clone, Copy, Default)]
pub struct CalculationSide<'a> {
    pub sheet_sizes: &'a [tabular_answer::SheetSize],
    pub analysis: bool,
}

/// The full mixed tier, from where `commands::mixed_tier` leaves off: entity linking, compute,
/// generation and the two post-generation checks, in that order
/// (`docs/SESSION-DATA-REFERENCE-report.md` section 5). `pending` is `tabular_answer::prepare`'s
/// own result and `document_sources`/`documents_unavailable` are retrieval's, both already
/// computed by the caller for the `IndexStore` reason `MixedContext` explains. `on_sources`/
/// `on_delta` are the only way this reaches the interface, called at the same two moments
/// `commands::Writer::write` already streams from, so the caller only has to forward them to the
/// real `Channel`. Returns whether a gateway call for generation was actually attempted, alongside
/// the answer, so the caller knows whether to also emit a `Completed` stream event.
pub async fn answer(
    question: &str,
    pending: PendingAnswer,
    document_sources: Vec<Evidence>,
    documents_unavailable: Option<MixedPartUnavailable>,
    context: &MixedContext<'_>,
    mut on_sources: impl FnMut(&[Evidence]),
    mut on_delta: impl FnMut(&str),
) -> Result<(MixedAnswer, bool), AppError> {
    // Capped to this tier's own share of the budget before anything else touches it: citation
    // numbers in the model's eventual answer must agree with exactly what is sent, not with
    // whatever retrieval originally found.
    let share = context
        .budget
        .scaled_over_baseline(MIXED_DOCUMENT_SHARE_CHARS);
    let document_sources = cap_document_share(&document_sources, share);
    on_sources(&document_sources);
    let documents_unavailable = documents_unavailable
        .or_else(|| document_sources.is_empty().then_some(MixedPartUnavailable::NoEvidence));

    // The tabular part, finished now that the excerpts exist to link an entity against.
    let has_cards = !context.cards_text.is_empty();
    let (table, table_unavailable) =
        resolve_table(pending, question, context, &document_sources, has_cards).await;
    let (table, mut table_unavailable) = if points_at_a_document(question, context.locale, &table) {
        (None, Some(MixedPartUnavailable::NotLinked))
    } else {
        (table, table_unavailable)
    };

    // Anything other than a computed value is a human choice or a refusal the interface already
    // renders for tier 2 - never combined with a generated answer (mechanism 6's "nothing
    // computed" case, among others: a disambiguation stops here, with no gateway call).
    if let Some(answer) = &table {
        if !matches!(answer, TabularAnswer::Value { .. }) {
            return Ok((
                MixedAnswer {
                    answer: String::new(),
                    table: table.clone(),
                    table_unavailable: None,
                    document_sources: Vec::new(),
                    documents_unavailable: None,
                    corrections: Vec::new(),
                    rejected_citations: Vec::new(),
                    unverified_numbers: Vec::new(),
                    model_arithmetic: Vec::new(),
                    ..MixedAnswer::default()
                },
                false,
            ));
        }
    }
    let computed_value = matches!(table, Some(TabularAnswer::Value { .. }));
    // A card stands for the table side of a question that asks for no calculation: the model reads
    // the real structure of the workbook it names. Without a card there is no table part at all.
    if table.is_none() && table_unavailable.is_none() && !has_cards {
        table_unavailable = Some(MixedPartUnavailable::NotAskedAbout);
    }

    // Neither side had anything to answer from at all.
    if documents_unavailable.is_some() && table_unavailable.is_some() {
        return Err(AppError::InsufficientEvidence);
    }

    let table_text = table_evidence_text(&table, context.calculation.sheet_sizes);
    let expected = match &table {
        Some(TabularAnswer::Value { value, .. }) => scalar_of(value),
        _ => None,
    };

    // Zero gateway calls when there is nothing on the document side to write about - the same
    // guarantee a deterministic tabular answer already keeps (`docs/DECISIONS.md`), extended to a
    // mixed question whose document half came back empty. Only a computed value can answer alone:
    // a question about a workbook's structure has no document evidence either, and the model still
    // reads the cards (the guard above already refused a turn with neither).
    // The analysis of a figure is the exception: it exists to give the model the figure, with or without
    // documents (TABULAR step 06).
    if let Some(reason) =
        documents_unavailable.filter(|_| computed_value && !context.calculation.analysis)
    {
        return Ok((
            MixedAnswer {
                answer: String::new(),
                table,
                table_unavailable,
                document_sources: Vec::new(),
                documents_unavailable: Some(reason),
                corrections: Vec::new(),
                rejected_citations: Vec::new(),
                unverified_numbers: Vec::new(),
                model_arithmetic: Vec::new(),
                ..MixedAnswer::default()
            },
            false,
        ));
    }

    let instruction = build_turn_for(
        &document_sources,
        non_empty(context.cards_text),
        non_empty(context.sample_text),
        table_text.as_deref(),
        non_empty(context.folder_facts),
        context.calculation.analysis,
    );
    let final_turn = format!("{instruction}\n\nQuestion: {question}");
    // Both blocks are sources: the memory yields to them (`conversation::history_beside_sources`).
    let remembered = conversation::history_beside_sources(context.history, context.budget);
    let mut turns = conversation::fit_history(
        &remembered,
        context.budget.history_chars(final_turn.chars().count()),
    );
    turns.push(ChatTurn {
        role: "user".to_string(),
        content: final_turn,
    });

    let generated = context
        .gateway
        .chat_at(
            context.server_url,
            context.model_alias,
            Some(context.locale),
            &turns,
            context.idle_timeout,
            Some(crate::gateway::GROUNDED_TEMPERATURE),
            |delta| on_delta(delta),
        )
        .await;

    let mut answer_text = match generated {
        Ok(text) => text,
        Err(_) if computed_value => {
            return Ok((
                MixedAnswer {
                    answer: String::new(),
                    table,
                    table_unavailable,
                    document_sources: Vec::new(),
                    documents_unavailable: Some(MixedPartUnavailable::GatewayUnavailable),
                    corrections: Vec::new(),
                    rejected_citations: Vec::new(),
                    unverified_numbers: Vec::new(),
                    model_arithmetic: Vec::new(),
                    ..MixedAnswer::default()
                },
                true,
            ));
        }
        Err(error) => return Err(error),
    };

    // The formulas the model wrote are looked up against the dictionaries and the cards; a function or a
    // column letter that does not exist is worth one more request, and only one (TABULAR step 07).
    let mut formula_report = check_formulas(&answer_text, context.formulas);
    let mut formula_repaired = false;
    let mut formula_repaired_from: Vec<FormulaNote> = Vec::new();
    if let Some(formulas) = context.formulas.filter(|_| formula_report.has_repairable()) {
        let mut again = turns.clone();
        again.push(ChatTurn {
            role: "assistant".to_string(),
            content: answer_text.clone(),
        });
        again.push(ChatTurn {
            role: "user".to_string(),
            content: repair_turn(&formula_report.notes),
        });
        // Not streamed: the first answer is on screen and the corrected one replaces it. A request that
        // fails, is stopped or writes no formula leaves the first answer and its notes.
        let second = context
            .gateway
            .chat_at(
                context.server_url,
                context.model_alias,
                Some(context.locale),
                &again,
                context.idle_timeout,
                Some(crate::gateway::GROUNDED_TEMPERATURE),
                |_| {},
            )
            .await;
        if let Ok(text) = second {
            if !formula_check::extract_formulas(&text).is_empty() {
                formula_repaired_from = formula_report
                    .notes
                    .iter()
                    .filter(|note| note.issue.code.is_repairable())
                    .cloned()
                    .collect();
                formula_report = formula_check::check_answer(&text, formulas);
                answer_text = text;
                formula_repaired = true;
            }
        }
    }

    // A figure a card states (a row count, a year) is a fact of the turn, not a claim to correct.
    let corrections = verify_numbers(
        &answer_text,
        expected,
        &document_sources,
        &[
            context.cards_text,
            context.sample_text,
            context.folder_facts,
            table_text.as_deref().unwrap_or_default(),
        ],
    );
    let rejected_citations = reject_citations(&answer_text, document_sources.len());
    // With no table figure on screen there is nothing to correct the prose against, so the figures it
    // states are held to the excerpts and the question, as a document answer's are.
    let unverified_numbers = if expected.is_none() && table.is_none() {
        let mut known: Vec<&str> = document_sources.iter().map(|item| item.text.as_str()).collect();
        known.extend(document_sources.iter().map(|item| item.relative_path.as_str()));
        known.push(context.cards_text);
        known.push(context.sample_text);
        known.push(context.folder_facts);
        known.push(question);
        number_check::unsupported_numbers(&answer_text, &known, 2)
    } else {
        Vec::new()
    };

    let model_arithmetic = number_check::arithmetic_expressions(&answer_text);

    Ok((
        MixedAnswer {
            answer: answer_text,
            table,
            table_unavailable,
            document_sources,
            documents_unavailable,
            corrections,
            rejected_citations,
            unverified_numbers,
            model_arithmetic,
            formula_notes: formula_report.notes,
            formulas_verified: formula_report.verified,
            formula_repaired,
            formula_repaired_from,
        },
        true,
    ))
}

/// The notes of the formulas in `answer`; nothing when there is no context to check against.
fn check_formulas(answer: &str, context: Option<&FormulaContext>) -> FormulaReport {
    match context {
        Some(context) => formula_check::check_answer(answer, context),
        None => FormulaReport::default(),
    }
}

/// One line of the repair request for a problem the program found: the formula and what is wrong, in
/// English, as machine text (the user reads the interface's own sentence).
fn repair_line(note: &FormulaNote) -> String {
    let FormulaIssue {
        code,
        found,
        expected,
        place,
    } = &note.issue;
    let mut line = format!("{} - {}: {found}", note.formula, code.as_str());
    if let Some(expected) = expected {
        line.push_str(&format!(" (expected: {expected})"));
    }
    if let Some(place) = place {
        line.push_str(&format!(" (in: {place})"));
    }
    line
}

/// The user turn of the formula repair: the instruction, then the problems that can be fixed.
pub(crate) fn repair_turn(notes: &[FormulaNote]) -> String {
    let lines: Vec<String> = notes
        .iter()
        .filter(|note| note.issue.code.is_repairable())
        .map(repair_line)
        .collect();
    format!(
        "{FORMULA_REPAIR_INSTRUCTION}\n\nProblems:\n{}",
        lines.join("\n")
    )
}

/// A question that points at a document whose table result carries no filter at all: a computed
/// whole-table value cannot be the figure for "the supplier named in this letter".
fn points_at_a_document(question: &str, locale: &str, table: &Option<TabularAnswer>) -> bool {
    matches!(
        table,
        Some(TabularAnswer::Value { locator, derivation: engine::TabularDerivation::Computed { .. }, .. })
            if locator.filters.is_empty()
    ) && crate::tabular::question::refers_to_a_document(question, locale)
}

/// The tabular part, from a classifier result that session 15's own router already decided it
/// could not answer alone. `PendingAnswer::Done` is kept as is - whatever it is, computed value or
/// disambiguation; `PendingAnswer::TryModel` is where this session's own work happens: a question
/// the classifier could not resolve from its own words at all (`fallback` carries `reason: None`)
/// has no data component, so neither entity linking nor the model is tried on it; one the
/// classifier recognised but could not resolve a filter for tries entity linking first, and only
/// falls back to session 14's hidden interpreter (`tabular_answer::resolve`) when that finds
/// nothing either.
async fn resolve_table(
    pending: PendingAnswer,
    question: &str,
    context: &MixedContext<'_>,
    document_sources: &[Evidence],
    has_cards: bool,
) -> (Option<TabularAnswer>, Option<MixedPartUnavailable>) {
    // The classifier found no data operation in the question. With a card in the turn the table side
    // is present as that card and only the table *value* is absent; without one the question has no
    // table part and the documents answer it alone.
    let no_operation = || match has_cards {
        true => (None, None),
        false => (None, Some(MixedPartUnavailable::NotAskedAbout)),
    };
    match pending {
        // The question has no table part (`prepare_for_mixed` says so with an unrecognised nudge, the same
        // sign a question the classifier could not read leaves).
        PendingAnswer::Done(TabularAnswer::Nudge { reason: None, .. }) => no_operation(),
        PendingAnswer::Done(answer) => (Some(answer), None),
        PendingAnswer::TryModel {
            file,
            workbook,
            inventory,
            allowed,
            fallback,
            skip_model,
        } => {
            if matches!(fallback, TabularAnswer::Nudge { reason: None, .. }) {
                return no_operation();
            }
            match try_entity_filled_operation(
                question,
                &inventory,
                &workbook,
                allowed.as_deref(),
                document_sources,
                context.locale,
                &file,
            ) {
                Some(found) => (Some(found), None),
                None => {
                    let assist = tabular_answer::ModelAssist {
                        gateway: context.gateway,
                        server_url: context.server_url,
                        model_alias: context.model_alias,
                        idle_timeout: context.idle_timeout,
                    };
                    let resolved = tabular_answer::resolve(
                        PendingAnswer::TryModel {
                            file,
                            workbook,
                            inventory,
                            allowed,
                            fallback,
                            skip_model,
                        },
                        question,
                        context.locale,
                        &assist,
                    )
                    .await;
                    (Some(resolved), None)
                }
            }
        }
    }
}

/// The table block, header included: `Table results (workbook "<file>"):` and then the calculation
/// record of the computed value (`tabular_answer::calculation_record`): the question is left out, the
/// turn carries it; never a raw row list, never a cell value outside the one computed result. The file
/// is named by its plain path (and the folder it lies in when it is the other family's), because the
/// model has to be able to tell which workbook a figure comes from when several are named. `None` for
/// anything that is not a computed `Value` - a disambiguation never reaches this function at all (the
/// caller returns before generation for those), and a question about no calculation has no table
/// evidence to send. This is the one place that decides what the block says.
pub(crate) fn table_evidence_text(
    table: &Option<TabularAnswer>,
    sizes: &[tabular_answer::SheetSize],
) -> Option<String> {
    let Some(answer @ TabularAnswer::Value { file, .. }) = table else {
        return None;
    };
    tabular_answer::calculation_record(None, answer, sizes).map(|record| {
        format!(
            "Table results (workbook {}):
{record}",
            crate::tabular::card::file_label(file)
        )
    })
}

/// `Some(text)` for a non-empty string.
fn non_empty(text: &str) -> Option<&str> {
    (!text.is_empty()).then_some(text)
}

// --- Building the turn sent to the model -----------------------------------------------------

/// `document_sources`, capped to this tier's own share of the context budget: whole excerpts only,
/// kept in rank order, dropped from the end once the share would be exceeded - the same shape
/// `retrieval::search_per_document`'s own cap takes, applied here to a second, smaller budget.
pub(crate) fn cap_document_share(evidence: &[Evidence], budget_chars: usize) -> Vec<Evidence> {
    let mut selected = Vec::new();
    let mut used = 0usize;
    for item in evidence {
        if used + item.text.len() > budget_chars && !selected.is_empty() {
            break;
        }
        used += item.text.len();
        selected.push(item.clone());
    }
    selected
}

/// The instruction plus the three blocks, one turn ready to send - the mixed-tier counterpart of
/// `retrieval::build_context_turn` and `tabular::escalation::build_context_turn`. `document_sources`
/// must already be the capped set (`cap_document_share`): this function only formats what it is
/// given, so citation numbers in the model's answer and `document_sources.len()` always agree.
/// `cards` is the block of workbook cards as `tabular::card` writes it (header included),
/// `sample` the block of sample and profile as `tabular::sample` writes it (markers included) and
/// `table_block` the block of `table_evidence_text` (header included); an absent block reads
/// `<name>: none.` The sample is the exception: with none there is no block at all and no sentence
/// about it in the instruction, so a turn with the setting `off` is the turn of step 03 to the byte.
#[cfg(test)]
pub(crate) fn build_turn(
    document_sources: &[Evidence],
    cards: Option<&str>,
    sample: Option<&str>,
    table_block: Option<&str>,
) -> String {
    build_turn_for(document_sources, cards, sample, table_block, None, false)
}

/// `build_turn`, with `ANALYSIS_ADDENDUM` ended onto the instruction when the user asked for the
/// analysis of a figure (TABULAR step 06). Without it the turn is `build_turn`'s, byte for byte.
pub(crate) fn build_turn_for(
    document_sources: &[Evidence],
    cards: Option<&str>,
    sample: Option<&str>,
    table_block: Option<&str>,
    folder_facts: Option<&str>,
    analysis: bool,
) -> String {
    let documents_block = if document_sources.is_empty() {
        "Document excerpts: none.".to_string()
    } else {
        format!(
            "Document excerpts:\n{}",
            crate::retrieval::format_evidence(document_sources)
        )
    };
    let cards_block = match cards {
        Some(text) => text.to_string(),
        None => "Workbook cards: none.".to_string(),
    };
    let table_block = match table_block {
        Some(text) => text.to_string(),
        None => "Table results: none.".to_string(),
    };
    let addendum = match analysis {
        true => format!(" {ANALYSIS_ADDENDUM}"),
        false => String::new(),
    };
    // The folder facts of a document the question names sit right after the excerpts, with one
    // sentence in the instruction; absent, the turn is the turn without them, byte for byte.
    let facts_sentence = match folder_facts {
        Some(_) => format!(" {FOLDER_FACTS_INSTRUCTION}"),
        None => String::new(),
    };
    let facts_block = match folder_facts {
        Some(text) => format!("\n\n{text}"),
        None => String::new(),
    };
    match sample {
        Some(sample) => {
            // With rows in the turn the writer does see data, so the formula sentence must not say
            // it does not: it cannot run the formula, which is what it is asked to admit.
            let instruction =
                MIXED_INSTRUCTION.replace(FORMULA_NO_DATA_PHRASE, FORMULA_WITH_SAMPLE_PHRASE);
            format!(
                "{instruction}{facts_sentence} {SAMPLE_INSTRUCTION}{addendum}\n\n{documents_block}{facts_block}\n\n{cards_block}\n\n{sample}\n\n{table_block}"
            )
        }
        None => format!(
            "{MIXED_INSTRUCTION}{facts_sentence}{addendum}\n\n{documents_block}{facts_block}\n\n{cards_block}\n\n{table_block}"
        ),
    }
}

/// What a question says about a document, among the documents of the selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedDocument {
    /// The question points at a file: exactly, ambiguously or outside the selection. Such a question is
    /// never narrowed by the knowledge base.
    pub names_a_document: bool,
    /// The one selected, readable document the question names exactly: the search is held to it and
    /// the model is told which file it is (TABULAR step 08, case I). `None` for an ambiguous name, a
    /// name outside the selection, or a document that cannot be searched.
    pub targeted: Option<String>,
}

/// Resolves the document a question names among `inventory` (the selected documents); `readable` are
/// the paths of those that were read and can be searched.
pub fn named_document(
    question: &str,
    inventory: &crate::inventory::WorkFolderInventory,
    readable: &[String],
) -> NamedDocument {
    let found =
        crate::file_reference::FileReferenceResolver::new(inventory).resolve_in_question(question);
    let names_a_document = found.is_some();
    let targeted = found
        .filter(|found| found.status == crate::file_reference::ReferenceStatus::Exact)
        .and_then(|found| found.exact_match)
        .map(|record| record.relative_path)
        .filter(|path| readable.contains(path));
    NamedDocument {
        names_a_document,
        targeted,
    }
}

/// The `Folder facts` block of a turn that targets one document: the counts of the folder and that one
/// file (name, kind, readability), never a line of its text (`work_folder_context`, view `targeted`).
/// Only the context text of tier 1 is wanted here, not its retrieval instruction.
pub fn folder_facts_block(
    inventory: &crate::inventory::WorkFolderInventory,
    relative_path: &str,
) -> String {
    format!(
        "Folder facts:\n{}",
        crate::work_folder_context::build(
            inventory,
            &crate::work_folder_context::ContextView::Targeted {
                relative_path: relative_path.to_string(),
            },
        )
        .to_prompt_block()
    )
}

// --- Entity linking: a retrieved excerpt naming a real table value ----------------------------

/// What scanning the retrieved excerpts for a real column value came to.
enum EntityLink {
    /// No excerpt word matched any reachable column's real data.
    None,
    /// Exactly one reachable column's real data matched - the filter to run the operation under.
    Filter(FilterSpec),
    /// A word matched real data in more than one reachable column: asked, never picked
    /// (`docs/SESSION-DATA-16-Mixed-Tier.md`, adversarial case "present in two columns").
    Ambiguous { value: String, candidates: Vec<String> },
}

/// Ties what the retrieved excerpts name to a real value of a text column of `sheet`/`sheet_data`.
///
/// Only text columns are linked: a number written in a document ("1 200", "2026") is not a name,
/// and matching it to an amount in the table would pick a row by accident. Two passes:
///
/// 1. **A whole value** that appears in the excerpts as the whole phrase ("MedSupply", "Fournitures
///    Dupont"). Exactly one such value links; the same value in two columns is asked about; two
///    different values (a document naming two suppliers) link nothing, rather than choosing one.
///    This pass is why "FOURNITURES MEDICALES" in a quote about MedSupply does not link to
///    "Fournitures Dupont" through the word they share (HAP-1, Q24).
/// 2. **A word of a value**, the older rule mirroring `tabular_answer::detect_filters`: the first
///    word found, in excerpt order, that is a word of a real value, folded so an unaccented
///    excerpt word still finds an accented real value. Used only when no whole value was found.
fn entity_link(
    evidence: &[Evidence],
    sheet: &crate::tabular::inventory::SheetInventory,
    sheet_data: &crate::tabular::SheetData,
) -> EntityLink {
    let text_columns: HashSet<&str> = sheet
        .columns
        .iter()
        .filter(|column| column.inferred_type == ColumnType::Categorical)
        .map(|column| column.name.as_str())
        .collect();
    let columns_words: Vec<_> = tabular_answer::column_value_words(sheet, sheet_data)
        .into_iter()
        .filter(|(column, _)| text_columns.contains(column.as_str()))
        .collect();

    let haystack: String = evidence
        .iter()
        .map(|item| question::spaced_words(&item.text))
        .collect::<Vec<_>>()
        .join(" ");
    // (folded value) -> (original value, the columns holding it), for every value that appears whole.
    let mut whole: Vec<(String, String, Vec<String>)> = Vec::new();
    for (column, words) in &columns_words {
        let mut seen: HashSet<&String> = HashSet::new();
        for value in words.values().flatten() {
            if !seen.insert(value) || !haystack.contains(&question::spaced_words(value)) {
                continue;
            }
            let folded = fold_text(value);
            match whole.iter_mut().find(|(known, _, _)| *known == folded) {
                Some((_, _, columns)) => columns.push(column.clone()),
                None => whole.push((folded, value.clone(), vec![column.clone()])),
            }
        }
    }
    match whole.as_slice() {
        [] => {}
        [(_, value, columns)] => {
            return match columns.as_slice() {
                [column] => EntityLink::Filter(FilterSpec {
                    column: column.clone(),
                    comparison: engine::Comparison::Equals(value.clone()),
                }),
                many => EntityLink::Ambiguous {
                    value: value.clone(),
                    candidates: many.to_vec(),
                },
            };
        }
        // A document naming two different values: nothing is chosen for her.
        _ => return EntityLink::None,
    }

    for item in evidence {
        // Raw words, original case kept - `question::name_words` folds (lowercases, strips
        // accents) for matching purposes, which is right for the lookup below but would report a
        // disambiguation or a filter value in the wrong case; `fold_text` is applied per word here
        // instead, the same split `detect_filters` uses on the question's own residual words.
        for word in item.text.split(|ch: char| !ch.is_alphanumeric()).filter(|word| !word.is_empty()) {
            let folded = fold_text(word);
            let matches: Vec<(&str, &str)> = columns_words
                .iter()
                .filter_map(|(column, values)| {
                    values
                        .get(&folded)
                        .and_then(|candidates| candidates.first())
                        .map(|value| (column.as_str(), value.as_str()))
                })
                .collect();
            match matches.as_slice() {
                [] => continue,
                [(column, value)] => {
                    return EntityLink::Filter(FilterSpec {
                        column: column.to_string(),
                        comparison: engine::Comparison::Equals(value.to_string()),
                    });
                }
                many => {
                    return EntityLink::Ambiguous {
                        value: word.to_string(),
                        candidates: many.iter().map(|(column, _)| column.to_string()).collect(),
                    };
                }
            }
        }
    }
    EntityLink::None
}

/// Re-classifies `question` against `inventory` (the same classifier
/// `tabular_answer::answer_sync` already ran once, re-derived here because `PendingAnswer::TryModel`
/// does not carry the operation it abandoned) and, when it is still a recognised operation, tries
/// to anchor a filter from `evidence` rather than from the question's own words. `None` whenever
/// there is no operation to re-run at all, or the sheet it named cannot be resolved, or nothing in
/// the excerpts anchors a filter either - the caller's own fallback (session 14's model-assisted
/// plan, or the nudge) is always correct in that case, since this function found nothing new to
/// add.
fn try_entity_filled_operation(
    question: &str,
    inventory: &TabularInventory,
    workbook: &Workbook,
    allowed: Option<&[String]>,
    evidence: &[Evidence],
    locale: &str,
    file: &str,
) -> Option<TabularAnswer> {
    let (route, _residual, _named_column) = question::classify_traced(question, inventory, allowed, locale);
    let TabularRoute::Operation { sheet, operation } = route else {
        return None;
    };
    let sheet_inventory = engine::resolve_sheet_inventory(inventory, sheet.as_deref(), allowed).ok()?;
    let sheet_data = workbook
        .sheets
        .iter()
        .find(|data| data.name == sheet_inventory.name)?;

    match entity_link(evidence, sheet_inventory, sheet_data) {
        EntityLink::None => None,
        EntityLink::Ambiguous { value, candidates } => Some(TabularAnswer::WhichColumn {
            file: file.to_string(),
            value,
            candidates,
        }),
        EntityLink::Filter(filter) => {
            let operation = tabular_answer::with_filters(operation, vec![filter]);
            Some(outcome_to_answer(
                file,
                engine::execute(workbook, inventory, sheet.as_deref(), allowed, &operation, locale),
            ))
        }
    }
}

/// A plain `TabularOutcome` turned into the `TabularAnswer` shape the rest of this tier and the
/// interface already expect - a smaller version of `tabular_answer`'s own `nudge()` (private to
/// that module): this path is reached only after a filter was actually found, so the richer
/// "here is an example question" nudge content is not worth re-deriving for what is already a rare
/// corner (an entity-linked filter the engine still refuses, a formula column most often).
fn outcome_to_answer(file: &str, outcome: engine::TabularOutcome) -> TabularAnswer {
    match outcome {
        engine::TabularOutcome::Value {
            value,
            locator,
            derivation,
        } => TabularAnswer::Value {
            file: file.to_string(),
            value,
            locator,
            derivation,
            model_attempt: None,
        },
        engine::TabularOutcome::NotDeterministicallyAnswerable {
            reason,
            available_sheets,
            available_columns,
        } => TabularAnswer::Nudge {
            file: file.to_string(),
            reason: Some(reason),
            available_sheets,
            available_columns,
            example_column: None,
            example_group: None,
            filter_column: None,
            filter_value: None,
            close_values: Vec::new(),
            model_attempt: None,
        },
    }
}

// --- Post-generation checks: mechanism 5 (numbers) and its citation sibling -------------------

/// The table's own scalar, when `value` carries exactly one number - the only shape this tier
/// checks the model's prose against. A list-shaped value (`Distinct`, `GroupSums`, a ranking, a
/// row) has no single number to compare against and is out of this check's scope: the interface
/// still shows the real value beside the answer either way, through `table` itself.
pub(crate) fn scalar_of(value: &TabularValue) -> Option<f64> {
    match value {
        TabularValue::Count(count) => Some(*count as f64),
        TabularValue::Sum(aggregate)
        | TabularValue::Min(aggregate)
        | TabularValue::Max(aggregate)
        | TabularValue::Mean(aggregate)
        | TabularValue::Median(aggregate) => Some(aggregate.value),
        _ => None,
    }
}

/// Every number in `answer` that matches neither `expected` (the table's own scalar) nor appears
/// verbatim in one of `excerpts` - mechanism 5. `expected: None` (no scalar table value exists for
/// this question) makes this a no-op: there is nothing to check a claimed number against, so
/// nothing is flagged, rather than guessing. Deduplicated: the same altered number repeated twice
/// in one answer is one correction, not two.
pub(crate) fn verify_numbers(
    answer: &str,
    expected: Option<f64>,
    excerpts: &[Evidence],
    other_known_texts: &[&str],
) -> Vec<NumericCorrection> {
    let Some(expected) = expected else {
        return Vec::new();
    };
    let known: Vec<number_check::Number> = excerpts
        .iter()
        .map(|excerpt| excerpt.text.as_str())
        .chain(other_known_texts.iter().copied())
        .flat_map(|text| number_check::scan(text, 1))
        .collect();
    let mut corrections = Vec::new();
    let mut seen = HashSet::new();
    // Read whole, the way a person writes it: "1 450,00" is one amount, not a "450,00" that would
    // contradict a table value of 1 450 (HAP-1, lot D replay: three false corrections).
    for number in number_check::scan(answer, 3) {
        if number.values.iter().any(|value| (value - expected).abs() < 1e-6) {
            continue;
        }
        let written_in_an_excerpt = excerpts.iter().any(|excerpt| excerpt.text.contains(&number.text))
            || other_known_texts.iter().any(|text| text.contains(&number.text))
            || known.iter().any(|other| {
                other
                    .values
                    .iter()
                    .any(|a| number.values.iter().any(|b| (a - b).abs() < 1e-6))
            });
        if written_in_an_excerpt {
            continue;
        }
        if seen.insert(number.text.clone()) {
            corrections.push(NumericCorrection {
                claimed: number.text,
                correct: expected,
            });
        }
    }
    corrections
}

/// Every bracketed citation (`[N]`) in `answer` whose number is not a valid 1-based index into the
/// `document_count` excerpts actually supplied - most often a table fact the model dressed up as a
/// document citation, or a citation left over from the model's own training rather than this
/// question's real sources. Deduplicated, same reasoning as `verify_numbers`.
pub(crate) fn reject_citations(answer: &str, document_count: usize) -> Vec<String> {
    let mut rejected = Vec::new();
    let mut seen = HashSet::new();
    let chars: Vec<char> = answer.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '[' {
            let digits_start = index + 1;
            let mut cursor = digits_start;
            while cursor < chars.len() && chars[cursor].is_ascii_digit() {
                cursor += 1;
            }
            if cursor > digits_start && chars.get(cursor) == Some(&']') {
                let number: String = chars[digits_start..cursor].iter().collect();
                if let Ok(parsed) = number.parse::<usize>() {
                    if parsed == 0 || parsed > document_count {
                        let marker = format!("[{number}]");
                        if seen.insert(marker.clone()) {
                            rejected.push(marker);
                        }
                    }
                }
                index = cursor + 1;
                continue;
            }
        }
        index += 1;
    }
    rejected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extraction::PageOrigin;

    #[test]
    fn the_mixed_instruction_stays_neutral_about_who_the_user_is() {
        let lower = MIXED_INSTRUCTION.to_lowercase();
        for word in ["patient", "doctor", "practitioner", "practice", "gp"] {
            assert!(!lower.contains(word), "{word:?} found in MIXED_INSTRUCTION");
        }
    }

    #[test]
    fn the_mixed_instruction_names_every_block_the_turn_can_carry() {
        for label in ["'Document excerpts'", "'Workbook cards'", "'Table results'"] {
            assert!(MIXED_INSTRUCTION.contains(label), "{label} missing");
        }
        // The formula rule step 07 relies on: one formula per line, starting with `=`.
        assert!(MIXED_INSTRUCTION.contains("on a line of its own starting with ="));
        assert!(MIXED_INSTRUCTION.contains("cannot test it"));
        // Measured on 22 October 2026: without this sentence `qwen3:4b-instruct` refused to summarise
        // or relate workbooks "for lack of data" although their structure was in the cards.
        assert!(MIXED_INSTRUCTION.contains("do not refuse a question about structure"));
        // The labels in the instruction are the ones the blocks are written with.
        assert!(crate::tabular::card::CARDS_HEADER.starts_with("Workbook cards"));
    }

    #[test]
    fn the_formula_repair_instruction_stays_neutral_about_who_the_user_is() {
        let lower = FORMULA_REPAIR_INSTRUCTION.to_lowercase();
        for word in ["patient", "doctor", "practitioner", "practice", "gp"] {
            assert!(
                !lower.contains(word),
                "{word:?} found in FORMULA_REPAIR_INSTRUCTION"
            );
        }
        // The same rule as the writer's: one formula per line starting with `=`, names in the
        // conventions of the answer language.
        assert!(FORMULA_REPAIR_INSTRUCTION.contains("on a line of its own starting with ="));
        assert!(FORMULA_REPAIR_INSTRUCTION.contains("conventions of the answer language"));
    }

    #[test]
    fn the_writer_is_told_to_describe_its_sources_in_its_own_words() {
        // Measured on 23 October 2026: `qwen3:1.7b` copied the English lines of the cards into an answer
        // in another language.
        assert!(MIXED_INSTRUCTION.contains("Write in the language of the question"));
        assert!(MIXED_INSTRUCTION.contains("do not copy their lines"));
    }

    fn note(
        code: formula_check::FormulaIssueCode,
        found: &str,
        expected: Option<&str>,
    ) -> FormulaNote {
        FormulaNote {
            formula: "=F(A1)".to_string(),
            issue: FormulaIssue {
                code,
                found: found.to_string(),
                expected: expected.map(str::to_string),
                place: None,
            },
        }
    }

    #[test]
    fn the_repair_turn_lists_the_problems_a_second_request_can_fix_and_no_other() {
        use formula_check::FormulaIssueCode::*;
        let turn = repair_turn(&[
            note(FunctionInOtherLanguage, "SUMIF", Some("SOMME.SI")),
            note(WrongSeparator, ",", Some(";")),
            note(UnknownColumnLetter, "F", Some("E")),
            note(ReachesOutside, "WEBSERVICE", None),
        ]);
        assert!(turn.starts_with(FORMULA_REPAIR_INSTRUCTION));
        assert!(turn.contains("\nProblems:\n"));
        assert!(turn.contains("=F(A1) - function_in_other_language: SUMIF (expected: SOMME.SI)"));
        assert!(turn.contains("unknown_column_letter: F (expected: E)"));
        assert!(!turn.contains("wrong_separator"));
        assert!(!turn.contains("reaches_outside"));
        let lower = turn.to_lowercase();
        for word in ["patient", "doctor", "practitioner", "practice"] {
            assert!(!lower.contains(word), "{word:?} found in the repair turn");
        }
    }

    #[test]
    fn the_analysis_addendum_stays_neutral_about_who_the_user_is() {
        let lower = ANALYSIS_ADDENDUM.to_lowercase();
        for word in ["patient", "doctor", "practitioner", "practice", "gp"] {
            assert!(!lower.contains(word), "{word:?} found in ANALYSIS_ADDENDUM");
        }
    }

    #[test]
    fn the_folder_facts_instruction_stays_neutral_about_who_the_user_is() {
        let lower = FOLDER_FACTS_INSTRUCTION.to_lowercase();
        for word in ["patient", "doctor", "practitioner", "practice", "gp"] {
            assert!(
                !lower.contains(word),
                "{word:?} found in FOLDER_FACTS_INSTRUCTION"
            );
        }
        assert!(FOLDER_FACTS_INSTRUCTION.contains("'Folder facts'"));
    }

    #[test]
    fn the_folder_facts_of_a_named_document_sit_after_the_excerpts_and_change_nothing_else() {
        let facts = "Folder facts:\nWORK_FOLDER_CONTEXT\n{\"view\":\"targeted\"}\n";
        let with = build_turn_for(
            &[],
            Some(CARDS),
            None,
            Some(FIGURE_BLOCK),
            Some(facts),
            false,
        );
        let without = build_turn_for(&[], Some(CARDS), None, Some(FIGURE_BLOCK), None, false);

        // One sentence joins the instruction, the block sits between the excerpts and the cards.
        assert_eq!(
            with,
            format!(
                "{MIXED_INSTRUCTION} {FOLDER_FACTS_INSTRUCTION}\n\nDocument excerpts: none.\n\n{facts}\n\n{CARDS}\n\n{FIGURE_BLOCK}"
            )
        );
        // Absent, the turn is the turn without them, byte for byte (the frozen turns of the baselines).
        assert_eq!(
            without,
            format!("{MIXED_INSTRUCTION}\n\nDocument excerpts: none.\n\n{CARDS}\n\n{FIGURE_BLOCK}")
        );
        assert!(!without.contains("Folder facts"));
    }

    #[test]
    fn the_analysis_turn_adds_the_addendum_to_the_instruction_and_nothing_else() {
        let ordinary = build_turn_for(&[], Some(CARDS), None, Some(FIGURE_BLOCK), None, false);
        assert_eq!(
            ordinary,
            build_turn(&[], Some(CARDS), None, Some(FIGURE_BLOCK))
        );
        assert!(!ordinary.contains("asked for an analysis"));

        let analysis = build_turn_for(&[], Some(CARDS), None, Some(FIGURE_BLOCK), None, true);
        assert_eq!(
            analysis,
            format!(
                "{MIXED_INSTRUCTION} {ANALYSIS_ADDENDUM}\n\nDocument excerpts: none.\n\n{CARDS}\n\n{FIGURE_BLOCK}"
            )
        );
    }

    #[test]
    fn the_table_block_carries_the_calculation_record_without_the_question() {
        use crate::tabular::engine::{NumericAggregate, TabularDerivation, TabularLocator};
        let answer = Some(TabularAnswer::Value {
            file: "commandes.xlsx".to_string(),
            value: TabularValue::Sum(NumericAggregate {
                value: 8412.5,
                unit: None,
                unparsed: 0,
            }),
            locator: TabularLocator {
                sheet: "Commandes".to_string(),
                header_row: Some(0),
                column: Some("Montant".to_string()),
                row_range: Some((0, 40)),
                filters: Vec::new(),
            },
            derivation: TabularDerivation::Computed {
                operation: "sum".to_string(),
                row_count: 41,
            },
            model_attempt: None,
        });
        let sizes = [tabular_answer::SheetSize {
            file: "commandes.xlsx".to_string(),
            sheet: "Commandes".to_string(),
            data_rows: 124,
        }];
        let block = table_evidence_text(&answer, &sizes).expect("a block");
        assert!(block
            .starts_with("Table results (workbook \"commandes.xlsx\"):\nCalculation record:\n"));
        assert!(block.contains("Rows: 41 kept out of 124 data rows"));
        assert!(block.ends_with("Result: 8412.5"));
        assert!(!block.contains("Question:"));
    }

    #[test]
    fn the_blocks_of_a_turn_stay_neutral_about_who_the_user_is() {
        let turn = build_turn(&[], None, None, None).to_lowercase();
        for word in ["patient", "doctor", "practitioner", "practice", "gp"] {
            assert!(!turn.contains(word), "{word:?} found in the empty turn");
        }
    }

    // The instruction, the document share, the card share, the sample and profile shares, a table
    // block and a long question fit under the default window with room left for the history.
    // Step 10 adds its rows share here.
    #[test]
    fn the_shares_fit_the_default_window() {
        use crate::tabular::sample::{PROFILE_SHARE_CHARS, SAMPLE_INSTRUCTION, SAMPLE_SHARE_CHARS};
        let question = "q".repeat(400);
        let table_block = format!("Table results (workbook \"t.xlsx\"):\n{}", "t".repeat(300));
        let fixed = MIXED_INSTRUCTION.chars().count()
            + SAMPLE_INSTRUCTION.chars().count()
            + MIXED_DOCUMENT_SHARE_CHARS
            + crate::tabular::card::CARD_SHARE_CHARS
            + SAMPLE_SHARE_CHARS
            + PROFILE_SHARE_CHARS
            + table_block.chars().count()
            + question.chars().count();
        let budget = ContextBudget::DEFAULT;
        assert!(
            fixed < budget.prompt_chars(),
            "{fixed} characters of fixed shares"
        );
        assert!(
            budget.history_chars(fixed) >= 3_000,
            "{} characters left for the history",
            budget.history_chars(fixed)
        );
    }

    #[test]
    fn the_sample_sits_between_the_cards_and_the_table_results_and_off_adds_nothing() {
        use crate::tabular::sample::SAMPLE_INSTRUCTION;
        let sample =
            "Table sample and profile of the user's tables:\n<<DATA-abc123>>\nrows\n<<END-abc123>>";
        let with = build_turn(&[], Some(CARDS), Some(sample), Some(FIGURE_BLOCK));
        let cards = with.find("Workbook cards (structure only").expect("cards");
        let block = with.find("<<DATA-abc123>>").expect("sample");
        let table = with.find("Table results (workbook").expect("table");
        assert!(cards < block && block < table);
        // The instruction of a turn with a sample no longer says the data is not seen.
        assert!(MIXED_INSTRUCTION.contains(FORMULA_NO_DATA_PHRASE));
        assert!(!with.contains(FORMULA_NO_DATA_PHRASE));
        assert!(with.contains(FORMULA_WITH_SAMPLE_PHRASE));
        assert!(with.contains(SAMPLE_INSTRUCTION));

        // With no sample the turn is the turn of step 03, byte for byte.
        let without = build_turn(&[], Some(CARDS), None, Some(FIGURE_BLOCK));
        assert_eq!(
            without,
            format!("{MIXED_INSTRUCTION}\n\nDocument excerpts: none.\n\n{CARDS}\n\n{FIGURE_BLOCK}")
        );
        assert!(!without.contains("Table sample"));
        assert!(!without.contains("data markers"));
    }

    fn evidence(text: &str) -> Evidence {
        Evidence {
            chunk_id: "chunk-1".to_string(),
            relative_path: "contrat.pdf".to_string(),
            page_number: 1,
            ordinal: 1,
            section: None,
            text: text.to_string(),
            score: 1.0,
            origin: PageOrigin::TextLayer,
            confidence: None,
        }
    }

    // The turn the model receives, frozen in full for TABULAR step 01 (20 October 2026) and changed
    // on purpose by step 03: the turn now carries three blocks (documents, workbook cards, table
    // results) and the table block names its workbook. The excerpt block ends with a blank line of
    // its own, hence the four newlines before the next block.
    const FIGURE_BLOCK: &str = "Table results (workbook \"planning.xlsx\"):\n\
         Sheet Planning, column amount: sum = 1840 (computed over 8 rows)";
    const CARDS: &str = "Workbook cards (structure only, no data values):\n\
         Workbook \"planning.xlsx\" (xlsx)\n  Sheet \"Planning\": 8 data rows";

    #[test]
    fn the_turn_with_documents_and_no_other_block() {
        let turn = build_turn(
            &[evidence("The contract covers all suppliers.")],
            None,
            None,
            None,
        );
        assert_eq!(
            turn,
            format!(
                "{MIXED_INSTRUCTION}\n\n\
                 Document excerpts:\n\
                 [1] contrat.pdf (page 1):\n\
                 The contract covers all suppliers.\n\n\n\n\
                 Workbook cards: none.\n\n\
                 Table results: none."
            )
        );
    }

    #[test]
    fn the_turn_with_documents_and_one_figure() {
        let turn = build_turn(
            &[evidence("The contract covers all suppliers.")],
            None,
            None,
            Some(FIGURE_BLOCK),
        );
        assert_eq!(
            turn,
            format!(
                "{MIXED_INSTRUCTION}\n\n\
                 Document excerpts:\n\
                 [1] contrat.pdf (page 1):\n\
                 The contract covers all suppliers.\n\n\n\n\
                 Workbook cards: none.\n\n\
                 {FIGURE_BLOCK}"
            )
        );
    }

    #[test]
    fn the_turn_with_no_documents_and_one_figure() {
        let turn = build_turn(&[], None, None, Some(FIGURE_BLOCK));
        assert_eq!(
            turn,
            format!(
                "{MIXED_INSTRUCTION}\n\n\
                 Document excerpts: none.\n\n\
                 Workbook cards: none.\n\n\
                 {FIGURE_BLOCK}"
            )
        );
    }

    #[test]
    fn the_cards_sit_between_the_documents_and_the_table_results() {
        let turn = build_turn(
            &[evidence("The contract covers all suppliers.")],
            Some(CARDS),
            None,
            Some(FIGURE_BLOCK),
        );
        let documents = turn.find("Document excerpts:").expect("documents");
        let cards = turn.find("Workbook cards (structure only").expect("cards");
        let table = turn.find("Table results (workbook").expect("table");
        assert!(documents < cards && cards < table);
        assert!(!turn.contains("Workbook cards: none."));
    }

    #[test]
    fn the_table_block_names_the_workbook_by_its_plain_path() {
        use crate::tabular::engine::{TabularDerivation, TabularLocator};
        let value = |file: &str| {
            Some(TabularAnswer::Value {
                file: file.to_string(),
                value: TabularValue::Count(8),
                locator: TabularLocator {
                    sheet: "Planning".to_string(),
                    header_row: Some(0),
                    column: None,
                    row_range: None,
                    filters: Vec::new(),
                },
                derivation: TabularDerivation::Computed {
                    operation: "count".to_string(),
                    row_count: 8,
                },
                model_attempt: None,
            })
        };
        let native = table_evidence_text(&value("factures/planning.xlsx"), &[]).expect("a block");
        assert!(
            native.starts_with("Table results (workbook \"factures/planning.xlsx\"):\n"),
            "{native}"
        );
        let foreign =
            table_evidence_text(&value("/documents/planning.xlsx"), &[]).expect("a block");
        assert!(
            foreign.starts_with(
                "Table results (workbook \"planning.xlsx\", in the documents folder):\n"
            ),
            "{foreign}"
        );
        assert_eq!(table_evidence_text(&None, &[]), None);
    }

    #[test]
    fn a_figure_a_card_states_is_not_corrected_against_the_table() {
        // The card says 124 data rows; the table figure is 8412.5. Quoting the row count is fine.
        let corrections = verify_numbers(
            "The sheet has 124 rows and the total is 8412.5.",
            Some(8412.5),
            &[],
            &["Sheet \"Factures\": 124 data rows"],
        );
        assert!(corrections.is_empty(), "{corrections:?}");
        let invented = verify_numbers(
            "The sheet has 999 rows.",
            Some(8412.5),
            &[],
            &["124 data rows"],
        );
        assert_eq!(invented.len(), 1);
    }

    #[test]
    fn an_altered_number_is_flagged_against_the_table() {
        let corrections =
            verify_numbers("The total billed was 1480 euros.", Some(1840.0), &[], &[]);
        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].claimed, "1480");
        assert_eq!(corrections[0].correct, 1840.0);
    }

    #[test]
    fn the_real_number_is_never_flagged() {
        let corrections =
            verify_numbers("The total billed was 1840 euros.", Some(1840.0), &[], &[]);
        assert!(corrections.is_empty());
    }

    #[test]
    fn a_number_quoted_verbatim_from_an_excerpt_is_not_flagged() {
        // The excerpt itself states 1480 - a genuine document fact, not a claim about the table -
        // so it must not be corrected away even though it differs from the table's own 1840.
        let excerpts = [evidence("The invoice referenced an earlier estimate of 1480 euros.")];
        let corrections = verify_numbers(
            "An earlier estimate of 1480 euros is mentioned.",
            Some(1840.0),
            &excerpts,
            &[],
        );
        assert!(corrections.is_empty());
    }

    #[test]
    fn a_short_number_such_as_a_page_reference_is_left_alone() {
        let corrections = verify_numbers("See page 3 for details.", Some(1840.0), &[], &[]);
        assert!(corrections.is_empty());
    }

    #[test]
    fn no_scalar_table_value_means_nothing_is_checked() {
        let corrections = verify_numbers("The total billed was 1480 euros.", None, &[], &[]);
        assert!(corrections.is_empty());
    }

    #[test]
    fn a_citation_within_range_is_accepted() {
        assert!(reject_citations("As shown in [1], the amount matches.", 2).is_empty());
    }

    #[test]
    fn a_table_fact_dressed_up_as_a_document_citation_is_rejected() {
        let rejected = reject_citations("As shown in [2], the total is 1840.", 1);
        assert_eq!(rejected, vec!["[2]".to_string()]);
    }

    #[test]
    fn a_repeated_bad_citation_is_reported_once() {
        let rejected = reject_citations("See [5] and again [5].", 1);
        assert_eq!(rejected, vec!["[5]".to_string()]);
    }
}
