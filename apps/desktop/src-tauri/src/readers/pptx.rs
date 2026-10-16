//! PowerPoint presentations (`.pptx`): a zip with one XML part per slide. A slide is a page, in the order the
//! presentation lists them (`ppt/presentation.xml` with its relationships), not in the order the parts are
//! numbered: a slide moved in the deck keeps its part name. The text of a slide is its paragraphs (`a:p`),
//! one per line; the speaker notes, the pictures, the charts and the diagrams are not read.

use std::collections::HashMap;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use super::{open_zip, xml_reference, zip_entry_text};
use crate::extraction::ExtractionError;

/// The text of a `.pptx`, one entry per slide.
pub fn slide_pages(path: &Path) -> Result<Vec<String>, ExtractionError> {
    let mut archive = open_zip(path)?;
    let parts = slide_parts(&mut archive)?;
    let mut pages = Vec::with_capacity(parts.len());
    for part in parts {
        let xml = zip_entry_text(&mut archive, &part)?;
        pages.push(slide_text(&xml)?);
    }
    Ok(pages)
}

/// The slide parts in presentation order; the numbered order when the presentation does not say.
fn slide_parts(
    archive: &mut zip::ZipArchive<std::fs::File>,
) -> Result<Vec<String>, ExtractionError> {
    let mut numbered: Vec<(u32, String)> = archive
        .file_names()
        .filter_map(|name| {
            let number = name
                .strip_prefix("ppt/slides/slide")?
                .strip_suffix(".xml")?
                .parse::<u32>()
                .ok()?;
            Some((number, name.to_string()))
        })
        .collect();
    numbered.sort();
    if numbered.is_empty() {
        return Err(ExtractionError::ParseFailed);
    }
    let fallback: Vec<String> = numbered.iter().map(|(_, name)| name.clone()).collect();

    let (Ok(presentation), Ok(relationships)) = (
        zip_entry_text(archive, "ppt/presentation.xml"),
        zip_entry_text(archive, "ppt/_rels/presentation.xml.rels"),
    ) else {
        return Ok(fallback);
    };
    let targets = relationship_targets(&relationships);
    let listed: Vec<String> = slide_ids(&presentation)
        .iter()
        .filter_map(|id| targets.get(id))
        .map(|target| {
            // A target is relative to `ppt/`, or written from the root of the archive.
            let target = target.trim_start_matches('/');
            if target.starts_with("ppt/") {
                target.to_string()
            } else {
                format!("ppt/{target}")
            }
        })
        .filter(|name| fallback.contains(name))
        .collect();
    Ok(if listed.is_empty() { fallback } else { listed })
}

/// Relationship id to target, from a `.rels` part.
fn relationship_targets(xml: &str) -> HashMap<String, String> {
    let mut reader = Reader::from_str(xml);
    let mut found = HashMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) | Ok(Event::Empty(tag))
                if tag.local_name().as_ref() == b"Relationship" =>
            {
                let mut id = None;
                let mut target = None;
                for attribute in tag.attributes().flatten() {
                    let value = attribute
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map(|value| value.into_owned())
                        .unwrap_or_default();
                    match attribute.key.as_ref() {
                        b"Id" => id = Some(value),
                        b"Target" => target = Some(value),
                        _ => {}
                    }
                }
                if let (Some(id), Some(target)) = (id, target) {
                    found.insert(id, target);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    found
}

/// The relationship ids of the slides, in the order the presentation lists them.
fn slide_ids(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    let mut ids = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) | Ok(Event::Empty(tag))
                if tag.local_name().as_ref() == b"sldId" =>
            {
                for attribute in tag.attributes().flatten() {
                    // `r:id`, whatever prefix the namespace is bound to.
                    if attribute.key.local_name().as_ref() == b"id"
                        && attribute.key.as_ref() != b"id"
                    {
                        if let Ok(value) =
                            attribute.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        {
                            ids.push(value.into_owned());
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    ids
}

/// The text of one slide: a line per paragraph, a line break inside a paragraph kept. The `Fallback` of a
/// `mc:AlternateContent` repeats what its `Choice` holds and is not read twice.
fn slide_text(xml: &str) -> Result<String, ExtractionError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut out = String::new();
    let mut in_run_text = false;
    let mut in_fallback = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) => match tag.local_name().as_ref() {
                b"t" => in_run_text = true,
                b"Fallback" => in_fallback += 1,
                _ => {}
            },
            Ok(Event::End(tag)) => match tag.local_name().as_ref() {
                b"t" => in_run_text = false,
                b"Fallback" => in_fallback = in_fallback.saturating_sub(1),
                b"p" if in_fallback == 0 => out.push('\n'),
                _ => {}
            },
            Ok(Event::Text(text)) if in_run_text && in_fallback == 0 => {
                let decoded = text.decode().map_err(|_| ExtractionError::ParseFailed)?;
                out.push_str(&decoded);
            }
            Ok(Event::GeneralRef(reference)) if in_run_text && in_fallback == 0 => {
                let name = reference
                    .decode()
                    .map_err(|_| ExtractionError::ParseFailed)?;
                if let Some(character) = xml_reference(&name) {
                    out.push(character);
                }
            }
            Ok(Event::Empty(tag)) if tag.local_name().as_ref() == b"br" && in_fallback == 0 => {
                out.push('\n')
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err(ExtractionError::ParseFailed),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn slide(paragraphs: &[&str]) -> String {
        let body: String = paragraphs
            .iter()
            .map(|text| format!("<a:p><a:r><a:t>{text}</a:t></a:r></a:p>"))
            .collect();
        slide_xml(&body)
    }

    fn slide_xml(body: &str) -> String {
        format!(
            "<p:sld xmlns:a=\"a\" xmlns:p=\"p\" xmlns:mc=\"m\"><p:cSld><p:spTree><p:sp><p:txBody>{body}</p:txBody></p:sp></p:spTree></p:cSld></p:sld>"
        )
    }

    /// A deck whose presentation lists `listed` (by part name, `/`-rooted when `rooted`) and holds `slides`.
    fn deck(
        dir: &Path,
        listed: &[&str],
        slides: &[(&str, String)],
        rooted: bool,
    ) -> std::path::PathBuf {
        let path = dir.join("deck.pptx");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, xml) in slides {
            writer
                .start_file(format!("ppt/slides/{name}.xml"), options)
                .unwrap();
            writer.write_all(xml.as_bytes()).unwrap();
        }
        let ids: String = listed
            .iter()
            .enumerate()
            .map(|(n, _)| format!("<p:sldId id=\"{}\" r:id=\"rId{}\"/>", 256 + n, n + 10))
            .collect();
        writer.start_file("ppt/presentation.xml", options).unwrap();
        writer
            .write_all(
                format!("<p:presentation xmlns:p=\"p\" xmlns:r=\"r\"><p:sldIdLst>{ids}</p:sldIdLst></p:presentation>")
                    .as_bytes(),
            )
            .unwrap();
        let prefix = if rooted { "/ppt/" } else { "" };
        let rels: String = listed
            .iter()
            .enumerate()
            .map(|(n, name)| {
                format!(
                    "<Relationship Id=\"rId{}\" Type=\"slide\" Target=\"{prefix}slides/{name}.xml\"/>",
                    n + 10
                )
            })
            .collect();
        writer
            .start_file("ppt/_rels/presentation.xml.rels", options)
            .unwrap();
        writer
            .write_all(format!("<Relationships xmlns=\"x\">{rels}</Relationships>").as_bytes())
            .unwrap();
        // The notes of a slide are never read.
        writer
            .start_file("ppt/notesSlides/notesSlide1.xml", options)
            .unwrap();
        writer
            .write_all(slide(&["Speaker note nobody should read"]).as_bytes())
            .unwrap();
        writer.finish().unwrap();
        path
    }

    #[test]
    fn a_slide_is_a_page_in_the_order_the_deck_lists_them_and_notes_are_left_out() {
        let dir = tempfile::tempdir().unwrap();
        // The deck lists slide2 first: it was moved to the front.
        let path = deck(
            dir.path(),
            &["slide2", "slide1"],
            &[
                ("slide1", slide(&["Agenda", "Budget CHF 9 800"])),
                ("slide2", slide(&["Welcome &amp; thanks &#233;"])),
            ],
            false,
        );
        assert_eq!(
            slide_pages(&path).unwrap(),
            vec![
                "Welcome & thanks \u{e9}\n".to_string(),
                "Agenda\nBudget CHF 9 800\n".to_string()
            ]
        );
    }

    #[test]
    fn a_target_written_from_the_root_is_followed() {
        let dir = tempfile::tempdir().unwrap();
        let path = deck(
            dir.path(),
            &["slide2", "slide1"],
            &[("slide1", slide(&["one"])), ("slide2", slide(&["two"]))],
            true,
        );
        assert_eq!(slide_pages(&path).unwrap(), vec!["two\n", "one\n"]);
    }

    #[test]
    fn without_a_listing_the_slides_are_read_in_numbered_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain.pptx");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, text) in [("slide10", "ten"), ("slide2", "two"), ("slide1", "one")] {
            writer
                .start_file(format!("ppt/slides/{name}.xml"), options)
                .unwrap();
            writer.write_all(slide(&[text]).as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        assert_eq!(slide_pages(&path).unwrap(), vec!["one\n", "two\n", "ten\n"]);
    }

    #[test]
    fn a_line_break_is_kept_and_the_fallback_twin_of_an_alternate_is_not_read() {
        let dir = tempfile::tempdir().unwrap();
        let body = "<a:p><a:r><a:t>Line one</a:t></a:r><a:br/><a:r><a:t>line two</a:t></a:r></a:p><mc:AlternateContent><mc:Choice><a:p><a:r><a:t>Shown once</a:t></a:r></a:p></mc:Choice><mc:Fallback><a:p><a:r><a:t>Shown once</a:t></a:r></a:p></mc:Fallback></mc:AlternateContent>";
        let path = deck(
            dir.path(),
            &["slide1"],
            &[("slide1", slide_xml(body))],
            false,
        );
        assert_eq!(
            slide_pages(&path).unwrap(),
            vec!["Line one\nline two\nShown once\n"]
        );
    }

    #[test]
    fn an_archive_with_no_slide_or_no_zip_is_a_parse_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.pptx");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        writer
            .start_file("docProps/app.xml", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.finish().unwrap();
        assert!(matches!(
            slide_pages(&path),
            Err(ExtractionError::ParseFailed)
        ));

        let broken = dir.path().join("broken.pptx");
        std::fs::write(&broken, b"not a zip").unwrap();
        assert!(matches!(
            slide_pages(&broken),
            Err(ExtractionError::ParseFailed)
        ));
    }
}
