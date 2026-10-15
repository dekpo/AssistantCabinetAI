//! The identifiers written in a question: codes such as `FAC-2026-014`, `756.1234.5678.97` or `2026-03-09`.
//! A passage that holds one comes first (`assembly::AssemblyOptions::pin_identifier`). This is the only thing the
//! words of a question still choose, and it reads codes, not words: a code is written the same way in the
//! question and in the document, whatever the language.
//!
//! The full-text index stores a code cut at its separators, so a code is searched as the phrase of its parts.

use unicode_normalization::UnicodeNormalization;

/// The alphanumeric pieces of `text`, composed (NFC) as the index stores them.
fn pieces_of(text: &str) -> Vec<String> {
    text.nfc()
        .collect::<String>()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|piece| !piece.is_empty())
        .map(str::to_string)
        .collect()
}

/// Words written as one code with separators inside: a digit, and a `-`, `.`, `/` or `_` that does not start
/// the word. Each is returned as its parts.
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
            let parts = pieces_of(piece);
            (parts.len() >= 2).then_some(parts)
        })
        .collect()
}

/// The identifiers written in the question as one full-text expression (their parts as phrases, alternatives
/// of one `OR`), or `None` when it holds none.
pub fn identifier_expression(question: &str) -> Option<String> {
    let phrases: Vec<String> = identifier_shaped(question)
        .iter()
        .map(|parts| format!("\"{}\"", parts.join(" ")))
        .collect();
    (!phrases.is_empty()).then(|| phrases.join(" OR "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_with_separators_is_read_as_the_phrase_of_its_parts() {
        assert_eq!(
            identifier_expression("Where is invoice FAC-2026-014 and 756.1234.5678.97?").as_deref(),
            Some("\"FAC 2026 014\" OR \"756 1234 5678 97\"")
        );
        assert_eq!(
            identifier_shaped("the date 2026-03-09."),
            vec![vec!["2026".to_string(), "03".to_string(), "09".to_string()]]
        );
    }

    #[test]
    fn a_plain_sentence_holds_no_identifier() {
        assert_eq!(
            identifier_expression("What is the annual fee of Camille?"),
            None
        );
        assert_eq!(identifier_expression("the 12 pages of re-work"), None);
        assert_eq!(identifier_expression("see -3 and .5"), None);
    }

    #[test]
    fn the_expression_is_accepted_by_the_full_text_index() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE VIRTUAL TABLE t USING fts5(chunk_id UNINDEXED, text);
                 INSERT INTO t VALUES ('a', 'The annual fee for invoice FAC-2026-014 is due.');
                 INSERT INTO t VALUES ('b', 'A fee that is yearly, nothing annual about it.');",
            )
            .unwrap();
        let expression = identifier_expression("which documents mention FAC-2026-014").unwrap();
        let matched: Vec<String> = connection
            .prepare("SELECT chunk_id FROM t WHERE t MATCH ?1 ORDER BY bm25(t)")
            .unwrap()
            .query_map([&expression], |row| row.get::<_, String>(0))
            .unwrap_or_else(|e| panic!("{expression}: {e}"))
            .map(|r| r.unwrap_or_else(|e| panic!("{expression}: {e}")))
            .collect();
        assert_eq!(matched, vec!["a".to_string()], "{expression}");
    }
}
