//! KB fix 4, batch 3 (`fix/kb-retrieval-rules`): the table, router and check defects of the register. Every
//! test here was written first and seen failing, then the fix was made.
//!
//! - F-22 (KBD-05): a figure a source writes across a line break is still that figure.
//! - F-12 (KBD-24, KBD-22): a title shared by several values of a column filters nothing; a month or a year
//!   on a date column the file does not settle is a nudge, never a filtered `0`.
//! - F-13 (KBD-21, KBD-23): with documents and workbooks selected, a plain question about the documents
//!   is not data-only; "which workbook" is asked only of a question that points at a table.
//! - F-23 (KBD-02): a folder and a file name (`mars/neurologie.pdf`) name the file that ends that way.
//!
//! Every name here is invented.

mod common;

use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
use assistant_cabinet_ai_lib::file_reference::{
    FileReferenceResolution, FileReferenceResolver, ReferenceStatus,
};
use assistant_cabinet_ai_lib::folder_questions::{self, FolderAnswer, NoCorpus, Selection};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use assistant_cabinet_ai_lib::number_check;
use assistant_cabinet_ai_lib::tabular::engine::{NotAnswerableReason, TabularValue};
use assistant_cabinet_ai_lib::tabular_answer::{self, PendingAnswer, TabularAnswer};

use common::knowledge::Lab;

// ---------------------------------------------------------------------------------------- F-22

#[test]
fn a_figure_split_by_a_line_break_in_the_source_is_supported() {
    let source = "Numero d'assurance : 1 85 03 69\n123 456 78 (fictif)";
    let answer = "Le numero est 1 85 03 69 123 456 78.";
    assert!(
        number_check::unsupported_numbers(answer, &[source], 2).is_empty(),
        "the answer copies the source exactly, only the line break differs"
    );
}

#[test]
fn a_figure_split_by_a_windows_line_break_in_the_source_is_supported() {
    let source = "Numero : 1 85 03 69\r\n123 456 78";
    let answer = "Le numero est 1 85 03 69 123 456 78.";
    assert!(number_check::unsupported_numbers(answer, &[source], 2).is_empty());
}

#[test]
fn a_figure_in_no_source_is_still_reported_when_the_sources_have_line_breaks() {
    let source = "Numero : 1 85 03 69\n123 456 78";
    let answer = "Le montant est 4 321.";
    assert_eq!(
        number_check::unsupported_numbers(answer, &[source], 2),
        vec!["4 321".to_string()]
    );
}

// ---------------------------------------------------------------------------------------- F-23

fn folder_of_two_months() -> Lab {
    let lab = Lab::new();
    lab.write(
        "2026/mars/neurologie.txt",
        "Compte rendu de neurologie de mars.",
    );
    lab.write(
        "2026/janvier/neurologie.txt",
        "Compte rendu de neurologie de janvier.",
    );
    lab.write("2026/mars/biologie.txt", "Bilan de biologie de mars.");
    lab
}

fn resolve(lab: &Lab, reference: &str) -> FileReferenceResolution {
    let inventory = WorkFolderInventory::discover(lab.work_path(), None).unwrap();
    FileReferenceResolver::new(&inventory).resolve(reference)
}

#[test]
fn a_folder_and_a_file_name_reach_the_one_file_that_ends_that_way() {
    let lab = folder_of_two_months();
    for reference in [
        "2026/mars/neurologie.txt",
        "mars/neurologie.txt",
        "Mars/Neurologie.txt",
        "mars\\neurologie.txt",
    ] {
        let resolution = resolve(&lab, reference);
        assert_eq!(resolution.status, ReferenceStatus::Exact, "{reference}");
        assert_eq!(
            resolution.exact_match.unwrap().relative_path,
            "2026/mars/neurologie.txt",
            "{reference}"
        );
    }
}

#[test]
fn a_folder_suffix_that_two_files_share_is_asked_never_chosen() {
    let lab = Lab::new();
    lab.write("2025/mars/neurologie.txt", "Compte rendu.");
    lab.write("2026/mars/neurologie.txt", "Compte rendu.");
    let resolution = resolve(&lab, "mars/neurologie.txt");
    assert_eq!(resolution.status, ReferenceStatus::MultipleMatches);
    assert_eq!(resolution.candidates.len(), 2);
}

#[test]
fn a_partial_folder_name_never_reaches_a_file_in_another_folder() {
    let lab = folder_of_two_months();
    // The suffix starts at a folder boundary: `ars/` is no folder, `avril/` holds nothing.
    for reference in [
        "ars/neurologie.txt",
        "avril/neurologie.txt",
        "mars/biologie-2.txt",
    ] {
        let resolution = resolve(&lab, reference);
        assert_eq!(resolution.status, ReferenceStatus::NoMatch, "{reference}");
    }
}

#[test]
fn a_folder_and_a_file_name_in_a_question_name_that_file() {
    let lab = folder_of_two_months();
    let inventory = WorkFolderInventory::discover(lab.work_path(), None).unwrap();
    let resolution = FileReferenceResolver::new(&inventory)
        .resolve_in_question("Que dit mars/neurologie.txt ?")
        .expect("the question names a file");
    assert_eq!(resolution.status, ReferenceStatus::Exact);
    assert_eq!(
        resolution.exact_match.unwrap().relative_path,
        "2026/mars/neurologie.txt"
    );
}

// ---------------------------------------------------------------------------------------- F-15

fn inventory_of(
    name: &str,
    text: &str,
) -> assistant_cabinet_ai_lib::tabular::inventory::TabularInventory {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(name), text).unwrap();
    assistant_cabinet_ai_lib::tabular::build_inventory(&dir.path().join(name), name, "fr-FR")
        .expect("builds the inventory")
}

#[test]
fn a_semicolon_export_with_a_title_row_and_no_comma_reads_as_four_columns() {
    let mut text = String::from("Export du 12/03/2026\ndate;Patiente;Medecin traitant;duree\n");
    for day in 1..=10 {
        text.push_str(&format!("{day:02}/03/2026;Alice Archer;Paul Martin;20\n"));
    }
    let inventory = inventory_of("rdv-export.csv", &text);
    let sheet = &inventory.sheets[0];
    let names: Vec<&str> = sheet
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect();
    assert_eq!(names, ["date", "Patiente", "Medecin traitant", "duree"]);
    assert_eq!(sheet.header_row, Some(1));
    assert_eq!(sheet.row_count, 10);
}

#[test]
fn a_comma_file_and_a_decimal_comma_semicolon_file_are_read_as_before() {
    let comma = inventory_of("virgule.csv", "nom,age\nCamille,42\nEsaie,7\n");
    assert_eq!(comma.sheets[0].column_count, 2);
    let french = inventory_of(
        "montants.csv",
        "nom;montant;date\nCamille;120,50;12/03/2026\nEsaie;75,00;05/01/2026\n",
    );
    assert_eq!(french.sheets[0].column_count, 3);
    // A prose note renamed `.csv` stays one column: no delimiter slices it.
    let note = inventory_of("note.csv", "Une note\nsans separateur\nni tableau\n");
    assert_eq!(note.sheets[0].column_count, 1);
}

// ---------------------------------------------------------------------------------------- F-12

fn ask_data(lab: &Lab, question: &str) -> TabularAnswer {
    let folder = lab.data_folder();
    match tabular_answer::prepare(
        question,
        &folder,
        &ScopeMode::WholeFolder,
        "fr-FR",
        &lab.index,
        false,
    ) {
        Ok(PendingAnswer::Done(answer)) => answer,
        Ok(_) => panic!("{question:?}: the engine handed the question to the model"),
        Err(error) => panic!("{question:?}: {error:?}"),
    }
}

fn count_of(question: &str, answer: &TabularAnswer) -> usize {
    match answer {
        TabularAnswer::Value {
            value: TabularValue::Count(rows),
            ..
        } => *rows,
        other => panic!("{question:?}: expected a count, got {other:?}"),
    }
}

/// Three doctors written with their title, two of them sharing a surname; one date is above 12, so the
/// date column is a real one.
const APPOINTMENTS_WITH_TITLES: &str = "patient;date;medecin;montant\n\
    Alice Archer;02/03/2026;Dr Jean Dupont;50\n\
    Bruno Blanc;07/03/2026;Dr Jean Dupont;60\n\
    Carole Caron;15/03/2026;Dr Paul Martin;70\n\
    Alice Archer;21/03/2026;Dr Pierre Martin;80\n\
    Bruno Blanc;22/03/2026;Dr Paul Martin;90\n\
    Carole Caron;24/03/2026;Dr Jean Dupont;40\n";

fn titled_doctors() -> Lab {
    let mut lab = Lab::new();
    lab.write_data("rdv.csv", APPOINTMENTS_WITH_TITLES);
    lab.analyse_data();
    lab
}

#[test]
fn a_doctor_named_with_his_title_is_counted_like_the_doctor_named_without_it() {
    let lab = titled_doctors();
    for (with_title, without_title, rows) in [
        (
            "Combien de lignes pour Dr Jean Dupont ?",
            "Combien de lignes pour Jean Dupont ?",
            3,
        ),
        (
            "Combien de lignes pour Dr Paul Martin ?",
            "Combien de lignes pour Paul Martin ?",
            2,
        ),
        (
            "Combien de lignes pour Dr Pierre Martin ?",
            "Combien de lignes pour Pierre Martin ?",
            1,
        ),
    ] {
        let a = ask_data(&lab, with_title);
        assert_eq!(count_of(with_title, &a), rows, "{with_title}");
        let b = ask_data(&lab, without_title);
        assert_eq!(count_of(without_title, &b), rows, "{without_title}");
    }
}

#[test]
fn the_sum_for_a_doctor_named_with_his_title_is_computed() {
    let lab = titled_doctors();
    for question in [
        "Quelle est la somme de montant pour Dr Jean Dupont ?",
        "Quelle est la somme de montant pour Jean Dupont ?",
    ] {
        match ask_data(&lab, question) {
            TabularAnswer::Value {
                value: TabularValue::Sum(total),
                ..
            } => assert_eq!(total.value, 150.0, "{question}"),
            other => panic!("{question:?}: {other:?}"),
        }
    }
}

#[test]
fn a_word_shared_by_two_doctors_counts_both_and_a_conflicting_pair_counts_none() {
    let lab = titled_doctors();
    // Martin alone is both Martins (three rows); it is not a value no cell equals.
    let question = "Combien de lignes pour Martin ?";
    assert_eq!(count_of(question, &ask_data(&lab, question)), 3);
    // Jean and Martin are two different doctors: nobody is called Jean Martin.
    let question = "Combien de lignes pour Jean Martin ?";
    assert_eq!(count_of(question, &ask_data(&lab, question)), 0);
}

// KBD-22: every day of the file is 12 or below, so the file does not settle which number is the day.
const APPOINTMENTS_WITH_AN_UNSETTLED_DATE: &str = "patient;date;medecin\n\
    Alice Archer;02/03/2026;Dr Jean Dupont\n\
    Alice Archer;05/03/2026;Dr Paul Martin\n\
    Bruno Blanc;07/03/2026;Dr Jean Dupont\n\
    Carole Caron;10/03/2026;Dr Pierre Martin\n";

#[test]
fn a_month_or_a_year_on_a_date_column_the_file_does_not_settle_is_a_nudge_naming_the_column() {
    let mut lab = Lab::new();
    lab.write_data("rdv-ambigu.csv", APPOINTMENTS_WITH_AN_UNSETTLED_DATE);
    lab.analyse_data();
    for question in [
        "Combien de lignes pour Alice Archer en mars 2026 ?",
        "Combien de lignes en mars 2026 ?",
        "Combien de lignes pour Alice Archer en 2026 ?",
        "Combien de lignes pour Alice Archer en mars ?",
    ] {
        match ask_data(&lab, question) {
            TabularAnswer::Nudge {
                reason,
                filter_column,
                ..
            } => {
                assert_eq!(
                    reason,
                    Some(NotAnswerableReason::NonDateColumn),
                    "{question}"
                );
                assert_eq!(filter_column.as_deref(), Some("date"), "{question}");
            }
            other => panic!("{question:?}: expected a nudge, got {other:?}"),
        }
    }
    // Nothing about a date: still an ordinary count.
    let question = "Combien de lignes pour Alice Archer ?";
    assert_eq!(count_of(question, &ask_data(&lab, question)), 2);
}

#[test]
fn a_date_column_the_file_settles_still_filters_by_month_and_year() {
    let lab = titled_doctors();
    let question = "Combien de lignes pour Alice Archer en mars 2026 ?";
    assert_eq!(count_of(question, &ask_data(&lab, question)), 2);
}

// ---------------------------------------------------------------------------------------- F-13

const APPOINTMENTS: &str = "patient;date;medecin\n\
    Alice Archer;02/03/2026;Dr Jean Dupont\n\
    Bruno Blanc;07/03/2026;Dr Paul Martin\n\
    Carole Caron;15/03/2026;Dr Jean Dupont\n";

const INVOICES: &str = "fournisseur;montant\n\
    MedSupply;1450\n\
    Papeterie;50\n";

/// Three documents and two workbooks: the selection of the mixed tier.
fn documents_and_two_workbooks() -> Lab {
    let mut lab = Lab::new();
    lab.write(
        "courrier-dupont.txt",
        "Courrier au sujet de Jean Dupont et de son bilan sanguin.",
    );
    lab.write("notes-internes.txt", "Notes internes du secretariat.");
    lab.write("compte-rendu.txt", "Compte rendu de la reunion de mars.");
    lab.write_data("rdv.csv", APPOINTMENTS);
    lab.write_data("factures.csv", INVOICES);
    lab.analyse_data();
    lab
}

fn data_only(lab: &Lab, question: &str, locale: &str) -> Option<PendingAnswer> {
    tabular_answer::prepare_if_data_only(
        question,
        &lab.data_folder(),
        &ScopeMode::WholeFolder,
        locale,
        &lab.index,
    )
    .unwrap()
}

#[test]
fn a_plain_question_about_the_documents_is_not_data_only_when_two_workbooks_are_selected() {
    let lab = documents_and_two_workbooks();
    for (question, locale) in [
        (
            "Que disent mes documents \u{e0} propos de Jean Dupont ?",
            "fr-FR",
        ),
        ("R\u{e9}sume chaque document.", "fr-FR"),
        ("Dans mes documents, que lit-on sur Jean Dupont ?", "fr-FR"),
        ("Que dit Jean Dupont sur son bilan sanguin ?", "fr-FR"),
        ("What do my documents say about Jean Dupont?", "en-US"),
    ] {
        assert!(
            data_only(&lab, question, locale).is_none(),
            "{question:?} is about the documents, not about a table"
        );
    }
}

#[test]
fn a_question_that_points_at_a_table_still_asks_which_workbook() {
    let lab = documents_and_two_workbooks();
    match data_only(&lab, "Combien de lignes ?", "fr-FR") {
        Some(PendingAnswer::Done(TabularAnswer::WhichWorkbook { candidates, .. })) => {
            assert_eq!(candidates.len(), 2)
        }
        _ => panic!("two workbooks and a row count: the choice of the workbook is the answer"),
    }
    // A column only one workbook has already says which one is meant.
    match data_only(&lab, "Quelle est la somme de montant ?", "fr-FR") {
        Some(PendingAnswer::Done(TabularAnswer::Value {
            value: TabularValue::Sum(total),
            ..
        })) => assert_eq!(total.value, 1500.0),
        _ => panic!("the amount column is the invoices' alone"),
    }
}

#[tokio::test]
async fn how_many_documents_is_never_read_as_a_row_count() {
    let mut lab = Lab::new();
    lab.write("courrier-dupont.txt", "Courrier de Jean Dupont.");
    lab.write("notes-internes.txt", "Notes internes.");
    lab.write("compte-rendu.txt", "Compte rendu.");
    lab.write_data("rdv.csv", APPOINTMENTS);
    lab.analyse().await;
    lab.analyse_data();
    let question = "Combien de documents ai-je ?";
    assert!(data_only(&lab, question, "fr-FR").is_none());
    // The folder answers it, from the selected documents.
    let full = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index)).unwrap();
    let selection = Selection {
        full: &full,
        scoped: &full,
        narrowed: false,
        no_documents_chosen: false,
    };
    match folder_questions::answer_in_mixed_selection(&selection, &NoCorpus, question, "fr-FR") {
        Some(FolderAnswer::FileCount { total })
        | Some(FolderAnswer::IndexedCount { count: total }) => {
            assert_eq!(total, 3)
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_folder_answers_a_mixed_selection_only_about_the_documents_themselves() {
    let lab = documents_and_two_workbooks();
    let full = WorkFolderInventory::discover(lab.work_path(), None).unwrap();
    let selection = Selection {
        full: &full,
        scoped: &full,
        narrowed: false,
        no_documents_chosen: false,
    };
    for question in [
        "Que disent mes documents \u{e0} propos de Jean Dupont ?",
        "Combien de lignes dans rdv.csv ?",
        "Combien de lignes ?",
        "Que dit rdv.csv ?",
    ] {
        assert_eq!(
            folder_questions::answer_in_mixed_selection(&selection, &NoCorpus, question, "fr-FR"),
            None,
            "{question:?}"
        );
    }
}

// ------------------------------------------------------------------------------ the human test

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn summary_of(answer: &TabularAnswer) -> String {
    match answer {
        TabularAnswer::Value { value, locator, .. } => {
            format!("{value:?} filters={:?}", locator.filters)
        }
        other => format!("{other:?}"),
    }
}

/// Prints what the application answers to the questions of `human-tests/fix-04-batch-3-...md`, on the folders
/// `lot11_helper.py prepare 30` and `fix04_batch3_helper.py files` wrote (`ACAI_WALK_DOCS`, `ACAI_WALK_DATA`).
/// `cargo test --test knowledge_fix_retrieval print_the_answers_of_the_human_test -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn print_the_answers_of_the_human_test() {
    let docs = std::env::var("ACAI_WALK_DOCS").expect("ACAI_WALK_DOCS");
    let data = std::env::var("ACAI_WALK_DATA").expect("ACAI_WALK_DATA");
    let mut lab = Lab::new();
    copy_tree(std::path::Path::new(&docs), lab.work_path());
    copy_tree(std::path::Path::new(&data), lab.data.path());
    lab.analyse().await;
    let summary = lab.analyse_data();
    println!("data pass: {summary:?}");
    let inventory = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index)).unwrap();
    println!("documents indexed: {}", inventory.indexed_files().len());

    println!("--- F-15, F-12 (one workbook named)");
    for question in [
        "Combien de lignes dans rdv-export-titre.csv ?",
        "Combien de lignes pour Dr Jean Dupont dans rdv-medecins.csv ?",
        "Combien de lignes pour Jean Dupont dans rdv-medecins.csv ?",
        "Combien de lignes pour Dr Paul Martin dans rdv-medecins.csv ?",
        "Quelle est la somme de montant pour Dr Jean Dupont dans rdv-medecins.csv ?",
        "Combien de lignes pour Martin dans rdv-medecins.csv ?",
        "Combien de lignes pour Alice Archer en mars 2026 dans rdv-medecins.csv ?",
        "Combien de lignes pour Alice Archer dans rdv-dates-ambigues.csv ?",
        "Combien de lignes pour Alice Archer en mars 2026 dans rdv-dates-ambigues.csv ?",
        "Combien de lignes en 2026 dans rdv-dates-ambigues.csv ?",
        "Combien de lignes pour Alice Archer en mars dans rdv-dates-ambigues.csv ?",
    ] {
        println!(
            "{question}\n    -> {}",
            summary_of(&ask_data(&lab, question))
        );
    }

    println!("--- F-23 (routing in the documents)");
    for question in [
        "Que dit mars/neurologie.txt ?",
        "Que dit 2026/mars/neurologie.txt ?",
        "Que dit neurologie.txt ?",
        "Que dit avril/neurologie.txt ?",
    ] {
        let routed = folder_questions::route(&inventory, &lab.index, question, "fr-FR");
        println!("{question}\n    -> {routed:?}");
    }

    println!("--- F-13 (documents and all the workbooks)");
    let selection = Selection {
        full: &inventory,
        scoped: &inventory,
        narrowed: false,
        no_documents_chosen: false,
    };
    for question in [
        "Combien de lignes ?",
        "Combien de documents ai-je ?",
        "Que disent mes documents \u{e0} propos de Jean Dupont ?",
        "Quelle est la somme de montant ?",
    ] {
        let data_only = match data_only(&lab, question, "fr-FR") {
            None => "None (the mixed tier)".to_string(),
            Some(PendingAnswer::Done(answer)) => format!("data-only: {}", summary_of(&answer)),
            Some(_) => "data-only: model".to_string(),
        };
        let folder =
            folder_questions::answer_in_mixed_selection(&selection, &lab.index, question, "fr-FR");
        println!("{question}\n    -> {data_only}\n    -> folder: {folder:?}");
    }
}
