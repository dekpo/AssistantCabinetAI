//! Lot 9 of the long-documents programme: safety of the new layout, and the spreadsheets.
//!
//! - an index from before is recognised by one cheap lookup, and left as it is (every table, byte for
//!   byte) until the user presses Analyse;
//! - the tables of the spreadsheet engine (`tabular_inventories`, `tabular_workbooks`,
//!   `kb_column_semantics`) are identical before and after a rebuild of the documents' index, after the
//!   layout upgrade and after a change of embedding model;
//! - the `data` questions of the sandbox exam get the same answers from a rebuilt index;
//! - nothing heavy runs when the index is opened or after a pass.

mod common;

use std::sync::Mutex;

use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
use assistant_cabinet_ai_lib::file_profile::store::{index_status, FolderProfileStatus};
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::indexing;
use assistant_cabinet_ai_lib::tabular_answer::{self, PendingAnswer};
use common::knowledge::{gateway_client, tabular_snapshot, Lab};
use common::validation::{library, Truth};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

const PACKS: [&str; 2] = ["health-fr", "health-ch"];

/// A hash of every table of the file but the ones a newer version adds: the content of each and the
/// definition of the old ones.
fn dump_of(connection: &Connection, tables: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for table in tables {
        let ddl: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = ?1",
                [table],
                |r| r.get(0),
            )
            .unwrap_or_default();
        hasher.update(ddl);
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table} ORDER BY 1, 2"))
            .unwrap();
        let width = statement.column_count();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for i in 0..width {
                hasher.update(format!("{:?}|", row.get_ref(i).unwrap()));
            }
            hasher.update("\n");
        }
    }
    format!("{:x}", hasher.finalize())
}

#[tokio::test]
async fn an_old_index_is_recognised_by_one_lookup_and_every_table_of_it_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite3");
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE documents (relative_path TEXT PRIMARY KEY, sha256 TEXT NOT NULL,
                    empty INTEGER NOT NULL DEFAULT 0, ocr_engine TEXT, ocr_engine_version TEXT);
                 CREATE TABLE chunks (chunk_id TEXT PRIMARY KEY, relative_path TEXT NOT NULL,
                    page_number INTEGER NOT NULL, section INTEGER NOT NULL, text TEXT NOT NULL,
                    embedding BLOB NOT NULL, origin TEXT NOT NULL DEFAULT 'text_layer', confidence REAL);
                 CREATE VIRTUAL TABLE chunks_fts USING fts5(chunk_id UNINDEXED, text);
                 CREATE TABLE tabular_inventories (relative_path TEXT PRIMARY KEY, workbook_id TEXT NOT NULL, inventory_json TEXT NOT NULL);
                 CREATE TABLE tabular_workbooks (workbook_id TEXT PRIMARY KEY, workbook_json TEXT NOT NULL);
                 INSERT INTO documents VALUES ('a.txt', 'h', 0, NULL, NULL);
                 INSERT INTO chunks VALUES ('a.txt#p1#s1', 'a.txt', 1, 1, 'old words here', x'0000803f00000040', 'text_layer', NULL);
                 INSERT INTO chunks_fts (chunk_id, text) VALUES ('a.txt#p1#s1', 'old words here');
                 INSERT INTO tabular_inventories VALUES ('t.xlsx', 'w1', '{}');
                 INSERT INTO tabular_workbooks VALUES ('w1', '{\"sheets\": []}');",
            )
            .unwrap();
    }
    let tables = [
        "documents",
        "chunks",
        "tabular_inventories",
        "tabular_workbooks",
    ];
    let before = dump_of(&Connection::open(&path).unwrap(), &tables);

    let store = IndexStore::open_at(&path).unwrap();
    assert!(!store.vectors_nullable(), "the old layout is recognised");
    let side = Connection::open(&path).unwrap();
    assert_eq!(
        index_status(&side).documents,
        FolderProfileStatus::NeedsAnalyse
    );
    assert_eq!(
        dump_of(&side, &tables),
        before,
        "opening it and asking its status change none of its tables"
    );
    drop(store);
}

#[tokio::test]
async fn the_spreadsheet_tables_are_identical_after_every_kind_of_rebuild() {
    let mut lab = Lab::new();
    lab.packs = PACKS.to_vec();
    lab.write("a.txt", "Madame Marie Martin a ete recue le 12 mars 2026.");
    lab.write(
        "b.txt",
        &"A long enough second document about invoices. ".repeat(40),
    );
    lab.write_data(
        "rdv.csv",
        "Patient;Date;Montant\nMarie Martin;2026-03-12;45\nJean Dupont;2026-03-13;60\n",
    );
    lab.analyse().await;
    lab.analyse_data();
    let reference = tabular_snapshot(&lab.side());
    assert!(
        !reference.values().all(Vec::is_empty),
        "the snapshot holds the workbook"
    );

    // 1. the documents' index emptied and rebuilt
    lab.index.clear().unwrap();
    lab.analyse().await;
    assert_eq!(
        tabular_snapshot(&lab.side()),
        reference,
        "after a rebuild of the documents"
    );

    // 2. an index from before: the passages table in the old layout, upgraded by the Analyse
    lab.side()
        .execute_batch(
            "CREATE TABLE chunks_old (chunk_id TEXT PRIMARY KEY, relative_path TEXT NOT NULL, page_number INTEGER NOT NULL,
                section INTEGER NOT NULL, text TEXT NOT NULL, embedding BLOB NOT NULL, origin TEXT NOT NULL DEFAULT 'text_layer', confidence REAL);
             INSERT INTO chunks_old SELECT chunk_id, relative_path, page_number, section, text, COALESCE(embedding, x'0000803f'), origin, confidence FROM chunks;
             DROP TABLE chunks; ALTER TABLE chunks_old RENAME TO chunks;",
        )
        .unwrap();
    let reopened = IndexStore::open_at(&lab.index_path()).unwrap();
    assert!(!reopened.vectors_nullable());
    drop(reopened);
    lab.index = IndexStore::open_at(&lab.index_path()).unwrap();
    lab.analyse().await;
    assert!(
        lab.index.vectors_nullable(),
        "the Analyse upgraded the layout"
    );
    assert_eq!(
        tabular_snapshot(&lab.side()),
        reference,
        "after the layout upgrade"
    );

    // 3. another embedding model
    let mut context = lab.context();
    let url = lab.gateway.url.clone();
    let progress = Mutex::new(Vec::new());
    indexing::run_cancellable(
        &mut lab.index,
        &gateway_client(),
        &url,
        "another-model",
        lab.work.path(),
        None,
        None,
        None,
        lab.locale,
        &|step| progress.lock().unwrap().push(step),
        Some(&mut context),
        &|| false,
    )
    .await
    .unwrap();
    assert_eq!(
        tabular_snapshot(&lab.side()),
        reference,
        "after a change of embedding model"
    );
}

/// What the spreadsheet engine says to each `data` question of the sandbox exam, in one line.
async fn data_answers(lab: &Lab, truth: &Truth) -> Vec<(String, String)> {
    let folder = lab.data_folder();
    truth
        .questions
        .iter()
        .filter(|q| q.category == "data" && q.min_tier <= 30)
        .map(|q| {
            let pending = tabular_answer::prepare(
                &q.text,
                &folder,
                &ScopeMode::WholeFolder,
                "fr-FR",
                &lab.index,
                false,
            );
            let said = match &pending {
                Ok(PendingAnswer::Done(answer)) => format!("{answer:?}"),
                Ok(_) => "to the model".to_string(),
                Err(error) => format!("error {error:?}"),
            };
            (q.id.clone(), said)
        })
        .collect()
}

#[tokio::test]
async fn the_data_questions_of_the_exam_are_answered_the_same_by_a_rebuilt_index() {
    let truth = Truth::load();
    let mut library = library(&truth, 30, &PACKS).await;
    let before = data_answers(&library.lab, &truth).await;
    assert!(
        !before.is_empty(),
        "the exam has data questions at this size"
    );

    library.lab.index.clear().unwrap();
    library.lab.analyse().await;
    library.lab.analyse_data();
    let after = data_answers(&library.lab, &truth).await;
    assert_eq!(after, before);
}

#[tokio::test]
async fn nothing_heavy_runs_when_the_index_is_opened_or_after_a_pass() {
    let mut lab = Lab::new();
    lab.write(
        "a.txt",
        &"Some words in a document that is long enough to be cut. ".repeat(60),
    );
    lab.analyse().await;
    let requests = lab.gateway.requests().len();
    let counts = lab.index.vector_counts().unwrap();
    assert_eq!(counts.done, counts.total);

    // Opening the index again, as the application does at launch, reads and asks for nothing.
    let started = std::time::Instant::now();
    let reopened = IndexStore::open_at(&lab.index_path()).unwrap();
    let _ = index_status(&Connection::open(lab.index_path()).unwrap());
    let _ = reopened.vector_counts().unwrap();
    assert!(
        started.elapsed() < std::time::Duration::from_millis(500),
        "a cheap lookup, not a pass"
    );
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(
        lab.gateway.requests().len(),
        requests,
        "no request to the model at launch or after the pass"
    );
    let unchanged = lab.index.vector_counts().unwrap();
    assert_eq!(
        (unchanged.done, unchanged.total),
        (counts.done, counts.total),
        "and no background work changed the index"
    );
}
