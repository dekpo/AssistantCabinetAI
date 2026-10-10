//! From what an extractor found in a document to the delta the store applies.
//!
//! The extractor (`extract::text`) only reads. This module decides, name by name, what each find
//! means for the knowledge base, and keeps the pass's long-lived state: the packs, the phonetic
//! encoder, the gazetteer that grows as names are learned, and the key that hashes personal
//! identifiers.
//!
//! ```text
//! chunks --extract--> candidates --resolve--> EntityRef (existing | new draft | nothing)
//!                                                |
//!                          one KnowledgeDelta per document, applied with the document's chunks
//! ```
//!
//! What it never does: contact the gateway, read the original file, write a passage of text, or
//! let a failure stop a document from being indexed (the caller bypasses the knowledge step and the
//! chunks are committed regardless).
//!
//! Order matters inside one document. Names that stand on their own (a full name, an organisation,
//! an identifier) are placed first; then the short forms (a surname, an initial and a surname) are
//! placed against the people the document has just been found to name, because a short form is only
//! ever read as "that person" when exactly one of them in this document answers to it
//! (`docs/DECISIONS.md`, "Owner answers after KB lot 3"). The entities this document creates are
//! not in the store yet, so the resolver reads the store through a layer that also shows it the
//! drafts made so far.

use std::collections::{BTreeSet, HashMap};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use rusqlite::Connection;

use super::extract::text::DeterministicTextExtractor;
use super::extract::{Candidate, ChunkInput, DocumentInput, EntityExtractor, ExtractContext};
use super::gazetteer::{Gazetteer, GazetteerEntry};
use super::maintenance;
use super::normalize::{normalize_name, TitleSet};
use super::packs::PackSet;
use super::phonetic::{encoder_for_locale, PhoneticEncoder};
use super::resolve::{
    aliases_for, AliasRequest, DeterministicResolver, EntityHit, EntityLookup, EntityResolver,
    InMemoryLookup, Resolution, ResolveContext, ResolveMode, SourceSet, StoreLookup, SurfaceInput,
    Via,
};
use super::secret::IdentifierKey;
use super::store;
use super::{
    AliasDraft, AliasKind, AttributeDraft, Domain, EntityDraft, EntityRef, EntityStatus,
    EntityTypeId, KnowledgeDelta, KnowledgeMode, KnowledgeSummary, MentionDraft, MentionLocator,
    Method, Origin, PossibleMatchDraft, SourceRef, CURRENT_KB_VERSION,
};
use crate::chunking::Chunk;
use crate::error::AppError;
use crate::extraction::PageOrigin;
use crate::index_store::{IndexStore, StoredChunkText};

/// Where the ids of entities that exist only inside one document's delta start. Far below any row
/// id, so a draft can never be mistaken for a stored entity.
const DRAFT_FIRST_ID: i64 = -1_000_000_000;

/// `kb_meta` key holding the fingerprint of the key that hashed the stored identifiers.
const KEY_CHECK_META: &str = "identifier_key_check";

/// `kb_meta` key holding how many (phrase, entity, kind) triples the gazetteer had when the epoch
/// was last settled. A list that is bigger at the start of a pass was taught by someone else (a
/// manual edit, the Data Folder's pass) and the documents read before are behind it.
const GAZETTEER_ENTRIES_META: &str = "gazetteer_entries";

/// The most rounds a refresh takes: each round reads every document that is behind the list of
/// names, and learning can make the list grow again. Whatever is still behind is picked up by the
/// next pass.
pub const MAX_REFRESH_ROUNDS: usize = 4;

// ---------------------------------------------------------------------------------------------
// The pass's state
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Tally {
    /// Documents whose knowledge was written by this pass, in the loop or by a refresh.
    touched: BTreeSet<String>,
    /// Of those, the ones the pass itself analysed (as opposed to read again from the index).
    analysed_now: BTreeSet<String>,
    refreshed: BTreeSet<String>,
    failed: BTreeSet<String>,
    truncated: BTreeSet<String>,
    entity_id_before: i64,
}

/// What building a document's delta produced, besides the delta itself.
#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub delta: KnowledgeDelta,
    /// The document held more names than are kept.
    pub truncated: bool,
    /// Candidates the resolver would not place: ambiguous, a lone surname no one in the document
    /// answers to, a name the user deleted. Counted, never named.
    pub unplaced: usize,
}

/// Everything a pass over documents needs to learn names from them. Built once per pass, never per
/// file or per question (loading the packs costs tens of milliseconds).
pub struct KnowledgeContext {
    pub mode: KnowledgeMode,
    /// The generation of the extractor in this build. A source read by a lower one is read again.
    /// A field so a test can move it without rebuilding the program.
    pub kb_version: u32,
    locale: String,
    packs: PackSet,
    titles: TitleSet,
    encoder: Box<dyn PhoneticEncoder>,
    extractor: Box<dyn EntityExtractor>,
    resolver: Box<dyn EntityResolver>,
    gazetteer: Gazetteer,
    /// The gazetteer's size when the epoch was last settled: growth beyond it means the documents
    /// read before are behind.
    counted_entries: usize,
    key: IdentifierKey,
    tally: Tally,
    /// Time spent reading names (building deltas and teaching the gazetteer) in this pass: what
    /// the knowledge base cost, apart from the embeddings and the writes.
    spent_micros: AtomicU64,
}

impl KnowledgeContext {
    /// A context for `KnowledgeMode::Off`: it reads nothing and writes nothing. The packs are
    /// loaded because the type needs them, the database is never touched.
    pub fn off(locale: &str) -> Result<Self, AppError> {
        Self::assemble(
            locale,
            &[],
            KnowledgeMode::Off,
            IdentifierKey::from_bytes([0; 32]),
        )
    }

    /// The context of a real pass: packs and encoder for `locale`, the phonetic keys and the
    /// packs' fingerprint brought up to date, the gazetteer built from the store. The epoch is
    /// raised when the vocabulary (the packs) or the identifier key changed since the last pass, so
    /// the documents read with the old ones are read again.
    pub fn standard(
        connection: &Connection,
        locale: &str,
        active_packs: &[&str],
        mode: KnowledgeMode,
        key: IdentifierKey,
    ) -> Result<Self, AppError> {
        let mut context = Self::assemble(locale, active_packs, mode, key)?;
        if !mode.reads_files() {
            return Ok(context);
        }
        maintenance::ensure_phonetic_keys(connection, &context.titles, context.encoder.as_ref())?;
        let packs_changed = maintenance::record_packs(connection, &context.packs)?;
        let key_changed = note_key(connection, &context.key)?;
        context.gazetteer = Gazetteer::rebuild_from_store(connection)?;
        let known = context.gazetteer.entry_count();
        let recorded: usize = store::meta(connection, GAZETTEER_ENTRIES_META)?
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0);
        if packs_changed || key_changed || known > recorded {
            context.gazetteer.bump_epoch(connection)?;
        }
        if known != recorded {
            store::set_meta(connection, GAZETTEER_ENTRIES_META, &known.to_string())?;
        }
        context.counted_entries = known;
        Ok(context)
    }

    /// [`KnowledgeContext::standard`] over an index, for callers outside the crate (the tests).
    pub fn for_index(
        index: &IndexStore,
        locale: &str,
        active_packs: &[&str],
        mode: KnowledgeMode,
        key: IdentifierKey,
    ) -> Result<Self, AppError> {
        Self::standard(index.connection(), locale, active_packs, mode, key)
    }

    fn assemble(
        locale: &str,
        active_packs: &[&str],
        mode: KnowledgeMode,
        key: IdentifierKey,
    ) -> Result<Self, AppError> {
        let packs = PackSet::load(locale, active_packs)?;
        let titles = packs.title_set();
        Ok(Self {
            mode,
            kb_version: CURRENT_KB_VERSION,
            locale: locale.to_string(),
            titles,
            packs,
            encoder: encoder_for_locale(locale),
            extractor: Box::new(DeterministicTextExtractor),
            resolver: Box::new(DeterministicResolver),
            gazetteer: Gazetteer::new(),
            counted_entries: 0,
            key,
            tally: Tally::default(),
            spent_micros: AtomicU64::new(0),
        })
    }

    /// Another extractor in place of the deterministic one (a test double, later a model).
    pub fn with_extractor(mut self, extractor: Box<dyn EntityExtractor>) -> Self {
        self.extractor = extractor;
        self
    }

    pub fn with_kb_version(mut self, kb_version: u32) -> Self {
        self.kb_version = kb_version;
        self
    }

    pub fn gazetteer(&self) -> &Gazetteer {
        &self.gazetteer
    }

    pub fn packs(&self) -> &PackSet {
        &self.packs
    }

    // ----------------------------------------------------------------------------- the pass

    /// Start counting a pass: forget the last one's tally, remember the highest entity id so that
    /// "new" can be told from "already known" afterwards.
    pub fn begin_pass(&mut self, connection: &Connection) {
        self.tally = Tally {
            entity_id_before: store::max_entity_id(connection).unwrap_or(0),
            ..Tally::default()
        };
        self.spent_micros.store(0, Ordering::Relaxed);
    }

    fn spend(&self, started: Instant) {
        self.spent_micros
            .fetch_add(started.elapsed().as_micros() as u64, Ordering::Relaxed);
    }

    /// Settle the epoch: if the pass taught the gazetteer anything, the documents read before are
    /// behind it, so the epoch moves and they become due. Returns whether it moved.
    pub fn settle_epoch(&mut self, connection: &Connection) -> Result<bool, AppError> {
        if self.gazetteer.entry_count() > self.counted_entries {
            self.gazetteer.bump_epoch(connection)?;
            self.counted_entries = self.gazetteer.entry_count();
            store::set_meta(
                connection,
                GAZETTEER_ENTRIES_META,
                &self.counted_entries.to_string(),
            )?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn epoch(&self) -> u32 {
        self.gazetteer.epoch()
    }

    pub fn note_failed(&mut self, relative_path: &str) {
        self.tally.failed.insert(relative_path.to_string());
    }

    /// A document was read again from its stored text. It counts as "read again" only when the
    /// pass did not analyse it itself: a file analysed in this pass and then read once more because
    /// the list of names grew is not an earlier document.
    pub fn note_refreshed(&mut self, relative_path: &str) {
        if !self.tally.analysed_now.contains(relative_path) {
            self.tally.refreshed.insert(relative_path.to_string());
        }
        self.tally.touched.insert(relative_path.to_string());
    }

    /// The counts for the Analyse summary. `Domain::Documents` only: tables are lot 5's.
    pub fn summary(&self, connection: &Connection) -> KnowledgeSummary {
        let touched: Vec<String> = self.tally.touched.iter().cloned().collect();
        let counts = store::entity_counts_for_sources(
            connection,
            Domain::Documents,
            &touched,
            self.tally.entity_id_before,
        )
        .unwrap_or_default();
        KnowledgeSummary {
            entities_detected: counts.distinct,
            matched_existing: counts.distinct.saturating_sub(counts.created),
            created_new: counts.created,
            candidates: counts.created_candidates,
            identifiers: counts.identifiers,
            relations: 0,
            refreshed_sources: self.tally.refreshed.len(),
            truncated_sources: self.tally.truncated.len(),
            errors: self.tally.failed.len(),
            elapsed_ms: self.spent_micros.load(Ordering::Relaxed) / 1_000,
        }
    }

    // ----------------------------------------------------------------------------- one document

    /// Write one document the way the pass does, whatever the mode: the chunks always, the knowledge
    /// when the mode reads files and nothing went wrong, a bypass counted otherwise. The only
    /// error is a failure of the index itself.
    #[allow(clippy::too_many_arguments)]
    pub fn store_document(
        &mut self,
        index: &mut IndexStore,
        relative_path: &str,
        sha256: &str,
        empty: bool,
        chunks: &[Chunk],
        embeddings: &[Vec<f32>],
        ocr_engine: Option<&str>,
        ocr_engine_version: Option<&str>,
    ) -> Result<(), AppError> {
        if !self.mode.reads_files() {
            return index.replace_document_plain(
                relative_path,
                sha256,
                empty,
                chunks,
                embeddings,
                ocr_engine,
                ocr_engine_version,
            );
        }

        let source = SourceRef {
            domain: Domain::Documents,
            relative_path: relative_path.to_string(),
            content_id: sha256.to_string(),
        };
        let inputs: Vec<ChunkInput> = chunks.iter().map(chunk_input).collect();
        let built = self.build(index.connection(), source.clone(), inputs);
        let delta = match &built {
            Ok(built) => built.delta.clone(),
            Err(_) => {
                self.note_failed(relative_path);
                // Not extracted: the previous version's knowledge is removed with the old text and
                // the source stays due, so the next pass tries again.
                KnowledgeDelta::empty(source)
            }
        };
        let write = index.replace_document_with_knowledge(
            relative_path,
            sha256,
            empty,
            chunks,
            embeddings,
            ocr_engine,
            ocr_engine_version,
            &delta,
        )?;
        match (write, built) {
            (crate::knowledge::KnowledgeWrite::Applied(_), Ok(built)) => {
                self.tally.touched.insert(relative_path.to_string());
                self.tally.analysed_now.insert(relative_path.to_string());
                if built.truncated {
                    self.tally.truncated.insert(relative_path.to_string());
                }
                self.learn(index.connection(), &built.delta);
            }
            (crate::knowledge::KnowledgeWrite::Applied(_), Err(_)) => {}
            (crate::knowledge::KnowledgeWrite::Bypassed, _) => self.note_failed(relative_path),
        }
        Ok(())
    }

    /// Teach the gazetteer the names this delta just made active: the entities it created or
    /// matched, and any candidate a second source promoted.
    pub fn learn(&mut self, connection: &Connection, delta: &KnowledgeDelta) {
        let started = Instant::now();
        let mut ids: BTreeSet<i64> = BTreeSet::new();
        for mention in &delta.mentions {
            if let EntityRef::Existing(id) = mention.entity {
                ids.insert(id);
            }
        }
        for draft in &delta.entities {
            if let Ok(Some(id)) = store::entity_id_by_name(
                connection,
                &draft.type_id,
                &draft.normalized_name,
                &draft.disambiguator,
            ) {
                ids.insert(id);
            }
        }
        for id in ids {
            let Ok(Some(record)) = store::entity_with_aliases(connection, id) else {
                continue;
            };
            if record.entity.status != EntityStatus::Active
                || record.entity.type_id == EntityTypeId::IDENTIFIER
            {
                continue;
            }
            for alias in &record.aliases {
                self.gazetteer.insert(
                    &alias.normalized,
                    GazetteerEntry {
                        entity_id: id,
                        type_id: record.entity.type_id.clone(),
                        kind: alias.kind,
                        origin: alias.origin,
                    },
                );
            }
        }
        self.spend(started);
    }

    // ----------------------------------------------------------------------------- building

    /// Read a document's chunks and decide what they mean. Pure with respect to the store: it only
    /// looks. A panic in an extractor, or a store that cannot be read, is an error the caller
    /// turns into a bypass.
    pub fn build(
        &self,
        connection: &Connection,
        source: SourceRef,
        chunks: Vec<ChunkInput>,
    ) -> Result<Built, AppError> {
        let started = Instant::now();
        let built = self.build_untimed(connection, source, chunks);
        self.spend(started);
        built
    }

    fn build_untimed(
        &self,
        connection: &Connection,
        source: SourceRef,
        chunks: Vec<ChunkInput>,
    ) -> Result<Built, AppError> {
        let mut delta = KnowledgeDelta::empty(source.clone());
        delta.kb_version = self.kb_version;
        delta.gazetteer_epoch = self.gazetteer.epoch();
        if chunks.is_empty() {
            return Ok(Built {
                delta,
                truncated: false,
                unplaced: 0,
            });
        }

        let input = DocumentInput { source, chunks };
        let found = catch_unwind(AssertUnwindSafe(|| {
            self.extractor.extract_document(
                &input,
                &ExtractContext {
                    packs: &self.packs,
                    locale: &self.locale,
                    gazetteer: &self.gazetteer,
                    encoder: self.encoder.as_ref(),
                },
            )
        }))
        .map_err(|_| AppError::KnowledgeUnavailable)?;

        let mut builder = DeltaBuilder::new(self, connection, delta);
        builder.place_all(found.candidates)?;
        let (delta, unplaced) = builder.finish();
        Ok(Built {
            delta,
            truncated: found.truncated,
            unplaced,
        })
    }
}

fn chunk_input(chunk: &Chunk) -> ChunkInput {
    ChunkInput {
        chunk_id: chunk.chunk_id.clone(),
        text: chunk.text.clone(),
        ocr_confidence: ocr_confidence(chunk.origin, chunk.confidence),
    }
}

/// How sure the engine was of a chunk, `None` for native text. A chunk a machine read with no score
/// is treated as read badly rather than well.
pub fn ocr_confidence(origin: PageOrigin, confidence: Option<f32>) -> Option<f32> {
    match origin {
        PageOrigin::Ocr => Some(confidence.unwrap_or(0.0)),
        PageOrigin::TextLayer => None,
    }
}

pub fn stored_chunk_input(chunk: &StoredChunkText) -> ChunkInput {
    ChunkInput {
        chunk_id: chunk.chunk_id.clone(),
        text: chunk.text.clone(),
        ocr_confidence: ocr_confidence(chunk.origin, chunk.confidence),
    }
}

/// Record the fingerprint of the identifier key and say whether it is not the one the stored
/// identifiers were hashed with. The first key ever made changes nothing (no identifier is stored
/// yet).
fn note_key(connection: &Connection, key: &IdentifierKey) -> Result<bool, AppError> {
    let current = key.fingerprint();
    let previous = store::meta(connection, KEY_CHECK_META)?.unwrap_or_default();
    if previous == current {
        return Ok(false);
    }
    store::set_meta(connection, KEY_CHECK_META, &current)?;
    Ok(!previous.is_empty())
}

// ---------------------------------------------------------------------------------------------
// The store, and the drafts of the document being read, as one lookup
// ---------------------------------------------------------------------------------------------

struct Layered<'a> {
    base: StoreLookup<'a>,
    drafts: InMemoryLookup,
}

impl EntityLookup for Layered<'_> {
    fn by_identifier(&self, scheme: &str, normalized: &str) -> Vec<EntityHit> {
        let mut hits = self.base.by_identifier(scheme, normalized);
        hits.extend(self.drafts.by_identifier(scheme, normalized));
        hits
    }

    fn by_normalized(&self, normalized: &str) -> Vec<EntityHit> {
        let mut hits = self.base.by_normalized(normalized);
        hits.extend(self.drafts.by_normalized(normalized));
        hits
    }

    fn by_alias(&self, normalized: &str) -> Vec<EntityHit> {
        let mut hits = self.base.by_alias(normalized);
        hits.extend(self.drafts.by_alias(normalized));
        hits
    }

    fn by_token_set(&self, key: &str) -> Vec<EntityHit> {
        let mut hits = self.base.by_token_set(key);
        hits.extend(self.drafts.by_token_set(key));
        hits
    }

    fn by_phonetic(&self, key: &str) -> Vec<EntityHit> {
        let mut hits = self.base.by_phonetic(key);
        hits.extend(self.drafts.by_phonetic(key));
        hits
    }

    fn is_tombstoned(&self, type_id: &str, normalized: &str) -> bool {
        self.base.is_tombstoned(type_id, normalized)
    }

    fn restrict_to_selection(
        &self,
        hits: Vec<EntityHit>,
        _selection: &SourceSet,
    ) -> Vec<EntityHit> {
        // Ingestion has no selection.
        hits
    }

    fn failed(&self) -> bool {
        self.base.failed()
    }
}

// ---------------------------------------------------------------------------------------------
// Placing candidates
// ---------------------------------------------------------------------------------------------

/// Letters in capitals only ("DUPONT") are shown in capitals and lower case ("Dupont"); anything
/// else is shown as it was written.
fn display_name(surface: &str) -> String {
    surface
        .split_whitespace()
        .map(|word| {
            let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
            let shouting = letters.len() > 1 && letters.iter().all(|c| c.is_uppercase());
            if !shouting {
                return word.to_string();
            }
            let mut previous_is_letter = false;
            word.chars()
                .flat_map(|c| {
                    let out: Vec<char> = if c.is_alphabetic() && previous_is_letter {
                        c.to_lowercase().collect()
                    } else {
                        vec![c]
                    };
                    previous_is_letter = c.is_alphabetic();
                    out
                })
                .collect()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

enum Placed {
    /// Linked to an entity (stored, or drafted by this document).
    To {
        target: EntityRef,
        alias: Option<String>,
        /// Reached through a surname, an initial or a title and a surname.
        through_short_form: bool,
        confidence: f32,
    },
    /// Not recorded.
    Nowhere,
}

struct Group {
    /// The member whose find was the most confident: it speaks for the group.
    lead: usize,
    members: Vec<usize>,
    titles_seen: Vec<String>,
}

struct DeltaBuilder<'a> {
    context: &'a KnowledgeContext,
    lookup: Layered<'a>,
    delta: KnowledgeDelta,
    /// Ids (stored and drafted) of the entities this document has been found to name so far.
    source_entities: BTreeSet<i64>,
    /// Draft id in the lookup -> temp id in the delta.
    draft_of: HashMap<i64, u32>,
    next_temp: u32,
    mention_index: HashMap<String, usize>,
    unplaced: usize,
}

impl<'a> DeltaBuilder<'a> {
    fn new(
        context: &'a KnowledgeContext,
        connection: &'a Connection,
        delta: KnowledgeDelta,
    ) -> Self {
        let drafts =
            InMemoryLookup::new(context.titles.clone(), encoder_for_locale(&context.locale))
                .starting_at(DRAFT_FIRST_ID);
        Self {
            context,
            lookup: Layered {
                base: StoreLookup::new(connection, &context.titles),
                drafts,
            },
            delta,
            source_entities: BTreeSet::new(),
            draft_of: HashMap::new(),
            next_temp: 1,
            mention_index: HashMap::new(),
            unplaced: 0,
        }
    }

    fn finish(self) -> (KnowledgeDelta, usize) {
        (self.delta, self.unplaced)
    }

    /// Every candidate of the document, in the order the table at the top of this file gives.
    fn place_all(&mut self, candidates: Vec<Candidate>) -> Result<(), AppError> {
        let mut identifiers: Vec<Candidate> = Vec::new();
        let mut from_file_name: Vec<Candidate> = Vec::new();
        let mut names: Vec<Candidate> = Vec::new();
        for candidate in candidates {
            if candidate.identifier.is_some() {
                identifiers.push(candidate);
            } else if candidate.locator == MentionLocator::Filename {
                from_file_name.push(candidate);
            } else {
                names.push(candidate);
            }
        }

        self.place_identifiers(&identifiers);

        let groups = self.group(&names);
        let (short, whole): (Vec<&Group>, Vec<&Group>) = groups
            .iter()
            .partition(|group| self.is_short_form(&names[group.lead]));
        for group in whole.into_iter().chain(short) {
            // A surname alone never makes a person: it is linked to one this document names in
            // full, or it records nothing (owner decision, 10 October 2026).
            let may_create = !self.is_surname_alone(&names[group.lead]);
            self.place_group(&names, group, may_create);
        }

        for candidate in &from_file_name {
            self.place_file_name(candidate);
        }

        if self.lookup.failed() {
            // Some read failed, so some candidates were passed over for that reason: the delta is
            // partial, and a partial delta would erase what the document's earlier version had
            // legitimately recorded. Better no knowledge now than a wrong subset.
            return Err(AppError::KnowledgeUnavailable);
        }
        Ok(())
    }

    fn group(&self, names: &[Candidate]) -> Vec<Group> {
        let mut order: Vec<String> = Vec::new();
        let mut by_key: HashMap<String, Group> = HashMap::new();
        for (position, candidate) in names.iter().enumerate() {
            let key = format!(
                "{}:{}",
                candidate.type_id.as_str(),
                normalize_name(&candidate.surface, &self.context.titles).joined
            );
            let group = by_key.entry(key.clone()).or_insert_with(|| {
                order.push(key.clone());
                Group {
                    lead: position,
                    members: Vec::new(),
                    titles_seen: Vec::new(),
                }
            });
            group.members.push(position);
            if candidate.confidence > names[group.lead].confidence {
                group.lead = position;
            }
            for title in &candidate.titles_seen {
                if !group.titles_seen.contains(title) {
                    group.titles_seen.push(title.clone());
                }
            }
        }
        order
            .into_iter()
            .filter_map(|key| by_key.remove(&key))
            .collect()
    }

    /// A surname alone, or an initial and a surname: placed after the full names.
    fn is_short_form(&self, candidate: &Candidate) -> bool {
        if candidate.type_id.as_str() != EntityTypeId::PERSON {
            return false;
        }
        let name = normalize_name(&candidate.surface, &self.context.titles);
        name.tokens.len() <= 1 || !name.initials.is_empty()
    }

    /// One word standing for a person: not enough to make an entity.
    fn is_surname_alone(&self, candidate: &Candidate) -> bool {
        candidate.type_id.as_str() == EntityTypeId::PERSON
            && normalize_name(&candidate.surface, &self.context.titles)
                .tokens
                .len()
                <= 1
    }

    fn place_group(&mut self, names: &[Candidate], group: &Group, may_create: bool) {
        let lead = &names[group.lead];
        let placed = self.place(lead, &group.titles_seen, may_create);
        match placed {
            Placed::Nowhere => {
                self.unplaced += group.members.len();
            }
            Placed::To {
                target,
                alias,
                through_short_form,
                confidence,
            } => {
                for &position in &group.members {
                    let member = &names[position];
                    let (method, kept_confidence) =
                        if through_short_form || member.method == Method::CorefInDocument {
                            (
                                Method::CorefInDocument,
                                confidence.min(member.confidence.value()),
                            )
                        } else {
                            (member.method, member.confidence.value())
                        };
                    self.add_mention(
                        target,
                        alias.clone(),
                        member.locator.clone(),
                        member.occurrence_count,
                        kept_confidence,
                        method,
                    );
                }
            }
        }
    }

    /// Ask the resolver, then act on the answer: link, create, or leave alone.
    fn place(&mut self, candidate: &Candidate, titles_seen: &[String], may_create: bool) -> Placed {
        let mut surface = SurfaceInput::new(&candidate.surface)
            .of_type(candidate.type_id.clone())
            .found_by(candidate.method);
        if candidate.method == Method::CorefInDocument {
            // A short form found again: it is read as the short form it is.
            surface = surface.found_by(Method::TitleSurname);
        }
        let answer = {
            let context = ResolveContext {
                mode: ResolveMode::Ingestion,
                titles: &self.context.titles,
                encoder: self.context.encoder.as_ref(),
                lookup: &self.lookup,
                selection: None,
                source_entities: Some(&self.source_entities),
            };
            self.context.resolver.resolve(&surface, &context)
        };

        match answer {
            Resolution::Existing {
                entity,
                via,
                confidence,
                ..
            } => {
                let through_short_form = matches!(
                    via,
                    Via::Alias(kind) if kind.is_ambiguous_by_nature() || kind == AliasKind::Initial
                );
                let alias = entity
                    .alias_normalized
                    .clone()
                    .or_else(|| Some(entity.normalized_name.clone()));
                self.source_entities.insert(entity.entity_id);
                Placed::To {
                    target: self.reference(entity.entity_id),
                    alias,
                    through_short_form,
                    confidence: confidence.value(),
                }
            }
            Resolution::NewEntity { status, .. } if may_create => {
                match self.create(candidate, titles_seen, status) {
                    Some((target, alias)) => Placed::To {
                        target,
                        alias,
                        through_short_form: false,
                        confidence: candidate.confidence.value(),
                    },
                    None => Placed::Nowhere,
                }
            }
            Resolution::PossibleMatch {
                entities,
                reason,
                score,
                create_separate,
            } if may_create && create_separate => {
                let status = if candidate.method.is_supported() {
                    EntityStatus::Active
                } else {
                    EntityStatus::Candidate
                };
                match self.create(candidate, titles_seen, status) {
                    Some((target, alias)) => {
                        for hit in &entities {
                            let other = self.reference(hit.entity_id);
                            if other != target {
                                self.delta.possible_matches.push(PossibleMatchDraft {
                                    subject: target,
                                    other,
                                    reason: reason.as_code().to_string(),
                                    score: score.value(),
                                });
                            }
                        }
                        Placed::To {
                            target,
                            alias,
                            through_short_form: false,
                            confidence: candidate.confidence.value(),
                        }
                    }
                    None => Placed::Nowhere,
                }
            }
            // Ambiguous, unresolved (a surname no one here answers to, a deleted name, a failed
            // read, which `place_all` turns into a bypass of the whole document), or a name that
            // may not create: recorded nowhere.
            _ => Placed::Nowhere,
        }
    }

    fn reference(&self, entity_id: i64) -> EntityRef {
        match self.draft_of.get(&entity_id) {
            Some(temp) => EntityRef::Draft(*temp),
            None => EntityRef::Existing(entity_id),
        }
    }

    /// A new entity from a candidate: the draft, its aliases, and a place in the lookup so the
    /// rest of the document finds it.
    fn create(
        &mut self,
        candidate: &Candidate,
        titles_seen: &[String],
        status: EntityStatus,
    ) -> Option<(EntityRef, Option<String>)> {
        // Capital letters are a style in a surname ("DUPONT Jean") and a fact in an organisation's
        // name ("SARL", "IBM"): only a person's name is re-cased.
        let canonical = if candidate.type_id.as_str() == EntityTypeId::PERSON {
            display_name(&candidate.surface)
        } else {
            candidate.surface.clone()
        };
        let normalized_name = normalize_name(&canonical, &self.context.titles).joined;
        if normalized_name.is_empty() {
            return None;
        }
        let aliases = aliases_for(
            &AliasRequest {
                type_id: &candidate.type_id,
                canonical: &canonical,
                titles_seen,
                acronym: None,
            },
            &self.context.titles,
            self.context.encoder.as_ref(),
        );

        let temp_id = self.next_temp;
        self.next_temp += 1;
        self.delta.entities.push(EntityDraft {
            temp_id,
            type_id: candidate.type_id.as_str().to_string(),
            subtype: candidate.subtype.clone(),
            canonical_name: canonical.clone(),
            normalized_name: normalized_name.clone(),
            disambiguator: String::new(),
            status,
        });
        for alias in &aliases {
            self.delta.aliases.push(AliasDraft {
                entity: EntityRef::Draft(temp_id),
                display: alias.display.clone(),
                normalized: alias.normalized.clone(),
                phonetic_key: alias.phonetic_key.clone(),
                kind: alias.kind,
                confidence: alias.confidence,
            });
        }

        let draft_id = self.lookup.drafts.add_with(
            candidate.type_id.as_str(),
            &canonical,
            status,
            Origin::Automatic,
            "",
        );
        for alias in aliases.iter().filter(|a| a.kind == AliasKind::TitleForm) {
            self.lookup
                .drafts
                .add_alias(draft_id, &alias.display, alias.kind, Origin::Automatic);
        }
        self.draft_of.insert(draft_id, temp_id);
        self.source_entities.insert(draft_id);
        Some((EntityRef::Draft(temp_id), Some(normalized_name)))
    }

    /// Words of the file's own name that matched a known name: a link to what exists, never a new
    /// entity.
    fn place_file_name(&mut self, candidate: &Candidate) {
        let placed = self.place(candidate, &[], false);
        if let Placed::To { target, alias, .. } = placed {
            self.add_mention(
                target,
                alias,
                MentionLocator::Filename,
                1,
                candidate.confidence.value(),
                Method::FilenameToken,
            );
        } else {
            self.unplaced += 1;
        }
    }

    /// An identifier is the entity: the same reduced value is the same entity, wherever it is
    /// written. A personal one (an e-mail address, a social security number, an IBAN) is stored as
    /// a keyed hash, and its name is a neutral label that says what kind of identifier it is and
    /// nothing of its value.
    fn place_identifiers(&mut self, identifiers: &[Candidate]) {
        let mut temp_of: HashMap<String, EntityRef> = HashMap::new();
        for candidate in identifiers {
            let Some(found) = &candidate.identifier else {
                continue;
            };
            let Some(scheme) = self.context.packs.identifier_scheme(&found.scheme) else {
                self.unplaced += 1;
                continue;
            };
            let personal = scheme.personal_key;
            let stored_value = if personal {
                self.context.key.digest(&scheme.id, &found.normalized)
            } else {
                found.normalized.clone()
            };
            let key = format!("{}:{}", scheme.id, stored_value);

            let target = match temp_of.get(&key) {
                Some(target) => *target,
                None => {
                    let tombstoned = self
                        .lookup
                        .base
                        .is_tombstoned(EntityTypeId::IDENTIFIER, &stored_value);
                    if tombstoned {
                        self.unplaced += 1;
                        continue;
                    }
                    let label = if personal {
                        format!("{} {}", scheme.kind, &stored_value[..6])
                    } else {
                        candidate.surface.clone()
                    };
                    let alias_normalized = normalize_name(&label, &self.context.titles).joined;
                    let temp_id = self.next_temp;
                    self.next_temp += 1;
                    self.delta.entities.push(EntityDraft {
                        temp_id,
                        type_id: EntityTypeId::IDENTIFIER.to_string(),
                        subtype: Some(scheme.kind.clone()),
                        canonical_name: label.clone(),
                        normalized_name: stored_value.clone(),
                        disambiguator: String::new(),
                        status: EntityStatus::Active,
                    });
                    if !alias_normalized.is_empty() {
                        self.delta.aliases.push(AliasDraft {
                            entity: EntityRef::Draft(temp_id),
                            display: label,
                            normalized: alias_normalized,
                            phonetic_key: String::new(),
                            kind: AliasKind::CanonicalVariant,
                            confidence: 1.0,
                        });
                    }
                    self.delta.attributes.push(AttributeDraft {
                        entity: EntityRef::Draft(temp_id),
                        key: format!("identifier:{}", scheme.id),
                        value: stored_value.clone(),
                    });
                    let target = EntityRef::Draft(temp_id);
                    temp_of.insert(key, target);
                    target
                }
            };
            self.add_mention(
                target,
                None,
                candidate.locator.clone(),
                candidate.occurrence_count,
                candidate.confidence.value(),
                Method::Identifier,
            );
        }
    }

    /// One mention per entity and place: two finds of the same entity in the same chunk are one
    /// mention counted twice.
    fn add_mention(
        &mut self,
        target: EntityRef,
        alias: Option<String>,
        locator: MentionLocator,
        occurrences: u32,
        confidence: f32,
        method: Method,
    ) {
        let key = format!("{target:?}|{locator:?}");
        match self.mention_index.get(&key) {
            Some(&position) => {
                let known = &mut self.delta.mentions[position];
                known.occurrence_count += occurrences;
                if confidence > known.confidence {
                    known.confidence = confidence;
                    known.method = method.as_code().to_string();
                    if alias.is_some() {
                        known.alias_normalized = alias;
                    }
                }
            }
            None => {
                self.mention_index.insert(key, self.delta.mentions.len());
                self.delta.mentions.push(MentionDraft {
                    entity: target,
                    alias_normalized: alias,
                    role: None,
                    locator,
                    occurrence_count: occurrences,
                    confidence,
                    method: method.as_code().to_string(),
                    extractor_version: self.context.extractor.version(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shouting_surname_is_shown_in_lower_case_and_the_rest_is_left_as_written() {
        assert_eq!(display_name("DUPONT Jean"), "Dupont Jean");
        assert_eq!(display_name("Jean DUPONT-MARTIN"), "Jean Dupont-Martin");
        assert_eq!(display_name("Jean Dupont"), "Jean Dupont");
        assert_eq!(display_name("J. Dupont"), "J. Dupont");
        assert_eq!(display_name("McDonald"), "McDonald");
    }

    #[test]
    fn a_chunk_read_by_a_machine_with_no_score_is_read_as_badly_read() {
        assert_eq!(ocr_confidence(PageOrigin::TextLayer, None), None);
        assert_eq!(ocr_confidence(PageOrigin::TextLayer, Some(0.2)), None);
        assert_eq!(ocr_confidence(PageOrigin::Ocr, Some(0.9)), Some(0.9));
        assert_eq!(ocr_confidence(PageOrigin::Ocr, None), Some(0.0));
    }
}
