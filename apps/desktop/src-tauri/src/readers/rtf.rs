//! Rich Text Format (`.rtf`): the text of the document, without its formatting.
//!
//! RTF is groups (`{ ... }`) of control words (`\par`, `\u233?`) and text. The reader keeps the text, turns
//! paragraphs, lines, tabs and cells into the characters they stand for, decodes the escaped characters
//! (`\'e9` in the document's code page, `\u233` as a Unicode character) and leaves out the groups that hold
//! no text of the document: fonts, colours, styles, the document information, pictures, headers and footers.

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

/// What a read is writing into: the text so far, the characters still to drop after a `\u` escape, and the
/// high half of a character outside the basic plane waiting for its low half.
struct Output {
    text: String,
    drop_next: usize,
    high_surrogate: Option<u32>,
}

impl Output {
    /// Write `piece` unless the group is skipped; the characters that stand in for a `\u` escape are dropped first.
    fn push(&mut self, state: &Group, piece: &str) {
        if state.skip {
            return;
        }
        if self.drop_next > 0 {
            let mut characters = piece.chars();
            while self.drop_next > 0 && characters.next().is_some() {
                self.drop_next -= 1;
            }
            self.text.push_str(characters.as_str());
            return;
        }
        self.text.push_str(piece);
    }

    /// A `\u` escape: a UTF-16 unit, signed. Two units make one character outside the basic plane.
    fn unicode(&mut self, state: &Group, value: i64) {
        let unit = value.rem_euclid(65_536) as u32;
        let character = match (self.high_surrogate.take(), unit) {
            (Some(high), 0xDC00..=0xDFFF) => {
                char::from_u32(0x10000 + ((high - 0xD800) << 10) + (unit - 0xDC00))
            }
            (_, 0xD800..=0xDBFF) => {
                self.high_surrogate = Some(unit);
                None
            }
            _ => char::from_u32(unit),
        };
        if let Some(character) = character {
            self.push(state, &character.to_string());
        }
        self.drop_next = state.unicode_skip;
    }
}

/// The text of an RTF document. Anything that is not RTF gives an empty text.
pub fn text(raw: &[u8]) -> String {
    let first = raw
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(raw.len());
    if !raw[first..].starts_with(b"{\\rtf") {
        return String::new();
    }
    let codepage = codepage_of(raw);
    let mut out = Output {
        text: String::new(),
        drop_next: 0,
        high_surrogate: None,
    };
    let mut stack: Vec<Group> = Vec::new();
    let mut state = Group {
        skip: false,
        unicode_skip: 1,
    };
    // The first thing in a group decides whether it is a skipped destination.
    let mut group_start = false;
    let mut index = 0usize;
    while index < raw.len() {
        match raw[index] {
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
                if next.is_ascii_alphabetic() {
                    let (word, parameter, after) = control_word(raw, index);
                    index = after;
                    if group_start && SKIPPED.contains(&word) {
                        state.skip = true;
                    }
                    group_start = false;
                    match word {
                        "par" | "line" | "sect" | "row" | "page" => out.push(&state, "\n"),
                        "tab" | "cell" => out.push(&state, "\t"),
                        "emdash" => out.push(&state, "\u{2014}"),
                        "endash" => out.push(&state, "\u{2013}"),
                        "bullet" => out.push(&state, "\u{2022}"),
                        "lquote" => out.push(&state, "\u{2018}"),
                        "rquote" => out.push(&state, "\u{2019}"),
                        "ldblquote" => out.push(&state, "\u{201c}"),
                        "rdblquote" => out.push(&state, "\u{201d}"),
                        "uc" => {
                            if let Some(count) = parameter {
                                state.unicode_skip = count.clamp(0, 8) as usize;
                            }
                        }
                        "u" => {
                            if let Some(code) = parameter {
                                out.unicode(&state, code);
                            }
                        }
                        // Binary data: as many bytes as the parameter says, whatever they look like.
                        "bin" => {
                            if let Some(length) = parameter {
                                index = (index + length.max(0) as usize).min(raw.len());
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                match next {
                    b'\\' | b'{' | b'}' => {
                        index += 1;
                        out.push(&state, &(next as char).to_string());
                    }
                    b'~' => {
                        index += 1;
                        out.push(&state, " ");
                    }
                    b'_' => {
                        index += 1;
                        out.push(&state, "-");
                    }
                    b'*' => {
                        index += 1;
                        if group_start {
                            state.skip = true;
                        }
                    }
                    b'\'' => {
                        // One or several escaped bytes in a row: a two-byte character is both of them.
                        // `index` is on the apostrophe of the first escape.
                        let mut bytes = Vec::new();
                        let mut at = index;
                        while let Some(value) = hex_pair(raw, at + 1) {
                            bytes.push(value);
                            at += 3;
                            if raw.get(at) == Some(&b'\\') && raw.get(at + 1) == Some(&b'\'') {
                                at += 1;
                            } else {
                                break;
                            }
                        }
                        if bytes.is_empty() {
                            index += 1;
                        } else {
                            index = at;
                            out.push(&state, &decode_run(&bytes, codepage));
                        }
                    }
                    b'\r' | b'\n' => {
                        index += 1;
                        out.push(&state, "\n");
                    }
                    _ => index += 1,
                }
                group_start = false;
            }
            b'\r' | b'\n' => index += 1,
            _ => {
                group_start = false;
                // Plain text: a run of ordinary bytes, in the document's code page when it is not ASCII.
                let start = index;
                while index < raw.len()
                    && !matches!(raw[index], b'{' | b'}' | b'\\' | b'\r' | b'\n')
                {
                    index += 1;
                }
                let run = decode_run(&raw[start..index], codepage);
                out.push(&state, &run);
            }
        }
    }
    tidy(&out.text)
}

/// The byte written as two hexadecimal digits at `at`.
fn hex_pair(raw: &[u8], at: usize) -> Option<u8> {
    let pair = std::str::from_utf8(raw.get(at..at + 2)?).ok()?;
    u8::from_str_radix(pair, 16).ok()
}

/// The control word that starts at `at` (just after the backslash), its numeric parameter if it has one, and
/// where the text goes on: the one space that ends a control word belongs to it.
fn control_word(raw: &[u8], at: usize) -> (&str, Option<i64>, usize) {
    let mut index = at;
    while index < raw.len() && raw[index].is_ascii_alphabetic() {
        index += 1;
    }
    let word = std::str::from_utf8(&raw[at..index]).unwrap_or("");
    let negative = raw.get(index) == Some(&b'-');
    let digits_from = if negative { index + 1 } else { index };
    let mut digits_to = digits_from;
    while digits_to < raw.len() && raw[digits_to].is_ascii_digit() {
        digits_to += 1;
    }
    let parameter = std::str::from_utf8(&raw[digits_from..digits_to])
        .ok()
        .and_then(|digits| digits.parse::<i64>().ok())
        .map(|value| if negative { -value } else { value });
    if digits_to > digits_from {
        index = digits_to;
    }
    if raw.get(index) == Some(&b' ') {
        index += 1;
    }
    (word, parameter, index)
}

/// The code page the document declares (`\ansicpg1252`); Windows-1252 when it says nothing.
fn codepage_of(raw: &[u8]) -> u16 {
    let head = String::from_utf8_lossy(&raw[..raw.len().min(4096)]);
    head.find("\\ansicpg")
        .and_then(|at| {
            let digits: String = head[at + 8..]
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

fn decode_run(bytes: &[u8], codepage: u16) -> String {
    if bytes.is_ascii() {
        return String::from_utf8_lossy(bytes).into_owned();
    }
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
        assert_eq!(text(br"{\rtf1\ansi\par}"), "");
    }

    #[test]
    fn a_document_in_another_code_page_is_decoded_with_it() {
        let raw = b"{\\rtf1\\ansi\\ansicpg1251 \\'cf\\'f0\\'e8\\'e2\\'e5\\'f2}";
        assert_eq!(text(raw), "\u{41f}\u{440}\u{438}\u{432}\u{435}\u{442}");
    }

    #[test]
    fn a_raw_eight_bit_byte_is_read_in_the_declared_code_page() {
        assert_eq!(text(b"{\\rtf1\\ansi\\ansicpg1252 caf\xe9}"), "caf\u{e9}");
    }

    #[test]
    fn binary_data_is_skipped_even_when_it_looks_like_braces() {
        let raw = b"{\\rtf1 Before.\\par{\\pict\\wmetafile8\\bin4 a}b{}After.}";
        assert_eq!(text(raw), "Before.\nAfter.");
    }

    #[test]
    fn a_character_outside_the_basic_plane_arrives_as_a_pair_of_escapes() {
        assert_eq!(
            text(br"{\rtf1 ok \u-10179?\u-8704? done}"),
            "ok \u{1f600} done"
        );
    }

    #[test]
    fn the_number_of_fallback_characters_follows_the_group_it_is_set_in() {
        // `\uc0`: no fallback after the escape, only inside its group; outside, one character follows.
        let raw = br"{\rtf1 {\uc0 \u233 e}\u233?e}";
        assert_eq!(text(raw), "\u{e9}e\u{e9}e");
        assert_eq!(text(br"{\rtf1 \uc2 \u233ab-c}"), "\u{e9}-c");
    }
}
