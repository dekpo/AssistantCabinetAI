//! What the Work Folder holds, one record per physical file.
//!
//! A `FileRecord` answers "which file is this?" and nothing else. It is deliberately generic:
//! the same record must describe a PDF the document pipeline indexed, an image OCR could not
//! read, and - later - a spreadsheet a tabular pipeline will own (`docs/WORK-FOLDER-INVENTORY.md`).
//! Page counts, chunk counts, OCR confidence, sheet dimensions and column types therefore do
//! **not** live here; they belong to whichever domain layer produced them. The one concession is
//! `index_metadata`, an optional per-domain summary, which is what keeps "is this file indexed?"
//! answerable without opening a second database.
//!
//! `FileRecord` does not replace `Source` (`docs/ARCHITECTURE.md`). `Source` answers "which
//! evidence from that file supports this answer?" and keeps its own origin, locator and
//! derivation. The two meet on one value: the content SHA-256.

use serde::Serialize;
use sha2::{Digest, Sha256};

/// How the document pipeline classifies a filesystem object. Extended by adding a variant: the
/// wire form is a snake_case string, so an older client reading a newer record sees an unknown
/// string rather than a shifted number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    /// Plain text the extractor reads directly: `.txt`, `.md`.
    DocumentText,
    DocumentPdf,
    /// Word Open XML: `.docx`.
    DocumentOffice,
    /// A picture. Its text, if any, is recovered by OCR at ingestion.
    Image,
    /// Recognised as a future tabular source and deliberately **not** ingested. Sprint 2b owns
    /// `.csv`, `.xls` and `.xlsx` and gives them their own pipeline; flattening one into
    /// document chunks now would be exactly the mistake `docs/RETRIEVAL.md` refuses.
    TabularCandidate,
    /// A known extension no pipeline in this product reads.
    Unsupported,
    /// No extension at all.
    Other,
}

impl FileKind {
    /// From the lowercase extension, without the dot. The only place a file's type is decided.
    pub fn from_extension(extension: &str) -> Self {
        match extension {
            "txt" | "md" => Self::DocumentText,
            "pdf" => Self::DocumentPdf,
            "docx" => Self::DocumentOffice,
            "jpg" | "jpeg" | "png" => Self::Image,
            "csv" | "xls" | "xlsx" | "xlsm" => Self::TabularCandidate,
            "" => Self::Other,
            _ => Self::Unsupported,
        }
    }

    /// Whether the document pipeline is the owner of this kind. A tabular candidate is not, and
    /// must not be counted as an unreadable document because nothing extracted it.
    pub fn is_document(self) -> bool {
        matches!(
            self,
            Self::DocumentText | Self::DocumentPdf | Self::DocumentOffice | Self::Image
        )
    }

    /// Stable machine code. The interface localises it; Rust never writes the word.
    pub fn as_code(self) -> &'static str {
        match self {
            Self::DocumentText => "document_text",
            Self::DocumentPdf => "document_pdf",
            Self::DocumentOffice => "document_office",
            Self::Image => "image",
            Self::TabularCandidate => "tabular_candidate",
            Self::Unsupported => "unsupported",
            Self::Other => "other",
        }
    }
}

/// Whether usable content could be recovered from this file. Kept apart from
/// `ProcessingStatus` on purpose: "we could not read it" and "we have not tried yet" are
/// different answers, and collapsing them is how a folder that was never analysed comes to
/// report five unreadable files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Readability {
    Readable,
    Unreadable,
    /// Nothing has tried to read it: never analysed, or not a kind this pipeline owns.
    NotAssessed,
}

impl Readability {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Readable => "readable",
            Self::Unreadable => "unreadable",
            Self::NotAssessed => "not_assessed",
        }
    }
}

/// Where the file has got to in the ingestion pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingStatus {
    /// Seen on disk, nothing more.
    Discovered,
    /// Indexed once, but the bytes have changed since: the stored content is stale and the next
    /// pass will redo it.
    Pending,
    /// Being processed right now. Reserved for a future progressive indexing pass; no code path
    /// stores it today, and it exists so that pass does not have to change this contract.
    Processing,
    Indexed,
    /// Processed, and produced nothing usable.
    Failed,
}

impl ProcessingStatus {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Discovered => "discovered",
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Indexed => "indexed",
            Self::Failed => "failed",
        }
    }
}

/// How the content was obtained. `RecognisedOcr` is not a flavour of `NativeText`: "the letter
/// says 6.8" and "a machine thinks the letter says 6.8" are different claims, which is the same
/// distinction `Source.derivation` carries between `Extracted` and `Recognised`
/// (`docs/ARCHITECTURE.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionMethod {
    NativeText,
    RecognisedOcr,
    /// Nothing was extracted: never processed, or processed with no usable result.
    None,
}

impl ExtractionMethod {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::NativeText => "native_text",
            Self::RecognisedOcr => "recognised_ocr",
            Self::None => "none",
        }
    }
}

/// The document index's own view of one file. Optional, and deliberately small: it is a view
/// over `IndexStore`, not a copy of it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexMetadata {
    /// The content hash the index recorded. Differs from `FileRecord::sha256` when the file has
    /// changed on disk since the last pass.
    pub indexed_sha256: String,
    pub chunk_count: u64,
    /// Present only when this file's stored chunks came from OCR.
    pub ocr_engine: Option<String>,
    pub ocr_engine_version: Option<String>,
}

/// One physical file in the Work Folder.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    /// Stable identity. The content SHA-256, which is the same value `Source.origin.sha256`
    /// carries, so a file that is renamed or moved keeps its identity and no second hashing
    /// model is introduced. Two byte-identical copies share it, which is a fact about the folder
    /// rather than a defect: `WorkFolderInventory::find_by_id` therefore returns every record
    /// carrying the id and lets the caller decide, exactly as it does for an ambiguous reference.
    /// When the bytes cannot be read at all, it falls back to a hash of the canonical relative
    /// path, so every discovered file has one.
    pub id: String,
    /// Canonical, relative to the Work Folder, forward-slash separated. The user-facing identity
    /// of a file inside the folder, and the only path form that reaches the model.
    pub relative_path: String,
    /// Exactly what the filesystem calls it, case included.
    pub name: String,
    pub stem: String,
    /// Lowercase, without the dot, empty when the file has none. Read from the filesystem and
    /// never inferred from content: a `.txt` file claiming to be a PDF stays a `.txt` file.
    pub extension: String,
    pub kind: FileKind,
    /// Media type for the extension, for a client that wants to render or open the file. Best
    /// effort; `application/octet-stream` when unknown.
    pub mime_type: String,
    pub size_bytes: u64,
    /// Seconds since the Unix epoch, or `None` when the platform did not report one.
    pub modified_at: Option<i64>,
    /// Content fingerprint. `None` when the bytes could not be read.
    pub sha256: Option<String>,
    pub readability: Readability,
    pub processing_status: ProcessingStatus,
    pub extraction_method: ExtractionMethod,
    pub indexed: bool,
    pub index_metadata: Option<IndexMetadata>,
}

impl FileRecord {
    /// The folder this file sits in, relative to the Work Folder root. Empty at the root.
    pub fn parent_path(&self) -> &str {
        match self.relative_path.rfind('/') {
            Some(position) => &self.relative_path[..position],
            None => "",
        }
    }
}

/// Split a file name into its stem and its lowercase extension, the way the filesystem means
/// them. A leading-dot name (`.gitignore`) is all stem and has no extension.
pub fn split_name(name: &str) -> (String, String) {
    match name.rfind('.') {
        Some(position) if position > 0 => (
            name[..position].to_string(),
            name[position + 1..].to_lowercase(),
        ),
        _ => (name.to_string(), String::new()),
    }
}

pub fn mime_type_for(extension: &str) -> &'static str {
    match extension {
        "txt" => "text/plain",
        "md" => "text/markdown",
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "csv" => "text/csv",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
}

/// The fallback identity for a file whose bytes could not be read. Prefixed so it can never
/// collide with a real content hash, and so a reader can tell the two apart.
pub fn path_identity(relative_path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(relative_path.as_bytes());
    format!("path-{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_extension_decides_the_kind() {
        assert_eq!(FileKind::from_extension("pdf"), FileKind::DocumentPdf);
        assert_eq!(FileKind::from_extension("md"), FileKind::DocumentText);
        assert_eq!(FileKind::from_extension("docx"), FileKind::DocumentOffice);
        assert_eq!(FileKind::from_extension("png"), FileKind::Image);
        assert_eq!(FileKind::from_extension("zip"), FileKind::Unsupported);
        assert_eq!(FileKind::from_extension(""), FileKind::Other);
    }

    #[test]
    fn a_spreadsheet_is_recognised_without_becoming_a_document() {
        for extension in ["csv", "xls", "xlsx"] {
            let kind = FileKind::from_extension(extension);
            assert_eq!(kind, FileKind::TabularCandidate);
            assert!(
                !kind.is_document(),
                "a tabular candidate must not enter the document pipeline"
            );
        }
    }

    #[test]
    fn a_name_splits_into_stem_and_lowercase_extension() {
        assert_eq!(
            split_name("neurologie.pdf"),
            ("neurologie".into(), "pdf".into())
        );
        assert_eq!(split_name("REPORT.PDF"), ("REPORT".into(), "pdf".into()));
        assert_eq!(
            split_name("archive.tar.gz"),
            ("archive.tar".into(), "gz".into())
        );
        assert_eq!(split_name("NOTICE"), ("NOTICE".into(), "".into()));
        assert_eq!(split_name(".gitignore"), (".gitignore".into(), "".into()));
    }

    #[test]
    fn a_path_identity_is_stable_and_cannot_be_mistaken_for_a_content_hash() {
        let first = path_identity("2026/mars/neurologie.pdf");

        assert_eq!(first, path_identity("2026/mars/neurologie.pdf"));
        assert_ne!(first, path_identity("2026/janvier/neurologie.pdf"));
        assert!(first.starts_with("path-"));
    }

    #[test]
    fn the_wire_form_of_every_enum_is_a_stable_string() {
        assert_eq!(
            serde_json::to_value(FileKind::TabularCandidate).unwrap(),
            serde_json::json!("tabular_candidate")
        );
        assert_eq!(
            serde_json::to_value(ExtractionMethod::RecognisedOcr).unwrap(),
            serde_json::json!("recognised_ocr")
        );
        assert_eq!(
            serde_json::to_value(ProcessingStatus::Indexed).unwrap(),
            serde_json::json!("indexed")
        );
    }
}
