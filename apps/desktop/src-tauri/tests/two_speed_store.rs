//! Lot 4 of the long-documents programme, the store: a passage can be written before its vector exists
//! (the vector is absent, NULL, never an empty blob), the vectors are stored by batch, an index from before
//! is left alone until the Analyse upgrades it, and a search over passages without a vector says so.

use assistant_cabinet_ai_lib::chunking::Chunk;
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::retrieval::search;
use rusqlite::Connection;
use sha2::{Digest, Sha256};

fn chunk(path: &str, page: u32, section: u32, text: &str) -> Chunk {
    Chunk {
        chunk_id: format!("{path}#p{page}#s{section}"),
        relative_path: path.to_string(),
        page_number: page,
        section,
        text: text.to_string(),
        index_text: None,
        origin: PageOrigin::TextLayer,
        confidence: None,
    }
}

fn vector(seed: f32) -> Vec<f32> {
    vec![seed, 1.0 - seed, 0.5]
}

fn dump(connection: &Connection) -> String {
    let mut hasher = Sha256::new();
    for sql in [
        "SELECT relative_path, sha256, empty FROM documents ORDER BY relative_path",
        "SELECT chunk_id, page_number, section, text, embedding FROM chunks ORDER BY chunk_id",
        "SELECT sql FROM sqlite_master WHERE name = 'chunks'",
    ] {
        let mut statement = connection.prepare(sql).unwrap();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for i in 0..row.as_ref().column_count() {
                hasher.update(format!("{:?}|", row.get_ref(i).unwrap()));
            }
        }
    }
    format!("{:x}", hasher.finalize())
}

/// The layout of an index written before the vectors could be absent.
fn old_index(path: &std::path::Path) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE documents (relative_path TEXT PRIMARY KEY, sha256 TEXT NOT NULL, empty INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE chunks (chunk_id TEXT PRIMARY KEY, relative_path TEXT NOT NULL, page_number INTEGER NOT NULL,
                section INTEGER NOT NULL, text TEXT NOT NULL, embedding BLOB NOT NULL,
                origin TEXT NOT NULL DEFAULT 'text_layer', confidence REAL);
             CREATE VIRTUAL TABLE chunks_fts USING fts5(chunk_id UNINDEXED, text);",
        )
        .unwrap();
}

#[test]
fn a_new_index_can_hold_a_passage_without_a_vector() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    assert!(store.vectors_nullable());
    let chunks = vec![
        chunk("a.txt", 1, 1, "first words"),
        chunk("a.txt", 1, 2, "second words"),
        chunk("a.txt", 2, 1, "third words"),
    ];
    store
        .replace_document_pending(
            "a.txt",
            "hash",
            false,
            &chunks,
            &[Some(vector(0.1)), None, None],
            None,
            None,
        )
        .unwrap();

    let counts = store.vector_counts().unwrap();
    assert_eq!((counts.done, counts.total), (1, 3));
    let all = store.all_chunks().unwrap();
    assert_eq!(
        all.len(),
        3,
        "every passage is stored, with or without a vector"
    );
    assert!(all.iter().filter(|c| c.embedding.is_empty()).count() == 2);
    let side = Connection::open(store.path()).unwrap();
    let stored: i64 = side
        .query_row(
            "SELECT COUNT(*) FROM chunks WHERE embedding IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, 2, "absent means NULL, not an empty blob");
}

#[test]
fn the_vectors_are_stored_by_batch_in_reading_order_and_a_gone_passage_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    let a = vec![chunk("a.txt", 1, 1, "a one"), chunk("a.txt", 2, 1, "a two")];
    let b = vec![chunk("b.txt", 1, 1, "b one")];
    store
        .replace_document_pending("b.txt", "hb", false, &b, &[None], None, None)
        .unwrap();
    store
        .replace_document_pending("a.txt", "ha", false, &a, &[None, None], None, None)
        .unwrap();

    let pending = store.pending_chunks(2).unwrap();
    let ids: Vec<&str> = pending.iter().map(|p| p.chunk_id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["a.txt#p1#s1", "a.txt#p2#s1"],
        "by file, page and rank, not by write order"
    );
    assert_eq!(pending[0].text, "a one");

    store
        .store_vectors(&[
            ("a.txt#p1#s1".to_string(), vector(0.2)),
            ("gone.txt#p1#s1".to_string(), vector(0.3)),
        ])
        .unwrap();
    let counts = store.vector_counts().unwrap();
    assert_eq!((counts.done, counts.total), (1, 3));
    let next: Vec<String> = store
        .pending_chunks(10)
        .unwrap()
        .into_iter()
        .map(|p| p.chunk_id)
        .collect();
    assert_eq!(
        next,
        vec!["a.txt#p2#s1".to_string(), "b.txt#p1#s1".to_string()]
    );
    assert_eq!(store.vector_counts_of("a.txt").unwrap().done, 1);
    assert_eq!(store.vector_counts_of("a.txt").unwrap().total, 2);
}

#[test]
fn an_index_from_before_is_untouched_until_the_upgrade_and_the_upgrade_loses_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite3");
    old_index(&path);
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "INSERT INTO chunks VALUES ('a.txt#p1#s1', 'a.txt', 1, 1, 'old words', ?1, 'text_layer', NULL)",
                [vec![0u8, 0, 128, 63, 0, 0, 0, 64]],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO chunks_fts (chunk_id, text) VALUES ('a.txt#p1#s1', 'old words')",
                [],
            )
            .unwrap();
        connection
            .execute("INSERT INTO documents VALUES ('a.txt', 'h', 0)", [])
            .unwrap();
    }
    let before = dump(&Connection::open(&path).unwrap());

    let mut store = IndexStore::open_at(&path).unwrap();
    assert!(
        !store.vectors_nullable(),
        "an old index is recognised by its layout"
    );
    assert_eq!(
        dump(&Connection::open(&path).unwrap()),
        before,
        "opening it changes nothing"
    );
    // And it still answers, as before.
    assert_eq!(store.all_chunks().unwrap()[0].embedding, vec![1.0, 2.0]);

    assert!(store.upgrade_layout().unwrap(), "the Analyse upgrades it");
    assert!(store.vectors_nullable());
    let after = store.all_chunks().unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(
        (after[0].text.as_str(), after[0].embedding.clone()),
        ("old words", vec![1.0, 2.0])
    );
    let paths: Vec<String> = store
        .all_documents()
        .unwrap()
        .into_iter()
        .map(|d| d.relative_path)
        .collect();
    let hits = store
        .search_lexical_terms_in(&["words".to_string()], 5, &paths)
        .unwrap();
    assert_eq!(hits.len(), 1, "the word index is intact");
    assert!(
        !store.upgrade_layout().unwrap(),
        "and a second upgrade does nothing"
    );
}

#[test]
fn a_search_finds_the_passages_with_a_vector_by_meaning_and_says_how_many_it_could_not_weigh() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    let chunks = vec![
        chunk("a.txt", 1, 1, "the registry of the nightingale cabinet"),
        chunk("a.txt", 1, 2, "something else entirely about invoices"),
    ];
    store
        .replace_document_pending(
            "a.txt",
            "h",
            false,
            &chunks,
            &[None, Some(vector(0.9))],
            None,
            None,
        )
        .unwrap();
    let found = search(&store, "nightingale registry", &vector(0.9)).unwrap();
    assert!(
        found.iter().any(|e| e.chunk_id == "a.txt#p1#s2"),
        "the passage with a vector is found by its meaning"
    );
    assert!(
        !found.iter().any(|e| e.chunk_id == "a.txt#p1#s1"),
        "the passage with no vector is not found by its words: nothing but the vector places it"
    );

    let outcome = assistant_cabinet_ai_lib::retrieval::search_scoped_counted(
        &store,
        "nightingale registry",
        &vector(0.9),
        assistant_cabinet_ai_lib::retrieval::RetrievalScope::WholeFolder,
    )
    .unwrap();
    assert_eq!(
        outcome.unvectorised_chunks, 1,
        "the machine code the interface turns into a sentence"
    );
}
