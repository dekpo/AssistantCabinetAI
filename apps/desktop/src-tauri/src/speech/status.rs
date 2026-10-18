//! What each speech feature can do right now, as machine codes.
//!
//! [`compute`] is a pure function of the settings and of what has been found on this machine
//! ([`Probes`]). In this phase nothing is probed, so every probe says "absent"; later phases fill
//! them in from the workers and the models. The command that exposes it is a thin shell
//! (`speech_commands::speech_status`).

use serde::Serialize;

use super::languages::{self, Feature, LanguageInfo};
use super::settings::SpeechSettings;
use crate::error::AppError;
use crate::settings::{Settings, DEFAULT_LOCALE};

/// The state of a model on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModelState {
    #[default]
    Missing,
    Present,
    /// Present but not matching its pinned SHA-256.
    Corrupt,
}

/// What was found on this machine. The default is "nothing found", which is also the truth until a
/// later phase adds a probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Probes {
    pub stt_engine: bool,
    pub stt_model: ModelState,
    pub tts_voice: bool,
}

/// Whether one feature can run, and if not the machine code of the first reason. The code is the
/// `code()` of an `AppError`, so the interface already has a sentence for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureStatus {
    pub available: bool,
    pub reason: Option<String>,
}

impl FeatureStatus {
    fn ready() -> Self {
        Self {
            available: true,
            reason: None,
        }
    }

    fn blocked(error: &AppError) -> Self {
        Self {
            available: false,
            reason: Some(error.code().to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    pub dictation: FeatureStatus,
    pub read_aloud: FeatureStatus,
    pub notes: FeatureStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechStatus {
    /// The voice master switch.
    pub enabled: bool,
    /// The locale speech works in: the override if set, else the interface locale.
    pub locale: String,
    pub features: Features,
    /// The language table, so the override list comes from Rust.
    pub languages: Vec<LanguageInfo>,
}

/// The locale speech works in: the override, else the interface locale, else the default.
pub fn effective_locale(settings: &Settings) -> String {
    settings
        .speech
        .language_override
        .clone()
        .or_else(|| settings.locale.clone())
        .unwrap_or_else(|| DEFAULT_LOCALE.to_string())
}

pub fn compute(settings: &Settings, probes: &Probes) -> SpeechStatus {
    let speech = &settings.speech;
    let locale = effective_locale(settings);
    SpeechStatus {
        enabled: speech.enabled,
        features: Features {
            dictation: dictation(speech, &locale, probes),
            read_aloud: read_aloud(speech, &locale, probes),
            notes: notes(settings),
        },
        locale,
        languages: languages::table(),
    }
}

fn dictation(speech: &SpeechSettings, locale: &str, probes: &Probes) -> FeatureStatus {
    if !speech.enabled || !speech.dictation_enabled {
        return FeatureStatus::blocked(&AppError::SpeechUnavailable);
    }
    if let Err(error) = languages::resolve(locale, Feature::Dictation) {
        return FeatureStatus::blocked(&error);
    }
    if !probes.stt_engine {
        return FeatureStatus::blocked(&AppError::SpeechEngineMissing);
    }
    match probes.stt_model {
        ModelState::Present => FeatureStatus::ready(),
        ModelState::Missing => FeatureStatus::blocked(&AppError::SpeechModelMissing),
        ModelState::Corrupt => FeatureStatus::blocked(&AppError::SpeechModelCorrupt),
    }
}

fn read_aloud(speech: &SpeechSettings, locale: &str, probes: &Probes) -> FeatureStatus {
    if !speech.enabled || !speech.read_aloud_enabled {
        return FeatureStatus::blocked(&AppError::SpeechUnavailable);
    }
    if let Err(error) = languages::resolve(locale, Feature::ReadAloud) {
        return FeatureStatus::blocked(&error);
    }
    if probes.tts_voice {
        FeatureStatus::ready()
    } else {
        FeatureStatus::blocked(&AppError::TtsNoLocalVoice)
    }
}

/// Notes need no engine and no language: typing a note works without voice. They need the
/// documents folder they are saved in, which is a setting, so nothing is read from the disk here.
fn notes(settings: &Settings) -> FeatureStatus {
    if !settings.speech.notes_enabled {
        return FeatureStatus::blocked(&AppError::SpeechUnavailable);
    }
    let folder_chosen = settings
        .work_folder
        .as_deref()
        .is_some_and(|folder| !folder.trim().is_empty());
    if folder_chosen {
        FeatureStatus::ready()
    } else {
        FeatureStatus::blocked(&AppError::NotesFolderUnavailable)
    }
}
