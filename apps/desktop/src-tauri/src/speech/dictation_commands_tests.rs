//! The settings-driven entry of the voice commands: the switch, the language, an unshipped language.
//! The cases of the commands themselves are in `tests/speech_dictation_commands.rs`.

use super::dictation_commands::{apply_for_settings, untouched};
use crate::error::AppError;
use crate::settings::Settings;

fn english() -> Settings {
    Settings {
        locale: Some("en-US".to_string()),
        ..Settings::default()
    }
}

#[test]
fn the_setting_turns_the_commands_off() {
    let mut settings = english();
    settings.speech.notes_voice_commands = false;

    let applied = apply_for_settings(&settings, "Hello comma friend", 0).unwrap();

    assert_eq!(applied, untouched("Hello comma friend"));
    assert_eq!(applied.pieces, vec!["Hello comma friend".to_string()]);
    assert_eq!(applied.pending_heading, 0);

    settings.speech.notes_voice_commands = true;
    let applied = apply_for_settings(&settings, "Hello comma friend", 0).unwrap();
    assert_eq!(applied.pieces, vec!["Hello, friend".to_string()]);
}

#[test]
fn the_commands_are_on_by_default() {
    let applied = apply_for_settings(&english(), "Hello comma friend", 0).unwrap();

    assert_eq!(applied.pieces, vec!["Hello, friend".to_string()]);
}

#[test]
fn the_language_is_the_one_dictation_works_in() {
    let mut settings = english();
    settings.speech.language_override = Some("fr-FR".to_string());

    let applied = apply_for_settings(&settings, "Bonjour virgule ami", 0).unwrap();

    assert_eq!(applied.pieces, vec!["Bonjour, ami".to_string()]);
}

#[test]
fn a_heading_left_open_comes_back_in_the_next_call() {
    let first = apply_for_settings(&english(), "Title", 0).unwrap();
    assert_eq!(first.pending_heading, 1);

    let second = apply_for_settings(&english(), "Weekly review.", first.pending_heading).unwrap();
    assert_eq!(second.pieces, vec!["\n\n# Weekly review\n".to_string()]);
}

#[test]
fn a_language_that_is_not_shipped_answers_with_its_code() {
    let mut settings = english();
    settings.speech.language_override = Some("de-DE".to_string());

    let error = apply_for_settings(&settings, "Hallo Komma", 0).unwrap_err();

    assert!(matches!(error, AppError::SpeechLocaleUnsupported { .. }));
}

#[test]
fn a_heading_level_outside_one_to_six_is_ignored() {
    let applied = apply_for_settings(&english(), "Hello", 9).unwrap();

    assert_eq!(applied.pieces, vec!["Hello".to_string()]);
}
