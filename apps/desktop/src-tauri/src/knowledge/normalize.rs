//! One place that turns text into comparable forms.
//!
//! Three kinds of text are compared in this product and each has its own normal form:
//!
//! - **free text and file names** (`fold`): lowercase, accents removed, one code point per accent
//!   whatever the platform stored. This is the form the product has always compared file names
//!   and tabular values in; `file_reference::fold_text` delegates here.
//! - **names of people and organisations** (`normalize_name`): `fold`, plus the structure a name
//!   has - tokens, titles, particles, initials, an order-free key - so "Jean Dupont",
//!   "DUPONT, Jean" and "Jean-Dupont" are recognisably the same name.
//! - **identifiers** (`normalize_identifier`): letters and digits only, so "FR76 3000" and
//!   "fr76-3000" collide.
//!
//! Everything here is a pure function of its arguments. The words that depend on a language or a
//! profession (titles, particles, stop-words) are never literals in this file: the caller passes a
//! [`TitleSet`], which lot 3 fills from the lexicon packs. Nothing here touches a database, a
//! file or a log.

use std::collections::HashSet;

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Lowercase, accents removed, one canonical form for every spelling of an accent. What she types
/// and what the folder holds are compared through this, so an accented "Esaie", "Esaie" and
/// "ESAIE" are one name, and a Mac's two-code-point accent is the same as Windows's one.
///
/// This is the body `file_reference::fold_text` has always had, moved here unchanged.
pub fn fold(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Appends `c` (already lowercase) to `out` as a letter or digit, expanding the two ligatures
/// that have no decomposition. Anything else is not part of a word and is dropped.
fn push_letter(out: &mut String, c: char) {
    match c {
        '\u{153}' => out.push_str("oe"),
        '\u{e6}' => out.push_str("ae"),
        c if c.is_alphanumeric() => out.push(c),
        _ => {}
    }
}

/// `text` as the letters and digits of one word, lowercase, with no accent except that a cedilla
/// has already done its work: `c` + cedilla is `s`, because the sound is the one thing a cedilla
/// carries and `fold` throws it away.
///
/// This is the form the phonetic encoders read. It is not used for exact comparison - `fold` is -
/// so a name typed without its cedilla still matches the one that has it.
pub fn sound_form(text: &str) -> String {
    let mut out = String::new();
    for c in text.nfd() {
        if c == '\u{327}' {
            if out.ends_with('c') {
                out.pop();
                out.push('s');
            }
            continue;
        }
        if is_combining_mark(c) {
            continue;
        }
        for lower in c.to_lowercase() {
            push_letter(&mut out, lower);
        }
    }
    out
}

/// `fold` of one word, ligatures expanded, punctuation gone.
fn fold_word(text: &str) -> String {
    let mut out = String::new();
    for c in fold(text).chars() {
        push_letter(&mut out, c);
    }
    out
}

/// Every mark that separates "l" from "Hopital" in "l'Hopital" - and marks the left side as an
/// elided article rather than an initial.
fn is_apostrophe(c: char) -> bool {
    matches!(
        c,
        '\'' | '\u{2019}' | '\u{2018}' | '\u{2032}' | '\u{2bc}' | '`' | '\u{b4}'
    )
}

/// One word of the original text, before any folding.
struct RawToken {
    /// The characters as written, accents (and their combining marks) included.
    written: String,
    /// Directly followed by an apostrophe: "l" in "l'Hopital", "d" in "d'Aubigne".
    elided: bool,
    /// Starts with a capital letter in the original text.
    capitalised: bool,
}

/// Splits on everything that is not a letter, a digit or an accent: spaces, commas, hyphens,
/// underscores, dots, slashes and any other punctuation. An apostrophe splits too, and remembers
/// that it did.
fn raw_tokens(text: &str) -> Vec<RawToken> {
    fn flush(current: &mut String, elided: bool, tokens: &mut Vec<RawToken>) {
        if current.is_empty() {
            return;
        }
        let written = std::mem::take(current);
        if fold_word(&written).is_empty() {
            return;
        }
        let capitalised = written.nfd().next().is_some_and(char::is_uppercase);
        tokens.push(RawToken {
            written,
            elided,
            capitalised,
        });
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        // An apostrophe is checked first: U+02BC is a letter as far as Unicode is concerned.
        if is_apostrophe(c) {
            flush(&mut current, true, &mut tokens);
        } else if c.is_alphanumeric() || is_combining_mark(c) {
            current.push(c);
        } else {
            flush(&mut current, false, &mut tokens);
        }
    }
    flush(&mut current, false, &mut tokens);
    tokens
}

/// The language-dependent words a name is read with. Titles, particles and stop-words are data,
/// never literals in Rust: lot 3 builds one of these from the active lexicon packs.
///
/// Every word is folded and stripped of punctuation on the way in, so "Dr.", "DR" and "dr" are
/// one entry and a pack may write a title the way people do.
#[derive(Debug, Clone, Default)]
pub struct TitleSet {
    titles: HashSet<String>,
    weak_titles: HashSet<String>,
    particles: HashSet<String>,
    stop_words: HashSet<String>,
}

impl TitleSet {
    pub fn new() -> Self {
        Self::default()
    }

    fn collect<I, S>(words: I) -> HashSet<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        words
            .into_iter()
            .map(|word| fold_word(word.as_ref()))
            .filter(|word| !word.is_empty())
            .collect()
    }

    /// Titles that are never anything else: a "Docteur" in front of a name is a title wherever it
    /// stands.
    pub fn with_titles<I, S>(mut self, words: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.titles = Self::collect(words);
        self
    }

    /// Titles that are also ordinary words or initials (a pronoun, a middle initial). They count
    /// as titles only at the start of a name, and in `Auto` mode only when written with a capital
    /// and followed by a capitalised word. See [`WeakTitles`].
    pub fn with_weak_titles<I, S>(mut self, words: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.weak_titles = Self::collect(words);
        self
    }

    /// Small words inside a name that carry no identity: kept as tokens, left out of the
    /// order-free key and of the sound of the name.
    pub fn with_particles<I, S>(mut self, words: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.particles = Self::collect(words);
        self
    }

    /// Words that, found in a file name, say it is not a name: months, "invoice", "scan".
    pub fn with_stop_words<I, S>(mut self, words: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.stop_words = Self::collect(words);
        self
    }

    pub fn is_particle(&self, folded_word: &str) -> bool {
        self.particles.contains(folded_word)
    }
}

/// Whether an ambiguous title ("Me", "M") is read as a title. The extractor, which sees the
/// surrounding sentence, picks; `normalize_name` uses `Auto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeakTitles {
    /// Only when written with a capital, at the start, and followed by a capitalised word:
    /// "Me Martin" is a title, "me" and "Jean M" are not.
    Auto,
    Always,
    Never,
}

/// A name in every comparable form. `display` is the text exactly as it was written; every other
/// field is derived from it and none is ever shown to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedName {
    pub display: String,
    /// Folded words in order, titles removed. Particles, elided articles and initials stay.
    pub tokens: Vec<String>,
    /// The titles that were removed, folded, in order.
    pub titles: Vec<String>,
    /// `tokens` separated by one space.
    pub joined: String,
    /// `tokens` with no separator: "du pont" and "dupont" share it. Two names that only share
    /// this are a *possible* match, never an exact one.
    pub compact: String,
    /// `joined` without the elided articles: the "hopital" of "l'Hopital".
    pub without_elision: String,
    /// The words that carry identity, sorted and separated by one space: no title, no particle,
    /// no elided article, no initial. "Jean Dupont", "Dupont, Jean" and "Jean-Dupont" share it.
    pub token_set_key: String,
    /// Single-letter words ("j" in "J. Dupont"), in order. An initial is never part of the key.
    pub initials: Vec<String>,
    /// What the phonetic encoder reads: every word that is not a title, a particle or an elided
    /// article, in order, initials included, in [`sound_form`].
    pub sounds_like: Vec<String>,
}

/// `normalize_name_with(text, titles, WeakTitles::Auto)`.
pub fn normalize_name(text: &str, titles: &TitleSet) -> NormalizedName {
    normalize_name_with(text, titles, WeakTitles::Auto)
}

pub fn normalize_name_with(text: &str, set: &TitleSet, weak: WeakTitles) -> NormalizedName {
    let raw = raw_tokens(text);
    let folded: Vec<String> = raw.iter().map(|token| fold_word(&token.written)).collect();

    let mut title_flags = vec![false; raw.len()];
    let mut still_leading = true;
    for index in 0..raw.len() {
        let word = &folded[index];
        let strong = set.titles.contains(word);
        let weak_title = set.weak_titles.contains(word)
            && still_leading
            && match weak {
                WeakTitles::Always => true,
                WeakTitles::Never => false,
                WeakTitles::Auto => {
                    raw[index].capitalised && raw.get(index + 1).is_some_and(|n| n.capitalised)
                }
            };
        if strong || weak_title {
            title_flags[index] = true;
        } else {
            still_leading = false;
        }
    }

    let mut tokens = Vec::new();
    let mut titles = Vec::new();
    let mut without_elision = Vec::new();
    let mut key_words = Vec::new();
    let mut initials = Vec::new();
    let mut sounds_like = Vec::new();

    for index in 0..raw.len() {
        let word = &folded[index];
        if title_flags[index] {
            titles.push(word.clone());
            continue;
        }
        tokens.push(word.clone());
        if !raw[index].elided {
            without_elision.push(word.clone());
        }
        if raw[index].elided || set.particles.contains(word) {
            continue;
        }
        sounds_like.push(sound_form(&raw[index].written));
        if word.chars().count() == 1 && word.chars().all(char::is_alphabetic) {
            initials.push(word.clone());
        } else {
            key_words.push(word.clone());
        }
    }

    key_words.sort();
    NormalizedName {
        display: text.to_string(),
        joined: tokens.join(" "),
        compact: tokens.concat(),
        without_elision: without_elision.join(" "),
        token_set_key: key_words.join(" "),
        tokens,
        titles,
        initials,
        sounds_like,
    }
}

/// An identifier with everything that is formatting removed: letters and digits only, accents
/// folded, upper case. "FR76 3000", "fr76-3000" and "FR76.3000" collide. Whether the collision
/// means "the same identifier" is the scheme's decision (an IBAN: yes; a phone number with and
/// without the country code: no), not this function's.
pub fn normalize_identifier(text: &str) -> String {
    let mut out = String::new();
    for c in fold(text).chars() {
        let mut letters = String::new();
        push_letter(&mut letters, c);
        out.extend(letters.chars().flat_map(char::to_uppercase));
    }
    out
}

/// The name-shaped words of a sanitised file name, in order, as runs of consecutive words.
///
/// "Dupont_Jean_2026-03" gives `[["dupont", "jean"], ["jean", "dupont"]]`: words with a digit,
/// stop-words and titles end a run and are dropped, and a run of exactly two words is offered in
/// both orders because a file name does not say which is the surname. Longer runs keep their
/// order. `stem` is the name without its extension.
pub fn filename_name_candidates(stem: &str, set: &TitleSet) -> Vec<Vec<String>> {
    let mut runs: Vec<Vec<String>> = Vec::new();
    let mut run: Vec<String> = Vec::new();

    for token in raw_tokens(stem) {
        let word = fold_word(&token.written);
        let dropped = word.chars().any(|c| c.is_numeric())
            || set.stop_words.contains(&word)
            || set.titles.contains(&word);
        if dropped {
            if !run.is_empty() {
                runs.push(std::mem::take(&mut run));
            }
        } else {
            run.push(word);
        }
    }
    if !run.is_empty() {
        runs.push(run);
    }

    let mut candidates: Vec<Vec<String>> = Vec::new();
    for run in runs {
        let reversed: Vec<String> = run.iter().rev().cloned().collect();
        let both_orders = run.len() == 2;
        for candidate in std::iter::once(run).chain(both_orders.then_some(reversed)) {
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles() -> TitleSet {
        TitleSet::new()
            .with_titles(["Dr.", "Docteur", "Mme"])
            .with_weak_titles(["Me", "M."])
            .with_particles(["de", "du", "d"])
            .with_stop_words(["scan", "invoice"])
    }

    #[test]
    fn fold_matches_the_function_it_replaces() {
        assert_eq!(fold("Cr\u{e8}me BR\u{db}L\u{c9}E"), "creme brulee");
        assert_eq!(fold("e\u{301}"), "e");
        assert_eq!(fold("plain"), "plain");
    }

    #[test]
    fn a_name_in_any_order_or_case_has_one_key() {
        let set = titles();
        let expected = normalize_name("Jean Dupont", &set).token_set_key;
        for spelling in [
            "jean dupont",
            "JEAN DUPONT",
            "Jean  Dupont",
            "Jean-Dupont",
            "Dupont, Jean",
            "Dr Jean Dupont",
        ] {
            assert_eq!(
                normalize_name(spelling, &set).token_set_key,
                expected,
                "{spelling}"
            );
        }
        assert_eq!(normalize_name("Jean Dupont", &set).display, "Jean Dupont");
    }

    #[test]
    fn a_weak_title_needs_a_capital_and_a_capitalised_word_after_it() {
        let set = titles();
        assert_eq!(normalize_name("Me Martin", &set).titles, ["me"]);
        assert_eq!(
            normalize_name("me martin", &set).titles,
            Vec::<String>::new()
        );
        assert_eq!(
            normalize_name("Jean M Martin", &set).titles,
            Vec::<String>::new()
        );
        assert_eq!(
            normalize_name_with("me martin", &set, WeakTitles::Always).titles,
            ["me"]
        );
        assert_eq!(
            normalize_name_with("Me Martin", &set, WeakTitles::Never).titles,
            Vec::<String>::new()
        );
    }

    #[test]
    fn an_apostrophe_keeps_both_the_elided_and_the_bare_form() {
        let set = titles();
        let name = normalize_name("l\u{2019}H\u{f4}pital", &set);
        assert_eq!(name.joined, "l hopital");
        assert_eq!(name.without_elision, "hopital");
        assert_eq!(name.token_set_key, "hopital");
        assert_eq!(name.initials, Vec::<String>::new());
    }

    #[test]
    fn the_same_text_in_nfc_and_nfd_is_one_name() {
        let set = titles();
        let composed = normalize_name("Ren\u{e9} Fran\u{e7}ois", &set);
        let decomposed = normalize_name("Rene\u{301} Franc\u{327}ois", &set);
        assert_eq!(composed.token_set_key, decomposed.token_set_key);
        assert_eq!(composed.sounds_like, decomposed.sounds_like);
        assert_eq!(composed.sounds_like[1], "fransois");
    }

    #[test]
    fn identifiers_lose_their_formatting_only() {
        assert_eq!(normalize_identifier("fr76 3000-4028.37"), "FR763000402837");
        assert_eq!(
            normalize_identifier("FR76 3000 4028 37"),
            normalize_identifier("fr7630004028 37")
        );
        assert_eq!(normalize_identifier("+33 6 12 34 56 78"), "33612345678");
    }

    #[test]
    fn a_file_name_gives_ordered_candidate_runs() {
        let set = titles();
        assert_eq!(
            filename_name_candidates("Dupont_Jean_2026-03", &set),
            [["dupont", "jean"], ["jean", "dupont"]]
        );
        assert_eq!(
            filename_name_candidates("scan_Martin_invoice_Durand-Paul", &set),
            [
                vec!["martin".to_string()],
                vec!["durand".to_string(), "paul".to_string()],
                vec!["paul".to_string(), "durand".to_string()],
            ]
        );
        assert!(filename_name_candidates("2026-03-12", &set).is_empty());
    }

    #[test]
    fn normalising_twice_changes_nothing() {
        let set = titles();
        for text in [
            "Dr. Jean-Marie d'Aubign\u{e9}",
            "DUPONT, J.",
            "  x  ",
            "o'brien",
        ] {
            let once = normalize_name(text, &set);
            let twice = normalize_name(&once.joined, &set);
            assert_eq!(once.joined, twice.joined, "{text}");
        }
    }
}
