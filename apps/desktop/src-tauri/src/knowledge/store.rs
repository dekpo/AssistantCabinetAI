//! The knowledge base's SQL: the `kb_*` schema, the lifecycle that ties it to the index, the reads
//! the later lots build on, and the primitives a manual edit needs.
//!
//! It lives in the same SQLite file as the index (`index.sqlite3`), and `IndexStore` stays the
//! owner of the connection. Every function that writes more than one statement takes a
//! `&Transaction`, so the compiler refuses a caller that would leave half an operation behind; the
//! index calls them inside the transaction it already opens for a document's chunks, so a source's
//! chunks and its knowledge are written, or discarded, together.
//!
//! Every failure is `AppError::KnowledgeUnavailable` (or a more specific machine code): the
//! caller falls back to the path that does not use the knowledge base (`SESSION-KB-00-master.md`,
//! I4). Nothing here panics, and nothing here holds a passage of text: the tables hold names,
//! locators, counts and codes.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde_json::{json, Value};

use super::{
    Alias, AliasKind, ApplyOutcome, AttributeDraft, Domain, Entity, EntityDraft, EntityFilter,
    EntityRecord, EntityRef, EntityStatus, FileSets, IntegrityReport, KnowledgeDelta, MentionDraft,
    MentionLocator, NameHit, NewAlias, NewEntity, Origin, Page, RelationDraft, SignalDraft,
    SourceRef, SourceRow, CURRENT_SCHEMA_VERSION,
};
use crate::error::AppError;

fn db<T>(result: rusqlite::Result<T>) -> Result<T, AppError> {
    result.map_err(|_| AppError::KnowledgeUnavailable)
}

// ---------------------------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------------------------

/// Additive and idempotent: every statement is `IF NOT EXISTS`, nothing is ever dropped or renamed.
/// A later column arrives through `ALTER TABLE ... ADD COLUMN`, guarded the way
/// `IndexStore::ensure_column` guards the index's own.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS kb_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS kb_source_domains (
    domain TEXT PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS kb_sources (
    source_id INTEGER PRIMARY KEY,
    domain TEXT NOT NULL REFERENCES kb_source_domains (domain),
    relative_path TEXT NOT NULL,
    content_id TEXT NOT NULL,
    kb_version INTEGER NOT NULL DEFAULT 0,
    gazetteer_epoch INTEGER NOT NULL DEFAULT 0,
    UNIQUE (domain, relative_path)
);
CREATE INDEX IF NOT EXISTS kb_sources_by_content ON kb_sources (domain, content_id);

CREATE TABLE IF NOT EXISTS kb_entity_types (
    type_id TEXT PRIMARY KEY,
    builtin INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS kb_entities (
    entity_id INTEGER PRIMARY KEY,
    type_id TEXT NOT NULL REFERENCES kb_entity_types (type_id),
    subtype TEXT,
    canonical_name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    disambiguator TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL CHECK (status IN ('candidate', 'active', 'merged', 'deleted')),
    merged_into INTEGER REFERENCES kb_entities (entity_id),
    origin TEXT NOT NULL CHECK (origin IN ('automatic', 'manual')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS kb_entities_unique_name
    ON kb_entities (type_id, normalized_name, disambiguator)
    WHERE status IN ('candidate', 'active', 'deleted');
CREATE INDEX IF NOT EXISTS kb_entities_by_name ON kb_entities (normalized_name);
CREATE INDEX IF NOT EXISTS kb_entities_by_target
    ON kb_entities (merged_into) WHERE merged_into IS NOT NULL;

CREATE TABLE IF NOT EXISTS kb_aliases (
    alias_id INTEGER PRIMARY KEY,
    entity_id INTEGER NOT NULL REFERENCES kb_entities (entity_id) ON DELETE CASCADE,
    display TEXT NOT NULL,
    normalized TEXT NOT NULL,
    phonetic_key TEXT NOT NULL DEFAULT '',
    kind TEXT NOT NULL,
    origin TEXT NOT NULL CHECK (origin IN ('automatic', 'manual')),
    confidence REAL NOT NULL DEFAULT 1.0,
    UNIQUE (entity_id, normalized)
);
CREATE INDEX IF NOT EXISTS kb_aliases_by_normalized ON kb_aliases (normalized);
CREATE INDEX IF NOT EXISTS kb_aliases_by_phonetic
    ON kb_aliases (phonetic_key) WHERE phonetic_key <> '';

-- Prefix search for autocomplete, with no minimum token length. The row id is the alias id, so a
-- row is found and removed by key rather than by scanning the table.
CREATE VIRTUAL TABLE IF NOT EXISTS kb_names_fts USING fts5(normalized, alias_id UNINDEXED);
CREATE TRIGGER IF NOT EXISTS kb_aliases_names_insert AFTER INSERT ON kb_aliases BEGIN
    INSERT INTO kb_names_fts (rowid, normalized, alias_id)
    VALUES (new.alias_id, new.normalized, new.alias_id);
END;
CREATE TRIGGER IF NOT EXISTS kb_aliases_names_delete AFTER DELETE ON kb_aliases BEGIN
    DELETE FROM kb_names_fts WHERE rowid = old.alias_id;
END;
CREATE TRIGGER IF NOT EXISTS kb_aliases_names_update
    AFTER UPDATE OF normalized ON kb_aliases BEGIN
    DELETE FROM kb_names_fts WHERE rowid = old.alias_id;
    INSERT INTO kb_names_fts (rowid, normalized, alias_id)
    VALUES (new.alias_id, new.normalized, new.alias_id);
END;

CREATE TABLE IF NOT EXISTS kb_mentions (
    mention_id INTEGER PRIMARY KEY,
    entity_id INTEGER NOT NULL REFERENCES kb_entities (entity_id) ON DELETE CASCADE,
    source_id INTEGER NOT NULL REFERENCES kb_sources (source_id) ON DELETE CASCADE,
    alias_id INTEGER REFERENCES kb_aliases (alias_id) ON DELETE SET NULL,
    role TEXT,
    locator_kind TEXT NOT NULL CHECK (locator_kind IN ('chunk', 'cells', 'filename', 'metadata')),
    chunk_id TEXT,
    sheet TEXT,
    column_name TEXT,
    first_row INTEGER,
    sample_rows TEXT,
    occurrence_count INTEGER NOT NULL DEFAULT 1,
    confidence REAL NOT NULL DEFAULT 1.0,
    method TEXT NOT NULL,
    origin TEXT NOT NULL CHECK (origin IN ('automatic', 'manual')),
    extractor_version INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS kb_mentions_by_entity ON kb_mentions (entity_id);
CREATE INDEX IF NOT EXISTS kb_mentions_by_source ON kb_mentions (source_id);
CREATE INDEX IF NOT EXISTS kb_mentions_by_alias ON kb_mentions (alias_id) WHERE alias_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS kb_relations (
    relation_id INTEGER PRIMARY KEY,
    subject INTEGER NOT NULL REFERENCES kb_entities (entity_id) ON DELETE CASCADE,
    predicate TEXT NOT NULL,
    object INTEGER NOT NULL REFERENCES kb_entities (entity_id) ON DELETE CASCADE,
    source_id INTEGER REFERENCES kb_sources (source_id) ON DELETE CASCADE,
    occurrence_count INTEGER NOT NULL DEFAULT 1,
    confidence REAL NOT NULL DEFAULT 1.0,
    origin TEXT NOT NULL CHECK (origin IN ('automatic', 'manual'))
);
CREATE INDEX IF NOT EXISTS kb_relations_by_subject ON kb_relations (subject);
CREATE INDEX IF NOT EXISTS kb_relations_by_object ON kb_relations (object);
CREATE INDEX IF NOT EXISTS kb_relations_by_source ON kb_relations (source_id) WHERE source_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS kb_attributes (
    attribute_id INTEGER PRIMARY KEY,
    entity_id INTEGER NOT NULL REFERENCES kb_entities (entity_id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    origin TEXT NOT NULL CHECK (origin IN ('automatic', 'manual')),
    source_id INTEGER REFERENCES kb_sources (source_id) ON DELETE CASCADE
);
-- One row per source that states it, so removing one source never takes away what another still
-- says, and one manual row (no source).
CREATE UNIQUE INDEX IF NOT EXISTS kb_attributes_unique
    ON kb_attributes (entity_id, key, value, COALESCE(source_id, 0));
CREATE INDEX IF NOT EXISTS kb_attributes_by_source ON kb_attributes (source_id) WHERE source_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS kb_possible_matches (
    entity_a INTEGER NOT NULL REFERENCES kb_entities (entity_id) ON DELETE CASCADE,
    entity_b INTEGER NOT NULL REFERENCES kb_entities (entity_id) ON DELETE CASCADE,
    reason TEXT NOT NULL,
    score REAL NOT NULL DEFAULT 0.0,
    state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'dismissed', 'merged')),
    PRIMARY KEY (entity_a, entity_b)
);

CREATE TABLE IF NOT EXISTS kb_ops_log (
    op_id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS kb_column_semantics (
    source_id INTEGER NOT NULL REFERENCES kb_sources (source_id) ON DELETE CASCADE,
    sheet TEXT NOT NULL,
    column_name TEXT NOT NULL,
    semantic_type TEXT NOT NULL,
    subtype TEXT,
    role TEXT,
    confidence REAL NOT NULL DEFAULT 1.0,
    reason TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (source_id, sheet, column_name)
);

CREATE TABLE IF NOT EXISTS kb_source_signals (
    source_id INTEGER NOT NULL REFERENCES kb_sources (source_id) ON DELETE CASCADE,
    pack_id TEXT NOT NULL,
    hits INTEGER NOT NULL DEFAULT 0,
    distinct_terms INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (source_id, pack_id)
);

CREATE TABLE IF NOT EXISTS kb_profile (
    pack_id TEXT PRIMARY KEY,
    score REAL NOT NULL DEFAULT 0.0,
    state TEXT NOT NULL CHECK (state IN ('suggested', 'confirmed', 'dismissed')),
    updated_at INTEGER NOT NULL
);

-- Counts and milliseconds only, never a question or a name. A ring buffer of 2000 rows, kept by
-- the code that writes it (lot 6).
CREATE TABLE IF NOT EXISTS kb_diagnostics (
    id INTEGER PRIMARY KEY,
    at INTEGER NOT NULL,
    mode TEXT NOT NULL,
    strategy TEXT NOT NULL,
    skipped_reason TEXT,
    selected_files INTEGER NOT NULL DEFAULT 0,
    candidate_files INTEGER NOT NULL DEFAULT 0,
    selected_data INTEGER NOT NULL DEFAULT 0,
    candidate_data INTEGER NOT NULL DEFAULT 0,
    cited_files INTEGER NOT NULL DEFAULT 0,
    cited_in_candidates INTEGER NOT NULL DEFAULT 0,
    entities_found INTEGER NOT NULL DEFAULT 0,
    widened INTEGER NOT NULL DEFAULT 0,
    knowledge_ms INTEGER NOT NULL DEFAULT 0,
    retrieval_ms INTEGER NOT NULL DEFAULT 0,
    total_ms INTEGER NOT NULL DEFAULT 0
);

-- Merge is a redirect, so every read goes through these two views. Chains are flattened when an
-- entity is merged, which is what makes one hop enough.
CREATE VIEW IF NOT EXISTS kb_effective_entity AS
    SELECT entity_id,
           CASE WHEN status = 'merged' AND merged_into IS NOT NULL
                THEN merged_into ELSE entity_id END AS effective_id
    FROM kb_entities;
CREATE VIEW IF NOT EXISTS kb_effective_mentions AS
    SELECT m.*, v.effective_id AS effective_entity_id
    FROM kb_mentions m
    JOIN kb_effective_entity v ON v.entity_id = m.entity_id;
";

const SEEDS: &str = "
INSERT OR IGNORE INTO kb_source_domains (domain) VALUES ('documents'), ('data');
INSERT OR IGNORE INTO kb_entity_types (type_id, builtin) VALUES
    ('person', 1), ('organization', 1), ('location', 1),
    ('identifier', 1), ('item', 1), ('term', 1);
INSERT OR IGNORE INTO kb_meta (key, value) VALUES
    ('extractor_version', ''), ('phonetic_version', ''), ('packs_hash', ''),
    ('gazetteer_epoch', '0');
";

/// Create whatever is missing, seed the fixed rows, record the schema version. Safe to run on every
/// open: a database that is already current is read, not written.
pub fn ensure_schema(connection: &Connection) -> Result<(), AppError> {
    let tx = db(connection.unchecked_transaction())?;
    db(tx.execute_batch(SCHEMA))?;
    db(tx.execute_batch(SEEDS))?;
    db(tx.execute(
        "INSERT INTO kb_meta (key, value) VALUES ('schema_version', ?1)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value
         WHERE CAST(kb_meta.value AS INTEGER) < CAST(excluded.value AS INTEGER)",
        [CURRENT_SCHEMA_VERSION.to_string()],
    ))?;
    db(tx.commit())
}

pub fn meta(connection: &Connection, key: &str) -> Result<Option<String>, AppError> {
    db(connection
        .query_row("SELECT value FROM kb_meta WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional())
}

pub fn set_meta(connection: &Connection, key: &str, value: &str) -> Result<(), AppError> {
    db(connection.execute(
        "INSERT INTO kb_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    ))?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Sources and their lifecycle
// ---------------------------------------------------------------------------------------------

/// Record that a source is known, with the extractor generation that read it. Returns its id.
/// Writes nothing when the row is already exactly this. It does not touch the source's mentions:
/// `apply_delta` is what makes them follow a changed file.
pub fn upsert_source(
    connection: &Connection,
    source: &SourceRef,
    kb_version: u32,
    gazetteer_epoch: u32,
) -> Result<i64, AppError> {
    db(connection.execute(
        "INSERT INTO kb_sources (domain, relative_path, content_id, kb_version, gazetteer_epoch)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (domain, relative_path) DO UPDATE SET
            content_id = excluded.content_id,
            kb_version = excluded.kb_version,
            gazetteer_epoch = excluded.gazetteer_epoch
         WHERE kb_sources.content_id IS NOT excluded.content_id
            OR kb_sources.kb_version IS NOT excluded.kb_version
            OR kb_sources.gazetteer_epoch IS NOT excluded.gazetteer_epoch",
        params![
            source.domain.as_code(),
            source.relative_path,
            source.content_id,
            kb_version,
            gazetteer_epoch,
        ],
    ))?;
    db(connection.query_row(
        "SELECT source_id FROM kb_sources WHERE domain = ?1 AND relative_path = ?2",
        params![source.domain.as_code(), source.relative_path],
        |row| row.get(0),
    ))
}

pub fn source_id(
    connection: &Connection,
    domain: Domain,
    relative_path: &str,
) -> Result<Option<i64>, AppError> {
    db(connection
        .query_row(
            "SELECT source_id FROM kb_sources WHERE domain = ?1 AND relative_path = ?2",
            params![domain.as_code(), relative_path],
            |row| row.get(0),
        )
        .optional())
}

/// Every known source, optionally of one domain, in path order.
pub fn sources(
    connection: &Connection,
    domain: Option<Domain>,
) -> Result<Vec<SourceRow>, AppError> {
    let mut statement = db(connection.prepare(
        "SELECT source_id, domain, relative_path, content_id, kb_version, gazetteer_epoch
         FROM kb_sources WHERE (?1 IS NULL OR domain = ?1)
         ORDER BY domain, relative_path",
    ))?;
    let rows = db(statement.query_map([domain.map(Domain::as_code)], |row| {
        let code: String = row.get(1)?;
        Ok(SourceRow {
            source_id: row.get(0)?,
            source: SourceRef {
                domain: Domain::from_code(&code).ok_or_else(|| bad_code(1, &code))?,
                relative_path: row.get(2)?,
                content_id: row.get(3)?,
            },
            kb_version: row.get(4)?,
            gazetteer_epoch: row.get(5)?,
        })
    }))?;
    db(rows.collect())
}

/// Forget the sources at these paths, with everything learned from them (their mentions,
/// relations, attributes, signals and column findings go with them), then collect the automatic
/// entities nothing holds on to any more. Returns how many sources were removed.
pub fn remove_sources(
    tx: &Transaction<'_>,
    domain: Domain,
    relative_paths: &[String],
) -> Result<usize, AppError> {
    let mut removed = 0;
    for path in relative_paths {
        removed += db(tx.execute(
            "DELETE FROM kb_sources WHERE domain = ?1 AND relative_path = ?2",
            params![domain.as_code(), path],
        ))?;
    }
    if removed > 0 {
        gc_orphans(tx)?;
    }
    Ok(removed)
}

pub fn remove_source(
    tx: &Transaction<'_>,
    domain: Domain,
    relative_path: &str,
) -> Result<bool, AppError> {
    Ok(remove_sources(tx, domain, &[relative_path.to_string()])? > 0)
}

/// A Reset of one folder: every source of that domain goes, manual entities, aliases and attributes
/// stay (only their mentions in those sources disappear with them).
pub fn clear_domain(tx: &Transaction<'_>, domain: Domain) -> Result<(), AppError> {
    db(tx.execute(
        "DELETE FROM kb_sources WHERE domain = ?1",
        [domain.as_code()],
    ))?;
    gc_orphans(tx)?;
    Ok(())
}

/// Every source of both domains, and the automatic entities that lose their last support. Manual
/// entities, aliases and attributes stay.
pub fn clear_all_automatic(tx: &Transaction<'_>) -> Result<(), AppError> {
    db(tx.execute("DELETE FROM kb_sources", []))?;
    gc_orphans(tx)?;
    Ok(())
}

/// "Reset the whole knowledge base": manual rows, tombstones and the operation log too. The
/// seeded domains, entity types and meta rows stay, and so do the diagnostics, which are counts
/// about questions rather than knowledge.
pub fn clear_everything(tx: &Transaction<'_>) -> Result<(), AppError> {
    db(tx.execute_batch(
        "DELETE FROM kb_ops_log;
         DELETE FROM kb_possible_matches;
         DELETE FROM kb_profile;
         DELETE FROM kb_sources;
         DELETE FROM kb_entities;",
    ))
}

/// An automatic, live entity that nothing holds on to: no mention, no relation, no manual alias or
/// attribute, and nothing merged into it. Tombstones, merged entities and manual entities are never
/// collected: they record a decision.
const ORPHAN_CONDITION: &str = "
    origin = 'automatic' AND status IN ('candidate', 'active')
    AND NOT EXISTS (SELECT 1 FROM kb_mentions m WHERE m.entity_id = kb_entities.entity_id)
    AND NOT EXISTS (SELECT 1 FROM kb_relations r
                    WHERE r.subject = kb_entities.entity_id OR r.object = kb_entities.entity_id)
    AND NOT EXISTS (SELECT 1 FROM kb_aliases a
                    WHERE a.entity_id = kb_entities.entity_id AND a.origin = 'manual')
    AND NOT EXISTS (SELECT 1 FROM kb_attributes t
                    WHERE t.entity_id = kb_entities.entity_id AND t.origin = 'manual')
    AND NOT EXISTS (SELECT 1 FROM kb_entities c WHERE c.merged_into = kb_entities.entity_id)";

pub fn gc_orphans(connection: &Connection) -> Result<usize, AppError> {
    db(connection.execute(
        &format!("DELETE FROM kb_entities WHERE {ORPHAN_CONDITION}"),
        [],
    ))
}

/// The file was renamed or moved: the source follows it, and so do the chunk ids of its mentions
/// (`path#p1#s1` becomes `new_path#p1#s1`, the rewrite the index itself uses). No-op when the
/// source is unknown; refused when the new path already names another source.
pub fn move_source(
    tx: &Transaction<'_>,
    domain: Domain,
    old_path: &str,
    new_path: &str,
) -> Result<bool, AppError> {
    if old_path == new_path {
        return Ok(false);
    }
    let Some(id) = source_id(tx, domain, old_path)? else {
        return Ok(false);
    };
    if source_id(tx, domain, new_path)?.is_some() {
        return Err(AppError::KnowledgeUnavailable);
    }
    db(tx.execute(
        "UPDATE kb_sources SET relative_path = ?1 WHERE source_id = ?2",
        params![new_path, id],
    ))?;
    db(tx.execute(
        "UPDATE kb_mentions
         SET chunk_id = ?1 || substr(chunk_id, length(?2) + 1)
         WHERE source_id = ?3 AND locator_kind = 'chunk'
           AND substr(chunk_id, 1, length(?2) + 1) = ?2 || '#'",
        params![new_path, old_path, id],
    ))?;
    Ok(true)
}

// ---------------------------------------------------------------------------------------------
// Applying a delta
// ---------------------------------------------------------------------------------------------

type MentionKey = (
    i64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

#[derive(Debug, PartialEq)]
struct MentionValues {
    alias_id: Option<i64>,
    first_row: Option<i64>,
    sample_rows: Option<String>,
    occurrence_count: i64,
    confidence: f64,
    method: String,
    extractor_version: i64,
}

fn confidence_value(confidence: f32) -> f64 {
    f64::from(confidence).clamp(0.0, 1.0)
}

struct LocatorColumns {
    kind: &'static str,
    chunk_id: Option<String>,
    sheet: Option<String>,
    column_name: Option<String>,
    first_row: Option<i64>,
    sample_rows: Option<String>,
}

fn locator_columns(locator: &MentionLocator) -> LocatorColumns {
    match locator {
        MentionLocator::Chunk { chunk_id } => LocatorColumns {
            kind: "chunk",
            chunk_id: Some(chunk_id.clone()),
            sheet: None,
            column_name: None,
            first_row: None,
            sample_rows: None,
        },
        MentionLocator::Cells {
            sheet,
            column_name,
            first_row,
            sample_rows,
        } => {
            let kept: Vec<u32> = sample_rows.iter().copied().take(3).collect();
            LocatorColumns {
                kind: "cells",
                chunk_id: None,
                sheet: Some(sheet.clone()),
                column_name: Some(column_name.clone()),
                first_row: first_row.map(i64::from),
                sample_rows: if kept.is_empty() {
                    None
                } else {
                    Some(json!(kept).to_string())
                },
            }
        }
        MentionLocator::Filename => LocatorColumns {
            kind: "filename",
            chunk_id: None,
            sheet: None,
            column_name: None,
            first_row: None,
            sample_rows: None,
        },
        MentionLocator::Metadata => LocatorColumns {
            kind: "metadata",
            chunk_id: None,
            sheet: None,
            column_name: None,
            first_row: None,
            sample_rows: None,
        },
    }
}

fn mention_key(entity_id: i64, columns: &LocatorColumns, role: &Option<String>) -> MentionKey {
    (
        entity_id,
        columns.kind.to_string(),
        columns.chunk_id.clone(),
        columns.sheet.clone(),
        columns.column_name.clone(),
        role.clone(),
    )
}

fn mention_values(
    draft: &MentionDraft,
    columns: &LocatorColumns,
    alias_id: Option<i64>,
) -> MentionValues {
    MentionValues {
        alias_id,
        first_row: columns.first_row,
        sample_rows: columns.sample_rows.clone(),
        occurrence_count: i64::from(draft.occurrence_count),
        confidence: confidence_value(draft.confidence),
        method: draft.method.clone(),
        extractor_version: i64::from(draft.extractor_version),
    }
}

fn insert_mention(
    connection: &Connection,
    source_id: i64,
    entity_id: i64,
    draft: &MentionDraft,
    alias_id: Option<i64>,
    origin: Origin,
) -> Result<i64, AppError> {
    let columns = locator_columns(&draft.locator);
    let values = mention_values(draft, &columns, alias_id);
    db(connection.execute(
        "INSERT INTO kb_mentions
            (entity_id, source_id, alias_id, role, locator_kind, chunk_id, sheet, column_name,
             first_row, sample_rows, occurrence_count, confidence, method, origin, extractor_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            entity_id,
            source_id,
            values.alias_id,
            draft.role,
            columns.kind,
            columns.chunk_id,
            columns.sheet,
            columns.column_name,
            values.first_row,
            values.sample_rows,
            values.occurrence_count,
            values.confidence,
            values.method,
            origin.as_code(),
            values.extractor_version,
        ],
    ))?;
    Ok(connection.last_insert_rowid())
}

/// Attach a manual mention to an entity inside a known source. `draft.entity` is ignored: the
/// entity is the argument.
pub fn add_mention(
    connection: &Connection,
    source_id: i64,
    entity_id: i64,
    draft: &MentionDraft,
    origin: Origin,
) -> Result<i64, AppError> {
    insert_mention(connection, source_id, entity_id, draft, None, origin)
}

/// Make the automatic rows of the delta's source equal to the delta, in the caller's transaction.
///
/// Rows that are already identical are not written (so a second pass over unchanged files changes
/// nothing, ids and timestamps included); rows the delta no longer carries are removed; manual rows
/// are never touched. A draft whose name is a tombstone is dropped with everything that points at
/// it: the user deleted that name and a re-analysis must not bring it back.
pub fn apply_delta(tx: &Transaction<'_>, delta: &KnowledgeDelta) -> Result<ApplyOutcome, AppError> {
    let source_id = upsert_source(tx, &delta.source, delta.kb_version, delta.gazetteer_epoch)?;
    let mut outcome = ApplyOutcome::default();
    let mut touched: BTreeSet<i64> = BTreeSet::new();

    let mut drafts: HashMap<u32, Option<i64>> = HashMap::new();
    for draft in &delta.entities {
        let resolved = resolve_draft(tx, draft, &mut outcome, &mut touched)?;
        drafts.insert(draft.temp_id, resolved);
    }

    for alias in &delta.aliases {
        if let Some(entity_id) = resolve_ref(tx, &drafts, alias.entity)? {
            let new_alias = NewAlias {
                display: alias.display.clone(),
                normalized: alias.normalized.clone(),
                phonetic_key: alias.phonetic_key.clone(),
                kind: alias.kind,
                confidence: alias.confidence,
            };
            if upsert_alias(tx, entity_id, &new_alias, Origin::Automatic)?.1 {
                touched.insert(entity_id);
            }
        }
    }

    apply_mentions(tx, source_id, delta, &drafts, &mut touched)?;
    apply_relations(tx, source_id, &delta.relations, &drafts, &mut touched)?;
    apply_attributes(tx, source_id, &delta.attributes, &drafts, &mut touched)?;
    apply_signals(tx, source_id, &delta.signals)?;

    for entity_id in &touched {
        db(tx.execute(
            "UPDATE kb_entities SET updated_at = CAST(strftime('%s', 'now') AS INTEGER)
             WHERE entity_id = ?1",
            [entity_id],
        ))?;
    }
    gc_orphans(tx)?;
    Ok(outcome)
}

/// Find the entity a draft names, or create it. `None` when the name is a tombstone.
fn resolve_draft(
    connection: &Connection,
    draft: &EntityDraft,
    outcome: &mut ApplyOutcome,
    touched: &mut BTreeSet<i64>,
) -> Result<Option<i64>, AppError> {
    if draft.canonical_name.trim().is_empty()
        || draft.normalized_name.trim().is_empty()
        || !matches!(draft.status, EntityStatus::Candidate | EntityStatus::Active)
    {
        return Err(AppError::Internal);
    }
    let found: Option<(i64, String)> = db(connection
        .query_row(
            "SELECT entity_id, status FROM kb_entities
             WHERE type_id = ?1 AND normalized_name = ?2 AND disambiguator = ?3
             ORDER BY (status = 'merged') ASC, entity_id ASC LIMIT 1",
            params![draft.type_id, draft.normalized_name, draft.disambiguator],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional())?;
    let Some((entity_id, status)) = found else {
        db(connection.execute(
            "INSERT INTO kb_entities
                (type_id, subtype, canonical_name, normalized_name, disambiguator, status, origin,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'automatic',
                     CAST(strftime('%s', 'now') AS INTEGER), CAST(strftime('%s', 'now') AS INTEGER))",
            params![
                draft.type_id,
                draft.subtype,
                draft.canonical_name,
                draft.normalized_name,
                draft.disambiguator,
                draft.status.as_code(),
            ],
        ))?;
        outcome.entities_created += 1;
        return Ok(Some(connection.last_insert_rowid()));
    };
    match EntityStatus::from_code(&status) {
        Some(EntityStatus::Deleted) => {
            outcome.entities_suppressed += 1;
            Ok(None)
        }
        Some(EntityStatus::Merged) => {
            // A name that was merged leads to the survivor; if that one was deleted, the name is
            // suppressed with it.
            match effective_entity(connection, entity_id)? {
                Some(survivor) if survivor.status != EntityStatus::Deleted => {
                    outcome.entities_matched += 1;
                    Ok(Some(survivor.entity_id))
                }
                _ => {
                    outcome.entities_suppressed += 1;
                    Ok(None)
                }
            }
        }
        Some(EntityStatus::Candidate) | Some(EntityStatus::Active) => {
            outcome.entities_matched += 1;
            if status == "candidate" && draft.status == EntityStatus::Active {
                db(connection.execute(
                    "UPDATE kb_entities SET status = 'active' WHERE entity_id = ?1",
                    [entity_id],
                ))?;
                touched.insert(entity_id);
            }
            Ok(Some(entity_id))
        }
        None => Err(AppError::KnowledgeUnavailable),
    }
}

/// The entity a reference points at, after following merges. `None` for a suppressed draft or a
/// deleted entity; an error for a reference to nothing.
fn resolve_ref(
    connection: &Connection,
    drafts: &HashMap<u32, Option<i64>>,
    reference: EntityRef,
) -> Result<Option<i64>, AppError> {
    match reference {
        EntityRef::Draft(temp_id) => drafts.get(&temp_id).copied().ok_or(AppError::Internal),
        EntityRef::Existing(entity_id) => match effective_entity(connection, entity_id)? {
            None => Err(AppError::KnowledgeEntityNotFound),
            Some(entity) if entity.status == EntityStatus::Deleted => Ok(None),
            Some(entity) => Ok(Some(entity.entity_id)),
        },
    }
}

fn apply_mentions(
    tx: &Transaction<'_>,
    source_id: i64,
    delta: &KnowledgeDelta,
    drafts: &HashMap<u32, Option<i64>>,
    touched: &mut BTreeSet<i64>,
) -> Result<(), AppError> {
    // What the source holds now, keyed by the entity it leads to after merges: a mention that sits
    // on a merged entity still matches the delta's mention of the survivor, so an unchanged file
    // rewrites nothing.
    let mut existing: HashMap<MentionKey, Vec<(i64, MentionValues)>> = HashMap::new();
    {
        let mut statement = db(tx.prepare(
            "SELECT m.mention_id, v.effective_id, m.alias_id, m.role, m.locator_kind, m.chunk_id,
                    m.sheet, m.column_name, m.first_row, m.sample_rows, m.occurrence_count,
                    m.confidence, m.method, m.extractor_version
             FROM kb_mentions m
             JOIN kb_effective_entity v ON v.entity_id = m.entity_id
             WHERE m.source_id = ?1 AND m.origin = 'automatic'
             ORDER BY m.mention_id",
        ))?;
        let rows = db(statement.query_map([source_id], |row| {
            let key: MentionKey = (
                row.get(1)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(3)?,
            );
            let values = MentionValues {
                alias_id: row.get(2)?,
                first_row: row.get(8)?,
                sample_rows: row.get(9)?,
                occurrence_count: row.get(10)?,
                confidence: row.get(11)?,
                method: row.get(12)?,
                extractor_version: row.get(13)?,
            };
            Ok((row.get::<_, i64>(0)?, key, values))
        }))?;
        for row in rows {
            let (mention_id, key, values) = db(row)?;
            existing.entry(key).or_default().push((mention_id, values));
        }
    }

    for draft in &delta.mentions {
        let Some(entity_id) = resolve_ref(tx, drafts, draft.entity)? else {
            continue;
        };
        let columns = locator_columns(&draft.locator);
        let alias_id = match &draft.alias_normalized {
            Some(normalized) => db(tx
                .query_row(
                    "SELECT alias_id FROM kb_aliases WHERE entity_id = ?1 AND normalized = ?2",
                    params![entity_id, normalized],
                    |row| row.get(0),
                )
                .optional())?,
            None => None,
        };
        let key = mention_key(entity_id, &columns, &draft.role);
        let wanted = mention_values(draft, &columns, alias_id);
        let kept = existing.get_mut(&key).and_then(|rows| {
            if rows.is_empty() {
                None
            } else {
                Some(rows.remove(0))
            }
        });
        match kept {
            Some((_, current)) if current == wanted => {}
            Some((mention_id, _)) => {
                db(tx.execute(
                    "UPDATE kb_mentions SET alias_id = ?1, first_row = ?2, sample_rows = ?3,
                            occurrence_count = ?4, confidence = ?5, method = ?6,
                            extractor_version = ?7
                     WHERE mention_id = ?8",
                    params![
                        wanted.alias_id,
                        wanted.first_row,
                        wanted.sample_rows,
                        wanted.occurrence_count,
                        wanted.confidence,
                        wanted.method,
                        wanted.extractor_version,
                        mention_id,
                    ],
                ))?;
                touched.insert(entity_id);
            }
            None => {
                insert_mention(tx, source_id, entity_id, draft, alias_id, Origin::Automatic)?;
                touched.insert(entity_id);
            }
        }
    }

    for (key, rows) in existing {
        for (mention_id, _) in rows {
            db(tx.execute(
                "DELETE FROM kb_mentions WHERE mention_id = ?1",
                [mention_id],
            ))?;
            touched.insert(key.0);
        }
    }
    Ok(())
}

fn apply_relations(
    tx: &Transaction<'_>,
    source_id: i64,
    relations: &[RelationDraft],
    drafts: &HashMap<u32, Option<i64>>,
    touched: &mut BTreeSet<i64>,
) -> Result<(), AppError> {
    type Key = (i64, String, i64);
    let mut existing: HashMap<Key, Vec<(i64, i64, f64)>> = HashMap::new();
    {
        let mut statement = db(tx.prepare(
            "SELECT r.relation_id, vs.effective_id, r.predicate, vo.effective_id,
                    r.occurrence_count, r.confidence
             FROM kb_relations r
             JOIN kb_effective_entity vs ON vs.entity_id = r.subject
             JOIN kb_effective_entity vo ON vo.entity_id = r.object
             WHERE r.source_id = ?1 AND r.origin = 'automatic'
             ORDER BY r.relation_id",
        ))?;
        let rows = db(statement.query_map([source_id], |row| {
            Ok((
                (row.get(1)?, row.get(2)?, row.get(3)?),
                (row.get::<_, i64>(0)?, row.get(4)?, row.get(5)?),
            ))
        }))?;
        for row in rows {
            let (key, value): (Key, (i64, i64, f64)) = db(row)?;
            existing.entry(key).or_default().push(value);
        }
    }

    for relation in relations {
        let (Some(subject), Some(object)) = (
            resolve_ref(tx, drafts, relation.subject)?,
            resolve_ref(tx, drafts, relation.object)?,
        ) else {
            continue;
        };
        let count = i64::from(relation.occurrence_count);
        let confidence = confidence_value(relation.confidence);
        let key = (subject, relation.predicate.clone(), object);
        let kept = existing.get_mut(&key).and_then(|rows| {
            if rows.is_empty() {
                None
            } else {
                Some(rows.remove(0))
            }
        });
        match kept {
            Some((_, current_count, current_confidence))
                if current_count == count && current_confidence == confidence => {}
            Some((relation_id, _, _)) => {
                db(tx.execute(
                    "UPDATE kb_relations SET occurrence_count = ?1, confidence = ?2
                     WHERE relation_id = ?3",
                    params![count, confidence, relation_id],
                ))?;
                touched.insert(subject);
                touched.insert(object);
            }
            None => {
                db(tx.execute(
                    "INSERT INTO kb_relations
                        (subject, predicate, object, source_id, occurrence_count, confidence, origin)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'automatic')",
                    params![subject, relation.predicate, object, source_id, count, confidence],
                ))?;
                touched.insert(subject);
                touched.insert(object);
            }
        }
    }

    for (key, rows) in existing {
        for (relation_id, _, _) in rows {
            db(tx.execute(
                "DELETE FROM kb_relations WHERE relation_id = ?1",
                [relation_id],
            ))?;
            touched.insert(key.0);
            touched.insert(key.2);
        }
    }
    Ok(())
}

fn apply_attributes(
    tx: &Transaction<'_>,
    source_id: i64,
    attributes: &[AttributeDraft],
    drafts: &HashMap<u32, Option<i64>>,
    touched: &mut BTreeSet<i64>,
) -> Result<(), AppError> {
    type Key = (i64, String, String);
    let mut existing: HashMap<Key, Vec<i64>> = HashMap::new();
    {
        let mut statement = db(tx.prepare(
            "SELECT a.attribute_id, v.effective_id, a.key, a.value
             FROM kb_attributes a
             JOIN kb_effective_entity v ON v.entity_id = a.entity_id
             WHERE a.source_id = ?1 AND a.origin = 'automatic'
             ORDER BY a.attribute_id",
        ))?;
        let rows = db(statement.query_map([source_id], |row| {
            Ok((
                (row.get(1)?, row.get(2)?, row.get(3)?),
                row.get::<_, i64>(0)?,
            ))
        }))?;
        for row in rows {
            let (key, attribute_id): (Key, i64) = db(row)?;
            existing.entry(key).or_default().push(attribute_id);
        }
    }

    for attribute in attributes {
        let Some(entity_id) = resolve_ref(tx, drafts, attribute.entity)? else {
            continue;
        };
        let key = (entity_id, attribute.key.clone(), attribute.value.clone());
        let kept = existing.get_mut(&key).and_then(|rows| {
            if rows.is_empty() {
                None
            } else {
                Some(rows.remove(0))
            }
        });
        if kept.is_none() {
            let written = db(tx.execute(
                "INSERT OR IGNORE INTO kb_attributes (entity_id, key, value, origin, source_id)
                 VALUES (?1, ?2, ?3, 'automatic', ?4)",
                params![entity_id, attribute.key, attribute.value, source_id],
            ))?;
            if written > 0 {
                touched.insert(entity_id);
            }
        }
    }

    for (key, rows) in existing {
        for attribute_id in rows {
            db(tx.execute(
                "DELETE FROM kb_attributes WHERE attribute_id = ?1",
                [attribute_id],
            ))?;
            touched.insert(key.0);
        }
    }
    Ok(())
}

fn apply_signals(
    tx: &Transaction<'_>,
    source_id: i64,
    signals: &[SignalDraft],
) -> Result<(), AppError> {
    let mut existing: HashMap<String, (i64, i64)> = HashMap::new();
    {
        let mut statement = db(tx.prepare(
            "SELECT pack_id, hits, distinct_terms FROM kb_source_signals WHERE source_id = ?1",
        ))?;
        let rows = db(statement.query_map([source_id], |row| {
            Ok((row.get::<_, String>(0)?, (row.get(1)?, row.get(2)?)))
        }))?;
        for row in rows {
            let (pack_id, value) = db(row)?;
            existing.insert(pack_id, value);
        }
    }
    for signal in signals {
        let wanted = (i64::from(signal.hits), i64::from(signal.distinct_terms));
        match existing.remove(&signal.pack_id) {
            Some(current) if current == wanted => {}
            _ => {
                db(tx.execute(
                    "INSERT INTO kb_source_signals (source_id, pack_id, hits, distinct_terms)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT (source_id, pack_id) DO UPDATE SET
                        hits = excluded.hits, distinct_terms = excluded.distinct_terms",
                    params![source_id, signal.pack_id, wanted.0, wanted.1],
                ))?;
            }
        }
    }
    for pack_id in existing.keys() {
        db(tx.execute(
            "DELETE FROM kb_source_signals WHERE source_id = ?1 AND pack_id = ?2",
            params![source_id, pack_id],
        ))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Entities, aliases, attributes: shared primitives
// ---------------------------------------------------------------------------------------------

const ENTITY_COLUMNS: [&str; 9] = [
    "entity_id",
    "type_id",
    "subtype",
    "canonical_name",
    "normalized_name",
    "disambiguator",
    "status",
    "merged_into",
    "origin",
];

/// The columns `entity_from_row` reads, prefixed with a table alias.
fn entity_columns(alias: &str) -> String {
    ENTITY_COLUMNS
        .iter()
        .map(|column| format!("{alias}.{column}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn bad_code(index: usize, code: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Text,
        format!("unknown code {code}").into(),
    )
}

fn entity_from_row(row: &Row<'_>) -> rusqlite::Result<Entity> {
    let status: String = row.get(6)?;
    let origin: String = row.get(8)?;
    Ok(Entity {
        entity_id: row.get(0)?,
        type_id: row.get(1)?,
        subtype: row.get(2)?,
        canonical_name: row.get(3)?,
        normalized_name: row.get(4)?,
        disambiguator: row.get(5)?,
        status: EntityStatus::from_code(&status).ok_or_else(|| bad_code(6, &status))?,
        merged_into: row.get(7)?,
        origin: Origin::from_code(&origin).ok_or_else(|| bad_code(8, &origin))?,
    })
}

fn alias_from_row(row: &Row<'_>) -> rusqlite::Result<Alias> {
    let kind: String = row.get(5)?;
    let origin: String = row.get(6)?;
    Ok(Alias {
        alias_id: row.get(0)?,
        entity_id: row.get(1)?,
        display: row.get(2)?,
        normalized: row.get(3)?,
        phonetic_key: row.get(4)?,
        kind: AliasKind::from_code(&kind).ok_or_else(|| bad_code(5, &kind))?,
        origin: Origin::from_code(&origin).ok_or_else(|| bad_code(6, &origin))?,
        confidence: row.get::<_, f64>(7)? as f32,
    })
}

pub fn get_entity(connection: &Connection, entity_id: i64) -> Result<Option<Entity>, AppError> {
    db(connection
        .query_row(
            &format!(
                "SELECT {} FROM kb_entities e WHERE e.entity_id = ?1",
                entity_columns("e")
            ),
            [entity_id],
            entity_from_row,
        )
        .optional())
}

/// The entity a given one stands for once merges are followed (itself when it is not merged).
/// `None` when the id names nothing. A deleted survivor is returned as it is, tombstone and all.
pub fn effective_entity(
    connection: &Connection,
    entity_id: i64,
) -> Result<Option<Entity>, AppError> {
    db(connection
        .query_row(
            &format!(
                "SELECT {} FROM kb_effective_entity v
                 JOIN kb_entities s ON s.entity_id = v.effective_id
                 WHERE v.entity_id = ?1",
                entity_columns("s")
            ),
            [entity_id],
            entity_from_row,
        )
        .optional())
}

/// Create an entity by hand. A name that already exists (live or deleted) for the same type and
/// disambiguator is refused by the unique index and comes back as `KnowledgeUnavailable`: the
/// caller looks first.
pub fn create_entity(connection: &Connection, entity: &NewEntity) -> Result<i64, AppError> {
    if entity.canonical_name.trim().is_empty()
        || entity.normalized_name.trim().is_empty()
        || !matches!(
            entity.status,
            EntityStatus::Candidate | EntityStatus::Active
        )
    {
        return Err(AppError::Internal);
    }
    db(connection.execute(
        "INSERT INTO kb_entities
            (type_id, subtype, canonical_name, normalized_name, disambiguator, status, origin,
             created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                 CAST(strftime('%s', 'now') AS INTEGER), CAST(strftime('%s', 'now') AS INTEGER))",
        params![
            entity.type_id,
            entity.subtype,
            entity.canonical_name,
            entity.normalized_name,
            entity.disambiguator,
            entity.status.as_code(),
            entity.origin.as_code(),
        ],
    ))?;
    Ok(connection.last_insert_rowid())
}

/// Insert or refresh an alias; returns its id and whether a row was written. An automatic write
/// never overrides a manual alias; a manual write turns an automatic one manual.
fn upsert_alias(
    connection: &Connection,
    entity_id: i64,
    alias: &NewAlias,
    origin: Origin,
) -> Result<(i64, bool), AppError> {
    if alias.normalized.trim().is_empty() {
        return Err(AppError::Internal);
    }
    let written = db(connection.execute(
        "INSERT INTO kb_aliases (entity_id, display, normalized, phonetic_key, kind, origin, confidence)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT (entity_id, normalized) DO UPDATE SET
            display = excluded.display,
            phonetic_key = excluded.phonetic_key,
            kind = excluded.kind,
            confidence = excluded.confidence,
            origin = excluded.origin
         WHERE (kb_aliases.origin = 'automatic' OR excluded.origin = 'manual')
           AND (kb_aliases.display IS NOT excluded.display
                OR kb_aliases.phonetic_key IS NOT excluded.phonetic_key
                OR kb_aliases.kind IS NOT excluded.kind
                OR kb_aliases.confidence IS NOT excluded.confidence
                OR kb_aliases.origin IS NOT excluded.origin)",
        params![
            entity_id,
            alias.display,
            alias.normalized,
            alias.phonetic_key,
            alias.kind.as_code(),
            origin.as_code(),
            confidence_value(alias.confidence),
        ],
    ))?;
    let alias_id = db(connection.query_row(
        "SELECT alias_id FROM kb_aliases WHERE entity_id = ?1 AND normalized = ?2",
        params![entity_id, alias.normalized],
        |row| row.get(0),
    ))?;
    Ok((alias_id, written > 0))
}

pub fn add_alias(
    connection: &Connection,
    entity_id: i64,
    alias: &NewAlias,
    origin: Origin,
) -> Result<i64, AppError> {
    Ok(upsert_alias(connection, entity_id, alias, origin)?.0)
}

/// Remove one alias (its name-index row goes with it through the trigger).
pub fn remove_alias(connection: &Connection, alias_id: i64) -> Result<bool, AppError> {
    Ok(db(connection.execute("DELETE FROM kb_aliases WHERE alias_id = ?1", [alias_id]))? > 0)
}

/// State a fact about an entity. With `source_id: None` the row is manual and survives any
/// re-analysis; with a source it lives and dies with that source.
pub fn set_attribute(
    connection: &Connection,
    entity_id: i64,
    key: &str,
    value: &str,
    origin: Origin,
    source_id: Option<i64>,
) -> Result<(), AppError> {
    db(connection.execute(
        "INSERT OR IGNORE INTO kb_attributes (entity_id, key, value, origin, source_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![entity_id, key, value, origin.as_code(), source_id],
    ))?;
    Ok(())
}

pub fn remove_attribute(
    connection: &Connection,
    entity_id: i64,
    key: &str,
    value: &str,
) -> Result<usize, AppError> {
    db(connection.execute(
        "DELETE FROM kb_attributes WHERE entity_id = ?1 AND key = ?2 AND value = ?3
           AND origin = 'manual'",
        params![entity_id, key, value],
    ))
}

// ---------------------------------------------------------------------------------------------
// Merge, unmerge, delete, restore: the entity operations a person performs
// ---------------------------------------------------------------------------------------------

fn log_op(connection: &Connection, kind: &str, payload: Value) -> Result<(), AppError> {
    db(connection.execute(
        "INSERT INTO kb_ops_log (kind, payload_json, at)
         VALUES (?1, ?2, CAST(strftime('%s', 'now') AS INTEGER))",
        params![kind, payload.to_string()],
    ))?;
    Ok(())
}

/// The live entity an id leads to, or the reason it cannot be used.
fn live_effective(connection: &Connection, entity_id: i64) -> Result<Entity, AppError> {
    match effective_entity(connection, entity_id)? {
        None => Err(AppError::KnowledgeEntityNotFound),
        Some(entity) if entity.status == EntityStatus::Deleted => {
            Err(AppError::KnowledgeMergeRefused)
        }
        Some(entity) => Ok(entity),
    }
}

/// Redirect `victim` to `survivor`. Nothing is moved: the victim keeps its mentions and aliases and
/// every read goes through the effective view. Both ids are first resolved to what they currently
/// stand for, which is what makes a cycle impossible (merging B into A and then A into B finds both
/// already equal) and keeps chains one hop long: whatever pointed at the victim now points at the
/// survivor. Entities of different types are never merged.
pub fn merge_entities(tx: &Transaction<'_>, survivor: i64, victim: i64) -> Result<(), AppError> {
    let survivor = live_effective(tx, survivor)?;
    let victim = live_effective(tx, victim)?;
    if survivor.entity_id == victim.entity_id || survivor.type_id != victim.type_id {
        return Err(AppError::KnowledgeMergeRefused);
    }

    let redirected: Vec<i64> = {
        let mut statement = db(tx.prepare(
            "SELECT entity_id FROM kb_entities WHERE merged_into = ?1 ORDER BY entity_id",
        ))?;
        let rows = db(statement.query_map([victim.entity_id], |row| row.get(0)))?;
        db(rows.collect())?
    };
    db(tx.execute(
        "UPDATE kb_entities SET merged_into = ?1,
                updated_at = CAST(strftime('%s', 'now') AS INTEGER)
         WHERE merged_into = ?2",
        params![survivor.entity_id, victim.entity_id],
    ))?;
    db(tx.execute(
        "UPDATE kb_entities SET status = 'merged', merged_into = ?1,
                updated_at = CAST(strftime('%s', 'now') AS INTEGER)
         WHERE entity_id = ?2",
        params![survivor.entity_id, victim.entity_id],
    ))?;
    log_op(
        tx,
        "merge",
        json!({
            "survivor": survivor.entity_id,
            "victim": victim.entity_id,
            "victim_previous_status": victim.status.as_code(),
            "redirected": redirected,
        }),
    )
}

/// Undo a merge: the entity becomes itself again, and whatever the merge had redirected to the
/// survivor on its behalf points at it once more. Refused for an entity that is not merged, and
/// when a live entity has taken its name in the meantime.
pub fn unmerge_entity(tx: &Transaction<'_>, entity_id: i64) -> Result<(), AppError> {
    let entity = get_entity(tx, entity_id)?.ok_or(AppError::KnowledgeEntityNotFound)?;
    if entity.status != EntityStatus::Merged {
        return Err(AppError::KnowledgeMergeRefused);
    }
    let payload: Option<String> = db(tx
        .query_row(
            "SELECT payload_json FROM kb_ops_log
             WHERE kind = 'merge' AND json_extract(payload_json, '$.victim') = ?1
             ORDER BY op_id DESC LIMIT 1",
            [entity_id],
            |row| row.get(0),
        )
        .optional())?;
    let payload: Value = payload
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    let previous = payload["victim_previous_status"]
        .as_str()
        .and_then(EntityStatus::from_code)
        .filter(|status| matches!(status, EntityStatus::Candidate | EntityStatus::Active))
        .unwrap_or(EntityStatus::Active);
    let survivor = payload["survivor"].as_i64().or(entity.merged_into);
    let redirected: Vec<i64> = payload["redirected"]
        .as_array()
        .map(|ids| ids.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default();

    let name_taken: bool = db(tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM kb_entities
                        WHERE type_id = ?1 AND normalized_name = ?2 AND disambiguator = ?3
                          AND status IN ('candidate', 'active', 'deleted') AND entity_id <> ?4)",
        params![
            entity.type_id,
            entity.normalized_name,
            entity.disambiguator,
            entity.entity_id
        ],
        |row| row.get(0),
    ))?;
    if name_taken {
        return Err(AppError::KnowledgeMergeRefused);
    }

    db(tx.execute(
        "UPDATE kb_entities SET status = ?1, merged_into = NULL,
                updated_at = CAST(strftime('%s', 'now') AS INTEGER)
         WHERE entity_id = ?2",
        params![previous.as_code(), entity.entity_id],
    ))?;
    if let Some(survivor) = survivor {
        for id in &redirected {
            db(tx.execute(
                "UPDATE kb_entities SET merged_into = ?1 WHERE entity_id = ?2 AND status = 'merged'
                   AND merged_into = ?3",
                params![entity.entity_id, id, survivor],
            ))?;
        }
    }
    log_op(
        tx,
        "unmerge",
        json!({ "entity": entity.entity_id, "survivor": survivor }),
    )
}

/// Delete an entity the way a person does: a tombstone. Its mentions and relations go, its name
/// stays (suppressed), its aliases and attributes stay for a restore. No file, chunk or table row
/// outside the knowledge base is touched. Deleting a merged entity is refused: unmerge it, or
/// delete the survivor.
pub fn delete_entity(tx: &Transaction<'_>, entity_id: i64) -> Result<(), AppError> {
    let entity = get_entity(tx, entity_id)?.ok_or(AppError::KnowledgeEntityNotFound)?;
    match entity.status {
        EntityStatus::Deleted => return Ok(()),
        EntityStatus::Merged => return Err(AppError::KnowledgeMergeRefused),
        EntityStatus::Candidate | EntityStatus::Active => {}
    }
    db(tx.execute("DELETE FROM kb_mentions WHERE entity_id = ?1", [entity_id]))?;
    db(tx.execute(
        "DELETE FROM kb_relations WHERE subject = ?1 OR object = ?1",
        [entity_id],
    ))?;
    db(tx.execute(
        "UPDATE kb_entities SET status = 'deleted',
                updated_at = CAST(strftime('%s', 'now') AS INTEGER)
         WHERE entity_id = ?1",
        [entity_id],
    ))?;
    log_op(
        tx,
        "delete",
        json!({ "entity": entity_id, "previous_status": entity.status.as_code() }),
    )
}

/// Bring a tombstone back. Its mentions return with the next analysis of the files that held them.
pub fn restore_entity(tx: &Transaction<'_>, entity_id: i64) -> Result<(), AppError> {
    let entity = get_entity(tx, entity_id)?.ok_or(AppError::KnowledgeEntityNotFound)?;
    if entity.status != EntityStatus::Deleted {
        return Err(AppError::KnowledgeMergeRefused);
    }
    let payload: Option<String> = db(tx
        .query_row(
            "SELECT payload_json FROM kb_ops_log
             WHERE kind = 'delete' AND json_extract(payload_json, '$.entity') = ?1
             ORDER BY op_id DESC LIMIT 1",
            [entity_id],
            |row| row.get(0),
        )
        .optional())?;
    let previous = payload
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| {
            value["previous_status"]
                .as_str()
                .and_then(EntityStatus::from_code)
        })
        .filter(|status| matches!(status, EntityStatus::Candidate | EntityStatus::Active))
        .unwrap_or(EntityStatus::Active);
    db(tx.execute(
        "UPDATE kb_entities SET status = ?1,
                updated_at = CAST(strftime('%s', 'now') AS INTEGER)
         WHERE entity_id = ?2",
        params![previous.as_code(), entity_id],
    ))?;
    log_op(tx, "restore", json!({ "entity": entity_id }))
}

// ---------------------------------------------------------------------------------------------
// Reads used by the later lots
// ---------------------------------------------------------------------------------------------

/// The live entities (candidate or active) whose canonical normalized name is `normalized`, after
/// following merges. Tombstones are not returned: ask `is_tombstoned`.
pub fn find_entities_by_normalized(
    connection: &Connection,
    normalized: &str,
) -> Result<Vec<Entity>, AppError> {
    let mut statement = db(connection.prepare(&format!(
        "SELECT DISTINCT {} FROM kb_entities e
         JOIN kb_effective_entity v ON v.entity_id = e.entity_id
         JOIN kb_entities s ON s.entity_id = v.effective_id
         WHERE e.normalized_name = ?1 AND s.status IN ('candidate', 'active')
         ORDER BY s.entity_id",
        entity_columns("s")
    )))?;
    let rows = db(statement.query_map([normalized], entity_from_row))?;
    db(rows.collect())
}

/// The live entities that have an alias with this phonetic key, after following merges.
pub fn find_entities_by_phonetic(
    connection: &Connection,
    phonetic_key: &str,
) -> Result<Vec<Entity>, AppError> {
    if phonetic_key.is_empty() {
        return Ok(Vec::new());
    }
    let mut statement = db(connection.prepare(&format!(
        "SELECT DISTINCT {} FROM kb_aliases a
         JOIN kb_effective_entity v ON v.entity_id = a.entity_id
         JOIN kb_entities s ON s.entity_id = v.effective_id
         WHERE a.phonetic_key = ?1 AND s.status IN ('candidate', 'active')
         ORDER BY s.entity_id",
        entity_columns("s")
    )))?;
    let rows = db(statement.query_map([phonetic_key], entity_from_row))?;
    db(rows.collect())
}

/// The columns of a joined `(entity, alias)` row: the nine entity columns of the survivor, then the
/// eight alias columns.
fn entity_alias_columns() -> String {
    format!(
        "{}, a.alias_id, a.entity_id, a.display, a.normalized, a.phonetic_key, a.kind, a.origin, a.confidence",
        entity_columns("s")
    )
}

fn entity_alias_from_row(row: &Row<'_>) -> rusqlite::Result<(Entity, Alias)> {
    let entity = entity_from_row(row)?;
    let kind: String = row.get(14)?;
    let origin: String = row.get(15)?;
    let alias = Alias {
        alias_id: row.get(9)?,
        entity_id: row.get(10)?,
        display: row.get(11)?,
        normalized: row.get(12)?,
        phonetic_key: row.get(13)?,
        kind: AliasKind::from_code(&kind).ok_or_else(|| bad_code(14, &kind))?,
        origin: Origin::from_code(&origin).ok_or_else(|| bad_code(15, &origin))?,
        confidence: row.get::<_, f64>(16)? as f32,
    };
    Ok((entity, alias))
}

/// The live entities that answer to this exact alias, each with the alias that matched, after
/// following merges. The canonical name is also an alias of its entity, so this finds it too.
pub fn find_entities_by_alias(
    connection: &Connection,
    normalized: &str,
) -> Result<Vec<(Entity, Alias)>, AppError> {
    let mut statement = db(connection.prepare(&format!(
        "SELECT {} FROM kb_aliases a
         JOIN kb_effective_entity v ON v.entity_id = a.entity_id
         JOIN kb_entities s ON s.entity_id = v.effective_id
         WHERE a.normalized = ?1 AND s.status IN ('candidate', 'active')
         ORDER BY s.entity_id, a.alias_id",
        entity_alias_columns()
    )))?;
    let rows = db(statement.query_map([normalized], entity_alias_from_row))?;
    db(rows.collect())
}

/// Like `find_entities_by_phonetic`, with the alias whose key matched.
pub fn find_aliases_by_phonetic(
    connection: &Connection,
    phonetic_key: &str,
) -> Result<Vec<(Entity, Alias)>, AppError> {
    if phonetic_key.is_empty() {
        return Ok(Vec::new());
    }
    let mut statement = db(connection.prepare(&format!(
        "SELECT {} FROM kb_aliases a
         JOIN kb_effective_entity v ON v.entity_id = a.entity_id
         JOIN kb_entities s ON s.entity_id = v.effective_id
         WHERE a.phonetic_key = ?1 AND s.status IN ('candidate', 'active')
         ORDER BY s.entity_id, a.alias_id",
        entity_alias_columns()
    )))?;
    let rows = db(statement.query_map([phonetic_key], entity_alias_from_row))?;
    db(rows.collect())
}

/// The live entities with an alias that contains `word` as a whole word (at most `limit` rows).
/// The resolver narrows these with an exact comparison; the name index only finds the candidates.
pub fn find_aliases_with_word(
    connection: &Connection,
    word: &str,
    limit: usize,
) -> Result<Vec<(Entity, Alias)>, AppError> {
    let word: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
    if word.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let mut statement = db(connection.prepare(&format!(
        "SELECT {} FROM kb_names_fts
         JOIN kb_aliases a ON a.alias_id = kb_names_fts.rowid
         JOIN kb_effective_entity v ON v.entity_id = a.entity_id
         JOIN kb_entities s ON s.entity_id = v.effective_id
         WHERE kb_names_fts MATCH ?1 AND s.status IN ('candidate', 'active')
         ORDER BY s.entity_id, a.alias_id
         LIMIT ?2",
        entity_alias_columns()
    )))?;
    let rows = db(statement.query_map(
        params![format!("\"{word}\""), limit as i64],
        entity_alias_from_row,
    ))?;
    db(rows.collect())
}

/// The live entities that carry the identifier `normalized` of scheme `scheme`: through an
/// `identifier:<scheme>` attribute, or because the entity *is* the identifier (type `identifier`,
/// named by its reduced value).
pub fn find_entities_by_identifier(
    connection: &Connection,
    scheme: &str,
    normalized: &str,
) -> Result<Vec<Entity>, AppError> {
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let mut statement = db(connection.prepare(&format!(
        "SELECT DISTINCT {columns} FROM kb_attributes t
         JOIN kb_effective_entity v ON v.entity_id = t.entity_id
         JOIN kb_entities s ON s.entity_id = v.effective_id
         WHERE t.key = ?1 AND t.value = ?2 AND s.status IN ('candidate', 'active')
         UNION
         SELECT DISTINCT {columns} FROM kb_entities e
         JOIN kb_effective_entity v ON v.entity_id = e.entity_id
         JOIN kb_entities s ON s.entity_id = v.effective_id
         WHERE e.type_id = 'identifier' AND e.normalized_name = ?2
           AND s.status IN ('candidate', 'active')
         ORDER BY 1",
        columns = entity_columns("s")
    )))?;
    let rows = db(statement.query_map(
        params![format!("identifier:{scheme}"), normalized],
        entity_from_row,
    ))?;
    db(rows.collect())
}

/// One alias of a live entity as the in-memory name list wants it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameListRow {
    /// The entity after following merges.
    pub entity_id: i64,
    pub type_id: String,
    pub normalized: String,
    pub kind: AliasKind,
    pub origin: Origin,
}

/// Every alias the gazetteer should know: those of active entities, plus the manual ones (an alias
/// or an entity the user typed counts even while the entity is still a candidate). Candidates the
/// extractor guessed are left out, so a guess cannot make the next guess look like a certainty.
pub fn name_list_rows(connection: &Connection) -> Result<Vec<NameListRow>, AppError> {
    let mut statement = db(connection.prepare(
        "SELECT s.entity_id, s.type_id, a.normalized, a.kind, a.origin
         FROM kb_aliases a
         JOIN kb_effective_entity v ON v.entity_id = a.entity_id
         JOIN kb_entities s ON s.entity_id = v.effective_id
         WHERE s.status = 'active'
            OR (s.status = 'candidate' AND (s.origin = 'manual' OR a.origin = 'manual'))
         ORDER BY s.entity_id, a.alias_id",
    ))?;
    let rows = db(statement.query_map([], |row| {
        let kind: String = row.get(3)?;
        let origin: String = row.get(4)?;
        Ok(NameListRow {
            entity_id: row.get(0)?,
            type_id: row.get(1)?,
            normalized: row.get(2)?,
            kind: AliasKind::from_code(&kind).ok_or_else(|| bad_code(3, &kind))?,
            origin: Origin::from_code(&origin).ok_or_else(|| bad_code(4, &origin))?,
        })
    }))?;
    db(rows.collect())
}

/// One alias as the recomputation of phonetic keys needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasKeyRow {
    pub alias_id: i64,
    pub display: String,
    pub kind: AliasKind,
    pub phonetic_key: String,
}

/// Every alias with its display form, its kind and its stored key, for the recomputation that
/// follows a change of encoder.
pub fn alias_phonetic_rows(connection: &Connection) -> Result<Vec<AliasKeyRow>, AppError> {
    let mut statement = db(connection.prepare(
        "SELECT alias_id, display, kind, phonetic_key FROM kb_aliases ORDER BY alias_id",
    ))?;
    let rows = db(statement.query_map([], |row| {
        let kind: String = row.get(2)?;
        Ok(AliasKeyRow {
            alias_id: row.get(0)?,
            display: row.get(1)?,
            kind: AliasKind::from_code(&kind).ok_or_else(|| bad_code(2, &kind))?,
            phonetic_key: row.get(3)?,
        })
    }))?;
    db(rows.collect())
}

pub fn set_alias_phonetic_key(
    connection: &Connection,
    alias_id: i64,
    phonetic_key: &str,
) -> Result<(), AppError> {
    db(connection.execute(
        "UPDATE kb_aliases SET phonetic_key = ?2 WHERE alias_id = ?1 AND phonetic_key <> ?2",
        params![alias_id, phonetic_key],
    ))?;
    Ok(())
}

/// Whether the user deleted this name for this type, so a re-analysis must not recreate it.
pub fn is_tombstoned(
    connection: &Connection,
    type_id: &str,
    normalized: &str,
) -> Result<bool, AppError> {
    db(connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM kb_entities
                        WHERE type_id = ?1 AND normalized_name = ?2 AND status = 'deleted')",
        params![type_id, normalized],
        |row| row.get(0),
    ))
}

/// Autocomplete: the aliases whose words start with `prefix` (every word but the last must be
/// complete), best first, one hit per entity after merges, live entities only. There is no minimum
/// length: one letter is a valid prefix.
pub fn prefix_search(
    connection: &Connection,
    prefix: &str,
    limit: usize,
) -> Result<Vec<NameHit>, AppError> {
    let words: Vec<&str> = prefix
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    if words.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
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
    let mut statement = db(connection.prepare(
        "SELECT a.alias_id, v.effective_id, a.display, a.normalized
         FROM kb_names_fts
         JOIN kb_aliases a ON a.alias_id = kb_names_fts.rowid
         JOIN kb_effective_entity v ON v.entity_id = a.entity_id
         JOIN kb_entities s ON s.entity_id = v.effective_id
         WHERE kb_names_fts MATCH ?1 AND s.status IN ('candidate', 'active')
         ORDER BY bm25(kb_names_fts), a.normalized, a.alias_id
         LIMIT ?2",
    ))?;
    // Several aliases of one entity can match; fetch more than asked for and keep the best of each.
    let rows = db(statement.query_map(
        params![expression, (limit.saturating_mul(4)) as i64],
        |row| {
            Ok(NameHit {
                alias_id: row.get(0)?,
                entity_id: row.get(1)?,
                display: row.get(2)?,
                normalized: row.get(3)?,
            })
        },
    ))?;
    let mut seen = BTreeSet::new();
    let mut hits = Vec::new();
    for row in rows {
        let hit = db(row)?;
        if seen.insert(hit.entity_id) {
            hits.push(hit);
            if hits.len() == limit {
                break;
            }
        }
    }
    Ok(hits)
}

/// The ids of the sources that match `(relative_path, content_id)` pairs of one domain. A path that
/// is known under another content id is not a match: it is a different file.
pub fn source_ids_for_scope(
    connection: &Connection,
    domain: Domain,
    scope: &[(String, String)],
) -> Result<Vec<i64>, AppError> {
    if scope.is_empty() {
        return Ok(Vec::new());
    }
    let pairs: Vec<Value> = scope
        .iter()
        .map(|(path, content_id)| json!({ "path": path, "content_id": content_id }))
        .collect();
    let pairs = Value::Array(pairs).to_string();
    let mut statement = db(connection.prepare(
        "SELECT s.source_id FROM kb_sources s
         JOIN json_each(?2) j
           ON s.relative_path = json_extract(j.value, '$.path')
          AND s.content_id = json_extract(j.value, '$.content_id')
         WHERE s.domain = ?1
         ORDER BY s.source_id",
    ))?;
    let rows = db(statement.query_map(params![domain.as_code(), pairs], |row| row.get(0)))?;
    db(rows.collect())
}

/// For each entity, the sources (among `source_ids`) that mention it or anything merged into it,
/// and the union of them. An empty set of sources allows nothing. Tombstones have no mentions.
pub fn files_for_entities(
    connection: &Connection,
    entity_ids: &[i64],
    source_ids: &[i64],
) -> Result<FileSets, AppError> {
    let mut result = FileSets::default();
    if entity_ids.is_empty() || source_ids.is_empty() {
        return Ok(result);
    }
    let sources = serde_json::to_string(source_ids).map_err(|_| AppError::Internal)?;
    let mut statement = db(connection.prepare(
        "SELECT DISTINCT s.domain, s.relative_path, s.content_id
         FROM kb_effective_mentions m
         JOIN kb_sources s ON s.source_id = m.source_id
         JOIN kb_entities e ON e.entity_id = m.effective_entity_id
         WHERE m.effective_entity_id = ?1
           AND e.status IN ('candidate', 'active')
           AND m.source_id IN (SELECT value FROM json_each(?2))",
    ))?;
    for entity_id in entity_ids {
        let Some(effective) = effective_entity(connection, *entity_id)? else {
            continue;
        };
        let rows = db(
            statement.query_map(params![effective.entity_id, sources], |row| {
                let code: String = row.get(0)?;
                Ok(SourceRef {
                    domain: Domain::from_code(&code).ok_or_else(|| bad_code(0, &code))?,
                    relative_path: row.get(1)?,
                    content_id: row.get(2)?,
                })
            }),
        )?;
        let mut files = BTreeSet::new();
        for row in rows {
            let source = db(row)?;
            result.union.insert(source.clone());
            files.insert(source);
        }
        result.per_entity.insert(*entity_id, files);
    }
    Ok(result)
}

pub fn entity_with_aliases(
    connection: &Connection,
    entity_id: i64,
) -> Result<Option<EntityRecord>, AppError> {
    let Some(entity) = get_entity(connection, entity_id)? else {
        return Ok(None);
    };
    let mut statement = db(connection.prepare(
        "SELECT alias_id, entity_id, display, normalized, phonetic_key, kind, origin, confidence
         FROM kb_aliases WHERE entity_id = ?1 ORDER BY normalized, alias_id",
    ))?;
    let rows = db(statement.query_map([entity_id], alias_from_row))?;
    let aliases = db(rows.collect())?;
    Ok(Some(EntityRecord { entity, aliases }))
}

pub fn list_entities(
    connection: &Connection,
    filter: &EntityFilter,
    page: Page,
) -> Result<Vec<Entity>, AppError> {
    let mut statement = db(connection.prepare(&format!(
        "SELECT {} FROM kb_entities e
         WHERE (?1 IS NULL OR e.type_id = ?1)
           AND (CASE WHEN ?2 IS NULL THEN e.status IN ('candidate', 'active') ELSE e.status = ?2 END)
           AND (?3 IS NULL OR e.origin = ?3)
           AND (?4 IS NULL OR substr(e.normalized_name, 1, length(?4)) = ?4)
         ORDER BY e.normalized_name, e.entity_id
         LIMIT ?5 OFFSET ?6",
        entity_columns("e")
    )))?;
    let rows = db(statement.query_map(
        params![
            filter.type_id,
            filter.status.map(EntityStatus::as_code),
            filter.origin.map(Origin::as_code),
            filter.name_prefix,
            page.limit as i64,
            page.offset as i64,
        ],
        entity_from_row,
    ))?;
    db(rows.collect())
}

// ---------------------------------------------------------------------------------------------
// Integrity
// ---------------------------------------------------------------------------------------------

fn count(connection: &Connection, sql: &str) -> Result<usize, AppError> {
    db(connection.query_row(sql, [], |row| row.get::<_, i64>(0))).map(|value| value as usize)
}

/// Count everything that should be impossible. Eleven small queries over indexed columns: cheap
/// enough to run after every scenario in a test, and for a diagnostics screen later.
pub fn integrity_check(connection: &Connection) -> Result<IntegrityReport, AppError> {
    Ok(IntegrityReport {
        mentions_without_source: count(
            connection,
            "SELECT COUNT(*) FROM kb_mentions m
             WHERE NOT EXISTS (SELECT 1 FROM kb_sources s WHERE s.source_id = m.source_id)",
        )?,
        mentions_without_entity: count(
            connection,
            "SELECT COUNT(*) FROM kb_mentions m
             WHERE NOT EXISTS (SELECT 1 FROM kb_entities e WHERE e.entity_id = m.entity_id)",
        )?,
        aliases_without_entity: count(
            connection,
            "SELECT COUNT(*) FROM kb_aliases a
             WHERE NOT EXISTS (SELECT 1 FROM kb_entities e WHERE e.entity_id = a.entity_id)",
        )?,
        relations_without_endpoint: count(
            connection,
            "SELECT COUNT(*) FROM kb_relations r
             WHERE NOT EXISTS (SELECT 1 FROM kb_entities e WHERE e.entity_id = r.subject)
                OR NOT EXISTS (SELECT 1 FROM kb_entities e WHERE e.entity_id = r.object)",
        )?,
        broken_merges: count(
            connection,
            "SELECT COUNT(*) FROM kb_entities e
             WHERE (e.status = 'merged'
                    AND (e.merged_into IS NULL
                         OR NOT EXISTS (SELECT 1 FROM kb_entities t
                                        WHERE t.entity_id = e.merged_into AND t.status <> 'merged')))
                OR (e.status <> 'merged' AND e.merged_into IS NOT NULL)",
        )?,
        merge_cycles: count(
            connection,
            "WITH RECURSIVE chain (start, current, depth) AS (
                 SELECT entity_id, merged_into, 1 FROM kb_entities WHERE status = 'merged'
                 UNION ALL
                 SELECT chain.start, e.merged_into, chain.depth + 1
                 FROM chain JOIN kb_entities e ON e.entity_id = chain.current
                 WHERE e.status = 'merged' AND chain.depth < 64)
             SELECT COUNT(DISTINCT start) FROM chain WHERE current = start",
        )?,
        duplicate_live_entities: count(
            connection,
            "SELECT COUNT(*) FROM (
                 SELECT 1 FROM kb_entities
                 WHERE status IN ('candidate', 'active')
                 GROUP BY type_id, normalized_name, disambiguator HAVING COUNT(*) > 1)",
        )?,
        duplicate_sources: count(
            connection,
            "SELECT COUNT(*) FROM (
                 SELECT 1 FROM kb_sources GROUP BY domain, relative_path HAVING COUNT(*) > 1)",
        )?,
        name_index_rows_without_alias: count(
            connection,
            "SELECT COUNT(*) FROM kb_names_fts f
             WHERE NOT EXISTS (SELECT 1 FROM kb_aliases a WHERE a.alias_id = f.rowid)",
        )?,
        aliases_without_name_index_row: count(
            connection,
            "SELECT COUNT(*) FROM kb_aliases a
             WHERE NOT EXISTS (SELECT 1 FROM kb_names_fts f WHERE f.rowid = a.alias_id)",
        )?,
        orphan_automatic_entities: count(
            connection,
            &format!("SELECT COUNT(*) FROM kb_entities WHERE {ORPHAN_CONDITION}"),
        )?,
    })
}

/// How many rows each knowledge table holds, for a test that wants to say "empty" and for the
/// human check (`sqlite3 index.sqlite3 "SELECT ..."`).
pub fn table_counts(connection: &Connection) -> Result<BTreeMap<&'static str, usize>, AppError> {
    const TABLES: [&str; 11] = [
        "kb_sources",
        "kb_entities",
        "kb_aliases",
        "kb_mentions",
        "kb_relations",
        "kb_attributes",
        "kb_possible_matches",
        "kb_ops_log",
        "kb_column_semantics",
        "kb_source_signals",
        "kb_profile",
    ];
    let mut counts = BTreeMap::new();
    for table in TABLES {
        counts.insert(
            table,
            count(connection, &format!("SELECT COUNT(*) FROM {table}"))?,
        );
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::knowledge::{AttributeDraft, CURRENT_KB_VERSION};

    fn open() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        crate::index_store::configure_connection(&connection).unwrap();
        ensure_schema(&connection).unwrap();
        connection
    }

    fn source(path: &str) -> SourceRef {
        SourceRef {
            domain: Domain::Documents,
            relative_path: path.to_string(),
            content_id: format!("sha-{path}"),
        }
    }

    fn person(temp_id: u32, name: &str) -> EntityDraft {
        EntityDraft {
            temp_id,
            type_id: "person".to_string(),
            subtype: None,
            canonical_name: name.to_string(),
            normalized_name: name.to_lowercase(),
            disambiguator: String::new(),
            status: EntityStatus::Active,
        }
    }

    fn organization(temp_id: u32, name: &str) -> EntityDraft {
        EntityDraft {
            type_id: "organization".to_string(),
            ..person(temp_id, name)
        }
    }

    fn mention(entity: EntityRef, chunk_id: &str) -> MentionDraft {
        MentionDraft {
            entity,
            alias_normalized: None,
            role: None,
            locator: MentionLocator::Chunk {
                chunk_id: chunk_id.to_string(),
            },
            occurrence_count: 1,
            confidence: 0.9,
            method: "test_method".to_string(),
            extractor_version: 1,
        }
    }

    fn alias(entity: EntityRef, display: &str) -> crate::knowledge::AliasDraft {
        crate::knowledge::AliasDraft {
            entity,
            display: display.to_string(),
            normalized: display.to_lowercase(),
            phonetic_key: String::new(),
            kind: AliasKind::CanonicalVariant,
            confidence: 1.0,
        }
    }

    fn new_person(name: &str, origin: Origin) -> NewEntity {
        NewEntity {
            type_id: "person".to_string(),
            subtype: None,
            canonical_name: name.to_string(),
            normalized_name: name.to_lowercase(),
            disambiguator: String::new(),
            status: EntityStatus::Active,
            origin,
        }
    }

    fn manual_alias(display: &str) -> NewAlias {
        NewAlias {
            display: display.to_string(),
            normalized: display.to_lowercase(),
            phonetic_key: String::new(),
            kind: AliasKind::Manual,
            confidence: 1.0,
        }
    }

    /// A delta for `path` that finds Alice Archer once, in chunk `path#p1#s1`.
    fn alice_in(path: &str) -> KnowledgeDelta {
        let mut delta = KnowledgeDelta::empty(source(path));
        delta.kb_version = CURRENT_KB_VERSION;
        delta.entities.push(person(1, "Alice Archer"));
        delta
            .aliases
            .push(alias(EntityRef::Draft(1), "alice archer"));
        delta
            .mentions
            .push(mention(EntityRef::Draft(1), &format!("{path}#p1#s1")));
        delta
    }

    fn apply(connection: &mut Connection, delta: &KnowledgeDelta) -> ApplyOutcome {
        let tx = connection.transaction().unwrap();
        let outcome = apply_delta(&tx, delta).unwrap();
        tx.commit().unwrap();
        outcome
    }

    fn remove(connection: &mut Connection, path: &str) {
        let tx = connection.transaction().unwrap();
        remove_source(&tx, Domain::Documents, path).unwrap();
        tx.commit().unwrap();
    }

    fn entity_named(connection: &Connection, normalized: &str) -> Entity {
        let all = list_entities(
            connection,
            &EntityFilter {
                name_prefix: Some(normalized.to_string()),
                ..EntityFilter::default()
            },
            Page {
                offset: 0,
                limit: 10,
            },
        )
        .unwrap();
        all.into_iter()
            .find(|entity| entity.normalized_name == normalized)
            .unwrap_or_else(|| panic!("no live entity named {normalized}"))
    }

    fn count_rows(connection: &Connection, table: &str) -> usize {
        count(connection, &format!("SELECT COUNT(*) FROM {table}")).unwrap()
    }

    fn assert_clean(connection: &Connection) {
        assert_eq!(
            integrity_check(connection).unwrap(),
            IntegrityReport::default()
        );
    }

    /// Every row of every table a delta can touch, as text, in a stable order.
    fn snapshot(connection: &Connection) -> String {
        let mut out = String::new();
        for table in [
            "kb_meta",
            "kb_sources",
            "kb_entities",
            "kb_aliases",
            "kb_mentions",
            "kb_relations",
            "kb_attributes",
            "kb_ops_log",
            "kb_source_signals",
        ] {
            let mut statement = connection
                .prepare(&format!("SELECT * FROM {table} ORDER BY 1, 2"))
                .unwrap();
            let columns = statement.column_count();
            let mut rows = statement.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                out.push_str(table);
                for index in 0..columns {
                    out.push('|');
                    out.push_str(&format!("{:?}", row.get_ref(index).unwrap()));
                }
                out.push('\n');
            }
        }
        out
    }

    #[test]
    fn the_schema_is_idempotent_and_seeded() {
        let connection = open();
        ensure_schema(&connection).unwrap();
        ensure_schema(&connection).unwrap();

        assert_eq!(
            meta(&connection, "schema_version").unwrap().as_deref(),
            Some("1")
        );
        assert_eq!(count_rows(&connection, "kb_source_domains"), 2);
        assert_eq!(count_rows(&connection, "kb_entity_types"), 6);
        for (table, rows) in table_counts(&connection).unwrap() {
            assert_eq!(rows, 0, "{table} starts empty");
        }
        assert_clean(&connection);
    }

    #[test]
    fn a_newer_schema_version_is_never_lowered() {
        let connection = open();
        set_meta(&connection, "schema_version", "9").unwrap();

        ensure_schema(&connection).unwrap();

        assert_eq!(
            meta(&connection, "schema_version").unwrap().as_deref(),
            Some("9")
        );
    }

    #[test]
    fn foreign_keys_refuse_a_mention_of_nothing() {
        let connection = open();

        let result = connection.execute(
            "INSERT INTO kb_mentions (entity_id, source_id, locator_kind, method, origin)
             VALUES (1, 1, 'chunk', 'm', 'automatic')",
            [],
        );

        assert!(result.is_err());
    }

    #[test]
    fn removing_a_source_removes_its_mentions_and_collects_the_entity() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        assert_eq!(count_rows(&connection, "kb_mentions"), 1);
        assert_eq!(count_rows(&connection, "kb_entities"), 1);

        remove(&mut connection, "a.txt");

        assert_eq!(count_rows(&connection, "kb_sources"), 0);
        assert_eq!(count_rows(&connection, "kb_mentions"), 0);
        assert_eq!(count_rows(&connection, "kb_entities"), 0);
        assert_eq!(count_rows(&connection, "kb_aliases"), 0);
        assert_eq!(count_rows(&connection, "kb_names_fts"), 0);
        assert_clean(&connection);
    }

    #[test]
    fn an_entity_seen_in_two_sources_outlives_the_loss_of_one() {
        let mut connection = open();
        let first = apply(&mut connection, &alice_in("a.txt"));
        let second = apply(&mut connection, &alice_in("b.txt"));
        assert_eq!(first.entities_created, 1);
        assert_eq!(second.entities_created, 0);
        assert_eq!(second.entities_matched, 1);

        remove(&mut connection, "a.txt");

        assert_eq!(count_rows(&connection, "kb_entities"), 1);
        assert_eq!(count_rows(&connection, "kb_mentions"), 1);
        assert_clean(&connection);
    }

    #[test]
    fn applying_the_same_delta_twice_changes_no_row() {
        let mut connection = open();
        let mut delta = alice_in("a.txt");
        delta.attributes.push(AttributeDraft {
            entity: EntityRef::Draft(1),
            key: "email".to_string(),
            value: "alice@example.test".to_string(),
        });
        delta.signals.push(SignalDraft {
            pack_id: "billing".to_string(),
            hits: 4,
            distinct_terms: 2,
        });
        apply(&mut connection, &delta);
        // Age every timestamp: a no-op must leave them alone, not refresh them.
        connection
            .execute("UPDATE kb_entities SET created_at = 1, updated_at = 1", [])
            .unwrap();
        let before = snapshot(&connection);

        apply(&mut connection, &delta);

        assert_eq!(snapshot(&connection), before);
        assert_clean(&connection);
    }

    #[test]
    fn a_changed_file_replaces_the_automatic_rows_of_its_source() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        let mut changed = KnowledgeDelta::empty(SourceRef {
            content_id: "sha-new".to_string(),
            ..source("a.txt")
        });
        changed.kb_version = CURRENT_KB_VERSION;
        changed.entities.push(person(1, "Bruno Baker"));
        changed
            .mentions
            .push(mention(EntityRef::Draft(1), "a.txt#p2#s1"));

        apply(&mut connection, &changed);

        let names: Vec<String> = list_entities(
            &connection,
            &EntityFilter::default(),
            Page {
                offset: 0,
                limit: 10,
            },
        )
        .unwrap()
        .into_iter()
        .map(|entity| entity.normalized_name)
        .collect();
        assert_eq!(names, vec!["bruno baker".to_string()]);
        let rows = sources(&connection, Some(Domain::Documents)).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source.content_id, "sha-new");
        assert_clean(&connection);
    }

    #[test]
    fn a_mention_that_changed_is_updated_in_place() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        let id_before: i64 = connection
            .query_row("SELECT mention_id FROM kb_mentions", [], |row| row.get(0))
            .unwrap();
        let mut again = alice_in("a.txt");
        again.mentions[0].occurrence_count = 5;

        apply(&mut connection, &again);

        let (id_after, occurrences): (i64, i64) = connection
            .query_row(
                "SELECT mention_id, occurrence_count FROM kb_mentions",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(id_after, id_before);
        assert_eq!(occurrences, 5);
    }

    #[test]
    fn manual_rows_survive_an_automatic_rebuild_of_their_source() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        let alice = entity_named(&connection, "alice archer");
        let source_id = source_id(&connection, Domain::Documents, "a.txt")
            .unwrap()
            .unwrap();
        add_mention(
            &connection,
            source_id,
            alice.entity_id,
            &mention(EntityRef::Existing(alice.entity_id), "a.txt#p9#s9"),
            Origin::Manual,
        )
        .unwrap();
        set_attribute(
            &connection,
            alice.entity_id,
            "note",
            "kept",
            Origin::Manual,
            None,
        )
        .unwrap();

        // The new version of the file no longer mentions her at all.
        let mut nothing_left = KnowledgeDelta::empty(source("a.txt"));
        nothing_left.kb_version = CURRENT_KB_VERSION;
        apply(&mut connection, &nothing_left);

        let manual_mentions: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM kb_mentions WHERE origin = 'manual'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let automatic_mentions: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM kb_mentions WHERE origin = 'automatic'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(manual_mentions, 1);
        assert_eq!(automatic_mentions, 0);
        assert_eq!(count_rows(&connection, "kb_attributes"), 1);
        assert_clean(&connection);
    }

    #[test]
    fn an_entity_with_a_manual_alias_is_not_collected() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        let alice = entity_named(&connection, "alice archer");
        add_alias(
            &connection,
            alice.entity_id,
            &manual_alias("Ally"),
            Origin::Manual,
        )
        .unwrap();

        remove(&mut connection, "a.txt");

        assert_eq!(count_rows(&connection, "kb_entities"), 1);
        assert_eq!(count_rows(&connection, "kb_mentions"), 0);
        assert_clean(&connection);
    }

    #[test]
    fn a_manual_entity_is_never_collected() {
        let mut connection = open();
        create_entity(&connection, &new_person("Carla Chen", Origin::Manual)).unwrap();

        let tx = connection.transaction().unwrap();
        assert_eq!(gc_orphans(&tx).unwrap(), 0);
        tx.commit().unwrap();

        assert_eq!(count_rows(&connection, "kb_entities"), 1);
        assert_clean(&connection);
    }

    #[test]
    fn a_manual_alias_is_not_overridden_by_an_automatic_one() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        let alice = entity_named(&connection, "alice archer");
        add_alias(
            &connection,
            alice.entity_id,
            &NewAlias {
                display: "Alice ARCHER".to_string(),
                normalized: "alice archer".to_string(),
                phonetic_key: "k1".to_string(),
                kind: AliasKind::Manual,
                confidence: 1.0,
            },
            Origin::Manual,
        )
        .unwrap();

        apply(&mut connection, &alice_in("a.txt"));

        let record = entity_with_aliases(&connection, alice.entity_id)
            .unwrap()
            .unwrap();
        assert_eq!(record.aliases.len(), 1);
        assert_eq!(record.aliases[0].origin, Origin::Manual);
        assert_eq!(record.aliases[0].display, "Alice ARCHER");
    }

    #[test]
    fn a_candidate_seen_with_support_becomes_active() {
        let mut connection = open();
        let mut weak = alice_in("a.txt");
        weak.entities[0].status = EntityStatus::Candidate;
        apply(&mut connection, &weak);
        assert_eq!(
            entity_named(&connection, "alice archer").status,
            EntityStatus::Candidate
        );

        apply(&mut connection, &alice_in("b.txt"));

        assert_eq!(
            entity_named(&connection, "alice archer").status,
            EntityStatus::Active
        );
    }

    #[test]
    fn attributes_keep_one_row_per_source_that_states_them() {
        let mut connection = open();
        let attribute = AttributeDraft {
            entity: EntityRef::Draft(1),
            key: "email".to_string(),
            value: "alice@example.test".to_string(),
        };
        for path in ["a.txt", "b.txt"] {
            let mut delta = alice_in(path);
            delta.attributes.push(attribute.clone());
            apply(&mut connection, &delta);
        }
        assert_eq!(count_rows(&connection, "kb_attributes"), 2);

        remove(&mut connection, "a.txt");

        assert_eq!(count_rows(&connection, "kb_attributes"), 1);
        assert_clean(&connection);
    }

    #[test]
    fn a_deleted_name_is_not_recreated_by_the_next_analysis() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        let alice = entity_named(&connection, "alice archer");
        let tx = connection.transaction().unwrap();
        delete_entity(&tx, alice.entity_id).unwrap();
        tx.commit().unwrap();
        assert!(is_tombstoned(&connection, "person", "alice archer").unwrap());

        let outcome = apply(&mut connection, &alice_in("a.txt"));

        assert_eq!(outcome.entities_suppressed, 1);
        assert_eq!(outcome.entities_created, 0);
        assert_eq!(count_rows(&connection, "kb_mentions"), 0);
        assert_eq!(
            get_entity(&connection, alice.entity_id)
                .unwrap()
                .unwrap()
                .status,
            EntityStatus::Deleted
        );
        assert_clean(&connection);
    }

    #[test]
    fn restoring_a_tombstone_lets_the_name_come_back() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        let alice = entity_named(&connection, "alice archer");
        let tx = connection.transaction().unwrap();
        delete_entity(&tx, alice.entity_id).unwrap();
        restore_entity(&tx, alice.entity_id).unwrap();
        tx.commit().unwrap();

        let outcome = apply(&mut connection, &alice_in("a.txt"));

        assert_eq!(outcome.entities_matched, 1);
        assert_eq!(count_rows(&connection, "kb_mentions"), 1);
        assert_eq!(
            get_entity(&connection, alice.entity_id)
                .unwrap()
                .unwrap()
                .status,
            EntityStatus::Active
        );
        assert_clean(&connection);
    }

    fn two_people(connection: &mut Connection) -> (i64, i64) {
        let mut delta = KnowledgeDelta::empty(source("a.txt"));
        delta.kb_version = CURRENT_KB_VERSION;
        delta.entities.push(person(1, "Alice Archer"));
        delta.entities.push(person(2, "Alyce Archer"));
        delta
            .aliases
            .push(alias(EntityRef::Draft(1), "alice archer"));
        delta
            .aliases
            .push(alias(EntityRef::Draft(2), "alyce archer"));
        delta
            .mentions
            .push(mention(EntityRef::Draft(1), "a.txt#p1#s1"));
        delta
            .mentions
            .push(mention(EntityRef::Draft(2), "a.txt#p1#s2"));
        apply(connection, &delta);
        (
            entity_named(connection, "alice archer").entity_id,
            entity_named(connection, "alyce archer").entity_id,
        )
    }

    fn merge(connection: &mut Connection, survivor: i64, victim: i64) {
        let tx = connection.transaction().unwrap();
        merge_entities(&tx, survivor, victim).unwrap();
        tx.commit().unwrap();
    }

    #[test]
    fn a_merge_is_a_redirect_and_reads_follow_it() {
        let mut connection = open();
        let (alice, alyce) = two_people(&mut connection);

        merge(&mut connection, alice, alyce);

        let victim = get_entity(&connection, alyce).unwrap().unwrap();
        assert_eq!(victim.status, EntityStatus::Merged);
        assert_eq!(victim.merged_into, Some(alice));
        assert_eq!(
            effective_entity(&connection, alyce)
                .unwrap()
                .unwrap()
                .entity_id,
            alice
        );
        // Nothing was moved: the victim still owns its mention and its alias.
        assert_eq!(count_rows(&connection, "kb_mentions"), 2);
        assert_eq!(count_rows(&connection, "kb_aliases"), 2);
        // A search by the victim's spelling leads to the survivor.
        let hits = prefix_search(&connection, "alyce", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entity_id, alice);
        // The victim's mention counts for the survivor.
        let scope = source_ids_for_scope(
            &connection,
            Domain::Documents,
            &[("a.txt".to_string(), "sha-a.txt".to_string())],
        )
        .unwrap();
        let sets = files_for_entities(&connection, &[alice, alyce], &scope).unwrap();
        assert_eq!(sets.union.len(), 1);
        assert_eq!(sets.per_entity[&alyce], sets.per_entity[&alice]);
        assert_clean(&connection);
    }

    #[test]
    fn a_merge_into_a_merged_entity_follows_the_chain_and_stays_one_hop() {
        let mut connection = open();
        let (alice, alyce) = two_people(&mut connection);
        let mut third = KnowledgeDelta::empty(source("b.txt"));
        third.kb_version = CURRENT_KB_VERSION;
        third.entities.push(person(1, "Carla Chen"));
        third
            .mentions
            .push(mention(EntityRef::Draft(1), "b.txt#p1#s1"));
        apply(&mut connection, &third);
        let carla = entity_named(&connection, "carla chen").entity_id;

        // alyce -> alice, then alice -> carla: alyce must now point at carla directly.
        merge(&mut connection, alice, alyce);
        merge(&mut connection, carla, alice);

        assert_eq!(
            get_entity(&connection, alyce).unwrap().unwrap().merged_into,
            Some(carla)
        );
        assert_eq!(
            get_entity(&connection, alice).unwrap().unwrap().merged_into,
            Some(carla)
        );
        assert_eq!(
            effective_entity(&connection, alyce)
                .unwrap()
                .unwrap()
                .entity_id,
            carla
        );
        assert_clean(&connection);
    }

    #[test]
    fn a_merge_that_would_make_a_cycle_is_refused() {
        let mut connection = open();
        let (alice, alyce) = two_people(&mut connection);
        merge(&mut connection, alice, alyce);

        let tx = connection.transaction().unwrap();
        let backwards = merge_entities(&tx, alyce, alice);
        let onto_itself = merge_entities(&tx, alice, alice);
        drop(tx);

        assert_eq!(backwards.unwrap_err().code(), "knowledge_merge_refused");
        assert_eq!(onto_itself.unwrap_err().code(), "knowledge_merge_refused");
        assert_clean(&connection);
    }

    #[test]
    fn entities_of_two_types_are_never_merged() {
        let mut connection = open();
        let mut delta = KnowledgeDelta::empty(source("a.txt"));
        delta.entities.push(person(1, "Alice Archer"));
        delta.entities.push(organization(2, "Acme Supplies"));
        delta
            .mentions
            .push(mention(EntityRef::Draft(1), "a.txt#p1#s1"));
        delta
            .mentions
            .push(mention(EntityRef::Draft(2), "a.txt#p1#s2"));
        apply(&mut connection, &delta);
        let alice = entity_named(&connection, "alice archer").entity_id;
        let acme = entity_named(&connection, "acme supplies").entity_id;

        let tx = connection.transaction().unwrap();
        let refused = merge_entities(&tx, alice, acme);
        let missing = merge_entities(&tx, alice, 9999);
        drop(tx);

        assert_eq!(refused.unwrap_err().code(), "knowledge_merge_refused");
        assert_eq!(missing.unwrap_err().code(), "knowledge_entity_not_found");
    }

    #[test]
    fn an_unmerge_gives_back_the_entity_and_whatever_was_redirected_to_it() {
        let mut connection = open();
        let (alice, alyce) = two_people(&mut connection);
        let mut third = KnowledgeDelta::empty(source("b.txt"));
        third.entities.push(person(1, "Carla Chen"));
        third
            .mentions
            .push(mention(EntityRef::Draft(1), "b.txt#p1#s1"));
        apply(&mut connection, &third);
        let carla = entity_named(&connection, "carla chen").entity_id;
        merge(&mut connection, alice, alyce);
        merge(&mut connection, carla, alice);

        // Undo the last merge: alice is herself again, and alyce goes back behind alice.
        let tx = connection.transaction().unwrap();
        unmerge_entity(&tx, alice).unwrap();
        tx.commit().unwrap();

        let alice_row = get_entity(&connection, alice).unwrap().unwrap();
        assert_eq!(alice_row.status, EntityStatus::Active);
        assert_eq!(alice_row.merged_into, None);
        assert_eq!(
            get_entity(&connection, alyce).unwrap().unwrap().merged_into,
            Some(alice)
        );
        assert_clean(&connection);

        let tx = connection.transaction().unwrap();
        unmerge_entity(&tx, alyce).unwrap();
        tx.commit().unwrap();
        assert_eq!(
            get_entity(&connection, alyce).unwrap().unwrap().status,
            EntityStatus::Active
        );
        assert_clean(&connection);
    }

    #[test]
    fn an_unmerge_is_refused_when_the_name_has_been_taken() {
        let mut connection = open();
        let (alice, alyce) = two_people(&mut connection);
        merge(&mut connection, alice, alyce);
        // A new analysis meets the merged name and, finding it merged, links to the survivor.
        let mut again = KnowledgeDelta::empty(source("c.txt"));
        again.entities.push(person(1, "Alyce Archer"));
        let outcome = apply(&mut connection, &again);
        assert_eq!(outcome.entities_created, 0);
        assert_eq!(outcome.entities_matched, 1);

        // Someone creates the name by hand while the original is still merged.
        create_entity(&connection, &new_person("Alyce Archer", Origin::Manual)).unwrap();

        let tx = connection.transaction().unwrap();
        let refused = unmerge_entity(&tx, alyce);
        drop(tx);

        assert_eq!(refused.unwrap_err().code(), "knowledge_merge_refused");
    }

    #[test]
    fn a_merged_entity_cannot_be_deleted_and_a_tombstone_hides_its_merged_names() {
        let mut connection = open();
        let (alice, alyce) = two_people(&mut connection);
        merge(&mut connection, alice, alyce);

        let tx = connection.transaction().unwrap();
        assert_eq!(
            delete_entity(&tx, alyce).unwrap_err().code(),
            "knowledge_merge_refused"
        );
        delete_entity(&tx, alice).unwrap();
        tx.commit().unwrap();

        assert!(prefix_search(&connection, "alyce", 5).unwrap().is_empty());
        assert!(prefix_search(&connection, "alice", 5).unwrap().is_empty());
        assert!(find_entities_by_normalized(&connection, "alyce archer")
            .unwrap()
            .is_empty());
        assert_clean(&connection);
    }

    #[test]
    fn the_name_search_takes_one_letter_and_several_words() {
        let mut connection = open();
        let (alice, _) = two_people(&mut connection);

        let one_letter = prefix_search(&connection, "a", 10).unwrap();
        let two_words = prefix_search(&connection, "alice ar", 10).unwrap();
        let none = prefix_search(&connection, "zz", 10).unwrap();
        let blank = prefix_search(&connection, "  ", 10).unwrap();

        assert_eq!(one_letter.len(), 2);
        assert_eq!(two_words.len(), 1);
        assert_eq!(two_words[0].entity_id, alice);
        assert!(none.is_empty());
        assert!(blank.is_empty());
    }

    #[test]
    fn entities_are_found_by_name_and_by_sound() {
        let mut connection = open();
        let mut delta = alice_in("a.txt");
        delta.aliases[0].phonetic_key = "alis".to_string();
        apply(&mut connection, &delta);

        assert_eq!(
            find_entities_by_normalized(&connection, "alice archer")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            find_entities_by_phonetic(&connection, "alis")
                .unwrap()
                .len(),
            1
        );
        assert!(find_entities_by_phonetic(&connection, "")
            .unwrap()
            .is_empty());
        assert!(find_entities_by_phonetic(&connection, "other")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_scope_maps_to_sources_only_when_the_content_matches() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        apply(&mut connection, &alice_in("b.txt"));

        let both = source_ids_for_scope(
            &connection,
            Domain::Documents,
            &[
                ("a.txt".to_string(), "sha-a.txt".to_string()),
                ("b.txt".to_string(), "sha-b.txt".to_string()),
            ],
        )
        .unwrap();
        let stale = source_ids_for_scope(
            &connection,
            Domain::Documents,
            &[("a.txt".to_string(), "another-content".to_string())],
        )
        .unwrap();
        let other_folder = source_ids_for_scope(
            &connection,
            Domain::Data,
            &[("a.txt".to_string(), "sha-a.txt".to_string())],
        )
        .unwrap();

        assert_eq!(both.len(), 2);
        assert!(
            stale.is_empty(),
            "a same-named file with other content is another file"
        );
        assert!(
            other_folder.is_empty(),
            "the other folder is another source"
        );
    }

    #[test]
    fn the_files_of_an_entity_are_limited_to_the_sources_asked_for() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        apply(&mut connection, &alice_in("b.txt"));
        let alice = entity_named(&connection, "alice archer").entity_id;
        let only_a = source_ids_for_scope(
            &connection,
            Domain::Documents,
            &[("a.txt".to_string(), "sha-a.txt".to_string())],
        )
        .unwrap();

        let sets = files_for_entities(&connection, &[alice], &only_a).unwrap();
        let nothing = files_for_entities(&connection, &[alice], &[]).unwrap();

        assert_eq!(sets.union.len(), 1);
        assert_eq!(
            sets.per_entity[&alice].iter().next().unwrap().relative_path,
            "a.txt"
        );
        assert!(nothing.union.is_empty(), "an empty scope allows nothing");
    }

    #[test]
    fn a_renamed_source_carries_its_chunk_ids() {
        let mut connection = open();
        apply(&mut connection, &alice_in("old.txt"));
        apply(&mut connection, &alice_in("old.txt.bak"));

        let tx = connection.transaction().unwrap();
        assert!(move_source(&tx, Domain::Documents, "old.txt", "new.txt").unwrap());
        tx.commit().unwrap();

        let chunk_ids: Vec<String> = {
            let mut statement = connection
                .prepare("SELECT chunk_id FROM kb_mentions ORDER BY chunk_id")
                .unwrap();
            let rows = statement.query_map([], |row| row.get(0)).unwrap();
            rows.map(Result::unwrap).collect()
        };
        // The prefix test must not touch a sibling whose path merely starts the same way.
        assert_eq!(
            chunk_ids,
            vec!["new.txt#p1#s1".to_string(), "old.txt.bak#p1#s1".to_string()]
        );
        assert!(source_id(&connection, Domain::Documents, "old.txt")
            .unwrap()
            .is_none());
        assert!(source_id(&connection, Domain::Documents, "new.txt")
            .unwrap()
            .is_some());
        assert_clean(&connection);
    }

    #[test]
    fn a_source_cannot_be_moved_onto_another() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        apply(&mut connection, &alice_in("b.txt"));

        let tx = connection.transaction().unwrap();
        let refused = move_source(&tx, Domain::Documents, "a.txt", "b.txt");
        let unknown = move_source(&tx, Domain::Documents, "nothing.txt", "c.txt");
        drop(tx);

        assert_eq!(refused.unwrap_err().code(), "knowledge_unavailable");
        assert!(!unknown.unwrap());
    }

    #[test]
    fn a_reference_to_nothing_rolls_the_whole_delta_back() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        let before = snapshot(&connection);

        let mut undefined_draft = alice_in("a.txt");
        undefined_draft
            .mentions
            .push(mention(EntityRef::Draft(77), "a.txt#p3#s1"));
        let tx = connection.transaction().unwrap();
        let first = apply_delta(&tx, &undefined_draft);
        drop(tx);

        let mut missing_entity = alice_in("a.txt");
        missing_entity
            .mentions
            .push(mention(EntityRef::Existing(9999), "a.txt#p3#s2"));
        let tx = connection.transaction().unwrap();
        let second = apply_delta(&tx, &missing_entity);
        drop(tx);

        assert_eq!(first.unwrap_err().code(), "internal_error");
        assert_eq!(second.unwrap_err().code(), "knowledge_entity_not_found");
        assert_eq!(snapshot(&connection), before);
    }

    #[test]
    fn a_draft_with_no_name_is_refused() {
        let mut connection = open();
        let mut delta = KnowledgeDelta::empty(source("a.txt"));
        delta.entities.push(person(1, " "));

        let tx = connection.transaction().unwrap();
        let result = apply_delta(&tx, &delta);
        drop(tx);

        assert_eq!(result.unwrap_err().code(), "internal_error");
        assert_eq!(count_rows(&connection, "kb_sources"), 0);
    }

    fn data_source(path: &str, content_id: &str) -> SourceRef {
        SourceRef {
            domain: Domain::Data,
            relative_path: path.to_string(),
            content_id: content_id.to_string(),
        }
    }

    #[test]
    fn the_same_relative_path_in_both_folders_stays_two_sources() {
        let mut connection = open();
        apply(&mut connection, &alice_in("shared.csv"));
        let mut data = KnowledgeDelta::empty(data_source("shared.csv", "sha-data"));
        data.entities.push(person(1, "Alice Archer"));
        data.mentions.push(MentionDraft {
            locator: MentionLocator::Cells {
                sheet: "Sheet1".to_string(),
                column_name: "name".to_string(),
                first_row: Some(2),
                sample_rows: vec![2, 3, 4, 5],
            },
            ..mention(EntityRef::Draft(1), "")
        });
        apply(&mut connection, &data);

        assert_eq!(sources(&connection, None).unwrap().len(), 2);
        let sets = files_for_entities(
            &connection,
            &[entity_named(&connection, "alice archer").entity_id],
            &source_ids_for_scope(
                &connection,
                Domain::Data,
                &[("shared.csv".to_string(), "sha-data".to_string())],
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(sets.union.len(), 1);
        assert_eq!(sets.union.iter().next().unwrap().domain, Domain::Data);
        // Only three sample rows are kept.
        let sample: String = connection
            .query_row(
                "SELECT sample_rows FROM kb_mentions WHERE locator_kind = 'cells'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sample, "[2,3,4]");
        assert_clean(&connection);
    }

    #[test]
    fn clearing_one_domain_keeps_the_other_and_every_manual_row() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        let mut data = KnowledgeDelta::empty(data_source("t.csv", "sha-t"));
        data.entities.push(organization(1, "Acme Supplies"));
        data.mentions.push(MentionDraft {
            locator: MentionLocator::Filename,
            ..mention(EntityRef::Draft(1), "")
        });
        apply(&mut connection, &data);
        let alice = entity_named(&connection, "alice archer").entity_id;
        set_attribute(&connection, alice, "note", "kept", Origin::Manual, None).unwrap();

        let tx = connection.transaction().unwrap();
        clear_domain(&tx, Domain::Documents).unwrap();
        tx.commit().unwrap();

        let remaining = sources(&connection, None).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].source.domain, Domain::Data);
        // Alice has a manual attribute: she stays, with no mention left.
        assert_eq!(count_rows(&connection, "kb_entities"), 2);
        assert_eq!(count_rows(&connection, "kb_mentions"), 1);
        assert_clean(&connection);
    }

    #[test]
    fn clearing_everything_empties_the_base_but_not_its_seeds() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        let alice = entity_named(&connection, "alice archer").entity_id;
        let tx = connection.transaction().unwrap();
        delete_entity(&tx, alice).unwrap();
        tx.commit().unwrap();

        let tx = connection.transaction().unwrap();
        clear_everything(&tx).unwrap();
        tx.commit().unwrap();

        for (table, rows) in table_counts(&connection).unwrap() {
            assert_eq!(rows, 0, "{table}");
        }
        assert_eq!(count_rows(&connection, "kb_names_fts"), 0);
        assert_eq!(count_rows(&connection, "kb_entity_types"), 6);
        assert_eq!(count_rows(&connection, "kb_source_domains"), 2);
        assert_clean(&connection);
    }

    #[test]
    fn clearing_the_automatic_part_keeps_what_a_person_wrote() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        let alice = entity_named(&connection, "alice archer").entity_id;
        set_attribute(&connection, alice, "note", "kept", Origin::Manual, None).unwrap();
        let mut other = KnowledgeDelta::empty(source("b.txt"));
        other.entities.push(person(1, "Bruno Baker"));
        other
            .mentions
            .push(mention(EntityRef::Draft(1), "b.txt#p1#s1"));
        apply(&mut connection, &other);

        let tx = connection.transaction().unwrap();
        clear_all_automatic(&tx).unwrap();
        tx.commit().unwrap();

        assert_eq!(count_rows(&connection, "kb_sources"), 0);
        assert_eq!(
            count_rows(&connection, "kb_entities"),
            1,
            "only the manually annotated one stays"
        );
        assert_eq!(count_rows(&connection, "kb_attributes"), 1);
        assert_clean(&connection);
    }

    #[test]
    fn a_relation_follows_its_source_and_can_be_updated_in_place() {
        let mut connection = open();
        let mut delta = KnowledgeDelta::empty(source("t.txt"));
        delta.entities.push(person(1, "Alice Archer"));
        delta.entities.push(organization(2, "Acme Supplies"));
        delta.relations.push(RelationDraft {
            subject: EntityRef::Draft(1),
            predicate: "supplied_by".to_string(),
            object: EntityRef::Draft(2),
            occurrence_count: 1,
            confidence: 0.9,
        });
        apply(&mut connection, &delta);
        assert_eq!(count_rows(&connection, "kb_relations"), 1);

        delta.relations[0].occurrence_count = 3;
        apply(&mut connection, &delta);
        let occurrences: i64 = connection
            .query_row("SELECT occurrence_count FROM kb_relations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(occurrences, 3);

        remove(&mut connection, "t.txt");
        assert_eq!(count_rows(&connection, "kb_relations"), 0);
        assert_eq!(count_rows(&connection, "kb_entities"), 0);
        assert_clean(&connection);
    }

    #[test]
    fn the_integrity_check_sees_what_a_connection_without_keys_can_break() {
        let mut connection = open();
        apply(&mut connection, &alice_in("a.txt"));
        connection.execute("PRAGMA foreign_keys = OFF", []).unwrap();
        // An orphan mention, a merge pointing nowhere, and a name-index row of nothing.
        connection
            .execute(
                "INSERT INTO kb_mentions (entity_id, source_id, locator_kind, method, origin)
                 VALUES (900, 901, 'chunk', 'm', 'automatic')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO kb_entities (type_id, canonical_name, normalized_name, status,
                                          merged_into, origin, created_at, updated_at)
                 VALUES ('person', 'Nobody', 'nobody', 'merged', 903, 'manual', 1, 1)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO kb_names_fts (rowid, normalized, alias_id)
                 VALUES (5000, 'ghost', 5000)",
                [],
            )
            .unwrap();

        let report = integrity_check(&connection).unwrap();

        assert_eq!(report.mentions_without_source, 1);
        assert_eq!(report.mentions_without_entity, 1);
        assert_eq!(report.broken_merges, 1);
        assert_eq!(report.name_index_rows_without_alias, 1);
        assert!(!report.is_clean());
    }

    #[test]
    fn the_integrity_check_sees_a_cycle() {
        let mut connection = open();
        let (alice, alyce) = two_people(&mut connection);
        connection.execute("PRAGMA foreign_keys = OFF", []).unwrap();
        for (entity, target) in [(alice, alyce), (alyce, alice)] {
            connection
                .execute(
                    "UPDATE kb_entities SET status = 'merged', merged_into = ?2
                     WHERE entity_id = ?1",
                    params![entity, target],
                )
                .unwrap();
        }

        let report = integrity_check(&connection).unwrap();

        assert_eq!(report.merge_cycles, 2);
        assert_eq!(report.broken_merges, 2, "each points at a merged entity");
    }

    #[test]
    fn the_integrity_check_sees_an_entity_the_collector_missed() {
        let connection = open();
        create_entity(&connection, &new_person("Alice Archer", Origin::Automatic)).unwrap();

        assert_eq!(
            integrity_check(&connection)
                .unwrap()
                .orphan_automatic_entities,
            1
        );
        assert_eq!(gc_orphans(&connection).unwrap(), 1);
        assert_clean(&connection);
    }

    #[test]
    fn a_locked_database_gives_a_clean_code_and_never_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.sqlite3");
        let writer = Connection::open(&path).unwrap();
        crate::index_store::configure_connection(&writer).unwrap();
        ensure_schema(&writer).unwrap();
        let reader = Connection::open(&path).unwrap();
        crate::index_store::configure_connection(&reader).unwrap();
        // Waiting three seconds in a test proves nothing more than waiting fifty milliseconds.
        reader.busy_timeout(Duration::from_millis(50)).unwrap();

        writer.execute_batch("BEGIN IMMEDIATE").unwrap();
        set_meta(&writer, "packs_hash", "held").unwrap();

        // Reading while another connection holds the write lock still works...
        assert!(meta(&reader, "schema_version").unwrap().is_some());
        // ...writing gives up with a machine code once the wait is over.
        let refused = upsert_source(&reader, &source("a.txt"), 1, 0);
        assert_eq!(refused.unwrap_err().code(), "knowledge_unavailable");

        writer.execute_batch("COMMIT").unwrap();
        assert!(upsert_source(&reader, &source("a.txt"), 1, 0).is_ok());
    }

    #[test]
    fn the_sources_of_a_domain_are_listed_with_their_versions() {
        let mut connection = open();
        let mut delta = alice_in("a.txt");
        delta.gazetteer_epoch = 4;
        apply(&mut connection, &delta);
        apply(&mut connection, &KnowledgeDelta::empty(source("b.txt")));

        let rows = sources(&connection, Some(Domain::Documents)).unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0].kb_version, rows[0].gazetteer_epoch),
            (CURRENT_KB_VERSION, 4)
        );
        assert_eq!(
            rows[1].kb_version,
            crate::knowledge::NOT_EXTRACTED,
            "a source nobody extracted from is due for extraction"
        );
        assert!(sources(&connection, Some(Domain::Data)).unwrap().is_empty());
    }
}
