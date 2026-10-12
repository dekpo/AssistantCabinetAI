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
pub(crate) fn fold_word(text: &str) -> String {
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
    let chars: Vec<char> = text.chars().collect();
    for (position, &c) in chars.iter().enumerate() {
        // An apostrophe is checked first: U+02BC is a letter as far as Unicode is concerned.
        if is_apostrophe(c) {
            if apostrophe_joins_a_name(&current, chars.get(position + 1).copied()) {
                // "D'Amico": the apostrophe is inside the surname, which stays one word.
                current.push('\'');
            } else {
                flush(&mut current, true, &mut tokens);
            }
        } else if c.is_alphanumeric() || is_combining_mark(c) {
            current.push(c);
        } else {
            flush(&mut current, false, &mut tokens);
        }
    }
    flush(&mut current, false, &mut tokens);
    tokens
}

/// The capital letters that open a surname with an apostrophe: `D'Amico`, `N'Diaye`, `O'Brien`.
/// `L'` is left out on purpose: `L'Hotel de Ville` is an article and a noun.
const NAME_ELISION_LETTERS: [char; 3] = ['D', 'N', 'O'];

/// Whether the apostrophe that follows `before` (the letters of the word read so far) and precedes
/// `after` belongs to a surname (`D'Amico`) and not to an elided article (`l'Hopital`,
/// `d'Aubigne`).
///
/// The rule is about how the name is *written*: a single capital letter, an apostrophe and a capital
/// letter. An article before a noun is written in lower case (`d'Aubigne`), or is followed by a
/// lower case word at the start of a sentence (`D'apres`, `N'oubliez pas`). The one function the
/// name normaliser, the text extractor and the question reader all ask, so the three agree.
pub(crate) fn apostrophe_joins_a_name(before: &str, after: Option<char>) -> bool {
    let mut letters = before.chars();
    let (Some(first), None) = (letters.next(), letters.next()) else {
        return false;
    };
    NAME_ELISION_LETTERS.contains(&first) && after.is_some_and(|next| next.is_uppercase())
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

    /// A title that is never anything else ("Docteur"), by its folded word.
    pub fn is_title(&self, folded_word: &str) -> bool {
        self.titles.contains(folded_word)
    }

    /// A title that is also an ordinary word or an initial ("Me", "M"), by its folded word.
    pub fn is_weak_title(&self, folded_word: &str) -> bool {
        self.weak_titles.contains(folded_word)
    }

    /// A word that says a text is not a name: a month, a document word, a null-like value.
    pub fn is_stop_word(&self, folded_word: &str) -> bool {
        self.stop_words.contains(folded_word)
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
    /// article, in order, initials included, in [`sound_form`]. A name made only of titles and
    /// particles keeps them, so that no name has an empty sound.
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
        // An elided article is one letter before an apostrophe ("l'", "d'"). A longer word before
        // one is a name ("Floc'h"), and dropping it would leave nothing to encode.
        let elided = raw[index].elided && word.chars().count() == 1;
        if !elided {
            without_elision.push(word.clone());
        }
        if elided || set.particles.contains(word) {
            continue;
        }
        sounds_like.push(sound_form(&raw[index].written));
        if word.chars().count() == 1 && word.chars().all(char::is_alphabetic) {
            initials.push(word.clone());
        } else {
            key_words.push(word.clone());
        }
    }

    // A name made only of particles or titles ("Du", "Maitre" alone) still has a sound.
    if sounds_like.is_empty() {
        sounds_like = raw
            .iter()
            .enumerate()
            .filter(|(index, _)| !title_flags[*index])
            .map(|(_, token)| sound_form(&token.written))
            .collect();
    }
    if sounds_like.is_empty() {
        sounds_like = raw.iter().map(|token| sound_form(&token.written)).collect();
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

/// A person's name written surname first, then one letter: `Dupont J`, `DUPONT J.`, `de Vries A`.
/// A table of appointments or a roll call writes names that way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurnameInitial {
    /// The surname as folded words, the particles in front of it kept (`de`, `vries`).
    pub surname: Vec<String>,
    /// The initial, folded.
    pub initial: String,
}

impl SurnameInitial {
    /// The alias every full name carrying this surname and this initial has: `j dupont`.
    pub fn initial_surname_key(&self) -> String {
        format!("{} {}", self.initial, self.surname.join(" "))
    }
}

/// Reads the form `Surname Initial`. **The one place that decides it** (aliases and resolution
/// both ask here), and only for a name that is a person's: the caller checks the type.
///
/// The rule, narrow on purpose because a wrong reading would put a given name where a surname
/// belongs:
///
/// - the last word is one letter (a period after it is no different: `Dupont J.`);
/// - what comes before it is exactly one word of identity, optionally preceded by particles
///   (`Dupont`, `de Vries`), and that word is not itself a single letter, so `A B` and `J. P.`
///   stay as they are;
/// - so `J. Dupont` (Initial Surname, the form handled since lot 3) is never this form, nor is
///   `Jean Dupont J`, `Jean-Pierre Dupont J` or a name of one word.
///
/// Titles are already out of `name.tokens`, so `Dr Martin A` is read as `Martin A`.
pub fn surname_then_initial(name: &NormalizedName, set: &TitleSet) -> Option<SurnameInitial> {
    let (initial, before) = name.tokens.split_last()?;
    if initial.chars().count() != 1 || !initial.chars().all(char::is_alphabetic) {
        return None;
    }
    let (surname_word, particles) = before.split_last()?;
    if surname_word.chars().count() < 2 || set.is_particle(surname_word) {
        return None;
    }
    if !particles.iter().all(|word| set.is_particle(word)) {
        return None;
    }
    Some(SurnameInitial {
        surname: before.to_vec(),
        initial: initial.clone(),
    })
}

/// Whether a word is written in capitals only (two letters or more): `DUPONT`, `VAN DEN BERG`.
fn is_shouting(word: &str) -> bool {
    let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase())
}

/// The span of the first run of words written in capitals, when a word with lower case letters
/// follows it: the administrative way of writing a person, surname first (`DUPONT Jean`,
/// `VAN DEN BERG Anne`). `(start, end)` are positions among the words, `end` excluded.
fn surname_first_run(words: &[&str]) -> Option<(usize, usize)> {
    let start = words.iter().position(|word| is_shouting(word))?;
    let mut end = start;
    while end < words.len() && is_shouting(words[end]) {
        end += 1;
    }
    let mixed_after = words[end..]
        .iter()
        .any(|word| word.chars().any(char::is_lowercase));
    mixed_after.then_some((start, end))
}

/// Whether a person is written surname first, in capitals (`DUPONT Jean`). The order of the words
/// of any other spelling says nothing: `Roy Lucas` can be the surname Roy or the surname Lucas.
pub fn writes_surname_first(text: &str) -> bool {
    let words: Vec<&str> = text.split_whitespace().collect();
    surname_first_run(&words).is_some()
}

/// The name with the given names first, when it is written surname first in capitals
/// (`LUCAS Roy` is `Roy LUCAS`: the person whose surname is Lucas). Any other spelling is returned as
/// it is. Titles in front are kept in front.
pub fn given_name_first(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    match surname_first_run(&words) {
        Some((start, end)) => words[..start]
            .iter()
            .chain(&words[end..])
            .chain(&words[start..end])
            .copied()
            .collect::<Vec<_>>()
            .join(" "),
        None => text.to_string(),
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

    fn surname_initial(text: &str) -> Option<(Vec<String>, String)> {
        let set = titles();
        surname_then_initial(&normalize_name(text, &set), &set)
            .map(|form| (form.surname, form.initial))
    }

    #[test]
    fn a_surname_then_one_letter_is_read_as_a_surname_and_its_initial() {
        let words = |list: &[&str]| list.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        for text in [
            "Dupont J",
            "Dupont J.",
            "DUPONT J",
            "dupont, j.",
            "Dr Dupont J",
        ] {
            assert_eq!(
                surname_initial(text),
                Some((words(&["dupont"]), "j".to_string())),
                "{text}"
            );
        }
        // Particles stay with the surname.
        assert_eq!(
            surname_initial("de Vries A"),
            Some((words(&["de", "vries"]), "a".to_string()))
        );
        // The accent is folded away like in every name.
        assert_eq!(
            surname_initial("\u{c9}tienne \u{c9}"),
            Some((words(&["etienne"]), "e".to_string()))
        );
    }

    #[test]
    fn the_other_forms_are_not_surname_then_initial() {
        for text in [
            // Initial then surname: the other way round, handled since lot 3.
            "J. Dupont",
            "J Dupont",
            // A full name, a hyphenated given name, one word, nothing.
            "Jean Dupont",
            "Jean-Pierre Dupont",
            "Dupont",
            "J",
            "",
            // Two single letters stay as they are, with or without periods.
            "A B",
            "J. P.",
            "Dupont J P",
            // More than one word of identity before the letter: a middle initial, not this form.
            "Jean Dupont J",
            // The word before the letter is only a particle.
            "de A",
            // A trailing digit is not an initial.
            "Dupont 7",
        ] {
            assert_eq!(surname_initial(text), None, "{text:?}");
        }
    }

    #[test]
    fn the_initial_form_and_the_surname_initial_form_never_read_alike() {
        let set = titles();
        let after = normalize_name("Dupont J.", &set);
        let before = normalize_name("J. Dupont", &set);
        assert_ne!(after.joined, before.joined);
        assert!(surname_then_initial(&after, &set).is_some());
        assert!(surname_then_initial(&before, &set).is_none());
        assert_eq!(
            surname_then_initial(&after, &set)
                .expect("the form")
                .initial_surname_key(),
            before.joined,
            "the alias a full name carries for its initial is the other order"
        );
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
