//! The part of read-aloud preparation that needs the application's settings, which tests outside the
//! crate cannot name. The rules themselves are tested in `tests/speech_speakable.rs`.

use super::speakable::{prepare_for_locale, reading_allowed, Part, PartKind, Segment};
use crate::error::AppError;
use crate::settings::Settings;

#[test]
fn reading_needs_the_master_switch_and_its_own_switch() {
    let mut settings = Settings::default();
    settings.speech.enabled = false;
    assert!(matches!(
        reading_allowed(&settings),
        Err(AppError::SpeechUnavailable)
    ));

    settings.speech.enabled = true;
    settings.speech.read_aloud_enabled = true;
    assert!(reading_allowed(&settings).is_ok());

    settings.speech.read_aloud_enabled = false;
    assert!(matches!(
        reading_allowed(&settings),
        Err(AppError::SpeechUnavailable)
    ));

    // Dictation being off changes nothing for read-aloud.
    settings.speech.read_aloud_enabled = true;
    settings.speech.dictation_enabled = false;
    assert!(reading_allowed(&settings).is_ok());
}

#[test]
fn the_default_pack_choice_of_the_settings_prepares_text() {
    let settings = Settings::default();
    let packs = settings.knowledge_pack_ids();
    let ids: Vec<&str> = packs.iter().map(String::as_str).collect();
    let parts = [Part {
        kind: PartKind::Plain,
        text: "Total: 12 EUR.".to_string(),
    }];
    let segments = prepare_for_locale(&parts, "en-US", &ids).expect("prepares");
    assert_eq!(
        segments,
        [Segment::Speech {
            text: "Total: twelve euros.".to_string(),
            pause_after: super::speakable::Pause::Paragraph,
        }]
    );
}
