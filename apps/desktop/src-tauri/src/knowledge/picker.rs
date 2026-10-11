//! The names the composer's `@` picker offers (lot 7).
//!
//! The user types `@` and the first letters of a name; this lists the names of **the current
//! selection** that begin with them, each with the number of selected files that hold it. Nothing
//! here calls a model, reads a file or writes a row, and everything it returns is for the screen:
//! ids, counts, paths and the names themselves (user data, never logged).
//!
//! One surface is allowed to mention a name that is not in the selection, and it is labelled as such
//! (master invariant I7): a name found only in analysed files the user did not tick comes back in
//! `outside_selection`, as paths, so the interface can offer "add these files to my selection". The
//! click that does it is the user's own.

use rusqlite::Connection;
use serde::Serialize;

use super::query::SelectedFile;
use super::store;
use super::{Domain, EntityStatus};
use crate::error::AppError;

/// The names of the outside-selection group shown at once. A hint to a possible next step, not a
/// second list.
pub const MAX_OUTSIDE_NAMES: usize = 3;

/// The most paths listed for one outside name, per side.
pub const MAX_OUTSIDE_PATHS: usize = 50;

/// Names listed when the caller does not say.
pub const PICKER_LIMIT: usize = 8;

/// The longest list a caller may ask for.
pub const MAX_PICKER_LIMIT: usize = 20;

/// Files, documents and workbooks apart (the same relative path can exist in both folders).
#[derive(Debug, Clone, Copy)]
pub struct PickerFiles<'a> {
    pub documents: &'a [SelectedFile],
    pub data: &'a [SelectedFile],
}

impl PickerFiles<'_> {
    fn is_empty(&self) -> bool {
        self.documents.is_empty() && self.data.is_empty()
    }
}

/// A name of the selection, with how many selected files hold it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityChoice {
    pub entity_id: i64,
    pub display: String,
    pub type_id: String,
    pub documents: usize,
    pub workbooks: usize,
}

/// A name found only in analysed files that are not selected: paths, never a sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutsideSelection {
    pub entity_id: i64,
    pub display: String,
    pub type_id: String,
    pub documents: Vec<String>,
    pub workbooks: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitySearch {
    pub entities: Vec<EntityChoice>,
    pub outside_selection: Vec<OutsideSelection>,
}

fn pairs(files: &[SelectedFile]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|file| (file.relative_path.clone(), file.content_id.clone()))
        .collect()
}

fn source_ids(connection: &Connection, files: PickerFiles<'_>) -> Result<Vec<i64>, AppError> {
    let mut ids =
        store::source_ids_for_scope(connection, Domain::Documents, &pairs(files.documents))?;
    ids.extend(store::source_ids_for_scope(
        connection,
        Domain::Data,
        &pairs(files.data),
    )?);
    Ok(ids)
}

/// The names that begin with `prefix`, among the `selected` files, and the names that are only in
/// `analysed` files outside the selection.
///
/// - An empty selection finds nothing, whatever the base holds: the picker needs something to pick
///   among, and an empty selection must not become a way to read the base's names.
/// - A candidate (a capitalised phrase the base has not confirmed) is never offered (defect KBD-11).
/// - A name with no file in the selection is not in `entities`; if it has files among the analysed
///   ones, it is in `outside_selection` (at most `MAX_OUTSIDE_NAMES` of them).
pub fn search_entities(
    connection: &Connection,
    prefix: &str,
    selected: PickerFiles<'_>,
    analysed: PickerFiles<'_>,
    limit: usize,
) -> Result<EntitySearch, AppError> {
    let limit = limit.clamp(1, MAX_PICKER_LIMIT);
    if prefix.trim().is_empty() || selected.is_empty() {
        return Ok(EntitySearch::default());
    }
    let selected_ids = source_ids(connection, selected)?;
    if selected_ids.is_empty() {
        // Nothing the selection holds is known to the base, so there is no name to offer from it.
        return Ok(EntitySearch::default());
    }

    // More than asked for, because candidates and names outside the selection are filtered out.
    let hits = store::prefix_search(connection, prefix.trim(), limit.saturating_mul(4))?;
    let mut live = Vec::new();
    for hit in hits {
        let Some(entity) = store::effective_entity(connection, hit.entity_id)? else {
            continue;
        };
        if entity.status != EntityStatus::Active {
            continue;
        }
        live.push(entity);
    }
    let ids: Vec<i64> = live.iter().map(|entity| entity.entity_id).collect();
    let inside = store::files_for_entities(connection, &ids, &selected_ids)?;

    let mut result = EntitySearch::default();
    let mut outside_candidates = Vec::new();
    for entity in &live {
        let sources = inside.per_entity.get(&entity.entity_id);
        let documents = sources.map_or(0, |set| {
            set.iter().filter(|s| s.domain == Domain::Documents).count()
        });
        let workbooks = sources.map_or(0, |set| {
            set.iter().filter(|s| s.domain == Domain::Data).count()
        });
        if documents + workbooks > 0 {
            if result.entities.len() < limit {
                result.entities.push(EntityChoice {
                    entity_id: entity.entity_id,
                    display: entity.canonical_name.clone(),
                    type_id: entity.type_id.clone(),
                    documents,
                    workbooks,
                });
            }
        } else {
            outside_candidates.push(entity);
        }
    }

    // Names with no file in the selection but files among the analysed ones: the offer to add them.
    if !outside_candidates.is_empty() {
        let analysed_ids = source_ids(connection, analysed)?;
        let outside_ids: Vec<i64> = outside_candidates
            .iter()
            .map(|entity| entity.entity_id)
            .collect();
        // Such a name has no mention in a selected source, so every file found here is unselected.
        let elsewhere = store::files_for_entities(connection, &outside_ids, &analysed_ids)?;
        for entity in outside_candidates {
            let Some(sources) = elsewhere.per_entity.get(&entity.entity_id) else {
                continue;
            };
            let mut documents = Vec::new();
            let mut workbooks = Vec::new();
            for source in sources {
                match source.domain {
                    Domain::Documents => documents.push(source.relative_path.clone()),
                    Domain::Data => workbooks.push(source.relative_path.clone()),
                }
            }
            documents.truncate(MAX_OUTSIDE_PATHS);
            workbooks.truncate(MAX_OUTSIDE_PATHS);
            if documents.is_empty() && workbooks.is_empty() {
                continue;
            }
            result.outside_selection.push(OutsideSelection {
                entity_id: entity.entity_id,
                display: entity.canonical_name.clone(),
                type_id: entity.type_id.clone(),
                documents,
                workbooks,
            });
            if result.outside_selection.len() == MAX_OUTSIDE_NAMES {
                break;
            }
        }
    }
    Ok(result)
}
