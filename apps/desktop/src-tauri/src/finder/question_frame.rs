//! The set question: a sentence that asks which files talk about something, answered by the finder and
//! not by a model (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 7, `docs/DECISIONS.md` 14 October 2026).
//!
//! ```text
//! [opener] [determiners] collection-noun [linker] verb  X
//! Which     the          documents       that     talk about  LAMal
//! ```
//!
//! Pure: no database and no I/O. The words of the frame (openers, determiners, collection nouns,
//! linkers, verbs) are **data** in `resources/work-folder-questions/*.json`, under `set_question`, and
//! are compared on folded forms, so an accent nobody typed never matters. A sentence is a set question
//! only when it is exactly this shape:
//!
//! - any other word between the noun and the verb is a **filter** (`courriers of 2026 that ...`) and the
//!   sentence keeps its road;
//! - X is one to `max_terms` words, or a quoted phrase, or an identifier (the caller decides that last
//!   one, it holds the packs); a longer X is a question about meaning and keeps its road;
//! - an interrogative as X (`about what`) is not a term.
//!
//! The collection noun may carry a business type (`letters` is the type `letter`): it then becomes a
//! real filter of the search.

use serde::Deserialize;

use crate::knowledge::normalize::fold;

/// The most words of a quoted phrase.
const DEFAULT_MAX_QUOTED_TERMS: usize = 8;

fn default_max_terms() -> usize {
    4
}

fn default_max_quoted_terms() -> usize {
    DEFAULT_MAX_QUOTED_TERMS
}

/// A collection noun: `words` as a token sequence, and the business type it stands for, if any.
#[derive(Debug, Clone, Deserialize)]
pub struct CollectionNoun {
    pub words: Vec<String>,
    #[serde(default)]
    pub kind: Option<String>,
    /// A noun that names no kind of document (`documents`, `files`): a ranking over it covers every readable file.
    #[serde(default)]
    pub generic: bool,
}

/// The words of the frame for one language.
#[derive(Debug, Clone, Deserialize)]
pub struct SetQuestionVocabulary {
    #[serde(default = "default_max_terms")]
    pub max_terms: usize,
    #[serde(default = "default_max_quoted_terms")]
    pub max_quoted_terms: usize,
    pub openers: Vec<Vec<String>>,
    pub determiners: Vec<Vec<String>>,
    pub nouns: Vec<CollectionNoun>,
    pub linkers: Vec<Vec<String>>,
    pub verbs: Vec<Vec<String>>,
    /// Words dropped from the start of X (`the LAMal` is `LAMal`).
    pub articles: Vec<String>,
    /// A word that makes X a question and not a term.
    pub stop_terms: Vec<String>,
}

impl SetQuestionVocabulary {
    /// The same vocabulary with every word folded, so a comparison is one string equality.
    pub fn folded(mut self) -> Self {
        let fold_all = |sequences: &mut Vec<Vec<String>>| {
            for sequence in sequences {
                for word in sequence {
                    *word = fold(word);
                }
            }
        };
        fold_all(&mut self.openers);
        fold_all(&mut self.determiners);
        fold_all(&mut self.linkers);
        fold_all(&mut self.verbs);
        for noun in &mut self.nouns {
            for word in &mut noun.words {
                *word = fold(word);
            }
        }
        for word in self.articles.iter_mut().chain(self.stop_terms.iter_mut()) {
            *word = fold(word);
        }
        self
    }
}

/// What a set question asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetQuestion {
    /// X as the user wrote it (the inside of the quotes for a quoted phrase), articles removed.
    pub text: String,
    pub quoted: bool,
    /// The collection noun as written, folded: `courriers`.
    pub noun: String,
    /// The business type the noun stands for.
    pub kind: Option<String>,
}

pub(super) struct Token {
    pub(super) folded: String,
    pub(super) start: usize,
    pub(super) end: usize,
}

/// A word of the question, or the one placeholder of a quoted phrase.
const QUOTE_MARK: &str = "\u{0}quoted";

pub(super) fn tokens_of(text: &str, offset: usize, into: &mut Vec<Token>) {
    let mut start: Option<usize> = None;
    for (index, c) in text.char_indices() {
        if c.is_alphanumeric() {
            start.get_or_insert(index);
        } else if let Some(begin) = start.take() {
            into.push(Token {
                folded: fold(&text[begin..index]),
                start: offset + begin,
                end: offset + index,
            });
        }
    }
    if let Some(begin) = start {
        into.push(Token {
            folded: fold(&text[begin..]),
            start: offset + begin,
            end: offset + text.len(),
        });
    }
}

/// The first quoted phrase of `question`: the quotes' positions and the inside.
fn first_quote(question: &str) -> Option<(usize, usize, usize, usize)> {
    for (open, close) in [('"', '"'), ('\u{ab}', '\u{bb}'), ('\u{201c}', '\u{201d}')] {
        let Some(from) = question.find(open) else {
            continue;
        };
        let inside_from = from + open.len_utf8();
        if let Some(length) = question[inside_from..].find(close) {
            let inside_to = inside_from + length;
            return Some((from, inside_from, inside_to, inside_to + close.len_utf8()));
        }
    }
    None
}

/// The length of the longest sequence of `sequences` that starts at `at`.
pub(super) fn longest(sequences: &[Vec<String>], tokens: &[Token], at: usize) -> Option<usize> {
    sequences
        .iter()
        .filter(|sequence| {
            !sequence.is_empty()
                && at + sequence.len() <= tokens.len()
                && sequence
                    .iter()
                    .zip(&tokens[at..])
                    .all(|(word, token)| *word == token.folded)
        })
        .map(Vec::len)
        .max()
}

impl SetQuestionVocabulary {
    /// Read `question` as a set question, or `None` when it is not exactly one.
    pub fn parse(&self, question: &str) -> Option<SetQuestion> {
        let mut tokens: Vec<Token> = Vec::new();
        let mut quoted: Option<(usize, usize)> = None;
        match first_quote(question) {
            Some((from, inside_from, inside_to, after)) => {
                tokens_of(&question[..from], 0, &mut tokens);
                tokens.push(Token {
                    folded: QUOTE_MARK.to_string(),
                    start: inside_from,
                    end: inside_to,
                });
                quoted = Some((inside_from, inside_to));
                tokens_of(&question[after..], after, &mut tokens);
            }
            None => tokens_of(question, 0, &mut tokens),
        }

        let mut at = 0;
        if let Some(length) = longest(&self.openers, &tokens, at) {
            at += length;
        }
        while let Some(length) = longest(&self.determiners, &tokens, at) {
            at += length;
        }
        let (noun_length, noun) = self
            .nouns
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
            .map(|noun| (noun.words.len(), noun))
            .max_by_key(|(length, _)| *length)?;
        let noun_words = tokens[at..at + noun_length]
            .iter()
            .map(|token| token.folded.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        at += noun_length;
        if let Some(length) = longest(&self.linkers, &tokens, at) {
            at += length;
        }
        at += longest(&self.verbs, &tokens, at)?;

        let mut rest = &tokens[at..];
        while let Some((first, others)) = rest.split_first() {
            if self.articles.contains(&first.folded) {
                rest = others;
            } else {
                break;
            }
        }
        let (first, last) = (rest.first()?, rest.last()?);

        if let [only] = rest {
            if only.folded == QUOTE_MARK {
                let (from, to) = quoted?;
                let phrase = question[from..to].trim();
                let words = phrase
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|word| !word.is_empty())
                    .count();
                if words == 0 || words > self.max_quoted_terms {
                    return None;
                }
                return Some(SetQuestion {
                    text: phrase.to_string(),
                    quoted: true,
                    noun: noun_words,
                    kind: noun.kind.clone(),
                });
            }
        }
        if rest.iter().any(|token| token.folded == QUOTE_MARK)
            || rest.len() > self.max_terms
            || rest
                .iter()
                .any(|token| self.stop_terms.contains(&token.folded))
        {
            return None;
        }
        Some(SetQuestion {
            text: question[first.start..last.end].to_string(),
            quoted: false,
            noun: noun_words,
            kind: noun.kind.clone(),
        })
    }
}
