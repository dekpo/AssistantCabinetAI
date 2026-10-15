//! Lot B of the retrieval-quality programme, the index side: the stems of a passage, the words of its section
//! path and its characters are written beside the full-text index only when the matching switch is on, are
//! found by the word search, follow their document, can be rebuilt from the stored text, and a search whose
//! table is empty answers as if its switch were off. The options are process-wide, so the tests of this file
//! take turns.

use std::sync::Mutex;

use assistant_cabinet_ai_lib::chunking::Chunk;
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::lexical::{set_lexical_options_for_measurement, LexicalOptions};
use assistant_cabinet_ai_lib::structure::{Section, SectionSource, Structure};

static TURN: Mutex<()> = Mutex::new(());

const STEMS: LexicalOptions = LexicalOptions {
    french_stems: true,
    ..LexicalOptions::OFF
};
const WEIGHTED: LexicalOptions = LexicalOptions {
    french_stems: true,
    field_weights: true,
    ..LexicalOptions::OFF
};
const TRIGRAM: LexicalOptions = LexicalOptions {
    trigram: true,
    ..LexicalOptions::OFF
};

struct Doc {
    path: &'static str,
    /// (passage text, title of the section it starts in)
    passages: Vec<(&'static str, Option<&'static str>)>,
}

fn write(store: &mut IndexStore, doc: &Doc) {
    let chunks: Vec<Chunk> = doc
        .passages
        .iter()
        .enumerate()
        .map(|(n, (text, _))| Chunk {
            chunk_id: format!("{}#p1#s{}", doc.path, n + 1),
            relative_path: doc.path.to_string(),
            page_number: 1,
            section: n as u32 + 1,
            text: (*text).to_string(),
            index_text: None,
            origin: PageOrigin::TextLayer,
            confidence: None,
        })
        .collect();
    let vectors: Vec<Vec<f32>> = chunks.iter().map(|_| vec![1.0, 0.0]).collect();
    store
        .replace_document(doc.path, "hash", false, &chunks, &vectors, None, None)
        .unwrap();
    let mut sections = Vec::new();
    let mut owners = Vec::new();
    for (n, (_, title)) in doc.passages.iter().enumerate() {
        if let Some(title) = title {
            let position = sections.len();
            sections.push(Section {
                position,
                parent: None,
                level: 1,
                title: (*title).to_string(),
                first_page: 1,
                last_page: 1,
                source: SectionSource::Markdown,
                anchor_page: 1,
                anchor_paragraph: 0,
                located: true,
            });
            owners.push((format!("{}#p1#s{}", doc.path, n + 1), position));
        }
    }
    store
        .replace_structure(
            doc.path,
            1,
            2,
            &Structure {
                sections,
                chunk_sections: owners,
                chunk_ends: Vec::new(),
            },
        )
        .unwrap();
}

fn store_with(options: LexicalOptions, docs: &[Doc]) -> (IndexStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    set_lexical_options_for_measurement(Some(options));
    for doc in docs {
        write(&mut store, doc);
    }
    (store, dir)
}

fn ids(store: &IndexStore, question: &str, limit: usize) -> Vec<String> {
    store
        .search_lexical(question, limit)
        .unwrap()
        .into_iter()
        .map(|hit| hit.chunk_id.trim_end_matches("#p1#s1").to_string())
        .collect()
}

/// Run `body` with the options set, alone in the process.
fn alone(options: LexicalOptions, body: impl FnOnce()) {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    set_lexical_options_for_measurement(Some(options));
    body();
    set_lexical_options_for_measurement(None);
}

fn letters() -> Vec<Doc> {
    vec![
        Doc {
            path: "a.md",
            passages: vec![("Les courriers recus au cabinet.", None)],
        },
        Doc {
            path: "b.md",
            passages: vec![("Un autre sujet sans aucun lien.", None)],
        },
    ]
}

#[test]
fn a_plural_and_its_singular_meet_through_the_stems_and_not_without_them() {
    alone(STEMS, || {
        let (store, _dir) = store_with(STEMS, &letters());
        assert_eq!(ids(&store, "courrier", 5), vec!["a.md".to_string()]);
        assert_eq!(ids(&store, "Quels courriers ?", 5), vec!["a.md".to_string()]);
    });
    alone(LexicalOptions::OFF, || {
        let (store, _dir) = store_with(LexicalOptions::OFF, &letters());
        assert!(ids(&store, "courrier", 5).is_empty(), "no stems, no match");
    });
}

#[test]
fn a_search_whose_stems_table_is_empty_answers_as_the_plain_search_does() {
    // The index was written with the switch off, then the switch was turned on: the table is empty.
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let (store, _dir) = store_with(LexicalOptions::OFF, &letters());
    set_lexical_options_for_measurement(Some(STEMS));
    assert!(ids(&store, "courrier", 5).is_empty());
    assert_eq!(ids(&store, "courriers", 5), vec!["a.md".to_string()]);
    set_lexical_options_for_measurement(None);
}

#[test]
fn the_stems_are_rebuilt_from_the_stored_text_when_a_switch_is_turned_on_and_only_then() {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let (mut store, _dir) = store_with(LexicalOptions::OFF, &letters());
    set_lexical_options_for_measurement(Some(STEMS));
    assert!(ids(&store, "courrier", 5).is_empty());
    let written = store.ensure_lexical_layout().unwrap();
    assert_eq!(written, Some(2), "two passages written, nothing read from a file");
    assert_eq!(ids(&store, "courrier", 5), vec!["a.md".to_string()]);
    assert_eq!(store.ensure_lexical_layout().unwrap(), None, "already right: nothing to do");
    // turned off again: the tables are emptied and the plain search is back
    set_lexical_options_for_measurement(Some(LexicalOptions::OFF));
    assert_eq!(store.ensure_lexical_layout().unwrap(), Some(0));
    assert!(ids(&store, "courrier", 5).is_empty());
    set_lexical_options_for_measurement(None);
}

#[test]
fn a_word_of_the_section_title_counts_more_than_the_same_word_in_the_body() {
    let docs = vec![
        Doc {
            path: "x.md",
            passages: vec![("Duree de deux ans a partir de la livraison.", Some("Garantie legale"))],
        },
        Doc {
            path: "y.md",
            passages: vec![("Cette clause evoque la garantie en passant, sans plus.", None)],
        },
    ];
    alone(WEIGHTED, || {
        let (store, _dir) = store_with(WEIGHTED, &docs);
        assert_eq!(
            ids(&store, "garantie", 5),
            vec!["x.md".to_string(), "y.md".to_string()],
            "the passage under the section « Garantie legale » comes first"
        );
    });
    alone(STEMS, || {
        let (store, _dir) = store_with(STEMS, &docs);
        assert_eq!(
            ids(&store, "garantie", 5),
            vec!["y.md".to_string()],
            "without the weights the section path is not searched"
        );
    });
}

#[test]
fn a_piece_of_a_word_finds_the_word_through_the_trigrams() {
    let docs = vec![Doc {
        path: "p.md",
        passages: vec![("Declaration annuelle de pharmacovigilance.", None)],
    }];
    alone(TRIGRAM, || {
        let (store, _dir) = store_with(TRIGRAM, &docs);
        assert_eq!(ids(&store, "pharmaco", 5), vec!["p.md".to_string()]);
    });
    alone(LexicalOptions::OFF, || {
        let (store, _dir) = store_with(LexicalOptions::OFF, &docs);
        assert!(ids(&store, "pharmaco", 5).is_empty());
    });
}

#[test]
fn a_document_written_again_leaves_no_stale_stems_behind() {
    alone(STEMS, || {
        let (mut store, _dir) = store_with(STEMS, &letters());
        write(
            &mut store,
            &Doc {
                path: "a.md",
                passages: vec![("Un texte entierement nouveau.", None)],
            },
        );
        assert!(ids(&store, "courrier", 5).is_empty(), "the old words are gone");
        assert_eq!(ids(&store, "nouveau", 5), vec!["a.md".to_string()]);
    });
}
