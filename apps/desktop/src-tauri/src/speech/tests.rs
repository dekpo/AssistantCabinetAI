//! Tests of the speech foundations. They drive pure functions only: nothing here builds a Tauri
//! `State` or an `AppHandle`.

use std::fs;
use std::path::{Path, PathBuf};

use super::languages::{self, Feature, Support, VoiceGender, LANGUAGES};
use super::settings::{
    SpeechSettings, DEFAULT_MAX_DICTATION_SECONDS, DEFAULT_MAX_NOTE_SECONDS,
    DEFAULT_NOTES_FOLDER_NAME, DEFAULT_SPEECH_MODELS_URL, TTS_ENGINE_PIPER,
};
use super::status::{self, ModelState, Probes};
use super::{
    DEFAULT_TTS_RATE, MAX_DICTATION_SECONDS, MAX_NOTE_SECONDS, MAX_TTS_RATE, MIN_DICTATION_SECONDS,
    MIN_NOTE_SECONDS, MIN_TTS_RATE,
};
use crate::error::AppError;
use crate::settings::Settings;

// ---- settings --------------------------------------------------------------------------------

#[test]
fn voice_is_off_by_default_and_notes_are_on_by_their_own_switch() {
    let speech = SpeechSettings::default();

    assert!(!speech.enabled);
    assert!(speech.dictation_enabled);
    assert!(speech.read_aloud_enabled);
    assert!(speech.notes_enabled);
    assert_eq!(speech.language_override, None);
    assert_eq!(speech.max_dictation_seconds, DEFAULT_MAX_DICTATION_SECONDS);
    assert_eq!(speech.max_note_seconds, DEFAULT_MAX_NOTE_SECONDS);
    assert_eq!(speech.tts_engine, None);
    assert_eq!(speech.tts_voice, None);
    assert_eq!(speech.tts_rate, DEFAULT_TTS_RATE);
    assert_eq!(speech.notes_folder_name, DEFAULT_NOTES_FOLDER_NAME);
    assert!(speech.notes_voice_commands);
    assert_eq!(speech.speech_models_url, DEFAULT_SPEECH_MODELS_URL);
    assert_eq!(speech.sanitised(), speech, "the defaults are already clean");
    assert_eq!(Settings::default().speech, speech);
}

#[test]
fn the_documented_defaults_have_the_documented_values() {
    assert_eq!(DEFAULT_MAX_DICTATION_SECONDS, 120);
    assert_eq!(DEFAULT_MAX_NOTE_SECONDS, 1_800);
    assert_eq!(DEFAULT_NOTES_FOLDER_NAME, "Notes");
    assert_eq!(
        DEFAULT_SPEECH_MODELS_URL,
        "https://assistantcabinet.ai/models/speech/"
    );
}

#[test]
fn settings_are_written_in_camel_case_and_survive_a_round_trip() {
    let settings = Settings {
        speech: SpeechSettings {
            enabled: true,
            dictation_enabled: false,
            language_override: Some("en-US".into()),
            max_dictation_seconds: 90,
            tts_engine: Some(TTS_ENGINE_PIPER.into()),
            tts_voice: Some("siwis".into()),
            tts_rate: 1.25,
            notes_folder_name: "Dictations".into(),
            notes_voice_commands: false,
            ..SpeechSettings::default()
        },
        ..Settings::default()
    };

    let text = serde_json::to_string(&settings).expect("serialises");
    let restored: Settings = serde_json::from_str(&text).expect("deserialises");
    assert_eq!(restored.speech, settings.speech);

    let json: serde_json::Value = serde_json::from_str(&text).expect("json");
    let speech = &json["speech"];
    assert_eq!(speech["enabled"], true);
    assert_eq!(speech["dictationEnabled"], false);
    assert_eq!(speech["readAloudEnabled"], true);
    assert_eq!(speech["notesEnabled"], true);
    assert_eq!(speech["languageOverride"], "en-US");
    assert_eq!(speech["maxDictationSeconds"], 90);
    assert_eq!(speech["maxNoteSeconds"], DEFAULT_MAX_NOTE_SECONDS);
    assert_eq!(speech["ttsEngine"], "piper");
    assert_eq!(speech["ttsVoice"], "siwis");
    assert_eq!(speech["ttsRate"], 1.25);
    assert_eq!(speech["notesFolderName"], "Dictations");
    assert_eq!(speech["notesVoiceCommands"], false);
    assert_eq!(speech["speechModelsUrl"], DEFAULT_SPEECH_MODELS_URL);
}

#[test]
fn a_file_written_before_speech_existed_loads_with_voice_off() {
    let old: Settings =
        serde_json::from_str(r#"{"serverUrl":"http://127.0.0.1:8080"}"#).expect("loads");
    assert_eq!(old.speech, SpeechSettings::default());

    // A partial speech block keeps the defaults for what it does not say.
    let partial: Settings = serde_json::from_str(r#"{"speech":{"enabled":true}}"#).expect("loads");
    assert!(partial.speech.enabled);
    assert!(partial.speech.notes_enabled);
    assert_eq!(
        partial.speech.max_dictation_seconds,
        DEFAULT_MAX_DICTATION_SECONDS
    );
}

#[test]
fn caps_and_rate_are_clamped_rather_than_trusted() {
    let wild = SpeechSettings {
        max_dictation_seconds: 0,
        max_note_seconds: 0,
        tts_rate: 99.0,
        ..SpeechSettings::default()
    }
    .sanitised();
    assert_eq!(wild.max_dictation_seconds, MIN_DICTATION_SECONDS);
    assert_eq!(wild.max_note_seconds, MIN_NOTE_SECONDS);
    assert_eq!(wild.tts_rate, MAX_TTS_RATE);

    let huge = SpeechSettings {
        max_dictation_seconds: u32::MAX,
        max_note_seconds: u32::MAX,
        tts_rate: -3.0,
        ..SpeechSettings::default()
    }
    .sanitised();
    assert_eq!(huge.max_dictation_seconds, MAX_DICTATION_SECONDS);
    assert_eq!(huge.max_note_seconds, MAX_NOTE_SECONDS);
    assert_eq!(huge.tts_rate, MIN_TTS_RATE);

    for not_a_number in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let rate = SpeechSettings {
            tts_rate: not_a_number,
            ..SpeechSettings::default()
        }
        .sanitised()
        .tts_rate;
        assert_eq!(rate, DEFAULT_TTS_RATE);
    }

    let inside = SpeechSettings {
        max_dictation_seconds: 45,
        max_note_seconds: 600,
        tts_rate: 1.5,
        ..SpeechSettings::default()
    };
    assert_eq!(inside.sanitised(), inside);
}

#[test]
fn the_notes_folder_name_is_one_clean_name_that_falls_back_to_notes() {
    for (typed, expected) in [
        ("Notes", "Notes"),
        ("  Dict\u{e9}es du jour ", "Dictees-du-jour"),
        ("", "Notes"),
        ("   ", "Notes"),
        ("..", "Notes"),
        ("../../elsewhere", "elsewhere"),
        ("a/b\\c", "a-b-c"),
    ] {
        let speech = SpeechSettings {
            notes_folder_name: typed.into(),
            ..SpeechSettings::default()
        }
        .sanitised();
        assert_eq!(speech.notes_folder_name, expected, "{typed:?}");
    }
}

#[test]
fn engine_voice_language_and_url_fall_back_when_they_cannot_be_used() {
    let odd = SpeechSettings {
        tts_engine: Some("cloud".into()),
        tts_voice: Some("../../voice".into()),
        language_override: Some("fr FR; drop".into()),
        speech_models_url: "ftp://example.test/models".into(),
        ..SpeechSettings::default()
    }
    .sanitised();
    assert_eq!(odd.tts_engine, None);
    assert_eq!(odd.tts_voice, None);
    assert_eq!(odd.language_override, None);
    assert_eq!(odd.speech_models_url, DEFAULT_SPEECH_MODELS_URL);

    let fine = SpeechSettings {
        tts_engine: Some(" system ".into()),
        tts_voice: Some(" libritts_r ".into()),
        language_override: Some(" de-CH ".into()),
        speech_models_url: " https://models.example.test/speech/ ".into(),
        ..SpeechSettings::default()
    }
    .sanitised();
    assert_eq!(fine.tts_engine.as_deref(), Some("system"));
    assert_eq!(fine.tts_voice.as_deref(), Some("libritts_r"));
    assert_eq!(fine.language_override.as_deref(), Some("de-CH"));
    assert_eq!(
        fine.speech_models_url,
        "https://models.example.test/speech/"
    );

    let empty = SpeechSettings {
        tts_voice: Some("  ".into()),
        language_override: Some("".into()),
        speech_models_url: "https://".into(),
        ..SpeechSettings::default()
    }
    .sanitised();
    assert_eq!(empty.tts_voice, None);
    assert_eq!(empty.language_override, None);
    assert_eq!(empty.speech_models_url, DEFAULT_SPEECH_MODELS_URL);
}

#[test]
fn unknown_keys_in_the_speech_block_are_ignored() {
    // Unknown keys are ignored: a file written by a newer build must open.
    let newer: Settings =
        serde_json::from_str(r#"{"speech":{"enabled":true,"somethingNew":1}}"#).expect("loads");
    assert!(newer.speech.enabled);
}

// ---- language table --------------------------------------------------------------------------

#[test]
fn french_and_english_are_enabled_for_both_features() {
    for locale in ["fr-FR", "en-US"] {
        for feature in [Feature::Dictation, Feature::ReadAloud] {
            let row = languages::resolve(locale, feature).expect(locale);
            assert_eq!(row.id, locale);
            assert_eq!(row.support, Support::Enabled);
        }
    }
    assert_eq!(
        languages::resolve("fr-FR", Feature::Dictation)
            .expect("fr")
            .engine_code,
        "fr"
    );
    assert_eq!(
        languages::resolve("en-US", Feature::Dictation)
            .expect("en")
            .engine_code,
        "en"
    );
    // The case of a tag does not matter.
    assert_eq!(
        languages::resolve("FR-fr", Feature::Dictation)
            .expect("fr")
            .id,
        "fr-FR"
    );
}

#[test]
fn german_and_italian_are_declared_but_answer_unsupported_until_enabled() {
    for locale in ["de-DE", "de-CH", "it-IT"] {
        let row = languages::find(locale).expect("declared");
        assert_eq!(row.support, Support::Planned, "{locale}");
        assert!(row.voices.is_empty(), "{locale}");
        for feature in [Feature::Dictation, Feature::ReadAloud] {
            let error = languages::resolve(locale, feature).expect_err(locale);
            assert_eq!(error.code(), "speech_locale_unsupported");
            assert_eq!(error.data()["locale"], locale);
        }
    }
}

#[test]
fn an_unknown_or_empty_locale_answers_unsupported() {
    for locale in ["", "  ", "xx-XX", "fr", "klingon", "fr-FR; drop"] {
        let error = languages::resolve(locale, Feature::Dictation).expect_err(locale);
        assert_eq!(error.code(), "speech_locale_unsupported", "{locale:?}");
    }
}

#[test]
fn a_row_without_the_capability_a_feature_needs_is_unsupported_for_that_feature() {
    // Built in the test: an enabled row that can dictate but not read aloud.
    let row = languages::LanguageRow {
        read_aloud: false,
        ..LANGUAGES[0]
    };
    assert!(row.supports(Feature::Dictation));
    assert!(!row.supports(Feature::ReadAloud));
}

#[test]
fn rows_are_unique_and_each_names_its_packs_and_engine_code() {
    for (index, row) in LANGUAGES.iter().enumerate() {
        assert!(!row.engine_code.is_empty(), "{}", row.id);
        assert!(!row.rule_packs.is_empty(), "{}", row.id);
        assert!(
            LANGUAGES[index + 1..]
                .iter()
                .all(|other| other.id != row.id),
            "{} appears twice",
            row.id
        );
        if row.support == Support::Planned {
            assert!(!row.dictation && !row.read_aloud, "{}", row.id);
        }
    }
}

#[test]
fn default_voices_are_listed_by_manifest_id_and_gender_key() {
    let french = languages::find("fr-FR").expect("fr");
    let voices: Vec<(VoiceGender, &str)> = french
        .voices
        .iter()
        .map(|voice| (voice.gender, voice.id))
        .collect();
    assert_eq!(
        voices,
        [(VoiceGender::Female, "siwis"), (VoiceGender::Male, "css10")]
    );

    let english = languages::find("en-US").expect("en");
    let ids: Vec<&str> = english.voices.iter().map(|voice| voice.id).collect();
    assert_eq!(ids, ["ljspeech", "libritts_r"]);
}

#[test]
fn the_table_is_handed_to_the_interface_with_its_support_and_capabilities() {
    let table = languages::table();
    assert_eq!(table.len(), LANGUAGES.len());

    let json = serde_json::to_value(&table).expect("serialises");
    assert_eq!(json[0]["id"], "fr-FR");
    assert_eq!(json[0]["support"], "enabled");
    assert_eq!(json[0]["dictation"], true);
    assert_eq!(json[0]["readAloud"], true);
    assert_eq!(json[0]["voices"][0]["gender"], "female");
    assert_eq!(json[0]["voices"][0]["id"], "siwis");
    let german = json
        .as_array()
        .expect("array")
        .iter()
        .find(|row| row["id"] == "de-CH")
        .expect("de-CH is listed");
    assert_eq!(german["support"], "planned");
    assert_eq!(german["dictation"], false);
}

// ---- status ----------------------------------------------------------------------------------

fn reason(feature: &status::FeatureStatus) -> Option<&str> {
    feature.reason.as_deref()
}

#[test]
fn with_everything_off_each_feature_says_why() {
    let report = status::compute(&Settings::default(), &Probes::default());

    assert!(!report.enabled);
    assert!(!report.features.dictation.available);
    assert_eq!(
        reason(&report.features.dictation),
        Some("speech_unavailable")
    );
    assert!(!report.features.read_aloud.available);
    assert_eq!(
        reason(&report.features.read_aloud),
        Some("speech_unavailable")
    );
    // Notes have their own switch (on) but no documents folder is chosen yet.
    assert!(!report.features.notes.available);
    assert_eq!(
        reason(&report.features.notes),
        Some("notes_folder_unavailable")
    );
    assert_eq!(report.languages.len(), LANGUAGES.len());
}

#[test]
fn notes_do_not_depend_on_the_voice_master_switch() {
    let mut settings = Settings {
        work_folder: Some("D:\\work".into()),
        ..Settings::default()
    };
    assert!(!settings.speech.enabled);
    let report = status::compute(&settings, &Probes::default());
    assert!(report.features.notes.available);
    assert_eq!(report.features.notes.reason, None);

    settings.speech.notes_enabled = false;
    let off = status::compute(&settings, &Probes::default());
    assert_eq!(reason(&off.features.notes), Some("speech_unavailable"));
}

#[test]
fn a_blank_documents_folder_does_not_count_as_chosen() {
    let settings = Settings {
        work_folder: Some("   ".into()),
        ..Settings::default()
    };
    let report = status::compute(&settings, &Probes::default());
    assert_eq!(
        reason(&report.features.notes),
        Some("notes_folder_unavailable")
    );
}

#[test]
fn voice_on_without_an_engine_reports_the_missing_piece_in_order() {
    let mut settings = Settings::default();
    settings.speech.enabled = true;

    let none = status::compute(&settings, &Probes::default());
    assert_eq!(
        reason(&none.features.dictation),
        Some("speech_engine_missing")
    );
    assert_eq!(
        reason(&none.features.read_aloud),
        Some("tts_no_local_voice")
    );

    let engine_only = Probes {
        stt_engine: true,
        ..Probes::default()
    };
    assert_eq!(
        reason(&status::compute(&settings, &engine_only).features.dictation),
        Some("speech_model_missing")
    );

    let corrupt = Probes {
        stt_engine: true,
        stt_model: ModelState::Corrupt,
        ..Probes::default()
    };
    assert_eq!(
        reason(&status::compute(&settings, &corrupt).features.dictation),
        Some("speech_model_corrupt")
    );

    let ready = Probes {
        stt_engine: true,
        stt_model: ModelState::Present,
        tts_voice: true,
    };
    let report = status::compute(&settings, &ready);
    assert!(report.features.dictation.available);
    assert!(report.features.read_aloud.available);
}

#[test]
fn each_voice_feature_has_its_own_switch_under_the_master_switch() {
    let ready = Probes {
        stt_engine: true,
        stt_model: ModelState::Present,
        tts_voice: true,
    };
    let mut settings = Settings::default();
    settings.speech.enabled = true;
    settings.speech.dictation_enabled = false;

    let report = status::compute(&settings, &ready);
    assert_eq!(
        reason(&report.features.dictation),
        Some("speech_unavailable")
    );
    assert!(report.features.read_aloud.available);

    settings.speech.dictation_enabled = true;
    settings.speech.read_aloud_enabled = false;
    let report = status::compute(&settings, &ready);
    assert!(report.features.dictation.available);
    assert_eq!(
        reason(&report.features.read_aloud),
        Some("speech_unavailable")
    );

    // The master switch wins over a feature switch.
    settings.speech.enabled = false;
    settings.speech.read_aloud_enabled = true;
    let report = status::compute(&settings, &ready);
    assert!(!report.features.dictation.available);
    assert!(!report.features.read_aloud.available);
}

#[test]
fn the_language_follows_the_interface_unless_overridden_then_falls_back_to_the_default() {
    let mut settings = Settings::default();
    assert_eq!(settings.locale, None);
    assert_eq!(
        status::effective_locale(&settings),
        crate::settings::DEFAULT_LOCALE
    );

    settings.locale = Some("en-US".into());
    assert_eq!(status::effective_locale(&settings), "en-US");

    settings.speech.language_override = Some("fr-FR".into());
    assert_eq!(status::effective_locale(&settings), "fr-FR");
}

#[test]
fn an_unsupported_language_blocks_the_voice_features_with_its_own_code() {
    let ready = Probes {
        stt_engine: true,
        stt_model: ModelState::Present,
        tts_voice: true,
    };
    let mut settings = Settings::default();
    settings.speech.enabled = true;
    settings.speech.language_override = Some("de-CH".into());

    let report = status::compute(&settings, &ready);
    assert_eq!(report.locale, "de-CH");
    assert_eq!(
        reason(&report.features.dictation),
        Some("speech_locale_unsupported")
    );
    assert_eq!(
        reason(&report.features.read_aloud),
        Some("speech_locale_unsupported")
    );
}

#[test]
fn the_model_state_is_reported_even_when_the_master_switch_is_off() {
    // Voice is off, so the dictation reason is `speech_unavailable`; the model state still shows.
    let settings = Settings::default();
    for (found, word) in [
        (ModelState::Missing, "missing"),
        (ModelState::Present, "present"),
        (ModelState::Corrupt, "corrupt"),
    ] {
        let probes = Probes {
            stt_model: found,
            ..Probes::default()
        };
        let report = status::compute(&settings, &probes);
        assert_eq!(
            reason(&report.features.dictation),
            Some("speech_unavailable")
        );
        assert_eq!(report.stt_model, found);
        let json = serde_json::to_value(&report).expect("serialises");
        assert_eq!(json["sttModel"], word);
    }
}

#[test]
fn the_status_crosses_the_bridge_in_camel_case_with_codes_only() {
    let json = serde_json::to_value(status::compute(&Settings::default(), &Probes::default()))
        .expect("serialises");

    assert_eq!(json["enabled"], false);
    assert_eq!(json["locale"], "fr-FR");
    assert_eq!(json["features"]["dictation"]["available"], false);
    assert_eq!(
        json["features"]["dictation"]["reason"],
        "speech_unavailable"
    );
    assert_eq!(
        json["features"]["readAloud"]["reason"],
        "speech_unavailable"
    );
    assert_eq!(
        json["features"]["notes"]["reason"],
        "notes_folder_unavailable"
    );
    assert!(json["languages"].is_array());
}

// ---- error codes -----------------------------------------------------------------------------

#[test]
fn every_speech_code_is_its_own_variant_with_its_own_code() {
    let all = [
        (AppError::SpeechUnavailable, "speech_unavailable"),
        (AppError::SpeechEngineMissing, "speech_engine_missing"),
        (AppError::SpeechModelMissing, "speech_model_missing"),
        (AppError::SpeechModelCorrupt, "speech_model_corrupt"),
        (AppError::SpeechAudioTooShort, "speech_audio_too_short"),
        (AppError::SpeechAudioTooQuiet, "speech_audio_too_quiet"),
        (AppError::SpeechAudioTooLong, "speech_audio_too_long"),
        (AppError::SpeechAudioInvalid, "speech_audio_invalid"),
        (AppError::SpeechBusy, "speech_busy"),
        (AppError::SpeechCancelled, "speech_cancelled"),
        (AppError::SpeechTimeout, "speech_timeout"),
        (AppError::SpeechEngineFailed, "speech_engine_failed"),
        (
            AppError::SpeechLocaleUnsupported {
                locale: "xx-XX".into(),
            },
            "speech_locale_unsupported",
        ),
        (AppError::SpeechOutputRejected, "speech_output_rejected"),
        (AppError::TtsNoLocalVoice, "tts_no_local_voice"),
        (AppError::TtsFailed, "tts_failed"),
        (AppError::NotesFolderUnavailable, "notes_folder_unavailable"),
        (AppError::NotesNameInvalid, "notes_name_invalid"),
        (AppError::NotesTooLarge, "notes_too_large"),
        (AppError::NotesTooMany, "notes_too_many"),
        (AppError::NotesWriteFailed, "notes_write_failed"),
        (AppError::NotesAnalysisFailed, "notes_analysis_failed"),
    ];
    assert_eq!(all.len(), 22);
    for (error, code) in all {
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), code, "the display form is the code");
        let payload = serde_json::to_value(&error).expect("serialises");
        assert_eq!(payload["code"], code);
    }
}

// ---- nothing runs at launch ------------------------------------------------------------------

fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The files of this phase. A later phase that adds an engine adds its own files, and says in its
/// report which of them start a process or open a connection.
const FOUNDATION_FILES: [&str; 4] = [
    "speech/mod.rs",
    "speech/languages.rs",
    "speech/settings.rs",
    "speech/status.rs",
];

#[test]
fn launch_does_no_speech_work() {
    // No `.setup()` hook exists to hang speech work on, and the state is built empty.
    let lib = fs::read_to_string(source_root().join("lib.rs")).expect("lib.rs reads");
    assert!(!lib.contains(".setup("), "a setup hook would run at launch");
    assert!(
        lib.contains(".manage(speech_commands::SpeechState::new())"),
        "the state is registered empty, through its constructor"
    );

    let state = crate::speech_commands::SpeechState::new();
    assert_eq!(
        state.probes().expect("lock"),
        Probes::default(),
        "a fresh state has found nothing, because nothing was looked for"
    );
}

#[test]
fn the_foundations_start_no_process_thread_connection_or_file_access() {
    const FORBIDDEN: [&str; 10] = [
        "reqwest",
        "std::net",
        "TcpStream",
        "UdpSocket",
        "std::process",
        "Command::new",
        "std::fs",
        "std::thread",
        "tokio::spawn",
        "tauri::async_runtime",
    ];
    for name in FOUNDATION_FILES {
        let text = fs::read_to_string(source_root().join(name)).expect("source reads");
        for token in FORBIDDEN {
            assert!(!text.contains(token), "{name} uses {token}");
        }
    }
}
