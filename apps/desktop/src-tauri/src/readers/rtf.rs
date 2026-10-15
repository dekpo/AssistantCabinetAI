//! Rich Text Format (`.rtf`): the text of the document, without its formatting.
//!
//! RTF is groups (`{ ... }`) of control words (`\par`, `\u233?`) and text. The reader keeps the text, turns
//! paragraphs, lines, tabs and cells into the characters they stand for, decodes the escaped characters
//! (`\'e9` in the document's code page, `\u233` as a Unicode character) and leaves out the groups that hold
//! no text of the document: fonts, colours, styles, the document information, pictures, headers and footers.

use std::borrow::Cow;

/// Destinations whose group is not the text of the document.
const SKIPPED: &[&str] = &[
    "fonttbl",
    "colortbl",
    "stylesheet",
    "info",
    "pict",
    "header",
    "headerl",
    "headerr",
    "headerf",
    "footer",
    "footerl",
    "footerr",
    "footerf",
    "footnote",
    "object",
    "datastore",
    "themedata",
    "colorschememapping",
    "latentstyles",
    "listtable",
    "listoverridetable",
    "rsidtbl",
    "generator",
    "xmlnstbl",
    "fldinst",
    "revtbl",
    "private",
    "bkmkstart",
    "bkmkend",
    "template",
    "mmathPr",
];

#[derive(Clone, Copy)]
struct Group {
    skip: bool,
    /// How many characters follow a `\u` escape as its plain-text fallback.
    unicode_skip: usize,
}

/// The text of an RTF document. Anything that is not RTF gives an empty text.
pub fn text(raw: &[u8]) -> String {
    let trimmed = raw
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(raw.len());
    if !raw[trimmed..].starts_with(b"{\\rtf") {
        return String::new();
    }
    let codepage = codepage_of(raw);
    let mut out = String::new();
    let mut stack: Vec<Group> = Vec::new();
    let mut state = Group {
        skip: false,
        unicode_skip: 1,
    };
    // The first thing in a group decides whether it is a skipped destination.
    let mut group_start = false;
    // Characters still to drop after a `\u` escape.
    let mut drop_next = 0usize;
    let mut index = 0usize;
    while index < raw.len() {
        let byte = raw[index];
        match byte {
            b'{' => {
                stack.push(state);
                group_start = true;
                index += 1;
            }
            b'}' => {
                state = stack.pop().unwrap_or(state);
                group_start = false;
                index += 1;
            }
            b'\\' => {
                index += 1;
                let Some(&next) = raw.get(index) else { break };
                if !next.is_ascii_alphabetic() {
                    index += 1;
                    match next {
                        b'\\' | b'{' | b'}' => push(
                            &mut out,
                            &state,
                            &mut drop_next,
                            &(next as char).to_string(),
                        ),
                        b'~' => push(&mut out, &state, &mut drop_next, " "),
                        b'-' | b'_' => push(
                            &mut out,
                            &state,
                            &mut drop_next,
                            if next == b'_' { "-" } else { "" },
                        ),
                        b'*' => {
                            if group_start {
                                state.skip = true;
                            }
                        }
                        b'\'' => {
                            let hex = raw
                                .get(index..index + 2)
                                .and_then(|pair| std::str::from_utf8(pair).ok());
                            if let Some(value) =
                                hex.and_then(|pair| u8::from_str_radix(pair, 16).ok())
                            {
                                index += 2;
                                let decoded = decode_byte(value, codepage);
                                push(&mut out, &state, &mut drop_next, &decoded);
                            }
                        }
                        b'\r' | b'\n' => push(&mut out, &state, &mut drop_next, "\n"),
                        _ => {}
                    }
                    group_start = false;
                    continue;
                }
                let start = index;
                while index < raw.len() && raw[index].is_ascii_alphabetic() {
                    index += 1;
                }
                let word = std::str::from_utf8(&raw[start..index]).unwrap_or("");
                let negative = raw.get(index) == Some(&b'-');
                let digits_from = if negative { index + 1 } else { index };
                let mut digits_to = digits_from;
                while digits_to < raw.len() && raw[digits_to].is_ascii_digit() {
                    digits_to += 1;
                }
                let parameter: Option<i64> = std::str::from_utf8(&raw[digits_from..digits_to])
                    .ok()
                    .and_then(|digits| digits.parse().ok())
                    .map(|value: i64| if negative { -value } else { value });
                if digits_to > digits_from {
                    index = digits_to;
                }
                // One space after a control word belongs to it.
                if raw.get(index) == Some(&b' ') {
                    index += 1;
                }
                if group_start && SKIPPED.contains(&word) {
                    state.skip = true;
                }
                group_start = false;
                match word {
                    "par" | "line" | "sect" | "row" | "page" => {
                        push(&mut out, &state, &mut drop_next, "\n")
                    }
                    "tab" | "cell" => push(&mut out, &state, &mut drop_next, "\t"),
                    "emdash" => push(&mut out, &state, &mut drop_next, "\u{2014}"),
                    "endash" => push(&mut out, &state, &mut drop_next, "\u{2013}"),
                    "bullet" => push(&mut out, &state, &mut drop_next, "\u{2022}"),
                    "lquote" => push(&mut out, &state, &mut drop_next, "\u{2018}"),
                    "rquote" => push(&mut out, &state, &mut drop_next, "\u{2019}"),
                    "ldblquote" => push(&mut out, &state, &mut drop_next, "\u{201c}"),
                    "rdblquote" => push(&mut out, &state, &mut drop_next, "\u{201d}"),
                    "uc" => {
                        if let Some(count) = parameter {
                            state.unicode_skip = count.clamp(0, 8) as usize;
                        }
                    }
                    "u" => {
                        if let Some(code) = parameter {
                            let code = if code < 0 { code + 65_536 } else { code } as u32;
                            if let Some(character) = char::from_u32(code) {
                                push(&mut out, &state, &mut drop_next, &character.to_string());
                            }
                            drop_next = state.unicode_skip;
                        }
                    }
                    _ => {}
                }
            }
            b'\r' | b'\n' => {
                index += 1;
            }
            _ => {
                group_start = false;
                // Plain text: a run of ordinary bytes, in the document's code page when it is not ASCII.
                let start = index;
                while index < raw.len()
                    && !matches!(raw[index], b'{' | b'}' | b'\\' | b'\r' | b'\n')
                {
                    index += 1;
                }
                let run = &raw[start..index];
                let decoded: Cow<'_, str> = if run.is_ascii() {
                    Cow::Borrowed(std::str::from_utf8(run).unwrap_or(""))
                } else {
                    Cow::Owned(decode_run(run, codepage))
                };
                push(&mut out, &state, &mut drop_next, &decoded);
            }
        }
    }
    tidy(&out)
}

/// Write `piece` unless the group is skipped; the characters that stand in for a `\u` escape are dropped first.
fn push(out: &mut String, state: &Group, drop_next: &mut usize, piece: &str) {
    if state.skip {
        return;
    }
    if *drop_next > 0 {
        let mut characters = piece.chars();
        while *drop_next > 0 && characters.next().is_some() {
            *drop_next -= 1;
        }
        out.push_str(characters.as_str());
        return;
    }
    out.push_str(piece);
}

/// The code page the document declares (`\ansicpg1252`); Windows-1252 when it says nothing.
fn codepage_of(raw: &[u8]) -> u16 {
    let head = &raw[..raw.len().min(4096)];
    let text = String::from_utf8_lossy(head);
    text.find("\\ansicpg")
        .and_then(|at| {
            let digits: String = text[at + 8..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            digits.parse().ok()
        })
        .unwrap_or(1252)
}

fn encoding_for(codepage: u16) -> &'static encoding_rs::Encoding {
    match codepage {
        65001 => encoding_rs::UTF_8,
        932 => encoding_rs::SHIFT_JIS,
        936 => encoding_rs::GBK,
        949 => encoding_rs::EUC_KR,
        950 => encoding_rs::BIG5,
        874 => encoding_rs::WINDOWS_874,
        1250 => encoding_rs::WINDOWS_1250,
        1251 => encoding_rs::WINDOWS_1251,
        1253 => encoding_rs::WINDOWS_1253,
        1254 => encoding_rs::WINDOWS_1254,
        1255 => encoding_rs::WINDOWS_1255,
        1256 => encoding_rs::WINDOWS_1256,
        1257 => encoding_rs::WINDOWS_1257,
        1258 => encoding_rs::WINDOWS_1258,
        _ => encoding_rs::WINDOWS_1252,
    }
}

fn decode_byte(value: u8, codepage: u16) -> String {
    decode_run(&[value], codepage)
}

fn decode_run(bytes: &[u8], codepage: u16) -> String {
    encoding_for(codepage).decode(bytes).0.into_owned()
}

/// Trailing blanks off each line, and no more than one blank line in a row.
fn tidy(text: &str) -> String {
    let mut lines: Vec<&str> = Vec::new();
    let mut blank = 0usize;
    for line in text.split('\n') {
        let line = line.trim_end();
        if line.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        lines.push(line);
    }
    lines.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_tabs_and_escaped_characters_are_read() {
        let raw = br"{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0 Arial;}}{\colortbl;\red0\green0\blue0;}\f0\fs24 Minutes of the meeting\par Cost:\tab CHF 9\~800 caf\'e9\par \u8364? 120 \u233?t\u233?\par}";
        assert_eq!(
            text(raw),
            "Minutes of the meeting\nCost:\tCHF 9 800 caf\u{e9}\n\u{20ac} 120 \u{e9}t\u{e9}"
        );
    }

    #[test]
    fn fonts_styles_info_pictures_and_ignorable_destinations_are_left_out() {
        let raw = br"{\rtf1{\stylesheet{\s1 heading;}}{\info{\title Secret title}{\author Nobody}}{\*\generator Some Editor;}{\pict\wmetafile8 0102030405}Visible text.\par{\footer Page footer}End.}";
        assert_eq!(text(raw), "Visible text.\nEnd.");
    }

    #[test]
    fn a_table_row_gives_tab_separated_cells_and_a_field_keeps_its_result() {
        let raw = br"{\rtf1 Name\cell Amount\cell\row Smith\cell 10\cell\row {\field{\*\fldinst HYPERLINK x}{\fldrslt linked text}}\par}";
        assert_eq!(text(raw), "Name\tAmount\nSmith\t10\nlinked text");
    }

    #[test]
    fn a_file_that_is_not_rtf_is_empty_and_an_unclosed_group_does_not_panic() {
        assert_eq!(text(b"plain words"), "");
        assert_eq!(text(br"{\rtf1 {{{ still reading"), "still reading");
    }

    #[test]
    fn a_document_in_another_code_page_is_decoded_with_it() {
        let raw = b"{\\rtf1\\ansi\\ansicpg1251 \\'cf\\'f0\\'e8\\'e2\\'e5\\'f2}";
        assert_eq!(text(raw), "\u{41f}\u{440}\u{438}\u{432}\u{435}\u{442}");
    }
}
