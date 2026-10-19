//! The file name and the folder, read as a first-class source of facts (`docs/LEXICAL-RETRIEVAL-DESIGN.md`
//! 5.3). One function builds the profile of a name and serves both the Analyse (the words are stored)
//! and a file that was never analysed (the same words are built on the fly from the inventory), so the
//! left panel works before any Analyse and for a scan whose text could not be read.
//!
//! What is read, with its source and confidence, or not read at all:
//!
//! - **words** of the stem and of every folder, folded and cut at `-`, `_`, `.`, `/` and between a letter
//!   and a digit, so `Ordonnance pour Esaie`, `ordonnance-pour-esaie` and `ordonnance esaie` meet;
//! - **exact keys**: the full name, the stem, the stem without its date prefix, the relative path, and the
//!   same for the name the file had before cleaning;
//! - the **date prefix** (`2026-10-01_`), which is a date and not words, and a `YYYY` or `YYYY-MM` folder;
//! - **identifiers** of the packs found in the stem (`FAC-2026-014`);
//! - **links to known people and organisations**, for every run of words, not only for the whole stem
//!   (`hug-martin-marie` holds an organisation and a person). A link goes to an entity the base **already
//!   holds** and is never created from a name alone; a phrase that two entities answer to is not linked.

use crate::knowledge::extract::text::names_the_whole_entity;
use crate::knowledge::gazetteer::{is_listed_type, Gazetteer};
use crate::knowledge::normalize::fold;
use crate::knowledge::packs::PackSet;

/// The confidence of a link made only from a file name: a person chose the name, but a name is short
/// and a word of it can be anything. The same value as the knowledge base's `filename_token` method.
pub const NAME_LINK_CONFIDENCE: f32 = 0.50;

/// The confidence of a date written as a prefix of the name. The convention is strong in a practice
/// that follows it and absent in one that does not, so a prefix is read only when it is a real date.
pub const NAME_DATE_CONFIDENCE: f32 = 0.70;

/// The confidence of a folder that is a year or a month (`2026-03`).
pub const FOLDER_DATE_CONFIDENCE: f32 = 0.40;

/// The shortest word start the finder matches while the user types. Two letters is a start, one is not.
pub const MIN_PREFIX_LETTERS: usize = 2;

/// The folded words of `text`: split at every character that is not a letter or a digit and between a
/// letter and a digit (`cpam74` is `cpam`, `74`; `FAC-2026-014` is `fac`, `2026`, `014`).
pub fn words_of(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut current_is_digit = false;
    // The ligatures oe and ae and the sharp s have no decomposition: a name typed with them meets the
    // plain spelling a cleaned file name has.
    let folded = fold(text)
        .replace('\u{153}', "oe")
        .replace('\u{e6}', "ae")
        .replace('\u{df}', "ss");
    for c in folded.chars() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            continue;
        }
        let digit = c.is_numeric();
        if !current.is_empty() && digit != current_is_digit {
            words.push(std::mem::take(&mut current));
        }
        current.push(c);
        current_is_digit = digit;
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// A calendar date read from a name or a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameDate {
    /// `YYYY-MM-DD` for a prefix, `YYYY-MM` or `YYYY` for a folder.
    pub value: String,
    pub source: NameDateSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameDateSource {
    FilenamePrefix,
    Folder,
}

impl NameDateSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FilenamePrefix => "filename_prefix",
            Self::Folder => "folder",
        }
    }
}

/// `2026-10-01_`, `2026_10_01-`, `2026.10.01 ` or `20261001_` at the start of a stem, as a valid date.
/// Returns the date as `YYYY-MM-DD` and the rest of the stem. A stem that merely starts with digits
/// that are not a date (`2026-14-99_x`, `1234567_x`) has no prefix.
pub fn split_date_prefix(stem: &str) -> (Option<String>, &str) {
    let bytes = stem.as_bytes();
    let digit_run = |from: usize, len: usize| {
        bytes
            .get(from..from + len)
            .is_some_and(|part| part.iter().all(u8::is_ascii_digit))
    };
    let is_separator = |index: usize| matches!(bytes.get(index), Some(b'-' | b'_' | b'.'));
    let (year, month, day, consumed) = if digit_run(0, 4)
        && is_separator(4)
        && digit_run(5, 2)
        && is_separator(7)
        && digit_run(8, 2)
    {
        (&stem[0..4], &stem[5..7], &stem[8..10], 10)
    } else if digit_run(0, 8) && !digit_run(8, 1) {
        (&stem[0..4], &stem[4..6], &stem[6..8], 8)
    } else {
        return (None, stem);
    };
    let valid = chrono::NaiveDate::from_ymd_opt(
        year.parse().unwrap_or(0),
        month.parse().unwrap_or(0),
        day.parse().unwrap_or(0),
    )
    .is_some();
    if !valid {
        return (None, stem);
    }
    let rest = &stem[consumed..];
    let rest = rest.trim_start_matches(['-', '_', '.', ' ']);
    (Some(format!("{year}-{month}-{day}")), rest)
}

/// What the name and the folder of one file say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameProfile {
    pub relative_path: String,
    pub extension: String,
    /// The words of the stem, without its date prefix.
    pub name_words: Vec<String>,
    /// The words of the folders, without a folder that is only a year or a month.
    pub folder_words: Vec<String>,
    /// The words of the name the file had before it was cleaned, when that name differs.
    pub original_words: Vec<String>,
    /// The folded keys a name typed in full is compared with.
    pub exact_keys: Vec<String>,
    pub date: Option<NameDate>,
    pub folder_period: Option<NameDate>,
}

impl NameProfile {
    /// Build the profile of `relative_path` (forward slashes, as the inventory writes it).
    /// `original_name` is the name the file had before the Analyse cleaned it, from the rename log.
    pub fn build(relative_path: &str, original_name: Option<&str>) -> Self {
        let (folders, name) = match relative_path.rsplit_once('/') {
            Some((folders, name)) => (folders, name),
            None => ("", relative_path),
        };
        let (stem, extension) = match name.rsplit_once('.') {
            Some((stem, extension)) if !stem.is_empty() => (stem, fold(extension)),
            _ => (name, String::new()),
        };
        let (date, rest) = split_date_prefix(stem);

        let mut folder_words = Vec::new();
        let mut folder_period = None;
        for segment in folders.split('/').filter(|segment| !segment.is_empty()) {
            match folder_period_of(segment) {
                Some(period) => folder_period = Some(period),
                None => folder_words.extend(words_of(segment)),
            }
        }

        let mut exact_keys: Vec<String> = Vec::new();
        let mut keep = |key: String| {
            if !key.is_empty() && !exact_keys.contains(&key) {
                exact_keys.push(key);
            }
        };
        keep(fold(name));
        keep(fold(stem));
        keep(fold(rest));
        keep(fold(relative_path));

        let mut original_words = Vec::new();
        if let Some(original) = original_name {
            let original_stem = original.rsplit_once('.').map_or(original, |(stem, _)| stem);
            let (_, original_rest) = split_date_prefix(original_stem);
            keep(fold(original));
            keep(fold(original_stem));
            keep(fold(original_rest));
            let words = words_of(original_rest);
            if words != words_of(rest) {
                original_words = words;
            }
        }

        Self {
            relative_path: relative_path.to_string(),
            extension,
            name_words: words_of(rest),
            folder_words,
            original_words,
            exact_keys,
            date: date.map(|value| NameDate {
                value,
                source: NameDateSource::FilenamePrefix,
            }),
            folder_period,
        }
    }

    /// Every word the name, the original name and the folders carry.
    pub fn all_words(&self) -> impl Iterator<Item = &str> {
        self.name_words
            .iter()
            .chain(&self.folder_words)
            .chain(&self.original_words)
            .map(String::as_str)
    }

    /// Whether `query`, as a person types it, finds this file by its name.
    ///
    /// A name typed in full (or its stem, or its path, or the name before cleaning) matches through the
    /// exact keys. Otherwise every word of the query must be a word of the file, in any order, and the
    /// **last** word may be only the start of one (the field is used while typing). A date prefix and the
    /// file's own extension typed in the query are not words.
    pub fn matches(&self, query: &str) -> bool {
        self.matches_as(query, true)
    }

    /// [`NameProfile::matches`], with the last word read as a start only when `last_may_be_start` (a field
    /// being typed), and as a whole word otherwise (a sentence already finished).
    pub fn matches_as(&self, query: &str, last_may_be_start: bool) -> bool {
        let wanted = fold(query.trim()).replace('\\', "/");
        if wanted.is_empty() {
            return false;
        }
        if self.exact_keys.contains(&wanted) {
            return true;
        }
        let (wanted_date, rest) = split_date_prefix(wanted.trim());
        // A date typed in front of the name is that file's date: a file dated otherwise is not it.
        if let (Some(typed), Some(own)) = (&wanted_date, &self.date) {
            if *typed != own.value {
                return false;
            }
        }
        let mut tokens = words_of(rest);
        if tokens.len() > 1 && !self.extension.is_empty() && tokens.last() == Some(&self.extension)
        {
            tokens.pop();
        }
        let Some((last, others)) = tokens.split_last() else {
            return false;
        };
        let words: Vec<&str> = self.all_words().collect();
        let exact_word = |token: &str| words.contains(&token);
        others.iter().all(|token| exact_word(token))
            && (exact_word(last)
                || (last_may_be_start
                    && last.chars().count() >= MIN_PREFIX_LETTERS
                    && words.iter().any(|word| word.starts_with(last.as_str()))))
    }
}

/// `2026` or `2026-03` (or `2026_03`) as a folder: a period, not words.
fn folder_period_of(segment: &str) -> Option<NameDate> {
    let bytes = segment.as_bytes();
    let all_digits = |part: &[u8]| !part.is_empty() && part.iter().all(u8::is_ascii_digit);
    let year_ok = |year: &str| {
        year.parse::<i32>()
            .is_ok_and(|y| (1990..=2100).contains(&y))
    };
    match bytes.len() {
        4 if all_digits(bytes) && year_ok(segment) => Some(NameDate {
            value: segment.to_string(),
            source: NameDateSource::Folder,
        }),
        7 if all_digits(&bytes[..4])
            && matches!(bytes[4], b'-' | b'_')
            && all_digits(&bytes[5..])
            && year_ok(&segment[..4])
            && (1..=12).contains(&segment[5..].parse::<u32>().unwrap_or(0)) =>
        {
            Some(NameDate {
                value: format!("{}-{}", &segment[..4], &segment[5..]),
                source: NameDateSource::Folder,
            })
        }
        _ => None,
    }
}

/// An identifier of a pack found in a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameIdentifier {
    pub scheme_id: String,
    pub kind: String,
    /// The reduced form the scheme compares on.
    pub normalized: String,
}

/// The identifiers of the packs' schemes found in the stem (`facture-medsupply-fac-2026-014` holds
/// `FAC2026014`). The stem is read in capitals because a name is lower case and the schemes are written
/// for capitals. A scheme that points at one person (a health insurance number, an e-mail address) is
/// never read from a name: the profile does not copy a personal identifier into a searchable field.
pub fn name_identifiers(stem_without_date: &str, packs: &PackSet) -> Vec<NameIdentifier> {
    let upper = stem_without_date.to_ascii_uppercase();
    let mut found: Vec<NameIdentifier> = Vec::new();
    for scheme in packs.identifier_schemes() {
        if scheme.personal_key {
            continue;
        }
        for hit in scheme.scan(&upper) {
            let identifier = NameIdentifier {
                scheme_id: scheme.id.clone(),
                kind: scheme.kind.clone(),
                normalized: hit.normalized,
            };
            if !found.contains(&identifier) {
                found.push(identifier);
            }
        }
    }
    found
}

/// A run of words of a name that is a person or an organisation the base already holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameLink {
    /// Index of the first word, in the name's words.
    pub start: usize,
    pub len: usize,
    pub entity_id: i64,
    pub type_id: String,
    /// True when two words were found in the other order (`martin-marie` for `Marie Martin`).
    pub reversed: bool,
}

/// Link the runs of `words` to entities the gazetteer holds: at each position the longest known phrase,
/// else a pair of words in the other order. Only a phrase that names the **whole** entity is linked (a
/// surname alone never is), only a person or an organisation, and only when exactly one entity answers
/// to the phrase: the finder lists, it never chooses between two people.
pub fn link_name_words(words: &[String], gazetteer: &Gazetteer) -> Vec<NameLink> {
    let mut links = Vec::new();
    let mut position = 0;
    while position < words.len() {
        if let Some(link) = link_at(words, position, gazetteer, false) {
            position += link.len;
            links.push(link);
        } else if position + 1 < words.len() {
            let reversed = [words[position + 1].clone(), words[position].clone()];
            match link_at(&reversed, 0, gazetteer, true).filter(|link| link.len == 2) {
                Some(mut link) => {
                    link.start = position;
                    position += 2;
                    links.push(link);
                }
                None => position += 1,
            }
        } else {
            position += 1;
        }
    }
    links
}

fn link_at(
    words: &[String],
    start: usize,
    gazetteer: &Gazetteer,
    reversed: bool,
) -> Option<NameLink> {
    let hit = gazetteer.longest_match_at(words, start)?;
    let mut entities: Vec<(i64, &str)> = hit
        .entries
        .iter()
        .filter(|entry| names_the_whole_entity(entry.kind) && is_listed_type(&entry.type_id))
        .map(|entry| (entry.entity_id, entry.type_id.as_str()))
        .collect();
    entities.sort_unstable();
    entities.dedup();
    // Another entity answering to the same phrase, even by a weaker alias, makes the phrase ambiguous.
    let others = hit.entries.iter().any(|entry| {
        entities
            .first()
            .is_some_and(|(id, _)| entry.entity_id != *id)
    });
    match entities.as_slice() {
        [(entity_id, type_id)] if !others => Some(NameLink {
            start,
            len: hit.len,
            entity_id: *entity_id,
            type_id: (*type_id).to_string(),
            reversed,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::gazetteer::GazetteerEntry;
    use crate::knowledge::{AliasKind, Origin};

    fn profile(path: &str) -> NameProfile {
        NameProfile::build(path, None)
    }

    #[test]
    fn words_are_folded_and_cut_at_separators_and_between_letters_and_digits() {
        assert_eq!(
            words_of("Ordonnance pour Esa\u{ef}e"),
            vec!["ordonnance", "pour", "esaie"]
        );
        assert_eq!(
            words_of("cpam74-dupont_jean.pdf"),
            vec!["cpam", "74", "dupont", "jean", "pdf"]
        );
        assert_eq!(words_of("FAC-2026-014"), vec!["fac", "2026", "014"]);
        assert!(words_of("--__..").is_empty());
    }

    #[test]
    fn a_date_prefix_is_a_date_and_not_words() {
        let profile = profile("courriers/2026-02/2026-02-12_courrier-orl-dupont-jean.pdf");
        assert_eq!(
            profile.name_words,
            vec!["courrier", "orl", "dupont", "jean"]
        );
        assert_eq!(profile.folder_words, vec!["courriers"]);
        let date = profile.date.expect("the prefix is a date");
        assert_eq!(
            (date.value.as_str(), date.source),
            ("2026-02-12", NameDateSource::FilenamePrefix)
        );
        let period = profile.folder_period.expect("the folder is a month");
        assert_eq!(
            (period.value.as_str(), period.source),
            ("2026-02", NameDateSource::Folder)
        );

        for (stem, expected) in [
            ("20261001_note", Some("2026-10-01")),
            ("2026_10_01-note", Some("2026-10-01")),
            ("2026-14-99_note", None),
            ("1234567_note", None),
            ("2026-10-01", Some("2026-10-01")),
            ("note-2026-10-01", None),
        ] {
            assert_eq!(split_date_prefix(stem).0.as_deref(), expected, "{stem}");
        }
    }

    #[test]
    fn a_name_typed_in_full_or_in_part_finds_the_file() {
        let profile = profile("courriers/2026-02/2026-02-12_courrier-orl-dupont-jean.pdf");
        for query in [
            "2026-02-12_courrier-orl-dupont-jean.pdf",
            "2026-02-12_courrier-orl-dupont-jean",
            "courrier-orl-dupont-jean",
            "COURRIER ORL DUPONT JEAN",
            "courriers/2026-02/2026-02-12_courrier-orl-dupont-jean.pdf",
            "courriers\\2026-02\\2026-02-12_courrier-orl-dupont-jean.pdf",
            "jean dupont",
            "dupont",
            "orl",
            "cour",
            "jean du",
            "dupont jean pdf",
            "courriers orl",
        ] {
            assert!(profile.matches(query), "{query:?} must find the file");
        }
        for query in [
            "",
            "   ",
            "dermatologie",
            "jean martin",
            "dupo jean",
            "x",
            "orlando",
        ] {
            assert!(!profile.matches(query), "{query:?} must not find the file");
        }
    }

    #[test]
    fn a_date_typed_before_the_name_must_be_the_files_own_date() {
        let march = profile("2026-03-01_courrier-orl.pdf");
        let april = profile("2026-04-01_courrier-orl.pdf");
        assert!(march.matches("2026-03-01_courrier-orl.pdf"));
        assert!(!april.matches("2026-03-01_courrier-orl.pdf"));
        assert!(!april.matches("2026-03-01 courrier orl"));
        assert!(
            april.matches("courrier orl"),
            "without a date, both files answer"
        );
        assert!(march.matches("courrier orl"));
    }

    #[test]
    fn a_name_typed_with_decomposed_accents_or_ligatures_finds_the_plain_spelling() {
        let resume = profile("2026-01-02_resume-cardiologie.pdf");
        // `e` + combining acute, twice: the decomposed spelling some keyboards and file systems produce.
        assert!(resume.matches("re\u{301}sume\u{301} cardiologie"));
        assert!(resume.matches("r\u{e9}sum\u{e9}"));

        let heart = profile("2026-01-02_coeur.pdf");
        assert!(heart.matches("c\u{153}ur"));
        assert!(heart.matches("C\u{152}UR"));
        // The other way round: a name that still carries the ligature meets the plain word.
        assert!(profile("c\u{153}ur-gauche.pdf").matches("coeur"));
        assert!(profile("caecum.pdf").matches("c\u{e6}cum"));
        assert!(profile("rue-strasse.pdf").matches("stra\u{df}e"));
        assert!(profile("rue-stra\u{df}e.pdf").matches("strasse"));
    }

    #[test]
    fn a_single_letter_is_a_whole_word_and_never_the_start_of_one() {
        let profile = profile("2026-01-02_note-a-classer.txt");
        assert!(profile.matches("a"), "a one-letter word of the name is found");
        assert!(!profile.matches("c"), "one letter is not the start of `classer`");
        assert!(profile.matches("cl"), "two letters are");
    }

    #[test]
    fn punctuation_alone_finds_nothing() {
        let profile = profile("2026-01-02_note-a-classer.txt");
        for query in ["!!", "...", "-", "_ -", "/", "?"] {
            assert!(!profile.matches(query), "{query:?} must not find the file");
        }
    }

    #[test]
    fn only_the_last_word_may_be_a_start() {
        let profile = profile("2026-10-01_courrier-dermatologie.txt");
        assert!(profile.matches("derma"));
        assert!(profile.matches("courrier derma"));
        assert!(
            !profile.matches("cour derma"),
            "a start in the middle is not a word"
        );
    }

    #[test]
    fn the_name_before_cleaning_is_searchable() {
        let profile = NameProfile::build(
            "2026-10-01_ordonnance-pour-esaie.pdf",
            Some("Ordonnance pour Esa\u{ef}e.pdf"),
        );
        assert!(profile.matches("Ordonnance pour Esa\u{ef}e.pdf"));
        assert!(profile.matches("ordonnance pour esaie"));
        assert!(profile.matches("pour esaie"));

        let renamed = NameProfile::build("scan-17.pdf", Some("Scan du mercredi.pdf"));
        assert!(
            renamed.matches("mercredi"),
            "a word only the original name carried"
        );
        assert!(renamed.matches("Scan du mercredi.pdf"));
        assert!(!profile.matches("mercredi"));
    }

    #[test]
    fn a_file_without_extension_or_folder_has_a_profile() {
        let bare = profile("README");
        assert_eq!(bare.name_words, vec!["readme"]);
        assert!(bare.extension.is_empty() && bare.folder_words.is_empty());
        assert!(bare.matches("readme") && bare.matches("read"));
        let hidden = profile(".gitignore");
        assert!(hidden.matches("gitignore"));
    }

    #[test]
    fn folders_that_are_only_a_year_or_a_month_are_periods() {
        let profile = profile("accounts/2026/2026-03/q1-summary.txt");
        assert_eq!(profile.folder_words, vec!["accounts"]);
        assert_eq!(
            profile.folder_period.as_ref().expect("period").value,
            "2026-03"
        );
        assert!(profile.matches("accounts summary"));
        let not_a_period = self::profile("archive/2026-13/x.txt");
        assert_eq!(not_a_period.folder_words, vec!["archive", "2026", "13"]);
    }

    #[test]
    fn identifiers_of_the_packs_are_read_from_the_stem_but_never_a_personal_one() {
        let packs = PackSet::load("fr-FR", &["health-fr", "health-ch"]).expect("loads");
        let (_, rest) = split_date_prefix("2026-04-14_facture-medsupply-fac-2026-014");
        let found = name_identifiers(rest, &packs);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].scheme_id, "invoice_number");
        assert_eq!(found[0].normalized, "FAC2026014");
        assert!(name_identifiers("courrier-orl-dupont-jean", &packs).is_empty());
        assert!(
            name_identifiers("jean.dupont@example.test", &packs).is_empty(),
            "an e-mail address is a personal identifier and is not copied into the profile"
        );
    }

    fn gazetteer() -> Gazetteer {
        let mut gazetteer = Gazetteer::new();
        let mut add = |phrase: &str, entity_id: i64, type_id: &str, kind: AliasKind| {
            gazetteer.insert(
                phrase,
                GazetteerEntry {
                    entity_id,
                    type_id: type_id.to_string(),
                    kind,
                    origin: Origin::Automatic,
                },
            );
        };
        add("marie martin", 1, "person", AliasKind::CanonicalVariant);
        add("martin marie", 1, "person", AliasKind::Reordered);
        add("martin", 1, "person", AliasKind::PartialSurname);
        add("martin", 2, "person", AliasKind::PartialSurname);
        add("hug", 3, "organization", AliasKind::Abbreviation);
        add("helsana", 4, "organization", AliasKind::CanonicalVariant);
        add("paul roy", 5, "person", AliasKind::CanonicalVariant);
        add("paul roy", 6, "person", AliasKind::CanonicalVariant);
        add("dermatologie", 7, "item", AliasKind::CanonicalVariant);
        gazetteer
    }

    fn link_words(text: &str) -> Vec<(usize, usize, i64, bool)> {
        link_name_words(&words_of(text), &gazetteer())
            .into_iter()
            .map(|link| (link.start, link.len, link.entity_id, link.reversed))
            .collect()
    }

    #[test]
    fn an_organisation_and_a_person_in_one_name_are_both_linked() {
        assert_eq!(
            link_words("hug-martin-marie"),
            vec![(0, 1, 3, false), (1, 2, 1, false)]
        );
        assert_eq!(
            link_words("decompte-helsana-marie-martin"),
            vec![(1, 1, 4, false), (2, 2, 1, false)]
        );
        assert_eq!(link_words("martin-marie"), vec![(0, 2, 1, false)]);
    }

    #[test]
    fn a_pair_of_words_is_found_in_the_other_order_when_only_one_order_is_known() {
        let mut only_one_order = Gazetteer::new();
        only_one_order.insert(
            "marie martin",
            GazetteerEntry {
                entity_id: 1,
                type_id: "person".to_string(),
                kind: AliasKind::CanonicalVariant,
                origin: Origin::Automatic,
            },
        );
        let links = link_name_words(&words_of("note-martin-marie"), &only_one_order);
        assert_eq!(links.len(), 1);
        assert_eq!(
            (links[0].start, links[0].len, links[0].reversed),
            (1, 2, true)
        );
    }

    #[test]
    fn a_surname_alone_an_ambiguous_phrase_and_a_non_name_are_never_linked() {
        assert!(
            link_words("courrier-martin").is_empty(),
            "a surname two people carry"
        );
        assert!(
            link_words("ordonnance-paul-roy").is_empty(),
            "two entities answer to the phrase"
        );
        assert!(
            link_words("courrier-dermatologie").is_empty(),
            "an item is not a person or an organisation"
        );
        assert!(
            link_words("courrier-jean").is_empty(),
            "nothing known is not created"
        );
        assert!(link_name_words(&[], &gazetteer()).is_empty());
    }
}
