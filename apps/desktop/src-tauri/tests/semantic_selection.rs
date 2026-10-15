//! The excerpts the model reads are chosen by the meaning of the question (owner decision, 15 October 2026):
//! the cosine similarity of the passage, plus the passages of the entities the question resolved to, plus the
//! identifier and the facts written in it. The words of the question choose nothing by themselves.

use assistant_cabinet_ai_lib::chunking::Chunk;
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::retrieval::{
    search_scoped_counted_with_terms, RetrievalLimits, RetrievalScope,
};

fn chunk(path: &str, n: u32, text: &str) -> Chunk {
    Chunk {
        chunk_id: format!("{path}#p{n}#s1"),
        relative_path: path.to_string(),
        page_number: n,
        section: 1,
        text: text.to_string(),
        index_text: None,
        origin: PageOrigin::TextLayer,
        confidence: None,
    }
}

/// One document per passage, each with the two-dimensional vector given: the question below points along the
/// first axis, so the first number of a vector is its cosine similarity to the question.
fn index_of(passages: &[(&str, &str, [f32; 2])]) -> (IndexStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    for (path, text, vector) in passages {
        let norm = (vector[0] * vector[0] + vector[1] * vector[1]).sqrt();
        let unit = vec![vector[0] / norm, vector[1] / norm];
        store
            .replace_document(
                path,
                "hash",
                false,
                &[chunk(path, 1, text)],
                &[unit],
                None,
                None,
            )
            .unwrap();
    }
    (store, dir)
}

const QUESTION_VECTOR: [f32; 2] = [1.0, 0.0];

fn search(index: &IndexStore, question: &str, entity_forms: &[String]) -> Vec<(String, f32)> {
    search_scoped_counted_with_terms(
        index,
        question,
        entity_forms,
        &QUESTION_VECTOR,
        RetrievalScope::WholeFolder,
        RetrievalLimits::BASELINE,
    )
    .unwrap()
    .evidence
    .into_iter()
    .map(|e| (e.relative_path, e.score))
    .collect()
}

#[test]
fn a_passage_that_shares_the_words_of_the_question_gets_no_bonus_for_it() {
    // `words.txt` repeats the words of the question and points elsewhere in meaning (cosine 0.8); `meaning.txt`
    // shares none of them and points the way of the question (cosine 0.9). Before, the words added 0.2 and put
    // `words.txt` first; now only the meaning counts.
    let (index, _dir) = index_of(&[
        (
            "words.txt",
            "table matieres chapitres trouver pouvoir fin document",
            [0.8, 0.6],
        ),
        (
            "meaning.txt",
            "Sommaire: 1 Introduction, 2 Methode",
            [0.9, 0.436_f32],
        ),
    ]);
    let found = search(
        &index,
        "Si tu dois pouvoir trouver la table matieres chapitres a la fin de ce document",
        &[],
    );
    let paths: Vec<&str> = found.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(paths.first().copied(), Some("meaning.txt"), "{found:?}");
    let words_score = found.iter().find(|(p, _)| p == "words.txt").unwrap().1;
    assert!(
        (words_score - 0.8).abs() < 0.01,
        "its score is its cosine, nothing added: {words_score}"
    );
}

#[test]
fn a_passage_far_in_meaning_is_not_brought_in_by_its_words() {
    let (index, _dir) = index_of(&[
        (
            "words.txt",
            "table matieres chapitres trouver pouvoir fin document",
            [0.05, 1.0],
        ),
        ("meaning.txt", "Sommaire: 1 Introduction", [1.0, 0.0]),
    ]);
    let found = search(
        &index,
        "table matieres chapitres trouver pouvoir fin document",
        &[],
    );
    let paths: Vec<&str> = found.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(paths, vec!["meaning.txt"], "{found:?}");
}

#[test]
fn a_code_written_in_the_question_still_pins_its_passage_first() {
    let (index, _dir) = index_of(&[
        (
            "facture.txt",
            "Facture FAC-2026-014 de MedSupply, 300 francs.",
            [0.0, 1.0],
        ),
        (
            "autre.txt",
            "Courrier general sur un autre sujet.",
            [1.0, 0.0],
        ),
    ]);
    let found = search(&index, "Que dit la facture FAC-2026-014 ?", &[]);
    assert_eq!(
        found.first().map(|(p, _)| p.as_str()),
        Some("facture.txt"),
        "{found:?}"
    );
}

#[test]
fn a_name_the_question_resolved_to_still_brings_the_passages_that_write_it() {
    let (index, _dir) = index_of(&[
        (
            "dupont.txt",
            "Compte rendu de la visite de Jean Dupont.",
            [0.0, 1.0],
        ),
        (
            "autre.txt",
            "Courrier general sur un autre sujet.",
            [1.0, 0.0],
        ),
    ]);
    // Searched over the files of the scope, as the targeting step does for an entity.
    let scope_paths = vec!["dupont.txt".to_string(), "autre.txt".to_string()];
    let found = search_scoped_counted_with_terms(
        &index,
        "Que dit le compte rendu ?",
        &["Jean Dupont".to_string(), "Dupont".to_string()],
        &QUESTION_VECTOR,
        RetrievalScope::Files(&scope_paths),
        RetrievalLimits::BASELINE,
    )
    .unwrap();
    let paths: Vec<&str> = found
        .evidence
        .iter()
        .map(|e| e.relative_path.as_str())
        .collect();
    assert!(paths.contains(&"dupont.txt"), "{paths:?}");
}

#[test]
fn a_question_about_an_unknown_name_and_code_still_returns_the_closest_passages_for_the_model() {
    // No refusal before the model: whatever the question names, the model receives the passages closest in
    // meaning and says itself that they do not answer.
    let (index, _dir) = index_of(&[(
        "lettre.txt",
        "Courrier de la caisse sur la franchise annuelle.",
        [0.7, 0.714],
    )]);
    let found = search(&index, "Que dit la facture FAC-2026-999 de Zircon-9 ?", &[]);
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn an_excerpt_carries_no_marked_words() {
    let (index, _dir) = index_of(&[("a.txt", "Sommaire", [1.0, 0.0])]);
    let found = search_scoped_counted_with_terms(
        &index,
        "sommaire",
        &[],
        &QUESTION_VECTOR,
        RetrievalScope::WholeFolder,
        RetrievalLimits::BASELINE,
    )
    .unwrap();
    let json = serde_json::to_value(&found.evidence[0]).unwrap();
    assert!(json.get("matches").is_none(), "{json}");
}
