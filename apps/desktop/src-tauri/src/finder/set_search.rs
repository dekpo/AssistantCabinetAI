//! The chat's set sentence, answered by the finder (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 7): which of the
//! selected files talk about X. No model, no embedding, no gateway: the answer is read from the index and
//! the inventory, so it is the same with the AI machine stopped.
//!
//! The answer states that it is a **word search** (`terms`, `prefix`): a synonym or another form of the
//! word is not matched. The first [`LISTED`] files carry their excerpt, the others only their path, and the
//! total is always stated; the candidate limit of retrieval never truncates a listing.

use std::collections::HashMap;

use rusqlite::Connection;

use super::contracts::{
    DocumentListing, EntityFilter, FileHit, FinderQuery, FinderResult, FinderScope, InventoryFile,
    Locator, SectionListing, MAX_LIMIT, SECTION_KIND,
};
use super::engine::find;
use super::names::{NameResolver, NoNames};
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

/// Answer `set` over the selected files `scoped`, which are also the only ones searched. X stays a phrase.
pub fn answer(
    connection: &Connection,
    scoped: &[InventoryFile],
    set: &SetQuestion,
    identifiers: Vec<String>,
) -> Result<FolderAnswer, AppError> {
    answer_with_names(connection, scoped, set, identifiers, &NoNames)
}

/// `answer`, where X may be the name of exactly one entity: the answer is then the files linked to that
/// entity (the knowledge base's mentions and the profile's entity rows) together with the files that write
/// one of its surfaces as a phrase. Two entities, a possible match or none leave X a phrase.
pub fn answer_with_names(
    connection: &Connection,
    scoped: &[InventoryFile],
    set: &SetQuestion,
    identifiers: Vec<String>,
    names: &dyn NameResolver,
) -> Result<FolderAnswer, AppError> {
    answer_in(
        Domain::Documents,
        connection,
        scoped,
        set,
        identifiers,
        names,
    )
}

/// The set sentence with documents and workbooks both selected: one search per folder, each answer stating
/// its own folder. No model, no embedding: the same index, the same engine.
pub fn answer_both(
    connection: &Connection,
    documents: &[InventoryFile],
    workbooks: &[InventoryFile],
    set: &SetQuestion,
    identifiers: Vec<String>,
    names: &dyn NameResolver,
) -> Result<FolderAnswer, AppError> {
    let in_documents = answer_in(
        Domain::Documents,
        connection,
        documents,
        set,
        identifiers.clone(),
        names,
    )?;
    let in_workbooks = answer_in(Domain::Data, connection, workbooks, set, identifiers, names)?;
    Ok(FolderAnswer::DocumentSearchBoth {
        documents: Box::new(in_documents),
        workbooks: Box::new(in_workbooks),
    })
}

/// `answer_with_names` over the files of one folder: the documents, or the workbooks (their cell words,
/// headers and names are in the profile). The answer states which folder it searched.
pub fn answer_in(
    domain: Domain,
    connection: &Connection,
    scoped: &[InventoryFile],
    set: &SetQuestion,
    identifiers: Vec<String>,
    names: &dyn NameResolver,
) -> Result<FolderAnswer, AppError> {
    let identifier = !identifiers.is_empty();
    // "Which sections talk about X" is the same search, listed by section; the noun is no business type.
    let by_section = set.kind.as_deref() == Some(SECTION_KIND);
    let business_type = set.kind.clone().filter(|_| !by_section);
    let named = if identifier || set.quoted {
        None
    } else {
        names.resolve_one(&set.text)
    };
    let base = FinderQuery {
        whole_words: true,
        kinds: business_type.iter().cloned().collect(),
        limit: Some(MAX_LIMIT),
        ..FinderQuery::default()
    };
    let scope = FinderScope::Selection(
        scoped
            .iter()
            .map(|file| file.relative_path.clone())
            .collect(),
    );
    let run = |query: FinderQuery| find(connection, domain, scoped, &query, &scope);

    let mut result = run(FinderQuery {
        text: Some(set.text.clone()),
        identifiers,
        ..base.clone()
    })?;
    let mut entity = None;
    if let Some(named) = named {
        // The files linked to the entity, then the files that write another of its surfaces.
        let linked = run(FinderQuery {
            entities: vec![EntityFilter {
                entity_id: named.entity_id,
                role: None,
            }],
            ..base.clone()
        })?;
        let mut merged = Merged::new(result);
        merged.add(linked);
        for surface in &named.surfaces {
            merged.add(run(FinderQuery {
                text: Some(surface.clone()),
                ..base.clone()
            })?);
        }
        result = merged.finish();
        entity = Some(named.display);
    }

    let mut listed = Vec::new();
    let mut others = Vec::new();
    for (position, hit) in result.files.into_iter().enumerate() {
        if position < LISTED {
            let sections = sections_of(&hit.locators);
            listed.push(DocumentListing {
                relative_path: hit.relative_path,
                title: hit.card.title,
                business_type: hit.card.business_type,
                excerpt: hit.excerpt,
                page: hit.locator.and_then(|locator| locator.page),
                pages: {
                    let mut pages: Vec<i64> = hit
                        .locators
                        .iter()
                        .filter_map(|locator| locator.page)
                        .collect();
                    pages.sort_unstable();
                    pages.dedup();
                    pages
                },
                matches_total: hit.locators_total,
                sections,
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
        domain,
        query: set.text.clone(),
        terms,
        prefix,
        quoted: set.quoted,
        business_type,
        identifier,
        entity,
        by_section,
        total: result.total,
        listed,
        others,
        coverage: result.coverage,
    })
}

/// The sections where a file's words matched, in reading order, each with the pages of its places.
fn sections_of(locators: &[Locator]) -> Vec<SectionListing> {
    let mut sections: Vec<SectionListing> = Vec::new();
    for locator in locators {
        let Some(label) = locator.section_label.clone() else {
            continue;
        };
        match sections.iter_mut().find(|known| known.label == label) {
            Some(known) => {
                if let Some(page) = locator.page {
                    if !known.pages.contains(&page) {
                        known.pages.push(page);
                    }
                }
            }
            None => sections.push(SectionListing {
                label,
                pages: locator.page.into_iter().collect(),
            }),
        }
    }
    for section in &mut sections {
        section.pages.sort_unstable();
    }
    sections
}

/// The union of several runs of the finder over the same scope: a file once, the first run's order, the
/// passage of the first run that has one.
struct Merged {
    result: FinderResult,
    position: HashMap<String, usize>,
}

impl Merged {
    fn new(result: FinderResult) -> Self {
        let position = result
            .files
            .iter()
            .enumerate()
            .map(|(at, hit)| (hit.relative_path.clone(), at))
            .collect();
        Self { result, position }
    }

    fn add(&mut self, other: FinderResult) {
        for hit in other.files {
            self.push(hit);
        }
    }

    fn push(&mut self, hit: FileHit) {
        match self.position.get(&hit.relative_path) {
            Some(&at) => {
                let held = &mut self.result.files[at];
                if held.excerpt.is_none() {
                    held.excerpt = hit.excerpt;
                    held.locator = hit.locator;
                }
                for reason in hit.reasons {
                    if !held.reasons.contains(&reason) {
                        held.reasons.push(reason);
                    }
                }
            }
            None => {
                self.position
                    .insert(hit.relative_path.clone(), self.result.files.len());
                self.result.files.push(hit);
            }
        }
    }

    fn finish(mut self) -> FinderResult {
        self.result.total = self.result.files.len();
        self.result
    }
}
