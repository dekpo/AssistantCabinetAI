//! A deterministic speech-to-text provider for tests: scripted text, scripted failures, scripted
//! delays. It reads no model and hears nothing. Only the tests build it (the module is declared
//! under `#[cfg(test)]`, and a test scans the commands file to prove no command constructs it), so
//! the application without a real engine answers `speech_engine_missing`.

use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::stt::{CancelToken, SpeechToTextProvider, SttRequest, SttResult};
use super::wav::{HEADER_BYTES, SAMPLE_RATE_HZ};
use crate::error::AppError;

/// The engine id the fake reports.
pub const FAKE_ENGINE_ID: &str = "fake";
/// How often a delayed fake looks at the cancel token, in milliseconds.
const CANCEL_POLL_MS: u64 = 2;

/// What the fake does for one call.
#[derive(Debug, Clone)]
pub enum Step {
    /// Answers with this text.
    Text(String),
    /// Fails with `speech_engine_failed`.
    Fail,
    /// Panics, to prove a panic cannot leave the service busy.
    Panic,
}

/// What the fake saw of one call, so a test can check the request the service built.
#[derive(Debug, Clone)]
pub struct SeenCall {
    pub request: SttRequest,
    /// Whether the clip file existed on disk while the engine ran.
    pub wav_existed: bool,
}

pub struct FakeProvider {
    script: Mutex<VecDeque<Step>>,
    fallback: Step,
    delay: Duration,
    availability: Option<fn() -> AppError>,
    seen: Mutex<Vec<SeenCall>>,
}

impl FakeProvider {
    /// A fake that answers every call with `text`.
    pub fn saying(text: &str) -> Self {
        Self {
            script: Mutex::new(VecDeque::new()),
            fallback: Step::Text(text.to_string()),
            delay: Duration::ZERO,
            availability: None,
            seen: Mutex::new(Vec::new()),
        }
    }

    /// Queues steps used before the fallback, one per call.
    pub fn then(self, steps: impl IntoIterator<Item = Step>) -> Self {
        self.script.lock().expect("script lock").extend(steps);
        self
    }

    /// Every call takes this long, unless cancelled first.
    pub fn after(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// `availability` answers this error instead of `Ok`.
    pub fn unavailable(mut self, error: fn() -> AppError) -> Self {
        self.availability = Some(error);
        self
    }

    pub fn seen(&self) -> Vec<SeenCall> {
        self.seen.lock().expect("seen lock").clone()
    }

    pub fn calls(&self) -> usize {
        self.seen.lock().expect("seen lock").len()
    }
}

impl SpeechToTextProvider for FakeProvider {
    fn engine_id(&self) -> &str {
        FAKE_ENGINE_ID
    }

    fn availability(&self, _locale: &str) -> Result<(), AppError> {
        match self.availability {
            Some(error) => Err(error()),
            None => Ok(()),
        }
    }

    fn transcribe(
        &self,
        request: &SttRequest,
        cancel: &CancelToken,
    ) -> Result<SttResult, AppError> {
        let started = Instant::now();
        let wav_path: PathBuf = request.wav_path.clone();
        let wav_existed = wav_path.is_file();
        let clip_bytes = fs::metadata(&wav_path).map(|meta| meta.len()).unwrap_or(0) as usize;
        self.seen.lock().expect("seen lock").push(SeenCall {
            request: request.clone(),
            wav_existed,
        });

        while started.elapsed() < self.delay {
            if cancel.is_cancelled() {
                return Err(AppError::SpeechCancelled);
            }
            std::thread::sleep(Duration::from_millis(CANCEL_POLL_MS));
        }
        if cancel.is_cancelled() {
            return Err(AppError::SpeechCancelled);
        }

        let step = self
            .script
            .lock()
            .expect("script lock")
            .pop_front()
            .unwrap_or_else(|| self.fallback.clone());
        match step {
            Step::Text(text) => Ok(SttResult {
                text,
                audio_ms: (clip_bytes.saturating_sub(HEADER_BYTES) / 2) as u64 * 1_000
                    / u64::from(SAMPLE_RATE_HZ),
                processing_ms: started.elapsed().as_millis() as u64,
                engine_id: FAKE_ENGINE_ID.to_string(),
            }),
            Step::Fail => Err(AppError::SpeechEngineFailed),
            Step::Panic => panic!("scripted panic of the fake provider"),
        }
    }
}
