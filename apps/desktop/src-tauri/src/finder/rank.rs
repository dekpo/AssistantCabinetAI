//! The rank question: "what is the latest letter received?" (P-2, `docs/LEXICAL-RETRIEVAL-DESIGN.md`).
//!
//! ```text
//! [opener] [determiners] latest|earliest  noun   [received]
//! What is   the          latest           letter  received
//! ```
//!
//! or the noun first (`the letter that is the latest`). Answered from the inventory alone: no model, no
//! retrieval. The sentence is exactly this shape or it keeps its road; any other word is a filter.
//!
//! A collection noun that stands for a business type (`factures`, `ordonnances`) filters by the **type in the
//! file's profile**; any other noun filters by **words of the file's path** (`courrier` meets
//! `courrier-orl.pdf` and a folder `courriers/`), and so does a file with no profile yet, which the
//! Analyse has not read. A noun that names no kind (`documents`) covers every readable document.
//!
//! Files are ranked by the document date of their profile (the date written in the document, else the one
//! at the start of the clean name, with the source the profile recorded), else by the date at the start of
//! the clean name, else by the modification date. The answer says what each date comes from, because a
//! modification date is the date of a copy and not always the date of the letter.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::question_frame::{longest, tokens_of, CollectionNoun, Token};
use crate::file_profile::name::{split_date_prefix, NameProfile};
use crate::file_record::{FileRecord, Readability};
use crate::folder_questions::FolderAnswer;
use crate::inventory::WorkFolderInventory;
use crate::knowledge::normalize::fold;

/// The files listed under the answer, the first being the answer itself.
pub const RANKING_SHOWN: usize = 5;

/// What the profile of one file says that a ranking needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileFact {
    /// The content identity the profile was built from; a profile of other content is not read.
    pub content_id: String,
    pub business_type: Option<String>,
    /// The chosen document date, `YYYY-MM-DD`, and the source the profile recorded for it.
    pub date: Option<(String, String)>,
}

/// The rank sentence reads the profiles through this port: the router holds no connection.
pub trait ProfileFacts {
    /// One entry per profiled document, by relative path.
    fn documents(&self) -> HashMap<String, ProfileFact>;
}

/// The source code of the profile's date as the answer names it: the date at the start of the name is
/// `filename` in both places.
fn basis_of(profile_source: &str) -> String {
    match profile_source {
        "filename_prefix" => "filename".to_string(),
        other => other.to_string(),
    }
}

/// The words of the rank sentence for one language.
#[derive(Debug, Clone, Deserialize)]
pub struct RankQuestionVocabulary {
    pub openers: Vec<Vec<String>>,
    pub determiners: Vec<Vec<String>>,
    pub latest: Vec<Vec<String>>,
    pub earliest: Vec<Vec<String>>,
    /// Words that may end the sentence (`received`).
    pub trailing: Vec<Vec<String>>,
}

impl RankQuestionVocabulary {
    pub fn folded(mut self) -> Self {
        for sequences in [
            &mut self.openers,
            &mut self.determiners,
            &mut self.latest,
            &mut self.earliest,
            &mut self.trailing,
        ] {
            for sequence in sequences.iter_mut() {
                for word in sequence.iter_mut() {
                    *word = fold(word);
                }
            }
        }
        self
    }
}

/// What a rank sentence asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankQuestion {
    /// True for the latest, false for the earliest.
    pub latest: bool,
    /// The noun as the user wrote it, `None` for a noun that names no kind of document.
    pub noun: Option<String>,
    /// The folded singular word the file path must hold (or that word with an `s`), `None` for every document.
    stem: Option<String>,
    /// The business type the noun stands for, read from the profile.
    kind: Option<String>,
}

fn last_noun_at<'a>(
    nouns: &'a [CollectionNoun],
    tokens: &[Token],
    at: usize,
) -> Option<&'a CollectionNoun> {
    nouns
        .iter()
        .filter(|noun| {
            !noun.words.is_empty()
                && at + noun.words.len() <= tokens.len()
                && noun
                    .words
                    .iter()
                    .zip(&tokens[at..])
                    .all(|(word, token)| *word == token.folded)
        })
        .max_by_key(|noun| noun.words.len())
}

impl RankQuestionVocabulary {
    /// Read `question` as a rank sentence, or `None`. `nouns` are the collection nouns of the set frame.
    pub fn parse(&self, question: &str, nouns: &[CollectionNoun]) -> Option<RankQuestion> {
        let mut tokens: Vec<Token> = Vec::new();
        tokens_of(question, 0, &mut tokens);
        let mut at = 0;
        if let Some(length) = longest(&self.openers, &tokens, at) {
            at += length;
        }
        let skip_determiners = |mut at: usize| {
            while let Some(length) = longest(&self.determiners, &tokens, at) {
                at += length;
            }
            at
        };
        at = skip_determiners(at);

        let order_at = |at: usize| -> Option<(bool, usize)> {
            let latest = longest(&self.latest, &tokens, at);
            let earliest = longest(&self.earliest, &tokens, at);
            match (latest, earliest) {
                (Some(l), Some(e)) if l >= e => Some((true, l)),
                (Some(_), Some(e)) => Some((false, e)),
                (Some(l), None) => Some((true, l)),
                (None, Some(e)) => Some((false, e)),
                (None, None) => None,
            }
        };

        let (latest, noun, noun_at, end) = if let Some((latest, length)) = order_at(at) {
            let noun_at = at + length;
            let noun = last_noun_at(nouns, &tokens, noun_at)?;
            (latest, noun, noun_at, noun_at + noun.words.len())
        } else {
            let noun = last_noun_at(nouns, &tokens, at)?;
            let after = skip_determiners(at + noun.words.len());
            let (latest, length) = order_at(after)?;
            (latest, noun, at, after + length)
        };
        let mut end = end;
        while let Some(length) = longest(&self.trailing, &tokens, end) {
            end += length;
        }
        if end != tokens.len() {
            return None;
        }
        let written = &question[tokens[noun_at].start..tokens[noun_at + noun.words.len() - 1].end];
        // The word the path must hold, in the singular: `letters` meets `letter-orl.pdf`.
        let stem = noun.words.first().map(|word| match word.strip_suffix('s') {
            Some(single) if single.chars().count() >= 3 => single.to_string(),
            _ => word.clone(),
        });
        Some(RankQuestion {
            latest,
            noun: (!noun.generic).then(|| written.to_string()),
            stem: if noun.generic { None } else { stem },
            kind: noun.kind.clone(),
        })
    }
}

/// One file of a ranking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedEntry {
    pub relative_path: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// What the date comes from: `filename` (the start of the name), `modified` (the modification date),
    /// or the source the profile recorded (`header_label`, `header_dated`, `header_title`,
    /// `header_text`, `metadata_created`).
    pub basis: String,
}

fn date_of_modification(seconds: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(seconds, 0).map(|moment| moment.format("%Y-%m-%d").to_string())
}

fn ranked_entry(file: &FileRecord, fact: Option<&ProfileFact>) -> Option<RankedEntry> {
    if let Some((date, source)) = fact.and_then(|fact| fact.date.as_ref()) {
        return Some(RankedEntry {
            relative_path: file.relative_path.clone(),
            date: date.clone(),
            basis: basis_of(source),
        });
    }
    if let (Some(date), _) = split_date_prefix(&file.stem) {
        return Some(RankedEntry {
            relative_path: file.relative_path.clone(),
            date,
            basis: "filename".to_string(),
        });
    }
    let date = date_of_modification(file.modified_at?)?;
    Some(RankedEntry {
        relative_path: file.relative_path.clone(),
        date,
        basis: "modified".to_string(),
    })
}

/// Whether the path of `file` holds the word `stem` (or `stem` with an `s`).
fn holds_word(file: &FileRecord, stem: &str) -> bool {
    let plural = format!("{stem}s");
    NameProfile::build(&file.relative_path, None)
        .all_words()
        .any(|found| found == stem || *found == *plural)
}

/// Rank the readable documents of `inventory` for `question`. `profiles` are the documents' profiles when
/// the caller has an index; without them the ranking reads names and modification dates only.
pub fn answer(
    inventory: &WorkFolderInventory,
    question: &RankQuestion,
    profiles: Option<&dyn ProfileFacts>,
) -> FolderAnswer {
    let facts = profiles.map(ProfileFacts::documents).unwrap_or_default();
    // A profile of other content than the file's is not the file's.
    let fact_of = |file: &FileRecord| {
        facts
            .get(&file.relative_path)
            .filter(|fact| fact.content_id == file.id)
    };
    let readable: Vec<&FileRecord> = inventory
        .all_files()
        .iter()
        .filter(|file| file.kind.is_document() && file.readability == Readability::Readable)
        .filter(|file| {
            let by_path = || {
                question
                    .stem
                    .as_deref()
                    .is_none_or(|stem| holds_word(file, stem))
            };
            match (&question.kind, fact_of(file)) {
                // The noun is a type and the file has a profile: the profile decides.
                (Some(kind), Some(fact)) => fact.business_type.as_deref() == Some(kind.as_str()),
                // The file has no profile yet, or the noun is no type: the words of its path decide.
                _ => by_path(),
            }
        })
        .collect();
    let candidates = readable.len();
    let mut ranked: Vec<RankedEntry> = readable
        .iter()
        .filter_map(|file| ranked_entry(file, fact_of(file)))
        .collect();
    let unranked = candidates - ranked.len();
    if question.latest {
        ranked.sort_by(|a, b| {
            b.date
                .cmp(&a.date)
                .then_with(|| a.relative_path.cmp(&b.relative_path))
        });
    } else {
        ranked.sort_by(|a, b| {
            a.date
                .cmp(&b.date)
                .then_with(|| a.relative_path.cmp(&b.relative_path))
        });
    }
    // A modification date is the date of a copy: the answer warns when one stands among dates of documents.
    let copies = ranked
        .iter()
        .filter(|entry| entry.basis == "modified")
        .count();
    let mixed_basis = copies > 0 && copies < ranked.len();
    ranked.truncate(RANKING_SHOWN);
    FolderAnswer::RankedFile {
        latest: question.latest,
        noun: question.noun.clone(),
        by_type: question.kind.is_some(),
        candidates,
        unranked,
        mixed_basis,
        ranking: ranked,
    }
}
