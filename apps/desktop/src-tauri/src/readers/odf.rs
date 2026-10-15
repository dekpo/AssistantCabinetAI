//! OpenDocument text (`.odt`) and presentations (`.odp`): a zip whose `content.xml` holds the text.
//!
//! A text document is one page, as a DOCX is (the file has no fixed pagination). A presentation has one page
//! per slide, in order; the speaker notes of a slide are not part of it. Headings and paragraphs are lines,
//! the paragraphs of a table cell included.

use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use super::{open_zip, zip_entry_text};
use crate::extraction::ExtractionError;

/// The text of a `.odt`, as one page.
pub fn text_pages(path: &Path) -> Result<Vec<String>, ExtractionError> {
    let mut archive = open_zip(path)?;
    let xml = zip_entry_text(&mut archive, "content.xml")?;
    Ok(vec![pages_of(&xml, false)?.concat()])
}

/// The text of a `.odp`, one entry per slide.
pub fn slide_pages(path: &Path) -> Result<Vec<String>, ExtractionError> {
    let mut archive = open_zip(path)?;
    let xml = zip_entry_text(&mut archive, "content.xml")?;
    pages_of(&xml, true)
}

fn attribute(tag: &BytesStart<'_>, local: &[u8]) -> Option<String> {
    tag.attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().as_ref() == local)
        .and_then(|attribute| {
            attribute
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|value| value.into_owned())
        })
}

/// `content.xml` as text. With `by_slide` every `draw:page` is a page; without it the whole body is one.
fn pages_of(xml: &str, by_slide: bool) -> Result<Vec<String>, ExtractionError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut pages: Vec<String> = Vec::new();
    let mut current = String::new();
    // Text counts only inside a paragraph or a heading; notes of a slide are left out.
    let mut in_paragraph = 0usize;
    let mut in_notes = 0usize;
    let mut in_slide = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => match tag.local_name().as_ref() {
                b"p" | b"h" => in_paragraph += 1,
                b"notes" => in_notes += 1,
                b"page" if by_slide => {
                    in_slide = true;
                    current.clear();
                }
                _ => {}
            },
            Ok(Event::Empty(tag)) => match tag.local_name().as_ref() {
                b"s" if in_paragraph > 0 && in_notes == 0 => {
                    let count = attribute(&tag, b"c")
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(1)
                        .min(64);
                    current.push_str(&" ".repeat(count));
                }
                b"tab" if in_paragraph > 0 && in_notes == 0 => current.push('\t'),
                b"line-break" if in_paragraph > 0 && in_notes == 0 => current.push('\n'),
                _ => {}
            },
            Ok(Event::Text(text)) if in_paragraph > 0 && in_notes == 0 => {
                let decoded = text.decode().map_err(|_| ExtractionError::ParseFailed)?;
                current.push_str(&decoded);
            }
            Ok(Event::GeneralRef(reference)) if in_paragraph > 0 && in_notes == 0 => {
                // &amp; &lt; and the like inside the text.
                let name = reference
                    .decode()
                    .map_err(|_| ExtractionError::ParseFailed)?;
                match name.as_ref() {
                    "amp" => current.push('&'),
                    "lt" => current.push('<'),
                    "gt" => current.push('>'),
                    "quot" => current.push('"'),
                    "apos" => current.push('\''),
                    other => {
                        if let Some(code) = other
                            .strip_prefix("#x")
                            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                            .or_else(|| other.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                        {
                            if let Some(character) = char::from_u32(code) {
                                current.push(character);
                            }
                        }
                    }
                }
            }
            Ok(Event::End(tag)) => match tag.local_name().as_ref() {
                b"p" | b"h" => {
                    in_paragraph = in_paragraph.saturating_sub(1);
                    if in_paragraph == 0 && in_notes == 0 {
                        current.push('\n');
                    }
                }
                b"notes" => in_notes = in_notes.saturating_sub(1),
                b"page" if by_slide && in_slide => {
                    in_slide = false;
                    pages.push(std::mem::take(&mut current));
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err(ExtractionError::ParseFailed),
        }
    }
    if !by_slide {
        pages.push(current);
    }
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn archive(dir: &Path, name: &str, content_xml: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("mimetype", options).unwrap();
        writer
            .write_all(b"application/vnd.oasis.opendocument.text")
            .unwrap();
        writer.start_file("content.xml", options).unwrap();
        writer.write_all(content_xml.as_bytes()).unwrap();
        writer.finish().unwrap();
        path
    }

    const TEXT_HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content xmlns:office="o" xmlns:text="t" xmlns:table="b" xmlns:draw="d" xmlns:presentation="p">"#;

    #[test]
    fn a_text_document_is_one_page_with_a_line_per_paragraph() {
        let xml = format!(
            "{TEXT_HEAD}<office:body><office:text><text:h>Minutes</text:h><text:p>Budget: <text:span>CHF 9<text:s text:c=\"1\"/>800</text:span> &amp; more.</text:p><text:p>Next<text:tab/>item<text:line-break/>here</text:p></office:text></office:body></office:document-content>"
        );
        let dir = tempfile::tempdir().unwrap();
        let path = archive(dir.path(), "a.odt", &xml);
        let pages = text_pages(&path).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(
            pages[0],
            "Minutes\nBudget: CHF 9 800 & more.\nNext\titem\nhere\n"
        );
    }

    #[test]
    fn a_presentation_has_one_page_per_slide_and_leaves_the_notes_out() {
        let xml = format!(
            "{TEXT_HEAD}<office:body><office:presentation><draw:page draw:name=\"one\"><draw:frame><draw:text-box><text:p>Opening</text:p></draw:text-box></draw:frame><presentation:notes><draw:frame><draw:text-box><text:p>Say hello first</text:p></draw:text-box></draw:frame></presentation:notes></draw:page><draw:page draw:name=\"two\"><draw:frame><draw:text-box><text:p>Budget CHF 9 800</text:p></draw:text-box></draw:frame></draw:page></office:presentation></office:body></office:document-content>"
        );
        let dir = tempfile::tempdir().unwrap();
        let path = archive(dir.path(), "a.odp", &xml);
        let pages = slide_pages(&path).unwrap();
        assert_eq!(
            pages,
            vec!["Opening\n".to_string(), "Budget CHF 9 800\n".to_string()]
        );
    }

    #[test]
    fn a_file_that_is_not_a_zip_or_has_no_content_is_a_parse_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.odt");
        std::fs::write(&path, b"not a zip").unwrap();
        assert!(matches!(
            text_pages(&path),
            Err(ExtractionError::ParseFailed)
        ));
    }
}
