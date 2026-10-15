//! Clean text for long documents (long-documents programme, lot 1; `docs/LONG-DOCUMENTS.md` 5.3).
//!
//! Applied between extraction and chunking, and only to a document above the long threshold
//! (`chunking::is_long`): a short document keeps today's chunks byte for byte (golden test).
//!
//! What it does, in this order, and nothing else:
//! 1. Unicode: ligatures expanded, unusual spaces (no-break, narrow no-break, thin) turned into a
//!    plain space, soft hyphens dropped, composed accents (NFC).
//! 2. Lines with no letter and no digit (rules, dot leaders) dropped.
//! 3. Headers and footers: a short line, with few numbers, that sits among the first or last two
//!    lines of at least half of the pages (digits compared as one symbol, so `Page 3 / 60` and
//!    `Page 4 / 60` are the same line) is removed from every page where it sits there.
//! 4. Hyphenation: a line ending in a hyphen after a letter, followed by a line that starts with a
//!    lowercase letter, is joined, inside a page and across a page break (the word stays on the
//!    page where it starts). A short list of prefixes that French writes with a real hyphen is
//!    protected, and a hyphen before a capital letter or a digit is never touched.

use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;

use crate::chunking::is_long;
use crate::extraction::ExtractedDocument;

/// Fewer pages than this and a "repeated" line proves nothing.
const MIN_PAGES_FOR_HEADERS: usize = 4;
/// A running header or footer is a short line.
const MAX_HEADER_CHARS: usize = 100;
/// ... with few numbers (a page number, a total, a version): a table row has more.
const MAX_HEADER_NUMBERS: usize = 3;
/// How many lines at the top and at the bottom of a page can be a header or a footer.
const EDGE_LINES: usize = 2;

/// Words that French writes with a hyphen even at the end of a line (compared without accents).
const KEEP_HYPHEN_AFTER: [&str; 11] = [
    "peut", "c", "ci", "vis", "tout", "apres", "avant", "sous", "entre", "grand", "quelque",
];

/// Clean a long document; a short one is returned as it came.
pub fn clean(mut document: ExtractedDocument) -> ExtractedDocument {
    if !is_long(&document) {
        return document;
    }
    let mut pages: Vec<Vec<String>> = document
        .pages
        .iter()
        .map(|page| {
            normalise(&page.text)
                .split('\n')
                .filter(|line| line.trim().is_empty() || line.chars().any(char::is_alphanumeric))
                .map(str::to_string)
                .collect()
        })
        .collect();
    strip_headers_and_footers(&mut pages);
    join_hyphenated_words(&mut pages);
    for (page, lines) in document.pages.iter_mut().zip(pages) {
        page.text = lines.join("\n");
    }
    document
}

fn normalise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\u{FB00}' => out.push_str("ff"),
            '\u{FB01}' => out.push_str("fi"),
            '\u{FB02}' => out.push_str("fl"),
            '\u{FB03}' => out.push_str("ffi"),
            '\u{FB04}' => out.push_str("ffl"),
            '\u{FB05}' | '\u{FB06}' => out.push_str("st"),
            '\u{00A0}' | '\u{202F}' | '\u{2007}' | '\u{2009}' | '\u{2002}' | '\u{2003}' => {
                out.push(' ')
            }
            '\u{00AD}' => {}
            other => out.push(other),
        }
    }
    out.nfc().collect()
}

// ---------------------------------------------------------------------------------------------
// Headers and footers
// ---------------------------------------------------------------------------------------------

/// The line with every run of digits replaced by one symbol and the spaces collapsed; `None` when it
/// cannot be a running header or footer.
fn header_key(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_HEADER_CHARS {
        return None;
    }
    let mut key = String::with_capacity(trimmed.len());
    let mut numbers = 0;
    let mut in_digits = false;
    let mut in_space = false;
    for c in trimmed.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                key.push('#');
                numbers += 1;
            }
            in_digits = true;
            in_space = false;
        } else if c.is_whitespace() {
            if !in_space {
                key.push(' ');
            }
            in_space = true;
            in_digits = false;
        } else {
            key.push(c);
            in_digits = false;
            in_space = false;
        }
    }
    (numbers <= MAX_HEADER_NUMBERS).then_some(key)
}

/// Positions (in the page's line list) of the first and last non-empty lines.
fn edge_positions(lines: &[String]) -> Vec<usize> {
    let filled: Vec<usize> = (0..lines.len())
        .filter(|i| !lines[*i].trim().is_empty())
        .collect();
    let mut edges: Vec<usize> = filled.iter().take(EDGE_LINES).copied().collect();
    for position in filled.iter().rev().take(EDGE_LINES) {
        if !edges.contains(position) {
            edges.push(*position);
        }
    }
    edges
}

fn strip_headers_and_footers(pages: &mut [Vec<String>]) {
    if pages.len() < MIN_PAGES_FOR_HEADERS {
        return;
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    for lines in pages.iter() {
        let mut seen: Vec<String> = Vec::new();
        for position in edge_positions(lines) {
            if let Some(key) = header_key(&lines[position]) {
                if !seen.contains(&key) {
                    seen.push(key);
                }
            }
        }
        for key in seen {
            *counts.entry(key).or_default() += 1;
        }
    }
    let needed = 3.max(pages.len().div_ceil(2));
    for lines in pages.iter_mut() {
        let doomed: Vec<usize> = edge_positions(lines)
            .into_iter()
            .filter(|position| {
                header_key(&lines[*position])
                    .and_then(|key| counts.get(&key))
                    .is_some_and(|count| *count >= needed)
            })
            .collect();
        if doomed.is_empty() {
            continue;
        }
        let mut index = 0;
        lines.retain(|_| {
            let keep = !doomed.contains(&index);
            index += 1;
            keep
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Hyphenation
// ---------------------------------------------------------------------------------------------

fn fold(word: &str) -> String {
    word.nfd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .collect::<String>()
        .to_lowercase()
}

/// Join `above` (ending in a hyphen) with the first word of `below` when both sides allow it.
/// Returns the new `below`, or `None` when nothing was joined.
fn join_pair(above: &mut String, below: &str) -> Option<String> {
    let head = above.trim_end();
    let before = head.strip_suffix('-')?;
    let last = before.chars().last()?;
    if !last.is_alphabetic() {
        return None;
    }
    let part: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_alphabetic())
        .collect::<Vec<char>>()
        .into_iter()
        .rev()
        .collect();
    if KEEP_HYPHEN_AFTER.contains(&fold(&part).as_str()) {
        return None;
    }
    let below = below.trim_start();
    if !below.chars().next()?.is_lowercase() {
        return None;
    }
    let (word, rest) = match below.find(char::is_whitespace) {
        Some(end) => (&below[..end], below[end..].trim_start()),
        None => (below, ""),
    };
    let joined = format!("{before}{word}");
    *above = joined;
    Some(rest.to_string())
}

fn join_hyphenated_words(pages: &mut [Vec<String>]) {
    // Inside a page.
    for lines in pages.iter_mut() {
        let mut i = 0;
        while i + 1 < lines.len() {
            let below = lines[i + 1].clone();
            let mut above = lines[i].clone();
            if let Some(rest) = join_pair(&mut above, &below) {
                lines[i] = above;
                if rest.is_empty() {
                    lines.remove(i + 1);
                } else {
                    lines[i + 1] = rest;
                }
            }
            i += 1;
        }
    }
    // Across a page break: the word stays on the page where it starts.
    for p in 0..pages.len().saturating_sub(1) {
        let Some(last) = (0..pages[p].len())
            .rev()
            .find(|i| !pages[p][*i].trim().is_empty())
        else {
            continue;
        };
        let Some(first) = (0..pages[p + 1].len()).find(|i| !pages[p + 1][*i].trim().is_empty())
        else {
            continue;
        };
        let mut above = pages[p][last].clone();
        let below = pages[p + 1][first].clone();
        if let Some(rest) = join_pair(&mut above, &below) {
            pages[p][last] = above;
            if rest.is_empty() {
                pages[p + 1].remove(first);
            } else {
                pages[p + 1][first] = rest;
            }
        }
    }
}
