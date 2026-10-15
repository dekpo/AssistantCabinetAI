//! Refusing without a similarity threshold (retrieval-quality programme, lot E). No threshold separates a
//! passage that answers from one that only resembles the question: the closest passage of a question the
//! documents cannot answer is as close as that of one they can (`docs/RETRIEVAL-LAB.md`, 3.2). What the
//! application can check is simpler: **a name or a code the question is about, which no document holds**.
//!
//! A question that asks for `FAC-2026-999`, for "Zircon-9" or for a person nobody wrote about has
//! nothing to be answered from, whatever the closest passage looks like. The check is cheap (a few
//! full-text lookups, no embedding and no model), language-neutral (it reads capital letters and digits,
//! not words), and conservative: it refuses only when **every** distinctive word of the question is unknown
//! to the index, so a misspelt name next to a known one, a title, or a paraphrase is never refused by it.
//! The model's own refusal stays the second line.

use std::collections::HashSet;

use crate::error::AppError;
use crate::index_store::IndexStore;
use crate::knowledge::normalize::fold_word;
use crate::lexical::Lexicon;

/// A word or a code of the question that stands for something specific: its parts, folded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Distinctive {
    /// As written in the question, for the diagnostics and the tests.
    pub written: String,
    /// The parts, folded: `Zircon-9` is `zircon`, `9`.
    pub parts: Vec<String>,
    /// A code (it holds a digit): known only if the whole code is written somewhere. A name is known as
    /// soon as one of its parts is.
    pub is_code: bool,
}

/// Strip what surrounds a word in a sentence, and the elided article in front of it (`l'`, `d'`).
fn core_of(piece: &str) -> &str {
    let trimmed = piece.trim_matches(|c: char| !c.is_alphanumeric());
    match trimmed.rsplit_once(['\'', '\u{2019}']) {
        Some((_, rest)) if !rest.is_empty() => rest,
        _ => trimmed,
    }
}

/// Whether `core` ends with a file extension: a dot, a letter, and up to four letters or digits.
fn looks_like_a_file_name(core: &str) -> bool {
    match core.rsplit_once('.') {
        Some((stem, extension)) => {
            !stem.is_empty()
                && (1..=5).contains(&extension.chars().count())
                && extension
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic())
                && extension.chars().all(|c| c.is_ascii_alphanumeric())
        }
        None => false,
    }
}

/// The distinctive words of `question`: the codes (a digit and something else) and the words that start with
/// a capital letter, the first word of the sentence and the words of the question's frame left out.
pub fn distinctive_words(question: &str, lexicon: &Lexicon) -> Vec<Distinctive> {
    let mut found: Vec<Distinctive> = Vec::new();
    for (position, piece) in question.split_whitespace().enumerate() {
        let core = core_of(piece);
        if core.chars().count() < 2 {
            continue;
        }
        // The name of a file the question points at (`report-2026.pdf`) is where to look, not a fact to find.
        if looks_like_a_file_name(core) {
            continue;
        }
        let has_digit = core.chars().any(|c| c.is_ascii_digit());
        let has_other = core.chars().any(char::is_alphabetic);
        let capital = core.chars().next().is_some_and(char::is_uppercase);
        let is_code = has_digit && has_other;
        if !(is_code || (capital && position > 0 && !has_digit)) {
            continue;
        }
        let parts: Vec<String> = core
            .split(|c: char| !c.is_alphanumeric())
            .filter(|part| !part.is_empty())
            .map(fold_word)
            .collect();
        if parts.is_empty() || (!is_code && parts.iter().all(|p| lexicon.frame.contains(p))) {
            continue;
        }
        if !found.iter().any(|known| known.parts == parts) {
            found.push(Distinctive {
                written: core.to_string(),
                parts,
                is_code,
            });
        }
    }
    found
}

/// What the check decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Go on: nothing distinctive, or at least one distinctive word is in the documents.
    Proceed,
    /// Every distinctive word of the question is unknown to the index (the words, as written).
    Unknown(Vec<String>),
}

/// Whether the documents hold any of the distinctive words of `question`. Reads the full-text index of the
/// passages and of the file profiles (names, entities, headers), whatever the selection.
pub fn check(index: &IndexStore, question: &str) -> Result<Verdict, AppError> {
    let words = distinctive_words(question, Lexicon::shipped());
    if words.is_empty() {
        return Ok(Verdict::Proceed);
    }
    for word in &words {
        let known = match word.is_code {
            // The whole code, as the phrase of its parts.
            true => index.phrase_in_index(&word.parts)?,
            // A name: one of its parts is enough.
            false => {
                let mut any = false;
                for part in &word.parts {
                    if index.phrase_in_index(std::slice::from_ref(part))? {
                        any = true;
                        break;
                    }
                }
                any
            }
        };
        if known {
            return Ok(Verdict::Proceed);
        }
    }
    let mut seen = HashSet::new();
    Ok(Verdict::Unknown(
        words
            .into_iter()
            .map(|word| word.written)
            .filter(|written| seen.insert(written.clone()))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(question: &str) -> Vec<String> {
        distinctive_words(question, Lexicon::shipped())
            .into_iter()
            .map(|word| word.written)
            .collect()
    }

    #[test]
    fn a_code_and_a_name_in_the_middle_of_a_sentence_are_distinctive() {
        assert_eq!(
            written("What is the total of invoice FAC-2026-999 for Camille Dupont?"),
            vec!["FAC-2026-999", "Camille", "Dupont"]
        );
        assert_eq!(written("Who runs the site Beryl-12?"), vec!["Beryl-12"]);
    }

    #[test]
    fn the_name_of_a_file_the_question_points_at_is_not_a_fact_to_find() {
        assert!(written("What does 2026-07-20_letter-skin.docx say?").is_empty());
        assert_eq!(written("Who runs the site Beryl-12?"), vec!["Beryl-12"]);
        assert_eq!(written("Is the value 6.8 high for site X-9?"), vec!["X-9"]);
    }

    #[test]
    fn the_first_word_a_plain_word_and_a_lone_number_are_not() {
        assert!(written("Which documents mention the annual fee?").is_empty());
        assert!(
            written("What is 300 francs worth?").is_empty(),
            "a number alone is no code"
        );
        assert!(written("what is the fee?").is_empty());
    }

    #[test]
    fn an_elided_article_is_cut_before_the_name() {
        assert_eq!(
            written("what does l'Assurance Helsana say?"),
            vec!["Assurance", "Helsana"]
        );
        assert_eq!(written("notes about d'Obsidienne"), vec!["Obsidienne"]);
    }

    #[test]
    fn the_words_of_the_frame_that_start_with_a_capital_are_left_out() {
        // "Which" in the middle of a sentence is the frame, not a name
        assert!(written("tell me Which documents").is_empty());
    }

    #[test]
    fn a_code_keeps_its_parts_and_a_name_with_a_hyphen_too() {
        let words = distinctive_words("the site Zircon-9 and Tschumi-Bauer", Lexicon::shipped());
        assert_eq!(words[0].parts, vec!["zircon", "9"]);
        assert!(words[0].is_code);
        assert_eq!(words[1].parts, vec!["tschumi", "bauer"]);
        assert!(!words[1].is_code);
    }
}
