//! The card of a document (long-documents programme, lot 5; stage T0 of the summaries).
//!
//! One short digest per document, built with no model from what the Analyse already stored: its title,
//! type, date and roles (the file profile), its size, and its outline with the first sentence of each
//! section. It answers "what is this document" and "give me its plan" without reading the document, and
//! it is what a summary question reads first (tab 3). It is indexed by words and embedded like a
//! passage, but it is not a passage: it is never cited and is in no excerpt.
//!
//! The labels (`Title:`, `Outline:`...) are fixed English words, the same for any profession; what
//! follows them is the document's own text.

use rusqlite::{Connection, OptionalExtension};

use crate::chunking_long::count_tables;
use crate::error::AppError;
use crate::index_store::IndexStore;

/// Bump when the card changes: every card is built again by the next Analyse.
pub const CARD_VERSION: i64 = 1;

/// The most outline entries on a card, and the most characters of a card.
const MAX_OUTLINE_ENTRIES: usize = 120;
const MAX_CARD_CHARS: usize = 12_000;
/// A first sentence is cut at this length.
const MAX_SENTENCE_CHARS: usize = 160;
const MAX_ROLES: usize = 6;

/// Build the cards that are missing or out of date, from the stored text, structure and profile. Local
/// and cheap (a few milliseconds a document); returns how many were built.
pub fn build_missing(index: &mut IndexStore) -> Result<usize, AppError> {
    let missing = index.documents_needing_a_card(CARD_VERSION)?;
    let mut built = 0;
    for (path, sha256) in missing {
        let text = card_text(index.connection(), &path);
        index.replace_card(&path, &sha256, CARD_VERSION, &text)?;
        built += 1;
    }
    Ok(built)
}

fn first_sentence(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let end = flat
        .char_indices()
        .find(|(_, c)| matches!(c, '.' | '!' | '?'))
        .map(|(position, c)| position + c.len_utf8())
        .unwrap_or(flat.len());
    let sentence = &flat[..end];
    if sentence.chars().count() > MAX_SENTENCE_CHARS {
        let cut: String = sentence.chars().take(MAX_SENTENCE_CHARS).collect();
        format!("{}...", cut.trim_end())
    } else {
        sentence.to_string()
    }
}

/// The words of a line without its markup: lower case, letters and digits only.
fn key_of(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn name_of(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.')
        .map_or(name, |(stem, _)| stem)
        .replace(['-', '_'], " ")
}

fn card_text(connection: &Connection, path: &str) -> String {
    let profile: Option<(Option<String>, Option<String>, Option<String>)> = connection
        .query_row(
            "SELECT title, business_type, document_date FROM file_profiles
             WHERE domain = 'documents' AND relative_path = ?1",
            [path],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .ok()
        .flatten();
    let (title, business_type, date) = profile.unwrap_or((None, None, None));
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "Title: {}",
        title
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| name_of(path))
    ));
    if let Some(kind) = business_type.filter(|k| !k.is_empty()) {
        lines.push(format!("Type: {kind}"));
    }
    if let Some(date) = date.filter(|d| !d.is_empty()) {
        lines.push(format!("Date: {date}"));
    }
    if let Ok(mut statement) = connection.prepare(
        "SELECT role, surface FROM file_profile_entities
         WHERE domain = 'documents' AND relative_path = ?1 AND role NOT IN ('mention', 'mentioned', 'file')
         ORDER BY confidence DESC, rowid LIMIT ?2",
    ) {
        if let Ok(rows) = statement.query_map(rusqlite::params![path, MAX_ROLES as i64], |row| {
            Ok(format!("{}: {}", row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            let roles: Vec<String> = rows.flatten().collect();
            if !roles.is_empty() {
                lines.push(format!("Roles: {}", roles.join("; ")));
            }
        }
    }

    // The text in reading order, a line at a time (a book is never all in memory): the first sentence
    // after each heading, the tables, the pages. A heading is found by its words, whatever markup the
    // text keeps around it (`#`, a number).
    let sections: Vec<(i64, i64, String)> = connection
        .prepare("SELECT position, level, title FROM doc_sections WHERE relative_path = ?1 ORDER BY position")
        .and_then(|mut s| s.query_map([path], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect())
        .unwrap_or_default();
    let keys: Vec<String> = sections.iter().map(|(_, _, title)| key_of(title)).collect();
    let mut firsts: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    let mut start = String::new();
    let mut tables = 0usize;
    let mut pages = 0u32;
    let mut next_heading = 0usize;
    let mut waiting: Option<i64> = None;
    if let Ok(mut statement) = connection.prepare(
        "SELECT page_number, text FROM chunks WHERE relative_path = ?1 ORDER BY page_number, section",
    ) {
        if let Ok(mut rows) = statement.query([path]) {
            while let Ok(Some(row)) = rows.next() {
                let page: i64 = row.get(0).unwrap_or(0);
                let text: String = row.get(1).unwrap_or_default();
                pages = pages.max(page.max(0) as u32);
                tables += count_tables(&text);
                for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
                    let line_key = key_of(line);
                    if next_heading < keys.len() && !keys[next_heading].is_empty() && line_key == keys[next_heading] {
                        waiting = Some(sections[next_heading].0);
                        next_heading += 1;
                        continue;
                    }
                    if let Some(position) = waiting.take() {
                        firsts.entry(position).or_insert_with(|| first_sentence(line));
                    }
                    if start.is_empty() {
                        start = first_sentence(line);
                    }
                }
            }
        }
    }

    lines.push(format!(
        "Size: {}, {}, {}",
        plural(pages as usize, "page", "pages"),
        plural(sections.len(), "section", "sections"),
        plural(tables, "table", "tables"),
    ));
    if sections.is_empty() {
        if !start.is_empty() {
            lines.push(format!("Start: {start}"));
        }
    } else {
        lines.push("Outline:".to_string());
        for (position, level, title) in sections.iter().take(MAX_OUTLINE_ENTRIES) {
            let indent = " ".repeat((*level as usize).saturating_sub(1).min(4) * 2);
            let label = if title.is_empty() {
                "(part)".to_string()
            } else {
                title.clone()
            };
            match firsts.get(position) {
                Some(sentence) => lines.push(format!("{indent}{label}: {sentence}")),
                None => lines.push(format!("{indent}{label}")),
            }
        }
        if sections.len() > MAX_OUTLINE_ENTRIES {
            lines.push(format!(
                "({} more sections)",
                sections.len() - MAX_OUTLINE_ENTRIES
            ));
        }
    }
    let mut text = lines.join("\n");
    if text.chars().count() > MAX_CARD_CHARS {
        text = text.chars().take(MAX_CARD_CHARS).collect();
    }
    text
}
