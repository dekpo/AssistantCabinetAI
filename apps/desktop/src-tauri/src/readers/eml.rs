//! E-mail messages (`.eml`, RFC 5322 with MIME): who wrote to whom, when, about what, and the text of the
//! message.
//!
//! The header fields a reader looks for (From, To, Cc, Date, Subject) are written first, each on a line of
//! its own, then a blank line and the text of the message. A multipart message gives its text parts in order
//! (the plain text; the HTML only when there is no plain text, with its tags removed); attachments are not
//! read. Transfer encodings (base64, quoted-printable) and character sets are decoded.

use std::collections::BTreeMap;

/// The header fields written before the text, in this order. They are protocol names, not user language.
const SHOWN: &[&str] = &["From", "To", "Cc", "Date", "Subject"];
/// How deep a message inside a message is followed.
const MAX_DEPTH: usize = 6;

/// The text of an e-mail message. A file with no header field gives an empty text.
pub fn text(raw: &[u8]) -> String {
    let message = parse(raw);
    if message.headers.is_empty() {
        return String::new();
    }
    let mut lines: Vec<String> = Vec::new();
    for name in SHOWN {
        if let Some(value) = message.headers.get(&name.to_ascii_lowercase()) {
            let value = decode_words(value);
            if !value.trim().is_empty() {
                lines.push(format!("{name}: {}", value.trim()));
            }
        }
    }
    let body = body_text(&message, 0);
    let body = body.trim().replace("\r\n", "\n");
    if !lines.is_empty() && !body.is_empty() {
        lines.push(String::new());
    }
    if !body.is_empty() {
        lines.push(body);
    }
    lines.join("\n")
}

struct Part<'a> {
    /// Lowercase field name to its unfolded value (the first of a repeated field).
    headers: BTreeMap<String, String>,
    body: &'a [u8],
}

fn parse(raw: &[u8]) -> Part<'_> {
    // The header ends at the first empty line, whatever the line ends are.
    let (header_end, body_start) = find_blank_line(raw);
    let head = super::decode_text(&raw[..header_end]);
    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    let mut current: Option<(String, String)> = None;
    for line in head.lines() {
        if line.starts_with([' ', '\t']) {
            if let Some((_, value)) = current.as_mut() {
                value.push(' ');
                value.push_str(line.trim());
            }
            continue;
        }
        if let Some((name, value)) = current.take() {
            headers.entry(name).or_insert(value);
        }
        if let Some((name, value)) = line.split_once(':') {
            // A field name has no blank in it: a sentence with a colon is not a header.
            if !name.is_empty() && !name.contains(char::is_whitespace) {
                current = Some((name.to_ascii_lowercase(), value.trim().to_string()));
            }
        }
    }
    if let Some((name, value)) = current.take() {
        headers.entry(name).or_insert(value);
    }
    Part {
        headers,
        body: &raw[body_start.min(raw.len())..],
    }
}

/// Where the header ends and the body starts.
fn find_blank_line(raw: &[u8]) -> (usize, usize) {
    let mut index = 0usize;
    while index < raw.len() {
        let line_end = raw[index..]
            .iter()
            .position(|b| *b == b'\n')
            .map_or(raw.len(), |at| index + at);
        let line = &raw[index..line_end];
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            return (index, (line_end + 1).min(raw.len()));
        }
        index = line_end + 1;
    }
    (raw.len(), raw.len())
}

/// A header value's parameter, unquoted: `boundary` of `multipart/mixed; boundary="x"`.
fn parameter(value: &str, name: &str) -> Option<String> {
    for piece in value.split(';').skip(1) {
        if let Some((key, found)) = piece.split_once('=') {
            if key.trim().eq_ignore_ascii_case(name) {
                return Some(found.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

fn body_text(part: &Part<'_>, depth: usize) -> String {
    let content_type = part.headers.get("content-type").map_or("", String::as_str);
    let media = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let media = if media.is_empty() {
        "text/plain".to_string()
    } else {
        media
    };
    let disposition = part
        .headers
        .get("content-disposition")
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    if disposition.trim_start().starts_with("attachment") || depth >= MAX_DEPTH {
        return String::new();
    }
    if media.starts_with("multipart/") {
        let Some(boundary) = parameter(content_type, "boundary") else {
            return String::new();
        };
        let texts: Vec<(bool, String)> = split_multipart(part.body, &boundary)
            .iter()
            .map(|child| {
                let inner = parse(child);
                let is_html = inner
                    .headers
                    .get("content-type")
                    .is_some_and(|value| value.to_ascii_lowercase().starts_with("text/html"));
                (is_html, body_text(&inner, depth + 1))
            })
            .filter(|(_, text)| !text.trim().is_empty())
            .collect();
        // `multipart/alternative` offers the same text twice: the plain one is read.
        let any_plain = texts.iter().any(|(is_html, _)| !is_html);
        return texts
            .into_iter()
            .filter(|(is_html, _)| !(any_plain && *is_html))
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join("\n\n");
    }
    if media == "message/rfc822" {
        return body_text(&parse(part.body), depth + 1);
    }
    if !media.starts_with("text/") {
        return String::new();
    }
    let decoded = decode_transfer(
        part.body,
        part.headers
            .get("content-transfer-encoding")
            .map_or("7bit", String::as_str),
    );
    let text = decode_charset(&decoded, parameter(content_type, "charset").as_deref());
    if media == "text/html" {
        strip_html(&text)
    } else {
        text
    }
}

/// The parts of a multipart body, between its boundary lines.
fn split_multipart<'a>(body: &'a [u8], boundary: &str) -> Vec<&'a [u8]> {
    let delimiter = format!("--{boundary}");
    let closing = format!("{delimiter}--");
    let mut parts = Vec::new();
    let mut start: Option<usize> = None;
    let mut index = 0usize;
    while index <= body.len() {
        let line_end = body[index..]
            .iter()
            .position(|b| *b == b'\n')
            .map_or(body.len(), |at| index + at);
        let line = &body[index..line_end];
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let trimmed = trim_end_blanks(line);
        let is_closing = trimmed == closing.as_bytes();
        if is_closing || trimmed == delimiter.as_bytes() {
            if let Some(from) = start {
                // The line end before a boundary belongs to the boundary.
                let mut to = index.saturating_sub(1);
                if to > from && body[to - 1] == b'\r' {
                    to -= 1;
                }
                parts.push(&body[from..to.max(from)]);
            }
            if is_closing {
                break;
            }
            start = Some((line_end + 1).min(body.len()));
        }
        if line_end >= body.len() {
            break;
        }
        index = line_end + 1;
    }
    parts
}

fn trim_end_blanks(line: &[u8]) -> &[u8] {
    let mut end = line.len();
    while end > 0 && matches!(line[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    &line[..end]
}

fn decode_transfer(body: &[u8], encoding: &str) -> Vec<u8> {
    match encoding.trim().to_ascii_lowercase().as_str() {
        "base64" => decode_base64(body),
        "quoted-printable" => decode_quoted_printable(body),
        _ => body.to_vec(),
    }
}

fn decode_base64(input: &[u8]) -> Vec<u8> {
    let value = |byte: u8| -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some(u32::from(byte - b'A')),
            b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for &byte in input {
        let Some(sextet) = value(byte) else { continue };
        buffer = (buffer << 6) | sextet;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xFF) as u8);
        }
    }
    out
}

fn decode_quoted_printable(input: &[u8]) -> Vec<u8> {
    let hex = |byte: u8| -> Option<u8> { (byte as char).to_digit(16).map(|d| d as u8) };
    let mut out = Vec::with_capacity(input.len());
    let mut index = 0usize;
    while index < input.len() {
        let byte = input[index];
        if byte == b'=' {
            // A soft line break: `=` at the end of a line.
            if input.get(index + 1) == Some(&b'\n') {
                index += 2;
                continue;
            }
            if input.get(index + 1) == Some(&b'\r') && input.get(index + 2) == Some(&b'\n') {
                index += 3;
                continue;
            }
            if let (Some(high), Some(low)) = (
                input.get(index + 1).and_then(|b| hex(*b)),
                input.get(index + 2).and_then(|b| hex(*b)),
            ) {
                out.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        out.push(byte);
        index += 1;
    }
    out
}

fn decode_charset(bytes: &[u8], charset: Option<&str>) -> String {
    let encoding = charset
        .and_then(|label| encoding_rs::Encoding::for_label(label.trim().as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    let (text, _, had_errors) = encoding.decode(bytes);
    if had_errors && encoding == encoding_rs::UTF_8 {
        // Declared nothing and not UTF-8: what a French Windows client writes.
        return encoding_rs::WINDOWS_1252.decode(bytes).0.into_owned();
    }
    text.into_owned()
}

/// Header words encoded as `=?charset?B?...?=` or `=?charset?Q?...?=`.
fn decode_words(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    let mut previous_was_word = false;
    while let Some(open) = rest.find("=?") {
        let (before, tail) = rest.split_at(open);
        // Blanks between two encoded words are not part of the text.
        if !(previous_was_word && before.trim().is_empty()) {
            out.push_str(before);
        }
        let inner = &tail[2..];
        let parts: Vec<&str> = inner.splitn(4, '?').collect();
        if let [charset, encoding, text, remainder] = parts.as_slice() {
            if let Some(after) = remainder.strip_prefix('=') {
                let bytes = match encoding.to_ascii_lowercase().as_str() {
                    "b" => Some(decode_base64(text.as_bytes())),
                    "q" => Some(decode_quoted_printable(text.replace('_', " ").as_bytes())),
                    _ => None,
                };
                if let Some(bytes) = bytes {
                    out.push_str(&decode_charset(&bytes, Some(charset)));
                    rest = after;
                    previous_was_word = true;
                    continue;
                }
            }
        }
        out.push_str("=?");
        rest = inner;
        previous_was_word = false;
    }
    out.push_str(rest);
    out
}

/// The text of an HTML part: tags removed, scripts and styles dropped, entities decoded, blocks kept as lines.
fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    let mut skip_until: Option<&str> = None;
    while let Some(open) = rest.find('<') {
        if skip_until.is_none() {
            out.push_str(&rest[..open]);
        }
        let Some(close) = rest[open..].find('>') else {
            rest = "";
            break;
        };
        let tag = rest[open + 1..open + close].trim().to_ascii_lowercase();
        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        match skip_until {
            Some(end) => {
                if closing && name == end {
                    skip_until = None;
                }
            }
            None => match name {
                "script" if !closing => skip_until = Some("script"),
                "style" if !closing => skip_until = Some("style"),
                "br" | "p" | "div" | "tr" | "li" | "h1" | "h2" | "h3" | "h4" | "table" => {
                    out.push('\n')
                }
                "td" | "th" if closing => out.push('\t'),
                _ => {}
            },
        }
        rest = &rest[open + close + 1..];
    }
    if skip_until.is_none() {
        out.push_str(rest);
    }
    decode_html_entities(&out)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The named entities a message written in a Western European language uses, as ASCII escapes.
const NAMED_ENTITIES: &[(&str, &str)] = &[
    ("amp", "&"),
    ("lt", "<"),
    ("gt", ">"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", " "),
    ("eacute", "\u{e9}"),
    ("egrave", "\u{e8}"),
    ("ecirc", "\u{ea}"),
    ("euml", "\u{eb}"),
    ("agrave", "\u{e0}"),
    ("acirc", "\u{e2}"),
    ("auml", "\u{e4}"),
    ("ccedil", "\u{e7}"),
    ("icirc", "\u{ee}"),
    ("iuml", "\u{ef}"),
    ("ocirc", "\u{f4}"),
    ("ouml", "\u{f6}"),
    ("ugrave", "\u{f9}"),
    ("ucirc", "\u{fb}"),
    ("uuml", "\u{fc}"),
    ("Eacute", "\u{c9}"),
    ("Egrave", "\u{c8}"),
    ("Agrave", "\u{c0}"),
    ("Ccedil", "\u{c7}"),
    ("szlig", "\u{df}"),
    ("oelig", "\u{153}"),
    ("euro", "\u{20ac}"),
    ("laquo", "\u{ab}"),
    ("raquo", "\u{bb}"),
    ("lsquo", "\u{2018}"),
    ("rsquo", "\u{2019}"),
    ("ldquo", "\u{201c}"),
    ("rdquo", "\u{201d}"),
    ("hellip", "\u{2026}"),
    ("ndash", "\u{2013}"),
    ("mdash", "\u{2014}"),
    ("copy", "\u{a9}"),
    ("deg", "\u{b0}"),
];

/// `&name;`, `&#233;` and `&#xE9;` read as the characters they stand for; anything else is left as written.
fn decode_html_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp + 1..];
        let decoded = tail
            .find(';')
            .filter(|end| *end <= 10)
            .and_then(|end| entity_char(&tail[..end]).map(|plain| (plain, end)));
        match decoded {
            Some((plain, end)) => {
                out.push_str(&plain);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity_char(name: &str) -> Option<String> {
    if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse().ok()?,
        };
        return char::from_u32(code).map(String::from);
    }
    NAMED_ENTITIES
        .iter()
        .find(|(entity, _)| *entity == name)
        .map(|(_, plain)| (*plain).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_message_gives_its_header_fields_then_its_text() {
        let raw = b"From: Anna Martin <anna@example.test>\r\nTo: office@example.test\r\nSubject: Invoice\r\n  FAC-2026-014\r\nDate: Thu, 12 Mar 2026 09:30:00 +0100\r\nMessage-ID: <x@y>\r\n\r\nPlease find the total: 1 200,00 EUR.\r\nRegards\r\n";
        assert_eq!(
            text(raw),
            "From: Anna Martin <anna@example.test>\nTo: office@example.test\nDate: Thu, 12 Mar 2026 09:30:00 +0100\nSubject: Invoice FAC-2026-014\n\nPlease find the total: 1 200,00 EUR.\nRegards"
        );
    }

    #[test]
    fn copies_are_shown_between_the_recipient_and_the_date() {
        let raw =
            b"Subject: s\nCc: bob@example.test\nTo: a@example.test\nFrom: c@example.test\n\nBody\n";
        assert_eq!(
            text(raw),
            "From: c@example.test\nTo: a@example.test\nCc: bob@example.test\nSubject: s\n\nBody"
        );
    }

    #[test]
    fn encoded_words_quoted_printable_and_charsets_are_decoded() {
        let raw = b"From: =?UTF-8?Q?Andr=C3=A9?= <a@example.test>\nSubject: =?ISO-8859-1?B?Q2Fm6Q==?=\nContent-Type: text/plain; charset=iso-8859-1\nContent-Transfer-Encoding: quoted-printable\n\nCaf=E9 =\nnoir =3D good\n";
        assert_eq!(
            text(raw),
            "From: Andr\u{e9} <a@example.test>\nSubject: Caf\u{e9}\n\nCaf\u{e9} noir = good"
        );
    }

    #[test]
    fn a_body_in_windows_1252_with_no_charset_is_read() {
        let raw = b"Subject: s\n\nCaf\xe9 noir\n";
        assert_eq!(text(raw), "Subject: s\n\nCaf\u{e9} noir");
    }

    #[test]
    fn a_multipart_message_reads_the_plain_text_and_skips_attachments_and_the_html_twin() {
        let raw = b"Subject: Report\nContent-Type: multipart/mixed; boundary=\"outer\"\n\n--outer\nContent-Type: multipart/alternative; boundary=\"inner\"\n\n--inner\nContent-Type: text/plain; charset=utf-8\n\nPlain version.\n--inner\nContent-Type: text/html; charset=utf-8\n\n<p>Html <b>version</b></p>\n--inner--\n--outer\nContent-Type: application/pdf\nContent-Disposition: attachment; filename=\"a.pdf\"\nContent-Transfer-Encoding: base64\n\nJVBERi0xLjQ=\n--outer--\n";
        assert_eq!(text(raw), "Subject: Report\n\nPlain version.");
    }

    #[test]
    fn a_text_attachment_is_not_read() {
        let raw = b"Subject: Notes\nContent-Type: multipart/mixed; boundary=b\n\n--b\nContent-Type: text/plain\n\nThe message.\n--b\nContent-Type: text/plain; name=secret.txt\nContent-Disposition: attachment; filename=secret.txt\n\nHidden attachment words.\n--b--\n";
        assert_eq!(text(raw), "Subject: Notes\n\nThe message.");
    }

    #[test]
    fn a_message_with_only_html_is_read_without_its_tags_scripts_or_styles() {
        let raw = b"Subject: Newsletter\nContent-Type: text/html\n\n<html><style>p {color:red}</style><body><p>Hello &amp; welcome</p><script>alert(1)</script><div>Total: CHF 9&nbsp;800 &#233;t&eacute; &#x20AC;</div></body></html>\n";
        assert_eq!(
            text(raw),
            "Subject: Newsletter\n\nHello & welcome\nTotal: CHF 9 800 \u{e9}t\u{e9} \u{20ac}"
        );
    }

    #[test]
    fn a_base64_body_is_read_and_a_file_with_no_header_is_empty() {
        let raw = b"Subject: B\nContent-Type: text/plain\nContent-Transfer-Encoding: base64\n\nSGVsbG8gd29ybGQ=\n";
        assert_eq!(text(raw), "Subject: B\n\nHello world");
        assert_eq!(text(b""), "");
        assert_eq!(text(b"just some words\nwith no header field\n"), "");
    }

    #[test]
    fn a_forwarded_message_is_followed_but_not_without_end() {
        let raw = b"Subject: Fwd\nContent-Type: message/rfc822\n\nSubject: Inner\n\nInner words.\n";
        assert_eq!(text(raw), "Subject: Fwd\n\nInner words.");
    }
}
