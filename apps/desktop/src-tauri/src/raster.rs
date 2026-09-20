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
