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

use super::extract::table::{TableEntityExtractor, TableReading};
use super::extract::text::DeterministicTextExtractor;
use super::extract::{
    Candidate, ChunkInput, ColumnInput, ColumnKind, DocumentInput, EntityExtractor, ExtractContext,
    SheetInput, WorkbookInput,
};
use super::gazetteer::{is_listed_type, Gazetteer, GazetteerEntry};
use super::maintenance;
use super::normalize::{fold_word, given_name_first, normalize_name, TitleSet};
use super::packs::PackSet;
use super::phonetic::{encoder_for_locale, PhoneticEncoder};
use super::resolve::{
    alias_phonetic_key, aliases_for, AliasRequest, DeterministicResolver, EntityHit, EntityLookup,
    EntityResolver, InMemoryLookup, Resolution, ResolveContext, ResolveMode, SourceSet,
    StoreLookup, SurfaceInput, Via,
};
use super::secret::IdentifierKey;
use super::store;
use super::{
    AliasDraft, AliasKind, AttributeDraft, ColumnSemanticsDraft, Domain, EntityDraft, EntityRef,
    EntityStatus, EntityTypeId, KnowledgeDelta, KnowledgeMode, KnowledgeSummary, MentionDraft,
    MentionLocator, Method, NewAlias, Origin, PossibleMatchDraft, RelationDraft, SourceRef,
    CURRENT_KB_VERSION,
};
use crate::chunking::Chunk;
use crate::error::AppError;
use crate::extraction::PageOrigin;
use crate::index_store::{IndexStore, StoredChunkText};
use crate::tabular::inventory::{data_rows, text_value, ColumnType, TabularInventory};
use crate::tabular::{CellValue, SourceRows, Workbook};

/// Where the ids of entities that exist only inside one document's delta start. Far below any row
/// id, so a draft can never be mistaken for a stored entity.
const DRAFT_FIRST_ID: i64 = -1_000_000_000;

/// `kb_meta` key holding the fingerprint of the key that hashed the stored identifiers.
const KEY_CHECK_META: &str = "identifier_key_check";

/// `kb_meta` key holding how many (phrase, entity, kind) triples the gazetteer had when the epoch
/// was last settled. A list that is bigger at the start of a pass was taught by someone else (a
/// manual edit, the Data Folder's pass) and the documents read before are behind it.
const GAZETTEER_ENTRIES_META: &str = "gazetteer_entries";

/// `kb_meta` key holding the epoch at which the packs or the identifier key last changed. A
/// workbook read before it was read with another vocabulary, so it is read again; one read after it
/// is left alone however much the list of names has grown since (a table does not depend on it).
const VOCABULARY_EPOCH_META: &str = "vocabulary_epoch";

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
    /// The Data Folder side of the pass: workbooks whose knowledge this pass wrote, which of them
    /// failed or held more than is kept, and how many columns were read as something.
    data_touched: BTreeSet<String>,
    data_failed: BTreeSet<String>,
    data_truncated: BTreeSet<String>,
    semantic_columns: usize,
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

/// What building a workbook's delta produced, besides the delta itself.
#[derive(Debug, Clone, PartialEq)]
pub struct BuiltWorkbook {
    pub delta: KnowledgeDelta,
    /// The workbook held more values than are kept.
    pub truncated: bool,
    /// Values the resolver would not place: a deleted name, a surname that is ambiguous.
    pub unplaced: usize,
    /// Columns read as a name, an organisation, an identifier, a place or an item.
    pub semantic_columns: usize,
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
    /// The pack ids the user switched on, whatever the mode: the file profile reads its business types
    /// and header labels with them even when the knowledge base reads nothing.
    profile_packs: Vec<String>,
    /// The user's own names (the settings): a property that holds one of them says nothing about a file.
    own_names: Vec<String>,
    titles: TitleSet,
    encoder: Box<dyn PhoneticEncoder>,
    extractor: Box<dyn EntityExtractor>,
    /// Reads workbooks. `None` when the mode reads nothing: the vocabulary of every shipped pack is
    /// only loaded for a pass that will use it.
    tables: Option<TableEntityExtractor>,
    resolver: Box<dyn EntityResolver>,
    gazetteer: Gazetteer,
    /// The gazetteer's size when the epoch was last settled: growth beyond it means the documents
    /// read before are behind.
    counted_entries: usize,
    /// See [`VOCABULARY_EPOCH_META`].
    vocabulary_epoch: u32,
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
        maintenance::ensure_alias_generation(
            connection,
            &context.titles,
            context.encoder.as_ref(),
        )?;
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
        if packs_changed || key_changed {
            store::set_meta(
                connection,
                VOCABULARY_EPOCH_META,
                &context.gazetteer.epoch().to_string(),
            )?;
        }
        context.vocabulary_epoch = store::meta(connection, VOCABULARY_EPOCH_META)?
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0);
        if known != recorded {
            store::set_meta(connection, GAZETTEER_ENTRIES_META, &known.to_string())?;
        }
        context.counted_entries = known;
        Ok(context)
    }

    /// The same context, told which packs the user switched on. A context that reads nothing (the mode
    /// `off`) is built without them; the file profile still needs them.
    pub fn with_profile_packs(mut self, ids: &[String]) -> Self {
        self.profile_packs = ids.to_vec();
        self
    }

    /// The same context, told the user's own names: the file profile never takes one of them from a
    /// property as the author or the issuer of a file.
    pub fn with_own_names(mut self, names: &[String]) -> Self {
        self.own_names = names.to_vec();
        self
    }

    pub fn own_names(&self) -> &[String] {
        &self.own_names
    }

    /// The pack ids the user switched on (`with_profile_packs` or the ones the context was built with).
    pub fn profile_pack_ids(&self) -> Vec<&str> {
        self.profile_packs.iter().map(String::as_str).collect()
    }

    pub fn locale(&self) -> &str {
        &self.locale
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
        // The type vocabulary of every shipped pack is always read: a module never changes a type.
        let packs = PackSet::load_with_type_vocabulary(locale, active_packs)?;
        let titles = packs.title_set();
        Ok(Self {
            mode,
            kb_version: CURRENT_KB_VERSION,
            locale: locale.to_string(),
            titles,
            packs,
            profile_packs: active_packs.iter().map(|id| id.to_string()).collect(),
            own_names: Vec::new(),
            encoder: encoder_for_locale(locale),
            extractor: Box::new(DeterministicTextExtractor),
            tables: if mode.reads_files() {
                Some(TableEntityExtractor::for_locale(locale)?)
            } else {
                None
            },
            resolver: Box::new(DeterministicResolver),
            gazetteer: Gazetteer::new(),
            counted_entries: 0,
            vocabulary_epoch: 0,
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

    /// The generation of the table extractor, recorded on the mentions it wrote.
    pub(crate) fn tables_version(&self) -> u32 {
        self.tables.as_ref().map_or(0, |tables| tables.version())
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

    /// The counts for the Analyse summary of a documents pass.
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
            semantic_columns: 0,
            workbooks_analysed: 0,
            refreshed_sources: self.tally.refreshed.len(),
            truncated_sources: self.tally.truncated.len(),
            errors: self.tally.failed.len(),
            elapsed_ms: self.spent_micros.load(Ordering::Relaxed) / 1_000,
        }
    }

    /// The counts for the Analyse summary of a Data Folder pass: what the workbooks gave, plus the
    /// documents that were read again because the workbooks taught the base new names.
    pub fn data_summary(&self, connection: &Connection) -> KnowledgeSummary {
        let touched: Vec<String> = self.tally.data_touched.iter().cloned().collect();
        let counts = store::entity_counts_for_sources(
            connection,
            Domain::Data,
            &touched,
            self.tally.entity_id_before,
        )
        .unwrap_or_default();
        let relations =
            store::relation_count_for_sources(connection, Domain::Data, &touched).unwrap_or(0);
        KnowledgeSummary {
            entities_detected: counts.distinct,
            matched_existing: counts.distinct.saturating_sub(counts.created),
            created_new: counts.created,
            candidates: counts.created_candidates,
            identifiers: counts.identifiers,
            relations,
            semantic_columns: self.tally.semantic_columns,
            workbooks_analysed: self.tally.data_touched.len(),
            refreshed_sources: self.tally.refreshed.len(),
            truncated_sources: self.tally.data_truncated.len(),
            errors: self.tally.data_failed.len() + self.tally.failed.len(),
            elapsed_ms: self.spent_micros.load(Ordering::Relaxed) / 1_000,
        }
    }

    /// A workbook of the Data Folder could not be read for its knowledge.
    pub fn note_data_failed(&mut self, relative_path: &str) {
        self.tally.data_failed.insert(relative_path.to_string());
    }

    // ----------------------------------------------------------------------------- one workbook

    /// Write one workbook's inventory the way the Data pass does: always the inventory, and the
    /// knowledge when the mode reads files, the workbook is a usable table and its values have not
    /// been read already. A knowledge failure is applied in a savepoint and never costs the
    /// inventory (the knowledge base never blocks an analysis). The only error is a failure of the
    /// index itself.
    pub fn store_workbook(
        &mut self,
        index: &mut IndexStore,
        inventory: &TabularInventory,
        workbook: &Workbook,
        source_rows: &SourceRows,
    ) -> Result<(), AppError> {
        if !self.mode.reads_files() || !inventory.has_a_usable_sheet() {
            // Nothing to read, or nothing worth reading: what an earlier version of the file taught
            // the base is dropped with it.
            index.put_tabular_inventory_with_knowledge(inventory, None)?;
            return Ok(());
        }
        let source = SourceRef {
            domain: Domain::Data,
            relative_path: inventory.relative_path.clone(),
            content_id: inventory.workbook_id.clone(),
        };
        if self.workbook_is_current(index.connection(), &source) {
            index.put_tabular_inventory(inventory)?;
            return Ok(());
        }

        let input = workbook_input(inventory, workbook, source_rows, &source);
        let built = self.build_workbook(index.connection(), source.clone(), input);
        let delta = match &built {
            Ok(built) => built.delta.clone(),
            Err(_) => {
                self.note_data_failed(&inventory.relative_path);
                // Not extracted: the previous version's knowledge goes with the old content and
                // the source stays due, so the next pass tries again.
                KnowledgeDelta::empty(source)
            }
        };
        let write = index.put_tabular_inventory_with_knowledge(inventory, Some(&delta))?;
        match (write, built) {
            (crate::knowledge::KnowledgeWrite::Applied(_), Ok(built)) => {
                self.tally
                    .data_touched
                    .insert(inventory.relative_path.clone());
                self.tally.semantic_columns += built.semantic_columns;
                if built.truncated {
                    self.tally
                        .data_truncated
                        .insert(inventory.relative_path.clone());
                }
                self.learn(index.connection(), &built.delta);
            }
            (crate::knowledge::KnowledgeWrite::Applied(_), Err(_)) => {}
            (crate::knowledge::KnowledgeWrite::Bypassed, _) => {
                self.note_data_failed(&inventory.relative_path)
            }
        }
        Ok(())
    }

    /// The workbook was read for its knowledge already: same content, an extractor at least as new,
    /// and read after the last change of vocabulary. A change of packs makes a workbook due again
    /// (a column that carried no meaning may carry one now); the list of names growing does not.
    fn workbook_is_current(&self, connection: &Connection, source: &SourceRef) -> bool {
        match store::source_row(connection, source.domain, &source.relative_path) {
            Ok(Some(row)) => {
                row.source.content_id == source.content_id
                    && row.kb_version >= self.kb_version
                    && row.gazetteer_epoch >= self.vocabulary_epoch
            }
            _ => false,
        }
    }

    /// Read a workbook's columns and values and decide what they mean. Pure with respect to the
    /// store: it only looks. A panic in the extractor, or a store that cannot be read, is an error
    /// the caller turns into a bypass.
    pub fn build_workbook(
        &self,
        connection: &Connection,
        source: SourceRef,
        input: WorkbookInput,
    ) -> Result<BuiltWorkbook, AppError> {
        let started = Instant::now();
        let built = self.build_workbook_untimed(connection, source, input);
        self.spend(started);
        built
    }

    fn build_workbook_untimed(
        &self,
        connection: &Connection,
        source: SourceRef,
        input: WorkbookInput,
    ) -> Result<BuiltWorkbook, AppError> {
        let Some(tables) = &self.tables else {
            return Err(AppError::KnowledgeUnavailable);
        };
        let mut delta = KnowledgeDelta::empty(source);
        delta.kb_version = self.kb_version;
        delta.gazetteer_epoch = self.gazetteer.epoch();

        let reading = catch_unwind(AssertUnwindSafe(|| {
            tables.read(
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

        let semantic_columns = reading.stats.semantic_columns;
        delta.column_semantics = reading
            .columns
            .iter()
            .map(|column| ColumnSemanticsDraft {
                sheet: column.sheet.clone(),
                column_name: column.column.clone(),
                semantic_type: column.entity_type.as_str().to_string(),
                subtype: column.subtype.clone(),
                role: column.role.as_ref().map(|role| role.as_str().to_string()),
                confidence: column.confidence.value(),
                reason: column.reason.as_code().to_string(),
            })
            .collect();

        let truncated = reading.knowledge.truncated;
        let mut builder = DeltaBuilder::new(self, connection, delta);
        builder.place_table(&reading)?;
        let (delta, unplaced) = builder.finish();
        Ok(BuiltWorkbook {
            delta,
            truncated,
            unplaced,
            semantic_columns,
        })
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
                || !is_listed_type(&record.entity.type_id)
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

/// A workbook as the table extractor reads it: per sheet, the columns the inventory found, each
/// with its cells as text in data-row order. An empty cell is an empty string so that a value's
/// position is its row; a formula cell is empty too and is counted, because what a formula computed
/// is a claim the file makes, not a name to remember.
///
/// Only the sheets the engine itself would answer from are handed over (`looks_tabular`): a sheet of
/// notes, or one made of formulas, is not a table. The cells of a date column are not copied at all,
/// since a date is never a name.
pub fn workbook_input(
    inventory: &TabularInventory,
    workbook: &Workbook,
    source_rows: &SourceRows,
    source: &SourceRef,
) -> WorkbookInput {
    let mut sheets = Vec::with_capacity(inventory.sheets.len());
    for sheet_inventory in &inventory.sheets {
        let Some(data) = workbook
            .sheets
            .iter()
            .find(|candidate| candidate.name == sheet_inventory.name)
        else {
            continue;
        };
        if !sheet_inventory.looks_tabular() {
            continue;
        }
        let rows = data_rows(sheet_inventory, data);
        let columns = sheet_inventory
            .columns
            .iter()
            .map(|column| {
                let mut formula_cells = 0;
                let values = if column.inferred_type == ColumnType::Date {
                    Vec::new()
                } else {
                    rows.iter()
                        .map(|row| match row.get(column.index) {
                            None | Some(CellValue::Empty) => String::new(),
                            Some(CellValue::Formula { .. }) => {
                                formula_cells += 1;
                                String::new()
                            }
                            Some(other) => text_value(other),
                        })
                        .collect()
                };
                ColumnInput {
                    header: column.name.clone(),
                    values,
                    kind: match column.inferred_type {
                        ColumnType::Numeric => ColumnKind::Numeric,
                        ColumnType::Date => ColumnKind::Date,
                        ColumnType::Categorical => ColumnKind::Text,
                    },
                    formula_cells,
                }
            })
            .collect();
        // The row of the file each data row came from, aligned with the cells above.
        let first_data = sheet_inventory.header_row.map_or(0, |header| header + 1);
        let row_numbers = source_rows
            .get(&sheet_inventory.name)
            .map(|all| all.iter().skip(first_data).copied().collect())
            .unwrap_or_default();
        sheets.push(SheetInput {
            name: sheet_inventory.name.clone(),
            has_header: sheet_inventory.header_row.is_some(),
            row_numbers,
            columns,
        });
    }
    WorkbookInput {
        source: source.clone(),
        sheets,
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
    /// The folder of the source being read.
    domain: Domain,
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

    fn names_in_the_same_folder(&self, entity_id: i64) -> bool {
        // An entity this very document just drafted is in the same folder by definition.
        entity_id < 0
            || store::entity_has_mention_in_domain(self.base.connection(), entity_id, self.domain)
                .unwrap_or(true)
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

/// The name of an organisation without its legal form (`Dupont Immobilier SA` gives `Dupont
/// Immobilier`), by the legal forms the packs list. `None` when it has none or nothing else.
fn without_legal_suffix(name: &str, context: &KnowledgeContext) -> Option<String> {
    let words: Vec<&str> = name.split_whitespace().collect();
    let folded: Vec<String> = words.iter().map(|word| fold_word(word)).collect();
    for marker in &context.packs.org_markers().suffix {
        if marker.is_empty() || words.len() <= marker.len() {
            continue;
        }
        let cut = words.len() - marker.len();
        if folded[cut..] == marker[..] {
            return Some(words[..cut].join(" "));
        }
    }
    None
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
                domain: delta.source.domain,
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

        self.place_identifiers(&identifiers, Method::Identifier);

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
                normalize_name(&self.read_as_written(candidate), &self.context.titles).joined
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

    /// The name of a candidate as the resolver reads it: a person written surname first in capitals
    /// (`DUPONT Jean`) is read given name first, so the two spellings are one name.
    fn read_as_written(&self, candidate: &Candidate) -> String {
        if candidate.type_id.as_str() == EntityTypeId::PERSON {
            given_name_first(&candidate.surface)
        } else {
            candidate.surface.clone()
        }
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
                if !through_short_form && !group.titles_seen.is_empty() {
                    self.add_title_forms(lead, target, &group.titles_seen);
                }
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
                        None,
                    );
                }
            }
        }
    }

    /// A person already in the base, found again with a title (`Dr Jean Dupont`): the title and the
    /// surname (`Dr Dupont`) becomes one of the names the base knows her by, as it would have if this
    /// document had been the first to name her. Only for a name written in full: a short form never
    /// teaches the base anything.
    fn add_title_forms(&mut self, lead: &Candidate, target: EntityRef, titles_seen: &[String]) {
        if lead.type_id.as_str() != EntityTypeId::PERSON
            || !matches!(target, EntityRef::Existing(_))
        {
            return;
        }
        let canonical = display_name(&given_name_first(&lead.surface));
        let made = aliases_for(
            &AliasRequest {
                type_id: &lead.type_id,
                canonical: &canonical,
                titles_seen,
                acronym: None,
            },
            &self.context.titles,
            self.context.encoder.as_ref(),
        );
        for alias in made
            .iter()
            .filter(|alias| alias.kind == AliasKind::TitleForm)
        {
            self.delta.aliases.push(AliasDraft {
                entity: target,
                display: alias.display.clone(),
                normalized: alias.normalized.clone(),
                phonetic_key: alias.phonetic_key.clone(),
                kind: alias.kind,
                confidence: alias.confidence,
            });
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
            display_name(&given_name_first(&candidate.surface))
        } else {
            candidate.surface.clone()
        };
        let normalized_name = normalize_name(&canonical, &self.context.titles).joined;
        if normalized_name.is_empty() {
            return None;
        }
        let mut aliases = aliases_for(
            &AliasRequest {
                type_id: &candidate.type_id,
                canonical: &canonical,
                titles_seen,
                acronym: candidate.acronym.as_deref(),
            },
            &self.context.titles,
            self.context.encoder.as_ref(),
        );
        // An organisation known by its legal form (`Dupont Immobilier SA`) is also written without
        // it: the same name in the next letter is this organisation and not a person.
        if candidate.method == Method::OrganizationMarker {
            if let Some(short) = without_legal_suffix(&canonical, self.context) {
                let name = normalize_name(&short, &self.context.titles);
                if !name.joined.is_empty()
                    && !aliases.iter().any(|alias| alias.normalized == name.joined)
                {
                    aliases.push(NewAlias {
                        phonetic_key: alias_phonetic_key(
                            &short,
                            AliasKind::Abbreviation,
                            &self.context.titles,
                            self.context.encoder.as_ref(),
                        ),
                        display: short,
                        normalized: name.joined,
                        kind: AliasKind::Abbreviation,
                        confidence: 0.8,
                    });
                }
            }
        }

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
                None,
            );
        } else {
            self.unplaced += 1;
        }
    }

    /// What a workbook's columns named: the same resolution as for a document, with the column as
    /// the evidence. Identifiers first (they stand on their own), then the names with more than one
    /// word, then the single words, so that a surname alone can be linked to a person this very
    /// workbook names in full. A value the resolver will not place is counted and left out.
    ///
    /// A mention is the unit "this entity, in this column of this sheet": every value of the column
    /// that leads to the entity adds to it, so a person written two ways in one column is one
    /// mention with the rows of both. The method of a table's mention is always the column; unlike
    /// a document's, it is never rewritten by a later round that reads the names it has just taught
    /// the base.
    fn place_table(&mut self, reading: &TableReading) -> Result<(), AppError> {
        let candidates = &reading.knowledge.candidates;
        let mut target_of: Vec<Option<EntityRef>> = vec![None; candidates.len()];

        let identifier_positions: Vec<usize> = (0..candidates.len())
            .filter(|position| candidates[*position].identifier.is_some())
            .collect();
        let identifiers: Vec<Candidate> = identifier_positions
            .iter()
            .map(|position| candidates[*position].clone())
            .collect();
        let placed = self.place_identifiers(&identifiers, Method::ColumnValue);
        for (position, target) in identifier_positions.iter().zip(placed) {
            target_of[*position] = target;
        }

        let words_in = |candidate: &Candidate| {
            normalize_name(&candidate.surface, &self.context.titles)
                .tokens
                .len()
        };
        let mut names: Vec<usize> = (0..candidates.len())
            .filter(|position| candidates[*position].identifier.is_none())
            .collect();
        // Stable: within a length, the order of the columns and rows.
        names.sort_by_key(|position| std::cmp::Reverse(words_in(&candidates[*position]).min(2)));
        for position in names {
            let candidate = &candidates[position];
            match self.place(candidate, &[], true) {
                Placed::To {
                    target,
                    alias,
                    confidence,
                    ..
                } => {
                    target_of[position] = Some(target);
                    self.add_cells_mention(
                        target,
                        alias,
                        candidate,
                        confidence.min(candidate.confidence.value()),
                    );
                }
                Placed::Nowhere => self.unplaced += 1,
            }
        }

        for relation in &reading.knowledge.relations {
            let (Some(Some(subject)), Some(Some(object))) = (
                target_of.get(relation.subject),
                target_of.get(relation.object),
            ) else {
                continue;
            };
            if subject == object {
                continue;
            }
            self.delta.relations.push(RelationDraft {
                subject: *subject,
                predicate: relation.predicate.clone(),
                object: *object,
                occurrence_count: relation.occurrence_count,
                confidence: relation.confidence.value(),
            });
        }

        for attribute in &reading.attributes {
            let Some(Some(person)) = target_of.get(attribute.candidate) else {
                continue;
            };
            let Some(scheme) = self.context.packs.identifier_scheme(&attribute.scheme) else {
                continue;
            };
            if !scheme.personal_key {
                continue;
            }
            self.delta.attributes.push(AttributeDraft {
                entity: *person,
                key: format!("identifier:{}", scheme.id),
                value: self.stored_identifier(&scheme.id, &attribute.normalized, true),
            });
        }

        if self.lookup.failed() {
            // Same rule as for a document: a partial delta would erase what the workbook's earlier
            // version had legitimately recorded.
            return Err(AppError::KnowledgeUnavailable);
        }
        Ok(())
    }

    /// One mention per entity, sheet, column and role. Occurrences add up, the first row is the
    /// smallest, the sample is the first three rows in order, the confidence the highest.
    fn add_cells_mention(
        &mut self,
        target: EntityRef,
        alias: Option<String>,
        candidate: &Candidate,
        confidence: f32,
    ) {
        let MentionLocator::Cells {
            sheet,
            column_name,
            first_row,
            sample_rows,
        } = &candidate.locator
        else {
            return;
        };
        let role = candidate
            .role
            .as_ref()
            .map(|role| role.as_str().to_string());
        let key = format!("{target:?}|{sheet}|{column_name}|{role:?}");
        match self.mention_index.get(&key) {
            Some(&position) => {
                let known = &mut self.delta.mentions[position];
                known.occurrence_count += candidate.occurrence_count;
                if confidence > known.confidence {
                    known.confidence = confidence;
                }
                if let MentionLocator::Cells {
                    first_row: known_first,
                    sample_rows: known_rows,
                    ..
                } = &mut known.locator
                {
                    *known_first = match (*known_first, *first_row) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        (a, b) => a.or(b),
                    };
                    known_rows.extend(sample_rows.iter().copied());
                    known_rows.sort_unstable();
                    known_rows.dedup();
                    known_rows.truncate(3);
                }
            }
            None => {
                self.mention_index.insert(key, self.delta.mentions.len());
                self.delta.mentions.push(MentionDraft {
                    entity: target,
                    alias_normalized: alias,
                    role,
                    locator: candidate.locator.clone(),
                    occurrence_count: candidate.occurrence_count,
                    confidence,
                    method: Method::ColumnValue.as_code().to_string(),
                    extractor_version: self.context.tables_version(),
                });
            }
        }
    }

    /// An identifier is the entity: the same reduced value is the same entity, wherever it is
    /// written. A personal one (an e-mail address, a social security number, an IBAN) is stored as
    /// a keyed hash, and its name is a neutral label that says what kind of identifier it is and
    /// nothing of its value.
    ///
    /// Returns, for each candidate in order, the entity it was placed on (`None` when it was not
    /// placed). `method` is what the mentions are recorded as: the pattern for a document, the
    /// column for a table.
    fn place_identifiers(
        &mut self,
        identifiers: &[Candidate],
        method: Method,
    ) -> Vec<Option<EntityRef>> {
        let mut placed: Vec<Option<EntityRef>> = Vec::with_capacity(identifiers.len());
        let mut temp_of: HashMap<String, EntityRef> = HashMap::new();
        for candidate in identifiers {
            let Some(found) = &candidate.identifier else {
                placed.push(None);
                continue;
            };
            let Some(scheme) = self.context.packs.identifier_scheme(&found.scheme) else {
                self.unplaced += 1;
                placed.push(None);
                continue;
            };
            let personal = scheme.personal_key;
            let stored_value = self.stored_identifier(&found.scheme, &found.normalized, personal);
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
                        placed.push(None);
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
                method,
                None,
            );
            placed.push(Some(target));
        }
        placed
    }

    /// What is stored for an identifier: the reduced value, or for a personal one (an e-mail address,
    /// a social security number, an IBAN) the keyed hash of it. A personal identifier is never
    /// written as itself.
    fn stored_identifier(&self, scheme_id: &str, normalized: &str, personal: bool) -> String {
        if personal {
            self.context.key.digest(scheme_id, normalized)
        } else {
            normalized.to_string()
        }
    }

    /// One mention per entity and place: two finds of the same entity in the same chunk are one
    /// mention counted twice.
    #[allow(clippy::too_many_arguments)]
    fn add_mention(
        &mut self,
        target: EntityRef,
        alias: Option<String>,
        locator: MentionLocator,
        occurrences: u32,
        confidence: f32,
        method: Method,
        role: Option<String>,
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
                    role,
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
    fn a_workbook_is_handed_over_column_by_column_with_formulas_blanked_and_counted() {
        use crate::tabular::inventory::{TabularFormat, TabularInventory};
        use crate::tabular::SheetData;

        let text = |value: &str| CellValue::Text(value.to_string());
        // Twenty-five data rows: one formula among them is under the 5 % a usable sheet may hold.
        let mut rows = vec![
            vec![text("client"), text("amount"), text("note")],
            vec![
                text("Alice Archer"),
                CellValue::Number(12.5),
                CellValue::Formula {
                    expression: "A1&B1".to_string(),
                    cached_value: Some(Box::new(text("computed"))),
                },
            ],
            vec![CellValue::Empty, CellValue::Number(30.0), text("plain")],
        ];
        for _ in 0..23 {
            rows.push(vec![
                text("Bruno Baker"),
                CellValue::Number(1.0),
                text("plain"),
            ]);
        }
        let workbook = Workbook {
            sheets: vec![SheetData {
                name: "Sheet".to_string(),
                rows,
            }],
        };
        let inventory =
            TabularInventory::build("a.csv", "id", TabularFormat::Csv, &workbook, "fr-FR");
        let source = SourceRef {
            domain: Domain::Data,
            relative_path: "a.csv".to_string(),
            content_id: "id".to_string(),
        };

        let input = workbook_input(&inventory, &workbook, &SourceRows::new(), &source);

        let sheet = &input.sheets[0];
        assert!(sheet.has_header);
        let [client, amount, note] = &sheet.columns[..] else {
            panic!("three columns expected");
        };
        assert_eq!(client.values.len(), 25);
        assert_eq!(
            client.values[..2],
            ["Alice Archer", ""],
            "an empty cell is an empty string"
        );
        assert_eq!(amount.kind, ColumnKind::Numeric);
        assert_eq!(amount.values[..2], ["12.5", "30"]);
        assert_eq!(
            note.values[..2],
            ["", "plain"],
            "a formula's result is never read"
        );
        assert_eq!(note.formula_cells, 1);
    }

    #[test]
    fn a_chunk_read_by_a_machine_with_no_score_is_read_as_badly_read() {
        assert_eq!(ocr_confidence(PageOrigin::TextLayer, None), None);
        assert_eq!(ocr_confidence(PageOrigin::TextLayer, Some(0.2)), None);
        assert_eq!(ocr_confidence(PageOrigin::Ocr, Some(0.9)), Some(0.9));
        assert_eq!(ocr_confidence(PageOrigin::Ocr, None), Some(0.0));
    }
}
