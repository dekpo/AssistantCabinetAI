//! The local OCR port (`docs/SPRINT-2.5-ASSESSMENT.md`, section G).
//!
//! `OcrProvider` is the only thing the rest of the application knows about optical character
//! recognition. It is constructed once, beside extraction, and never in `apps/server` - a
//! scanned page is a document in its most complete form, so the code that reads one lives on
//! the workstation like every other extraction path.
//!
//! No `#[cfg(windows)]` anywhere in this module, nor in anything under `src/ocr/`. A platform
//! difference belongs in the bundler configuration or in sidecar discovery, never here: see
//! `docs/SPRINT-2.5-ASSESSMENT.md` section O and `AGENTS.md`.

/// One local OCR engine. Implementations must not touch the network, and must not write
/// recognised text to disk - patient text belongs only in the local index.
pub trait OcrProvider: Send + Sync {
    /// Stable engine identity, stored with every recognised chunk so a later engine change
    /// invalidates what this one produced. For example "tesseract".
    fn id(&self) -> &str;

    /// Engine build, stored alongside the id. "5.3.3".
    fn version(&self) -> &str;

    /// Whether this engine can read this image format at all. Cheap, format-only: it must not
    /// open the file, and it must never be used to judge content quality.
    fn supports(&self, format: ImageFormat) -> bool;

    /// Recognise one page. One call, one page, so provenance stays page-shaped all the way down
    /// to a citation.
    fn recognise(&self, input: &OcrInput<'_>) -> Result<OcrPage, OcrError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
}

pub struct OcrInput<'a> {
    /// Encoded image bytes, held in memory. A rasterised PDF page, or an image file read
    /// straight off the work folder.
    pub image: &'a [u8],
    pub format: ImageFormat,
    /// Relative to the work folder, so the result carries its own locator.
    pub relative_path: &'a str,
    /// 1-indexed, matching `ExtractedPage` and the citation shown to her.
    pub page_number: u32,
    /// BCP 47 from settings. The engine maps it to its own vocabulary; "fr-FR" -> "fra" is a
    /// lookup table in the implementation, never a literal in the caller.
    pub locale: &'a str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OcrPage {
    pub relative_path: String,
    pub page_number: u32,
    pub text: String,
    /// Mean word confidence in `0.0..=1.0` when the engine reports one. Kept internally, never
    /// rendered as a number to the user (`docs/SPRINT-2.5-ASSESSMENT.md` section G).
    pub confidence: Option<f32>,
    pub engine: String,
    pub engine_version: String,
    pub status: OcrStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrStatus {
    /// Usable text, above the confidence floor.
    Recognised,
    /// The engine read the page but is not confident enough to feed retrieval. The text is
    /// kept for the record and the page is treated as empty.
    BelowThreshold,
    /// The engine ran and found no text. A blank page, or a photograph of a wall.
    NoTextFound,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum OcrError {
    /// The engine is not installed or did not start. The caller degrades to Sprint 2a
    /// behaviour: report the page empty, never guess.
    #[error("engine unavailable")]
    EngineUnavailable,
    /// The engine runs but has no model for this locale.
    #[error("language unavailable: {locale}")]
    LanguageUnavailable { locale: String },
    /// These bytes are not a decodable image.
    #[error("unreadable image")]
    UnreadableImage,
    /// The page took longer than the per-page budget.
    #[error("timeout")]
    Timeout,
}

/// Test double. Public so integration tests (`tests/end_to_end_retrieval.rs`) can inject it
/// beside the fake gateway; `#[cfg(test)]` would hide it from that crate.
pub mod fake;
pub mod tesseract;
