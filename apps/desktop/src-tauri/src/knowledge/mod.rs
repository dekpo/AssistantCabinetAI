//! The Knowledge Base programme's home in the client.
//!
//! `diagnostics` measures the question path and the Analyse path. `store` is the persistence
//! layer: the `kb_*` tables in the same SQLite file as the index, and the lifecycle that keeps
//! them in step with the documents they were learned from. The extractors, the resolver and the
//! scope reducer arrive in the lots that follow, each beside this file.
//!
//! Nothing here is user-facing prose: types, roles and codes are local metadata, and a sentence
//! is an interface catalogue entry (`docs/SESSION-KB-00-master.md`, I9 and I10).

pub mod backfill;
pub mod diagnostics;
pub mod extract;
pub mod gazetteer;
pub mod ingest;
pub mod maintenance;
pub mod normalize;
pub mod packs;
pub mod phonetic;
pub mod resolve;
pub mod secret;
pub mod store;

use serde::{Deserialize, Serialize};

/// Whether the knowledge base reads the files at all (`docs/SESSION-KB-00-master.md`, D4). Stored in
/// `settings.json`. `Off` is the kill switch: no extraction, no lookup, no `kb_*` row written, the
/// product as it was before the knowledge base. `Suggest` is the default: extraction on, and (from
/// lot 6) suggestions and the entity picker, but never an automatic reduction of the user's
/// selection. `Auto` also reduces automatically when the rules allow; it becomes a default only
/// after the release gate of lot 11.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KnowledgeMode {
    Off,
    #[default]
    Suggest,
    Auto,
}

impl KnowledgeMode {
    /// Whether the files are read for names. The one question lot 4 asks of the mode.
    pub fn reads_files(self) -> bool {
        self != Self::Off
    }
}

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
    /// An alias that several people can carry: a surname alone, or a title and a surname. The
    /// resolver never reads one of them as "this person" without checking how many people in the
    /// selection (or in the source) answer to it.
    pub fn is_ambiguous_by_nature(self) -> bool {
        matches!(self, Self::PartialSurname | Self::TitleForm)
    }

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

/// Two entities the source makes one wonder about but nothing proves the same (a spelling that
/// sounds alike, the same words in another order). Never a merge: a row in `kb_possible_matches`
/// for the user to decide on later.
#[derive(Debug, Clone, PartialEq)]
pub struct PossibleMatchDraft {
    pub subject: EntityRef,
    pub other: EntityRef,
    /// `resolve::PossibleReason::as_code`.
    pub reason: String,
    pub score: f32,
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
    pub possible_matches: Vec<PossibleMatchDraft>,
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
            possible_matches: Vec::new(),
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
    /// Candidates that became active because a second source named them.
    pub entities_promoted: usize,
}

/// What became of the knowledge half of a document's write. The chunks are committed either way
/// (the knowledge base never blocks indexing, `docs/DECISIONS.md`, 9 October 2026).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeWrite {
    Applied(ApplyOutcome),
    /// The knowledge step failed and was rolled back alone. The source is left due, so a later
    /// pass reads it again.
    Bypassed,
}

/// What an Analyse pass learned about names, as counts only: no name, no file name. Localised by
/// the interface (`analysis.knowledge.*`); Rust builds no sentence from it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeSummary {
    /// Distinct people and organisations (and places, items, terms) named in the files read by
    /// this pass. Identifiers are in `identifiers`.
    pub entities_detected: usize,
    /// Of those, how many the knowledge base already held before the pass.
    pub matched_existing: usize,
    pub created_new: usize,
    /// Of the new ones, how many are only guesses (candidates) until a second file names them.
    pub candidates: usize,
    /// Distinct identifiers (an e-mail address, an invoice number, an IBAN) found. Counted, never
    /// shown: a personal one is stored as a hash.
    pub identifiers: usize,
    /// Links stated by a file's own structure. Documents state none; tables do (lot 5).
    pub relations: usize,
    /// Documents analysed earlier that were read again from their stored text, with no request to
    /// the AI.
    pub refreshed_sources: usize,
    /// Files that held more names than are kept.
    pub truncated_sources: usize,
    /// Files whose names could not all be recorded. The file itself is indexed; analysing again
    /// retries the names.
    pub errors: usize,
    pub elapsed_ms: u64,
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

// ---------------------------------------------------------------------------------------------
// Vocabulary shared by the ports (extractor, resolver) and the lexicon packs
// ---------------------------------------------------------------------------------------------

/// A machine id of the shape `[a-z][a-z0-9_]*`: what a type, a role, a subtype or a predicate is
/// called in packs, in the store and in code. Never shown to the user (the interface resolves a
/// label key).
fn is_machine_id(code: &str) -> bool {
    let mut characters = code.chars();
    characters.next().is_some_and(|c| c.is_ascii_lowercase())
        && characters.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The type of an entity. The six built-in ones are fixed by the master plan (D1) and seeded in
/// `kb_entity_types`; a role is an attribute, never a type. A type is an id, not an enum, so a
/// future domain does not need a code change in the store.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityTypeId(String);

impl EntityTypeId {
    pub const PERSON: &'static str = "person";
    pub const ORGANIZATION: &'static str = "organization";
    pub const LOCATION: &'static str = "location";
    pub const IDENTIFIER: &'static str = "identifier";
    pub const ITEM: &'static str = "item";
    pub const TERM: &'static str = "term";

    /// The built-in types, in the order the store seeds them.
    pub const BUILTIN: [&'static str; 6] = [
        Self::PERSON,
        Self::ORGANIZATION,
        Self::LOCATION,
        Self::IDENTIFIER,
        Self::ITEM,
        Self::TERM,
    ];

    /// `None` when `code` is not a machine id.
    pub fn new(code: &str) -> Option<Self> {
        is_machine_id(code).then(|| Self(code.to_string()))
    }

    pub fn person() -> Self {
        Self(Self::PERSON.to_string())
    }

    pub fn organization() -> Self {
        Self(Self::ORGANIZATION.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_builtin(&self) -> bool {
        Self::BUILTIN.contains(&self.0.as_str())
    }
}

/// The part an entity plays in a file (`client`, `provider`, `counterparty`, `sender`,
/// `recipient`, `author`, `supplier`, ...). An open vocabulary of neutral ids (D2): the label a
/// profession shows for one comes from its lexicon pack, never from a Rust enum.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoleId(String);

impl RoleId {
    /// `None` when `code` is not a machine id.
    pub fn new(code: &str) -> Option<Self> {
        is_machine_id(code).then(|| Self(code.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How sure a piece of knowledge is, between 0 and 1. Clamped on the way in, so a stored or
/// computed value can never fall outside the range the thresholds are written for; `NaN` is 0.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Confidence(f32);

impl Confidence {
    pub const NONE: Self = Self(0.0);
    pub const CERTAIN: Self = Self(1.0);

    pub fn new(value: f32) -> Self {
        if value.is_nan() {
            Self(0.0)
        } else {
            Self(value.clamp(0.0, 1.0))
        }
    }

    pub fn value(self) -> f32 {
        self.0
    }

    /// The lower of two.
    pub fn min(self, other: Self) -> Self {
        if other.0 < self.0 {
            other
        } else {
            self
        }
    }
}

/// How an extractor found a mention. The code is what `kb_mentions.method` stores, so it never
/// changes once released; the confidence is the initial value of `docs/SESSION-KB-00-master.md`
/// section 7, tuned in lot 11 and kept in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    /// A pattern of the pack's identifier schemes (an e-mail address, an invoice number).
    Identifier,
    /// A cell of a column the semantic layer classified, matched exactly.
    ColumnValue,
    /// A name from the in-memory list of names the knowledge base already holds.
    GazetteerHit,
    /// A title followed by a full name ("Dr Jean Dupont").
    TitleFullName,
    /// A title followed by a surname alone ("Dr Dupont").
    TitleSurname,
    /// A name with a legal-form suffix ("Dupont SARL"): backed by the suffix, so trusted.
    OrganizationMarker,
    /// A name after a word that usually opens an organisation's name ("Association Dupont"). Such a
    /// word is also an ordinary one, so the entity stays a candidate until a second source agrees.
    OrganizationPrefix,
    /// A surname, an initial and a surname, or a title and a surname, that the same file also
    /// names in full: it points at that person, never at a new one.
    CorefInDocument,
    /// A word of a sanitised file name that reads like a name.
    FilenameToken,
    /// A capitalised word run with no support at all.
    CapitalisedName,
    /// Typed by the user.
    Manual,
}

impl Method {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Identifier => "identifier",
            Self::ColumnValue => "column_value",
            Self::GazetteerHit => "gazetteer_hit",
            Self::TitleFullName => "title_full_name",
            Self::TitleSurname => "title_surname",
            Self::OrganizationMarker => "organization_marker",
            Self::OrganizationPrefix => "organization_prefix",
            Self::CorefInDocument => "coref_in_document",
            Self::FilenameToken => "filename_token",
            Self::CapitalisedName => "capitalised_name",
            Self::Manual => "manual",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "identifier" => Some(Self::Identifier),
            "column_value" => Some(Self::ColumnValue),
            "gazetteer_hit" => Some(Self::GazetteerHit),
            "title_full_name" => Some(Self::TitleFullName),
            "title_surname" => Some(Self::TitleSurname),
            "organization_marker" => Some(Self::OrganizationMarker),
            "organization_prefix" => Some(Self::OrganizationPrefix),
            "coref_in_document" => Some(Self::CorefInDocument),
            "filename_token" => Some(Self::FilenameToken),
            "capitalised_name" => Some(Self::CapitalisedName),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }

    /// Identifier 1.00, column value 0.95, gazetteer hit 0.90, organisation marker 0.80, title and
    /// surname 0.70, filename token 0.50, untitled capitalised name 0.40 (master section 7). A
    /// title with a full name sits between the gazetteer hit and the organisation marker; an
    /// organisation prefix and a name found again in the same file share the title and surname's
    /// 0.70 (lot 4 specification, "coref_in_document 0.70").
    pub fn default_confidence(self) -> Confidence {
        Confidence::new(match self {
            Self::Identifier | Self::Manual => 1.0,
            Self::ColumnValue => 0.95,
            Self::GazetteerHit => 0.90,
            Self::TitleFullName => 0.85,
            Self::OrganizationMarker => 0.80,
            Self::TitleSurname | Self::OrganizationPrefix | Self::CorefInDocument => 0.70,
            Self::FilenameToken => 0.50,
            Self::CapitalisedName => 0.40,
        })
    }

    /// Whether a name found this way is backed by something besides its own capital letter, so
    /// that a new entity created from it starts `Active` (master section 6, rule 5). A surname
    /// alone, a file-name word and an untitled capitalised run stay candidates until a second
    /// source or a column confirms them.
    pub fn is_supported(self) -> bool {
        matches!(
            self,
            Self::Identifier
                | Self::ColumnValue
                | Self::GazetteerHit
                | Self::TitleFullName
                | Self::OrganizationMarker
                | Self::Manual
        )
    }
}
