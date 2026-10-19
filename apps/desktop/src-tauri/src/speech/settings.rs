//! The nested `speech` field of `settings.json` (`docs/SPEECH.md` section 4).
//!
//! Every value is clamped by [`SpeechSettings::sanitised`], on the way in (`load`) and on the way
//! out (`save`), so a hand-edited file or a stale webview can neither switch voice on by accident
//! nor hand a worker a value it cannot use. Nothing here reads the disk or the network.

use serde::{Deserialize, Serialize};

use super::{
    DEFAULT_TTS_RATE, MAX_DICTATION_SECONDS, MAX_LOCALE_CHARS, MAX_NOTE_SECONDS, MAX_TTS_RATE,
    MAX_SYSTEM_VOICE_ID_CHARS, MAX_VOICE_ID_CHARS, MIN_DICTATION_SECONDS, MIN_NOTE_SECONDS,
    MIN_TTS_RATE,
};

/// Cap of one chat dictation until the user changes it, in seconds.
pub const DEFAULT_MAX_DICTATION_SECONDS: u32 = 120;
/// Cap of one note dictation until the user changes it, in seconds (30 minutes).
pub const DEFAULT_MAX_NOTE_SECONDS: u32 = 1_800;
/// The subfolder of the documents folder that holds saved notes, until the user names another.
pub const DEFAULT_NOTES_FOLDER_NAME: &str = "Notes";
/// Where the speech models are downloaded from, on an explicit user action (decision D5).
pub const DEFAULT_SPEECH_MODELS_URL: &str = "https://assistantcabinet.ai/models/speech/";

/// The engine ids `ttsEngine` may hold. `null` means automatic: the local synthesiser worker if it
/// is installed, else the system voices only if they are proven local.
pub const TTS_ENGINE_PIPER: &str = "piper";
pub const TTS_ENGINE_SYSTEM: &str = "system";
const TTS_ENGINES: [&str; 2] = [TTS_ENGINE_PIPER, TTS_ENGINE_SYSTEM];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpeechSettings {
    /// The voice master switch: dictation and read-aloud only exist while it is on. Off by default.
    pub enabled: bool,
    /// Counts only once `enabled` is on.
    pub dictation_enabled: bool,
    /// Counts only once `enabled` is on.
    pub read_aloud_enabled: bool,
    /// The Notes button and section. Independent of `enabled`: typing a note needs no voice.
    pub notes_enabled: bool,
    /// `None` follows the interface locale.
    pub language_override: Option<String>,
    /// Cap of one chat dictation, in seconds.
    pub max_dictation_seconds: u32,
    /// Cap of one note dictation, in seconds.
    pub max_note_seconds: u32,
    /// `None` is automatic; otherwise one of the `TTS_ENGINE_*` ids.
    pub tts_engine: Option<String>,
    /// A voice manifest id, or `None` for the first voice of the resolved locale.
    pub tts_voice: Option<String>,
    /// The operating-system voice used when the engine is `system`: the id the system gives it, or
    /// `None` for the first local voice of the language. Separate from `tts_voice`, whose ids are
    /// manifest slugs; this one only ever goes back to the window's own voice list.
    pub tts_system_voice: Option<String>,
    /// Read-aloud speed as a multiple of the voice's natural rate.
    pub tts_rate: f32,
    /// One clean folder name under the documents folder, never a path.
    pub notes_folder_name: String,
    /// Spoken punctuation, title, subtitle, new line, new paragraph and new note.
    pub notes_voice_commands: bool,
    /// Root of the explicit model download. A setting, never a constant.
    pub speech_models_url: String,
}

impl Default for SpeechSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            dictation_enabled: true,
            read_aloud_enabled: true,
            notes_enabled: true,
            language_override: None,
            max_dictation_seconds: DEFAULT_MAX_DICTATION_SECONDS,
            max_note_seconds: DEFAULT_MAX_NOTE_SECONDS,
            tts_engine: None,
            tts_voice: None,
            tts_system_voice: None,
            tts_rate: DEFAULT_TTS_RATE,
            notes_folder_name: DEFAULT_NOTES_FOLDER_NAME.to_string(),
            notes_voice_commands: true,
            speech_models_url: DEFAULT_SPEECH_MODELS_URL.to_string(),
        }
    }
}

impl SpeechSettings {
    /// Every value brought back into what the code can use. Clamped or replaced, never refused: a
    /// bad value must not make the whole settings file unreadable.
    pub fn sanitised(&self) -> Self {
        Self {
            enabled: self.enabled,
            dictation_enabled: self.dictation_enabled,
            read_aloud_enabled: self.read_aloud_enabled,
            notes_enabled: self.notes_enabled,
            language_override: self.language_override.as_deref().and_then(clean_locale),
            max_dictation_seconds: self
                .max_dictation_seconds
                .clamp(MIN_DICTATION_SECONDS, MAX_DICTATION_SECONDS),
            max_note_seconds: self
                .max_note_seconds
                .clamp(MIN_NOTE_SECONDS, MAX_NOTE_SECONDS),
            tts_engine: self
                .tts_engine
                .as_deref()
                .map(str::trim)
                .filter(|engine| TTS_ENGINES.contains(engine))
                .map(str::to_string),
            tts_voice: self.tts_voice.as_deref().and_then(clean_voice_id),
            tts_system_voice: self.tts_system_voice.as_deref().and_then(clean_system_voice_id),
            tts_rate: if self.tts_rate.is_finite() {
                self.tts_rate.clamp(MIN_TTS_RATE, MAX_TTS_RATE)
            } else {
                DEFAULT_TTS_RATE
            },
            notes_folder_name: crate::settings::clean_folder_name_or(
                &self.notes_folder_name,
                DEFAULT_NOTES_FOLDER_NAME,
            ),
            notes_voice_commands: self.notes_voice_commands,
            speech_models_url: clean_models_url(&self.speech_models_url),
        }
    }
}

/// A locale tag as it is kept: short, letters, digits and hyphens. Whether the language is
/// supported is not decided here but by the language table, which answers `speech_locale_unsupported`.
fn clean_locale(typed: &str) -> Option<String> {
    let tag = typed.trim();
    let usable = !tag.is_empty()
        && tag.chars().count() <= MAX_LOCALE_CHARS
        && tag
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    usable.then(|| tag.to_string())
}

/// A manifest id: a short ASCII slug. Anything else reads as "the first voice of the locale".
fn clean_voice_id(typed: &str) -> Option<String> {
    let id = typed.trim();
    let usable = !id.is_empty()
        && id.chars().count() <= MAX_VOICE_ID_CHARS
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'));
    usable.then(|| id.to_string())
}

/// The id of an operating-system voice: what the system calls it, trimmed. It is kept whole when it
/// is short and free of control characters; anything else reads as "the first local voice".
fn clean_system_voice_id(typed: &str) -> Option<String> {
    let id = typed.trim();
    let usable = !id.is_empty()
        && id.chars().count() <= MAX_SYSTEM_VOICE_ID_CHARS
        && !id.chars().any(char::is_control);
    usable.then(|| id.to_string())
}

/// An address shaped like a web root; anything else falls back to the default, so the download
/// button can never be pointed at a path or a script.
fn clean_models_url(typed: &str) -> String {
    let url = typed.trim();
    let shaped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .is_some_and(|rest| !rest.is_empty() && !rest.chars().any(char::is_whitespace));
    if shaped {
        url.to_string()
    } else {
        DEFAULT_SPEECH_MODELS_URL.to_string()
    }
}
