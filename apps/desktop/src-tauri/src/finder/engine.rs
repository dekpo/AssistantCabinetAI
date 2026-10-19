//! The finder: files found with no model, no embedding and no file read (`docs/LEXICAL-RETRIEVAL-DESIGN.md`
//! 6). Four sources answer a typed word, each saying why:
//!
//! 1. the **name** and the folder, through the one name function the Analyse also uses, over the inventory,
//!    so a file never analysed (or a scan nothing could be read from) is still found;
//! 2. the **profile's full-text row** (title, author, subject, entities, header words, cell words,
//!    identifiers), one query per column to say which column answered;
//! 3. the **passages** (`chunks_fts`), documents only, as a phrase;
//! 4. nothing else: a word is a word, a synonym is not matched, and the result says so.
//!
//! Filters narrow the result together: entities (the knowledge base and the profile for an entity, the
//! profile alone for a role, and a link with no entity never filters), business type, date, folder, format
//! and state. The scope is a hard limit: a selection is never searched beyond. Facets are counted over the
//! whole result and the coverage over the whole scope, so a count always carries its denominator.
//!
//! This module only reads. It never writes to the profile, to `kb_*` or to the spreadsheet tables.

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{params, Connection};

use super::contracts::{
    CardEntity, Coverage, DateRange, EntityFilter, Facet, Facets, FileCard, FileHit, FinderQuery,
    FinderResult, FinderScope, InventoryFile, Locator, Reason, TextSearch, DEFAULT_LIMIT,
    MAX_LIMIT, MAX_LOCATORS,
};
use crate::discovery::is_supported;
use crate::error::AppError;
use crate::file_profile::build::Domain;
use crate::file_profile::name::{split_date_prefix, words_of, NameProfile, MIN_PREFIX_LETTERS};
use crate::file_record::FileKind;

/// The people and organisations shown on a file's card.
const CARD_ENTITIES: usize = 5;
/// The entities the entity facet lists.
const FACET_ENTITIES: usize = 10;
/// The words of an excerpt.
const EXCERPT_WORDS: usize = 28;
/// How many words before the match an excerpt starts.
const EXCERPT_LEAD: usize = 8;

/// The page cache of a finder call, in KiB.
const FINDER_CACHE_KIB: i64 = 65_536;

/// The columns of the profile's full-text row, in the order their reasons are reported.
const PROFILE_COLUMNS: [&str; 9] = [
    "name",
    "folder",
    "title",
    "author",
    "subject",
    "entities",
    "headers",
    "cells",
    "identifiers",
];

/// An identifier met in its stored forms is as sure as a word of the name.
const IDENTIFIER_WEIGHT: u32 = 95;

/// A date typed that a file carries (in its profile or at the start of its name) is as sure as an
/// identifier.
const DATE_WEIGHT: u32 = 95;

/// What a word found in the name counts for against the same word found elsewhere: the name is chosen by
/// a person to describe the file.
fn weight(detail: &str) -> u32 {
    match detail {
        "name" => 100,
        "identifiers" => 90,
        "title" => 80,
        "entities" => 70,
        "author" | "subject" => 60,
        "profile" => 50,
        "headers" | "cells" => 40,
        "folder" => 30,
        "passage" => 20,
        _ => 10,
    }
}

fn unavailable<E>(_: E) -> AppError {
    AppError::IndexUnavailable
}

struct ProfileRow {
    format: String,
    state: String,
    business_type: Option<String>,
    title: Option<String>,
    document_date: Option<String>,
}

/// What one file matched on, before the page is cut.
#[derive(Default)]
struct Found {
    reasons: Vec<Reason>,
    /// The first passage that matched: `(chunk id, page, section)`.
    passage: Option<(String, i64, i64)>,
    /// The first passages that matched, in reading order, at most `MAX_LOCATORS`.
    passages: Vec<(String, i64, i64)>,
    /// How many passages matched in all.
    passage_total: usize,
}

impl Found {
    fn add(&mut self, kind: &str, detail: &str) {
        let reason = Reason {
            kind: kind.to_string(),
            detail: Some(detail.to_string()),
        };
        if !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
    }

    fn score(&self) -> u32 {
        self.reasons
            .iter()
            .filter_map(|reason| match reason.kind.as_str() {
                "word" => Some(weight(reason.detail.as_deref().unwrap_or(""))),
                "identifier" => Some(IDENTIFIER_WEIGHT),
                "date" => Some(DATE_WEIGHT),
                _ => None,
            })
            .max()
            .unwrap_or(0)
    }
}

/// Find the files of one folder that answer `query`, inside `scope`. `inventory` is what the folder holds
/// right now; the index says what is known about each file.
pub fn find(
    connection: &Connection,
    domain: Domain,
    inventory: &[InventoryFile],
    query: &FinderQuery,
    scope: &FinderScope,
) -> Result<FinderResult, AppError> {
    // A search reads every profile row of the folder: the default cache (about 2 MB) is smaller than the
    // profile tables of a big folder and would read the same pages again and again. The cache is held
    // by this connection only and grows only as far as the pages read.
    let _ = connection.pragma_update(None, "cache_size", -FINDER_CACHE_KIB);
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let profiles = load_profiles(connection, domain)?;

    let selection: Option<HashSet<&str>> = match scope {
        FinderScope::WholeFolder => None,
        FinderScope::Selection(paths) => Some(paths.iter().map(String::as_str).collect()),
    };
    let in_scope: Vec<&InventoryFile> = inventory
        .iter()
        .filter(|file| {
            selection
                .as_ref()
                .is_none_or(|paths| paths.contains(file.relative_path.as_str()))
        })
        .collect();
    let coverage = coverage_of(connection, domain, &in_scope, &profiles)?;

    // The words typed: found by name, by profile and by passage.
    let typed = query
        .text
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let terms: Vec<String> = typed.map(words_of).unwrap_or_default();
    let last_is_prefix = !query.whole_words
        && terms
            .last()
            .is_some_and(|last| last.chars().count() >= MIN_PREFIX_LETTERS);
    let text_search = typed.map(|_| TextSearch {
        terms: terms.clone(),
        last_is_prefix,
    });
    let identifiers: Vec<&str> = query
        .identifiers
        .iter()
        .map(String::as_str)
        .filter(|form| !form.is_empty() && form.chars().all(char::is_alphanumeric))
        .collect();
    let mut found: Option<HashMap<String, Found>> = None;
    if typed.is_some() || !identifiers.is_empty() || query.typed_date.is_some() {
        let mut matches: HashMap<String, Found> = HashMap::new();
        if let (Some(typed), false) = (typed, terms.is_empty()) {
            by_name(
                connection,
                domain,
                typed,
                &terms,
                !query.whole_words,
                &in_scope,
                &mut matches,
            )?;
            by_profile(connection, domain, &terms, last_is_prefix, &mut matches)?;
            // A date typed is looked for in the names and the profile: the passages of every document
            // that mentions the year would bury the files dated so.
            if domain == Domain::Documents && query.typed_date.is_none() {
                by_passage(
                    connection,
                    &terms,
                    last_is_prefix,
                    selection.as_ref().map(|_| &in_scope[..]),
                    &mut matches,
                )?;
            }
        }
        by_identifier(connection, domain, &identifiers, &mut matches)?;
        if let Some(range) = &query.typed_date {
            by_date(&in_scope, &profiles, range, &mut matches);
        }
        found = Some(matches);
    }

    // The entities: every filter must hold.
    let mut entity_reasons: Vec<HashMap<String, Vec<String>>> = Vec::new();
    for filter in &query.entities {
        entity_reasons.push(files_of_entity(connection, domain, filter)?);
    }
    let date_ranges = if domain == Domain::Data && query.date.is_some() {
        load_date_ranges(connection)?
    } else {
        HashMap::new()
    };

    let mut matched: Vec<(&InventoryFile, Found)> = Vec::new();
    for file in &in_scope {
        let path = file.relative_path.as_str();
        let mut hit = match &mut found {
            Some(map) => match map.remove(path) {
                Some(hit) => hit,
                None => continue,
            },
            None => Found::default(),
        };
        if !entity_reasons.iter().all(|files| files.contains_key(path)) {
            continue;
        }
        let profile = profiles.get(path);
        if !passes_filters(
            query,
            file,
            state_code(domain, file, profile),
            profile,
            date_ranges.get(path),
        ) {
            continue;
        }
        for files in &entity_reasons {
            for detail in &files[path] {
                hit.add("entity", detail);
            }
        }
        matched.push((file, hit));
    }
    if found.is_some() {
        matched.sort_by(|a, b| {
            b.1.score()
                .cmp(&a.1.score())
                .then_with(|| a.0.relative_path.cmp(&b.0.relative_path))
        });
    }
    let facets = facets_of(connection, domain, &matched, &profiles)?;
    let total = matched.len();
    let page: Vec<&(&InventoryFile, Found)> =
        matched.iter().skip(query.offset).take(limit).collect();
    let files = build_hits(
        connection,
        domain,
        &page,
        &profiles,
        &terms,
        last_is_prefix,
        query.paths_only,
    )?;
    Ok(FinderResult {
        domain,
        total,
        offset: query.offset,
        limit,
        files,
        facets,
        coverage,
        text_search,
    })
}

// ---------------------------------------------------------------------------------------------
// What the index knows
// ---------------------------------------------------------------------------------------------

fn load_profiles(
    connection: &Connection,
    domain: Domain,
) -> Result<HashMap<String, ProfileRow>, AppError> {
    let mut statement = connection
        .prepare(
            "SELECT relative_path, format, state, business_type, title, document_date
             FROM file_profiles WHERE domain = ?1",
        )
        .map_err(unavailable)?;
    let rows = statement
        .query_map([domain.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                ProfileRow {
                    format: row.get(1)?,
                    state: row.get(2)?,
                    business_type: row.get(3)?,
                    title: row.get(4)?,
                    document_date: row.get(5)?,
                },
            ))
        })
        .map_err(unavailable)?;
    rows.collect::<Result<_, _>>().map_err(unavailable)
}

fn load_original_names(
    connection: &Connection,
    domain: Domain,
) -> Result<HashMap<String, String>, AppError> {
    let mut statement = connection
        .prepare("SELECT relative_path, original_name FROM file_profile_original_names WHERE domain = ?1")
        .map_err(unavailable)?;
    let rows = statement
        .query_map([domain.as_str()], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(unavailable)?;
    rows.collect::<Result<_, _>>().map_err(unavailable)
}

/// The date ranges of workbooks' date columns: `(from, to)` per file.
fn load_date_ranges(
    connection: &Connection,
) -> Result<HashMap<String, Vec<(String, String)>>, AppError> {
    let mut statement = connection
        .prepare(
            "SELECT relative_path, value FROM file_profile_fields
             WHERE kind = 'date_range' AND domain = 'data'",
        )
        .map_err(unavailable)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(unavailable)?;
    let mut ranges: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for (path, value) in rows.flatten() {
        if let Some((from, to)) = value.split_once('/') {
            ranges
                .entry(path)
                .or_default()
                .push((from.to_string(), to.to_string()));
        }
    }
    Ok(ranges)
}

/// The state of a file for the finder: the profile's own, or why there is none.
fn state_code<'a>(
    domain: Domain,
    file: &InventoryFile,
    profile: Option<&'a ProfileRow>,
) -> &'a str {
    if let Some(profile) = profile {
        return &profile.state;
    }
    let readable = match domain {
        Domain::Documents => is_supported(&file.extension),
        Domain::Data => FileKind::from_extension(&file.extension) == FileKind::TabularCandidate,
    };
    if readable {
        "unanalysed"
    } else {
        "unsupported"
    }
}

fn coverage_of(
    connection: &Connection,
    domain: Domain,
    in_scope: &[&InventoryFile],
    profiles: &HashMap<String, ProfileRow>,
) -> Result<Coverage, AppError> {
    let mut coverage = Coverage::default();
    let mut analysed: HashSet<&str> = HashSet::new();
    for file in in_scope {
        match state_code(domain, file, profiles.get(&file.relative_path)) {
            "analysed" => {
                coverage.searched += 1;
                analysed.insert(file.relative_path.as_str());
            }
            "unanalysed" => coverage.unanalysed += 1,
            _ => coverage.unreadable += 1,
        }
    }
    if !analysed.is_empty() {
        let mut statement = connection
            .prepare(
                "SELECT DISTINCT relative_path FROM file_profile_fields
                 WHERE kind IN ('cells_truncated', 'identifiers_truncated') AND domain = ?1",
            )
            .map_err(unavailable)?;
        let rows = statement
            .query_map([domain.as_str()], |row| row.get::<_, String>(0))
            .map_err(unavailable)?;
        coverage.partial = rows
            .flatten()
            .filter(|path| analysed.contains(path.as_str()))
            .count();
    }
    Ok(coverage)
}

// ---------------------------------------------------------------------------------------------
// The words
// ---------------------------------------------------------------------------------------------

/// `"a" "b" "c"*`: every word quoted so nothing typed can carry full-text syntax; the last one a start
/// when it is long enough to be one.
fn word_expression(terms: &[String], last_is_prefix: bool) -> String {
    terms
        .iter()
        .enumerate()
        .map(|(position, term)| {
            if last_is_prefix && position + 1 == terms.len() {
                format!("\"{term}\"*")
            } else {
                format!("\"{term}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn by_name(
    connection: &Connection,
    domain: Domain,
    typed: &str,
    terms: &[String],
    last_may_be_start: bool,
    in_scope: &[&InventoryFile],
    matches: &mut HashMap<String, Found>,
) -> Result<(), AppError> {
    let originals = load_original_names(connection, domain)?;
    for file in in_scope {
        let original = originals.get(&file.relative_path).map(String::as_str);
        // A clean name is ASCII: when every typed word is not even a piece of it, nothing more to build.
        if original.is_none() && file.relative_path.is_ascii() {
            let lower = file.relative_path.to_ascii_lowercase();
            if !terms.iter().all(|term| lower.contains(term.as_str())) {
                continue;
            }
        }
        if NameProfile::build(&file.relative_path, original).matches_as(typed, last_may_be_start) {
            matches
                .entry(file.relative_path.clone())
                .or_default()
                .add("word", "name");
        }
    }
    Ok(())
}

fn by_profile(
    connection: &Connection,
    domain: Domain,
    terms: &[String],
    last_is_prefix: bool,
    matches: &mut HashMap<String, Found>,
) -> Result<(), AppError> {
    let paths_of = |expression: &str| -> Result<Vec<String>, AppError> {
        let mut statement = connection
            .prepare(
                "SELECT relative_path FROM file_profile_fts
                 WHERE file_profile_fts MATCH ?1 AND domain = ?2",
            )
            .map_err(unavailable)?;
        let rows = statement
            .query_map(params![expression, domain.as_str()], |row| {
                row.get::<_, String>(0)
            })
            .map_err(unavailable)?;
        rows.collect::<Result<_, _>>().map_err(unavailable)
    };
    let row_paths = paths_of(&word_expression(terms, last_is_prefix))?;
    if row_paths.is_empty() {
        return Ok(());
    }
    let mut explained: HashMap<String, Vec<&str>> = HashMap::new();
    for column in PROFILE_COLUMNS {
        let in_column = terms
            .iter()
            .enumerate()
            .map(|(position, term)| {
                if last_is_prefix && position + 1 == terms.len() {
                    format!("{column} : \"{term}\"*")
                } else {
                    format!("{column} : \"{term}\"")
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        for path in paths_of(&in_column)? {
            explained.entry(path).or_default().push(column);
        }
    }
    for path in row_paths {
        let entry = matches.entry(path.clone()).or_default();
        match explained.get(&path) {
            Some(columns) => {
                for column in columns {
                    entry.add("word", column);
                }
            }
            None => entry.add("word", "profile"),
        }
    }
    Ok(())
}

/// The files that store one of these identifiers, in any spelling the user typed it: the profile keeps the
/// compact form of every identifier of the active packs.
fn by_identifier(
    connection: &Connection,
    domain: Domain,
    forms: &[&str],
    matches: &mut HashMap<String, Found>,
) -> Result<(), AppError> {
    for form in forms {
        let mut statement = connection
            .prepare(
                "SELECT relative_path FROM file_profile_fts
                 WHERE file_profile_fts MATCH ?1 AND domain = ?2",
            )
            .map_err(unavailable)?;
        let expression = format!("identifiers : \"{}\"", form.to_lowercase());
        let rows = statement
            .query_map(params![expression, domain.as_str()], |row| {
                row.get::<_, String>(0)
            })
            .map_err(unavailable)?;
        for path in rows.flatten() {
            matches
                .entry(path)
                .or_default()
                .add("identifier", "identifiers");
        }
    }
    Ok(())
}

/// The files dated as a date typed in the field: by their profile's date, or by the clean date their
/// name starts with, which a file never analysed has too.
fn by_date(
    in_scope: &[&InventoryFile],
    profiles: &HashMap<String, ProfileRow>,
    range: &DateRange,
    matches: &mut HashMap<String, Found>,
) {
    for file in in_scope {
        let path = file.relative_path.as_str();
        let profile_date = profiles
            .get(path)
            .and_then(|row| row.document_date.as_deref())
            .is_some_and(|date| date_within(date, range));
        let name_dated = name_date(path).is_some_and(|date| date_within(&date, range));
        if profile_date {
            matches
                .entry(path.to_string())
                .or_default()
                .add("date", "profile");
        }
        if name_dated {
            matches
                .entry(path.to_string())
                .or_default()
                .add("date", "name");
        }
    }
}

/// The date at the start of a file's name (`2026-02-06_commandes.csv`), as `YYYY-MM-DD`.
fn name_date(relative_path: &str) -> Option<String> {
    let name = relative_path.rsplit('/').next().unwrap_or(relative_path);
    let stem = match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    };
    split_date_prefix(stem).0
}

fn by_passage(
    connection: &Connection,
    terms: &[String],
    last_is_prefix: bool,
    restrict_to: Option<&[&InventoryFile]>,
    matches: &mut HashMap<String, Found>,
) -> Result<(), AppError> {
    let phrase = format!(
        "\"{}\"{}",
        terms.join(" "),
        if last_is_prefix { "*" } else { "" }
    );
    let allowed: Option<HashSet<&str>> = restrict_to.map(|files| {
        files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect()
    });
    let mut statement = connection
        .prepare(
            "SELECT chunks.relative_path, chunks.chunk_id, chunks.page_number, chunks.section
             FROM chunks_fts JOIN chunks ON chunks.chunk_id = chunks_fts.chunk_id
             WHERE chunks_fts MATCH ?1
             ORDER BY chunks.relative_path, chunks.page_number, chunks.section",
        )
        .map_err(unavailable)?;
    let rows = statement
        .query_map([phrase], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(unavailable)?;
    for (path, chunk_id, page, section) in rows.flatten() {
        if allowed
            .as_ref()
            .is_some_and(|paths| !paths.contains(path.as_str()))
        {
            continue;
        }
        let entry = matches.entry(path).or_default();
        entry.add("word", "passage");
        entry.passage_total += 1;
        if entry.passages.len() < MAX_LOCATORS {
            entry.passages.push((chunk_id.clone(), page, section));
        }
        let earlier = entry
            .passage
            .as_ref()
            .is_some_and(|(_, known_page, known_section)| {
                (*known_page, *known_section) <= (page, section)
            });
        if !earlier {
            entry.passage = Some((chunk_id, page, section));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------------------------

/// The files linked to one entity, with the role or `mention` that links them. A merged entity answers to
/// the one it was merged into. With a role, only the profile is read.
fn files_of_entity(
    connection: &Connection,
    domain: Domain,
    filter: &EntityFilter,
) -> Result<HashMap<String, Vec<String>>, AppError> {
    let ids = {
        let mut statement = connection
            .prepare("SELECT entity_id FROM kb_entities WHERE merged_into = ?1")
            .map_err(unavailable)?;
        let merged = statement
            .query_map([filter.entity_id], |row| row.get::<_, i64>(0))
            .map_err(unavailable)?;
        let mut ids: Vec<i64> = merged.flatten().collect();
        ids.push(filter.entity_id);
        serde_json::to_string(&ids).map_err(|_| AppError::Internal)?
    };
    let mut files: HashMap<String, Vec<String>> = HashMap::new();
    let mut note = |path: String, detail: String| {
        let details = files.entry(path).or_default();
        if !details.contains(&detail) {
            details.push(detail);
        }
    };

    let profile_sql = if filter.role.is_some() {
        "SELECT relative_path, role FROM file_profile_entities
         WHERE domain = ?1 AND role = ?3 AND entity_id IN (SELECT value FROM json_each(?2))"
    } else {
        "SELECT relative_path, role FROM file_profile_entities
         WHERE domain = ?1 AND ?3 IS NULL AND entity_id IN (SELECT value FROM json_each(?2))"
    };
    let mut statement = connection.prepare(profile_sql).map_err(unavailable)?;
    let rows = statement
        .query_map(params![domain.as_str(), ids, filter.role], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(unavailable)?;
    for (path, role) in rows.flatten() {
        note(path, role);
    }

    if filter.role.is_none() {
        let mut statement = connection
            .prepare(
                "SELECT sources.relative_path, COALESCE(mentions.role, 'mention')
                 FROM kb_mentions AS mentions
                 JOIN kb_sources AS sources ON sources.source_id = mentions.source_id
                 WHERE sources.domain = ?1
                   AND mentions.entity_id IN (SELECT value FROM json_each(?2))",
            )
            .map_err(unavailable)?;
        let rows = statement
            .query_map(params![domain.as_str(), ids], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(unavailable)?;
        for (path, role) in rows.flatten() {
            note(path, role);
        }
    }
    Ok(files)
}

// ---------------------------------------------------------------------------------------------
// Filters
// ---------------------------------------------------------------------------------------------

fn passes_filters(
    query: &FinderQuery,
    file: &InventoryFile,
    state: &str,
    profile: Option<&ProfileRow>,
    ranges: Option<&Vec<(String, String)>>,
) -> bool {
    if !query.formats.is_empty() && !query.formats.contains(&file.extension) {
        return false;
    }
    if !query.states.is_empty() && !query.states.iter().any(|s| s == state) {
        return false;
    }
    if !query.kinds.is_empty() {
        let kind = profile.and_then(|row| row.business_type.as_deref());
        if !kind.is_some_and(|kind| query.kinds.iter().any(|k| k == kind)) {
            return false;
        }
    }
    if !query.folders.is_empty()
        && !query.folders.iter().any(|folder| {
            let folder = folder.trim_end_matches('/');
            file.relative_path
                .strip_prefix(folder)
                .is_some_and(|rest| rest.starts_with('/'))
        })
    {
        return false;
    }
    if let Some(range) = &query.date {
        // A file never analysed has no profile date: the date its name starts with stands for it.
        let dated = profile
            .and_then(|row| row.document_date.as_deref())
            .is_some_and(|date| date_within(date, range))
            || name_date(&file.relative_path).is_some_and(|date| date_within(&date, range));
        let spanned = ranges.is_some_and(|spans| {
            spans
                .iter()
                .any(|(from, to)| span_overlaps(from, to, range))
        });
        if !dated && !spanned {
            return false;
        }
    }
    true
}

/// `date` (`YYYY-MM-DD`) is inside `range`; an end written as a month or a year covers all of it.
fn date_within(date: &str, range: &DateRange) -> bool {
    let after = range.from.as_deref().is_none_or(|from| date >= from);
    let before = range
        .to
        .as_deref()
        .is_none_or(|to| date.get(..to.len()).map_or(date <= to, |head| head <= to));
    after && before
}

/// A span of dates `from..=to` shares a day with `range`.
fn span_overlaps(from: &str, to: &str, range: &DateRange) -> bool {
    let ends_after_start = range.from.as_deref().is_none_or(|start| to >= start);
    let starts_before_end = range.to.as_deref().is_none_or(|end| {
        from.get(..end.len())
            .map_or(from <= end, |head| head <= end)
    });
    ends_after_start && starts_before_end
}

// ---------------------------------------------------------------------------------------------
// Facets and the page
// ---------------------------------------------------------------------------------------------

fn counted(counts: BTreeMap<String, usize>, newest_first: bool) -> Vec<Facet> {
    let mut facets: Vec<Facet> = counts
        .into_iter()
        .map(|(value, count)| Facet {
            value,
            count,
            label: None,
        })
        .collect();
    if newest_first {
        facets.sort_by(|a, b| b.value.cmp(&a.value));
    } else {
        facets.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.value.cmp(&b.value)));
    }
    facets
}

fn facets_of(
    connection: &Connection,
    domain: Domain,
    matched: &[(&InventoryFile, Found)],
    profiles: &HashMap<String, ProfileRow>,
) -> Result<Facets, AppError> {
    let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_year: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_folder: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_format: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_state: BTreeMap<String, usize> = BTreeMap::new();
    for (file, _) in matched {
        let profile = profiles.get(&file.relative_path);
        if let Some(kind) = profile.and_then(|row| row.business_type.as_ref()) {
            *by_type.entry(kind.clone()).or_default() += 1;
        }
        if let Some(year) = profile
            .and_then(|row| row.document_date.as_deref())
            .and_then(|date| date.get(..4))
        {
            *by_year.entry(year.to_string()).or_default() += 1;
        }
        if let Some((folder, _)) = file.relative_path.split_once('/') {
            *by_folder.entry(folder.to_string()).or_default() += 1;
        }
        if !file.extension.is_empty() {
            *by_format.entry(file.extension.clone()).or_default() += 1;
        }
        *by_state
            .entry(state_code(domain, file, profile).to_string())
            .or_default() += 1;
    }
    Ok(Facets {
        by_type: counted(by_type, false),
        by_year: counted(by_year, true),
        by_entity: entity_facet(connection, domain, matched)?,
        by_folder: counted(by_folder, false),
        by_format: counted(by_format, false),
        by_state: counted(by_state, false),
    })
}

/// The entities of the result, most files first. Counts files, from the profile and the knowledge base
/// together, once per file and entity; an entity the base has deleted or merged is left out.
fn entity_facet(
    connection: &Connection,
    domain: Domain,
    matched: &[(&InventoryFile, Found)],
) -> Result<Vec<Facet>, AppError> {
    if matched.is_empty() {
        return Ok(Vec::new());
    }
    let wanted: HashMap<&str, usize> = matched
        .iter()
        .enumerate()
        .map(|(position, (file, _))| (file.relative_path.as_str(), position))
        .collect();
    // One count per file and entity, whichever table says it.
    let mut seen: HashSet<(i64, usize)> = HashSet::new();
    for sql in [
        "SELECT entity_id, relative_path FROM file_profile_entities
         WHERE domain = ?1 AND entity_id IS NOT NULL",
        "SELECT mentions.entity_id, sources.relative_path
         FROM kb_mentions AS mentions
         JOIN kb_sources AS sources ON sources.source_id = mentions.source_id
         WHERE sources.domain = ?1",
    ] {
        let mut statement = connection.prepare(sql).map_err(unavailable)?;
        let mut rows = statement.query([domain.as_str()]).map_err(unavailable)?;
        while let Some(row) = rows.next().map_err(unavailable)? {
            let Ok(entity_id) = row.get::<_, i64>(0) else {
                continue;
            };
            let Some(path) = row.get_ref(1).ok().and_then(|value| value.as_str().ok()) else {
                continue;
            };
            if let Some(position) = wanted.get(path) {
                seen.insert((entity_id, *position));
            }
        }
    }
    let mut counts: HashMap<i64, usize> = HashMap::new();
    for (entity_id, _) in seen {
        *counts.entry(entity_id).or_default() += 1;
    }
    let mut ranked: Vec<(i64, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut facets = Vec::new();
    for (entity_id, count) in ranked {
        let name: Option<String> = connection
            .query_row(
                "SELECT canonical_name FROM kb_entities WHERE entity_id = ?1 AND status = 'active'",
                [entity_id],
                |row| row.get(0),
            )
            .ok();
        if let Some(name) = name {
            facets.push(Facet {
                value: entity_id.to_string(),
                count,
                label: Some(name),
            });
        }
        if facets.len() >= FACET_ENTITIES {
            break;
        }
    }
    Ok(facets)
}

/// The title of the section a passage starts in, by one lookup on the stored structure.
fn section_label_of(connection: &Connection, chunk_id: &str) -> Option<String> {
    connection
        .query_row(
            "SELECT s.title FROM chunk_sections c
             JOIN doc_sections s ON s.relative_path = c.relative_path AND s.position = c.position
             WHERE c.chunk_id = ?1",
            [chunk_id],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .filter(|title| !title.is_empty())
}

fn build_hits(
    connection: &Connection,
    domain: Domain,
    page: &[&(&InventoryFile, Found)],
    profiles: &HashMap<String, ProfileRow>,
    terms: &[String],
    last_is_prefix: bool,
    paths_only: bool,
) -> Result<Vec<FileHit>, AppError> {
    let mut entities = if paths_only {
        HashMap::new()
    } else {
        card_entities(connection, domain, page)?
    };
    let mut hits = Vec::with_capacity(page.len());
    for (file, found) in page.iter().map(|pair| (&pair.0, &pair.1)) {
        let profile = profiles.get(&file.relative_path);
        let (excerpt, locator) = match &found.passage {
            _ if paths_only => (None, None),
            Some((chunk_id, page_number, section)) => (
                excerpt_of(connection, chunk_id, terms, last_is_prefix),
                Some(Locator {
                    page: Some(*page_number),
                    ordinal: Some(*section),
                    section_label: section_label_of(connection, chunk_id),
                }),
            ),
            None => (None, None),
        };
        hits.push(FileHit {
            domain,
            relative_path: file.relative_path.clone(),
            card: FileCard {
                title: profile.and_then(|row| row.title.clone()),
                business_type: profile.and_then(|row| row.business_type.clone()),
                document_date: profile.and_then(|row| row.document_date.clone()),
                format: profile.map_or_else(|| file.extension.clone(), |row| row.format.clone()),
                state: state_code(domain, file, profile).to_string(),
                entities: entities.remove(&file.relative_path).unwrap_or_default(),
            },
            reasons: found.reasons.clone(),
            excerpt,
            locator,
            locators: if paths_only {
                Vec::new()
            } else {
                found
                    .passages
                    .iter()
                    .map(|(chunk_id, page_number, ordinal)| Locator {
                        page: Some(*page_number),
                        ordinal: Some(*ordinal),
                        section_label: section_label_of(connection, chunk_id),
                    })
                    .collect()
            },
            locators_total: found.passage_total,
        });
    }
    Ok(hits)
}

/// The people and organisations on the cards of one page, best evidence first.
fn card_entities(
    connection: &Connection,
    domain: Domain,
    page: &[&(&InventoryFile, Found)],
) -> Result<HashMap<String, Vec<CardEntity>>, AppError> {
    let mut cards: HashMap<String, Vec<CardEntity>> = HashMap::new();
    if page.is_empty() {
        return Ok(cards);
    }
    let paths: Vec<&str> = page
        .iter()
        .map(|pair| pair.0.relative_path.as_str())
        .collect();
    let json = serde_json::to_string(&paths).map_err(|_| AppError::Internal)?;
    let mut statement = connection
        .prepare(
            "SELECT relative_path, entity_id, surface, role, confidence FROM file_profile_entities
             WHERE domain = ?1 AND relative_path IN (SELECT value FROM json_each(?2))
             ORDER BY confidence DESC, role, surface",
        )
        .map_err(unavailable)?;
    let rows = statement
        .query_map(params![domain.as_str(), json], |row| {
            Ok((
                row.get::<_, String>(0)?,
                CardEntity {
                    entity_id: row.get(1)?,
                    surface: row.get(2)?,
                    role: row.get(3)?,
                    confidence: row.get(4)?,
                },
            ))
        })
        .map_err(unavailable)?;
    for (path, entity) in rows.flatten() {
        let card = cards.entry(path).or_default();
        let same = |other: &CardEntity| {
            other.role == entity.role
                && match (other.entity_id, entity.entity_id) {
                    (Some(a), Some(b)) => a == b,
                    _ => other.surface == entity.surface,
                }
        };
        if card.len() < CARD_ENTITIES && !card.iter().any(same) {
            card.push(entity);
        }
    }
    Ok(cards)
}

/// A few words of the passage around the first typed word. The text is the passage as stored; nothing is
/// logged and nothing leaves the workstation.
fn excerpt_of(
    connection: &Connection,
    chunk_id: &str,
    terms: &[String],
    last_is_prefix: bool,
) -> Option<String> {
    let text: String = connection
        .query_row(
            "SELECT text FROM chunks WHERE chunk_id = ?1",
            [chunk_id],
            |row| row.get(0),
        )
        .ok()?;
    let words: Vec<&str> = text.split_whitespace().collect();
    let first = terms.first()?;
    let only_one = terms.len() == 1;
    let at = words
        .iter()
        .position(|word| {
            words_of(word).iter().any(|folded| {
                folded == first
                    || (only_one && last_is_prefix && folded.starts_with(first.as_str()))
            })
        })
        .unwrap_or(0);
    let start = at.saturating_sub(EXCERPT_LEAD);
    let end = (start + EXCERPT_WORDS).min(words.len());
    Some(words[start..end].join(" "))
}
