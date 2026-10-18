//! Name suggestions after a dictation (`speech::repair`): the contract file of the transcript
//! repair is run through the function the command wraps, over the real Analyse of invented files.
//! The cases of `repair-fr.json` are reused as they are; no fixture is added.
//!
//! These tests open no socket and start no engine: a transcript is a string.

mod common;

use assistant_cabinet_ai_lib::knowledge::query::{QuerySelection, SelectedFile};
use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;
use assistant_cabinet_ai_lib::speech::repair::{or_nothing, propose_for, RepairSet};
use serde_json::Value;

use common::corpus::{clinic, doc, world};
use common::knowledge::{snapshot, Lab};

const REPAIR_CONTRACT: &str = include_str!("fixtures/knowledge/repair-fr.json");

fn contract() -> Value {
    serde_json::from_str(REPAIR_CONTRACT).expect("the contract is JSON")
}

fn contract_files(contract: &Value) -> Vec<(String, String)> {
    contract["known_names"]
        .as_array()
        .expect("names")
        .iter()
        .enumerate()
        .map(|(index, name)| {
            (
                format!("dossier-{index}.txt"),
                doc(
                    index as u32 + 1,
                    &format!(
                        "Docteur {} a recu le patient pour un bilan de sante.",
                        name.as_str().expect("a name")
                    ),
                ),
            )
        })
        .collect()
}

fn selection_of<'a>(documents: &'a [SelectedFile]) -> QuerySelection<'a> {
    QuerySelection {
        documents,
        data: &[],
    }
}

/// The words of a transcript between two UTF-16 offsets, the way the window slices a string.
fn utf16_slice(text: &str, start: usize, end: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16(&units[start..end]).expect("whole characters")
}

fn read(lab: &Lab, transcript: &str, case_reliable: bool, names_allowed: bool) -> RepairSet {
    let documents = lab.documents();
    propose_for(
        &lab.side(),
        &lab.vocabulary(),
        transcript,
        Some(&selection_of(&documents)),
        case_reliable,
        names_allowed,
    )
    .expect("reads")
}

#[tokio::test]
async fn the_contract_file_holds_through_the_function_the_command_wraps() {
    let contract = contract();
    let lab = world(contract_files(&contract)).await;

    for case in contract["must_propose"].as_array().expect("cases") {
        let transcript = case["transcript"].as_str().expect("text");
        let set = read(&lab, transcript, true, true);

        assert_eq!(set.proposals.len(), 1, "{transcript:?}: {set:?}");
        let proposal = &set.proposals[0];
        assert_eq!(
            proposal.heard,
            case["heard"].as_str().unwrap(),
            "{transcript:?}"
        );
        assert_eq!(
            utf16_slice(transcript, proposal.start, proposal.end),
            proposal.heard,
            "the span is the words heard, counted the way the window counts"
        );
        let offered: Vec<&str> = proposal
            .candidates
            .iter()
            .map(|candidate| candidate.display.as_str())
            .collect();
        let expected: Vec<&str> = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap())
            .collect();
        assert_eq!(offered, expected, "{transcript:?}");
        assert_eq!(
            proposal.action,
            case["action"].as_str().unwrap(),
            "{transcript:?}"
        );
        assert_eq!(set.hidden_count, 0);
    }

    for transcript in contract["must_not_change"].as_array().expect("texts") {
        let transcript = transcript.as_str().expect("text");
        let set = read(&lab, transcript, true, true);

        assert!(
            set.proposals.is_empty(),
            "{transcript:?} was changed: {set:?}"
        );
    }

    for case in contract["cases_without_case"].as_array().expect("cases") {
        let transcript = case["transcript"].as_str().expect("text");
        let set = read(&lab, transcript, false, true);

        match case["heard"].as_str() {
            Some(heard) => {
                assert_eq!(set.proposals.len(), 1, "{transcript:?}: {set:?}");
                assert_eq!(set.proposals[0].heard, heard);
            }
            None => assert!(set.proposals.is_empty(), "{transcript:?}: {set:?}"),
        }
    }
}

#[tokio::test]
async fn repair_never_leaves_the_selection() {
    let lab = world(clinic()).await;
    let documents = lab.documents();
    let martin_only: Vec<SelectedFile> = documents
        .iter()
        .filter(|file| file.relative_path == "martin-controle.txt")
        .cloned()
        .collect();
    let all = |names_allowed| {
        propose_for(
            &lab.side(),
            &lab.vocabulary(),
            "J'ai vu Dupond hier",
            Some(&selection_of(&documents)),
            true,
            names_allowed,
        )
        .expect("reads")
    };
    let narrowed = |names_allowed| {
        propose_for(
            &lab.side(),
            &lab.vocabulary(),
            "J'ai vu Dupond hier",
            Some(&selection_of(&martin_only)),
            true,
            names_allowed,
        )
        .expect("reads")
    };

    assert_eq!(all(true).proposals.len(), 1);
    // The name is in other files only: nothing is proposed, and nothing is counted as hidden
    // either, so the count cannot tell the window that a name exists outside the selection.
    assert_eq!(narrowed(true), RepairSet::default());
    assert_eq!(narrowed(false), RepairSet::default());
}

#[tokio::test]
async fn with_the_knowledge_base_off_there_is_nothing_to_repair() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.mode = KnowledgeMode::Off;
    for (name, text) in clinic() {
        lab.write(&name, &text);
    }
    lab.analyse().await;

    let set = read(&lab, "J'ai vu Dupond hier", true, true);

    assert_eq!(set, RepairSet::default());
}

#[test]
fn the_command_answers_before_it_opens_the_index_when_the_base_is_off() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands.rs"),
    )
    .expect("reads");
    let body = &source[source
        .find("pub fn speech_repair_names(")
        .expect("the command")..];
    let body = &body[..body.find("\n}").expect("its end")];

    let off = body.find("reads_files()").expect("checks the mode");
    assert!(off < body.find("open_index(").expect("opens the index"));
    assert!(
        body.contains("repair::or_nothing("),
        "any failure is an empty answer"
    );
    assert!(
        !body.contains("ingest") && !body.contains("kb::"),
        "reads only"
    );
}

#[tokio::test]
async fn hidden_names_reach_the_window_as_a_count_only() {
    let contract = contract();
    let lab = world(contract_files(&contract)).await;

    let hidden = read(&lab, "Dupond et Du pont et Thibaut", true, false);
    let shown = read(&lab, "Dupond et Du pont et Thibaut", true, true);

    assert!(hidden.proposals.is_empty());
    assert_eq!(hidden.hidden_count, 3);
    assert_eq!(shown.proposals.len(), 3);
    let wire = serde_json::to_string(&hidden).expect("serialises");
    for name in contract["known_names"].as_array().unwrap() {
        assert!(!wire.contains(name.as_str().unwrap()), "{wire}");
    }
    assert!(!wire.contains("Dupond"), "not even the words heard: {wire}");
}

#[tokio::test]
async fn dictating_does_not_change_the_knowledge_base() {
    let contract = contract();
    let lab = world(contract_files(&contract)).await;
    let before = snapshot(&lab.side());

    for transcript in [
        "Dupond et Du pont et Thibaut",
        "Lef\u{e8}vre",
        "rien ici",
        "J'ai croise Dupond au cabinet ce matin",
    ] {
        for names_allowed in [true, false] {
            read(&lab, transcript, true, names_allowed);
        }
    }

    assert_eq!(
        snapshot(&lab.side()),
        before,
        "every kb_* table is as it was"
    );
}

#[tokio::test]
async fn a_failure_of_the_reading_is_an_empty_answer() {
    let contract = contract();
    let lab = world(contract_files(&contract)).await;
    let documents = lab.documents();
    // A connection to a database with no knowledge tables: the reading fails.
    let broken = rusqlite::Connection::open_in_memory().expect("opens");

    let result = propose_for(
        &broken,
        &lab.vocabulary(),
        "Dupond",
        Some(&selection_of(&documents)),
        true,
        true,
    );

    assert!(result.is_err());
    assert_eq!(or_nothing(result), RepairSet::default());
}

#[tokio::test]
async fn the_entity_type_changes_nothing() {
    let contract = contract();
    let lab = world(contract_files(&contract)).await;
    let usual = read(&lab, "Dupond et Du pont et Thibaut", true, true);
    assert_eq!(usual.proposals.len(), 3);

    // The same names read as an organisation and as an item with a subtype nobody shipped: the
    // suggestions are the same, because the reading never looks at a type.
    let connection = lab.side();
    connection
        .execute(
            "UPDATE kb_entities SET type_id = 'organization' WHERE canonical_name = 'Jean Dupont'",
            [],
        )
        .expect("retypes");
    connection
        .execute(
            "UPDATE kb_entities SET type_id = 'item', subtype = 'any-subtype' \
             WHERE canonical_name = 'Paul Thibault'",
            [],
        )
        .expect("retypes");

    assert_eq!(
        read(&lab, "Dupond et Du pont et Thibaut", true, true),
        usual
    );
}

#[test]
fn the_module_names_no_entity_type_and_no_medicine() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/speech/repair.rs"),
    )
    .expect("reads");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .take_while(|line| !line.contains("#[cfg(test)]"))
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();

    for word in [
        "type_id",
        "person",
        "organization",
        "medic",
        "drug",
        "subtype",
    ] {
        assert!(!code.contains(word), "{word}");
    }
}
