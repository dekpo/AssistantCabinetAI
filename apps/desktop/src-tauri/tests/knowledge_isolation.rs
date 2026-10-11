//! Lot 6: what a question can and cannot touch (master invariants I3, I8 and I9).
//!
//! The knowledge base is built from the user's files and from nothing else: a question, an answer, a
//! conversation, a chip, a widen action change no row of it. And nothing it knows (a name, an alias,
//! a type, a role, a method, a pack) reaches the model, the gateway or a log: a model-facing string
//! is built from the instruction, the folder's counts, the excerpts and the question, and a
//! diagnostics line holds numbers and machine codes.
//!
//! Every name here is invented.

mod common;

use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use assistant_cabinet_ai_lib::knowledge::diagnostics::{self, RetrievalDiagnostics};
use assistant_cabinet_ai_lib::knowledge::targeting::{self, RequestMode, TargetingRequest};
use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;
use assistant_cabinet_ai_lib::retrieval;
use assistant_cabinet_ai_lib::work_folder_context::{self, ContextView};

use common::corpus::{
    auto, chips, clean, clinic, embedding_of, entity_id, plan_for, plan_with, search, world,
};
use common::knowledge::{every_text_value, small_world_policy, snapshot, Lab};

// -------------------------------------------------------------------- I3: a question writes nothing

#[tokio::test]
async fn asking_a_question_does_not_change_the_knowledge_base() {
    let lab = world(clinic()).await;
    let before = snapshot(&lab.side());
    let jean = entity_id(&lab, "Jean Dupont");

    for (mode, request, question) in [
        (
            KnowledgeMode::Auto,
            auto(),
            "Que dit Jean Dupont du bilan ?",
        ),
        (KnowledgeMode::Suggest, auto(), "Que dit J. Dupont ?"),
        (KnowledgeMode::Suggest, chips(&[jean]), "Que dit-il ?"),
        (
            KnowledgeMode::Auto,
            TargetingRequest {
                carry_over: vec![jean],
                ..auto()
            },
            "Et son adresse ?",
        ),
        (
            KnowledgeMode::Auto,
            TargetingRequest {
                mode: RequestMode::Off,
                ..auto()
            },
            "Que dit Jean Dupont ?",
        ),
    ] {
        let plan = plan_with(&lab, mode, &request, question, small_world_policy());
        let searched = search(
            &lab,
            &plan,
            question,
            &embedding_of("para-0000 Docteur Jean Dupont"),
        );
        let mut measured = RetrievalDiagnostics::default();
        targeting::note_documents(&lab.index, &plan, &searched, &mut measured);
    }

    assert_eq!(snapshot(&lab.side()), before);
    clean(&lab);
}

#[tokio::test]
async fn a_question_with_a_never_seen_full_name_creates_no_entity() {
    let lab = world(clinic()).await;
    let before = snapshot(&lab.side());
    let entities_before = common::knowledge::entities(&lab.side()).len();

    let unknown = plan_for(
        &lab,
        KnowledgeMode::Auto,
        "Que dit le Docteur Zacharie Quenelle ?",
    );
    assert!(unknown.analysis.as_ref().unwrap().entities.is_empty());
    // A surname that one person of the selection carries is that person; the new full name beside
    // it is nobody, and nothing is learned from either.
    let mixed = plan_for(
        &lab,
        KnowledgeMode::Auto,
        "Dr Martin est cardiologue, que dit Madame Ophelie Vasseur ?",
    );
    let found: Vec<String> = mixed
        .analysis
        .as_ref()
        .unwrap()
        .entities
        .iter()
        .map(|entity| entity.display.clone())
        .collect();
    assert_eq!(found, vec!["Claire Martin".to_string()]);

    assert_eq!(
        common::knowledge::entities(&lab.side()).len(),
        entities_before
    );
    assert_eq!(snapshot(&lab.side()), before);
}

#[tokio::test]
async fn a_model_answer_does_not_change_the_knowledge_base() {
    // The question path has no input for an answer: it is not read back. What the user types next
    // may repeat what the model said, and that is a question like any other (I3).
    let lab = world(clinic()).await;
    let before = snapshot(&lab.side());

    let next =
        "D'apres la reponse, Dr Martin est cardiologue et Jean Dupont est son associe : confirmez";
    let plan = plan_for(&lab, KnowledgeMode::Auto, next);
    let _ = plan.report(false);

    assert_eq!(snapshot(&lab.side()), before);
}

#[tokio::test]
async fn the_history_of_a_conversation_does_not_change_the_knowledge_base() {
    let lab = world(clinic()).await;
    let before = snapshot(&lab.side());
    let jean = entity_id(&lab, "Jean Dupont");
    let claire = entity_id(&lab, "Claire Martin");
    let request = TargetingRequest {
        carry_over: vec![jean, claire, 999_999],
        ..auto()
    };

    let plan = plan_with(
        &lab,
        KnowledgeMode::Auto,
        &request,
        "Et alors ?",
        small_world_policy(),
    );

    assert!(plan.analysis.is_some());
    assert_eq!(snapshot(&lab.side()), before);
}

// ---------------------------------------------------------- I8 and I9: nothing it knows leaves

/// The prompt the application builds for a question, from the pieces `commands::sourced_answer` and
/// `Writer::write` use: the retrieval instruction, the folder's counts, the numbered excerpts, then
/// the question.
fn prompt_for(lab: &Lab, evidence: &[retrieval::Evidence], question: &str) -> String {
    let inventory = WorkFolderInventory::discover(lab.work.path(), Some(&lab.index))
        .expect("discovers the folder");
    let folder_context = work_folder_context::build(&inventory, &ContextView::Summary);
    let grounding = format!(
        "{}\n{}",
        work_folder_context::build_system_turn(retrieval::RETRIEVAL_INSTRUCTION, &folder_context),
        retrieval::format_evidence(evidence)
    );
    format!("{grounding}\n\nQuestion: {question}")
}

#[tokio::test]
async fn no_knowledge_text_reaches_a_model_facing_string() {
    let lab = world(clinic()).await;
    let connection = lab.side();
    let question = "Que dit Jean Dupont du bilan ?";
    let embedding = embedding_of("para-0000 Docteur Jean Dupont");

    // The text of every document, which legitimately holds names.
    let documents: String = [
        "dupont-bilan.txt",
        "dupont-lettre.txt",
        "martin-controle.txt",
    ]
    .iter()
    .map(|name| std::fs::read_to_string(lab.work.path().join(name)).expect("reads"))
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase();
    let boilerplate = format!("{} {question}", retrieval::RETRIEVAL_INSTRUCTION).to_lowercase();

    // What only the knowledge base holds: its types, roles, methods, alias kinds, packs and tables.
    let mut only_the_base: Vec<String> = vec![
        "kb_".into(),
        "health-fr".into(),
        "gazetteer".into(),
        "title_full_name".into(),
        "partial_surname".into(),
        "title_form".into(),
        "canonical_variant".into(),
        "organization".into(),
        "candidate".into(),
    ];
    for query in [
        "SELECT DISTINCT type_id FROM kb_entity_types",
        "SELECT DISTINCT method FROM kb_mentions",
        "SELECT DISTINCT kind FROM kb_aliases",
        "SELECT DISTINCT value FROM kb_meta WHERE value != ''",
        "SELECT DISTINCT phonetic_key FROM kb_aliases WHERE phonetic_key != ''",
    ] {
        let mut statement = connection.prepare(query).expect("prepares");
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("queries");
        only_the_base.extend(rows.map(|row| row.expect("reads").to_lowercase()));
    }
    // Every alias of every entity: `j dupont`, `dupont jean`, `docteur dupont`...
    let mut statement = connection
        .prepare("SELECT DISTINCT normalized FROM kb_aliases")
        .expect("prepares");
    let aliases: Vec<String> = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("queries")
        .map(|row| row.expect("reads"))
        .collect();
    assert!(
        aliases.iter().any(|alias| alias == "j dupont"),
        "the base holds aliases to look for"
    );

    for mode in [
        KnowledgeMode::Off,
        KnowledgeMode::Suggest,
        KnowledgeMode::Auto,
    ] {
        let plan = plan_for(&lab, mode, question);
        let searched = search(&lab, &plan, question, &embedding);
        let prompt = prompt_for(&lab, &searched.outcome.evidence, question).to_lowercase();

        for text in only_the_base.iter().chain(aliases.iter()) {
            // A counter such as `2` or a key of two letters is in every text; only a string the
            // base could have put there on purpose is looked for. And a word the documents or the
            // question themselves hold is not the base's.
            if text.chars().count() < 4
                || text.chars().all(|c| c.is_ascii_digit())
                || documents.contains(text.as_str())
                || boilerplate.contains(text.as_str())
            {
                continue;
            }
            assert!(
                !prompt.contains(text.as_str()),
                "{mode:?}: {text:?} reached the prompt"
            );
        }
        assert!(
            prompt.contains(&retrieval::RETRIEVAL_INSTRUCTION.to_lowercase()),
            "the instruction is the one it has always been"
        );
    }
}

#[tokio::test]
async fn the_prompt_is_built_from_the_same_pieces_whatever_the_knowledge_mode() {
    // Same evidence in, same prompt out: the base changes which excerpts there are, not the words
    // around them.
    let lab = world(clinic()).await;
    let question = "Que dit Jean Dupont du bilan ?";
    let embedding = embedding_of(
        "para-0000 Docteur Jean Dupont recommande un bilan sanguin complet pour le suivi annuel.",
    );
    let off = plan_for(&lab, KnowledgeMode::Off, question);
    let suggest = plan_for(&lab, KnowledgeMode::Suggest, question);

    let from_off = prompt_for(
        &lab,
        &search(&lab, &off, question, &embedding).outcome.evidence,
        question,
    );
    let from_suggest = prompt_for(
        &lab,
        &search(&lab, &suggest, question, &embedding)
            .outcome
            .evidence,
        question,
    );

    assert_eq!(from_off, from_suggest);
}

#[tokio::test]
async fn diagnostics_rows_and_the_timing_log_contain_only_numbers_and_codes() {
    let lab = world(clinic()).await;
    let question = "Que dit Jean Dupont du bilan ?";
    let plan = plan_for(&lab, KnowledgeMode::Auto, question);
    let searched = search(
        &lab,
        &plan,
        question,
        &embedding_of("para-0000 Docteur Jean Dupont"),
    );
    let mut measured = RetrievalDiagnostics::default();
    measured.selected_documents = 12;
    targeting::note_documents(&lab.index, &plan, &searched, &mut measured);
    let targeting_part = measured.targeting.as_ref().expect("measured");
    assert_eq!(
        targeting_part.query_entities.len(),
        1,
        "an entity is there to leak"
    );

    let log = lab.app_data.path().join("retrieval-timings-test.jsonl");
    diagnostics::append_timing_line(&log, &measured, 1_760_000_000).expect("writes");
    let written = std::fs::read_to_string(&log).expect("reads");

    for planted in [
        "Jean",
        "Dupont",
        "Claire",
        "Martin",
        "dupont-bilan",
        "note-0",
        ".txt",
        "bilan",
        "para-",
    ] {
        assert!(
            !written.contains(planted),
            "{planted:?} leaked into {written}"
        );
    }
    let codes = [
        "documents",
        "auto",
        "entity_files",
        "applied",
        "person",
        "canonical_name",
        "alias",
        "whole_folder",
    ];
    for line in written.lines() {
        let value: serde_json::Value = serde_json::from_str(line).expect("json");
        only_numbers_and_codes(&value, &codes);
    }
}

fn only_numbers_and_codes(value: &serde_json::Value, codes: &[&str]) {
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
        serde_json::Value::String(code) => {
            assert!(codes.contains(&code.as_str()), "unexpected string {code:?}")
        }
        serde_json::Value::Array(items) => items
            .iter()
            .for_each(|item| only_numbers_and_codes(item, codes)),
        serde_json::Value::Object(fields) => fields
            .values()
            .for_each(|field| only_numbers_and_codes(field, codes)),
    }
}

#[tokio::test]
async fn no_name_the_base_holds_is_in_a_row_the_questions_write() {
    let lab = world(clinic()).await;
    let connection = lab.side();
    let embedding = embedding_of("para-0000 Docteur Jean Dupont");
    for question in [
        "Que dit Jean Dupont ?",
        "Que dit Claire Martin ?",
        "Que dit J. Dupont ?",
    ] {
        let plan = plan_for(&lab, KnowledgeMode::Auto, question);
        let searched = search(&lab, &plan, question, &embedding);
        let mut measured = RetrievalDiagnostics::default();
        targeting::note_documents(&lab.index, &plan, &searched, &mut measured);
        diagnostics::record_question(&connection, 1, &measured).expect("records");
    }

    let mut statement = connection
        .prepare("SELECT * FROM kb_diagnostics")
        .expect("prepares");
    let width = statement.column_count();
    let rows: Vec<String> = statement
        .query_map([], |row| {
            let mut cells = Vec::new();
            for index in 0..width {
                if let rusqlite::types::ValueRef::Text(bytes) = row.get_ref(index)? {
                    cells.push(String::from_utf8_lossy(bytes).to_string());
                }
            }
            Ok(cells.join(" "))
        })
        .expect("queries")
        .map(|row| row.expect("reads"))
        .collect();
    assert_eq!(rows.len(), 3);
    for text in rows.join(" ").split_whitespace() {
        for name in ["Jean", "Dupont", "Claire", "Martin", ".txt"] {
            assert!(
                !text.contains(name),
                "{name:?} in a diagnostics row: {text:?}"
            );
        }
    }
    // And the base itself still holds its names: the rows above are not where they live.
    assert!(every_text_value(&connection)
        .iter()
        .any(|text| text == "Jean Dupont"));
}

// ------------------------------------------------ lot 9: the services built on the shared vocabulary

#[tokio::test]
async fn the_vocabulary_services_change_no_row_and_are_the_same_every_time() {
    use assistant_cabinet_ai_lib::knowledge::guard::OutboundGuard;
    use assistant_cabinet_ai_lib::knowledge::query::QuerySelection;
    use assistant_cabinet_ai_lib::knowledge::repair::propose;
    use assistant_cabinet_ai_lib::knowledge::sheet::entity_sheet;
    use assistant_cabinet_ai_lib::knowledge::suggest::{suggest, SuggestionInput};
    use assistant_cabinet_ai_lib::knowledge::surface::{surface_forms, Purpose, SurfaceRequest};

    let lab = world(clinic()).await;
    let documents = lab.documents();
    let selection = QuerySelection {
        documents: &documents,
        data: &[],
    };
    let vocabulary = lab.vocabulary();
    let jean = entity_id(&lab, "Jean Dupont");
    let before = snapshot(&lab.side());

    let run = || {
        let forms = surface_forms(
            &lab.side(),
            &vocabulary.packs,
            &SurfaceRequest {
                selection: Some(&selection),
                purpose: Purpose::SpeechHints,
                limit: 40,
                prefix: None,
            },
        )
        .expect("lists");
        let proposals = propose(
            &lab.side(),
            &vocabulary,
            "J'ai vu Dupond",
            Some(&selection),
            true,
        )
        .expect("proposes");
        let sheet = entity_sheet(&lab.side(), &vocabulary.packs, jean, &selection).expect("reads");
        let suggestions = suggest(&SuggestionInput {
            index: &lab.index,
            vocabulary: &vocabulary,
            data: None,
            documents: &documents,
            workbooks: &[],
            locale: "fr-FR",
            names_allowed: true,
            names_a_file: &|_| false,
            limit: 6,
            max_dry_runs: 5,
            max_cold_read_bytes: 2_000_000,
        })
        .expect("suggests");
        let guard = OutboundGuard::build(&lab.side()).expect("builds");
        let hits = guard.scan(&vocabulary, None, "Jean Dupont");
        format!("{forms:?}{proposals:?}{sheet:?}{suggestions:?}{hits:?}")
    };

    let first = run();
    let second = run();

    assert_eq!(first, second, "a read-only service answers the same twice");
    assert_eq!(before, snapshot(&lab.side()), "and writes nothing");
    clean(&lab);
}
