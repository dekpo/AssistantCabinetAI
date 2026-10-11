//! What the question names: the entities of the knowledge base inside the user's selection.
//!
//! **No model, no gateway call.** The question is read with the same rules the files were read
//! with: runs of up to five words are looked up as a name, an alias, a title and a surname or an
//! identifier, the resolver of lot 3 decides in query mode, and what it cannot settle becomes a
//! question to the user (an ambiguity, a possible match) and never a guess.
//!
//! ```text
//! chips (the user's own choice)                     -> resolved, confidence 1.0
//! runs of 1..5 words, longest first                 -> resolver, restricted to the selection (I7)
//! capitalised words nothing explained, >= 4 letters -> a possible match at best, never resolved
//! an identifier of a pack scheme                    -> the entity that carries it
//! a short follow-up that names nothing              -> the entities of the previous answer
//! ```
//!
//! Ambiguity is judged **inside the selection** (master invariant I7): two Martins in the base and
//! one in the files she selected is a resolved Martin, and the other one is never mentioned.
//! Dates, months and the stop-words of the packs are not entities: the tabular engine owns them.
//!
//! Nothing here writes: the knowledge base is built from files, never from what a question says
//! (invariant I3).

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::Connection;

use super::normalize::{normalize_name, surname_then_initial, TitleSet};
use super::packs::PackSet;
use super::phonetic::PhoneticEncoder;
use super::resolve::{
    initial_surname_key_of_full_name, EntityHit, EntityResolver, IdentifierRef, Resolution,
    ResolveContext, ResolveMode, SourceSet, StoreLookup, SurfaceInput, UnresolvedReason,
};
use super::secret::IdentifierKey;
use super::store;
use super::{Domain, EntityStatus, SourceRef};
use crate::conversation::FOLLOW_UP_MAX_WORDS;
use crate::error::AppError;

/// The longest run of words read as one name.
pub const MAX_SPAN_WORDS: usize = 5;

/// The shortest capitalised word offered to the "did you mean" rule (lot 6 and lot 9 agree on 4;
/// `resolve::FUZZY_MIN_TOKEN_LEN` applies a second time inside the resolver).
pub const MIN_FUZZY_WORD_LEN: usize = 4;

/// How sure the previous answer's entities are taken to be in a short follow-up that names nobody.
/// Above the reduction threshold of 0.80: the user is still talking about the same person. A
/// candidate is capped at the resolver's candidate cap, as everywhere else.
pub const CARRIED_OVER_CONFIDENCE: f32 = 0.85;

/// What a chip is worth: the user chose it.
pub const CHIP_CONFIDENCE: f32 = 1.0;

/// At most this many capitalised words are offered to the fuzzy rule in one question.
const MAX_FUZZY_WORDS: usize = 12;

/// One file of the selection, as the knowledge base keys it: the relative path and the content
/// id (`FileRecord::id`, the SHA-256). The domain is the list it sits in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFile {
    pub relative_path: String,
    pub content_id: String,
}

impl SelectedFile {
    pub fn new(relative_path: &str, content_id: &str) -> Self {
        Self {
            relative_path: relative_path.to_string(),
            content_id: content_id.to_string(),
        }
    }
}

/// The files a question may look at, documents and workbooks apart (the same relative path can
/// exist in both folders, invariant I6).
#[derive(Debug, Clone, Copy)]
pub struct QuerySelection<'a> {
    pub documents: &'a [SelectedFile],
    pub data: &'a [SelectedFile],
}

/// Everything the detector is asked.
#[derive(Debug, Clone, Copy)]
pub struct DetectionInput<'a> {
    pub question: &'a str,
    pub selection: QuerySelection<'a>,
    /// Entities the user picked as chips: resolved by definition.
    pub explicit_entity_ids: &'a [i64],
    /// The previous answer's entities, held by the client; honoured only for a short follow-up that
    /// names nobody. Rust stores nothing about the conversation (invariant I3).
    pub carry_over: &'a [i64],
}

/// A name the question settled on.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryEntity {
    pub entity_id: i64,
    /// The entity's canonical name: user data of an entity inside the selection, shown only in the
    /// interface and never logged.
    pub display: String,
    pub type_id: String,
    pub status: EntityStatus,
    pub confidence: f32,
    /// `chip`, `carried_over`, or the resolver's signal (`canonical_name`, `alias`, `identifier`).
    pub method: &'static str,
    pub from_chip: bool,
    pub carried_over: bool,
    /// The files of the selection that mention it, by side.
    pub document_files: Vec<String>,
    pub data_files: Vec<String>,
    /// The share of the selection's files that mention it, by side (0 when the side is empty).
    pub document_selectivity: f32,
    pub data_selectivity: f32,
    /// Its surface forms, folded: what the lexical safety net searches for.
    pub terms: Vec<String>,
}

/// An entity named in an ambiguity or a possible match.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityBrief {
    pub entity_id: i64,
    pub display: String,
    pub type_id: String,
    /// How many files of the selection name it, by side. Filled once the whole question is read.
    /// A candidate that names none is not in the selection, and the interface drops it even if it
    /// were sent (invariant I7).
    pub documents: usize,
    pub workbooks: usize,
}

impl EntityBrief {
    fn of(hit: &EntityHit) -> Self {
        Self {
            entity_id: hit.entity_id,
            display: hit.canonical_name.clone(),
            type_id: hit.type_id.clone(),
            documents: 0,
            workbooks: 0,
        }
    }
}

/// Several entities of the selection answer equally to a name: asked, never picked.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryAmbiguity {
    /// The words of the question that fit them, exactly as the user typed them (her own text,
    /// shown only on screen and never logged).
    pub span: String,
    pub candidates: Vec<EntityBrief>,
}

/// Probably one of these, never to be applied on that evidence: offered as a "did you mean".
#[derive(Debug, Clone, PartialEq)]
pub struct QueryPossible {
    /// The word of the question that sounds like them, as typed.
    pub span: String,
    pub candidates: Vec<EntityBrief>,
    /// `PossibleReason::as_code`.
    pub reason: &'static str,
    pub score: f32,
}

/// What the knowledge base made of a question. Plain data in this process: it is never written to a
/// log or to the store, and only the parts the interface needs reach the webview
/// (`knowledge::targeting::TargetingReport`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QueryAnalysis {
    pub entities: Vec<QueryEntity>,
    pub ambiguities: Vec<QueryAmbiguity>,
    pub possibles: Vec<QueryPossible>,
    /// Capitalised words that nothing in the base explains.
    pub unresolved_spans: usize,
    /// For each selected document the base knows: the version of the extractor that read it (`0`
    /// when none did). A file missing from this map is one the base has never seen, or has seen
    /// under another content: nothing it holds can say the file is irrelevant.
    pub known_documents: BTreeMap<String, u32>,
    pub known_data: BTreeMap<String, u32>,
    /// The sources of the selection, for a later read of the store (sheet hints).
    pub document_source_ids: Vec<i64>,
    pub data_source_ids: Vec<i64>,
}

impl QueryAnalysis {
    pub fn found_anything(&self) -> bool {
        !self.entities.is_empty() || !self.ambiguities.is_empty() || !self.possibles.is_empty()
    }

    /// Every entity the question mentioned in any way: resolved, ambiguous or possible.
    pub fn every_entity_id(&self) -> BTreeSet<i64> {
        let mut ids: BTreeSet<i64> = self
            .entities
            .iter()
            .map(|entity| entity.entity_id)
            .collect();
        for ambiguity in &self.ambiguities {
            ids.extend(ambiguity.candidates.iter().map(|brief| brief.entity_id));
        }
        for possible in &self.possibles {
            ids.extend(possible.candidates.iter().map(|brief| brief.entity_id));
        }
        ids
    }
}

/// The detector. Built once per question from the pieces the application keeps for the whole
/// session (the packs are loaded once, never per question: tens of milliseconds).
pub struct QueryEntityDetector<'a> {
    pub connection: &'a Connection,
    pub packs: &'a PackSet,
    pub titles: &'a TitleSet,
    pub encoder: &'a dyn PhoneticEncoder,
    pub resolver: &'a dyn EntityResolver,
    /// The key that hashes personal identifiers. Without it a personal identifier written in a
    /// question cannot be recognised (the base only holds its hash) and is left alone.
    pub key: Option<&'a IdentifierKey>,
}

/// A word of the question with its place in it.
#[derive(Debug, Clone)]
pub(crate) struct Word {
    pub(crate) text: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl Word {
    pub(crate) fn is_capitalised(&self) -> bool {
        self.text.chars().next().is_some_and(char::is_uppercase)
    }

    fn is_all_digits(&self) -> bool {
        !self.text.is_empty() && self.text.chars().all(|c| c.is_ascii_digit())
    }
}

/// An entity found, before its files are counted.
struct Found {
    hit: EntityHit,
    confidence: f32,
    method: &'static str,
    from_chip: bool,
    carried_over: bool,
}

impl QueryEntityDetector<'_> {
    /// Read the question. An error means the base could not be read: the caller falls back to the
    /// full selection (invariant I4) and never says "nothing found".
    pub fn detect(&self, input: &DetectionInput<'_>) -> Result<QueryAnalysis, AppError> {
        let documents = pairs(input.selection.documents);
        let data = pairs(input.selection.data);
        let document_sources =
            store::sources_in_scope(self.connection, Domain::Documents, &documents)?;
        let data_sources = store::sources_in_scope(self.connection, Domain::Data, &data)?;
        let document_source_ids: Vec<i64> =
            document_sources.iter().map(|row| row.source_id).collect();
        let data_source_ids: Vec<i64> = data_sources.iter().map(|row| row.source_id).collect();
        let every_source = SourceSet::new(
            document_source_ids
                .iter()
                .chain(data_source_ids.iter())
                .copied(),
        );

        let mut analysis = QueryAnalysis {
            known_documents: document_sources
                .iter()
                .map(|row| (row.relative_path.clone(), row.kb_version))
                .collect(),
            known_data: data_sources
                .iter()
                .map(|row| (row.relative_path.clone(), row.kb_version))
                .collect(),
            document_source_ids: document_source_ids.clone(),
            data_source_ids: data_source_ids.clone(),
            ..QueryAnalysis::default()
        };
        // Nothing of the selection is known to the base: there is nothing to find, and nothing to
        // reveal about files outside it.
        if every_source.is_empty() && input.explicit_entity_ids.is_empty() {
            return Ok(analysis);
        }

        let lookup = StoreLookup::new(self.connection, self.titles);
        let context = ResolveContext {
            mode: ResolveMode::Query,
            titles: self.titles,
            encoder: self.encoder,
            lookup: &lookup,
            selection: Some(&every_source),
            source_entities: None,
        };

        let mut found: Vec<Found> = Vec::new();

        // 1. Chips: the user's own choice.
        for entity_id in input.explicit_entity_ids {
            if let Some(hit) = self.live_entity(*entity_id)? {
                found.push(Found {
                    hit,
                    confidence: CHIP_CONFIDENCE,
                    method: "chip",
                    from_chip: true,
                    carried_over: false,
                });
            }
        }

        let words = words_of(input.question);
        let mut consumed = vec![false; words.len()];

        // 6. Identifiers of the packs' schemes, read in the text as written.
        self.read_identifiers(
            input.question,
            &words,
            &mut consumed,
            &context,
            &mut found,
            &mut analysis,
        )?;

        // 2-4. Runs of words, longest first, each looked up in the base before the resolver is asked.
        self.read_spans(
            input.question,
            &words,
            &mut consumed,
            &context,
            &mut found,
            &mut analysis,
        )?;

        // 5. A capitalised word nothing explained: a possible match at best.
        self.read_misspellings(&words, &consumed, &context, &mut analysis)?;

        // 7. A short follow-up that names nobody carries the previous answer's entities.
        let short = input.question.split_whitespace().count() <= FOLLOW_UP_MAX_WORDS;
        if short && found.iter().all(|entry| entry.from_chip) && analysis.ambiguities.is_empty() {
            for entity_id in input.carry_over {
                if let Some(hit) = self.live_entity(*entity_id)? {
                    let confidence = match hit.status {
                        EntityStatus::Candidate => super::resolve::CANDIDATE_CONFIDENCE_CAP,
                        _ => CARRIED_OVER_CONFIDENCE,
                    };
                    found.push(Found {
                        hit,
                        confidence,
                        method: "carried_over",
                        from_chip: false,
                        carried_over: true,
                    });
                }
            }
        }

        // 8. Files and selectivity of each entity inside the selection.
        self.describe(&found, &input.selection, &every_source, &mut analysis)?;
        // 9. How many files of the selection hold each name that is asked about.
        self.count_offered(&every_source, &mut analysis)?;
        Ok(analysis)
    }

    /// Fill in, for every entity a "which one?" or a "did you mean?" will offer, how many files of
    /// the selection name it. One query for all of them.
    fn count_offered(
        &self,
        every_source: &SourceSet,
        analysis: &mut QueryAnalysis,
    ) -> Result<(), AppError> {
        let offered: Vec<i64> = analysis
            .ambiguities
            .iter()
            .flat_map(|ambiguity| ambiguity.candidates.iter())
            .chain(
                analysis
                    .possibles
                    .iter()
                    .flat_map(|possible| possible.candidates.iter()),
            )
            .map(|brief| brief.entity_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if offered.is_empty() {
            return Ok(());
        }
        let files = store::files_for_entities(self.connection, &offered, &every_source.ids())?;
        let count = |entity_id: i64| -> (usize, usize) {
            let mut documents = 0;
            let mut workbooks = 0;
            if let Some(sources) = files.per_entity.get(&entity_id) {
                for source in sources {
                    match source.domain {
                        Domain::Documents => documents += 1,
                        Domain::Data => workbooks += 1,
                    }
                }
            }
            (documents, workbooks)
        };
        let fill = |briefs: &mut Vec<EntityBrief>| {
            for brief in briefs.iter_mut() {
                (brief.documents, brief.workbooks) = count(brief.entity_id);
            }
        };
        for ambiguity in &mut analysis.ambiguities {
            fill(&mut ambiguity.candidates);
        }
        for possible in &mut analysis.possibles {
            fill(&mut possible.candidates);
        }
        Ok(())
    }

    /// A live (candidate or active) entity after following merges, or `None`.
    fn live_entity(&self, entity_id: i64) -> Result<Option<EntityHit>, AppError> {
        let Some(entity) = store::effective_entity(self.connection, entity_id)? else {
            return Ok(None);
        };
        Ok(matches!(
            entity.status,
            EntityStatus::Candidate | EntityStatus::Active
        )
        .then(|| EntityHit::from_entity(&entity)))
    }

    fn read_identifiers(
        &self,
        question: &str,
        words: &[Word],
        consumed: &mut [bool],
        context: &ResolveContext<'_>,
        found: &mut Vec<Found>,
        analysis: &mut QueryAnalysis,
    ) -> Result<(), AppError> {
        for scheme in self.packs.identifier_schemes() {
            if scheme.personal_key && self.key.is_none() {
                continue;
            }
            for matched in scheme.scan(question) {
                let stored = match (scheme.personal_key, self.key) {
                    (true, Some(key)) => key.digest(&scheme.id, &matched.normalized),
                    _ => matched.normalized.clone(),
                };
                let surface = SurfaceInput::new(&question[matched.range.clone()]).with_identifier(
                    IdentifierRef {
                        scheme: scheme.id.clone(),
                        normalized: stored,
                        strength: scheme.strength,
                    },
                );
                let shown = surface.text.clone();
                let settled = self.settle(&surface, &shown, context, found, analysis)?;
                if settled {
                    for (index, word) in words.iter().enumerate() {
                        if word.start < matched.range.end && matched.range.start < word.end {
                            consumed[index] = true;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn read_spans(
        &self,
        question: &str,
        words: &[Word],
        consumed: &mut [bool],
        context: &ResolveContext<'_>,
        found: &mut Vec<Found>,
        analysis: &mut QueryAnalysis,
    ) -> Result<(), AppError> {
        // Every run, with the forms it could be known under; one query tells which are known.
        struct Span {
            first: usize,
            length: usize,
            text: String,
            /// The same words exactly as typed, punctuation between them kept (`J. Dupont`): what a
            /// "which one?" row quotes back to the user.
            shown: String,
            forms: Vec<String>,
        }
        let mut spans: Vec<Span> = Vec::new();
        let mut every_form: BTreeSet<String> = BTreeSet::new();
        for length in (1..=MAX_SPAN_WORDS.min(words.len())).rev() {
            for first in 0..=(words.len() - length) {
                let run = &words[first..first + length];
                let text = run
                    .iter()
                    .map(|word| word.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                if length == 1 && !self.may_be_a_name(&run[0]) {
                    continue;
                }
                let forms = self.forms_of(&text);
                if forms.is_empty() {
                    continue;
                }
                every_form.extend(forms.iter().cloned());
                let shown = question[run[0].start..run[length - 1].end].to_string();
                spans.push(Span {
                    first,
                    length,
                    text,
                    shown,
                    forms,
                });
            }
        }
        let known =
            store::known_name_forms(self.connection, &every_form.into_iter().collect::<Vec<_>>())?;
        for span in spans {
            let range = span.first..span.first + span.length;
            if consumed[range.clone()].iter().any(|taken| *taken) {
                continue;
            }
            if !span.forms.iter().any(|form| known.contains(form)) {
                continue;
            }
            let surface = SurfaceInput::new(&span.text);
            if self.settle(&surface, &span.shown, context, found, analysis)? {
                for index in range {
                    consumed[index] = true;
                }
            }
        }
        Ok(())
    }

    /// The forms a run of words could be stored under, mirroring what the resolver looks up: the
    /// folded words, without an elided article, with the title in front, and the initial-and-surname
    /// key of a full name or of a surname-then-initial name.
    fn forms_of(&self, text: &str) -> Vec<String> {
        let name = normalize_name(text, self.titles);
        if name.joined.is_empty() {
            return Vec::new();
        }
        let mut forms = vec![name.joined.clone()];
        if !name.without_elision.is_empty() && name.without_elision != name.joined {
            forms.push(name.without_elision.clone());
        }
        if !name.titles.is_empty() {
            forms.push(format!("{} {}", name.titles.join(" "), name.joined));
        }
        match surname_then_initial(&name, self.titles) {
            Some(form) => forms.push(form.initial_surname_key()),
            None => {
                if let Some(key) = initial_surname_key_of_full_name(&name, self.titles) {
                    forms.push(key);
                }
            }
        }
        forms
    }

    /// Whether a single word can be a name: not a stop-word, not a number, not a title on its own,
    /// at least three letters.
    fn may_be_a_name(&self, word: &Word) -> bool {
        if word.is_all_digits() || word.text.chars().count() < 3 {
            return false;
        }
        !self.packs.is_stop_word(&word.text)
    }

    /// Ask the resolver about one surface and file the answer. `true` when it settled the surface
    /// (resolved, ambiguous or possible), so the words it covered are not read again.
    fn settle(
        &self,
        surface: &SurfaceInput,
        shown: &str,
        context: &ResolveContext<'_>,
        found: &mut Vec<Found>,
        analysis: &mut QueryAnalysis,
    ) -> Result<bool, AppError> {
        match self.resolver.resolve(surface, context) {
            Resolution::Existing {
                entity,
                confidence,
                signal,
                ..
            } => {
                found.push(Found {
                    hit: entity,
                    confidence: confidence.value(),
                    method: signal.as_code(),
                    from_chip: false,
                    carried_over: false,
                });
                Ok(true)
            }
            Resolution::Ambiguous { entities, .. } => {
                analysis.ambiguities.push(QueryAmbiguity {
                    span: shown.to_string(),
                    candidates: entities.iter().map(EntityBrief::of).collect(),
                });
                Ok(true)
            }
            Resolution::PossibleMatch {
                entities,
                reason,
                score,
                ..
            } => {
                analysis.possibles.push(QueryPossible {
                    span: shown.to_string(),
                    candidates: entities.iter().map(EntityBrief::of).collect(),
                    reason: reason.as_code(),
                    score: score.value(),
                });
                Ok(true)
            }
            Resolution::Unresolved {
                reason: UnresolvedReason::LookupFailed,
            } => Err(AppError::KnowledgeUnavailable),
            Resolution::Unresolved { .. } | Resolution::NewEntity { .. } => Ok(false),
        }
    }

    /// Capitalised words no run explained, offered to the "same sound, one edit" rule. Whatever it
    /// finds is a possible match: a spelling is never silently taken for a person.
    fn read_misspellings(
        &self,
        words: &[Word],
        consumed: &[bool],
        context: &ResolveContext<'_>,
        analysis: &mut QueryAnalysis,
    ) -> Result<(), AppError> {
        let mut offered = 0usize;
        for (index, word) in words.iter().enumerate() {
            if consumed[index]
                || !word.is_capitalised()
                || word.text.chars().count() < MIN_FUZZY_WORD_LEN
                || !word
                    .text
                    .chars()
                    .all(|c| c.is_alphabetic() || c == '-' || c == '\'')
                || self.packs.is_stop_word(&word.text)
            {
                continue;
            }
            let name = normalize_name(&word.text, self.titles);
            if name.joined.is_empty() {
                continue;
            }
            offered += 1;
            if offered > MAX_FUZZY_WORDS {
                break;
            }
            match self
                .resolver
                .resolve(&SurfaceInput::new(&word.text), context)
            {
                Resolution::PossibleMatch {
                    entities,
                    reason,
                    score,
                    ..
                } => analysis.possibles.push(QueryPossible {
                    span: word.text.clone(),
                    candidates: entities.iter().map(EntityBrief::of).collect(),
                    reason: reason.as_code(),
                    score: score.value(),
                }),
                Resolution::Unresolved {
                    reason: UnresolvedReason::LookupFailed,
                } => return Err(AppError::KnowledgeUnavailable),
                // Anything the resolver would have settled was settled by a run before this point;
                // here only a possible match is accepted, never an entity.
                _ => analysis.unresolved_spans += 1,
            }
        }
        Ok(())
    }

    /// The folded forms an entity is written under, for the lexical safety net. A surname alone or a
    /// title and a surname is kept only when no other entity of the base carries it: searching a
    /// shared `martin` would pull in the files of every other Martin and undo the reduction, and a
    /// document that says only `M. Martin` cannot be told apart from theirs anyway.
    ///
    /// "Other entity" means one that is mentioned **inside the selection**: a patient who is only in
    /// a workbook she did not tick does not make `dupont` ambiguous among the documents she did.
    fn surface_forms(
        &self,
        entity_id: i64,
        every_source: &SourceSet,
    ) -> Result<Vec<String>, AppError> {
        let Some(record) = store::entity_with_aliases(self.connection, entity_id)? else {
            return Ok(Vec::new());
        };
        let mut forms: BTreeSet<String> = BTreeSet::new();
        // An identifier entity is named by its stored value, which for a personal identifier is a
        // keyed hash: not a word of any document, and not something to search for.
        if record.entity.type_id != super::EntityTypeId::IDENTIFIER {
            forms.insert(record.entity.normalized_name.clone());
        }
        for alias in &record.aliases {
            if alias.kind.is_ambiguous_by_nature() {
                let owners: Vec<i64> =
                    store::find_entities_by_alias(self.connection, &alias.normalized)?
                        .into_iter()
                        .map(|(entity, _)| entity.entity_id)
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect();
                let in_selection =
                    store::files_for_entities(self.connection, &owners, &every_source.ids())?;
                let sharing = owners
                    .iter()
                    .filter(|owner| {
                        in_selection
                            .per_entity
                            .get(owner)
                            .is_some_and(|files| !files.is_empty())
                    })
                    .count();
                if sharing > 1 {
                    continue;
                }
            }
            forms.insert(alias.normalized.clone());
        }
        Ok(forms.into_iter().collect())
    }

    /// Count each entity's files inside the selection and its share of it.
    fn describe(
        &self,
        found: &[Found],
        selection: &QuerySelection<'_>,
        every_source: &SourceSet,
        analysis: &mut QueryAnalysis,
    ) -> Result<(), AppError> {
        // One entity once, at its best confidence; a chip stays a chip.
        let mut merged: BTreeMap<i64, &Found> = BTreeMap::new();
        let mut order: Vec<i64> = Vec::new();
        for entry in found {
            match merged.get(&entry.hit.entity_id) {
                Some(known) if known.confidence >= entry.confidence => {}
                Some(_) => {
                    merged.insert(entry.hit.entity_id, entry);
                }
                None => {
                    order.push(entry.hit.entity_id);
                    merged.insert(entry.hit.entity_id, entry);
                }
            }
        }
        let ids: Vec<i64> = order.clone();
        let files = store::files_for_entities(self.connection, &ids, &every_source.ids())?;
        for entity_id in order {
            let entry = merged[&entity_id];
            let mut document_files = Vec::new();
            let mut data_files = Vec::new();
            if let Some(sources) = files.per_entity.get(&entity_id) {
                for SourceRef {
                    domain,
                    relative_path,
                    ..
                } in sources
                {
                    match domain {
                        Domain::Documents => document_files.push(relative_path.clone()),
                        Domain::Data => data_files.push(relative_path.clone()),
                    }
                }
            }
            let share = |count: usize, selected: usize| {
                if selected == 0 {
                    0.0
                } else {
                    count as f32 / selected as f32
                }
            };
            let terms = self.surface_forms(entity_id, every_source)?;
            analysis.entities.push(QueryEntity {
                entity_id,
                display: entry.hit.canonical_name.clone(),
                type_id: entry.hit.type_id.clone(),
                status: entry.hit.status,
                confidence: entry.confidence,
                method: entry.method,
                from_chip: entry.from_chip,
                carried_over: entry.carried_over,
                document_selectivity: share(document_files.len(), selection.documents.len()),
                data_selectivity: share(data_files.len(), selection.data.len()),
                document_files,
                data_files,
                terms,
            });
        }
        Ok(())
    }
}

fn pairs(files: &[SelectedFile]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|file| (file.relative_path.clone(), file.content_id.clone()))
        .collect()
}

/// The words of a question, each with its byte range. Whitespace separates words; punctuation at
/// either end of a word is dropped (`Dupont,` and `(Dupont)` are `Dupont`), punctuation inside is
/// kept (`Jean-Pierre`, `d'Alembert`).
pub(crate) fn words_of(question: &str) -> Vec<Word> {
    let mut words = Vec::new();
    let mut start: Option<usize> = None;
    let flush = |from: usize, to: usize, words: &mut Vec<Word>| {
        let slice = &question[from..to];
        let trimmed_start = slice
            .char_indices()
            .find(|(_, c)| c.is_alphanumeric())
            .map(|(index, _)| index);
        let Some(trimmed_start) = trimmed_start else {
            return;
        };
        let trimmed_end = slice
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_alphanumeric())
            .map(|(index, c)| index + c.len_utf8())
            .unwrap_or(slice.len());
        words.push(Word {
            text: slice[trimmed_start..trimmed_end].to_string(),
            start: from + trimmed_start,
            end: from + trimmed_end,
        });
    };
    for (index, character) in question.char_indices() {
        if character.is_whitespace() {
            if let Some(from) = start.take() {
                flush(from, index, &mut words);
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(from) = start {
        flush(from, question.len(), &mut words);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_keep_inner_punctuation_and_lose_the_edges() {
        let words: Vec<String> = words_of("What did (Jean-Pierre Dupont), d'Alembert ? Dr. Roy")
            .into_iter()
            .map(|word| word.text)
            .collect();

        assert_eq!(
            words,
            [
                "What",
                "did",
                "Jean-Pierre",
                "Dupont",
                "d'Alembert",
                "Dr",
                "Roy"
            ]
        );
    }

    #[test]
    fn a_word_knows_where_it_sits_in_the_question() {
        let question = "Who knows Dupont ?";
        let words = words_of(question);

        let dupont = &words[2];
        assert_eq!(&question[dupont.start..dupont.end], "Dupont");
    }

    #[test]
    fn an_empty_or_punctuation_only_question_has_no_words() {
        assert!(words_of("").is_empty());
        assert!(words_of("  ?! ... ").is_empty());
    }
}
