//! The names the knowledge base already holds, in memory, for finding them in a text.
//!
//! A gazetteer is a list of phrases (up to [`MAX_NGRAM`] words, folded) with, for each, the
//! entities that answer to it. Given the words of a passage it finds the longest known phrase at
//! each position, so "Jean Pierre Dupont" is one hit and not three. It is built **once per Analyse
//! pass** from `kb_aliases`, grows with `insert` while the pass discovers new names, and is thrown
//! away with the pass: nothing here is persisted but the epoch.
//!
//! Only names that deserve trust go in: the aliases of active entities, and anything the user typed
//! (`store::name_list_rows`). A guess the extractor made and nobody confirmed (a candidate) is not
//! in the list, so a guess cannot vouch for the next one.
//!
//! The **epoch** is the generation of the list. Documents scanned against an older generation did
//! not have today's names to find, so lot 4 compares a source's `gazetteer_epoch` with
//! [`Gazetteer::epoch`] and reads again the ones that are behind. [`Gazetteer::bump_epoch`] is how
//! a pass that grew the list says so; it is mirrored in `kb_meta.gazetteer_epoch`.

use std::collections::HashMap;

use rusqlite::Connection;

use super::{store, AliasKind, Origin};
use crate::error::AppError;

/// The longest phrase the list keeps, in words. A name of more words is not looked for as a whole.
pub const MAX_NGRAM: usize = 5;

const EPOCH_KEY: &str = "gazetteer_epoch";

/// One entity a phrase can stand for, and how it came to know the phrase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GazetteerEntry {
    pub entity_id: i64,
    pub type_id: String,
    pub kind: AliasKind,
    pub origin: Origin,
}

/// A phrase found in a passage: where it starts (a word index), how many words it spans, and who
/// answers to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GazetteerMatch {
    pub start: usize,
    pub len: usize,
    pub entries: Vec<GazetteerEntry>,
}

#[derive(Debug, Clone)]
struct Phrase {
    words: Vec<String>,
    entries: Vec<GazetteerEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct Gazetteer {
    /// Phrases keyed by their first word, so a scan looks at a handful of candidates per position.
    by_first_word: HashMap<String, Vec<Phrase>>,
    phrases: usize,
    epoch: u32,
}

fn words_of_phrase(normalized: &str) -> Vec<String> {
    normalized.split_whitespace().map(str::to_string).collect()
}

impl Gazetteer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load every name the store trusts, and the epoch it was last at.
    pub fn rebuild_from_store(connection: &Connection) -> Result<Self, AppError> {
        let mut list = Self::new();
        for row in store::name_list_rows(connection)? {
            list.insert(
                &row.normalized,
                GazetteerEntry {
                    entity_id: row.entity_id,
                    type_id: row.type_id,
                    kind: row.kind,
                    origin: row.origin,
                },
            );
        }
        list.epoch = read_epoch(connection)?;
        Ok(list)
    }

    /// Add a phrase (already folded, words separated by one space). Returns whether anything was
    /// learned: a phrase, or a new entity behind a known phrase. A phrase of more than
    /// [`MAX_NGRAM`] words, or an empty one, is ignored.
    pub fn insert(&mut self, normalized: &str, entry: GazetteerEntry) -> bool {
        let words = words_of_phrase(normalized);
        if words.is_empty() || words.len() > MAX_NGRAM {
            return false;
        }
        let bucket = self.by_first_word.entry(words[0].clone()).or_default();
        if let Some(phrase) = bucket.iter_mut().find(|phrase| phrase.words == words) {
            let known = phrase
                .entries
                .iter()
                .any(|known| known.entity_id == entry.entity_id && known.kind == entry.kind);
            if !known {
                phrase.entries.push(entry);
            }
            return !known;
        }
        bucket.push(Phrase {
            words,
            entries: vec![entry],
        });
        self.phrases += 1;
        true
    }

    /// The number of distinct phrases.
    pub fn size(&self) -> usize {
        self.phrases
    }

    pub fn is_empty(&self) -> bool {
        self.phrases == 0
    }

    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Declare the list one generation newer and record it in `kb_meta`. Called by a pass that
    /// added names, so the documents read before it are known to be behind.
    pub fn bump_epoch(&mut self, connection: &Connection) -> Result<u32, AppError> {
        let next = read_epoch(connection)?.max(self.epoch).saturating_add(1);
        store::set_meta(connection, EPOCH_KEY, &next.to_string())?;
        self.epoch = next;
        Ok(next)
    }

    /// The longest phrase that starts at word `start` of `words`, if any.
    pub fn longest_match_at(&self, words: &[String], start: usize) -> Option<GazetteerMatch> {
        let first = words.get(start)?;
        let bucket = self.by_first_word.get(first)?;
        bucket
            .iter()
            .filter(|phrase| {
                words.len() >= start + phrase.words.len()
                    && words[start..start + phrase.words.len()] == phrase.words[..]
            })
            .max_by_key(|phrase| phrase.words.len())
            .map(|phrase| GazetteerMatch {
                start,
                len: phrase.words.len(),
                entries: phrase.entries.clone(),
            })
    }

    /// Every phrase in `words`, scanning left to right and taking the longest at each position, so
    /// matches never overlap.
    pub fn matches_in(&self, words: &[String]) -> Vec<GazetteerMatch> {
        let mut found = Vec::new();
        let mut position = 0;
        while position < words.len() {
            match self.longest_match_at(words, position) {
                Some(hit) => {
                    position += hit.len;
                    found.push(hit);
                }
                None => position += 1,
            }
        }
        found
    }
}

fn read_epoch(connection: &Connection) -> Result<u32, AppError> {
    Ok(store::meta(connection, EPOCH_KEY)?
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{EntityStatus, NewAlias, NewEntity};

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_string).collect()
    }

    fn entry(entity_id: i64, kind: AliasKind) -> GazetteerEntry {
        GazetteerEntry {
            entity_id,
            type_id: "person".to_string(),
            kind,
            origin: Origin::Automatic,
        }
    }

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        crate::index_store::configure_connection(&connection).unwrap();
        store::ensure_schema(&connection).unwrap();
        connection
    }

    fn add(
        connection: &Connection,
        name: &str,
        status: EntityStatus,
        origin: Origin,
        aliases: &[&str],
    ) -> i64 {
        let id = store::create_entity(
            connection,
            &NewEntity {
                type_id: "person".to_string(),
                subtype: None,
                canonical_name: name.to_string(),
                normalized_name: name.to_lowercase(),
                disambiguator: String::new(),
                status,
                origin,
            },
        )
        .unwrap();
        for alias in aliases {
            store::add_alias(
                connection,
                id,
                &NewAlias {
                    display: alias.to_string(),
                    normalized: alias.to_lowercase(),
                    phonetic_key: String::new(),
                    kind: AliasKind::CanonicalVariant,
                    confidence: 1.0,
                },
                origin,
            )
            .unwrap();
        }
        id
    }

    #[test]
    fn the_longest_phrase_wins() {
        let mut list = Gazetteer::new();
        list.insert("alice", entry(1, AliasKind::PartialSurname));
        list.insert("alice archer", entry(2, AliasKind::CanonicalVariant));
        list.insert("alice archer smith", entry(3, AliasKind::CanonicalVariant));

        let text = words("meet alice archer smith today");
        let hit = list.longest_match_at(&text, 1).unwrap();
        assert_eq!((hit.start, hit.len), (1, 3));
        assert_eq!(hit.entries[0].entity_id, 3);

        let shorter = list.longest_match_at(&words("alice archer"), 0).unwrap();
        assert_eq!(shorter.len, 2);
        assert!(list.longest_match_at(&text, 0).is_none());
        assert!(list.longest_match_at(&text, 99).is_none());
    }

    #[test]
    fn a_scan_finds_every_name_without_overlap() {
        let mut list = Gazetteer::new();
        list.insert("alice archer", entry(1, AliasKind::CanonicalVariant));
        list.insert("archer", entry(2, AliasKind::PartialSurname));
        list.insert("bob", entry(3, AliasKind::CanonicalVariant));

        let found = list.matches_in(&words("bob met alice archer and bob"));
        let spans: Vec<(usize, usize)> = found.iter().map(|m| (m.start, m.len)).collect();
        assert_eq!(spans, [(0, 1), (2, 2), (5, 1)]);
    }

    #[test]
    fn insert_grows_incrementally_and_reports_what_it_learned() {
        let mut list = Gazetteer::new();
        assert!(list.insert("alice archer", entry(1, AliasKind::CanonicalVariant)));
        assert_eq!(list.size(), 1);
        // Same phrase, same entity, same kind: nothing new.
        assert!(!list.insert("alice archer", entry(1, AliasKind::CanonicalVariant)));
        // Same phrase, another entity: the phrase is now shared and says so.
        assert!(list.insert("alice archer", entry(2, AliasKind::CanonicalVariant)));
        assert_eq!(list.size(), 1);
        let hit = list.longest_match_at(&words("alice archer"), 0).unwrap();
        assert_eq!(hit.entries.len(), 2);
    }

    #[test]
    fn an_empty_or_overlong_phrase_is_ignored() {
        let mut list = Gazetteer::new();
        assert!(!list.insert("   ", entry(1, AliasKind::Manual)));
        assert!(!list.insert("a b c d e f", entry(1, AliasKind::Manual)));
        assert!(list.insert("a b c d e", entry(1, AliasKind::Manual)));
        assert!(!list.is_empty() && list.size() == 1);
    }

    #[test]
    fn the_store_gives_active_entities_and_manual_rows_but_not_guesses() {
        let connection = connection();
        let active = add(
            &connection,
            "Alice Archer",
            EntityStatus::Active,
            Origin::Automatic,
            &["alice archer", "archer"],
        );
        add(
            &connection,
            "Guess Person",
            EntityStatus::Candidate,
            Origin::Automatic,
            &["guess person"],
        );
        let typed = add(
            &connection,
            "Typed Person",
            EntityStatus::Candidate,
            Origin::Manual,
            &["typed person"],
        );

        let list = Gazetteer::rebuild_from_store(&connection).unwrap();
        assert_eq!(list.size(), 3);
        let hit = list
            .longest_match_at(&words("alice archer"), 0)
            .expect("active entity");
        assert_eq!(hit.entries[0].entity_id, active);
        assert!(list.longest_match_at(&words("guess person"), 0).is_none());
        assert_eq!(
            list.longest_match_at(&words("typed person"), 0)
                .unwrap()
                .entries[0]
                .entity_id,
            typed
        );
    }

    #[test]
    fn a_merged_entity_answers_with_its_survivor() {
        let mut connection = connection();
        let survivor = add(
            &connection,
            "Alice Archer",
            EntityStatus::Active,
            Origin::Automatic,
            &["alice archer"],
        );
        let victim = add(
            &connection,
            "Alyce Archer",
            EntityStatus::Active,
            Origin::Automatic,
            &["alyce archer"],
        );
        let tx = connection.transaction().unwrap();
        store::merge_entities(&tx, survivor, victim).unwrap();
        tx.commit().unwrap();

        let list = Gazetteer::rebuild_from_store(&connection).unwrap();
        let hit = list.longest_match_at(&words("alyce archer"), 0).unwrap();
        assert_eq!(hit.entries[0].entity_id, survivor);
    }

    #[test]
    fn the_epoch_is_mirrored_in_the_store() {
        let connection = connection();
        let mut list = Gazetteer::rebuild_from_store(&connection).unwrap();
        assert_eq!(list.epoch(), 0);
        assert_eq!(list.bump_epoch(&connection).unwrap(), 1);
        assert_eq!(list.bump_epoch(&connection).unwrap(), 2);
        assert_eq!(list.epoch(), 2);
        assert_eq!(
            store::meta(&connection, "gazetteer_epoch")
                .unwrap()
                .as_deref(),
            Some("2")
        );
        // A list built later starts at the stored generation, and a stale one catches up.
        let later = Gazetteer::rebuild_from_store(&connection).unwrap();
        assert_eq!(later.epoch(), 2);
        let mut stale = Gazetteer::new();
        assert_eq!(stale.bump_epoch(&connection).unwrap(), 3);
    }
}
