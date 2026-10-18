//! The one table of languages (decision D6, `docs/SPEECH.md` section 5).
//!
//! interface locale -> engine language code -> rule pack names -> capabilities -> default voices.
//! No other file of the application names a language for speech: a caller resolves a locale
//! through [`resolve`] and reads what the row says. Adding German or Italian is a row here plus
//! data files, with no change of logic: its row exists already, as `planned`, and answers
//! `speech_locale_unsupported` until its capabilities are switched on.

use serde::Serialize;

use crate::error::AppError;

/// A speech feature that needs a language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    Dictation,
    ReadAloud,
}

/// Whether a language is shipped or only declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Support {
    Enabled,
    Planned,
}

/// The key of a default voice. A key, not a language test: a row lists the voices it ships by key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceGender {
    Female,
    Male,
    /// A voice that is not tied to one speaker.
    Mixed,
}

/// A default voice of a language: the manifest id of the voice model, by gender key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DefaultVoice {
    pub gender: VoiceGender,
    pub id: &'static str,
}

/// One language of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageRow {
    /// The interface locale this row answers to, as the client stores it (`settings.locale`).
    pub id: &'static str,
    /// The language code the speech engines are given.
    pub engine_code: &'static str,
    /// File names, without extension, of the rule packs under `resources/speech/rules/`.
    pub rule_packs: &'static [&'static str],
    pub support: Support,
    pub dictation: bool,
    pub read_aloud: bool,
    /// The voices read-aloud offers first, in order. The first one is the default.
    pub voices: &'static [DefaultVoice],
}

impl LanguageRow {
    pub fn supports(&self, feature: Feature) -> bool {
        self.support == Support::Enabled
            && match feature {
                Feature::Dictation => self.dictation,
                Feature::ReadAloud => self.read_aloud,
            }
    }
}

/// The table. Rows are in the order the interface lists them.
pub const LANGUAGES: &[LanguageRow] = &[
    LanguageRow {
        id: "fr-FR",
        engine_code: "fr",
        rule_packs: &["fr-FR"],
        support: Support::Enabled,
        dictation: true,
        read_aloud: true,
        voices: &[
            DefaultVoice {
                gender: VoiceGender::Female,
                id: "siwis",
            },
            DefaultVoice {
                gender: VoiceGender::Male,
                id: "css10",
            },
        ],
    },
    LanguageRow {
        id: "en-US",
        engine_code: "en",
        rule_packs: &["en-US"],
        support: Support::Enabled,
        dictation: true,
        read_aloud: true,
        voices: &[
            DefaultVoice {
                gender: VoiceGender::Female,
                id: "ljspeech",
            },
            DefaultVoice {
                gender: VoiceGender::Mixed,
                id: "libritts_r",
            },
        ],
    },
    // Declared so that the Swiss and Italian targets are anticipated; switched on in phase 15,
    // after the licence review of their models.
    LanguageRow {
        id: "de-DE",
        engine_code: "de",
        rule_packs: &["de-DE"],
        support: Support::Planned,
        dictation: false,
        read_aloud: false,
        voices: &[],
    },
    LanguageRow {
        id: "de-CH",
        engine_code: "de",
        rule_packs: &["de-CH"],
        support: Support::Planned,
        dictation: false,
        read_aloud: false,
        voices: &[],
    },
    LanguageRow {
        id: "it-IT",
        engine_code: "it",
        rule_packs: &["it-IT"],
        support: Support::Planned,
        dictation: false,
        read_aloud: false,
        voices: &[],
    },
];

/// The row of a locale, shipped or not. Case does not matter: `fr-fr` is `fr-FR`.
pub fn find(locale: &str) -> Option<&'static LanguageRow> {
    let wanted = locale.trim();
    LANGUAGES
        .iter()
        .find(|row| row.id.eq_ignore_ascii_case(wanted))
}

/// The row a feature can use for this locale, or `speech_locale_unsupported`: the locale is not in
/// the table, it is only planned, or its row lacks the capability the feature needs.
pub fn resolve(locale: &str, feature: Feature) -> Result<&'static LanguageRow, AppError> {
    match find(locale) {
        Some(row) if row.supports(feature) => Ok(row),
        _ => Err(AppError::SpeechLocaleUnsupported {
            locale: locale.trim().to_string(),
        }),
    }
}

/// What `speech_status` hands the interface for one row, so that the language override list comes
/// from Rust and not from a copy kept in the interface.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageInfo {
    pub id: &'static str,
    pub support: Support,
    pub dictation: bool,
    pub read_aloud: bool,
    pub voices: Vec<DefaultVoice>,
}

pub fn table() -> Vec<LanguageInfo> {
    LANGUAGES
        .iter()
        .map(|row| LanguageInfo {
            id: row.id,
            support: row.support,
            dictation: row.supports(Feature::Dictation),
            read_aloud: row.supports(Feature::ReadAloud),
            voices: row.voices.to_vec(),
        })
        .collect()
}
