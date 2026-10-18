//! The sentence splitter of read-aloud: where a sentence ends in a text that may carry
//! abbreviations, initials, decimals, times and list numbers.
//!
//! Rules, the same for every language (the exceptions are data of the rule pack):
//! - `. ! ? ...` end a sentence when followed by a space or the end of the text, and when the next
//!   visible word does not start with a lower-case letter. Closing quotes, guillemets and brackets
//!   belong to the sentence.
//! - A dot after a known abbreviation, a title of the knowledge packs, a lone capital (an initial) or
//!   a list number at the start of a sentence does not end it. Decimals and times have no space
//!   after the dot, so they never end it.
//! - A line break ends a sentence; a blank line also ends the paragraph.
//! - A piece without any letter or digit is merged into its neighbour, never spoken alone.
//! - A sentence longer than [`MAX_SENTENCE_CHARS`] is cut at a clause mark, or at a space.

use super::speakable_rules::Rules;
use crate::knowledge::packs::PackSet;

/// The longest piece handed to a voice, in characters. A long sentence is cut at the last clause
/// mark below this, else at the last space.
pub const MAX_SENTENCE_CHARS: usize = 240;

/// One sentence of the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sentence {
    /// The sentence as written, with no leading or trailing white space.
    pub text: String,
    /// A blank line, or the end of the text, follows: the pause after it is longer.
    pub paragraph_end: bool,
    /// The sentence continues in the next piece: it was cut for its length.
    pub continues: bool,
}

fn is_terminal(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | '\u{2026}')
}

fn is_closer(c: char) -> bool {
    matches!(
        c,
        '"' | '\'' | '\u{bb}' | '\u{201d}' | '\u{2019}' | ')' | ']' | '}'
    )
}

fn is_opener(c: char) -> bool {
    matches!(
        c,
        '"' | '\''
            | '\u{ab}'
            | '\u{201c}'
            | '\u{2018}'
            | '('
            | '['
            | '{'
            | '\u{2014}'
            | '\u{2013}'
            | '-'
            | '\u{bf}'
            | '\u{a1}'
    )
}

/// The first visible character at or after `from`, skipping white space and opening marks.
fn next_visible(chars: &[char], mut from: usize) -> Option<char> {
    while from < chars.len() {
        let c = chars[from];
        if !(c.is_whitespace() || is_opener(c)) {
            return Some(c);
        }
        from += 1;
    }
    None
}

/// The word glued to the left of `dot`, without opening marks.
fn token_before(chars: &[char], dot: usize) -> String {
    let mut start = dot;
    while start > 0 && !chars[start - 1].is_whitespace() {
        start -= 1;
    }
    let raw: String = chars[start..dot].iter().collect();
    raw.trim_start_matches(is_opener).to_string()
}

/// Is the dot at `dot` part of an abbreviation, a title, an initial or a list number?
fn dot_is_not_final(
    chars: &[char],
    dot: usize,
    sentence_start: usize,
    after: usize,
    rules: &Rules,
    titles: &PackSet,
) -> bool {
    let token = token_before(chars, dot);
    if token.is_empty() {
        return false;
    }
    let lower = token.to_lowercase();
    if rules.sentences.never_end.contains(&lower) {
        return true;
    }
    if token.chars().next().is_some_and(char::is_uppercase) && titles.title_spoken(&token).is_some()
    {
        return true;
    }
    if rules.sentences.ends_before_capital.contains(&lower) {
        return !matches!(next_visible(chars, after), Some(c) if c.is_uppercase());
    }
    let mut it = token.chars();
    // An initial, one capital letter.
    if let (Some(c), None) = (it.next(), it.next()) {
        if c.is_alphabetic() && c.is_uppercase() {
            return true;
        }
    }
    // A list number at the start of the sentence, "1. Take it".
    if token.chars().all(|c| c.is_ascii_digit()) {
        let token_start = dot - token.chars().count();
        let only_blanks_before = chars[sentence_start..token_start]
            .iter()
            .all(|c| c.is_whitespace() || is_opener(*c));
        if only_blanks_before {
            return true;
        }
    }
    false
}

/// Splits `text` into sentences. Never fails: text with no sentence mark is one sentence.
pub fn split_sentences(text: &str, rules: &Rules, titles: &PackSet) -> Vec<Sentence> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    // (first char, end char exclusive, paragraph end)
    let mut spans: Vec<(usize, usize, bool)> = Vec::new();

    let emit = |spans: &mut Vec<(usize, usize, bool)>, from: usize, to: usize| {
        let (mut a, mut b) = (from, to);
        while a < b && chars[a].is_whitespace() {
            a += 1;
        }
        while b > a && chars[b - 1].is_whitespace() {
            b -= 1;
        }
        if a >= b {
            return;
        }
        if chars[a..b].iter().any(|c| c.is_alphanumeric()) {
            spans.push((a, b, false));
        } else if let Some(previous) = spans.last_mut() {
            // Only marks: glued to the previous sentence on the same line, else dropped.
            if !chars[previous.1..a].contains(&'\n') {
                previous.1 = b;
            }
        }
    };

    let mut start = 0usize;
    let mut i = 0usize;
    while i < n {
        let c = chars[i];
        if c.is_whitespace() {
            let gap_start = i;
            let mut newlines = 0;
            while i < n && chars[i].is_whitespace() {
                if chars[i] == '\n' {
                    newlines += 1;
                }
                i += 1;
            }
            if newlines >= 1 {
                emit(&mut spans, start, gap_start);
                if newlines >= 2 {
                    if let Some(last) = spans.last_mut() {
                        last.2 = true;
                    }
                }
                start = i;
            } else if gap_start == start {
                start = i;
            }
            continue;
        }
        if is_terminal(c) {
            let mut j = i;
            while j < n && is_terminal(chars[j]) {
                j += 1;
            }
            let run_end = j;
            // Closing marks belong to the sentence; only a guillemet may follow a space.
            let mut end = run_end;
            loop {
                let mut k = end;
                while k < n && chars[k].is_whitespace() && chars[k] != '\n' {
                    k += 1;
                }
                if k < n && is_closer(chars[k]) && (k == end || chars[k] == '\u{bb}') {
                    end = k + 1;
                } else {
                    break;
                }
            }
            let at_end = end >= n;
            let mut boundary = at_end || chars[end].is_whitespace();
            if boundary && !at_end {
                // A line break always closes the sentence, whatever comes next.
                let gap_has_newline = chars[end..]
                    .iter()
                    .take_while(|c| c.is_whitespace())
                    .any(|c| *c == '\n');
                if !gap_has_newline {
                    if matches!(next_visible(&chars, end), Some(next) if next.is_lowercase()) {
                        boundary = false;
                    }
                    let single_dot = run_end - i == 1 && c == '.';
                    if boundary
                        && single_dot
                        && dot_is_not_final(&chars, i, start, run_end, rules, titles)
                    {
                        boundary = false;
                    }
                }
            }
            if boundary {
                emit(&mut spans, start, end);
                start = end;
                i = end;
            } else {
                i = run_end;
            }
            continue;
        }
        i += 1;
    }
    emit(&mut spans, start, n);

    if let Some(last) = spans.last_mut() {
        last.2 = true;
    }
    spans
        .into_iter()
        .flat_map(|(a, b, paragraph_end)| {
            cut_long(&chars[a..b].iter().collect::<String>(), paragraph_end)
        })
        .collect()
}

/// Cuts a sentence that is too long at the last clause mark below the cap, else at the last space,
/// else at the cap.
fn cut_long(text: &str, paragraph_end: bool) -> Vec<Sentence> {
    let chars: Vec<char> = text.chars().collect();
    let mut pieces: Vec<String> = Vec::new();
    let mut start = 0usize;
    while chars.len() - start > MAX_SENTENCE_CHARS {
        let window = &chars[start..start + MAX_SENTENCE_CHARS];
        let cut = window
            .iter()
            .rposition(|c| matches!(c, ',' | ';' | ':'))
            .map(|p| p + 1)
            .filter(|p| *p > MAX_SENTENCE_CHARS / 3)
            .or_else(|| {
                window
                    .iter()
                    .rposition(|c| c.is_whitespace())
                    .filter(|p| *p > 0)
            })
            .unwrap_or(MAX_SENTENCE_CHARS);
        let piece: String = chars[start..start + cut].iter().collect();
        pieces.push(piece.trim().to_string());
        start += cut;
        while start < chars.len() && chars[start].is_whitespace() {
            start += 1;
        }
    }
    let tail: String = chars[start..].iter().collect();
    pieces.push(tail.trim().to_string());
    pieces.retain(|piece| !piece.is_empty());
    let count = pieces.len();
    pieces
        .into_iter()
        .enumerate()
        .map(|(index, text)| Sentence {
            text,
            paragraph_end: paragraph_end && index + 1 == count,
            continues: index + 1 < count,
        })
        .collect()
}
