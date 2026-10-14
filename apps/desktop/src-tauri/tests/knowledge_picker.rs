//! Lot 7: the names the composer's `@` picker offers.
//!
//! These tests drive the real Analyse over invented files in a temporary folder, then ask
//! `knowledge::picker` for the names that begin with some letters. Every name is invented.

mod common;

use assistant_cabinet_ai_lib::knowledge::picker::{
    search_entities, EntitySearch, PickerFiles, MAX_OUTSIDE_NAMES,
};
use assistant_cabinet_ai_lib::knowledge::query::SelectedFile;

use common::corpus::{clean, clinic, doc, filler, world};
use common::knowledge::{entity_named, snapshot, Lab};

fn pick(lab: &Lab, prefix: &str, selected_documents: &[SelectedFile]) -> EntitySearch {
    pick_both(lab, prefix, selected_documents, &[])
}

fn pick_both(
    lab: &Lab,
    prefix: &str,
    selected_documents: &[SelectedFile],
    selected_workbooks: &[SelectedFile],
) -> EntitySearch {
    let analysed_documents = lab.documents();
    let analysed_workbooks = lab.workbooks();
    search_entities(
        &lab.side(),
        prefix,
        PickerFiles {
            documents: selected_documents,
            data: selected_workbooks,
        },
        PickerFiles {
            documents: &analysed_documents,
            data: &analysed_workbooks,
        },
        8,
    )
    .expect("searches")
}

fn only(lab: &Lab, names: &[&str]) -> Vec<SelectedFile> {
    lab.documents()
        .into_iter()
        .filter(|file| names.contains(&file.relative_path.as_str()))
        .collect()
}

#[tokio::test]
async fn the_picker_lists_the_names_of_the_selection_that_begin_with_the_letters_typed() {
    let lab = world(clinic()).await;
    let everything = lab.documents();

    for prefix in ["Dup", "dup", "Jean", "J", "Jean Du"] {
        let found = pick(&lab, prefix, &everything);
        let names: Vec<&str> = found.entities.iter().map(|e| e.display.as_str()).collect();
        assert_eq!(names, ["Jean Dupont"], "{prefix}");
        assert_eq!(found.entities[0].documents, 2, "{prefix}");
        assert_eq!(found.entities[0].workbooks, 0, "{prefix}");
        assert_eq!(found.entities[0].type_id, "person", "{prefix}");
    }
    let martin = pick(&lab, "Mar", &everything);
    assert_eq!(martin.entities.len(), 1);
    assert_eq!(martin.entities[0].display, "Claire Martin");
    assert_eq!(martin.entities[0].documents, 1);
    clean(&lab);
}

#[tokio::test]
async fn a_name_counts_only_the_selected_files_that_hold_it() {
    let lab = world(clinic()).await;
    let one = only(&lab, &["dupont-bilan.txt", "martin-controle.txt"]);

    let found = pick(&lab, "Dup", &one);

    assert_eq!(found.entities.len(), 1);
    assert_eq!(
        found.entities[0].documents, 1,
        "dupont-lettre.txt is not selected"
    );
    assert!(
        found.outside_selection.is_empty(),
        "it has a file in the selection"
    );
}

#[tokio::test]
async fn an_empty_selection_finds_nothing_even_when_the_base_holds_the_name() {
    let lab = world(clinic()).await;

    let found = pick(&lab, "Dup", &[]);

    assert_eq!(found, EntitySearch::default());
}

#[tokio::test]
async fn an_empty_prefix_finds_nothing() {
    let lab = world(clinic()).await;
    let everything = lab.documents();

    assert_eq!(pick(&lab, "", &everything), EntitySearch::default());
    assert_eq!(pick(&lab, "   ", &everything), EntitySearch::default());
}

#[tokio::test]
async fn a_name_only_in_unselected_files_comes_back_apart_with_its_paths() {
    let lab = world(clinic()).await;
    let elsewhere = only(&lab, &["martin-controle.txt", "note-01.txt"]);

    let found = pick(&lab, "Dup", &elsewhere);

    assert!(
        found.entities.is_empty(),
        "Jean Dupont is not in the selection"
    );
    assert_eq!(found.outside_selection.len(), 1);
    let outside = &found.outside_selection[0];
    assert_eq!(outside.display, "Jean Dupont");
    assert_eq!(
        outside.documents,
        vec![
            "dupont-bilan.txt".to_string(),
            "dupont-lettre.txt".to_string()
        ]
    );
    assert!(outside.workbooks.is_empty());
    clean(&lab);
}

#[tokio::test]
async fn the_outside_group_never_lists_a_selected_file() {
    // Invariant I7: the only surface that names a name outside the selection lists files outside it.
    let lab = world(clinic()).await;
    let elsewhere = only(&lab, &["martin-controle.txt"]);
    let selected: Vec<String> = elsewhere
        .iter()
        .map(|file| file.relative_path.clone())
        .collect();

    for prefix in ["D", "J", "M", "Cl", "Dup", "Mar"] {
        let found = pick(&lab, prefix, &elsewhere);
        for outside in &found.outside_selection {
            for path in outside.documents.iter().chain(&outside.workbooks) {
                assert!(!selected.contains(path), "{prefix}: {path} is selected");
            }
        }
    }
}

#[tokio::test]
async fn the_outside_group_is_short() {
    let mut files = vec![filler(1)];
    for index in 0..8 {
        files.push((
            format!("patient-{index}.txt"),
            doc(
                10 + index,
                &format!(
                    "Docteur Albert Zorn{} recoit le lundi.",
                    ["a", "b", "c", "d", "e", "f", "g", "h"][index as usize]
                ),
            ),
        ));
    }
    let lab = world(files).await;
    let selection = only(&lab, &["note-01.txt"]);

    let found = pick(&lab, "Albert", &selection);

    assert!(found.outside_selection.len() <= MAX_OUTSIDE_NAMES);
}

#[tokio::test]
async fn a_candidate_is_never_offered() {
    // An untitled capitalised phrase is a candidate: it can rank, never narrow, and never be
    // offered as a name (defect KBD-11).
    let mut files = clinic();
    files.push((
        "reunion.txt".into(),
        doc(
            77,
            "La reunion du lundi a reuni Zacharie Quintellier et quelques collegues.",
        ),
    ));
    let lab = world(files).await;
    let held =
        entity_named(&lab.side(), "Zacharie Quintellier").expect("the base holds the phrase");
    assert_eq!(held.status, "candidate");

    let found = pick(&lab, "Zach", &lab.documents());

    assert!(found.entities.is_empty());
    assert!(found.outside_selection.is_empty());
}

#[tokio::test]
async fn a_workbook_counts_beside_the_documents() {
    let mut lab = world(clinic()).await;
    lab.write_data(
        "rdv.csv",
        "date;patient;duree_min\n02/03/2026;Alice Archer;20\n03/03/2026;Bruno Baker;25\n",
    );
    lab.analyse_data();
    let documents = lab.documents();
    let workbooks = lab.workbooks();
    assert_eq!(workbooks.len(), 1);

    let found = pick_both(&lab, "Alice", &documents, &workbooks);

    assert_eq!(found.entities.len(), 1);
    assert_eq!(found.entities[0].display, "Alice Archer");
    assert_eq!(found.entities[0].documents, 0);
    assert_eq!(found.entities[0].workbooks, 1);

    // Not selected: it is offered apart, as a workbook path.
    let without = pick_both(&lab, "Alice", &documents, &[]);
    assert!(without.entities.is_empty());
    assert_eq!(without.outside_selection.len(), 1);
    assert_eq!(
        without.outside_selection[0].workbooks,
        vec!["rdv.csv".to_string()]
    );
    assert!(without.outside_selection[0].documents.is_empty());
}

#[tokio::test]
async fn a_file_changed_since_its_analysis_is_not_counted() {
    // The selection names a file under a content id the base has not read: it is a different file,
    // and what the base knows of the old content is not a reason to offer a name from it.
    let lab = world(clinic()).await;
    let stale: Vec<SelectedFile> = lab
        .documents()
        .into_iter()
        .filter(|file| file.relative_path == "dupont-bilan.txt")
        .map(|file| SelectedFile::new(&file.relative_path, "0000-changed-since"))
        .collect();

    let found = pick(&lab, "Dup", &stale);

    assert!(found.entities.is_empty());
}

#[tokio::test]
async fn searching_names_writes_nothing_to_the_base() {
    let lab = world(clinic()).await;
    let everything = lab.documents();
    let before = snapshot(&lab.side());

    let _ = pick(&lab, "Dup", &everything);
    let _ = pick(&lab, "Zzz", &everything);
    let _ = pick(&lab, "Mar", &[]);

    assert_eq!(snapshot(&lab.side()), before);
}

#[tokio::test]
async fn the_list_is_bounded_by_the_limit_asked() {
    let mut files = Vec::new();
    for (index, name) in ["Adam", "Adele", "Adrien", "Agathe", "Aimee", "Alain"]
        .iter()
        .enumerate()
    {
        files.push((
            format!("a-{index}.txt"),
            doc(
                20 + index as u32,
                &format!("Madame {name} Lambert a ete recue lundi."),
            ),
        ));
    }
    let lab = world(files).await;
    let everything = lab.documents();

    let found = search_entities(
        &lab.side(),
        "A",
        PickerFiles {
            documents: &everything,
            data: &[],
        },
        PickerFiles {
            documents: &everything,
            data: &[],
        },
        3,
    )
    .expect("searches");

    assert_eq!(found.entities.len(), 3);
}

// ------------------------------------------------------- what a report tells the "which one?" row

use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;
use common::corpus::plan_for;

#[tokio::test]
async fn a_question_asked_about_two_doctors_reports_the_words_that_fit_and_the_files_of_each() {
    let lab = world(vec![
        (
            "paul.txt".into(),
            doc(1, "Docteur Paul Martin recoit le lundi."),
        ),
        (
            "paul-bis.txt".into(),
            doc(3, "Courrier du Docteur Paul Martin pour le suivi."),
        ),
        (
            "pierre.txt".into(),
            doc(2, "Docteur Pierre Martin recoit le mardi."),
        ),
        filler(1),
    ])
    .await;

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit le Dr Martin ?");
    let report = plan.report(false).expect("the user is asked");

    assert_eq!(report.ambiguities.len(), 1);
    let ambiguity = &report.ambiguities[0];
    assert!(
        ambiguity.span.to_lowercase().contains("martin"),
        "the words as typed: {:?}",
        ambiguity.span
    );
    let by_name: std::collections::BTreeMap<&str, usize> = ambiguity
        .candidates
        .iter()
        .map(|candidate| (candidate.display.as_str(), candidate.documents))
        .collect();
    assert_eq!(by_name["Paul Martin"], 2);
    assert_eq!(by_name["Pierre Martin"], 1);
    for candidate in &ambiguity.candidates {
        assert_eq!(candidate.workbooks, 0);
    }
}

#[tokio::test]
async fn a_possible_match_reports_the_word_as_typed_and_where_the_name_lives() {
    let lab = world(clinic()).await;

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Dupond du bilan ?");
    let report = plan.report(false).expect("the user is offered the name");

    assert_eq!(report.possibles.len(), 1);
    assert_eq!(report.possibles[0].span, "Dupond");
    assert_eq!(report.possibles[0].candidates[0].display, "Jean Dupont");
    assert_eq!(report.possibles[0].candidates[0].documents, 2);
}

#[tokio::test]
async fn the_report_serialises_the_new_fields_in_camel_case() {
    let lab = world(clinic()).await;
    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit J. Dupont ?");
    let report = plan.report(false).expect("offered");

    let json = serde_json::to_value(&report).expect("serialises");

    assert_eq!(json["possibles"][0]["span"], "J. Dupont");
    assert_eq!(json["possibles"][0]["candidates"][0]["documents"], 2);
    assert_eq!(json["possibles"][0]["candidates"][0]["workbooks"], 0);
}

// ------------------------------------------------------------- the human test files and numbers

use assistant_cabinet_ai_lib::folder_questions::{self, QuestionRoute};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use assistant_cabinet_ai_lib::knowledge::reduce::{SideInput, TargetingPolicy};
use assistant_cabinet_ai_lib::knowledge::targeting::{RequestMode, TargetingRequest};

/// Prints what the interface is told for each step of the lot 7 human test, from the thirty invented
/// documents that `lot07_helper.py write-fixtures <folder>` writes, with the settings the application
/// ships. The numbers of the protocol come from here.
///
/// ```text
/// python docs\test-reports\knowledge-base-pass-1\human-tests\lot07_helper.py write-fixtures <folder>
/// set ACAI_LOT07_DIR=<folder>
/// set ACAI_LOT07_OUT=<file.jsonl>      (optional: one JSON line per report, for the sentences)
/// cargo test --test knowledge_picker print_what_the_interface_shows -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore]
async fn print_what_the_interface_shows_for_the_human_test() {
    let Ok(folder) = std::env::var("ACAI_LOT07_DIR") else {
        println!("set ACAI_LOT07_DIR to a folder written by lot07_helper.py write-fixtures");
        return;
    };
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    for entry in std::fs::read_dir(&folder).expect("reads the folder") {
        let path = entry.expect("entry").path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        lab.write(&name, &std::fs::read_to_string(&path).expect("reads"));
    }
    lab.analyse().await;
    let full = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index)).expect("discovers");
    let mut out = String::new();

    // Two selections: everything, and everything but the files that name Dupont.
    let all = lab.documents();
    let without_dupont: Vec<SelectedFile> = all
        .iter()
        .filter(|file| !file.relative_path.contains("dupont"))
        .cloned()
        .collect();
    println!(
        "documents: {}, without Dupont: {}",
        all.len(),
        without_dupont.len()
    );

    for (label, selected) in [("all", &all), ("without-dupont", &without_dupont)] {
        for prefix in ["Dup", "Mar", "Cl", "Pau", "Pie", "J", "cab", "Zz"] {
            let found = pick(&lab, prefix, selected);
            let names: Vec<String> = found
                .entities
                .iter()
                .map(|e| format!("{} ({} file(s))", e.display, e.documents + e.workbooks))
                .collect();
            let outside: Vec<String> = found
                .outside_selection
                .iter()
                .map(|o| format!("{} -> {:?}", o.display, o.documents))
                .collect();
            println!("[{label}] @{prefix}: {names:?}  outside: {outside:?}");
        }
    }

    // The questions of the protocol: where the folder router sends each, and the report.
    let policy = TargetingPolicy::default();
    let vocabulary = lab.vocabulary();
    let jean = common::corpus::entity_id(&lab, "Jean Dupont");
    let paul = common::corpus::entity_id(&lab, "Paul Martin");
    let pierre = common::corpus::entity_id(&lab, "Pierre Martin");
    let claire = common::corpus::entity_id(&lab, "Claire Martin");
    let scoped = WorkFolderInventory::from_records(full.root(), full.all_files().to_vec());
    let selection = folder_questions::Selection {
        full: &full,
        scoped: &scoped,
        narrowed: false,
        no_documents_chosen: false,
    };
    let ask = |entity_ids: Vec<i64>, carry_over: Vec<i64>, mode: RequestMode| TargetingRequest {
        mode,
        entity_ids,
        carry_over,
    };
    let cases: Vec<(&str, &str, TargetingRequest, KnowledgeMode)> = vec![
        (
            "Q1",
            "Que dit le Docteur Jean Dupont du bilan sanguin ?",
            ask(vec![], vec![], RequestMode::Auto),
            KnowledgeMode::Auto,
        ),
        (
            "Q1-suggest",
            "Que dit le Docteur Jean Dupont du bilan sanguin ?",
            ask(vec![], vec![], RequestMode::Auto),
            KnowledgeMode::Suggest,
        ),
        (
            "Q2",
            "Que dit le Dr Martin ?",
            ask(vec![], vec![], RequestMode::Auto),
            KnowledgeMode::Auto,
        ),
        (
            "Q2-suggest",
            "Que dit le Dr Martin ?",
            ask(vec![], vec![], RequestMode::Auto),
            KnowledgeMode::Suggest,
        ),
        (
            "Q2-chip-paul",
            "Que dit le Dr Martin ?",
            ask(vec![paul], vec![], RequestMode::Auto),
            KnowledgeMode::Suggest,
        ),
        (
            "Q3",
            "Que dit Madame Claire Martin du contrôle ?",
            ask(vec![], vec![], RequestMode::Auto),
            KnowledgeMode::Auto,
        ),
        (
            "Q4-misspelt",
            "Que dit le Docteur Dupond du bilan sanguin ?",
            ask(vec![], vec![], RequestMode::Auto),
            KnowledgeMode::Suggest,
        ),
        (
            "Q5-follow-up",
            "Quand reçoit-il ?",
            ask(vec![], vec![jean], RequestMode::Auto),
            KnowledgeMode::Auto,
        ),
        (
            "Q6-widen",
            "Que dit le Docteur Jean Dupont du bilan sanguin ?",
            ask(vec![], vec![], RequestMode::Off),
            KnowledgeMode::Auto,
        ),
        (
            "Q7-carried",
            "Que contiennent les notes internes ?",
            ask(vec![], vec![jean], RequestMode::Auto),
            KnowledgeMode::Auto,
        ),
        (
            "Q7-no-name",
            "Que contiennent les notes internes ?",
            ask(vec![], vec![], RequestMode::Auto),
            KnowledgeMode::Auto,
        ),
        (
            "Q8-photocopier",
            "Le Docteur Dupont a-t-il parlé de la photocopieuse ?",
            ask(vec![], vec![], RequestMode::Auto),
            KnowledgeMode::Auto,
        ),
        // The picker writes the name it offers into the question: what the router makes of it.
        (
            "Q9-picked-pierre",
            "Que dit Pierre Martin ?",
            ask(vec![pierre], vec![], RequestMode::Auto),
            KnowledgeMode::Suggest,
        ),
        (
            "Q10-picked-claire",
            "Que dit Claire Martin ?",
            ask(vec![claire], vec![], RequestMode::Auto),
            KnowledgeMode::Suggest,
        ),
        (
            "Q11-picked-jean",
            "Que dit Jean Dupont ?",
            ask(vec![jean], vec![], RequestMode::Auto),
            KnowledgeMode::Suggest,
        ),
        (
            "Q12-picked-jean-sentence",
            "Que dit Jean Dupont du bilan sanguin ?",
            ask(vec![jean], vec![], RequestMode::Auto),
            KnowledgeMode::Auto,
        ),
        (
            "Q4b-misspelt-then-picked",
            "Que dit le Docteur Dupond du bilan sanguin ?",
            ask(vec![jean], vec![], RequestMode::Auto),
            KnowledgeMode::Suggest,
        ),
    ];
    for (label, question, request, mode) in cases {
        let route =
            match folder_questions::route_in_selection(&selection, &lab.index, question, "fr-FR") {
                QuestionRoute::Deterministic(_) => "deterministic folder answer".to_string(),
                QuestionRoute::TargetedRetrieval { file } => {
                    format!("named file {}", file.relative_path)
                }
                QuestionRoute::PerDocumentRetrieval { .. } => "each document".to_string(),
                QuestionRoute::GlobalRetrieval => "global retrieval".to_string(),
                QuestionRoute::SetSearch(_) => "word search".to_string(),
            };
        let documents = lab.documents();
        let plan = lab.plan(
            &vocabulary,
            mode,
            &request,
            question,
            SideInput::new(&documents),
            SideInput::new(&[]),
            policy,
        );
        let report = plan.report(false);
        println!("{label} | {question}");
        println!("   route: {route}; mode {mode:?}");
        match &report {
            None => println!("   report: none (nothing is drawn under the answer)"),
            Some(report) => {
                println!(
                    "   report: applied files {} / {} (cut {}%), requestedBy {}, widened {}, entities {:?}",
                    report.candidate_paths.len(),
                    report.selected_documents,
                    report.reduction_percent,
                    report.requested_by,
                    report.widened,
                    report
                        .entities
                        .iter()
                        .map(|e| (e.display.clone(), e.confidence))
                        .collect::<Vec<_>>()
                );
                println!("   files: {:?}", report.candidate_paths);
                for ambiguity in &report.ambiguities {
                    println!(
                        "   asks about {:?}: {:?}",
                        ambiguity.span,
                        ambiguity
                            .candidates
                            .iter()
                            .map(|c| format!("{} ({}+{})", c.display, c.documents, c.workbooks))
                            .collect::<Vec<_>>()
                    );
                }
                for possible in &report.possibles {
                    println!(
                        "   did you mean for {:?}: {:?}",
                        possible.span,
                        possible
                            .candidates
                            .iter()
                            .map(|c| format!("{} ({}+{})", c.display, c.documents, c.workbooks))
                            .collect::<Vec<_>>()
                    );
                }
            }
        }
        let line = serde_json::json!({ "step": label, "question": question, "report": report });
        out.push_str(&format!("{line}\n"));
    }
    if let Ok(path) = std::env::var("ACAI_LOT07_OUT") {
        std::fs::write(&path, out).expect("writes the reports");
        println!("reports written to {path}");
    }
}

#[tokio::test]
async fn a_which_one_row_may_offer_unconfirmed_names_when_the_users_own_words_fit_them() {
    // Two untitled people of one surname, each seen in one file: candidates, never offered by the
    // picker (above) and never driving a narrowing. When the user types their surname, though, the
    // question is which of the names that fit it she means, and the answer is these two: the row
    // quotes her words and lists what fits them (defect KBD-11, row updated in lot 7).
    let lab = world(vec![
        (
            "a.txt".into(),
            doc(1, "La reunion a reuni Alice Quintel et quelques collegues."),
        ),
        (
            "b.txt".into(),
            doc(
                2,
                "La reunion a reuni Berthe Quintel et quelques collegues.",
            ),
        ),
        filler(1),
        filler(2),
    ])
    .await;
    let held = common::knowledge::entities(&lab.side());
    assert!(held.iter().all(|row| row.status == "candidate"));

    let plan = plan_for(&lab, KnowledgeMode::Auto, "Que dit Quintel ?");
    let report = plan.report(false).expect("the user is asked");

    assert_eq!(report.ambiguities.len(), 1);
    assert_eq!(report.ambiguities[0].span, "Quintel");
    let names: Vec<&str> = report.ambiguities[0]
        .candidates
        .iter()
        .map(|candidate| candidate.display.as_str())
        .collect();
    assert_eq!(names, ["Alice Quintel", "Berthe Quintel"]);
    assert!(
        report.candidate_paths.is_empty(),
        "nothing is narrowed on a candidate"
    );
}

/// The optional part of the human test: two spreadsheets selected, no document, the thirty documents
/// analysed in the same index (as in the owner index), the minimum number of files set to 2 in the
/// settings window. Prints the report the interface is given for the questions of that part.
#[tokio::test]
#[ignore]
async fn print_the_workbook_choice_for_the_human_test() {
    use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
    use assistant_cabinet_ai_lib::knowledge::targeting;
    use assistant_cabinet_ai_lib::tabular_answer::{self, TabularAnswer};

    let Ok(folder) = std::env::var("ACAI_LOT07_DIR") else {
        println!("set ACAI_LOT07_DIR to a folder written by lot07_helper.py write-fixtures");
        return;
    };
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    for entry in std::fs::read_dir(&folder).expect("reads the folder") {
        let path = entry.expect("entry").path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        lab.write(&name, &std::fs::read_to_string(&path).expect("reads"));
    }
    lab.analyse().await;
    let fixtures = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs/test-reports/human-acceptance-pass-1/fixtures/data");
    for name in ["rdv-mars-2026.xlsx", "factures-fournisseurs-2026.xlsx"] {
        std::fs::copy(fixtures.join(name), lab.data.path().join(name)).expect("copies");
    }
    lab.analyse_data();

    let policy = TargetingPolicy {
        min_scope_files: 2,
        ..TargetingPolicy::default()
    };
    let vocabulary = lab.vocabulary();
    let workbooks = lab.workbooks();
    let data_folder = lab.data_folder();
    for question in [
        "Combien de lignes pour Dupont J ?",
        "Combien de lignes pour MedSupply ?",
    ] {
        for mode in [KnowledgeMode::Suggest, KnowledgeMode::Auto] {
            let plan = lab.plan(
                &vocabulary,
                mode,
                &TargetingRequest::default(),
                question,
                SideInput::new(&[]),
                SideInput::new(&workbooks),
                policy,
            );
            println!("{question} | {mode:?}");
            let selection = match plan.applied_workbooks() {
                Some(candidates) => targeting::narrowed_data_selection(
                    &data_folder,
                    &ScopeMode::WholeFolder,
                    candidates,
                ),
                None => ScopeMode::WholeFolder,
            };
            let answer = match tabular_answer::prepare(
                question,
                &data_folder,
                &selection,
                "fr-FR",
                &lab.index,
                false,
            )
            .expect("prepares")
            {
                tabular_answer::PendingAnswer::Done(answer) => answer,
                tabular_answer::PendingAnswer::TryModel { fallback, .. } => fallback,
            };
            let answer = targeting::decorate_which_workbook(answer, &plan);
            match &answer {
                TabularAnswer::WhichWorkbook { candidates, .. } => {
                    println!("   engine asks which workbook: {candidates:?}")
                }
                other => println!(
                    "   engine answers from {:?}",
                    targeting::answered_file(other)
                ),
            }
            match plan.report(false) {
                None => println!("   report: none"),
                Some(report) => println!(
                    "   report: workbooks {:?} of {}, chosen {:?}, requestedBy {}, entities {:?}",
                    report.workbook_paths,
                    report.selected_workbooks,
                    report.chosen_workbook.as_ref().map(|c| c.file.clone()),
                    report.requested_by,
                    report
                        .entities
                        .iter()
                        .map(|e| (e.display.clone(), e.confidence))
                        .collect::<Vec<_>>()
                ),
            }
        }
    }
}
