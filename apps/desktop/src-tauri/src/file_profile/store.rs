//! The profile tables and the pass that fills them (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 4.8, 5, 8).
//!
//! ```text
//! file_profiles                one row per file: identity, type, title, chosen date, build and epoch
//! file_profile_fields          what was read, one row per fact, with its source and confidence
//! file_profile_entities        the people and organisations a file is linked to, with role and method
//! file_profile_sections        (empty for now) the table of contents and per-section facts of a long document
//! file_profile_original_names  the name a file had before the Analyse cleaned it
//! file_profile_fts             the profile's text, one column per kind of text, for `bm25()` weights later
//! file_profile_meta            the schema version each folder was last profiled under
//! ```
//!
//! **New tables only, and the spreadsheet engine is not touched.** The pass reads
//! `documents`/`chunks` (the text the Analyse already stored) and `tabular_inventories` (read only),
//! never writes to them, and never writes to `kb_*`: new evidence goes in `file_profile_entities`
//! (design 4.8), because a new kind of mention in `kb_mentions` would change which files targeting
//! keeps and which workbooks the deterministic engine is offered.
//!
//! **Two layers, one pass.** Layer 1 (type, title, dates, properties, header facts, name words) needs no
//! knowledge base and is rebuilt only when the file's content, the builder or the vocabulary changed.
//! Layer 2 (links to the people and organisations the base already holds) is recomputed from the stored
//! surfaces, with no file read, when the base's gazetteer epoch moved. With the knowledge mode `off`
//! layer 1 is complete and layer 2 keeps only surfaces. A second pass over unchanged files writes
//! nothing. The pass is per-file, each file in its own transaction, so a pass interrupted at any point
//! loses nothing and the next one completes it; a failing file is counted and skipped, never blocking.
//!
//! **Old index.** Opening an index written before these tables existed adds them empty and changes
//! nothing else. `index_status` says, by one cheap query, that a folder has content and no profile yet,
//! so the interface can say "the index must be redone: press Analyse"; only an Analyse the user
//! started fills them.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::build::{build_profile, Domain, FileProfile, ProfileInput, WorkbookFacts};
use super::cells::{read_cells, CellsReading, ColumnRole};
use super::header::LETTERHEAD_LINES;
use super::identifiers::{identifiers_in, IdentifierForms, MAX_IDENTIFIERS_PER_FILE};
use super::metadata::{read_metadata, MetadataReading};
use super::name::{link_name_words, words_of, NameProfile, NAME_LINK_CONFIDENCE};
use super::vocabulary::ProfileVocabulary;
use crate::error::AppError;
use crate::index_store::IndexStore;
use crate::knowledge::extract::text::names_the_whole_entity;
use crate::knowledge::gazetteer::{is_listed_type, Gazetteer};
use crate::knowledge::packs::PackSet;
use crate::knowledge::store::column_semantics_for;
use crate::tabular::inventory::TabularInventory;

/// The layout of the tables. A change here is a migration.
pub const SCHEMA_VERSION: u32 = 1;
/// The generation of the builder's logic. Bumped when what a profile holds for the same file changes:
/// every profile is then rebuilt by the next Analyse.
pub const BUILD_VERSION: u32 = 1;
/// The most header words stored per file.
const MAX_HEADER_WORDS: usize = 400;
/// The confidence of a link made by finding a known name in a free-text cell.
pub const CELL_LINK_CONFIDENCE: f32 = 0.60;
/// The most links kept for one workbook's free-text cells.
const MAX_CELL_LINKS: usize = 2_000;
/// The confidence of an issuer read only from the letterhead.
pub const LETTERHEAD_CONFIDENCE: f32 = 0.60;
/// A letterhead line is at most this many words, and the organisation's name covers two thirds of it.
const LETTERHEAD_MAX_WORDS: usize = 10;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS file_profile_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS file_profiles (
    profile_id INTEGER PRIMARY KEY AUTOINCREMENT,
    domain TEXT NOT NULL CHECK (domain IN ('documents', 'data')),
    relative_path TEXT NOT NULL,
    content_id TEXT NOT NULL,
    format TEXT NOT NULL,
    state TEXT NOT NULL,
    build_version INTEGER NOT NULL,
    vocabulary_hash TEXT NOT NULL,
    gazetteer_epoch INTEGER,
    business_type TEXT,
    business_type_confidence REAL,
    business_type_sources TEXT,
    title TEXT,
    title_source TEXT,
    title_confidence REAL,
    document_date TEXT,
    document_date_source TEXT,
    document_date_confidence REAL,
    UNIQUE (domain, relative_path)
);
CREATE TABLE IF NOT EXISTS file_profile_fields (
    domain TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    kind TEXT NOT NULL,
    detail TEXT NOT NULL DEFAULT '',
    value TEXT NOT NULL,
    source TEXT NOT NULL,
    confidence REAL,
    position INTEGER NOT NULL DEFAULT 0,
    chosen INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS file_profile_fields_by_file
    ON file_profile_fields (domain, relative_path);
CREATE TABLE IF NOT EXISTS file_profile_entities (
    domain TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    entity_id INTEGER,
    surface TEXT NOT NULL,
    type_id TEXT,
    role TEXT NOT NULL,
    source TEXT NOT NULL,
    method TEXT NOT NULL,
    confidence REAL NOT NULL,
    locator TEXT NOT NULL DEFAULT 'file'
);
CREATE INDEX IF NOT EXISTS file_profile_entities_by_file
    ON file_profile_entities (domain, relative_path);
CREATE INDEX IF NOT EXISTS file_profile_entities_by_entity
    ON file_profile_entities (entity_id);
CREATE TABLE IF NOT EXISTS file_profile_sections (
    domain TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    position INTEGER NOT NULL,
    level INTEGER NOT NULL,
    title TEXT NOT NULL,
    page_from INTEGER,
    page_to INTEGER,
    source TEXT NOT NULL,
    PRIMARY KEY (domain, relative_path, position)
);
CREATE TABLE IF NOT EXISTS file_profile_original_names (
    domain TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    original_name TEXT NOT NULL,
    PRIMARY KEY (domain, relative_path)
);
CREATE VIRTUAL TABLE IF NOT EXISTS file_profile_fts USING fts5(
    domain UNINDEXED,
    relative_path UNINDEXED,
    name,
    folder,
    title,
    author,
    subject,
    entities,
    headers,
    cells,
    identifiers,
    prefix = '2 3 4'
);
";

/// Create the tables if they are not there. Additive: nothing existing is read or changed.
pub fn ensure_schema(connection: &Connection) -> Result<(), AppError> {
    connection
        .execute_batch(SCHEMA)
        .map_err(|_| AppError::IndexUnavailable)
}

/// What the profile of one folder looks like to the interface at launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FolderProfileStatus {
    /// Nothing is analysed in this folder: there is nothing to rebuild.
    Empty,
    /// The folder's files were profiled under the current layout.
    Current,
    /// The folder holds analysed files and no profile of the current layout: the user presses Analyse.
    NeedsAnalyse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileIndexStatus {
    pub documents: FolderProfileStatus,
    pub data: FolderProfileStatus,
}

fn meta_key(domain: Domain) -> String {
    format!("profile_schema.{}", domain.as_str())
}

/// The status of both folders, from four cheap lookups. Reads only; an old index is left exactly as it is.
pub fn index_status(connection: &Connection) -> ProfileIndexStatus {
    let has_rows = |sql: &str| -> bool {
        connection
            .query_row(sql, [], |row| row.get::<_, i64>(0))
            .map(|found| found != 0)
            .unwrap_or(false)
    };
    let current = |domain: Domain| -> bool {
        connection
            .query_row(
                "SELECT value FROM file_profile_meta WHERE key = ?1",
                [meta_key(domain)],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()
            .is_some_and(|value| value == SCHEMA_VERSION.to_string())
    };
    let status = |content: bool, domain: Domain| {
        if !content {
            FolderProfileStatus::Empty
        } else if current(domain) {
            FolderProfileStatus::Current
        } else {
            FolderProfileStatus::NeedsAnalyse
        }
    };
    ProfileIndexStatus {
        documents: status(
            has_rows("SELECT EXISTS (SELECT 1 FROM documents)"),
            Domain::Documents,
        ),
        data: status(
            has_rows("SELECT EXISTS (SELECT 1 FROM tabular_inventories)"),
            Domain::Data,
        ),
    }
}

/// Everything a pass needs besides the index.
#[derive(Clone, Copy)]
pub struct ProfileSettings<'a> {
    pub vocabulary: &'a ProfileVocabulary,
    pub packs: &'a PackSet,
    /// The people and organisations the base holds. `None` with the knowledge mode `off`: layer 2 then
    /// keeps surfaces only.
    pub gazetteer: Option<&'a Gazetteer>,
}

/// Counts, never a name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    /// Profiles built or rebuilt (layer 1, and layer 2 with it).
    pub built: usize,
    /// Profiles whose layer 1 was current and whose links were recomputed from the stored surfaces.
    pub relinked: usize,
    pub unchanged: usize,
    /// Profiles of files that are gone, forgotten.
    pub removed: usize,
    /// Files whose profile could not be written. The pass went on; the next one tries again.
    pub errors: usize,
    /// False when the pass was cancelled before the last file.
    pub complete: bool,
    /// What the pass cost, in milliseconds, so the Analyse's summary shows the price of the profile.
    pub elapsed_ms: u64,
}

/// Record that `renamed` files had these names before the Analyse cleaned them. Keeps the earliest
/// original when a file is renamed again.
pub fn record_original_names(
    connection: &Connection,
    domain: Domain,
    renamed: &[crate::filename_sanitizer::Renamed],
) -> Result<(), AppError> {
    for entry in renamed {
        let carried: Option<String> = connection
            .query_row(
                "SELECT original_name FROM file_profile_original_names WHERE domain = ?1 AND relative_path = ?2",
                params![domain.as_str(), entry.from],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| AppError::IndexUnavailable)?;
        let original = carried.unwrap_or_else(|| {
            entry
                .from
                .rsplit('/')
                .next()
                .unwrap_or(&entry.from)
                .to_string()
        });
        connection
            .execute(
                "INSERT OR REPLACE INTO file_profile_original_names (domain, relative_path, original_name)
                 VALUES (?1, ?2, ?3)",
                params![domain.as_str(), entry.to, original],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
    }
    Ok(())
}

impl IndexStore {
    /// Forget the profile of every file of one folder (the folder's Reset). Nothing else is touched.
    pub fn clear_profiles(&mut self, domain: Domain) -> Result<(), AppError> {
        let tx = self.transaction()?;
        for path in profile_paths(&tx, domain)? {
            delete_profile(&tx, domain, &path)?;
        }
        tx.execute(
            "DELETE FROM file_profile_original_names WHERE domain = ?1",
            [domain.as_str()],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        tx.execute(
            "DELETE FROM file_profile_meta WHERE key = ?1",
            [meta_key(domain)],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)
    }
}

// ---------------------------------------------------------------------------------------------
// The pass
// ---------------------------------------------------------------------------------------------

/// The profile of every analysed document. `work_folder` is read for the properties of changed files only.
pub fn refresh_documents(
    index: &mut IndexStore,
    work_folder: &Path,
    settings: &ProfileSettings<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<ProfileSummary, AppError> {
    let documents = index.all_documents()?;
    let items: Vec<Item> = documents
        .into_iter()
        .map(|row| Item {
            relative_path: row.relative_path,
            content_id: row.sha256,
            empty: row.empty,
            workbook: None,
        })
        .collect();
    refresh(
        index,
        Domain::Documents,
        work_folder,
        items,
        settings,
        cancelled,
    )
}

/// The profile of every analysed workbook. Reads `tabular_inventories` and never writes to it.
pub fn refresh_workbooks(
    index: &mut IndexStore,
    data_folder: &Path,
    settings: &ProfileSettings<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<ProfileSummary, AppError> {
    let rows: Vec<(String, String, String)> = {
        let connection = index.connection();
        let mut statement = connection
            .prepare("SELECT relative_path, workbook_id, inventory_json FROM tabular_inventories ORDER BY relative_path")
            .map_err(|_| AppError::IndexUnavailable)?;
        let found = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|_| AppError::IndexUnavailable)?;
        found
            .collect::<Result<_, _>>()
            .map_err(|_| AppError::IndexUnavailable)?
    };
    let mut items = Vec::new();
    for (relative_path, content_id, json) in rows {
        let inventory = serde_json::from_str::<TabularInventory>(&json).ok();
        let facts = inventory.as_ref().map(|inventory| {
            let sheet_names = inventory
                .sheets
                .iter()
                .map(|sheet| sheet.name.clone())
                .collect();
            let mut column_headers: Vec<String> = Vec::new();
            for sheet in &inventory.sheets {
                for column in &sheet.columns {
                    if !column_headers.contains(&column.name) {
                        column_headers.push(column.name.clone());
                    }
                }
            }
            WorkbookFacts {
                sheet_names,
                column_headers,
            }
        });
        items.push(Item {
            relative_path,
            content_id,
            empty: false,
            workbook: facts,
        });
    }
    refresh(index, Domain::Data, data_folder, items, settings, cancelled)
}

struct Item {
    relative_path: String,
    content_id: String,
    empty: bool,
    workbook: Option<WorkbookFacts>,
}

struct StoredProfile {
    state: String,
    content_id: String,
    build_version: u32,
    vocabulary_hash: String,
    gazetteer_epoch: Option<i64>,
}

fn refresh(
    index: &mut IndexStore,
    domain: Domain,
    folder: &Path,
    items: Vec<Item>,
    settings: &ProfileSettings<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<ProfileSummary, AppError> {
    let mut summary = ProfileSummary::default();

    // A file that is no longer analysed loses its profile first, whatever happens next.
    let present: HashSet<&str> = items
        .iter()
        .map(|item| item.relative_path.as_str())
        .collect();
    {
        let tx = index.transaction()?;
        for path in profile_paths(&tx, domain)? {
            if !present.contains(path.as_str()) {
                delete_profile(&tx, domain, &path)?;
                summary.removed += 1;
            }
        }
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
    }

    let stored = stored_profiles(index.connection(), domain)?;
    let originals = original_names(index.connection(), domain)?;
    let epoch = settings
        .gazetteer
        .map(|gazetteer| i64::from(gazetteer.epoch()));

    summary.complete = true;
    for item in &items {
        if cancelled() {
            summary.complete = false;
            break;
        }
        let current_layer_one = stored.get(&item.relative_path).is_some_and(|row| {
            row.content_id == item.content_id
                && row.build_version == BUILD_VERSION
                && row.vocabulary_hash == settings.vocabulary.hash()
                && row.state != STATE_CELLS_MISSING
        });
        let original = originals.get(&item.relative_path).map(String::as_str);
        let outcome = if current_layer_one {
            let linked_at = stored[&item.relative_path].gazetteer_epoch;
            if linked_at == epoch {
                summary.unchanged += 1;
                continue;
            }
            // A workbook's known-names pass reads its cells again; when they are not at rest the links
            // wait for the next pass and the epoch is not stamped.
            let cells = load_cells(index, domain, folder, item, settings);
            if domain == Domain::Data && settings.gazetteer.is_some() && cells.is_none() {
                summary.unchanged += 1;
                continue;
            }
            write_links(
                index,
                domain,
                &item.relative_path,
                original,
                epoch,
                settings,
                cells.as_ref(),
            )
            .map(|()| summary.relinked += 1)
        } else {
            let cells = load_cells(index, domain, folder, item, settings);
            build_one(
                index,
                domain,
                folder,
                item,
                original,
                epoch,
                settings,
                cells.as_ref(),
            )
            .map(|()| summary.built += 1)
        };
        if outcome.is_err() {
            summary.errors += 1;
        }
    }
    if summary.complete && summary.errors == 0 {
        index
            .connection()
            .execute(
                "INSERT OR REPLACE INTO file_profile_meta (key, value) VALUES (?1, ?2)",
                params![meta_key(domain), SCHEMA_VERSION.to_string()],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
    }
    Ok(summary)
}

/// A workbook whose cells were not at rest when it was profiled: its words are missing, so the next pass
/// builds it again.
const STATE_CELLS_MISSING: &str = "cells_missing";

/// The words and identifiers of a workbook's cells. The typed cell cache is emptied by every Analyse of the
/// Data Folder by design (it is rebuilt when a question needs it), so the file is read again here with the
/// same reader the Analyse used, and only when a profile is built or its links are recomputed: a second
/// Analyse over unchanged workbooks reads nothing. `None` for a document, for a file that cannot be read
/// any more, or for one that is no longer the content that was analysed.
fn load_cells(
    index: &IndexStore,
    domain: Domain,
    folder: &Path,
    item: &Item,
    settings: &ProfileSettings<'_>,
) -> Option<CellsReading> {
    if domain != Domain::Data {
        return None;
    }
    let (inventory, workbook, _) = crate::tabular::build_inventory_with_source_rows(
        &folder.join(&item.relative_path),
        &item.relative_path,
        settings.packs.locale(),
    )
    .ok()?;
    if inventory.workbook_id != item.content_id {
        return None;
    }
    let entity_columns: HashSet<(String, String)> = column_semantics_for(
        index.connection(),
        crate::knowledge::Domain::Data,
        &item.relative_path,
    )
    .unwrap_or_default()
    .into_iter()
    .filter(|row| row.is_read())
    .map(|row| (row.sheet, row.column_name))
    .collect();
    Some(read_cells(
        &workbook,
        &inventory,
        &entity_columns,
        settings.packs,
        settings.vocabulary,
    ))
}

/// Every identifier of the active packs in all the passages of a document, in the forms a person may
/// type them. Reads the stored passages; opens no file.
fn text_identifiers(
    connection: &Connection,
    path: &str,
    settings: &ProfileSettings<'_>,
) -> (Vec<IdentifierForms>, bool) {
    let Ok(mut statement) = connection
        .prepare("SELECT text FROM chunks WHERE relative_path = ?1 ORDER BY page_number, section")
    else {
        return (Vec::new(), false);
    };
    let Ok(rows) = statement.query_map([path], |row| row.get::<_, String>(0)) else {
        return (Vec::new(), false);
    };
    let mut found: Vec<IdentifierForms> = Vec::new();
    let mut truncated = false;
    for text in rows.flatten() {
        for identifier in identifiers_in(&text, settings.packs, settings.vocabulary) {
            if found.iter().any(|known| {
                known.scheme_id == identifier.scheme_id && known.compact == identifier.compact
            }) {
                continue;
            }
            if found.len() >= MAX_IDENTIFIERS_PER_FILE {
                truncated = true;
                break;
            }
            found.push(identifier);
        }
    }
    (found, truncated)
}

fn stored_profiles(
    connection: &Connection,
    domain: Domain,
) -> Result<HashMap<String, StoredProfile>, AppError> {
    let mut statement = connection
        .prepare(
            "SELECT relative_path, content_id, build_version, vocabulary_hash, gazetteer_epoch, state
             FROM file_profiles WHERE domain = ?1",
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    let rows = statement
        .query_map([domain.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                StoredProfile {
                    state: row.get(5)?,
                    content_id: row.get(1)?,
                    build_version: row.get::<_, i64>(2)? as u32,
                    vocabulary_hash: row.get(3)?,
                    gazetteer_epoch: row.get(4)?,
                },
            ))
        })
        .map_err(|_| AppError::IndexUnavailable)?;
    rows.collect::<Result<_, _>>()
        .map_err(|_| AppError::IndexUnavailable)
}

fn original_names(
    connection: &Connection,
    domain: Domain,
) -> Result<HashMap<String, String>, AppError> {
    let mut statement = connection
        .prepare("SELECT relative_path, original_name FROM file_profile_original_names WHERE domain = ?1")
        .map_err(|_| AppError::IndexUnavailable)?;
    let rows = statement
        .query_map([domain.as_str()], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|_| AppError::IndexUnavailable)?;
    rows.collect::<Result<_, _>>()
        .map_err(|_| AppError::IndexUnavailable)
}

fn profile_paths(connection: &Connection, domain: Domain) -> Result<Vec<String>, AppError> {
    let mut statement = connection
        .prepare("SELECT relative_path FROM file_profiles WHERE domain = ?1")
        .map_err(|_| AppError::IndexUnavailable)?;
    let rows = statement
        .query_map([domain.as_str()], |row| row.get::<_, String>(0))
        .map_err(|_| AppError::IndexUnavailable)?;
    rows.collect::<Result<_, _>>()
        .map_err(|_| AppError::IndexUnavailable)
}

fn delete_profile(connection: &Connection, domain: Domain, path: &str) -> Result<(), AppError> {
    let id: Option<i64> = connection
        .query_row(
            "SELECT profile_id FROM file_profiles WHERE domain = ?1 AND relative_path = ?2",
            params![domain.as_str(), path],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| AppError::IndexUnavailable)?;
    if let Some(id) = id {
        connection
            .execute("DELETE FROM file_profile_fts WHERE rowid = ?1", [id])
            .map_err(|_| AppError::IndexUnavailable)?;
    }
    for table in [
        "file_profile_fields",
        "file_profile_entities",
        "file_profile_sections",
    ] {
        connection
            .execute(
                &format!("DELETE FROM {table} WHERE domain = ?1 AND relative_path = ?2"),
                params![domain.as_str(), path],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
    }
    connection
        .execute(
            "DELETE FROM file_profiles WHERE domain = ?1 AND relative_path = ?2",
            params![domain.as_str(), path],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    Ok(())
}

/// The text of the first chunk of a document: the top of its first page.
fn first_text(connection: &Connection, path: &str) -> Option<String> {
    connection
        .query_row(
            "SELECT text FROM chunks WHERE relative_path = ?1 ORDER BY page_number, section LIMIT 1",
            [path],
            |row| row.get(0),
        )
        .optional()
        .ok()
        .flatten()
}

/// Layer 1 for one file, then its layer 2, in one transaction.
#[allow(clippy::too_many_arguments)]
fn build_one(
    index: &mut IndexStore,
    domain: Domain,
    folder: &Path,
    item: &Item,
    original_name: Option<&str>,
    epoch: Option<i64>,
    settings: &ProfileSettings<'_>,
    cells: Option<&CellsReading>,
) -> Result<(), AppError> {
    let extension = item
        .relative_path
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    let metadata = read_metadata(
        &folder.join(&item.relative_path),
        &extension,
        settings.vocabulary,
    );
    let text = match domain {
        Domain::Documents if !item.empty => first_text(index.connection(), &item.relative_path),
        _ => None,
    };
    let profile = build_profile(
        &ProfileInput {
            relative_path: &item.relative_path,
            original_name,
            domain,
            first_page_text: text.as_deref(),
            metadata: Some(&metadata),
            workbook: item.workbook.as_ref(),
        },
        settings.vocabulary,
        settings.packs,
    );
    let state = if item.empty {
        "empty"
    } else if domain == Domain::Documents && text.is_none() {
        "no_text"
    } else if domain == Domain::Data && cells.is_none() {
        STATE_CELLS_MISSING
    } else {
        "analysed"
    };
    let (identifiers, truncated) = match domain {
        Domain::Documents if !item.empty => {
            text_identifiers(index.connection(), &item.relative_path, settings)
        }
        _ => (Vec::new(), false),
    };

    let tx = index.transaction()?;
    write_layer_one(
        &tx,
        domain,
        item,
        &extension,
        state,
        &profile,
        &metadata,
        item.workbook.as_ref(),
        cells,
        &identifiers,
        truncated,
        settings,
    )?;
    write_links_in(
        &tx,
        domain,
        &item.relative_path,
        original_name,
        epoch,
        settings,
        cells,
    )?;
    tx.commit().map_err(|_| AppError::IndexUnavailable)
}

#[allow(clippy::too_many_arguments)]
fn write_layer_one(
    tx: &Connection,
    domain: Domain,
    item: &Item,
    extension: &str,
    state: &str,
    profile: &FileProfile,
    metadata: &MetadataReading,
    workbook: Option<&WorkbookFacts>,
    cells: Option<&CellsReading>,
    identifiers: &[IdentifierForms],
    identifiers_truncated: bool,
    settings: &ProfileSettings<'_>,
) -> Result<(), AppError> {
    let existing: Option<i64> = tx
        .query_row(
            "SELECT profile_id FROM file_profiles WHERE domain = ?1 AND relative_path = ?2",
            params![domain.as_str(), item.relative_path],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| AppError::IndexUnavailable)?;
    for table in ["file_profile_fields", "file_profile_entities"] {
        tx.execute(
            &format!("DELETE FROM {table} WHERE domain = ?1 AND relative_path = ?2"),
            params![domain.as_str(), item.relative_path],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    }
    let chosen = profile.dates.chosen();
    let columns = (
        item.content_id.as_str(),
        extension,
        state,
        BUILD_VERSION,
        settings.vocabulary.hash(),
        profile.document_type.as_ref().map(|t| t.code.clone()),
        profile
            .document_type
            .as_ref()
            .map(|t| f64::from(t.confidence)),
        profile.document_type.as_ref().map(|t| {
            t.sources
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(",")
        }),
        profile.title.as_ref().map(|t| t.text.clone()),
        profile.title.as_ref().map(|t| t.source),
        profile.title.as_ref().map(|t| f64::from(t.confidence)),
        chosen.map(|d| d.iso.clone()),
        chosen.map(|d| d.source),
        chosen.map(|d| f64::from(d.confidence)),
    );
    match existing {
        Some(id) => {
            tx.execute(
                "UPDATE file_profiles SET content_id = ?1, format = ?2, state = ?3, build_version = ?4,
                    vocabulary_hash = ?5, gazetteer_epoch = NULL,
                    business_type = ?6, business_type_confidence = ?7, business_type_sources = ?8,
                    title = ?9, title_source = ?10, title_confidence = ?11,
                    document_date = ?12, document_date_source = ?13, document_date_confidence = ?14
                 WHERE profile_id = ?15",
                params![
                    columns.0, columns.1, columns.2, columns.3, columns.4, columns.5, columns.6, columns.7,
                    columns.8, columns.9, columns.10, columns.11, columns.12, columns.13, id
                ],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        }
        None => {
            tx.execute(
                "INSERT INTO file_profiles (content_id, format, state, build_version, vocabulary_hash,
                    business_type, business_type_confidence, business_type_sources,
                    title, title_source, title_confidence,
                    document_date, document_date_source, document_date_confidence,
                    domain, relative_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    columns.0, columns.1, columns.2, columns.3, columns.4, columns.5, columns.6, columns.7,
                    columns.8, columns.9, columns.10, columns.11, columns.12, columns.13,
                    domain.as_str(), item.relative_path
                ],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        }
    }

    let mut writer = FieldWriter {
        tx,
        domain,
        path: &item.relative_path,
        position: 0,
    };
    for (position, date) in profile.dates.all.iter().enumerate() {
        writer.put(
            "date",
            "",
            &date.iso,
            date.source,
            Some(date.confidence),
            profile.dates.chosen == Some(position),
        )?;
    }
    for reference in &profile.header.references {
        writer.put(
            "reference",
            &reference.kind,
            &reference.value,
            "header_reference",
            None,
            false,
        )?;
    }
    for identifier in &profile.name_identifiers {
        writer.put(
            "identifier",
            &identifier.scheme_id,
            &identifier.normalized,
            "filename",
            Some(0.70),
            false,
        )?;
    }
    if let Some(subject) = &profile.header.subject_line {
        writer.put(
            "subject_line",
            "",
            subject,
            "header_label",
            Some(0.85),
            false,
        )?;
    }
    for field in &metadata.fields {
        writer.put(
            "metadata",
            field.kind.as_str(),
            &field.value,
            field.source.as_str(),
            Some(field.confidence),
            false,
        )?;
    }
    for ignored in &metadata.ignored {
        writer.put(
            "metadata_ignored",
            ignored.kind.as_str(),
            ignored.reason.as_str(),
            ignored.source.as_str(),
            None,
            false,
        )?;
    }
    if metadata.unreadable {
        writer.put("metadata_state", "", "unreadable", "metadata", None, false)?;
    }
    for role in &profile.roles {
        writer.put(
            "role_surface",
            role.role,
            &role.surface,
            role.source,
            Some(role.confidence),
            false,
        )?;
    }
    for line in &profile.header.letterhead {
        writer.put("letterhead", "", line, "header", None, false)?;
    }
    if !profile.header.header_words.is_empty() {
        let words: Vec<&str> = profile
            .header
            .header_words
            .iter()
            .take(MAX_HEADER_WORDS)
            .map(String::as_str)
            .collect();
        writer.put("header_words", "", &words.join(" "), "header", None, false)?;
    }
    if let Some(period) = &profile.name.folder_period {
        writer.put(
            "folder_period",
            "",
            &period.value,
            "folder",
            Some(super::name::FOLDER_DATE_CONFIDENCE),
            false,
        )?;
    }
    if let Some(facts) = workbook {
        for name in &facts.sheet_names {
            writer.put("sheet_name", "", name, "inventory", None, false)?;
        }
        for header in &facts.column_headers {
            writer.put("column_header", "", header, "inventory", None, false)?;
        }
    }
    // Identifiers of the text, in the forms a person may type them: compact, and grouped when the pack
    // knows the layout.
    for identifier in identifiers {
        writer.put(
            "identifier",
            &identifier.scheme_id,
            &identifier.compact,
            "text",
            Some(0.90),
            false,
        )?;
        if let Some(grouped) = &identifier.grouped {
            writer.put(
                "identifier_grouped",
                &identifier.scheme_id,
                grouped,
                "text",
                Some(0.90),
                false,
            )?;
        }
    }
    if identifiers_truncated {
        writer.put(
            "identifiers_truncated",
            "",
            &MAX_IDENTIFIERS_PER_FILE.to_string(),
            "text",
            None,
            false,
        )?;
    }
    // A workbook's cells: what each column holds, its words, its date range, its identifiers.
    if let Some(cells) = cells {
        for column in &cells.columns {
            let place = column.locator();
            writer.put(
                "cells_column",
                &place,
                column.role.as_str(),
                "cells",
                None,
                false,
            )?;
            writer.put(
                "cells_distinct",
                &place,
                &column.distinct.to_string(),
                "cells",
                None,
                false,
            )?;
            if column.truncated {
                writer.put(
                    "cells_truncated",
                    &place,
                    &super::cells::MAX_FREE_TEXT_DISTINCT.to_string(),
                    "cells",
                    None,
                    false,
                )?;
            }
            let words: Vec<String> = column.words().into_iter().collect();
            if !words.is_empty() {
                writer.put("cell_words", &place, &words.join(" "), "cells", None, false)?;
            }
        }
        for range in &cells.date_ranges {
            writer.put(
                "date_range",
                &format!("{}!{}", range.sheet, range.column),
                &format!("{}/{}", range.from, range.to),
                "cells",
                None,
                false,
            )?;
        }
        for identifier in &cells.identifiers {
            writer.put(
                "identifier",
                &identifier.scheme_id,
                &identifier.compact,
                "cells",
                Some(0.90),
                false,
            )?;
            if let Some(grouped) = &identifier.grouped {
                writer.put(
                    "identifier_grouped",
                    &identifier.scheme_id,
                    grouped,
                    "cells",
                    Some(0.90),
                    false,
                )?;
            }
        }
    }
    Ok(())
}

struct FieldWriter<'a> {
    tx: &'a Connection,
    domain: Domain,
    path: &'a str,
    position: i64,
}

impl FieldWriter<'_> {
    fn put(
        &mut self,
        kind: &str,
        detail: &str,
        value: &str,
        source: &str,
        confidence: Option<f32>,
        chosen: bool,
    ) -> Result<(), AppError> {
        self.tx
            .execute(
                "INSERT INTO file_profile_fields (domain, relative_path, kind, detail, value, source, confidence, position, chosen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    self.domain.as_str(),
                    self.path,
                    kind,
                    detail,
                    value,
                    source,
                    confidence.map(f64::from),
                    self.position,
                    i64::from(chosen)
                ],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        self.position += 1;
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Layer 2: links to the entities the base holds, and the full-text row
// ---------------------------------------------------------------------------------------------

fn write_links(
    index: &mut IndexStore,
    domain: Domain,
    path: &str,
    original_name: Option<&str>,
    epoch: Option<i64>,
    settings: &ProfileSettings<'_>,
    cells: Option<&CellsReading>,
) -> Result<(), AppError> {
    let tx = index.transaction()?;
    write_links_in(&tx, domain, path, original_name, epoch, settings, cells)?;
    tx.commit().map_err(|_| AppError::IndexUnavailable)
}

/// One stored surface and what it says.
struct Surface {
    role: String,
    text: String,
    source: String,
    method: &'static str,
    confidence: f32,
}

/// Recompute the entity rows of one file from its stored surfaces, then its full-text row. Reads the
/// fields layer 1 wrote; opens no file.
fn write_links_in(
    tx: &Connection,
    domain: Domain,
    path: &str,
    original_name: Option<&str>,
    epoch: Option<i64>,
    settings: &ProfileSettings<'_>,
    cells: Option<&CellsReading>,
) -> Result<(), AppError> {
    let fields = stored_fields(tx, domain, path)?;
    tx.execute(
        "DELETE FROM file_profile_entities WHERE domain = ?1 AND relative_path = ?2",
        params![domain.as_str(), path],
    )
    .map_err(|_| AppError::IndexUnavailable)?;

    // What is written about the file by a label or a property.
    let mut surfaces: Vec<Surface> = Vec::new();
    for field in fields.iter().filter(|field| field.kind == "role_surface") {
        surfaces.push(Surface {
            role: field.detail.clone(),
            text: field.value.clone(),
            source: field.source.clone(),
            method: if field.source == "header_label" {
                "header_label"
            } else {
                "metadata"
            },
            confidence: field.confidence.unwrap_or(0.5),
        });
    }

    let mut rows: Vec<EntityRow> = Vec::new();
    let mut linked_entities: HashSet<(i64, String)> = HashSet::new();
    for surface in &surfaces {
        let link = settings
            .gazetteer
            .and_then(|gazetteer| link_surface(&words_of(&surface.text), gazetteer));
        if let Some((entity_id, _)) = &link {
            linked_entities.insert((*entity_id, surface.role.clone()));
        }
        rows.push(EntityRow {
            entity_id: link.as_ref().map(|(id, _)| *id),
            surface: surface.text.clone(),
            type_id: link.map(|(_, type_id)| type_id),
            role: surface.role.clone(),
            source: surface.source.clone(),
            method: surface.method.to_string(),
            confidence: f64::from(surface.confidence),
            locator: "header".to_string(),
        });
    }

    if let Some(gazetteer) = settings.gazetteer {
        // The issuer named in the letterhead: only when exactly one organisation answers.
        let letterhead: Vec<&FieldRow> = fields
            .iter()
            .filter(|f| f.kind == "letterhead")
            .take(LETTERHEAD_LINES)
            .collect();
        let mut organisations: Vec<(i64, String, String)> = Vec::new();
        for line in letterhead {
            let words = words_of(&line.value);
            for hit in gazetteer.matches_in(&words) {
                // A letterhead line is the organisation's name and little else: a sentence that merely
                // mentions one (`recu aux HUG pour un controle`) is not a letterhead.
                if words.len() > LETTERHEAD_MAX_WORDS || hit.len * 3 < words.len() * 2 {
                    continue;
                }
                let surface = words[hit.start..hit.start + hit.len].join(" ");
                for entry in hit
                    .entries
                    .iter()
                    .filter(|e| names_the_whole_entity(e.kind) && e.type_id == "organization")
                {
                    if !organisations
                        .iter()
                        .any(|(id, _, _)| *id == entry.entity_id)
                    {
                        organisations.push((
                            entry.entity_id,
                            entry.type_id.clone(),
                            surface.clone(),
                        ));
                    }
                }
            }
        }
        if let [(entity_id, type_id, surface)] = organisations.as_slice() {
            if !linked_entities.contains(&(*entity_id, "issuer".to_string())) {
                rows.push(EntityRow {
                    entity_id: Some(*entity_id),
                    surface: surface.clone(),
                    type_id: Some(type_id.clone()),
                    role: "issuer".to_string(),
                    source: "letterhead".to_string(),
                    method: "letterhead".to_string(),
                    confidence: f64::from(LETTERHEAD_CONFIDENCE),
                    locator: "header".to_string(),
                });
            }
        }
        // The people and organisations named in the file name, anywhere in it.
        let name = NameProfile::build(path, original_name);
        let mut name_words = vec![name.name_words.clone()];
        if !name.original_words.is_empty() {
            name_words.push(name.original_words.clone());
        }
        let mut seen: HashSet<i64> = HashSet::new();
        for words in &name_words {
            for link in link_name_words(words, gazetteer) {
                if seen.insert(link.entity_id) {
                    rows.push(EntityRow {
                        entity_id: Some(link.entity_id),
                        surface: words[link.start..link.start + link.len].join(" "),
                        type_id: Some(link.type_id),
                        role: "mentioned".to_string(),
                        source: "filename".to_string(),
                        method: "filename_token".to_string(),
                        confidence: f64::from(NAME_LINK_CONFIDENCE),
                        locator: "filename".to_string(),
                    });
                }
            }
        }
    }

    // The known-names pass over a workbook's free-text columns: a cell that holds the name of a person or
    // an organisation the base already knows is linked to it; nothing is ever created from free text, and
    // a phrase that two entities answer to is not linked. The columns the base already reads as entity
    // columns are not repeated here: the base holds their mentions.
    if let (Some(gazetteer), Some(cells)) = (settings.gazetteer, cells) {
        let mut seen: HashSet<(i64, String)> = HashSet::new();
        for column in cells
            .columns
            .iter()
            .filter(|column| column.role == ColumnRole::FreeText)
        {
            for value in &column.values {
                if rows.len() >= MAX_CELL_LINKS {
                    break;
                }
                if let Some((entity_id, type_id)) = link_surface(&words_of(value), gazetteer) {
                    if seen.insert((entity_id, column.locator())) {
                        rows.push(EntityRow {
                            entity_id: Some(entity_id),
                            surface: value.clone(),
                            type_id: Some(type_id),
                            role: "mentioned".to_string(),
                            source: "cells".to_string(),
                            method: "gazetteer".to_string(),
                            confidence: f64::from(CELL_LINK_CONFIDENCE),
                            locator: format!("cells:{}", column.locator()),
                        });
                    }
                }
            }
        }
    }

    for row in &rows {
        tx.execute(
            "INSERT INTO file_profile_entities (domain, relative_path, entity_id, surface, type_id, role, source, method, confidence, locator)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                domain.as_str(), path, row.entity_id, row.surface, row.type_id, row.role, row.source, row.method,
                row.confidence, row.locator
            ],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    }

    tx.execute(
        "UPDATE file_profiles SET gazetteer_epoch = ?1 WHERE domain = ?2 AND relative_path = ?3",
        params![epoch, domain.as_str(), path],
    )
    .map_err(|_| AppError::IndexUnavailable)?;
    write_fts(tx, domain, path, original_name, &fields, &rows)
}

struct EntityRow {
    entity_id: Option<i64>,
    surface: String,
    type_id: Option<String>,
    role: String,
    source: String,
    method: String,
    confidence: f64,
    locator: String,
}

/// The entity a surface names, when exactly one person or organisation of the base answers to it.
fn link_surface(words: &[String], gazetteer: &Gazetteer) -> Option<(i64, String)> {
    let mut found: BTreeMap<i64, String> = BTreeMap::new();
    for hit in gazetteer.matches_in(words) {
        for entry in &hit.entries {
            if names_the_whole_entity(entry.kind) && is_listed_type(&entry.type_id) {
                found.insert(entry.entity_id, entry.type_id.clone());
            }
        }
    }
    if found.len() == 1 {
        found.into_iter().next()
    } else {
        None
    }
}

struct FieldRow {
    kind: String,
    detail: String,
    value: String,
    source: String,
    confidence: Option<f32>,
}

fn stored_fields(
    connection: &Connection,
    domain: Domain,
    path: &str,
) -> Result<Vec<FieldRow>, AppError> {
    let mut statement = connection
        .prepare(
            "SELECT kind, detail, value, source, confidence FROM file_profile_fields
             WHERE domain = ?1 AND relative_path = ?2 ORDER BY position",
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    let rows = statement
        .query_map(params![domain.as_str(), path], |row| {
            Ok(FieldRow {
                kind: row.get(0)?,
                detail: row.get(1)?,
                value: row.get(2)?,
                source: row.get(3)?,
                confidence: row.get::<_, Option<f64>>(4)?.map(|c| c as f32),
            })
        })
        .map_err(|_| AppError::IndexUnavailable)?;
    rows.collect::<Result<_, _>>()
        .map_err(|_| AppError::IndexUnavailable)
}

fn folded(text: &str) -> String {
    words_of(text).join(" ")
}

/// The profile's full-text row: one column per kind of text, every word folded, so a query folded the
/// same way meets it and `bm25()` can weight the columns later without another migration.
fn write_fts(
    tx: &Connection,
    domain: Domain,
    path: &str,
    original_name: Option<&str>,
    fields: &[FieldRow],
    entities: &[EntityRow],
) -> Result<(), AppError> {
    let (id, title): (i64, Option<String>) = tx
        .query_row(
            "SELECT profile_id, title FROM file_profiles WHERE domain = ?1 AND relative_path = ?2",
            params![domain.as_str(), path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    let name = NameProfile::build(path, original_name);
    let join = |parts: Vec<String>| {
        parts
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let by = |kind: &str, detail: Option<&str>| -> Vec<String> {
        fields
            .iter()
            .filter(|f| f.kind == kind && detail.is_none_or(|d| f.detail == d))
            .map(|f| folded(&f.value))
            .collect()
    };

    let name_column = join(vec![
        name.name_words.join(" "),
        name.original_words.join(" "),
    ]);
    let folder_column = name.folder_words.join(" ");
    let title_column = title.as_deref().map(folded).unwrap_or_default();
    let mut authors = by("metadata", Some("author"));
    authors.extend(
        fields
            .iter()
            .filter(|f| f.kind == "role_surface" && (f.detail == "author" || f.detail == "sender"))
            .map(|f| folded(&f.value)),
    );
    let mut subjects = by("subject_line", None);
    subjects.extend(by("metadata", Some("subject")));
    subjects.extend(by("metadata", Some("keywords")));
    let mut entity_parts: Vec<String> = Vec::new();
    for entity in entities {
        entity_parts.push(folded(&entity.surface));
        if let Some(entity_id) = entity.entity_id {
            if let Some(canonical) = tx
                .query_row(
                    "SELECT canonical_name FROM kb_entities WHERE entity_id = ?1",
                    [entity_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|_| AppError::IndexUnavailable)?
            {
                entity_parts.push(folded(&canonical));
            }
        }
    }
    let mut headers = by("header_words", None);
    headers.extend(by("sheet_name", None));
    headers.extend(by("column_header", None));
    // Words are folded before they are stored, so cell words are kept as the stored words.
    let cells: Vec<String> = fields
        .iter()
        .filter(|f| f.kind == "cell_words")
        .map(|f| f.value.clone())
        .collect();
    let mut identifiers: Vec<String> = Vec::new();
    for field in fields.iter().filter(|f| {
        matches!(
            f.kind.as_str(),
            "identifier" | "identifier_grouped" | "reference"
        )
    }) {
        identifiers.push(field.value.to_lowercase());
        identifiers.push(folded(&field.value));
    }

    tx.execute("DELETE FROM file_profile_fts WHERE rowid = ?1", [id])
        .map_err(|_| AppError::IndexUnavailable)?;
    tx.execute(
        "INSERT INTO file_profile_fts (rowid, domain, relative_path, name, folder, title, author, subject, entities, headers, cells, identifiers)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            id,
            domain.as_str(),
            path,
            name_column,
            folder_column,
            title_column,
            join(authors),
            join(subjects),
            join(entity_parts),
            join(headers),
            join(cells),
            join(identifiers),
        ],
    )
    .map_err(|_| AppError::IndexUnavailable)?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// What an Analyse calls
// ---------------------------------------------------------------------------------------------

/// The profile pass of the Documents Folder's Analyse. `None` when it could not even start (a damaged
/// vocabulary): the Analyse goes on without it.
pub fn run_documents(
    index: &mut IndexStore,
    folder: &Path,
    knowledge: Option<&crate::knowledge::ingest::KnowledgeContext>,
    locale: &str,
) -> Option<ProfileSummary> {
    run(index, folder, knowledge, locale, Domain::Documents)
}

/// The profile pass of the Data Folder's Analyse.
pub fn run_workbooks(
    index: &mut IndexStore,
    folder: &Path,
    knowledge: Option<&crate::knowledge::ingest::KnowledgeContext>,
    locale: &str,
) -> Option<ProfileSummary> {
    run(index, folder, knowledge, locale, Domain::Data)
}

fn run(
    index: &mut IndexStore,
    folder: &Path,
    knowledge: Option<&crate::knowledge::ingest::KnowledgeContext>,
    locale: &str,
    domain: Domain,
) -> Option<ProfileSummary> {
    let ids: Vec<&str> = knowledge
        .map(|context| context.profile_pack_ids())
        .unwrap_or_default();
    let vocabulary = ProfileVocabulary::load(&ids).ok()?;
    let packs = PackSet::load(knowledge.map_or(locale, |context| context.locale()), &ids).ok()?;
    let gazetteer = knowledge
        .filter(|context| context.mode.reads_files())
        .map(|context| context.gazetteer());
    let settings = ProfileSettings {
        vocabulary: &vocabulary,
        packs: &packs,
        gazetteer,
    };
    let never = || false;
    let started = std::time::Instant::now();
    let mut summary = match domain {
        Domain::Documents => refresh_documents(index, folder, &settings, &never),
        Domain::Data => refresh_workbooks(index, folder, &settings, &never),
    }
    .ok()?;
    summary.elapsed_ms = started.elapsed().as_millis() as u64;
    Some(summary)
}
