//! Lot 6: the knowledge base at question time.
//!
//! These tests drive the real Analyse over invented files in a temporary folder, then plan and run
//! questions with `knowledge::targeting` (the Tauri-free part of the question path). The gateway is
//! faked at Analyse time, and a question's embedding is chosen by the test, so which excerpt ranks
//! first is a fact of the fixture and not of a model.
//!
//! Every name and number here is invented. The documents are French because the pilot is; the code
//! under test holds no French word (the packs do).

mod common;

use assistant_cabinet_ai_lib::analysis_scope::{ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::knowledge::diagnostics::{self, RetrievalDiagnostics};
use assistant_cabinet_ai_lib::knowledge::query::{QueryAnalysis, QueryEntity, SelectedFile};
use assistant_cabinet_ai_lib::knowledge::reduce::{
    reduce, Outcome, ReduceInput, SideInput, SkipReason, TargetingPolicy,
};
use assistant_cabinet_ai_lib::knowledge::store as kb;
use assistant_cabinet_ai_lib::knowledge::targeting::{
    self, DocumentPlan, RequestMode, TargetingPlan, TargetingRequest,
};
use assistant_cabinet_ai_lib::knowledge::{EntityStatus, KnowledgeMode};
use assistant_cabinet_ai_lib::tabular_answer::{self, TabularAnswer};

use common::corpus::{
    auto, chips, clean, clinic, doc, embedding_of, entity_id, filler, found_in, names, paths_of,
    plan_for, plan_with, search, world,
};
use common::knowledge::{small_world_policy, snapshot, Lab};

/// A small, fixed pseudo-random sequence: the property tests need variety, not entropy.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }
}

// ------------------------------------------------------------------------ what a question names

#[tokio::test]
async fn a_name_in_the_question_is_found_by_its_full_form_its_surname_and_its_title() {
    let lab = world(clinic()).await;

    for question in [
        "Que dit Jean Dupont du bilan ?",
        "Que dit le Dr Dupont du bilan ?",
        "Que dit Dupont du bilan ?",
        "Que dit docteur Jean Dupont du bilan ?",
    ] {
        let plan = plan_for(&lab, KnowledgeMode::Auto, question);
        let analysis = plan.analysis.as_ref().expect("read the question");
        assert_eq!(analysis.entities.len(), 1, "{question}");
        let jean = &analysis.entities[0];
        assert_eq!(jean.display, "Jean Dupont", "{question}");
        assert!(jean.confidence >= 0.80, "{question}: {}", jean.confidence);
        assert_eq!(
            names(&plan.reduction.documents.candidates),
            names(&["dupont-bilan.txt".into(), "dupont-lettre.txt".into()]),
            "{question}"
        );
    }
    clean(&lab);
}

#[tokio::test]
async fn a_question_with_no_name_in_it_reduces_nothing() {
    let lab = world(clinic()).await;

    let plan = plan_for(
        &lab,
        KnowledgeMode::Auto,
        "Que contiennent les notes internes ?",
    );

    assert_eq!(plan.reduction.documents.skipped, Some(SkipReason::NoEntity));
    assert!(plan.reduction.documents.candidates.is_empty());
    assert_eq!(plan.reduction.skipped_reason(), Some(SkipReason::NoEntity));
    assert!(plan.report(false).is_none(), "nothing to tell the user");
}

#[tokio::test]
async fn an_initial_and_a_surname_is_offered_as_a_possible_match_and_never_applied() {
    let lab = world(clinic()).await;

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit J. Dupont ?");

    let analysis = plan.analysis.as_ref().expect("read the question");
    assert!(
        analysis.entities.is_empty(),
        "a possible match is not a name"
    );
    assert_eq!(analysis.possibles.len(), 1);
    assert_eq!(analysis.possibles[0].candidates[0].display, "Jean Dupont");
    assert!(plan.reduction.documents.candidates.is_empty());
    let report = plan.report(false).expect("the user is offered the name");
    assert_eq!(report.possibles.len(), 1);
}

#[tokio::test]
async fn a_misspelled_name_is_a_possible_match_never_a_reduction() {
    let lab = world(clinic()).await;

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Dupond du bilan ?");

    let analysis = plan.analysis.as_ref().expect("read the question");
    assert!(analysis.entities.is_empty());
    assert_eq!(analysis.possibles.len(), 1);
    assert!(!plan.reduction.would_apply());
}

#[tokio::test]
async fn two_doctors_of_the_same_surname_are_asked_about_never_picked() {
    let lab = world(vec![
        (
            "paul.txt".into(),
            doc(1, "Docteur Paul Martin recoit le lundi."),
        ),
        (
            "pierre.txt".into(),
            doc(2, "Docteur Pierre Martin recoit le mardi."),
        ),
        filler(1),
        filler(2),
    ])
    .await;

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit le Dr Martin ?");

    let analysis = plan.analysis.as_ref().expect("read the question");
    assert_eq!(analysis.ambiguities.len(), 1);
    assert_eq!(analysis.ambiguities[0].candidates.len(), 2);
    assert_eq!(
        plan.reduction.documents.skipped,
        Some(SkipReason::Ambiguous)
    );
    let report = plan.report(false).expect("the user is asked");
    assert_eq!(report.ambiguities.len(), 1);

    // The full name settles it, and the user's own chip does too.
    let exact = plan_for(&lab, KnowledgeMode::Auto, "Que dit le Dr Pierre Martin ?");
    assert_eq!(
        exact.reduction.documents.candidates,
        vec!["pierre.txt".to_string()]
    );
    let pierre = entity_id(&lab, "Pierre Martin");
    let picked = plan_with(
        &lab,
        KnowledgeMode::Suggest,
        &chips(&[pierre]),
        "Que dit le Dr Martin ?",
        small_world_policy(),
    );
    assert_eq!(
        picked.reduction.documents.candidates,
        vec!["pierre.txt".to_string()]
    );
}

#[tokio::test]
async fn ambiguity_chips_only_offer_entities_present_in_the_selection() {
    // Invariant I7: two Martins in the base, one in the files she selected.
    let lab = world(vec![
        (
            "paul.txt".into(),
            doc(1, "Docteur Paul Martin recoit le lundi."),
        ),
        (
            "pierre.txt".into(),
            doc(2, "Docteur Pierre Martin recoit le mardi."),
        ),
        filler(1),
        filler(2),
    ])
    .await;
    let vocabulary = lab.vocabulary();
    let only_paul: Vec<SelectedFile> = lab
        .documents()
        .into_iter()
        .filter(|file| file.relative_path != "pierre.txt")
        .collect();

    let plan = lab.plan(
        &vocabulary,
        KnowledgeMode::Auto,
        &auto(),
        "Que dit le Dr Martin ?",
        SideInput::new(&only_paul),
        SideInput::new(&[]),
        small_world_policy(),
    );

    let analysis = plan.analysis.as_ref().expect("read the question");
    assert!(
        analysis.ambiguities.is_empty(),
        "the other Martin is not hers"
    );
    assert_eq!(analysis.entities.len(), 1);
    assert_eq!(analysis.entities[0].display, "Paul Martin");
    assert_eq!(
        plan.reduction.documents.candidates,
        vec!["paul.txt".to_string()]
    );
    let report = plan.report(false).expect("applied");
    assert!(!format!("{report:?}").contains("Pierre"));
}

#[tokio::test]
async fn a_document_that_only_says_a_title_and_a_surname_is_reached_by_the_lexical_net() {
    let mut files = clinic();
    files.push((
        "rappel.txt".into(),
        doc(0, "Rappel : M. Dupont est attendu mardi pour le suivi."),
    ));
    files.push((
        "facture.txt".into(),
        doc(0, "Reglement de Jean-Dupont pour la facture de mars."),
    ));
    let lab = world(files).await;

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Jean Dupont ?");

    let documents = &plan.reduction.documents;
    assert!(documents
        .candidates
        .contains(&"dupont-bilan.txt".to_string()));
    assert!(
        documents
            .added_by_lexical
            .contains(&"rappel.txt".to_string()),
        "{documents:?}"
    );
    assert!(
        documents.candidates.contains(&"facture.txt".to_string()),
        "the hyphenated spelling is the same words: {documents:?}"
    );
    assert!(!documents.candidates.contains(&"note-01.txt".to_string()));
}

#[tokio::test]
async fn the_forms_of_a_name_shared_with_other_people_do_not_widen_the_net() {
    // Many documents say Martin; only a few are about Dr Pierre Martin. His surname alone is not a
    // form the net searches, because other Martins carry it.
    let mut files = vec![
        (
            "pierre.txt".to_string(),
            doc(1, "Docteur Pierre Martin recoit le mardi."),
        ),
        (
            "paul.txt".to_string(),
            doc(2, "Docteur Paul Martin recoit le lundi."),
        ),
    ];
    for index in 0..10 {
        files.push((
            format!("client-{index:02}.txt"),
            doc(
                10 + index,
                &format!("Madame Sophie Martin, dossier numero {index}, reglement en attente."),
            ),
        ));
    }
    let lab = world(files).await;

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit le Dr Pierre Martin ?");

    assert_eq!(
        plan.reduction.documents.candidates,
        vec!["pierre.txt".to_string()],
        "{:?}",
        plan.reduction.documents
    );
}

#[tokio::test]
async fn a_surname_shared_only_with_a_workbook_outside_the_selection_is_still_searched() {
    // The owner's own index, step B1 of the human test: a patient `Dupont J` sits in a workbook
    // that was analysed but is not selected. She did not choose it, so it does not make `dupont`
    // ambiguous among the documents she did choose, and the document that only says `M. Dupont`
    // is still reached.
    let mut files = clinic();
    files.push((
        "rappel-m-dupont.txt".into(),
        doc(0, "Rappel : M. Dupont est attendu mardi pour le suivi."),
    ));
    let mut lab = world(files).await;
    lab.write_data(
        "rdv.csv",
        "date;patient;duree_min
02/03/2026;Dupont J;20
03/03/2026;Martin A;25
",
    );
    lab.analyse_data();

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Jean Dupont du bilan ?");
    let documents = &plan.reduction.documents;
    assert!(
        documents
            .added_by_lexical
            .contains(&"rappel-m-dupont.txt".to_string()),
        "{documents:?}"
    );

    // With the workbook selected too, `Dupont J` is in the selection and the surname is shared: the
    // net no longer searches it, and the possible match keeps the workbook instead.
    let vocabulary = lab.vocabulary();
    let docs = lab.documents();
    let books = lab.workbooks();
    let both = lab.plan(
        &vocabulary,
        KnowledgeMode::Auto,
        &auto(),
        "Que dit Jean Dupont du bilan ?",
        SideInput::new(&docs),
        SideInput::new(&books),
        small_world_policy(),
    );
    assert!(
        !both
            .reduction
            .documents
            .added_by_lexical
            .contains(&"rappel-m-dupont.txt".to_string()),
        "{:?}",
        both.reduction.documents
    );
}

#[tokio::test]
async fn several_people_in_one_question_rank_the_files_holding_most_of_them_first() {
    let mut files = clinic();
    files.push((
        "dupont-martin.txt".into(),
        doc(
            0,
            "Entretien entre le Docteur Jean Dupont et Madame Claire Martin.",
        ),
    ));
    let lab = world(files).await;

    let plan = plan_for(
        &lab,
        KnowledgeMode::Auto,
        "Que disent Jean Dupont et Claire Martin ?",
    );

    let candidates = &plan.reduction.documents.candidates;
    assert_eq!(candidates[0], "dupont-martin.txt", "both of them: first");
    // No hard intersection: a file with only one of them is still searched.
    assert!(candidates.contains(&"dupont-bilan.txt".to_string()));
    assert!(candidates.contains(&"martin-controle.txt".to_string()));
    assert_eq!(plan.analysis.as_ref().unwrap().entities.len(), 2);
}

#[tokio::test]
async fn an_identifier_in_the_question_finds_the_file_that_carries_it() {
    let mut files = clinic();
    files.push((
        "facture-mars.txt".into(),
        doc(1, "Facture FAC-2026-0042 a regler avant la fin du mois."),
    ));
    files.push((
        "contact.txt".into(),
        doc(2, "Contact : claire.martin@exemple.fr pour toute question."),
    ));
    let lab = world(files).await;

    // An invoice number is stored as itself.
    let invoice = plan_for(
        &lab,
        KnowledgeMode::Auto,
        "Que dit la facture FAC-2026-0042 ?",
    );
    let analysis = invoice.analysis.as_ref().expect("read the question");
    assert_eq!(analysis.entities.len(), 1, "{analysis:?}");
    assert_eq!(analysis.entities[0].type_id, "identifier");
    assert_eq!(analysis.entities[0].method, "identifier");
    assert_eq!(
        invoice.reduction.documents.candidates,
        vec!["facture-mars.txt".to_string()]
    );

    // A personal one (an e-mail address) is stored as a keyed hash: the question is hashed the same
    // way, and the address is written nowhere.
    let personal = plan_for(
        &lab,
        KnowledgeMode::Auto,
        "Qui ecrit a claire.martin@exemple.fr ?",
    );
    let found = personal.analysis.as_ref().expect("read the question");
    assert_eq!(found.entities.len(), 1, "{found:?}");
    assert_eq!(found.entities[0].type_id, "identifier");
    assert_eq!(
        personal.reduction.documents.candidates,
        vec!["contact.txt".to_string()]
    );
    assert!(
        !found.entities[0].display.contains("exemple"),
        "the label of a personal identifier never carries it: {}",
        found.entities[0].display
    );
}

#[tokio::test]
async fn a_short_follow_up_carries_the_previous_answers_entities_but_a_long_question_does_not() {
    let lab = world(clinic()).await;
    let jean = entity_id(&lab, "Jean Dupont");
    let request = TargetingRequest {
        carry_over: vec![jean],
        ..TargetingRequest::default()
    };

    let short = plan_with(
        &lab,
        KnowledgeMode::Auto,
        &request,
        "Et son adresse ?",
        small_world_policy(),
    );
    let analysis = short.analysis.as_ref().unwrap();
    assert_eq!(analysis.entities.len(), 1);
    assert!(analysis.entities[0].carried_over);
    assert_eq!(analysis.entities[0].method, "carried_over");
    assert_eq!(short.reduction.documents.candidates.len(), 2);

    let long = plan_with(
        &lab,
        KnowledgeMode::Auto,
        &request,
        "Pouvez-vous me donner la liste complete des fournitures commandees le mois dernier par le cabinet ?",
        small_world_policy(),
    );
    assert!(long.analysis.as_ref().unwrap().entities.is_empty());
    assert_eq!(long.reduction.documents.skipped, Some(SkipReason::NoEntity));
}

#[tokio::test]
async fn a_name_found_in_a_question_is_matched_by_the_whole_header_free_exact_forms_only() {
    // A word that is a common word of the packs is not a name, whatever the base holds.
    let lab = world(clinic()).await;

    let plan = plan_for(
        &lab,
        KnowledgeMode::Auto,
        "Quels sont les rendez-vous de mars 2026 ?",
    );

    assert!(plan.analysis.as_ref().unwrap().entities.is_empty());
}

// --------------------------------------------------------------------------- invariants I1 and I2

#[tokio::test]
async fn reduced_scope_is_always_a_subset_of_the_selection() {
    let lab = world(clinic()).await;
    let vocabulary = lab.vocabulary();
    let everything = lab.documents();
    let mut random = Seeded(0x9E37_79B9_7F4A_7C15);
    let questions = [
        "Que dit Jean Dupont ?",
        "Que dit Claire Martin du controle ?",
        "Que disent Jean Dupont et Claire Martin ?",
        "Que contiennent les notes internes ?",
        "Que dit le Dr Dupont ?",
    ];

    for round in 0..120 {
        // A random selection of the twelve documents, from none to all.
        let selection: Vec<SelectedFile> = everything
            .iter()
            .filter(|_| random.below(100) < 20 + (round % 70))
            .cloned()
            .collect();
        let question = questions[random.below(questions.len())];
        let plan = lab.plan(
            &vocabulary,
            KnowledgeMode::Auto,
            &auto(),
            question,
            SideInput::new(&selection),
            SideInput::new(&[]),
            small_world_policy(),
        );
        let allowed = names(&paths_of(&selection));
        for path in &plan.reduction.documents.candidates {
            assert!(
                allowed.contains(path),
                "round {round}: {path} is not in the selection {allowed:?} ({question})"
            );
        }
        // And no search ever returns a passage from outside it.
        if !selection.is_empty() {
            let selected_paths = paths_of(&selection);
            let searched = targeting::search_documents(
                &lab.index,
                &plan,
                DocumentPlan::WholeFolder {
                    narrowed: true,
                    scoped_paths: &selected_paths,
                    present_paths: &selected_paths,
                },
                question,
                &embedding_of("para-0000 Docteur Jean Dupont"),
            )
            .expect("searches");
            for path in found_in(&searched) {
                assert!(
                    allowed.contains(&path),
                    "round {round}: {path} was searched"
                );
            }
        }
    }
}

#[tokio::test]
async fn the_reducer_drops_a_path_outside_the_selection_even_when_an_entity_lists_it() {
    // The second lock of I1, on hand-made input: an entity whose file list holds paths the
    // selection does not (a stale mention, a bug elsewhere) cannot make them candidates.
    let lab = world(clinic()).await;
    let selection: Vec<SelectedFile> = (1..=6)
        .map(|index| SelectedFile::new(&format!("note-0{index}.txt"), "id"))
        .collect();
    let mut random = Seeded(0xDEAD_BEEF_CAFE_F00D);
    for round in 0..200 {
        let mut listed: Vec<String> = Vec::new();
        for _ in 0..random.below(8) {
            listed.push(match random.below(2) {
                0 => format!("note-0{}.txt", 1 + random.below(6)),
                _ => format!("elsewhere-{}.txt", random.below(5)),
            });
        }
        listed.sort();
        listed.dedup();
        let analysis = QueryAnalysis {
            entities: vec![QueryEntity {
                entity_id: 1,
                display: "X".into(),
                type_id: "person".into(),
                status: EntityStatus::Active,
                confidence: 0.95,
                method: "canonical_name",
                from_chip: false,
                carried_over: false,
                document_files: listed,
                data_files: Vec::new(),
                document_selectivity: 0.1,
                data_selectivity: 0.0,
                terms: Vec::new(),
            }],
            known_documents: selection
                .iter()
                .map(|file| (file.relative_path.clone(), 1))
                .collect(),
            ..QueryAnalysis::default()
        };
        let plan = reduce(
            &lab.index,
            &analysis,
            &ReduceInput {
                mode: KnowledgeMode::Auto,
                user_widened: false,
                documents: SideInput::new(&selection),
                data: SideInput::new(&[]),
                policy: TargetingPolicy {
                    min_scope_files: 1,
                    min_reduction: 0.0,
                    ..TargetingPolicy::default()
                },
            },
        );
        let allowed = names(&paths_of(&selection));
        for path in &plan.documents.candidates {
            assert!(allowed.contains(path), "round {round}: {path}");
        }
    }
}

#[tokio::test]
async fn the_same_relative_path_in_both_folders_stays_two_sources_in_a_reduction() {
    // Invariant I6 at question time: a document and a workbook that happen to share a relative path
    // are two sources, and an entity's files on one side say nothing about the other.
    let lab = world(clinic()).await;
    let documents = lab.documents();
    let shared = "note-01.txt";
    let workbooks = vec![
        SelectedFile::new(shared, "a-workbook"),
        SelectedFile::new("other.csv", "x"),
    ];
    let analysis = QueryAnalysis {
        entities: vec![QueryEntity {
            entity_id: 1,
            display: "X".into(),
            type_id: "person".into(),
            status: EntityStatus::Active,
            confidence: 0.95,
            method: "canonical_name",
            from_chip: false,
            carried_over: false,
            // Only the workbook named `note-01.txt` mentions her; no document does.
            document_files: Vec::new(),
            data_files: vec![shared.to_string()],
            document_selectivity: 0.0,
            data_selectivity: 0.5,
            terms: Vec::new(),
        }],
        known_documents: documents
            .iter()
            .map(|file| (file.relative_path.clone(), 1))
            .collect(),
        known_data: workbooks
            .iter()
            .map(|file| (file.relative_path.clone(), 1))
            .collect(),
        ..QueryAnalysis::default()
    };

    let plan = reduce(
        &lab.index,
        &analysis,
        &ReduceInput {
            mode: KnowledgeMode::Auto,
            user_widened: false,
            documents: SideInput::new(&documents),
            data: SideInput::new(&workbooks),
            policy: small_world_policy(),
        },
    );

    assert_eq!(plan.data.candidates, vec![shared.to_string()]);
    assert!(
        plan.documents.candidates.is_empty(),
        "the document of the same name is not hers: {:?}",
        plan.documents
    );
    assert_eq!(plan.documents.skipped, Some(SkipReason::NoCandidates));
}

#[tokio::test]
async fn a_selection_of_three_files_never_shows_a_fourth_that_names_the_same_person() {
    // §83: the user selected A, B and C; D mentions the person too and is never searched.
    let lab = world(clinic()).await;
    let vocabulary = lab.vocabulary();
    let selection: Vec<SelectedFile> = lab
        .documents()
        .into_iter()
        .filter(|file| {
            ["dupont-bilan.txt", "martin-controle.txt", "note-01.txt"]
                .contains(&file.relative_path.as_str())
        })
        .collect();
    assert_eq!(selection.len(), 3);

    let plan = lab.plan(
        &vocabulary,
        KnowledgeMode::Auto,
        &auto(),
        "Que dit Jean Dupont ?",
        SideInput::new(&selection),
        SideInput::new(&[]),
        small_world_policy(),
    );

    assert_eq!(
        plan.reduction.documents.candidates,
        vec!["dupont-bilan.txt".to_string()]
    );
    let paths = paths_of(&selection);
    let searched = targeting::search_documents(
        &lab.index,
        &plan,
        DocumentPlan::WholeFolder {
            narrowed: true,
            scoped_paths: &paths,
            present_paths: &paths,
        },
        "Que dit Jean Dupont ?",
        &embedding_of("para-0000 Docteur Jean Dupont"),
    )
    .expect("searches");
    assert!(!found_in(&searched).contains("dupont-lettre.txt"));
}

#[tokio::test]
async fn a_file_that_left_the_folder_is_never_used_even_if_the_index_still_knows_it() {
    // §82: the index keeps the previous folder until the next pass. The selection is built from the
    // folder as it is now, so the old file is not in it.
    let lab = world(clinic()).await;
    lab.remove("dupont-lettre.txt");

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Jean Dupont ?");

    assert_eq!(
        plan.reduction.documents.candidates,
        vec!["dupont-bilan.txt".to_string()]
    );
    let paths = paths_of(&lab.documents());
    assert!(!paths.contains(&"dupont-lettre.txt".to_string()));
}

#[tokio::test]
async fn a_changed_file_is_not_trusted_for_what_the_base_knew_of_the_old_one() {
    // The base keyed the file by its old content. A file whose content changed since is unknown to
    // the base until the next Analyse, so it is kept in the search rather than dropped.
    let lab = world(clinic()).await;
    lab.write(
        "note-05.txt",
        &doc(
            5005,
            "Docteur Jean Dupont a ete ajoute a cette note apres coup.",
        ),
    );

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Jean Dupont ?");

    let documents = &plan.reduction.documents;
    assert!(
        documents.kept_unread.contains(&"note-05.txt".to_string()),
        "{documents:?}"
    );
    assert!(documents.candidates.contains(&"note-05.txt".to_string()));
}

#[tokio::test]
async fn a_named_file_is_never_reduced() {
    let lab = world(clinic()).await;
    let vocabulary = lab.vocabulary();
    let documents = lab.documents();
    let one = vec![SelectedFile::new(
        "dupont-bilan.txt",
        &documents
            .iter()
            .find(|file| file.relative_path == "dupont-bilan.txt")
            .unwrap()
            .content_id,
    )];

    // The folder router found a named file: the documents side is blocked, whatever the base knows
    // and even when the user picked a chip.
    let jean = entity_id(&lab, "Jean Dupont");
    for request in [auto(), chips(&[jean])] {
        let plan = lab.plan(
            &vocabulary,
            KnowledgeMode::Auto,
            &request,
            "Que dit dupont-bilan.txt sur Jean Dupont ?",
            SideInput::new(&one).blocked_by(SkipReason::NamedFile),
            SideInput::new(&[]),
            small_world_policy(),
        );
        assert_eq!(
            plan.reduction.documents.skipped,
            Some(SkipReason::NamedFile)
        );
        assert!(plan.reduction.documents.candidates.is_empty());
        assert!(plan.applied_documents().is_none());
        assert!(
            plan.analysis.is_none(),
            "a settled question is not even read"
        );
    }

    // And the search for one named file stays on that file.
    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Jean Dupont ?");
    let searched = targeting::search_documents(
        &lab.index,
        &plan,
        DocumentPlan::OneFile("dupont-lettre.txt"),
        "Que dit Jean Dupont ?",
        &embedding_of("para-0000 Docteur Jean Dupont"),
    )
    .expect("searches");
    assert_eq!(
        found_in(&searched),
        names(&["dupont-lettre.txt".to_string()])
    );
    assert!(!searched.reduced);
}

// ------------------------------------------------------------------------------- skip reasons

#[tokio::test]
async fn off_reads_nothing_and_reduces_nothing() {
    let lab = world(clinic()).await;
    let before = snapshot(&lab.side());

    let plan = plan_for(&lab, KnowledgeMode::Off, "Que dit Jean Dupont ?");

    assert_eq!(plan.reduction.documents.skipped, Some(SkipReason::Off));
    assert!(plan.analysis.is_none(), "off is no lookup at all");
    assert!(plan.report(false).is_none());
    assert_eq!(snapshot(&lab.side()), before);
}

#[tokio::test]
async fn the_widen_action_runs_the_old_path() {
    let lab = world(clinic()).await;
    let request = TargetingRequest {
        mode: RequestMode::Off,
        ..TargetingRequest::default()
    };

    let plan = plan_with(
        &lab,
        KnowledgeMode::Auto,
        &request,
        "Que dit Jean Dupont ?",
        small_world_policy(),
    );

    assert_eq!(
        plan.reduction.documents.skipped,
        Some(SkipReason::UserWidened)
    );
    assert!(plan.applied_documents().is_none());
    assert!(plan.extra_lexical_terms().is_empty(), "no expansion either");
}

#[tokio::test]
async fn a_small_selection_is_left_alone_unless_the_user_picked_a_chip() {
    let lab = world(clinic()).await;
    let defaults = TargetingPolicy::default();
    assert_eq!(defaults.min_scope_files, 8);
    let vocabulary = lab.vocabulary();
    let five: Vec<SelectedFile> = lab.documents().into_iter().take(5).collect();
    let jean = entity_id(&lab, "Jean Dupont");

    let plan = lab.plan(
        &vocabulary,
        KnowledgeMode::Auto,
        &auto(),
        "Que dit Jean Dupont ?",
        SideInput::new(&five),
        SideInput::new(&[]),
        defaults,
    );
    assert_eq!(
        plan.reduction.documents.skipped,
        Some(SkipReason::SmallScope)
    );

    let picked = lab.plan(
        &vocabulary,
        KnowledgeMode::Suggest,
        &chips(&[jean]),
        "Que dit Jean Dupont ?",
        SideInput::new(&five),
        SideInput::new(&[]),
        defaults,
    );
    assert!(
        picked.reduction.documents.is_applied(),
        "{:?}",
        picked.reduction.documents
    );
    assert!(picked.reduction.documents.chip_driven);
    assert_eq!(picked.requested_by(), "chip");
}

#[tokio::test]
async fn the_users_own_name_in_every_file_never_drives_a_reduction() {
    // Dr Dupont signs every letter: he is in 11 of the 12 files, so he names none in particular.
    let mut files = Vec::new();
    for index in 0..11 {
        files.push((
            format!("lettre-{index:02}.txt"),
            doc(
                index,
                &format!("Docteur Jean Dupont. Lettre numero {index} adressee a un confrere."),
            ),
        ));
    }
    files.push(filler(1));
    let lab = world(files).await;

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Jean Dupont ?");

    let analysis = plan.analysis.as_ref().unwrap();
    assert!(analysis.entities[0].document_selectivity > 0.6);
    assert_eq!(
        plan.reduction.documents.skipped,
        Some(SkipReason::NoDiscriminatingEntity)
    );
    assert!(plan.applied_documents().is_none());
}

#[tokio::test]
async fn a_reduction_that_would_save_too_little_is_not_worth_it() {
    // Dupont is in 8 of 10 files: allowed by a loose selectivity ceiling, but dropping 2 files is
    // not the 30 % the setting asks for.
    let mut files = Vec::new();
    for index in 0..8 {
        files.push((
            format!("lettre-{index:02}.txt"),
            doc(
                index,
                &format!("Docteur Jean Dupont. Lettre numero {index}."),
            ),
        ));
    }
    files.push(filler(1));
    files.push(filler(2));
    let lab = world(files).await;

    let plan = plan_with(
        &lab,
        KnowledgeMode::Auto,
        &auto(),
        "Que dit Jean Dupont ?",
        TargetingPolicy {
            max_selectivity: 0.9,
            ..small_world_policy()
        },
    );

    assert_eq!(
        plan.reduction.documents.skipped,
        Some(SkipReason::Insignificant)
    );
}

#[tokio::test]
async fn zero_candidates_fall_back_to_the_full_selection() {
    // The person is known, and only a workbook holds her: no document side to reduce. The
    // documents are searched whole and the question is still answered (§32).
    let mut lab = world(clinic()).await;
    lab.write_data(
        "rdv.csv",
        "date;patient;medecin traitant;duree_min\n02/03/2026;Alice Archer;Dr Bruno Baker;20\n",
    );
    lab.analyse_data();
    let vocabulary = lab.vocabulary();
    let documents = lab.documents();

    let plan = lab.plan(
        &vocabulary,
        KnowledgeMode::Auto,
        &auto(),
        "Que dit Alice Archer ?",
        SideInput::new(&documents),
        SideInput::new(&[]),
        small_world_policy(),
    );
    let analysis = plan.analysis.as_ref().unwrap();
    assert!(
        analysis.entities.is_empty(),
        "outside the documents she is not a name"
    );
    assert_eq!(plan.reduction.documents.skipped, Some(SkipReason::NoEntity));

    // With a chip for a person the documents do not mention: nothing to reduce to.
    let alice = entity_id(&lab, "Alice Archer");
    let picked = plan_with(
        &lab,
        KnowledgeMode::Auto,
        &chips(&[alice]),
        "Que dit-elle ?",
        small_world_policy(),
    );
    assert_eq!(
        picked.reduction.documents.skipped,
        Some(SkipReason::NoCandidates)
    );
    let searched = search(
        &lab,
        &picked,
        "Que dit-elle ?",
        &embedding_of("para-0000 Docteur Jean Dupont"),
    );
    assert!(!searched.reduced && !searched.widened);
    assert!(
        !searched.outcome.evidence.is_empty(),
        "the old path still answers"
    );
    clean(&lab);
}

#[tokio::test]
async fn a_knowledge_lookup_failure_falls_back() {
    let lab = world(clinic()).await;
    // A read the question needs breaks: the view that follows merges.
    lab.side()
        .execute_batch("DROP VIEW kb_effective_entity;")
        .expect("breaks the base");

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Jean Dupont ?");

    assert_eq!(
        plan.reduction.documents.skipped,
        Some(SkipReason::LookupError)
    );
    assert!(plan.applied_documents().is_none());
    // The search that follows is the old one on the full selection, and it answers.
    let searched = search(
        &lab,
        &plan,
        "Que dit Jean Dupont ?",
        &embedding_of("para-0000 Docteur Jean Dupont"),
    );
    assert!(!searched.reduced);
    assert!(!searched.outcome.evidence.is_empty());
}

#[tokio::test]
async fn a_writer_holding_the_database_does_not_stop_a_question() {
    // An Analyse running elsewhere holds the write lock: a question reads around it.
    let lab = world(clinic()).await;
    let documents = lab.documents();
    let vocabulary = lab.vocabulary();
    let writer = lab.side();
    writer
        .execute_batch("BEGIN IMMEDIATE;")
        .expect("holds the write lock");

    let plan = lab.plan(
        &vocabulary,
        KnowledgeMode::Auto,
        &auto(),
        "Que dit Jean Dupont ?",
        SideInput::new(&documents),
        SideInput::new(&[]),
        small_world_policy(),
    );
    let paths = paths_of(&documents);
    let searched = targeting::search_documents(
        &lab.index,
        &plan,
        DocumentPlan::WholeFolder {
            narrowed: true,
            scoped_paths: &paths,
            present_paths: &paths,
        },
        "Que dit Jean Dupont ?",
        &embedding_of("para-0000 Docteur Jean Dupont"),
    )
    .expect("searches");

    assert!(
        plan.reduction.documents.is_applied(),
        "{:?}",
        plan.reduction.documents
    );
    assert!(!searched.outcome.evidence.is_empty());
    writer.execute_batch("ROLLBACK;").expect("releases it");
}

// -------------------------------------------------------------------------------- modes

#[tokio::test]
async fn suggest_searches_exactly_as_off_does_and_only_measures() {
    let lab = world(clinic()).await;
    let question = "Que dit Jean Dupont du bilan ?";
    let embedding = embedding_of("para-0000 Docteur Jean Dupont");

    let off = plan_for(&lab, KnowledgeMode::Off, question);
    let suggest = plan_for(&lab, KnowledgeMode::Suggest, question);
    let from_off = search(&lab, &off, question, &embedding);
    let from_suggest = search(&lab, &suggest, question, &embedding);

    assert_eq!(suggest.reduction.documents.outcome, Outcome::Shadow);
    assert!(suggest.reduction.would_apply(), "it would have reduced");
    assert!(suggest.applied_documents().is_none());
    assert!(
        suggest.extra_lexical_terms().is_empty(),
        "no expansion in suggest"
    );
    assert_eq!(
        from_suggest.outcome, from_off.outcome,
        "the same evidence, in the same order, with the same scores"
    );
    assert!(!from_suggest.reduced && !from_suggest.widened);
    assert!(suggest.report(false).is_none(), "nothing changes on screen");

    // And it records what it would have done.
    let mut measured = RetrievalDiagnostics::default();
    targeting::note_documents(&lab.index, &suggest, &from_suggest, &mut measured);
    let recorded = measured.targeting.expect("measured");
    assert_eq!(recorded.mode, "suggest");
    assert_eq!(recorded.outcome, "shadow");
    assert!(recorded.shadow.would_apply);
    assert_eq!(recorded.candidate_documents, 2);
    assert_eq!(recorded.reduction_percent, 83);
}

#[tokio::test]
async fn auto_searches_only_the_candidates_and_says_so() {
    let lab = world(clinic()).await;
    let question = "Que dit Jean Dupont du bilan ?";
    let plan = plan_for(&lab, KnowledgeMode::Auto, question);

    let searched = search(
        &lab,
        &plan,
        question,
        &embedding_of("para-0000 Docteur Jean Dupont"),
    );

    assert!(searched.reduced && !searched.widened);
    assert_eq!(
        found_in(&searched),
        names(&["dupont-bilan.txt".into(), "dupont-lettre.txt".into()])
    );
    // 12 selected, 2 searched.
    assert!(searched.outcome.chunks_considered <= 2);
    let report = plan.report(false).expect("something to tell");
    assert_eq!(report.requested_by, "auto");
    assert_eq!(report.widened, "none");
    assert_eq!(report.selected_documents, 12);
    assert_eq!(report.candidate_documents, 2);
    assert_eq!(report.reduction_percent, 83);
    assert_eq!(report.strategy, "entity_files");
    assert_eq!(report.entities[0].display, "Jean Dupont");
}

#[tokio::test]
async fn a_chip_reduces_even_in_suggest() {
    let lab = world(clinic()).await;
    let claire = entity_id(&lab, "Claire Martin");
    let plan = plan_with(
        &lab,
        KnowledgeMode::Suggest,
        &chips(&[claire]),
        "Que dit-elle du controle ?",
        small_world_policy(),
    );

    let searched = search(
        &lab,
        &plan,
        "Que dit-elle du controle ?",
        &embedding_of("para-9000 Madame Claire Martin"),
    );

    assert!(plan.reduction.documents.is_applied());
    assert_eq!(plan.requested_by(), "chip");
    assert!(searched.reduced);
    assert_eq!(found_in(&searched), names(&["martin-controle.txt".into()]));
    assert_eq!(plan.report(false).unwrap().requested_by, "chip");
}

#[tokio::test]
async fn the_other_spellings_of_a_name_are_searched_in_auto_only() {
    let mut files = clinic();
    files.push((
        "rappel.txt".into(),
        doc(0, "Rappel : M. Dupont est attendu mardi pour le suivi."),
    ));
    let lab = world(files).await;

    let auto_plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Jean Dupont ?");
    let suggest_plan = plan_for(&lab, KnowledgeMode::Suggest, "Que dit Jean Dupont ?");

    let terms = auto_plan.extra_lexical_terms();
    assert!(terms.contains(&"jean dupont".to_string()), "{terms:?}");
    assert!(terms.contains(&"dupont".to_string()), "{terms:?}");
    assert!(suggest_plan.extra_lexical_terms().is_empty());
}

// ------------------------------------------------------------------------------- widening

#[tokio::test]
async fn a_reduced_search_that_finds_nothing_is_repeated_on_the_whole_selection() {
    // The question is about Dr Dupont, whose files hold vectors unlike the question's; the passage
    // that answers is in a file that never names him.
    let lab = world(clinic()).await;
    let jean = entity_id(&lab, "Jean Dupont");
    let plan = plan_with(
        &lab,
        KnowledgeMode::Suggest,
        &chips(&[jean]),
        "ce que dit la consultation",
        small_world_policy(),
    );
    assert!(plan.reduction.documents.is_applied());
    let embedding =
        embedding_of("para-9000 Madame Claire Martin a ete recue pour une consultation");

    let searched = search(&lab, &plan, "ce que dit la consultation", &embedding);

    assert!(
        searched.widened,
        "the reduced search found nothing above the noise floor"
    );
    assert!(!searched.reduced);
    assert!(found_in(&searched).contains("martin-controle.txt"));
    let mut measured = RetrievalDiagnostics::default();
    targeting::note_documents(&lab.index, &plan, &searched, &mut measured);
    assert!(measured.targeting.as_ref().unwrap().widened);
    assert_eq!(plan.report(true).unwrap().widened, "auto");
}

#[tokio::test]
async fn a_reduced_search_with_only_a_weak_match_is_widened() {
    // The candidates hold the name, so the lexical bonus brings them in, but with no vector support:
    // the best score (0.2) is under the widening threshold (0.30).
    let lab = world(clinic()).await;
    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Jean Dupont ?");
    let embedding =
        embedding_of("para-9000 Madame Claire Martin a ete recue pour une consultation");

    let searched = search(&lab, &plan, "Que dit Jean Dupont ?", &embedding);

    assert!(searched.widened);
    assert!(
        searched
            .outcome
            .evidence
            .iter()
            .any(|item| item.relative_path == "martin-controle.txt"),
        "the full selection holds the passage that answers"
    );
}

#[tokio::test]
async fn a_name_match_does_not_hide_a_distant_passage() {
    // The reduced set was chosen for the name it holds, so its excerpts always carry the lexical
    // bonus (0.2). Meaning alone says how close they are to the question: here 0.5, under the 0.65
    // threshold, although 0.5 + 0.2 would pass it.
    let lab = world(clinic()).await;
    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Jean Dupont ?");

    let searched = search(&lab, &plan, "Que dit Jean Dupont ?", &[0.866, 0.5]);

    assert!(searched.widened, "the closest passage is far in meaning");
    assert!(
        found_in(&searched)
            .iter()
            .any(|file| !file.starts_with("dupont-")),
        "the whole selection was searched: {:?}",
        found_in(&searched)
    );
}

#[tokio::test]
async fn a_good_reduced_search_is_not_widened() {
    let lab = world(clinic()).await;
    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Jean Dupont du bilan ?");

    let searched = search(
        &lab,
        &plan,
        "Que dit Jean Dupont du bilan ?",
        &embedding_of("para-0000 Docteur Jean Dupont recommande un bilan sanguin complet pour le suivi annuel."),
    );

    assert!(searched.reduced && !searched.widened);
}

#[tokio::test]
async fn the_widen_action_gives_the_evidence_the_old_path_gives() {
    let lab = world(clinic()).await;
    let question = "Que dit Jean Dupont du bilan ?";
    let embedding = embedding_of(
        "para-0000 Docteur Jean Dupont recommande un bilan sanguin complet pour le suivi annuel.",
    );
    let widen = TargetingRequest {
        mode: RequestMode::Off,
        ..TargetingRequest::default()
    };

    let widened = plan_with(
        &lab,
        KnowledgeMode::Auto,
        &widen,
        question,
        small_world_policy(),
    );
    let old = plan_for(&lab, KnowledgeMode::Off, question);

    assert_eq!(
        search(&lab, &widened, question, &embedding).outcome,
        search(&lab, &old, question, &embedding).outcome
    );
}

#[tokio::test]
async fn the_reduced_search_and_the_full_one_agree_on_the_passage_that_answers() {
    let lab = world(clinic()).await;
    let question = "Que dit Jean Dupont du bilan ?";
    let embedding = embedding_of(
        "para-0000 Docteur Jean Dupont recommande un bilan sanguin complet pour le suivi annuel.",
    );

    let reduced = search(
        &lab,
        &plan_for(&lab, KnowledgeMode::Auto, question),
        question,
        &embedding,
    );
    let full = search(
        &lab,
        &plan_for(&lab, KnowledgeMode::Off, question),
        question,
        &embedding,
    );

    assert!(reduced.reduced);
    let top_reduced = &reduced.outcome.evidence[0];
    let top_full = &full.outcome.evidence[0];
    assert_eq!(top_reduced.chunk_id, top_full.chunk_id);
    // The citation is the one it has always been: file, page and section of the stored chunk.
    assert_eq!(top_reduced.relative_path, top_full.relative_path);
    assert_eq!(top_reduced.page_number, top_full.page_number);
    assert_eq!(top_reduced.section, top_full.section);
    assert_eq!(top_reduced.text, top_full.text);
}

#[tokio::test]
async fn each_document_questions_are_ordered_by_what_the_question_names() {
    let lab = world(clinic()).await;
    let question = "Resume chaque document sur Jean Dupont";
    let all_paths = paths_of(&lab.documents());
    let plan = plan_for(&lab, KnowledgeMode::Auto, question);

    let searched = targeting::search_documents(
        &lab.index,
        &plan,
        DocumentPlan::EveryDocument(&all_paths),
        question,
        &embedding_of("para-0000 Docteur Jean Dupont"),
    )
    .expect("searches");

    assert!(searched.reduced);
    let files: Vec<&str> = searched
        .outcome
        .evidence
        .iter()
        .map(|item| item.relative_path.as_str())
        .collect();
    assert_eq!(files.len(), 2);
    assert!(files.iter().all(|file| file.starts_with("dupont-")));
}

// ---------------------------------------------------------------------------- the knowledge base is read-only

// ------------------------------------------------------------------------------ diagnostics

#[tokio::test]
async fn a_recorded_question_holds_counts_and_codes_and_the_ring_stays_bounded() {
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
    measured.total_ms = 40;
    targeting::note_documents(&lab.index, &plan, &searched, &mut measured);

    let connection = lab.side();
    diagnostics::record_question(&connection, 1_760_000_000, &measured).expect("records");

    let (mode, strategy, skipped, selected, candidates, cited, in_candidates, entities): (
        String,
        String,
        Option<String>,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = connection
        .query_row(
            "SELECT mode, strategy, skipped_reason, selected_files, candidate_files, cited_files,
                    cited_in_candidates, entities_found FROM kb_diagnostics",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        .expect("reads the row");
    assert_eq!((mode.as_str(), strategy.as_str()), ("auto", "entity_files"));
    assert_eq!(skipped, None);
    assert_eq!((selected, candidates, entities), (12, 2, 1));
    assert_eq!(
        cited, in_candidates,
        "everything cited was in the candidates"
    );

    // The ring keeps the last 2000.
    for _ in 0..2_050 {
        diagnostics::record_question(&connection, 1_760_000_001, &measured).expect("records");
    }
    let rows: i64 = connection
        .query_row("SELECT COUNT(*) FROM kb_diagnostics", [], |row| row.get(0))
        .unwrap();
    assert_eq!(rows, diagnostics::QUESTIONS_KEPT);

    // No text column of the table can hold a name: every text is a code.
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT mode, strategy, IFNULL(skipped_reason, ''), path FROM kb_diagnostics",
        )
        .unwrap();
    let texts: Vec<(String, String, String, String)> = statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .map(|row| row.unwrap())
        .collect();
    for (mode, strategy, skipped, path) in texts {
        for text in [mode, strategy, skipped, path] {
            assert!(
                text.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{text:?} is not a code"
            );
        }
    }
}

#[tokio::test]
async fn a_question_in_off_mode_leaves_no_row() {
    let lab = world(clinic()).await;
    let plan = plan_for(&lab, KnowledgeMode::Off, "Que dit Jean Dupont ?");
    let mut measured = RetrievalDiagnostics::default();
    let searched = search(
        &lab,
        &plan,
        "Que dit Jean Dupont ?",
        &embedding_of("para-0000 x"),
    );
    targeting::note_documents(&lab.index, &plan, &searched, &mut measured);

    diagnostics::record_question(&lab.side(), 1, &measured).expect("accepts");

    assert_eq!(common::knowledge::count(&lab.side(), "kb_diagnostics"), 0);
}

#[tokio::test]
async fn the_aggregates_add_up_what_was_recorded() {
    let lab = world(clinic()).await;
    let connection = lab.side();
    let embedding = embedding_of("para-0000 Docteur Jean Dupont");
    for question in [
        "Que dit Jean Dupont du bilan ?",
        "Que dit Jean Dupont de la lettre ?",
        "Que contiennent les notes internes ?",
    ] {
        let plan = plan_for(&lab, KnowledgeMode::Suggest, question);
        let searched = search(&lab, &plan, question, &embedding);
        let mut measured = RetrievalDiagnostics::default();
        measured.selected_documents = 12;
        targeting::note_documents(&lab.index, &plan, &searched, &mut measured);
        diagnostics::record_question(&connection, 5, &measured).expect("records");
    }

    let totals = diagnostics::aggregate(&connection, 100).expect("adds up");

    assert_eq!(totals.questions, 3);
    assert_eq!(totals.with_entities, 2);
    assert_eq!(totals.would_apply, 2);
    assert_eq!(totals.applied, 0, "suggest applies nothing");
    assert_eq!(totals.median_reduction_percent, Some(83));
    assert_eq!(totals.skipped_by_reason.get("no_entity"), Some(&1));
    assert!(totals.recall_overall.unwrap() > 0.99);
    assert!(totals.recall_when_would_apply.is_some());
}

#[tokio::test]
async fn a_miss_is_classified_by_why_the_reduced_set_left_the_file_out() {
    // The answer cites a file the reduction would have left out: the base holds no mention of Dupont
    // in it (an extraction gap), so the miss is `no_mention`.
    let lab = world(clinic()).await;
    let plan = plan_for(&lab, KnowledgeMode::Suggest, "Que dit Jean Dupont ?");
    // The full search, as suggest runs it, with an embedding that makes Claire Martin's file win.
    let searched = search(
        &lab,
        &plan,
        "Que dit Jean Dupont ?",
        &embedding_of("para-9000 Madame Claire Martin a ete recue pour une consultation"),
    );
    let mut measured = RetrievalDiagnostics::default();

    targeting::note_documents(&lab.index, &plan, &searched, &mut measured);

    let shadow = measured.targeting.unwrap().shadow;
    assert!(shadow.would_apply);
    assert!(shadow.cited > shadow.cited_in_candidates);
    assert!(shadow.miss_no_mention >= 1, "{shadow:?}");
}

#[tokio::test]
async fn the_diagnostics_table_made_by_lot_one_gains_its_columns_without_losing_rows() {
    let lab = world(clinic()).await;
    let connection = lab.side();
    // The table as lot 1 created it, with a row in it.
    connection
        .execute_batch(
            "DROP TABLE kb_diagnostics;
             CREATE TABLE kb_diagnostics (
                id INTEGER PRIMARY KEY, at INTEGER NOT NULL, mode TEXT NOT NULL, strategy TEXT NOT NULL,
                skipped_reason TEXT, selected_files INTEGER NOT NULL DEFAULT 0,
                candidate_files INTEGER NOT NULL DEFAULT 0, selected_data INTEGER NOT NULL DEFAULT 0,
                candidate_data INTEGER NOT NULL DEFAULT 0, cited_files INTEGER NOT NULL DEFAULT 0,
                cited_in_candidates INTEGER NOT NULL DEFAULT 0, entities_found INTEGER NOT NULL DEFAULT 0,
                widened INTEGER NOT NULL DEFAULT 0, knowledge_ms INTEGER NOT NULL DEFAULT 0,
                retrieval_ms INTEGER NOT NULL DEFAULT 0, total_ms INTEGER NOT NULL DEFAULT 0);
             INSERT INTO kb_diagnostics (at, mode, strategy) VALUES (1, 'suggest', 'none');",
        )
        .unwrap();

    kb::ensure_schema(&connection).expect("migrates");
    kb::ensure_schema(&connection).expect("and again, harmlessly");

    let (rows, path): (i64, String) = connection
        .query_row(
            "SELECT COUNT(*), MAX(path) FROM kb_diagnostics",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((rows, path.as_str()), (1, ""));
}

// ---------------------------------------------------------------------------------- workbooks

fn appointments() -> &'static str {
    "date;patient;medecin traitant;duree_min\n\
     02/03/2026;Alice Archer;Dr Bruno Baker;20\n\
     03/03/2026;Carole Clark;Dr Bruno Baker;25\n\
     04/03/2026;Alice Archer;Dr Denis Dumas;20\n\
     05/03/2026;Emma Evans;Dr Bruno Baker;45\n"
}

fn invoices() -> &'static str {
    "date;fournisseur;montant\n\
     02/03/2026;MedSupply;450\n\
     03/03/2026;PapeterieDuParc;180\n\
     04/03/2026;MedSupply;620\n"
}

fn workbook_lab() -> Lab {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data("rdv.csv", appointments());
    lab.write_data("factures.csv", invoices());
    lab.analyse_data();
    lab
}

fn plan_workbooks(lab: &Lab, mode: KnowledgeMode, question: &str, named: bool) -> TargetingPlan {
    let vocabulary = lab.vocabulary();
    let workbooks = lab.workbooks();
    assert_eq!(workbooks.len(), 2);
    let data = match named {
        true => SideInput::new(&workbooks).blocked_by(SkipReason::NamedFile),
        false => SideInput::new(&workbooks),
    };
    lab.plan(
        &vocabulary,
        mode,
        &auto(),
        question,
        SideInput::new(&[]),
        data,
        small_world_policy(),
    )
}

fn ask(lab: &Lab, selection: &ScopeMode, question: &str) -> TabularAnswer {
    let folder = lab.data_folder();
    match tabular_answer::prepare(question, &folder, selection, "fr-FR", &lab.index, false)
        .expect("prepares")
    {
        tabular_answer::PendingAnswer::Done(answer) => answer,
        tabular_answer::PendingAnswer::TryModel { fallback, .. } => fallback,
    }
}

#[test]
fn in_suggest_the_which_workbook_buttons_are_ordered_and_annotated_and_nothing_else_changes() {
    let lab = workbook_lab();
    let question = "Combien de lignes pour Alice Archer ?";
    let plan = plan_workbooks(&lab, KnowledgeMode::Suggest, question, false);

    assert_eq!(plan.reduction.data.outcome, Outcome::Shadow);
    assert!(plan.applied_workbooks().is_none());
    let plain = ask(&lab, &ScopeMode::WholeFolder, question);
    let TabularAnswer::WhichWorkbook { candidates, hints } = &plain else {
        panic!("two workbooks and no name must ask: {plain:?}");
    };
    assert_eq!(
        candidates,
        &vec!["factures.csv".to_string(), "rdv.csv".to_string()]
    );
    assert!(hints.is_empty());

    let decorated = targeting::decorate_which_workbook(plain.clone(), &plan);

    let TabularAnswer::WhichWorkbook { candidates, hints } = decorated else {
        panic!("still a question");
    };
    assert_eq!(
        candidates[0], "rdv.csv",
        "the workbook that holds her comes first"
    );
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].file, "rdv.csv");
    assert_eq!(hints[0].entities[0].display, "Alice Archer");
    assert!(plan.report(false).is_none(), "nothing is said in suggest");
}

#[test]
fn in_auto_the_only_workbook_that_holds_the_name_answers_directly() {
    let lab = workbook_lab();
    let folder = lab.data_folder();
    let question = "Combien de lignes pour Alice Archer ?";
    let plan = plan_workbooks(&lab, KnowledgeMode::Auto, question, false);

    let narrowed = targeting::narrowed_data_selection(
        &folder,
        &ScopeMode::WholeFolder,
        plan.applied_workbooks().expect("applied"),
    );
    let answer = ask(&lab, &narrowed, question);

    assert!(
        !matches!(answer, TabularAnswer::WhichWorkbook { .. }),
        "{answer:?}"
    );
    assert_eq!(
        targeting::answered_file(&answer).as_deref(),
        Some("rdv.csv")
    );
    let report = plan.report(false).expect("something to tell");
    assert_eq!(report.chosen_workbook.as_ref().unwrap().file, "rdv.csv");
    assert_eq!(report.candidate_workbooks, 1);
    assert_eq!(report.selected_workbooks, 2);
    assert_eq!(report.workbook_paths, vec!["rdv.csv".to_string()]);
}

#[test]
fn a_named_workbook_is_never_reduced() {
    let lab = workbook_lab();
    let folder = lab.data_folder();
    let question = "Combien de lignes dans factures.csv pour Alice Archer ?";
    assert!(tabular_answer::names_a_workbook(
        question,
        &folder,
        &ScopeMode::WholeFolder
    ));

    let plan = plan_workbooks(&lab, KnowledgeMode::Auto, question, true);

    assert_eq!(plan.reduction.data.skipped, Some(SkipReason::NamedFile));
    assert!(plan.applied_workbooks().is_none());
    assert!(!tabular_answer::names_a_workbook(
        "Combien de lignes pour Alice Archer ?",
        &folder,
        &ScopeMode::WholeFolder
    ));
}

#[test]
fn narrowing_the_workbooks_builds_a_subset_of_the_entries_the_user_made() {
    let lab = workbook_lab();
    let folder = lab.data_folder();
    let records = folder.files().all_files().to_vec();
    let rdv = records
        .iter()
        .find(|r| r.relative_path == "rdv.csv")
        .unwrap();
    let factures = records
        .iter()
        .find(|r| r.relative_path == "factures.csv")
        .unwrap();
    let entry =
        |record: &assistant_cabinet_ai_lib::file_record::FileRecord, sheets: &[&str]| ScopeEntry {
            relative_path: record.relative_path.clone(),
            pinned_id: record.id.clone(),
            added_at: 7,
            sheet_names: sheets.iter().map(|s| s.to_string()).collect(),
        };
    let chosen = ScopeMode::Explicit(vec![entry(rdv, &["rdv"]), entry(factures, &[])]);

    // Candidates that include a workbook the user did not choose cannot add it.
    let narrowed = targeting::narrowed_data_selection(
        &folder,
        &chosen,
        &["rdv.csv".to_string(), "intrus.csv".to_string()],
    );

    let ScopeMode::Explicit(entries) = narrowed else {
        panic!("explicit");
    };
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].relative_path, "rdv.csv");
    assert_eq!(
        entries[0].sheet_names,
        vec!["rdv".to_string()],
        "the user's sheets stay"
    );
    assert_eq!(entries[0].added_at, 7);

    // A narrowing that kept nothing is no narrowing.
    let nothing = targeting::narrowed_data_selection(&folder, &chosen, &["intrus.csv".to_string()]);
    assert_eq!(nothing, chosen);
}

#[test]
fn the_engine_answers_the_same_from_the_narrowed_selection_as_from_the_user_naming_the_workbook() {
    let lab = workbook_lab();
    let folder = lab.data_folder();
    let question = "Combien de lignes pour Alice Archer ?";
    let plan = plan_workbooks(&lab, KnowledgeMode::Auto, question, false);
    let narrowed = targeting::narrowed_data_selection(
        &folder,
        &ScopeMode::WholeFolder,
        plan.applied_workbooks().unwrap(),
    );

    let reduced = ask(&lab, &narrowed, question);
    let by_name = ask(
        &lab,
        &ScopeMode::WholeFolder,
        "Combien de lignes dans rdv.csv pour Alice Archer ?",
    );

    // Same workbook, same locator, same figure: the engine and its citations are untouched.
    assert_eq!(
        serde_json::to_value(&reduced).unwrap(),
        serde_json::to_value(&by_name).unwrap()
    );
}

#[test]
fn a_user_named_sheet_the_base_did_not_hint_still_answers() {
    // The base records the sheets where a name appears, as a hint. The user's own sheet restriction
    // is never replaced by it.
    let lab = workbook_lab();
    let plan = plan_workbooks(
        &lab,
        KnowledgeMode::Auto,
        "Combien de lignes pour Alice Archer ?",
        false,
    );
    let hints = &plan.reduction.data.sheet_hints;
    assert_eq!(hints.get("rdv.csv").map(|sheets| sheets.len()), Some(1));

    let folder = lab.data_folder();
    let rdv = folder
        .files()
        .all_files()
        .iter()
        .find(|r| r.relative_path == "rdv.csv")
        .unwrap();
    let chosen = ScopeMode::Explicit(vec![ScopeEntry {
        relative_path: "rdv.csv".into(),
        pinned_id: rdv.id.clone(),
        added_at: 0,
        sheet_names: vec!["rdv".into()],
    }]);
    let narrowed = targeting::narrowed_data_selection(&folder, &chosen, &["rdv.csv".to_string()]);
    assert_eq!(narrowed, chosen);
    let answer = ask(&lab, &narrowed, "Combien de lignes ?");
    assert_eq!(
        targeting::answered_file(&answer).as_deref(),
        Some("rdv.csv")
    );
}

#[test]
fn a_workbook_the_base_has_never_read_is_kept() {
    let mut lab = workbook_lab();
    // A third workbook appears after the last Analyse: unknown to the base.
    lab.write_data(
        "nouveau.csv",
        "date;patient;duree_min\n06/03/2026;Zoe Zimmer;30\n",
    );
    lab.analyse_data_with(&mut lab.context().with_kb_version(0));
    let plan = plan_workbooks_n(
        &lab,
        KnowledgeMode::Auto,
        "Combien de lignes pour Alice Archer ?",
    );

    let data = &plan.reduction.data;
    assert!(
        data.kept_unread.contains(&"nouveau.csv".to_string()),
        "{data:?}"
    );
    assert!(data.candidates.contains(&"rdv.csv".to_string()));
    assert!(!data.candidates.contains(&"factures.csv".to_string()));
}

fn plan_workbooks_n(lab: &Lab, mode: KnowledgeMode, question: &str) -> TargetingPlan {
    let vocabulary = lab.vocabulary();
    let workbooks = lab.workbooks();
    lab.plan(
        &vocabulary,
        mode,
        &auto(),
        question,
        SideInput::new(&[]),
        SideInput::new(&workbooks),
        small_world_policy(),
    )
}

// ----------------------------------------------------------------------- both sides together

#[tokio::test]
async fn documents_and_workbooks_are_decided_independently() {
    let mut lab = world(clinic()).await;
    lab.write_data("rdv.csv", appointments());
    lab.write_data("factures.csv", invoices());
    lab.analyse_data();
    let vocabulary = lab.vocabulary();
    let documents = lab.documents();
    let workbooks = lab.workbooks();

    // A person only the documents name: documents reduced, workbooks left whole.
    let plan = lab.plan(
        &vocabulary,
        KnowledgeMode::Auto,
        &auto(),
        "Que dit Jean Dupont ?",
        SideInput::new(&documents),
        SideInput::new(&workbooks),
        small_world_policy(),
    );
    assert!(plan.reduction.documents.is_applied());
    assert_eq!(plan.reduction.data.skipped, Some(SkipReason::NoCandidates));
    assert!(plan.applied_workbooks().is_none());

    // A person only a workbook names: the other way round.
    let plan = lab.plan(
        &vocabulary,
        KnowledgeMode::Auto,
        &auto(),
        "Que dit Alice Archer ?",
        SideInput::new(&documents),
        SideInput::new(&workbooks),
        small_world_policy(),
    );
    assert!(plan.reduction.data.is_applied());
    assert!(plan.applied_documents().is_none());
    let report = plan.report(false).unwrap();
    assert_eq!(report.candidate_workbooks, 1);
    assert_eq!(report.candidate_documents, 12, "documents unchanged");
}

// ---------------------------------------------------------------- the pilot's two spellings

#[tokio::test]
async fn a_possible_match_keeps_the_files_of_the_other_spelling_in_the_search() {
    // KBD-15: the appointments write `Dupont J`, the mail merge writes `Jean Dupont`. A question
    // for the full name must not leave the appointments out.
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data(
        "rdv.csv",
        "date;patient;duree_min\n02/03/2026;Dupont J;20\n03/03/2026;Martin A;25\n",
    );
    lab.write_data(
        "publipostage.csv",
        "ID_Client,Civilite,Prenom,Nom,Email\n101,M.,Jean,Dupont,jean.dupont@exemple.fr\n\
         102,Mme,Sophie,Martin,sophie.martin@exemple.fr\n",
    );
    lab.write_data("factures.csv", invoices());
    lab.write_data("achats.csv", invoices());
    lab.analyse_data();
    let vocabulary = lab.vocabulary();
    let workbooks = lab.workbooks();
    assert_eq!(workbooks.len(), 4);

    let plan = lab.plan(
        &vocabulary,
        KnowledgeMode::Auto,
        &auto(),
        "Combien de rendez-vous pour Jean Dupont ?",
        SideInput::new(&[]),
        SideInput::new(&workbooks),
        small_world_policy(),
    );

    let data = &plan.reduction.data;
    assert!(data.is_applied(), "{data:?}");
    assert!(data.candidates.contains(&"publipostage.csv".to_string()));
    assert!(
        data.candidates.contains(&"rdv.csv".to_string()),
        "the other spelling of the same person: {data:?}"
    );
    assert!(data.from_possible_matches.contains(&"rdv.csv".to_string()));
    assert!(!data.candidates.contains(&"factures.csv".to_string()));
}

// --------------------------------------------------------------- several people called Dupont

fn duponts() -> Vec<(String, String)> {
    let mut files = vec![
        (
            "jean.txt".to_string(),
            doc(1, "Docteur Jean Dupont recoit le lundi."),
        ),
        (
            "dupontet.txt".to_string(),
            doc(2, "Dossier de Jean Dupontet, rendez-vous reporte."),
        ),
        (
            "jean-pierre.txt".to_string(),
            doc(3, "Monsieur Jean-Pierre Dupont a ete recu ce matin."),
        ),
        (
            "marie.txt".to_string(),
            doc(4, "Madame Marie Dupont a ete recue ce matin."),
        ),
        (
            "ordonnance.txt".to_string(),
            doc(5, "Dr Jean Dupont a signe l'ordonnance."),
        ),
    ];
    files.extend((1..=5).map(filler));
    files
}

#[tokio::test]
async fn people_who_share_a_surname_are_never_taken_for_one_person() {
    let lab = world(duponts()).await;
    let candidates_for = |question: &str| {
        let plan = plan_for(&lab, KnowledgeMode::Auto, question);
        (plan.reduction.documents.candidates.clone(), plan)
    };

    // The full name is exact, and only its own files are searched: not Dupontet, not Marie, not
    // Jean-Pierre.
    let (jean, _) = candidates_for("Que dit Jean Dupont ?");
    assert_eq!(
        jean,
        vec!["jean.txt".to_string(), "ordonnance.txt".to_string()]
    );
    let (marie, _) = candidates_for("Que dit Marie Dupont ?");
    assert_eq!(marie, vec!["marie.txt".to_string()]);
    let (jean_pierre, _) = candidates_for("Que dit Jean-Pierre Dupont ?");
    assert_eq!(jean_pierre, vec!["jean-pierre.txt".to_string()]);

    // A title and a surname, a surname, an initial and a surname: several Duponts answer, so the
    // user is asked and nothing is narrowed.
    for question in [
        "Que dit le Dr Dupont ?",
        "Que dit Dupont ?",
        "Que dit J. Dupont ?",
    ] {
        let (candidates, plan) = candidates_for(question);
        assert!(candidates.is_empty(), "{question}");
        assert_eq!(
            plan.reduction.documents.skipped,
            Some(SkipReason::Ambiguous),
            "{question}"
        );
        assert!(!plan.analysis.as_ref().unwrap().ambiguities.is_empty());
    }

    // A guess (an untitled name met once) can be recognised but never narrows a scope.
    let (_, guess) = candidates_for("Que dit Jean Dupontet ?");
    let entity = &guess.analysis.as_ref().unwrap().entities[0];
    assert!(entity.confidence <= 0.40, "{}", entity.confidence);
    assert_eq!(
        guess.reduction.documents.skipped,
        Some(SkipReason::NoDiscriminatingEntity)
    );
}

// ------------------------------------------------------------------------- the human test's files

/// Prints what each question of the human test makes of the thirty invented documents that
/// `lot06_helper.py write-fixtures <folder>` writes, with the settings the application ships.
///
/// ```text
/// python docs\test-reports\knowledge-base-pass-1\human-tests\lot06_helper.py write-fixtures <folder>
/// set ACAI_LOT06_DIR=<folder>
/// cargo test --test knowledge_targeting print_the_plan_for_the_human_test -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore]
async fn print_the_plan_for_the_human_test() {
    let Ok(folder) = std::env::var("ACAI_LOT06_DIR") else {
        println!("set ACAI_LOT06_DIR to a folder written by lot06_helper.py write-fixtures");
        return;
    };
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    for entry in std::fs::read_dir(&folder).expect("reads the folder") {
        let path = entry.expect("entry").path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        lab.write(&name, &std::fs::read_to_string(&path).expect("reads"));
    }
    let summary = lab.analyse().await;
    println!("summary: {:?}", summary.knowledge);
    for row in common::knowledge::entities(&lab.side()) {
        println!(
            "entity {} | {} | {}",
            row.type_id, row.canonical, row.status
        );
    }
    let defaults = TargetingPolicy::default();
    for question in [
        "Que dit le Docteur Jean Dupont du bilan sanguin ?",
        "Que dit le Dr Martin ?",
        "Que dit Madame Claire Martin du contrôle ?",
        "Que contiennent les notes internes ?",
        "Le Docteur Dupont a-t-il parlé de la photocopieuse ?",
    ] {
        for mode in [KnowledgeMode::Suggest, KnowledgeMode::Auto] {
            let plan = plan_with(&lab, mode, &auto(), question, defaults);
            let documents = &plan.reduction.documents;
            println!("{question}");
            println!(
                "   {mode:?}: {} selected, outcome {:?}, reason {:?}, {} candidates, {}% cut",
                documents.selected,
                documents.outcome,
                documents.skipped.map(|reason| reason.as_code()),
                documents.candidates.len(),
                documents.reduction_percent(),
            );
            if documents.has_candidates() {
                println!("   candidates {:?}", documents.candidates);
                println!("   by the lexical net {:?}", documents.added_by_lexical);
            }
            // What the application records for this question: a row of counts and codes.
            let searched = search(&lab, &plan, question, &embedding_of("para-0000 x"));
            let mut measured = RetrievalDiagnostics::default();
            measured.selected_documents = documents.selected;
            targeting::note_documents(&lab.index, &plan, &searched, &mut measured);
            diagnostics::record_question(&lab.side(), 1_760_000_000, &measured).expect("records");
            let analysis = plan.analysis.as_ref().expect("read");
            for entity in &analysis.entities {
                println!(
                    "   names {} ({:.2}, {}, in {} of {} files)",
                    entity.display,
                    entity.confidence,
                    entity.method,
                    entity.document_files.len(),
                    documents.selected
                );
            }
            for ambiguity in &analysis.ambiguities {
                println!(
                    "   asks about {:?}",
                    ambiguity
                        .candidates
                        .iter()
                        .map(|brief| brief.display.clone())
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    // The index with its rows, for the read-only script of the human test to be run against.
    if let Ok(out) = std::env::var("ACAI_LOT06_INDEX_OUT") {
        std::fs::copy(lab.index_path(), &out).expect("copies the index");
        println!("index copied to {out}");
    }
}

// ---------------------------------------------------------- the pilot's two fictional workbooks

/// The human test asks a question about a supplier with both workbooks of the pilot selected. The
/// files are the repository's own fictional fixtures.
fn pilot_workbooks() -> Lab {
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs/test-reports/human-acceptance-pass-1/fixtures/data");
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    for name in ["rdv-mars-2026.xlsx", "factures-fournisseurs-2026.xlsx"] {
        std::fs::copy(fixtures.join(name), lab.data.path().join(name)).expect("copies");
    }
    lab.analyse_data();
    lab
}

#[test]
fn a_supplier_named_in_the_question_points_at_the_invoices_among_the_pilots_workbooks() {
    let lab = pilot_workbooks();
    let folder = lab.data_folder();
    let question = "Combien de lignes pour MedSupply ?";

    // suggest: the engine asks which workbook, and the base only orders the buttons.
    let suggest = plan_workbooks_n(&lab, KnowledgeMode::Suggest, question);
    let asked = ask(&lab, &ScopeMode::WholeFolder, question);
    let TabularAnswer::WhichWorkbook { candidates, .. } = &asked else {
        panic!("two workbooks and no name must ask: {asked:?}");
    };
    assert_eq!(
        candidates[0], "factures-fournisseurs-2026.xlsx",
        "alphabetical order"
    );
    let ordered = targeting::decorate_which_workbook(asked.clone(), &suggest);
    let TabularAnswer::WhichWorkbook { hints, .. } = ordered else {
        panic!("still a question");
    };
    assert_eq!(hints[0].file, "factures-fournisseurs-2026.xlsx");
    assert_eq!(hints[0].entities[0].display, "MedSupply");

    // auto: the invoices alone, and the engine answers.
    let auto_plan = plan_workbooks_n(&lab, KnowledgeMode::Auto, question);
    assert_eq!(
        auto_plan.applied_workbooks(),
        Some(&["factures-fournisseurs-2026.xlsx".to_string()][..])
    );
    let narrowed = targeting::narrowed_data_selection(
        &folder,
        &ScopeMode::WholeFolder,
        auto_plan.applied_workbooks().unwrap(),
    );
    let answer = ask(&lab, &narrowed, question);
    assert_eq!(
        targeting::answered_file(&answer).as_deref(),
        Some("factures-fournisseurs-2026.xlsx")
    );
    assert!(!matches!(answer, TabularAnswer::WhichWorkbook { .. }));
}

#[test]
fn a_patient_named_in_the_question_moves_the_appointments_first_and_in_auto_answers_from_them() {
    // The human test asks this question with both workbooks of the pilot selected.
    let lab = pilot_workbooks();
    let folder = lab.data_folder();
    let question = "Combien de lignes pour Dupont J ?";

    // suggest: the engine asks which workbook, as it always did (alphabetical order), and the base
    // puts the appointments first and says why.
    let suggest = plan_workbooks_n(&lab, KnowledgeMode::Suggest, question);
    let asked = ask(&lab, &ScopeMode::WholeFolder, question);
    let TabularAnswer::WhichWorkbook { candidates, .. } = &asked else {
        panic!("two workbooks and no name must ask: {asked:?}");
    };
    assert_eq!(
        candidates,
        &vec![
            "factures-fournisseurs-2026.xlsx".to_string(),
            "rdv-mars-2026.xlsx".to_string()
        ]
    );
    let TabularAnswer::WhichWorkbook { candidates, hints } =
        targeting::decorate_which_workbook(asked.clone(), &suggest)
    else {
        panic!("still a question");
    };
    assert_eq!(candidates[0], "rdv-mars-2026.xlsx");
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].entities[0].display, "Dupont J");
    assert_eq!(hints[0].sheets, vec!["RDV".to_string()]);

    // auto: the appointments alone, and the engine counts the two rows of that patient.
    let auto_plan = plan_workbooks_n(&lab, KnowledgeMode::Auto, question);
    let narrowed = targeting::narrowed_data_selection(
        &folder,
        &ScopeMode::WholeFolder,
        auto_plan.applied_workbooks().expect("applied"),
    );
    let answer = ask(&lab, &narrowed, question);
    let json = serde_json::to_value(&answer).unwrap();
    assert_eq!(json["file"], "rdv-mars-2026.xlsx");
    assert_eq!(json["value"]["value"], 2);
    assert_eq!(json["locator"]["filters"][0]["value"], "Dupont J");
}
