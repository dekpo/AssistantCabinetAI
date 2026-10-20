//! What the left panel's search field means before the finder runs: the text she typed is read for the
//! two things that are not words, a file extension and a date (`docs/DECISIONS.md`, "Taken while
//! building" of the explorer step 1). Pure, with no index and no file system, so it is tested alone.
//!
//! - A typed word that is a file extension becomes a **format filter**, not a word: `.docx` and `docx`
//!   list every docx, `rapport .docx` is the word `rapport` among the docx. With a leading dot a word
//!   is always an extension; without it, only when the product reads that extension or the folder
//!   holds files of it.
//! - A typed text that is **only** a date (`2026`, `2026-02`, `2026-02-06`, `06/02/2026`,
//!   `06-02-2026`) also asks for the files dated that year, month or day. The words stay in the query,
//!   so a date that is part of an identifier (`FAC-2026-014`) keeps matching as words.

use super::contracts::{DateRange, FinderQuery, InventoryFile};
use crate::file_profile::name::words_of;
use crate::file_record::FileKind;
use crate::knowledge::normalize::fold;

/// The years read as a year when typed alone: the same bounds as a year folder. Another four-digit
/// number (an amount, a part of an identifier) is only a word.
const YEARS: std::ops::RangeInclusive<i32> = 1990..=2100;

/// The longest extension read from a dotted word.
const MAX_EXTENSION_LETTERS: usize = 10;

/// The typed text, split into what the finder is given.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypedText {
    /// The words left once the extensions are taken out; `None` when nothing is left.
    pub words: Option<String>,
    /// The extensions typed, lower case, without the dot, each once.
    pub formats: Vec<String>,
    /// The day, month or year the words say, when they are only that.
    pub date: Option<DateRange>,
}

/// Whether the product reads (or lists) files of this extension.
fn is_known_extension(extension: &str) -> bool {
    !matches!(
        FileKind::from_extension(extension),
        FileKind::Unsupported | FileKind::Other
    )
}

/// `docx` or `.DOCX` as an extension, when `word` is one: a dotted word always is (it needs a letter
/// and nothing but letters and digits), a bare word only if the product knows the extension or the
/// folder holds files of it (`folder_extensions`, lower case).
fn extension_of(word: &str, folder_extensions: &[String]) -> Option<String> {
    let (dotted, body) = match word.strip_prefix('.') {
        Some(body) => (true, body),
        None => (false, word),
    };
    let body = fold(body);
    let plausible = !body.is_empty()
        && body.chars().count() <= MAX_EXTENSION_LETTERS
        && body.chars().all(|c| c.is_ascii_alphanumeric())
        && body.chars().any(|c| c.is_ascii_alphabetic());
    if !plausible {
        return None;
    }
    if dotted || is_known_extension(&body) || folder_extensions.contains(&body) {
        Some(body)
    } else {
        None
    }
}

/// Read what was typed. `folder_extensions` are the extensions of the folder's files, lower case.
pub fn read_typed(text: &str, folder_extensions: &[String]) -> TypedText {
    let mut formats: Vec<String> = Vec::new();
    let mut kept: Vec<&str> = Vec::new();
    for word in text.split_whitespace() {
        match extension_of(word, folder_extensions) {
            Some(extension) => {
                if !formats.contains(&extension) {
                    formats.push(extension);
                }
            }
            None => kept.push(word),
        }
    }
    // A bare dot, or punctuation alone, is nothing to search for.
    let words = kept.join(" ");
    let words = (!words_of(&words).is_empty()).then_some(words);
    let date = words.as_deref().and_then(date_of);
    TypedText {
        words,
        formats,
        date,
    }
}

/// Rewrite `query` the way the left panel's field means it: the extensions typed leave the words and
/// join `formats` (added to the formats already asked for), and a date typed alone sets `typed_date`.
/// A text with no word, no extension and no date in it (punctuation only) is left as typed, so the finder
/// finds nothing for it rather than listing the whole folder. `inventory` gives the extensions the
/// folder holds. Returns the extensions that were taken out of the text (not those the caller already
/// asked for), so the answer can echo them and the panel can light the matching format choice.
pub fn apply_typed_text(query: &mut FinderQuery, inventory: &[InventoryFile]) -> Vec<String> {
    let Some(text) = query.text.as_deref() else {
        return Vec::new();
    };
    let mut held: Vec<String> = inventory
        .iter()
        .map(|file| file.extension.clone())
        .collect();
    held.sort();
    held.dedup();
    let typed = read_typed(text, &held);
    if typed.words.is_none() && typed.formats.is_empty() {
        return Vec::new();
    }
    // A date typed in a French form is searched as words in its ISO form, so that `06/02/2026` and
    // `2026-02-06` find exactly the same files.
    query.text = match &typed.date {
        Some(DateRange {
            from: Some(iso), ..
        }) => Some(iso.clone()),
        _ => typed.words,
    };
    for format in &typed.formats {
        if !query.formats.contains(format) {
            query.formats.push(format.clone());
        }
    }
    query.typed_date = typed.date;
    typed.formats
}

/// `text` as one day, month or year, when it is only that. Day and month are checked as a calendar.
pub fn date_of(text: &str) -> Option<DateRange> {
    let text = text.trim();
    let digits =
        |part: &str, len: usize| part.len() == len && part.bytes().all(|b| b.is_ascii_digit());
    let iso = |year: &str, month: Option<&str>, day: Option<&str>| -> Option<String> {
        let y: i32 = year.parse().ok()?;
        if !YEARS.contains(&y) {
            return None;
        }
        let m: Option<u32> = month.map(str::parse).transpose().ok()?;
        let d: Option<u32> = day.map(str::parse).transpose().ok()?;
        match (m, d) {
            (None, None) => Some(year.to_string()),
            (Some(m), None) => (1..=12)
                .contains(&m)
                .then(|| format!("{year}-{}", month.unwrap_or_default())),
            (Some(m), Some(d)) => chrono::NaiveDate::from_ymd_opt(y, m, d).map(|_| {
                format!(
                    "{year}-{}-{}",
                    month.unwrap_or_default(),
                    day.unwrap_or_default()
                )
            }),
            (None, Some(_)) => None,
        }
    };
    let parts: Vec<&str> = text.split(['-', '/']).collect();
    let separator_is = |wanted: char| {
        text.chars()
            .filter(|c| matches!(c, '-' | '/'))
            .all(|c| c == wanted)
    };
    let value = match parts.as_slice() {
        [year] if digits(year, 4) => iso(year, None, None),
        [year, month] if digits(year, 4) && digits(month, 2) && separator_is('-') => {
            iso(year, Some(month), None)
        }
        [year, month, day]
            if digits(year, 4) && digits(month, 2) && digits(day, 2) && separator_is('-') =>
        {
            iso(year, Some(month), Some(day))
        }
        // French order, day first: `06/02/2026` and `06-02-2026`.
        [day, month, year]
            if digits(day, 2)
                && digits(month, 2)
                && digits(year, 4)
                && (separator_is('/') || separator_is('-')) =>
        {
            iso(year, Some(month), Some(day))
        }
        _ => None,
    }?;
    Some(DateRange {
        from: Some(value.clone()),
        to: Some(value),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> TypedText {
        read_typed(text, &[])
    }

    fn day(value: &str) -> Option<DateRange> {
        Some(DateRange {
            from: Some(value.to_string()),
            to: Some(value.to_string()),
        })
    }

    #[test]
    fn a_dotted_word_is_an_extension_whatever_the_folder_holds() {
        let typed = read(".docx");
        assert_eq!(typed.formats, vec!["docx"]);
        assert_eq!(typed.words, None);
        assert_eq!(read(".xyz").formats, vec!["xyz"]);
    }

    #[test]
    fn a_bare_word_is_an_extension_when_the_product_reads_it() {
        for word in ["docx", "xlsx", "pdf", "csv", "ods", "txt"] {
            let typed = read(word);
            assert_eq!(typed.formats, vec![word.to_string()], "{word}");
            assert_eq!(typed.words, None, "{word}");
        }
    }

    #[test]
    fn a_bare_word_the_product_does_not_know_is_an_extension_only_if_the_folder_holds_it() {
        assert_eq!(read("xyz").formats, Vec::<String>::new());
        assert_eq!(read("xyz").words.as_deref(), Some("xyz"));
        let held = read_typed("xyz", &["xyz".to_string()]);
        assert_eq!(held.formats, vec!["xyz"]);
        assert_eq!(held.words, None);
    }

    #[test]
    fn the_capitals_and_the_dot_do_not_matter() {
        assert_eq!(read(".DOCX").formats, vec!["docx"]);
        assert_eq!(read("PDF").formats, vec!["pdf"]);
        assert_eq!(read(".Xlsx").formats, vec!["xlsx"]);
    }

    #[test]
    fn a_word_and_an_extension_are_told_apart() {
        let typed = read("rapport .docx");
        assert_eq!(typed.words.as_deref(), Some("rapport"));
        assert_eq!(typed.formats, vec!["docx"]);
        let typed = read(".docx rapport mensuel .pdf");
        assert_eq!(typed.words.as_deref(), Some("rapport mensuel"));
        assert_eq!(typed.formats, vec!["docx", "pdf"]);
    }

    #[test]
    fn an_extension_typed_twice_is_one_format() {
        assert_eq!(read("docx .docx").formats, vec!["docx"]);
    }

    #[test]
    fn a_dotted_word_without_a_letter_or_too_long_is_a_word() {
        assert_eq!(read(".5").formats, Vec::<String>::new());
        assert_eq!(read(".extensionlongue").formats, Vec::<String>::new());
        // The dot alone is nothing at all.
        assert_eq!(read(".").words, None);
        assert_eq!(read(". ,").words, None);
    }

    #[test]
    fn a_name_that_ends_with_its_extension_is_not_split() {
        // A whole name typed with its extension is the finder's business (the exact keys), not ours.
        let typed = read("rapport.docx");
        assert_eq!(typed.formats, Vec::<String>::new());
        assert_eq!(typed.words.as_deref(), Some("rapport.docx"));
    }

    #[test]
    fn a_year_a_month_and_a_day_are_dates() {
        assert_eq!(read("2026").date, day("2026"));
        assert_eq!(read("2026-02").date, day("2026-02"));
        assert_eq!(read("2026-02-06").date, day("2026-02-06"));
    }

    #[test]
    fn the_french_forms_fold_to_the_same_day() {
        assert_eq!(read("06/02/2026").date, day("2026-02-06"));
        assert_eq!(read("06-02-2026").date, day("2026-02-06"));
        assert_eq!(date_of("06/02/2026"), date_of("2026-02-06"));
    }

    #[test]
    fn the_words_stay_so_that_a_date_inside_an_identifier_keeps_matching_as_words() {
        assert_eq!(read("2026").words.as_deref(), Some("2026"));
        assert_eq!(read("2026-02-06").words.as_deref(), Some("2026-02-06"));
    }

    #[test]
    fn what_is_not_a_calendar_date_is_only_words() {
        for text in [
            "2026-13",
            "2026-02-30",
            "30/02/2026",
            "1234",
            "0999",
            "12345",
            "2026-2",
            "06.02.2026",
            "2026/02/06",
            "2026-02/06",
            "FAC-2026-014",
            "fac 2026",
            "2026 02",
            "rapport 2026",
        ] {
            assert_eq!(read(text).date, None, "{text}");
        }
    }

    #[test]
    fn a_date_can_come_with_an_extension() {
        let typed = read(".xlsx 2026");
        assert_eq!(typed.formats, vec!["xlsx"]);
        assert_eq!(typed.date, day("2026"));
    }

    #[test]
    fn the_query_loses_its_extensions_and_keeps_its_words() {
        let mut query = FinderQuery {
            text: Some("rapport .DOCX".to_string()),
            ..FinderQuery::default()
        };
        apply_typed_text(&mut query, &[]);
        assert_eq!(query.text.as_deref(), Some("rapport"));
        assert_eq!(query.formats, vec!["docx"]);
        assert_eq!(query.typed_date, None);
    }

    #[test]
    fn an_extension_alone_leaves_no_text_and_a_date_alone_sets_the_date() {
        let mut query = FinderQuery {
            text: Some("xlsx".to_string()),
            ..FinderQuery::default()
        };
        apply_typed_text(&mut query, &[]);
        assert_eq!(query.text, None);
        assert_eq!(query.formats, vec!["xlsx"]);

        let mut query = FinderQuery {
            text: Some("2026-02-06".to_string()),
            ..FinderQuery::default()
        };
        apply_typed_text(&mut query, &[]);
        assert_eq!(query.typed_date, day("2026-02-06"));
        assert_eq!(query.text.as_deref(), Some("2026-02-06"));
        assert!(query.formats.is_empty());
    }

    #[test]
    fn a_french_date_is_searched_as_the_same_words_as_the_iso_one() {
        for text in ["06/02/2026", "06-02-2026", "2026-02-06"] {
            let mut query = FinderQuery {
                text: Some(text.to_string()),
                ..FinderQuery::default()
            };
            apply_typed_text(&mut query, &[]);
            assert_eq!(query.text.as_deref(), Some("2026-02-06"), "{text}");
            assert_eq!(query.typed_date, day("2026-02-06"), "{text}");
        }
    }

    #[test]
    fn the_extensions_the_folder_holds_count_as_extensions() {
        let inventory = vec![InventoryFile {
            relative_path: "a/plan.xyz".to_string(),
            extension: "xyz".to_string(),
        }];
        let mut query = FinderQuery {
            text: Some("xyz".to_string()),
            ..FinderQuery::default()
        };
        apply_typed_text(&mut query, &inventory);
        assert_eq!(query.formats, vec!["xyz"]);
        assert_eq!(query.text, None);
    }

    #[test]
    fn the_formats_already_asked_for_are_kept_and_not_repeated() {
        let mut query = FinderQuery {
            text: Some("pdf .docx".to_string()),
            formats: vec!["pdf".to_string()],
            ..FinderQuery::default()
        };
        apply_typed_text(&mut query, &[]);
        assert_eq!(query.formats, vec!["pdf", "docx"]);
    }

    #[test]
    fn the_typed_extensions_are_returned_apart_from_the_formats_already_asked_for() {
        let mut query = FinderQuery {
            text: Some("rapport .docx XLSX".to_string()),
            formats: vec!["pdf".to_string()],
            ..FinderQuery::default()
        };
        assert_eq!(apply_typed_text(&mut query, &[]), vec!["docx", "xlsx"]);
        assert_eq!(query.formats, vec!["pdf", "docx", "xlsx"]);

        let mut query = FinderQuery {
            text: Some("rapport".to_string()),
            ..FinderQuery::default()
        };
        assert!(apply_typed_text(&mut query, &[]).is_empty());
    }

    #[test]
    fn punctuation_alone_is_left_as_typed_so_the_finder_finds_nothing() {
        for text in ["!!", ".", "...", "-"] {
            let mut query = FinderQuery {
                text: Some(text.to_string()),
                ..FinderQuery::default()
            };
            apply_typed_text(&mut query, &[]);
            assert_eq!(query.text.as_deref(), Some(text));
            assert!(query.formats.is_empty() && query.typed_date.is_none());
        }
    }

    #[test]
    fn no_text_is_left_alone() {
        let mut query = FinderQuery::default();
        apply_typed_text(&mut query, &[]);
        assert_eq!(query, FinderQuery::default());
    }

    #[test]
    fn nothing_typed_is_nothing() {
        assert_eq!(read(""), TypedText::default());
        assert_eq!(read("   "), TypedText::default());
    }
}
