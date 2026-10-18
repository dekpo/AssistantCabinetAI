//! The speech-to-text service: everything a transcription needs that is not the engine itself.
//!
//! [`SpeechService`] owns the provider (none until a real engine exists), the one-job-at-a-time
//! flag, the cancel token of the running job and the time limits. The Tauri commands are thin
//! shells over it, and the tests drive it with the fake provider in temporary folders: nothing
//! here builds a `State` or an `AppHandle`.
//!
//! One job, in this order: switches and language -> provider and its availability -> the busy flag
//! (taken before any file exists) -> the lazy sweep of the temporary folder -> the clip is parsed
//! and judged from its samples -> the clip is written to the private temporary folder -> the engine
//! runs off the calling thread under a time limit -> the output guard -> the clip is removed, always.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::languages::{self, Feature};
use super::output_guard;
use super::status;
use super::stt::{CancelToken, SpeechToTextProvider, SttRequest, SttResult};
use super::temp_audio;
use super::wav::{self, WavClip};
use crate::error::AppError;
use crate::settings::Settings;

/// Name of the request header that says what the clip is for. The body carries the audio and
/// nothing else, so the use travels beside it; the webview never sends a number of seconds.
pub const PURPOSE_HEADER: &str = "purpose";

/// Shortest time an engine is given, in seconds, whatever the clip length: loading a model and
/// warming up cost the same for one second of audio as for ten.
pub const TIMEOUT_FLOOR_SECONDS: u64 = 30;
/// Time an engine is given per second of audio, on top of the floor. 1.0 means real time, which
/// leaves the slowest supported machine room to be several times slower than SpeechLab measured.
pub const TIMEOUT_PER_AUDIO_SECOND: f32 = 1.0;

/// What a clip is for. It decides the cap on its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Dictation,
    Note,
}

impl Purpose {
    /// The value of the [`PURPOSE_HEADER`] header. Absent, or anything other than `note`, is a
    /// chat dictation: the shorter cap is the safe default.
    pub fn from_header(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("note") => Self::Note,
            _ => Self::Dictation,
        }
    }
}

/// What the settings and the language table decided for one job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The locale speech works in, as the table spells it (`fr-FR`).
    pub locale: String,
    /// The language code the engine is given.
    pub language: String,
    /// The longest clip this use accepts, in seconds.
    pub cap_seconds: u32,
}

/// Decides whether a transcription may start and with which language and cap. Pure: it reads the
/// settings it is given and nothing else.
pub fn plan(settings: &Settings, purpose: Purpose) -> Result<Plan, AppError> {
    let speech = &settings.speech;
    if !speech.enabled || !speech.dictation_enabled {
        return Err(AppError::SpeechUnavailable);
    }
    let row = languages::resolve(&status::effective_locale(settings), Feature::Dictation)?;
    let cap_seconds = match purpose {
        Purpose::Dictation => speech.max_dictation_seconds,
        Purpose::Note => speech.max_note_seconds,
    };
    Ok(Plan {
        locale: row.id.to_string(),
        language: row.engine_code.to_string(),
        cap_seconds,
    })
}

/// How long an engine may take.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub timeout_floor: Duration,
    pub timeout_per_audio_second: f32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout_floor: Duration::from_secs(TIMEOUT_FLOOR_SECONDS),
            timeout_per_audio_second: TIMEOUT_PER_AUDIO_SECOND,
        }
    }
}

impl Limits {
    /// The time limit for a clip of this length: proportional to it, never under the floor.
    pub fn timeout_for(&self, audio_ms: u64) -> Duration {
        self.timeout_floor
            + Duration::from_secs_f32(audio_ms as f32 / 1_000.0 * self.timeout_per_audio_second)
    }
}

/// Holds the "a job is running" flag and its cancel token. Dropping it releases both, so an error,
/// a cancel or a panic cannot leave the service busy.
struct BusyGuard {
    busy: Arc<AtomicBool>,
    current: Arc<Mutex<Option<CancelToken>>>,
    token: CancelToken,
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        if let Ok(mut current) = self.current.lock() {
            *current = None;
        }
        self.busy.store(false, Ordering::SeqCst);
    }
}

pub struct SpeechService {
    provider: Option<Arc<dyn SpeechToTextProvider>>,
    busy: Arc<AtomicBool>,
    current: Arc<Mutex<Option<CancelToken>>>,
    swept: AtomicBool,
    limits: Limits,
}

impl Default for SpeechService {
    /// No provider: answers `speech_engine_missing`. The application builds its service with the worker.
    fn default() -> Self {
        Self::new(None, Limits::default())
    }
}

impl SpeechService {
    /// A service over a real engine, with the default time limits.
    pub fn with_provider(provider: Arc<dyn SpeechToTextProvider>) -> Self {
        Self::new(Some(provider), Limits::default())
    }

    pub fn new(provider: Option<Arc<dyn SpeechToTextProvider>>, limits: Limits) -> Self {
        Self {
            provider,
            busy: Arc::new(AtomicBool::new(false)),
            current: Arc::new(Mutex::new(None)),
            swept: AtomicBool::new(false),
            limits,
        }
    }

    /// Takes the one-job flag, or answers `speech_busy`.
    fn begin(&self) -> Result<BusyGuard, AppError> {
        if self
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(AppError::SpeechBusy);
        }
        let token = CancelToken::new();
        let guard = BusyGuard {
            busy: Arc::clone(&self.busy),
            current: Arc::clone(&self.current),
            token: token.clone(),
        };
        match self.current.lock() {
            Ok(mut current) => *current = Some(token),
            Err(_) => return Err(AppError::Internal),
        }
        Ok(guard)
    }

    /// Whether a job is running.
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    /// Stops the running job, if any. Returns whether there was one. Safe to call at any time.
    pub fn cancel(&self) -> bool {
        match self.current.lock() {
            Ok(current) => match current.as_ref() {
                Some(token) => {
                    token.cancel();
                    true
                }
                None => false,
            },
            Err(_) => false,
        }
    }

    /// Clears the clips a crash left behind, once per run of the application, at the first speech
    /// command. Called with the busy flag held, so it can never remove a live clip.
    fn sweep_once(&self, directory: &Path) {
        if !self.swept.swap(true, Ordering::SeqCst) {
            temp_audio::sweep(directory);
        }
    }

    /// Transcribes one clip. `wav` is the raw body the webview sent, `temp_dir` the private folder
    /// for the clip. The engine runs on a blocking thread so the interface never waits on it.
    pub async fn transcribe(
        &self,
        wav: &[u8],
        plan: &Plan,
        temp_dir: &Path,
    ) -> Result<SttResult, AppError> {
        let provider = self.provider.clone().ok_or(AppError::SpeechEngineMissing)?;
        provider.availability(&plan.locale)?;

        let guard = self.begin()?;
        self.sweep_once(temp_dir);

        let clip = wav::parse(wav)?;
        wav::check_usable(&clip, plan.cap_seconds)?;
        let file = temp_audio::write(temp_dir, wav)?;

        let request = SttRequest {
            wav_path: file.path().to_path_buf(),
            language: plan.language.clone(),
            max_seconds: plan.cap_seconds,
        };
        let timeout = self.limits.timeout_for(clip.duration_ms);
        let on_timeout = guard.token.clone();
        // The guard and the clip file move into the job: they are released when the engine really
        // stops, which is after a timeout too, so a hung engine keeps the service busy rather than
        // letting a second job run beside it.
        let job = move || {
            let guard = guard;
            let _file = file;
            run(provider.as_ref(), &request, &guard.token, &clip)
        };
        let handle = tauri::async_runtime::spawn_blocking(job);
        match tokio::time::timeout(timeout, handle).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(_panicked)) => Err(AppError::Internal),
            Err(_elapsed) => {
                on_timeout.cancel();
                Err(AppError::SpeechTimeout)
            }
        }
    }
}

/// The blocking part of a job: the engine call and the checks on what it returned.
fn run(
    provider: &dyn SpeechToTextProvider,
    request: &SttRequest,
    cancel: &CancelToken,
    clip: &WavClip,
) -> Result<SttResult, AppError> {
    if cancel.is_cancelled() {
        return Err(AppError::SpeechCancelled);
    }
    let mut result = provider.transcribe(request, cancel)?;
    if cancel.is_cancelled() {
        return Err(AppError::SpeechCancelled);
    }
    output_guard::check(&result.text, clip.duration_ms, clip.is_quiet())?;
    result.text = result.text.trim().to_string();
    Ok(result)
}

/// The one line the module writes about a job: which engine, how long the clip, how long the work,
/// how many words. Never the words themselves.
pub fn timing_line(result: &SttResult) -> String {
    format!(
        "speech: engine={} audio_ms={} processing_ms={} words={}",
        result.engine_id,
        result.audio_ms,
        result.processing_ms,
        output_guard::word_count(&result.text)
    )
}
