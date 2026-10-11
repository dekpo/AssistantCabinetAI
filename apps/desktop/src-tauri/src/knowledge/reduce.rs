//! From the entities a question names to a smaller set of files to look at.
//!
//! ```text
//! AnalysisScope is the user's authority. The reducer proposes a smaller candidate set inside it.
//! When in doubt, the old path on the full selection runs.
//! ```
//!
//! A pure function of the analysis, the selection, the plan kind, the settings and the mode, plus
//! reads of the store. Documents and workbooks are decided **independently**: an entity that only a
//! workbook knows leaves the documents alone.
//!
//! What can happen to a side, in the order the rules are tried:
//!
//! | reason | meaning |
//! | --- | --- |
//! | `off` | the knowledge mode is off |
//! | `user_widened` | the user pressed "widen": the old path, exactly |
//! | `named_file` | the question names a file (or the plan is one file): never reduced (I2) |
//! | `small_scope` | fewer selected files than `knowledge_min_scope_files` (chips apply at any size) |
//! | `no_entity` | the question names nothing the base knows |
//! | `no_discriminating_entity` | it names something, but nothing sure and selective enough |
//! | `ambiguous` | a name inside the selection fits several entities: asked, never picked |
//! | `no_candidates` | the entities and the lexical net found no file of this side |
//! | `insignificant` | the reduction would drop less than `knowledge_min_reduction` |
//! | `lookup_error` | the base could not be read: the old path runs (I4) |
//!
//! Every candidate is a member of the selection, by construction (the list is built by filtering the
//! selection) and by a final check (I1). A file the base has never read is **kept**: what the base
//! does not know cannot be a reason to leave a file out.

use std::collections::{BTreeMap, BTreeSet};

use super::query::{QueryAnalysis, QueryEntity, SelectedFile};
use super::store;
use super::{Domain, KnowledgeMode, NOT_EXTRACTED};
use crate::error::AppError;
use crate::index_store::IndexStore;

/// A resolved entity drives a reduction only at or above this confidence (master section 7).
pub const MIN_REDUCTION_CONFIDENCE: f32 = 0.80;

/// The targeting settings, clamped (`Settings::targeting_policy`). Plain data, so the knowledge
/// layer takes it without knowing what a settings file is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetingPolicy {
    pub min_scope_files: usize,
    pub max_selectivity: f32,
    pub min_reduction: f32,
    pub max_candidates: usize,
    pub widen_below_score: f32,
}

impl Default for TargetingPolicy {
    /// The settings' own defaults: one place decides them (`settings::DEFAULT_KNOWLEDGE_*`).
    fn default() -> Self {
        use crate::settings::{
            DEFAULT_KNOWLEDGE_MAX_CANDIDATES, DEFAULT_KNOWLEDGE_MAX_SELECTIVITY,
            DEFAULT_KNOWLEDGE_MIN_REDUCTION, DEFAULT_KNOWLEDGE_MIN_SCOPE_FILES,
            DEFAULT_KNOWLEDGE_WIDEN_BELOW_SCORE,
        };
        Self {
            min_scope_files: DEFAULT_KNOWLEDGE_MIN_SCOPE_FILES,
            max_selectivity: DEFAULT_KNOWLEDGE_MAX_SELECTIVITY,
            min_reduction: DEFAULT_KNOWLEDGE_MIN_REDUCTION,
            max_candidates: DEFAULT_KNOWLEDGE_MAX_CANDIDATES,
            widen_below_score: DEFAULT_KNOWLEDGE_WIDEN_BELOW_SCORE,
        }
    }
}

/// Why a side was not reduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    Off,
    UserWidened,
    NamedFile,
    SmallScope,
    NoEntity,
    NoDiscriminatingEntity,
    Ambiguous,
    NoCandidates,
    Insignificant,
    LookupError,
}

impl SkipReason {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::UserWidened => "user_widened",
            Self::NamedFile => "named_file",
            Self::SmallScope => "small_scope",
            Self::NoEntity => "no_entity",
            Self::NoDiscriminatingEntity => "no_discriminating_entity",
            Self::Ambiguous => "ambiguous",
            Self::NoCandidates => "no_candidates",
            Self::Insignificant => "insignificant",
            Self::LookupError => "lookup_error",
        }
    }
}

/// What became of a side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The reduced set is what the search uses.
    Applied,
    /// Computed and measured, not used: the search runs on the full selection.
    Shadow,
    Skipped,
}

impl Outcome {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Shadow => "shadow",
            Self::Skipped => "skipped",
        }
    }
}

/// One side of the selection, as the reducer is given it.
#[derive(Debug, Clone, Copy)]
pub struct SideInput<'a> {
    /// The files the plan would cover on this side.
    pub files: &'a [SelectedFile],
    /// Set when the side must not be reduced whatever the base says: a named file, a plan about one
    /// file (invariant I2).
    pub blocked: Option<SkipReason>,
}

impl<'a> SideInput<'a> {
    pub fn new(files: &'a [SelectedFile]) -> Self {
        Self {
            files,
            blocked: None,
        }
    }

    pub fn blocked_by(mut self, reason: SkipReason) -> Self {
        self.blocked = Some(reason);
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ReduceInput<'a> {
    pub mode: KnowledgeMode,
    /// The user pressed "widen": the old path on the full selection.
    pub user_widened: bool,
    pub documents: SideInput<'a>,
    pub data: SideInput<'a>,
    pub policy: TargetingPolicy,
}

/// The decision for one side of the selection.
#[derive(Debug, Clone, PartialEq)]
pub struct SideReduction {
    /// Files the plan would cover on this side.
    pub selected: usize,
    pub outcome: Outcome,
    pub skipped: Option<SkipReason>,
    /// The files to search, best first. Empty when the side is skipped.
    pub candidates: Vec<String>,
    /// Files added by the lexical safety net (a form of a resolved name appears in them).
    pub added_by_lexical: Vec<String>,
    /// Files added because an open possible match of a resolved entity names them.
    pub from_possible_matches: Vec<String>,
    /// Files the base has never read: kept, because nothing it holds can rule them out.
    pub kept_unread: Vec<String>,
    /// Files the cap on candidates left out.
    pub capped: Vec<String>,
    /// The entities that drove the reduction.
    pub entity_ids: Vec<i64>,
    /// For each of them, how many files of this side mention it.
    pub per_entity_counts: Vec<(i64, usize)>,
    /// Workbooks only: the sheets in which the entities appear. A hint for ordering and for the
    /// note under an answer; never a sheet restriction (invariant I5).
    pub sheet_hints: BTreeMap<String, Vec<String>>,
    /// A chip the user picked drove it (so it applies in every mode and at any size).
    pub chip_driven: bool,
}

impl SideReduction {
    fn skipped(selected: usize, reason: SkipReason) -> Self {
        Self {
            selected,
            outcome: Outcome::Skipped,
            skipped: Some(reason),
            candidates: Vec::new(),
            added_by_lexical: Vec::new(),
            from_possible_matches: Vec::new(),
            kept_unread: Vec::new(),
            capped: Vec::new(),
            entity_ids: Vec::new(),
            per_entity_counts: Vec::new(),
            sheet_hints: BTreeMap::new(),
            chip_driven: false,
        }
    }

    /// Whether a reduced set exists (applied or only measured).
    pub fn has_candidates(&self) -> bool {
        self.outcome != Outcome::Skipped
    }

    pub fn is_applied(&self) -> bool {
        self.outcome == Outcome::Applied
    }

    /// The share of the side's files the reduced set leaves out, in whole percent.
    pub fn reduction_percent(&self) -> u32 {
        if self.selected == 0 || !self.has_candidates() {
            return 0;
        }
        let dropped = self.selected.saturating_sub(self.candidates.len());
        ((dropped * 100) / self.selected) as u32
    }
}

/// The decision for the whole question.
#[derive(Debug, Clone, PartialEq)]
pub struct ReductionPlan {
    pub documents: SideReduction,
    pub data: SideReduction,
}

impl ReductionPlan {
    /// A plan that reduces nothing, for a reason.
    pub fn skipped_everywhere(documents: usize, data: usize, reason: SkipReason) -> Self {
        Self {
            documents: SideReduction::skipped(documents, reason),
            data: SideReduction::skipped(data, reason),
        }
    }

    /// `entity_files` when any side has a reduced set, `none` otherwise.
    pub fn strategy(&self) -> &'static str {
        match (self.documents.has_candidates(), self.data.has_candidates()) {
            (false, false) => "none",
            _ if self.documents.chip_driven || self.data.chip_driven => "chip_files",
            _ => "entity_files",
        }
    }

    /// The reason to report for the question: the first side that was asked about and skipped.
    pub fn skipped_reason(&self) -> Option<SkipReason> {
        if self.documents.has_candidates() || self.data.has_candidates() {
            return None;
        }
        match (self.documents.selected, self.data.selected) {
            (0, _) => self.data.skipped,
            _ => self.documents.skipped,
        }
    }

    pub fn applies_anywhere(&self) -> bool {
        self.documents.is_applied() || self.data.is_applied()
    }

    pub fn would_apply(&self) -> bool {
        self.documents.has_candidates() || self.data.has_candidates()
    }

    /// Every file that would be searched across both sides, for the counts.
    pub fn candidate_total(&self) -> usize {
        self.documents.candidates.len() + self.data.candidates.len()
    }
}

/// Decide, for both sides, what to search. Never panics and never fails: a base that cannot be read
/// is `lookup_error` on both sides, and the caller runs the old path (invariant I4).
pub fn reduce(
    index: &IndexStore,
    analysis: &QueryAnalysis,
    input: &ReduceInput<'_>,
) -> ReductionPlan {
    let documents_selected = input.documents.files.len();
    let data_selected = input.data.files.len();
    if !input.mode.reads_files() {
        return ReductionPlan::skipped_everywhere(
            documents_selected,
            data_selected,
            SkipReason::Off,
        );
    }
    if input.user_widened {
        return ReductionPlan::skipped_everywhere(
            documents_selected,
            data_selected,
            SkipReason::UserWidened,
        );
    }
    let chips = analysis.entities.iter().any(|entity| entity.from_chip);
    // A name that fits several entities inside the selection is a question for the user. A chip is
    // the answer to such a question, so with a chip the user has chosen.
    let ambiguous = !analysis.ambiguities.is_empty() && !chips;

    let side = |files: SideInput<'_>, domain: Domain| match reduce_side(
        index, analysis, input, files, domain, ambiguous,
    ) {
        Ok(reduction) => reduction,
        Err(_) => SideReduction::skipped(files.files.len(), SkipReason::LookupError),
    };
    ReductionPlan {
        documents: side(input.documents, Domain::Documents),
        data: side(input.data, Domain::Data),
    }
}

/// An entity drives the reduction of one side when it is sure enough and selective enough there.
/// A chip is the user's own choice: sure by definition, and selective whatever its share.
fn passes(entity: &QueryEntity, selectivity: f32, policy: &TargetingPolicy) -> bool {
    if entity.from_chip {
        return true;
    }
    entity.confidence >= MIN_REDUCTION_CONFIDENCE
        && entity.status != super::EntityStatus::Candidate
        && selectivity <= policy.max_selectivity
}

/// The files of the entity on one side of the selection.
fn files_of(entity: &QueryEntity, domain: Domain) -> &[String] {
    match domain {
        Domain::Documents => &entity.document_files,
        Domain::Data => &entity.data_files,
    }
}

fn reduce_side(
    index: &IndexStore,
    analysis: &QueryAnalysis,
    input: &ReduceInput<'_>,
    side: SideInput<'_>,
    domain: Domain,
    ambiguous: bool,
) -> Result<SideReduction, AppError> {
    let selected = side.files.len();
    if let Some(reason) = side.blocked {
        return Ok(SideReduction::skipped(selected, reason));
    }
    if selected == 0 {
        return Ok(SideReduction::skipped(selected, SkipReason::NoEntity));
    }
    let policy = &input.policy;
    let selectivity_of = |entity: &QueryEntity| match domain {
        Domain::Documents => entity.document_selectivity,
        Domain::Data => entity.data_selectivity,
    };

    let driving: Vec<&QueryEntity> = analysis
        .entities
        .iter()
        .filter(|entity| passes(entity, selectivity_of(entity), policy))
        .collect();
    let chip_driven = driving.iter().any(|entity| entity.from_chip);

    // A small selection is left alone, unless the user asked for the reduction.
    if selected < policy.min_scope_files && !chip_driven {
        return Ok(SideReduction::skipped(selected, SkipReason::SmallScope));
    }
    if analysis.entities.is_empty() {
        return Ok(SideReduction::skipped(
            selected,
            if analysis.ambiguities.is_empty() {
                SkipReason::NoEntity
            } else {
                SkipReason::Ambiguous
            },
        ));
    }
    if ambiguous {
        return Ok(SideReduction::skipped(selected, SkipReason::Ambiguous));
    }
    if driving.is_empty() {
        return Ok(SideReduction::skipped(
            selected,
            SkipReason::NoDiscriminatingEntity,
        ));
    }

    // The selection of this side, by path: every candidate is taken from here (invariant I1).
    let selection: BTreeSet<&str> = side
        .files
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect();
    let known = match domain {
        Domain::Documents => &analysis.known_documents,
        Domain::Data => &analysis.known_data,
    };
    let source_ids = match domain {
        Domain::Documents => &analysis.document_source_ids,
        Domain::Data => &analysis.data_source_ids,
    };

    // Files of the entities, scored by how many of them each holds.
    let mut scored: BTreeMap<&str, (usize, f32)> = BTreeMap::new();
    let mut per_entity_counts = Vec::new();
    for entity in &driving {
        let files = files_of(entity, domain);
        per_entity_counts.push((entity.entity_id, files.len()));
        for path in files {
            if !selection.contains(path.as_str()) {
                continue;
            }
            let slot = scored.entry(path.as_str()).or_insert((0, 0.0));
            slot.0 += 1;
            slot.1 = slot.1.max(entity.confidence);
        }
    }
    let mut ranked: Vec<&str> = scored.keys().copied().collect();
    ranked.sort_by(|a, b| {
        let (count_a, confidence_a) = scored[a];
        let (count_b, confidence_b) = scored[b];
        count_b
            .cmp(&count_a)
            .then(
                confidence_b
                    .partial_cmp(&confidence_a)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.cmp(b))
    });
    let mut candidates: Vec<String> = ranked.iter().map(|path| path.to_string()).collect();

    // The other spelling of a person the base has recorded as a possible match (`Dupont J` and
    // `Jean Dupont`): same person until the user says otherwise, so its files stay in the search.
    let driving_ids: Vec<i64> = driving.iter().map(|entity| entity.entity_id).collect();
    let partners = store::open_possible_partners(index.connection(), &driving_ids)?;
    let partner_ids: Vec<i64> = partners
        .values()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut from_possible_matches = Vec::new();
    if !partner_ids.is_empty() {
        let partner_files =
            store::files_for_entities(index.connection(), &partner_ids, source_ids)?;
        for source in &partner_files.union {
            if source.domain == domain
                && selection.contains(source.relative_path.as_str())
                && !candidates.contains(&source.relative_path)
            {
                from_possible_matches.push(source.relative_path.clone());
            }
        }
        from_possible_matches.sort();
        candidates.extend(from_possible_matches.iter().cloned());
    }

    // The lexical safety net: documents only (workbooks have no full-text index). A document that
    // only says "M. Dupont", or whose scan damaged the full name, still holds a form of the name.
    let mut added_by_lexical = Vec::new();
    if domain == Domain::Documents {
        let terms: Vec<String> = driving
            .iter()
            .flat_map(|entity| entity.terms.iter().cloned())
            .collect();
        let paths: Vec<String> = side
            .files
            .iter()
            .map(|file| file.relative_path.clone())
            .collect();
        for path in index.files_matching_terms_in(&terms, &paths)? {
            if selection.contains(path.as_str()) && !candidates.contains(&path) {
                added_by_lexical.push(path);
            }
        }
        candidates.extend(added_by_lexical.iter().cloned());
    }

    // Nothing the base or the full-text index points at: the old path, never "nothing found".
    if candidates.is_empty() {
        return Ok(SideReduction::skipped(selected, SkipReason::NoCandidates));
    }

    // The cap on documents; workbooks are not capped.
    let mut capped = Vec::new();
    if domain == Domain::Documents && candidates.len() > policy.max_candidates {
        capped = candidates.split_off(policy.max_candidates);
    }

    // What the base has never read cannot be ruled out by what the base knows.
    let mut kept_unread = Vec::new();
    for file in side.files {
        let version = known.get(&file.relative_path).copied();
        let unread = matches!(version, None | Some(NOT_EXTRACTED));
        if unread && !candidates.contains(&file.relative_path) {
            kept_unread.push(file.relative_path.clone());
        }
    }
    kept_unread.sort();
    candidates.extend(kept_unread.iter().cloned());

    // Invariant I1, enforced a second time: whatever the steps above did, a path that is not in the
    // selection cannot leave this function.
    candidates.retain(|path| selection.contains(path.as_str()));
    debug_assert!(candidates
        .iter()
        .all(|path| selection.contains(path.as_str())));
    let mut seen = BTreeSet::new();
    candidates.retain(|path| seen.insert(path.clone()));

    let dropped = selected.saturating_sub(candidates.len());
    let share = dropped as f32 / selected as f32;
    let worthwhile = if chip_driven {
        dropped > 0
    } else {
        share >= policy.min_reduction
    };
    if !worthwhile {
        return Ok(SideReduction::skipped(selected, SkipReason::Insignificant));
    }

    let sheet_hints = match domain {
        Domain::Data => {
            let hints = store::sheets_for_entities(index.connection(), &driving_ids, source_ids)?;
            hints
                .into_iter()
                .filter(|(path, _)| candidates.contains(path))
                .map(|(path, sheets)| (path, sheets.into_iter().collect()))
                .collect()
        }
        Domain::Documents => BTreeMap::new(),
    };

    let applied = input.mode == KnowledgeMode::Auto || chip_driven;
    Ok(SideReduction {
        selected,
        outcome: if applied {
            Outcome::Applied
        } else {
            Outcome::Shadow
        },
        skipped: None,
        candidates,
        added_by_lexical,
        from_possible_matches,
        kept_unread,
        capped,
        entity_ids: driving_ids,
        per_entity_counts,
        sheet_hints,
        chip_driven,
    })
}

/// How a reduction that was only measured compares with what the answer really cited.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShadowComparison {
    pub would_apply: bool,
    /// Distinct files the answer cited (documents), or the workbook that answered (data).
    pub cited: usize,
    pub cited_in_candidates: usize,
    /// A cited file the reduced set left out, and the base holds no mention of any entity of the
    /// question in it: an extraction gap.
    pub miss_no_mention: usize,
    /// Left out by the cap on candidates.
    pub miss_capped: usize,
    /// The file does mention an entity of the question that was not used (ambiguous, possible, or
    /// not sure enough).
    pub miss_entity_not_resolved: usize,
}

/// Compare what a side would have searched with the files the answer cited. Counts only. A side
/// that was skipped has nothing to compare and reports `would_apply: false`.
pub fn compare_with_cited(
    index: &IndexStore,
    analysis: &QueryAnalysis,
    side: &SideReduction,
    domain: Domain,
    cited: &[String],
) -> ShadowComparison {
    let cited: BTreeSet<&str> = cited.iter().map(String::as_str).collect();
    if !side.has_candidates() {
        return ShadowComparison {
            would_apply: false,
            cited: cited.len(),
            ..ShadowComparison::default()
        };
    }
    let candidates: BTreeSet<&str> = side.candidates.iter().map(String::as_str).collect();
    let capped: BTreeSet<&str> = side.capped.iter().map(String::as_str).collect();
    let source_ids = match domain {
        Domain::Documents => &analysis.document_source_ids,
        Domain::Data => &analysis.data_source_ids,
    };
    // Where every entity the question mentioned is found, used or not.
    let mentioned: Vec<i64> = analysis.every_entity_id().into_iter().collect();
    let mentioned_in: BTreeSet<String> =
        match store::files_for_entities(index.connection(), &mentioned, source_ids) {
            Ok(files) => files
                .union
                .into_iter()
                .filter(|source| source.domain == domain)
                .map(|source| source.relative_path)
                .collect(),
            Err(_) => BTreeSet::new(),
        };
    let mut comparison = ShadowComparison {
        would_apply: true,
        cited: cited.len(),
        ..ShadowComparison::default()
    };
    for path in cited {
        if candidates.contains(path) {
            comparison.cited_in_candidates += 1;
        } else if capped.contains(path) {
            comparison.miss_capped += 1;
        } else if mentioned_in.contains(path) {
            comparison.miss_entity_not_resolved += 1;
        } else {
            comparison.miss_no_mention += 1;
        }
    }
    comparison
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_skip_reason_has_its_own_code() {
        let reasons = [
            SkipReason::Off,
            SkipReason::UserWidened,
            SkipReason::NamedFile,
            SkipReason::SmallScope,
            SkipReason::NoEntity,
            SkipReason::NoDiscriminatingEntity,
            SkipReason::Ambiguous,
            SkipReason::NoCandidates,
            SkipReason::Insignificant,
            SkipReason::LookupError,
        ];
        let codes: BTreeSet<&str> = reasons.iter().map(|reason| reason.as_code()).collect();
        assert_eq!(codes.len(), reasons.len());
    }

    #[test]
    fn a_skipped_side_reports_no_reduction() {
        let side = SideReduction::skipped(30, SkipReason::SmallScope);

        assert!(!side.has_candidates());
        assert_eq!(side.reduction_percent(), 0);
    }

    #[test]
    fn the_reduction_is_a_whole_percent_of_the_selection() {
        let mut side = SideReduction::skipped(30, SkipReason::Off);
        side.outcome = Outcome::Applied;
        side.skipped = None;
        side.candidates = (0..12).map(|n| format!("f{n}")).collect();

        assert_eq!(side.reduction_percent(), 60);
    }

    #[test]
    fn the_default_policy_is_the_one_of_the_master_plan() {
        let policy = TargetingPolicy::default();

        assert_eq!(policy.min_scope_files, 8);
        assert_eq!(policy.max_candidates, 15);
        assert!((policy.max_selectivity - 0.6).abs() < 1e-6);
        assert!((policy.min_reduction - 0.3).abs() < 1e-6);
    }
}
