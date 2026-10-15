//! Readers for the formats that carry text and are not a PDF, a DOCX, a plain text or an image: OpenDocument
//! text and presentations, PowerPoint presentations, RTF, e-mail messages and WebVTT transcripts.
//!
//! These are **readers, never converters**: each takes a file and returns its text, a page at a time when the
//! format has pages (a slide is a page), and leaves the file as it is. They use the zip and XML crates the DOCX
//! reader already uses; RTF, e-mail and WebVTT are read by small parsers of their own, so no new dependency
//! ships. Nothing here depends on the platform. OCR is not involved: a picture inside a presentation is not
//! read.
//!
//! A reader returns the text of each page; `extraction` turns it into pages of the document, and a file that
//! yields no text is reported empty there, never guessed at.

pub mod eml;
pub mod odf;
pub mod pptx;
pub mod rtf;
pub mod vtt;

use std::path::Path;

use crate::extraction::ExtractionError;

/// The extensions this module reads (lowercase, without the dot).
pub const EXTENSIONS: &[&str] = &["odt", "odp", "pptx", "rtf", "eml", "vtt"];

/// The text of the file at `path`, one entry per page. `None` when `extension` is not one of
/// [`EXTENSIONS`].
pub fn read(extension: &str, path: &Path) -> Option<Result<Vec<String>, ExtractionError>> {
    let bytes = |path: &Path| std::fs::read(path).map_err(|_| ExtractionError::ReadFailed);
    Some(match extension {
        "odt" => odf::text_pages(path),
        "odp" => odf::slide_pages(path),
        "pptx" => pptx::slide_pages(path),
        "rtf" => bytes(path).map(|raw| vec![rtf::text(&raw)]),
        "eml" => bytes(path).map(|raw| vec![eml::text(&raw)]),
        "vtt" => bytes(path).map(|raw| vec![vtt::text(&decode_text(&raw))]),
        _ => return None,
    })
}

/// Text from bytes that may or may not be UTF-8: UTF-8 when it is valid, else Windows-1252, which is what a
/// French Windows export that was not saved as UTF-8 holds. A byte order mark is dropped.
pub fn decode_text(raw: &[u8]) -> String {
    let raw = raw.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(raw);
    match std::str::from_utf8(raw) {
        Ok(text) => text.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252.decode(raw).0.into_owned(),
    }
}

/// The text of one entry of a zip archive, or `ParseFailed` when it is missing or unreadable.
pub(crate) fn zip_entry_text(
    archive: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
) -> Result<String, ExtractionError> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| ExtractionError::ParseFailed)?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut entry, &mut text)
        .map_err(|_| ExtractionError::ParseFailed)?;
    Ok(text)
}

pub(crate) fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>, ExtractionError> {
    let file = std::fs::File::open(path).map_err(|_| ExtractionError::ReadFailed)?;
    zip::ZipArchive::new(file).map_err(|_| ExtractionError::ParseFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_in_windows_1252_is_read_and_a_byte_order_mark_is_dropped() {
        assert_eq!(decode_text(b"caf\xe9"), "caf\u{e9}");
        assert_eq!(decode_text("caf\u{e9}".as_bytes()), "caf\u{e9}");
        assert_eq!(decode_text(b"\xef\xbb\xbfWEBVTT"), "WEBVTT");
    }

    #[test]
    fn an_extension_this_module_does_not_read_is_not_claimed() {
        assert!(read("docx", Path::new("a.docx")).is_none());
        assert!(read("txt", Path::new("a.txt")).is_none());
    }
}
