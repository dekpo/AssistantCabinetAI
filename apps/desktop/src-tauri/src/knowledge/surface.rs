//! The names of the knowledge base, ranked, for the features that need a vocabulary (lot 9).
//!
//! `surface_forms` is a read-only service. A speech engine asks it for a short list of names to
//! lean on, an autocomplete asks it for the names that begin with some letters, the suggestions ask
//! it which names are worth a question. None of them gets a row written: the knowledge base is built
//! from the user's files, never from what is asked of it (master invariant I3).
//!
//! **Selection first (invariant I7).** With a selection, only the names that are mentioned in the
//! selected files come back, and a name known only from other files is not revealed. `None` means
//! "the whole base" and is for code that has no selection to give (no command passes it).
//!
//! The composer's `@` list is `knowledge::picker`, which also reports the names found only outside
//! the selection; it stays the autocomplete of the interface. `surface_forms` with
//! [`Purpose::Autocomplete`] is the same idea ranked by weight, for the callers that have no
//! "outside the selection" offer to make.

use std::collections::BTreeMap;

use rusqlite::Connection;

use super::normalize::fold;
use super::packs::PackSet;
use super::query::QuerySelection;
use super::store;
use super::{Alias, AliasKind, Domain, Entity, EntityStatus, Origin};
use crate::error::AppError;

/// What the list is for. It decides who is left out and how long the list may be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Names that begin with typed letters. Everything is allowed, an unconfirmed name ranks last.
    Autocomplete,
    /// A short list given to a speech engine to lean on. An unconfirmed name is left out (a guess
    /// must not make the next guess look like a certainty), and the list is capped.
    SpeechHints,
    /// Names worth a question. Confirmed names only.
    Suggestions,
}

/// The most names a speech engine is given. A Whisper-style initial prompt holds roughly 200 tokens.
pub const SPEECH_HINTS_MAX_FORMS: usize = 40;

/// The size of that prompt, in tokens, as estimated by [`estimated_tokens`].
pub const SPEECH_HINTS_MAX_TOKENS: usize = 200;

/// Characters per token in [`estimated_tokens`]. An estimate for names written in the Latin alphabet,
/// not a measurement of any engine's tokenizer.
pub const CHARS_PER_TOKEN: usize = 3;

/// The longest list any purpose returns when the caller gives no smaller limit.
pub const MAX_FORMS: usize = 200;

/// What a name is worth in the ranking. A file counts most, then the mentions, then how sure the
/// reading was; a name typed by hand gets a boost, and an unconfirmed one is divided down and then
/// sorted after every confirmed one.
const WEIGHT_PER_FILE: f32 = 10.0;
const WEIGHT_PER_MENTION: f32 = 0.2;
const MENTIONS_COUNTED: i64 = 50;
const WEIGHT_PER_CONFIDENCE: f32 = 2.0;
const WEIGHT_MANUAL_BOOST: f32 = 5.0;
const WEIGHT_CANDIDATE_DIVISOR: f32 = 10.0;

/// One name, in the forms a feature can use.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceForm {
    pub entity_id: i64,
    /// The canonical name, as the user would read it.
    pub display: String,
    /// The folded form the name is compared by.
    pub normalized: String,
    /// What the name sounds like (the phonetic key of its canonical form). Empty when unknown.
    pub phonetic_key: String,
    /// How the name is said aloud: a typed pronunciation, else the display with its titles spelled
    /// out by the packs (`dr` as `docteur`), else the display.
    pub spoken_form: String,
    pub type_id: String,
    pub subtype: Option<String>,
    pub weight: f32,
}

/// What a caller asks for.
#[derive(Debug, Clone, Copy)]
pub struct SurfaceRequest<'a> {
    /// The files the names must come from. `None` is the whole base.
    pub selection: Option<&'a QuerySelection<'a>>,
    pub purpose: Purpose,
    pub limit: usize,
    /// For an autocomplete: the letters typed. The same rule as the name index: every word but the
    /// last must be complete, the last may be a beginning. Ignored by the other purposes.
    pub prefix: Option<&'a str>,
}

/// A name of the base with what the ranking and the other readers of the selection need.
#[derive(Debug, Clone)]
pub(crate) struct KnownEntity {
    /// The entity after following merges.
    pub entity: Entity,
    /// Files of the selection that mention it (or of the whole base without a selection).
    pub files: usize,
    pub mentions: i64,
    pub confidence: f32,
    pub aliases: Vec<Alias>,
    /// A pronunciation typed by hand, if any.
    pub spoken_form: Option<String>,
}

impl KnownEntity {
    fn is_candidate(&self) -> bool {
        self.entity.status == EntityStatus::Candidate
    }

    /// A guess of the extractor that nobody confirmed. A name typed by hand is not one, even while
    /// it is still a candidate (the name list counts it the same way).
    pub(crate) fn is_unconfirmed(&self) -> bool {
        self.is_candidate() && self.entity.origin == Origin::Automatic
    }

    fn weight(&self) -> f32 {
        let mut weight = self.files as f32 * WEIGHT_PER_FILE
            + self.mentions.min(MENTIONS_COUNTED) as f32 * WEIGHT_PER_MENTION
            + self.confidence * WEIGHT_PER_CONFIDENCE;
        if self.entity.origin == Origin::Manual
            || self
                .aliases
                .iter()
                .any(|alias| alias.origin == Origin::Manual)
        {
            weight += WEIGHT_MANUAL_BOOST;
        }
        if self.is_candidate() {
            weight /= WEIGHT_CANDIDATE_DIVISOR;
        }
        weight
    }

    /// The key of the canonical spelling: the alias that is the canonical name itself.
    fn phonetic_key(&self) -> String {
        self.aliases
            .iter()
            .find(|alias| alias.normalized == self.entity.normalized_name)
            .or_else(|| {
                self.aliases
                    .iter()
                    .find(|alias| alias.kind == AliasKind::CanonicalVariant)
            })
            .map(|alias| alias.phonetic_key.clone())
            .unwrap_or_default()
    }
}

fn db<T>(result: rusqlite::Result<T>) -> Result<T, AppError> {
    result.map_err(|_| AppError::KnowledgeUnavailable)
}

fn json_ids(ids: &[i64]) -> Result<String, AppError> {
    serde_json::to_string(ids).map_err(|_| AppError::Internal)
}

/// The ids of the sources a selection covers, both folders.
pub(crate) fn selection_source_ids(
    connection: &Connection,
    selection: &QuerySelection<'_>,
) -> Result<Vec<i64>, AppError> {
    let pairs = |files: &[super::query::SelectedFile]| -> Vec<(String, String)> {
        files
            .iter()
            .map(|file| (file.relative_path.clone(), file.content_id.clone()))
            .collect()
    };
    let mut ids =
        store::source_ids_for_scope(connection, Domain::Documents, &pairs(selection.documents))?;
    ids.extend(store::source_ids_for_scope(
        connection,
        Domain::Data,
        &pairs(selection.data),
    )?);
    Ok(ids)
}

/// Every live name of the selection (or of the whole base), with its aliases and its weight inputs.
/// Three indexed queries, whatever the number of names.
pub(crate) fn known_entities(
    connection: &Connection,
    selection: Option<&QuerySelection<'_>>,
) -> Result<Vec<KnownEntity>, AppError> {
    let mut found: BTreeMap<i64, KnownEntity> = BTreeMap::new();
    let columns = "e.entity_id, e.type_id, e.subtype, e.canonical_name, e.normalized_name,
                   e.disambiguator, e.status, e.origin";
    let read = |row: &rusqlite::Row<'_>| -> rusqlite::Result<KnownEntity> {
        let status: String = row.get(6)?;
        let origin: String = row.get(7)?;
        Ok(KnownEntity {
            entity: Entity {
                entity_id: row.get(0)?,
                type_id: row.get(1)?,
                subtype: row.get(2)?,
                canonical_name: row.get(3)?,
                normalized_name: row.get(4)?,
                disambiguator: row.get(5)?,
                status: EntityStatus::from_code(&status).unwrap_or(EntityStatus::Candidate),
                merged_into: None,
                origin: Origin::from_code(&origin).unwrap_or(Origin::Automatic),
            },
            files: row.get::<_, i64>(8)?.max(0) as usize,
            mentions: row.get(9)?,
            confidence: row.get::<_, f64>(10)? as f32,
            aliases: Vec::new(),
            spoken_form: None,
        })
    };
    match selection {
        Some(selection) => {
            let sources = selection_source_ids(connection, selection)?;
            if sources.is_empty() {
                return Ok(Vec::new());
            }
            let mut statement = db(connection.prepare(&format!(
                "SELECT {columns}, COUNT(DISTINCT m.source_id),
                        COALESCE(SUM(m.occurrence_count), 0), COALESCE(MAX(m.confidence), 0.0)
                 FROM kb_effective_mentions m
                 JOIN kb_entities e ON e.entity_id = m.effective_entity_id
                 WHERE e.status IN ('candidate', 'active')
                   AND m.source_id IN (SELECT value FROM json_each(?1))
                 GROUP BY e.entity_id
                 ORDER BY e.entity_id"
            )))?;
            let rows = db(statement.query_map([json_ids(&sources)?], read))?;
            for row in rows {
                let known = db(row)?;
                found.insert(known.entity.entity_id, known);
            }
        }
        None => {
            let mut statement = db(connection.prepare(&format!(
                "SELECT {columns}, COUNT(DISTINCT m.source_id),
                        COALESCE(SUM(m.occurrence_count), 0), COALESCE(MAX(m.confidence), 0.0)
                 FROM kb_entities e
                 LEFT JOIN kb_effective_mentions m ON m.effective_entity_id = e.entity_id
                 WHERE e.status IN ('candidate', 'active')
                 GROUP BY e.entity_id
                 ORDER BY e.entity_id"
            )))?;
            let rows = db(statement.query_map([], read))?;
            for row in rows {
                let known = db(row)?;
                found.insert(known.entity.entity_id, known);
            }
        }
    }
    if found.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<i64> = found.keys().copied().collect();
    let ids = json_ids(&ids)?;

    // The aliases of every absorbed entity belong to the survivor.
    let mut statement = db(connection.prepare(
        "SELECT v.effective_id, a.alias_id, a.entity_id, a.display, a.normalized, a.phonetic_key,
                a.kind, a.origin, a.confidence
         FROM kb_aliases a
         JOIN kb_effective_entity v ON v.entity_id = a.entity_id
         WHERE v.effective_id IN (SELECT value FROM json_each(?1))
         ORDER BY v.effective_id, a.alias_id",
    ))?;
    let rows = db(statement.query_map([&ids], |row| {
        let kind: String = row.get(6)?;
        let origin: String = row.get(7)?;
        Ok((
            row.get::<_, i64>(0)?,
            Alias {
                alias_id: row.get(1)?,
                entity_id: row.get(2)?,
                display: row.get(3)?,
                normalized: row.get(4)?,
                phonetic_key: row.get(5)?,
                kind: AliasKind::from_code(&kind).unwrap_or(AliasKind::Manual),
                origin: Origin::from_code(&origin).unwrap_or(Origin::Automatic),
                confidence: row.get::<_, f64>(8)? as f32,
            },
        ))
    }))?;
    for row in rows {
        let (effective, alias) = db(row)?;
        if let Some(known) = found.get_mut(&effective) {
            known.aliases.push(alias);
        }
    }

    // A typed pronunciation. A manual row wins over an automatic one.
    let mut statement = db(connection.prepare(
        "SELECT v.effective_id, t.value, t.origin
         FROM kb_attributes t
         JOIN kb_effective_entity v ON v.entity_id = t.entity_id
         WHERE t.key = 'spoken_form' AND v.effective_id IN (SELECT value FROM json_each(?1))
         ORDER BY v.effective_id, CASE t.origin WHEN 'manual' THEN 0 ELSE 1 END, t.attribute_id",
    ))?;
    let rows = db(statement.query_map([&ids], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    }))?;
    for row in rows {
        let (effective, value) = db(row)?;
        if let Some(known) = found.get_mut(&effective) {
            if known.spoken_form.is_none() && !value.trim().is_empty() {
                known.spoken_form = Some(value);
            }
        }
    }
    Ok(found.into_values().collect())
}

/// How a name is said aloud: a typed pronunciation, else the display with each title spelled out
/// the way the packs say it, else the display.
pub fn spoken_form(display: &str, typed: Option<&str>, packs: &PackSet) -> String {
    if let Some(typed) = typed {
        if !typed.trim().is_empty() {
            return typed.trim().to_string();
        }
    }
    display
        .split_whitespace()
        .map(|word| packs.title_spoken(word).unwrap_or(word))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether `normalized` (an alias, folded) begins with the typed letters: every word of the prefix
/// but the last is a whole word, the last is the start of one, and the words follow each other.
fn begins_with(normalized: &str, prefix_words: &[String]) -> bool {
    let words: Vec<&str> = normalized
        .split(' ')
        .filter(|word| !word.is_empty())
        .collect();
    let Some((last, complete)) = prefix_words.split_last() else {
        return false;
    };
    if words.len() < prefix_words.len() {
        return false;
    }
    (0..=words.len() - prefix_words.len()).any(|start| {
        complete
            .iter()
            .enumerate()
            .all(|(offset, word)| words[start + offset] == word)
            && words[start + complete.len()].starts_with(last.as_str())
    })
}

/// The prefix as folded words.
fn prefix_words(prefix: &str) -> Vec<String> {
    fold(prefix)
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

/// The ranked names for a purpose.
///
/// Ranking: confirmed names before unconfirmed ones, then by weight (files of the selection,
/// mentions, confidence, a boost for what was typed by hand), then by name so that the order is
/// the same every time. Nothing here depends on the conversation: a client that wants a recency
/// boost keeps it in memory itself.
pub fn surface_forms(
    connection: &Connection,
    packs: &PackSet,
    request: &SurfaceRequest<'_>,
) -> Result<Vec<SurfaceForm>, AppError> {
    let mut entities = known_entities(connection, request.selection)?;
    if request.purpose != Purpose::Autocomplete {
        entities.retain(|known| !known.is_unconfirmed());
    }
    if request.purpose == Purpose::Autocomplete {
        if let Some(prefix) = request.prefix {
            let words = prefix_words(prefix);
            if words.is_empty() {
                return Ok(Vec::new());
            }
            entities.retain(|known| {
                known
                    .aliases
                    .iter()
                    .any(|alias| begins_with(&alias.normalized, &words))
                    || begins_with(&known.entity.normalized_name, &words)
            });
        }
    }
    let mut ranked: Vec<(f32, KnownEntity)> = entities
        .into_iter()
        .map(|known| (known.weight(), known))
        .collect();
    ranked.sort_by(|(left_weight, left), (right_weight, right)| {
        left.is_candidate()
            .cmp(&right.is_candidate())
            .then(right_weight.total_cmp(left_weight))
            .then(
                left.entity
                    .normalized_name
                    .cmp(&right.entity.normalized_name),
            )
            .then(left.entity.entity_id.cmp(&right.entity.entity_id))
    });

    let cap = match request.purpose {
        Purpose::SpeechHints => request.limit.min(SPEECH_HINTS_MAX_FORMS),
        _ => request.limit.min(MAX_FORMS),
    };
    let mut forms: Vec<SurfaceForm> = Vec::new();
    for (weight, known) in ranked {
        if forms.len() >= cap {
            break;
        }
        let form = SurfaceForm {
            entity_id: known.entity.entity_id,
            display: known.entity.canonical_name.clone(),
            normalized: known.entity.normalized_name.clone(),
            phonetic_key: known.phonetic_key(),
            spoken_form: spoken_form(
                &known.entity.canonical_name,
                known.spoken_form.as_deref(),
                packs,
            ),
            type_id: known.entity.type_id.clone(),
            subtype: known.entity.subtype.clone(),
            weight,
        };
        if request.purpose == Purpose::SpeechHints
            && estimated_tokens(&forms) + estimated_tokens(std::slice::from_ref(&form))
                > SPEECH_HINTS_MAX_TOKENS
        {
            break;
        }
        forms.push(form);
    }
    Ok(forms)
}

/// How many tokens a prompt made of these names takes, **estimated** as the characters of the
/// displays (plus a separator each) divided by [`CHARS_PER_TOKEN`], rounded up. A budget for the
/// speech prompt, not a measurement: a real engine counts with its own tokenizer.
pub fn estimated_tokens(forms: &[SurfaceForm]) -> usize {
    let characters: usize = forms
        .iter()
        .map(|form| form.display.chars().count() + 2)
        .sum();
    characters.div_ceil(CHARS_PER_TOKEN)
}
