//! The retrieval lab's question about an AVS number written without dots (`docs/RETRIEVAL-LAB.md` 2.2): the
//! finder meets the stored form, whatever spelling the question typed. Alone in its file because it points
//! the sandbox loader at the lab's fixtures with an environment variable, which would race with any other test.

mod common;

use assistant_cabinet_ai_lib::discovery::discover_all;
use assistant_cabinet_ai_lib::file_profile::vocabulary::ProfileVocabulary;
use assistant_cabinet_ai_lib::finder::set_search::{answer, identifier_forms};
use assistant_cabinet_ai_lib::finder::InventoryFile;
use assistant_cabinet_ai_lib::folder_questions::{set_question, FolderAnswer};
use assistant_cabinet_ai_lib::knowledge::packs::PackSet;
use common::validation::{library, Truth};

const LOCALE: &str = "fr-FR";

fn inventory(folder: &std::path::Path) -> Vec<InventoryFile> {
    discover_all(folder)
        .iter()
        .map(InventoryFile::from)
        .collect()
}

#[tokio::test]
async fn the_lab_question_about_an_avs_number_without_dots_finds_its_file_and_the_absent_one_finds_none(
) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/retrieval-lab");
    std::env::set_var("ACAI_SANDBOX_DIR", &root);
    let truth = Truth::load();
    std::env::remove_var("ACAI_SANDBOX_DIR");
    let library = library(&truth, truth.docs.len(), &["health-fr", "health-ch"]).await;
    let files = inventory(library.lab.work_path());
    let connection = library.lab.side();
    let packs = PackSet::load(LOCALE, &["health-fr", "health-ch"]).expect("loads the packs");
    let vocabulary = ProfileVocabulary::load(&["health-fr", "health-ch"]).expect("loads");
    for (typed, expected) in [
        (
            "7561234567897",
            vec!["patients/2026-03-09_dossier-administratif-praz.docx"],
        ),
        ("7569999999991", Vec::new()),
    ] {
        let set = set_question(&format!("Quels documents contiennent {typed} ?"), LOCALE)
            .expect("a set question");
        let forms = identifier_forms(&set.text, &packs, &vocabulary);
        let found = answer(&connection, &files, &set, forms).expect("answers");
        let FolderAnswer::DocumentSearch { total, listed, .. } = &found else {
            unreachable!()
        };
        let listed: Vec<&str> = listed.iter().map(|d| d.relative_path.as_str()).collect();
        assert_eq!((*total, listed), (expected.len(), expected), "{typed}");
    }
}
