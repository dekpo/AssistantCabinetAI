//! The entity sheet of a selection (lot 9): what the knowledge base knows about one name, **inside
//! the files the user selected**.
//!
//! It opens from a chip or from the name list of the composer, to answer "who is this, and where do
//! I find them?". It is not the management dialog's sheet (`manage::entity_detail`, lot 8), which
//! shows everything the base holds with where each association came from, for an administrator.
//! This one is for the person who is asking a question:
//!
//! - the other ways the name is written (aliases), the type, the roles it plays;
//! - the files **of the selection** that mention it: documents with the pages, tables with the
//!   sheet, the column and the number of rows;
//! - **counts only** for the files outside the selection: how many, never which (invariant I7);
//! - the relations stated by the files of the selection, and the details that are not personal
//!   identifiers.
//!
//! Read-only. Nothing is logged: a sheet holds names, which are the user's data.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::Connection;
use serde::Serialize;

use super::packs::PackSet;
use super::query::QuerySelection;
use super::store;
use super::surface::selection_source_ids;
use super::EntityStatus;
use super::SourceRef;
use crate::error::AppError;

/// Pages listed for one document. A hint to where to look, not an index.
pub const MAX_PAGES: usize = 20;

/// Relations listed on a sheet.
pub const MAX_RELATIONS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetRole {
    pub role: String,
    pub mentions: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetDocument {
    pub relative_path: String,
    pub mentions: i64,
    /// Where it is named, when the file has pages. Sorted, at most `MAX_PAGES`.
    pub pages: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetTable {
    pub relative_path: String,
    pub sheet: String,
    pub column: String,
    pub rows: i64,
}

/// How many analysed files outside the selection also name it. Never which.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetOutside {
    pub documents: usize,
    pub workbooks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetRelation {
    /// A machine id; the interface turns it into words.
    pub predicate: String,
    pub other_entity_id: i64,
    pub other_display: String,
    /// `subject` when this entity is the one that "does" it (`client` of the other), `object`
    /// when it is the other way round.
    pub direction: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetAttribute {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitySheet {
    pub entity_id: i64,
    pub display: String,
    pub type_id: String,
    pub subtype: Option<String>,
    /// `candidate` or `active`: the interface says "to confirm" for the first.
    pub status: &'static str,
    /// Other ways the name is written, the canonical name left out.
    pub aliases: Vec<String>,
    pub roles: Vec<SheetRole>,
    pub documents: Vec<SheetDocument>,
    pub tables: Vec<SheetTable>,
    pub outside: SheetOutside,
    pub relations: Vec<SheetRelation>,
    pub attributes: Vec<SheetAttribute>,
}

fn db<T>(result: rusqlite::Result<T>) -> Result<T, AppError> {
    result.map_err(|_| AppError::KnowledgeUnavailable)
}

/// The sheet of one entity inside a selection. `None` when the entity does not exist or was
/// deleted. An entity with no file in the selection still has a sheet (its outside counts), because
/// the id came from a surface that offered it: a chip, the name list, or the labelled offer to add
/// files.
pub fn entity_sheet(
    connection: &Connection,
    packs: &PackSet,
    entity_id: i64,
    selection: &QuerySelection<'_>,
) -> Result<Option<EntitySheet>, AppError> {
    let Some(entity) = store::effective_entity(connection, entity_id)? else {
        return Ok(None);
    };
    let status = match entity.status {
        EntityStatus::Active => "active",
        EntityStatus::Candidate => "candidate",
        EntityStatus::Merged | EntityStatus::Deleted => return Ok(None),
    };
    let effective = entity.entity_id;
    let sources = selection_source_ids(connection, selection)?;
    let inside = serde_json::to_string(&sources).map_err(|_| AppError::Internal)?;

    // The other spellings. An absorbed entity's names belong to the survivor.
    let mut statement = db(connection.prepare(
        "SELECT a.display, a.normalized
         FROM kb_aliases a
         JOIN kb_effective_entity v ON v.entity_id = a.entity_id
         WHERE v.effective_id = ?1
         ORDER BY a.alias_id",
    ))?;
    let rows = db(statement.query_map([effective], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }))?;
    let mut aliases: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    seen.insert(entity.normalized_name.clone());
    for row in rows {
        let (display, normalized) = db(row)?;
        if seen.insert(normalized) {
            aliases.push(display);
        }
    }
    drop(statement);

    let mut statement = db(connection.prepare(
        "SELECT m.role, COALESCE(SUM(m.occurrence_count), 0)
         FROM kb_effective_mentions m
         WHERE m.effective_entity_id = ?1 AND m.role IS NOT NULL
           AND m.source_id IN (SELECT value FROM json_each(?2))
         GROUP BY m.role ORDER BY 2 DESC, m.role",
    ))?;
    let rows = db(
        statement.query_map(rusqlite::params![effective, inside], |row| {
            Ok(SheetRole {
                role: row.get(0)?,
                mentions: row.get(1)?,
            })
        }),
    )?;
    let roles = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::KnowledgeUnavailable)?;
    drop(statement);

    let mut statement = db(connection.prepare(
        "SELECT s.relative_path, COALESCE(SUM(m.occurrence_count), 0),
                GROUP_CONCAT(DISTINCT c.page_number)
         FROM kb_effective_mentions m
         JOIN kb_sources s ON s.source_id = m.source_id
         LEFT JOIN chunks c ON c.chunk_id = m.chunk_id
         WHERE m.effective_entity_id = ?1 AND s.domain = 'documents'
           AND m.source_id IN (SELECT value FROM json_each(?2))
         GROUP BY s.source_id ORDER BY s.relative_path",
    ))?;
    let rows = db(
        statement.query_map(rusqlite::params![effective, inside], |row| {
            let pages: Option<String> = row.get(2)?;
            let mut pages: Vec<i64> = pages
                .unwrap_or_default()
                .split(',')
                .filter_map(|page| page.trim().parse::<i64>().ok())
                .collect();
            pages.sort_unstable();
            pages.dedup();
            pages.truncate(MAX_PAGES);
            Ok(SheetDocument {
                relative_path: row.get(0)?,
                mentions: row.get(1)?,
                pages,
            })
        }),
    )?;
    let documents = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::KnowledgeUnavailable)?;
    drop(statement);

    let mut statement = db(connection.prepare(
        "SELECT s.relative_path, COALESCE(m.sheet, ''), COALESCE(m.column_name, ''),
                COALESCE(SUM(m.occurrence_count), 0)
         FROM kb_effective_mentions m
         JOIN kb_sources s ON s.source_id = m.source_id
         WHERE m.effective_entity_id = ?1 AND s.domain = 'data' AND m.locator_kind = 'cells'
           AND m.source_id IN (SELECT value FROM json_each(?2))
         GROUP BY s.source_id, m.sheet, m.column_name
         ORDER BY s.relative_path, m.sheet, m.column_name",
    ))?;
    let rows = db(
        statement.query_map(rusqlite::params![effective, inside], |row| {
            Ok(SheetTable {
                relative_path: row.get(0)?,
                sheet: row.get(1)?,
                column: row.get(2)?,
                rows: row.get(3)?,
            })
        }),
    )?;
    let tables = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::KnowledgeUnavailable)?;
    drop(statement);

    // Counts only for the rest of the base: how many files, never which.
    let mut statement = db(connection.prepare(
        "SELECT s.domain, COUNT(DISTINCT m.source_id)
         FROM kb_effective_mentions m
         JOIN kb_sources s ON s.source_id = m.source_id
         WHERE m.effective_entity_id = ?1
           AND m.source_id NOT IN (SELECT value FROM json_each(?2))
         GROUP BY s.domain",
    ))?;
    let rows = db(
        statement.query_map(rusqlite::params![effective, inside], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        }),
    )?;
    let mut outside = SheetOutside::default();
    for row in rows {
        let (domain, count) = db(row)?;
        match domain.as_str() {
            "documents" => outside.documents = count.max(0) as usize,
            "data" => outside.workbooks = count.max(0) as usize,
            _ => {}
        }
    }
    drop(statement);

    // Relations the files of the selection state. The other end must be a confirmed name.
    let mut statement = db(connection.prepare(
        "SELECT r.predicate, vs.effective_id, vo.effective_id
         FROM kb_relations r
         JOIN kb_effective_entity vs ON vs.entity_id = r.subject
         JOIN kb_effective_entity vo ON vo.entity_id = r.object
         WHERE r.source_id IN (SELECT value FROM json_each(?2))
           AND (vs.effective_id = ?1 OR vo.effective_id = ?1)
         GROUP BY r.predicate, vs.effective_id, vo.effective_id
         ORDER BY r.predicate, vs.effective_id, vo.effective_id",
    ))?;
    let rows = db(
        statement.query_map(rusqlite::params![effective, inside], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        }),
    )?;
    let mut relations = Vec::new();
    for row in rows {
        let (predicate, subject, object) = db(row)?;
        let (other, direction) = if subject == effective {
            (object, "subject")
        } else {
            (subject, "object")
        };
        if other == effective {
            continue;
        }
        let Some(other_entity) = store::effective_entity(connection, other)? else {
            continue;
        };
        if other_entity.status != EntityStatus::Active {
            continue;
        }
        relations.push(SheetRelation {
            predicate,
            other_entity_id: other_entity.entity_id,
            other_display: other_entity.canonical_name,
            direction,
        });
        if relations.len() == MAX_RELATIONS {
            break;
        }
    }
    drop(statement);

    // Details that are not personal identifiers: those stay out of the sheet (they are stored as
    // keyed hashes, and a screen is visible to whoever stands behind the user).
    let sensitive: BTreeSet<&str> = packs
        .identifier_schemes()
        .iter()
        .flat_map(|scheme| [scheme.id.as_str(), scheme.kind.as_str()])
        .collect();
    let mut statement = db(connection.prepare(
        "SELECT t.key, t.value
         FROM kb_attributes t
         JOIN kb_effective_entity v ON v.entity_id = t.entity_id
         WHERE v.effective_id = ?1
           AND (t.source_id IS NULL OR t.source_id IN (SELECT value FROM json_each(?2)))
         ORDER BY t.key, t.value",
    ))?;
    let rows = db(
        statement.query_map(rusqlite::params![effective, inside], |row| {
            Ok(SheetAttribute {
                key: row.get(0)?,
                value: row.get(1)?,
            })
        }),
    )?;
    let mut attributes: BTreeMap<(String, String), SheetAttribute> = BTreeMap::new();
    for row in rows {
        let attribute = db(row)?;
        if attribute.key.starts_with("identifier:") || sensitive.contains(attribute.key.as_str()) {
            continue;
        }
        attributes.insert((attribute.key.clone(), attribute.value.clone()), attribute);
    }
    drop(statement);

    Ok(Some(EntitySheet {
        entity_id: effective,
        display: entity.canonical_name,
        type_id: entity.type_id,
        subtype: entity.subtype,
        status,
        aliases,
        roles,
        documents,
        tables,
        outside,
        relations,
        attributes: attributes.into_values().collect(),
    }))
}

/// For each of `entity_ids`, the files **of the selection** that mention it (after following
/// merges), as sources. The same query the sheet and the suggestions are built on, offered to the
/// features that come later (a calendar, a file organiser) so that they never write their own.
/// A name with no file in the selection is simply absent from the result: nothing outside the
/// selection is revealed. No command exposes it yet, because nothing in the interface needs it.
pub fn files_for_entities(
    connection: &Connection,
    entity_ids: &[i64],
    selection: &QuerySelection<'_>,
) -> Result<BTreeMap<i64, Vec<SourceRef>>, AppError> {
    let sources = selection_source_ids(connection, selection)?;
    let found = store::files_for_entities(connection, entity_ids, &sources)?;
    Ok(found
        .per_entity
        .into_iter()
        .filter(|(_, files)| !files.is_empty())
        .map(|(entity_id, files)| (entity_id, files.into_iter().collect()))
        .collect())
}
