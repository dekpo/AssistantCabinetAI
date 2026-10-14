//! The chat's set sentence, answered by the finder (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 7): which of the
//! selected files talk about X. No model, no embedding, no gateway: the answer is read from the index and
//! the inventory, so it is the same with the AI machine stopped.
//!
//! The answer states that it is a **word search** (`terms`, `prefix`): a synonym or another form of the
//! word is not matched. The first [`LISTED`] files carry their excerpt, the others only their path, and the
//! total is always stated; the candidate limit of retrieval never truncates a listing.

use rusqlite::Connection;

use super::contracts::{DocumentListing, FinderQuery, FinderScope, InventoryFile, MAX_LIMIT};
use super::engine::find;
use super::question_frame::SetQuestion;
use crate::error::AppError;
use crate::file_profile::build::Domain;
use crate::file_profile::identifiers::identifiers_in;
use crate::file_profile::vocabulary::ProfileVocabulary;
use crate::folder_questions::FolderAnswer;
use crate::knowledge::packs::PackSet;

/// The files shown with their excerpt; the others are listed as paths.
pub const LISTED: usize = 50;

/// The compact forms of the identifiers of the active packs found in what the user typed: `756.3052.5778.43`
/// and `7563052577843` give the same one, which is what the profile stores for the number wherever the
/// document wrote it.
pub fn identifier_forms(
    text: &str,
    packs: &PackSet,
    vocabulary: &ProfileVocabulary,
) -> Vec<String> {
    let mut forms: Vec<String> = Vec::new();
    for found in identifiers_in(text, packs, vocabulary) {
        if !forms.contains(&found.compact) {
            forms.push(found.compact);
        }
    }
    forms
}

/// Answer `set` over the selected files `scoped`, which are also the only ones searched.
pub fn answer(
    connection: &Connection,
    scoped: &[InventoryFile],
    set: &SetQuestion,
    identifiers: Vec<String>,
) -> Result<FolderAnswer, AppError> {
    let identifier = !identifiers.is_empty();
    let query = FinderQuery {
        text: Some(set.text.clone()),
        whole_words: true,
        identifiers,
        kinds: set.kind.iter().cloned().collect(),
        limit: Some(MAX_LIMIT),
        ..FinderQuery::default()
    };
    let scope = FinderScope::Selection(
        scoped
            .iter()
            .map(|file| file.relative_path.clone())
            .collect(),
    );
    let result = find(connection, Domain::Documents, scoped, &query, &scope)?;

    let mut listed = Vec::new();
    let mut others = Vec::new();
    for (position, hit) in result.files.into_iter().enumerate() {
        if position < LISTED {
            listed.push(DocumentListing {
                relative_path: hit.relative_path,
                title: hit.card.title,
                business_type: hit.card.business_type,
                excerpt: hit.excerpt,
                page: hit.locator.and_then(|locator| locator.page),
            });
        } else {
            others.push(hit.relative_path);
        }
    }
    let (terms, prefix) = result
        .text_search
        .map(|search| (search.terms, search.last_is_prefix))
        .unwrap_or_default();
    Ok(FolderAnswer::DocumentSearch {
        query: set.text.clone(),
        terms,
        prefix,
        quoted: set.quoted,
        business_type: set.kind.clone(),
        identifier,
        total: result.total,
        listed,
        others,
        coverage: result.coverage,
    })
}
