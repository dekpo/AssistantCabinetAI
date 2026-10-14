//! Tab 1 of the long-documents chain, item 5: with documents and tables both selected, the sentence that asks
//! which files talk about X answers for both folders, one list each, each stating its folder. No model and no
//! embedding. Every document is invented.

mod common;

use assistant_cabinet_ai_lib::discovery::discover_all;
use assistant_cabinet_ai_lib::file_profile::build::Domain;
use assistant_cabinet_ai_lib::finder::names::NoNames;
use assistant_cabinet_ai_lib::finder::set_search::answer_both;
use assistant_cabinet_ai_lib::finder::InventoryFile;
use assistant_cabinet_ai_lib::folder_questions::{set_question, FolderAnswer};
use common::knowledge::{tabular_snapshot, Lab};

const LOCALE: &str = "fr-FR";

fn inventory(folder: &std::path::Path) -> Vec<InventoryFile> {
    discover_all(folder)
        .iter()
        .map(InventoryFile::from)
        .collect()
}

fn search(answer: &FolderAnswer) -> (Domain, usize, Vec<String>, usize) {
    let FolderAnswer::DocumentSearch {
        domain,
        total,
        listed,
        others,
        coverage,
        ..
    } = answer
    else {
        panic!("expected one search, got {answer:?}");
    };
    let mut paths: Vec<String> = listed
        .iter()
        .map(|d| d.relative_path.clone())
        .chain(others.iter().cloned())
        .collect();
    paths.sort();
    (*domain, *total, paths, coverage.unanalysed)
}

#[tokio::test]
async fn a_list_of_the_files_that_talk_about_x_covers_the_documents_and_the_tables_each_in_its_folder(
) {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write(
        "2026-05-20_courrier-dermatologie.txt",
        "Courrier\nConsultation de dermatologie le 20 mai.\n",
    );
    lab.write("2026-05-21_note.txt", "Note sans rapport.\n");
    lab.analyse().await;
    lab.write_data(
        "rendez-vous.csv",
        "Patient;Date du rendez-vous;Motif\nMarie Martin;2026-05-20;suivi dermatologie\nPaul Roy;2026-06-02;controle\n",
    );
    lab.write_data("tarifs.csv", "Acte;Montant\nconsultation;30\nradio;45\n");
    lab.analyse_data();
    // A workbook added after the Analyse: findable by its name only, and counted.
    lab.write_data("dermatologie-a-venir.csv", "A;B\n1;2\n");

    let documents = inventory(lab.work_path());
    let workbooks = inventory(lab.data.path());
    let connection = lab.side();
    let before = tabular_snapshot(&connection);
    let requests = lab.gateway.requests().len();
    let set =
        set_question("Quels documents parlent de dermatologie ?", LOCALE).expect("a set question");

    let both = answer_both(
        &connection,
        &documents,
        &workbooks,
        &set,
        Vec::new(),
        &NoNames,
    )
    .expect("answers");
    let FolderAnswer::DocumentSearchBoth {
        documents: in_documents,
        workbooks: in_workbooks,
    } = &both
    else {
        panic!("expected both folders, got {both:?}");
    };
    assert_eq!(
        search(in_documents),
        (
            Domain::Documents,
            1,
            vec!["2026-05-20_courrier-dermatologie.txt".to_string()],
            0
        )
    );
    // The word of a cell is found, and so is the workbook that is only named for it.
    let (domain, total, found, unanalysed) = search(in_workbooks);
    assert_eq!(domain, Domain::Data);
    assert_eq!(
        found,
        vec![
            "dermatologie-a-venir.csv".to_string(),
            "rendez-vous.csv".to_string()
        ]
    );
    assert_eq!(total, 2);
    assert_eq!(unanalysed, 1, "the workbook not analysed yet is said so");

    assert_eq!(
        before,
        tabular_snapshot(&lab.side()),
        "the spreadsheet tables are only read"
    );
    assert_eq!(lab.gateway.requests().len(), requests, "no gateway call");
}
