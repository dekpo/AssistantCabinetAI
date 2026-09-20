//! Native text extraction, with OCR as the fallback path for a page that has no usable
//! text layer (`docs/SPRINT-2.5-ASSESSMENT.md` sections H and I).
//!
//! Per page, cheapest first: keep a PDF text layer of at least `MIN_TEXT_LAYER_CHARS`,
//! otherwise rasterise that page in memory and call `OcrProvider`. A missing provider
//! degrades like `OcrError::EngineUnavailable`: the page is reported empty, never guessed
//! at, and never panics.

use std::path::Path;

use serde::Serialize;

use crate::discovery::DiscoveredFile;
use crate::ocr::{ImageFormat, OcrError, OcrInput, OcrProvider, OcrStatus};
use crate::raster::PageRasterizer;

/// Tuned against `fixtures/gp-sandbox/inbox/2026-03-24_compte-rendu-mixte.pdf`, whose native
/// covering page is a short placeholder. 48 characters, the assessment's starting value,
/// would have sent that page to OCR. A page number or a scanner header still falls below.
const MIN_TEXT_LAYER_CHARS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PageOrigin {
    TextLayer,
    Ocr,
}

impl PageOrigin {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::TextLayer => "text_layer",
            Self::Ocr => "ocr",
        }
    }

    pub fn from_db_str(value: &str) -> Self {
        if value == "ocr" {
            Self::Ocr
        } else {
            Self::TextLayer
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedPage {
    /// 1-indexed, so it can be shown to her directly in a citation.
    pub page_number: u32,
    pub text: String,
    pub origin: PageOrigin,
    /// Mean word confidence in `0.0..=1.0` when the page came from OCR. Never rendered as a
    /// number; kept so a later engine change and the low-confidence summary can use it.
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedDocument {
    pub relative_path: String,
    pub pages: Vec<ExtractedPage>,
    /// True when extraction ran but found no text on any page by any route - a scan the engine
    /// could not read, or a genuinely empty file. The caller must report this rather than
    /// pretend the file had nothing to say.
    pub empty: bool,
    /// True when at least one page went through `OcrProvider`, successfully or not.
    pub used_ocr: bool,
    /// True when at least one page came back `OcrStatus::BelowThreshold`.
    pub low_confidence: bool,
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

pub fn extract(
    file: &DiscoveredFile,
    ocr: Option<&dyn OcrProvider>,
    rasterizer: Option<&dyn PageRasterizer>,
    locale: &str,
) -> Result<ExtractedDocument, ExtractionError> {
    let path = Path::new(&file.absolute_path);
    let (pages, low_confidence) = match file.extension.as_str() {
        "pdf" => extract_pdf(path, &file.relative_path, ocr, rasterizer, locale)?,
        "docx" => (extract_docx(path)?, false),
        "txt" | "md" => (extract_plain_text(path)?, false),
        "jpg" | "jpeg" | "png" => extract_image(path, &file.relative_path, file.extension.as_str(), ocr, locale)?,
        _ => return Err(ExtractionError::UnsupportedExtension),
    };
    let empty = pages.iter().all(|page| page.text.trim().is_empty());
    let used_ocr = pages.iter().any(|page| page.origin == PageOrigin::Ocr);
    Ok(ExtractedDocument {
        relative_path: file.relative_path.clone(),
        pages,
        empty,
        used_ocr,
        low_confidence,
    })
}

fn extract_plain_text(path: &Path) -> Result<Vec<ExtractedPage>, ExtractionError> {
    let text = std::fs::read_to_string(path).map_err(|_| ExtractionError::ReadFailed)?;
    Ok(vec![native_page(1, text)])
}

fn extract_image(
    path: &Path,
    relative_path: &str,
    extension: &str,
    ocr: Option<&dyn OcrProvider>,
    locale: &str,
) -> Result<(Vec<ExtractedPage>, bool), ExtractionError> {
    let bytes = std::fs::read(path).map_err(|_| ExtractionError::ReadFailed)?;
    let format = if extension == "png" {
        ImageFormat::Png
    } else {
        ImageFormat::Jpeg
    };
    let (page, low_confidence) = recognise_page(relative_path, 1, &bytes, format, locale, ocr);
    Ok((vec![page], low_confidence))
}

/// One page of text per PDF page, using `pdf-extract`'s own per-page extraction so pagination
/// comes from the library rather than from splitting on a separator we would have to trust.
/// Pages whose trimmed text layer is shorter than `MIN_TEXT_LAYER_CHARS` are rasterised and
/// sent to OCR (`docs/SPRINT-2.5-ASSESSMENT.md` section H).
fn extract_pdf(
    path: &Path,
    relative_path: &str,
    ocr: Option<&dyn OcrProvider>,
    rasterizer: Option<&dyn PageRasterizer>,
    locale: &str,
) -> Result<(Vec<ExtractedPage>, bool), ExtractionError> {
    let bytes = std::fs::read(path).map_err(|_| ExtractionError::ReadFailed)?;
    match pdf_extract::extract_text_from_mem_by_pages(&bytes) {
        Ok(pages) if !pages.is_empty() => {
            let mut extracted = Vec::with_capacity(pages.len());
            let mut low_confidence = false;
            for (index, page_text) in pages.into_iter().enumerate() {
                let page_number = (index + 1) as u32;
                let trimmed = page_text.trim();
                if trimmed.chars().count() >= MIN_TEXT_LAYER_CHARS {
                    extracted.push(native_page(page_number, trimmed.to_string()));
                    continue;
                }
                let (page, page_low) =
                    ocr_pdf_page(path, relative_path, page_number, ocr, rasterizer, locale);
                low_confidence |= page_low;
                extracted.push(page);
            }
            Ok((extracted, low_confidence))
        }
        // A PDF with no text layer (a scan) makes the extractor fail rather than return empty
        // pages. With a rasteriser, that failure becomes "ask how many pages there are, and
        // send every one of them to OCR" rather than collapsing into a single empty page.
        Ok(_) | Err(_) => {
            let Some(rasterizer) = rasterizer else {
                return Ok((vec![empty_page(1, PageOrigin::TextLayer)], false));
            };
            let page_count = match rasterizer.page_count(path) {
                Ok(count) if count > 0 => count,
                _ => return Ok((vec![empty_page(1, PageOrigin::TextLayer)], false)),
            };
            let mut extracted = Vec::with_capacity(page_count as usize);
            let mut low_confidence = false;
            for page_number in 1..=page_count {
                let (page, page_low) =
                    ocr_pdf_page(path, relative_path, page_number, ocr, Some(rasterizer), locale);
                low_confidence |= page_low;
                extracted.push(page);
            }
            Ok((extracted, low_confidence))
        }
    }
}

fn ocr_pdf_page(
    path: &Path,
    relative_path: &str,
    page_number: u32,
    ocr: Option<&dyn OcrProvider>,
    rasterizer: Option<&dyn PageRasterizer>,
    locale: &str,
) -> (ExtractedPage, bool) {
    let Some(rasterizer) = rasterizer else {
        return (empty_page(page_number, PageOrigin::TextLayer), false);
    };
    match rasterizer.rasterize_page(path, page_number) {
        Ok(png) => recognise_page(
            relative_path,
            page_number,
            &png,
            ImageFormat::Png,
            locale,
            ocr,
        ),
        Err(OcrError::EngineUnavailable) => (empty_page(page_number, PageOrigin::TextLayer), false),
        Err(_) => (empty_page(page_number, PageOrigin::Ocr), false),
    }
}

fn recognise_page(
    relative_path: &str,
    page_number: u32,
    image: &[u8],
    format: ImageFormat,
    locale: &str,
    ocr: Option<&dyn OcrProvider>,
) -> (ExtractedPage, bool) {
    let Some(provider) = ocr else {
        return (empty_page(page_number, PageOrigin::TextLayer), false);
    };
    if !provider.supports(format) {
        return (empty_page(page_number, PageOrigin::TextLayer), false);
    }
    match provider.recognise(&OcrInput {
        image,
        format,
        relative_path,
        page_number,
        locale,
    }) {
        Ok(page) if page.status == OcrStatus::Recognised => (
            ExtractedPage {
                page_number,
                text: page.text,
                origin: PageOrigin::Ocr,
                confidence: page.confidence,
            },
            false,
        ),
        Ok(page) if page.status == OcrStatus::BelowThreshold => (
            ExtractedPage {
                page_number,
                text: String::new(),
                origin: PageOrigin::Ocr,
                confidence: page.confidence,
            },
            true,
        ),
        Ok(_) => (empty_page(page_number, PageOrigin::Ocr), false),
        Err(OcrError::EngineUnavailable) | Err(OcrError::LanguageUnavailable { .. }) => {
            (empty_page(page_number, PageOrigin::TextLayer), false)
        }
        Err(_) => (empty_page(page_number, PageOrigin::Ocr), false),
    }
}

fn native_page(page_number: u32, text: String) -> ExtractedPage {
    ExtractedPage {
        page_number,
        text,
        origin: PageOrigin::TextLayer,
        confidence: None,
    }
}

fn empty_page(page_number: u32, origin: PageOrigin) -> ExtractedPage {
    ExtractedPage {
        page_number,
        text: String::new(),
        origin,
        confidence: None,
    }
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
    Ok(vec![native_page(1, text)])
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
    use crate::ocr::fake::FakeOcrProvider;
    use crate::ocr::{OcrError, OcrStatus};
    use crate::raster::FakeRasterizer;
    use std::path::PathBuf;
    use walkdir::WalkDir;

    fn file_for(path: &std::path::Path) -> DiscoveredFile {
        DiscoveredFile {
            relative_path: path.file_name().unwrap().to_string_lossy().to_string(),
            absolute_path: path.display().to_string(),
            extension: path.extension().unwrap().to_string_lossy().to_lowercase(),
            size_bytes: 0,
            modified_at: None,
        }
    }

    fn extract_native(file: &DiscoveredFile) -> Result<ExtractedDocument, ExtractionError> {
        extract(file, None, None, "fr-FR")
    }

    fn sandbox(name: &str) -> DiscoveredFile {
        let path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("fixtures")
            .join("gp-sandbox")
            .join("inbox")
            .join(name);
        assert!(path.exists(), "fixture missing: {}", path.display());
        file_for(&path)
    }

    fn list_files(root: &Path) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.into_path())
            .collect();
        files.sort();
        files
    }

    fn write_text_pdf(path: &Path, pages: &[&str]) {
        let mut objects: Vec<Vec<u8>> = Vec::new();
        let mut add = |body: Vec<u8>| {
            objects.push(body);
            objects.len() as u32
        };

        let font_num = add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec());
        let mut content_nums = Vec::new();
        for page_text in pages {
            let escaped = page_text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
            let stream = format!("BT /F1 11 Tf 72 760 Td ({escaped}) Tj ET");
            let body = format!("<< /Length {} >>\nstream\n{}\nendstream", stream.len(), stream);
            content_nums.push(add(body.into_bytes()));
        }
        let mut page_nums = Vec::new();
        for content_num in &content_nums {
            page_nums.push(add(
                format!(
                    "<< /Type /Page /Parent 0 0 R /MediaBox [0 0 612 792] \
                     /Resources << /Font << /F1 {font_num} 0 R >> >> /Contents {content_num} 0 R >>"
                )
                .into_bytes(),
            ));
        }
        let kids = page_nums
            .iter()
            .map(|n| format!("{n} 0 R"))
            .collect::<Vec<_>>()
            .join(" ");
        let pages_num = add(
            format!(
                "<< /Type /Pages /Kids [{kids}] /Count {} >>",
                page_nums.len()
            )
            .into_bytes(),
        );
        let catalog_num = add(format!("<< /Type /Catalog /Pages {pages_num} 0 R >>").into_bytes());
        for page_num in &page_nums {
            let index = (*page_num as usize) - 1;
            let patched = objects[index]
                .clone()
                .into_iter()
                .collect::<Vec<u8>>();
            let as_text = String::from_utf8(patched).expect("ascii pdf");
            objects[index] = as_text
                .replace("/Parent 0 0 R", &format!("/Parent {pages_num} 0 R"))
                .into_bytes();
        }

        let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut offsets = vec![0u32];
        for (index, body) in objects.iter().enumerate() {
            offsets.push(out.len() as u32);
            out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref_offset = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets.iter().skip(1) {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(b"trailer\n");
        out.extend_from_slice(
            format!("<< /Size {} /Root {catalog_num} 0 R >>\n", objects.len() + 1).as_bytes(),
        );
        out.extend_from_slice(b"startxref\n");
        out.extend_from_slice(format!("{xref_offset}\n").as_bytes());
        out.extend_from_slice(b"%%EOF");
        std::fs::write(path, out).expect("writes pdf");
    }

    #[test]
    fn a_text_file_extracts_verbatim_as_one_page() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.txt");
        std::fs::write(&path, "Bonjour Camille").unwrap();

        let document = extract_native(&file_for(&path)).expect("extracts");

        assert_eq!(document.pages.len(), 1);
        assert_eq!(document.pages[0].text, "Bonjour Camille");
        assert_eq!(document.pages[0].origin, PageOrigin::TextLayer);
        assert!(!document.empty);
    }

    #[test]
    fn an_empty_text_file_is_reported_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blank.md");
        std::fs::write(&path, "   \n").unwrap();

        let document = extract_native(&file_for(&path)).expect("extracts");

        assert!(document.empty);
    }

    #[test]
    fn an_unsupported_extension_is_refused() {
        let file = DiscoveredFile {
            relative_path: "data.csv".into(),
            absolute_path: "data.csv".into(),
            extension: "csv".into(),
            size_bytes: 0,
            modified_at: None,
        };

        assert!(matches!(
            extract_native(&file),
            Err(ExtractionError::UnsupportedExtension)
        ));
    }

    #[test]
    fn a_pdf_with_a_text_layer_invokes_the_provider_zero_times() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("letter.pdf");
        write_text_pdf(
            &path,
            &["Native covering letter with enough characters to pass the text-layer threshold."],
        );
        let ocr = FakeOcrProvider::new();
        let rasterizer = FakeRasterizer::new();

        let document = extract(&file_for(&path), Some(&ocr), Some(&rasterizer), "fr-FR").expect("extracts");

        assert_eq!(ocr.call_count(), 0);
        assert_eq!(document.pages[0].origin, PageOrigin::TextLayer);
        assert!(!document.used_ocr);
        assert!(!document.empty);
    }

    #[test]
    fn ocr_writes_no_file_outside_the_work_folder() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.png");
        std::fs::write(&path, b"not-a-real-png").unwrap();
        let before = list_files(dir.path());
        let hash_before = sha2_hex(&path);

        let ocr = FakeOcrProvider::new();
        extract(&file_for(&path), Some(&ocr), None, "fr-FR").expect("extracts");

        let after = list_files(dir.path());
        assert_eq!(before, after);
        assert_eq!(hash_before, sha2_hex(&path));
        assert_eq!(ocr.call_count(), 1);
    }

    #[test]
    fn an_image_only_pdf_is_read_by_ocr() {
        let ocr = FakeOcrProvider::new();
        let rasterizer = FakeRasterizer::new();
        let file = sandbox("2026-03-22_courrier-rhumatologie-scan.pdf");

        let document = extract(&file, Some(&ocr), Some(&rasterizer), "fr-FR").expect("extracts");

        assert!(ocr.call_count() >= 1);
        assert_eq!(document.pages[0].origin, PageOrigin::Ocr);
        assert_eq!(document.pages[0].page_number, 1);
        assert!(!document.pages[0].text.trim().is_empty());
        assert!(document.used_ocr);
    }

    #[test]
    fn a_mixed_pdf_calls_ocr_exactly_once_for_the_scanned_page() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mixed.pdf");
        write_text_pdf(
            &path,
            &[
                "Native covering letter with enough characters to pass the text-layer threshold.",
                "",
            ],
        );
        let ocr = FakeOcrProvider::new();
        let rasterizer = FakeRasterizer::with_page_count(2);

        let document = extract(&file_for(&path), Some(&ocr), Some(&rasterizer), "fr-FR").expect("extracts");

        assert_eq!(ocr.call_count(), 1);
        assert_eq!(ocr.calls(), vec![(path.file_name().unwrap().to_string_lossy().to_string(), 2)]);
        assert_eq!(document.pages.len(), 2);
        assert_eq!(document.pages[0].origin, PageOrigin::TextLayer);
        assert_eq!(document.pages[1].origin, PageOrigin::Ocr);
        assert_eq!(document.pages[1].page_number, 2);
    }

    #[test]
    fn the_sandbox_mixed_pdf_costs_exactly_one_ocr_call() {
        let ocr = FakeOcrProvider::new();
        let rasterizer = FakeRasterizer::with_page_count(2);
        let file = sandbox("2026-03-24_compte-rendu-mixte.pdf");

        let document = extract(&file, Some(&ocr), Some(&rasterizer), "fr-FR").expect("extracts");

        assert_eq!(ocr.call_count(), 1);
        assert_eq!(ocr.calls()[0].1, 2);
        assert_eq!(document.pages[1].origin, PageOrigin::Ocr);
    }

    #[test]
    fn a_jpeg_is_a_one_page_document() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.jpg");
        std::fs::write(&path, b"not-a-real-jpeg").unwrap();
        let ocr = FakeOcrProvider::new();

        let document = extract(&file_for(&path), Some(&ocr), None, "fr-FR").expect("extracts");

        assert_eq!(document.pages.len(), 1);
        assert_eq!(document.pages[0].page_number, 1);
        assert_eq!(document.pages[0].origin, PageOrigin::Ocr);
        assert_eq!(ocr.call_count(), 1);
    }

    #[test]
    fn a_png_is_a_one_page_document() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.png");
        std::fs::write(&path, b"not-a-real-png").unwrap();
        let ocr = FakeOcrProvider::new();

        let document = extract(&file_for(&path), Some(&ocr), None, "fr-FR").expect("extracts");

        assert_eq!(document.pages.len(), 1);
        assert_eq!(document.pages[0].page_number, 1);
        assert_eq!(document.pages[0].origin, PageOrigin::Ocr);
    }

    #[test]
    fn an_unreadable_image_produces_an_empty_page() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.png");
        std::fs::write(&path, b"not-a-real-png").unwrap();
        let ocr = FakeOcrProvider::new();
        ocr.set_fallback_error(OcrError::UnreadableImage);

        let document = extract(&file_for(&path), Some(&ocr), None, "fr-FR").expect("extracts");

        assert!(document.empty);
        assert_eq!(document.pages[0].text, "");
        assert_eq!(document.pages[0].origin, PageOrigin::Ocr);
    }

    #[test]
    fn below_threshold_text_does_not_appear_on_the_page() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.png");
        std::fs::write(&path, b"not-a-real-png").unwrap();
        let ocr = FakeOcrProvider::new();
        ocr.set_fallback_status(OcrStatus::BelowThreshold);

        let document = extract(&file_for(&path), Some(&ocr), None, "fr-FR").expect("extracts");

        assert!(document.empty);
        assert!(document.low_confidence);
        assert!(document.pages[0].text.is_empty());
    }

    #[test]
    fn engine_unavailable_degrades_to_an_empty_page_rather_than_failing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.png");
        std::fs::write(&path, b"not-a-real-png").unwrap();
        let ocr = FakeOcrProvider::new();
        ocr.set_fallback_error(OcrError::EngineUnavailable);

        let document = extract(&file_for(&path), Some(&ocr), None, "fr-FR").expect("extracts");

        assert!(document.empty);
        assert!(!document.used_ocr);
        assert_eq!(document.pages[0].origin, PageOrigin::TextLayer);
    }

    #[test]
    fn a_missing_provider_degrades_like_engine_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.png");
        std::fs::write(&path, b"not-a-real-png").unwrap();

        let document = extract_native(&file_for(&path)).expect("extracts");

        assert!(document.empty);
        assert!(!document.used_ocr);
    }

    fn sha2_hex(path: &Path) -> String {
        use sha2::{Digest, Sha256};
        let bytes = std::fs::read(path).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        format!("{:x}", hasher.finalize())
    }
}
