//! Dates and amounts as facts (`ld_facts`): what a passage states, written once in a normal form at the
//! Analyse, so that a question about `12/03/2026` finds the passage that says `12 mars 2026`, and a question
//! about `1200 euros` the passage that says `1 200,00 EUR`, which the word search cannot do.
//!
//! **Deterministic, no model.** The facts are read from the text the Analyse stored, inside the file-profile
//! pass of that Analyse (`file_profile::store`), and nowhere else. The dates are read by the reader of the
//! header (`file_profile::header::date_spans`) and the numbers by the scanner that checks an answer
//! (`number_check::scan_spans`): there is no second parser. What is **data, not code**: the month names and
//! the currencies, in `resources/profile/*/<locale>.json`, merged for every pack shipped.
//!
//! **A fact never stands alone.** Each keeps its passage (`chunk_id`), its page and where it is written, and
//! is used only to put that passage among the candidates; the passage is shown and the answer's figures are
//! checked against it like any other (the "Passages found" block, the "to check" note).
//!
//! Document passages only. The tables of the data folder are never read here.

use rusqlite::{params, Connection};

use crate::error::AppError;
use crate::file_profile::header::date_spans;
use crate::file_profile::vocabulary::ProfileVocabulary;
use crate::number_check::scan_spans;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS ld_facts (
    fact_id INTEGER PRIMARY KEY AUTOINCREMENT,
    relative_path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    kind TEXT NOT NULL,
    value TEXT NOT NULL,
    currency TEXT,
    surface TEXT NOT NULL,
    chunk_id TEXT NOT NULL,
    page INTEGER NOT NULL,
    char_offset INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS ld_facts_by_value ON ld_facts (kind, value);
CREATE INDEX IF NOT EXISTS ld_facts_by_path ON ld_facts (relative_path);
";

/// The most facts kept for one passage and for one document: a table of dates or a ledger must not make the
/// table of facts larger than the text it comes from.
const MAX_FACTS_PER_PASSAGE: usize = 100;
const MAX_FACTS_PER_DOCUMENT: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Date,
    Amount,
}

impl Kind {
    pub fn code(self) -> &'static str {
        match self {
            Kind::Date => "date",
            Kind::Amount => "amount",
        }
    }
}

/// One thing a passage states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    pub kind: Kind,
    /// `YYYY-MM-DD` for a date; the amount with two decimals (`1200.00`) for an amount.
    pub value: String,
    /// The currency code of an amount (`EUR`); `None` for a date.
    pub currency: Option<String>,
    /// As written in the passage.
    pub surface: String,
    /// Where it starts, in characters from the start of the passage.
    pub offset: usize,
}

/// The vocabulary a question is read with: the month names and the currencies, which belong to no pack in
/// particular (every pack shipped is read), built once.
pub fn shipped_vocabulary() -> &'static ProfileVocabulary {
    static SHIPPED: std::sync::OnceLock<ProfileVocabulary> = std::sync::OnceLock::new();
    SHIPPED.get_or_init(|| ProfileVocabulary::load(&[]).unwrap_or_default())
}

/// An amount in its normal form: two decimals, no grouping.
fn amount_value(value: f64) -> String {
    format!("{value:.2}")
}

fn is_blank(ch: char) -> bool {
    matches!(ch, ' ' | '\u{a0}' | '\u{202f}')
}

/// The currency written right after the number that ends at `end`: a word (`EUR`, `euros`, `fr.`) or a
/// symbol, after at most two blanks. Returns the code and where the writing ends.
fn currency_after(
    chars: &[char],
    end: usize,
    vocabulary: &ProfileVocabulary,
) -> Option<(String, usize)> {
    let mut at = end;
    while at < chars.len() && is_blank(chars[at]) && at - end < 2 {
        at += 1;
    }
    let first = *chars.get(at)?;
    if first.is_alphabetic() {
        let mut stop = at;
        while stop < chars.len() && chars[stop].is_alphabetic() {
            stop += 1;
        }
        let word: String = chars[at..stop].iter().collect();
        if chars.get(stop) == Some(&'.') {
            if let Some(code) = vocabulary.currency_of(&format!("{word}.")) {
                return Some((code.to_string(), stop + 1));
            }
        }
        return vocabulary
            .currency_of(&word)
            .map(|code| (code.to_string(), stop));
    }
    if !first.is_alphanumeric() && !first.is_whitespace() {
        return vocabulary
            .currency_of(&first.to_string())
            .map(|code| (code.to_string(), at + 1));
    }
    None
}

/// The currency written right before the number that starts at `start`. Returns the code and where the
/// writing starts.
fn currency_before(
    chars: &[char],
    start: usize,
    vocabulary: &ProfileVocabulary,
) -> Option<(String, usize)> {
    let mut at = start;
    while at > 0 && is_blank(chars[at - 1]) && start - at < 2 {
        at -= 1;
    }
    if at == 0 {
        return None;
    }
    let last = chars[at - 1];
    if last.is_alphabetic() || last == '.' {
        let word_end = if last == '.' { at - 1 } else { at };
        let mut from = word_end;
        while from > 0 && chars[from - 1].is_alphabetic() {
            from -= 1;
        }
        if from == word_end {
            return None;
        }
        let word: String = chars[from..word_end].iter().collect();
        if last == '.' {
            if let Some(code) = vocabulary.currency_of(&format!("{word}.")) {
                return Some((code.to_string(), from));
            }
        }
        return vocabulary
            .currency_of(&word)
            .map(|code| (code.to_string(), from));
    }
    if !last.is_alphanumeric() && !last.is_whitespace() {
        return vocabulary
            .currency_of(&last.to_string())
            .map(|code| (code.to_string(), at - 1));
    }
    None
}

/// Every date and every amount a text states, in order of appearance. An amount is a number with a currency
/// written right beside it; a number with none is not a fact (a page, a count, a reference).
pub fn extract(text: &str, vocabulary: &ProfileVocabulary) -> Vec<Fact> {
    let mut facts: Vec<Fact> = Vec::new();

    // The dates, line by line (the reader of dates reads one line), keeping where each line starts.
    let mut line_start_byte = 0usize;
    let mut line_start_char = 0usize;
    for line in text.split_inclusive('\n') {
        for span in date_spans(line, vocabulary) {
            facts.push(Fact {
                kind: Kind::Date,
                value: span.iso,
                currency: None,
                surface: line[span.start..span.end].to_string(),
                offset: line_start_char + line[..span.start].chars().count(),
            });
        }
        line_start_byte += line.len();
        line_start_char += line.chars().count();
    }
    debug_assert_eq!(line_start_byte, text.len());

    let chars: Vec<char> = text.chars().collect();
    for (number, start, end) in scan_spans(text, 1) {
        // The first reading of "1,500" is the thousand: a currency amount has no third decimal.
        let Some(value) = number.values.first() else {
            continue;
        };
        let (currency, from, to) =
            if let Some((code, stop)) = currency_after(&chars, end, vocabulary) {
                (code, start, stop)
            } else if let Some((code, first)) = currency_before(&chars, start, vocabulary) {
                (code, first, end)
            } else {
                continue;
            };
        facts.push(Fact {
            kind: Kind::Amount,
            value: amount_value(*value),
            currency: Some(currency),
            surface: chars[from..to].iter().collect(),
            offset: from,
        });
    }
    facts.sort_by_key(|fact| fact.offset);
    facts.truncate(MAX_FACTS_PER_PASSAGE);
    facts
}

/// Write the facts of the passages stored for `relative_path`, replacing the ones it had. Called by the
/// file-profile pass of an Analyse, inside its transaction. Returns how many were written.
pub fn write(
    connection: &Connection,
    relative_path: &str,
    vocabulary: &ProfileVocabulary,
) -> Result<usize, AppError> {
    let unavailable = |_| AppError::IndexUnavailable;
    forget(connection, relative_path)?;
    let sha256: Option<String> = connection
        .query_row(
            "SELECT sha256 FROM documents WHERE relative_path = ?1",
            [relative_path],
            |row| row.get(0),
        )
        .ok();
    let Some(sha256) = sha256 else {
        return Ok(0);
    };
    // Row by row: a book's passages are never all in memory.
    let mut passages = connection
        .prepare(
            "SELECT chunk_id, page_number, text FROM chunks WHERE relative_path = ?1 ORDER BY rowid",
        )
        .map_err(unavailable)?;
    let mut insert = connection
        .prepare(
            "INSERT INTO ld_facts
             (relative_path, sha256, kind, value, currency, surface, chunk_id, page, char_offset)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .map_err(unavailable)?;
    let mut rows = passages
        .query(params![relative_path])
        .map_err(unavailable)?;
    let mut written = 0usize;
    while let Some(row) = rows.next().map_err(unavailable)? {
        let chunk_id: String = row.get(0).map_err(unavailable)?;
        let page: i64 = row.get(1).map_err(unavailable)?;
        let text: String = row.get(2).map_err(unavailable)?;
        for fact in extract(&text, vocabulary) {
            if written >= MAX_FACTS_PER_DOCUMENT {
                return Ok(written);
            }
            insert
                .execute(params![
                    relative_path,
                    sha256,
                    fact.kind.code(),
                    fact.value,
                    fact.currency,
                    fact.surface,
                    chunk_id,
                    page,
                    fact.offset as i64,
                ])
                .map_err(unavailable)?;
            written += 1;
        }
    }
    Ok(written)
}

/// Drop the facts of one document.
pub fn forget(connection: &Connection, relative_path: &str) -> Result<(), AppError> {
    connection
        .execute(
            "DELETE FROM ld_facts WHERE relative_path = ?1",
            [relative_path],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    Ok(())
}

/// The passages that state any of the dates and amounts written in `question`, found by their normal form,
/// whatever way the passage writes them. Held to `relative_paths` when given. Nothing when the question
/// states no date and no amount.
pub fn passages_for_question(
    connection: &Connection,
    question: &str,
    vocabulary: &ProfileVocabulary,
    relative_paths: Option<&[String]>,
    limit: usize,
) -> Result<Vec<String>, AppError> {
    let wanted = extract(question, vocabulary);
    if wanted.is_empty() || relative_paths.is_some_and(|paths| paths.is_empty()) {
        return Ok(Vec::new());
    }
    let unavailable = |_| AppError::IndexUnavailable;
    let paths = relative_paths
        .map(|paths| serde_json::to_string(paths).map_err(|_| AppError::Internal))
        .transpose()?;
    let mut found: Vec<String> = Vec::new();
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT chunk_id FROM ld_facts
             WHERE kind = ?1 AND value = ?2 AND (currency IS NULL OR currency = ?3 OR ?3 IS NULL)
               AND (?4 IS NULL OR relative_path IN (SELECT value FROM json_each(?4)))
               AND EXISTS (SELECT 1 FROM chunks WHERE chunks.chunk_id = ld_facts.chunk_id)
             LIMIT ?5",
        )
        .map_err(unavailable)?;
    for fact in &wanted {
        let rows = statement
            .query_map(
                params![
                    fact.kind.code(),
                    fact.value,
                    fact.currency,
                    paths,
                    limit as i64
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(unavailable)?;
        for row in rows {
            let chunk_id = row.map_err(unavailable)?;
            if !found.contains(&chunk_id) {
                found.push(chunk_id);
            }
        }
    }
    found.truncate(limit);
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocabulary() -> ProfileVocabulary {
        ProfileVocabulary::load(&["health-ch"]).expect("loads")
    }

    fn found(text: &str) -> Vec<(Kind, String, Option<String>, String)> {
        extract(text, &vocabulary())
            .into_iter()
            .map(|fact| (fact.kind, fact.value, fact.currency, fact.surface))
            .collect()
    }

    #[test]
    fn a_date_is_found_in_every_format_and_normalised() {
        let text = "Facture du 12 mars 2026, regle 14/03/2026. Echeance 2026-04-12.";
        let dates: Vec<String> = found(text).into_iter().map(|f| f.1).collect();
        assert_eq!(dates, vec!["2026-03-12", "2026-03-14", "2026-04-12"]);
        let first = &extract(text, &vocabulary())[0];
        assert_eq!(first.surface, "12 mars 2026");
        assert_eq!(first.offset, text.find("12 mars").unwrap());
    }

    #[test]
    fn an_amount_is_a_number_beside_a_currency_in_any_writing() {
        for (text, value, currency, surface) in [
            ("Total : 1 200,00 EUR", "1200.00", "EUR", "1 200,00 EUR"),
            ("Total 1200 euros TTC", "1200.00", "EUR", "1200 euros"),
            ("Montant : 1'200.50 CHF", "1200.50", "CHF", "1'200.50 CHF"),
            (
                "Prix 85,5\u{a0}\u{20ac}",
                "85.50",
                "EUR",
                "85,5\u{a0}\u{20ac}",
            ),
            ("Cout : CHF 300.00", "300.00", "CHF", "CHF 300.00"),
            ("Cout : \u{20ac} 40", "40.00", "EUR", "\u{20ac} 40"),
            ("Frais 120 fr. au total", "120.00", "CHF", "120 fr."),
        ] {
            let facts = found(text);
            assert_eq!(
                facts,
                vec![(
                    Kind::Amount,
                    value.to_string(),
                    Some(currency.to_string()),
                    surface.to_string()
                )],
                "{text}"
            );
        }
    }

    #[test]
    fn a_number_with_no_currency_is_no_fact_and_a_date_is_not_an_amount() {
        assert!(found("Page 12 sur 40, reference 2026-0417, 3 exemplaires.").is_empty());
        let facts = found("On 12/03/2026 : 5 euros");
        assert_eq!(facts.len(), 2, "{facts:?}");
        assert_eq!(facts[0].0, Kind::Date);
        assert_eq!(facts[1].1, "5.00");
    }

    #[test]
    fn the_offset_counts_characters_not_bytes() {
        let text = "Caf\u{e9} \u{e0} 12,50 \u{20ac}";
        let fact = &extract(text, &vocabulary())[0];
        assert_eq!(fact.offset, text.chars().position(|c| c == '1').unwrap());
    }

    #[test]
    fn a_passage_keeps_at_most_the_cap() {
        let text = "1 EUR ".repeat(MAX_FACTS_PER_PASSAGE + 20);
        assert_eq!(extract(&text, &vocabulary()).len(), MAX_FACTS_PER_PASSAGE);
    }
}
