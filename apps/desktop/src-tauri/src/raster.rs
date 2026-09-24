//! PDF page to bitmap, in memory, feeding the OCR port
//! (`docs/SPRINT-2.5-ASSESSMENT.md` section E).
//!
//! This module answers exactly one question: how many pages does this PDF have, and what does
//! page N look like as a bitmap. It never decides whether a page needs OCR - `extraction.rs`
//! does that - and it never writes a bitmap to disk. No `#[cfg(windows)]`: the platform
//! difference is which prebuilt `pdfium` library ships as a resource, resolved by path at
//! construction, never here.
//!
//! There is exactly one rasteriser per process, reached through `shared`. pdfium binds itself
//! globally and refuses to be bound twice, so building one per indexing pass silently disabled
//! OCR for scanned PDFs from the second pass onwards (`docs/TROUBLESHOOTING.md`).

use std::path::Path;
use std::sync::OnceLock;

use pdfium_render::prelude::*;

use crate::ocr::OcrError;

/// Recognition-quality DPI for a scanned letter. High enough for Tesseract's LSTM model to read
/// body text reliably, without producing bitmaps so large that a twelve-page report becomes slow
/// to rasterise.
pub const RASTER_DPI: f32 = 300.0;

/// The one slot `shared` fills. Private, and the only place in the process that may call
/// `Rasterizer::new`: see that function's own note on why a second call can never succeed.
static SHARED: OnceLock<Option<Rasterizer>> = OnceLock::new();

/// The process's rasteriser, built on first use and kept for the lifetime of the application.
///
/// `pdfium-render` binds its library into a **process-global** cell: `Pdfium::bind_to_library`
/// returns `PdfiumLibraryBindingsAlreadyInitialized` on every call after the first, however
/// healthy the library file is. A `Rasterizer` built per indexing pass therefore worked on the
/// first pass after launch and silently vanished on every pass after it, taking OCR for every
/// scanned PDF with it while JPEG and PNG - which need no rasteriser - kept working. That is the
/// shape of the bug recorded in `docs/TROUBLESHOOTING.md`; callers must come through here.
///
/// `library_path` is read only on the first call that reaches the library. A later call with a
/// different path returns the instance already bound, because the process cannot bind twice.
/// `None` is not cached against the path either: a caller that cannot find the library yet
/// simply does not call this, so staging the resource mid-session still works on the next pass.
pub fn shared(library_path: &Path) -> Option<&'static Rasterizer> {
    SHARED
        .get_or_init(|| Rasterizer::new(library_path).ok())
        .as_ref()
}

/// Loads the bundled `pdfium` library once and rasterises pages on demand. Constructed once,
/// beside the OCR provider, and shared for the lifetime of the application: reach it through
/// `shared`, never by building one.
pub struct Rasterizer {
    pdfium: Pdfium,
}

impl Rasterizer {
    /// `library_path` is the platform's bundled `pdfium` dynamic library, resolved by the caller
    /// from `app.path().resolve(...)` - never a literal here. Binding failure means the resource
    /// is missing or the wrong build for this platform, which degrades exactly like a missing
    /// Tesseract sidecar: `OcrError::EngineUnavailable`, never a panic.
    ///
    /// Private on purpose. Exactly one call to this may ever happen in a process - the one
    /// inside `shared` - because the binding it performs is global to the process and cannot be
    /// repeated. A second caller anywhere, including a test, would consume the binding and leave
    /// `shared` permanently empty.
    fn new(library_path: &Path) -> Result<Self, OcrError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn crate_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn sandbox_scan() -> PathBuf {
        crate_root()
            .join("..")
            .join("..")
            .join("..")
            .join("fixtures")
            .join("gp-sandbox")
            .join("inbox")
            .join("2026-03-22_courrier-rhumatologie-scan.pdf")
    }

    /// The regression test for the bug in `docs/TROUBLESHOOTING.md`: a rasteriser was built for
    /// every indexing pass, and pdfium's process-global binding made every pass after the first
    /// fail, so a scanned PDF added later in the session was reported unreadable. Asking three
    /// times here stands for three passes; before the fix the second call was already `None`.
    ///
    /// Ignored by default because `resources/pdfium/pdfium.dll` is gitignored
    /// (`resources/README.md`) and only exists once `scripts/fetch-ocr-resources.ps1` has run -
    /// never on CI. Run it by hand after that script with
    /// `cargo test --lib -- --ignored the_shared_rasterizer`.
    ///
    /// The only test allowed to touch `SHARED`: the binding it takes is global to the process,
    /// so a second test that built its own rasteriser would leave this one permanently empty.
    #[test]
    #[ignore]
    fn the_shared_rasterizer_answers_every_pass_not_only_the_first() {
        let library = crate_root()
            .join("resources")
            .join("pdfium")
            .join("pdfium.dll");
        if !library.exists() {
            eprintln!("skipping: pdfium not staged, run scripts/fetch-ocr-resources.ps1");
            return;
        }
        let scan = sandbox_scan();
        assert!(scan.exists(), "fixture missing: {}", scan.display());

        for pass in 1..=3 {
            let rasterizer = shared(&library)
                .unwrap_or_else(|| panic!("pass {pass} must still have a rasteriser"));

            assert_eq!(
                rasterizer.page_count(&scan).expect("counts pages"),
                1,
                "pass {pass} must still read the scanned PDF"
            );
            assert!(
                !rasterizer
                    .rasterize_page(&scan, 1)
                    .expect("rasterises page 1")
                    .is_empty(),
                "pass {pass} must still produce a bitmap for OCR"
            );
        }
    }

    #[test]
    fn the_fake_rasterizer_reports_the_page_count_it_was_given() {
        let rasterizer = FakeRasterizer::with_page_count(3);

        assert_eq!(
            rasterizer
                .page_count(Path::new("scan.pdf"))
                .expect("counts"),
            3
        );
    }

    #[test]
    fn the_fake_rasterizer_records_which_page_was_asked_for() {
        let rasterizer = FakeRasterizer::new();

        let _ = rasterizer.rasterize_page(Path::new("scan.pdf"), 2);

        assert_eq!(
            rasterizer.rasterize_calls(),
            vec![("scan.pdf".to_string(), 2)]
        );
    }
}
