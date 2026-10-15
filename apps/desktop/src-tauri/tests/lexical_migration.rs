//! The stems and trigram tables of an earlier version were removed (`docs/RETRIEVAL-QUALITY.md`, "Methods
//! removed, and why"). An index written by that version still holds them, empty or not: opening it drops
//! them, leaves every other table and every passage as they were, and the word search still answers.

use assistant_cabinet_ai_lib::chunking::Chunk;
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use rusqlite::Connection;

fn tables(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare("SELECT name FROM sqlite_master WHERE name LIKE 'chunks_%' ORDER BY name")
        .unwrap();
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn an_index_with_the_removed_tables_loses_them_and_keeps_its_passages() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite3");
    {
        let mut store = IndexStore::open_at(&path).unwrap();
        let chunk = Chunk {
            chunk_id: "a.pdf#p1#s1".to_string(),
            relative_path: "a.pdf".to_string(),
            page_number: 1,
            section: 1,
            text: "Facture FAC-2026-014, montant 300 francs.".to_string(),
            index_text: None,
            origin: PageOrigin::TextLayer,
            confidence: None,
        };
        store
            .replace_document(
                "a.pdf",
                "hash",
                false,
                &[chunk],
                &[vec![1.0, 0.0]],
                None,
                None,
            )
            .unwrap();
    }
    // What the previous version had added: both tables, one of them filled, and the recorded layout.
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE VIRTUAL TABLE chunks_lex USING fts5(chunk_id UNINDEXED, section, stems);
                 CREATE VIRTUAL TABLE chunks_tri USING fts5(chunk_id UNINDEXED, body, tokenize = 'trigram');
                 INSERT INTO chunks_lex (chunk_id, section, stems) VALUES ('a.pdf#p1#s1', '', 'factur');
                 INSERT INTO file_profile_meta (key, value) VALUES ('lexical_layout', 'stems=1,trigram=0');",
            )
            .unwrap();
        assert!(tables(&connection).contains(&"chunks_lex".to_string()));
        assert!(tables(&connection).contains(&"chunks_tri".to_string()));
    }

    let store = IndexStore::open_at(&path).unwrap();
    let side = Connection::open(&path).unwrap();
    let left = tables(&side);
    assert!(
        !left.iter().any(|name| name.starts_with("chunks_lex")),
        "{left:?}"
    );
    assert!(
        !left.iter().any(|name| name.starts_with("chunks_tri")),
        "{left:?}"
    );
    let recorded: i64 = side
        .query_row(
            "SELECT COUNT(*) FROM file_profile_meta WHERE key = 'lexical_layout'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(recorded, 0);
    let paths: Vec<String> = store
        .all_documents()
        .unwrap()
        .into_iter()
        .map(|d| d.relative_path)
        .collect();
    let hits = store
        .search_lexical_terms_in(&["montant".to_string()], 3, &paths)
        .unwrap();
    assert_eq!(hits.len(), 1, "the passage is still in the full-text index");

    // Opening it again changes nothing.
    drop(store);
    IndexStore::open_at(&path).unwrap();
}
