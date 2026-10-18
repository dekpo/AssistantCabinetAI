//! Markdown to speakable blocks. A small line-based reader for the subset the answers use: headings,
//! lists, quotes, tables, code blocks, emphasis, links and rules. No parser dependency: the spoken
//! text is derived in Rust, by code, never by the model.
//!
//! The reader produces [`Block`]s: a line of text with a flag that a paragraph ends there, or a note
//! standing for something that is not read (a large table, a block of code). Inline cleanup (links,
//! emphasis, addresses, paths, symbols) is [`clean_inline`], applied to every line whatever its kind.

use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

/// A table up to this many body rows and columns is read row by row; a bigger one is replaced by a
/// note, because a listener cannot follow a grid.
pub const SMALL_TABLE_ROWS: usize = 8;
pub const SMALL_TABLE_COLUMNS: usize = 5;

/// What stands in the place of a part that is not read aloud. Machine codes: the interface says them
/// in the user's language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteCode {
    /// A table too large to be followed by ear; `count` is its number of rows.
    TableOmitted,
    /// A block of code; `count` is its number of lines.
    CodeOmitted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// A line of text still in its written form, and whether a paragraph ends after it.
    Text {
        text: String,
        paragraph_end: bool,
    },
    Note {
        code: NoteCode,
        count: usize,
    },
}

/// The blocks of a part. `markdown` false reads plain text: every line is a block, nothing is
/// interpreted.
pub fn blocks(text: &str, markdown: bool) -> Vec<Block> {
    let normalised = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalised.split('\n').collect();
    let mut out: Vec<Block> = Vec::new();
    if markdown {
        read_markdown(&lines, &mut out);
    } else {
        for line in &lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                end_paragraph(&mut out);
            } else {
                out.push(Block::Text {
                    text: trimmed.to_string(),
                    paragraph_end: false,
                });
            }
        }
    }
    end_paragraph(&mut out);
    out
}

fn end_paragraph(out: &mut [Block]) {
    if let Some(Block::Text { paragraph_end, .. }) = out.last_mut() {
        *paragraph_end = true;
    }
}

fn regex(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("pattern compiles"))
}

fn list_item(line: &str) -> Option<String> {
    static ITEM: OnceLock<Regex> = OnceLock::new();
    let found = regex(&ITEM, r"^\s*(?:[-*+]|\d{1,3}[.)])\s+(.*)$").captures(line)?;
    let mut content = found.get(1)?.as_str().trim().to_string();
    for mark in ["[ ] ", "[x] ", "[X] "] {
        if let Some(rest) = content.strip_prefix(mark) {
            content = rest.to_string();
        }
    }
    Some(content)
}

fn heading(line: &str) -> Option<String> {
    static HEADING: OnceLock<Regex> = OnceLock::new();
    let found = regex(&HEADING, r"^\s{0,3}#{1,6}\s+(.*?)\s*#*\s*$").captures(line)?;
    Some(found.get(1)?.as_str().to_string())
}

fn fence(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") {
        Some("```")
    } else if trimmed.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

fn is_rule(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    compact.chars().count() >= 3
        && ["-", "*", "_"]
            .iter()
            .any(|mark| compact.chars().all(|c| mark.starts_with(c)))
}

fn is_separator_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('-')
        && trimmed.contains('|')
        && trimmed
            .chars()
            .all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
}

fn table_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn read_markdown(lines: &[&str], out: &mut Vec<Block>) {
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if trimmed.is_empty() {
            end_paragraph(out);
            i += 1;
            continue;
        }
        if let Some(marker) = fence(line) {
            let mut count = 0usize;
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(marker) {
                count += 1;
                i += 1;
            }
            i += 1;
            end_paragraph(out);
            out.push(Block::Note {
                code: NoteCode::CodeOmitted,
                count,
            });
            continue;
        }
        if trimmed.contains('|') && i + 1 < lines.len() && is_separator_row(lines[i + 1]) {
            let header = table_cells(line);
            let mut rows: Vec<Vec<String>> = Vec::new();
            i += 2;
            while i < lines.len() && !lines[i].trim().is_empty() && lines[i].contains('|') {
                rows.push(table_cells(lines[i]));
                i += 1;
            }
            end_paragraph(out);
            push_table(&header, &rows, out);
            continue;
        }
        if is_rule(trimmed) {
            end_paragraph(out);
            i += 1;
            continue;
        }
        if let Some(text) = heading(line) {
            out.push(Block::Text {
                text,
                paragraph_end: true,
            });
            i += 1;
            continue;
        }
        if let Some(mut item) = list_item(line) {
            i += 1;
            // Indented lines that are not items continue this item.
            while i < lines.len()
                && !lines[i].trim().is_empty()
                && lines[i].starts_with([' ', '\t'])
                && list_item(lines[i]).is_none()
            {
                item.push(' ');
                item.push_str(lines[i].trim());
                i += 1;
            }
            out.push(Block::Text {
                text: item,
                paragraph_end: false,
            });
            continue;
        }
        let unquoted = trimmed.trim_start_matches(['>', ' ']);
        out.push(Block::Text {
            text: unquoted.to_string(),
            paragraph_end: false,
        });
        i += 1;
    }
}

/// A small table is read one sentence per row, "Column: value, Column: value"; a large one becomes
/// a note.
fn push_table(header: &[String], rows: &[Vec<String>], out: &mut Vec<Block>) {
    if rows.len() > SMALL_TABLE_ROWS || header.len() > SMALL_TABLE_COLUMNS {
        out.push(Block::Note {
            code: NoteCode::TableOmitted,
            count: rows.len(),
        });
        return;
    }
    for row in rows {
        let parts: Vec<String> = row
            .iter()
            .enumerate()
            .filter(|(_, cell)| !cell.is_empty() && cell.as_str() != "-")
            .map(|(index, cell)| match header.get(index) {
                Some(name) if !name.is_empty() => format!("{name}: {cell}"),
                _ => cell.clone(),
            })
            .collect();
        if !parts.is_empty() {
            out.push(Block::Text {
                text: parts.join(", "),
                paragraph_end: false,
            });
        }
    }
    end_paragraph(out);
}

// -------------------------------------------------------------------------------------------------
// Inline cleanup
// -------------------------------------------------------------------------------------------------

/// File extensions that make a word a file name. Not words of a language.
const FILE_EXTENSIONS: &str = "pdf|docx?|xlsx?|pptx?|csv|txt|md|odt|ods|odp|rtf|png|jpe?g|gif|tiff?|eml|msg|zip|json|xml|html?";

fn is_pictograph(c: char) -> bool {
    matches!(c as u32,
        0x1F000..=0x1FAFF   // emoji, symbols and pictographs
        | 0x2600..=0x27BF   // miscellaneous symbols and dingbats
        | 0x2190..=0x21FF   // arrows
        | 0x2B00..=0x2BFF   // arrows and stars
        | 0x2300..=0x23FF   // technical symbols
        | 0xFE00..=0xFE0F   // variation selectors
        | 0x200B..=0x200F   // zero-width marks
        | 0x2028..=0x202E
        | 0x2060..=0x2064
        | 0xE000..=0xF8FF   // private use
    ) || c == '\u{2022}'
        || c == '\u{25cf}'
        || c == '\u{25aa}'
        || c == '\u{25a0}'
        || c == '\u{2713}'
        || c == '\u{2717}'
        || c == '\u{b7}'
}

/// Removes what a listener should not hear from one line: Markdown marks, addresses, emoji, and
/// reduces paths and file names to words. The text on screen is never changed; this works on a copy.
pub fn clean_inline(text: &str) -> String {
    static IMAGE: OnceLock<Regex> = OnceLock::new();
    static LINK: OnceLock<Regex> = OnceLock::new();
    static REFERENCE_LINK: OnceLock<Regex> = OnceLock::new();
    static AUTOLINK: OnceLock<Regex> = OnceLock::new();
    static URL: OnceLock<Regex> = OnceLock::new();
    static HTML: OnceLock<Regex> = OnceLock::new();
    static CODE: OnceLock<Regex> = OnceLock::new();
    static BOLD_STAR: OnceLock<Regex> = OnceLock::new();
    static BOLD_UNDERSCORE: OnceLock<Regex> = OnceLock::new();
    static STRIKE: OnceLock<Regex> = OnceLock::new();
    static ITALIC_STAR: OnceLock<Regex> = OnceLock::new();
    static ITALIC_UNDERSCORE: OnceLock<Regex> = OnceLock::new();
    static NUMBERED_REFERENCE: OnceLock<Regex> = OnceLock::new();
    static DRIVE_PATH: OnceLock<Regex> = OnceLock::new();
    static RELATIVE_PATH: OnceLock<Regex> = OnceLock::new();
    static FILE_NAME: OnceLock<Regex> = OnceLock::new();

    let mut s = text.to_string();
    s = regex(&IMAGE, r"!\[([^\]]*)\]\([^)]*\)")
        .replace_all(&s, "$1")
        .into_owned();
    s = regex(&LINK, r"\[([^\]]+)\]\([^)]*\)")
        .replace_all(&s, "$1")
        .into_owned();
    s = regex(&REFERENCE_LINK, r"\[([^\]]+)\]\[[^\]]*\]")
        .replace_all(&s, "$1")
        .into_owned();
    s = regex(&AUTOLINK, r"<https?://[^>\s]+>")
        .replace_all(&s, "")
        .into_owned();
    s = regex(
        &URL,
        r#"(?:https?://|www\.)[^\s<>()\[\]]*[^\s<>()\[\].,;:!?'"]"#,
    )
    .replace_all(&s, "")
    .into_owned();
    s = regex(&HTML, r"</?[A-Za-z][^>]*>")
        .replace_all(&s, " ")
        .into_owned();
    s = regex(&CODE, r"`([^`]*)`")
        .replace_all(&s, "$1")
        .into_owned();
    s = regex(&BOLD_STAR, r"\*\*([^*]+)\*\*")
        .replace_all(&s, "$1")
        .into_owned();
    s = regex(&BOLD_UNDERSCORE, r"__([^_]+)__")
        .replace_all(&s, "$1")
        .into_owned();
    s = regex(&STRIKE, r"~~([^~]+)~~")
        .replace_all(&s, "$1")
        .into_owned();
    s = regex(&ITALIC_STAR, r"(^|[^\w*])\*([^\s*][^*]*?)\*([^\w*]|$)")
        .replace_all(&s, "$1$2$3")
        .into_owned();
    s = regex(&ITALIC_UNDERSCORE, r"(^|[^\w])_([^\s_][^_]*?)_([^\w]|$)")
        .replace_all(&s, "$1$2$3")
        .into_owned();
    s = regex(
        &NUMBERED_REFERENCE,
        r"\s*\[\d+(?:\s*[,\-\u{2013}]\s*\d+)*\]",
    )
    .replace_all(&s, "")
    .into_owned();
    s = regex(
        &DRIVE_PATH,
        r#"[A-Za-z]:[\\/](?:[^\\/:*?"<>|\s]+[\\/])*([^\\/:*?"<>|\s]+)"#,
    )
    .replace_all(&s, "$1")
    .into_owned();
    s = regex(
        &RELATIVE_PATH,
        r"[\\/]?(?:[\w.~\-]+[\\/])+([\w.\-]+\.[A-Za-z][A-Za-z0-9]{1,4})\b",
    )
    .replace_all(&s, "$1")
    .into_owned();
    // A file name is read as words: separators become spaces and the extension is dropped.
    let file_name_pattern = format!(r"(?i)\b([\w\-]+(?:\.[\w\-]+)*)\.(?:{FILE_EXTENSIONS})\b");
    let file_name =
        FILE_NAME.get_or_init(|| Regex::new(&file_name_pattern).expect("pattern compiles"));
    s = file_name
        .replace_all(&s, |found: &regex::Captures| {
            found[1].replace(['-', '_', '.'], " ")
        })
        .into_owned();

    let mut cleaned = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{2014}' => cleaned.push_str(", "),
            '|' => cleaned.push_str(", "),
            '`' | '#' => {}
            '\u{a0}' | '\u{202f}' | '\t' => cleaned.push(' '),
            c if is_pictograph(c) => cleaned.push(' '),
            c => cleaned.push(c),
        }
    }
    // A star left over is a mark, except a multiplication sign between spaces.
    let chars: Vec<char> = cleaned.chars().collect();
    let mut out = String::with_capacity(chars.len());
    for (index, c) in chars.iter().enumerate() {
        if *c == '*' {
            let before = index.checked_sub(1).and_then(|p| chars.get(p));
            let after = chars.get(index + 1);
            let spaced = before.is_some_and(|b| b.is_whitespace())
                && after.is_some_and(|a| a.is_whitespace());
            if !spaced {
                continue;
            }
        }
        out.push(*c);
    }
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" ,", ",")
}
