//! Lot 9: the services built on the shared vocabulary - ranked names for speech and autocomplete,
//! transcript repair, question suggestions, the entity sheet and the outbound guard.
//!
//! These tests drive the real Analyse over invented files in temporary folders, then ask each service
//! from a second connection to the same index. The sentences the interface writes for the
//! suggestions are read from the real catalogues and put through the real classifier and router, in
//! both languages: a suggestion that earned a refusal would be worse than none.
//!
//! Every name here is invented. The documents are French because the pilot is; the code under test
//! holds no French word (the packs do).

mod common;

use std::collections::BTreeSet;

use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
use assistant_cabinet_ai_lib::folder_questions::{
    route_in_selection, FolderAnswer, QuestionRoute, Selection,
};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use assistant_cabinet_ai_lib::knowledge::guard::{strip, OutboundGuard, Strength};
use assistant_cabinet_ai_lib::knowledge::query::{QuerySelection, SelectedFile};
use assistant_cabinet_ai_lib::knowledge::repair::{propose, RepairAction, RepairReason};
use assistant_cabinet_ai_lib::knowledge::secret::IdentifierKey;
use assistant_cabinet_ai_lib::knowledge::sheet::{entity_sheet, files_for_entities};
use assistant_cabinet_ai_lib::knowledge::store as kb;
use assistant_cabinet_ai_lib::knowledge::suggest::{
    cache_key, template, Suggestion, SuggestionCache, SuggestionSet, MAX_DRY_RUNS,
};
use assistant_cabinet_ai_lib::knowledge::surface::{
    estimated_tokens, spoken_form, surface_forms, Purpose, SurfaceRequest, SPEECH_HINTS_MAX_FORMS,
    SPEECH_HINTS_MAX_TOKENS,
};
use assistant_cabinet_ai_lib::knowledge::{IntegrityReport, Origin};
use assistant_cabinet_ai_lib::tabular::engine::TabularValue;
use assistant_cabinet_ai_lib::tabular_answer::{
    prepare, prepare_if_data_only, PendingAnswer, TabularAnswer,
};
use serde_json::Value;

use common::corpus::{clinic, doc, entity_id, world};
use common::knowledge::{entity_named, snapshot, Lab};
use common::suggestions::{rig_for, sentence, suggestions, suggestions_with, templates, Rig};

const REPAIR_CONTRACT: &str = include_str!("fixtures/knowledge/repair-fr.json");

// ------------------------------------------------------------------------------------- fixtures

/// Appointments: one sheet, a real date column (a day above 12 settles day-first), an amount.
fn appointments() -> &'static str {
    "date;patient;medecin traitant;duree_min;montant\n\
     02/03/2026;Alice Archer;Dr Bruno Baker;20;50\n\
     13/03/2026;Carole Clark;Dr Bruno Baker;25;60\n\
     24/03/2026;Alice Archer;Dr Denis Dumas;20;50\n\
     25/03/2026;Emma Evans;Dr Bruno Baker;45;80\n"
}

fn invoices() -> &'static str {
    "date;fournisseur;montant\n\
     02/03/2026;MedSupply;450\n\
     03/03/2026;PapeterieDuParc;180\n\
     14/03/2026;MedSupply;620\n"
}

/// The clinic of the question-time tests plus a letter about Alice Archer, so that she is in a
/// document and in a table.
fn clinic_with_archer() -> Vec<(String, String)> {
    let mut files = clinic();
    files.push((
        "archer-courrier.txt".to_string(),
        doc(
            4000,
            "Madame Alice Archer a ete recue pour un bilan de sante.",
        ),
    ));
    files
}

async fn mixed_world() -> Lab {
    let mut lab = world(clinic_with_archer()).await;
    lab.write_data("rdv.csv", appointments());
    lab.analyse_data();
    lab
}

async fn two_workbooks_world() -> Lab {
    let mut lab = world(clinic_with_archer()).await;
    lab.write_data("rdv.csv", appointments());
    lab.write_data("factures.csv", invoices());
    lab.analyse_data();
    lab
}

fn selection_of<'a>(documents: &'a [SelectedFile], data: &'a [SelectedFile]) -> QuerySelection<'a> {
    QuerySelection { documents, data }
}

fn only(files: &[SelectedFile], names: &[&str]) -> Vec<SelectedFile> {
    files
        .iter()
        .filter(|file| names.contains(&file.relative_path.as_str()))
        .cloned()
        .collect()
}

fn files_of(lab: &Lab) -> (Vec<SelectedFile>, Vec<SelectedFile>) {
    (lab.documents(), lab.workbooks())
}

fn clean(lab: &Lab) {
    assert_eq!(
        kb::integrity_check(&lab.side()).expect("checks"),
        IntegrityReport::default(),
        "the knowledge base must be consistent"
    );
}

// ---------------------------------------------------------------------------------- the surface

fn forms(
    lab: &Lab,
    selection: Option<&QuerySelection<'_>>,
    purpose: Purpose,
    limit: usize,
    prefix: Option<&str>,
) -> Vec<assistant_cabinet_ai_lib::knowledge::surface::SurfaceForm> {
    let vocabulary = lab.vocabulary();
    surface_forms(
        &lab.side(),
        &vocabulary.packs,
        &SurfaceRequest {
            selection,
            purpose,
            limit,
            prefix,
        },
    )
    .expect("lists the names")
}

fn displays(forms: &[assistant_cabinet_ai_lib::knowledge::surface::SurfaceForm]) -> Vec<&str> {
    forms.iter().map(|form| form.display.as_str()).collect()
}

#[tokio::test]
async fn the_ranked_names_come_only_from_the_selection() {
    let lab = world(clinic()).await;
    let documents = lab.documents();
    let two = only(&documents, &["dupont-bilan.txt", "dupont-lettre.txt"]);

    let inside = forms(
        &lab,
        Some(&selection_of(&two, &[])),
        Purpose::Suggestions,
        20,
        None,
    );
    let everything = forms(&lab, None, Purpose::Suggestions, 20, None);

    assert_eq!(displays(&inside), ["Jean Dupont"]);
    assert_eq!(inside[0].type_id, "person");
    assert_eq!(
        displays(&everything),
        ["Jean Dupont", "Claire Martin"],
        "no selection means the whole base, for code that has none to give"
    );
    let nothing = forms(
        &lab,
        Some(&selection_of(&[], &[])),
        Purpose::Suggestions,
        20,
        None,
    );
    assert!(nothing.is_empty(), "an empty selection reveals no name");
}

#[tokio::test]
async fn a_name_known_only_from_other_files_is_not_revealed() {
    let lab = world(clinic()).await;
    let documents = lab.documents();
    let martin_only = only(&documents, &["martin-controle.txt", "note-01.txt"]);

    let found = forms(
        &lab,
        Some(&selection_of(&martin_only, &[])),
        Purpose::Autocomplete,
        20,
        Some("Dup"),
    );

    assert!(found.is_empty(), "Jean Dupont is not in these files");
}

#[tokio::test]
async fn files_weigh_most_and_the_order_is_the_same_every_time() {
    let lab = world(clinic()).await;
    let documents = lab.documents();
    let selection = selection_of(&documents, &[]);

    let first = forms(&lab, Some(&selection), Purpose::Suggestions, 20, None);
    let second = forms(&lab, Some(&selection), Purpose::Suggestions, 20, None);

    assert_eq!(first, second);
    assert_eq!(
        displays(&first),
        ["Jean Dupont", "Claire Martin"],
        "two files outrank one"
    );
    assert!(first[0].weight > first[1].weight);
}

#[tokio::test]
async fn a_name_typed_by_hand_is_boosted() {
    let lab = world(clinic()).await;
    let documents = lab.documents();
    let selection = selection_of(&documents, &[]);
    let martin = entity_id(&lab, "Claire Martin");
    let before = forms(&lab, Some(&selection), Purpose::Suggestions, 20, None);
    let weight_before = before
        .iter()
        .find(|form| form.entity_id == martin)
        .expect("listed")
        .weight;

    kb::set_attribute(
        &lab.side(),
        martin,
        "spoken_form",
        "Claire Martane",
        Origin::Manual,
        None,
    )
    .expect("writes");
    let manual_alias = lab.side();
    manual_alias
        .execute(
            "INSERT INTO kb_aliases (entity_id, display, normalized, phonetic_key, kind, origin, confidence)
             VALUES (?1, 'Clairette', 'clairette', '', 'manual', 'manual', 1.0)",
            [martin],
        )
        .expect("adds a typed name");
    let after = forms(&lab, Some(&selection), Purpose::Suggestions, 20, None);

    let boosted = after
        .iter()
        .find(|form| form.entity_id == martin)
        .expect("listed");
    assert!(
        boosted.weight > weight_before + 4.0,
        "a typed name gets the manual boost"
    );
    assert_eq!(
        boosted.spoken_form, "Claire Martane",
        "a typed pronunciation wins"
    );
}

#[tokio::test]
async fn an_unconfirmed_name_ranks_last_and_only_an_autocomplete_lists_it() {
    let mut files = clinic();
    files.push((
        "quintel.txt".to_string(),
        doc(
            8000,
            "Rendez-vous avec Pascal Quintel hier matin pour un controle.",
        ),
    ));
    let lab = world(files).await;
    let documents = lab.documents();
    let selection = selection_of(&documents, &[]);
    let quintel = entity_named(&lab.side(), "Pascal Quintel").expect("the base holds the guess");
    assert_eq!(quintel.status, "candidate", "an untitled run is a guess");

    let auto = forms(&lab, Some(&selection), Purpose::Autocomplete, 20, None);
    let speech = forms(&lab, Some(&selection), Purpose::SpeechHints, 20, None);
    let questions = forms(&lab, Some(&selection), Purpose::Suggestions, 20, None);

    assert_eq!(
        displays(&auto).last(),
        Some(&"Pascal Quintel"),
        "it ranks last"
    );
    assert!(!displays(&speech).contains(&"Pascal Quintel"));
    assert!(!displays(&questions).contains(&"Pascal Quintel"));
    assert_eq!(displays(&speech).len(), 2);
}

#[tokio::test]
async fn an_autocomplete_follows_the_rule_of_the_name_index() {
    let lab = world(clinic()).await;
    let documents = lab.documents();
    let selection = selection_of(&documents, &[]);

    for prefix in ["Dup", "dup", "Jean", "J", "Jean Du", "jean dupont"] {
        let found = forms(
            &lab,
            Some(&selection),
            Purpose::Autocomplete,
            8,
            Some(prefix),
        );
        assert_eq!(displays(&found), ["Jean Dupont"], "{prefix}");
    }
    for prefix in ["upont", "Jea Dupont", "Zed", "  "] {
        let found = forms(
            &lab,
            Some(&selection),
            Purpose::Autocomplete,
            8,
            Some(prefix),
        );
        assert!(found.is_empty(), "{prefix:?}");
    }
}

#[tokio::test]
async fn speech_hints_are_capped_in_names_and_in_tokens() {
    let mut files = Vec::new();
    let firsts = [
        "Alice", "Bruno", "Carole", "Denis", "Emma", "Felix", "Gisele", "Hugo",
    ];
    let lasts = [
        "Archer", "Baker", "Clark", "Dumas", "Evans", "Fabre", "Gomez", "Huet",
    ];
    for (index, first) in firsts.iter().enumerate() {
        for (other, last) in lasts.iter().enumerate() {
            files.push((
                format!("dossier-{index}-{other}.txt"),
                doc(
                    (index * 10 + other) as u32,
                    &format!("Docteur {first} {last} a recu le patient pour un bilan."),
                ),
            ));
        }
    }
    let lab = world(files).await;
    let documents = lab.documents();
    let selection = selection_of(&documents, &[]);

    let everything = forms(&lab, Some(&selection), Purpose::Suggestions, 200, None);
    let hints = forms(&lab, Some(&selection), Purpose::SpeechHints, 500, None);

    assert!(
        everything.len() > SPEECH_HINTS_MAX_FORMS,
        "the fixture must hold more names than the cap: {}",
        everything.len()
    );
    assert!(hints.len() <= SPEECH_HINTS_MAX_FORMS);
    assert!(
        estimated_tokens(&hints) <= SPEECH_HINTS_MAX_TOKENS,
        "{} tokens",
        estimated_tokens(&hints)
    );
    let weights: Vec<f32> = hints.iter().map(|form| form.weight).collect();
    assert!(
        weights.windows(2).all(|pair| pair[0] >= pair[1]),
        "chosen by weight"
    );
}

#[tokio::test]
async fn a_title_is_spoken_by_the_packs_and_a_typed_pronunciation_wins() {
    let lab = world(clinic()).await;
    let packs = lab.vocabulary().packs;

    assert_eq!(spoken_form("Dr Martin", None, &packs), "docteur Martin");
    assert_eq!(spoken_form("Mme Dupont", None, &packs), "madame Dupont");
    assert_eq!(spoken_form("Claire Martin", None, &packs), "Claire Martin");
    assert_eq!(
        spoken_form("Dr Martin", Some("Martaine"), &packs),
        "Martaine"
    );
    assert_eq!(
        spoken_form("Dr Martin", Some("  "), &packs),
        "docteur Martin"
    );
}

#[test]
fn the_token_estimate_is_characters_over_three_and_says_so() {
    use assistant_cabinet_ai_lib::knowledge::surface::{SurfaceForm, CHARS_PER_TOKEN};
    let form = |display: &str| SurfaceForm {
        entity_id: 1,
        display: display.to_string(),
        normalized: String::new(),
        phonetic_key: String::new(),
        spoken_form: String::new(),
        type_id: "person".to_string(),
        subtype: None,
        weight: 1.0,
    };
    assert_eq!(CHARS_PER_TOKEN, 3);
    assert_eq!(estimated_tokens(&[]), 0);
    // "Jean Dupont" is 11 characters plus a separator of 2 each: 13 / 3, rounded up.
    assert_eq!(estimated_tokens(&[form("Jean Dupont")]), 5);
}

// ----------------------------------------------------------------------------------- the repair

fn contract_lab_files(contract: &Value) -> Vec<(String, String)> {
    contract["known_names"]
        .as_array()
        .expect("names")
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let name = name.as_str().expect("a name");
            (
                format!("dossier-{index}.txt"),
                doc(
                    index as u32 + 1,
                    &format!("Docteur {name} a recu le patient pour un bilan de sante."),
                ),
            )
        })
        .collect()
}

fn display_of(candidate: &assistant_cabinet_ai_lib::knowledge::repair::RepairCandidate) -> &str {
    &candidate.display
}

#[tokio::test]
async fn the_repair_contract_file_holds() {
    let contract: Value = serde_json::from_str(REPAIR_CONTRACT).expect("the contract is JSON");
    let lab = world(contract_lab_files(&contract)).await;
    let documents = lab.documents();
    let selection = selection_of(&documents, &[]);
    let vocabulary = lab.vocabulary();
    let connection = lab.side();

    for case in contract["must_propose"].as_array().expect("cases") {
        let transcript = case["transcript"].as_str().expect("text");
        let proposals = propose(&connection, &vocabulary, transcript, Some(&selection), true)
            .expect("proposes");
        assert_eq!(proposals.len(), 1, "{transcript:?}: {proposals:?}");
        let proposal = &proposals[0];
        assert_eq!(
            proposal.heard,
            case["heard"].as_str().unwrap(),
            "{transcript:?}"
        );
        assert_eq!(
            &transcript[proposal.span.start..proposal.span.end],
            proposal.heard,
            "the span is the words heard"
        );
        let expected: Vec<&str> = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap())
            .collect();
        let offered: Vec<&str> = proposal.candidates.iter().map(display_of).collect();
        assert_eq!(offered, expected, "{transcript:?}");
        assert_eq!(
            proposal.action.as_code(),
            case["action"].as_str().unwrap(),
            "{transcript:?}"
        );
    }

    for transcript in contract["must_not_change"].as_array().expect("texts") {
        let transcript = transcript.as_str().expect("text");
        let proposals = propose(&connection, &vocabulary, transcript, Some(&selection), true)
            .expect("proposes");
        assert!(
            proposals.is_empty(),
            "{transcript:?} was changed: {proposals:?}"
        );
    }

    for case in contract["cases_without_case"].as_array().expect("cases") {
        let transcript = case["transcript"].as_str().expect("text");
        let proposals = propose(
            &connection,
            &vocabulary,
            transcript,
            Some(&selection),
            false,
        )
        .expect("proposes");
        match case["heard"].as_str() {
            Some(heard) => {
                assert_eq!(proposals.len(), 1, "{transcript:?}: {proposals:?}");
                assert_eq!(proposals[0].heard, heard);
                assert_eq!(
                    proposals[0].candidates[0].display,
                    case["expected"][0].as_str().unwrap()
                );
            }
            None => assert!(proposals.is_empty(), "{transcript:?}: {proposals:?}"),
        }
    }
    clean(&lab);
}

#[tokio::test]
async fn a_repair_never_rewrites_and_always_says_where_and_why() {
    let contract: Value = serde_json::from_str(REPAIR_CONTRACT).expect("json");
    let lab = world(contract_lab_files(&contract)).await;
    let documents = lab.documents();
    let selection = selection_of(&documents, &[]);
    let vocabulary = lab.vocabulary();
    let connection = lab.side();
    let transcript = "Dupond et Du pont et Thibaut";

    let proposals =
        propose(&connection, &vocabulary, transcript, Some(&selection), true).expect("proposes");

    assert_eq!(proposals.len(), 3);
    let reasons: Vec<RepairReason> = proposals.iter().map(|p| p.candidates[0].reason).collect();
    assert_eq!(
        reasons,
        [
            RepairReason::Phonetic,
            RepairReason::Alias,
            RepairReason::Phonetic
        ]
    );
    assert!(proposals.iter().all(|p| p.action == RepairAction::Suggest));
    assert!(
        proposals
            .windows(2)
            .all(|pair| pair[0].span.end <= pair[1].span.start),
        "in order and apart"
    );
    // The function takes the text by reference and returns spans: the text itself is unchanged.
    assert_eq!(transcript, "Dupond et Du pont et Thibaut");
}

#[tokio::test]
async fn a_repair_proposes_only_names_of_the_selection() {
    let lab = world(clinic()).await;
    let documents = lab.documents();
    let martin_only = only(&documents, &["martin-controle.txt", "note-01.txt"]);
    let vocabulary = lab.vocabulary();
    let connection = lab.side();

    let outside = propose(
        &connection,
        &vocabulary,
        "J'ai vu Dupond hier",
        Some(&selection_of(&martin_only, &[])),
        true,
    )
    .expect("proposes");
    let inside = propose(
        &connection,
        &vocabulary,
        "J'ai vu Dupond hier",
        Some(&selection_of(&documents, &[])),
        true,
    )
    .expect("proposes");

    assert!(
        outside.is_empty(),
        "Jean Dupont is not in the selected files"
    );
    assert_eq!(inside.len(), 1);
}

#[tokio::test]
async fn a_repair_leaves_a_word_that_is_already_a_name_alone() {
    let lab = world(clinic()).await;
    let documents = lab.documents();
    let selection = selection_of(&documents, &[]);
    let vocabulary = lab.vocabulary();
    let connection = lab.side();

    for transcript in [
        "Dupont",
        "Jean Dupont",
        "Docteur Dupont",
        "Claire Martin et Dupont",
    ] {
        let proposals = propose(&connection, &vocabulary, transcript, Some(&selection), true)
            .expect("proposes");
        assert!(proposals.is_empty(), "{transcript:?}: {proposals:?}");
    }
}

#[tokio::test]
async fn an_unconfirmed_name_is_never_a_target() {
    let mut files = clinic();
    files.push((
        "quintel.txt".to_string(),
        doc(
            8000,
            "Rendez-vous avec Pascal Quintel hier matin pour un controle.",
        ),
    ));
    let lab = world(files).await;
    let documents = lab.documents();
    let selection = selection_of(&documents, &[]);
    let vocabulary = lab.vocabulary();

    let proposals = propose(
        &lab.side(),
        &vocabulary,
        "J'ai vu Quintell hier",
        Some(&selection),
        true,
    )
    .expect("proposes");

    assert!(proposals.is_empty());
}

// --------------------------------------------------------------------------------- the questions

fn assert_each_suggestion_is_answered(lab: &Lab, rig: &Rig, set: &SuggestionSet, locale: &str) {
    let folder = lab.data_folder();
    let inventory = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index))
        .expect("discovers the folder");
    let documents_selected = !rig.documents.is_empty();
    let tables_selected = !rig.choices.is_empty();
    assert!(!set.suggestions.is_empty(), "{locale}: nothing to check");
    for suggestion in &set.suggestions {
        let question = sentence(locale, suggestion);
        let note = format!("{locale} {}: {question:?}", suggestion.template_id);
        let route = route_in_selection(
            &Selection {
                full: &inventory,
                scoped: &inventory,
                narrowed: false,
                no_documents_chosen: !documents_selected,
            },
            &lab.index,
            &question,
            locale,
        );
        let pending = prepare(
            &question,
            &folder,
            &ScopeMode::WholeFolder,
            locale,
            &lab.index,
            false,
        )
        .expect("prepares");
        let data_only = prepare_if_data_only(
            &question,
            &folder,
            &ScopeMode::WholeFolder,
            locale,
            &lab.index,
        )
        .expect("prepares");

        match suggestion.template_id {
            template::ENTITY_DOCUMENTS | template::ENTITY_COMPARE_DOCS_DATA => {
                assert!(
                    matches!(route, QuestionRoute::GlobalRetrieval),
                    "{note}: the router answered {route:?}"
                );
                assert!(
                    !tables_selected || data_only.is_none(),
                    "{note}: the tables took a question about documents"
                );
            }
            template::EACH_DOCUMENT => {
                assert!(
                    matches!(route, QuestionRoute::PerDocumentRetrieval { .. }),
                    "{note}: {route:?}"
                );
                assert!(
                    !tables_selected || data_only.is_none(),
                    "{note}: the tables took it"
                );
            }
            template::FOLDER_COUNT => {
                assert!(
                    matches!(
                        route,
                        QuestionRoute::Deterministic(FolderAnswer::IndexedCount { .. })
                            | QuestionRoute::Deterministic(FolderAnswer::FileCount { .. })
                    ),
                    "{note}: {route:?}"
                );
                assert!(!tables_selected, "{note}: offered with a table selected");
            }
            _ => {
                let PendingAnswer::Done(TabularAnswer::Value { value, .. }) = pending else {
                    panic!("{note}: the tables did not answer");
                };
                let worth = match &value {
                    TabularValue::Count(rows) => *rows > 0,
                    TabularValue::Sum(_) => true,
                    TabularValue::GroupSums(groups) => !groups.is_empty(),
                    other => panic!("{note}: unexpected value {other:?}"),
                };
                assert!(worth, "{note}: an empty answer {value:?}");
                if documents_selected {
                    assert!(
                        matches!(
                            data_only,
                            Some(PendingAnswer::Done(TabularAnswer::Value { .. }))
                        ),
                        "{note}: with documents selected too, the table tier must take it"
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn a_mixed_selection_gets_every_kind_and_each_one_is_answered_in_both_languages() {
    let lab = mixed_world().await;
    let (documents, _) = files_of(&lab);
    let rig = rig_for(&lab, documents);

    for locale in ["fr-FR", "en-US"] {
        let set = suggestions(&lab, &rig, locale, true);
        assert_eq!(
            templates(&set),
            [
                template::ENTITY_DOCUMENTS,
                template::ENTITY_ROWS_COUNT,
                template::ENTITY_TOTAL,
                template::ENTITY_PERIOD_COUNT,
                template::ENTITY_COMPARE_DOCS_DATA,
                template::TOP_GROUP,
            ],
            "{locale}"
        );
        assert_each_suggestion_is_answered(&lab, &rig, &set, locale);
    }
}

#[tokio::test]
async fn a_documents_only_selection_gets_the_folder_questions() {
    let lab = world(clinic_with_archer()).await;
    let (documents, _) = files_of(&lab);
    let rig = rig_for(&lab, documents);

    for locale in ["fr-FR", "en-US"] {
        let named = suggestions(&lab, &rig, locale, true);
        assert!(
            templates(&named).contains(&template::ENTITY_DOCUMENTS),
            "{locale}"
        );
        assert!(
            templates(&named).contains(&template::FOLDER_COUNT),
            "{locale}"
        );
        assert_each_suggestion_is_answered(&lab, &rig, &named, locale);
    }
    let small = only(
        &lab.documents(),
        &[
            "dupont-bilan.txt",
            "dupont-lettre.txt",
            "martin-controle.txt",
        ],
    );
    let rig_small = Rig {
        documents: small,
        choices: Vec::new(),
    };
    let set = suggestions(&lab, &rig_small, "fr-FR", false);
    assert_eq!(
        templates(&set),
        [template::EACH_DOCUMENT, template::FOLDER_COUNT]
    );
    assert_each_suggestion_is_answered(&lab, &rig_small, &set, "fr-FR");
    let big = suggestions(&lab, &rig_for(&lab, lab.documents()), "fr-FR", false);
    assert!(
        !templates(&big).contains(&template::EACH_DOCUMENT),
        "thirteen documents are too many for one request"
    );
}

#[tokio::test]
async fn names_are_left_out_when_hidden_and_the_set_says_there_were_some() {
    let lab = mixed_world().await;
    let (documents, _) = files_of(&lab);
    let rig = rig_for(&lab, documents);

    let hidden = suggestions(&lab, &rig, "fr-FR", false);
    let shown = suggestions(&lab, &rig, "fr-FR", true);

    assert!(hidden.names_hidden, "the interface may offer to show them");
    assert!(!shown.names_hidden);
    assert!(
        hidden
            .suggestions
            .iter()
            .all(|suggestion| !suggestion.names_somebody()),
        "{:?}",
        templates(&hidden)
    );
    assert!(shown
        .suggestions
        .iter()
        .any(|suggestion| suggestion.names_somebody()));
    for suggestion in &hidden.suggestions {
        assert!(suggestion.entity.is_none() && suggestion.value.is_none());
    }
    let nobody = world(vec![(
        "note.txt".to_string(),
        doc(1, "Un texte sans aucun nom propre."),
    )])
    .await;
    let empty_rig = rig_of(&nobody);
    let none = suggestions(&nobody, &empty_rig, "fr-FR", false);
    assert!(
        !none.names_hidden,
        "nothing is hidden when the selection holds no name"
    );
}

fn rig_of(lab: &Lab) -> Rig {
    rig_for(lab, lab.documents())
}

#[tokio::test]
async fn an_impossible_intent_is_dropped() {
    // No date column, no amount: a person in a table gets only the row count.
    let mut lab = world(clinic_with_archer()).await;
    lab.write_data(
        "liste.csv",
        "patient;medecin traitant\nAlice Archer;Dr Bruno Baker\nCarole Clark;Dr Bruno Baker\n",
    );
    lab.analyse_data();
    let rig = rig_of(&lab);
    let set = suggestions(&lab, &rig, "fr-FR", true);
    assert!(
        templates(&set).contains(&template::ENTITY_ROWS_COUNT),
        "{:?}",
        templates(&set)
    );
    assert!(
        !templates(&set).contains(&template::ENTITY_PERIOD_COUNT),
        "no date column"
    );
    assert!(
        !templates(&set).contains(&template::ENTITY_TOTAL),
        "no amount"
    );
    assert!(
        !templates(&set).contains(&template::TOP_GROUP),
        "nothing to total"
    );
    assert_each_suggestion_is_answered(&lab, &rig, &set, "fr-FR");

    // A date column the file does not settle (every day is 12 or below) is not a date column.
    let mut lab = world(clinic_with_archer()).await;
    lab.write_data(
        "rdv.csv",
        "date;patient;montant\n02/03/2026;Alice Archer;50\n03/03/2026;Carole Clark;60\n04/03/2026;Alice Archer;50\n",
    );
    lab.analyse_data();
    let set = suggestions(&lab, &rig_of(&lab), "fr-FR", true);
    assert!(
        !templates(&set).contains(&template::ENTITY_PERIOD_COUNT),
        "{:?}",
        templates(&set)
    );

    // A person in one document only gets no "about this person in my documents".
    let lab = world(vec![(
        "seul.txt".to_string(),
        doc(
            1,
            "Docteur Jean Dupont recommande un bilan sanguin complet.",
        ),
    )])
    .await;
    let set = suggestions(&lab, &rig_of(&lab), "fr-FR", true);
    assert!(
        !templates(&set).contains(&template::ENTITY_DOCUMENTS),
        "{:?}",
        templates(&set)
    );
}

#[tokio::test]
async fn a_name_that_reads_as_a_file_name_is_not_offered_for_documents() {
    // Defect KBD-18: the folder router takes the words of a person's name for the name of a file.
    let mut files = clinic();
    files.push((
        "jean-dupont.txt".to_string(),
        doc(3, "Fiche administrative de Docteur Jean Dupont."),
    ));
    let lab = world(files).await;
    let rig = rig_of(&lab);

    let set = suggestions(&lab, &rig, "fr-FR", true);

    assert!(
        !templates(&set).contains(&template::ENTITY_DOCUMENTS),
        "{:?}: the question would be read as being about one file",
        set.suggestions
    );
}

#[tokio::test]
async fn dry_runs_are_capped_and_a_smaller_cap_is_respected() {
    let lab = mixed_world().await;
    let rig = rig_of(&lab);

    let full = suggestions(&lab, &rig, "fr-FR", true);
    let one = suggestions_with(&lab, &rig, "fr-FR", true, 1);
    let none = suggestions_with(&lab, &rig, "fr-FR", true, 0);

    assert!(full.dry_runs <= MAX_DRY_RUNS, "{}", full.dry_runs);
    assert!(
        full.dry_runs > 1,
        "the full budget is used: {}",
        full.dry_runs
    );
    assert_eq!(one.dry_runs, 1);
    assert!(
        templates(&one).contains(&template::ENTITY_ROWS_COUNT),
        "the first dry run goes to the rows: {:?}",
        templates(&one)
    );
    assert!(
        !templates(&one).contains(&template::ENTITY_TOTAL),
        "the budget ran out before the total: {:?}",
        templates(&one)
    );
    assert_eq!(none.dry_runs, 0);
    assert!(
        none.suggestions.iter().all(|s| !matches!(
            s.template_id,
            template::ENTITY_ROWS_COUNT
                | template::ENTITY_TOTAL
                | template::ENTITY_PERIOD_COUNT
                | template::TOP_GROUP
        )),
        "{:?}",
        templates(&none)
    );
}

#[tokio::test]
async fn several_workbooks_make_a_sentence_name_its_file_and_it_is_answered() {
    let lab = two_workbooks_world().await;
    let rig = rig_of(&lab);
    assert_eq!(rig.choices.len(), 2);

    for locale in ["fr-FR", "en-US"] {
        let set = suggestions(&lab, &rig, locale, true);
        let tables: Vec<&Suggestion> = set
            .suggestions
            .iter()
            .filter(|s| {
                matches!(
                    s.template_id,
                    template::ENTITY_ROWS_COUNT
                        | template::ENTITY_TOTAL
                        | template::ENTITY_PERIOD_COUNT
                        | template::TOP_GROUP
                )
            })
            .collect();
        assert!(!tables.is_empty(), "{locale}");
        // Defect KBD-21: with documents and two workbooks selected, a question that names no column
        // is answered "which workbook?", so no question about the documents is offered.
        assert!(
            !templates(&set).iter().any(|id| matches!(
                *id,
                template::ENTITY_DOCUMENTS
                    | template::ENTITY_COMPARE_DOCS_DATA
                    | template::EACH_DOCUMENT
            )),
            "{locale}: {:?}",
            templates(&set)
        );
        assert!(
            tables.iter().all(|s| s.file.is_some()),
            "{locale}: without a file the product would ask which workbook"
        );
        assert!(tables
            .iter()
            .all(|s| sentence(locale, s).contains(s.file.as_deref().unwrap())));
        assert_each_suggestion_is_answered(&lab, &rig, &set, locale);
    }
}

#[tokio::test]
async fn suggestions_use_only_the_names_of_the_selection() {
    let lab = mixed_world().await;
    let documents = lab.documents();
    let martin_only = only(
        &documents,
        &["martin-controle.txt", "note-01.txt", "note-02.txt"],
    );
    let rig = Rig {
        documents: martin_only,
        choices: Vec::new(),
    };

    let set = suggestions(&lab, &rig, "fr-FR", true);

    let serialised = serde_json::to_string(&set).expect("serialises");
    for outside in ["Dupont", "Archer", "Clark", "rdv.csv"] {
        assert!(
            !serialised.contains(outside),
            "{outside} leaked into {serialised}"
        );
    }
    assert!(set
        .suggestions
        .iter()
        .all(|s| s.template_id != template::ENTITY_DOCUMENTS));
}

#[tokio::test]
async fn the_cache_serves_the_same_screen_and_a_new_analyse_changes_the_key() {
    let mut lab = mixed_world().await;
    let rig = rig_of(&lab);
    let key = |lab: &Lab, rig: &Rig, names: bool| {
        cache_key(&lab.side(), &rig.documents, &rig.choices, "fr-FR", names).expect("keys")
    };
    let first = key(&lab, &rig, true);

    assert_eq!(
        first,
        key(&lab, &rig, true),
        "the same screen, the same key"
    );
    assert_ne!(
        first,
        key(&lab, &rig, false),
        "names shown or hidden are two screens"
    );
    let other_language =
        cache_key(&lab.side(), &rig.documents, &rig.choices, "en-US", true).expect("keys");
    assert_ne!(first, other_language);
    let fewer = Rig {
        documents: rig.documents[..3].to_vec(),
        choices: rig.choices.clone(),
    };
    assert_ne!(first, key(&lab, &fewer, true), "another selection");

    let cache = SuggestionCache::default();
    assert!(cache.get(first).is_none());
    let set = suggestions(&lab, &rig, "fr-FR", true);
    cache.put(first, set.clone());
    assert_eq!(cache.get(first), Some(set));

    lab.write(
        "nouveau.txt",
        &doc(
            77,
            "Docteur Paul Lambert recoit les patients le mardi matin.",
        ),
    );
    lab.analyse().await;
    let rig_after = rig_of(&lab);
    assert_ne!(first, key(&lab, &rig_after, true), "the base changed");
}

#[tokio::test]
async fn the_cache_forgets_the_oldest_screens() {
    let cache = SuggestionCache::default();
    for key in 0..20u64 {
        cache.put(key, SuggestionSet::default());
    }
    assert!(cache.get(0).is_none(), "the oldest is gone");
    assert!(cache.get(19).is_some());
}

// ------------------------------------------------------------------------------------ the sheet

#[tokio::test]
async fn the_sheet_lists_the_files_of_the_selection_and_only_counts_the_rest() {
    let lab = mixed_world().await;
    let (documents, workbooks) = files_of(&lab);
    let vocabulary = lab.vocabulary();
    let archer = entity_id(&lab, "Alice Archer");
    let jean = entity_id(&lab, "Jean Dupont");
    let connection = lab.side();

    let everything = entity_sheet(
        &connection,
        &vocabulary.packs,
        archer,
        &selection_of(&documents, &workbooks),
    )
    .expect("reads")
    .expect("a sheet");
    assert_eq!(everything.display, "Alice Archer");
    assert_eq!(everything.type_id, "person");
    assert_eq!(everything.status, "active");
    assert_eq!(
        everything
            .documents
            .iter()
            .map(|d| d.relative_path.as_str())
            .collect::<Vec<_>>(),
        ["archer-courrier.txt"]
    );
    assert_eq!(everything.documents[0].pages, [1]);
    assert_eq!(everything.tables.len(), 1);
    assert_eq!(everything.tables[0].relative_path, "rdv.csv");
    assert_eq!(everything.tables[0].column, "patient");
    assert_eq!(everything.tables[0].rows, 2);
    assert_eq!(everything.outside.documents, 0);
    assert!(
        everything.aliases.contains(&"A. Archer".to_string()),
        "{:?}",
        everything.aliases
    );
    assert!(
        !everything.aliases.contains(&"Alice Archer".to_string()),
        "the canonical name is not an alias"
    );
    assert_eq!(everything.roles[0].role, "client");

    // Select only the table: the letter is now "outside", and only counted.
    let sheet = entity_sheet(
        &connection,
        &vocabulary.packs,
        archer,
        &selection_of(&[], &workbooks),
    )
    .expect("reads")
    .expect("a sheet");
    assert!(sheet.documents.is_empty());
    assert_eq!(sheet.tables.len(), 1);
    assert_eq!(sheet.outside.documents, 1);
    assert_eq!(sheet.outside.workbooks, 0);
    let text = serde_json::to_string(&sheet).expect("serialises");
    assert!(
        !text.contains("archer-courrier"),
        "a file outside the selection is never named: {text}"
    );

    // A name that is in no selected file still has a sheet, with counts only.
    let nowhere = entity_sheet(
        &connection,
        &vocabulary.packs,
        jean,
        &selection_of(&only(&documents, &["note-01.txt"]), &[]),
    )
    .expect("reads")
    .expect("a sheet");
    assert!(nowhere.documents.is_empty() && nowhere.tables.is_empty());
    assert_eq!(nowhere.outside.documents, 2);
}

#[tokio::test]
async fn the_sheet_follows_a_merge_and_refuses_a_deleted_or_unknown_name() {
    let lab = mixed_world().await;
    let (documents, workbooks) = files_of(&lab);
    let vocabulary = lab.vocabulary();
    let connection = lab.side();
    let selection = selection_of(&documents, &workbooks);

    assert!(
        entity_sheet(&connection, &vocabulary.packs, 9_999, &selection)
            .expect("reads")
            .is_none()
    );
    let archer = entity_id(&lab, "Alice Archer");
    connection
        .execute(
            "UPDATE kb_entities SET status = 'deleted' WHERE entity_id = ?1",
            [archer],
        )
        .expect("deletes");
    assert!(
        entity_sheet(&connection, &vocabulary.packs, archer, &selection)
            .expect("reads")
            .is_none()
    );
}

#[tokio::test]
async fn the_sheet_leaves_out_personal_identifiers_and_keeps_the_pronunciation() {
    let lab = mixed_world().await;
    let (documents, workbooks) = files_of(&lab);
    let vocabulary = lab.vocabulary();
    let connection = lab.side();
    let archer = entity_id(&lab, "Alice Archer");
    for (key, value) in [
        ("spoken_form", "Alisse Archerre"),
        ("identifier:email", "0123456789abcdef"),
        ("email", "someone@example.test"),
    ] {
        kb::set_attribute(&connection, archer, key, value, Origin::Manual, None).expect("writes");
    }

    let sheet = entity_sheet(
        &connection,
        &vocabulary.packs,
        archer,
        &selection_of(&documents, &workbooks),
    )
    .expect("reads")
    .expect("a sheet");

    let keys: Vec<&str> = sheet.attributes.iter().map(|a| a.key.as_str()).collect();
    assert_eq!(keys, ["spoken_form"], "{:?}", sheet.attributes);
}

// ------------------------------------------------------------------------------------ the guard

#[tokio::test]
async fn the_guard_catches_a_full_name_a_surname_and_an_initial_under_any_spelling() {
    let lab = mixed_world().await;
    let vocabulary = lab.vocabulary();
    let connection = lab.side();
    let guard = OutboundGuard::build(&connection).expect("builds");
    let jean = entity_id(&lab, "Jean Dupont");
    let archer = entity_id(&lab, "Alice Archer");

    let text =
        "Dossier de ALICE ARCHER; rappeler Dupont; voir J. Dupont; Archer Alice aussi; ARCHER";
    let hits = guard.scan(&vocabulary, None, text);

    let of = |id: i64, strength: Strength| -> Vec<&str> {
        hits.iter()
            .filter(|hit| hit.entity_id == id && hit.strength == strength)
            .map(|hit| &text[hit.start..hit.end])
            .collect()
    };
    assert_eq!(
        of(archer, Strength::Strong),
        ["ALICE ARCHER", "Archer Alice"]
    );
    assert_eq!(of(archer, Strength::Weak), ["ARCHER"]);
    assert_eq!(of(jean, Strength::Weak), ["Dupont", "J. Dupont"]);
    assert!(hits
        .iter()
        .all(|hit| text.is_char_boundary(hit.start) && text.is_char_boundary(hit.end)));
}

#[tokio::test]
async fn the_guard_is_global_and_ignores_every_selection() {
    // The one deliberate exception to selection scoping: it catches a name whose file is not ticked.
    let lab = world(clinic()).await;
    let vocabulary = lab.vocabulary();
    let guard = OutboundGuard::build(&lab.side()).expect("builds");
    let martin = entity_id(&lab, "Claire Martin");

    let hits = guard.scan(
        &vocabulary,
        None,
        "Merci de chercher Claire Martin dans le registre",
    );

    assert!(hits
        .iter()
        .any(|hit| hit.entity_id == martin && hit.strength == Strength::Strong));
}

#[tokio::test]
async fn two_people_who_share_a_surname_are_both_reported_and_a_full_name_wins_over_its_surname() {
    let mut files = clinic();
    files.push((
        "martin-pierre.txt".to_string(),
        doc(5, "Docteur Pierre Martin recoit le lundi."),
    ));
    let lab = world(files).await;
    let vocabulary = lab.vocabulary();
    let guard = OutboundGuard::build(&lab.side()).expect("builds");
    let claire = entity_id(&lab, "Claire Martin");
    let pierre = entity_id(&lab, "Pierre Martin");

    let alone = guard.scan(&vocabulary, None, "Voir Martin");
    let full = guard.scan(&vocabulary, None, "Voir Claire Martin");

    let ids: BTreeSet<i64> = alone.iter().map(|hit| hit.entity_id).collect();
    assert_eq!(
        ids,
        BTreeSet::from([claire, pierre]),
        "both Martins, weakly"
    );
    assert!(alone.iter().all(|hit| hit.strength == Strength::Weak));
    assert_eq!(full.len(), 1, "{full:?}");
    assert_eq!(full[0].entity_id, claire);
    assert_eq!(full[0].strength, Strength::Strong);
}

#[tokio::test]
async fn the_guard_follows_a_merge_and_forgets_a_deleted_name() {
    let lab = world(clinic()).await;
    let vocabulary = lab.vocabulary();
    let connection = lab.side();
    let martin = entity_id(&lab, "Claire Martin");
    connection
        .execute(
            "UPDATE kb_entities SET status = 'deleted' WHERE entity_id = ?1",
            [martin],
        )
        .expect("deletes");

    let guard = OutboundGuard::build(&connection).expect("builds");

    assert!(guard.scan(&vocabulary, None, "Claire Martin").is_empty());
    assert!(!guard.scan(&vocabulary, None, "Jean Dupont").is_empty());
}

#[tokio::test]
async fn the_guard_catches_an_identifier_by_its_stored_form() {
    let mut files = clinic();
    files.push((
        "devis.txt".to_string(),
        doc(
            6,
            "Devis DV-2026-0117 pour Madame Claire Martin, a regler sous huit jours.",
        ),
    ));
    let lab = world(files).await;
    let vocabulary = lab.vocabulary();
    let key = IdentifierKey::from_bytes(lab.key);
    let guard = OutboundGuard::build(&lab.side()).expect("builds");

    let text = "Relancer le devis DV-2026-0117 aujourd'hui";
    let hits = guard.scan(&vocabulary, Some(&key), text);

    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(&text[hits[0].start..hits[0].end], "DV-2026-0117");
    assert_eq!(hits[0].strength, Strength::Strong);
}

#[test]
fn stripping_replaces_each_span_once_and_survives_bad_spans() {
    use assistant_cabinet_ai_lib::knowledge::guard::GuardHit;
    let text = "Alice Archer et Dupont, puis Alice Archer";
    let hit = |start: usize, end: usize| GuardHit {
        start,
        end,
        entity_id: 1,
        strength: Strength::Strong,
    };
    let hits = [
        hit(0, 12),
        hit(6, 12),
        hit(16, 22),
        hit(29, 41),
        hit(500, 600),
        hit(3, 3),
    ];

    assert_eq!(strip(text, &hits, "[x]"), "[x] et [x], puis [x]");
    assert_eq!(strip(text, &[], "[x]"), text);
    let accented = "Lefèvre ici";
    assert_eq!(
        strip(accented, &[hit(4, 6)], "[x]"),
        accented,
        "inside a character: skipped"
    );
}

#[tokio::test]
async fn the_guard_holds_ten_thousand_names_and_still_finds_the_right_one() {
    let lab = world(clinic()).await;
    let vocabulary = lab.vocabulary();
    let connection = lab.side();
    let transaction = connection.unchecked_transaction().expect("begins");
    for index in 0..10_000 {
        let name = format!("prenom{index} nom{index}");
        transaction
            .execute(
                "INSERT INTO kb_entities (type_id, canonical_name, normalized_name, disambiguator, status, origin, created_at, updated_at)
                 VALUES ('person', ?1, ?1, '', 'active', 'manual', 0, 0)",
                [&name],
            )
            .expect("inserts an entity");
        let id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO kb_aliases (entity_id, display, normalized, phonetic_key, kind, origin, confidence)
                 VALUES (?1, ?2, ?2, '', 'canonical_variant', 'manual', 1.0)",
                rusqlite::params![id, name],
            )
            .expect("inserts an alias");
    }
    transaction.commit().expect("commits");
    let guard = OutboundGuard::build(&connection).expect("builds");
    let mut text = String::new();
    while text.len() < 2_000 {
        text.push_str("Le registre du mardi mentionne des visites et des rappels. ");
    }
    text.push_str("Contact: Prenom4242 Nom4242.");

    let hits = guard.scan(&vocabulary, None, &text);

    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(&text[hits[0].start..hits[0].end], "Prenom4242 Nom4242");
    assert!(guard.forms() >= 10_000);
}

/// Release only: `cargo test --release --test knowledge_surfaces -- --ignored --nocapture`.
#[tokio::test]
#[ignore]
async fn the_guard_scans_two_kilobytes_against_ten_thousand_names_in_under_five_milliseconds() {
    let lab = world(clinic()).await;
    let vocabulary = lab.vocabulary();
    let connection = lab.side();
    let transaction = connection.unchecked_transaction().expect("begins");
    for index in 0..10_000 {
        let name = format!("prenom{index} nom{index}");
        transaction
            .execute(
                "INSERT INTO kb_entities (type_id, canonical_name, normalized_name, disambiguator, status, origin, created_at, updated_at)
                 VALUES ('person', ?1, ?1, '', 'active', 'manual', 0, 0)",
                [&name],
            )
            .expect("inserts an entity");
        let id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO kb_aliases (entity_id, display, normalized, phonetic_key, kind, origin, confidence)
                 VALUES (?1, ?2, ?2, '', 'canonical_variant', 'manual', 1.0)",
                rusqlite::params![id, name],
            )
            .expect("inserts an alias");
    }
    transaction.commit().expect("commits");
    let build = std::time::Instant::now();
    let guard = OutboundGuard::build(&connection).expect("builds");
    let built = build.elapsed();
    let mut text = String::new();
    while text.len() < 2_000 {
        text.push_str("Le registre du mardi mentionne des visites et des rappels. ");
    }
    let mut timings = Vec::new();
    for _ in 0..50 {
        let started = std::time::Instant::now();
        let hits = guard.scan(&vocabulary, None, &text);
        timings.push(started.elapsed());
        assert!(hits.is_empty());
    }
    timings.sort();
    let median = timings[timings.len() / 2];
    println!(
        "build {built:?}, scan of {} bytes: median {median:?}",
        text.len()
    );
    assert!(median < std::time::Duration::from_millis(5), "{median:?}");
}

// --------------------------------------------------------------------------------- the isolation

#[tokio::test]
async fn no_service_of_the_lot_writes_a_row() {
    let lab = mixed_world().await;
    let (documents, workbooks) = files_of(&lab);
    let selection = selection_of(&documents, &workbooks);
    let vocabulary = lab.vocabulary();
    let rig = rig_of(&lab);
    let before = snapshot(&lab.side());
    let archer = entity_id(&lab, "Alice Archer");

    for purpose in [
        Purpose::Autocomplete,
        Purpose::SpeechHints,
        Purpose::Suggestions,
    ] {
        let _ = forms(&lab, Some(&selection), purpose, 50, Some("A"));
    }
    let _ = propose(
        &lab.side(),
        &vocabulary,
        "Alise Archer et Dupond",
        Some(&selection),
        true,
    );
    let _ = suggestions(&lab, &rig, "fr-FR", true);
    let _ = entity_sheet(&lab.side(), &vocabulary.packs, archer, &selection);
    let guard = OutboundGuard::build(&lab.side()).expect("builds");
    let _ = guard.scan(&vocabulary, None, "Alice Archer");

    assert_eq!(
        before,
        snapshot(&lab.side()),
        "a read-only service changed the base"
    );
    clean(&lab);
}

// ------------------------------------------------------------------------------- the source guard

#[test]
fn the_new_modules_have_no_network_no_log_and_no_prompt() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/knowledge");
    for module in [
        "surface.rs",
        "repair.rs",
        "suggest.rs",
        "sheet.rs",
        "guard.rs",
    ] {
        let source = std::fs::read_to_string(root.join(module)).expect("reads the module");
        for forbidden in [
            "reqwest",
            "GatewayClient",
            "http://",
            "https://",
            "println!",
            "eprintln!",
            "tracing::",
            "log::",
            "dbg!",
            "RETRIEVAL_INSTRUCTION",
            "system_prompt",
        ] {
            assert!(
                !source.contains(forbidden),
                "{module} mentions {forbidden}: a read-only vocabulary service reaches no model, no network and no log"
            );
        }
    }
}

#[tokio::test]
async fn the_repair_reads_english_names_with_the_english_packs() {
    let mut lab = Lab::new();
    lab.locale = "en-US";
    lab.packs = vec![];
    lab.write(
        "review.txt",
        &doc(
            1,
            "Dr Mark Thompson reviewed the file with Mrs Helen Carver.",
        ),
    );
    lab.write(
        "second.txt",
        &doc(2, "Dr Mark Thompson called again on Monday."),
    );
    lab.analyse().await;
    let documents = lab.documents();
    let selection = selection_of(&documents, &[]);
    let vocabulary = lab.vocabulary();

    let proposals = propose(
        &lab.side(),
        &vocabulary,
        "Please call Thomson about the file",
        Some(&selection),
        true,
    )
    .expect("proposes");
    let ordinary = propose(
        &lab.side(),
        &vocabulary,
        "Please call me about the file on Monday",
        Some(&selection),
        true,
    )
    .expect("proposes");

    assert_eq!(proposals.len(), 1, "{proposals:?}");
    assert_eq!(proposals[0].heard, "Thomson");
    assert_eq!(proposals[0].candidates[0].display, "Mark Thompson");
    assert!(ordinary.is_empty(), "{ordinary:?}");
}

#[tokio::test]
async fn the_files_of_a_name_are_asked_inside_the_selection_only() {
    let lab = mixed_world().await;
    let (documents, workbooks) = files_of(&lab);
    let archer = entity_id(&lab, "Alice Archer");
    let jean = entity_id(&lab, "Jean Dupont");
    let connection = lab.side();

    let everything = files_for_entities(
        &connection,
        &[archer, jean],
        &selection_of(&documents, &workbooks),
    )
    .expect("reads");
    let narrow = files_for_entities(
        &connection,
        &[archer, jean],
        &selection_of(&only(&documents, &["archer-courrier.txt"]), &[]),
    )
    .expect("reads");

    let paths = |id: i64,
                 found: &std::collections::BTreeMap<
        i64,
        Vec<assistant_cabinet_ai_lib::knowledge::SourceRef>,
    >|
     -> Vec<String> {
        found
            .get(&id)
            .map(|files| {
                files
                    .iter()
                    .map(|file| file.relative_path.clone())
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(
        paths(archer, &everything),
        ["archer-courrier.txt", "rdv.csv"]
    );
    assert_eq!(
        paths(jean, &everything),
        ["dupont-bilan.txt", "dupont-lettre.txt"]
    );
    assert_eq!(paths(archer, &narrow), ["archer-courrier.txt"]);
    assert!(
        !narrow.contains_key(&jean),
        "a name with no file in the selection is absent, not empty"
    );
}

#[tokio::test]
async fn the_guard_reads_accents_and_capitals_alike() {
    let lab = world(vec![(
        "dossier.txt".to_string(),
        doc(1, "Docteur Éloïse Dumézil recoit le mardi matin."),
    )])
    .await;
    let vocabulary = lab.vocabulary();
    let guard = OutboundGuard::build(&lab.side()).expect("builds");
    let eloise = entity_id(&lab, "Éloïse Dumézil");

    for text in [
        "ELOISE DUMEZIL",
        "eloise dumezil",
        "Éloïse Dumézil",
        "Dumezil Eloise",
    ] {
        let hits = guard.scan(&vocabulary, None, text);
        assert!(
            hits.iter()
                .any(|hit| hit.entity_id == eloise && hit.strength == Strength::Strong),
            "{text}: {hits:?}"
        );
    }
}

#[tokio::test]
async fn a_name_written_with_a_title_in_a_column_is_asked_without_the_title() {
    // Defect KBD-24: the tables read the word `Dr` of a question as an equality on `Dr` when several
    // doctors of the column are written `Dr ...`, and the answer is "0 rows". The sentence carries
    // only the words that point at the one cell.
    let mut files = clinic();
    files.push((
        "baker-1.txt".to_string(),
        doc(11, "Docteur Bruno Baker recoit le mardi matin au cabinet."),
    ));
    files.push((
        "baker-2.txt".to_string(),
        doc(
            12,
            "Courrier du Docteur Bruno Baker au sujet du traitement.",
        ),
    ));
    let mut lab = world(files).await;
    lab.write_data(
        "rdv.csv",
        "date;patient;medecin;montant\n\
         02/03/2026;Alice Archer;Dr Bruno Baker;50\n\
         13/03/2026;Carole Clark;Dr Bruno Baker;60\n\
         14/03/2026;Emma Evans;Dr Denis Dumas;80\n\
         15/03/2026;Alice Archer;Dr Eric Evans;60\n",
    );
    lab.analyse_data();
    let rig = rig_of(&lab);

    let set = suggestions(&lab, &rig, "fr-FR", true);

    let rows = set
        .suggestions
        .iter()
        .find(|s| s.template_id == template::ENTITY_ROWS_COUNT)
        .unwrap_or_else(|| panic!("no rows question: {:?}", templates(&set)));
    assert_eq!(rows.value.as_deref(), Some("Bruno Baker"));
    assert_eq!(
        rows.entity.as_ref().map(|e| e.display.as_str()),
        Some("Bruno Baker")
    );
    assert_eq!(
        sentence("fr-FR", rows),
        "Combien de lignes pour Bruno Baker ?"
    );
    assert!(
        templates(&set).contains(&template::ENTITY_PERIOD_COUNT),
        "{:?}",
        templates(&set)
    );
    for locale in ["fr-FR", "en-US"] {
        let set = suggestions(&lab, &rig, locale, true);
        assert_each_suggestion_is_answered(&lab, &rig, &set, locale);
    }
}

#[tokio::test]
async fn a_suggestion_does_not_start_the_reading_of_a_large_workbook() {
    // Owner rule of 11 October 2026: a screen that no one asked for a file by name must not read a
    // large one. A cold cache and a file above the limit give no table suggestion; once a question
    // has filled the cache, the limit no longer matters because nothing is read.
    let lab = mixed_world().await;
    let rig = rig_of(&lab);
    let folder = lab.data_folder();
    let inventory = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index)).unwrap();
    let resolver = assistant_cabinet_ai_lib::file_reference::FileReferenceResolver::new(&inventory);
    let vocabulary = lab.vocabulary();
    let ask = |max_cold_read_bytes: u64| {
        assistant_cabinet_ai_lib::knowledge::suggest::suggest(
            &assistant_cabinet_ai_lib::knowledge::suggest::SuggestionInput {
                index: &lab.index,
                vocabulary: &vocabulary,
                data: Some(&folder),
                documents: &rig.documents,
                workbooks: &rig.choices,
                locale: "fr-FR",
                names_allowed: true,
                names_a_file: &|text| resolver.resolve_in_question(text).is_some(),
                limit: 8,
                max_dry_runs: 5,
                max_cold_read_bytes,
            },
        )
        .expect("suggests")
    };
    let id = &rig.choices[0].content_id;
    assert!(
        !lab.index.has_tabular_workbook(id),
        "an Analyse leaves the cell cache cold"
    );

    let too_large = ask(10);
    assert!(
        too_large.dry_runs == 0 && !templates(&too_large).contains(&template::ENTITY_ROWS_COUNT),
        "{:?}",
        templates(&too_large)
    );
    assert!(
        !lab.index.has_tabular_workbook(id),
        "nothing was read, so nothing was cached"
    );

    // A question about the workbook fills the cache; the same refresh then has what it needs.
    let key_before = cache_key(&lab.side(), &rig.documents, &rig.choices, "fr-FR", true).unwrap();
    let _ = prepare(
        "Quelle est la somme de montant ?",
        &folder,
        &ScopeMode::WholeFolder,
        "fr-FR",
        &lab.index,
        false,
    )
    .unwrap();
    assert!(lab.index.has_tabular_workbook(id));
    let key_after = cache_key(&lab.side(), &rig.documents, &rig.choices, "fr-FR", true).unwrap();
    assert_ne!(
        key_before, key_after,
        "a screen kept before the cache was filled is not served"
    );
    let warm = ask(10);
    assert!(
        templates(&warm).contains(&template::ENTITY_ROWS_COUNT),
        "{:?}",
        templates(&warm)
    );

    // And a small file is read cold when it is under the limit.
    lab.side()
        .execute("DELETE FROM tabular_workbooks", [])
        .unwrap();
    let small = ask(2_000_000);
    assert!(
        templates(&small).contains(&template::ENTITY_ROWS_COUNT),
        "{:?}",
        templates(&small)
    );
}
