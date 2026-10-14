//! Tab 1 of the long-documents chain, item 4: the rank sentence reads the profile. The document date written
//! in the document outranks the one in the file name, each date says what it comes from, a noun that stands
//! for a business type filters by the profile's type, and a file the Analyse has not read is still ranked
//! by its name. Every name is invented.

mod common;

use assistant_cabinet_ai_lib::folder_questions::{route, FolderAnswer, NoCorpus, QuestionRoute};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use common::knowledge::Lab;

const LOCALE: &str = "fr-FR";

const INVOICE_MARCH_NAME: &str = "Facture FAC-2026-014\nMedSupply SA\nFacture n FAC-2026-014\nDate : 20 mai 2026\nClient : Cabinet medical des Voirons\nTotal HT : CHF 326.30\n";
const INVOICE_NO_DATE_IN_NAME: &str = "Facture FAC-2026-020\nMedSupply SA\nFacture n FAC-2026-020\nDate : 2 juin 2026\nClient : Cabinet medical des Voirons\nTotal HT : CHF 118.00\n";
const LETTER: &str =
    "Courrier du 3 juin 2026\nHopitaux universitaires de Geneve\nConsultation de dermatologie.\n";

async fn lab() -> Lab {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    // The header says 20 May, the name says 1 March: the document says 20 May.
    lab.write("2026-03-01_facture-medsupply.txt", INVOICE_MARCH_NAME);
    // An invoice whose name holds neither the word nor a date.
    lab.write("medsupply-fac-020.txt", INVOICE_NO_DATE_IN_NAME);
    lab.write("2026-06-03_courrier-dermatologie.txt", LETTER);
    lab.write(
        "courrier-sans-date.txt",
        "Courrier\nMerci de votre visite.\n",
    );
    lab.analyse().await;
    lab
}

fn ranking(lab: &Lab, question: &str) -> FolderAnswer {
    let inventory = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index))
        .expect("discovers the folder");
    match route(&inventory, &lab.index, question, LOCALE) {
        QuestionRoute::Deterministic(answer @ FolderAnswer::RankedFile { .. }) => answer,
        other => panic!("{question:?} is a ranking, not {other:?}"),
    }
}

fn entries(answer: &FolderAnswer) -> Vec<(String, String, String)> {
    let FolderAnswer::RankedFile { ranking, .. } = answer else {
        panic!("expected a ranking");
    };
    ranking
        .iter()
        .map(|entry| {
            (
                entry.relative_path.clone(),
                entry.date.clone(),
                entry.basis.clone(),
            )
        })
        .collect()
}

#[tokio::test]
async fn the_date_written_in_the_document_outranks_the_one_in_its_name_and_says_where_it_comes_from(
) {
    let lab = lab().await;
    let answer = ranking(&lab, "Quelle est la derni\u{e8}re facture ?");
    let found = entries(&answer);
    // The noun is a business type: both invoices are found, the one whose name holds no "facture" too.
    assert_eq!(
        found
            .iter()
            .map(|(path, _, _)| path.as_str())
            .collect::<Vec<_>>(),
        vec!["medsupply-fac-020.txt", "2026-03-01_facture-medsupply.txt"],
        "{found:?}"
    );
    assert_eq!(found[0].1, "2026-06-02");
    assert!(found[0].2.starts_with("header_"), "{found:?}");
    // 20 May from the header, not 1 March from the name.
    assert_eq!(found[1].1, "2026-05-20");
    assert!(found[1].2.starts_with("header_"), "{found:?}");
    let FolderAnswer::RankedFile {
        by_type,
        mixed_basis,
        candidates,
        ..
    } = answer
    else {
        unreachable!()
    };
    assert!(by_type);
    assert!(!mixed_basis, "no copy date among them");
    assert_eq!(candidates, 2);
}

#[tokio::test]
async fn a_noun_that_is_no_type_still_filters_by_the_words_of_the_path_and_a_missing_date_falls_back(
) {
    let lab = lab().await;
    let answer = ranking(&lab, "Quel est le dernier courrier re\u{e7}u ?");
    let found = entries(&answer);
    let FolderAnswer::RankedFile { by_type, .. } = &answer else {
        unreachable!()
    };
    assert!(!by_type, "courrier is a word of the path, not a type");
    assert_eq!(
        found
            .iter()
            .map(|(path, _, _)| path.as_str())
            .collect::<Vec<_>>()
            .first()
            .copied(),
        Some("courrier-sans-date.txt"),
        "the dateless letter takes the date of its copy, which is today: {found:?}"
    );
    assert_eq!(found[0].2, "modified");
    let dermatology = found
        .iter()
        .find(|(path, _, _)| path == "2026-06-03_courrier-dermatologie.txt")
        .expect("the dated letter is ranked");
    assert_eq!(dermatology.1, "2026-06-03");
    // A copy date stands among the dates of documents: the answer warns.
    let FolderAnswer::RankedFile { mixed_basis, .. } = answer else {
        unreachable!()
    };
    assert!(mixed_basis);
}

#[tokio::test]
async fn a_readable_file_without_a_profile_is_ranked_by_the_words_and_the_date_of_its_name() {
    let lab = lab().await;
    // An index written before the profiles existed: the file is read, its profile is not there.
    lab.side()
        .execute(
            "DELETE FROM file_profiles WHERE relative_path = '2026-03-01_facture-medsupply.txt'",
            [],
        )
        .expect("edits the profile");
    let found = entries(&ranking(&lab, "Quelle est la derni\u{e8}re facture ?"));
    assert_eq!(
        found
            .iter()
            .map(|(path, date, basis)| (path.as_str(), date.as_str(), basis.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("medsupply-fac-020.txt", "2026-06-02", "header_label"),
            ("2026-03-01_facture-medsupply.txt", "2026-03-01", "filename"),
        ]
    );
}

#[test]
fn without_an_index_the_ranking_reads_names_and_modification_dates_only() {
    let root = std::path::Path::new("unused");
    let empty = WorkFolderInventory::from_records(root, Vec::new());
    match route(
        &empty,
        &NoCorpus,
        "Quel est le dernier courrier re\u{e7}u ?",
        LOCALE,
    ) {
        QuestionRoute::Deterministic(FolderAnswer::RankedFile { ranking, .. }) => {
            assert!(ranking.is_empty());
        }
        other => panic!("a ranking, not {other:?}"),
    }
}
