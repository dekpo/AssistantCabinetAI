//! Lot C of the retrieval-quality programme: the reduction by passage, in shadow. The base knows in which
//! passage it found each mention of an entity (`kb_mentions.chunk_id`); `chunk_ids_mentioning` says which of
//! the passages an answer cited are those, so the diagnostics can record what a reduction by passage would
//! have kept. It is read only: nothing in the search uses it.

mod common;

use assistant_cabinet_ai_lib::knowledge::store as kb;
use common::knowledge::{entity_named, Lab};

/// Enough filler, with no name in it, to make the letter more than one passage.
fn filler(tag: &str) -> String {
    (0..30)
        .map(|n| format!("Ligne ordinaire numero {n} du paragraphe {tag} qui se poursuit avec les mots habituels."))
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn only_the_passages_that_hold_a_mention_of_the_entity_are_the_ones_a_reduction_would_keep() {
    let mut lab = Lab::new();
    let letter = format!(
        "Docteur Jean Dupont\nObjet : suivi\n\nMadame Claire Martin a été reçue le 12 mars 2026.\n{}\n\n{}\n",
        filler("un"),
        filler("deux")
    );
    lab.write("lettre.txt", &letter);
    lab.analyse().await;

    let connection = lab.side();
    let claire = entity_named(&connection, "Claire Martin").expect("the base knows her");
    let passages: Vec<String> = lab
        .index
        .chunks_for_document("lettre.txt")
        .unwrap()
        .into_iter()
        .map(|chunk| chunk.chunk_id)
        .collect();
    assert!(passages.len() >= 3, "the letter is several passages: {}", passages.len());

    let kept = kb::chunk_ids_mentioning(&connection, &[claire.entity_id], &passages).unwrap();
    assert!(!kept.is_empty(), "the passage that names her is found");
    assert!(
        kept.len() < passages.len(),
        "a reduction by passage would leave passages out: {} of {}",
        kept.len(),
        passages.len()
    );
    for id in &kept {
        assert!(passages.contains(id));
    }

    // nothing to ask: nothing returned, and an unknown entity mentions nothing
    assert!(kb::chunk_ids_mentioning(&connection, &[], &passages).unwrap().is_empty());
    assert!(kb::chunk_ids_mentioning(&connection, &[claire.entity_id], &[])
        .unwrap()
        .is_empty());
    assert!(kb::chunk_ids_mentioning(&connection, &[999_999], &passages)
        .unwrap()
        .is_empty());
}
