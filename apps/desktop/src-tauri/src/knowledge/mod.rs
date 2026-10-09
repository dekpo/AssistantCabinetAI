//! The Knowledge Base programme's home in the client.
//!
//! `diagnostics` measures the question path and the Analyse path. `store` is the persistence
//! layer: the `kb_*` tables in the same SQLite file as the index, and the lifecycle that keeps
//! them in step with the documents they were learned from. The extractors, the resolver and the
//! scope reducer arrive in the lots that follow, each beside this file.
//!
//! Nothing here is user-facing prose: types, roles and codes are local metadata, and a sentence
//! is an interface catalogue entry (`docs/SESSION-KB-00-master.md`, I9 and I10).

pub mod diagnostics;
pub mod normalize;
pub mod phonetic;
pub mod store;

/// What the extractor of the current build knows how to read. A source whose `kb_version` is lower
/// is read again by the next pass, so improving an extractor means raising this number.
pub const CURRENT_KB_VERSION: u32 = 1;

/// The layout of the `kb_*` tables, stored in `kb_meta`. Raised only by an additive migration.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// The `kb_version` of a source no extractor has read. A document the index analysed before the
/// knowledge base existed, or one analysed while extraction was off, carries this version, so any
/// later extractor sees it as due.
pub const NOT_EXTRACTED: u32 = 0;

/// Which of the two folders a source lives in. The same relative path can exist in both, so every
/// source is keyed by `(domain, relative_path)` and a document row is never joined onto a workbook
/// by path (`docs/DECISIONS.md`, "the two inventories stay separate").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Domain {
    Documents,
    Data,
}

impl Domain {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Documents => "documents",
            Self::Data => "data",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "documents" => Some(Self::Documents),
            "data" => Some(Self::Data),
            _ => None,
        }
    }
}

/// One analysed file as the knowledge base knows it. `content_id` is the file's SHA-256, the same
/// value as `FileRecord::id` and as a workbook's id, so a source follows its content rather than
/// its name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceRef {
    pub domain: Domain,
    pub relative_path: String,
    pub content_id: String,
}

/// Where a row came from. Re-analysis rebuilds the automatic rows of a source and leaves every
/// manual row alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Automatic,
    Manual,
}

impl Origin {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Manual => "manual",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "automatic" => Some(Self::Automatic),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityStatus {
    /// Seen without enough support to be trusted: it can rank, never narrow a scope.
    Candidate,
    Active,
    /// Redirected to another entity (`merged_into`); nothing was moved.
    Merged,
    /// A tombstone: the name is suppressed so the next pass does not recreate it.
    Deleted,
}

impl EntityStatus {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Active => "active",
            Self::Merged => "merged",
            Self::Deleted => "deleted",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "candidate" => Some(Self::Candidate),
            "active" => Some(Self::Active),
            "merged" => Some(Self::Merged),
            "deleted" => Some(Self::Deleted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasKind {
    CanonicalVariant,
    Reordered,
    Initial,
    PartialSurname,
    TitleForm,
    Abbreviation,
    FilenameForm,
    Manual,
}

impl AliasKind {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::CanonicalVariant => "canonical_variant",
            Self::Reordered => "reordered",
            Self::Initial => "initial",
            Self::PartialSurname => "partial_surname",
            Self::TitleForm => "title_form",
            Self::Abbreviation => "abbreviation",
            Self::FilenameForm => "filename_form",
            Self::Manual => "manual",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "canonical_variant" => Some(Self::CanonicalVariant),
            "reordered" => Some(Self::Reordered),
            "initial" => Some(Self::Initial),
            "partial_surname" => Some(Self::PartialSurname),
            "title_form" => Some(Self::TitleForm),
            "abbreviation" => Some(Self::Abbreviation),
            "filename_form" => Some(Self::FilenameForm),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// Where in a source a mention was found. Never the text itself: a locator is a pointer, and the
/// passage is read back from the chunk or the cells when it is needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MentionLocator {
    Chunk {
        chunk_id: String,
    },
    Cells {
        sheet: String,
        column_name: String,
        first_row: Option<u32>,
        /// At most three row numbers are kept; the store drops the rest.
        sample_rows: Vec<u32>,
    },
    Filename,
    Metadata,
}

/// How a draft names an entity: one that already has a row, or one the same delta creates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityRef {
    Existing(i64),
    /// The `temp_id` of an `EntityDraft` in the same delta.
    Draft(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityDraft {
    pub temp_id: u32,
    pub type_id: String,
    pub subtype: Option<String>,
    pub canonical_name: String,
    pub normalized_name: String,
    pub disambiguator: String,
    /// `Candidate` or `Active`; the other two are not something an extractor can ask for.
    pub status: EntityStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AliasDraft {
    pub entity: EntityRef,
    pub display: String,
    pub normalized: String,
    pub phonetic_key: String,
    pub kind: AliasKind,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MentionDraft {
    pub entity: EntityRef,
    /// The alias of that entity (by its normalized form) the mention matched, when it matched one.
    pub alias_normalized: Option<String>,
    pub role: Option<String>,
    pub locator: MentionLocator,
    pub occurrence_count: u32,
    pub confidence: f32,
    /// A stable code naming the extraction method (`knowledge::Method::as_code` once it exists).
    pub method: String,
    pub extractor_version: u32,
}

/// A link stated by the file's own structure (two role columns of one table). Never inferred from
/// two names appearing together.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationDraft {
    pub subject: EntityRef,
    pub predicate: String,
    pub object: EntityRef,
    pub occurrence_count: u32,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeDraft {
    pub entity: EntityRef,
    pub key: String,
    pub value: String,
}

/// How strongly a source points at an activity profile (words and counts only, never text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalDraft {
    pub pack_id: String,
    pub hits: u32,
    pub distinct_terms: u32,
}

/// Everything extraction produced for **one source**, applied in the same transaction as that
/// source's chunks. Applying a delta makes the automatic rows of the source equal to it: rows that
/// are already identical are left untouched, the others are written, and the automatic rows the
/// delta no longer carries are removed. Manual rows are never touched.
#[derive(Debug, Clone, PartialEq)]
pub struct KnowledgeDelta {
    pub source: SourceRef,
    /// The extractor generation that produced this delta. Leave it at `NOT_EXTRACTED` when no
    /// extractor ran; set it to `CURRENT_KB_VERSION` when one ran and found nothing.
    pub kb_version: u32,
    /// The generation of the name list the source was scanned against.
    pub gazetteer_epoch: u32,
    pub entities: Vec<EntityDraft>,
    pub aliases: Vec<AliasDraft>,
    pub mentions: Vec<MentionDraft>,
    pub relations: Vec<RelationDraft>,
    pub attributes: Vec<AttributeDraft>,
    pub signals: Vec<SignalDraft>,
}

impl KnowledgeDelta {
    /// "This source has no entities, and no extractor has read it." Valid, and what the index
    /// writes for a document when it is not given anything better.
    pub fn empty(source: SourceRef) -> Self {
        Self {
            source,
            kb_version: NOT_EXTRACTED,
            gazetteer_epoch: 0,
            entities: Vec::new(),
            aliases: Vec::new(),
            mentions: Vec::new(),
            relations: Vec::new(),
            attributes: Vec::new(),
            signals: Vec::new(),
        }
    }
}

/// What applying a delta did to the entity table, for the Analyse summary of a later lot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub entities_created: usize,
    /// Drafts that matched an entity the knowledge base already held.
    pub entities_matched: usize,
    /// Drafts dropped because the name was deleted by the user (a tombstone).
    pub entities_suppressed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub entity_id: i64,
    pub type_id: String,
    pub subtype: Option<String>,
    pub canonical_name: String,
    pub normalized_name: String,
    pub disambiguator: String,
    pub status: EntityStatus,
    pub merged_into: Option<i64>,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Alias {
    pub alias_id: i64,
    pub entity_id: i64,
    pub display: String,
    pub normalized: String,
    pub phonetic_key: String,
    pub kind: AliasKind,
    pub origin: Origin,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityRecord {
    pub entity: Entity,
    pub aliases: Vec<Alias>,
}

/// One row of `kb_sources`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRow {
    pub source_id: i64,
    pub source: SourceRef,
    pub kb_version: u32,
    pub gazetteer_epoch: u32,
}

/// A manual or hand-built entity.
#[derive(Debug, Clone, PartialEq)]
pub struct NewEntity {
    pub type_id: String,
    pub subtype: Option<String>,
    pub canonical_name: String,
    pub normalized_name: String,
    pub disambiguator: String,
    pub status: EntityStatus,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewAlias {
    pub display: String,
    pub normalized: String,
    pub phonetic_key: String,
    pub kind: AliasKind,
    pub confidence: f32,
}

/// A hit of the autocomplete prefix search: the alias that matched and the entity it leads to,
/// after following merges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameHit {
    pub entity_id: i64,
    pub alias_id: i64,
    pub display: String,
    pub normalized: String,
}

/// The files that mention each entity inside a set of sources, and the union of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileSets {
    pub per_entity: std::collections::BTreeMap<i64, std::collections::BTreeSet<SourceRef>>,
    pub union: std::collections::BTreeSet<SourceRef>,
}

/// Which entities `list_entities` returns. `status: None` means the live ones (candidate and
/// active); merged entities and tombstones are only listed when asked for by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntityFilter {
    pub type_id: Option<String>,
    pub status: Option<EntityStatus>,
    pub origin: Option<Origin>,
    /// A prefix of the normalized name.
    pub name_prefix: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    pub offset: usize,
    pub limit: usize,
}

/// Counts of everything that should be impossible. Zero everywhere means the knowledge base is
/// consistent; the tests assert it after every scenario.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IntegrityReport {
    pub mentions_without_source: usize,
    pub mentions_without_entity: usize,
    pub aliases_without_entity: usize,
    pub relations_without_endpoint: usize,
    /// A merged entity whose target is missing, itself merged (the chain was not flattened), or
    /// an entity that points somewhere without being merged.
    pub broken_merges: usize,
    pub merge_cycles: usize,
    pub duplicate_live_entities: usize,
    pub duplicate_sources: usize,
    pub name_index_rows_without_alias: usize,
    pub aliases_without_name_index_row: usize,
    /// Automatic entities that nothing holds on to: the garbage collector missed them.
    pub orphan_automatic_entities: usize,
}

impl IntegrityReport {
    pub fn is_clean(&self) -> bool {
        *self == Self::default()
    }
}
