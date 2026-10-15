//! The lexical channel, made precise (retrieval-quality programme, lot B). Each method sits behind its own
//! switch in `LexicalOptions`, is measured alone against the baseline (`docs/RETRIEVAL-QUALITY.md`) and is
//! kept only if it helps and nothing regresses. With every switch off the query is today's, byte for byte.
//!
//! **The plan.** A question becomes a few full-text expressions, strongest first (`LexicalPlan::tiers`):
//! the exact phrases (what the user quoted, identifier-shaped words such as `FAC-2026-014` or
//! `756.1234.5678.97`, two content words side by side), then the content words close to each other, then
//! every content word. The index runs the tiers in turn and keeps the first hits, so a passage that holds
//! the phrase wins the places a passage that only holds one word of it would take.
//!
//! **Data, not code.** The words that belong to the way a question is asked (the frame) come from the
//! question vocabulary of the router, and the synonyms from `resources/thesaurus/<pack>.json`. Every
//! shipped pack is read, cumulatively: a synonym group only adds terms, so a Swiss term never harms a
//! French user.

use std::collections::{BTreeMap, HashSet};
use std::sync::{OnceLock, RwLock};

use serde::Deserialize;
use unicode_normalization::UnicodeNormalization;

use crate::knowledge::normalize::fold_word;

/// What to do with the words that only frame a question ("which documents mention ...").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentTerms {
    /// Today: every word of four letters or more is searched.
    #[default]
    Off,
    /// The words of the frame are left out of the search.
    On,
}

/// One switch per method. `Default` is today's behaviour exactly.
///
/// The last three change what the Analyse writes (a table beside `chunks_fts`); a search whose table is
/// not there falls back to the plain words, so an index from before them is never searched wrongly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LexicalOptions {
    /// Exact phrases first: what the user quoted, identifier-shaped words, two content words side by side.
    pub phrases: bool,
    /// Then the content words that stand close to each other.
    pub near: bool,
    /// Leave the words of the question's frame out of the search.
    pub content_terms: ContentTerms,
    /// Search the synonyms of the active packs too.
    pub thesaurus: bool,
    /// Search the stems of the words (French Snowball): a plural and its singular, a verb and its forms meet.
    pub french_stems: bool,
    /// A word in the title or the section path of a passage counts more than the same word in its body.
    /// Needs the stems table, which holds the section path.
    pub field_weights: bool,
    /// A last tier that finds a word or a code as a piece of a longer one (three characters at least).
    pub trigram: bool,
}

impl LexicalOptions {
    /// Every switch off.
    pub const OFF: Self = Self {
        phrases: false,
        near: false,
        content_terms: ContentTerms::Off,
        thesaurus: false,
        french_stems: false,
        field_weights: false,
        trigram: false,
    };

    /// Whether any switch changes the query.
    pub fn is_off(&self) -> bool {
        *self == Self::OFF
    }

    /// Whether the Analyse writes the stems table (the stems, and the section path the weights use).
    pub fn wants_stems_table(&self) -> bool {
        self.french_stems || self.field_weights
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

/// The methods kept by the measures of lot B (`docs/RETRIEVAL-QUALITY.md`, section 12): the phrases. They
/// raised the recall of the exam by 0.059 on what the product sends (0.784 to 0.843, the `identifier` family by
/// 0.273) and the planted fact found by 0.286, with no family lower; on the library of 1 007 documents the
/// recall rose by 0.047. The synonyms, the content words, the near group, the stems, the weights and the
/// trigrams did not raise anything that the phrases did not, or made a family fall: they stay off.
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

/// Which full-text table an expression is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `chunks_fts`: the passage as written.
    Plain,
    /// `chunks_lex`: the stems of the passage, and with `weighted` its section path counts more.
    Stems { weighted: bool },
    /// `chunks_tri`: the passage as characters, to find a piece of a word or of a code.
    Trigram,
}

/// One expression and the table it is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tier {
    pub source: Source,
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

// ---------------------------------------------------------------------------------------------
// Stems
// ---------------------------------------------------------------------------------------------

/// The stem of a word under the French Snowball rules, without accents. A word with a digit in it (a code,
/// a date, an amount) is left alone: only its case and accents are folded.
pub fn stem_word(word: &str) -> String {
    let lower = word.nfc().collect::<String>().to_lowercase();
    if lower.chars().any(|c| c.is_ascii_digit()) {
        return fold_word(&lower);
    }
    let stemmer = rust_stemmers::Stemmer::create(rust_stemmers::Algorithm::French);
    fold_word(&stemmer.stem(&lower))
}

/// The text as the stems table stores it: its words stemmed, one space apart.
pub fn stem_text(text: &str) -> String {
    text.nfc()
        .collect::<String>()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|piece| !piece.is_empty())
        .map(stem_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn quoted(words: &[String]) -> String {
    format!("\"{}\"", words.join(" "))
}

/// How many positions apart content words may stand for the `near` tier.
const NEAR_DISTANCE: usize = 10;
/// A `near` group of more than this many words matches almost nothing.
const NEAR_MAX_WORDS: usize = 5;
/// How much a word in the section path weighs against the same word in the body.
pub const SECTION_WEIGHT: f64 = 4.0;

/// The expressions for `question` under `options`. With every switch off the one tier is the OR of the
/// words of four letters or more, over the passage as written, as it always was.
pub fn plan(question: &str, options: &LexicalOptions, lexicon: &Lexicon) -> LexicalPlan {
    let all_words = words_of(question);
    let is_frame = |word: &Word| lexicon.frame.contains(&fold_word(&word.text));
    let wanted = |word: &Word| {
        word.content && (options.content_terms == ContentTerms::Off || !is_frame(word))
    };
    let kept: Vec<&Word> = all_words.iter().filter(|word| wanted(word)).collect();

    let stems = options.wants_stems_table();
    let stem_source = Source::Stems {
        weighted: options.field_weights,
    };

    // The expression for one table: `stemmed` writes every word as its stem and names the stem columns.
    // The pieces are alternatives of one `OR`, not tiers one after the other: the ranking adds up what a
    // passage holds, each piece weighing what its rarity makes it weigh, so an identifier or a rare word
    // beats a phrase every passage of a template repeats (a first version put the phrases first and lost
    // the rare word: `docs/RETRIEVAL-QUALITY.md`, section 12).
    let expression_for = |stemmed: bool| -> Option<String> {
        let term = |word: &str| -> String {
            if stemmed {
                stem_word(word)
            } else {
                word.to_string()
            }
        };
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
                if wanted(&pair[0]) && wanted(&pair[1]) {
                    phrases.push(vec![pair[0].text.clone(), pair[1].text.clone()]);
                }
            }
            for phrase in phrases {
                let phrase: Vec<String> = phrase.iter().map(|w| term(w)).collect();
                add(&mut alternatives, quoted(&phrase));
            }
        }

        if options.near && (2..=NEAR_MAX_WORDS).contains(&kept.len()) {
            let group: Vec<String> = kept
                .iter()
                .map(|w| format!("\"{}\"", term(&w.text)))
                .collect();
            add(
                &mut alternatives,
                format!("NEAR({}, {NEAR_DISTANCE})", group.join(" ")),
            );
        }

        // The words are never de-duplicated: with every switch off this is the expression it always was.
        for word in &kept {
            alternatives.push(format!("\"{}\"", term(&word.text)));
        }
        if options.thesaurus && !lexicon.thesaurus.is_empty() {
            let folded: Vec<String> = all_words.iter().map(|w| fold_word(&w.text)).collect();
            for equivalent in lexicon.thesaurus.equivalents(&folded) {
                let phrase: Vec<String> = equivalent.iter().map(|w| term(w)).collect();
                add(&mut alternatives, quoted(&phrase));
            }
        }
        if alternatives.is_empty() {
            return None;
        }
        let joined = alternatives.join(" OR ");
        Some(match (stemmed, options.field_weights) {
            (false, _) => joined,
            (true, true) => format!("{{section stems}} : ({joined})"),
            (true, false) => format!("{{stems}} : ({joined})"),
        })
    };

    // The passage as written comes first. The stems are a second tier: they fill the places the exact words
    // left empty, and never push an exact match out (stems first put "Martine" where "Martin" was asked:
    // `docs/RETRIEVAL-QUALITY.md`, section 12).
    let plain = expression_for(false).map(|expression| Tier {
        source: Source::Plain,
        expression,
    });
    let stemmed = if stems {
        expression_for(true).map(|expression| Tier {
            source: stem_source,
            expression,
        })
    } else {
        None
    };
    // With the weights the title and the section path outrank the body, which only the stems table can say:
    // that tier goes first. Without them the stems only fill the places the exact words left empty.
    let mut tiers = Vec::new();
    if options.field_weights {
        tiers.extend(stemmed);
        tiers.extend(plain);
    } else {
        tiers.extend(plain);
        tiers.extend(stemmed);
    }

    if options.trigram {
        // The words and the codes as the text has them, folded to lower case by the table itself. A trigram
        // needs three characters at least; the words kept have four.
        let mut pieces: Vec<String> = kept.iter().map(|w| format!("\"{}\"", w.text)).collect();
        for code in question.split_whitespace().filter_map(|piece| {
            let piece = piece.trim_matches(|c: char| !c.is_alphanumeric());
            let coded = piece.chars().any(|c| c.is_ascii_digit())
                && piece.chars().any(|c| matches!(c, '-' | '.' | '/' | '_'));
            (coded && piece.chars().count() >= 4).then(|| format!("\"{piece}\""))
        }) {
            if !pieces.contains(&code) {
                pieces.push(code);
            }
        }
        if !pieces.is_empty() {
            tiers.push(Tier {
                source: Source::Trigram,
                expression: pieces.join(" OR "),
            });
        }
    }
    LexicalPlan { tiers }
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

/// The words the content-terms switch would leave out of this question, for the diagnostics and the tests.
pub fn frame_words_of(question: &str, lexicon: &Lexicon) -> BTreeMap<usize, String> {
    words_of(question)
        .into_iter()
        .enumerate()
        .filter(|(_, word)| word.content && lexicon.frame.contains(&fold_word(&word.text)))
        .map(|(position, word)| (position, word.text))
        .collect()
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
    fn the_near_group_holds_two_to_five_content_words() {
        let options = LexicalOptions {
            near: true,
            ..LexicalOptions::OFF
        };
        let found = tiers("annual fee Camille", options);
        assert_eq!(
            found,
            vec!["NEAR(\"annual\" \"Camille\", 10) OR \"annual\" OR \"Camille\"".to_string()]
        );
        let alone = tiers("Camille", options);
        assert_eq!(
            alone,
            vec!["\"Camille\"".to_string()],
            "one word has nothing near it"
        );
    }

    #[test]
    fn the_frame_words_are_left_out_only_when_asked() {
        let on = LexicalOptions {
            content_terms: ContentTerms::On,
            ..LexicalOptions::OFF
        };
        assert_eq!(
            tiers("Which documents mention Camille", on),
            vec!["\"Camille\"".to_string()]
        );
        assert_eq!(
            tiers("Which documents mention Camille", LexicalOptions::OFF),
            vec!["\"Which\" OR \"documents\" OR \"mention\" OR \"Camille\"".to_string()]
        );
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
    fn the_stem_of_a_plural_and_of_its_singular_is_the_same_and_a_code_is_left_alone() {
        assert_eq!(stem_word("courriers"), stem_word("courrier"));
        assert_eq!(stem_word("factures"), stem_word("facture"));
        assert_eq!(
            stem_word("Facture"),
            stem_word("facture"),
            "case does not count"
        );
        assert_eq!(stem_word("FAC"), "fac");
        assert_eq!(stem_word("0114"), "0114");
        assert_eq!(stem_word("756"), "756");
        assert_eq!(stem_text("Factures annuelles"), "factur annuel");
    }

    #[test]
    fn the_stems_follow_the_exact_words_and_the_weights_put_the_section_path_first() {
        let stems = LexicalOptions {
            french_stems: true,
            ..LexicalOptions::OFF
        };
        let weighted = LexicalOptions {
            field_weights: true,
            ..stems
        };
        let found = plan("factures annuelles", &stems, &lexicon());
        assert_eq!(found.tiers.len(), 2);
        assert_eq!(found.tiers[0].source, Source::Plain);
        assert_eq!(found.tiers[0].expression, "\"factures\" OR \"annuelles\"");
        assert_eq!(found.tiers[1].source, Source::Stems { weighted: false });
        assert_eq!(
            found.tiers[1].expression,
            "{stems} : (\"factur\" OR \"annuel\")"
        );
        let found = plan("factures annuelles", &weighted, &lexicon());
        assert_eq!(found.tiers[0].source, Source::Stems { weighted: true });
        assert!(found.tiers[0].expression.starts_with("{section stems} : ("));
        assert_eq!(found.tiers[1].source, Source::Plain);
    }

    #[test]
    fn the_trigram_tier_comes_last_and_carries_the_words_and_the_codes_as_written() {
        let on = LexicalOptions {
            trigram: true,
            ..LexicalOptions::OFF
        };
        let found = plan("invoice FAC-2026-014 total", &on, &lexicon());
        assert_eq!(found.tiers.len(), 2);
        assert_eq!(found.tiers[0].source, Source::Plain);
        assert_eq!(found.tiers[1].source, Source::Trigram);
        assert!(found.tiers[1].expression.contains("\"FAC-2026-014\""));
        assert!(found.tiers[1].expression.contains("\"invoice\""));
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
            near: true,
            content_terms: ContentTerms::On,
            thesaurus: true,
            ..LexicalOptions::OFF
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
