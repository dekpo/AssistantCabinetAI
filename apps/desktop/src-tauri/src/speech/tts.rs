//! The text-to-speech contract and the pure rules around it: which voice reads a language, what
//! speed the engine is asked for, and what the settings are told about the installed voices.
//!
//! No file is written, no process started and no network used here: the worker client is
//! [`super::worker_tts`], the installer is [`super::model_download`]. The voices themselves are rows
//! of the models manifest; the language table says which of them a language offers.

use std::path::Path;

use serde::Serialize;

use super::languages::{self, DefaultVoice, Feature};
use super::models::{self, LicenceRating, ModelSpec};
use super::status::ModelState;
use super::{DEFAULT_TTS_RATE, MAX_TTS_RATE, MIN_TTS_RATE};
use crate::error::AppError;

/// Longest text one synthesis accepts, in characters. The interface sends a sentence at a time, so
/// this only stops a stray request from tying the worker up.
pub const MAX_TEXT_CHARS: usize = 5_000;

/// The reading speed as the percentage the worker takes: 100 is the voice's natural rate. A value
/// that is not a number is the natural rate; anything else is held between the slowest and the
/// fastest speed of the settings.
pub fn speed_percent(rate: f32) -> u32 {
    let rate = if rate.is_finite() {
        rate.clamp(MIN_TTS_RATE, MAX_TTS_RATE)
    } else {
        DEFAULT_TTS_RATE
    };
    (rate * 100.0).round() as u32
}

/// One voice of the manifest, as the settings are told about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceModelStatus {
    /// The manifest id, which is also the voice id of the language table and of `ttsVoice`.
    pub id: String,
    pub state: ModelState,
    pub rating: LicenceRating,
    /// Size of the download, in bytes, so the interface can say what Install fetches.
    pub bytes: u64,
}

/// The state of every voice of the manifest under `models_root`. A few `stat` calls per voice: no
/// hashing and no process.
pub fn voice_statuses(models_root: &Path) -> Vec<VoiceModelStatus> {
    models::voice_models()
        .into_iter()
        .filter_map(|spec| {
            Some(VoiceModelStatus {
                id: spec.id.clone(),
                state: models::state(spec, &spec.folder(models_root)),
                rating: spec.voice.as_ref()?.rating,
                bytes: spec.archive.bytes,
            })
        })
        .collect()
}

/// Whether a voice of the language row of `locale` is installed and checked.
pub fn any_voice_ready(locale: &str, statuses: &[VoiceModelStatus]) -> bool {
    languages::resolve(locale, Feature::ReadAloud).is_ok_and(|row| {
        row.voices.iter().any(|voice| {
            statuses
                .iter()
                .any(|status| status.id == voice.id && status.state == ModelState::Present)
        })
    })
}

/// The voice that reads `locale`: the one the user chose if the language offers it and it is
/// installed, else the first installed voice of the language, in the order of the language table.
/// `tts_no_local_voice` when the language has none installed, `speech_model_corrupt` when one is
/// there but damaged, `speech_locale_unsupported` for a language without read-aloud.
pub fn choose_voice(
    locale: &str,
    preferred: Option<&str>,
    state_of: impl Fn(&ModelSpec) -> ModelState,
) -> Result<&'static ModelSpec, AppError> {
    let row = languages::resolve(locale, Feature::ReadAloud)?;
    let chosen = preferred.and_then(|wanted| row.voices.iter().find(|voice| voice.id == wanted));
    let ordered: Vec<&DefaultVoice> = chosen
        .into_iter()
        .chain(
            row.voices
                .iter()
                .filter(|voice| Some(voice.id) != chosen.map(|c| c.id)),
        )
        .collect();
    let mut damaged = false;
    for voice in ordered {
        let Some(spec) = models::find(voice.id).filter(|spec| spec.kind == "tts") else {
            continue;
        };
        match state_of(spec) {
            ModelState::Present => return Ok(spec),
            ModelState::Corrupt => damaged = true,
            ModelState::Missing | ModelState::Unverified => {}
        }
    }
    Err(if damaged {
        AppError::SpeechModelCorrupt
    } else {
        AppError::TtsNoLocalVoice
    })
}

/// What a synthesis returned to the interface. The audio itself travels as the raw body of the
/// answer; these numbers travel in a header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Synthesis {
    /// A canonical 16-bit mono WAV.
    pub wav: Vec<u8>,
    pub voice_id: String,
    pub audio_ms: u64,
    pub processing_ms: u64,
}
