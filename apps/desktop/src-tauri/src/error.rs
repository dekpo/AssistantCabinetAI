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

    /// The Data Folder equivalent, reported separately from `WorkFolderNoLongerAllowed` so the
    /// interface names the folder that actually stopped passing the rules.
    #[error("data_folder_no_longer_allowed")]
    DataFolderNoLongerAllowed,

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

    /// The viewer asked for a file that is no longer there: renamed, moved or deleted outside the
    /// application since the last listing. Nothing was read or changed.
    #[error("viewer_file_missing")]
    ViewerFileMissing,

    /// The viewer asked for something that is not a plain file of the folder it names: a path
    /// that could lead elsewhere, a folder, a link, or a file the system would not open.
    #[error("viewer_file_unreadable")]
    ViewerFileUnreadable,

    /// The file is larger than the viewer's memory budget (`viewer_source::MAX_VIEWER_BYTES`).
    #[error("viewer_file_too_large")]
    ViewerFileTooLarge,

    #[error("server_unreachable")]
    ServerUnreachable { url: String },

    #[error("server_timeout")]
    ServerTimeout { url: String },

    /// One embeddings request ran past its deadline, twice. Distinct from `ServerTimeout`, which
    /// is a chat that went silent: its sentence points at a waiting time in the settings that does
    /// not govern indexing at all.
    #[error("embedding_timeout")]
    EmbeddingTimeout,

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

    /// The Data Folder equivalent of `NoWorkFolderSet`: distinct from it because the two folders
    /// are chosen independently, and the interface has to tell a missing Data Folder apart from a
    /// missing Documents Folder to say the right sentence.
    #[error("no_data_folder_set")]
    NoDataFolderSet,

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

    /// The files she chose for this conversation are gone or have changed since, so the scope
    /// holds nothing to answer from. Distinct from `InsufficientEvidence`: the documents were not
    /// searched and found wanting, the ones she named could not be used, and the fix is to choose
    /// them again.
    #[error("scope_unavailable")]
    ScopeUnavailable,

    /// Documents and tables are both selected for this conversation. Answering from both at once
    /// is not designed yet (`docs/SELECTION-AND-MEMORY.md`, "documents and tables together"), and
    /// quietly picking one side would be worse than saying so: she unticks one list, or the other.
    #[error("documents_and_tables_together")]
    DocumentsAndTablesTogether,

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

    /// The template of a mail merge could not be read as a letter (HAP-1, lot E).
    #[error("fill_template_unreadable")]
    FillTemplateUnreadable { path: String },

    /// The template or the table the plan was made from is no longer there, no longer analysed, or
    /// no longer readable.
    #[error("fill_source_unavailable")]
    FillSourceUnavailable,

    /// The table has no row to make a letter from.
    #[error("fill_nothing_to_fill")]
    FillNothingToFill,

    /// One approval would write more letters than the limit.
    #[error("fill_too_many")]
    FillTooMany { count: usize, limit: usize },

    /// The drafted template of a mail merge is larger than the limit (TABULAR step 09).
    #[error("fill_draft_too_large")]
    FillDraftTooLarge { limit: usize },

    /// A letter could not be written; the ones already written stay, the template and the data are
    /// untouched.
    #[error("fill_write_failed")]
    FillWriteFailed,

    /// The knowledge base could not be read or written (a locked or damaged database). Whatever
    /// asked falls back to the path that does not use it (`docs/SESSION-KB-00-master.md`, I4).
    #[error("knowledge_unavailable")]
    KnowledgeUnavailable,

    /// An operation named an entity the knowledge base does not hold.
    #[error("knowledge_entity_not_found")]
    KnowledgeEntityNotFound,

    /// A merge, an unmerge or a deletion that would leave the entities inconsistent: two types, a
    /// cycle, an entity that is already merged. Nothing was changed.
    #[error("knowledge_merge_refused")]
    KnowledgeMergeRefused,

    /// A name typed in the management dialog that cannot be kept: empty once cleaned, too long, or
    /// made only of titles. Nothing was changed.
    #[error("knowledge_name_invalid")]
    KnowledgeNameInvalid,

    /// The name typed already belongs to another entity of the same kind. Nothing was changed.
    #[error("knowledge_name_taken")]
    KnowledgeNameTaken,

    /// A manual operation the knowledge base cannot do on that item (an automatic alias to remove,
    /// a split that would leave nothing behind, a personal identifier typed by hand). Nothing was
    /// changed.
    #[error("knowledge_operation_refused")]
    KnowledgeOperationRefused,

    /// "Undo" was asked and the last operation cannot be undone, or there is none.
    #[error("knowledge_nothing_to_undo")]
    KnowledgeNothingToUndo,

    /// A lexicon pack bundled with the application does not validate. `pack` is its id and `path`
    /// the place in the file (a JSON path, or a line and column for a syntax error). Both are
    /// ours, never document text. The knowledge base stays inactive; nothing else is affected.
    #[error("knowledge_pack_invalid")]
    KnowledgePackInvalid { pack: String, path: String },

    /// Speech is switched off, or this feature is. The reason is in `speech_status`.
    #[error("speech_unavailable")]
    SpeechUnavailable,

    /// The speech worker executable was not found.
    #[error("speech_engine_missing")]
    SpeechEngineMissing,

    /// The model files of the speech engine are not on this machine.
    #[error("speech_model_missing")]
    SpeechModelMissing,

    /// A model file does not match its pinned SHA-256.
    #[error("speech_model_corrupt")]
    SpeechModelCorrupt,

    /// The speech model could not be downloaded, or a file of it could not be written. `folder` is
    /// the model's folder on this machine, where the four files can be placed by hand.
    #[error("speech_model_download_failed")]
    SpeechModelDownloadFailed { folder: String },

    /// The recording is too short to hold a sentence. Refused before any engine runs.
    #[error("speech_audio_too_short")]
    SpeechAudioTooShort,

    /// The recording is too quiet to be speech. Refused before any engine runs.
    #[error("speech_audio_too_quiet")]
    SpeechAudioTooQuiet,

    /// The recording is longer than the cap of the setting.
    #[error("speech_audio_too_long")]
    SpeechAudioTooLong,

    /// The audio is not in the format the engine takes.
    #[error("speech_audio_invalid")]
    SpeechAudioInvalid,

    /// Another speech job is running. One at a time.
    #[error("speech_busy")]
    SpeechBusy,

    /// The user stopped the job. Not a failure.
    #[error("speech_cancelled")]
    SpeechCancelled,

    /// The engine took longer than its deadline and was stopped.
    #[error("speech_timeout")]
    SpeechTimeout,

    /// The worker failed. The typed application is unaffected.
    #[error("speech_engine_failed")]
    SpeechEngineFailed,

    /// The language is not in the speech language table, is only planned, or lacks the capability
    /// the feature needs. `locale` is a BCP 47 tag, not translated.
    #[error("speech_locale_unsupported")]
    SpeechLocaleUnsupported { locale: String },

    /// The engine output was rejected by the output guard (empty, runaway repetition, implausible
    /// speed). Nothing reaches the text box.
    #[error("speech_output_rejected")]
    SpeechOutputRejected,

    /// No voice that is proven local is available for read-aloud.
    #[error("tts_no_local_voice")]
    TtsNoLocalVoice,

    /// The synthesiser failed on this text.
    #[error("tts_failed")]
    TtsFailed,

    /// The folder notes are saved in cannot be used: no documents folder, or not writable.
    #[error("notes_folder_unavailable")]
    NotesFolderUnavailable,

    /// A note name that cannot be kept once cleaned.
    #[error("notes_name_invalid")]
    NotesNameInvalid,

    /// A note larger than the limit. Nothing was written.
    #[error("notes_too_large")]
    NotesTooLarge,

    /// More notes in one save than the limit. Nothing was written.
    #[error("notes_too_many")]
    NotesTooMany,

    /// A note could not be written. Nothing existing was overwritten.
    #[error("notes_write_failed")]
    NotesWriteFailed,

    /// A note was saved but could not be analysed. It stays in its folder.
    #[error("notes_analysis_failed")]
    NotesAnalysisFailed,
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
            Self::DataFolderNoLongerAllowed => "data_folder_no_longer_allowed",
            Self::WorkFolderIsSymlink { .. } => "work_folder_is_symlink",
            Self::WorkFolderUncNotSupported { .. } => "work_folder_unc_not_supported",
            Self::WorkFolderNotWritable { .. } => "work_folder_not_writable",
            Self::WorkFolderRevealFailed => "work_folder_reveal_failed",
            Self::ViewerFileMissing => "viewer_file_missing",
            Self::ViewerFileUnreadable => "viewer_file_unreadable",
            Self::ViewerFileTooLarge => "viewer_file_too_large",
            Self::ServerUnreachable { .. } => "server_unreachable",
            Self::ServerTimeout { .. } => "server_timeout",
            Self::EmbeddingTimeout => "embedding_timeout",
            Self::ServerError { .. } => "server_error",
            Self::ServerResponseInvalid => "server_response_invalid",
            Self::ContextTooLarge { .. } => "context_too_large",
            Self::Gateway { code, .. } => code,
            Self::NoWorkFolderSet => "no_work_folder_set",
            Self::NoDataFolderSet => "no_data_folder_set",
            Self::IndexUnavailable => "index_unavailable",
            Self::ExtractionEmpty { .. } => "extraction_empty",
            Self::ExtractionFailed { .. } => "extraction_failed",
            Self::InsufficientEvidence => "insufficient_evidence",
            Self::ScopeUnavailable => "scope_unavailable",
            Self::DocumentsAndTablesTogether => "documents_and_tables_together",
            Self::ChatCancelled => "chat_cancelled",
            Self::OcrUnavailable => "ocr_unavailable",
            Self::OcrLanguageUnavailable { .. } => "ocr_language_unavailable",
            Self::OcrFailed { .. } => "ocr_failed",
            Self::OcrPageUnreadable { .. } => "ocr_page_unreadable",
            Self::FillTemplateUnreadable { .. } => "fill_template_unreadable",
            Self::FillSourceUnavailable => "fill_source_unavailable",
            Self::FillNothingToFill => "fill_nothing_to_fill",
            Self::FillTooMany { .. } => "fill_too_many",
            Self::FillDraftTooLarge { .. } => "fill_draft_too_large",
            Self::FillWriteFailed => "fill_write_failed",
            Self::KnowledgeUnavailable => "knowledge_unavailable",
            Self::KnowledgeEntityNotFound => "knowledge_entity_not_found",
            Self::KnowledgeMergeRefused => "knowledge_merge_refused",
            Self::KnowledgeNameInvalid => "knowledge_name_invalid",
            Self::KnowledgeNameTaken => "knowledge_name_taken",
            Self::KnowledgeOperationRefused => "knowledge_operation_refused",
            Self::KnowledgeNothingToUndo => "knowledge_nothing_to_undo",
            Self::KnowledgePackInvalid { .. } => "knowledge_pack_invalid",
            Self::SpeechUnavailable => "speech_unavailable",
            Self::SpeechEngineMissing => "speech_engine_missing",
            Self::SpeechModelMissing => "speech_model_missing",
            Self::SpeechModelCorrupt => "speech_model_corrupt",
            Self::SpeechModelDownloadFailed { .. } => "speech_model_download_failed",
            Self::SpeechAudioTooShort => "speech_audio_too_short",
            Self::SpeechAudioTooQuiet => "speech_audio_too_quiet",
            Self::SpeechAudioTooLong => "speech_audio_too_long",
            Self::SpeechAudioInvalid => "speech_audio_invalid",
            Self::SpeechBusy => "speech_busy",
            Self::SpeechCancelled => "speech_cancelled",
            Self::SpeechTimeout => "speech_timeout",
            Self::SpeechEngineFailed => "speech_engine_failed",
            Self::SpeechLocaleUnsupported { .. } => "speech_locale_unsupported",
            Self::SpeechOutputRejected => "speech_output_rejected",
            Self::TtsNoLocalVoice => "tts_no_local_voice",
            Self::TtsFailed => "tts_failed",
            Self::NotesFolderUnavailable => "notes_folder_unavailable",
            Self::NotesNameInvalid => "notes_name_invalid",
            Self::NotesTooLarge => "notes_too_large",
            Self::NotesTooMany => "notes_too_many",
            Self::NotesWriteFailed => "notes_write_failed",
            Self::NotesAnalysisFailed => "notes_analysis_failed",
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
            | Self::DataFolderNoLongerAllowed
            | Self::ServerResponseInvalid
            | Self::EmbeddingTimeout
            | Self::NoWorkFolderSet
            | Self::NoDataFolderSet
            | Self::IndexUnavailable
            | Self::InsufficientEvidence
            | Self::ScopeUnavailable
            | Self::DocumentsAndTablesTogether
            | Self::ChatCancelled
            | Self::WorkFolderRevealFailed
            | Self::ViewerFileMissing
            | Self::ViewerFileUnreadable
            | Self::ViewerFileTooLarge
            | Self::FillSourceUnavailable
            | Self::FillNothingToFill
            | Self::FillWriteFailed
            | Self::KnowledgeUnavailable
            | Self::KnowledgeEntityNotFound
            | Self::KnowledgeMergeRefused
            | Self::KnowledgeNameInvalid
            | Self::KnowledgeNameTaken
            | Self::KnowledgeOperationRefused
            | Self::KnowledgeNothingToUndo
            | Self::OcrUnavailable
            | Self::SpeechUnavailable
            | Self::SpeechEngineMissing
            | Self::SpeechModelMissing
            | Self::SpeechModelCorrupt
            | Self::SpeechAudioTooShort
            | Self::SpeechAudioTooQuiet
            | Self::SpeechAudioTooLong
            | Self::SpeechAudioInvalid
            | Self::SpeechBusy
            | Self::SpeechCancelled
            | Self::SpeechTimeout
            | Self::SpeechEngineFailed
            | Self::SpeechOutputRejected
            | Self::TtsNoLocalVoice
            | Self::TtsFailed
            | Self::NotesFolderUnavailable
            | Self::NotesNameInvalid
            | Self::NotesTooLarge
            | Self::NotesTooMany
            | Self::NotesWriteFailed
            | Self::NotesAnalysisFailed => json!({}),
            Self::SpeechLocaleUnsupported { locale } => json!({ "locale": locale }),
            Self::SpeechModelDownloadFailed { folder } => json!({ "folder": folder }),
            Self::KnowledgePackInvalid { pack, path } => json!({ "pack": pack, "path": path }),
            Self::FillTemplateUnreadable { path } => json!({ "path": path }),
            Self::FillTooMany { count, limit } => json!({ "count": count, "limit": limit }),
            Self::FillDraftTooLarge { limit } => json!({ "limit": limit }),
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
    fn an_embedding_timeout_is_not_a_chat_timeout() {
        let embedding = serde_json::to_value(AppError::EmbeddingTimeout).expect("serialises");
        let chat = serde_json::to_value(AppError::ServerTimeout {
            url: "http://example.test".into(),
        })
        .expect("serialises");

        assert_eq!(embedding["code"], "embedding_timeout");
        assert_eq!(chat["code"], "server_timeout");
        assert_eq!(embedding["data"], json!({}));
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
