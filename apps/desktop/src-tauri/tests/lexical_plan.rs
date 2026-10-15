//! Lot B of the retrieval-quality programme, the query side: phrases and
//! synonyms change which passages the word search puts first, each only when its own switch is on, and
//! with every switch off the search is the one it always was. The options are process-wide, so the tests of
//! this file take turns.

use std::sync::Mutex;

use assistant_cabinet_ai_lib::chunking::Chunk;
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::lexical::{
    set_lexical_options_for_measurement, LexicalOptions,
};

static TURN: Mutex<()> = Mutex::new(());

fn store(passages: &[(&str, &str)]) -> (IndexStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    for (id, text) in passages {
        let chunk = Chunk {
            chunk_id: format!("{id}#p1#s1"),
            relative_path: format!("{id}.pdf"),
            page_number: 1,
            section: 1,
            text: (*text).to_string(),
            index_text: None,
            origin: PageOrigin::TextLayer,
            confidence: None,
        };
        store
            .replace_document(
                &format!("{id}.pdf"),
                "hash",
                false,
                &[chunk],
                &[vec![1.0, 0.0]],
                None,
                None,
            )
            .unwrap();
    }
    (store, dir)
}

fn first(store: &IndexStore, question: &str, limit: usize) -> Vec<String> {
    store
        .search_lexical(question, limit)
        .unwrap()
        .into_iter()
        .map(|hit| hit.chunk_id.trim_end_matches("#p1#s1").to_string())
        .collect()
}

fn with<T>(options: LexicalOptions, run: impl FnOnce() -> T) -> T {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    set_lexical_options_for_measurement(Some(options));
    let out = run();
    set_lexical_options_for_measurement(None);
    out
}

fn invoices() -> (IndexStore, tempfile::TempDir) {
    store(&[
        ("a", "Facture FAC-2026-114 du mois, montant 500 francs."),
        ("b", "Facture FAC-2026-014 du mois, montant 300 francs."),
        ("c", "Facture FAC-2026-116 du mois, montant 700 francs."),
    ])
}

#[test]
fn an_identifier_in_the_question_is_a_phrase_that_picks_its_own_passage() {
    let (store, _dir) = invoices();
    let question = "Quel est le montant de la facture FAC-2026-014 ?";
    // The three passages tie on the words the search keeps (the "014" of the code is dropped, three letters).
    let off = with(LexicalOptions::OFF, || first(&store, question, 3));
    assert_eq!(off.len(), 3);
    let on = with(
        LexicalOptions {
            phrases: true,
            ..LexicalOptions::OFF
        },
        || first(&store, question, 1),
    );
    assert_eq!(on, vec!["b".to_string()], "the code itself decides");
}

#[test]
fn with_every_switch_off_the_search_is_the_old_one_and_the_product_runs_with_the_phrases() {
    let (store, _dir) = invoices();
    let question = "Quel est le montant de la facture FAC-2026-014 ?";
    // Off: the three passages tie on the words the search keeps, so they come in the order they were stored.
    let off = with(LexicalOptions::OFF, || first(&store, question, 3));
    assert_eq!(off, vec!["a", "b", "c"]);
    // The product's own choice (nothing set) is the phrases, which put the code's passage first.
    let product = {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        set_lexical_options_for_measurement(None);
        first(&store, question, 3)
    };
    assert_eq!(product.first().map(String::as_str), Some("b"));
}

#[test]
fn a_synonym_of_the_active_packs_finds_the_passage_that_uses_the_other_word() {
    let (store, _dir) = store(&[
        ("a", "Le decompte de la LAMal indique la franchise restante."),
        ("b", "Courrier sans rapport avec le sujet."),
    ]);
    let question = "Que dit l'assurance maladie obligatoire ?";
    let off = with(LexicalOptions::OFF, || first(&store, question, 3));
    assert!(!off.contains(&"a".to_string()), "no shared word: {off:?}");
    let on = with(
        LexicalOptions {
            thesaurus: true,
            ..LexicalOptions::OFF
        },
        || first(&store, question, 3),
    );
    assert_eq!(on, vec!["a".to_string()]);
}

#[test]
fn two_words_side_by_side_beat_two_words_far_apart() {
    let (store, _dir) = store(&[
        ("a", "La franchise est reglee. Plus loin, en fin de page, la participation annuelle."),
        ("b", "Le montant de la franchise annuelle est de trois cents francs."),
    ]);
    let question = "franchise annuelle";
    let on = with(
        LexicalOptions {
            phrases: true,
            ..LexicalOptions::OFF
        },
        || first(&store, question, 1),
    );
    assert_eq!(on, vec!["b".to_string()]);
}
