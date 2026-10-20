//! What the knowledge base and the file profile know of each analysed file, read once per folder, for the
//! explorer's filters (`docs/KNOWLEDGE-BASE.md`, "Filters of the explorer"). The explorer works the counts
//! and the filters out of these facts itself, so that the number beside a choice can leave out the choices of
//! its own kind, which a single result's facets cannot do, and so that a filter change costs no call at all.
//!
//! Data and machine codes only: no sentence. This module only reads. It never writes to the profile, to
//! `kb_*` or to the spreadsheet tables, and it reads no file.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use rusqlite::{params, Connection};
use serde::Serialize;

use super::engine::name_date;
use crate::error::AppError;
use crate::file_profile::build::Domain;

/// A merge chain is followed at most this far, against a loop in a hand-edited base.
const MAX_MERGE_STEPS: usize = 8;
/// A date span of a workbook counts for at most this many years, against a degenerate range.
const MAX_SPAN_YEARS: i32 = 100;

fn unavailable<E>(_: E) -> AppError {
    AppError::IndexUnavailable
}

/// One person or organisation linked to a file, with every role that links them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntityLink {
    pub entity_id: i64,
    /// The roles of the file's profile (`author`, `subject`...) and of the knowledge base (`client`...), or
    /// `mention` for a name the base met in the text with no role. Sorted, once each.
    pub roles: Vec<String>,
}

/// The facts of one analysed file. A file that carries none is not listed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileFacts {
    pub relative_path: String,
    pub business_type: Option<String>,
    /// `YYYY`, ascending: the year of the document's date, of the date its name starts with, and every year
    /// a workbook's date columns span.
    pub years: Vec<String>,
    pub entities: Vec<FileEntityLink>,
}

/// A person or an organisation the files link to, under the name the base gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactEntity {
    pub entity_id: i64,
    pub name: String,
    /// `person` or `organization`: the interface sorts them into two lists whatever packs are active.
    pub type_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderFacts {
    pub files: Vec<FileFacts>,
    /// Only the people and organisations some file is linked to, in the order of their ids.
    pub entities: Vec<FactEntity>,
}

struct KnownEntity {
    name: String,
    type_id: String,
    status: String,
    merged_into: Option<i64>,
}

/// Every analysed file of `domain` with what is known of it, and the names of the entities they link to.
/// Two reads of the profile tables and two of the knowledge base: no file, no model.
pub fn folder_facts(connection: &Connection, domain: Domain) -> Result<FolderFacts, AppError> {
    let known = load_entities(connection)?;
    let mut files: BTreeMap<String, FileFacts> = BTreeMap::new();
    let mut years: HashMap<String, BTreeSet<String>> = HashMap::new();

    {
        let mut statement = connection
            .prepare(
                "SELECT relative_path, business_type, document_date FROM file_profiles
                 WHERE domain = ?1 AND state = 'analysed'",
            )
            .map_err(unavailable)?;
        let rows = statement
            .query_map([domain.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(unavailable)?;
        for (path, business_type, date) in rows.flatten() {
            let set = years.entry(path.clone()).or_default();
            for candidate in [date, name_date(&path)].into_iter().flatten() {
                if let Some(year) = year_of(&candidate) {
                    set.insert(year);
                }
            }
            files.insert(
                path.clone(),
                FileFacts {
                    relative_path: path,
                    business_type,
                    years: Vec::new(),
                    entities: Vec::new(),
                },
            );
        }
    }

    {
        let mut statement = connection
            .prepare(
                "SELECT relative_path, value FROM file_profile_fields
                 WHERE kind = 'date_range' AND domain = ?1",
            )
            .map_err(unavailable)?;
        let rows = statement
            .query_map([domain.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(unavailable)?;
        for (path, value) in rows.flatten() {
            let Some((from, to)) = value.split_once('/') else {
                continue;
            };
            if let (Some(first), Some(last)) = (year_of(from), year_of(to)) {
                if let (Ok(first), Ok(last)) = (first.parse::<i32>(), last.parse::<i32>()) {
                    let set = years.entry(path).or_default();
                    for year in first..=last.min(first + MAX_SPAN_YEARS) {
                        set.insert(format!("{year:04}"));
                    }
                }
            }
        }
    }

    // One link per (file, entity, role), whichever table says it; a merged entity answers as the one it was
    // merged into, and an entity the base deleted links nothing.
    let mut links: HashSet<(String, i64, String)> = HashSet::new();
    for sql in [
        "SELECT relative_path, entity_id, role FROM file_profile_entities
         WHERE domain = ?1 AND entity_id IS NOT NULL",
        "SELECT sources.relative_path, mentions.entity_id, COALESCE(mentions.role, 'mention')
         FROM kb_mentions AS mentions
         JOIN kb_sources AS sources ON sources.source_id = mentions.source_id
         WHERE sources.domain = ?1",
    ] {
        let mut statement = connection.prepare(sql).map_err(unavailable)?;
        let mut rows = statement.query([domain.as_str()]).map_err(unavailable)?;
        while let Some(row) = rows.next().map_err(unavailable)? {
            let (Ok(path), Ok(entity_id), Ok(role)) = (
                row.get::<_, String>(0),
                row.get::<_, i64>(1),
                row.get::<_, String>(2),
            ) else {
                continue;
            };
            if !files.contains_key(&path) {
                continue;
            }
            if let Some(survivor) = survivor_of(&known, entity_id) {
                links.insert((path, survivor, role));
            }
        }
    }

    let mut by_file: HashMap<String, BTreeMap<i64, BTreeSet<String>>> = HashMap::new();
    let mut listed: BTreeSet<i64> = BTreeSet::new();
    for (path, entity_id, role) in links {
        listed.insert(entity_id);
        by_file
            .entry(path)
            .or_default()
            .entry(entity_id)
            .or_default()
            .insert(role);
    }

    let mut result = Vec::new();
    for (path, mut facts) in files {
        facts.years = years
            .remove(&path)
            .map(|set| set.into_iter().collect())
            .unwrap_or_default();
        facts.entities = by_file
            .remove(&path)
            .map(|linked| {
                linked
                    .into_iter()
                    .map(|(entity_id, roles)| FileEntityLink {
                        entity_id,
                        roles: roles.into_iter().collect(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        if facts.business_type.is_some() || !facts.years.is_empty() || !facts.entities.is_empty() {
            result.push(facts);
        }
    }
    let entities = listed
        .into_iter()
        .filter_map(|entity_id| {
            known.get(&entity_id).map(|entity| FactEntity {
                entity_id,
                name: entity.name.clone(),
                type_id: entity.type_id.clone(),
            })
        })
        .collect();
    Ok(FolderFacts {
        files: result,
        entities,
    })
}

/// The people and organisations of the base, by id. The other types (places, identifiers, items, terms)
/// narrow nothing in the explorer.
fn load_entities(connection: &Connection) -> Result<HashMap<i64, KnownEntity>, AppError> {
    let mut statement = connection
        .prepare(
            "SELECT entity_id, canonical_name, type_id, status, merged_into FROM kb_entities
             WHERE type_id IN ('person', 'organization')",
        )
        .map_err(unavailable)?;
    let rows = statement
        .query_map(params![], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                KnownEntity {
                    name: row.get(1)?,
                    type_id: row.get(2)?,
                    status: row.get(3)?,
                    merged_into: row.get(4)?,
                },
            ))
        })
        .map_err(unavailable)?;
    Ok(rows.flatten().collect())
}

/// The entity a link answers to: itself, or the one it was merged into. `None` for a deleted entity, an
/// entity of another type and a chain that does not end.
fn survivor_of(known: &HashMap<i64, KnownEntity>, entity_id: i64) -> Option<i64> {
    let mut current = entity_id;
    for _ in 0..=MAX_MERGE_STEPS {
        let entity = known.get(&current)?;
        match entity.merged_into {
            Some(next) => current = next,
            None => return (entity.status != "deleted").then_some(current),
        }
    }
    None
}

/// The year of a date written `YYYY`, `YYYY-MM` or `YYYY-MM-DD`.
fn year_of(date: &str) -> Option<String> {
    let year = date.get(..4)?;
    year.chars()
        .all(|c| c.is_ascii_digit())
        .then(|| year.to_string())
}
