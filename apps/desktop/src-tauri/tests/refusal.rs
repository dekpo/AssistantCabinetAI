//! Lot E of the retrieval-quality programme: a question about a name or a code that no document holds is
//! refused by the application, with no embedding and no model; anything the documents do hold, in a passage
//! or in a file name, is never refused by this check.

mod common;

use assistant_cabinet_ai_lib::refusal::{check, Verdict};
use common::knowledge::Lab;

async fn lab() -> Lab {
    let mut lab = Lab::new();
    lab.write(
        "2026-03-05_facture-medsupply.txt",
        "Facture FAC-2026-014 de MedSupply.\nMontant total : 300 francs. Reglement sous trente jours.\n",
    );
    lab.write(
        "2026-04-01_courrier-claire-martin.txt",
        "Courrier de la caisse a Claire Martin.\nLa franchise annuelle est de trois cents francs.\n",
    );
    lab.write(
        "dossier-zenobie-quarante.txt",
        "Note sans nom propre dans le texte.\nRendez-vous le douze mars.\n",
    );
    lab.analyse().await;
    lab
}

fn unknown(verdict: Verdict) -> Vec<String> {
    match verdict {
        Verdict::Unknown(words) => words,
        Verdict::Proceed => Vec::new(),
    }
}

#[tokio::test]
async fn a_code_no_document_holds_is_refused_and_one_it_holds_is_not() {
    let lab = lab().await;
    let missing = check(&lab.index, "Montre la facture FAC-2026-999").unwrap();
    assert_eq!(unknown(missing), vec!["FAC-2026-999".to_string()]);
    let present = check(&lab.index, "Montre la facture FAC-2026-014").unwrap();
    assert_eq!(present, Verdict::Proceed);
}

#[tokio::test]
async fn a_code_is_known_only_as_a_whole_not_by_its_parts() {
    let lab = lab().await;
    // « 2026 » and « 014 » are both written, but not as this code
    let verdict = check(&lab.index, "Quel est le total de FAC-014-2026 ?").unwrap();
    assert_eq!(unknown(verdict), vec!["FAC-014-2026".to_string()]);
}

#[tokio::test]
async fn a_person_nobody_wrote_about_is_refused() {
    let lab = lab().await;
    let verdict = check(&lab.index, "Que dit le dossier de Hyacinthe Vaudroz ?").unwrap();
    assert_eq!(
        unknown(verdict),
        vec!["Hyacinthe".to_string(), "Vaudroz".to_string()]
    );
}

#[tokio::test]
async fn one_known_name_is_enough_so_a_misspelling_next_to_a_known_name_is_not_refused() {
    let lab = lab().await;
    let verdict = check(
        &lab.index,
        "Quelle correspondance entre Claire Dupontet et Martin ?",
    )
    .unwrap();
    assert_eq!(
        verdict,
        Verdict::Proceed,
        "Claire and Martin are in the documents"
    );
}

#[tokio::test]
async fn a_name_that_is_only_in_a_file_name_is_known() {
    let lab = lab().await;
    let verdict = check(&lab.index, "Que dit le dossier de Zenobie Quarante ?").unwrap();
    assert_eq!(
        verdict,
        Verdict::Proceed,
        "the profile keeps the words of the name of the file"
    );
}

#[tokio::test]
async fn a_question_with_nothing_distinctive_is_never_refused_here() {
    let lab = lab().await;
    for question in [
        "Quelle est la franchise annuelle ?",
        "quels sont les horaires du cabinet ?",
        "Combien de documents ai-je ?",
    ] {
        assert_eq!(
            check(&lab.index, question).unwrap(),
            Verdict::Proceed,
            "{question}"
        );
    }
}
