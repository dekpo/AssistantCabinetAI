//! The outbound-text guard (lot 9), for the external connectors that may exist later.
//!
//! Before any text leaves the workstation for a service the user did not write (a search of legal
//! texts, a public drug list, a timetable), the connector asks this guard which known names the
//! text contains, so that it can refuse the request, strip them, or show the user the exact text
//! before sending. **No connector exists yet**: this is a Rust-only API and no command returns any
//! of it to the interface (an interface that listed "the names found in this text" would be a
//! directory of the names the base holds).
//!
//! # The one deliberate exception to selection scoping
//!
//! Everything else the knowledge base offers is limited to the files the user selected
//! (`docs/SESSION-KB-00-master.md`, invariant I7). **This guard is not**, on purpose: a guard that
//! only knew the names of the current selection would let through the name of a person whose file
//! is not ticked. It therefore reads **every** live name and alias of the base, candidates
//! included, and it matches whatever it finds. The result stays in the process that asked: it is
//! spans of the text the caller already holds, and ids, never the names themselves. The exception
//! is recorded in `docs/PRIVACY-AND-SECURITY.md`.
//!
//! # Cost
//!
//! The names are read once, by [`OutboundGuard::build`] (one pass over the alias table), and held
//! in a hash map. A scan never touches the database: it looks every run of words of the text up in
//! the map, so a scan of two kilobytes against ten thousand names takes well under the five
//! milliseconds the lot asks for. Build a guard when a connector starts a request, not when the
//! application starts: the names change whenever an Analyse or a management write runs.
//!
//! A name found is a **hit** with a strength. `Strong` is a full name (two words or more, not an
//! initial, not a surname alone) or an identifier; `Weak` is a surname alone, a title and a
//! surname, or an initial and a surname. The caller decides what each strength means; the usual
//! answer is to refuse on a strong hit and to warn on a weak one.

use std::collections::{BTreeSet, HashMap};

use rusqlite::Connection;
use unicode_normalization::char::is_combining_mark;

use super::normalize::fold_word;
use super::secret::IdentifierKey;
use super::targeting::QueryVocabulary;
use super::AliasKind;
use crate::error::AppError;

/// The longest run of words looked up as one name.
const MAX_RUN_WORDS: usize = 6;

/// How sure a hit is that the text names the entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Strength {
    /// A surname alone, a title and a surname, an initial and a surname.
    Weak,
    /// A full name, or an identifier.
    Strong,
}

impl Strength {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Weak => "weak",
            Self::Strong => "strong",
        }
    }
}

/// One place of the text that names a known entity. Offsets are bytes of the text scanned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GuardHit {
    pub start: usize,
    pub end: usize,
    pub entity_id: i64,
    pub strength: Strength,
}

#[derive(Debug, Clone, Copy)]
struct Target {
    entity_id: i64,
    strength: Strength,
}

/// Every live name of the base, ready to scan a text against.
pub struct OutboundGuard {
    /// Folded words joined by one space, to the entities that answer to them.
    names: HashMap<String, Vec<Target>>,
    /// The stored form of an identifier (a keyed hash for a personal one) to its entities.
    identifier_values: HashMap<String, Vec<i64>>,
    /// `scheme:stored form` to the entities that carry it as an attribute.
    identifier_attributes: HashMap<String, Vec<i64>>,
    longest_name: usize,
}

fn db<T>(result: rusqlite::Result<T>) -> Result<T, AppError> {
    result.map_err(|_| AppError::KnowledgeUnavailable)
}

fn strength_of(words: usize, kind: Option<AliasKind>) -> Strength {
    let by_nature_weak = kind.is_some_and(|kind| {
        matches!(
            kind,
            AliasKind::Initial | AliasKind::PartialSurname | AliasKind::TitleForm
        )
    });
    if words >= 2 && !by_nature_weak {
        Strength::Strong
    } else {
        Strength::Weak
    }
}

impl OutboundGuard {
    /// Read every live name, alias and identifier of the base. **Global on purpose** (see the
    /// module note). `key` is the identifier key: without it a personal identifier written in a
    /// text cannot be recognised, because the base only holds its hash.
    pub fn build(connection: &Connection) -> Result<Self, AppError> {
        let mut names: HashMap<String, Vec<Target>> = HashMap::new();
        let mut longest_name = 1;
        let mut add = |entity_id: i64, normalized: &str, kind: Option<AliasKind>| {
            let words = normalized
                .split(' ')
                .filter(|word| !word.is_empty())
                .count();
            // A single letter is an initial, never a name a text would be refused for.
            if words == 0 || (words == 1 && normalized.chars().count() < 2) {
                return;
            }
            let target = Target {
                entity_id,
                strength: strength_of(words, kind),
            };
            let entry = names.entry(normalized.to_string()).or_default();
            match entry
                .iter_mut()
                .find(|known| known.entity_id == target.entity_id)
            {
                Some(known) => known.strength = known.strength.max(target.strength),
                None => entry.push(target),
            }
            longest_name = longest_name.max(words.min(MAX_RUN_WORDS));
        };

        // Aliases: the survivor of a merge answers to the names of everything absorbed into it.
        let mut statement = db(connection.prepare(
            "SELECT s.entity_id, a.normalized, a.kind
             FROM kb_aliases a
             JOIN kb_effective_entity v ON v.entity_id = a.entity_id
             JOIN kb_entities s ON s.entity_id = v.effective_id
             WHERE s.status IN ('candidate', 'active') AND s.type_id <> 'identifier'",
        ))?;
        let rows = db(statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        }))?;
        for row in rows {
            let (entity_id, normalized, kind) = db(row)?;
            add(entity_id, &normalized, AliasKind::from_code(&kind));
        }
        drop(statement);

        // Canonical names, including those of the entities absorbed into a live one.
        let mut statement = db(connection.prepare(
            "SELECT s.entity_id, e.normalized_name
             FROM kb_entities e
             JOIN kb_effective_entity v ON v.entity_id = e.entity_id
             JOIN kb_entities s ON s.entity_id = v.effective_id
             WHERE s.status IN ('candidate', 'active') AND s.type_id <> 'identifier'
               AND e.status IN ('candidate', 'active', 'merged')",
        ))?;
        let rows = db(statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        }))?;
        for row in rows {
            let (entity_id, normalized) = db(row)?;
            add(entity_id, &normalized, None);
        }
        drop(statement);

        let mut identifier_values: HashMap<String, Vec<i64>> = HashMap::new();
        let mut statement = db(connection.prepare(
            "SELECT s.entity_id, e.normalized_name
             FROM kb_entities e
             JOIN kb_effective_entity v ON v.entity_id = e.entity_id
             JOIN kb_entities s ON s.entity_id = v.effective_id
             WHERE e.type_id = 'identifier' AND s.status IN ('candidate', 'active')",
        ))?;
        let rows = db(statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        }))?;
        for row in rows {
            let (entity_id, value) = db(row)?;
            identifier_values.entry(value).or_default().push(entity_id);
        }
        drop(statement);

        let mut identifier_attributes: HashMap<String, Vec<i64>> = HashMap::new();
        let mut statement = db(connection.prepare(
            "SELECT s.entity_id, t.key, t.value
             FROM kb_attributes t
             JOIN kb_effective_entity v ON v.entity_id = t.entity_id
             JOIN kb_entities s ON s.entity_id = v.effective_id
             WHERE t.key LIKE 'identifier:%' AND s.status IN ('candidate', 'active')",
        ))?;
        let rows = db(statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        }))?;
        for row in rows {
            let (entity_id, key, value) = db(row)?;
            let scheme = key.trim_start_matches("identifier:");
            identifier_attributes
                .entry(format!("{scheme}:{value}"))
                .or_default()
                .push(entity_id);
        }
        drop(statement);

        Ok(Self {
            names,
            identifier_values,
            identifier_attributes,
            longest_name,
        })
    }

    /// How many distinct written forms the guard knows. For tests and for a developer's panel.
    pub fn forms(&self) -> usize {
        self.names.len()
    }

    /// Every place of `text` that names a known entity, left to right.
    ///
    /// A full name wins over the surname inside it: the text `Jean Dupont` is one strong hit, not a
    /// strong hit and a weak one. A name shared by several entities (two Martins) is one hit per
    /// entity on the same span. Case, accents and the order of a reordered form do not matter (the
    /// base keeps a reordered alias); a title in the text is not part of the span unless the base
    /// holds the title form.
    pub fn scan(
        &self,
        vocabulary: &QueryVocabulary,
        key: Option<&IdentifierKey>,
        text: &str,
    ) -> Vec<GuardHit> {
        let mut hits: BTreeSet<GuardHit> = BTreeSet::new();

        let tokens = tokens_of(text);
        let mut at = 0;
        while at < tokens.len() {
            let mut matched = false;
            for length in (1..=self.longest_name.min(tokens.len() - at)).rev() {
                let run = &tokens[at..at + length];
                let form = run
                    .iter()
                    .map(|token| token.folded.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Some(targets) = self.names.get(&form) {
                    for target in targets {
                        hits.insert(GuardHit {
                            start: run[0].start,
                            end: run[length - 1].end,
                            entity_id: target.entity_id,
                            strength: target.strength,
                        });
                    }
                    at += length;
                    matched = true;
                    break;
                }
            }
            if !matched {
                at += 1;
            }
        }

        if !self.identifier_values.is_empty() || !self.identifier_attributes.is_empty() {
            for scheme in vocabulary.packs.identifier_schemes() {
                if scheme.personal_key && key.is_none() {
                    continue;
                }
                for found in scheme.scan(text) {
                    let stored = match (scheme.personal_key, key) {
                        (true, Some(key)) => key.digest(&scheme.id, &found.normalized),
                        _ => found.normalized.clone(),
                    };
                    let mut owners: BTreeSet<i64> = BTreeSet::new();
                    if let Some(ids) = self.identifier_values.get(&stored) {
                        owners.extend(ids.iter().copied());
                    }
                    if let Some(ids) = self
                        .identifier_attributes
                        .get(&format!("{}:{}", scheme.id, stored))
                    {
                        owners.extend(ids.iter().copied());
                    }
                    for entity_id in owners {
                        hits.insert(GuardHit {
                            start: found.range.start,
                            end: found.range.end,
                            entity_id,
                            strength: Strength::Strong,
                        });
                    }
                }
            }
        }
        hits.into_iter().collect()
    }
}

/// `text` with every span of `hits` replaced by `placeholder`. Overlapping and touching spans are
/// replaced once. The hits must come from a scan of this same text; a span that does not fall on a
/// character boundary of it is skipped rather than allowed to panic.
pub fn strip(text: &str, hits: &[GuardHit], placeholder: &str) -> String {
    let mut spans: Vec<(usize, usize)> = hits
        .iter()
        .filter(|hit| {
            hit.start < hit.end
                && hit.end <= text.len()
                && text.is_char_boundary(hit.start)
                && text.is_char_boundary(hit.end)
        })
        .map(|hit| (hit.start, hit.end))
        .collect();
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in spans {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    for (start, end) in merged {
        out.push_str(&text[from..start]);
        out.push_str(placeholder);
        from = end;
    }
    out.push_str(&text[from..]);
    out
}

struct Token {
    folded: String,
    start: usize,
    end: usize,
}

/// The words of a text as a name is split into words: runs of letters and digits, an apostrophe or
/// a hyphen or any punctuation ending one, each folded (case, accents, ligatures).
fn tokens_of(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut from: Option<usize> = None;
    let flush = |from: usize, to: usize, tokens: &mut Vec<Token>| {
        let folded = fold_word(&text[from..to]);
        if !folded.is_empty() {
            tokens.push(Token {
                folded,
                start: from,
                end: to,
            });
        }
    };
    for (index, character) in text.char_indices() {
        if character.is_alphanumeric() || is_combining_mark(character) {
            from.get_or_insert(index);
        } else if let Some(start) = from.take() {
            flush(start, index, &mut tokens);
        }
    }
    if let Some(start) = from {
        flush(start, text.len(), &mut tokens);
    }
    tokens
}
