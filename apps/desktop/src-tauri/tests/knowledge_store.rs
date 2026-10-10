//! The knowledge base's persistence, seen from outside the crate: how it joins an index that
//! already exists, and how it follows the index's own lifecycle (a document analysed, replaced,
//! gone from the folder, reset).
//!
//! The store's own primitives (merge, tombstones, the name search, the integrity check) are tested
//! beside the code in `src/knowledge/store.rs`; what is checked here is what only shows with a real
//! `IndexStore`, a real file and a real pass: the migration of a database written before the
//! knowledge base existed, the transaction that holds a document's chunks and its knowledge
//! together, and the two Resets that must not touch each other's side.
//!
//! Every name in this file is invented. Nothing here reads a real document.

mod common;

use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use assistant_cabinet_ai_lib::chunking::Chunk;
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::gateway::{EmbeddingDeadlines, GatewayClient};
use assistant_cabinet_ai_lib::index_store::{configure_connection, IndexStore};
use assistant_cabinet_ai_lib::indexing;
use assistant_cabinet_ai_lib::knowledge::store as kb;
use assistant_cabinet_ai_lib::knowledge::{
    AliasDraft, AliasKind, Domain, EntityDraft, EntityFilter, EntityRef, EntityStatus,
    IntegrityReport, KnowledgeDelta, KnowledgeWrite, MentionDraft, MentionLocator, NewAlias,
    Origin, Page, SourceRef, CURRENT_KB_VERSION, NOT_EXTRACTED,
};
use rusqlite::Connection;

use common::fake_gateway::{FakeGateway, Reply};

// ----------------------------------------------------------------------------- helpers

struct Workspace {
    _dir: tempfile::TempDir,
    index_path: std::path::PathBuf,
    store: IndexStore,
}

impl Workspace {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let index_path = dir.path().join("index.sqlite3");
        let store = IndexStore::open_at(&index_path).expect("opens the index");
        Self {
            _dir: dir,
            index_path,
            store,
        }
    }

    /// A second connection to the same file, set up the way the index sets up its own.
    fn side_connection(&self) -> Connection {
        let connection = Connection::open(&self.index_path).expect("opens a second connection");
        configure_connection(&connection).expect("configures it");
        connection
    }

    fn assert_clean(&self) {
        assert_eq!(
            kb::integrity_check(&self.side_connection()).expect("checks"),
            IntegrityReport::default()
        );
    }
}

fn document_source(path: &str, sha: &str) -> SourceRef {
    SourceRef {
        domain: Domain::Documents,
        relative_path: path.to_string(),
        content_id: sha.to_string(),
    }
}

fn data_source(path: &str, sha: &str) -> SourceRef {
    SourceRef {
        domain: Domain::Data,
        relative_path: path.to_string(),
        content_id: sha.to_string(),
    }
}

fn person(temp_id: u32, name: &str) -> EntityDraft {
    EntityDraft {
        temp_id,
        type_id: "person".to_string(),
        subtype: None,
        canonical_name: name.to_string(),
        normalized_name: name.to_lowercase(),
        disambiguator: String::new(),
        status: EntityStatus::Active,
    }
}

fn chunk_mention(entity: EntityRef, chunk_id: &str) -> MentionDraft {
    MentionDraft {
        entity,
        alias_normalized: None,
        role: None,
        locator: MentionLocator::Chunk {
            chunk_id: chunk_id.to_string(),
        },
        occurrence_count: 1,
        confidence: 0.9,
        method: "test_method".to_string(),
        extractor_version: 1,
    }
}

fn chunk_for(path: &str, text: &str) -> Chunk {
    Chunk {
        chunk_id: format!("{path}#p1#s1"),
        relative_path: path.to_string(),
        page_number: 1,
        section: 1,
        text: text.to_string(),
        origin: PageOrigin::TextLayer,
        confidence: None,
    }
}

/// The delta of a document that mentions the given people, one chunk each.
fn delta_for(path: &str, sha: &str, names: &[&str]) -> KnowledgeDelta {
    let mut delta = KnowledgeDelta::empty(document_source(path, sha));
    delta.kb_version = CURRENT_KB_VERSION;
    for (position, name) in names.iter().enumerate() {
        let temp_id = position as u32 + 1;
        delta.entities.push(person(temp_id, name));
        delta.aliases.push(AliasDraft {
            entity: EntityRef::Draft(temp_id),
            display: name.to_string(),
            normalized: name.to_lowercase(),
            phonetic_key: String::new(),
            kind: AliasKind::CanonicalVariant,
            confidence: 1.0,
        });
        delta.mentions.push(chunk_mention(
            EntityRef::Draft(temp_id),
            &format!("{path}#p1#s1"),
        ));
    }
    delta
}

/// Store a document with its knowledge, through the real method.
fn store_document(
    store: &mut IndexStore,
    path: &str,
    sha: &str,
    text: &str,
    delta: &KnowledgeDelta,
) -> Result<KnowledgeWrite, assistant_cabinet_ai_lib::error::AppError> {
    store.replace_document_with_knowledge(
        path,
        sha,
        false,
        &[chunk_for(path, text)],
        &[vec![0.5, 0.5]],
        None,
        None,
        delta,
    )
}

fn apply_on(connection: &mut Connection, delta: &KnowledgeDelta) {
    let tx = connection.transaction().expect("transaction");
    kb::apply_delta(&tx, delta).expect("applies");
    tx.commit().expect("commits");
}

fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("counts")
}

/// Every row of the file-side tables, so a test can say "nothing there moved".
fn file_side_counts(connection: &Connection) -> Vec<(String, i64)> {
    [
        "documents",
        "chunks",
        "chunks_fts",
        "tabular_inventories",
        "tabular_workbooks",
    ]
    .iter()
    .map(|table| (table.to_string(), count(connection, table)))
    .collect()
}

fn sources_of(connection: &Connection, domain: Domain) -> Vec<String> {
    kb::sources(connection, Some(domain))
        .expect("lists")
        .into_iter()
        .map(|row| row.source.relative_path)
        .collect()
}

fn live_names(connection: &Connection) -> Vec<String> {
    kb::list_entities(
        connection,
        &EntityFilter::default(),
        Page {
            offset: 0,
            limit: 100,
        },
    )
    .expect("lists")
    .into_iter()
    .map(|entity| entity.normalized_name)
    .collect()
}

// ----------------------------------------------------------------------------- migration

/// The index as it was written before the knowledge base: the oldest layout the code still opens
/// (no OCR columns), copied here so the test does not move when `migrate` does.
const PRE_KB_DDL: &str = "
    CREATE TABLE documents (
        relative_path TEXT PRIMARY KEY,
        sha256 TEXT NOT NULL,
        empty INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE chunks (
        chunk_id TEXT PRIMARY KEY,
        relative_path TEXT NOT NULL,
        page_number INTEGER NOT NULL,
        section INTEGER NOT NULL,
        text TEXT NOT NULL,
        embedding BLOB NOT NULL
    );
    CREATE VIRTUAL TABLE chunks_fts USING fts5(chunk_id UNINDEXED, text);
    CREATE TABLE tabular_inventories (
        relative_path TEXT PRIMARY KEY,
        workbook_id TEXT NOT NULL,
        inventory_json TEXT NOT NULL
    );
    CREATE INDEX tabular_inventories_by_hash ON tabular_inventories (workbook_id);
    CREATE TABLE tabular_workbooks (
        workbook_id TEXT PRIMARY KEY,
        workbook_json TEXT NOT NULL
    );
";

fn write_pre_kb_database(path: &Path) {
    let connection = Connection::open(path).expect("creates the old database");
    connection.execute_batch(PRE_KB_DDL).expect("old DDL");
    connection
        .execute(
            "INSERT INTO documents (relative_path, sha256, empty) VALUES ('inbox/old.txt', 'sha-old', 0)",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO chunks (chunk_id, relative_path, page_number, section, text, embedding)
             VALUES ('inbox/old.txt#p1#s1', 'inbox/old.txt', 1, 1, 'Quarterly report for Alice Archer',
                     x'0000803F00000040')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO chunks_fts (chunk_id, text)
             VALUES ('inbox/old.txt#p1#s1', 'Quarterly report for Alice Archer')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO tabular_inventories VALUES ('data/old.csv', 'wb-old', '{}')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO tabular_workbooks VALUES ('wb-old', '{\"sheets\":[]}')",
            [],
        )
        .unwrap();
}

#[test]
fn an_index_created_by_the_current_schema_migrates_without_losing_anything() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite3");
    write_pre_kb_database(&path);

    let store = IndexStore::open_at(&path).expect("opens a pre-KB database");

    // The old rows are exactly what they were.
    assert_eq!(
        store.stored_hash("inbox/old.txt").unwrap().as_deref(),
        Some("sha-old")
    );
    assert_eq!(store.chunk_count().unwrap(), 1);
    let chunks = store.all_chunks().unwrap();
    assert_eq!(chunks[0].text, "Quarterly report for Alice Archer");
    assert_eq!(chunks[0].embedding, vec![1.0, 2.0]);
    assert_eq!(
        chunks[0].origin,
        PageOrigin::TextLayer,
        "the default of the added column"
    );
    assert_eq!(store.search_lexical("Quarterly", 5).unwrap().len(), 1);
    assert_eq!(store.all_tabular_inventories().unwrap().len(), 1);

    // The knowledge base is there, seeded, and empty.
    let connection = Connection::open(&path).unwrap();
    let tables: Vec<String> = {
        let mut statement = connection
            .prepare("SELECT name FROM sqlite_master WHERE name LIKE 'kb_%' AND type IN ('table', 'view') ORDER BY name")
            .unwrap();
        let rows = statement.query_map([], |row| row.get(0)).unwrap();
        rows.map(Result::unwrap).collect()
    };
    for expected in [
        "kb_aliases",
        "kb_attributes",
        "kb_column_semantics",
        "kb_diagnostics",
        "kb_effective_entity",
        "kb_effective_mentions",
        "kb_entities",
        "kb_entity_types",
        "kb_mentions",
        "kb_meta",
        "kb_names_fts",
        "kb_ops_log",
        "kb_possible_matches",
        "kb_profile",
        "kb_relations",
        "kb_source_domains",
        "kb_source_signals",
        "kb_sources",
    ] {
        assert!(
            tables.contains(&expected.to_string()),
            "{expected} is missing"
        );
    }
    assert_eq!(
        kb::meta(&connection, "schema_version").unwrap().as_deref(),
        Some("1")
    );
    for (table, rows) in kb::table_counts(&connection).unwrap() {
        assert_eq!(rows, 0, "{table} starts empty");
    }
    assert_eq!(count(&connection, "kb_entity_types"), 6);
    assert_eq!(count(&connection, "kb_source_domains"), 2);
}

#[test]
fn opening_the_index_again_is_idempotent_and_keeps_what_the_knowledge_base_holds() {
    let mut workspace = Workspace::new();
    store_document(
        &mut workspace.store,
        "inbox/a.txt",
        "sha-a",
        "Alice Archer writes",
        &delta_for("inbox/a.txt", "sha-a", &["Alice Archer"]),
    )
    .unwrap();
    let before = {
        let connection = workspace.side_connection();
        (
            count(&connection, "kb_entities"),
            count(&connection, "kb_mentions"),
        )
    };
    let path = workspace.index_path.clone();
    drop(workspace.store);

    let reopened = IndexStore::open_at(&path).expect("opens again");
    let reopened_again = IndexStore::open_at(&path).expect("and again, at the same time");

    let connection = Connection::open(&path).unwrap();
    assert_eq!(
        (
            count(&connection, "kb_entities"),
            count(&connection, "kb_mentions")
        ),
        before
    );
    assert_eq!(reopened.chunk_count().unwrap(), 1);
    assert_eq!(reopened_again.chunk_count().unwrap(), 1);
}

// ----------------------------------------------------------------------------- the product's behaviour

#[test]
fn a_document_stored_the_old_way_is_a_known_source_nobody_extracted_from() {
    let mut workspace = Workspace::new();

    workspace
        .store
        .replace_document(
            "inbox/letter.txt",
            "sha-letter",
            false,
            &[chunk_for("inbox/letter.txt", "Alice Archer writes")],
            &[vec![0.5, 0.5]],
            None,
            None,
        )
        .unwrap();

    let connection = workspace.side_connection();
    let rows = kb::sources(&connection, Some(Domain::Documents)).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].source.content_id, "sha-letter");
    assert_eq!(
        rows[0].kb_version, NOT_EXTRACTED,
        "so any extractor sees it as due"
    );
    assert_eq!(count(&connection, "kb_entities"), 0);
    assert_eq!(count(&connection, "kb_mentions"), 0);
    workspace.assert_clean();
}

#[test]
fn a_knowledge_step_that_fails_is_bypassed_and_the_document_is_still_indexed() {
    let mut workspace = Workspace::new();
    store_document(
        &mut workspace.store,
        "inbox/a.txt",
        "sha-1",
        "Alice Archer first version",
        &delta_for("inbox/a.txt", "sha-1", &["Alice Archer"]),
    )
    .unwrap();
    let connection = workspace.side_connection();

    // The new version of the file carries a delta that cannot be applied: its mention names an
    // entity that no draft defines. Until lot 4 this rolled the chunks back with the knowledge;
    // the owner's rule of 9 October 2026 is that the knowledge base never blocks indexing, so the
    // chunks are committed and only the knowledge step is undone.
    let mut broken = delta_for("inbox/a.txt", "sha-2", &["Bruno Baker"]);
    broken
        .mentions
        .push(chunk_mention(EntityRef::Draft(99), "inbox/a.txt#p1#s1"));
    let result = store_document(
        &mut workspace.store,
        "inbox/a.txt",
        "sha-2",
        "Bruno Baker second version",
        &broken,
    );

    assert_eq!(result.unwrap(), KnowledgeWrite::Bypassed);
    assert_eq!(
        workspace
            .store
            .stored_hash("inbox/a.txt")
            .unwrap()
            .as_deref(),
        Some("sha-2"),
        "the document has the version it was given"
    );
    assert_eq!(
        workspace.store.all_chunks().unwrap()[0].text,
        "Bruno Baker second version"
    );
    // Nothing of the broken delta, and nothing of the old version either: that knowledge described
    // text that is gone. The source is absent, so a later pass sees the document as due.
    assert_eq!(count(&connection, "kb_sources"), 0);
    assert_eq!(count(&connection, "kb_mentions"), 0);
    assert!(live_names(&connection).is_empty());
    workspace.assert_clean();
}

#[test]
fn a_first_write_with_a_failing_knowledge_step_still_indexes_the_document() {
    let mut workspace = Workspace::new();
    let mut broken = delta_for("inbox/new.txt", "sha-n", &["Alice Archer"]);
    broken.mentions.push(chunk_mention(
        EntityRef::Existing(4242),
        "inbox/new.txt#p1#s1",
    ));

    let result = store_document(
        &mut workspace.store,
        "inbox/new.txt",
        "sha-n",
        "Alice Archer",
        &broken,
    );

    assert_eq!(result.unwrap(), KnowledgeWrite::Bypassed);
    let connection = workspace.side_connection();
    assert_eq!(count(&connection, "documents"), 1);
    assert_eq!(count(&connection, "chunks"), 1);
    assert_eq!(count(&connection, "kb_sources"), 0);
    assert_eq!(count(&connection, "kb_entities"), 0);
    assert_eq!(count(&connection, "kb_names_fts"), 0);
    workspace.assert_clean();
}

#[test]
fn a_delta_for_another_file_is_refused_before_anything_is_written() {
    let mut workspace = Workspace::new();
    let wrong_path = delta_for("inbox/other.txt", "sha-a", &["Alice Archer"]);
    let wrong_content = delta_for("inbox/a.txt", "sha-other", &["Alice Archer"]);
    let mut wrong_domain = delta_for("inbox/a.txt", "sha-a", &["Alice Archer"]);
    wrong_domain.source.domain = Domain::Data;

    for delta in [wrong_path, wrong_content, wrong_domain] {
        let result = store_document(&mut workspace.store, "inbox/a.txt", "sha-a", "text", &delta);
        assert_eq!(result.unwrap_err().code(), "internal_error");
    }

    assert_eq!(workspace.store.chunk_count().unwrap(), 0);
    assert_eq!(count(&workspace.side_connection(), "kb_sources"), 0);
}

#[test]
fn retain_documents_removes_the_knowledge_of_vanished_files() {
    let mut workspace = Workspace::new();
    store_document(
        &mut workspace.store,
        "inbox/kept.txt",
        "sha-k",
        "Alice Archer and Bruno Baker",
        &delta_for("inbox/kept.txt", "sha-k", &["Alice Archer"]),
    )
    .unwrap();
    store_document(
        &mut workspace.store,
        "inbox/gone.txt",
        "sha-g",
        "Alice Archer and Bruno Baker",
        &delta_for("inbox/gone.txt", "sha-g", &["Alice Archer", "Bruno Baker"]),
    )
    .unwrap();

    let dropped = workspace
        .store
        .retain_documents(&["inbox/kept.txt".to_string()])
        .unwrap();

    assert_eq!(dropped, vec!["inbox/gone.txt".to_string()]);
    let connection = workspace.side_connection();
    assert_eq!(
        sources_of(&connection, Domain::Documents),
        vec!["inbox/kept.txt"]
    );
    // Bruno was only in the vanished file: collected. Alice is still in the kept one.
    assert_eq!(live_names(&connection), vec!["alice archer".to_string()]);
    assert_eq!(count(&connection, "kb_mentions"), 1);
    workspace.assert_clean();
}

#[test]
fn clear_removes_document_knowledge_but_not_data_knowledge() {
    let mut workspace = Workspace::new();
    store_document(
        &mut workspace.store,
        "inbox/a.txt",
        "sha-a",
        "Alice Archer",
        &delta_for("inbox/a.txt", "sha-a", &["Alice Archer"]),
    )
    .unwrap();
    let mut connection = workspace.side_connection();
    let mut data = KnowledgeDelta::empty(data_source("sheets/clients.csv", "wb-1"));
    data.entities.push(person(1, "Bruno Baker"));
    data.mentions.push(MentionDraft {
        locator: MentionLocator::Cells {
            sheet: "Sheet1".to_string(),
            column_name: "name".to_string(),
            first_row: Some(2),
            sample_rows: vec![2],
        },
        ..chunk_mention(EntityRef::Draft(1), "")
    });
    apply_on(&mut connection, &data);

    workspace.store.clear().unwrap();

    assert_eq!(
        sources_of(&connection, Domain::Documents),
        Vec::<String>::new()
    );
    assert_eq!(
        sources_of(&connection, Domain::Data),
        vec!["sheets/clients.csv"]
    );
    assert_eq!(live_names(&connection), vec!["bruno baker".to_string()]);
    assert_eq!(workspace.store.chunk_count().unwrap(), 0);
    workspace.assert_clean();
}

/// The typed cell cache is a cache of the file, not something the knowledge base learned from it:
/// clearing it must not remove a single knowledge row.
#[test]
fn clearing_the_cell_cache_is_not_a_knowledge_event() {
    let mut workspace = Workspace::new();
    let mut connection = workspace.side_connection();
    let mut data = KnowledgeDelta::empty(data_source("sheets/clients.csv", "wb-1"));
    data.entities.push(person(1, "Bruno Baker"));
    data.mentions.push(MentionDraft {
        locator: MentionLocator::Filename,
        ..chunk_mention(EntityRef::Draft(1), "")
    });
    apply_on(&mut connection, &data);
    connection
        .execute(
            "INSERT INTO tabular_workbooks VALUES ('wb-1', '{\"sheets\":[]}')",
            [],
        )
        .unwrap();

    workspace.store.clear_tabular_workbooks().unwrap();

    assert_eq!(count(&connection, "tabular_workbooks"), 0);
    assert_eq!(
        sources_of(&connection, Domain::Data),
        vec!["sheets/clients.csv"]
    );
    assert_eq!(live_names(&connection), vec!["bruno baker".to_string()]);
    workspace.assert_clean();
}

#[test]
fn reset_data_removes_data_knowledge_only() {
    let mut workspace = Workspace::new();
    store_document(
        &mut workspace.store,
        "inbox/a.txt",
        "sha-a",
        "Alice Archer",
        &delta_for("inbox/a.txt", "sha-a", &["Alice Archer"]),
    )
    .unwrap();
    let mut connection = workspace.side_connection();
    let mut data = KnowledgeDelta::empty(data_source("sheets/clients.csv", "wb-1"));
    data.entities.push(person(1, "Bruno Baker"));
    data.mentions.push(MentionDraft {
        locator: MentionLocator::Filename,
        ..chunk_mention(EntityRef::Draft(1), "")
    });
    apply_on(&mut connection, &data);

    workspace.store.clear_tabular_inventories().unwrap();

    assert_eq!(sources_of(&connection, Domain::Data), Vec::<String>::new());
    assert_eq!(
        sources_of(&connection, Domain::Documents),
        vec!["inbox/a.txt"]
    );
    assert_eq!(live_names(&connection), vec!["alice archer".to_string()]);
    assert_eq!(workspace.store.chunk_count().unwrap(), 1);
    workspace.assert_clean();
}

#[test]
fn a_workbook_gone_from_the_data_folder_takes_its_knowledge_with_it() {
    let mut workspace = Workspace::new();
    let mut connection = workspace.side_connection();
    for (path, sha, name) in [
        ("sheets/kept.csv", "wb-k", "Alice Archer"),
        ("sheets/gone.csv", "wb-g", "Bruno Baker"),
    ] {
        let mut data = KnowledgeDelta::empty(data_source(path, sha));
        data.entities.push(person(1, name));
        data.mentions.push(MentionDraft {
            locator: MentionLocator::Filename,
            ..chunk_mention(EntityRef::Draft(1), "")
        });
        apply_on(&mut connection, &data);
        connection
            .execute(
                "INSERT INTO tabular_inventories VALUES (?1, ?2, '{}')",
                rusqlite::params![path, sha],
            )
            .unwrap();
    }

    let dropped = workspace
        .store
        .retain_tabular_inventories(&["sheets/kept.csv".to_string()])
        .unwrap();

    assert_eq!(dropped, vec!["sheets/gone.csv".to_string()]);
    assert_eq!(
        sources_of(&connection, Domain::Data),
        vec!["sheets/kept.csv"]
    );
    assert_eq!(live_names(&connection), vec!["alice archer".to_string()]);
    workspace.assert_clean();
}

#[test]
fn the_same_relative_path_in_both_folders_stays_two_sources() {
    let mut workspace = Workspace::new();
    store_document(
        &mut workspace.store,
        "shared/clients.csv",
        "sha-doc",
        "Alice Archer",
        &delta_for("shared/clients.csv", "sha-doc", &["Alice Archer"]),
    )
    .unwrap();
    let mut connection = workspace.side_connection();
    let mut data = KnowledgeDelta::empty(data_source("shared/clients.csv", "wb-1"));
    data.entities.push(person(1, "Alice Archer"));
    data.mentions.push(MentionDraft {
        locator: MentionLocator::Filename,
        ..chunk_mention(EntityRef::Draft(1), "")
    });
    apply_on(&mut connection, &data);

    let all = kb::sources(&connection, None).unwrap();
    assert_eq!(all.len(), 2);
    assert_ne!(all[0].source.domain, all[1].source.domain);

    // Dropping the document leaves the workbook's source, with its own mention.
    workspace
        .store
        .retain_documents(&[])
        .expect("the documents folder is empty");
    assert_eq!(
        sources_of(&connection, Domain::Documents),
        Vec::<String>::new()
    );
    assert_eq!(
        sources_of(&connection, Domain::Data),
        vec!["shared/clients.csv"]
    );
    assert_eq!(live_names(&connection), vec!["alice archer".to_string()]);
    workspace.assert_clean();
}

#[test]
fn deleting_an_entity_is_a_tombstone_and_touches_no_file_row() {
    let mut workspace = Workspace::new();
    store_document(
        &mut workspace.store,
        "inbox/a.txt",
        "sha-a",
        "Alice Archer",
        &delta_for("inbox/a.txt", "sha-a", &["Alice Archer"]),
    )
    .unwrap();
    let mut connection = workspace.side_connection();
    connection
        .execute(
            "INSERT INTO tabular_inventories VALUES ('sheets/t.csv', 'wb-1', '{}')",
            [],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO tabular_workbooks VALUES ('wb-1', '{\"sheets\":[]}')",
            [],
        )
        .unwrap();
    let files_before = file_side_counts(&connection);
    let chunk_before: String = connection
        .query_row("SELECT text FROM chunks", [], |row| row.get(0))
        .unwrap();
    let alice = kb::find_entities_by_normalized(&connection, "alice archer").unwrap()[0].entity_id;

    let tx = connection.transaction().unwrap();
    kb::delete_entity(&tx, alice).unwrap();
    tx.commit().unwrap();

    let entity = kb::get_entity(&connection, alice).unwrap().unwrap();
    assert_eq!(entity.status, EntityStatus::Deleted);
    assert_eq!(count(&connection, "kb_mentions"), 0);
    assert_eq!(file_side_counts(&connection), files_before);
    let chunk_after: String = connection
        .query_row("SELECT text FROM chunks", [], |row| row.get(0))
        .unwrap();
    assert_eq!(chunk_after, chunk_before);

    // A later analysis of the same file does not bring the name back.
    store_document(
        &mut workspace.store,
        "inbox/a.txt",
        "sha-a2",
        "Alice Archer again",
        &delta_for("inbox/a.txt", "sha-a2", &["Alice Archer"]),
    )
    .unwrap();
    assert_eq!(live_names(&connection), Vec::<String>::new());
    assert_eq!(count(&connection, "kb_mentions"), 0);
    workspace.assert_clean();
}

#[test]
fn manual_rows_survive_the_next_analysis_of_their_file() {
    let mut workspace = Workspace::new();
    store_document(
        &mut workspace.store,
        "inbox/a.txt",
        "sha-1",
        "Alice Archer",
        &delta_for("inbox/a.txt", "sha-1", &["Alice Archer"]),
    )
    .unwrap();
    let connection = workspace.side_connection();
    let alice = kb::find_entities_by_normalized(&connection, "alice archer").unwrap()[0].entity_id;
    kb::add_alias(
        &connection,
        alice,
        &NewAlias {
            display: "Ally".to_string(),
            normalized: "ally".to_string(),
            phonetic_key: String::new(),
            kind: AliasKind::Manual,
            confidence: 1.0,
        },
        Origin::Manual,
    )
    .unwrap();

    // The file changes and no longer names her at all.
    store_document(
        &mut workspace.store,
        "inbox/a.txt",
        "sha-2",
        "Nothing to see",
        &{
            let mut delta = KnowledgeDelta::empty(document_source("inbox/a.txt", "sha-2"));
            delta.kb_version = CURRENT_KB_VERSION;
            delta
        },
    )
    .unwrap();

    assert_eq!(live_names(&connection), vec!["alice archer".to_string()]);
    assert_eq!(count(&connection, "kb_mentions"), 0);
    let found = kb::prefix_search(&connection, "all", 5).unwrap();
    assert_eq!(found.len(), 1, "the manual alias still leads to her");
    workspace.assert_clean();
}

#[test]
fn the_knowledge_base_can_be_reset_with_or_without_the_users_own_rows() {
    let mut workspace = Workspace::new();
    store_document(
        &mut workspace.store,
        "inbox/a.txt",
        "sha-a",
        "Alice Archer",
        &delta_for("inbox/a.txt", "sha-a", &["Alice Archer"]),
    )
    .unwrap();
    let connection = workspace.side_connection();
    let alice = kb::find_entities_by_normalized(&connection, "alice archer").unwrap()[0].entity_id;
    kb::set_attribute(&connection, alice, "note", "kept", Origin::Manual, None).unwrap();
    let files_before = file_side_counts(&connection);

    workspace.store.clear_knowledge(false).unwrap();
    assert_eq!(count(&connection, "kb_sources"), 0);
    assert_eq!(
        count(&connection, "kb_entities"),
        1,
        "the annotated entity stays"
    );
    assert_eq!(count(&connection, "kb_attributes"), 1);
    workspace.assert_clean();

    workspace.store.clear_knowledge(true).unwrap();
    for (table, rows) in kb::table_counts(&connection).unwrap() {
        assert_eq!(rows, 0, "{table}");
    }
    assert_eq!(
        file_side_counts(&connection),
        files_before,
        "no file row was touched"
    );
    workspace.assert_clean();
}

// ----------------------------------------------------------------------------- connections

#[test]
fn a_writer_waits_for_a_lock_instead_of_failing() {
    let mut workspace = Workspace::new();
    let path = workspace.index_path.clone();
    let (locked_tx, locked_rx) = mpsc::channel();
    let holder = std::thread::spawn(move || {
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        locked_tx.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        connection.execute_batch("COMMIT").unwrap();
    });
    locked_rx.recv().unwrap();

    let started = Instant::now();
    let result = workspace.store.replace_document(
        "inbox/a.txt",
        "sha-a",
        false,
        &[chunk_for("inbox/a.txt", "Alice Archer")],
        &[vec![0.5, 0.5]],
        None,
        None,
    );
    holder.join().unwrap();

    assert!(result.is_ok(), "the busy timeout outlasts a short lock");
    assert!(
        started.elapsed() >= Duration::from_millis(300),
        "it did wait for the other connection"
    );
    assert_eq!(workspace.store.chunk_count().unwrap(), 1);
}

#[test]
fn a_second_connection_reads_while_the_index_writes() {
    let mut workspace = Workspace::new();
    store_document(
        &mut workspace.store,
        "inbox/a.txt",
        "sha-a",
        "Alice Archer",
        &delta_for("inbox/a.txt", "sha-a", &["Alice Archer"]),
    )
    .unwrap();
    let reader = workspace.side_connection();
    let reading = std::thread::spawn(move || {
        (0..50)
            .map(|_| {
                kb::find_entities_by_normalized(&reader, "alice archer")
                    .map(|found| found.len())
                    .unwrap_or(usize::MAX)
            })
            .collect::<Vec<_>>()
    });

    for round in 0..20 {
        let sha = format!("sha-{round}");
        store_document(
            &mut workspace.store,
            "inbox/b.txt",
            &sha,
            "Bruno Baker",
            &delta_for("inbox/b.txt", &sha, &["Bruno Baker"]),
        )
        .unwrap();
    }

    for found in reading.join().unwrap() {
        assert_eq!(
            found, 1,
            "the reader never saw a half-written state or an error"
        );
    }
    workspace.assert_clean();
}

// ----------------------------------------------------------------------------- a real pass

fn write_document(work: &Path, name: &str, text: &str) {
    std::fs::write(work.join(name), text).expect("writes the fixture");
}

fn gateway_client() -> GatewayClient {
    GatewayClient::new()
        .expect("builds a client")
        .with_embedding_deadlines(EmbeddingDeadlines {
            first_batch: Duration::from_millis(400),
            batch: Duration::from_millis(400),
            retry_pause: Duration::ZERO,
        })
}

async fn run_pass(
    store: &mut IndexStore,
    gateway_url: &str,
    work: &Path,
) -> assistant_cabinet_ai_lib::indexing::IndexSummary {
    indexing::run(
        store,
        &gateway_client(),
        gateway_url,
        "cabinet-embed",
        work,
        None,
        None,
        "fr-FR",
        &|_| {},
    )
    .await
    .expect("the pass completes")
}

#[tokio::test]
async fn a_real_pass_registers_each_document_and_a_second_pass_changes_nothing() {
    let work = tempfile::tempdir().unwrap();
    write_document(
        work.path(),
        "letter-one.txt",
        "Alice Archer wrote the first letter.",
    );
    write_document(
        work.path(),
        "letter-two.txt",
        "Bruno Baker wrote the second letter.",
    );
    let gateway = FakeGateway::start(|_| Reply::vectors());
    let mut workspace = Workspace::new();

    let first = run_pass(&mut workspace.store, &gateway.url, work.path()).await;
    assert_eq!(first.indexed_files, 2);
    let connection = workspace.side_connection();
    let sources = kb::sources(&connection, Some(Domain::Documents)).unwrap();
    assert_eq!(sources.len(), 2);
    for row in &sources {
        assert_eq!(
            Some(row.source.content_id.clone()),
            workspace
                .store
                .stored_hash(&row.source.relative_path)
                .unwrap(),
            "a source follows the content hash of its document"
        );
        assert_eq!(row.kb_version, NOT_EXTRACTED);
    }
    assert_eq!(
        count(&connection, "kb_entities"),
        0,
        "no extractor exists yet"
    );

    let before = kb::sources(&connection, None).unwrap();
    let second = run_pass(&mut workspace.store, &gateway.url, work.path()).await;
    assert_eq!(second.unchanged_files, 2);
    assert_eq!(kb::sources(&connection, None).unwrap(), before);

    // A file that leaves the folder leaves the knowledge base at the start of the next pass.
    std::fs::remove_file(work.path().join("letter-two.txt")).unwrap();
    let third = run_pass(&mut workspace.store, &gateway.url, work.path()).await;
    assert_eq!(third.removed_files, vec!["letter-two.txt".to_string()]);
    assert_eq!(
        sources_of(&connection, Domain::Documents),
        vec!["letter-one.txt"]
    );
    workspace.assert_clean();
}

#[tokio::test]
async fn an_embedding_failure_leaves_no_knowledge_rows_for_that_file() {
    let work = tempfile::tempdir().unwrap();
    write_document(work.path(), "a-fine.txt", "Alice Archer wrote a letter.");
    write_document(
        work.path(),
        "b-stalls.txt",
        "POISONED passage that stalls the gateway.",
    );
    let gateway = FakeGateway::start(|seen| {
        if seen.inputs.iter().any(|text| text.contains("POISONED")) {
            Reply::slow(Duration::from_millis(900))
        } else {
            Reply::vectors()
        }
    });
    let mut workspace = Workspace::new();

    let summary = run_pass(&mut workspace.store, &gateway.url, work.path()).await;

    assert_eq!(summary.failed_files.len(), 1);
    let connection = workspace.side_connection();
    assert_eq!(
        sources_of(&connection, Domain::Documents),
        vec!["a-fine.txt"]
    );
    assert!(workspace
        .store
        .stored_hash("b-stalls.txt")
        .unwrap()
        .is_none());
    workspace.assert_clean();
}
