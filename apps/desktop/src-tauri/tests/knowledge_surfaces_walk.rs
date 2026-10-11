//! Walks the lot 9 human test with the real Rust code, on the same thirty-one invented documents and
//! the two invented spreadsheets, and prints what each step shows, so the protocol's figures come
//! from the code and not from a guess.
//!
//! Not a regression test (it asserts almost nothing): run it with
//!
//! ```text
//! python docs\test-reports\knowledge-base-pass-1\human-tests\lot09_helper.py write-fixtures <dir>
//! set ACAI_LOT09_DIR=<dir>
//! cargo test --test knowledge_surfaces_walk -- --ignored --nocapture
//! ```

mod common;

use std::path::PathBuf;

use assistant_cabinet_ai_lib::analysis_scope::{AnalysisScope, ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::folder_questions::{route_in_selection, Selection};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use assistant_cabinet_ai_lib::knowledge::query::{QuerySelection, SelectedFile};
use assistant_cabinet_ai_lib::knowledge::sheet::entity_sheet;
use assistant_cabinet_ai_lib::tabular_answer::{
    prepare, prepare_if_data_only, PendingAnswer, TabularAnswer, WorkbookChoice,
};
use common::knowledge::{entities, Lab};
use common::suggestions::{sentence, suggestions};

fn fixtures() -> PathBuf {
    PathBuf::from(
        std::env::var("ACAI_LOT09_DIR").expect("set ACAI_LOT09_DIR to the fixtures folder"),
    )
}

async fn lab() -> Lab {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    for entry in std::fs::read_dir(fixtures().join("docs")).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        lab.write(path.file_name().unwrap().to_str().unwrap(), &text);
    }
    let documents_summary = lab.analyse().await;
    println!("DOCUMENTS ANALYSE: {:?}", documents_summary.knowledge);
    for entry in std::fs::read_dir(fixtures().join("data")).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        lab.write_data(path.file_name().unwrap().to_str().unwrap(), &text);
    }
    let data_summary = lab.analyse_data();
    println!("DATA ANALYSE: {:?}", data_summary.knowledge);
    lab
}

fn only_documents(all: &[SelectedFile], names: &[&str]) -> Vec<SelectedFile> {
    all.iter()
        .filter(|file| names.contains(&file.relative_path.as_str()))
        .cloned()
        .collect()
}

fn choice(lab: &Lab, names: &[&str]) -> Vec<WorkbookChoice> {
    let folder = lab.data_folder();
    assistant_cabinet_ai_lib::tabular_answer::selected_workbook_choices(
        &folder,
        &ScopeMode::WholeFolder,
    )
    .into_iter()
    .filter(|choice| names.contains(&choice.relative_path.as_str()))
    .collect()
}

/// What the real path says to a sentence, as short text.
fn answer_of(
    lab: &Lab,
    question: &str,
    locale: &str,
    documents: &[SelectedFile],
    chosen: &[WorkbookChoice],
) -> String {
    let tables = !chosen.is_empty();
    let selection = ScopeMode::Explicit(
        chosen
            .iter()
            .map(|choice| ScopeEntry {
                relative_path: choice.relative_path.clone(),
                pinned_id: choice.content_id.clone(),
                added_at: 1,
                sheet_names: Vec::new(),
            })
            .collect(),
    );
    let inventory = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index)).unwrap();
    let folder = lab.data_folder();
    let narrowing = AnalysisScope {
        mode: ScopeMode::Explicit(
            documents
                .iter()
                .map(|file| ScopeEntry {
                    relative_path: file.relative_path.clone(),
                    pinned_id: file.content_id.clone(),
                    added_at: 1,
                    sheet_names: Vec::new(),
                })
                .collect(),
        ),
        data_mode: ScopeMode::nothing(),
        created_at: 1,
        updated_at: 1,
    };
    let resolution = narrowing.resolve(&inventory);
    let route = route_in_selection(
        &Selection {
            full: &inventory,
            scoped: &resolution.inventory,
            narrowed: resolution.narrowed,
            no_documents_chosen: resolution.no_documents_chosen,
        },
        &lab.index,
        question,
        locale,
    );
    let route = format!("{route:?}");
    let route = route.chars().take(40).collect::<String>();
    if !tables {
        return format!("documents route: {route}");
    }
    let data_only =
        prepare_if_data_only(question, &folder, &selection, locale, &lab.index).unwrap();
    let pending = prepare(question, &folder, &selection, locale, &lab.index, false).unwrap();
    let table = match pending {
        PendingAnswer::Done(TabularAnswer::Value { value, locator, .. }) => {
            format!("VALUE {value:?} filters {:?}", locator.filters)
        }
        PendingAnswer::Done(other) => format!("{other:?}").chars().take(80).collect(),
        PendingAnswer::TryModel { .. } => "TryModel".to_string(),
    };
    format!(
        "table tier takes it: {} | {table} | documents route: {route}",
        data_only.is_some()
    )
}

async fn show(lab: &Lab, label: &str, documents: Vec<SelectedFile>, workbooks: &[&str]) {
    let rig = common::suggestions::Rig {
        documents: documents.clone(),
        choices: choice(lab, workbooks),
    };
    println!(
        "\n=== {label}: {} documents, workbooks {workbooks:?}",
        documents.len()
    );
    for names in [false, true] {
        let started = std::time::Instant::now();
        let set = suggestions(lab, &rig, "fr-FR", names);
        let took = started.elapsed();
        println!(
            "  names {} -> namesHidden {}, dry runs {}, {} suggestions, {:?}",
            if names { "shown " } else { "hidden" },
            set.names_hidden,
            set.dry_runs,
            set.suggestions.len(),
            took
        );
        for suggestion in &set.suggestions {
            for locale in ["fr-FR", "en-US"] {
                let text = sentence(locale, suggestion);
                println!(
                    "    [{locale}] {text}\n        -> {}",
                    answer_of(lab, &text, locale, &documents, &rig.choices)
                );
            }
        }
    }
}

#[tokio::test]
#[ignore]
async fn walk_the_human_test() {
    let lab = lab().await;
    println!("ENTITIES:");
    for entity in entities(&lab.side()) {
        println!(
            "  {} [{}/{}]",
            entity.canonical, entity.type_id, entity.status
        );
    }
    let all = lab.documents();

    show(&lab, "S1 every document", all.clone(), &[]).await;
    show(
        &lab,
        "S2 three documents (Claire Martin twice)",
        only_documents(
            &all,
            &[
                "martin-controle.txt",
                "martin-ordonnance.txt",
                "note-01.txt",
            ],
        ),
        &[],
    )
    .await;
    show(
        &lab,
        "S3 every document and rdv.csv",
        all.clone(),
        &["rdv.csv"],
    )
    .await;
    show(
        &lab,
        "S4 every document and both workbooks",
        all.clone(),
        &["rdv.csv", "factures.csv"],
    )
    .await;
    show(&lab, "S5 rdv.csv alone", Vec::new(), &["rdv.csv"]).await;
    show(
        &lab,
        "S6 both workbooks, no document",
        Vec::new(),
        &["rdv.csv", "factures.csv"],
    )
    .await;

    // The sheet of Jean Dupont, from a chip.
    let jean = common::corpus::entity_id(&lab, "Jean Dupont");
    let vocabulary = lab.vocabulary();
    let workbooks = lab.workbooks();
    let selection = QuerySelection {
        documents: &all,
        data: &workbooks,
    };
    println!("\nSHEET Jean Dupont (all documents, both workbooks):");
    println!(
        "{:#?}",
        entity_sheet(&lab.side(), &vocabulary.packs, jean, &selection).unwrap()
    );
    let narrow = only_documents(&all, &["dupont-bilan-2026.txt", "note-01.txt"]);
    println!("\nSHEET Jean Dupont (two documents, no workbook):");
    println!(
        "{:#?}",
        entity_sheet(
            &lab.side(),
            &vocabulary.packs,
            jean,
            &QuerySelection {
                documents: &narrow,
                data: &[]
            }
        )
        .unwrap()
    );
}
