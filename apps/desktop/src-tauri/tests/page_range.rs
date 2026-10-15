//! Lot A of the retrieval-quality programme: the page where a passage ends. It is stored beside the
//! structure (only for a passage that runs over a page break), follows its document, reaches the
//! evidence the search returns, and an index from before it reads every passage as one page.

use assistant_cabinet_ai_lib::chunking::Chunk;
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::retrieval::{self, RetrievalScope};
use assistant_cabinet_ai_lib::structure::Structure;

const PATH: &str = "docs/recueil.pdf";

fn chunk(id: &str, page: u32, ordinal: u32, text: &str) -> Chunk {
    Chunk {
        chunk_id: id.to_string(),
        relative_path: PATH.to_string(),
        page_number: page,
        section: ordinal,
        text: text.to_string(),
        index_text: None,
        origin: PageOrigin::TextLayer,
        confidence: None,
    }
}

/// Two passages: the first starts on page 10 and runs to page 11 (a table cut by the page break), the
/// second sits on page 12.
fn store() -> (IndexStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    let chunks = vec![
        chunk(
            "docs/recueil.pdf#p10#s1",
            10,
            1,
            "Site | Delai | Tarif\nSite1 | 3 | 40\nSite2 | 6 | 80",
        ),
        chunk(
            "docs/recueil.pdf#p12#s1",
            12,
            1,
            "Une phrase ordinaire sur autre chose.",
        ),
    ];
    store
        .replace_document(
            PATH,
            "hash",
            false,
            &chunks,
            &[vec![1.0, 0.0], vec![0.0, 1.0]],
            None,
            None,
        )
        .unwrap();
    (store, dir)
}

fn with_end(store: &mut IndexStore) {
    let structure = Structure {
        chunk_ends: vec![("docs/recueil.pdf#p10#s1".to_string(), 11)],
        ..Structure::default()
    };
    store.replace_structure(PATH, 1, 2, &structure).unwrap();
}

fn ids() -> Vec<String> {
    vec![
        "docs/recueil.pdf#p10#s1".to_string(),
        "docs/recueil.pdf#p12#s1".to_string(),
    ]
}

#[test]
fn the_end_page_of_a_passage_over_a_break_is_stored_and_the_others_have_none() {
    let (mut store, _dir) = store();
    with_end(&mut store);
    let ends = store.last_pages_of_chunks(&ids()).unwrap();
    assert_eq!(ends.len(), 1, "{ends:?}");
    assert_eq!(ends.get("docs/recueil.pdf#p10#s1"), Some(&11));
}

#[test]
fn an_index_from_before_the_end_pages_has_no_range_and_reads_one_page_per_passage() {
    let (store, _dir) = store();
    assert!(store.last_pages_of_chunks(&ids()).unwrap().is_empty());
    let found = retrieval::search_scoped(
        &store,
        "Tarif Delai",
        &[1.0, 0.0],
        RetrievalScope::WholeFolder,
    )
    .unwrap();
    assert!(!found.is_empty());
    for item in &found {
        assert_eq!(item.last_page, item.page_number, "{}", item.chunk_id);
    }
}

#[test]
fn the_search_returns_the_range_of_a_passage_over_a_break() {
    let (mut store, _dir) = store();
    with_end(&mut store);
    let found = retrieval::search_scoped(
        &store,
        "Tarif Delai",
        &[1.0, 0.0],
        RetrievalScope::WholeFolder,
    )
    .unwrap();
    let table = found
        .iter()
        .find(|item| item.chunk_id == "docs/recueil.pdf#p10#s1")
        .expect("the table passage is found");
    assert_eq!((table.page_number, table.last_page), (10, 11));
    if let Some(other) = found
        .iter()
        .find(|item| item.chunk_id == "docs/recueil.pdf#p12#s1")
    {
        assert_eq!((other.page_number, other.last_page), (12, 12));
    }
}

#[test]
fn a_document_written_again_loses_its_old_ranges() {
    let (mut store, _dir) = store();
    with_end(&mut store);
    let chunks = vec![chunk(
        "docs/recueil.pdf#p10#s1",
        10,
        1,
        "Un texte tout neuf.",
    )];
    store
        .replace_document(PATH, "other", false, &chunks, &[vec![1.0, 0.0]], None, None)
        .unwrap();
    assert!(
        store.last_pages_of_chunks(&ids()).unwrap().is_empty(),
        "a range belongs to the text it was cut from"
    );
}

#[test]
fn forgetting_the_documents_forgets_the_ranges() {
    let (mut store, _dir) = store();
    with_end(&mut store);
    store.clear().unwrap();
    assert!(store.last_pages_of_chunks(&ids()).unwrap().is_empty());
}
