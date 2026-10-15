//! The lexical channel, made precise (retrieval-quality programme, lot B). Each method kept sits behind its own
//! switch in `LexicalOptions`, was measured alone against the baseline (`docs/RETRIEVAL-QUALITY.md`) and is kept
//! only if it helps and nothing regresses. With every switch off the query is today's, byte for byte.
//!
//! **The plan.** A question becomes one full-text expression over the passages as written: the words of four
//! letters or more, and, with the phrases on, the exact phrases (what the user quoted, identifier-shaped words
//! such as `FAC-2026-014` or `756.1234.5678.97`, two content words side by side) as alternatives of the same
//! `OR`, so that the ranking, which weighs a word by its rarity, decides between a phrase and a rare word.
//!
//! **Methods removed.** The stems, the trigrams, the proximity group, the content terms and the field weights
//! were measured useless or harmful and are gone, with their tables: `docs/RETRIEVAL-QUALITY.md`, "Methods
//! removed, and why", says what each was and when to try it again.
//!
//! **Data, not code.** The words that belong to the way a question is asked (the frame) come from the
//! question vocabulary of the router, and the synonyms from `resources/thesaurus/<pack>.json`. Every
//! shipped pack is read, cumulatively: a synonym group only adds terms, so a Swiss term never harms a
//! French user.

use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};

use serde::Deserialize;
use unicode_normalization::UnicodeNormalization;

use crate::knowledge::normalize::fold_word;

/// One switch per method. `Default` is today's behaviour exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LexicalOptions {
    /// Exact phrases: what the user quoted, identifier-shaped words, two content words side by side.
    pub phrases: bool,
    /// Search the synonyms of the active packs too. Kept off until a real document shows a synonym it finds
    /// (no question of the exam uses one): `docs/RETRIEVAL-QUALITY.md`, "Methods removed, and why".
    pub thesaurus: bool,
}

impl LexicalOptions {
    /// Every switch off.
    pub const OFF: Self = Self {
        phrases: false,
        thesaurus: false,
    };

    /// Whether any switch changes the query.
    pub fn is_off(&self) -> bool {
        *self == Self::OFF
    }
}

static OPTIONS: RwLock<Option<LexicalOptions>> = RwLock::new(None);

/// The options the product runs with: `PRODUCT_CHOICE` unless a measurement set something else (the lab
/// does, through `set_lexical_options_for_measurement`).
pub fn lexical_options() -> LexicalOptions {
    OPTIONS
        .read()
        .ok()
        .and_then(|guard| *guard)
        .unwrap_or(PRODUCT_CHOICE)
}

/// The method kept by the measures of lot B (`docs/RETRIEVAL-QUALITY.md`, section 12): the phrases. They
/// raised the recall of the exam by 0.059 on what the product sends (0.784 to 0.843, the `identifier` family by
/// 0.273) and the planted fact found by 0.286, with no family lower; on the library of 1 007 documents the
/// recall rose by 0.047. The synonyms stay off, with their reason written beside the switch.
const PRODUCT_CHOICE: LexicalOptions = LexicalOptions {
    phrases: true,
    ..LexicalOptions::OFF
};

/// Measurement only: run with other options, or `None` for the product's.
pub fn set_lexical_options_for_measurement(options: Option<LexicalOptions>) {
    if let Ok(mut guard) = OPTIONS.write() {
        *guard = options;
    }
}

// ---------------------------------------------------------------------------------------------
// The words of a question
// ---------------------------------------------------------------------------------------------

/// A word of the question, as the word search sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Word {
    /// As written, accents included.
    text: String,
    /// Four letters or more: the rule of the word search (`index_store::lexical_words`).
    content: bool,
}

fn words_of(text: &str) -> Vec<Word> {
    text.nfc()
        .collect::<String>()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|piece| !piece.is_empty())
        .map(|piece| Word {
            text: piece.to_string(),
            content: piece.chars().count() >= 4,
        })
        .collect()
}

/// The phrases the user quoted: text between straight or angle quotation marks, at least two words.
fn quoted_phrases(question: &str) -> Vec<Vec<String>> {
    let mut found = Vec::new();
    let mut chars = question.chars().peekable();
    while let Some(open) = chars.next() {
        let close = match open {
            '"' => '"',
            '\u{ab}' => '\u{bb}',
            '\u{201c}' => '\u{201d}',
            _ => continue,
        };
        let mut inside = String::new();
        for c in chars.by_ref() {
            if c == close {
                break;
            }
            inside.push(c);
        }
        let words: Vec<String> = words_of(&inside).into_iter().map(|w| w.text).collect();
        if words.len() >= 2 {
            found.push(words);
        }
    }
    found
}

/// Words written as one code with separators inside: `FAC-2026-014`, `756.1234.5678.97`, `2026-03-09`.
/// The full-text index stores them cut at the separators, so they are searched as the phrase of their parts.
pub fn identifier_shaped(question: &str) -> Vec<Vec<String>> {
    question
        .split_whitespace()
        .filter_map(|piece| {
            let piece = piece.trim_matches(|c: char| !c.is_alphanumeric());
            let has_digit = piece.chars().any(|c| c.is_ascii_digit());
            let has_separator = piece
                .chars()
                .any(|c| matches!(c, '-' | '.' | '/' | '_') && !piece.starts_with(c));
            if !(has_digit && has_separator) {
                return None;
            }
            let parts: Vec<String> = words_of(piece).into_iter().map(|w| w.text).collect();
            (parts.len() >= 2).then_some(parts)
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The lexicon: the frame words and the thesaurus
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawThesaurus {
    schema: u32,
    id: String,
    #[serde(default, rename = "_comment")]
    _comment: String,
    groups: Vec<Vec<String>>,
}

const THESAURI: &[(&str, &str)] = &[
    (
        "health-fr",
        include_str!("../resources/thesaurus/health-fr.json"),
    ),
    (
        "health-ch",
        include_str!("../resources/thesaurus/health-ch.json"),
    ),
];

const QUESTION_VOCABULARIES: &[&str] = &[
    include_str!("../resources/work-folder-questions/fr-FR.json"),
    include_str!("../resources/work-folder-questions/en-US.json"),
];

/// The synonym groups of the shipped packs. Each entry is the words of an expression, folded.
#[derive(Debug, Clone, Default)]
pub struct Thesaurus {
    groups: Vec<Vec<Vec<String>>>,
}

impl Thesaurus {
    /// Every shipped pack, cumulatively. An invalid file is an error a test catches, never a user.
    pub fn shipped() -> Result<Self, String> {
        let mut groups = Vec::new();
        for (expected, body) in THESAURI {
            let raw: RawThesaurus = serde_json::from_str(body).map_err(|e| e.to_string())?;
            if raw.schema != 1 || raw.id != *expected {
                return Err(format!("thesaurus {expected}: unsupported schema or id"));
            }
            for group in raw.groups {
                let entries: Vec<Vec<String>> = group
                    .iter()
                    .map(|entry| words_of(entry).iter().map(|w| fold_word(&w.text)).collect())
                    .filter(|entry: &Vec<String>| !entry.is_empty())
                    .collect();
                if entries.len() >= 2 {
                    groups.push(entries);
                }
            }
        }
        Ok(Self { groups })
    }

    pub fn from_groups(groups: &[&[&str]]) -> Self {
        Self {
            groups: groups
                .iter()
                .map(|group| {
                    group
                        .iter()
                        .map(|entry| words_of(entry).iter().map(|w| fold_word(&w.text)).collect())
                        .collect()
                })
                .collect(),
        }
    }

    /// The other entries of every group that has an entry in `folded_words` (the question's words, folded,
    /// in order). Each entry is a list of folded words; an expression matches as a run of words.
    fn equivalents(&self, folded_words: &[String]) -> Vec<Vec<String>> {
        let mut found: Vec<Vec<String>> = Vec::new();
        for group in &self.groups {
            let triggered: Vec<&Vec<String>> = group
                .iter()
                .filter(|entry| {
                    folded_words
                        .windows(entry.len())
                        .any(|window| window == entry.as_slice())
                })
                .collect();
            if triggered.is_empty() {
                continue;
            }
            for entry in group {
                if !triggered.contains(&entry) && !found.contains(entry) {
                    found.push(entry.clone());
                }
            }
        }
        found
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

/// What the plan needs besides the question.
#[derive(Debug, Clone, Default)]
pub struct Lexicon {
    /// Folded words that only frame a question.
    pub frame: HashSet<String>,
    pub thesaurus: Thesaurus,
}

impl Lexicon {
    /// The shipped lexicon, built once.
    pub fn shipped() -> &'static Lexicon {
        static SHIPPED: OnceLock<Lexicon> = OnceLock::new();
        SHIPPED.get_or_init(|| Lexicon {
            frame: shipped_frame(),
            thesaurus: Thesaurus::shipped().unwrap_or_default(),
        })
    }
}

/// The words that make a sentence a question rather than say what it is about: the router's filler, the
/// openers, determiners, linkers and verbs of the set sentence, its stop terms and its generic nouns
/// ("document", "file"). The nouns that name a kind of paper (invoice, letter) stay content.
fn shipped_frame() -> HashSet<String> {
    let mut frame = HashSet::new();
    let mut add = |word: &str| {
        let folded = fold_word(word);
        if !folded.is_empty() {
            frame.insert(folded);
        }
    };
    for body in QUESTION_VOCABULARIES {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
            continue;
        };
        let strings = |value: &serde_json::Value| -> Vec<String> {
            let mut out = Vec::new();
            fn walk(value: &serde_json::Value, out: &mut Vec<String>) {
                match value {
                    serde_json::Value::String(text) => out.push(text.clone()),
                    serde_json::Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
                    _ => {}
                }
            }
            walk(value, &mut out);
            out
        };
        if let Some(filler) = value.get("filler") {
            strings(filler).iter().for_each(|w| add(w));
        }
        if let Some(set) = value.get("set_question") {
            for key in ["openers", "determiners", "linkers", "verbs", "stop_terms"] {
                if let Some(list) = set.get(key) {
                    strings(list).iter().for_each(|w| add(w));
                }
            }
            if let Some(nouns) = set.get("nouns").and_then(|n| n.as_array()) {
                for noun in nouns {
                    if noun.get("generic").and_then(|g| g.as_bool()) == Some(true) {
                        if let Some(words) = noun.get("words") {
                            strings(words).iter().for_each(|w| add(w));
                        }
                    }
                }
            }
        }
    }
    frame
}

// ---------------------------------------------------------------------------------------------
// The plan
// ---------------------------------------------------------------------------------------------

/// One full-text expression, a valid `MATCH` argument for `chunks_fts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tier {
    pub expression: String,
}

/// The full-text expressions to run, strongest first. Each is a valid `MATCH` argument for its table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LexicalPlan {
    pub tiers: Vec<Tier>,
}

impl LexicalPlan {
    pub fn is_empty(&self) -> bool {
        self.tiers.is_empty()
    }
}

fn quoted(words: &[String]) -> String {
    format!("\"{}\"", words.join(" "))
}

/// The expression for `question` under `options`. With every switch off the one tier is the OR of the words
/// of four letters or more, over the passage as written, as it always was.
///
/// The pieces are alternatives of one `OR`, not tiers one after the other: the ranking adds up what a passage
/// holds, each piece weighing what its rarity makes it weigh, so an identifier or a rare word beats a phrase
/// every passage of a template repeats (a first version put the phrases first and lost the rare word:
/// `docs/RETRIEVAL-QUALITY.md`, section 12).
pub fn plan(question: &str, options: &LexicalOptions, lexicon: &Lexicon) -> LexicalPlan {
    let all_words = words_of(question);
    let kept: Vec<&Word> = all_words.iter().filter(|word| word.content).collect();

    let mut alternatives: Vec<String> = Vec::new();
    fn add(alternatives: &mut Vec<String>, alternative: String) {
        if !alternatives
            .iter()
            .any(|known| known.eq_ignore_ascii_case(&alternative))
        {
            alternatives.push(alternative);
        }
    }

    if options.phrases {
        let mut phrases: Vec<Vec<String>> = Vec::new();
        phrases.extend(quoted_phrases(question));
        phrases.extend(identifier_shaped(question));
        // Two content words side by side in the question.
        for pair in all_words.windows(2) {
            if pair[0].content && pair[1].content {
                phrases.push(vec![pair[0].text.clone(), pair[1].text.clone()]);
            }
        }
        for phrase in phrases {
            add(&mut alternatives, quoted(&phrase));
        }
    }

    // The words are never de-duplicated: with every switch off this is the expression it always was.
    for word in &kept {
        alternatives.push(format!("\"{}\"", word.text));
    }
    if options.thesaurus && !lexicon.thesaurus.is_empty() {
        let folded: Vec<String> = all_words.iter().map(|w| fold_word(&w.text)).collect();
        for equivalent in lexicon.thesaurus.equivalents(&folded) {
            add(&mut alternatives, quoted(&equivalent));
        }
    }
    if alternatives.is_empty() {
        return LexicalPlan { tiers: Vec::new() };
    }
    LexicalPlan {
        tiers: vec![Tier {
            expression: alternatives.join(" OR "),
        }],
    }
}

/// The identifiers written in the question as one full-text expression (their parts as phrases, alternatives
/// of one `OR`), or `None` when it holds none.
pub fn identifier_expression(question: &str) -> Option<String> {
    let phrases: Vec<String> = identifier_shaped(question)
        .iter()
        .map(|parts| quoted(parts))
        .collect();
    (!phrases.is_empty()).then(|| phrases.join(" OR "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lexicon() -> Lexicon {
        Lexicon {
            frame: ["which", "documents", "mention"]
                .iter()
                .map(|w| w.to_string())
                .collect(),
            thesaurus: Thesaurus::from_groups(&[
                &["cover", "insurance cover", "indemnity"],
                &["annual fee", "yearly fee"],
            ]),
        }
    }

    fn tiers(question: &str, options: LexicalOptions) -> Vec<String> {
        plan(question, &options, &lexicon())
            .tiers
            .into_iter()
            .map(|tier| tier.expression)
            .collect()
    }

    #[test]
    fn with_every_switch_off_the_plan_is_the_old_or_of_the_words_of_four_letters() {
        assert_eq!(
            tiers("What is the annual fee of Camille?", LexicalOptions::OFF),
            vec!["\"What\" OR \"annual\" OR \"Camille\"".to_string()]
        );
        assert_eq!(
            tiers("Camille Camille", LexicalOptions::OFF),
            vec!["\"Camille\" OR \"Camille\"".to_string()],
            "a repeated word stays repeated, as it always was"
        );
        assert!(tiers("a is of it fee", LexicalOptions::OFF).is_empty());
    }

    #[test]
    fn a_quoted_phrase_an_identifier_and_two_content_words_side_by_side_join_the_words() {
        let options = LexicalOptions {
            phrases: true,
            ..LexicalOptions::OFF
        };
        let found = tiers(
            "Where is \"annual fee\" for invoice FAC-2026-014 and the yearly report?",
            options,
        );
        assert_eq!(found.len(), 1, "{found:?}");
        let all = &found[0];
        assert!(all.contains("\"annual fee\""), "{all}");
        assert!(all.contains("\"FAC 2026 014\""), "{all}");
        assert!(all.contains("\"yearly report\""), "{all}");
        // the words are still there: a phrase adds to them, it does not replace them
        assert!(
            all.contains("\"invoice\"") && all.contains("\"yearly\""),
            "{all}"
        );
    }

    #[test]
    fn a_phrase_is_not_made_of_a_short_word_next_to_a_long_one() {
        let options = LexicalOptions {
            phrases: true,
            ..LexicalOptions::OFF
        };
        let found = tiers("the fee of Camille", options);
        assert_eq!(found, vec!["\"Camille\"".to_string()]);
    }

    #[test]
    fn a_synonym_group_adds_the_other_entries_as_phrases() {
        let on = LexicalOptions {
            thesaurus: true,
            ..LexicalOptions::OFF
        };
        let found = tiers("What does the cover include", on);
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("\"insurance cover\""), "{}", found[0]);
        assert!(found[0].contains("\"indemnity\""), "{}", found[0]);
        assert_eq!(
            found[0].matches("\"cover\"").count(),
            1,
            "the entry that triggered is not added twice"
        );
        // an expression of several words triggers on the run of words
        let phrase = tiers("What is the Annual Fee", on);
        assert!(phrase[0].contains("\"yearly fee\""), "{}", phrase[0]);
        // off: nothing is added
        assert!(
            !tiers("What does the cover include", LexicalOptions::OFF)[0].contains("indemnity")
        );
    }

    #[test]
    fn every_shipped_thesaurus_and_the_frame_load() {
        let thesaurus = Thesaurus::shipped().expect("the shipped files are valid");
        assert!(!thesaurus.is_empty());
        let frame = shipped_frame();
        assert!(frame.contains("documents") && frame.contains("which"));
        assert!(
            !frame.contains("facture") && !frame.contains("invoice"),
            "a kind of paper stays a content word"
        );
    }

    #[test]
    fn the_planned_expressions_are_accepted_by_the_full_text_index() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE VIRTUAL TABLE t USING fts5(chunk_id UNINDEXED, text);
                 INSERT INTO t VALUES ('a', 'The annual fee for invoice FAC-2026-014 is due.');
                 INSERT INTO t VALUES ('b', 'A fee that is yearly, nothing annual about it.');",
            )
            .unwrap();
        let options = LexicalOptions {
            phrases: true,
            thesaurus: true,
        };
        let found = tiers(
            "which documents mention \"annual fee\" FAC-2026-014 cover",
            options,
        );
        assert_eq!(found.len(), 1, "{found:?}");
        let expression = &found[0];
        let matched: Vec<String> = connection
            .prepare("SELECT chunk_id FROM t WHERE t MATCH ?1 ORDER BY bm25(t)")
            .unwrap()
            .query_map([expression], |row| row.get::<_, String>(0))
            .unwrap_or_else(|e| panic!("{expression}: {e}"))
            .map(|r| r.unwrap_or_else(|e| panic!("{expression}: {e}")))
            .collect();
        assert_eq!(
            matched.first().map(String::as_str),
            Some("a"),
            "the passage with the phrase, the code and the words comes first: {expression} found {matched:?}"
        );
    }
}
