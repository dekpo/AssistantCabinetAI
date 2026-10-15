//! Lot C of the retrieval-quality programme, the assembly: a cap per section spreads the places over the
//! sections of a long document, the passage after the best one is read with it, and the best passages take
//! the two ends of the list the model reads. Each only when its own switch is on; the number of passages and
//! of characters never grows. The identifier of the question comes first. The options are process-wide, so the
//! tests of this file take turns.
//!
//! The passages are ranked by their meaning alone, so every fixture gives each passage the similarity to the
//! question it should have (`vector_for`).

use std::sync::Mutex;

use assistant_cabinet_ai_lib::assembly::{set_assembly_options_for_measurement, AssemblyOptions};
use assistant_cabinet_ai_lib::chunking::Chunk;
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::retrieval::{self, Evidence, RetrievalLimits, RetrievalScope};
use assistant_cabinet_ai_lib::structure::{Section, SectionSource, Structure};

static TURN: Mutex<()> = Mutex::new(());

/// The question points along the first axis, so a passage whose vector is `vector_for(c)` has the cosine
/// similarity `c` to it.
fn vector_for(cosine: f32) -> Vec<f32> {
    vec![cosine, (1.0 - cosine * cosine).sqrt()]
}

/// A document made of passages, each with the similarity to the question given; every passage starts in the one
/// section « Part » when `sectioned`.
fn write(store: &mut IndexStore, path: &str, passages: &[(&str, f32)], sectioned: bool) {
    let chunks: Vec<Chunk> = passages
        .iter()
        .enumerate()
        .map(|(n, (text, _))| Chunk {
            chunk_id: format!("{path}#p1#s{}", n + 1),
            relative_path: path.to_string(),
            page_number: 1,
            section: n as u32 + 1,
            text: (*text).to_string(),
            index_text: None,
            origin: PageOrigin::TextLayer,
            confidence: None,
        })
        .collect();
    let vectors: Vec<Vec<f32>> = passages
        .iter()
        .map(|(_, cosine)| vector_for(*cosine))
        .collect();
    store
        .replace_document(path, "hash", false, &chunks, &vectors, None, None)
        .unwrap();
    let structure = if sectioned {
        Structure {
            sections: vec![Section {
                position: 0,
                parent: None,
                level: 1,
                title: "Part".to_string(),
                first_page: 1,
                last_page: 1,
                source: SectionSource::Markdown,
                anchor_page: 1,
                anchor_paragraph: 0,
                located: true,
            }],
            chunk_sections: chunks.iter().map(|c| (c.chunk_id.clone(), 0)).collect(),
            chunk_ends: Vec::new(),
        }
    } else {
        Structure::default()
    };
    store.replace_structure(path, 1, 2, &structure).unwrap();
}

fn library() -> (IndexStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    // The long document: four passages close in meaning to the question, all in one section.
    write(
        &mut store,
        "long.md",
        &[
            ("warranty one", 0.99),
            ("warranty two", 0.98),
            ("warranty three", 0.97),
            ("warranty four", 0.96),
        ],
        true,
    );
    // Another document, a little less close.
    write(
        &mut store,
        "short.md",
        &[("a note that mentions the warranty once", 0.95)],
        true,
    );
    (store, dir)
}

fn limits(max_chunks: usize) -> RetrievalLimits {
    RetrievalLimits {
        max_chunks,
        max_chars: 10_000,
        max_per_document_chars: 10_000,
    }
}

fn search(store: &IndexStore, options: AssemblyOptions, max_chunks: usize) -> Vec<Evidence> {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    set_assembly_options_for_measurement(Some(options));
    let found = retrieval::search_scoped_counted_with_terms(
        store,
        "warranty",
        &[],
        &[1.0, 0.0],
        RetrievalScope::WholeFolder,
        limits(max_chunks),
    )
    .unwrap()
    .evidence;
    set_assembly_options_for_measurement(None);
    found
}

fn paths(found: &[Evidence]) -> Vec<&str> {
    found.iter().map(|e| e.relative_path.as_str()).collect()
}

#[test]
fn a_cap_of_one_per_section_gives_the_other_document_a_place() {
    let (store, _dir) = library();
    let off = search(&store, AssemblyOptions::OFF, 3);
    assert_eq!(paths(&off), vec!["long.md", "long.md", "long.md"]);
    let capped = search(
        &store,
        AssemblyOptions {
            per_section_cap: Some(1),
            ..AssemblyOptions::OFF
        },
        3,
    );
    assert_eq!(capped.len(), 3, "the total did not change");
    assert!(
        paths(&capped).contains(&"short.md"),
        "the other document is read too: {:?}",
        paths(&capped)
    );
    assert_eq!(
        capped[0].chunk_id, off[0].chunk_id,
        "the best passage is still the first"
    );
}

#[test]
fn the_cap_never_leaves_a_place_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    write(
        &mut store,
        "only.md",
        &[
            ("warranty one", 0.9),
            ("warranty two", 0.8),
            ("warranty three", 0.7),
            ("warranty four", 0.6),
        ],
        true,
    );
    let capped = search(
        &store,
        AssemblyOptions {
            per_section_cap: Some(1),
            ..AssemblyOptions::OFF
        },
        3,
    );
    assert_eq!(
        capped.len(),
        3,
        "one section, three places: the cap gives way"
    );
}

#[test]
fn a_passage_with_no_section_is_never_capped() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    write(
        &mut store,
        "plain.md",
        &[
            ("warranty one", 0.9),
            ("warranty two", 0.8),
            ("warranty three", 0.7),
        ],
        false,
    );
    write(
        &mut store,
        "other.md",
        &[("warranty elsewhere", 0.6)],
        false,
    );
    let capped = search(
        &store,
        AssemblyOptions {
            per_section_cap: Some(1),
            ..AssemblyOptions::OFF
        },
        3,
    );
    let off = search(&store, AssemblyOptions::OFF, 3);
    assert_eq!(
        capped.iter().map(|e| &e.chunk_id).collect::<Vec<_>>(),
        off.iter().map(|e| &e.chunk_id).collect::<Vec<_>>()
    );
}

#[test]
fn the_passage_after_the_best_one_takes_the_place_of_the_weakest() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    write(
        &mut store,
        "book.md",
        &[
            ("warranty and a first word", 0.8),
            ("warranty warranty warranty warranty warranty best", 0.99),
            ("a passage that says nothing of the subject", 0.05),
            ("warranty warranty warranty fourth", 0.7),
        ],
        false,
    );
    write(&mut store, "note.md", &[("warranty in a note", 0.6)], false);
    let off = search(&store, AssemblyOptions::OFF, 3);
    let best = off[0].chunk_id.clone();
    assert_eq!(best, "book.md#p1#s2");
    assert!(
        !off.iter().any(|e| e.chunk_id == "book.md#p1#s3"),
        "the passage after the best one is not ranked"
    );
    let with = search(
        &store,
        AssemblyOptions {
            neighbours: true,
            ..AssemblyOptions::OFF
        },
        3,
    );
    assert_eq!(with.len(), 3, "the total did not change");
    assert_eq!(with[0].chunk_id, best);
    assert_eq!(
        with[1].chunk_id, "book.md#p1#s3",
        "read right after the best one"
    );
}

#[test]
fn the_best_passages_take_the_two_ends_of_what_the_model_reads_and_the_interface_keeps_the_ranking()
{
    let (store, _dir) = library();
    let ranked = search(&store, AssemblyOptions::OFF, 4);
    assert_eq!(ranked.len(), 4);
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    set_assembly_options_for_measurement(Some(AssemblyOptions {
        edges_first: true,
        ..AssemblyOptions::OFF
    }));
    let for_model = retrieval::ordered_for_model(&ranked);
    set_assembly_options_for_measurement(None);
    let ids = |list: &[Evidence]| list.iter().map(|e| e.chunk_id.clone()).collect::<Vec<_>>();
    let ranking = ids(&ranked);
    assert_eq!(
        ids(&for_model),
        vec![
            ranking[0].clone(),
            ranking[2].clone(),
            ranking[3].clone(),
            ranking[1].clone()
        ]
    );
    // off: the order is the ranking
    assert_eq!(ids(&retrieval::ordered_for_model(&ranked)), ranking);
}

// ---- lot D: the identifier of the question first

fn first_paths(store: &IndexStore, question: &str, options: AssemblyOptions) -> Vec<String> {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    set_assembly_options_for_measurement(Some(options));
    let found = retrieval::search_scoped_counted_with_terms(
        store,
        question,
        &[],
        &[1.0, 0.0],
        RetrievalScope::WholeFolder,
        limits(6),
    )
    .unwrap()
    .evidence;
    set_assembly_options_for_measurement(None);
    found.into_iter().map(|e| e.relative_path).collect()
}

#[test]
fn the_passage_that_holds_the_identifier_of_the_question_comes_first_even_when_it_is_farther() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    write(
        &mut store,
        "near.md",
        &[("the invoice total of the month, FAC-2026-114", 1.0)],
        false,
    );
    write(
        &mut store,
        "code.md",
        &[("invoice FAC-2026-014 is due", 0.3)],
        false,
    );
    let question = "what is the total of invoice FAC-2026-014";
    let off = first_paths(&store, question, AssemblyOptions::OFF);
    assert_eq!(off.first().map(String::as_str), Some("near.md"));
    let pinned = first_paths(
        &store,
        question,
        AssemblyOptions {
            pin_identifier: true,
            ..AssemblyOptions::OFF
        },
    );
    assert_eq!(pinned.first().map(String::as_str), Some("code.md"));
    // a question with no identifier is not changed by the switch
    let plain = first_paths(
        &store,
        "what is the total of the month",
        AssemblyOptions {
            pin_identifier: true,
            ..AssemblyOptions::OFF
        },
    );
    let plain_off = first_paths(
        &store,
        "what is the total of the month",
        AssemblyOptions::OFF,
    );
    assert_eq!(plain, plain_off);
}
