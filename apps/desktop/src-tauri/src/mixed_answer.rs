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
//! The model never receives a cell value or a row: the table block is built from
//! `tabular::escalation::format_evidence`, the same function tier 2 keeps unsent for exactly this
//! session, which already refuses a raw row list. Entity linking (`entity_link`) reads retrieved
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
use crate::tabular::inventory::{ColumnType, TabularInventory};
use crate::tabular::question::{self, TabularRoute};
use crate::tabular::Workbook;
use crate::tabular_answer::{self, PendingAnswer, TabularAnswer};

/// English, profession-neutral, short - the same reasons every other model-facing constant in
/// this product is (`docs/DECISIONS.md`, "profession-neutral model-facing text"). Tells the model
/// the two blocks are the only sources, forbids it from computing or swapping one block in for
/// the other, and asks it to say plainly when a part cannot be answered rather than guessing -
/// mechanisms 3, 4 and 6 of `docs/SESSION-DATA-REFERENCE-report.md` section 5, stated as one
/// instruction the way `RETRIEVAL_INSTRUCTION` and `TABULAR_INSTRUCTION` each state their own tier.
pub const MIXED_INSTRUCTION: &str =
    "You are given two sources: a 'Document excerpts' block, retrieved from the user's own \
     documents, and a 'Table results' block, computed directly from the user's own data table, \
     not by you. Treat these two blocks as the only sources - never use one in place of the \
     other, and never compute, re-derive or second-guess a number in the table results: report it \
     exactly as given. Cite a document fact with its bracketed number, exactly as supplied, and \
     never cite a document number for a fact that came from the table instead. If one block \
     cannot establish part of the question, say so plainly instead of guessing.";

/// How much of the document share of the budget this tier sends, under the existing cap
/// (mechanism 4: "the context cap is split between the blocks, a documented share each"). Smaller
/// than retrieval's own `MAX_EVIDENCE_CHARS`, since the table block and the instruction also have
/// to fit beside it in the same turn.
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
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MixedAnswer {
    /// The model's prose. Empty when generation was never attempted - the table answered alone,
    /// with no gateway call, because the document side had nothing to add
    /// (`documents_unavailable`).
    pub answer: String,
    /// The tabular part's own computed value, when the question had one. `None` exactly when
    /// `table_unavailable` is set.
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
    let document_sources = cap_document_share(&document_sources, MIXED_DOCUMENT_SHARE_CHARS);
    on_sources(&document_sources);
    let documents_unavailable = documents_unavailable
        .or_else(|| document_sources.is_empty().then_some(MixedPartUnavailable::NoEvidence));

    // The tabular part, finished now that the excerpts exist to link an entity against.
    let (table, table_unavailable) = resolve_table(pending, question, context, &document_sources).await;
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
                },
                false,
            ));
        }
    }
    let computed_value = matches!(table, Some(TabularAnswer::Value { .. }));
    if table.is_none() && table_unavailable.is_none() {
        table_unavailable = Some(MixedPartUnavailable::NotAskedAbout);
    }

    // Neither side had anything to answer from at all.
    if documents_unavailable.is_some() && table_unavailable.is_some() {
        return Err(AppError::InsufficientEvidence);
    }

    let table_text = table_evidence_text(&table);
    let expected = match &table {
        Some(TabularAnswer::Value { value, .. }) => scalar_of(value),
        _ => None,
    };

    // Zero gateway calls when there is nothing on the document side to write about - the same
    // guarantee a deterministic tabular answer already keeps (`docs/DECISIONS.md`), extended to a
    // mixed question whose document half came back empty.
    if let Some(reason) = documents_unavailable {
        return Ok((
            MixedAnswer {
                answer: String::new(),
                table,
                table_unavailable,
                document_sources: Vec::new(),
                documents_unavailable: Some(reason),
                corrections: Vec::new(),
                rejected_citations: Vec::new(),
            },
            false,
        ));
    }

    let instruction = build_two_block_turn(&document_sources, table_text.as_deref());
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
        .chat(
            context.server_url,
            context.model_alias,
            Some(context.locale),
            &turns,
            context.idle_timeout,
            |delta| on_delta(delta),
        )
        .await;

    let answer_text = match generated {
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
                },
                true,
            ));
        }
        Err(error) => return Err(error),
    };

    let corrections = verify_numbers(&answer_text, expected, &document_sources);
    let rejected_citations = reject_citations(&answer_text, document_sources.len());

    Ok((
        MixedAnswer {
            answer: answer_text,
            table,
            table_unavailable,
            document_sources,
            documents_unavailable: None,
            corrections,
            rejected_citations,
        },
        true,
    ))
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
) -> (Option<TabularAnswer>, Option<MixedPartUnavailable>) {
    match pending {
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
                return (None, Some(MixedPartUnavailable::NotAskedAbout));
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

/// The table block's own evidence text, built the same way tier 2 keeps unsent
/// (`tabular::escalation::format_evidence`): never a raw row list, never a cell value outside the
/// one computed result. `None` for anything that is not a computed `Value` - a disambiguation
/// never reaches this function at all (the caller returns before generation for those), and a
/// content-only question legitimately has no table evidence to send.
fn table_evidence_text(table: &Option<TabularAnswer>) -> Option<String> {
    match table {
        Some(TabularAnswer::Value {
            value,
            locator,
            derivation,
            ..
        }) => crate::tabular::escalation::format_evidence(&engine::TabularOutcome::Value {
            value: value.clone(),
            locator: locator.clone(),
            derivation: derivation.clone(),
        }),
        _ => None,
    }
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

/// The instruction plus both blocks, one turn ready to send - the mixed-tier counterpart of
/// `retrieval::build_context_turn` and `tabular::escalation::build_context_turn`. `document_sources`
/// must already be the capped set (`cap_document_share`): this function only formats what it is
/// given, so citation numbers in the model's answer and `document_sources.len()` always agree.
pub(crate) fn build_two_block_turn(document_sources: &[Evidence], table_text: Option<&str>) -> String {
    let documents_block = if document_sources.is_empty() {
        "Document excerpts: none.".to_string()
    } else {
        format!(
            "Document excerpts:\n{}",
            crate::retrieval::format_evidence(document_sources)
        )
    };
    let table_block = match table_text {
        Some(text) => format!("Table results:\n{text}"),
        None => "Table results: none.".to_string(),
    };
    format!("{MIXED_INSTRUCTION}\n\n{documents_block}\n\n{table_block}")
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
) -> Vec<NumericCorrection> {
    let Some(expected) = expected else {
        return Vec::new();
    };
    let known: Vec<number_check::Number> = excerpts
        .iter()
        .flat_map(|excerpt| number_check::scan(&excerpt.text, 1))
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

    fn evidence(text: &str) -> Evidence {
        Evidence {
            chunk_id: "chunk-1".to_string(),
            relative_path: "contrat.pdf".to_string(),
            page_number: 1,
            section: 1,
            text: text.to_string(),
            score: 1.0,
            origin: PageOrigin::TextLayer,
            confidence: None,
        }
    }

    #[test]
    fn an_altered_number_is_flagged_against_the_table() {
        let corrections = verify_numbers("The total billed was 1480 euros.", Some(1840.0), &[]);
        assert_eq!(corrections.len(), 1);
        assert_eq!(corrections[0].claimed, "1480");
        assert_eq!(corrections[0].correct, 1840.0);
    }

    #[test]
    fn the_real_number_is_never_flagged() {
        let corrections = verify_numbers("The total billed was 1840 euros.", Some(1840.0), &[]);
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
        );
        assert!(corrections.is_empty());
    }

    #[test]
    fn a_short_number_such_as_a_page_reference_is_left_alone() {
        let corrections = verify_numbers("See page 3 for details.", Some(1840.0), &[]);
        assert!(corrections.is_empty());
    }

    #[test]
    fn no_scalar_table_value_means_nothing_is_checked() {
        let corrections = verify_numbers("The total billed was 1480 euros.", None, &[]);
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
