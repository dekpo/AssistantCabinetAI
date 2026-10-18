//! The speech-to-text contract (decision D2): what the application asks of an engine, whichever
//! engine it is. The real engine arrives in phase 3b as a separate worker executable behind this
//! trait; the tests use a scripted fake. Nothing here knows an engine, a model or a language.
//!
//! Types and one trait only: no file, no thread, no network.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;

use crate::error::AppError;

/// The speech module's own cancel flag: shared between the job that is running and the command that
/// stops it. Cloning shares the flag.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// One clip to transcribe. The language and the cap come from Rust settings and the language
/// table, never from the webview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SttRequest {
    /// The clip, a canonical WAV in the private temporary folder. The engine reads it and never
    /// keeps it: the service removes the file whatever the outcome.
    pub wav_path: PathBuf,
    /// The language code the engine is given, from the language table (for example `fr`).
    pub language: String,
    /// The longest clip this use accepts, in seconds. The service has already enforced it; an
    /// engine may use it to size its work.
    pub max_seconds: u32,
}

/// What an engine heard. The text goes to a text box and nowhere else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SttResult {
    pub text: String,
    /// Length of the clip, in milliseconds.
    pub audio_ms: u64,
    /// Time the engine spent, in milliseconds.
    pub processing_ms: u64,
    pub engine_id: String,
    /// Whether this engine writes capitals where a name has them (a property of the engine, from its
    /// manifest row), so that a name repair may trust a capital.
    pub case_reliable: bool,
}

/// A speech-to-text engine. Blocking: the service runs it off the interface thread, with a time
/// limit, and sets the cancel token when the user stops or the limit passes.
pub trait SpeechToTextProvider: Send + Sync {
    /// A short stable id for the timing line, for example `parakeet-tdt-0.6b-v3-int8`.
    fn engine_id(&self) -> &str;

    /// Whether this engine can transcribe this interface locale right now: `Ok` or the machine code
    /// of the first obstacle (`speech_model_missing`, `speech_model_corrupt`...).
    fn availability(&self, locale: &str) -> Result<(), AppError>;

    /// Transcribes one clip. Must return `speech_cancelled` soon after the token is set, and must
    /// not write the audio or the text anywhere.
    fn transcribe(&self, request: &SttRequest, cancel: &CancelToken)
        -> Result<SttResult, AppError>;
}
