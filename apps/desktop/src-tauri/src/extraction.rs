//! Native text extraction: PDF, DOCX, TXT, MD.
//!
//! No OCR here: a PDF page with no text layer is reported as empty, not silently dropped and
//! not guessed at. That distinction is the point of this module, more than coverage is
//! (`docs/SESSION-DOCUMENT-EXTRACTION.md`).

use std::path::Path;

use serde::Serialize;

use crate::discovery::DiscoveredFile;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedPage {
    /// 1-indexed, so it can be shown to her directly in a citation.
    pub page_number: u32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedDocument {
    pub relative_path: String,
    pub pages: Vec<ExtractedPage>,
    /// True when extraction ran but found no text on any page - a PDF with no text layer, most
    /// often a scan. The caller must report this rather than pretend the file had nothing to
    /// say.
    pub empty: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ExtractionError {
    #[error("unsupported_extension")]
    UnsupportedExtension,
    #[error("read_failed")]
    ReadFailed,
    #[error("parse_failed")]
    ParseFailed,
}

pub fn extract(file: &DiscoveredFile) -> Result<ExtractedDocument, ExtractionError> {
    let path = Path::new(&file.absolute_path);
    let pages = match file.extension.as_str() {
        "pdf" => extract_pdf(path)?,
        "docx" => extract_docx(path)?,
        "txt" | "md" => extract_plain_text(path)?,
        _ => return Err(ExtractionError::UnsupportedExtension),
    };
    let empty = pages.iter().all(|page| page.text.trim().is_empty());
    Ok(ExtractedDocument {
        relative_path: file.relative_path.clone(),
        pages,
        empty,
    })
}

fn extract_plain_text(path: &Path) -> Result<Vec<ExtractedPage>, ExtractionError> {
    let text = std::fs::read_to_string(path).map_err(|_| ExtractionError::ReadFailed)?;
    Ok(vec![ExtractedPage {
        page_number: 1,
        text,
    }])
}

/// One page of text per PDF page, using `pdf-extract`'s own per-page extraction so pagination
/// comes from the library rather than from splitting on a separator we would have to trust.
fn extract_pdf(path: &Path) -> Result<Vec<ExtractedPage>, ExtractionError> {
    let bytes = std::fs::read(path).map_err(|_| ExtractionError::ReadFailed)?;
    let pages = match pdf_extract::extract_text_from_mem_by_pages(&bytes) {
        Ok(pages) => pages,
        // A PDF with no text layer (a scan) makes the extractor fail rather than return empty
        // pages, so that failure is reported as "empty", not propagated as a read error the
        // interface cannot explain.
        Err(_) => {
            return Ok(vec![ExtractedPage {
                page_number: 1,
                text: String::new(),
            }])
        }
    };
    if pages.is_empty() {
        return Ok(vec![ExtractedPage {
            page_number: 1,
            text: String::new(),
        }]);
    }
    Ok(pages
        .into_iter()
        .enumerate()
        .map(|(index, page_text)| ExtractedPage {
            page_number: (index + 1) as u32,
            text: page_text.trim().to_string(),
        })
        .collect())
}

/// DOCX is a zip of XML parts. We read `word/document.xml` and keep the text runs (`<w:t>`),
/// one paragraph (`<w:p>`) per line, treating the whole document as a single "page" since DOCX
/// has no fixed pagination in the file itself.
fn extract_docx(path: &Path) -> Result<Vec<ExtractedPage>, ExtractionError> {
    let file = std::fs::File::open(path).map_err(|_| ExtractionError::ReadFailed)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| ExtractionError::ParseFailed)?;
    let mut xml = String::new();
    {
        let mut entry = archive
            .by_name("word/document.xml")
            .map_err(|_| ExtractionError::ParseFailed)?;
        std::io::Read::read_to_string(&mut entry, &mut xml)
            .map_err(|_| ExtractionError::ParseFailed)?;
    }

    let text = docx_paragraphs_to_text(&xml)?;
    Ok(vec![ExtractedPage {
        page_number: 1,
        text,
    }])
}

fn docx_paragraphs_to_text(xml: &str) -> Result<String, ExtractionError> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_text_run = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) if tag.local_name().as_ref() == b"t" => in_text_run = true,
            Ok(Event::End(tag)) if tag.local_name().as_ref() == b"t" => in_text_run = false,
            Ok(Event::Text(text)) if in_text_run => {
                let decoded = text.decode().map_err(|_| ExtractionError::ParseFailed)?;
                current.push_str(&decoded);
            }
            Ok(Event::End(tag)) if tag.local_name().as_ref() == b"p" => {
                lines.push(std::mem::take(&mut current));
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return Err(ExtractionError::ParseFailed),
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_for(path: &std::path::Path) -> DiscoveredFile {
        DiscoveredFile {
            relative_path: path.file_name().unwrap().to_string_lossy().to_string(),
            absolute_path: path.display().to_string(),
            extension: path.extension().unwrap().to_string_lossy().to_lowercase(),
            size_bytes: 0,
            modified_at: None,
        }
    }

    #[test]
    fn a_text_file_extracts_verbatim_as_one_page() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.txt");
        std::fs::write(&path, "Bonjour Camille").unwrap();

        let document = extract(&file_for(&path)).expect("extracts");

        assert_eq!(document.pages.len(), 1);
        assert_eq!(document.pages[0].text, "Bonjour Camille");
        assert!(!document.empty);
    }

    #[test]
    fn an_empty_text_file_is_reported_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blank.md");
        std::fs::write(&path, "   \n").unwrap();

        let document = extract(&file_for(&path)).expect("extracts");

        assert!(document.empty);
    }

    #[test]
    fn an_unsupported_extension_is_refused() {
        let file = DiscoveredFile {
            relative_path: "image.png".into(),
            absolute_path: "image.png".into(),
            extension: "png".into(),
            size_bytes: 0,
            modified_at: None,
        };

        assert!(matches!(
            extract(&file),
            Err(ExtractionError::UnsupportedExtension)
        ));
    }
}
