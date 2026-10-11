//! The knowledge base at question time, without the window: plan the reduction, search with it,
//! widen when it was too narrow, and say what happened.
//!
//! `commands.rs` is the thin wrapper that reads the settings and the folders and forwards events;
//! everything that decides lives here so it is tested without a Tauri runtime:
//!
//! ```text
//! plan()              before the question is embedded: detect the entities, decide the reduction
//! search_documents()  after it: the reduced search, widened once on the full selection when it
//!                     finds nothing or little; the old search, untouched, whenever nothing applies
//! narrowed_data_selection() / decorate_which_workbook()   the workbook side
//! note_*() / report() what the question's diagnostics and the interface are told
//! ```
//!
//! The principle of `reduce` holds throughout: the user's selection is the authority, and every
//! failure runs the old path on the full selection (invariant I4). In `suggest` the search is
//! exactly the one that runs with the knowledge base off, so the shadow comparison compares like
//! with like.

use std::collections::BTreeSet;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::diagnostics::{
    elapsed_ms, QueryEntityDiagnostic, RetrievalDiagnostics, ShadowDiagnostics,
    TargetingDiagnostics,
};
use super::normalize::TitleSet;
use super::packs::PackSet;
use super::phonetic::{encoder_for_locale, PhoneticEncoder};
use super::query::{DetectionInput, QueryAnalysis, QueryEntityDetector, QuerySelection};
use super::reduce::{
    compare_with_cited, reduce, ReduceInput, ReductionPlan, SideInput, SkipReason, TargetingPolicy,
    MIN_REDUCTION_CONFIDENCE,
};
use super::resolve::DeterministicResolver;
use super::secret::IdentifierKey;
use super::{Domain, EntityStatus, KnowledgeMode};
use crate::analysis_scope::{ScopeEntry, ScopeMode};
use crate::data_folder::DataFolder;
use crate::error::AppError;
use crate::index_store::IndexStore;
use crate::retrieval::{self, Evidence, RetrievalScope, SearchOutcome};
use crate::tabular_answer::{TabularAnswer, WorkbookHint, WorkbookHintEntity};

/// The words and rules a question is read with: the packs, the titles and the phonetic encoder of
/// the locale. Loaded once and kept for the session (tens of milliseconds), never per question.
pub struct QueryVocabulary {
    pub packs: PackSet,
    pub titles: TitleSet,
    pub encoder: Box<dyn PhoneticEncoder>,
    resolver: DeterministicResolver,
}

impl QueryVocabulary {
    /// The vocabulary a pass reads names with: the packs of the settings, and the type vocabulary
    /// of every shipped pack (a module never changes a type).
    pub fn load(locale: &str, active_packs: &[&str]) -> Result<Self, AppError> {
        let packs = PackSet::load_with_type_vocabulary(locale, active_packs)?;
        Ok(Self {
            titles: packs.title_set(),
            packs,
            encoder: encoder_for_locale(locale),
            resolver: DeterministicResolver,
        })
    }
}

/// What the client asks of the knowledge base for one question.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TargetingRequest {
    /// `off` is the widen action: the old path on the full selection.
    pub mode: RequestMode,
    /// Entities the user picked as chips.
    pub entity_ids: Vec<i64>,
    /// The previous answer's entities, held by the client; honoured only for a short follow-up.
    pub carry_over: Vec<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestMode {
    #[default]
    Auto,
    Off,
}

/// What a question is planned with, besides the question: the vocabulary, the key for personal
/// identifiers, the settings.
pub struct TargetingTools<'a> {
    pub vocabulary: &'a QueryVocabulary,
    pub key: Option<&'a IdentifierKey>,
    pub policy: TargetingPolicy,
}

/// The knowledge base's decision for one question.
#[derive(Debug, Clone)]
pub struct TargetingPlan {
    pub mode: KnowledgeMode,
    pub user_widened: bool,
    /// What the question named. `None` when the base was not asked (mode off, widen, nothing to
    /// reduce).
    pub analysis: Option<QueryAnalysis>,
    pub reduction: ReductionPlan,
    /// Milliseconds spent reading the question and deciding.
    pub knowledge_ms: u64,
    policy: TargetingPolicy,
}

/// Read the question and decide the reduction. Never fails: a base that cannot be read gives a plan
/// that reduces nothing, for the reason `lookup_error`.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    index: &IndexStore,
    tools: &TargetingTools<'_>,
    mode: KnowledgeMode,
    request: &TargetingRequest,
    question: &str,
    documents: SideInput<'_>,
    data: SideInput<'_>,
) -> TargetingPlan {
    let started = Instant::now();
    let selected_documents = documents.files.len();
    let selected_workbooks = data.files.len();
    let finish = |analysis: Option<QueryAnalysis>, reduction: ReductionPlan, user_widened: bool| {
        TargetingPlan {
            mode,
            user_widened,
            analysis,
            reduction,
            knowledge_ms: elapsed_ms(started.elapsed()),
            policy: tools.policy,
        }
    };
    if !mode.reads_files() {
        return finish(
            None,
            ReductionPlan::skipped_everywhere(
                selected_documents,
                selected_workbooks,
                SkipReason::Off,
            ),
            false,
        );
    }
    if request.mode == RequestMode::Off {
        return finish(
            None,
            ReductionPlan::skipped_everywhere(
                selected_documents,
                selected_workbooks,
                SkipReason::UserWidened,
            ),
            true,
        );
    }

    let input = ReduceInput {
        mode,
        user_widened: false,
        documents,
        data,
        policy: tools.policy,
    };
    // Both sides are settled before the base is asked (a named file, nothing selected): the
    // question is not even read.
    let settled = |side: &SideInput<'_>| side.blocked.is_some() || side.files.is_empty();
    if settled(&documents) && settled(&data) {
        let reduction = reduce(index, &QueryAnalysis::default(), &input);
        return finish(None, reduction, false);
    }

    let detector = QueryEntityDetector {
        connection: index.connection(),
        packs: &tools.vocabulary.packs,
        titles: &tools.vocabulary.titles,
        encoder: tools.vocabulary.encoder.as_ref(),
        resolver: &tools.vocabulary.resolver,
        key: tools.key,
    };
    let analysis = match detector.detect(&DetectionInput {
        question,
        selection: QuerySelection {
            documents: documents.files,
            data: data.files,
        },
        explicit_entity_ids: &request.entity_ids,
        carry_over: &request.carry_over,
    }) {
        Ok(analysis) => analysis,
        Err(_) => {
            return finish(
                None,
                ReductionPlan::skipped_everywhere(
                    selected_documents,
                    selected_workbooks,
                    SkipReason::LookupError,
                ),
                false,
            );
        }
    };
    let reduction = reduce(index, &analysis, &input);
    finish(Some(analysis), reduction, false)
}

impl TargetingPlan {
    /// A plan that reduces nothing, for a reason: the vocabulary could not be loaded, say. The
    /// question runs on the full selection as if the knowledge base were off.
    pub fn unplanned(
        mode: KnowledgeMode,
        policy: TargetingPolicy,
        selected_documents: usize,
        selected_workbooks: usize,
        reason: SkipReason,
    ) -> Self {
        Self {
            mode,
            user_widened: false,
            analysis: None,
            reduction: ReductionPlan::skipped_everywhere(
                selected_documents,
                selected_workbooks,
                reason,
            ),
            knowledge_ms: 0,
            policy,
        }
    }

    /// The documents to search, when a reduction applies to them.
    pub fn applied_documents(&self) -> Option<&[String]> {
        self.reduction
            .documents
            .is_applied()
            .then_some(self.reduction.documents.candidates.as_slice())
    }

    /// The workbooks to answer from, when a reduction applies to them.
    pub fn applied_workbooks(&self) -> Option<&[String]> {
        self.reduction
            .data
            .is_applied()
            .then_some(self.reduction.data.candidates.as_slice())
    }

    /// The other spellings of the names the question resolved to, added to the **lexical** part of
    /// the search only, and only in `auto` (in `suggest` the answer is exactly the one the base
    /// being off gives, so the shadow comparison means something). Never after a widen.
    pub fn extra_lexical_terms(&self) -> Vec<String> {
        if self.mode != KnowledgeMode::Auto || self.user_widened {
            return Vec::new();
        }
        let Some(analysis) = &self.analysis else {
            return Vec::new();
        };
        let mut terms: BTreeSet<String> = BTreeSet::new();
        for entity in &analysis.entities {
            if entity.confidence >= MIN_REDUCTION_CONFIDENCE
                && entity.status != EntityStatus::Candidate
            {
                terms.extend(entity.terms.iter().cloned());
            }
        }
        terms.into_iter().collect()
    }

    /// Who asked for the reduction: the rules, or a chip the user picked.
    pub fn requested_by(&self) -> &'static str {
        if self.reduction.documents.chip_driven || self.reduction.data.chip_driven {
            "chip"
        } else {
            "auto"
        }
    }

    /// What to tell the interface, or `None` when there is nothing to tell: no reduction was applied
    /// and no name needs a question. In `suggest` an unapplied reduction is invisible by design.
    pub fn report(&self, widened: bool) -> Option<TargetingReport> {
        let analysis = self.analysis.as_ref();
        let applied = self.reduction.applies_anywhere();
        let offers = analysis
            .is_some_and(|found| !found.ambiguities.is_empty() || !found.possibles.is_empty());
        if !applied && !offers {
            return None;
        }
        let documents = &self.reduction.documents;
        let data = &self.reduction.data;
        let brief = |candidates: &[super::query::EntityBrief]| -> Vec<ReportCandidate> {
            candidates
                .iter()
                .map(|candidate| ReportCandidate {
                    entity_id: candidate.entity_id,
                    display: candidate.display.clone(),
                    type_id: candidate.type_id.clone(),
                    documents: candidate.documents,
                    workbooks: candidate.workbooks,
                })
                .collect()
        };
        let chosen_workbook = match self.applied_workbooks() {
            Some([only]) => Some(ChosenWorkbook {
                file: only.clone(),
                reason: "entity_files",
            }),
            _ => None,
        };
        Some(TargetingReport {
            strategy: self.reduction.strategy(),
            reason: self.reduction.skipped_reason().map(SkipReason::as_code),
            requested_by: self.requested_by(),
            widened: if widened { "auto" } else { "none" },
            selected_documents: documents.selected,
            candidate_documents: if documents.is_applied() {
                documents.candidates.len()
            } else {
                documents.selected
            },
            selected_workbooks: data.selected,
            candidate_workbooks: if data.is_applied() {
                data.candidates.len()
            } else {
                data.selected
            },
            reduction_percent: if applied {
                documents
                    .is_applied()
                    .then(|| documents.reduction_percent())
                    .into_iter()
                    .chain(data.is_applied().then(|| data.reduction_percent()))
                    .max()
                    .unwrap_or(0)
            } else {
                0
            },
            entities: analysis
                .map(|found| {
                    found
                        .entities
                        .iter()
                        .map(|entity| ReportEntity {
                            entity_id: entity.entity_id,
                            display: entity.display.clone(),
                            type_id: entity.type_id.clone(),
                            confidence: entity.confidence,
                            method: entity.method,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            candidate_paths: if documents.is_applied() {
                documents.candidates.clone()
            } else {
                Vec::new()
            },
            workbook_paths: if data.is_applied() {
                data.candidates.clone()
            } else {
                Vec::new()
            },
            ambiguities: analysis
                .map(|found| {
                    found
                        .ambiguities
                        .iter()
                        .map(|ambiguity| ReportAmbiguity {
                            span: ambiguity.span.clone(),
                            candidates: brief(&ambiguity.candidates),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            possibles: analysis
                .map(|found| {
                    found
                        .possibles
                        .iter()
                        .map(|possible| ReportPossible {
                            span: possible.span.clone(),
                            candidates: brief(&possible.candidates),
                            reason: possible.reason,
                            score: possible.score,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            chosen_workbook,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// What the interface is told
// ---------------------------------------------------------------------------------------------

/// What a question's targeting came to, for the interface. Machine codes and ids; the entity
/// names are those of entities **inside the selection**, shown only on screen (invariant I7) and
/// never logged.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetingReport {
    pub strategy: &'static str,
    /// Why nothing was reduced, when nothing was.
    pub reason: Option<&'static str>,
    /// `auto` or `chip`.
    pub requested_by: &'static str,
    /// `none` or `auto`.
    pub widened: &'static str,
    pub selected_documents: usize,
    pub candidate_documents: usize,
    pub selected_workbooks: usize,
    pub candidate_workbooks: usize,
    pub reduction_percent: u32,
    pub entities: Vec<ReportEntity>,
    pub candidate_paths: Vec<String>,
    pub workbook_paths: Vec<String>,
    pub ambiguities: Vec<ReportAmbiguity>,
    pub possibles: Vec<ReportPossible>,
    pub chosen_workbook: Option<ChosenWorkbook>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportEntity {
    pub entity_id: i64,
    pub display: String,
    pub type_id: String,
    pub confidence: f32,
    pub method: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportCandidate {
    pub entity_id: i64,
    pub display: String,
    pub type_id: String,
    /// How many files of the selection name it, by side. The interface offers no candidate that
    /// names none.
    pub documents: usize,
    pub workbooks: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportAmbiguity {
    /// The words of the question that fit these names, as the user typed them.
    pub span: String,
    pub candidates: Vec<ReportCandidate>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportPossible {
    /// The word of the question that sounds like these names, as typed.
    pub span: String,
    pub candidates: Vec<ReportCandidate>,
    pub reason: &'static str,
    pub score: f32,
}

/// The workbook the answer was read from when the reduction left one, and why.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChosenWorkbook {
    pub file: String,
    /// `entity_files`: the only workbook that holds what the question names.
    pub reason: &'static str,
}

// ---------------------------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------------------------

/// What the document search was asked to cover, as `commands::sourced_answer` decided it.
#[derive(Debug, Clone, Copy)]
pub enum DocumentPlan<'a> {
    /// One named file: never reduced.
    OneFile(&'a str),
    /// One excerpt from each of these files, in this order.
    EveryDocument(&'a [String]),
    /// Whatever ranks best: in the selection (`narrowed`) or in the whole current folder.
    WholeFolder {
        narrowed: bool,
        scoped_paths: &'a [String],
        present_paths: &'a [String],
    },
}

/// The result of the document search, with what the knowledge base did to it.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentSearch {
    pub outcome: SearchOutcome,
    /// The reduced search found nothing or little and the search ran again on the full selection.
    pub widened: bool,
    /// The evidence comes from the reduced set.
    pub reduced: bool,
}

/// Search the documents the way the plan says. With no applied reduction this is the search that
/// runs with the knowledge base off, byte for byte; with one, the reduced search runs first and is
/// repeated **once** on the full selection when it finds nothing (a ranked search also when its
/// closest passage, by meaning alone, scores below `knowledge_widen_below_score`: the lexical bonus
/// is left out because a reduced set was chosen for the name it holds, so it always has the bonus
/// and the bonus says nothing about the rest of the question). The refusal for insufficient evidence
/// belongs to the caller and only applies after the full selection has failed too.
pub fn search_documents(
    index: &IndexStore,
    plan: &TargetingPlan,
    kind: DocumentPlan<'_>,
    search_text: &str,
    query_embedding: &[f32],
) -> Result<DocumentSearch, AppError> {
    let terms = plan.extra_lexical_terms();
    let reduced = match kind {
        DocumentPlan::OneFile(_) => None,
        _ => plan.applied_documents(),
    };
    let run = |candidates: Option<&[String]>| -> Result<SearchOutcome, AppError> {
        match kind {
            DocumentPlan::OneFile(relative_path) => retrieval::search_scoped_counted_with_terms(
                index,
                search_text,
                &terms,
                query_embedding,
                RetrievalScope::File(relative_path),
            ),
            DocumentPlan::EveryDocument(paths) => {
                retrieval::search_per_document_counted_with_terms(
                    index,
                    search_text,
                    &terms,
                    query_embedding,
                    candidates.unwrap_or(paths),
                )
            }
            DocumentPlan::WholeFolder {
                narrowed,
                scoped_paths,
                present_paths,
            } => retrieval::search_scoped_counted_with_terms(
                index,
                search_text,
                &terms,
                query_embedding,
                match (candidates, narrowed) {
                    (Some(candidates), _) => RetrievalScope::Files(candidates),
                    (None, true) => RetrievalScope::Files(scoped_paths),
                    (None, false) => RetrievalScope::CurrentFolder(present_paths),
                },
            ),
        }
    };

    let Some(candidates) = reduced else {
        return Ok(DocumentSearch {
            outcome: run(None)?,
            widened: false,
            reduced: false,
        });
    };
    let first = run(Some(candidates))?;
    let too_weak = first.evidence.is_empty()
        || (matches!(kind, DocumentPlan::WholeFolder { .. })
            && first.best_vector_score < plan.policy.widen_below_score);
    if !too_weak {
        return Ok(DocumentSearch {
            outcome: first,
            widened: false,
            reduced: true,
        });
    }
    Ok(DocumentSearch {
        outcome: run(None)?,
        widened: true,
        reduced: false,
    })
}

/// Record what the knowledge base did to a documents search in the question's diagnostics: the
/// lookup time, what was found, what was reduced, and how a reduction that was only measured
/// compares with what the answer cited. Counts and codes only.
pub fn note_documents(
    index: &IndexStore,
    plan: &TargetingPlan,
    searched: &DocumentSearch,
    measured: &mut RetrievalDiagnostics,
) {
    let cited = distinct_paths(&searched.outcome.evidence);
    let shadow = match &plan.analysis {
        Some(analysis) => compare_with_cited(
            index,
            analysis,
            &plan.reduction.documents,
            Domain::Documents,
            &cited,
        ),
        None => Default::default(),
    };
    set_targeting(plan, searched.widened, shadow.into(), cited.len(), measured);
}

/// The same for the workbook side: the workbook that answered is the one cited.
pub fn note_workbooks(
    index: &IndexStore,
    plan: &TargetingPlan,
    answered: Option<&str>,
    measured: &mut RetrievalDiagnostics,
) {
    let cited: Vec<String> = answered.map(str::to_string).into_iter().collect();
    let shadow = match &plan.analysis {
        Some(analysis) => {
            compare_with_cited(index, analysis, &plan.reduction.data, Domain::Data, &cited)
        }
        None => Default::default(),
    };
    set_targeting(plan, false, shadow.into(), cited.len(), measured);
}

fn set_targeting(
    plan: &TargetingPlan,
    widened: bool,
    shadow: ShadowDiagnostics,
    cited: usize,
    measured: &mut RetrievalDiagnostics,
) {
    let documents = &plan.reduction.documents;
    let data = &plan.reduction.data;
    let outcome = if plan.reduction.applies_anywhere() {
        "applied"
    } else if plan.reduction.would_apply() {
        "shadow"
    } else {
        "skipped"
    };
    let reduction_percent = documents.reduction_percent().max(data.reduction_percent());
    let mut shadow = shadow;
    shadow.cited = shadow.cited.max(cited);
    measured.targeting = Some(TargetingDiagnostics {
        mode: match plan.mode {
            KnowledgeMode::Off => "off",
            KnowledgeMode::Suggest => "suggest",
            KnowledgeMode::Auto => "auto",
        },
        knowledge_lookup_ms: plan.knowledge_ms,
        strategy: plan.reduction.strategy(),
        skipped_reason: plan.reduction.skipped_reason().map(SkipReason::as_code),
        outcome,
        query_entities: plan
            .analysis
            .as_ref()
            .map(|analysis| {
                analysis
                    .entities
                    .iter()
                    .map(|entity| QueryEntityDiagnostic {
                        entity_id: entity.entity_id,
                        type_id: entity.type_id.clone(),
                        confidence: entity.confidence,
                        method: entity.method.to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        candidate_documents: if documents.has_candidates() {
            documents.candidates.len()
        } else {
            0
        },
        candidate_workbooks: if data.has_candidates() {
            data.candidates.len()
        } else {
            0
        },
        reduction_percent,
        widened,
        shadow,
    });
}

impl From<super::reduce::ShadowComparison> for ShadowDiagnostics {
    fn from(comparison: super::reduce::ShadowComparison) -> Self {
        Self {
            would_apply: comparison.would_apply,
            cited: comparison.cited,
            cited_in_candidates: comparison.cited_in_candidates,
            miss_no_mention: comparison.miss_no_mention,
            miss_capped: comparison.miss_capped,
            miss_entity_not_resolved: comparison.miss_entity_not_resolved,
        }
    }
}

fn distinct_paths(evidence: &[Evidence]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    evidence
        .iter()
        .filter(|item| seen.insert(item.relative_path.clone()))
        .map(|item| item.relative_path.clone())
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Workbooks
// ---------------------------------------------------------------------------------------------

/// The workbook selection narrowed to the candidates, built **from the existing entries** (or, for
/// "all workbooks", from the folder's own records), so a workbook the user did not select cannot
/// enter (invariant I1). An entry keeps the sheets it was narrowed to. The tabular engine and its
/// sheet restrictions are never touched (I5): it is simply given a shorter list.
pub fn narrowed_data_selection(
    data: &DataFolder,
    original: &ScopeMode,
    candidates: &[String],
) -> ScopeMode {
    let narrowed: Vec<ScopeEntry> = match original {
        ScopeMode::Explicit(entries) => entries
            .iter()
            .filter(|entry| candidates.contains(&entry.relative_path))
            .cloned()
            .collect(),
        ScopeMode::WholeFolder => candidates
            .iter()
            .filter_map(|path| data.files().find_by_relative_path(path))
            .map(|record| ScopeEntry {
                relative_path: record.relative_path.clone(),
                pinned_id: record.id.clone(),
                added_at: 0,
                sheet_names: Vec::new(),
            })
            .collect(),
    };
    // Defensive: a narrowing that kept nothing is no narrowing.
    if narrowed.is_empty() {
        return original.clone();
    }
    ScopeMode::Explicit(narrowed)
}

/// Order and annotate the buttons of "which workbook?" with what the knowledge base knows: the
/// workbooks that hold the entities of the question come first and say which entity and which
/// sheets. Any other answer is returned as it is. Never makes the choice.
pub fn decorate_which_workbook(answer: TabularAnswer, plan: &TargetingPlan) -> TabularAnswer {
    let TabularAnswer::WhichWorkbook { candidates, .. } = &answer else {
        return answer;
    };
    let data = &plan.reduction.data;
    let (Some(analysis), true) = (&plan.analysis, data.has_candidates()) else {
        return answer;
    };
    let rank = |file: &String| {
        data.candidates
            .iter()
            .position(|candidate| candidate == file)
            .unwrap_or(usize::MAX)
    };
    let mut ordered = candidates.clone();
    ordered.sort_by_key(|file| rank(file));
    let hints: Vec<WorkbookHint> = ordered
        .iter()
        .filter(|file| data.candidates.contains(file))
        .map(|file| WorkbookHint {
            file: file.clone(),
            entities: analysis
                .entities
                .iter()
                .filter(|entity| data.entity_ids.contains(&entity.entity_id))
                .filter(|entity| entity.data_files.contains(file))
                .map(|entity| WorkbookHintEntity {
                    entity_id: entity.entity_id,
                    display: entity.display.clone(),
                })
                .collect(),
            sheets: data.sheet_hints.get(file).cloned().unwrap_or_default(),
        })
        .collect();
    TabularAnswer::WhichWorkbook {
        candidates: ordered,
        hints,
    }
}

/// The workbook a tabular answer was read from, when it says (`file` is in every answer that has
/// one).
pub fn answered_file(answer: &TabularAnswer) -> Option<String> {
    serde_json::to_value(answer)
        .ok()?
        .get("file")?
        .as_str()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_without_fields_is_the_automatic_one() {
        let request: TargetingRequest = serde_json::from_str("{}").expect("parses");

        assert_eq!(request.mode, RequestMode::Auto);
        assert!(request.entity_ids.is_empty());
        assert!(request.carry_over.is_empty());
    }

    #[test]
    fn the_widen_action_reads_as_off() {
        let request: TargetingRequest =
            serde_json::from_str(r#"{"mode":"off","entityIds":[3],"carryOver":[4]}"#)
                .expect("parses");

        assert_eq!(request.mode, RequestMode::Off);
        assert_eq!(request.entity_ids, vec![3]);
        assert_eq!(request.carry_over, vec![4]);
    }

    #[test]
    fn a_workbook_answer_names_its_file() {
        let answer = TabularAnswer::WorkbookUnreadable {
            file: "a.csv".to_string(),
        };

        assert_eq!(answered_file(&answer).as_deref(), Some("a.csv"));
        assert_eq!(answered_file(&TabularAnswer::NoUsableTable), None);
    }
}
