//! Tests of the rules around the voices: the manifest rows, which voice reads a language, the speed
//! the engine is given, what the settings are told, and the state of a voice with a pinned folder.
//! Everything uses the tiny voice of `test_support`; no real voice is read.

use std::fs;

use super::languages::{self, Feature};
use super::models::{self, LicenceRating, MARKER_FILE};
use super::status::ModelState;
use super::stt::CancelToken;
use super::test_support::{place_tiny_voice, tiny_voice};
use super::tts::{self, VoiceModelStatus};
use super::worker_protocol::VoiceFamily;
use crate::error::AppError;

fn code<T: std::fmt::Debug>(outcome: Result<T, AppError>) -> String {
    outcome
        .expect_err("an error was expected")
        .code()
        .to_string()
}

fn status(id: &str, state: ModelState) -> VoiceModelStatus {
    VoiceModelStatus {
        id: id.into(),
        state,
        rating: LicenceRating::Clear,
        bytes: 1,
    }
}

// ---- the manifest ------------------------------------------------------------------------------

#[test]
fn the_manifest_holds_the_four_voices_the_language_table_names() {
    let voices = models::voice_models();
    let ids: Vec<&str> = voices.iter().map(|voice| voice.id.as_str()).collect();
    assert_eq!(ids, ["siwis", "css10", "ljspeech", "libritts_r"]);

    // Every voice a language offers is a row of the manifest, and the other way round.
    for row in languages::LANGUAGES {
        for voice in row.voices {
            let spec = models::find(voice.id).expect("the manifest has the voice");
            assert_eq!(spec.kind, "tts", "{}", voice.id);
        }
    }
    for voice in &voices {
        assert!(
            languages::LANGUAGES
                .iter()
                .any(|row| row.voices.iter().any(|offered| offered.id == voice.id)),
            "{} is in no language",
            voice.id
        );
    }
}

#[test]
fn each_voice_row_pins_its_files_its_family_and_its_licence_rating() {
    for spec in models::voice_models() {
        let voice = spec.voice.as_ref().expect("a voice row has a voice part");
        let roles: Vec<&str> = spec.files.iter().map(|file| file.role.as_str()).collect();
        assert_eq!(roles, ["model", "tokens"], "{}", spec.id);
        assert!(spec.archive.file.ends_with(".zip") && !spec.archive.file.contains('/'));
        for hash in spec
            .files
            .iter()
            .map(|file| &file.sha256)
            .chain([&spec.archive.sha256])
        {
            assert_eq!(hash.len(), 64, "{}", spec.id);
            assert!(hash
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        }
        assert!(!spec.licence.is_empty() && !spec.attribution.is_empty());
        // The phonemizer folder belongs to the Piper family and to it alone.
        match voice.family {
            VoiceFamily::Piper => {
                let tree = voice.tree.as_ref().expect("a Piper voice pins its data");
                assert_eq!(tree.dir, "espeak-ng-data");
                assert!(tree.files > 100 && tree.bytes > 1_000_000 && tree.sha256.len() == 64);
            }
            VoiceFamily::Coqui => assert!(voice.tree.is_none(), "{}", spec.id),
        }
    }
}

#[test]
fn the_ratings_are_the_owners_review_and_the_prototype_voice_is_flagged() {
    let rating = |id: &str| {
        models::find(id)
            .and_then(|spec| spec.voice.as_ref())
            .map(|voice| voice.rating)
    };
    assert_eq!(rating("ljspeech"), Some(LicenceRating::Clear));
    assert_eq!(rating("libritts_r"), Some(LicenceRating::Attribution));
    assert_eq!(rating("css10"), Some(LicenceRating::Attribution));
    assert_eq!(
        rating("siwis"),
        Some(LicenceRating::Review),
        "siwis is a controlled-prototype voice until its licence is settled"
    );
}

#[test]
fn the_voice_files_come_from_the_row_and_the_folder() {
    let (spec, _) = tiny_voice();
    let files = spec
        .voice_files(std::path::Path::new("models/tiny-voice"))
        .expect("the files");
    assert_eq!(files.family, VoiceFamily::Piper);
    assert!(files.model.ends_with("voice.onnx"));
    assert!(files.tokens.ends_with("tokens.txt"));
    assert!(files
        .data_dir
        .expect("a Piper voice has a data folder")
        .ends_with("phon-data"));
    // A speech-to-text row is not a voice.
    assert!(models::stt_model()
        .expect("manifest")
        .voice_files(std::path::Path::new("m"))
        .is_none());
}

// ---- the folder pinned as a whole ---------------------------------------------------------------

#[test]
fn a_voice_folder_is_verified_as_a_whole_and_the_marker_follows_it() {
    let (spec, _) = tiny_voice();
    let dir = tempfile::tempdir().expect("temp dir");
    let folder = spec.folder(dir.path());
    assert_eq!(models::state(&spec, &folder), ModelState::Missing);

    // Files without their pinned folder: still missing.
    fs::create_dir_all(&folder).expect("folder");
    fs::write(folder.join("voice.onnx"), vec![9_u8; 2_000]).expect("model");
    fs::write(folder.join("tokens.txt"), b"a 0\nb 1\n").expect("tokens");
    assert_eq!(models::state(&spec, &folder), ModelState::Missing);

    place_tiny_voice(&folder);
    assert_eq!(
        models::state(&spec, &folder),
        ModelState::Unverified,
        "placed by hand: not trusted until Verify has hashed it"
    );
    models::verify(&spec, &folder, &CancelToken::new()).expect("verifies");
    assert_eq!(models::state(&spec, &folder), ModelState::Present);
    assert!(folder.join(MARKER_FILE).is_file());
}

#[test]
fn verify_refuses_a_changed_a_missing_and_an_extra_file_in_the_pinned_folder() {
    let (spec, _) = tiny_voice();
    for (what, damage) in [
        (
            "changed",
            Box::new(|folder: &std::path::Path| {
                fs::write(folder.join("phon-data/phondata"), vec![0_u8; 600]).expect("damage");
            }) as Box<dyn Fn(&std::path::Path)>,
        ),
        (
            "missing",
            Box::new(|folder: &std::path::Path| {
                fs::remove_file(folder.join("phon-data/voices/default")).expect("remove");
            }),
        ),
        (
            "extra",
            Box::new(|folder: &std::path::Path| {
                fs::write(folder.join("phon-data/lang/extra"), b"x").expect("extra");
            }),
        ),
    ] {
        let dir = tempfile::tempdir().expect("temp dir");
        let folder = spec.folder(dir.path());
        place_tiny_voice(&folder);
        damage(&folder);
        assert_eq!(
            code(models::verify(&spec, &folder, &CancelToken::new())),
            "speech_model_corrupt",
            "{what}"
        );
        assert!(!folder.join(MARKER_FILE).exists(), "{what}: no marker");
    }
}

#[test]
fn the_pinned_folder_is_part_of_the_fingerprint() {
    let (spec, _) = tiny_voice();
    let mut other = spec.clone();
    assert_eq!(spec.fingerprint(), other.fingerprint());
    other
        .voice
        .as_mut()
        .and_then(|voice| voice.tree.as_mut())
        .expect("tree")
        .sha256 = "0".repeat(64);
    assert_ne!(
        spec.fingerprint(),
        other.fingerprint(),
        "a new folder version in a later release makes the old marker stale"
    );
}

// ---- which voice reads ---------------------------------------------------------------------------

#[test]
fn the_chosen_voice_reads_when_it_is_installed_else_the_first_installed_of_the_language() {
    let all_present = |_: &models::ModelSpec| ModelState::Present;
    let id = |outcome: Result<&models::ModelSpec, AppError>| outcome.expect("a voice").id.clone();
    assert_eq!(id(tts::choose_voice("fr-FR", None, all_present)), "siwis");
    assert_eq!(
        id(tts::choose_voice("fr-FR", Some("css10"), all_present)),
        "css10"
    );
    assert_eq!(
        id(tts::choose_voice("en-US", Some("libritts_r"), all_present)),
        "libritts_r"
    );
    // A voice of another language, or one nobody knows, is ignored.
    assert_eq!(
        id(tts::choose_voice("fr-FR", Some("ljspeech"), all_present)),
        "siwis"
    );
    assert_eq!(
        id(tts::choose_voice("fr-FR", Some("../x"), all_present)),
        "siwis"
    );
    // The chosen one is not installed: the language still has a voice.
    let only_css10 = |spec: &models::ModelSpec| {
        if spec.id == "css10" {
            ModelState::Present
        } else {
            ModelState::Missing
        }
    };
    assert_eq!(
        id(tts::choose_voice("fr-FR", Some("siwis"), only_css10)),
        "css10"
    );
}

#[test]
fn no_installed_voice_is_no_local_voice_and_a_damaged_one_is_said_so() {
    let none = |_: &models::ModelSpec| ModelState::Missing;
    assert_eq!(
        code(tts::choose_voice("fr-FR", None, none)),
        "tts_no_local_voice"
    );
    let side_loaded = |_: &models::ModelSpec| ModelState::Unverified;
    assert_eq!(
        code(tts::choose_voice("en-US", None, side_loaded)),
        "tts_no_local_voice",
        "a voice nobody verified does not read"
    );
    let damaged = |spec: &models::ModelSpec| {
        if spec.id == "siwis" {
            ModelState::Corrupt
        } else {
            ModelState::Missing
        }
    };
    assert_eq!(
        code(tts::choose_voice("fr-FR", None, damaged)),
        "speech_model_corrupt"
    );
    // A voice of another language never reads this one.
    let english_only = |spec: &models::ModelSpec| {
        if spec.id == "ljspeech" {
            ModelState::Present
        } else {
            ModelState::Missing
        }
    };
    assert_eq!(
        code(tts::choose_voice("fr-FR", None, english_only)),
        "tts_no_local_voice"
    );
}

#[test]
fn a_language_without_read_aloud_is_unsupported_not_voiceless() {
    let all_present = |_: &models::ModelSpec| ModelState::Present;
    for locale in ["de-DE", "it-IT", "xx-XX", ""] {
        assert_eq!(
            code(tts::choose_voice(locale, None, all_present)),
            "speech_locale_unsupported",
            "{locale}"
        );
    }
}

#[test]
fn the_probe_is_about_the_language_being_read() {
    let statuses = vec![
        status("siwis", ModelState::Present),
        status("ljspeech", ModelState::Missing),
        status("libritts_r", ModelState::Corrupt),
    ];
    assert!(tts::any_voice_ready("fr-FR", &statuses));
    assert!(!tts::any_voice_ready("en-US", &statuses));
    assert!(!tts::any_voice_ready("de-DE", &statuses));
    assert!(!tts::any_voice_ready("fr-FR", &[]));
}

#[test]
fn the_settings_are_told_the_state_of_every_voice_without_any_hashing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let listed = tts::voice_statuses(dir.path());
    let ids: Vec<&str> = listed.iter().map(|voice| voice.id.as_str()).collect();
    assert_eq!(ids, ["siwis", "css10", "ljspeech", "libritts_r"]);
    assert!(listed
        .iter()
        .all(|voice| voice.state == ModelState::Missing));
    assert!(listed.iter().all(|voice| voice.bytes > 1_000_000));
    let siwis = listed
        .iter()
        .find(|voice| voice.id == "siwis")
        .expect("siwis");
    assert_eq!(siwis.rating, LicenceRating::Review);
    // Nothing was created on disk by looking.
    assert_eq!(fs::read_dir(dir.path()).expect("dir").count(), 0);
}

#[test]
fn the_voice_status_crosses_the_bridge_in_camel_case_with_a_lowercase_rating() {
    let json = serde_json::to_value(status("siwis", ModelState::Present)).expect("serialises");
    assert_eq!(
        json,
        serde_json::json!({ "id": "siwis", "state": "present", "rating": "clear", "bytes": 1 })
    );
}

// ---- the speed -------------------------------------------------------------------------------------

#[test]
fn the_rate_becomes_a_percentage_held_between_the_slowest_and_the_fastest() {
    assert_eq!(tts::speed_percent(1.0), 100);
    assert_eq!(tts::speed_percent(1.25), 125);
    assert_eq!(tts::speed_percent(0.5), 50);
    assert_eq!(tts::speed_percent(2.0), 200);
    assert_eq!(tts::speed_percent(0.01), 50);
    assert_eq!(tts::speed_percent(99.0), 200);
    assert_eq!(tts::speed_percent(f32::NAN), 100);
    assert_eq!(tts::speed_percent(f32::INFINITY), 100);
}

#[test]
fn read_aloud_is_a_feature_of_the_languages_that_offer_voices() {
    for row in languages::LANGUAGES {
        assert_eq!(
            row.supports(Feature::ReadAloud),
            !row.voices.is_empty() && row.read_aloud && row.support == languages::Support::Enabled,
            "{}",
            row.id
        );
    }
}
