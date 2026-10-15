//! Checking an answer against the passages it was written from (retrieval-quality programme, lot E). Two
//! things are checked, and neither is ever rewritten: what the answer **quotes** between guillemets must be in
//! one of the passages, and a **reference to a passage** (`[3]`) must be to a passage that exists. An anomaly
//! is reported beside the answer and the reader decides (`docs/SELECTION-AND-MEMORY.md`).
//!
//! The comparison reads the text the way a person does: quotation marks and apostrophes of every style are
//! one, dashes are one, a page break or a line break is a space. The figure check (`number_check`) reads a
//! source with the same line joining.

use unicode_normalization::UnicodeNormalization;

/// `text` in one form for comparing: composed accents, one kind of quote, apostrophe and dash, no soft
/// hyphen, every run of white space (a line or a page break, a no-break space) one space, trimmed.
pub fn normalise(text: &str) -> String {
    let mapped: String = text
        .nfc()
        .filter(|c| *c != '\u{ad}')
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{2032}' | '\u{2bc}' | '`' | '\u{b4}' => '\'',
            '\u{ab}' | '\u{bb}' | '\u{201c}' | '\u{201d}' | '\u{201e}' => '"',
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
            other => other,
        })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A source with every line break taken for a space: a figure or a phrase that a page cut in two is whole.
pub fn join_lines(text: &str) -> String {
    text.replace("\r\n", " ").replace(['\n', '\r'], " ")
}

/// What `answer` quotes: the text between guillemets or double quotes, of at least `min_words` words. A short
/// mark ("yes") is not a quotation. Not normalised: as written, apart from the spaces around it.
pub fn quotations_in(answer: &str, min_words: usize) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut inside: Option<(char, String)> = None;
    for c in answer.chars() {
        match (&mut inside, c) {
            (None, '\u{ab}') => inside = Some(('\u{bb}', String::new())),
            (None, '\u{201c}') => inside = Some(('\u{201d}', String::new())),
            (None, '"') => inside = Some(('"', String::new())),
            (Some((close, text)), c) if c == *close => {
                let quotation = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if quotation.split_whitespace().count() >= min_words && !found.contains(&quotation)
                {
                    found.push(quotation);
                }
                inside = None;
            }
            (Some((_, text)), c) => text.push(c),
            (None, _) => {}
        }
    }
    found
}

/// A quotation is cut into pieces at an ellipsis (`...`, `\u{2026}`, `[...]`): each piece must be in a source.
fn pieces(quotation: &str) -> Vec<String> {
    let normalised = normalise(quotation)
        .replace("[\u{2026}]", "\u{2026}")
        .replace("[...]", "\u{2026}")
        .replace("...", "\u{2026}");
    normalised
        .split('\u{2026}')
        .map(|piece| {
            piece
                .trim_matches(|c: char| c.is_whitespace() || c == '"')
                .to_string()
        })
        .filter(|piece| !piece.is_empty())
        .collect()
}

/// The quotations of `answer` that are in none of `sources`, as written, each once. A piece of a quotation
/// shorter than three words (what is left around an ellipsis) is not held against it.
pub fn unverified_quotations(answer: &str, sources: &[&str], min_words: usize) -> Vec<String> {
    // Compared without regard to case: a model starts its quotation with a capital or without one.
    let known: Vec<String> = sources
        .iter()
        .map(|source| normalise(&join_lines(source)).to_lowercase())
        .collect();
    quotations_in(answer, min_words)
        .into_iter()
        .filter(|quotation| {
            pieces(quotation).iter().any(|piece| {
                let piece = piece.to_lowercase();
                piece.split_whitespace().count() >= 3
                    && !known.iter().any(|source| source.contains(piece.as_str()))
            })
        })
        .collect()
}

/// The references like `[3]` in `answer` that name no passage: 0, or more than `passages`. Each once, ascending.
pub fn unknown_passage_numbers(answer: &str, passages: usize) -> Vec<u32> {
    let chars: Vec<char> = answer.chars().collect();
    let mut unknown: Vec<u32> = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '[' {
            let mut end = index + 1;
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }
            if end > index + 1 && end < chars.len() && chars[end] == ']' {
                let digits: String = chars[index + 1..end].iter().collect();
                if let Ok(number) = digits.parse::<u32>() {
                    if (number == 0 || number as usize > passages) && !unknown.contains(&number) {
                        unknown.push(number);
                    }
                }
                index = end + 1;
                continue;
            }
        }
        index += 1;
    }
    unknown.sort_unstable();
    unknown
}
