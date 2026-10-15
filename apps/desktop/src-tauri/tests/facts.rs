//! Dates and amounts as facts: written once in a normal form by the Analyse, each with its passage, its page
//! and where it is written; a question that writes the same date or amount another way finds the passage;
//! a fact whose passage is gone is never returned; the document passages only. The options are process-wide,
//! so the tests that set them take turns.

mod common;

use std::sync::Mutex;

use assistant_cabinet_ai_lib::assembly::{set_assembly_options_for_measurement, AssemblyOptions};
use assistant_cabinet_ai_lib::chunking::Chunk;
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::facts::{self, shipped_vocabulary};
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::retrieval::{self, RetrievalLimits, RetrievalScope};
use common::knowledge::Lab;
use rusqlite::Connection;

static TURN: Mutex<()> = Mutex::new(());

fn store_with(documents: &[(&str, &str)]) -> (IndexStore, Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite3");
    let mut store = IndexStore::open_at(&path).unwrap();
    for (name, text) in documents {
        let chunk = Chunk {
            chunk_id: format!("{name}#p2#s1"),
            relative_path: (*name).to_string(),
            page_number: 2,
            section: 1,
            text: (*text).to_string(),
            index_text: None,
            origin: PageOrigin::TextLayer,
            confidence: None,
        };
        store
            .replace_document(name, "hash", false, &[chunk], &[vec![1.0, 0.0]], None, None)
            .unwrap();
    }
    let side = Connection::open(&path).unwrap();
    for (name, _) in documents {
        facts::write(&side, name, shipped_vocabulary()).unwrap();
    }
    (store, side, dir)
}

fn rows(
    connection: &Connection,
) -> Vec<(String, String, Option<String>, String, String, i64, i64)> {
    let mut statement = connection
        .prepare(
            "SELECT relative_path, kind, currency, value, surface, page, char_offset
             FROM ld_facts ORDER BY relative_path, char_offset",
        )
        .unwrap();
    statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn a_fact_keeps_its_normal_form_its_passage_its_page_and_where_it_is_written() {
    let text = "Invoice dated 12 mars 2026, total 1 200,00 EUR.";
    let (_store, side, _dir) = store_with(&[("a.md", text)]);
    let found = rows(&side);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(
        found[0],
        (
            "a.md".to_string(),
            "date".to_string(),
            None,
            "2026-03-12".to_string(),
            "12 mars 2026".to_string(),
            2,
            text.find("12 mars").unwrap() as i64
        )
    );
    assert_eq!(found[1].1, "amount");
    assert_eq!(found[1].2.as_deref(), Some("EUR"));
    assert_eq!(found[1].3, "1200.00");
    assert_eq!(found[1].4, "1 200,00 EUR");
    let chunk: String = side
        .query_row("SELECT chunk_id FROM ld_facts LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(chunk, "a.md#p2#s1");
    // Writing again replaces: never twice.
    facts::write(&side, "a.md", shipped_vocabulary()).unwrap();
    assert_eq!(rows(&side).len(), 2);
}

#[test]
fn a_question_that_writes_the_date_or_the_amount_another_way_finds_the_passage() {
    let (_store, side, _dir) = store_with(&[
        ("a.md", "Invoice dated 12 mars 2026, total 1 200,00 EUR."),
        ("b.md", "Invoice dated 14/03/2026, total 300 euros."),
        ("c.md", "Meeting minutes, nothing to count."),
    ]);
    let ask = |question: &str, paths: Option<&[String]>| {
        facts::passages_for_question(&side, question, shipped_vocabulary(), paths, 10).unwrap()
    };
    assert_eq!(
        ask("Which invoice is dated 12/03/2026?", None),
        vec!["a.md#p2#s1"]
    );
    assert_eq!(
        ask("Which invoice is dated 2026-03-14?", None),
        vec!["b.md#p2#s1"]
    );
    assert_eq!(
        ask("Which invoice is for 1200 euros?", None),
        vec!["a.md#p2#s1"]
    );
    assert_eq!(
        ask("Which invoice is for EUR 300", None),
        vec!["b.md#p2#s1"]
    );
    // the wrong currency is not the same amount
    assert!(ask("Which invoice is for 300 CHF?", None).is_empty());
    // a question with no date and no amount finds nothing
    assert!(ask("Which invoice is the latest?", None).is_empty());
    // held to the files asked for; nothing allowed is nothing
    let only_b = ["b.md".to_string()];
    assert!(ask("Which invoice is dated 12/03/2026?", Some(&only_b)).is_empty());
    assert!(ask("Which invoice is dated 12/03/2026?", Some(&[])).is_empty());
}

#[test]
fn a_fact_whose_passage_is_gone_is_never_returned() {
    let (mut store, side, _dir) = store_with(&[("a.md", "Invoice dated 12 mars 2026.")]);
    let ask = || {
        facts::passages_for_question(
            &side,
            "Which invoice is dated 12/03/2026?",
            shipped_vocabulary(),
            None,
            10,
        )
        .unwrap()
    };
    assert_eq!(ask().len(), 1);
    // The document is read again and cut differently: the old passage no longer exists.
    let chunk = Chunk {
        chunk_id: "a.md#p9#s1".to_string(),
        relative_path: "a.md".to_string(),
        page_number: 9,
        section: 1,
        text: "Something else.".to_string(),
        index_text: None,
        origin: PageOrigin::TextLayer,
        confidence: None,
    };
    store
        .replace_document(
            "a.md",
            "hash2",
            false,
            &[chunk],
            &[vec![1.0, 0.0]],
            None,
            None,
        )
        .unwrap();
    assert!(ask().is_empty());
}

fn first_paths(store: &IndexStore, question: &str, options: AssemblyOptions) -> Vec<String> {
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    set_assembly_options_for_measurement(Some(options));
    let found = retrieval::search_scoped_counted_with_terms(
        store,
        question,
        &[],
        &[1.0, 0.0],
        RetrievalScope::WholeFolder,
        RetrievalLimits::BASELINE,
    )
    .unwrap();
    set_assembly_options_for_measurement(None);
    found
        .evidence
        .into_iter()
        .map(|item| item.relative_path)
        .collect()
}

#[test]
fn the_switch_puts_the_passage_that_states_the_date_first_and_changes_nothing_when_off() {
    let (store, _side, _dir) = store_with(&[
        (
            "a.md",
            "Invoice dated 14 mars 2026 for the physiotherapy sessions.",
        ),
        (
            "b.md",
            "Invoice dated 12 mars 2026 for the physiotherapy sessions.",
        ),
    ]);
    let question = "Which physiotherapy invoice is dated 12/03/2026?";
    let off = first_paths(&store, question, AssemblyOptions::OFF);
    assert_eq!(
        off.first().map(String::as_str),
        Some("a.md"),
        "a tie: stored order"
    );
    let on = first_paths(
        &store,
        question,
        AssemblyOptions {
            facts: true,
            ..AssemblyOptions::OFF
        },
    );
    assert_eq!(on.first().map(String::as_str), Some("b.md"));
    // a question with no date or amount is not changed by the switch
    let plain = "Which physiotherapy invoice is the latest?";
    assert_eq!(
        first_paths(&store, plain, AssemblyOptions::OFF),
        first_paths(
            &store,
            plain,
            AssemblyOptions {
                facts: true,
                ..AssemblyOptions::OFF
            }
        )
    );
}

#[tokio::test]
async fn the_analyse_writes_the_facts_of_the_documents_and_drops_those_of_a_removed_file() {
    let mut lab = Lab::new();
    lab.write(
        "invoice.md",
        "# Invoice\n\nDate: 12 mars 2026\n\nTotal: 1 200,00 EUR\n",
    );
    lab.write("notes.md", "# Notes\n\nNothing dated, nothing priced.\n");
    lab.analyse().await;
    let found = rows(&lab.side());
    let values: Vec<&str> = found.iter().map(|row| row.3.as_str()).collect();
    assert_eq!(values, vec!["2026-03-12", "1200.00"], "{found:?}");
    assert!(found.iter().all(|row| row.0 == "invoice.md"));

    std::fs::remove_file(lab.work_path().join("invoice.md")).unwrap();
    lab.analyse().await;
    assert!(
        rows(&lab.side()).is_empty(),
        "the removed file takes its facts with it"
    );
}

#[tokio::test]
async fn the_data_folder_is_never_read_for_facts() {
    let mut lab = Lab::new();
    lab.write_data(
        "ledger.csv",
        "date,amount\n2026-03-12,1200.00 EUR\n2026-03-13,300.00 EUR\n",
    );
    lab.analyse().await;
    assert!(rows(&lab.side()).is_empty());
}

#[test]
fn a_date_that_many_passages_state_pins_none_of_them() {
    let documents: Vec<(String, String)> = (0..5)
        .map(|n| {
            (
                format!("d{n}.md"),
                format!("Invoice number {n} dated 12 mars 2026 for sessions."),
            )
        })
        .collect();
    let refs: Vec<(&str, &str)> = documents
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let (store, _side, _dir) = store_with(&refs);
    let question = "Which invoice is dated 12/03/2026?";
    let off = first_paths(&store, question, AssemblyOptions::OFF);
    let on = first_paths(
        &store,
        question,
        AssemblyOptions {
            facts: true,
            ..AssemblyOptions::OFF
        },
    );
    assert_eq!(
        off, on,
        "five passages state the date: it tells nothing about which one is meant"
    );
}
