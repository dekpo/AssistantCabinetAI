//! Machine codes crossing the bridge.
//!
//! Rust never writes a sentence for the user. Every failure is a stable code plus structured
//! data, and the React catalogues turn it into the practice's language. A French literal in this
//! crate is a bug: see docs/LANGUAGE-AND-LOCALE.md.

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use serde_json::{json, Value};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("internal_error")]
    Internal,

    #[error("settings_read_failed")]
    SettingsReadFailed,

    #[error("settings_write_failed")]
    SettingsWriteFailed,

    #[error("server_url_invalid")]
    ServerUrlInvalid { url: String },

    /// The dialog was closed without a choice. Not a failure, but the caller must know.
    #[error("work_folder_selection_cancelled")]
    WorkFolderSelectionCancelled,

    #[error("work_folder_not_found")]
    WorkFolderNotFound { path: String },

    #[error("work_folder_not_a_directory")]
    WorkFolderNotADirectory { path: String },

    #[error("work_folder_path_not_absolute")]
    WorkFolderPathNotAbsolute { path: String },

    #[error("work_folder_is_drive_root")]
    WorkFolderIsDriveRoot { path: String },

    #[error("work_folder_is_protected")]
    WorkFolderIsProtected { path: String },

    /// A folder a sync client mirrors. `service` carries the product name, which is a trademark
    /// rather than prose, so it travels as data and is not translated.
    #[error("work_folder_is_cloud_synced")]
    WorkFolderIsCloudSynced { path: String, service: String },

    /// A folder that passed the rules when it was chosen and no longer does - most often because
    /// OneDrive was switched on afterwards. Reported at startup, not while writing.
    #[error("work_folder_no_longer_allowed")]
    WorkFolderNoLongerAllowed,

    #[error("work_folder_is_symlink")]
    WorkFolderIsSymlink { path: String },

    #[error("work_folder_unc_not_supported")]
    WorkFolderUncNotSupported { path: String },

    #[error("work_folder_not_writable")]
    WorkFolderNotWritable { path: String },

    /// The system file manager could not be started. Nothing was read, written or moved: the
    /// folder is exactly as it was, and she can still open it herself.
    #[error("work_folder_reveal_failed")]
    WorkFolderRevealFailed,

    #[error("server_unreachable")]
    ServerUnreachable { url: String },

    #[error("server_timeout")]
    ServerTimeout { url: String },

    #[error("server_error")]
    ServerError { status: u16 },

    #[error("server_response_invalid")]
    ServerResponseInvalid,

    #[error("context_too_large")]
    ContextTooLarge { chars: usize, limit: usize },

    /// A code the gateway produced. It travels through unchanged so the catalogue localises the
    /// gateway's own vocabulary rather than a paraphrase of it.
    #[error("{code}")]
    Gateway { code: String, data: Value },

    #[error("no_work_folder_set")]
    NoWorkFolderSet,

    #[error("index_unavailable")]
    IndexUnavailable,

    /// Extraction ran and found no text at all in the file: a scan, or a genuinely empty file.
    /// Reported, never silently skipped (`docs/RETRIEVAL.md`).
    #[error("extraction_empty")]
    ExtractionEmpty { path: String },

    #[error("extraction_failed")]
    ExtractionFailed { path: String },

    /// The sourced-answer path found too little evidence to answer responsibly. The product
    /// says so instead of generating an unsupported answer (`docs/ARCHITECTURE.md`).
    #[error("insufficient_evidence")]
    InsufficientEvidence,

    /// She stopped the question herself, while it was still being worked on. Not a failure: the
    /// interface keeps no banner and nothing of the abandoned run (`docs/CHAT-UX-ASSESSMENT.md`).
    #[error("chat_cancelled")]
    ChatCancelled,

    /// The OCR engine is not installed, or its startup probe failed. The product degrades to
    /// Sprint 2a behaviour: a page with no usable text layer is reported empty, not guessed at.
    #[error("ocr_unavailable")]
    OcrUnavailable,

    /// The engine has no model for this locale. `locale` is BCP 47, not translated.
    #[error("ocr_language_unavailable")]
    OcrLanguageUnavailable { locale: String },

    /// The engine ran on this page and failed: an unreadable image, or the per-page timeout.
    #[error("ocr_failed")]
    OcrFailed { path: String, page: u32 },

    /// The engine read the page but found nothing worth keeping - no text, or text below the
    /// confidence floor. The page is reported empty, exactly like a page with no text layer.
    #[error("ocr_page_unreadable")]
    OcrPageUnreadable { path: String, page: u32 },
}

impl AppError {
    pub fn code(&self) -> &str {
        match self {
            Self::Internal => "internal_error",
            Self::SettingsReadFailed => "settings_read_failed",
            Self::SettingsWriteFailed => "settings_write_failed",
            Self::ServerUrlInvalid { .. } => "server_url_invalid",
            Self::WorkFolderSelectionCancelled => "work_folder_selection_cancelled",
            Self::WorkFolderNotFound { .. } => "work_folder_not_found",
            Self::WorkFolderNotADirectory { .. } => "work_folder_not_a_directory",
            Self::WorkFolderPathNotAbsolute { .. } => "work_folder_path_not_absolute",
            Self::WorkFolderIsDriveRoot { .. } => "work_folder_is_drive_root",
            Self::WorkFolderIsProtected { .. } => "work_folder_is_protected",
            Self::WorkFolderIsCloudSynced { .. } => "work_folder_is_cloud_synced",
            Self::WorkFolderNoLongerAllowed => "work_folder_no_longer_allowed",
            Self::WorkFolderIsSymlink { .. } => "work_folder_is_symlink",
            Self::WorkFolderUncNotSupported { .. } => "work_folder_unc_not_supported",
            Self::WorkFolderNotWritable { .. } => "work_folder_not_writable",
            Self::WorkFolderRevealFailed => "work_folder_reveal_failed",
            Self::ServerUnreachable { .. } => "server_unreachable",
            Self::ServerTimeout { .. } => "server_timeout",
            Self::ServerError { .. } => "server_error",
            Self::ServerResponseInvalid => "server_response_invalid",
            Self::ContextTooLarge { .. } => "context_too_large",
            Self::Gateway { code, .. } => code,
            Self::NoWorkFolderSet => "no_work_folder_set",
            Self::IndexUnavailable => "index_unavailable",
            Self::ExtractionEmpty { .. } => "extraction_empty",
            Self::ExtractionFailed { .. } => "extraction_failed",
            Self::InsufficientEvidence => "insufficient_evidence",
            Self::ChatCancelled => "chat_cancelled",
            Self::OcrUnavailable => "ocr_unavailable",
            Self::OcrLanguageUnavailable { .. } => "ocr_language_unavailable",
            Self::OcrFailed { .. } => "ocr_failed",
            Self::OcrPageUnreadable { .. } => "ocr_page_unreadable",
        }
    }

    /// What the sentence needs interpolated. Never document text.
    pub fn data(&self) -> Value {
        match self {
            Self::Internal
            | Self::SettingsReadFailed
            | Self::SettingsWriteFailed
            | Self::WorkFolderSelectionCancelled
            | Self::WorkFolderNoLongerAllowed
            | Self::ServerResponseInvalid
            | Self::NoWorkFolderSet
            | Self::IndexUnavailable
            | Self::InsufficientEvidence
            | Self::ChatCancelled
            | Self::WorkFolderRevealFailed
            | Self::OcrUnavailable => json!({}),
            Self::ExtractionEmpty { path } | Self::ExtractionFailed { path } => {
                json!({ "path": path })
            }
            Self::OcrLanguageUnavailable { locale } => json!({ "locale": locale }),
            Self::OcrFailed { path, page } | Self::OcrPageUnreadable { path, page } => {
                json!({ "path": path, "page": page })
            }
            Self::WorkFolderIsCloudSynced { path, service } => {
                json!({ "path": path, "service": service })
            }
            Self::ServerUrlInvalid { url }
            | Self::ServerUnreachable { url }
            | Self::ServerTimeout { url } => json!({ "url": url }),
            Self::WorkFolderNotFound { path }
            | Self::WorkFolderNotADirectory { path }
            | Self::WorkFolderPathNotAbsolute { path }
            | Self::WorkFolderIsDriveRoot { path }
            | Self::WorkFolderIsProtected { path }
            | Self::WorkFolderIsSymlink { path }
            | Self::WorkFolderUncNotSupported { path }
            | Self::WorkFolderNotWritable { path } => json!({ "path": path }),
            Self::ServerError { status } => json!({ "status": status }),
            Self::ContextTooLarge { chars, limit } => json!({ "chars": chars, "limit": limit }),
            Self::Gateway { data, .. } => data.clone(),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("data", &self.data())?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_and_its_data_cross_the_bridge() {
        let error = AppError::WorkFolderIsProtected {
            path: "D:\\Documents".into(),
        };

        let payload = serde_json::to_value(&error).expect("serialises");

        assert_eq!(payload["code"], "work_folder_is_protected");
        assert_eq!(payload["data"]["path"], "D:\\Documents");
    }

    #[test]
    fn a_gateway_code_is_not_rewritten() {
        let error = AppError::Gateway {
            code: "model_alias_not_allowed".into(),
            data: json!({ "requested": "gpt-4o" }),
        };

        let payload = serde_json::to_value(&error).expect("serialises");

        assert_eq!(payload["code"], "model_alias_not_allowed");
        assert_eq!(payload["data"]["requested"], "gpt-4o");
    }

    #[test]
    fn a_refused_sync_folder_names_the_product_without_translating_it() {
        let error = AppError::WorkFolderIsCloudSynced {
            path: "C:\\Users\\practice\\OneDrive\\work".into(),
            service: "OneDrive".into(),
        };

        let payload = serde_json::to_value(&error).expect("serialises");

        assert_eq!(payload["code"], "work_folder_is_cloud_synced");
        assert_eq!(payload["data"]["service"], "OneDrive");
    }

    #[test]
    fn no_variant_carries_prose() {
        // The display form is the code itself, so a message cannot become a sentence by accident.
        let error = AppError::ContextTooLarge {
            chars: 30_000,
            limit: 24_000,
        };

        assert_eq!(error.to_string(), error.code());
    }
}
