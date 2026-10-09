//! What a name sounds like, so "Dupont", "Dupond" and "Dupon" - or a name a speech engine
//! misheard - land on one key.
//!
//! A key is a *hint*, never an identity. Two names with the same key are a possible match: the
//! resolver (lot 3) offers them as a "did you mean" and never merges them. That is why the rules
//! below may be generous: a false collision costs one question to the user, a missed one costs a
//! name nobody finds.
//!
//! The encoder is a port ([`PhoneticEncoder`]). [`FrenchPhonetic`] is the pilot's; [`EnglishPhonetic`]
//! is a conventional Soundex. A better engine (a trained grapheme-to-phoneme model) would implement
//! the same trait, and bumping `version` makes lot 3 recompute every stored key.
//!
//! **The key alphabet is plain ASCII** so a key can be stored, compared and indexed anywhere:
//!
//! ```text
//! consonants   B D F G J K L M N P R S T V Z   X = the "ch" sound, J = the "j" of "jour"
//! vowels       A E I O U   Y = the French "u"   W = "eu"   UA = "oi"
//! nasals       a o e       lowercase of the vowel they come from: "an" -> a, "on" -> o, "in" -> e
//! digits       kept as they are
//! ```
//!
//! The rules that most often decide a collision are in `encode_french`; the table of names that
//! must and must not collide is `tests/fixtures/knowledge/phonetic-fr.json`, which the owner can
//! extend without touching Rust.

use super::normalize::{sound_form, NormalizedName};

/// Turns one normalised word into a key. Implementations are pure and deterministic.
pub trait PhoneticEncoder: Send + Sync {
    /// Stable identifier of the algorithm and its language, for example `fr-rules`.
    fn id(&self) -> &str;

    /// Raised whenever the output for some input changes. `id` and `version` together are what
    /// `kb_meta.phonetic_version` stores; when they differ from the stored value, every stored key
    /// is recomputed.
    fn version(&self) -> u32;

    /// The key of one word. Accepts a folded token or the [`sound_form`] of one; a word with no
    /// letter or digit gives an empty key.
    fn encode_token(&self, normalized_token: &str) -> String;

    /// The key of a whole name: each word that is not a title, particle or elided article,
    /// encoded and joined with `-`. Initials are encoded like any other word.
    fn encode_name(&self, name: &NormalizedName) -> String {
        name.sounds_like
            .iter()
            .map(|word| self.encode_token(word))
            .filter(|key| !key.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }

    /// `id:version`, the single string kept in `kb_meta`.
    fn signature(&self) -> String {
        format!("{}:{}", self.id(), self.version())
    }
}

/// The encoder for a locale. French is the pilot's language and the fallback, as everywhere else
/// in the client.
pub fn encoder_for_locale(locale: &str) -> Box<dyn PhoneticEncoder> {
    if locale.to_ascii_lowercase().starts_with("en") {
        Box::new(EnglishPhonetic)
    } else {
        Box::new(FrenchPhonetic)
    }
}

/// Rule-based French grapheme-to-sound, pure Rust, no dependency.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrenchPhonetic;

impl PhoneticEncoder for FrenchPhonetic {
    fn id(&self) -> &str {
        "fr-rules"
    }

    fn version(&self) -> u32 {
        1
    }

    fn encode_token(&self, normalized_token: &str) -> String {
        encode_french(normalized_token)
    }
}

/// American Soundex: the first letter, then three digits. Coarse by design - it is the standard
/// key every English-speaking tool understands.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnglishPhonetic;

impl PhoneticEncoder for EnglishPhonetic {
    fn id(&self) -> &str {
        "en-soundex"
    }

    fn version(&self) -> u32 {
        1
    }

    fn encode_token(&self, normalized_token: &str) -> String {
        encode_soundex(normalized_token)
    }
}

// ----------------------------------------------------------------------------- English

fn soundex_digit(c: char) -> Option<char> {
    match c {
        'b' | 'f' | 'p' | 'v' => Some('1'),
        'c' | 'g' | 'j' | 'k' | 'q' | 's' | 'x' | 'z' => Some('2'),
        'd' | 't' => Some('3'),
        'l' => Some('4'),
        'm' | 'n' => Some('5'),
        'r' => Some('6'),
        _ => None,
    }
}

fn encode_soundex(token: &str) -> String {
    let word: Vec<char> = sound_form(token).chars().collect();
    let Some(&first) = word.first() else {
        return String::new();
    };
    if word.iter().all(char::is_ascii_digit) {
        return word.iter().collect();
    }
    let mut key = String::new();
    key.extend(first.to_uppercase());
    let mut previous = soundex_digit(first);
    for &c in &word[1..] {
        match soundex_digit(c) {
            Some(digit) => {
                if previous != Some(digit) {
                    key.push(digit);
                }
                previous = Some(digit);
            }
            // "h" and "w" do not separate two letters of the same code; a vowel does.
            None if c == 'h' || c == 'w' => {}
            None => previous = None,
        }
        if key.len() == 4 {
            break;
        }
    }
    while key.len() < 4 {
        key.push('0');
    }
    key
}

// ----------------------------------------------------------------------------- French

/// A silent "e" that is not one: "er" and "ez" at the end of a word are pronounced like "e acute",
/// which the working copy of the word writes with this character.
const SOUNDED_E: char = '\u{e9}';

const VOWELS: [char; 7] = ['a', 'e', 'i', 'o', 'u', 'y', SOUNDED_E];
const FRONT_VOWELS: [char; 4] = ['e', 'i', 'y', SOUNDED_E];

fn is_vowel(c: char) -> bool {
    VOWELS.contains(&c)
}

/// A vowel group, its sound, and whether it is a nasal (which only counts when the "n" or "m" it
/// ends in is not followed by a vowel or by another "n"/"m"). Longest groups first.
const VOWEL_RULES: &[(&str, &str, bool)] = &[
    ("eau", "O", false),
    ("oeu", "W", false),
    ("ain", "e", true),
    ("aim", "e", true),
    ("ein", "e", true),
    ("eim", "e", true),
    ("ean", "a", true),
    ("ien", "Ie", true),
    ("oin", "UAe", true),
    ("au", "O", false),
    ("ou", "U", false),
    ("oi", "UA", false),
    ("ai", "E", false),
    ("ei", "E", false),
    ("eu", "W", false),
    ("oe", "E", false),
    ("ea", "A", false),
    ("an", "a", true),
    ("am", "a", true),
    ("en", "a", true),
    ("em", "a", true),
    ("on", "o", true),
    ("om", "o", true),
    ("in", "e", true),
    ("im", "e", true),
    ("yn", "e", true),
    ("ym", "e", true),
    ("un", "e", true),
    ("um", "e", true),
];

fn single_vowel(c: char) -> Option<&'static str> {
    match c {
        'a' => Some("A"),
        'e' | SOUNDED_E => Some("E"),
        'i' | 'y' => Some("I"),
        'o' => Some("O"),
        'u' => Some("Y"),
        _ => None,
    }
}

fn starts_with_at(word: &[char], at: usize, pattern: &str) -> bool {
    let mut position = at;
    for expected in pattern.chars() {
        if word.get(position) != Some(&expected) {
            return false;
        }
        position += 1;
    }
    true
}

/// A nasal needs a consonant (or the end of the word) after its "n"/"m"; a mute "e" that was
/// stripped from the end still counts as a vowel there ("Martine" keeps its "n").
fn nasal_allowed(word: &[char], after: usize, mute_e_stripped: bool) -> bool {
    match word.get(after) {
        None => !mute_e_stripped,
        Some(&next) => !is_vowel(next) && next != 'n' && next != 'm',
    }
}

fn encode_french(token: &str) -> String {
    let letters = sound_form(token);
    if letters.is_empty() {
        return String::new();
    }
    if letters.chars().all(|c| c.is_ascii_digit()) {
        return letters;
    }
    let mut word: Vec<char> = letters.chars().collect();

    // Endings first: they decide what is silent.
    if word.len() >= 4 && word.ends_with(&['a', 'u', 'l', 't']) {
        word.truncate(word.len() - 4);
        word.push('o');
    }
    if word.len() >= 5 && word.ends_with(&['e', 'r']) {
        word.truncate(word.len() - 2);
        word.push(SOUNDED_E);
    }
    if word.len() >= 3 && word.ends_with(&['e', 'z']) {
        word.truncate(word.len() - 2);
        word.push(SOUNDED_E);
    }
    let mut mute_e_stripped = false;
    if word.len() > 2 && word.last() == Some(&'e') {
        word.pop();
        mute_e_stripped = true;
    }
    // A silent final consonant goes, but a name never shrinks below two letters, so the first
    // sound of a key is always the first sound of the name. When a mute "e" was just stripped the
    // consonant before it is pronounced ("Simone", "Louise", "Laurette"), with one exception kept
    // on purpose: the "p" of "Philippe", so that it meets "Filip".
    while word.len() > 2 {
        let last = word[word.len() - 1];
        let before = word[word.len() - 2];
        let silent = if mute_e_stripped {
            last == 'p'
        } else {
            matches!(last, 't' | 'd' | 's' | 'p') || (last == 'x' && before == 'u')
        };
        if !silent {
            break;
        }
        word.pop();
    }

    let mut out = String::new();
    let mut at = 0;
    while at < word.len() {
        let c = word[at];
        let next = word.get(at + 1).copied();
        let after_next = word.get(at + 2).copied();

        if is_vowel(c) {
            let rule = VOWEL_RULES.iter().find(|(pattern, _, nasal)| {
                starts_with_at(&word, at, pattern)
                    && (!nasal
                        || nasal_allowed(&word, at + pattern.chars().count(), mute_e_stripped))
            });
            if let Some((pattern, sound, _)) = rule {
                out.push_str(sound);
                at += pattern.chars().count();
            } else {
                out.push_str(single_vowel(c).unwrap_or(""));
                at += 1;
            }
            continue;
        }

        // The end of a word whose mute "e" was stripped still has a vowel after it for the
        // letters ("c", "g", "s") that sound differently in front of one.
        let front = |index: usize| match word.get(index) {
            Some(letter) => FRONT_VOWELS.contains(letter),
            None => mute_e_stripped && index == word.len(),
        };
        match c {
            'b' => {
                // "Lefebvre": the "b" is not pronounced.
                if next != Some('v') {
                    out.push('B');
                }
            }
            'c' => match next {
                Some('h') => {
                    out.push('X');
                    at += 1;
                }
                Some('k') => {
                    out.push('K');
                    at += 1;
                }
                // "Francois" written without its cedilla, as every sanitised file name is.
                Some('o')
                    if at > 0 && word[at - 1] == 'n' && starts_with_at(&word, at + 1, "oi") =>
                {
                    out.push('S')
                }
                _ if front(at + 1) => out.push('S'),
                _ => out.push('K'),
            },
            'g' => match next {
                Some('n') => {
                    out.push_str("NI");
                    at += 1;
                }
                Some('u') if front(at + 2) => {
                    out.push('G');
                    at += 1;
                }
                _ if front(at + 1) => out.push('J'),
                _ => out.push('G'),
            },
            // Silent. "ch", "ph", "sh" and "th" were consumed with their first letter.
            'h' => {}
            'p' => {
                if next == Some('h') {
                    out.push('F');
                    at += 1;
                } else {
                    out.push('P');
                }
            }
            'q' => {
                out.push('K');
                if next == Some('u') {
                    at += 1;
                }
            }
            's' => {
                let before_vowel =
                    next.is_some_and(is_vowel) || (next.is_none() && mute_e_stripped);
                let between_vowels = at > 0 && is_vowel(word[at - 1]) && before_vowel;
                match next {
                    Some('s') => {
                        out.push('S');
                        at += 1;
                    }
                    Some('c') if after_next == Some('h') => {
                        out.push('X');
                        at += 2;
                    }
                    Some('h') => {
                        out.push('X');
                        at += 1;
                    }
                    Some('c') if front(at + 2) => {
                        out.push('S');
                        at += 1;
                    }
                    _ if between_vowels => out.push('Z'),
                    _ => out.push('S'),
                }
            }
            't' => {
                out.push('T');
                if next == Some('h') {
                    at += 1;
                }
            }
            'k' => out.push('K'),
            'j' => out.push('J'),
            'w' | 'v' => out.push('V'),
            'x' => out.push_str("KS"),
            'z' => out.push('Z'),
            'd' | 'f' | 'l' | 'm' | 'n' | 'r' => out.extend(c.to_uppercase()),
            digit if digit.is_ascii_digit() => out.push(digit),
            other => out.extend(other.to_uppercase()),
        }
        at += 1;
    }

    // Double letters are one sound.
    let mut key = String::with_capacity(out.len());
    for c in out.chars() {
        if key.chars().last() != Some(c) || c.is_ascii_digit() {
            key.push(c);
        }
    }
    key
}

// ----------------------------------------------------------------------------- distance

/// Damerau-Levenshtein distance (insertion, deletion, substitution and transposition of two
/// neighbours, each costing one), or `None` as soon as it is certain to exceed `max`. Compares
/// characters, not bytes. The early exit is what makes it cheap to run against every alias of an
/// index: a length gap above `max`, or a whole row above it, ends the work.
pub fn bounded_damerau_levenshtein(a: &str, b: &str, max: usize) -> Option<usize> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > max {
        return None;
    }
    if a.is_empty() || b.is_empty() {
        return Some(a.len().max(b.len())).filter(|distance| *distance <= max);
    }

    let mut before_previous: Vec<usize> = vec![0; b.len() + 1];
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current: Vec<usize> = vec![0; b.len() + 1];

    for i in 1..=a.len() {
        current[0] = i;
        let mut row_minimum = current[0];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (previous[j] + 1)
                .min(current[j - 1] + 1)
                .min(previous[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(before_previous[j - 2] + 1);
            }
            current[j] = best;
            row_minimum = row_minimum.min(best);
        }
        if row_minimum > max {
            return None;
        }
        std::mem::swap(&mut before_previous, &mut previous);
        std::mem::swap(&mut previous, &mut current);
    }
    Some(previous[b.len()]).filter(|distance| *distance <= max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn french(word: &str) -> String {
        FrenchPhonetic.encode_token(word)
    }

    #[test]
    fn homophones_share_a_key() {
        assert_eq!(french("Dupont"), french("Dupond"));
        assert_eq!(french("Dupont"), french("Dupon"));
        assert_eq!(french("Lefebvre"), french("Lefevre"));
        assert_eq!(french("Rousseau"), french("Rousso"));
    }

    #[test]
    fn different_names_keep_different_keys() {
        assert_ne!(french("Dupont"), french("Dumont"));
        assert_ne!(french("Martin"), french("Martine"));
        assert_ne!(french("Simon"), french("Simone"));
    }

    #[test]
    fn the_key_is_plain_ascii_and_deterministic() {
        let key = french("Fran\u{e7}ois");
        assert!(key.is_ascii());
        assert_eq!(key, french("Fran\u{e7}ois"));
        assert_eq!(french("Fran\u{e7}ois"), french("Fran\u{63}\u{327}ois"));
    }

    #[test]
    fn digits_and_empty_input_are_left_alone() {
        assert_eq!(french("2026"), "2026");
        assert_eq!(french("..."), "");
        assert_eq!(FrenchPhonetic.signature(), "fr-rules:1");
    }

    #[test]
    fn soundex_follows_the_convention() {
        let english = EnglishPhonetic;
        assert_eq!(english.encode_token("Robert"), "R163");
        assert_eq!(english.encode_token("Rupert"), "R163");
        assert_eq!(english.encode_token("Ashcraft"), "A261");
        assert_eq!(english.encode_token("Tymczak"), "T522");
        assert_eq!(english.encode_token("Smith"), english.encode_token("Smyth"));
        assert_eq!(english.signature(), "en-soundex:1");
    }

    #[test]
    fn the_locale_picks_the_encoder() {
        assert_eq!(encoder_for_locale("fr-FR").id(), "fr-rules");
        assert_eq!(encoder_for_locale("en-US").id(), "en-soundex");
        assert_eq!(encoder_for_locale("de-DE").id(), "fr-rules");
    }

    #[test]
    fn the_edit_distance_stops_at_its_bound() {
        assert_eq!(bounded_damerau_levenshtein("dupont", "dupond", 2), Some(1));
        assert_eq!(bounded_damerau_levenshtein("dupont", "dupnot", 2), Some(1));
        assert_eq!(bounded_damerau_levenshtein("abc", "abc", 0), Some(0));
        assert_eq!(bounded_damerau_levenshtein("kitten", "sitting", 2), None);
        assert_eq!(bounded_damerau_levenshtein("kitten", "sitting", 3), Some(3));
        assert_eq!(bounded_damerau_levenshtein("", "ab", 2), Some(2));
        assert_eq!(bounded_damerau_levenshtein("", "abc", 2), None);
    }
}
