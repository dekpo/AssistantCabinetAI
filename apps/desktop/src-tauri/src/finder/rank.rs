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
//! The collection noun filters by **words of the file's path** (`courrier` meets `courrier-orl.pdf` and a
//! folder `courriers/`), over the readable documents. A noun that names no kind (`documents`) covers every
//! readable document. Files are ranked by the date at the start of their clean name (`2026-10-01_...`),
//! else by their modification date, and the answer says which of the two each date is, because a
//! modification date is the date of a copy and not always the date of the letter.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::question_frame::{longest, tokens_of, CollectionNoun, Token};
use crate::file_profile::name::{split_date_prefix, NameProfile};
use crate::file_record::{FileRecord, Readability};
use crate::folder_questions::FolderAnswer;
use crate::inventory::WorkFolderInventory;
use crate::knowledge::normalize::fold;

/// The files listed under the answer, the first being the answer itself.
pub const RANKING_SHOWN: usize = 5;

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
    /// `filename` (the date at the start of the name) or `modified` (the modification date).
    pub basis: String,
}

fn date_of_modification(seconds: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(seconds, 0).map(|moment| moment.format("%Y-%m-%d").to_string())
}

fn ranked_entry(file: &FileRecord) -> Option<RankedEntry> {
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

/// Rank the readable documents of `inventory` for `question`.
pub fn answer(inventory: &WorkFolderInventory, question: &RankQuestion) -> FolderAnswer {
    let readable: Vec<&FileRecord> = inventory
        .all_files()
        .iter()
        .filter(|file| file.kind.is_document() && file.readability == Readability::Readable)
        .filter(|file| {
            question
                .stem
                .as_deref()
                .is_none_or(|stem| holds_word(file, stem))
        })
        .collect();
    let candidates = readable.len();
    let mut ranked: Vec<RankedEntry> = readable
        .iter()
        .filter_map(|file| ranked_entry(file))
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
    let mixed: BTreeSet<&str> = ranked.iter().map(|entry| entry.basis.as_str()).collect();
    let mixed_basis = mixed.len() > 1;
    ranked.truncate(RANKING_SHOWN);
    FolderAnswer::RankedFile {
        latest: question.latest,
        noun: question.noun.clone(),
        candidates,
        unranked,
        mixed_basis,
        ranking: ranked,
    }
}
