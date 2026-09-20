//! PDF page to bitmap, in memory, feeding the OCR port
//! (`docs/SPRINT-2.5-ASSESSMENT.md` section E).
//!
//! This module answers exactly one question: how many pages does this PDF have, and what does
//! page N look like as a bitmap. It never decides whether a page needs OCR - `extraction.rs`
//! does that - and it never writes a bitmap to disk. No `#[cfg(windows)]`: the platform
//! difference is which prebuilt `pdfium` library ships as a resource, resolved by path at
//! construction, never here.

use std::path::Path;

use pdfium_render::prelude::*;

use crate::ocr::OcrError;

/// Recognition-quality DPI for a scanned letter. High enough for Tesseract's LSTM model to read
/// body text reliably, without producing bitmaps so large that a twelve-page report becomes slow
/// to rasterise.
pub const RASTER_DPI: f32 = 300.0;

/// Loads the bundled `pdfium` library once and rasterises pages on demand. Constructed once,
/// beside the OCR provider, and shared for the lifetime of the application.
pub struct Rasterizer {
    pdfium: Pdfium,
}

impl Rasterizer {
    /// `library_path` is the platform's bundled `pdfium` dynamic library, resolved by the caller
    /// from `app.path().resolve(...)` - never a literal here. Binding failure means the resource
    /// is missing or the wrong build for this platform, which degrades exactly like a missing
    /// Tesseract sidecar: `OcrError::EngineUnavailable`, never a panic.
    pub fn new(library_path: &Path) -> Result<Self, OcrError> {
        let bindings = Pdfium::bind_to_library(library_path).map_err(|_| OcrError::EngineUnavailable)?;
        Ok(Self {
            pdfium: Pdfium::new(bindings),
        })
    }

    /// How many pages this PDF has. Used when `pdf_extract` fails to parse the file at all - an
    /// image-only PDF - so every real page reaches OCR instead of the file collapsing into one
    /// empty page (`docs/SPRINT-2.5-ASSESSMENT.md` section H).
    pub fn page_count(&self, pdf_path: &Path) -> Result<u32, OcrError> {
        let document = self.load(pdf_path)?;
        Ok(document.pages().len() as u32)
    }

    /// Renders one page (1-indexed, matching `ExtractedPage`) to PNG bytes, held in memory.
    /// Nothing under this path touches disk: the bitmap lives only in the returned `Vec<u8>`.
    pub fn rasterize_page(&self, pdf_path: &Path, page_number: u32) -> Result<Vec<u8>, OcrError> {
        let document = self.load(pdf_path)?;
        let index = page_number
            .checked_sub(1)
            .ok_or(OcrError::UnreadableImage)?;
        let page = document
            .pages()
            .get(index as i32)
            .map_err(|_| OcrError::UnreadableImage)?;

        let render_config = PdfRenderConfig::new()
            .set_target_width((page.width().value / 72.0 * RASTER_DPI) as i32)
            .set_maximum_height((page.height().value / 72.0 * RASTER_DPI) as i32);

        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|_| OcrError::UnreadableImage)?;
        let rendered = bitmap
            .as_image()
            .map_err(|_| OcrError::UnreadableImage)?;

        let mut png_bytes = Vec::new();
        rendered
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .map_err(|_| OcrError::UnreadableImage)?;
        Ok(png_bytes)
    }

    fn load(&self, pdf_path: &Path) -> Result<PdfDocument<'_>, OcrError> {
        self.pdfium
            .load_pdf_from_file(pdf_path, None)
            .map_err(|_| OcrError::UnreadableImage)
    }
}

/// The only rasterisation surface `extraction.rs` depends on, so tests can inject a fake
/// without loading pdfium (`docs/SPRINT-2.5-ASSESSMENT.md` section M).
pub trait PageRasterizer: Send + Sync {
    fn page_count(&self, pdf_path: &Path) -> Result<u32, OcrError>;
    fn rasterize_page(&self, pdf_path: &Path, page_number: u32) -> Result<Vec<u8>, OcrError>;
}

impl PageRasterizer for Rasterizer {
    fn page_count(&self, pdf_path: &Path) -> Result<u32, OcrError> {
        Rasterizer::page_count(self, pdf_path)
    }

    fn rasterize_page(&self, pdf_path: &Path, page_number: u32) -> Result<Vec<u8>, OcrError> {
        Rasterizer::rasterize_page(self, pdf_path, page_number)
    }
}

/// Returns placeholder PNG bytes and a configurable page count. Extraction tests use this so a
/// scanned page still reaches `OcrProvider` without the bundled pdfium library.
pub struct FakeRasterizer {
    default_page_count: u32,
    page_counts: std::sync::Mutex<std::collections::HashMap<String, u32>>,
    rasterize_calls: std::sync::Mutex<Vec<(String, u32)>>,
}

impl FakeRasterizer {
    pub fn new() -> Self {
        Self {
            default_page_count: 1,
            page_counts: std::sync::Mutex::new(std::collections::HashMap::new()),
            rasterize_calls: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn with_page_count(page_count: u32) -> Self {
        Self {
            default_page_count: page_count,
            ..Self::new()
        }
    }

    /// Override `page_count` for a path or a file name, used when the parse-failure path has
    /// to invent the right number of pages without opening pdfium.
    pub fn set_page_count(&self, path_or_name: &str, page_count: u32) {
        self.page_counts
            .lock()
            .expect("fake rasterizer lock")
            .insert(path_or_name.to_string(), page_count);
    }

    pub fn rasterize_calls(&self) -> Vec<(String, u32)> {
        self.rasterize_calls
            .lock()
            .expect("fake rasterizer lock")
            .clone()
    }
}

impl Default for FakeRasterizer {
    fn default() -> Self {
        Self::new()
    }
}

impl PageRasterizer for FakeRasterizer {
    fn page_count(&self, pdf_path: &Path) -> Result<u32, OcrError> {
        let counts = self.page_counts.lock().expect("fake rasterizer lock");
        let full = pdf_path.to_string_lossy();
        if let Some(&count) = counts.get(full.as_ref()) {
            return Ok(count);
        }
        if let Some(name) = pdf_path.file_name().and_then(|name| name.to_str()) {
            if let Some(&count) = counts.get(name) {
                return Ok(count);
            }
        }
        Ok(self.default_page_count)
    }

    fn rasterize_page(&self, pdf_path: &Path, page_number: u32) -> Result<Vec<u8>, OcrError> {
        self.rasterize_calls
            .lock()
            .expect("fake rasterizer lock")
            .push((pdf_path.to_string_lossy().to_string(), page_number));
        Ok(PLACEHOLDER_PNG.to_vec())
    }
}

/// 1x1 white PNG. Never decoded by the fake provider; present so a caller that does inspect
/// bytes still sees a well-formed image rather than an empty buffer.
const PLACEHOLDER_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xFF, 0xFF, 0x3F,
    0x00, 0x05, 0xFE, 0x02, 0xFE, 0xDC, 0xCC, 0x59, 0xE7, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];
