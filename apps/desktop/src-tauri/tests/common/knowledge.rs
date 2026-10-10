//! A small laboratory for the tests of the knowledge base that read documents: a work folder, an
//! index in a temporary application-data folder, a fake gateway, and a pass that builds its
//! knowledge context the way the application does (fresh for every pass).
//!
//! Also the **snapshot**: every row of every `kb_*` table as text, so a test can say "this pass
//! changed nothing" (invariant I14) or "that question changed nothing" (I3) by comparing two of
//! them. Every name used with it is invented.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use assistant_cabinet_ai_lib::gateway::{EmbeddingDeadlines, GatewayClient};
use assistant_cabinet_ai_lib::index_store::{configure_connection, IndexStore};
use assistant_cabinet_ai_lib::indexing::{self, IndexSummary};
use assistant_cabinet_ai_lib::knowledge::ingest::KnowledgeContext;
use assistant_cabinet_ai_lib::knowledge::secret::IdentifierKey;
use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;
use assistant_cabinet_ai_lib::ocr::OcrProvider;
use assistant_cabinet_ai_lib::raster::PageRasterizer;
use rusqlite::types::ValueRef;
use rusqlite::Connection;

use super::fake_gateway::{FakeGateway, Reply};

/// Every table of the knowledge base that holds rows a pass writes. `kb_diagnostics` is a log of
/// questions and `kb_names_fts` is derived from `kb_aliases`; neither is knowledge.
const TABLES: [&str; 15] = [
    "kb_meta",
    "kb_source_domains",
    "kb_sources",
    "kb_entity_types",
    "kb_entities",
    "kb_aliases",
    "kb_mentions",
    "kb_relations",
    "kb_attributes",
    "kb_possible_matches",
    "kb_ops_log",
    "kb_column_semantics",
    "kb_source_signals",
    "kb_profile",
    "kb_diagnostics",
];

/// Each table's rows as one string per row, sorted, so two snapshots compare with `==`.
pub fn snapshot(connection: &Connection) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for table in TABLES {
        if table == "kb_diagnostics" {
            continue;
        }
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table}"))
            .expect("prepares");
        let width = statement.column_count();
        let mut rows: Vec<String> = statement
            .query_map([], |row| {
                let mut cells = Vec::with_capacity(width);
                for index in 0..width {
                    cells.push(match row.get_ref(index)? {
                        ValueRef::Null => "NULL".to_string(),
                        ValueRef::Integer(value) => value.to_string(),
                        ValueRef::Real(value) => format!("{value:.6}"),
                        ValueRef::Text(bytes) => String::from_utf8_lossy(bytes).to_string(),
                        ValueRef::Blob(bytes) => format!("blob:{}", bytes.len()),
                    });
                }
                Ok(cells.join("|"))
            })
            .expect("queries")
            .collect::<Result<_, _>>()
            .expect("reads");
        rows.sort();
        out.insert(table.to_string(), rows);
    }
    out
}

/// The number of rows a pass could have written, over every knowledge table but the seeds.
pub fn knowledge_rows(connection: &Connection) -> usize {
    [
        "kb_sources",
        "kb_entities",
        "kb_aliases",
        "kb_mentions",
        "kb_relations",
        "kb_attributes",
    ]
    .iter()
    .map(|table| count(connection, table) as usize)
    .sum()
}

pub fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("counts")
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityRow {
    pub entity_id: i64,
    pub type_id: String,
    pub subtype: Option<String>,
    pub canonical: String,
    pub normalized: String,
    pub status: String,
}

/// Every entity, whatever its status, by canonical name.
pub fn entities(connection: &Connection) -> Vec<EntityRow> {
    let mut statement = connection
        .prepare(
            "SELECT entity_id, type_id, subtype, canonical_name, normalized_name, status
             FROM kb_entities ORDER BY canonical_name, entity_id",
        )
        .expect("prepares");
    let rows = statement
        .query_map([], |row| {
            Ok(EntityRow {
                entity_id: row.get(0)?,
                type_id: row.get(1)?,
                subtype: row.get(2)?,
                canonical: row.get(3)?,
                normalized: row.get(4)?,
                status: row.get(5)?,
            })
        })
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads");
    rows
}

pub fn entity_named(connection: &Connection, canonical: &str) -> Option<EntityRow> {
    entities(connection)
        .into_iter()
        .find(|row| row.canonical == canonical)
}

#[derive(Debug, Clone, PartialEq)]
pub struct MentionRow {
    pub path: String,
    pub locator_kind: String,
    pub chunk_id: Option<String>,
    pub occurrences: i64,
    pub method: String,
    pub confidence: f64,
}

/// The mentions of one entity, by source path.
pub fn mentions_of(connection: &Connection, entity_id: i64) -> Vec<MentionRow> {
    let mut statement = connection
        .prepare(
            "SELECT s.relative_path, m.locator_kind, m.chunk_id, m.occurrence_count, m.method,
                    m.confidence
             FROM kb_mentions m JOIN kb_sources s ON s.source_id = m.source_id
             WHERE m.entity_id = ?1 ORDER BY s.relative_path, m.mention_id",
        )
        .expect("prepares");
    let rows = statement
        .query_map([entity_id], |row| {
            Ok(MentionRow {
                path: row.get(0)?,
                locator_kind: row.get(1)?,
                chunk_id: row.get(2)?,
                occurrences: row.get(3)?,
                method: row.get(4)?,
                confidence: row.get(5)?,
            })
        })
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads");
    rows
}

/// Every text value of every knowledge table, to look for something that must not be there.
pub fn every_text_value(connection: &Connection) -> Vec<String> {
    let mut found = Vec::new();
    for table in TABLES {
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table}"))
            .expect("prepares");
        let width = statement.column_count();
        let rows = statement
            .query_map([], |row| {
                let mut texts = Vec::new();
                for index in 0..width {
                    if let ValueRef::Text(bytes) = row.get_ref(index)? {
                        texts.push(String::from_utf8_lossy(bytes).to_string());
                    }
                }
                Ok(texts)
            })
            .expect("queries");
        for row in rows {
            found.extend(row.expect("reads"));
        }
    }
    found
}

/// A client whose embedding deadlines are short, so a gateway that stalls on purpose makes a file
/// fail within a second instead of two minutes.
pub fn gateway_client() -> GatewayClient {
    GatewayClient::new()
        .expect("builds a client")
        .with_embedding_deadlines(EmbeddingDeadlines {
            first_batch: std::time::Duration::from_millis(400),
            batch: std::time::Duration::from_millis(400),
            retry_pause: std::time::Duration::ZERO,
        })
}

/// A work folder, an index, a gateway, and the settings of the pass.
pub struct Lab {
    pub work: tempfile::TempDir,
    pub app_data: tempfile::TempDir,
    pub index: IndexStore,
    pub gateway: FakeGateway,
    pub locale: &'static str,
    pub packs: Vec<&'static str>,
    pub mode: KnowledgeMode,
    pub key: [u8; 32],
}

impl Lab {
    pub fn new() -> Self {
        let app_data = tempfile::tempdir().expect("temp app-data folder");
        let index =
            IndexStore::open_at(&app_data.path().join("index.sqlite3")).expect("opens the index");
        Self {
            work: tempfile::tempdir().expect("temp work folder"),
            app_data,
            index,
            gateway: FakeGateway::start(|_| Reply::vectors()),
            locale: "fr-FR",
            packs: Vec::new(),
            mode: KnowledgeMode::Suggest,
            key: [7; 32],
        }
    }

    pub fn index_path(&self) -> PathBuf {
        self.app_data.path().join("index.sqlite3")
    }

    /// A second connection to the same file, configured the way the index configures its own.
    pub fn side(&self) -> Connection {
        let connection = Connection::open(self.index_path()).expect("opens a second connection");
        configure_connection(&connection).expect("configures it");
        connection
    }

    pub fn write(&self, name: &str, text: &str) {
        let path = self.work.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("creates the folder");
        }
        std::fs::write(path, text).expect("writes the document");
    }

    pub fn write_bytes(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.work.path().join(name), bytes).expect("writes the document");
    }

    pub fn remove(&self, name: &str) {
        std::fs::remove_file(self.work.path().join(name)).expect("removes the document");
    }

    /// The context a pass would build now: new every time, as in the application.
    pub fn context(&self) -> KnowledgeContext {
        KnowledgeContext::for_index(
            &self.index,
            self.locale,
            &self.packs,
            self.mode,
            IdentifierKey::from_bytes(self.key),
        )
        .expect("builds the context")
    }

    pub async fn analyse(&mut self) -> IndexSummary {
        let mut context = self.context();
        self.analyse_with(&mut context).await
    }

    pub async fn analyse_with(&mut self, context: &mut KnowledgeContext) -> IndexSummary {
        self.analyse_through(context, &self.gateway.url.clone(), None, None)
            .await
    }

    pub async fn analyse_through(
        &mut self,
        context: &mut KnowledgeContext,
        server_url: &str,
        ocr: Option<&dyn OcrProvider>,
        rasterizer: Option<&dyn PageRasterizer>,
    ) -> IndexSummary {
        indexing::run_with_knowledge(
            &mut self.index,
            &gateway_client(),
            server_url,
            "cabinet-embed",
            self.work.path(),
            ocr,
            rasterizer,
            self.locale,
            &|_| {},
            Some(context),
        )
        .await
        .expect("the pass completes")
    }

    pub fn work_path(&self) -> &Path {
        self.work.path()
    }
}
