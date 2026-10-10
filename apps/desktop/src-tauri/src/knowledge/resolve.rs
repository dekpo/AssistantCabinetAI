//! The resolution port: is this name one the knowledge base already holds?
//!
//! The same code answers two questions. **Ingestion** (an extractor read a name in a file): link
//! the mention to an entity, create one, or leave it alone. **Query** (the user wrote a name in a
//! question): which entity does it mean, inside the files she selected? The rules of
//! `docs/SESSION-KB-00-master.md` section 7 apply to both; the table there says what changes.
//!
//! ```text
//! signals, strongest first
//!   1 exact identifier       2 exact canonical name      3 exact alias
//!   4 strong pattern         5 column + exact cell value (these two describe a NEW entity)
//!   6 safe fuzzy: same sound and one edit on a word of >= FUZZY_MIN_TOKEN_LEN letters,
//!     or the same words in another order        -> only ever a PossibleMatch, never a merge
//!   7 nothing                                   -> a candidate entity (ingestion) / Unresolved (query)
//! ```
//!
//! What the resolver guarantees, each with a test:
//!
//! - **A suppressed name resolves to nothing and creates nothing** (the user deleted it).
//! - **Ambiguity is judged inside the selection first** (I7): two Martins in the store and one in
//!   the files she selected is a resolved Martin; the other one is never mentioned.
//! - **A surname alone, or a title and a surname, is never "this person" without counting.**
//! - **Fuzzy never merges.** `Dupond` for `Dupont`, `Rene` for `Reine`: a question to the user.
//! - **A manual alias beats an automatic one.**
//! - **A guess cannot drive a reduction**: a candidate entity resolves, in query mode, with a
//!   confidence under the reduction threshold.
//!
//! The resolver reads the store through [`EntityLookup`], so it is tested against
//! [`InMemoryLookup`] with no database and runs in production against [`StoreLookup`]. A better
//! resolver (an embedding model, a trained matcher) implements [`EntityResolver`] and replaces
//! [`DeterministicResolver`] without the callers noticing.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use rusqlite::Connection;

use super::normalize::{normalize_name, normalize_name_with, NormalizedName, TitleSet, WeakTitles};
use super::packs::IdentifierStrength;
use super::phonetic::{bounded_damerau_levenshtein, PhoneticEncoder};
use super::{
    store, Alias, AliasKind, Confidence, Entity, EntityStatus, EntityTypeId, Method, NewAlias,
    Origin,
};

// ---------------------------------------------------------------------------------------------
// Named thresholds: policy lives here, in one place, and is tuned by measurement (lot 11)
// ---------------------------------------------------------------------------------------------

/// The shortest word the fuzzy rule (same sound and one edit) will bridge. Below it a single
/// edit changes a third of the word (`Lee`/`Lea`) and the pairs multiply. Measured on the complete
/// Insee, US Census and SSA lists and reviewed by the owner in lot 2 bis: 4 letters keep the real
/// spelling variants (`Marc`/`Mark`, `Cook`/`Cooke`) that 6 would lose, at the price of about one
/// more "did you mean" in a hundred selections. **Master section 7 says 6**; the owner can overrule
/// this one constant (`docs/DECISIONS.md`, "Settled by KB lot 3").
pub const FUZZY_MIN_TOKEN_LEN: usize = 4;

const CONFIDENCE_IDENTIFIER: f32 = 1.00;
const CONFIDENCE_CANONICAL: f32 = 0.95;
const CONFIDENCE_ALIAS: f32 = 0.90;
const CONFIDENCE_MANUAL_ALIAS: f32 = 0.95;
/// A surname alone, unique inside the selection (master section 7). Equal to the reduction
/// threshold on purpose: exactly one candidate in the selection is enough to narrow.
const CONFIDENCE_SURNAME_IN_SELECTION: f32 = 0.80;
/// A surname alone, or an initial and a surname, unique among the entities of the source being
/// read. Lower than in a selection: nobody chose these files.
const CONFIDENCE_UNIQUE_IN_SOURCE: f32 = 0.70;
const CONFIDENCE_AMBIGUOUS: f32 = 0.50;
/// The most a candidate entity can be trusted at in query mode: under the reduction threshold of
/// 0.80, so a guess can rank but never narrow a scope (master section 6, rule 5).
pub const CANDIDATE_CONFIDENCE_CAP: f32 = 0.40;
const SCORE_PHONETIC: f32 = 0.70;
const SCORE_TOKEN_SET: f32 = 0.75;
const SCORE_INITIAL: f32 = 0.60;
const SCORE_SHARED_IDENTIFIER: f32 = 0.70;

// ---------------------------------------------------------------------------------------------
// What the resolver reads
// ---------------------------------------------------------------------------------------------

/// The sources a question is restricted to, as `kb_sources.source_id`s. Built by the caller from
/// `store::source_ids_for_scope`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceSet {
    ids: BTreeSet<i64>,
}

impl SourceSet {
    pub fn new<I: IntoIterator<Item = i64>>(ids: I) -> Self {
        Self {
            ids: ids.into_iter().collect(),
        }
    }

    pub fn contains(&self, source_id: i64) -> bool {
        self.ids.contains(&source_id)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn ids(&self) -> Vec<i64> {
        self.ids.iter().copied().collect()
    }
}

/// An entity a lookup returned, with the alias it answered through when there was one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityHit {
    /// The entity after following merges.
    pub entity_id: i64,
    pub type_id: String,
    pub canonical_name: String,
    pub normalized_name: String,
    pub disambiguator: String,
    pub status: EntityStatus,
    pub origin: Origin,
    pub alias_kind: Option<AliasKind>,
    pub alias_origin: Option<Origin>,
    pub alias_normalized: Option<String>,
}

impl EntityHit {
    pub fn from_entity(entity: &Entity) -> Self {
        Self {
            entity_id: entity.entity_id,
            type_id: entity.type_id.clone(),
            canonical_name: entity.canonical_name.clone(),
            normalized_name: entity.normalized_name.clone(),
            disambiguator: entity.disambiguator.clone(),
            status: entity.status,
            origin: entity.origin,
            alias_kind: None,
            alias_origin: None,
            alias_normalized: None,
        }
    }

    pub fn from_alias(entity: &Entity, alias: &Alias) -> Self {
        Self {
            alias_kind: Some(alias.kind),
            alias_origin: Some(alias.origin),
            alias_normalized: Some(alias.normalized.clone()),
            ..Self::from_entity(entity)
        }
    }

    /// Typed by the user, either the entity itself or the alias that matched.
    pub fn is_manual(&self) -> bool {
        match self.alias_origin {
            Some(origin) => origin == Origin::Manual,
            None => self.origin == Origin::Manual,
        }
    }
}

/// What the resolver reads. Implemented by [`StoreLookup`] for the real knowledge base and by
/// [`InMemoryLookup`] for tests and dry runs. Hits are live entities (candidate or active) after
/// following merges; a tombstone is only visible through `is_tombstoned`.
pub trait EntityLookup {
    fn by_identifier(&self, scheme: &str, normalized: &str) -> Vec<EntityHit>;
    fn by_normalized(&self, normalized: &str) -> Vec<EntityHit>;
    fn by_alias(&self, normalized: &str) -> Vec<EntityHit>;
    fn by_token_set(&self, key: &str) -> Vec<EntityHit>;
    fn by_phonetic(&self, key: &str) -> Vec<EntityHit>;
    fn is_tombstoned(&self, type_id: &str, normalized: &str) -> bool;
    /// Keep the hits that have at least one mention in `selection`.
    fn restrict_to_selection(&self, hits: Vec<EntityHit>, selection: &SourceSet) -> Vec<EntityHit>;

    /// Whether any read failed since the lookup was made. The signature of the reads above cannot
    /// carry an error (a lookup is a question, and "I could not look" must not look like "there is
    /// nothing"), so a lookup that can fail reports it here and the resolver turns it into
    /// `Unresolved(LookupFailed)`: the caller falls back (master invariant I4).
    fn failed(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------------------------
// What the resolver is asked, and what it answers
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveMode {
    /// An extractor read this surface in a file being analysed.
    Ingestion,
    /// The user wrote this surface in a question.
    Query,
}

/// An identifier next to a surface (or the whole surface), already read and reduced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifierRef {
    pub scheme: String,
    pub normalized: String,
    pub strength: IdentifierStrength,
}

/// A name to resolve.
#[derive(Debug, Clone)]
pub struct SurfaceInput {
    /// The name as written.
    pub text: String,
    /// The expected type; `None` when the user's text does not say (a query), in which case any
    /// type answers.
    pub type_id: Option<EntityTypeId>,
    /// How it was found. Decides the status and the confidence of a new entity.
    pub method: Method,
    pub identifier: Option<IdentifierRef>,
    /// What tells two entities of the same name apart (a birth year, a city), when the source says.
    pub disambiguator: Option<String>,
    /// Whether "Me" and "M" are read as titles; the extractor, which sees the sentence, decides.
    pub weak_titles: WeakTitles,
}

impl SurfaceInput {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_string(),
            type_id: None,
            method: Method::CapitalisedName,
            identifier: None,
            disambiguator: None,
            weak_titles: WeakTitles::Auto,
        }
    }

    pub fn of_type(mut self, type_id: EntityTypeId) -> Self {
        self.type_id = Some(type_id);
        self
    }

    pub fn found_by(mut self, method: Method) -> Self {
        self.method = method;
        self
    }

    pub fn with_identifier(mut self, identifier: IdentifierRef) -> Self {
        self.identifier = Some(identifier);
        self
    }

    pub fn with_disambiguator(mut self, disambiguator: &str) -> Self {
        self.disambiguator = Some(disambiguator.to_string());
        self
    }
}

/// Everything the resolver may consult besides the surface.
pub struct ResolveContext<'a> {
    pub mode: ResolveMode,
    pub titles: &'a TitleSet,
    pub encoder: &'a dyn PhoneticEncoder,
    pub lookup: &'a dyn EntityLookup,
    /// Query mode: the files the user selected. `None` means no restriction.
    pub selection: Option<&'a SourceSet>,
    /// Ingestion mode: the entities already known in the source being read (found earlier in the
    /// same file). `None` means the caller cannot say, and the resolver then declines the links
    /// that need it rather than guess.
    pub source_entities: Option<&'a BTreeSet<i64>>,
}

/// Which of the seven signals decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Identifier,
    CanonicalName,
    Alias,
    StrongPattern,
    ColumnValue,
    SafeFuzzy,
    None,
}

impl Signal {
    pub fn rank(self) -> u8 {
        match self {
            Self::Identifier => 1,
            Self::CanonicalName => 2,
            Self::Alias => 3,
            Self::StrongPattern => 4,
            Self::ColumnValue => 5,
            Self::SafeFuzzy => 6,
            Self::None => 7,
        }
    }

    pub fn as_code(self) -> &'static str {
        match self {
            Self::Identifier => "identifier",
            Self::CanonicalName => "canonical_name",
            Self::Alias => "alias",
            Self::StrongPattern => "strong_pattern",
            Self::ColumnValue => "column_value",
            Self::SafeFuzzy => "safe_fuzzy",
            Self::None => "none",
        }
    }
}

/// Through what an existing entity was reached, so the caller can link the mention to the alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Identifier,
    Canonical,
    Alias(AliasKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PossibleReason {
    /// Same sound, one edit.
    Phonetic,
    /// The same words in another order.
    TokenSet,
    /// An initial and a surname that one full name is compatible with.
    Initial,
    /// An identifier two people can share.
    SharedIdentifier,
}

impl PossibleReason {
    /// What `kb_possible_matches.reason` stores.
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Phonetic => "phonetic",
            Self::TokenSet => "token_set",
            Self::Initial => "initial",
            Self::SharedIdentifier => "shared_identifier",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnresolvedReason {
    /// No name and no identifier to resolve.
    Empty,
    /// The user deleted this name; it is not recreated.
    Suppressed,
    /// The knowledge base holds nothing like it (query mode).
    NoMatch,
    /// It is known, but not in the files that were selected.
    NotInSelection,
    /// A surname alone that the source being read cannot settle (ingestion).
    SurnameOnly,
    /// The knowledge base could not be read: fall back (master invariant I4).
    LookupFailed,
}

impl UnresolvedReason {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Suppressed => "suppressed",
            Self::NoMatch => "no_match",
            Self::NotInSelection => "not_in_selection",
            Self::SurnameOnly => "surname_only",
            Self::LookupFailed => "lookup_failed",
        }
    }
}

/// The answer. The five outcomes of master section 7, each carrying what the caller needs to store
/// `method` and `confidence`.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    /// Nothing known matches: create an entity (ingestion only).
    NewEntity {
        /// `Active` when the name is backed by an identifier, a column, a marker; otherwise
        /// `Candidate`.
        status: EntityStatus,
        signal: Signal,
        confidence: Confidence,
    },
    /// This is that entity.
    Existing {
        entity: EntityHit,
        via: Via,
        signal: Signal,
        confidence: Confidence,
    },
    /// Probably one of these, never to be merged on this evidence: a question to the user. In
    /// ingestion `create_separate` says the caller should also create the new entity and record a
    /// `kb_possible_matches` row for each.
    PossibleMatch {
        entities: Vec<EntityHit>,
        reason: PossibleReason,
        score: Confidence,
        create_separate: bool,
    },
    /// Several entities answer equally: ask, never pick.
    Ambiguous {
        entities: Vec<EntityHit>,
        confidence: Confidence,
    },
    Unresolved {
        reason: UnresolvedReason,
    },
}

impl Resolution {
    /// A stable name for logs of counts and for tests: `new_entity`, `existing`, `possible_match`,
    /// `ambiguous`, `unresolved`.
    pub fn outcome_code(&self) -> &'static str {
        match self {
            Self::NewEntity { .. } => "new_entity",
            Self::Existing { .. } => "existing",
            Self::PossibleMatch { .. } => "possible_match",
            Self::Ambiguous { .. } => "ambiguous",
            Self::Unresolved { .. } => "unresolved",
        }
    }

    pub fn confidence(&self) -> Confidence {
        match self {
            Self::NewEntity { confidence, .. }
            | Self::Existing { confidence, .. }
            | Self::Ambiguous { confidence, .. } => *confidence,
            Self::PossibleMatch { score, .. } => *score,
            Self::Unresolved { .. } => Confidence::NONE,
        }
    }

    /// The entities the answer points at, in the order the lookup gave them.
    pub fn entity_ids(&self) -> Vec<i64> {
        match self {
            Self::Existing { entity, .. } => vec![entity.entity_id],
            Self::PossibleMatch { entities, .. } | Self::Ambiguous { entities, .. } => {
                entities.iter().map(|hit| hit.entity_id).collect()
            }
            Self::NewEntity { .. } | Self::Unresolved { .. } => Vec::new(),
        }
    }
}

/// The resolution port.
pub trait EntityResolver: Send + Sync {
    fn resolve(&self, surface: &SurfaceInput, context: &ResolveContext<'_>) -> Resolution;
}

// ---------------------------------------------------------------------------------------------
// The deterministic resolver
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicResolver;

impl EntityResolver for DeterministicResolver {
    fn resolve(&self, surface: &SurfaceInput, context: &ResolveContext<'_>) -> Resolution {
        let run = Run {
            surface,
            context,
            narrowed_away: Cell::new(false),
        };
        let answer = run.resolve();
        if context.lookup.failed() {
            Resolution::Unresolved {
                reason: UnresolvedReason::LookupFailed,
            }
        } else {
            answer
        }
    }
}

/// How an alias that matched is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AliasClass {
    /// The name itself or a full-name variant: "dupont jean", an acronym.
    Named,
    /// An initial and a surname: "j dupont".
    Initial,
    /// A surname alone or a title and a surname: several people can carry it.
    Surname,
}

fn class_of(hit: &EntityHit) -> AliasClass {
    match hit.alias_kind {
        Some(AliasKind::Initial) => AliasClass::Initial,
        Some(kind) if kind.is_ambiguous_by_nature() => AliasClass::Surname,
        _ => AliasClass::Named,
    }
}

struct Run<'a, 'b> {
    surface: &'a SurfaceInput,
    context: &'a ResolveContext<'b>,
    /// Some hit existed and the selection removed every one of them.
    narrowed_away: Cell<bool>,
}

impl Run<'_, '_> {
    fn is_query(&self) -> bool {
        self.context.mode == ResolveMode::Query
    }

    fn lookup(&self) -> &dyn EntityLookup {
        self.context.lookup
    }

    /// Keep what answers to the expected type and disambiguator, one hit per entity, and in query
    /// mode only what is mentioned inside the selection. Everything the resolver decides on has
    /// been through this, so the selection is always applied first (I7).
    fn keep(&self, hits: Vec<EntityHit>) -> Vec<EntityHit> {
        let mut by_entity: BTreeMap<i64, EntityHit> = BTreeMap::new();
        for hit in hits {
            if let Some(wanted) = &self.surface.type_id {
                if hit.type_id != wanted.as_str() {
                    continue;
                }
            }
            if let Some(wanted) = &self.surface.disambiguator {
                if hit.disambiguator != *wanted {
                    continue;
                }
            }
            match by_entity.get(&hit.entity_id) {
                Some(known) if known.is_manual() || !hit.is_manual() => {}
                _ => {
                    by_entity.insert(hit.entity_id, hit);
                }
            }
        }
        let mut hits: Vec<EntityHit> = by_entity.into_values().collect();
        if let (true, Some(selection)) = (self.is_query(), self.context.selection) {
            let before = hits.len();
            hits = self.lookup().restrict_to_selection(hits, selection);
            if before > 0 && hits.is_empty() {
                self.narrowed_away.set(true);
            }
        }
        hits
    }

    fn cap(&self, hit: &EntityHit, confidence: f32) -> Confidence {
        let confidence = Confidence::new(confidence);
        if self.is_query() && hit.status == EntityStatus::Candidate {
            confidence.min(Confidence::new(CANDIDATE_CONFIDENCE_CAP))
        } else {
            confidence
        }
    }

    fn suppressed(&self, normalized: &str) -> bool {
        match &self.surface.type_id {
            Some(type_id) => self.lookup().is_tombstoned(type_id.as_str(), normalized),
            None => EntityTypeId::BUILTIN
                .iter()
                .any(|type_id| self.lookup().is_tombstoned(type_id, normalized)),
        }
    }

    fn nothing(&self) -> Resolution {
        Resolution::Unresolved {
            reason: if self.narrowed_away.get() {
                UnresolvedReason::NotInSelection
            } else {
                UnresolvedReason::NoMatch
            },
        }
    }

    fn resolve(&self) -> Resolution {
        let surface = self.surface;
        let name = normalize_name_with(&surface.text, self.context.titles, surface.weak_titles);
        let has_name = !name.joined.is_empty();
        if !has_name && surface.identifier.is_none() {
            return Resolution::Unresolved {
                reason: UnresolvedReason::Empty,
            };
        }
        if has_name && self.suppressed(&name.joined) {
            return Resolution::Unresolved {
                reason: UnresolvedReason::Suppressed,
            };
        }

        // Signal 1: an identifier.
        if let Some(identifier) = &surface.identifier {
            let hits = self.keep(
                self.lookup()
                    .by_identifier(&identifier.scheme, &identifier.normalized),
            );
            match hits.len() {
                0 => {}
                1 => {
                    let hit = hits.into_iter().next().expect("one hit");
                    return match identifier.strength {
                        IdentifierStrength::Exact => Resolution::Existing {
                            confidence: self.cap(&hit, CONFIDENCE_IDENTIFIER),
                            entity: hit,
                            via: Via::Identifier,
                            signal: Signal::Identifier,
                        },
                        IdentifierStrength::Possible => Resolution::PossibleMatch {
                            entities: vec![hit],
                            reason: PossibleReason::SharedIdentifier,
                            score: Confidence::new(SCORE_SHARED_IDENTIFIER),
                            create_separate: !self.is_query(),
                        },
                    };
                }
                _ => {
                    return Resolution::Ambiguous {
                        entities: hits,
                        confidence: Confidence::new(CONFIDENCE_AMBIGUOUS),
                    };
                }
            }
            if !has_name {
                return self.nothing_found(&name);
            }
        }

        // Signal 2: the exact canonical name.
        let mut keys = vec![name.joined.clone()];
        if !name.without_elision.is_empty() && name.without_elision != name.joined {
            keys.push(name.without_elision.clone());
        }
        let canonical = self.keep(
            keys.iter()
                .flat_map(|key| self.lookup().by_normalized(key))
                .collect(),
        );
        if !canonical.is_empty() {
            return self.decide_exact(canonical, Signal::CanonicalName, Via::Canonical);
        }

        // Signal 3: an exact alias.
        let mut alias_keys = keys.clone();
        if !name.titles.is_empty() {
            alias_keys.push(format!("{} {}", name.titles.join(" "), name.joined));
        }
        let aliases = self.keep(
            alias_keys
                .iter()
                .flat_map(|key| self.lookup().by_alias(key))
                .collect(),
        );
        if !aliases.is_empty() {
            return self.decide_alias(aliases, &name);
        }

        // Signal 6: only a hint, never a link.
        if let Some(possible) = self.fuzzy(&name) {
            return possible;
        }

        self.nothing_found(&name)
    }

    /// Several entities, or one, that answer by name: a clear winner, or a question.
    fn decide_exact(&self, hits: Vec<EntityHit>, signal: Signal, via: Via) -> Resolution {
        let chosen = if hits.len() == 1 {
            hits.first().cloned()
        } else {
            // A name the user typed settles a tie with one the extractor guessed, unless two of
            // them were typed.
            let mut manual = hits.iter().filter(|hit| hit.is_manual());
            match (manual.next(), manual.next()) {
                (Some(only), None) => Some(only.clone()),
                _ => None,
            }
        };
        match chosen {
            Some(hit) => {
                let confidence = match (signal, hit.alias_origin) {
                    (Signal::CanonicalName, _) => CONFIDENCE_CANONICAL,
                    (_, Some(Origin::Manual)) => CONFIDENCE_MANUAL_ALIAS,
                    _ => CONFIDENCE_ALIAS,
                };
                let via = match (via, hit.alias_kind) {
                    (Via::Alias(_), Some(kind)) => Via::Alias(kind),
                    (other, _) => other,
                };
                Resolution::Existing {
                    confidence: self.cap(&hit, confidence),
                    entity: hit,
                    via,
                    signal,
                }
            }
            None => Resolution::Ambiguous {
                entities: hits,
                confidence: Confidence::new(CONFIDENCE_AMBIGUOUS),
            },
        }
    }

    fn decide_alias(&self, hits: Vec<EntityHit>, name: &NormalizedName) -> Resolution {
        let mut named = Vec::new();
        let mut initial = Vec::new();
        let mut surname = Vec::new();
        for hit in hits {
            match class_of(&hit) {
                AliasClass::Named => named.push(hit),
                AliasClass::Initial => initial.push(hit),
                AliasClass::Surname => surname.push(hit),
            }
        }
        if !named.is_empty() {
            return self.decide_exact(named, Signal::Alias, Via::Alias(AliasKind::Manual));
        }
        if surname.is_empty() {
            return self.decide_initial(initial, name);
        }
        surname.extend(initial);
        self.decide_surname(surname)
    }

    /// "J. Dupont": compatible with the full names that carry this initial and surname.
    fn decide_initial(&self, hits: Vec<EntityHit>, name: &NormalizedName) -> Resolution {
        if self.is_query() {
            return if hits.len() == 1 {
                Resolution::PossibleMatch {
                    entities: hits,
                    reason: PossibleReason::Initial,
                    score: Confidence::new(SCORE_INITIAL),
                    create_separate: false,
                }
            } else {
                Resolution::Ambiguous {
                    entities: hits,
                    confidence: Confidence::new(CONFIDENCE_AMBIGUOUS),
                }
            };
        }
        // Ingestion: link only when the surname cannot be anyone else in this source. Without the
        // list of the source's entities that cannot be known, so the name stands apart.
        if let (1, Some(in_source), Some(surname)) =
            (hits.len(), self.context.source_entities, name.tokens.last())
        {
            let hit = &hits[0];
            let others = self
                .lookup()
                .by_alias(surname)
                .into_iter()
                .filter(|other| other.alias_kind == Some(AliasKind::PartialSurname))
                .filter(|other| in_source.contains(&other.entity_id))
                .any(|other| other.entity_id != hit.entity_id);
            if !others {
                return Resolution::Existing {
                    entity: hit.clone(),
                    via: Via::Alias(AliasKind::Initial),
                    signal: Signal::Alias,
                    confidence: Confidence::new(CONFIDENCE_UNIQUE_IN_SOURCE),
                };
            }
        }
        self.standing_apart()
    }

    /// A surname alone, or a title and a surname.
    fn decide_surname(&self, hits: Vec<EntityHit>) -> Resolution {
        if self.is_query() {
            return if hits.len() == 1 {
                let hit = hits.into_iter().next().expect("one hit");
                Resolution::Existing {
                    via: Via::Alias(hit.alias_kind.unwrap_or(AliasKind::PartialSurname)),
                    confidence: self.cap(&hit, CONFIDENCE_SURNAME_IN_SELECTION),
                    entity: hit,
                    signal: Signal::Alias,
                }
            } else {
                Resolution::Ambiguous {
                    entities: hits,
                    confidence: Confidence::new(CONFIDENCE_AMBIGUOUS),
                }
            };
        }
        // Ingestion: never a new alias without support. The mention is linked only to the one
        // entity of this source that answers to the surname.
        if let Some(in_source) = self.context.source_entities {
            let mut local = hits.iter().filter(|hit| in_source.contains(&hit.entity_id));
            if let (Some(only), None) = (local.next(), local.next()) {
                return Resolution::Existing {
                    via: Via::Alias(only.alias_kind.unwrap_or(AliasKind::PartialSurname)),
                    entity: only.clone(),
                    signal: Signal::Alias,
                    confidence: Confidence::new(CONFIDENCE_UNIQUE_IN_SOURCE),
                };
            }
        }
        Resolution::Unresolved {
            reason: UnresolvedReason::SurnameOnly,
        }
    }

    /// Signal 6. `None` when nothing is close enough.
    fn fuzzy(&self, name: &NormalizedName) -> Option<Resolution> {
        let key = self.context.encoder.encode_name(name);
        // The safety test runs on every alias that shares the sound, before the hits are reduced
        // to one per entity: an entity whose first matching alias is not a safe one (an initial and
        // a surname) must not hide the one that is (the surname alone).
        let sounding: Vec<EntityHit> = self
            .lookup()
            .by_phonetic(&key)
            .into_iter()
            .filter(|hit| is_safe_fuzzy(&name.tokens, hit))
            .collect();
        let phonetic = self.keep(sounding);
        let token_set: Vec<EntityHit> = if name.token_set_key.split(' ').count() >= 2 {
            self.keep(self.lookup().by_token_set(&name.token_set_key))
        } else {
            Vec::new()
        };
        if phonetic.is_empty() && token_set.is_empty() {
            return None;
        }
        let (reason, score) = if token_set.is_empty() {
            (PossibleReason::Phonetic, SCORE_PHONETIC)
        } else {
            (PossibleReason::TokenSet, SCORE_TOKEN_SET)
        };
        let mut entities = token_set;
        for hit in phonetic {
            if !entities
                .iter()
                .any(|known| known.entity_id == hit.entity_id)
            {
                entities.push(hit);
            }
        }
        Some(Resolution::PossibleMatch {
            entities,
            reason,
            score: Confidence::new(score),
            create_separate: !self.is_query(),
        })
    }

    /// Signal 7: nothing matched.
    fn nothing_found(&self, name: &NormalizedName) -> Resolution {
        if self.is_query() {
            return self.nothing();
        }
        let method = self.surface.method;
        let single_word_person = name.tokens.iter().filter(|word| !word.is_empty()).count() <= 1
            && self
                .surface
                .type_id
                .as_ref()
                .is_none_or(|type_id| type_id.as_str() == EntityTypeId::PERSON)
            && !matches!(
                method,
                Method::ColumnValue | Method::Identifier | Method::Manual
            );
        let status = if method.is_supported() && !single_word_person {
            EntityStatus::Active
        } else {
            EntityStatus::Candidate
        };
        let signal = match method {
            Method::Identifier => Signal::Identifier,
            Method::ColumnValue => Signal::ColumnValue,
            Method::TitleFullName | Method::OrganizationMarker => Signal::StrongPattern,
            _ => Signal::None,
        };
        Resolution::NewEntity {
            status,
            signal,
            confidence: method.default_confidence(),
        }
    }

    /// An ingestion name that stays a separate candidate because the source cannot settle it.
    fn standing_apart(&self) -> Resolution {
        Resolution::NewEntity {
            status: EntityStatus::Candidate,
            signal: Signal::None,
            confidence: Method::CapitalisedName.default_confidence(),
        }
    }
}

/// Same sound, one edit: the words line up, exactly one differs, by one edit, and it is long
/// enough that one edit does not change a large part of it. Compared against the alias that
/// matched when there was one, the name otherwise.
fn is_safe_fuzzy(tokens: &[String], hit: &EntityHit) -> bool {
    let theirs: Vec<&str> = hit
        .alias_normalized
        .as_deref()
        .unwrap_or(&hit.normalized_name)
        .split(' ')
        .filter(|word| !word.is_empty())
        .collect();
    if theirs.len() != tokens.len() {
        return false;
    }
    let mut differing = tokens
        .iter()
        .zip(&theirs)
        .filter(|(mine, theirs)| mine.as_str() != **theirs);
    match (differing.next(), differing.next()) {
        (Some((mine, theirs)), None) => {
            mine.chars().count() >= FUZZY_MIN_TOKEN_LEN
                && theirs.chars().count() >= FUZZY_MIN_TOKEN_LEN
                && bounded_damerau_levenshtein(mine, theirs, 1) == Some(1)
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------------------------
// Aliases an entity is born with
// ---------------------------------------------------------------------------------------------

/// The phonetic key of an alias, from its display form. An alias built from the words of a name
/// (the reordered form, the initial and the surname, the surname alone, a title and the surname)
/// is read with the ambiguous titles switched off: its first word may be the initial "M", which is
/// a letter there and not "Monsieur". Anything written by a person (the canonical form, an
/// abbreviation, a file-name form, a typed alias) is read the way a name in a text is.
pub fn alias_phonetic_key(
    display: &str,
    kind: AliasKind,
    titles: &TitleSet,
    encoder: &dyn PhoneticEncoder,
) -> String {
    let weak = match kind {
        AliasKind::Reordered
        | AliasKind::Initial
        | AliasKind::PartialSurname
        | AliasKind::TitleForm => WeakTitles::Never,
        AliasKind::CanonicalVariant
        | AliasKind::Abbreviation
        | AliasKind::FilenameForm
        | AliasKind::Manual => WeakTitles::Auto,
    };
    encoder.encode_name(&normalize_name_with(display, titles, weak))
}

/// What `aliases_for` is asked to make aliases of.
#[derive(Debug, Clone)]
pub struct AliasRequest<'a> {
    pub type_id: &'a EntityTypeId,
    /// The name as it will be shown.
    pub canonical: &'a str,
    /// The titles that were actually seen in a source with this person (folded). A title and a
    /// surname becomes an alias only for a title that was seen.
    pub titles_seen: &'a [String],
    /// An acronym that appeared in a source next to an organisation's name. Never invented.
    pub acronym: Option<&'a str>,
}

fn display_of(words: &[&str]) -> String {
    words
        .iter()
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().chain(characters).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The aliases a new entity starts with, canonical first, each with the phonetic key of the active
/// encoder. For a person with a given name and a surname: the name reordered, the initial and the
/// surname (`j dupont`), the surname alone (flagged ambiguous by nature through its kind), and a
/// title and the surname for each title seen. For an organisation: its acronym when one was seen.
/// A duplicate of an earlier alias is dropped, so the unique key of `kb_aliases` is never hit.
pub fn aliases_for(
    request: &AliasRequest<'_>,
    titles: &TitleSet,
    encoder: &dyn PhoneticEncoder,
) -> Vec<NewAlias> {
    let name = normalize_name(request.canonical, titles);
    if name.joined.is_empty() {
        return Vec::new();
    }
    let mut made: Vec<NewAlias> = Vec::new();
    let mut push = |display: String, normalized: String, kind: AliasKind, confidence: f32| {
        if normalized.is_empty() || made.iter().any(|known| known.normalized == normalized) {
            return;
        }
        let phonetic_key = alias_phonetic_key(&display, kind, titles, encoder);
        made.push(NewAlias {
            display,
            normalized,
            phonetic_key,
            kind,
            confidence,
        });
    };

    push(
        request.canonical.to_string(),
        name.joined.clone(),
        AliasKind::CanonicalVariant,
        1.0,
    );
    // "l'Hopital Central" is also "Hopital Central" to anyone who leaves the article out.
    if name.without_elision != name.joined {
        let words: Vec<&str> = name.without_elision.split(' ').collect();
        push(
            display_of(&words),
            name.without_elision.clone(),
            AliasKind::CanonicalVariant,
            1.0,
        );
    }

    if request.type_id.as_str() == EntityTypeId::PERSON {
        let words: Vec<&str> = name.tokens.iter().map(String::as_str).collect();
        // The surname is the last word with the particles in front of it ("de fontaine").
        let mut surname_start = words.len().saturating_sub(1);
        while surname_start > 0 && titles.is_particle(words[surname_start - 1]) {
            surname_start -= 1;
        }
        let (given, surname) = words.split_at(surname_start);
        let first_given_is_a_word = given.first().is_some_and(|word| word.chars().count() > 1);

        if !surname.is_empty() && first_given_is_a_word {
            let reordered: Vec<&str> = surname.iter().chain(given.iter()).copied().collect();
            push(
                display_of(&reordered),
                reordered.join(" "),
                AliasKind::Reordered,
                0.9,
            );
            let initial = given[0].chars().next().expect("a word has a letter");
            let initial_words: Vec<&str> = std::iter::once(&given[0][..initial.len_utf8()])
                .chain(surname.iter().copied())
                .collect();
            push(
                format!("{}. {}", initial.to_uppercase(), display_of(surname)),
                initial_words.join(" "),
                AliasKind::Initial,
                0.7,
            );
            push(
                display_of(surname),
                surname.join(" "),
                AliasKind::PartialSurname,
                0.6,
            );
            if surname.len() > 1 {
                let last = &surname[surname.len() - 1..];
                push(
                    display_of(last),
                    last.join(" "),
                    AliasKind::PartialSurname,
                    0.6,
                );
            }
            for title in request.titles_seen {
                let title = title.trim().to_lowercase();
                if title.is_empty() {
                    continue;
                }
                push(
                    format!("{} {}", display_of(&[title.as_str()]), display_of(surname)),
                    format!("{title} {}", surname.join(" ")),
                    AliasKind::TitleForm,
                    0.7,
                );
            }
        }
    } else if let Some(acronym) = request.acronym {
        let folded = normalize_name(acronym, titles);
        push(
            acronym.to_string(),
            folded.joined,
            AliasKind::Abbreviation,
            0.8,
        );
    }
    made
}

// ---------------------------------------------------------------------------------------------
// The lookup over the real store
// ---------------------------------------------------------------------------------------------

/// [`EntityLookup`] over the knowledge base's tables. A read that fails sets a flag the resolver
/// reads through [`EntityLookup::failed`] and returns nothing for that read.
pub struct StoreLookup<'a> {
    connection: &'a Connection,
    titles: &'a TitleSet,
    failed: Cell<bool>,
}

/// How many alias rows the name index may hand back for one word before the exact comparison.
const WORD_CANDIDATES: usize = 500;

impl<'a> StoreLookup<'a> {
    pub fn new(connection: &'a Connection, titles: &'a TitleSet) -> Self {
        Self {
            connection,
            titles,
            failed: Cell::new(false),
        }
    }

    fn settle<T: Default>(&self, read: Result<T, crate::error::AppError>) -> T {
        read.unwrap_or_else(|_| {
            self.failed.set(true);
            T::default()
        })
    }

    fn from_pairs(pairs: Vec<(Entity, Alias)>) -> Vec<EntityHit> {
        pairs
            .iter()
            .map(|(entity, alias)| EntityHit::from_alias(entity, alias))
            .collect()
    }
}

impl EntityLookup for StoreLookup<'_> {
    fn by_identifier(&self, scheme: &str, normalized: &str) -> Vec<EntityHit> {
        self.settle(store::find_entities_by_identifier(
            self.connection,
            scheme,
            normalized,
        ))
        .iter()
        .map(EntityHit::from_entity)
        .collect()
    }

    fn by_normalized(&self, normalized: &str) -> Vec<EntityHit> {
        self.settle(store::find_entities_by_normalized(
            self.connection,
            normalized,
        ))
        .iter()
        .map(EntityHit::from_entity)
        .collect()
    }

    fn by_alias(&self, normalized: &str) -> Vec<EntityHit> {
        Self::from_pairs(self.settle(store::find_entities_by_alias(self.connection, normalized)))
    }

    fn by_token_set(&self, key: &str) -> Vec<EntityHit> {
        let Some(first) = key.split(' ').next() else {
            return Vec::new();
        };
        let pairs = self.settle(store::find_aliases_with_word(
            self.connection,
            first,
            WORD_CANDIDATES,
        ));
        Self::from_pairs(
            pairs
                .into_iter()
                .filter(|(_, alias)| {
                    normalize_name(&alias.normalized, self.titles).token_set_key == key
                })
                .collect(),
        )
    }

    fn by_phonetic(&self, key: &str) -> Vec<EntityHit> {
        Self::from_pairs(self.settle(store::find_aliases_by_phonetic(self.connection, key)))
    }

    fn is_tombstoned(&self, type_id: &str, normalized: &str) -> bool {
        self.settle(store::is_tombstoned(self.connection, type_id, normalized))
    }

    fn restrict_to_selection(&self, hits: Vec<EntityHit>, selection: &SourceSet) -> Vec<EntityHit> {
        let entity_ids: Vec<i64> = hits.iter().map(|hit| hit.entity_id).collect();
        let files = self.settle(store::files_for_entities(
            self.connection,
            &entity_ids,
            &selection.ids(),
        ));
        hits.into_iter()
            .filter(|hit| {
                files
                    .per_entity
                    .get(&hit.entity_id)
                    .is_some_and(|set| !set.is_empty())
            })
            .collect()
    }

    fn failed(&self) -> bool {
        self.failed.get()
    }
}

// ---------------------------------------------------------------------------------------------
// The lookup in memory: for tests and for dry runs
// ---------------------------------------------------------------------------------------------

struct MemoryAlias {
    normalized: String,
    phonetic_key: String,
    kind: AliasKind,
    origin: Origin,
}

struct MemoryEntity {
    id: i64,
    type_id: String,
    canonical_name: String,
    normalized_name: String,
    disambiguator: String,
    status: EntityStatus,
    origin: Origin,
    aliases: Vec<MemoryAlias>,
    identifiers: Vec<(String, String)>,
    sources: BTreeSet<i64>,
}

/// An [`EntityLookup`] that holds everything in memory, with the semantics of the store's: live
/// entities only, one hit per alias, a tombstone visible only through `is_tombstoned`. Not a
/// production path: it exists so the resolver's rules are tested without a database, and so a
/// suggestion can be dry-run against a small imagined state.
pub struct InMemoryLookup {
    titles: TitleSet,
    encoder: Box<dyn PhoneticEncoder>,
    entities: Vec<MemoryEntity>,
    tombstones: Vec<(String, String)>,
    next_id: i64,
}

impl InMemoryLookup {
    pub fn new(titles: TitleSet, encoder: Box<dyn PhoneticEncoder>) -> Self {
        Self {
            titles,
            encoder,
            entities: Vec::new(),
            tombstones: Vec::new(),
            next_id: 1,
        }
    }

    /// An active, automatic entity with the aliases `aliases_for` makes. Returns its id.
    pub fn add(&mut self, type_id: &str, name: &str) -> i64 {
        self.add_with(type_id, name, EntityStatus::Active, Origin::Automatic, "")
    }

    pub fn add_person(&mut self, name: &str) -> i64 {
        self.add(EntityTypeId::PERSON, name)
    }

    pub fn add_with(
        &mut self,
        type_id: &str,
        name: &str,
        status: EntityStatus,
        origin: Origin,
        disambiguator: &str,
    ) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        let type_ref = EntityTypeId::new(type_id).expect("a machine id");
        let aliases = aliases_for(
            &AliasRequest {
                type_id: &type_ref,
                canonical: name,
                titles_seen: &[],
                acronym: None,
            },
            &self.titles,
            self.encoder.as_ref(),
        )
        .into_iter()
        .map(|alias| MemoryAlias {
            normalized: alias.normalized,
            phonetic_key: alias.phonetic_key,
            kind: alias.kind,
            origin,
        })
        .collect();
        self.entities.push(MemoryEntity {
            id,
            type_id: type_id.to_string(),
            canonical_name: name.to_string(),
            normalized_name: normalize_name(name, &self.titles).joined,
            disambiguator: disambiguator.to_string(),
            status,
            origin,
            aliases,
            identifiers: Vec::new(),
            sources: BTreeSet::new(),
        });
        id
    }

    fn entity_mut(&mut self, id: i64) -> &mut MemoryEntity {
        self.entities
            .iter_mut()
            .find(|entity| entity.id == id)
            .expect("a known entity")
    }

    pub fn add_alias(&mut self, id: i64, display: &str, kind: AliasKind, origin: Origin) {
        let name = normalize_name(display, &self.titles);
        let key = alias_phonetic_key(display, kind, &self.titles, self.encoder.as_ref());
        self.entity_mut(id).aliases.push(MemoryAlias {
            normalized: name.joined,
            phonetic_key: key,
            kind,
            origin,
        });
    }

    pub fn add_identifier(&mut self, id: i64, scheme: &str, normalized: &str) {
        self.entity_mut(id)
            .identifiers
            .push((scheme.to_string(), normalized.to_string()));
    }

    /// Record that the entity is mentioned in source `source_id`.
    pub fn mention(&mut self, id: i64, source_id: i64) {
        self.entity_mut(id).sources.insert(source_id);
    }

    /// The user deleted this name.
    pub fn tombstone(&mut self, type_id: &str, name: &str) {
        let normalized = normalize_name(name, &self.titles).joined;
        self.tombstones.push((type_id.to_string(), normalized));
    }

    fn hit(entity: &MemoryEntity, alias: Option<&MemoryAlias>) -> EntityHit {
        EntityHit {
            entity_id: entity.id,
            type_id: entity.type_id.clone(),
            canonical_name: entity.canonical_name.clone(),
            normalized_name: entity.normalized_name.clone(),
            disambiguator: entity.disambiguator.clone(),
            status: entity.status,
            origin: entity.origin,
            alias_kind: alias.map(|alias| alias.kind),
            alias_origin: alias.map(|alias| alias.origin),
            alias_normalized: alias.map(|alias| alias.normalized.clone()),
        }
    }

    fn live(&self) -> impl Iterator<Item = &MemoryEntity> {
        self.entities.iter().filter(|entity| {
            matches!(
                entity.status,
                EntityStatus::Candidate | EntityStatus::Active
            )
        })
    }

    fn aliases_where<F: Fn(&MemoryAlias) -> bool>(&self, matches: F) -> Vec<EntityHit> {
        self.live()
            .flat_map(|entity| {
                entity
                    .aliases
                    .iter()
                    .filter(|alias| matches(alias))
                    .map(move |alias| Self::hit(entity, Some(alias)))
            })
            .collect()
    }
}

impl EntityLookup for InMemoryLookup {
    fn by_identifier(&self, scheme: &str, normalized: &str) -> Vec<EntityHit> {
        self.live()
            .filter(|entity| {
                entity
                    .identifiers
                    .iter()
                    .any(|(known_scheme, value)| known_scheme == scheme && value == normalized)
            })
            .map(|entity| Self::hit(entity, None))
            .collect()
    }

    fn by_normalized(&self, normalized: &str) -> Vec<EntityHit> {
        self.live()
            .filter(|entity| entity.normalized_name == normalized)
            .map(|entity| Self::hit(entity, None))
            .collect()
    }

    fn by_alias(&self, normalized: &str) -> Vec<EntityHit> {
        self.aliases_where(|alias| alias.normalized == normalized)
    }

    fn by_token_set(&self, key: &str) -> Vec<EntityHit> {
        self.aliases_where(|alias| {
            normalize_name(&alias.normalized, &self.titles).token_set_key == key
        })
    }

    fn by_phonetic(&self, key: &str) -> Vec<EntityHit> {
        if key.is_empty() {
            return Vec::new();
        }
        self.aliases_where(|alias| alias.phonetic_key == key)
    }

    fn is_tombstoned(&self, type_id: &str, normalized: &str) -> bool {
        self.tombstones
            .iter()
            .any(|(kind, name)| kind == type_id && name == normalized)
    }

    fn restrict_to_selection(&self, hits: Vec<EntityHit>, selection: &SourceSet) -> Vec<EntityHit> {
        hits.into_iter()
            .filter(|hit| {
                self.entities
                    .iter()
                    .find(|entity| entity.id == hit.entity_id)
                    .is_some_and(|entity| entity.sources.iter().any(|id| selection.contains(*id)))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::phonetic::FrenchPhonetic;

    fn titles() -> TitleSet {
        TitleSet::new()
            .with_titles(["dr", "mr"])
            .with_particles(["de", "van", "von", "du"])
    }

    fn lookup() -> InMemoryLookup {
        InMemoryLookup::new(titles(), Box::new(FrenchPhonetic))
    }

    fn resolve_in(
        lookup: &InMemoryLookup,
        mode: ResolveMode,
        text: &str,
        selection: Option<&SourceSet>,
        source_entities: Option<&BTreeSet<i64>>,
    ) -> Resolution {
        let set = titles();
        let encoder = FrenchPhonetic;
        DeterministicResolver.resolve(
            &SurfaceInput::new(text),
            &ResolveContext {
                mode,
                titles: &set,
                encoder: &encoder,
                lookup,
                selection,
                source_entities,
            },
        )
    }

    fn kinds(aliases: &[NewAlias]) -> Vec<(&str, AliasKind)> {
        aliases
            .iter()
            .map(|alias| (alias.normalized.as_str(), alias.kind))
            .collect()
    }

    fn aliases(name: &str, seen: &[&str]) -> Vec<NewAlias> {
        let seen: Vec<String> = seen.iter().map(|title| title.to_string()).collect();
        aliases_for(
            &AliasRequest {
                type_id: &EntityTypeId::person(),
                canonical: name,
                titles_seen: &seen,
                acronym: None,
            },
            &titles(),
            &FrenchPhonetic,
        )
    }

    #[test]
    fn a_person_gets_the_aliases_the_spec_lists() {
        let made = aliases("Jean Dupont", &[]);
        assert_eq!(
            kinds(&made),
            [
                ("jean dupont", AliasKind::CanonicalVariant),
                ("dupont jean", AliasKind::Reordered),
                ("j dupont", AliasKind::Initial),
                ("dupont", AliasKind::PartialSurname),
            ]
        );
        assert!(made.iter().all(|alias| !alias.phonetic_key.is_empty()));
    }

    #[test]
    fn an_initial_m_is_a_letter_in_an_alias_and_not_a_title() {
        // "M" is also "Monsieur". In "M. Dupont" built from "Marie Dupont" it is her initial, so
        // the alias must not sound like the surname alone.
        let with_weak_titles = TitleSet::new().with_weak_titles(["me", "m"]);
        let made = aliases_for(
            &AliasRequest {
                type_id: &EntityTypeId::person(),
                canonical: "Marie Dupont",
                titles_seen: &[],
                acronym: None,
            },
            &with_weak_titles,
            &FrenchPhonetic,
        );
        let key_of = |kind: AliasKind| {
            made.iter()
                .find(|alias| alias.kind == kind)
                .map(|alias| alias.phonetic_key.clone())
                .expect("the alias exists")
        };
        assert_ne!(
            key_of(AliasKind::Initial),
            key_of(AliasKind::PartialSurname)
        );
    }

    #[test]
    fn a_title_becomes_an_alias_only_when_it_was_seen() {
        assert!(!aliases("Jean Dupont", &[])
            .iter()
            .any(|alias| alias.kind == AliasKind::TitleForm));
        let made = aliases("Jean Dupont", &["dr"]);
        assert!(kinds(&made).contains(&("dr dupont", AliasKind::TitleForm)));
    }

    #[test]
    fn particles_stay_with_the_surname() {
        let made = aliases("Jean de Fontaine", &[]);
        let normalized: Vec<&str> = made.iter().map(|alias| alias.normalized.as_str()).collect();
        assert!(normalized.contains(&"de fontaine jean"));
        assert!(normalized.contains(&"j de fontaine"));
        assert!(normalized.contains(&"de fontaine"));
        assert!(normalized.contains(&"fontaine"));
    }

    #[test]
    fn a_name_with_an_elided_article_is_also_known_without_it() {
        let made = aliases_for(
            &AliasRequest {
                type_id: &EntityTypeId::new("organization").unwrap(),
                canonical: "l'Central Clinic",
                titles_seen: &[],
                acronym: None,
            },
            &titles(),
            &FrenchPhonetic,
        );
        assert_eq!(
            kinds(&made),
            [
                ("l central clinic", AliasKind::CanonicalVariant),
                ("central clinic", AliasKind::CanonicalVariant),
            ]
        );
    }

    #[test]
    fn one_word_and_initial_names_get_only_their_own_alias() {
        assert_eq!(aliases("Dupont", &[]).len(), 1);
        assert_eq!(aliases("J. Dupont", &[]).len(), 1);
        assert!(aliases("Dr", &[]).is_empty());
    }

    #[test]
    fn an_organisation_gets_an_acronym_only_when_one_was_seen() {
        let plain = aliases_for(
            &AliasRequest {
                type_id: &EntityTypeId::new("organization").unwrap(),
                canonical: "Acme Holdings",
                titles_seen: &[],
                acronym: None,
            },
            &titles(),
            &FrenchPhonetic,
        );
        assert_eq!(plain.len(), 1);
        let seen = aliases_for(
            &AliasRequest {
                type_id: &EntityTypeId::new("organization").unwrap(),
                canonical: "Acme Holdings",
                titles_seen: &[],
                acronym: Some("AH"),
            },
            &titles(),
            &FrenchPhonetic,
        );
        assert_eq!(kinds(&seen)[1], ("ah", AliasKind::Abbreviation));
    }

    #[test]
    fn generated_aliases_never_collide_on_the_unique_key() {
        for name in [
            "Jean Dupont",
            "J. Dupont",
            "Dupont",
            "Anne Marie de Fontaine",
        ] {
            let made = aliases(name, &["dr", "dr"]);
            let mut seen = BTreeSet::new();
            assert!(
                made.iter()
                    .all(|alias| seen.insert(alias.normalized.clone())),
                "{name}"
            );
        }
    }

    #[test]
    fn an_exact_name_resolves_to_its_entity() {
        let mut memory = lookup();
        let id = memory.add_person("Jean Dupont");
        let answer = resolve_in(
            &memory,
            ResolveMode::Ingestion,
            "Dr. Jean Dupont",
            None,
            None,
        );
        match answer {
            Resolution::Existing { entity, signal, .. } => {
                assert_eq!(entity.entity_id, id);
                assert_eq!(signal, Signal::CanonicalName);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn a_lookup_that_failed_means_unresolved_not_new() {
        struct Broken;
        impl EntityLookup for Broken {
            fn by_identifier(&self, _: &str, _: &str) -> Vec<EntityHit> {
                Vec::new()
            }
            fn by_normalized(&self, _: &str) -> Vec<EntityHit> {
                Vec::new()
            }
            fn by_alias(&self, _: &str) -> Vec<EntityHit> {
                Vec::new()
            }
            fn by_token_set(&self, _: &str) -> Vec<EntityHit> {
                Vec::new()
            }
            fn by_phonetic(&self, _: &str) -> Vec<EntityHit> {
                Vec::new()
            }
            fn is_tombstoned(&self, _: &str, _: &str) -> bool {
                false
            }
            fn restrict_to_selection(&self, hits: Vec<EntityHit>, _: &SourceSet) -> Vec<EntityHit> {
                hits
            }
            fn failed(&self) -> bool {
                true
            }
        }
        let set = titles();
        let encoder = FrenchPhonetic;
        let answer = DeterministicResolver.resolve(
            &SurfaceInput::new("Jean Dupont"),
            &ResolveContext {
                mode: ResolveMode::Ingestion,
                titles: &set,
                encoder: &encoder,
                lookup: &Broken,
                selection: None,
                source_entities: None,
            },
        );
        assert_eq!(
            answer,
            Resolution::Unresolved {
                reason: UnresolvedReason::LookupFailed
            }
        );
    }

    #[test]
    fn the_fuzzy_rule_needs_a_word_long_enough() {
        let hit = |name: &str| EntityHit {
            entity_id: 1,
            type_id: "person".to_string(),
            canonical_name: name.to_string(),
            normalized_name: name.to_string(),
            disambiguator: String::new(),
            status: EntityStatus::Active,
            origin: Origin::Automatic,
            alias_kind: None,
            alias_origin: None,
            alias_normalized: None,
        };
        let words = |text: &str| -> Vec<String> { text.split(' ').map(str::to_string).collect() };
        // One edit on a long word: safe.
        assert!(is_safe_fuzzy(&words("jean dupond"), &hit("jean dupont")));
        // One edit on a three-letter word: not safe.
        assert!(!is_safe_fuzzy(&words("lee"), &hit("lea")));
        // Two words differ: not safe.
        assert!(!is_safe_fuzzy(&words("jane dupond"), &hit("jean dupont")));
        // Two edits: not safe.
        assert!(!is_safe_fuzzy(&words("dupxnd"), &hit("dupont")));
        // A different number of words: not safe.
        assert!(!is_safe_fuzzy(&words("dupond"), &hit("jean dupont")));
        assert_eq!(FUZZY_MIN_TOKEN_LEN, 4);
    }
}
