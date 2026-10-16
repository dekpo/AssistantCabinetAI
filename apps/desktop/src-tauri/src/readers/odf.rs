//! OpenDocument text (`.odt`) and presentations (`.odp`): a zip whose `content.xml` holds the text.
//!
//! A text document is one page, as a DOCX is (the file has no fixed pagination). A presentation has one page
//! per slide, in order; the speaker notes of a slide are not part of it. Headings and paragraphs are lines,
//! the paragraphs of a table cell included. Comments and tracked deletions are not part of the text.

use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

use super::{open_zip, xml_reference, zip_entry_text};
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

/// Elements whose paragraphs are not the text of the document: the speaker notes of a slide, the comments of a
/// reviewer, and the changes a reviewer tracked (what they deleted is still in the file).
fn is_left_out(local_name: &[u8]) -> bool {
    matches!(local_name, b"notes" | b"annotation" | b"tracked-changes")
}

/// `content.xml` as text. With `by_slide` every `draw:page` is a page; without it the whole body is one.
fn pages_of(xml: &str, by_slide: bool) -> Result<Vec<String>, ExtractionError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut pages: Vec<String> = Vec::new();
    let mut current = String::new();
    // Text counts only inside a paragraph or a heading, and not inside what is left out.
    let mut in_paragraph = 0usize;
    let mut left_out = 0usize;
    let mut in_slide = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => {
                let name = tag.local_name();
                match name.as_ref() {
                    b"p" | b"h" => in_paragraph += 1,
                    b"page" if by_slide => {
                        in_slide = true;
                        current.clear();
                    }
                    other if is_left_out(other) => left_out += 1,
                    _ => {}
                }
            }
            Ok(Event::Empty(tag)) if in_paragraph > 0 && left_out == 0 => {
                match tag.local_name().as_ref() {
                    b"s" => {
                        let count = attribute(&tag, b"c")
                            .and_then(|value| value.parse::<usize>().ok())
                            .unwrap_or(1)
                            .min(64);
                        current.push_str(&" ".repeat(count));
                    }
                    b"tab" => current.push('\t'),
                    b"line-break" => current.push('\n'),
                    _ => {}
                }
            }
            Ok(Event::Text(text)) if in_paragraph > 0 && left_out == 0 => {
                let decoded = text.decode().map_err(|_| ExtractionError::ParseFailed)?;
                current.push_str(&decoded);
            }
            Ok(Event::GeneralRef(reference)) if in_paragraph > 0 && left_out == 0 => {
                // &amp; &lt; &#233; and the like inside the text.
                let name = reference
                    .decode()
                    .map_err(|_| ExtractionError::ParseFailed)?;
                if let Some(character) = xml_reference(&name) {
                    current.push(character);
                }
            }
            Ok(Event::End(tag)) => {
                let name = tag.local_name();
                match name.as_ref() {
                    b"p" | b"h" => {
                        in_paragraph = in_paragraph.saturating_sub(1);
                        if in_paragraph == 0 && left_out == 0 {
                            current.push('\n');
                        }
                    }
                    b"page" if by_slide && in_slide => {
                        in_slide = false;
                        pages.push(std::mem::take(&mut current));
                    }
                    other if is_left_out(other) => left_out = left_out.saturating_sub(1),
                    _ => {}
                }
            }
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

    const HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content xmlns:office="o" xmlns:text="t" xmlns:table="b" xmlns:draw="d" xmlns:presentation="p">"#;

    fn odt_pages(body: &str) -> Vec<String> {
        let xml = format!("{HEAD}<office:body><office:text>{body}</office:text></office:body></office:document-content>");
        let dir = tempfile::tempdir().unwrap();
        text_pages(&archive(dir.path(), "a.odt", &xml)).unwrap()
    }

    fn odp_pages(slides: &str) -> Vec<String> {
        let xml = format!("{HEAD}<office:body><office:presentation>{slides}</office:presentation></office:body></office:document-content>");
        let dir = tempfile::tempdir().unwrap();
        slide_pages(&archive(dir.path(), "a.odp", &xml)).unwrap()
    }

    #[test]
    fn a_text_document_is_one_page_with_a_line_per_paragraph() {
        let pages = odt_pages("<text:h>Minutes</text:h><text:p>Budget: <text:span>CHF 9<text:s text:c=\"1\"/>800</text:span> &amp; more.</text:p><text:p>Next<text:tab/>item<text:line-break/>here</text:p>");
        assert_eq!(
            pages,
            vec!["Minutes\nBudget: CHF 9 800 & more.\nNext\titem\nhere\n".to_string()]
        );
    }

    #[test]
    fn list_items_and_table_cells_are_lines_and_character_references_are_read() {
        let pages = odt_pages("<text:list><text:list-item><text:p>First &#233;t&#xE9;</text:p></text:list-item></text:list><table:table><table:table-row><table:table-cell><text:p>Cell A</text:p></table:table-cell><table:table-cell><text:p>Cell B</text:p></table:table-cell></table:table-row></table:table>");
        assert_eq!(
            pages,
            vec!["First \u{e9}t\u{e9}\nCell A\nCell B\n".to_string()]
        );
    }

    #[test]
    fn comments_and_tracked_deletions_are_not_part_of_the_text() {
        let pages = odt_pages("<text:tracked-changes><text:changed-region><text:deletion><text:p>Deleted words</text:p></text:deletion></text:changed-region></text:tracked-changes><text:p>Kept words<office:annotation><text:p>Reviewer comment</text:p></office:annotation> stay.</text:p>");
        assert_eq!(pages, vec!["Kept words stay.\n".to_string()]);
    }

    #[test]
    fn a_presentation_has_one_page_per_slide_and_leaves_the_notes_out() {
        let pages = odp_pages("<draw:page draw:name=\"one\"><draw:frame><draw:text-box><text:p>Opening</text:p></draw:text-box></draw:frame><presentation:notes><draw:frame><draw:text-box><text:p>Say hello first</text:p></draw:text-box></draw:frame></presentation:notes></draw:page><draw:page draw:name=\"two\"><draw:frame><draw:text-box><text:p>Budget CHF 9 800</text:p></draw:text-box></draw:frame></draw:page>");
        assert_eq!(
            pages,
            vec!["Opening\n".to_string(), "Budget CHF 9 800\n".to_string()]
        );
    }

    #[test]
    fn a_slide_with_no_text_keeps_its_place() {
        let pages = odp_pages("<draw:page><text:p>One</text:p></draw:page><draw:page></draw:page><draw:page><text:p>Three</text:p></draw:page>");
        assert_eq!(
            pages,
            vec!["One\n".to_string(), String::new(), "Three\n".to_string()]
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

        let no_content = dir.path().join("hollow.odp");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&no_content).unwrap());
        writer
            .start_file("mimetype", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.finish().unwrap();
        assert!(matches!(
            slide_pages(&no_content),
            Err(ExtractionError::ParseFailed)
        ));
    }

    #[test]
    fn a_content_part_that_is_not_xml_is_a_parse_failure() {
        let dir = tempfile::tempdir().unwrap();
        let mismatched = archive(dir.path(), "worse.odt", "<a><b></a></b>");
        assert!(matches!(
            text_pages(&mismatched),
            Err(ExtractionError::ParseFailed)
        ));
    }
}
