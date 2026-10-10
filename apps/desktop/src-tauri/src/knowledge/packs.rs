//! Lexicon packs: the words and patterns the knowledge base reads names with.
//!
//! A pack is a JSON file bundled with the application, read with `include_str!` exactly like
//! `resources/tabular-questions/*.json`. It holds **words and patterns only** - titles, particles,
//! stop-words, organisation markers, identifier patterns, header vocabulary, role label keys,
//! detection terms. It never holds a sentence a person reads: a sentence is an interface catalogue
//! entry, and a role is shown through its `label_key` (`docs/SESSION-KB-00-master.md`, I10).
//!
//! ```text
//! resources/knowledge/base/<locale>.json              always loaded: profession-neutral
//! resources/knowledge/packs/<pack-id>/<locale>.json   optional: adds vocabulary for one domain
//! ```
//!
//! **A domain pack only adds.** It cannot change an entity type (there is no field for it), cannot
//! map a header the base or another active pack already maps to a different type, and gives a role
//! a different *label*, never a different meaning. The schema and the Rust code stay
//! profession-neutral (D2).
//!
//! Loading is strict: an unknown field, a missing locale, a pattern that does not compile, a role
//! nobody defined or a header mapped to two types is `knowledge_pack_invalid`, with the pack id and
//! the place in the file. A test loads every bundled pack in every locale, so a typo fails the
//! build rather than a user's analysis.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ops::Range;

use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::normalize::{normalize_identifier, normalize_name, TitleSet};
use super::{Confidence, EntityTypeId, RoleId};
use crate::error::AppError;

/// The only pack layout this build reads.
pub const PACK_SCHEMA: u32 = 1;

/// The pack that is always loaded.
pub const BASE_PACK_ID: &str = "base";

/// The locale a missing one falls back to, as everywhere else in the client.
pub const FALLBACK_LOCALE: &str = "fr-FR";

/// A list entry longer than this is a sentence, not a word or a phrase.
const MAX_WORDS_PER_ENTRY: usize = 4;

/// A pattern whose compiled form exceeds this is refused: a pack is data, and data does not get to
/// make the program allocate without bound.
const REGEX_SIZE_LIMIT: usize = 256 * 1024;

struct BundledPack {
    id: &'static str,
    locale: &'static str,
    body: &'static str,
}

const BUNDLED: &[BundledPack] = &[
    BundledPack {
        id: "base",
        locale: "en-US",
        body: include_str!("../../resources/knowledge/base/en-US.json"),
    },
    BundledPack {
        id: "base",
        locale: "fr-FR",
        body: include_str!("../../resources/knowledge/base/fr-FR.json"),
    },
    BundledPack {
        id: "health",
        locale: "en-US",
        body: include_str!("../../resources/knowledge/packs/health/en-US.json"),
    },
    BundledPack {
        id: "health",
        locale: "fr-FR",
        body: include_str!("../../resources/knowledge/packs/health/fr-FR.json"),
    },
    BundledPack {
        id: "legal",
        locale: "en-US",
        body: include_str!("../../resources/knowledge/packs/legal/en-US.json"),
    },
    BundledPack {
        id: "legal",
        locale: "fr-FR",
        body: include_str!("../../resources/knowledge/packs/legal/fr-FR.json"),
    },
    BundledPack {
        id: "accounting",
        locale: "en-US",
        body: include_str!("../../resources/knowledge/packs/accounting/en-US.json"),
    },
    BundledPack {
        id: "accounting",
        locale: "fr-FR",
        body: include_str!("../../resources/knowledge/packs/accounting/fr-FR.json"),
    },
];

/// The ids of the optional packs this build ships, sorted. `base` is not among them: it is always
/// loaded.
pub fn available_pack_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = BUNDLED
        .iter()
        .map(|pack| pack.id)
        .filter(|id| *id != BASE_PACK_ID)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Every `(pack id, locale tag)` this build ships, base included, in a stable order. What the test
/// that loads everything iterates over.
pub fn bundled_packs() -> Vec<(&'static str, &'static str)> {
    let mut all: Vec<(&'static str, &'static str)> =
        BUNDLED.iter().map(|pack| (pack.id, pack.locale)).collect();
    all.sort_unstable();
    all
}

// ---------------------------------------------------------------------------------------------
// The compiled forms a loaded pack set hands out
// ---------------------------------------------------------------------------------------------

/// How an identifier is reduced before two of them are compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierForm {
    /// Letters and digits, upper case, accents folded (`normalize_identifier`).
    Alnum,
    /// Digits only.
    Digits,
}

impl IdentifierForm {
    pub fn apply(self, text: &str) -> String {
        match self {
            Self::Alnum => normalize_identifier(text),
            Self::Digits => text.chars().filter(char::is_ascii_digit).collect(),
        }
    }
}

/// Whether two identifiers that reduce to the same value are the same thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierStrength {
    /// Yes: an invoice number, an IBAN, an e-mail address.
    Exact,
    /// Not necessarily: a phone number two people of a household share. A collision is a question.
    Possible,
}

/// One occurrence of an identifier in a text: where, and its reduced form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifierMatch {
    pub range: Range<usize>,
    pub normalized: String,
}

/// An identifier pattern with its compiled regex.
#[derive(Debug, Clone)]
pub struct IdentifierScheme {
    pub pack: String,
    pub id: String,
    /// Becomes the entity subtype (`email`, `invoice`, ...).
    pub kind: String,
    pub form: IdentifierForm,
    pub strength: IdentifierStrength,
    /// The identifier points at one person (a social security number, an e-mail address): the
    /// resolver may use it to recognise that person under any spelling of the name.
    pub personal_key: bool,
    regex: Regex,
}

impl IdentifierScheme {
    /// Every non-overlapping occurrence in `text`, left to right. The text is read, never kept.
    pub fn scan(&self, text: &str) -> Vec<IdentifierMatch> {
        self.regex
            .find_iter(text)
            .map(|found| IdentifierMatch {
                range: found.range(),
                normalized: self.form.apply(found.as_str()),
            })
            .filter(|found| !found.normalized.is_empty())
            .collect()
    }

    pub fn pattern(&self) -> &str {
        self.regex.as_str()
    }
}

/// What a column header says about the cells under it.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnRule {
    pub pack: String,
    /// Each header as its folded words ("nom de famille" is `["nom", "de", "famille"]`).
    pub headers: Vec<Vec<String>>,
    pub entity_type: EntityTypeId,
    pub subtype: Option<String>,
    pub role: Option<RoleId>,
    pub confidence: Confidence,
}

/// "A client is served_by a provider": a relation a table states by having two role columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationRule {
    pub pack: String,
    pub from_role: RoleId,
    pub to_role: RoleId,
    pub predicate: String,
}

/// The i18n key the interface resolves to show a role. The last active pack that defines a role
/// wins, so a profession shows "Patient" where the base shows "Client".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleLabel {
    pub pack: String,
    pub id: RoleId,
    pub label_key: String,
}

/// Words that mark an organisation, as folded word sequences.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrgMarkers {
    /// "association", "clinique": they come before the name.
    pub prefix: Vec<Vec<String>>,
    /// "sarl", "inc": they come after it.
    pub suffix: Vec<Vec<String>>,
}

// ---------------------------------------------------------------------------------------------
// The file format
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPack {
    schema: u32,
    id: String,
    locale: String,
    #[serde(default)]
    titles: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    weak_titles: Vec<String>,
    #[serde(default)]
    particles: Vec<String>,
    #[serde(default)]
    title_spoken: BTreeMap<String, String>,
    #[serde(default)]
    stop_words: Vec<String>,
    #[serde(default)]
    org_markers: RawOrgMarkers,
    #[serde(default)]
    identifier_schemes: Vec<RawScheme>,
    #[serde(default)]
    columns: Vec<RawColumn>,
    #[serde(default)]
    relations: Vec<RawRelation>,
    #[serde(default)]
    roles: Vec<RawRole>,
    #[serde(default)]
    detection_terms: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOrgMarkers {
    #[serde(default)]
    prefix: Vec<String>,
    #[serde(default)]
    suffix: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScheme {
    id: String,
    kind: String,
    regex: String,
    normalise: String,
    strength: String,
    #[serde(default)]
    personal_key: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawColumn {
    headers: Vec<String>,
    semantic: RawSemantic,
    confidence: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSemantic {
    #[serde(rename = "type")]
    entity_type: String,
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default)]
    role: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRelation {
    from_role: String,
    to_role: String,
    predicate: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRole {
    id: String,
    label_key: String,
}

/// One validated pack, before it is merged with the others.
struct LoadedPack {
    id: String,
    locale: String,
    canonical: String,
    titles: Vec<String>,
    weak_titles: Vec<String>,
    particles: Vec<String>,
    title_spoken: BTreeMap<String, String>,
    stop_words: Vec<String>,
    org_markers: OrgMarkers,
    schemes: Vec<IdentifierScheme>,
    columns: Vec<(ColumnRule, usize)>,
    relations: Vec<(RelationRule, usize)>,
    roles: Vec<RoleLabel>,
    detection_terms: Vec<String>,
}

fn invalid(pack: &str, path: &str) -> AppError {
    AppError::KnowledgePackInvalid {
        pack: pack.to_string(),
        path: path.to_string(),
    }
}

/// The folded words of a text, in order: "Nom de famille" is `["nom", "de", "famille"]`.
fn words_of(text: &str) -> Vec<String> {
    normalize_name(text, &TitleSet::new()).tokens
}

/// A machine id: lower-case ASCII letters, digits and underscores, starting with a letter.
fn is_machine_id(text: &str) -> bool {
    let mut characters = text.chars();
    characters.next().is_some_and(|c| c.is_ascii_lowercase())
        && characters.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// A list entry is a word or a short phrase: something a person *matches against*, never something
/// a person *reads*.
fn check_entry(pack: &str, path: &str, text: &str) -> Result<(), AppError> {
    let trimmed = text.trim();
    let sentence_like = trimmed.split_whitespace().count() > MAX_WORDS_PER_ENTRY
        || trimmed.contains(['?', '!', ';', ':'])
        || trimmed.contains(". ")
        || trimmed.ends_with("...");
    if trimmed.is_empty() || sentence_like || words_of(trimmed).is_empty() {
        return Err(invalid(pack, path));
    }
    Ok(())
}

fn check_entries(pack: &str, path: &str, entries: &[String]) -> Result<(), AppError> {
    for (index, entry) in entries.iter().enumerate() {
        check_entry(pack, &format!("{path}[{index}]"), entry)?;
    }
    Ok(())
}

fn phrases(entries: &[String]) -> Vec<Vec<String>> {
    entries.iter().map(|entry| words_of(entry)).collect()
}

fn parse_pack(
    expected_id: &str,
    expected_locale: &str,
    body: &str,
) -> Result<LoadedPack, AppError> {
    let raw: RawPack = serde_json::from_str(body).map_err(|error| {
        invalid(
            expected_id,
            &format!("line {} column {}", error.line(), error.column()),
        )
    })?;
    if raw.schema != PACK_SCHEMA {
        return Err(invalid(expected_id, "schema"));
    }
    if raw.id != expected_id {
        return Err(invalid(expected_id, "id"));
    }
    if raw.locale != expected_locale {
        return Err(invalid(expected_id, "locale"));
    }
    let pack = raw.id.as_str();

    for (type_id, words) in &raw.titles {
        if !EntityTypeId::BUILTIN.contains(&type_id.as_str()) {
            return Err(invalid(pack, &format!("titles.{type_id}")));
        }
        check_entries(pack, &format!("titles.{type_id}"), words)?;
    }
    check_entries(pack, "weak_titles", &raw.weak_titles)?;
    check_entries(pack, "particles", &raw.particles)?;
    check_entries(pack, "stop_words", &raw.stop_words)?;
    check_entries(pack, "org_markers.prefix", &raw.org_markers.prefix)?;
    check_entries(pack, "org_markers.suffix", &raw.org_markers.suffix)?;
    check_entries(pack, "detection_terms", &raw.detection_terms)?;

    // A spoken form belongs to a title the same pack (or the base) declares; the key is the title
    // as it is folded, because that is how it is looked up.
    let mut title_spoken = BTreeMap::new();
    for (title, spoken) in &raw.title_spoken {
        check_entry(pack, &format!("title_spoken.{title}"), title)?;
        check_entry(pack, &format!("title_spoken.{title}"), spoken)?;
        title_spoken.insert(words_of(title).concat(), spoken.trim().to_string());
    }

    let mut schemes = Vec::new();
    let mut scheme_ids = HashSet::new();
    for (index, scheme) in raw.identifier_schemes.iter().enumerate() {
        let at = |field: &str| format!("identifier_schemes[{index}].{field}");
        if !is_machine_id(&scheme.id) || !scheme_ids.insert(scheme.id.clone()) {
            return Err(invalid(pack, &at("id")));
        }
        if !is_machine_id(&scheme.kind) {
            return Err(invalid(pack, &at("kind")));
        }
        let form = match scheme.normalise.as_str() {
            "alnum" => IdentifierForm::Alnum,
            "digits" => IdentifierForm::Digits,
            _ => return Err(invalid(pack, &at("normalise"))),
        };
        let strength = match scheme.strength.as_str() {
            "exact" => IdentifierStrength::Exact,
            "possible" => IdentifierStrength::Possible,
            _ => return Err(invalid(pack, &at("strength"))),
        };
        let regex = RegexBuilder::new(&scheme.regex)
            .size_limit(REGEX_SIZE_LIMIT)
            .build()
            .map_err(|_| invalid(pack, &at("regex")))?;
        // A pattern that matches nothing would "find" an identifier at every position.
        if regex.is_match("") {
            return Err(invalid(pack, &at("regex")));
        }
        schemes.push(IdentifierScheme {
            pack: pack.to_string(),
            id: scheme.id.clone(),
            kind: scheme.kind.clone(),
            form,
            strength,
            personal_key: scheme.personal_key,
            regex,
        });
    }

    let mut columns = Vec::new();
    for (index, column) in raw.columns.iter().enumerate() {
        let at = |field: &str| format!("columns[{index}].{field}");
        if column.headers.is_empty() {
            return Err(invalid(pack, &at("headers")));
        }
        check_entries(pack, &at("headers"), &column.headers)?;
        let entity_type = EntityTypeId::new(&column.semantic.entity_type)
            .filter(EntityTypeId::is_builtin)
            .ok_or_else(|| invalid(pack, &at("semantic.type")))?;
        let subtype = match &column.semantic.subtype {
            Some(subtype) if is_machine_id(subtype) => Some(subtype.clone()),
            Some(_) => return Err(invalid(pack, &at("semantic.subtype"))),
            None => None,
        };
        let role = match &column.semantic.role {
            Some(role) => {
                Some(RoleId::new(role).ok_or_else(|| invalid(pack, &at("semantic.role")))?)
            }
            None => None,
        };
        if !(column.confidence > 0.0 && column.confidence <= 1.0) {
            return Err(invalid(pack, &at("confidence")));
        }
        columns.push((
            ColumnRule {
                pack: pack.to_string(),
                headers: phrases(&column.headers),
                entity_type,
                subtype,
                role,
                confidence: Confidence::new(column.confidence),
            },
            index,
        ));
    }

    let mut relations = Vec::new();
    for (index, relation) in raw.relations.iter().enumerate() {
        let at = |field: &str| format!("relations[{index}].{field}");
        let from_role =
            RoleId::new(&relation.from_role).ok_or_else(|| invalid(pack, &at("from_role")))?;
        let to_role =
            RoleId::new(&relation.to_role).ok_or_else(|| invalid(pack, &at("to_role")))?;
        if !is_machine_id(&relation.predicate) {
            return Err(invalid(pack, &at("predicate")));
        }
        relations.push((
            RelationRule {
                pack: pack.to_string(),
                from_role,
                to_role,
                predicate: relation.predicate.clone(),
            },
            index,
        ));
    }

    let mut roles = Vec::new();
    let mut role_ids = HashSet::new();
    for (index, role) in raw.roles.iter().enumerate() {
        let at = |field: &str| format!("roles[{index}].{field}");
        let id = RoleId::new(&role.id).ok_or_else(|| invalid(pack, &at("id")))?;
        if !role_ids.insert(role.id.clone()) {
            return Err(invalid(pack, &at("id")));
        }
        // A key of the interface catalogue: dotted lower-case segments under `knowledge.`.
        let key_ok = role.label_key.starts_with("knowledge.")
            && role.label_key.split('.').all(|segment| {
                !segment.is_empty()
                    && segment
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
            });
        if !key_ok {
            return Err(invalid(pack, &at("label_key")));
        }
        roles.push(RoleLabel {
            pack: pack.to_string(),
            id,
            label_key: role.label_key.clone(),
        });
    }

    let value: Value = serde_json::from_str(body).map_err(|_| invalid(pack, "json"))?;
    let mut canonical = String::new();
    write_canonical(&value, &mut canonical);

    Ok(LoadedPack {
        id: raw.id.clone(),
        locale: raw.locale.clone(),
        canonical,
        titles: raw.titles.get("person").cloned().unwrap_or_default(),
        weak_titles: raw.weak_titles,
        particles: raw.particles,
        title_spoken,
        stop_words: raw.stop_words,
        org_markers: OrgMarkers {
            prefix: phrases(&raw.org_markers.prefix),
            suffix: phrases(&raw.org_markers.suffix),
        },
        schemes,
        columns,
        relations,
        roles,
        detection_terms: raw.detection_terms,
    })
}

/// JSON with every object's keys sorted and no whitespace, so that the hash of a pack depends on
/// its content and on nothing else (not on key order, not on line endings, not on indentation).
fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (position, key) in keys.into_iter().enumerate() {
                if position > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                write_canonical(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (position, item) in items.iter().enumerate() {
                if position > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// The bundled file for `pack_id` that best serves `locale`: the exact tag, then any tag of the
/// same language, then the fallback locale - the order the interface and the tabular question
/// packs already use.
fn bundled_for<'a>(pack_id: &str, locale: &str) -> Option<&'a BundledPack> {
    let language = locale
        .split('-')
        .next()
        .unwrap_or(locale)
        .to_ascii_lowercase();
    let of_pack = || BUNDLED.iter().filter(|pack| pack.id == pack_id);
    of_pack()
        .find(|pack| pack.locale.eq_ignore_ascii_case(locale))
        .or_else(|| {
            of_pack().find(|pack| {
                pack.locale
                    .split('-')
                    .next()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&language))
            })
        })
        .or_else(|| of_pack().find(|pack| pack.locale == FALLBACK_LOCALE))
}

// ---------------------------------------------------------------------------------------------
// The merged pack set
// ---------------------------------------------------------------------------------------------

/// The base pack and the active domain packs, validated and merged. Built once per Analyse pass or
/// per question, never per file.
#[derive(Debug, Clone)]
pub struct PackSet {
    locale: String,
    loaded: Vec<(String, String)>,
    titles: Vec<String>,
    weak_titles: Vec<String>,
    particles: Vec<String>,
    stop_words: Vec<String>,
    title_spoken: BTreeMap<String, String>,
    org_markers: OrgMarkers,
    schemes: Vec<IdentifierScheme>,
    columns: Vec<ColumnRule>,
    relations: Vec<RelationRule>,
    roles: BTreeMap<RoleId, RoleLabel>,
    detection_terms: BTreeMap<String, Vec<String>>,
    hash: String,
}

impl PackSet {
    /// Load `base` and every pack of `active_pack_ids`, in that order, for `locale`. An id twice is
    /// read once. An id no bundled pack has is `knowledge_pack_invalid`.
    pub fn load(locale: &str, active_pack_ids: &[&str]) -> Result<Self, AppError> {
        let mut ids: Vec<&str> = vec![BASE_PACK_ID];
        for id in active_pack_ids {
            if !ids.contains(id) {
                ids.push(id);
            }
        }

        let mut parsed = Vec::with_capacity(ids.len());
        for id in &ids {
            let bundled = bundled_for(id, locale).ok_or_else(|| invalid(id, "pack"))?;
            parsed.push(parse_pack(bundled.id, bundled.locale, bundled.body)?);
        }
        Self::merge(locale, parsed)
    }

    fn merge(locale: &str, parsed: Vec<LoadedPack>) -> Result<Self, AppError> {
        let mut set = Self {
            locale: locale.to_string(),
            loaded: Vec::new(),
            titles: Vec::new(),
            weak_titles: Vec::new(),
            particles: Vec::new(),
            stop_words: Vec::new(),
            title_spoken: BTreeMap::new(),
            org_markers: OrgMarkers::default(),
            schemes: Vec::new(),
            columns: Vec::new(),
            relations: Vec::new(),
            roles: BTreeMap::new(),
            detection_terms: BTreeMap::new(),
            hash: String::new(),
        };

        let mut hasher = Sha256::new();
        hasher.update(format!("schema:{PACK_SCHEMA}\n"));
        let mut scheme_ids: HashSet<String> = HashSet::new();
        let mut header_types: BTreeMap<String, EntityTypeId> = BTreeMap::new();
        let mut column_positions: Vec<(String, usize)> = Vec::new();
        let mut relation_positions: Vec<(String, usize)> = Vec::new();

        for pack in parsed {
            hasher.update(format!("{}:{}\n{}\n", pack.id, pack.locale, pack.canonical));
            set.loaded.push((pack.id.clone(), pack.locale.clone()));
            set.titles.extend(pack.titles);
            set.weak_titles.extend(pack.weak_titles);
            set.particles.extend(pack.particles);
            set.stop_words.extend(pack.stop_words);
            set.title_spoken.extend(pack.title_spoken);
            set.org_markers.prefix.extend(pack.org_markers.prefix);
            set.org_markers.suffix.extend(pack.org_markers.suffix);

            for scheme in pack.schemes {
                if !scheme_ids.insert(scheme.id.clone()) {
                    return Err(invalid(
                        &pack.id,
                        &format!("identifier_schemes.{}", scheme.id),
                    ));
                }
                set.schemes.push(scheme);
            }

            for (rule, index) in pack.columns {
                // A pack adds vocabulary; it never changes what a header already means.
                for header in &rule.headers {
                    let key = header.join(" ");
                    match header_types.get(&key) {
                        Some(existing) if *existing != rule.entity_type => {
                            return Err(invalid(&pack.id, &format!("columns[{index}].headers")));
                        }
                        _ => {
                            header_types.insert(key, rule.entity_type.clone());
                        }
                    }
                }
                column_positions.push((pack.id.clone(), index));
                set.columns.push(rule);
            }
            for (rule, index) in pack.relations {
                relation_positions.push((pack.id.clone(), index));
                set.relations.push(rule);
            }
            for label in pack.roles {
                set.roles.insert(label.id.clone(), label);
            }
            if !pack.detection_terms.is_empty() {
                set.detection_terms
                    .entry(pack.id.clone())
                    .or_default()
                    .extend(pack.detection_terms);
            }
        }

        // Roles are only known once every pack is merged: a domain pack may use a role the base
        // defines, and the base may not know a role only a domain pack defines.
        for (rule, (pack, index)) in set.columns.iter().zip(&column_positions) {
            if let Some(role) = &rule.role {
                if !set.roles.contains_key(role) {
                    return Err(invalid(pack, &format!("columns[{index}].semantic.role")));
                }
            }
        }
        for (rule, (pack, index)) in set.relations.iter().zip(&relation_positions) {
            for (role, field) in [(&rule.from_role, "from_role"), (&rule.to_role, "to_role")] {
                if !set.roles.contains_key(role) {
                    return Err(invalid(pack, &format!("relations[{index}].{field}")));
                }
            }
        }

        let digest = hasher.finalize();
        set.hash = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        Ok(set)
    }

    /// The locale that was asked for (the packs read may be those of the fallback).
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// `(pack id, locale tag actually read)` for each pack, base first.
    pub fn loaded(&self) -> &[(String, String)] {
        &self.loaded
    }

    /// A stable fingerprint of what was loaded, for `kb_meta.packs_hash`. It changes when a pack
    /// changes, when another one is activated and when the order of activation changes; it does
    /// not change with key order, indentation or line endings.
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// The words a name is read with, merged from every loaded pack.
    pub fn title_set(&self) -> TitleSet {
        TitleSet::new()
            .with_titles(&self.titles)
            .with_weak_titles(&self.weak_titles)
            .with_particles(&self.particles)
            .with_stop_words(&self.stop_words)
    }

    /// Whether `word` (any case, any accent) is one of the stop-words: a month, a document word, a
    /// null-like cell value.
    pub fn is_stop_word(&self, word: &str) -> bool {
        let wanted = words_of(word).concat();
        !wanted.is_empty()
            && self
                .stop_words
                .iter()
                .any(|candidate| words_of(candidate).concat() == wanted)
    }

    /// How a title is said aloud, for the text-to-speech surface (`title_spoken`), looked up by the
    /// title as written ("Dr.", "DR" and "dr" are one).
    pub fn title_spoken(&self, title: &str) -> Option<&str> {
        self.title_spoken
            .get(&words_of(title).concat())
            .map(String::as_str)
    }

    pub fn org_markers(&self) -> &OrgMarkers {
        &self.org_markers
    }

    pub fn identifier_schemes(&self) -> &[IdentifierScheme] {
        &self.schemes
    }

    pub fn identifier_scheme(&self, id: &str) -> Option<&IdentifierScheme> {
        self.schemes.iter().find(|scheme| scheme.id == id)
    }

    pub fn column_rules(&self) -> &[ColumnRule] {
        &self.columns
    }

    /// The rule for a header as written, if any pack knows it. The whole header is compared before
    /// it is split into words, so a column named after a vocabulary word is not mistaken for it
    /// (the lesson of defect KBD-01).
    pub fn column_rule_for(&self, header: &str) -> Option<&ColumnRule> {
        let wanted = words_of(header);
        if wanted.is_empty() {
            return None;
        }
        self.columns
            .iter()
            .find(|rule| rule.headers.contains(&wanted))
    }

    pub fn relations(&self) -> &[RelationRule] {
        &self.relations
    }

    /// Every role with the label key of the last pack that defined it, in id order.
    pub fn roles(&self) -> impl Iterator<Item = &RoleLabel> {
        self.roles.values()
    }

    pub fn role_label_key(&self, role: &RoleId) -> Option<&str> {
        self.roles.get(role).map(|label| label.label_key.as_str())
    }

    /// The words that, found in a source, point at the domain of pack `pack_id`.
    pub fn detection_terms(&self, pack_id: &str) -> &[String] {
        self.detection_terms
            .get(pack_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Every distinct entity type some column rule can produce.
    pub fn column_types(&self) -> BTreeSet<&str> {
        self.columns
            .iter()
            .map(|rule| rule.entity_type.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal(extra: &str) -> String {
        format!(
            r#"{{"schema":1,"id":"base","locale":"en-US","roles":[{{"id":"client","label_key":"knowledge.roles.client"}}]{extra}}}"#
        )
    }

    fn parse(body: &str) -> Result<LoadedPack, AppError> {
        parse_pack("base", "en-US", body)
    }

    fn code_and_path(result: Result<LoadedPack, AppError>) -> (String, String) {
        match result {
            Err(AppError::KnowledgePackInvalid { pack, path }) => (pack, path),
            Err(other) => panic!("unexpected error {other:?}"),
            Ok(_) => panic!("the pack was accepted"),
        }
    }

    #[test]
    fn every_bundled_pack_loads_in_every_locale() {
        for (id, locale) in bundled_packs() {
            let bundled = bundled_for(id, locale).expect("bundled");
            if let Err(error) = parse_pack(bundled.id, bundled.locale, bundled.body) {
                panic!("{id} {locale}: {error:?}");
            }
        }
    }

    #[test]
    fn a_minimal_pack_is_valid() {
        assert!(parse(&minimal("")).is_ok());
    }

    #[test]
    fn an_unknown_field_is_refused_with_the_pack_and_a_position() {
        let (pack, path) = code_and_path(parse(&minimal(r#","surprise":1"#)));
        assert_eq!(pack, "base");
        assert!(path.starts_with("line "), "{path}");
    }

    #[test]
    fn a_wrong_schema_id_or_locale_is_refused() {
        let body = minimal("");
        assert_eq!(
            code_and_path(parse(&body.replace("\"schema\":1", "\"schema\":2"))).1,
            "schema"
        );
        assert_eq!(code_and_path(parse_pack("health", "en-US", &body)).1, "id");
        assert_eq!(
            code_and_path(parse_pack("base", "fr-FR", &body)).1,
            "locale"
        );
    }

    #[test]
    fn a_sentence_in_a_word_list_is_refused() {
        let long = r#","stop_words":["one two three four five"]"#;
        assert_eq!(code_and_path(parse(&minimal(long))).1, "stop_words[0]");
        let question = r#","particles":["what now?"]"#;
        assert_eq!(code_and_path(parse(&minimal(question))).1, "particles[0]");
        let empty = r#","weak_titles":["  "]"#;
        assert_eq!(code_and_path(parse(&minimal(empty))).1, "weak_titles[0]");
    }

    #[test]
    fn a_pattern_that_does_not_compile_or_matches_nothing_is_refused() {
        let scheme = |regex: &str| {
            format!(
                r#","identifier_schemes":[{{"id":"x","kind":"x","regex":"{regex}","normalise":"alnum","strength":"exact"}}]"#
            )
        };
        assert_eq!(
            code_and_path(parse(&minimal(&scheme("(unclosed")))).1,
            "identifier_schemes[0].regex"
        );
        assert_eq!(
            code_and_path(parse(&minimal(&scheme("a*")))).1,
            "identifier_schemes[0].regex"
        );
        assert!(parse(&minimal(&scheme("[0-9]{3}"))).is_ok());
    }

    #[test]
    fn an_identifier_scheme_finds_and_reduces_its_matches() {
        let body = minimal(
            r#","identifier_schemes":[{"id":"ref","kind":"reference","regex":"\\b[A-Z]{2}-\\d{3}\\b","normalise":"alnum","strength":"exact"}]"#,
        );
        let pack = parse(&body).unwrap();
        let found = pack.schemes[0].scan("see AB-123 and CD-456.");
        assert_eq!(
            found
                .iter()
                .map(|m| m.normalized.as_str())
                .collect::<Vec<_>>(),
            ["AB123", "CD456"]
        );
        assert_eq!(&"see AB-123 and CD-456."[found[0].range.clone()], "AB-123");
    }

    #[test]
    fn a_bad_enum_value_names_its_field() {
        let scheme = r#","identifier_schemes":[{"id":"x","kind":"x","regex":"[0-9]+","normalise":"hex","strength":"exact"}]"#;
        assert_eq!(
            code_and_path(parse(&minimal(scheme))).1,
            "identifier_schemes[0].normalise"
        );
        let column =
            r#","columns":[{"headers":["name"],"semantic":{"type":"robot"},"confidence":0.5}]"#;
        assert_eq!(
            code_and_path(parse(&minimal(column))).1,
            "columns[0].semantic.type"
        );
        let confidence =
            r#","columns":[{"headers":["name"],"semantic":{"type":"person"},"confidence":1.5}]"#;
        assert_eq!(
            code_and_path(parse(&minimal(confidence))).1,
            "columns[0].confidence"
        );
    }

    #[test]
    fn a_label_key_outside_the_knowledge_namespace_is_refused() {
        let body = minimal("").replace("knowledge.roles.client", "settings.title");
        assert_eq!(code_and_path(parse(&body)).1, "roles[0].label_key");
    }

    #[test]
    fn the_same_content_hashes_the_same_whatever_the_formatting() {
        let compact = minimal("");
        let spaced = compact.replace(',', ",\n    ").replace('{', "{\n  ");
        let a = parse(&compact).unwrap().canonical;
        let b = parse(&spaced).unwrap().canonical;
        assert_eq!(a, b);
    }

    #[test]
    fn a_role_nobody_defines_is_refused_when_the_set_is_assembled() {
        let base = parse(&minimal("")).unwrap();
        let domain = parse_pack(
            "base",
            "en-US",
            &minimal(
                r#","columns":[{"headers":["boss"],"semantic":{"type":"person","role":"manager"},"confidence":0.5}]"#,
            ),
        )
        .unwrap();
        let error = PackSet::merge("en-US", vec![base, domain]).unwrap_err();
        match error {
            AppError::KnowledgePackInvalid { path, .. } => {
                assert_eq!(path, "columns[0].semantic.role");
            }
            other => panic!("unexpected error {other:?}"),
        }
    }

    #[test]
    fn a_header_cannot_change_type_between_packs() {
        let first = parse_pack(
            "base",
            "en-US",
            &minimal(
                r#","columns":[{"headers":["case"],"semantic":{"type":"identifier"},"confidence":0.5}]"#,
            ),
        )
        .unwrap();
        let second = parse_pack(
            "base",
            "en-US",
            &minimal(
                r#","columns":[{"headers":["Case"],"semantic":{"type":"item"},"confidence":0.5}]"#,
            ),
        )
        .unwrap();
        assert!(PackSet::merge("en-US", vec![first, second]).is_err());
    }

    #[test]
    fn an_unknown_pack_id_is_refused() {
        match PackSet::load("en-US", &["astrology"]) {
            Err(AppError::KnowledgePackInvalid { pack, .. }) => assert_eq!(pack, "astrology"),
            other => panic!("unexpected result {other:?}"),
        }
    }

    #[test]
    fn locale_falls_back_like_the_rest_of_the_client() {
        let british = PackSet::load("en-GB", &[]).unwrap();
        assert_eq!(british.loaded()[0].1, "en-US");
        let unknown = PackSet::load("de-DE", &[]).unwrap();
        assert_eq!(unknown.loaded()[0].1, "fr-FR");
        let lower = PackSet::load("EN-us", &[]).unwrap();
        assert_eq!(lower.loaded()[0].1, "en-US");
    }

    #[test]
    fn the_hash_depends_on_content_and_on_the_active_packs() {
        let base = PackSet::load("en-US", &[]).unwrap();
        let again = PackSet::load("en-US", &[]).unwrap();
        let with_health = PackSet::load("en-US", &["health"]).unwrap();
        let french = PackSet::load("fr-FR", &[]).unwrap();
        assert_eq!(base.hash(), again.hash());
        assert_ne!(base.hash(), with_health.hash());
        assert_ne!(base.hash(), french.hash());
        assert_eq!(base.hash().len(), 64);
    }

    #[test]
    fn a_domain_pack_adds_vocabulary_and_relabels_a_role() {
        let base = PackSet::load("en-US", &[]).unwrap();
        let health = PackSet::load("en-US", &["health"]).unwrap();
        let client = RoleId::new("client").unwrap();
        assert_eq!(base.role_label_key(&client), Some("knowledge.roles.client"));
        assert_eq!(
            health.role_label_key(&client),
            Some("knowledge.roles.health.client")
        );
        assert!(base.column_rule_for("patient").is_none());
        let rule = health.column_rule_for("Patient").expect("health knows it");
        assert_eq!(rule.entity_type.as_str(), "person");
        assert_eq!(rule.role.as_ref(), Some(&client));
        assert!(health.column_rules().len() > base.column_rules().len());
    }

    #[test]
    fn a_header_is_looked_up_whole_before_it_is_split() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        assert!(packs.column_rule_for("Last Name").is_some());
        assert!(packs.column_rule_for("last_name").is_some());
        assert!(packs.column_rule_for("Total_Line").is_none());
        assert!(packs.column_rule_for("").is_none());
    }

    #[test]
    fn titles_particles_and_stop_words_reach_the_name_normaliser() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let set = packs.title_set();
        let name = normalize_name("Dr. Jean van Dupont", &set);
        assert_eq!(name.titles, ["dr"]);
        assert_eq!(name.token_set_key, "dupont jean");
        assert!(packs.is_stop_word("JANUARY"));
        assert!(!packs.is_stop_word("Dupont"));
        assert_eq!(packs.title_spoken("Dr."), Some("doctor"));
    }
}
