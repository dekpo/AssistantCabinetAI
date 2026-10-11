//! The knowledge base's manual side: what a person sees, corrects and extends in the management
//! dialog (`SESSION-KB-04-surfaces.md`, lot 8).
//!
//! Two halves. **Reads** answer "what does the base hold, and why": the list of entities, one
//! entity's sheet with the provenance of every association (the source, the page or the cells, the
//! method, the confidence), the pairs that might be one, the library health counts. **Writes** are
//! the operations a person performs: create, edit, add an alias, merge and unmerge, split, delete and
//! restore, accept or dismiss a possible match, undo the last one, reset.
//!
//! Rules every write obeys:
//!
//! - **One transaction.** A write takes the database's write lock at once (`BEGIN IMMEDIATE`), does
//!   everything, appends one line to `kb_ops_log` and commits; a busy database gives
//!   `knowledge_unavailable` after the 3-second wait and changes nothing.
//! - **Ids only in the log.** The log line holds an operation kind and ids, never a name or a
//!   passage, which is why removing an alias or editing a name cannot be undone: undoing them would
//!   need the text she typed.
//! - **Files untouched.** Nothing here reads or writes a file, a chunk, a document row or a table
//!   row of the index (invariant I11); a deletion is a tombstone and a merge is a redirect.
//! - **Nothing heavy.** A change that adds or removes a name raises the generation of the name list
//!   (`kb_meta.gazetteer_epoch`), which makes the documents due for a new reading; that reading
//!   happens inside the next Analyse the user starts, from the text already stored, and never by
//!   itself (`AGENTS.md`, "No costly silent background work"). The result of every write says how
//!   many documents are due so the interface can tell her.
//! - **Machine codes only.** A refusal is an `AppError` code; a sentence is a catalogue entry.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::types::Value as SqlValue;
use rusqlite::{
    params, params_from_iter, Connection, OptionalExtension, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::gazetteer::Gazetteer;
use super::normalize::{fold, normalize_name, TitleSet};
use super::packs::{available_pack_ids, PackSet};
use super::phonetic::{encoder_for_locale, PhoneticEncoder};
use super::resolve::{alias_phonetic_key, aliases_for, AliasRequest};
use super::store;
use super::{
    AliasKind, Domain, Entity, EntityStatus, EntityTypeId, NewAlias, NewEntity, Origin,
    CURRENT_KB_VERSION,
};
use crate::error::AppError;

/// Rows per page of the list of entities.
pub const PAGE_SIZE: usize = 50;
/// The longest name, alias or value a person can type, in characters.
pub const MAX_TEXT_CHARS: usize = 120;
/// The longest disambiguator, in characters.
pub const MAX_DISAMBIGUATOR_CHARS: usize = 40;
/// The most mentions one entity sheet lists; the sheet says how many there are in all.
pub const MAX_MENTIONS_IN_SHEET: usize = 200;
/// How much of a passage is shown next to a mention, in characters.
pub const EXCERPT_CHARS: usize = 200;
/// How many ids a health count lists (the count itself is exact).
pub const HEALTH_LIST_LIMIT: usize = 50;
/// How many lines of the log are looked at to find the last operation.
const LOG_WINDOW: usize = 400;

/// The types a person may create or give to an entity by hand. An identifier is found by its
/// pattern and a personal one is stored as a keyed hash, so typing one in clear is not offered.
const MANUAL_TYPES: [&str; 5] = ["person", "organization", "location", "item", "term"];

/// The kinds of operation `undo_last_operation` can reverse. Removing an alias, editing a name and
/// setting an attribute are not here on purpose: their inverse needs text, and the log holds ids.
const UNDOABLE: [&str; 8] = [
    "create",
    "add_alias",
    "merge",
    "unmerge",
    "delete",
    "restore",
    "split",
    "dismiss_match",
];

fn db<T>(result: rusqlite::Result<T>) -> Result<T, AppError> {
    result.map_err(|_| AppError::KnowledgeUnavailable)
}

/// The write lock at once, so two writers queue on the busy timeout instead of one failing halfway.
fn begin(connection: &Connection) -> Result<Transaction<'_>, AppError> {
    db(Transaction::new_unchecked(
        connection,
        TransactionBehavior::Immediate,
    ))
}

fn commit(tx: Transaction<'_>) -> Result<(), AppError> {
    db(tx.commit())
}

// ---------------------------------------------------------------------------------------------
// The words a name is read with
// ---------------------------------------------------------------------------------------------

/// What a manual name needs to become an alias the scanner can find: the titles to leave out, the
/// phonetic encoder, and the identifier kinds that point at one person (never typed in clear).
pub struct NameTools {
    titles: TitleSet,
    encoder: Box<dyn PhoneticEncoder>,
    personal_kinds: BTreeSet<String>,
}

impl NameTools {
    pub fn from_packs(packs: &PackSet, locale: &str) -> Self {
        Self {
            titles: packs.title_set(),
            encoder: encoder_for_locale(locale),
            personal_kinds: packs
                .identifier_schemes()
                .iter()
                .filter(|scheme| scheme.personal_key)
                .map(|scheme| scheme.kind.clone())
                .collect(),
        }
    }

    pub fn new(
        titles: TitleSet,
        encoder: Box<dyn PhoneticEncoder>,
        personal_kinds: BTreeSet<String>,
    ) -> Self {
        Self {
            titles,
            encoder,
            personal_kinds,
        }
    }

    /// The folded form a name is compared by. `Err` when nothing is left once titles are removed.
    fn normalized(&self, text: &str) -> Result<String, AppError> {
        let joined = normalize_name(text, &self.titles).joined;
        if joined.is_empty() {
            Err(AppError::KnowledgeNameInvalid)
        } else {
            Ok(joined)
        }
    }

    fn manual_alias(&self, display: &str, normalized: String) -> NewAlias {
        NewAlias {
            display: display.to_string(),
            phonetic_key: alias_phonetic_key(
                display,
                AliasKind::Manual,
                &self.titles,
                self.encoder.as_ref(),
            ),
            normalized,
            kind: AliasKind::Manual,
            confidence: 1.0,
        }
    }
}

/// Text as typed, tidied: one space between words, nothing at the ends, no control character.
fn tidy(text: &str, limit: usize) -> Result<String, AppError> {
    if text.chars().any(char::is_control) {
        return Err(AppError::KnowledgeNameInvalid);
    }
    let tidy = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if tidy.is_empty() || tidy.chars().count() > limit {
        return Err(AppError::KnowledgeNameInvalid);
    }
    Ok(tidy)
}

fn manual_type(type_id: &str) -> Result<&str, AppError> {
    if MANUAL_TYPES.contains(&type_id) {
        Ok(type_id)
    } else {
        Err(AppError::KnowledgeOperationRefused)
    }
}

// ---------------------------------------------------------------------------------------------
// What every write reports
// ---------------------------------------------------------------------------------------------

/// The result of a write: the ids it produced, and how many documents the next Analyse will read
/// again because of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub entity_id: Option<i64>,
    pub alias_id: Option<i64>,
    /// For a new alias: how many other live entities answer to the same name already. The sheet
    /// says so, because a name shared by two people is never read as either of them alone.
    pub shared_with: usize,
    pub documents_due: usize,
}

fn current_epoch(connection: &Connection) -> Result<u32, AppError> {
    Ok(store::meta(connection, "gazetteer_epoch")?
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0))
}

/// How many documents are behind the current list of names, or were never read: what the next
/// Analyse will read again from the stored text.
pub fn documents_due(connection: &Connection) -> Result<usize, AppError> {
    Ok(store::documents_due(connection, CURRENT_KB_VERSION, current_epoch(connection)?)?.len())
}

/// A name was added, removed or changed: the documents read against the old list are behind it.
fn names_changed(tx: &Transaction<'_>) -> Result<(), AppError> {
    Gazetteer::new().bump_epoch(tx)?;
    Ok(())
}

fn finish(
    tx: Transaction<'_>,
    entity_id: Option<i64>,
    alias_id: Option<i64>,
    shared_with: usize,
) -> Result<Outcome, AppError> {
    commit(tx)?;
    Ok(Outcome {
        entity_id,
        alias_id,
        shared_with,
        documents_due: 0,
    })
}

/// `finish`, then the count of documents due, read on the caller's connection after the commit.
fn done(
    connection: &Connection,
    tx: Transaction<'_>,
    entity_id: Option<i64>,
    alias_id: Option<i64>,
    shared_with: usize,
) -> Result<Outcome, AppError> {
    let mut outcome = finish(tx, entity_id, alias_id, shared_with)?;
    outcome.documents_due = documents_due(connection).unwrap_or(0);
    Ok(outcome)
}

fn live_entity(connection: &Connection, entity_id: i64) -> Result<Entity, AppError> {
    match store::get_entity(connection, entity_id)? {
        None => Err(AppError::KnowledgeEntityNotFound),
        Some(entity)
            if matches!(
                entity.status,
                EntityStatus::Candidate | EntityStatus::Active
            ) =>
        {
            Ok(entity)
        }
        Some(_) => Err(AppError::KnowledgeOperationRefused),
    }
}

/// Is this (type, name, disambiguator) held by an entity other than `except`, whatever its status?
fn name_is_held(
    connection: &Connection,
    type_id: &str,
    normalized: &str,
    disambiguator: &str,
    except: Option<i64>,
) -> Result<bool, AppError> {
    db(connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM kb_entities
                        WHERE type_id = ?1 AND normalized_name = ?2 AND disambiguator = ?3
                          AND entity_id <> ?4)",
        params![type_id, normalized, disambiguator, except.unwrap_or(-1)],
        |row| row.get(0),
    ))
}

// ---------------------------------------------------------------------------------------------
// Create, edit, aliases, attributes
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewManualEntity {
    pub name: String,
    pub type_id: String,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub disambiguator: Option<String>,
}

/// A name typed by hand. It is `active` at once (a person said so) and `manual`, so no re-analysis
/// removes it; the aliases a name of its kind gets are written as manual too, and the next Analyse
/// finds it in the documents that name it.
pub fn create_entity(
    connection: &Connection,
    tools: &NameTools,
    input: &NewManualEntity,
) -> Result<Outcome, AppError> {
    let type_id = manual_type(&input.type_id)?;
    let name = tidy(&input.name, MAX_TEXT_CHARS)?;
    let normalized = tools.normalized(&name)?;
    let disambiguator = match &input.disambiguator {
        Some(text) if !text.trim().is_empty() => tidy(text, MAX_DISAMBIGUATOR_CHARS)?,
        _ => String::new(),
    };
    let subtype = match input.subtype.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(code) => Some(
            EntityTypeId::new(code)
                .ok_or(AppError::KnowledgeNameInvalid)?
                .as_str()
                .to_string(),
        ),
    };

    let tx = begin(connection)?;
    if name_is_held(&tx, type_id, &normalized, &disambiguator, None)? {
        return Err(AppError::KnowledgeNameTaken);
    }
    let entity_id = store::create_entity(
        &tx,
        &NewEntity {
            type_id: type_id.to_string(),
            subtype,
            canonical_name: name.clone(),
            normalized_name: normalized.clone(),
            disambiguator,
            status: EntityStatus::Active,
            origin: Origin::Manual,
        },
    )?;
    write_name_aliases(&tx, tools, entity_id, type_id, &name, &normalized)?;
    store::log_op(&tx, "create", json!({ "entity": entity_id }))?;
    names_changed(&tx)?;
    done(connection, tx, Some(entity_id), None, 0)
}

/// The canonical form of a name and the short forms a name of its kind answers to, all manual.
fn write_name_aliases(
    tx: &Transaction<'_>,
    tools: &NameTools,
    entity_id: i64,
    type_id: &str,
    display: &str,
    normalized: &str,
) -> Result<(), AppError> {
    let type_id = EntityTypeId::new(type_id).ok_or(AppError::Internal)?;
    let made = aliases_for(
        &AliasRequest {
            type_id: &type_id,
            canonical: display,
            titles_seen: &[],
            acronym: None,
        },
        &tools.titles,
        tools.encoder.as_ref(),
    );
    let mut wrote_canonical = false;
    for alias in made {
        wrote_canonical |= alias.normalized == normalized;
        store::add_alias(tx, entity_id, &alias, Origin::Manual)?;
    }
    if !wrote_canonical {
        store::add_alias(
            tx,
            entity_id,
            &tools.manual_alias(display, normalized.to_string()),
            Origin::Manual,
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditEntity {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub type_id: Option<String>,
    #[serde(default)]
    pub disambiguator: Option<String>,
}

/// Change an entity's name, type or disambiguator. An automatic entity becomes manual (the person
/// has spoken: no re-analysis garbage-collects it); its old name stays as an alias, so the documents
/// that still write the old name keep leading to it.
pub fn edit_entity(
    connection: &Connection,
    tools: &NameTools,
    entity_id: i64,
    edit: &EditEntity,
) -> Result<Outcome, AppError> {
    let tx = begin(connection)?;
    let entity = live_entity(&tx, entity_id)?;

    let type_id = match edit.type_id.as_deref() {
        None => entity.type_id.clone(),
        Some(code) if code == entity.type_id => code.to_string(),
        Some(code) => {
            // Neither to nor from an identifier: those are found by pattern and hashed when personal.
            if entity.type_id == EntityTypeId::IDENTIFIER {
                return Err(AppError::KnowledgeOperationRefused);
            }
            manual_type(code)?.to_string()
        }
    };
    let name = match edit.name.as_deref() {
        None => entity.canonical_name.clone(),
        Some(text) => tidy(text, MAX_TEXT_CHARS)?,
    };
    let disambiguator = match edit.disambiguator.as_deref() {
        None => entity.disambiguator.clone(),
        Some(text) if text.trim().is_empty() => String::new(),
        Some(text) => tidy(text, MAX_DISAMBIGUATOR_CHARS)?,
    };
    if entity.type_id == EntityTypeId::IDENTIFIER && name != entity.canonical_name {
        return Err(AppError::KnowledgeOperationRefused);
    }
    let normalized = if entity.type_id == EntityTypeId::IDENTIFIER {
        entity.normalized_name.clone()
    } else {
        tools.normalized(&name)?
    };

    let unchanged = type_id == entity.type_id
        && name == entity.canonical_name
        && disambiguator == entity.disambiguator;
    if unchanged {
        return done(connection, tx, Some(entity_id), None, 0);
    }
    if name_is_held(&tx, &type_id, &normalized, &disambiguator, Some(entity_id))? {
        return Err(AppError::KnowledgeNameTaken);
    }

    let renamed = name != entity.canonical_name;
    if renamed {
        // The old name keeps leading here.
        let old_normalized = entity.normalized_name.clone();
        let old = tools.manual_alias(&entity.canonical_name, old_normalized);
        if entity.type_id != EntityTypeId::IDENTIFIER {
            store::add_alias(&tx, entity_id, &old, Origin::Manual)?;
        }
    }
    db(tx.execute(
        "UPDATE kb_entities SET type_id = ?1, canonical_name = ?2, normalized_name = ?3,
                disambiguator = ?4, origin = 'manual',
                updated_at = CAST(strftime('%s', 'now') AS INTEGER)
         WHERE entity_id = ?5",
        params![type_id, name, normalized, disambiguator, entity_id],
    ))?;
    if renamed && entity.type_id != EntityTypeId::IDENTIFIER {
        write_name_aliases(&tx, tools, entity_id, &type_id, &name, &normalized)?;
    }
    store::log_op(&tx, "edit", json!({ "entity": entity_id }))?;
    names_changed(&tx)?;
    done(connection, tx, Some(entity_id), None, 0)
}

/// Add a name an entity answers to. The next Analyse reads the documents again with it; a name the
/// entity already answers to (by hand or by itself) is left as it is.
pub fn add_alias(
    connection: &Connection,
    tools: &NameTools,
    entity_id: i64,
    text: &str,
) -> Result<Outcome, AppError> {
    let display = tidy(text, MAX_TEXT_CHARS)?;
    let normalized = tools.normalized(&display)?;
    let tx = begin(connection)?;
    let entity = live_entity(&tx, entity_id)?;
    if entity.type_id == EntityTypeId::IDENTIFIER {
        return Err(AppError::KnowledgeOperationRefused);
    }
    let existing: Option<i64> = db(tx
        .query_row(
            "SELECT alias_id FROM kb_aliases WHERE entity_id = ?1 AND normalized = ?2",
            params![entity_id, normalized],
            |row| row.get(0),
        )
        .optional())?;
    if let Some(alias_id) = existing {
        return done(connection, tx, Some(entity_id), Some(alias_id), 0);
    }
    let shared_with = store::find_entities_by_alias(&tx, &normalized)?
        .into_iter()
        .map(|(other, _)| other.entity_id)
        .chain(
            store::find_entities_by_normalized(&tx, &normalized)?
                .into_iter()
                .map(|other| other.entity_id),
        )
        .filter(|other| *other != entity_id)
        .collect::<BTreeSet<_>>()
        .len();
    let alias_id = store::add_alias(
        &tx,
        entity_id,
        &tools.manual_alias(&display, normalized),
        Origin::Manual,
    )?;
    store::log_op(
        &tx,
        "add_alias",
        json!({ "entity": entity_id, "alias": alias_id }),
    )?;
    names_changed(&tx)?;
    done(connection, tx, Some(entity_id), Some(alias_id), shared_with)
}

/// Remove an alias a person added. An automatic alias is not hers to edit: the next Analyse would
/// write it again.
pub fn remove_alias(connection: &Connection, alias_id: i64) -> Result<Outcome, AppError> {
    let tx = begin(connection)?;
    let row: Option<(i64, String)> = db(tx
        .query_row(
            "SELECT entity_id, origin FROM kb_aliases WHERE alias_id = ?1",
            [alias_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional())?;
    let Some((entity_id, origin)) = row else {
        return Err(AppError::KnowledgeEntityNotFound);
    };
    if origin != "manual" {
        return Err(AppError::KnowledgeOperationRefused);
    }
    store::remove_alias(&tx, alias_id)?;
    store::log_op(
        &tx,
        "remove_alias",
        json!({ "entity": entity_id, "alias": alias_id }),
    )?;
    names_changed(&tx)?;
    done(connection, tx, Some(entity_id), None, 0)
}

/// State a fact about an entity by hand (`spoken_form`, a phone number...). The key is an open
/// vocabulary of machine ids; a key that names a personal identifier is refused, because such a
/// value is only ever stored as a keyed hash.
pub fn set_attribute(
    connection: &Connection,
    tools: &NameTools,
    entity_id: i64,
    key: &str,
    value: &str,
) -> Result<Outcome, AppError> {
    let key = key.trim();
    if EntityTypeId::new(key).is_none() {
        return Err(AppError::KnowledgeNameInvalid);
    }
    if key.starts_with("identifier") || tools.personal_kinds.contains(key) {
        return Err(AppError::KnowledgeOperationRefused);
    }
    let value = tidy(value, MAX_TEXT_CHARS)?;
    let tx = begin(connection)?;
    live_entity(&tx, entity_id)?;
    store::set_attribute(&tx, entity_id, key, &value, Origin::Manual, None)?;
    store::log_op(
        &tx,
        "set_attribute",
        json!({ "entity": entity_id, "key": key }),
    )?;
    done(connection, tx, Some(entity_id), None, 0)
}

pub fn remove_attribute(
    connection: &Connection,
    entity_id: i64,
    key: &str,
    value: &str,
) -> Result<Outcome, AppError> {
    let tx = begin(connection)?;
    if store::get_entity(&tx, entity_id)?.is_none() {
        return Err(AppError::KnowledgeEntityNotFound);
    }
    let removed = store::remove_attribute(
        &tx,
        entity_id,
        key.trim(),
        &value.split_whitespace().collect::<Vec<_>>().join(" "),
    )?;
    if removed == 0 {
        return Err(AppError::KnowledgeOperationRefused);
    }
    store::log_op(
        &tx,
        "remove_attribute",
        json!({ "entity": entity_id, "key": key.trim() }),
    )?;
    done(connection, tx, Some(entity_id), None, 0)
}

// ---------------------------------------------------------------------------------------------
// Merge, unmerge, delete, restore
// ---------------------------------------------------------------------------------------------

/// Pairs that were open and are now one entity are `merged`; pairs that were `merged` and are two
/// again are open once more. The pair's state follows the entities, whichever way it changed.
fn sync_pair_states(tx: &Transaction<'_>) -> Result<(), AppError> {
    db(tx.execute_batch(
        "UPDATE kb_possible_matches SET state = 'merged'
         WHERE state = 'open'
           AND (SELECT effective_id FROM kb_effective_entity WHERE entity_id = entity_a)
             = (SELECT effective_id FROM kb_effective_entity WHERE entity_id = entity_b);
         UPDATE kb_possible_matches SET state = 'open'
         WHERE state = 'merged'
           AND (SELECT effective_id FROM kb_effective_entity WHERE entity_id = entity_a)
            <> (SELECT effective_id FROM kb_effective_entity WHERE entity_id = entity_b);",
    ))
}

/// Make `absorbed` a redirect to `survivor`: nothing is moved, both stay readable through the
/// survivor, and the merge can be undone. Refused for two different types, the same entity, or a
/// tombstone.
pub fn merge(connection: &Connection, survivor: i64, absorbed: i64) -> Result<Outcome, AppError> {
    let tx = begin(connection)?;
    store::merge_entities(&tx, survivor, absorbed)?;
    sync_pair_states(&tx)?;
    names_changed(&tx)?;
    done(connection, tx, Some(survivor), None, 0)
}

/// Undo a merge: the absorbed entity is itself again.
pub fn unmerge(connection: &Connection, entity_id: i64) -> Result<Outcome, AppError> {
    let tx = begin(connection)?;
    store::unmerge_entity(&tx, entity_id)?;
    sync_pair_states(&tx)?;
    names_changed(&tx)?;
    done(connection, tx, Some(entity_id), None, 0)
}

/// A tombstone: the entity leaves every list and its mentions go, its name stays as a suppression so
/// the next Analyse does not bring a false positive back. No file, chunk or table row is touched.
pub fn delete_entity(connection: &Connection, entity_id: i64) -> Result<Outcome, AppError> {
    let tx = begin(connection)?;
    store::delete_entity(&tx, entity_id)?;
    names_changed(&tx)?;
    done(connection, tx, Some(entity_id), None, 0)
}

/// Bring a tombstone back. Its mentions return when the documents that named it are read again by
/// the next Analyse.
///
/// The entity becomes manual: a person vouched for it, and an automatic entity with no mention would
/// be collected as an orphan before the next Analyse had a chance to find its documents again.
pub fn restore_entity(connection: &Connection, entity_id: i64) -> Result<Outcome, AppError> {
    let tx = begin(connection)?;
    restore_and_keep(&tx, entity_id)?;
    names_changed(&tx)?;
    done(connection, tx, Some(entity_id), None, 0)
}

fn restore_and_keep(tx: &Transaction<'_>, entity_id: i64) -> Result<(), AppError> {
    store::restore_entity(tx, entity_id)?;
    db(tx.execute(
        "UPDATE kb_entities SET origin = 'manual' WHERE entity_id = ?1",
        [entity_id],
    ))?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Possible matches
// ---------------------------------------------------------------------------------------------

fn ordered(one: i64, other: i64) -> (i64, i64) {
    (one.min(other), one.max(other))
}

fn open_pair_exists(connection: &Connection, one: i64, other: i64) -> Result<bool, AppError> {
    let (low, high) = ordered(one, other);
    db(connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM kb_possible_matches
                        WHERE entity_a = ?1 AND entity_b = ?2 AND state = 'open')",
        params![low, high],
        |row| row.get(0),
    ))
}

/// The pair was a real match: merge the two. Refused (and the pair stays open) when the merge is.
pub fn accept_possible_match(
    connection: &Connection,
    survivor: i64,
    absorbed: i64,
) -> Result<Outcome, AppError> {
    if !open_pair_exists(connection, survivor, absorbed)? {
        return Err(AppError::KnowledgeOperationRefused);
    }
    merge(connection, survivor, absorbed)
}

/// The pair is not a match: it is never offered again.
pub fn dismiss_possible_match(
    connection: &Connection,
    one: i64,
    other: i64,
) -> Result<Outcome, AppError> {
    let (low, high) = ordered(one, other);
    let tx = begin(connection)?;
    let changed = db(tx.execute(
        "UPDATE kb_possible_matches SET state = 'dismissed'
         WHERE entity_a = ?1 AND entity_b = ?2 AND state = 'open'",
        params![low, high],
    ))?;
    if changed == 0 {
        return Err(AppError::KnowledgeOperationRefused);
    }
    store::log_op(&tx, "dismiss_match", json!({ "a": low, "b": high }))?;
    done(connection, tx, None, None, 0)
}

// ---------------------------------------------------------------------------------------------
// Split
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitRequest {
    pub entity_id: i64,
    /// The mentions to move, by id.
    #[serde(default)]
    pub mention_ids: Vec<i64>,
    /// Or every mention of the entity in these sources.
    #[serde(default)]
    pub source_ids: Vec<i64>,
    /// The new entity's name. It must differ from every name already held: two entities that share
    /// a name are one ambiguity for the scanner, which would then place neither.
    pub name: String,
}

struct Move {
    mention_id: i64,
    from_entity: i64,
    alias_id: Option<i64>,
    origin: String,
}

/// Homonyms: some mentions of an entity belong to another person who happens to be written the
/// same. The chosen mentions move to a new, manual entity; each keeps a note of where it came from,
/// both in the log (so the split can be undone) and on the mention itself (so a re-analysis does not
/// put it back). Refused when it would leave the entity with nothing.
pub fn split(
    connection: &Connection,
    tools: &NameTools,
    request: &SplitRequest,
) -> Result<Outcome, AppError> {
    let name = tidy(&request.name, MAX_TEXT_CHARS)?;
    let normalized = tools.normalized(&name)?;

    let tx = begin(connection)?;
    let entity = live_entity(&tx, request.entity_id)?;
    if entity.type_id == EntityTypeId::IDENTIFIER {
        return Err(AppError::KnowledgeOperationRefused);
    }

    // Every mention of the entity (merged ones included), by id.
    let mut owned: BTreeMap<i64, Move> = BTreeMap::new();
    let mut by_source: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    {
        let mut statement = db(tx.prepare(
            "SELECT m.mention_id, m.entity_id, m.alias_id, m.origin, m.source_id
             FROM kb_mentions m
             JOIN kb_effective_entity v ON v.entity_id = m.entity_id
             WHERE v.effective_id = ?1 ORDER BY m.mention_id",
        ))?;
        let rows = db(statement.query_map([entity.entity_id], |row| {
            Ok((
                Move {
                    mention_id: row.get(0)?,
                    from_entity: row.get(1)?,
                    alias_id: row.get(2)?,
                    origin: row.get(3)?,
                },
                row.get::<_, i64>(4)?,
            ))
        }))?;
        for row in rows {
            let (moved, source_id) = db(row)?;
            by_source
                .entry(source_id)
                .or_default()
                .push(moved.mention_id);
            owned.insert(moved.mention_id, moved);
        }
    }

    let mut chosen: BTreeSet<i64> = BTreeSet::new();
    for mention_id in &request.mention_ids {
        if !owned.contains_key(mention_id) {
            return Err(AppError::KnowledgeOperationRefused);
        }
        chosen.insert(*mention_id);
    }
    for source_id in &request.source_ids {
        if let Some(ids) = by_source.get(source_id) {
            chosen.extend(ids.iter().copied());
        }
    }
    if chosen.is_empty() || chosen.len() >= owned.len() {
        return Err(AppError::KnowledgeOperationRefused);
    }
    if name_is_held(&tx, &entity.type_id, &normalized, "", None)? {
        return Err(AppError::KnowledgeNameTaken);
    }

    let new_id = store::create_entity(
        &tx,
        &NewEntity {
            type_id: entity.type_id.clone(),
            subtype: entity.subtype.clone(),
            canonical_name: name.clone(),
            normalized_name: normalized.clone(),
            disambiguator: String::new(),
            status: EntityStatus::Active,
            origin: Origin::Manual,
        },
    )?;
    // Only the name itself: a surname or an initial would collide with the entity it was split from.
    store::add_alias(
        &tx,
        new_id,
        &tools.manual_alias(&name, normalized),
        Origin::Manual,
    )?;

    let mut record = Vec::with_capacity(chosen.len());
    for mention_id in &chosen {
        let moved = &owned[mention_id];
        db(tx.execute(
            "UPDATE kb_mentions SET entity_id = ?1, alias_id = NULL, origin = 'manual',
                    split_from = ?2
             WHERE mention_id = ?3",
            params![new_id, moved.from_entity, mention_id],
        ))?;
        record.push(json!({
            "mention": moved.mention_id,
            "from": moved.from_entity,
            "alias": moved.alias_id,
            "origin": moved.origin,
        }));
    }
    store::log_op(
        &tx,
        "split",
        json!({ "entity": entity.entity_id, "to": new_id, "moves": record }),
    )?;
    names_changed(&tx)?;
    done(connection, tx, Some(new_id), None, 0)
}

// ---------------------------------------------------------------------------------------------
// The log, and undo
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastOperation {
    pub op_id: i64,
    /// A machine code (`merge`, `split`, `delete`...). The interface names it.
    pub kind: String,
    pub undoable: bool,
    pub at: i64,
}

struct LogLine {
    op_id: i64,
    kind: String,
    payload: Value,
    at: i64,
}

fn recent_log(connection: &Connection) -> Result<Vec<LogLine>, AppError> {
    let mut statement = db(connection.prepare(
        "SELECT op_id, kind, payload_json, at FROM kb_ops_log ORDER BY op_id DESC LIMIT ?1",
    ))?;
    let rows = db(statement.query_map([LOG_WINDOW as i64], |row| {
        let payload: String = row.get(2)?;
        Ok(LogLine {
            op_id: row.get(0)?,
            kind: row.get(1)?,
            payload: serde_json::from_str(&payload).unwrap_or(Value::Null),
            at: row.get(3)?,
        })
    }))?;
    db(rows.collect())
}

/// The newest line of the log that is neither an undo, nor undone, nor written by an undo.
fn newest_operation(connection: &Connection) -> Result<Option<LogLine>, AppError> {
    let mut ignored: BTreeSet<i64> = BTreeSet::new();
    for line in recent_log(connection)? {
        if line.kind == "undo" {
            if let Some(id) = line.payload["of"].as_i64() {
                ignored.insert(id);
            }
            if let Some(caused) = line.payload["caused"].as_array() {
                ignored.extend(caused.iter().filter_map(Value::as_i64));
            }
            continue;
        }
        if ignored.contains(&line.op_id) {
            continue;
        }
        return Ok(Some(line));
    }
    Ok(None)
}

pub fn last_operation(connection: &Connection) -> Result<Option<LastOperation>, AppError> {
    Ok(newest_operation(connection)?.map(|line| LastOperation {
        op_id: line.op_id,
        undoable: UNDOABLE.contains(&line.kind.as_str()),
        kind: line.kind,
        at: line.at,
    }))
}

fn max_op_id(connection: &Connection) -> Result<i64, AppError> {
    db(connection.query_row(
        "SELECT COALESCE(MAX(op_id), 0) FROM kb_ops_log",
        [],
        |row| row.get(0),
    ))
}

fn id_of(payload: &Value, key: &str) -> Result<i64, AppError> {
    payload[key]
        .as_i64()
        .ok_or(AppError::KnowledgeNothingToUndo)
}

/// Reverse the last operation, if it can be reversed: merge, unmerge, delete, restore, split, a
/// created entity, an added alias, a dismissed pair. The one before it can be undone next. Anything
/// the base has moved on from (an entity that has since been renamed, deleted or merged
/// elsewhere) is refused with `knowledge_operation_refused` and nothing changes.
pub fn undo_last_operation(connection: &Connection) -> Result<Outcome, AppError> {
    let tx = begin(connection)?;
    let Some(line) = newest_operation(&tx)? else {
        return Err(AppError::KnowledgeNothingToUndo);
    };
    if !UNDOABLE.contains(&line.kind.as_str()) {
        return Err(AppError::KnowledgeNothingToUndo);
    }
    let before = max_op_id(&tx)?;
    let mut entity_id: Option<i64> = None;
    let mut changes_names = true;

    match line.kind.as_str() {
        "create" => {
            let id = id_of(&line.payload, "entity")?;
            let held: i64 = db(tx.query_row(
                "SELECT (SELECT COUNT(*) FROM kb_mentions WHERE entity_id = ?1)
                      + (SELECT COUNT(*) FROM kb_relations WHERE subject = ?1 OR object = ?1)
                      + (SELECT COUNT(*) FROM kb_entities WHERE merged_into = ?1)",
                [id],
                |row| row.get(0),
            ))?;
            if held > 0 || store::get_entity(&tx, id)?.is_none() {
                return Err(AppError::KnowledgeOperationRefused);
            }
            db(tx.execute("DELETE FROM kb_entities WHERE entity_id = ?1", [id]))?;
        }
        "add_alias" => {
            let alias = id_of(&line.payload, "alias")?;
            entity_id = line.payload["entity"].as_i64();
            if !store::remove_alias(&tx, alias)? {
                return Err(AppError::KnowledgeOperationRefused);
            }
        }
        "merge" => {
            let victim = id_of(&line.payload, "victim")?;
            entity_id = Some(victim);
            store::unmerge_entity(&tx, victim).map_err(|_| AppError::KnowledgeOperationRefused)?;
            sync_pair_states(&tx)?;
        }
        "unmerge" => {
            let entity = id_of(&line.payload, "entity")?;
            let survivor = id_of(&line.payload, "survivor")?;
            entity_id = Some(survivor);
            store::merge_entities(&tx, survivor, entity)
                .map_err(|_| AppError::KnowledgeOperationRefused)?;
            sync_pair_states(&tx)?;
        }
        "delete" => {
            let entity = id_of(&line.payload, "entity")?;
            entity_id = Some(entity);
            restore_and_keep(&tx, entity).map_err(|_| AppError::KnowledgeOperationRefused)?;
        }
        "restore" => {
            let entity = id_of(&line.payload, "entity")?;
            entity_id = Some(entity);
            store::delete_entity(&tx, entity).map_err(|_| AppError::KnowledgeOperationRefused)?;
        }
        "dismiss_match" => {
            let (a, b) = (id_of(&line.payload, "a")?, id_of(&line.payload, "b")?);
            let changed = db(tx.execute(
                "UPDATE kb_possible_matches SET state = 'open'
                 WHERE entity_a = ?1 AND entity_b = ?2 AND state = 'dismissed'",
                params![a, b],
            ))?;
            if changed == 0 {
                return Err(AppError::KnowledgeOperationRefused);
            }
            changes_names = false;
        }
        "split" => {
            entity_id = Some(undo_split(&tx, &line.payload)?);
        }
        _ => return Err(AppError::KnowledgeNothingToUndo),
    }

    let caused: Vec<i64> = {
        let mut statement =
            db(tx.prepare("SELECT op_id FROM kb_ops_log WHERE op_id > ?1 ORDER BY op_id"))?;
        let rows = db(statement.query_map([before], |row| row.get(0)))?;
        db(rows.collect())?
    };
    store::log_op(&tx, "undo", json!({ "of": line.op_id, "caused": caused }))?;
    if changes_names {
        names_changed(&tx)?;
    }
    done(connection, tx, entity_id, None, 0)
}

/// Put every moved mention back where it came from, then drop the entity the split made. Refused
/// when the new entity has since gained anything of its own.
fn undo_split(tx: &Transaction<'_>, payload: &Value) -> Result<i64, AppError> {
    let from = id_of(payload, "entity")?;
    let to = id_of(payload, "to")?;
    let moves = payload["moves"]
        .as_array()
        .ok_or(AppError::KnowledgeNothingToUndo)?;
    let moved_ids: BTreeSet<i64> = moves
        .iter()
        .filter_map(|line| line["mention"].as_i64())
        .collect();

    let on_new: BTreeSet<i64> = {
        let mut statement =
            db(tx.prepare("SELECT mention_id FROM kb_mentions WHERE entity_id = ?1"))?;
        let rows = db(statement.query_map([to], |row| row.get::<_, i64>(0)))?;
        db(rows.collect::<Result<_, _>>())?
    };
    let related: i64 = db(tx.query_row(
        "SELECT (SELECT COUNT(*) FROM kb_relations WHERE subject = ?1 OR object = ?1)
              + (SELECT COUNT(*) FROM kb_entities WHERE merged_into = ?1)",
        [to],
        |row| row.get(0),
    ))?;
    if related > 0 || !on_new.is_subset(&moved_ids) || store::get_entity(tx, to)?.is_none() {
        return Err(AppError::KnowledgeOperationRefused);
    }

    for line in moves {
        let (Some(mention), Some(back)) = (line["mention"].as_i64(), line["from"].as_i64()) else {
            continue;
        };
        if !on_new.contains(&mention) {
            continue;
        }
        let origin = match line["origin"].as_str() {
            Some("manual") => "manual",
            _ => "automatic",
        };
        let alias = line["alias"].as_i64().filter(|alias| {
            tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM kb_aliases WHERE alias_id = ?1 AND entity_id = ?2)",
                params![alias, back],
                |row| row.get::<_, bool>(0),
            )
            .unwrap_or(false)
        });
        db(tx.execute(
            "UPDATE kb_mentions SET entity_id = ?1, alias_id = ?2, origin = ?3, split_from = NULL
             WHERE mention_id = ?4",
            params![back, alias, origin, mention],
        ))?;
    }
    db(tx.execute("DELETE FROM kb_entities WHERE entity_id = ?1", [to]))?;
    Ok(from)
}

// ---------------------------------------------------------------------------------------------
// Reset
// ---------------------------------------------------------------------------------------------

/// "Reset the whole knowledge base". Without `include_manual` everything a pass derived goes (the
/// sources and what only they held up) and what a person wrote stays: manual entities, aliases and
/// attributes, and the tombstones, which keep a deleted false positive from coming back. With it,
/// the base is emptied, her rows, the tombstones and the log too. Files, chunks and inventories are
/// never touched. The documents are due again afterwards: the next Analyse reads them from the text
/// already stored.
pub fn reset(connection: &Connection, include_manual: bool) -> Result<Outcome, AppError> {
    let tx = begin(connection)?;
    if include_manual {
        store::clear_everything(&tx)?;
    } else {
        store::clear_all_automatic(&tx)?;
        // A barrier: nothing before it can be undone, the entities it names may be gone.
        store::log_op(&tx, "reset", json!({ "include_manual": false }))?;
    }
    names_changed(&tx)?;
    done(connection, tx, None, None, 0)
}

// ---------------------------------------------------------------------------------------------
// Reads: the list
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListView {
    /// Candidates and active entities.
    #[default]
    Live,
    /// Tombstones: what she deleted.
    Deleted,
    /// Entities redirected into another.
    Merged,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub type_id: Option<String>,
    /// `automatic` or `manual`.
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub view: ListView,
    /// Only what deserves a look: candidates, entities with a pair to decide on, entities seen once.
    #[serde(default)]
    pub problems_only: bool,
    #[serde(default)]
    pub page: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityRow {
    pub entity_id: i64,
    pub type_id: String,
    pub subtype: Option<String>,
    pub name: String,
    pub disambiguator: String,
    pub status: String,
    pub origin: String,
    pub merged_into: Option<i64>,
    pub mentions: usize,
    pub sources: usize,
    pub aliases: usize,
    pub open_matches: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityPage {
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub rows: Vec<EntityRow>,
}

fn escape_like(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// The words of a typed search, folded: letters and digits only.
fn search_words(text: &str) -> Vec<String> {
    fold(text)
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

const ROW_COLUMNS: &str = "e.entity_id, e.type_id, e.subtype, e.canonical_name, e.disambiguator,
        e.status, e.origin, e.merged_into, e.normalized_name,
        (SELECT COUNT(*) FROM kb_mentions m JOIN kb_effective_entity v ON v.entity_id = m.entity_id
          WHERE v.effective_id = e.entity_id) AS mentions,
        (SELECT COUNT(DISTINCT m.source_id) FROM kb_mentions m
          JOIN kb_effective_entity v ON v.entity_id = m.entity_id
          WHERE v.effective_id = e.entity_id) AS sources,
        (SELECT COUNT(*) FROM kb_aliases a WHERE a.entity_id = e.entity_id) AS aliases,
        (SELECT COUNT(*) FROM kb_possible_matches p
          WHERE p.state = 'open' AND (p.entity_a = e.entity_id OR p.entity_b = e.entity_id))
          AS open_matches";

fn entity_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EntityRow> {
    Ok(EntityRow {
        entity_id: row.get(0)?,
        type_id: row.get(1)?,
        subtype: row.get(2)?,
        name: row.get(3)?,
        disambiguator: row.get(4)?,
        status: row.get(5)?,
        origin: row.get(6)?,
        merged_into: row.get(7)?,
        mentions: row.get::<_, i64>(9)? as usize,
        sources: row.get::<_, i64>(10)? as usize,
        aliases: row.get::<_, i64>(11)? as usize,
        open_matches: row.get::<_, i64>(12)? as usize,
    })
}

/// A page of entities, filtered. Text is matched on the start of any alias word (the names index)
/// and anywhere in the folded name; the order is the folded name, then the id, so a page never
/// shifts under her.
pub fn list_entities(connection: &Connection, query: &ListQuery) -> Result<EntityPage, AppError> {
    let mut conditions: Vec<String> = Vec::new();
    let mut values: Vec<SqlValue> = Vec::new();

    conditions.push(
        match query.view {
            ListView::Live => "e.status IN ('candidate', 'active')",
            ListView::Deleted => "e.status = 'deleted'",
            ListView::Merged => "e.status = 'merged'",
        }
        .to_string(),
    );
    if let Some(type_id) = query.type_id.as_deref().filter(|code| !code.is_empty()) {
        conditions.push("e.type_id = ?".to_string());
        values.push(SqlValue::Text(type_id.to_string()));
    }
    if let Some(origin) = query.origin.as_deref().filter(|code| !code.is_empty()) {
        if origin != "automatic" && origin != "manual" {
            return Err(AppError::KnowledgeOperationRefused);
        }
        conditions.push("e.origin = ?".to_string());
        values.push(SqlValue::Text(origin.to_string()));
    }
    let words = search_words(&query.text);
    if words.is_empty() && !query.text.trim().is_empty() {
        // Something was typed but holds no letter or digit: nothing can match it.
        conditions.push("0".to_string());
    }
    if !words.is_empty() {
        let last = words.len() - 1;
        let expression = words
            .iter()
            .enumerate()
            .map(|(index, word)| {
                if index == last {
                    format!("\"{word}\"*")
                } else {
                    format!("\"{word}\"")
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        conditions.push(
            "(e.normalized_name LIKE ? ESCAPE '\\'
              OR e.entity_id IN (SELECT a.entity_id FROM kb_names_fts f
                                 JOIN kb_aliases a ON a.alias_id = f.rowid
                                 WHERE kb_names_fts MATCH ?))"
                .to_string(),
        );
        values.push(SqlValue::Text(format!(
            "%{}%",
            escape_like(&words.join(" "))
        )));
        values.push(SqlValue::Text(expression));
    }

    let outer = if query.problems_only {
        "WHERE status = 'candidate' OR open_matches > 0 OR mentions = 1"
    } else {
        ""
    };
    let inner = format!(
        "SELECT {ROW_COLUMNS} FROM kb_entities e WHERE {}",
        conditions.join(" AND ")
    );

    let total: i64 = db(connection.query_row(
        &format!("SELECT COUNT(*) FROM ({inner}) {outer}"),
        params_from_iter(values.iter()),
        |row| row.get(0),
    ))?;

    let mut page_values = values.clone();
    page_values.push(SqlValue::Integer(PAGE_SIZE as i64));
    page_values.push(SqlValue::Integer((query.page * PAGE_SIZE) as i64));
    let mut statement = db(connection.prepare(&format!(
        "SELECT * FROM ({inner}) {outer} ORDER BY normalized_name, entity_id LIMIT ? OFFSET ?"
    )))?;
    let rows = db(statement.query_map(params_from_iter(page_values.iter()), entity_row))?;
    Ok(EntityPage {
        total: total as usize,
        page: query.page,
        page_size: PAGE_SIZE,
        rows: db(rows.collect())?,
    })
}

// ---------------------------------------------------------------------------------------------
// Reads: one entity, with its provenance
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityBrief {
    pub entity_id: i64,
    pub type_id: String,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasRow {
    pub alias_id: i64,
    /// The entity the alias belongs to: the sheet's own, or one merged into it.
    pub entity_id: i64,
    pub display: String,
    pub kind: String,
    pub origin: String,
    pub confidence: f64,
}

/// Where in a source a mention was found, as machine data. A page and a section for a document, a
/// sheet, a column and some rows for a table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocatorView {
    /// `chunk`, `cells`, `filename` or `metadata`.
    pub kind: String,
    pub page: Option<i64>,
    pub section: Option<i64>,
    pub sheet: Option<String>,
    pub column: Option<String>,
    pub first_row: Option<i64>,
    pub sample_rows: Vec<i64>,
}

/// A few words of the passage a mention points at, around the first place one of the entity's names
/// appears. Read from the chunk the index already stores; never written anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Excerpt {
    pub text: String,
    pub cut_before: bool,
    pub cut_after: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MentionView {
    pub mention_id: i64,
    /// The entity the mention is stored on: the sheet's own, or one merged into it.
    pub entity_id: i64,
    pub locator: LocatorView,
    pub excerpt: Option<Excerpt>,
    /// The name the mention matched, when it matched one.
    pub alias: Option<String>,
    pub role: Option<String>,
    pub method: String,
    pub confidence: f64,
    pub origin: String,
    pub occurrences: i64,
    /// The entity a split moved this mention away from.
    pub split_from: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMentions {
    pub source_id: i64,
    pub domain: String,
    pub relative_path: String,
    pub mentions: Vec<MentionView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationView {
    pub relation_id: i64,
    pub predicate: String,
    pub subject: EntityBrief,
    pub object: EntityBrief,
    pub relative_path: Option<String>,
    pub occurrences: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributeView {
    pub key: String,
    pub value: String,
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PossibleView {
    pub other: EntityBrief,
    pub reason: String,
    pub score: f64,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnView {
    pub sheet: String,
    pub column: String,
    pub semantic_type: String,
    pub subtype: Option<String>,
    pub role: Option<String>,
    pub confidence: f64,
    pub reason: String,
    pub read: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookColumns {
    pub relative_path: String,
    pub columns: Vec<ColumnView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityDetail {
    pub entity: EntityRow,
    /// The entity this one stands for once merges are followed (itself when it is not merged).
    pub effective_id: i64,
    /// The survivor, when this entity was merged into another.
    pub survivor: Option<EntityBrief>,
    pub aliases: Vec<AliasRow>,
    /// The entities redirected into this one.
    pub merged_in: Vec<EntityBrief>,
    pub sources: Vec<SourceMentions>,
    /// How many mentions the entity has in all; the sheet lists at most `MAX_MENTIONS_IN_SHEET`.
    pub mention_count: usize,
    pub relations: Vec<RelationView>,
    pub attributes: Vec<AttributeView>,
    pub possible_matches: Vec<PossibleView>,
    pub columns: Vec<WorkbookColumns>,
}

fn brief(connection: &Connection, entity_id: i64) -> Result<EntityBrief, AppError> {
    Ok(db(connection
        .query_row(
            "SELECT entity_id, type_id, canonical_name, status FROM kb_entities WHERE entity_id = ?1",
            [entity_id],
            |row| {
                Ok(EntityBrief {
                    entity_id: row.get(0)?,
                    type_id: row.get(1)?,
                    name: row.get(2)?,
                    status: row.get(3)?,
                })
            },
        )
        .optional())?
    .unwrap_or(EntityBrief {
        entity_id,
        type_id: String::new(),
        name: String::new(),
        status: "deleted".to_string(),
    }))
}

/// `path#p3#s2` is page 3, section 2 of `path`; the path may itself hold a `#`.
fn page_and_section(chunk_id: &str) -> (Option<i64>, Option<i64>) {
    let mut parts = chunk_id.rsplitn(3, '#');
    let section = parts
        .next()
        .and_then(|part| part.strip_prefix('s'))
        .and_then(|digits| digits.parse().ok());
    let page = parts
        .next()
        .and_then(|part| part.strip_prefix('p'))
        .and_then(|digits| digits.parse().ok());
    (page, section)
}

fn lowercase_chars(text: &str) -> Vec<char> {
    text.chars()
        .map(|character| character.to_lowercase().next().unwrap_or(character))
        .collect()
}

/// A window of `EXCERPT_CHARS` characters around the first place any needle appears (compared
/// without regard to case); the start of the passage when none does. Newlines become spaces.
fn excerpt_of(text: &str, needles: &[String]) -> Excerpt {
    let chars: Vec<char> = text.chars().collect();
    let lowered = lowercase_chars(text);
    let mut at: Option<usize> = None;
    for needle in needles {
        let wanted = lowercase_chars(needle);
        if wanted.is_empty() || wanted.len() > lowered.len() {
            continue;
        }
        if let Some(found) = (0..=lowered.len() - wanted.len())
            .find(|start| lowered[*start..*start + wanted.len()] == wanted[..])
        {
            at = Some(at.map_or(found, |known| known.min(found)));
        }
    }
    let center = at.unwrap_or(0);
    let start = center.saturating_sub(EXCERPT_CHARS / 4);
    let end = (start + EXCERPT_CHARS).min(chars.len());
    let window: String = chars[start..end].iter().collect();
    Excerpt {
        text: window.split_whitespace().collect::<Vec<_>>().join(" "),
        cut_before: start > 0,
        cut_after: end < chars.len(),
    }
}

/// Everything the base holds about one entity and why. An entity merged into another is shown with
/// the survivor's id so the dialog can send her there; a tombstone shows what it was.
pub fn entity_detail(connection: &Connection, entity_id: i64) -> Result<EntityDetail, AppError> {
    let sheet_row: Option<EntityRow> = db(connection
        .query_row(
            &format!("SELECT {ROW_COLUMNS} FROM kb_entities e WHERE e.entity_id = ?1"),
            [entity_id],
            entity_row,
        )
        .optional())?;
    let entity = sheet_row.ok_or(AppError::KnowledgeEntityNotFound)?;
    let effective_id = store::effective_entity(connection, entity_id)?
        .map_or(entity_id, |effective| effective.entity_id);

    // The entity and everything merged into it.
    let group: Vec<i64> = {
        let mut statement = db(connection.prepare(
            "SELECT entity_id FROM kb_effective_entity WHERE effective_id = ?1 ORDER BY entity_id",
        ))?;
        let rows = db(statement.query_map([entity_id], |row| row.get(0)))?;
        db(rows.collect())?
    };
    let group_json = serde_json::to_string(&group).map_err(|_| AppError::Internal)?;

    let merged_in = group
        .iter()
        .filter(|id| **id != entity_id)
        .map(|id| brief(connection, *id))
        .collect::<Result<Vec<_>, _>>()?;

    let mut aliases = Vec::new();
    {
        let mut statement = db(connection.prepare(
            "SELECT alias_id, entity_id, display, kind, origin, confidence FROM kb_aliases
             WHERE entity_id IN (SELECT value FROM json_each(?1))
             ORDER BY (entity_id <> ?2), normalized, alias_id",
        ))?;
        let rows = db(statement.query_map(params![group_json, entity_id], |row| {
            Ok(AliasRow {
                alias_id: row.get(0)?,
                entity_id: row.get(1)?,
                display: row.get(2)?,
                kind: row.get(3)?,
                origin: row.get(4)?,
                confidence: row.get(5)?,
            })
        }))?;
        for row in rows {
            aliases.push(db(row)?);
        }
    }
    let needles: Vec<String> = aliases.iter().map(|alias| alias.display.clone()).collect();

    // Mentions, grouped by source.
    let mention_count: i64 = db(connection.query_row(
        "SELECT COUNT(*) FROM kb_mentions WHERE entity_id IN (SELECT value FROM json_each(?1))",
        [&group_json],
        |row| row.get(0),
    ))?;
    let mut sources: Vec<SourceMentions> = Vec::new();
    let mut workbooks: BTreeSet<String> = BTreeSet::new();
    {
        let mut statement = db(connection.prepare(
            "SELECT m.mention_id, m.entity_id, s.source_id, s.domain, s.relative_path,
                    m.locator_kind, m.chunk_id, m.sheet, m.column_name, m.first_row, m.sample_rows,
                    m.occurrence_count, m.confidence, m.method, m.origin, m.role, a.display,
                    m.split_from
             FROM kb_mentions m
             JOIN kb_sources s ON s.source_id = m.source_id
             LEFT JOIN kb_aliases a ON a.alias_id = m.alias_id
             WHERE m.entity_id IN (SELECT value FROM json_each(?1))
             ORDER BY s.domain, s.relative_path, m.mention_id
             LIMIT ?2",
        ))?;
        let rows = db(statement.query_map(
            params![group_json, MAX_MENTIONS_IN_SHEET as i64],
            |row| {
                let chunk_id: Option<String> = row.get(6)?;
                let sample: Option<String> = row.get(10)?;
                let (page, section) = chunk_id.as_deref().map_or((None, None), page_and_section);
                let view = MentionView {
                    mention_id: row.get(0)?,
                    entity_id: row.get(1)?,
                    locator: LocatorView {
                        kind: row.get(5)?,
                        page,
                        section,
                        sheet: row.get(7)?,
                        column: row.get(8)?,
                        first_row: row.get(9)?,
                        sample_rows: sample
                            .and_then(|text| serde_json::from_str(&text).ok())
                            .unwrap_or_default(),
                    },
                    excerpt: None,
                    alias: row.get(16)?,
                    role: row.get(15)?,
                    method: row.get(13)?,
                    confidence: row.get(12)?,
                    origin: row.get(14)?,
                    occurrences: row.get(11)?,
                    split_from: row.get(17)?,
                };
                Ok((
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    chunk_id,
                    view,
                ))
            },
        ))?;
        for row in rows {
            let (source_id, domain, relative_path, chunk_id, mut view) = db(row)?;
            if let Some(chunk_id) = chunk_id {
                let text: Option<String> = db(connection
                    .query_row(
                        "SELECT text FROM chunks WHERE chunk_id = ?1",
                        [chunk_id],
                        |row| row.get(0),
                    )
                    .optional())?;
                view.excerpt = text.map(|text| excerpt_of(&text, &needles));
            }
            if domain == Domain::Data.as_code() {
                workbooks.insert(relative_path.clone());
            }
            match sources.last_mut() {
                Some(last) if last.source_id == source_id => last.mentions.push(view),
                _ => sources.push(SourceMentions {
                    source_id,
                    domain,
                    relative_path,
                    mentions: vec![view],
                }),
            }
        }
    }

    // Relations and attributes of the entity and of what was merged into it.
    let mut relations = Vec::new();
    {
        let mut statement = db(connection.prepare(
            "SELECT r.relation_id, r.predicate, r.subject, r.object, s.relative_path,
                    r.occurrence_count
             FROM kb_relations r LEFT JOIN kb_sources s ON s.source_id = r.source_id
             WHERE r.subject IN (SELECT value FROM json_each(?1))
                OR r.object IN (SELECT value FROM json_each(?1))
             ORDER BY r.predicate, r.relation_id",
        ))?;
        let rows = db(statement.query_map([&group_json], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        }))?;
        for row in rows {
            let (relation_id, predicate, subject, object, relative_path, occurrences) = db(row)?;
            relations.push(RelationView {
                relation_id,
                predicate,
                subject: brief(connection, subject)?,
                object: brief(connection, object)?,
                relative_path,
                occurrences,
            });
        }
    }
    let mut attributes = Vec::new();
    {
        let mut statement = db(connection.prepare(
            "SELECT key, value, origin FROM kb_attributes
             WHERE entity_id IN (SELECT value FROM json_each(?1))
             GROUP BY key, value, origin ORDER BY key, value",
        ))?;
        let rows = db(statement.query_map([&group_json], |row| {
            Ok(AttributeView {
                key: row.get(0)?,
                value: row.get(1)?,
                origin: row.get(2)?,
            })
        }))?;
        for row in rows {
            attributes.push(db(row)?);
        }
    }
    let mut possible_matches = Vec::new();
    {
        let mut statement = db(connection.prepare(
            "SELECT p.entity_a, p.entity_b, p.reason, p.score, p.state FROM kb_possible_matches p
             WHERE p.entity_a IN (SELECT value FROM json_each(?1))
                OR p.entity_b IN (SELECT value FROM json_each(?1))
             ORDER BY p.state, p.score DESC, p.entity_a, p.entity_b",
        ))?;
        let rows = db(statement.query_map([&group_json], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, String>(4)?,
            ))
        }))?;
        for row in rows {
            let (a, b, reason, score, state) = db(row)?;
            let other = if group.contains(&a) { b } else { a };
            possible_matches.push(PossibleView {
                other: brief(connection, other)?,
                reason,
                score,
                state,
            });
        }
    }

    // Why each workbook's columns were read as they were: the evidence behind a table mention.
    let mut columns = Vec::new();
    for path in workbooks {
        let rows = store::column_semantics_for(connection, Domain::Data, &path)?;
        columns.push(WorkbookColumns {
            relative_path: path,
            columns: rows
                .into_iter()
                .map(|row| ColumnView {
                    read: row.is_read(),
                    sheet: row.sheet,
                    column: row.column_name,
                    semantic_type: row.semantic_type,
                    subtype: row.subtype,
                    role: row.role,
                    confidence: row.confidence,
                    reason: row.reason,
                })
                .collect(),
        });
    }

    let survivor = if effective_id == entity_id {
        None
    } else {
        Some(brief(connection, effective_id)?)
    };

    Ok(EntityDetail {
        entity,
        effective_id,
        survivor,
        aliases,
        merged_in,
        sources,
        mention_count: mention_count as usize,
        relations,
        attributes,
        possible_matches,
        columns,
    })
}

// ---------------------------------------------------------------------------------------------
// Reads: pairs to check, library health, overview, types and roles
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PossiblePair {
    pub one: EntityBrief,
    pub other: EntityBrief,
    pub one_mentions: usize,
    pub other_mentions: usize,
    pub reason: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PossiblePage {
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub pairs: Vec<PossiblePair>,
}

fn mention_total(connection: &Connection, entity_id: i64) -> Result<usize, AppError> {
    let total: i64 = db(connection.query_row(
        "SELECT COUNT(*) FROM kb_mentions m JOIN kb_effective_entity v ON v.entity_id = m.entity_id
         WHERE v.effective_id = ?1",
        [entity_id],
        |row| row.get(0),
    ))?;
    Ok(total as usize)
}

/// The open pairs of `kb_possible_matches` whose two sides are both live, best first.
pub fn possible_matches(connection: &Connection, page: usize) -> Result<PossiblePage, AppError> {
    let live = "SELECT COUNT(*) FROM kb_possible_matches p
                JOIN kb_entities a ON a.entity_id = p.entity_a
                JOIN kb_entities b ON b.entity_id = p.entity_b
                WHERE p.state = 'open'
                  AND a.status IN ('candidate', 'active') AND b.status IN ('candidate', 'active')";
    let total: i64 = db(connection.query_row(live, [], |row| row.get(0)))?;
    let mut statement = db(connection.prepare(
        "SELECT p.entity_a, p.entity_b, p.reason, p.score FROM kb_possible_matches p
         JOIN kb_entities a ON a.entity_id = p.entity_a
         JOIN kb_entities b ON b.entity_id = p.entity_b
         WHERE p.state = 'open'
           AND a.status IN ('candidate', 'active') AND b.status IN ('candidate', 'active')
         ORDER BY p.score DESC, p.entity_a, p.entity_b LIMIT ?1 OFFSET ?2",
    ))?;
    let rows = db(statement.query_map(
        params![PAGE_SIZE as i64, (page * PAGE_SIZE) as i64],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
            ))
        },
    ))?;
    let mut pairs = Vec::new();
    for row in rows {
        let (one, other, reason, score) = db(row)?;
        pairs.push(PossiblePair {
            one: brief(connection, one)?,
            other: brief(connection, other)?,
            one_mentions: mention_total(connection, one)?,
            other_mentions: mention_total(connection, other)?,
            reason,
            score,
        });
    }
    Ok(PossiblePage {
        total: total as usize,
        page,
        page_size: PAGE_SIZE,
        pairs,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBrief {
    pub source_id: i64,
    pub domain: String,
    pub relative_path: String,
}

/// A count that is exact and a list that is capped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counted<T> {
    pub total: usize,
    pub items: Vec<T>,
}

/// Read-only counts about the state of the library. Numbers and ids: the words around them belong
/// to the interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    /// Live entities with exactly one mention.
    pub seen_once: Counted<EntityBrief>,
    /// Entities nobody has confirmed yet.
    pub candidates: usize,
    /// Documents the base read and found no entity in.
    pub documents_without_entity: Counted<SourceBrief>,
    /// Workbooks of which no column was read as a name, an organisation, an identifier, a place or an item.
    pub workbooks_without_usable_column: Counted<SourceBrief>,
    /// Open pairs that might be one entity.
    pub probable_duplicates: usize,
    /// People and organisations named in a document and in no workbook (only counted when at
    /// least one workbook has been read).
    pub in_documents_not_in_tables: Counted<EntityBrief>,
}

fn briefs(connection: &Connection, sql: &str) -> Result<Counted<EntityBrief>, AppError> {
    let mut statement = db(connection.prepare(sql))?;
    let rows = db(statement.query_map([], |row| row.get::<_, i64>(0)))?;
    let ids: Vec<i64> = db(rows.collect())?;
    let total = ids.len();
    let items = ids
        .into_iter()
        .take(HEALTH_LIST_LIMIT)
        .map(|id| brief(connection, id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Counted { total, items })
}

fn source_briefs(
    connection: &Connection,
    domain: Domain,
    sql: &str,
) -> Result<Vec<SourceBrief>, AppError> {
    let mut statement = db(connection.prepare(sql))?;
    let rows = db(statement.query_map([], |row| {
        Ok(SourceBrief {
            source_id: row.get(0)?,
            domain: domain.as_code().to_string(),
            relative_path: row.get(1)?,
        })
    }))?;
    db(rows.collect())
}

pub fn health_report(connection: &Connection) -> Result<HealthReport, AppError> {
    let seen_once = briefs(
        connection,
        "SELECT e.entity_id FROM kb_entities e
         WHERE e.status IN ('candidate', 'active') AND e.type_id <> 'identifier'
           AND (SELECT COUNT(*) FROM kb_mentions m JOIN kb_effective_entity v ON v.entity_id = m.entity_id
                WHERE v.effective_id = e.entity_id) = 1
         ORDER BY e.normalized_name, e.entity_id",
    )?;
    let candidates: i64 = db(connection.query_row(
        "SELECT COUNT(*) FROM kb_entities WHERE status = 'candidate'",
        [],
        |row| row.get(0),
    ))?;

    let without_entity = source_briefs(
        connection,
        Domain::Documents,
        "SELECT s.source_id, s.relative_path FROM kb_sources s
         WHERE s.domain = 'documents' AND s.kb_version > 0
           AND NOT EXISTS (SELECT 1 FROM kb_mentions m WHERE m.source_id = s.source_id)
         ORDER BY s.relative_path",
    )?;
    let documents_without_entity = Counted {
        total: without_entity.len(),
        items: without_entity.into_iter().take(HEALTH_LIST_LIMIT).collect(),
    };

    let workbooks = source_briefs(
        connection,
        Domain::Data,
        "SELECT s.source_id, s.relative_path FROM kb_sources s
         WHERE s.domain = 'data' ORDER BY s.relative_path",
    )?;
    let workbook_count = workbooks.len();
    let mut without_column = Vec::new();
    for workbook in workbooks {
        let rows = store::column_semantics_for(connection, Domain::Data, &workbook.relative_path)?;
        if !rows.iter().any(|row| row.is_read()) {
            without_column.push(workbook);
        }
    }
    let workbooks_without_usable_column = Counted {
        total: without_column.len(),
        items: without_column.into_iter().take(HEALTH_LIST_LIMIT).collect(),
    };

    let probable_duplicates: i64 = db(connection.query_row(
        "SELECT COUNT(*) FROM kb_possible_matches p
         JOIN kb_entities a ON a.entity_id = p.entity_a
         JOIN kb_entities b ON b.entity_id = p.entity_b
         WHERE p.state = 'open'
           AND a.status IN ('candidate', 'active') AND b.status IN ('candidate', 'active')",
        [],
        |row| row.get(0),
    ))?;

    let in_documents_not_in_tables = if workbook_count == 0 {
        Counted {
            total: 0,
            items: Vec::new(),
        }
    } else {
        briefs(
            connection,
            "SELECT e.entity_id FROM kb_entities e
             WHERE e.status IN ('candidate', 'active') AND e.type_id IN ('person', 'organization')
               AND EXISTS (SELECT 1 FROM kb_mentions m
                           JOIN kb_effective_entity v ON v.entity_id = m.entity_id
                           JOIN kb_sources s ON s.source_id = m.source_id
                           WHERE v.effective_id = e.entity_id AND s.domain = 'documents')
               AND NOT EXISTS (SELECT 1 FROM kb_mentions m
                               JOIN kb_effective_entity v ON v.entity_id = m.entity_id
                               JOIN kb_sources s ON s.source_id = m.source_id
                               WHERE v.effective_id = e.entity_id AND s.domain = 'data')
             ORDER BY e.normalized_name, e.entity_id",
        )?
    };

    Ok(HealthReport {
        seen_once,
        candidates: candidates as usize,
        documents_without_entity,
        workbooks_without_usable_column,
        probable_duplicates: probable_duplicates as usize,
        in_documents_not_in_tables,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub entities_live: usize,
    pub entities_candidate: usize,
    pub entities_manual: usize,
    pub entities_merged: usize,
    pub entities_deleted: usize,
    pub aliases: usize,
    pub mentions: usize,
    pub documents_read: usize,
    pub workbooks_read: usize,
    pub open_possible_matches: usize,
    /// Documents the next Analyse reads again from the stored text.
    pub documents_due: usize,
    /// Is the base consistent (nothing in the integrity report)?
    pub consistent: bool,
    pub last_operation: Option<LastOperation>,
}

pub fn overview(connection: &Connection) -> Result<Overview, AppError> {
    let one = |sql: &str| -> Result<usize, AppError> {
        let value: i64 = db(connection.query_row(sql, [], |row| row.get(0)))?;
        Ok(value as usize)
    };
    Ok(Overview {
        entities_live: one("SELECT COUNT(*) FROM kb_entities WHERE status IN ('candidate', 'active')")?,
        entities_candidate: one("SELECT COUNT(*) FROM kb_entities WHERE status = 'candidate'")?,
        entities_manual: one(
            "SELECT COUNT(*) FROM kb_entities WHERE origin = 'manual' AND status IN ('candidate', 'active')",
        )?,
        entities_merged: one("SELECT COUNT(*) FROM kb_entities WHERE status = 'merged'")?,
        entities_deleted: one("SELECT COUNT(*) FROM kb_entities WHERE status = 'deleted'")?,
        aliases: one("SELECT COUNT(*) FROM kb_aliases")?,
        mentions: one("SELECT COUNT(*) FROM kb_mentions")?,
        documents_read: one("SELECT COUNT(*) FROM kb_sources WHERE domain = 'documents' AND kb_version > 0")?,
        workbooks_read: one("SELECT COUNT(*) FROM kb_sources WHERE domain = 'data' AND kb_version > 0")?,
        open_possible_matches: one(
            "SELECT COUNT(*) FROM kb_possible_matches p
             JOIN kb_entities a ON a.entity_id = p.entity_a
             JOIN kb_entities b ON b.entity_id = p.entity_b
             WHERE p.state = 'open'
               AND a.status IN ('candidate', 'active') AND b.status IN ('candidate', 'active')",
        )?,
        documents_due: documents_due(connection)?,
        consistent: store::integrity_check(connection)?.is_clean(),
        last_operation: last_operation(connection)?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeRow {
    pub id: String,
    pub builtin: bool,
    pub entities: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleRow {
    pub id: String,
    /// The catalogue key of the role's label (`knowledge.roles.client`).
    pub label_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackRow {
    pub id: String,
    pub locale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypesAndRoles {
    pub types: Vec<TypeRow>,
    pub roles: Vec<RoleRow>,
    /// The packs read for the interface language: the base, then the active modules.
    pub packs: Vec<PackRow>,
    /// Every module this build ships, active or not.
    pub available_packs: Vec<String>,
}

/// A read-only description of the vocabulary: the types of entity, the roles a pack knows by label
/// key, and the packs in use. Profiles are a later lot.
pub fn types_and_roles(
    connection: &Connection,
    packs: &PackSet,
) -> Result<TypesAndRoles, AppError> {
    let mut statement = db(connection.prepare(
        "SELECT t.type_id, t.builtin,
                (SELECT COUNT(*) FROM kb_entities e
                 WHERE e.type_id = t.type_id AND e.status IN ('candidate', 'active'))
         FROM kb_entity_types t ORDER BY t.builtin DESC, t.type_id",
    ))?;
    let rows = db(statement.query_map([], |row| {
        Ok(TypeRow {
            id: row.get(0)?,
            builtin: row.get::<_, i64>(1)? != 0,
            entities: row.get::<_, i64>(2)? as usize,
        })
    }))?;
    Ok(TypesAndRoles {
        types: db(rows.collect())?,
        roles: packs
            .roles()
            .map(|role| RoleRow {
                id: role.id.as_str().to_string(),
                label_key: role.label_key.clone(),
            })
            .collect(),
        packs: packs
            .loaded()
            .iter()
            .map(|(id, locale)| PackRow {
                id: id.clone(),
                locale: locale.clone(),
            })
            .collect(),
        available_packs: available_pack_ids()
            .iter()
            .map(|id| id.to_string())
            .collect(),
    })
}

#[cfg(test)]
mod tests;
