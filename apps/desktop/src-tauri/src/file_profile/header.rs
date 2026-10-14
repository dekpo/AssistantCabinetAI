//! The header block of a document: its title line, its labelled lines (`Objet :`, `Patient :`,
//! `Date :`), its references (`Facture n FAC-2026-014`), the dates in its first lines, and the roles
//! those lines give to a name (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 4.2 and 5).
//!
//! Everything a label means is **data** (`vocabulary.rs`, per pack and per language); this file only
//! knows how a labelled line is shaped. Each fact carries its confidence and nothing is guessed:
//!
//! - a role is recorded only from a **label** (`Patient : X`), never from a position or a salutation;
//! - a date is classified by where it stands (labelled, written as `<place>, <marker> <date>`, in the title, or
//!   only in the first lines), and a header with two different dates of the same best class has **no**
//!   chosen date (`dates.rs`), because a wrong date is worse than none.
//!
//! The text read is the first lines only (`HEADER_LINES`, `SCAN_LINES`): a header is at the top, and a
//! document of a thousand pages is never read past its first page here.

use std::sync::OnceLock;

use chrono::{Datelike, NaiveDate};
use regex::Regex;

use super::metadata::strip_front_matter;
use super::name::words_of;
use super::vocabulary::{Phrase, ProfileVocabulary};

/// Lines (non-empty) scanned for dates and for the header words.
pub const HEADER_LINES: usize = 12;
/// Lines (non-empty) scanned for labelled lines and references.
pub const SCAN_LINES: usize = 30;
/// A line longer than this is a paragraph, not a header line.
pub const MAX_LINE_CHARS: usize = 240;
/// A first line longer than this is not a title.
pub const MAX_TITLE_CHARS: usize = 120;
/// The longest value kept after a label (a name, an organisation).
pub const MAX_VALUE_CHARS: usize = 120;
/// The lines kept as the letterhead (where the issuing organisation is usually named).
pub const LETTERHEAD_LINES: usize = 4;

pub const ROLE_CONFIDENCE: f32 = 0.85;
pub const TITLE_HEADING_CONFIDENCE: f32 = 0.70;
pub const TITLE_FIRST_LINE_CONFIDENCE: f32 = 0.55;

/// Where a date stands in the header, from the most to the least trustworthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DateClass {
    /// Only in the first lines, in a sentence: `... takes place on 4 April 2026` is as likely a date the
    /// document talks about as the date it was written.
    Body,
    /// In the first line, which is the title: `Resultats d'analyses du 20 juin 2026`.
    Title,
    /// Written as `<place>, <marker> <date>`: the date a letter says it was written.
    Dated,
    /// After a date label: `Date : 14 avril 2026`.
    Labelled,
}

impl DateClass {
    pub fn confidence(self) -> f32 {
        match self {
            Self::Labelled => 0.90,
            Self::Dated => 0.80,
            Self::Title => 0.75,
            Self::Body => 0.60,
        }
    }

    pub fn source(self) -> &'static str {
        match self {
            Self::Labelled => "header_label",
            Self::Dated => "header_dated",
            Self::Title => "header_title",
            Self::Body => "header_text",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderDate {
    /// `YYYY-MM-DD`.
    pub iso: String,
    pub class: DateClass,
    /// Index among the non-empty lines.
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleSource {
    Heading,
    FirstLine,
}

impl TitleSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Heading => "heading",
            Self::FirstLine => "first_line",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TitleLine {
    pub text: String,
    pub source: TitleSource,
    pub confidence: f32,
}

/// A name given a role by a label. The `surface` is the text after the label, as written.
#[derive(Debug, Clone, PartialEq)]
pub struct RoleHit {
    pub role: String,
    pub surface: String,
    pub line: usize,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// `case`, `invoice`, `policy`, ... (a code from the pack's `references`).
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeaderReading {
    pub title: Option<TitleLine>,
    pub subject_line: Option<String>,
    pub references: Vec<Reference>,
    pub dates: Vec<HeaderDate>,
    pub roles: Vec<RoleHit>,
    /// The first lines, as written: the letterhead.
    pub letterhead: Vec<String>,
    /// The folded words of the first `HEADER_LINES` lines.
    pub header_words: Vec<String>,
}

/// Read the header block of `text` (the text of the first page is enough).
pub fn read_header(text: &str, vocabulary: &ProfileVocabulary) -> HeaderReading {
    let text = strip_front_matter(text);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(SCAN_LINES)
        .collect();
    let mut reading = HeaderReading::default();
    let labels = vocabulary.labels();

    reading.letterhead = lines
        .iter()
        .take(LETTERHEAD_LINES)
        .filter(|line| line.chars().count() <= MAX_LINE_CHARS)
        .map(|line| line.to_string())
        .collect();
    reading.header_words = words_of(
        &lines
            .iter()
            .take(HEADER_LINES)
            .copied()
            .collect::<Vec<_>>()
            .join(" "),
    );

    let mut title_line: Option<usize> = None;
    for (index, line) in lines.iter().enumerate() {
        if line.chars().count() > MAX_LINE_CHARS {
            continue;
        }
        let labelled = split_label(line);
        let label_words: Option<Phrase> = labelled.map(|(label, _)| words_of(label));

        // The title: the first line that is not a labelled line and is short enough to be a title.
        if reading.title.is_none() && title_line.is_none() && index == 0 {
            let is_label_line = label_words
                .as_ref()
                .is_some_and(|words| is_known_label(vocabulary, words));
            if !is_label_line {
                if let Some(title) = title_of(line) {
                    reading.title = Some(title);
                    title_line = Some(index);
                }
            }
        }

        let mut date_label_line = false;
        if let (Some((_, value)), Some(words)) = (labelled, &label_words) {
            let value = clean_value(value);
            if matches_label(&labels.subject_line, words) {
                if reading.subject_line.is_none() && !value.is_empty() {
                    reading.subject_line = Some(value.clone());
                }
            } else if matches_label(&labels.date, words) {
                date_label_line = true;
            } else if let Some(role) = role_of(vocabulary, words) {
                if !value.is_empty() {
                    reading.roles.push(RoleHit {
                        role: role.to_string(),
                        surface: value,
                        line: index,
                        confidence: ROLE_CONFIDENCE,
                    });
                }
            }
        }

        for reference in references_in(line, vocabulary) {
            if !reading.references.contains(&reference) {
                reading.references.push(reference);
            }
        }

        if index < HEADER_LINES {
            for (iso, after_marker) in dates_in(line, vocabulary) {
                let class = if date_label_line {
                    DateClass::Labelled
                } else if after_marker {
                    DateClass::Dated
                } else if title_line == Some(index) {
                    DateClass::Title
                } else {
                    DateClass::Body
                };
                let date = HeaderDate {
                    iso,
                    class,
                    line: index,
                };
                if !reading.dates.contains(&date) {
                    reading.dates.push(date);
                }
            }
        }
    }
    reading
}

/// `Label : value`, where the label is at most four words. A colon later in a sentence is not a label.
fn split_label(line: &str) -> Option<(&str, &str)> {
    let (label, value) = line.split_once(':')?;
    let label = label.trim();
    if label.is_empty() || label.chars().count() > 40 || label.split_whitespace().count() > 4 {
        return None;
    }
    Some((label, value))
}

fn matches_label(phrases: &[Phrase], label_words: &[String]) -> bool {
    phrases
        .iter()
        .any(|phrase| phrase.as_slice() == label_words)
}

fn is_known_label(vocabulary: &ProfileVocabulary, words: &[String]) -> bool {
    let labels = vocabulary.labels();
    matches_label(&labels.subject_line, words)
        || matches_label(&labels.date, words)
        || role_of(vocabulary, words).is_some()
}

fn role_of<'a>(vocabulary: &'a ProfileVocabulary, words: &[String]) -> Option<&'a str> {
    vocabulary
        .labels()
        .roles
        .iter()
        .find(|(_, phrases)| matches_label(phrases, words))
        .map(|(role, _)| role.as_str())
}

/// The text after a label: one line, no trailing punctuation, no more than `MAX_VALUE_CHARS`.
fn clean_value(value: &str) -> String {
    let value = value.trim().trim_end_matches(['.', ',', ';', ':']).trim();
    value.chars().take(MAX_VALUE_CHARS).collect()
}

fn title_of(line: &str) -> Option<TitleLine> {
    let (text, source, confidence) = match line.strip_prefix('#') {
        Some(heading) => (
            heading.trim_start_matches('#').trim(),
            TitleSource::Heading,
            TITLE_HEADING_CONFIDENCE,
        ),
        None => (line, TitleSource::FirstLine, TITLE_FIRST_LINE_CONFIDENCE),
    };
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    if letters < 3 || text.chars().count() > MAX_TITLE_CHARS {
        return None;
    }
    Some(TitleLine {
        text: text.to_string(),
        source,
        confidence,
    })
}

/// A reference introduced by a pack label (`Facture n FAC-2026-014`, `Police n 6.126.452-9`): the value
/// is the next word of the line and must contain a digit.
fn references_in(line: &str, vocabulary: &ProfileVocabulary) -> Vec<Reference> {
    let tokens: Vec<(String, &str)> = line
        .split_whitespace()
        .map(|token| (words_of(token).concat(), token))
        .collect();
    let mut found = Vec::new();
    for (kind, phrases) in &vocabulary.labels().references {
        for phrase in phrases {
            for start in 0..tokens.len() {
                let end = start + phrase.len();
                if end > tokens.len()
                    || !phrase
                        .iter()
                        .zip(&tokens[start..end])
                        .all(|(word, (folded, _))| word == folded)
                {
                    continue;
                }
                let value = tokens[end..]
                    .iter()
                    .find(|(folded, _)| !folded.is_empty())
                    .map(|(_, token)| token.trim_matches(|c: char| !c.is_alphanumeric()))
                    .unwrap_or("");
                if value.chars().any(|c| c.is_ascii_digit()) && value.chars().count() <= 40 {
                    found.push(Reference {
                        kind: kind.clone(),
                        value: value.to_string(),
                    });
                }
            }
        }
    }
    found
}

fn numeric_date_patterns() -> &'static (Regex, Regex) {
    static PATTERNS: OnceLock<(Regex, Regex)> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        (
            Regex::new(r"\b(\d{4})-(\d{2})-(\d{2})\b").expect("a valid pattern"),
            Regex::new(r"\b(\d{1,2})[/.](\d{1,2})[/.](\d{4})\b").expect("a valid pattern"),
        )
    })
}

/// Every date in `line`, as `YYYY-MM-DD`, with whether it is introduced as `<place>, <marker> <date>`
/// (a comma, then a marker word such as `on` or the French `du`).
fn dates_in(line: &str, vocabulary: &ProfileVocabulary) -> Vec<(String, bool)> {
    let mut found: Vec<(usize, NaiveDate)> = Vec::new();
    let (iso, european) = numeric_date_patterns();
    for captures in iso.captures_iter(line) {
        if let Some(date) = ymd(&captures[1], &captures[2], &captures[3]) {
            found.push((captures.get(0).map_or(0, |m| m.start()), date));
        }
    }
    for captures in european.captures_iter(line) {
        if let Some(date) = ymd(&captures[3], &captures[2], &captures[1]) {
            found.push((captures.get(0).map_or(0, |m| m.start()), date));
        }
    }

    // `30 mars 2026`, `1er mars 2026`, `March 30, 2026`: tokens with their byte offsets.
    let tokens = tokens_with_offsets(line);
    for index in 0..tokens.len() {
        let (start, token) = &tokens[index];
        // day month year
        if let (Some(day), Some((_, month)), Some((_, year))) = (
            day_number(token),
            tokens.get(index + 1),
            tokens.get(index + 2),
        ) {
            if let (Some(month), Ok(year)) = (vocabulary.month_number(month), year.parse::<i32>()) {
                if let Some(date) = NaiveDate::from_ymd_opt(year, month, day).filter(plausible_year)
                {
                    found.push((*start, date));
                }
            }
        }
        // month day year
        if let (Some(month), Some(day), Some((_, year))) = (
            vocabulary.month_number(token),
            tokens.get(index + 1).and_then(|(_, t)| day_number(t)),
            tokens.get(index + 2),
        ) {
            if let Ok(year) = year.parse::<i32>() {
                if let Some(date) = NaiveDate::from_ymd_opt(year, month, day).filter(plausible_year)
                {
                    found.push((*start, date));
                }
            }
        }
    }

    found.sort_by_key(|(start, _)| *start);
    found.dedup();
    found
        .into_iter()
        .filter(|(_, date)| plausible_year(date))
        .map(|(start, date)| {
            (
                date.format("%Y-%m-%d").to_string(),
                introduced_by_marker(&line[..start], vocabulary),
            )
        })
        .collect()
}

fn ymd(year: &str, month: &str, day: &str) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?)
}

fn plausible_year(date: &NaiveDate) -> bool {
    (1990..=2100).contains(&date.year())
}

/// `30`, `1er`, `2nd` as a day of the month.
fn day_number(token: &str) -> Option<u32> {
    let digits: String = token.chars().take_while(char::is_ascii_digit).collect();
    let suffix = &token[digits.len()..];
    if digits.is_empty()
        || digits.len() > 2
        || !matches!(suffix, "" | "er" | "ere" | "st" | "nd" | "rd" | "th")
    {
        return None;
    }
    digits.parse().ok().filter(|day| (1..=31).contains(day))
}

/// Whether the text before a date ends with a comma and then a date marker (`Annemasse, du`).
fn introduced_by_marker(before: &str, vocabulary: &ProfileVocabulary) -> bool {
    let Some(comma) = before.rfind(',') else {
        return false;
    };
    let after = words_of(&before[comma + 1..]);
    vocabulary
        .labels()
        .date_markers
        .iter()
        .any(|marker| after.as_slice() == marker.as_slice())
}

/// The alphanumeric runs of `line` with the byte offset each starts at.
fn tokens_with_offsets(line: &str) -> Vec<(usize, String)> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    for (offset, c) in line.char_indices() {
        match (c.is_alphanumeric(), start) {
            (true, None) => start = Some(offset),
            (false, Some(from)) => {
                tokens.push((from, line[from..offset].to_string()));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        tokens.push((from, line[from..].to_string()));
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocabulary() -> ProfileVocabulary {
        ProfileVocabulary::load(&["health-fr"]).expect("loads")
    }

    fn read(text: &str) -> HeaderReading {
        read_header(text, &vocabulary())
    }

    #[test]
    fn a_labelled_date_outranks_everything_and_a_dated_title_is_a_title_date() {
        let reading = read(
            "Invoice FAC-2026-014\nMedSupply SA\nDate : 14 avril 2026\nClient : Orbital Clinic\n",
        );
        let date = reading
            .dates
            .iter()
            .find(|d| d.iso == "2026-04-14")
            .expect("the labelled date");
        assert_eq!(date.class, DateClass::Labelled);

        let title = read("Resultats d'analyses du 20 juin 2026\nCerballiance Alpes\n");
        assert_eq!(title.dates.len(), 1);
        assert_eq!(
            (title.dates[0].iso.as_str(), title.dates[0].class),
            ("2026-06-20", DateClass::Title)
        );
    }

    #[test]
    fn a_place_and_date_written_with_a_marker_after_a_comma_is_a_dated_line() {
        let line = ["Annemasse, l", "e 30 mars 2026."].concat();
        let reading = read(&format!(
            "Letter body title\nDear colleague\nObjet : appointment. {line}\n"
        ));
        let date = reading
            .dates
            .iter()
            .find(|d| d.iso == "2026-03-30")
            .expect("found");
        assert_eq!(date.class, DateClass::Dated);

        let sentence = ["The next meeting takes place l", "e 4 avril 2026."].concat();
        let reading = read(&format!(
            "Minutes of the review on 3 fevrier 2026\n{sentence}\n"
        ));
        let next = reading
            .dates
            .iter()
            .find(|d| d.iso == "2026-04-04")
            .expect("found");
        assert_eq!(
            next.class,
            DateClass::Body,
            "a date a sentence mentions is not the document's date"
        );
    }

    #[test]
    fn dates_are_read_in_every_shipped_format_and_language() {
        let cases = [
            ("Date : 2026-03-30", "2026-03-30"),
            ("Date : 30/03/2026", "2026-03-30"),
            ("Date : 30.03.2026", "2026-03-30"),
            ("Date : 1er mars 2026", "2026-03-01"),
            ("Date : 30 mars 2026", "2026-03-30"),
            ("Date : 5 f\u{e9}vrier 2026", "2026-02-05"),
            ("Date : 5 fevr 2026", "2026-02-05"),
            ("Date : March 30, 2026", "2026-03-30"),
            ("Date : 2nd October 2026", "2026-10-02"),
        ];
        for (line, expected) in cases {
            let reading = read(&format!("Title line\n{line}\n"));
            assert_eq!(
                reading
                    .dates
                    .iter()
                    .map(|d| d.iso.as_str())
                    .collect::<Vec<_>>(),
                vec![expected],
                "{line}"
            );
        }
        for impossible in [
            "Date : 31/02/2026",
            "Date : 30 marsss 2026",
            "Date : 99 mars 2026",
            "Date : 30 mars 1850",
            "Date : 12345",
        ] {
            assert!(
                read(&format!("Title line\n{impossible}\n"))
                    .dates
                    .is_empty(),
                "{impossible}"
            );
        }
    }

    #[test]
    fn dates_below_the_header_are_not_read() {
        let mut text = String::from("Title line\n");
        for index in 0..HEADER_LINES + 2 {
            text.push_str(&format!("filler line {index}\n"));
        }
        text.push_str("Date : 14 avril 2026\n");
        assert!(read(&text).dates.is_empty());
    }

    #[test]
    fn a_label_gives_a_role_and_a_position_never_does() {
        let reading = read(
            "Ordonnance du 20 mai 2026\nCabinet medical\nPatient : Zorglub Metadatus, n\u{e9}(e) en 1998\nPrescripteur : Dr Ylva Reviewer\nDestinataire : Pharmacie Centrale\nDr Someone Unlabelled\n",
        );
        let roles: Vec<(&str, &str)> = reading
            .roles
            .iter()
            .map(|r| (r.role.as_str(), r.surface.as_str()))
            .collect();
        assert_eq!(
            roles,
            vec![
                ("subject", "Zorglub Metadatus, n\u{e9}(e) en 1998"),
                ("author", "Dr Ylva Reviewer"),
                ("recipient", "Pharmacie Centrale"),
            ]
        );
        assert!(reading
            .roles
            .iter()
            .all(|r| (r.confidence - ROLE_CONFIDENCE).abs() < f32::EPSILON));
        assert!(!roles
            .iter()
            .any(|(_, surface)| surface.contains("Unlabelled")));
    }

    #[test]
    fn a_label_with_nothing_after_it_gives_nothing_and_a_sentence_is_not_a_label() {
        let reading =
            read("Title line\nPatient :\nPatient : \nThe patient arrived at noon: she was calm.\n");
        assert!(reading.roles.is_empty(), "{:?}", reading.roles);
        assert!(reading.subject_line.is_none());
    }

    #[test]
    fn the_subject_line_and_the_references_are_read() {
        let reading = read(
            "Quote FAC-2026-115\nFiduciaire Rochat SA\nDevis n FAC-2026-115 du 6 avril 2026\nObjet : replacement of a workstation.\nPolice n 6.126.452-9\nDossier n DOS-2026-0057. Concerne : a lease\n",
        );
        assert_eq!(
            reading.subject_line.as_deref(),
            Some("replacement of a workstation")
        );
        let refs: Vec<(&str, &str)> = reading
            .references
            .iter()
            .map(|r| (r.kind.as_str(), r.value.as_str()))
            .collect();
        assert!(refs.contains(&("quote", "FAC-2026-115")), "{refs:?}");
        assert!(refs.contains(&("policy", "6.126.452-9")), "{refs:?}");
        assert!(refs.contains(&("case", "DOS-2026-0057")), "{refs:?}");
        assert!(
            read("Facture n pending payment\n").references.is_empty(),
            "a reference value has a digit"
        );
    }

    #[test]
    fn the_title_is_the_first_short_line_or_a_markdown_heading_and_never_a_paragraph() {
        let first = read("Annual review of the practice\nbody\n");
        let title = first.title.expect("a title");
        assert_eq!(
            (title.text.as_str(), title.source),
            ("Annual review of the practice", TitleSource::FirstLine)
        );
        let heading = read("# Meeting minutes\nbody\n").title.expect("a heading");
        assert_eq!(
            (heading.text.as_str(), heading.source),
            ("Meeting minutes", TitleSource::Heading)
        );
        assert!(heading.confidence > title.confidence);

        let paragraph = "word ".repeat(40);
        assert!(read(&format!("{paragraph}\nbody\n")).title.is_none());
        assert!(read("12 / 34\nbody\n").title.is_none());
        assert!(
            read("Patient : Someone\nbody\n").title.is_none(),
            "a labelled line is not a title"
        );
        assert!(read("").title.is_none() && read("   \n\n").dates.is_empty());
    }

    #[test]
    fn a_front_matter_block_is_not_part_of_the_header() {
        let reading = read("---\ntitle: In properties\nauthor: Ylva Reviewer\n---\n# Meeting minutes\nDate : 3 fevrier 2026\n");
        assert_eq!(
            reading.title.as_ref().map(|t| t.text.as_str()),
            Some("Meeting minutes")
        );
        assert!(
            reading.roles.is_empty(),
            "the properties are read from the block, not from the header"
        );
        assert_eq!(reading.dates.len(), 1);
    }

    #[test]
    fn the_letterhead_and_the_header_words_are_the_first_lines() {
        let reading = read("Facture\nMedSupply SA\nChemin du Bois-Gentil 7\nCity\nfifth line\n");
        assert_eq!(
            reading.letterhead,
            vec!["Facture", "MedSupply SA", "Chemin du Bois-Gentil 7", "City"]
        );
        assert!(reading.header_words.contains(&"medsupply".to_string()));
        assert!(reading.header_words.contains(&"fifth".to_string()));
    }
}
