//! Voice commands of a note dictation, phase 12: spoken punctuation, headings, line breaks and
//! "new note", in French and in English, from the rule packs. Not scanned by the language guard of
//! the Rust sources (`tests/` is not): the sentences are French where the rule pack is.
//!
//! The settings-driven entry (`apply_for_settings`) is tested inside the crate, where `Settings` is
//! visible: `speech/dictation_commands_tests.rs`.
//!
//! Every sentence in this file is invented.

use std::fs;
use std::path::PathBuf;

use assistant_cabinet_ai_lib::speech::dictation_commands::{apply, Applied, CommandRules};
use assistant_cabinet_ai_lib::speech::languages::Feature;
use assistant_cabinet_ai_lib::speech::speakable_rules::{parsed, rules_for_feature};

fn commands(locale: &str) -> &'static CommandRules {
    rules_for_feature(locale, Feature::Dictation)
        .expect("the pack loads")
        .dictation_commands
        .as_ref()
        .expect("the pack has commands")
}

fn french(text: &str) -> Applied {
    apply(text, commands("fr-FR"), 0)
}

fn english(text: &str) -> Applied {
    apply(text, commands("en-US"), 0)
}

fn one(applied: Applied) -> String {
    assert_eq!(applied.pieces.len(), 1, "{applied:?}");
    applied.pieces.into_iter().next().unwrap()
}

#[test]
fn both_packs_carry_their_commands() {
    for (name, rules) in parsed() {
        let rules = rules
            .as_ref()
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(rules.dictation_commands.is_some(), "{name} has no commands");
    }
}

#[test]
fn a_passage_without_a_command_is_left_as_it_is() {
    let text = "Le patient revient demain, pour un contr\u{f4}le.";

    assert_eq!(one(french(text)), text);
    assert_eq!(
        one(english("The visit is tomorrow.")),
        "The visit is tomorrow."
    );
}

#[test]
fn spoken_punctuation_replaces_what_the_engine_wrote() {
    assert_eq!(
        one(french(
            "Bonjour, virgule, comment allez-vous, point d'interrogation."
        )),
        "Bonjour, comment allez-vous?"
    );
    assert_eq!(one(french("Bonjour virgule merci")), "Bonjour, merci");
    assert_eq!(one(french("Bonjour, point. Merci")), "Bonjour. Merci");
}

#[test]
fn a_full_stop_starts_the_next_word_with_a_capital() {
    assert_eq!(
        one(french("bonjour point je vais bien")),
        "bonjour. Je vais bien"
    );
    assert_eq!(
        one(english("hello period this is fine question mark")),
        "hello. This is fine?"
    );
}

#[test]
fn the_longest_phrase_wins() {
    assert_eq!(one(french("Pardon point d'exclamation")), "Pardon!");
    assert_eq!(one(french("Il a dit deux points merci")), "Il a dit: merci");
    assert_eq!(
        one(french("Premier point virgule second")),
        "Premier; second"
    );
    // An apostrophe of any shape is the same.
    assert_eq!(one(french("Pardon point d\u{2019}exclamation")), "Pardon!");
}

#[test]
fn matching_ignores_case_and_accents() {
    assert_eq!(
        one(french(
            "OUVREZ LA PARENTHESE note fermez la parenth\u{e8}se"
        )),
        "(note)"
    );
    assert_eq!(
        one(french(
            "Il dit Ouvrez les guillemets bonjour fermer les guillemets"
        )),
        "Il dit \"bonjour\""
    );
    assert_eq!(
        one(english("Say Open Parenthesis yes CLOSE PARENTHESIS")),
        "Say (yes)"
    );
}

#[test]
fn only_whole_words_count() {
    assert_eq!(one(french("Le pointeur laser")), "Le pointeur laser");
    assert_eq!(
        one(french("Voir le titre-l\u{e0} demain")),
        "Voir le titre-l\u{e0} demain"
    );
    // A hyphen written in a command is the same as a blank.
    assert_eq!(one(french("Sous titre Suites")).trim(), "## Suites");
}

#[test]
fn a_title_runs_to_the_end_of_the_sentence() {
    assert_eq!(
        one(french("Titre Visite de contr\u{f4}le. Le patient va bien.")),
        "\n\n# Visite de contr\u{f4}le\nLe patient va bien."
    );
    assert_eq!(
        one(english("Title Weekly review. Everything is on track.")),
        "\n\n# Weekly review\nEverything is on track."
    );
}

#[test]
fn the_plural_the_engine_writes_is_a_subtitle_too() {
    assert_eq!(one(french("Sous-titres Motif")), "\n\n## Motif\n");
    assert_eq!(one(french("Sous titres. Motif.")), "\n\n## Motif\n");
    assert_eq!(one(english("Subtitles Reason")), "\n\n## Reason\n");
    assert_eq!(one(english("Subtitle Reason")), "\n\n## Reason\n");
}

#[test]
fn a_new_line_is_a_hard_break_the_preview_draws() {
    assert_eq!(
        one(french("Un nouvelle ligne deux \u{e0} la ligne trois")),
        "Un  \nDeux  \nTrois"
    );
    // Not after a title, not at the start of a text, not before a paragraph.
    assert_eq!(
        one(french("Titre Bilan \u{e0} la ligne Suite")),
        "\n\n# Bilan\nSuite"
    );
    assert_eq!(one(french("\u{c0} la ligne Suite")), "\nSuite");
    assert_eq!(
        one(french("Un nouveau paragraphe nouvelle ligne deux")),
        "Un\n\nDeux"
    );
}

#[test]
fn a_title_runs_to_a_new_line_command() {
    assert_eq!(
        one(french(
            "Sous-titre Ant\u{e9}c\u{e9}dents nouvelle ligne diab\u{e8}te"
        )),
        "\n\n## Ant\u{e9}c\u{e9}dents\nDiab\u{e8}te"
    );
}

#[test]
fn a_title_runs_to_the_end_of_the_passage() {
    let applied = french("Titre Compte rendu de la visite");

    assert_eq!(
        applied.pieces,
        vec!["\n\n# Compte rendu de la visite\n".to_string()]
    );
    assert_eq!(applied.pending_heading, 0);
}

#[test]
fn a_title_with_a_spoken_full_stop_loses_the_stop_only() {
    assert_eq!(one(french("Titre Bilan point Suite")), "\n\n# Bilan\nSuite");
    // A question mark belongs to the heading.
    assert_eq!(
        one(french("Titre Et maintenant point d'interrogation Suite")),
        "\n\n# Et maintenant?\nSuite"
    );
}

#[test]
fn a_heading_command_alone_stays_open_for_the_next_passage() {
    let first = french("Titre.");

    assert_eq!(first.pieces, vec![String::new()]);
    assert_eq!(first.pending_heading, 1);

    let second = apply("Compte rendu.", commands("fr-FR"), first.pending_heading);
    assert_eq!(second.pieces, vec!["\n\n# Compte rendu\n".to_string()]);
    assert_eq!(second.pending_heading, 0);

    let sub = english("Subtitle");
    assert_eq!(sub.pieces, vec![String::new()]);
    assert_eq!(sub.pending_heading, 2);
}

#[test]
fn lines_and_paragraphs_break_the_text() {
    assert_eq!(
        one(french(
            "Premier \u{e9}l\u{e9}ment nouvelle ligne second \u{e9}l\u{e9}ment"
        )),
        "Premier \u{e9}l\u{e9}ment  \nSecond \u{e9}l\u{e9}ment"
    );
    assert_eq!(
        one(french("Fin du paragraphe nouveau paragraphe d\u{e9}but")),
        "Fin du paragraphe\n\nD\u{e9}but"
    );
    assert_eq!(
        one(english("One new line two new paragraph three")),
        "One  \nTwo\n\nThree"
    );
}

#[test]
fn a_line_command_opening_a_passage_leads_with_its_break() {
    assert_eq!(one(french("\u{c0} la ligne Suite")), "\nSuite");
    assert_eq!(one(french("Nouvelle ligne.")), "\n");
    assert_eq!(one(english("New paragraph")), "\n\n");
}

#[test]
fn a_new_note_cuts_the_passage_in_two() {
    let applied = french("Premier sujet. Nouvelle note. Deuxi\u{e8}me sujet.");

    assert_eq!(
        applied.pieces,
        vec![
            "Premier sujet.".to_string(),
            "Deuxi\u{e8}me sujet.".to_string()
        ]
    );
    assert_eq!(english("First topic new note").pieces.len(), 2);
    assert_eq!(
        french("Nouvelle note Suite").pieces,
        vec![String::new(), "Suite".to_string()]
    );
}

#[test]
fn a_heading_does_not_cross_a_new_note() {
    let applied = french("Titre Premier sujet nouvelle note Suite");

    assert_eq!(
        applied.pieces,
        vec!["\n\n# Premier sujet".to_string(), "Suite".to_string()]
    );
    assert_eq!(applied.pending_heading, 0);
}

#[test]
fn a_heading_with_no_word_leaves_no_marker_behind() {
    assert_eq!(one(french("Titre nouvelle ligne Suite")), "\nSuite");
}

#[test]
fn commands_are_applied_in_the_order_they_are_spoken() {
    assert_eq!(
        one(french(
            "Titre Rendez-vous. Pr\u{e9}sents deux points Martin virgule Durand nouvelle ligne fin"
        )),
        "\n\n# Rendez-vous\nPr\u{e9}sents: Martin, Durand  \nFin"
    );
}

#[test]
fn the_command_is_registered_over_a_pure_function() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(manifest.join("src/lib.rs")).unwrap();
    assert!(lib.contains("speech_commands::speech_apply_dictation_commands"));
    let commands = fs::read_to_string(manifest.join("src/speech_commands.rs")).unwrap();
    let body = commands
        .split("pub fn speech_apply_dictation_commands")
        .nth(1)
        .expect("the command exists");
    for token in [
        "SpeechState",
        "worker",
        "open_index",
        "gateway",
        "write",
        "File::",
    ] {
        assert!(!body.contains(token), "the command names {token}");
    }
    let module = fs::read_to_string(manifest.join("src/speech/dictation_commands.rs")).unwrap();
    for token in [
        "std::fs",
        "reqwest",
        "std::net",
        "tokio",
        "rusqlite",
        "Command::new",
    ] {
        assert!(!module.contains(token), "the module names {token}");
    }
}
