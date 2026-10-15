//! The local index: SQLite, full-text search plus stored vectors, brute-force cosine.
//!
//! Lives in `%LOCALAPPDATA%` (`app_local_data_dir()`), never the roaming `%APPDATA%` that holds
//! `settings.json` - the index holds document text, and a roaming profile would copy it off the
//! machine (`docs/RETRIEVAL.md`).
//!
//! One index file, whichever work folder is configured. Choosing a different folder does **not**
//! empty it - and until `retain_documents` existed, that meant the previous folder's passages
//! stayed citable for ever. They are now dropped by the first pass over the new folder, because a
//! pass forgets every document that is not in front of it. The consequence to keep in mind: after
//! switching folders, the index still describes the old one until that pass has run.

use rusqlite::{Connection, OptionalExtension};

use crate::chunking::Chunk;
use crate::error::AppError;
use crate::extraction::PageOrigin;
use crate::knowledge::{self, Domain, KnowledgeDelta, KnowledgeWrite, SourceRef};
use crate::tabular::inventory::TabularInventory;
use crate::tabular::Workbook;

const INDEX_FILE_NAME: &str = "index.sqlite3";

/// How long a connection waits for a lock held by another connection before giving up with a
/// busy error. Three seconds is longer than any write the application makes and short enough that
/// a stuck writer shows up as an error rather than as a frozen window.
const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// The two settings every connection to the index must have. SQLite keeps both per connection and
/// starts each one with foreign keys off and no waiting, so a connection opened by anything other
/// than `IndexStore::open_at` (a second reader, a test) calls this itself.
///
/// No WAL: the journal mode changes the files that sit beside `index.sqlite3`, which the Reset and
/// the backup instructions assume do not exist (`docs/OPERATIONS.md`). Measure first, decide
/// separately.
pub fn configure_connection(connection: &Connection) -> Result<(), AppError> {
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|_| AppError::IndexUnavailable)?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(|_| AppError::IndexUnavailable)?;
    Ok(())
}

pub struct IndexStore {
    connection: Connection,
    path: std::path::PathBuf,
}

#[derive(Debug, Clone)]
pub struct StoredChunk {
    pub chunk_id: String,
    pub relative_path: String,
    pub page_number: u32,
    pub section: u32,
    pub text: String,
    pub embedding: Vec<f32>,
    pub origin: PageOrigin,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct StoredDocument {
    pub sha256: String,
    pub empty: bool,
    pub ocr_engine: Option<String>,
    pub ocr_engine_version: Option<String>,
}

/// What the index knows about one file, as one row. A read-only view for the Work Folder
/// inventory (`docs/WORK-FOLDER-INVENTORY.md`): the index stays the owner of this data, and the
/// inventory joins it onto what the filesystem reports rather than copying it into a second
/// database.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentIndexRow {
    pub relative_path: String,
    pub sha256: String,
    pub empty: bool,
    pub ocr_engine: Option<String>,
    pub ocr_engine_version: Option<String>,
    pub chunk_count: u64,
    /// How many of those chunks a machine read rather than copied. Zero means native text
    /// throughout.
    pub ocr_chunk_count: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexicalHit {
    pub chunk_id: String,
    pub rank: f64,
}

/// A stored chunk without its vector: what the knowledge base reads back when it learns from
/// documents that were analysed before it existed. The embedding is the heavy column of the table
/// and nothing here needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredChunkText {
    pub chunk_id: String,
    pub page_number: u32,
    pub section: u32,
    pub text: String,
    pub origin: PageOrigin,
    pub confidence: Option<f32>,
}

/// What became of a knowledge write made on its own (a refresh from stored text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshWrite {
    Applied(knowledge::ApplyOutcome),
    /// The knowledge step failed and was rolled back; nothing changed.
    Bypassed,
    /// The document changed (or went) while it was being read: the delta describes a version that
    /// is no longer stored, so it was not written. The newer version has its own pass.
    Stale,
}

/// The passages still waiting for their vector, in reading order, found without scanning the table.
const PENDING_INDEX: &str = "CREATE INDEX IF NOT EXISTS chunks_pending
    ON chunks (relative_path, page_number, section) WHERE embedding IS NULL;";

/// How many passages have their vector, out of how many.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VectorCounts {
    pub done: usize,
    pub total: usize,
}

/// One file whose passages are not all searchable by meaning yet.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileVectorShare {
    pub relative_path: String,
    pub chunks: usize,
    pub vectors: usize,
}

/// A passage that has no vector yet, with the text the model is given for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingChunk {
    pub chunk_id: String,
    pub relative_path: String,
    pub text: String,
}

impl IndexStore {
    pub fn open_at(path: &std::path::Path) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| AppError::IndexUnavailable)?;
        }
        let connection = Connection::open(path).map_err(|_| AppError::IndexUnavailable)?;
        configure_connection(&connection)?;
        Self::migrate(&connection)?;
        Ok(Self {
            connection,
            path: path.to_path_buf(),
        })
    }

    pub fn open_in_app_data(directory: &std::path::Path) -> Result<Self, AppError> {
        Self::open_at(&directory.join(INDEX_FILE_NAME))
    }

    fn migrate(connection: &Connection) -> Result<(), AppError> {
        connection
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS documents (
                    relative_path TEXT PRIMARY KEY,
                    sha256 TEXT NOT NULL,
                    empty INTEGER NOT NULL DEFAULT 0
                );
                CREATE TABLE IF NOT EXISTS chunks (
                    chunk_id TEXT PRIMARY KEY,
                    relative_path TEXT NOT NULL,
                    page_number INTEGER NOT NULL,
                    section INTEGER NOT NULL,
                    text TEXT NOT NULL,
                    embedding BLOB
                );
                CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                    chunk_id UNINDEXED,
                    text
                );
                CREATE TABLE IF NOT EXISTS tabular_inventories (
                    relative_path TEXT PRIMARY KEY,
                    workbook_id TEXT NOT NULL,
                    inventory_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS tabular_inventories_by_hash
                    ON tabular_inventories (workbook_id);
                CREATE TABLE IF NOT EXISTS tabular_workbooks (
                    workbook_id TEXT PRIMARY KEY,
                    workbook_json TEXT NOT NULL
                );
                ",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        Self::ensure_column(connection, "documents", "ocr_engine", "TEXT")?;
        Self::ensure_column(connection, "documents", "ocr_engine_version", "TEXT")?;
        Self::ensure_column(
            connection,
            "chunks",
            "origin",
            "TEXT NOT NULL DEFAULT 'text_layer'",
        )?;
        Self::ensure_column(connection, "chunks", "confidence", "REAL")?;
        // The chunks of one document, asked once per file by a search over explicit files
        // (`chunks_for_document`): without this, every file costs a scan of the whole table and a
        // selection of a thousand files takes seconds (defect KBD-16). One pass over the rows, once.
        connection
            .execute_batch(
                "CREATE INDEX IF NOT EXISTS chunks_by_document ON chunks (relative_path);",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        // A passage whose vector is not computed yet has none (NULL): the Analyse writes the text first
        // and the vectors afterwards. An index written before that has the column NOT NULL and is left
        // as it is until an Analyse upgrades it (`upgrade_layout`).
        if Self::layout_is_nullable(connection) {
            connection
                .execute_batch(PENDING_INDEX)
                .map_err(|_| AppError::IndexUnavailable)?;
        }
        // One card per document (`cards`): a short digest built with no model, embedded like a passage, and
        // never a passage of the document.
        connection
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS document_cards (
                    relative_path TEXT PRIMARY KEY,
                    sha256 TEXT NOT NULL,
                    version INTEGER NOT NULL,
                    text TEXT NOT NULL,
                    embedding BLOB
                );
                ",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        // The pages of a scan already recognised by a pass that did not finish (`ocr_resume`).
        connection
            .execute_batch(crate::ocr_resume::SCHEMA)
            .map_err(|_| AppError::IndexUnavailable)?;
        // The summaries the user asked the model for, by hand (`summaries`).
        connection
            .execute_batch(crate::summaries::SCHEMA)
            .map_err(|_| AppError::IndexUnavailable)?;
        // The dates and amounts the passages state, written by the file-profile pass of an Analyse (`facts`).
        connection
            .execute_batch(crate::facts::SCHEMA)
            .map_err(|_| AppError::IndexUnavailable)?;
        // The structure of the documents (`structure`): purely additive tables, beside the chunks. A
        // document with no `doc_structure` row, or an older version, is read again by the next Analyse
        // (its vectors are reused); until then the search works as before, with no section label.
        connection
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS doc_structure (
                    relative_path TEXT PRIMARY KEY,
                    version INTEGER NOT NULL,
                    chunker INTEGER NOT NULL DEFAULT 1
                );
                CREATE TABLE IF NOT EXISTS chunk_index_text (
                    chunk_id TEXT PRIMARY KEY,
                    relative_path TEXT NOT NULL,
                    prefix TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS chunk_index_text_by_document
                    ON chunk_index_text (relative_path);
                CREATE TABLE IF NOT EXISTS doc_sections (
                    relative_path TEXT NOT NULL,
                    position INTEGER NOT NULL,
                    parent INTEGER,
                    level INTEGER NOT NULL,
                    title TEXT NOT NULL,
                    first_page INTEGER NOT NULL,
                    last_page INTEGER NOT NULL,
                    source TEXT NOT NULL,
                    PRIMARY KEY (relative_path, position)
                );
                CREATE TABLE IF NOT EXISTS chunk_sections (
                    chunk_id TEXT PRIMARY KEY,
                    relative_path TEXT NOT NULL,
                    position INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS chunk_sections_by_document
                    ON chunk_sections (relative_path);
                -- The page where a passage ends, only for a passage that runs over a page break: a
                -- passage with no row ends on the page it starts. A new table, so an index from before it
                -- is opened and extended, never rewritten, and reads every passage as one page.
                CREATE TABLE IF NOT EXISTS chunk_pages (
                    chunk_id TEXT PRIMARY KEY,
                    relative_path TEXT NOT NULL,
                    last_page INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS chunk_pages_by_document
                    ON chunk_pages (relative_path);
                ",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        // The stems and trigram tables of an earlier version, and the word index of the cards (removed:
        // `docs/RETRIEVAL-QUALITY.md`, "Methods removed, and why"), are dropped, empty or not; a new index never
        // has them. The recorded layout goes
        // too; a new index has no such table yet, and an error there means there is nothing to forget.
        connection
            .execute_batch("DROP TABLE IF EXISTS chunks_lex; DROP TABLE IF EXISTS chunks_tri;
                 DROP TABLE IF EXISTS document_cards_fts;")
            .map_err(|_| AppError::IndexUnavailable)?;
        let _ = connection.execute("DELETE FROM file_profile_meta WHERE key = 'lexical_layout'", []);
        Self::ensure_column(
            connection,
            "doc_structure",
            "chunker",
            "INTEGER NOT NULL DEFAULT 1",
        )?;
        // After the index's own tables: the knowledge base lives in the same file and is purely
        // additive, so a database from before it exists is opened and extended, never rewritten.
        // A schema that cannot be completed leaves the file in a state nobody has tested, so it is
        // the index that cannot be opened, not "the knowledge base is unavailable".
        crate::knowledge::store::ensure_schema(connection)
            .map_err(|_| AppError::IndexUnavailable)?;
        // The file profile's own tables, also purely additive (`file_profile::store`).
        crate::file_profile::store::ensure_schema(connection)?;
        Ok(())
    }

    /// The connection, for `knowledge::store` to run its reads beside the index's own. Not public:
    /// the index stays the owner of the file, and only code inside the crate that belongs to the
    /// same feature may reach it. Nothing in the product reads the knowledge base yet (lot 6 does),
    /// hence the allowance.
    #[allow(dead_code)]
    pub(crate) fn connection(&self) -> &Connection {
        &self.connection
    }

    /// A transaction on the index's connection, for work that must succeed or fail together with
    /// the index's own rows. Commit it, or drop it to roll everything back.
    pub(crate) fn transaction(&mut self) -> Result<rusqlite::Transaction<'_>, AppError> {
        self.connection
            .transaction()
            .map_err(|_| AppError::IndexUnavailable)
    }

    fn ensure_column(
        connection: &Connection,
        table: &str,
        column: &str,
        decl: &str,
    ) -> Result<(), AppError> {
        let mut statement = connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(|_| AppError::IndexUnavailable)?;
        let names: Vec<String> = statement
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|_| AppError::IndexUnavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)?;
        if names.iter().any(|name| name == column) {
            return Ok(());
        }
        connection
            .execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
                [],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
    }

    /// What is already indexed for this path, so the caller can skip re-extracting an unchanged
    /// file. `None` when the file has never been indexed.
    pub fn stored_document(&self, relative_path: &str) -> Result<Option<StoredDocument>, AppError> {
        self.connection
            .query_row(
                "SELECT sha256, empty, ocr_engine, ocr_engine_version FROM documents WHERE relative_path = ?1",
                [relative_path],
                |row| {
                    Ok(StoredDocument {
                        sha256: row.get(0)?,
                        empty: row.get::<_, i64>(1)? != 0,
                        ocr_engine: row.get(2)?,
                        ocr_engine_version: row.get(3)?,
                    })
                },
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                _ => Err(AppError::IndexUnavailable),
            })
    }

    pub fn stored_hash(&self, relative_path: &str) -> Result<Option<String>, AppError> {
        Ok(self
            .stored_document(relative_path)?
            .map(|document| document.sha256))
    }

    /// Replace everything stored for one file: its chunks, its FTS rows, and its document
    /// record. Called before inserting fresh chunks, so a shrunk or re-worded file cannot leave
    /// a stale chunk behind that later gets cited.
    ///
    /// The knowledge base is told that the source exists and that no extractor has read it
    /// (`KnowledgeDelta::empty`); `replace_document_with_knowledge` is the same write with the
    /// knowledge the extraction found.
    pub fn replace_document(
        &mut self,
        relative_path: &str,
        sha256: &str,
        empty: bool,
        chunks: &[Chunk],
        embeddings: &[Vec<f32>],
        ocr_engine: Option<&str>,
        ocr_engine_version: Option<&str>,
    ) -> Result<(), AppError> {
        let source = SourceRef {
            domain: Domain::Documents,
            relative_path: relative_path.to_string(),
            content_id: sha256.to_string(),
        };
        self.replace_document_with_knowledge(
            relative_path,
            sha256,
            empty,
            chunks,
            embeddings,
            ocr_engine,
            ocr_engine_version,
            &KnowledgeDelta::empty(source),
        )
        .map(|_| ())
    }

    fn layout_is_nullable(connection: &Connection) -> bool {
        let Ok(mut statement) = connection.prepare("PRAGMA table_info(chunks)") else {
            return false;
        };
        let Ok(rows) = statement.query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(3)?))
        }) else {
            return false;
        };
        let nullable = rows
            .flatten()
            .any(|(name, not_null)| name == "embedding" && not_null == 0);
        nullable
    }

    /// Whether a passage may be stored before its vector (the layout of an index made or upgraded by
    /// this version). An index from before is not, and keeps working as it did until the Analyse
    /// upgrades it.
    pub fn vectors_nullable(&self) -> bool {
        Self::layout_is_nullable(&self.connection)
    }

    /// Bring an index from before to the layout where a vector may be absent: the passages and their
    /// vectors are copied as they are, in one transaction, and the word index is not touched. Called by
    /// the Analyse the user started, never at launch. `false` when there was nothing to do.
    pub fn upgrade_layout(&mut self) -> Result<bool, AppError> {
        if self.vectors_nullable() {
            return Ok(false);
        }
        let tx = self.transaction()?;
        tx.execute_batch(
            "CREATE TABLE chunks_next (
                chunk_id TEXT PRIMARY KEY,
                relative_path TEXT NOT NULL,
                page_number INTEGER NOT NULL,
                section INTEGER NOT NULL,
                text TEXT NOT NULL,
                embedding BLOB,
                origin TEXT NOT NULL DEFAULT 'text_layer',
                confidence REAL
             );
             INSERT INTO chunks_next (chunk_id, relative_path, page_number, section, text, embedding, origin, confidence)
                SELECT chunk_id, relative_path, page_number, section, text, embedding, origin, confidence FROM chunks;
             DROP TABLE chunks;
             ALTER TABLE chunks_next RENAME TO chunks;
             CREATE INDEX IF NOT EXISTS chunks_by_document ON chunks (relative_path);",
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        tx.execute_batch(PENDING_INDEX)
            .map_err(|_| AppError::IndexUnavailable)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(true)
    }

    /// The embedding model and the dimension the stored vectors were made with, if recorded. Vectors of
    /// two models cannot be compared, so the index says which one made its own.
    pub fn embedding_info(&self) -> Result<Option<(String, usize)>, AppError> {
        let read = |key: &str| -> Result<Option<String>, AppError> {
            self.connection
                .query_row(
                    "SELECT value FROM file_profile_meta WHERE key = ?1",
                    [key],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|_| AppError::IndexUnavailable)
        };
        let Some(model) = read("embedding_model")? else {
            return Ok(None);
        };
        let dimension = read("embedding_dimension")?
            .and_then(|d| d.parse().ok())
            .unwrap_or(0);
        Ok(Some((model, dimension)))
    }

    pub fn set_embedding_info(&mut self, model: &str, dimension: usize) -> Result<(), AppError> {
        for (key, value) in [
            ("embedding_model", model.to_string()),
            ("embedding_dimension", dimension.to_string()),
        ] {
            self.connection
                .execute(
                    "INSERT OR REPLACE INTO file_profile_meta (key, value) VALUES (?1, ?2)",
                    rusqlite::params![key, value],
                )
                .map_err(|_| AppError::IndexUnavailable)?;
        }
        Ok(())
    }

    /// The length of the vectors already stored, if any is.
    pub fn vector_dimension_in_use(&self) -> Result<Option<usize>, AppError> {
        self.connection
            .query_row(
                "SELECT LENGTH(embedding) / 4 FROM chunks WHERE embedding IS NOT NULL LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map(|found| found.map(|d| d as usize))
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// Forget every vector, of the passages and of the cards: they wait to be computed again, and the
    /// text, the words and everything else stay. Done by the Analyse when the embedding model changed.
    pub fn reset_vectors(&mut self) -> Result<(), AppError> {
        let tx = self.transaction()?;
        tx.execute_batch(
            "UPDATE chunks SET embedding = NULL; UPDATE document_cards SET embedding = NULL;",
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)
    }

    /// Where the index file is, so a worker that needs its own connection can open one.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// How many passages have their vector, out of all of them.
    pub fn vector_counts(&self) -> Result<VectorCounts, AppError> {
        self.counts(
            "SELECT COUNT(*), COALESCE(SUM(embedding IS NOT NULL), 0) FROM chunks",
            None,
        )
    }

    /// The same for one document.
    pub fn vector_counts_of(&self, relative_path: &str) -> Result<VectorCounts, AppError> {
        self.counts(
            "SELECT COUNT(*), COALESCE(SUM(embedding IS NOT NULL), 0) FROM chunks WHERE relative_path = ?1",
            Some(relative_path),
        )
    }

    /// The files with at least one passage that has no vector, with their counts: one grouped query, cheap
    /// enough for the file listing. A file with all its vectors is not listed.
    pub fn incomplete_vector_shares(&self) -> Result<Vec<FileVectorShare>, AppError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT relative_path, COUNT(*), COALESCE(SUM(embedding IS NOT NULL), 0) FROM chunks
                 GROUP BY relative_path
                 HAVING COALESCE(SUM(embedding IS NOT NULL), 0) < COUNT(*)
                 ORDER BY relative_path",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map([], |row| {
                Ok(FileVectorShare {
                    relative_path: row.get(0)?,
                    chunks: row.get::<_, i64>(1)?.max(0) as usize,
                    vectors: row.get::<_, i64>(2)?.max(0) as usize,
                })
            })
            .map_err(|_| AppError::IndexUnavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)
    }

    fn counts(&self, sql: &str, path: Option<&str>) -> Result<VectorCounts, AppError> {
        let read = |row: &rusqlite::Row<'_>| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?));
        let (total, done) = match path {
            Some(path) => self.connection.query_row(sql, [path], read),
            None => self.connection.query_row(sql, [], read),
        }
        .map_err(|_| AppError::IndexUnavailable)?;
        // The cards are computed like the passages and count with them.
        let cards_sql = match path {
            Some(_) => "SELECT COUNT(*), COALESCE(SUM(embedding IS NOT NULL), 0) FROM document_cards WHERE relative_path = ?1",
            None => "SELECT COUNT(*), COALESCE(SUM(embedding IS NOT NULL), 0) FROM document_cards",
        };
        let (card_total, card_done) = match path {
            Some(path) => self.connection.query_row(cards_sql, [path], read),
            None => self.connection.query_row(cards_sql, [], read),
        }
        .map_err(|_| AppError::IndexUnavailable)?;
        Ok(VectorCounts {
            done: (done + card_done) as usize,
            total: (total + card_total) as usize,
        })
    }

    /// The next passages that have no vector, by file, page and rank: the order the vectors are
    /// computed in, so a stopped pass leaves whole files done.
    pub fn pending_chunks(&self, limit: usize) -> Result<Vec<PendingChunk>, AppError> {
        self.pending_chunks_excluding(limit, &[])
    }

    /// `pending_chunks`, leaving out some files (the ones the server refused in this pass).
    pub fn pending_chunks_excluding(
        &self,
        limit: usize,
        excluded: &[String],
    ) -> Result<Vec<PendingChunk>, AppError> {
        let marks = vec!["?"; excluded.len()].join(",");
        let filter = if excluded.is_empty() {
            String::new()
        } else {
            format!("AND c.relative_path NOT IN ({marks})")
        };
        // The cards first: they are few, and a plan or a summary can use them before the passages.
        let card_filter = filter.replace("c.relative_path", "relative_path");
        let card_sql = format!(
            "SELECT relative_path, substr(text, 1, {CARD_EMBED_CHARS}) FROM document_cards
             WHERE embedding IS NULL {card_filter} ORDER BY relative_path LIMIT {limit}"
        );
        let mut card_statement = self
            .connection
            .prepare(&card_sql)
            .map_err(|_| AppError::IndexUnavailable)?;
        let cards = card_statement
            .query_map(rusqlite::params_from_iter(excluded.iter()), |row| {
                let path: String = row.get(0)?;
                Ok(PendingChunk {
                    chunk_id: format!("{CARD_ID_PREFIX}{path}"),
                    relative_path: path,
                    text: row.get(1)?,
                })
            })
            .map_err(|_| AppError::IndexUnavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)?;
        if !cards.is_empty() {
            return Ok(cards);
        }
        let sql = format!(
            "SELECT c.chunk_id, c.relative_path, c.text, p.prefix
             FROM chunks c LEFT JOIN chunk_index_text p ON p.chunk_id = c.chunk_id
             WHERE c.embedding IS NULL {filter}
             ORDER BY c.relative_path, c.page_number, c.section
             LIMIT {limit}"
        );
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(excluded.iter()), |row| {
                let text: String = row.get(2)?;
                let prefix: Option<String> = row.get(3)?;
                Ok(PendingChunk {
                    chunk_id: row.get(0)?,
                    relative_path: row.get(1)?,
                    text: match prefix {
                        Some(prefix) => format!("{prefix}\n{text}"),
                        None => text,
                    },
                })
            })
            .map_err(|_| AppError::IndexUnavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// Store one batch of vectors in one transaction. A passage that is gone, or that already has a
    /// vector, is skipped: the batch was computed for what was stored a moment ago.
    pub fn store_vectors(&mut self, vectors: &[(String, Vec<f32>)]) -> Result<(), AppError> {
        let tx = self.transaction()?;
        for (chunk_id, vector) in vectors {
            if let Some(path) = chunk_id.strip_prefix(CARD_ID_PREFIX) {
                tx.execute(
                    "UPDATE document_cards SET embedding = ?1 WHERE relative_path = ?2 AND embedding IS NULL",
                    rusqlite::params![encode_vector(vector), path],
                )
                .map_err(|_| AppError::IndexUnavailable)?;
                continue;
            }
            tx.execute(
                "UPDATE chunks SET embedding = ?1 WHERE chunk_id = ?2 AND embedding IS NULL",
                rusqlite::params![encode_vector(vector), chunk_id],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        }
        tx.commit().map_err(|_| AppError::IndexUnavailable)
    }

    /// `replace_document`, for a pass that writes the text first: `vectors` runs alongside `chunks`, and
    /// a passage with `None` is stored without a vector and gets it from a later batch. No knowledge
    /// delta: the document's knowledge is dropped, as in `replace_document_plain`.
    #[allow(clippy::too_many_arguments)]
    pub fn replace_document_pending_plain(
        &mut self,
        relative_path: &str,
        sha256: &str,
        empty: bool,
        chunks: &[Chunk],
        vectors: &[Option<Vec<f32>>],
        ocr_engine: Option<&str>,
        ocr_engine_version: Option<&str>,
    ) -> Result<(), AppError> {
        let tx = self.transaction()?;
        let borrowed: Vec<Option<&[f32]>> = vectors.iter().map(|v| v.as_deref()).collect();
        Self::write_document_rows(
            &tx,
            relative_path,
            sha256,
            empty,
            chunks,
            &borrowed,
            ocr_engine,
            ocr_engine_version,
        )?;
        Self::forget_source(
            &tx,
            &SourceRef {
                domain: Domain::Documents,
                relative_path: relative_path.to_string(),
                content_id: sha256.to_string(),
            },
        )?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)
    }

    /// `replace_document`, for a pass that writes the text first: the knowledge base is told that the
    /// source exists and that no extractor has read it (`KnowledgeDelta::empty`).
    #[allow(clippy::too_many_arguments)]
    pub fn replace_document_pending(
        &mut self,
        relative_path: &str,
        sha256: &str,
        empty: bool,
        chunks: &[Chunk],
        vectors: &[Option<Vec<f32>>],
        ocr_engine: Option<&str>,
        ocr_engine_version: Option<&str>,
    ) -> Result<(), AppError> {
        let source = SourceRef {
            domain: Domain::Documents,
            relative_path: relative_path.to_string(),
            content_id: sha256.to_string(),
        };
        self.replace_document_pending_with_knowledge(
            relative_path,
            sha256,
            empty,
            chunks,
            vectors,
            ocr_engine,
            ocr_engine_version,
            &KnowledgeDelta::empty(source),
        )
        .map(|_| ())
    }

    /// The same, with the knowledge the extraction found, applied in a savepoint as in
    /// `replace_document_with_knowledge`.
    #[allow(clippy::too_many_arguments)]
    pub fn replace_document_pending_with_knowledge(
        &mut self,
        relative_path: &str,
        sha256: &str,
        empty: bool,
        chunks: &[Chunk],
        vectors: &[Option<Vec<f32>>],
        ocr_engine: Option<&str>,
        ocr_engine_version: Option<&str>,
        delta: &KnowledgeDelta,
    ) -> Result<KnowledgeWrite, AppError> {
        if delta.source.domain != Domain::Documents
            || delta.source.relative_path != relative_path
            || delta.source.content_id != sha256
        {
            return Err(AppError::Internal);
        }
        let tx = self.transaction()?;
        let borrowed: Vec<Option<&[f32]>> = vectors.iter().map(|v| v.as_deref()).collect();
        Self::write_document_rows(
            &tx,
            relative_path,
            sha256,
            empty,
            chunks,
            &borrowed,
            ocr_engine,
            ocr_engine_version,
        )?;
        let write = Self::apply_delta_in_savepoint(&tx, delta)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(write)
    }

    /// The card of a document, if one was built.
    pub fn card_of(&self, relative_path: &str) -> Result<Option<StoredCard>, AppError> {
        self.connection
            .query_row(
                "SELECT text, version, embedding IS NOT NULL FROM document_cards WHERE relative_path = ?1",
                [relative_path],
                |row| {
                    Ok(StoredCard {
                        text: row.get(0)?,
                        version: row.get(1)?,
                        has_vector: row.get::<_, i64>(2)? != 0,
                    })
                },
            )
            .optional()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// The documents that have text and no up-to-date card: never built, built for another version of
    /// the file, or by an older card builder. (path, content hash)
    pub fn documents_needing_a_card(
        &self,
        version: i64,
    ) -> Result<Vec<(String, String)>, AppError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT d.relative_path, d.sha256 FROM documents d
                 LEFT JOIN document_cards c ON c.relative_path = d.relative_path
                 WHERE d.empty = 0 AND (c.relative_path IS NULL OR c.sha256 <> d.sha256 OR c.version <> ?1)
                 ORDER BY d.relative_path",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map([version], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| AppError::IndexUnavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// Store the card of a document, without a vector (phase 2 computes it).
    pub fn replace_card(
        &mut self,
        relative_path: &str,
        sha256: &str,
        version: i64,
        text: &str,
    ) -> Result<(), AppError> {
        // The same text keeps the vector it already has: a document read again for another reason
        // is not embedded again.
        let kept: Option<(String, Option<Vec<u8>>)> = self
            .connection
            .query_row(
                "SELECT text, embedding FROM document_cards WHERE relative_path = ?1",
                [relative_path],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|_| AppError::IndexUnavailable)?;
        let embedding = kept.and_then(|(old, vector)| (old == text).then_some(vector).flatten());
        let tx = self.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO document_cards (relative_path, sha256, version, text, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![relative_path, sha256, version, text, embedding],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)
    }

    fn forget_card(tx: &rusqlite::Transaction<'_>, relative_path: &str) -> Result<(), AppError> {
        tx.execute(
            "DELETE FROM document_cards WHERE relative_path = ?1",
            [relative_path],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
    }

    /// Everything stored beside the passages of a document: its structure and the prefixed texts.
    fn forget_structure(
        tx: &rusqlite::Transaction<'_>,
        relative_path: &str,
    ) -> Result<(), AppError> {
        Self::forget_structure_tables(tx, relative_path)?;
        tx.execute(
            "DELETE FROM chunk_index_text WHERE relative_path = ?1",
            [relative_path],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
    }

    /// The structure only (sections, the section of each passage, the version): what `replace_structure`
    /// replaces. The prefixed texts belong to the passage write that came just before it.
    fn forget_structure_tables(
        tx: &rusqlite::Transaction<'_>,
        relative_path: &str,
    ) -> Result<(), AppError> {
        for table in [
            "chunk_sections",
            "chunk_pages",
            "doc_sections",
            "doc_structure",
        ] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE relative_path = ?1"),
                [relative_path],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        }
        Ok(())
    }

    /// Store the structure of one document and mark it as read under `version`. Replaces whatever was
    /// stored for the path. An empty `Structure` is a valid answer ("no sections"): it records that the
    /// document was looked at, so it is not read again for that reason.
    pub fn replace_structure(
        &mut self,
        relative_path: &str,
        version: i64,
        chunker: i64,
        structure: &crate::structure::Structure,
    ) -> Result<(), AppError> {
        let tx = self.transaction()?;
        Self::forget_structure_tables(&tx, relative_path)?;
        for section in &structure.sections {
            tx.execute(
                "INSERT INTO doc_sections
                    (relative_path, position, parent, level, title, first_page, last_page, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    relative_path,
                    section.position as i64,
                    section.parent.map(|p| p as i64),
                    section.level,
                    section.title,
                    section.first_page,
                    section.last_page,
                    section.source.code(),
                ],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        }
        for (chunk_id, position) in &structure.chunk_sections {
            tx.execute(
                "INSERT OR REPLACE INTO chunk_sections (chunk_id, relative_path, position) VALUES (?1, ?2, ?3)",
                rusqlite::params![chunk_id, relative_path, *position as i64],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        }
        for (chunk_id, last_page) in &structure.chunk_ends {
            tx.execute(
                "INSERT OR REPLACE INTO chunk_pages (chunk_id, relative_path, last_page) VALUES (?1, ?2, ?3)",
                rusqlite::params![chunk_id, relative_path, last_page],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        }
        tx.execute(
            "INSERT INTO doc_structure (relative_path, version, chunker) VALUES (?1, ?2, ?3)",
            rusqlite::params![relative_path, version, chunker],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)
    }

    /// The chunker that cut this document (1 = the short-document chunker, 2 = the long one), `None` if
    /// its structure was never recorded.
    pub fn stored_chunker(&self, relative_path: &str) -> Result<Option<i64>, AppError> {
        self.connection
            .query_row(
                "SELECT chunker FROM doc_structure WHERE relative_path = ?1",
                [relative_path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// How many characters of text are stored for this document: the cheap way to know, before reading
    /// the file, whether it was long.
    pub fn stored_chars(&self, relative_path: &str) -> Result<usize, AppError> {
        self.connection
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(text)), 0) FROM chunks WHERE relative_path = ?1",
                [relative_path],
                |row| row.get::<_, i64>(0),
            )
            .map(|n| n as usize)
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// The context prefix that was embedded and indexed with the passages of a document that have one
    /// (the indexed text is the prefix, a line break, and the cited text).
    pub fn index_prefixes_of(
        &self,
        relative_path: &str,
    ) -> Result<std::collections::HashMap<String, String>, AppError> {
        let mut statement = self
            .connection
            .prepare("SELECT chunk_id, prefix FROM chunk_index_text WHERE relative_path = ?1")
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map([relative_path], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|_| AppError::IndexUnavailable)?;
        rows.collect::<Result<_, _>>()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// The version the structure of this document was read under, `None` if it never was.
    pub fn structure_version(&self, relative_path: &str) -> Result<Option<i64>, AppError> {
        self.connection
            .query_row(
                "SELECT version FROM doc_structure WHERE relative_path = ?1",
                [relative_path],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// The sections of one document, in order.
    pub fn sections_of(&self, relative_path: &str) -> Result<Vec<StoredSection>, AppError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT position, parent, level, title, first_page, last_page, source
                 FROM doc_sections WHERE relative_path = ?1 ORDER BY position",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map([relative_path], stored_section)
            .map_err(|_| AppError::IndexUnavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// The section of each of these passages (those that sit inside one), by chunk id.
    pub fn sections_of_chunks(
        &self,
        chunk_ids: &[String],
    ) -> Result<std::collections::HashMap<String, StoredSection>, AppError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT s.position, s.parent, s.level, s.title, s.first_page, s.last_page, s.source
                 FROM chunk_sections c
                 JOIN doc_sections s ON s.relative_path = c.relative_path AND s.position = c.position
                 WHERE c.chunk_id = ?1",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let mut out = std::collections::HashMap::new();
        for id in chunk_ids {
            if let Some(section) = statement
                .query_row([id], stored_section)
                .optional()
                .map_err(|_| AppError::IndexUnavailable)?
            {
                out.insert(id.clone(), section);
            }
        }
        Ok(out)
    }

    /// The last page of each of these passages that runs over a page break, by chunk id. A passage
    /// that ends on the page it starts has no entry.
    pub fn last_pages_of_chunks(
        &self,
        chunk_ids: &[String],
    ) -> Result<std::collections::HashMap<String, u32>, AppError> {
        let mut statement = self
            .connection
            .prepare("SELECT last_page FROM chunk_pages WHERE chunk_id = ?1")
            .map_err(|_| AppError::IndexUnavailable)?;
        let mut out = std::collections::HashMap::new();
        for id in chunk_ids {
            if let Some(last) = statement
                .query_row([id], |row| row.get::<_, i64>(0))
                .optional()
                .map_err(|_| AppError::IndexUnavailable)?
            {
                out.insert(id.clone(), last as u32);
            }
        }
        Ok(out)
    }

    /// The chunks, their full-text rows and the document record of one file, replaced inside the
    /// caller's transaction.
    #[allow(clippy::too_many_arguments)]
    fn write_document_rows(
        tx: &rusqlite::Transaction<'_>,
        relative_path: &str,
        sha256: &str,
        empty: bool,
        chunks: &[Chunk],
        embeddings: &[Option<&[f32]>],
        ocr_engine: Option<&str>,
        ocr_engine_version: Option<&str>,
    ) -> Result<(), AppError> {
        Self::forget_structure(tx, relative_path)?;
        if empty {
            // A document with no text has no card; one with text keeps its card until the card is built
            // again from the new text (`documents_needing_a_card`), so an unchanged card keeps its vector.
            Self::forget_card(tx, relative_path)?;
        }
        tx.execute(
            "DELETE FROM chunks WHERE relative_path = ?1",
            [relative_path],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        tx.execute(
            "DELETE FROM chunks_fts WHERE chunk_id IN (SELECT chunk_id FROM chunks WHERE relative_path = ?1)",
            [relative_path],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        // FTS rows for this path may already be gone (chunks just deleted above); delete by
        // chunk id prefix as a second pass covers any left over from an older schema.
        tx.execute(
            "DELETE FROM chunks_fts WHERE chunk_id LIKE ?1",
            [format!("{relative_path}#%")],
        )
        .map_err(|_| AppError::IndexUnavailable)?;

        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            // An absent vector is NULL: the passage is searchable by its words now and gets its
            // vector from a later batch.
            let bytes: Option<Vec<u8>> = embedding.map(encode_vector);
            tx.execute(
                "INSERT INTO chunks (chunk_id, relative_path, page_number, section, text, embedding, origin, confidence)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    chunk.chunk_id,
                    chunk.relative_path,
                    chunk.page_number,
                    chunk.section,
                    chunk.text,
                    bytes,
                    chunk.origin.as_db_str(),
                    chunk.confidence,
                ],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
            // The full-text index sees the text that was embedded (the context prefix included).
            tx.execute(
                "INSERT INTO chunks_fts (chunk_id, text) VALUES (?1, ?2)",
                rusqlite::params![
                    chunk.chunk_id,
                    chunk.index_text.as_deref().unwrap_or(&chunk.text)
                ],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
            // Only the prefix is kept: the indexed text is the prefix, a line break, and the cited text.
            if let Some(indexed) = chunk.index_text.as_deref() {
                let prefix = indexed
                    .strip_suffix(chunk.text.as_str())
                    .and_then(|head| head.strip_suffix('\n'))
                    .unwrap_or("");
                tx.execute(
                    "INSERT INTO chunk_index_text (chunk_id, relative_path, prefix) VALUES (?1, ?2, ?3)",
                    rusqlite::params![chunk.chunk_id, chunk.relative_path, prefix],
                )
                .map_err(|_| AppError::IndexUnavailable)?;
            }
        }

        tx.execute(
            "INSERT INTO documents (relative_path, sha256, empty, ocr_engine, ocr_engine_version)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(relative_path) DO UPDATE SET
                sha256 = excluded.sha256,
                empty = excluded.empty,
                ocr_engine = excluded.ocr_engine,
                ocr_engine_version = excluded.ocr_engine_version",
            rusqlite::params![
                relative_path,
                sha256,
                empty as i64,
                ocr_engine,
                ocr_engine_version,
            ],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
    }

    /// Apply a delta inside the caller's transaction **without letting it take the transaction
    /// down**: the delta runs in a savepoint, and if it fails only the savepoint is rolled back.
    /// The knowledge base is an addition to the product, never a condition of it
    /// (`docs/DECISIONS.md`, 9 October 2026), so a damaged `kb_*` table must not stop a document
    /// being indexed. An error from the savepoint statements themselves is a real database failure
    /// and is returned.
    fn apply_delta_in_savepoint(
        tx: &rusqlite::Transaction<'_>,
        delta: &KnowledgeDelta,
    ) -> Result<KnowledgeWrite, AppError> {
        tx.execute_batch("SAVEPOINT knowledge_delta")
            .map_err(|_| AppError::IndexUnavailable)?;
        match knowledge::store::apply_delta(tx, delta) {
            Ok(outcome) => {
                tx.execute_batch("RELEASE knowledge_delta")
                    .map_err(|_| AppError::IndexUnavailable)?;
                Ok(KnowledgeWrite::Applied(outcome))
            }
            Err(_) => {
                tx.execute_batch("ROLLBACK TO knowledge_delta; RELEASE knowledge_delta")
                    .map_err(|_| AppError::IndexUnavailable)?;
                // What the source's previous version taught the knowledge base describes text
                // that is no longer stored. Forget it if that works; if the knowledge tables are
                // the problem it will not, and the stale rows stay behind a source whose content
                // no longer matches the document (every reader checks that, and the next pass
                // reads the document again).
                Self::forget_source(tx, &delta.source)?;
                Ok(KnowledgeWrite::Bypassed)
            }
        }
    }

    /// Best effort, in a savepoint of its own: a failure here is not a failure of the write.
    fn forget_source(tx: &rusqlite::Transaction<'_>, source: &SourceRef) -> Result<(), AppError> {
        tx.execute_batch("SAVEPOINT knowledge_forget")
            .map_err(|_| AppError::IndexUnavailable)?;
        if knowledge::store::remove_source(tx, source.domain, &source.relative_path).is_ok() {
            tx.execute_batch("RELEASE knowledge_forget")
                .map_err(|_| AppError::IndexUnavailable)?;
        } else {
            tx.execute_batch("ROLLBACK TO knowledge_forget; RELEASE knowledge_forget")
                .map_err(|_| AppError::IndexUnavailable)?;
        }
        Ok(())
    }

    /// `replace_document`, with what extraction learned from the file. The chunks, their full-text
    /// rows, the document record and the knowledge delta are written in **one transaction**, so the
    /// file keeps exactly what it had if the *index* part fails (a full disk, a locked file). The
    /// caller calls this only once every embedding exists, which is already the boundary
    /// `indexing::run` keeps.
    ///
    /// The knowledge part is different: it is applied in a savepoint, and a failure there rolls
    /// back the knowledge alone. The chunks are committed, the answer is `Bypassed`, and the source
    /// is left due so a later pass reads it again. A document is never lost to a knowledge error.
    ///
    /// The delta must describe this very file: a different domain, path or content hash is a
    /// programming error and is refused before anything is written.
    #[allow(clippy::too_many_arguments)]
    pub fn replace_document_with_knowledge(
        &mut self,
        relative_path: &str,
        sha256: &str,
        empty: bool,
        chunks: &[Chunk],
        embeddings: &[Vec<f32>],
        ocr_engine: Option<&str>,
        ocr_engine_version: Option<&str>,
        delta: &KnowledgeDelta,
    ) -> Result<KnowledgeWrite, AppError> {
        if delta.source.domain != Domain::Documents
            || delta.source.relative_path != relative_path
            || delta.source.content_id != sha256
        {
            return Err(AppError::Internal);
        }
        let tx = self.transaction()?;
        let borrowed: Vec<Option<&[f32]>> = embeddings.iter().map(|v| Some(v.as_slice())).collect();
        Self::write_document_rows(
            &tx,
            relative_path,
            sha256,
            empty,
            chunks,
            &borrowed,
            ocr_engine,
            ocr_engine_version,
        )?;
        // Last, and in a savepoint of the same transaction.
        let write = Self::apply_delta_in_savepoint(&tx, delta)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(write)
    }

    /// `replace_document` for a pass that must not touch the knowledge base at all
    /// (`KnowledgeMode::Off`): the same rows, and no `kb_*` row written. What the knowledge base
    /// learned from the *previous* version of the file is dropped (a delete, never a write), so it
    /// cannot outlive the text it described; switching the mode back on reads the file again.
    #[allow(clippy::too_many_arguments)]
    pub fn replace_document_plain(
        &mut self,
        relative_path: &str,
        sha256: &str,
        empty: bool,
        chunks: &[Chunk],
        embeddings: &[Vec<f32>],
        ocr_engine: Option<&str>,
        ocr_engine_version: Option<&str>,
    ) -> Result<(), AppError> {
        let tx = self.transaction()?;
        let borrowed: Vec<Option<&[f32]>> = embeddings.iter().map(|v| Some(v.as_slice())).collect();
        Self::write_document_rows(
            &tx,
            relative_path,
            sha256,
            empty,
            chunks,
            &borrowed,
            ocr_engine,
            ocr_engine_version,
        )?;
        // Same savepoint rule: a damaged knowledge table cannot stop a document being indexed.
        Self::forget_source(
            &tx,
            &SourceRef {
                domain: Domain::Documents,
                relative_path: relative_path.to_string(),
                content_id: sha256.to_string(),
            },
        )?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
    }

    /// Write the knowledge of a document that is already indexed, learned from its stored chunks
    /// (`knowledge::backfill`). Touches no chunk, makes no request. The delta is applied only if
    /// the document is still the one it was read from, checked inside the transaction: a pass
    /// that replaced the file in the meantime wins, and this write is dropped as `Stale`.
    pub fn apply_knowledge_refresh(
        &mut self,
        delta: &KnowledgeDelta,
    ) -> Result<RefreshWrite, AppError> {
        if delta.source.domain != Domain::Documents {
            return Err(AppError::Internal);
        }
        let tx = self.transaction()?;
        let current: Option<String> = tx
            .query_row(
                "SELECT sha256 FROM documents WHERE relative_path = ?1",
                [&delta.source.relative_path],
                |row| row.get(0),
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                _ => Err(AppError::IndexUnavailable),
            })?;
        if current.as_deref() != Some(delta.source.content_id.as_str()) {
            return Ok(RefreshWrite::Stale);
        }
        let write = Self::apply_delta_in_savepoint(&tx, delta)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(match write {
            KnowledgeWrite::Applied(outcome) => RefreshWrite::Applied(outcome),
            KnowledgeWrite::Bypassed => RefreshWrite::Bypassed,
        })
    }

    /// The stored chunks of one file in reading order, without their vectors.
    pub fn chunk_texts_for_document(
        &self,
        relative_path: &str,
    ) -> Result<Vec<StoredChunkText>, AppError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT chunk_id, page_number, section, text, origin, confidence
                 FROM chunks WHERE relative_path = ?1
                 ORDER BY page_number, section",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map([relative_path], |row| {
                let origin: String = row.get(4)?;
                Ok(StoredChunkText {
                    chunk_id: row.get(0)?,
                    page_number: row.get::<_, i64>(1)? as u32,
                    section: row.get::<_, i64>(2)? as u32,
                    text: row.get(3)?,
                    origin: PageOrigin::from_db_str(&origin),
                    confidence: row.get(5)?,
                })
            })
            .map_err(|_| AppError::IndexUnavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)
    }

    pub fn empty_documents(&self) -> Result<Vec<String>, AppError> {
        let mut statement = self
            .connection
            .prepare("SELECT relative_path FROM documents WHERE empty = 1")
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|_| AppError::IndexUnavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// Every file the index holds a record for, with its chunk counts, in relative-path order.
    /// One query, no text: the inventory needs to know what happened to a file, never what it
    /// says.
    pub fn all_documents(&self) -> Result<Vec<DocumentIndexRow>, AppError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT d.relative_path, d.sha256, d.empty, d.ocr_engine, d.ocr_engine_version,
                        COUNT(c.chunk_id),
                        COALESCE(SUM(CASE WHEN c.origin = 'ocr' THEN 1 ELSE 0 END), 0)
                 FROM documents d
                 LEFT JOIN chunks c ON c.relative_path = d.relative_path
                 GROUP BY d.relative_path
                 ORDER BY d.relative_path",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map([], |row| {
                Ok(DocumentIndexRow {
                    relative_path: row.get(0)?,
                    sha256: row.get(1)?,
                    empty: row.get::<_, i64>(2)? != 0,
                    ocr_engine: row.get(3)?,
                    ocr_engine_version: row.get(4)?,
                    chunk_count: row.get::<_, i64>(5)? as u64,
                    ocr_chunk_count: row.get::<_, i64>(6)? as u64,
                })
            })
            .map_err(|_| AppError::IndexUnavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// The stored chunks of one file, so retrieval can be constrained to a file the user named
    /// instead of searching the whole corpus (`docs/WORK-FOLDER-INVENTORY.md`).
    pub fn chunks_for_document(&self, relative_path: &str) -> Result<Vec<StoredChunk>, AppError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT chunk_id, relative_path, page_number, section, text, embedding, origin, confidence
                 FROM chunks WHERE relative_path = ?1",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map([relative_path], |row| {
                let embedding_bytes: Vec<u8> =
                    row.get::<_, Option<Vec<u8>>>(5)?.unwrap_or_default();
                let origin: String = row.get(6)?;
                Ok(StoredChunk {
                    chunk_id: row.get(0)?,
                    relative_path: row.get(1)?,
                    page_number: row.get::<_, i64>(2)? as u32,
                    section: row.get::<_, i64>(3)? as u32,
                    text: row.get(4)?,
                    embedding: decode_vector(&embedding_bytes),
                    origin: PageOrigin::from_db_str(&origin),
                    confidence: row.get(7)?,
                })
            })
            .map_err(|_| AppError::IndexUnavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// Visit the vectors of one document (or of all) one row at a time, with the id, the path, the size of
    /// the passage's text in bytes and the raw vector (empty when the passage has none): the text itself
    /// is not read. The order is the one `all_chunks` and `chunks_for_document` have.
    pub fn for_each_vector(
        &self,
        relative_path: Option<&str>,
        visit: &mut dyn FnMut(&str, &str, usize, &[u8]),
    ) -> Result<(), AppError> {
        let sql = match relative_path {
            Some(_) => {
                "SELECT chunk_id, relative_path, LENGTH(CAST(text AS BLOB)), embedding
                 FROM chunks WHERE relative_path = ?1"
            }
            None => {
                "SELECT chunk_id, relative_path, LENGTH(CAST(text AS BLOB)), embedding FROM chunks"
            }
        };
        let mut statement = self
            .connection
            .prepare(sql)
            .map_err(|_| AppError::IndexUnavailable)?;
        let mut rows = match relative_path {
            Some(path) => statement.query([path]),
            None => statement.query([]),
        }
        .map_err(|_| AppError::IndexUnavailable)?;
        while let Some(row) = rows.next().map_err(|_| AppError::IndexUnavailable)? {
            let chunk_id = row.get_ref(0).map_err(|_| AppError::IndexUnavailable)?;
            let path = row.get_ref(1).map_err(|_| AppError::IndexUnavailable)?;
            let bytes: i64 = row.get(2).map_err(|_| AppError::IndexUnavailable)?;
            let raw = match row.get_ref(3).map_err(|_| AppError::IndexUnavailable)? {
                rusqlite::types::ValueRef::Blob(blob) => blob,
                _ => &[],
            };
            if let (Ok(chunk_id), Ok(path)) = (chunk_id.as_str(), path.as_str()) {
                visit(chunk_id, path, bytes as usize, raw);
            }
        }
        Ok(())
    }

    /// The section a passage starts in, as (document, section position), when its document has a
    /// structure. What a cap per section counts by.
    pub fn section_key_of(&self, chunk_id: &str) -> Result<Option<(String, i64)>, AppError> {
        self.connection
            .query_row(
                "SELECT relative_path, position FROM chunk_sections WHERE chunk_id = ?1",
                [chunk_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// The passage that follows this one in its document, with the size of its text in bytes. `None` for the
    /// last passage of a document.
    pub fn following_passage(&self, chunk_id: &str) -> Result<Option<(String, usize)>, AppError> {
        self.connection
            .query_row(
                "SELECT n.chunk_id, LENGTH(CAST(n.text AS BLOB))
                 FROM chunks c JOIN chunks n ON n.relative_path = c.relative_path
                   AND (n.page_number > c.page_number
                        OR (n.page_number = c.page_number AND n.section > c.section))
                 WHERE c.chunk_id = ?1
                 ORDER BY n.page_number, n.section LIMIT 1",
                [chunk_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize)),
            )
            .optional()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// The passages with these ids (text and place, no vector), by id.
    pub fn passages_by_id(
        &self,
        chunk_ids: &[String],
    ) -> Result<std::collections::HashMap<String, StoredChunk>, AppError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT chunk_id, relative_path, page_number, section, text, origin, confidence
                 FROM chunks WHERE chunk_id = ?1",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let mut out = std::collections::HashMap::with_capacity(chunk_ids.len());
        for id in chunk_ids {
            let found = statement
                .query_row([id], |row| {
                    let origin: String = row.get(5)?;
                    Ok(StoredChunk {
                        chunk_id: row.get(0)?,
                        relative_path: row.get(1)?,
                        page_number: row.get::<_, i64>(2)? as u32,
                        section: row.get::<_, i64>(3)? as u32,
                        text: row.get(4)?,
                        embedding: Vec::new(),
                        origin: PageOrigin::from_db_str(&origin),
                        confidence: row.get(6)?,
                    })
                })
                .optional()
                .map_err(|_| AppError::IndexUnavailable)?;
            if let Some(chunk) = found {
                out.insert(id.clone(), chunk);
            }
        }
        Ok(out)
    }

    /// Forget every document and every chunk, keeping the file and its schema.
    ///
    /// The index is emptied rather than deleted: the connection is open, and on Windows a file
    /// held open cannot be removed underneath it. One transaction, so a failure halfway leaves a
    /// whole index rather than a half-erased one. Nothing in the work folder is touched - this
    /// forgets what was read, never what was read *from* (`docs/PRIVACY-AND-SECURITY.md`).
    ///
    /// What the knowledge base learned from those documents goes in the same transaction
    /// (`knowledge::store::clear_domain`): the Documents Reset removes the documents' sources and
    /// the automatic entities nothing else supports. Manual entities and the Data folder's
    /// knowledge stay.
    pub fn clear(&mut self) -> Result<(), AppError> {
        let tx = self.transaction()?;
        tx.execute_batch(
            "DELETE FROM chunks_fts; DELETE FROM chunks; DELETE FROM documents;
             DELETE FROM chunk_sections; DELETE FROM chunk_pages; DELETE FROM chunk_index_text; DELETE FROM doc_sections;
             DELETE FROM doc_structure; DELETE FROM document_cards; DELETE FROM document_summaries; DELETE FROM ld_facts;",
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        knowledge::store::clear_domain(&tx, Domain::Documents)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
    }

    /// Forget what the knowledge base holds, on its own. Without `include_manual`, the sources and
    /// everything automatic go and the user's manual entities, aliases and attributes stay; with
    /// it, the whole base is emptied. Files, chunks and inventories are never touched. Not wired
    /// to a command yet: the knowledge dialog of a later lot calls it, after a confirmation.
    pub fn clear_knowledge(&mut self, include_manual: bool) -> Result<(), AppError> {
        let tx = self.transaction()?;
        if include_manual {
            knowledge::store::clear_everything(&tx)?;
        } else {
            knowledge::store::clear_all_automatic(&tx)?;
        }
        tx.commit().map_err(|_| AppError::KnowledgeUnavailable)?;
        Ok(())
    }

    /// Drop every document whose relative path is not in `present`, and its chunks with it.
    ///
    /// A file she deleted from the work folder must stop being citable. Until this existed, its
    /// rows outlived it: the folder panel stopped showing it, because that reads the filesystem,
    /// while retrieval went on offering its passages as evidence for an answer. Returns the paths
    /// dropped, so the pass can report them by name rather than as a count.
    pub fn retain_documents(&mut self, present: &[String]) -> Result<Vec<String>, AppError> {
        let known: std::collections::BTreeSet<&str> = present.iter().map(String::as_str).collect();
        let stored: Vec<String> = {
            let mut statement = self
                .connection
                .prepare("SELECT relative_path FROM documents")
                .map_err(|_| AppError::IndexUnavailable)?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|_| AppError::IndexUnavailable)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|_| AppError::IndexUnavailable)?
        };
        let gone: Vec<String> = stored
            .into_iter()
            .filter(|path| !known.contains(path.as_str()))
            .collect();
        if gone.is_empty() {
            return Ok(gone);
        }

        let tx = self.transaction()?;
        for path in &gone {
            // The FTS table mirrors `chunks` by `chunk_id`, so it is cleared from the same list
            // rather than from a second query that could disagree with it.
            tx.execute(
                "DELETE FROM chunks_fts WHERE chunk_id IN
                     (SELECT chunk_id FROM chunks WHERE relative_path = ?1)",
                [path],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
            Self::forget_structure(&tx, path)?;
            Self::forget_card(&tx, path)?;
            crate::facts::forget(&tx, path)?;
            tx.execute("DELETE FROM chunks WHERE relative_path = ?1", [path])
                .map_err(|_| AppError::IndexUnavailable)?;
            tx.execute("DELETE FROM documents WHERE relative_path = ?1", [path])
                .map_err(|_| AppError::IndexUnavailable)?;
        }
        // A vanished file takes its knowledge with it, in the same transaction.
        knowledge::store::remove_sources(&tx, Domain::Documents, &gone)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(gone)
    }

    pub fn chunk_count(&self) -> Result<u64, AppError> {
        self.connection
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| {
                row.get::<_, i64>(0)
            })
            .map(|count| count as u64)
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// The passages that hold an identifier written in the question (`FAC-2026-014`), found as the phrase of
    /// its parts in the full-text index. Empty when the question holds none. The only passages the words of a
    /// question choose, and they are codes, not words.
    pub fn search_identifier(
        &self,
        question: &str,
        limit: usize,
        relative_paths: Option<&[String]>,
    ) -> Result<Vec<LexicalHit>, AppError> {
        let Some(expression) = crate::identifier::identifier_expression(question) else {
            return Ok(Vec::new());
        };
        if relative_paths.is_some_and(|paths| paths.is_empty()) {
            return Ok(Vec::new());
        }
        self.run_expression(&expression, limit, relative_paths)
    }

    /// One full-text expression against the passages as written, best match first.
    fn run_expression(
        &self,
        expression: &str,
        limit: usize,
        relative_paths: Option<&[String]>,
    ) -> Result<Vec<LexicalHit>, AppError> {
        let (table, rank) = ("chunks_fts", "bm25(chunks_fts)");
        match relative_paths {
            None => {
                let sql = format!(
                    "SELECT chunk_id, {rank} FROM {table}
                     WHERE {table} MATCH ?1 ORDER BY {rank} LIMIT ?2"
                );
                let mut statement = self
                    .connection
                    .prepare(&sql)
                    .map_err(|_| AppError::IndexUnavailable)?;
                let rows = statement
                    .query_map(rusqlite::params![expression, limit as i64], |row| {
                        Ok(LexicalHit {
                            chunk_id: row.get(0)?,
                            // bm25() in SQLite FTS5 is lower-is-better; keep as-is and let the caller sort.
                            rank: row.get(1)?,
                        })
                    })
                    .map_err(|_| AppError::IndexUnavailable)?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(|_| AppError::IndexUnavailable)
            }
            Some(paths) => {
                let paths = serde_json::to_string(paths).map_err(|_| AppError::Internal)?;
                let sql = format!(
                    "SELECT {table}.chunk_id, {rank} FROM {table}
                     JOIN chunks ON chunks.chunk_id = {table}.chunk_id
                     WHERE {table} MATCH ?1
                       AND chunks.relative_path IN (SELECT value FROM json_each(?2))
                     ORDER BY {rank} LIMIT ?3"
                );
                let mut statement = self
                    .connection
                    .prepare(&sql)
                    .map_err(|_| AppError::IndexUnavailable)?;
                let rows = statement
                    .query_map(
                        rusqlite::params![expression, paths, limit as i64],
                        |row| {
                            Ok(LexicalHit {
                                chunk_id: row.get(0)?,
                                rank: row.get(1)?,
                            })
                        },
                    )
                    .map_err(|_| AppError::IndexUnavailable)?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(|_| AppError::IndexUnavailable)
            }
        }
    }

    /// The full-text search for the surface forms of entities (`entity_terms_expression`), held to
    /// the chunks of `relative_paths`. The forms of a name a question resolved to are searched whole and
    /// whatever their length, because `Roy` and `Ng` are short names.
    pub fn search_lexical_terms_in(
        &self,
        terms: &[String],
        limit: usize,
        relative_paths: &[String],
    ) -> Result<Vec<LexicalHit>, AppError> {
        let match_expression = entity_terms_expression(terms);
        if match_expression.is_empty() || relative_paths.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let paths = serde_json::to_string(relative_paths).map_err(|_| AppError::Internal)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT chunks_fts.chunk_id, bm25(chunks_fts) FROM chunks_fts
                 JOIN chunks ON chunks.chunk_id = chunks_fts.chunk_id
                 WHERE chunks_fts MATCH ?1
                   AND chunks.relative_path IN (SELECT value FROM json_each(?2))
                 ORDER BY bm25(chunks_fts) LIMIT ?3",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map(
                rusqlite::params![match_expression, paths, limit as i64],
                |row| {
                    Ok(LexicalHit {
                        chunk_id: row.get(0)?,
                        rank: row.get(1)?,
                    })
                },
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// The files of `relative_paths` that hold any of the surface forms in `terms`, best match first,
    /// each once. The safety net of the targeting step: a document that only says `M. Dupont`, or
    /// whose scan damaged the full name, still holds a form of the name the question used.
    pub fn files_matching_terms_in(
        &self,
        terms: &[String],
        relative_paths: &[String],
    ) -> Result<Vec<String>, AppError> {
        let match_expression = entity_terms_expression(terms);
        if match_expression.is_empty() || relative_paths.is_empty() {
            return Ok(Vec::new());
        }
        let paths = serde_json::to_string(relative_paths).map_err(|_| AppError::Internal)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT chunks.relative_path FROM chunks_fts
                 JOIN chunks ON chunks.chunk_id = chunks_fts.chunk_id
                 WHERE chunks_fts MATCH ?1
                   AND chunks.relative_path IN (SELECT value FROM json_each(?2))
                 ORDER BY bm25(chunks_fts), chunks.relative_path",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map(rusqlite::params![match_expression, paths], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|_| AppError::IndexUnavailable)?;
        let mut files: Vec<String> = Vec::new();
        for row in rows {
            let path = row.map_err(|_| AppError::IndexUnavailable)?;
            if !files.contains(&path) {
                files.push(path);
            }
        }
        Ok(files)
    }

    /// Every stored chunk with its vector, for brute-force cosine similarity. A work folder holds
    /// tens of files, not millions, so loading everything into memory is fast enough and avoids a
    /// vector-database dependency (`docs/RETRIEVAL.md`).
    pub fn all_chunks(&self) -> Result<Vec<StoredChunk>, AppError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT chunk_id, relative_path, page_number, section, text, embedding, origin, confidence FROM chunks",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map([], |row| {
                let embedding_bytes: Vec<u8> =
                    row.get::<_, Option<Vec<u8>>>(5)?.unwrap_or_default();
                let origin: String = row.get(6)?;
                Ok(StoredChunk {
                    chunk_id: row.get(0)?,
                    relative_path: row.get(1)?,
                    page_number: row.get::<_, i64>(2)? as u32,
                    section: row.get::<_, i64>(3)? as u32,
                    text: row.get(4)?,
                    embedding: decode_vector(&embedding_bytes),
                    origin: PageOrigin::from_db_str(&origin),
                    confidence: row.get(7)?,
                })
            })
            .map_err(|_| AppError::IndexUnavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)
    }

    pub fn chunk_by_id(&self, chunk_id: &str) -> Result<Option<StoredChunk>, AppError> {
        self.connection
            .query_row(
                "SELECT chunk_id, relative_path, page_number, section, text, embedding, origin, confidence
                 FROM chunks WHERE chunk_id = ?1",
                [chunk_id],
                |row| {
                    let embedding_bytes: Vec<u8> = row.get::<_, Option<Vec<u8>>>(5)?.unwrap_or_default();
                    let origin: String = row.get(6)?;
                    Ok(StoredChunk {
                        chunk_id: row.get(0)?,
                        relative_path: row.get(1)?,
                        page_number: row.get::<_, i64>(2)? as u32,
                        section: row.get::<_, i64>(3)? as u32,
                        text: row.get(4)?,
                        embedding: decode_vector(&embedding_bytes),
                        origin: PageOrigin::from_db_str(&origin),
                        confidence: row.get(7)?,
                    })
                },
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                _ => Err(AppError::IndexUnavailable),
            })
    }

    /// Store this workbook's inventory, replacing whatever was cached for the same path.
    /// Keyed by path rather than by content hash: a changed file naturally invalidates its old
    /// entry by overwriting the row at its path, the same "changed file -> invalidated"
    /// guarantee `put`/`get_by_hash`/`invalidate` promise (`docs/ARCHITECTURE.md`'s
    /// `InventoryStore`), without a second table to keep in step with this one.
    pub fn put_tabular_inventory(&self, inventory: &TabularInventory) -> Result<(), AppError> {
        Self::write_inventory_row(&self.connection, inventory)
    }

    fn write_inventory_row(
        connection: &Connection,
        inventory: &TabularInventory,
    ) -> Result<(), AppError> {
        let json = serde_json::to_string(inventory).map_err(|_| AppError::IndexUnavailable)?;
        connection
            .execute(
                "INSERT INTO tabular_inventories (relative_path, workbook_id, inventory_json)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(relative_path) DO UPDATE SET
                    workbook_id = excluded.workbook_id,
                    inventory_json = excluded.inventory_json",
                rusqlite::params![inventory.relative_path, inventory.workbook_id, json],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
    }

    /// `put_tabular_inventory`, with what extraction learned from the workbook. The inventory and
    /// the knowledge delta are written in **one transaction**, and the knowledge part is applied in a
    /// savepoint exactly as for a document: a failure there rolls back the knowledge alone, the
    /// inventory is committed, the answer is `Bypassed`, and the source is left due so a later pass
    /// reads the workbook again. A workbook is never lost to a knowledge error.
    ///
    /// `delta: None` writes the inventory and **forgets** what an earlier version of the workbook
    /// taught the base: the call for a workbook with no usable sheet, or for a pass with extraction
    /// off. It writes no `kb_*` row.
    ///
    /// The delta must describe this very workbook: another domain, path or content hash is a
    /// programming error and is refused before anything is written.
    pub fn put_tabular_inventory_with_knowledge(
        &mut self,
        inventory: &TabularInventory,
        delta: Option<&KnowledgeDelta>,
    ) -> Result<KnowledgeWrite, AppError> {
        if let Some(delta) = delta {
            if delta.source.domain != Domain::Data
                || delta.source.relative_path != inventory.relative_path
                || delta.source.content_id != inventory.workbook_id
            {
                return Err(AppError::Internal);
            }
        }
        let tx = self.transaction()?;
        Self::write_inventory_row(&tx, inventory)?;
        let write = match delta {
            Some(delta) => Self::apply_delta_in_savepoint(&tx, delta)?,
            None => {
                Self::forget_source(
                    &tx,
                    &SourceRef {
                        domain: Domain::Data,
                        relative_path: inventory.relative_path.clone(),
                        content_id: inventory.workbook_id.clone(),
                    },
                )?;
                KnowledgeWrite::Applied(knowledge::ApplyOutcome::default())
            }
        };
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(write)
    }

    /// The cached inventory for this content hash, if the file at whatever path produced it is
    /// still cached under that same hash. `None` both when nothing was ever cached and when the
    /// file has since changed - the two are indistinguishable from a hash alone, which is
    /// exactly why the caller re-parses on a miss rather than treating one as an error.
    pub fn tabular_inventory_by_hash(
        &self,
        workbook_id: &str,
    ) -> Result<Option<TabularInventory>, AppError> {
        self.connection
            .query_row(
                "SELECT inventory_json FROM tabular_inventories WHERE workbook_id = ?1
                 ORDER BY relative_path LIMIT 1",
                [workbook_id],
                |row| row.get::<_, String>(0),
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                _ => Err(AppError::IndexUnavailable),
            })?
            .map(|json| serde_json::from_str(&json).map_err(|_| AppError::IndexUnavailable))
            .transpose()
    }

    /// Forget whatever inventory is cached for this path. Used when a file is dropped from the
    /// Data Folder, so a stale entry cannot be found under its old hash for ever.
    pub fn invalidate_tabular_inventory(&self, relative_path: &str) -> Result<(), AppError> {
        self.connection
            .execute(
                "DELETE FROM tabular_inventories WHERE relative_path = ?1",
                [relative_path],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
    }

    /// Every workbook with a cached inventory: which path, and which content it describes. The
    /// tabular counterpart of `all_documents`, reading `tabular_inventories` only - the Data
    /// Folder panel joins this against the filesystem itself, and nothing document-related ever
    /// reads it (`docs/DECISIONS.md`, "the two inventories stay separate").
    pub fn all_tabular_inventories(&self) -> Result<Vec<TabularInventoryRow>, AppError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT relative_path, workbook_id FROM tabular_inventories
                 ORDER BY relative_path",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map([], |row| {
                Ok(TabularInventoryRow {
                    relative_path: row.get(0)?,
                    workbook_id: row.get(1)?,
                })
            })
            .map_err(|_| AppError::IndexUnavailable)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::IndexUnavailable)
    }

    /// Drop every cached workbook inventory whose path is not in `present`. The Data Folder's
    /// counterpart of `retain_documents`: a workbook removed from the folder stops being
    /// selectable and answerable. Returns the paths dropped.
    pub fn retain_tabular_inventories(
        &mut self,
        present: &[String],
    ) -> Result<Vec<String>, AppError> {
        let known: std::collections::BTreeSet<&str> = present.iter().map(String::as_str).collect();
        let gone: Vec<String> = self
            .all_tabular_inventories()?
            .into_iter()
            .map(|row| row.relative_path)
            .filter(|path| !known.contains(path.as_str()))
            .collect();
        if gone.is_empty() {
            return Ok(gone);
        }
        let tx = self.transaction()?;
        for path in &gone {
            tx.execute(
                "DELETE FROM tabular_inventories WHERE relative_path = ?1",
                [path],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        }
        knowledge::store::remove_sources(&tx, Domain::Data, &gone)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(gone)
    }

    /// Forget every cached workbook inventory, and nothing else. The Data Folder card's Reset: the
    /// two folders are reset from two cards, independently, so this must never touch `documents`
    /// or `chunks` - exactly as `clear` never touches this table. The workbooks' knowledge goes
    /// with the inventories, in the same transaction; the documents' knowledge stays.
    pub fn clear_tabular_inventories(&mut self) -> Result<(), AppError> {
        let tx = self.transaction()?;
        tx.execute("DELETE FROM tabular_inventories", [])
            .map_err(|_| AppError::IndexUnavailable)?;
        knowledge::store::clear_domain(&tx, Domain::Data)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
    }

    /// The typed workbook cache (`docs/SESSION-DATA-13-Column-Cache.md`): cell values already
    /// read and typed by an adapter, so a question over an unchanged workbook never re-reads or
    /// re-parses the file - `tabular_answer::answer` falls back to `tabular::load_current` (a
    /// full read) only on a miss, and stores the result here afterwards.
    ///
    /// Keyed by the content SHA-256, not by path, unlike `tabular_inventories`: the same bytes
    /// under two names (a rename, `docs/WORK-FOLDER-INVENTORY.md`'s "clean file names") share one
    /// entry, and a change in content is a different key rather than an overwrite to invalidate.
    /// `ON CONFLICT` is still harmless here - the same hash can only ever map to the same bytes -
    /// but keeps this in step with `put_tabular_inventory`'s own shape.
    ///
    /// Unlike `inventory_json`, this table holds cell values at rest on the workstation
    /// (`docs/PRIVACY-AND-SECURITY.md`): the same application-data SQLite file as every other
    /// index table, never a file beside the source workbook, and cleared by `clear_tabular_workbooks`
    /// on every Analyse pass and on the Data Folder's own Reset - never left to outlive either.
    pub fn put_tabular_workbook(
        &self,
        workbook_id: &str,
        workbook: &Workbook,
    ) -> Result<(), AppError> {
        let json = serde_json::to_string(workbook).map_err(|_| AppError::IndexUnavailable)?;
        self.connection
            .execute(
                "INSERT INTO tabular_workbooks (workbook_id, workbook_json)
                 VALUES (?1, ?2)
                 ON CONFLICT(workbook_id) DO UPDATE SET workbook_json = excluded.workbook_json",
                rusqlite::params![workbook_id, json],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
    }

    /// The cached cell values for this content hash, when a question has already read this exact
    /// workbook since the last Analyse pass or reset. `None` on a cold cache or a miss - the
    /// caller re-reads the file itself, the same refusal-free fallback `tabular_inventory_by_hash`
    /// already gives its own caller.
    pub fn tabular_workbook_by_hash(
        &self,
        workbook_id: &str,
    ) -> Result<Option<Workbook>, AppError> {
        self.connection
            .query_row(
                "SELECT workbook_json FROM tabular_workbooks WHERE workbook_id = ?1",
                [workbook_id],
                |row| row.get::<_, String>(0),
            )
            .map(Some)
            .or_else(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                _ => Err(AppError::IndexUnavailable),
            })?
            .map(|json| serde_json::from_str(&json).map_err(|_| AppError::IndexUnavailable))
            .transpose()
    }

    /// Whether the cell values of this content hash are cached, without reading them. Lets a caller
    /// that must not read a file by itself (the question suggestions) tell a warm cache from a cold one.
    pub fn has_tabular_workbook(&self, workbook_id: &str) -> bool {
        self.connection
            .query_row(
                "SELECT 1 FROM tabular_workbooks WHERE workbook_id = ?1",
                [workbook_id],
                |_| Ok(()),
            )
            .is_ok()
    }

    /// Forget every cached workbook's cell values, and nothing else - `tabular_inventories` and
    /// every document table are untouched. Called at the start of every `data_folder::analyse`
    /// pass (a workbook that left the folder, or whose content changed, must not go on serving
    /// stale cells under its old hash a moment longer than the pass that would have caught it)
    /// and from the Data Folder card's Reset, exactly where `clear_tabular_inventories` is.
    pub fn clear_tabular_workbooks(&mut self) -> Result<(), AppError> {
        self.connection
            .execute("DELETE FROM tabular_workbooks", [])
            .map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
    }
}

/// One cached workbook inventory, by path and content hash. No sheet, column or cell: the panel
/// needs to know whether a workbook was analysed, never what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabularInventoryRow {
    pub relative_path: String,
    pub workbook_id: String,
}

/// The index already holds every word in the corpus, so answering "is this a word from the
/// documents?" costs one lookup rather than a dictionary nobody maintains
/// (`docs/WORK-FOLDER-INVENTORY.md`).
impl crate::folder_questions::CorpusWords for IndexStore {
    fn contains(&self, word: &str) -> bool {
        let expression = corpus_word_expression(word);
        if expression.is_empty() {
            return false;
        }
        self.connection
            .query_row(
                "SELECT 1 FROM chunks_fts WHERE chunks_fts MATCH ?1 LIMIT 1",
                [&expression],
                |_| Ok(()),
            )
            .optional()
            .map(|found| found.is_some())
            .unwrap_or(false)
    }

    fn profiles(&self) -> Option<&dyn crate::finder::rank::ProfileFacts> {
        Some(self)
    }
}

/// The rank sentence reads the chosen document date and the business type of every document profile:
/// one query, no file.
impl crate::finder::rank::ProfileFacts for IndexStore {
    fn documents(&self) -> std::collections::HashMap<String, crate::finder::rank::ProfileFact> {
        let mut facts = std::collections::HashMap::new();
        let Ok(mut statement) = self.connection().prepare(
            "SELECT relative_path, content_id, business_type, document_date, document_date_source
             FROM file_profiles WHERE domain = 'documents'",
        ) else {
            return facts;
        };
        let Ok(rows) = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        }) else {
            return facts;
        };
        for (path, content_id, business_type, date, source) in rows.flatten() {
            facts.insert(
                path,
                crate::finder::rank::ProfileFact {
                    content_id,
                    business_type,
                    date: date.zip(source),
                },
            );
        }
        facts
    }
}

fn encode_vector(vector: &[f32]) -> Vec<u8> {
    vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// A document's card as the index stores it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCard {
    pub text: String,
    pub version: i64,
    pub has_vector: bool,
}

/// How much of a card is given to the embedding model: its start (title, size, outline).
pub const CARD_EMBED_CHARS: usize = 4_000;
/// The id a card has in the queue of vectors to compute, to tell it from a passage.
pub const CARD_ID_PREFIX: &str = "card:";

/// A section as the index stores it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSection {
    pub position: usize,
    pub parent: Option<usize>,
    pub level: u8,
    /// Empty for a slice of pages.
    pub title: String,
    pub first_page: u32,
    pub last_page: u32,
    pub source: String,
}

fn stored_section(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredSection> {
    Ok(StoredSection {
        position: row.get::<_, i64>(0)? as usize,
        parent: row.get::<_, Option<i64>>(1)?.map(|p| p as usize),
        level: row.get(2)?,
        title: row.get(3)?,
        first_page: row.get(4)?,
        last_page: row.get(5)?,
        source: row.get(6)?,
    })
}

fn decode_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// The expression that asks the passages whether the router's `word` is in the corpus: the pieces of the word
/// made of letters and digits, four characters or more (a shorter piece is a function word in French and in
/// English alike, and says nothing about the corpus), each quoted so that no character of the word can be read
/// as full-text syntax. Empty when no piece remains.
fn corpus_word_expression(word: &str) -> String {
    word.split(|c: char| !c.is_alphanumeric())
        .filter(|piece| piece.chars().count() >= 4)
        .map(|piece| format!("\"{piece}\""))
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// The most surface forms one expression carries: an entity has a handful of aliases, and a
/// question names a handful of entities, so this is a ceiling against a degenerate store and not a
/// limit anyone meets.
const MAX_ENTITY_TERMS: usize = 60;

/// An FTS5 expression for the surface forms of resolved entities: each form quoted as a phrase (so
/// `jean dupont` must appear in that order and `dupont` alone is a word), joined with OR. A form
/// of one or two letters is only kept when it has a letter and is not a lone initial; the generic
/// A form of two letters is kept (`Roy`, `Lee` and `Ng` are names, master section 4). Nothing but letters
/// and digits survives, so a form can never carry FTS syntax.
pub fn entity_terms_expression(terms: &[String]) -> String {
    let mut seen = std::collections::BTreeSet::new();
    let mut phrases = Vec::new();
    for term in terms {
        let words: Vec<String> = term
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_string)
            .collect();
        let long_enough = match words.as_slice() {
            [] => false,
            [only] => only.chars().count() >= 2,
            _ => words.iter().any(|word| word.chars().count() >= 2),
        };
        if !long_enough {
            continue;
        }
        let phrase = words.join(" ");
        if seen.insert(phrase.to_lowercase()) {
            phrases.push(format!("\"{phrase}\""));
        }
        if phrases.len() >= MAX_ENTITY_TERMS {
            break;
        }
    }
    phrases.join(" OR ")
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    cosine_similarity_with_norm(a, b, query_norm(b))
}

/// The length of a vector, as `cosine_similarity` computes it: ranking one question against thousands of
/// vectors computes it once and passes it down.
pub fn query_norm(b: &[f32]) -> f32 {
    b.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// `cosine_similarity` with the norm of `b` already known. The same operations in the same order, so the
/// same value to the last bit.
pub fn cosine_similarity_with_norm(a: &[f32], b: &[f32], norm_b: f32) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_chunk(id: &str, path: &str, text: &str) -> Chunk {
        Chunk {
            chunk_id: id.to_string(),
            relative_path: path.to_string(),
            page_number: 1,
            section: 1,
            text: text.to_string(),
            index_text: None,
            origin: crate::extraction::PageOrigin::TextLayer,
            confidence: None,
        }
    }

    #[test]
    fn the_index_connection_enforces_foreign_keys_and_waits_for_locks() {
        let dir = tempfile::tempdir().unwrap();
        let store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();

        let keys: i64 = store
            .connection()
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        let wait: i64 = store
            .connection()
            .pragma_query_value(None, "busy_timeout", |row| row.get(0))
            .unwrap();

        assert_eq!(keys, 1);
        assert_eq!(wait, 3000);
    }

    #[test]
    fn the_index_does_not_use_write_ahead_logging() {
        let dir = tempfile::tempdir().unwrap();
        let store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();

        let mode: String = store
            .connection()
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();

        assert_eq!(
            mode, "delete",
            "no -wal or -shm file may appear beside the index"
        );
        assert!(!dir.path().join("index.sqlite3-wal").exists());
    }

    #[test]
    fn a_connection_opened_elsewhere_gets_the_same_settings_from_configure_connection() {
        let connection = Connection::open_in_memory().unwrap();
        // Whether a fresh connection starts with keys on depends on how SQLite was compiled (the
        // bundled build turns them on); the index must not depend on that.
        connection
            .pragma_update(None, "foreign_keys", false)
            .unwrap();
        let before: i64 = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(before, 0);

        configure_connection(&connection).unwrap();

        let after: i64 = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(after, 1);
    }

    #[test]
    fn a_vector_round_trips_through_the_blob_encoding() {
        let vector = vec![0.1_f32, -0.5, 3.25];

        assert_eq!(decode_vector(&encode_vector(&vector)), vector);
    }

    #[test]
    fn cosine_similarity_is_one_for_identical_vectors() {
        let vector = vec![1.0_f32, 2.0, 3.0];

        assert!((cosine_similarity(&vector, &vector) - 1.0).abs() < 1e-6);
    }

    /// Stores one indexed document under `relative_path`, so a test about forgetting has
    /// something to forget.
    fn store_one(store: &mut IndexStore, relative_path: &str) {
        let chunk = sample_chunk(
            &format!("{relative_path}#p1#s1"),
            relative_path,
            "Bonjour Camille",
        );
        store
            .replace_document(
                relative_path,
                "abc123",
                false,
                std::slice::from_ref(&chunk),
                std::slice::from_ref(&vec![0.2_f32, 0.4, 0.6]),
                None,
                None,
            )
            .unwrap();
    }

    #[test]
    fn clearing_forgets_every_document_and_chunk_but_keeps_the_index_usable() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        store_one(&mut store, "inbox/letter.pdf");
        store_one(&mut store, "inbox/scan.pdf");

        store.clear().unwrap();

        assert_eq!(store.chunk_count().unwrap(), 0);
        assert!(store.all_documents().unwrap().is_empty());
        assert!(store.all_chunks().unwrap().is_empty());
        // Still writable afterwards: clearing empties the index, it does not close it.
        store_one(&mut store, "inbox/letter.pdf");
        assert_eq!(store.chunk_count().unwrap(), 1);
    }

    #[test]
    fn a_document_no_longer_in_the_folder_is_dropped_with_its_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        store_one(&mut store, "inbox/kept.pdf");
        store_one(&mut store, "inbox/deleted.pdf");

        let gone = store
            .retain_documents(&["inbox/kept.pdf".to_string()])
            .unwrap();

        assert_eq!(gone, vec!["inbox/deleted.pdf".to_string()]);
        assert_eq!(store.all_documents().unwrap().len(), 1);
        // The point of the whole thing: its passages can no longer be offered as evidence.
        let both = ["inbox/kept.pdf".to_string(), "inbox/deleted.pdf".to_string()];
        assert!(store
            .search_lexical_terms_in(&["Camille".to_string()], 10, &both)
            .unwrap()
            .iter()
            .all(|hit| !hit.chunk_id.contains("deleted")));
    }

    #[test]
    fn retaining_every_document_that_is_still_there_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        store_one(&mut store, "inbox/kept.pdf");

        let gone = store
            .retain_documents(&["inbox/kept.pdf".to_string()])
            .unwrap();

        assert!(gone.is_empty());
        assert_eq!(store.chunk_count().unwrap(), 1);
    }

    #[test]
    fn an_empty_folder_forgets_everything_rather_than_keeping_orphans() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        store_one(&mut store, "inbox/letter.pdf");

        let gone = store.retain_documents(&[]).unwrap();

        assert_eq!(gone.len(), 1);
        assert_eq!(store.chunk_count().unwrap(), 0);
    }

    #[test]
    fn cosine_similarity_is_zero_for_orthogonal_vectors() {
        assert!((cosine_similarity(&[1.0, 0.0], &[0.0, 1.0])).abs() < 1e-6);
    }

    #[test]
    fn storing_then_reading_a_document_round_trips_its_chunks_and_vectors() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        let chunk = sample_chunk(
            "inbox/letter.pdf#p1#s1",
            "inbox/letter.pdf",
            "Bonjour Camille",
        );
        let embedding = vec![0.2_f32, 0.4, 0.6];

        store
            .replace_document(
                "inbox/letter.pdf",
                "abc123",
                false,
                std::slice::from_ref(&chunk),
                std::slice::from_ref(&embedding),
                None,
                None,
            )
            .unwrap();

        let stored = store.stored_hash("inbox/letter.pdf").unwrap();
        assert_eq!(stored, Some("abc123".to_string()));

        let all = store.all_chunks().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].chunk_id, chunk.chunk_id);
        assert_eq!(all[0].embedding, embedding);
        assert_eq!(all[0].origin, crate::extraction::PageOrigin::TextLayer);
    }

    #[test]
    fn an_ocr_chunk_round_trips_origin_confidence_and_engine() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        let mut chunk = sample_chunk(
            "inbox/scan.pdf#p1#s1",
            "inbox/scan.pdf",
            "Recognised letter body",
        );
        chunk.origin = crate::extraction::PageOrigin::Ocr;
        chunk.confidence = Some(0.84);

        store
            .replace_document(
                "inbox/scan.pdf",
                "hash-ocr",
                false,
                std::slice::from_ref(&chunk),
                &[vec![0.1, 0.2]],
                Some("fake"),
                Some("0.0.0-test"),
            )
            .unwrap();

        let stored = store.stored_document("inbox/scan.pdf").unwrap().unwrap();
        assert_eq!(stored.ocr_engine.as_deref(), Some("fake"));
        assert_eq!(stored.ocr_engine_version.as_deref(), Some("0.0.0-test"));

        let all = store.all_chunks().unwrap();
        assert_eq!(all[0].origin, crate::extraction::PageOrigin::Ocr);
        assert_eq!(all[0].confidence, Some(0.84));
    }

    #[test]
    fn replacing_a_document_drops_its_previous_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        let first = sample_chunk(
            "inbox/letter.pdf#p1#s1",
            "inbox/letter.pdf",
            "Ancien contenu",
        );
        store
            .replace_document(
                "inbox/letter.pdf",
                "hash1",
                false,
                &[first],
                &[vec![0.1, 0.2]],
                None,
                None,
            )
            .unwrap();

        let second = sample_chunk(
            "inbox/letter.pdf#p1#s1v2",
            "inbox/letter.pdf",
            "Nouveau contenu",
        );
        store
            .replace_document(
                "inbox/letter.pdf",
                "hash2",
                false,
                &[second],
                &[vec![0.3, 0.4]],
                None,
                None,
            )
            .unwrap();

        let all = store.all_chunks().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].text, "Nouveau contenu");
        assert_eq!(
            store.stored_hash("inbox/letter.pdf").unwrap(),
            Some("hash2".to_string())
        );
    }

    #[test]
    fn an_empty_document_is_recorded_without_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();

        store
            .replace_document("inbox/scan.pdf", "hash-scan", true, &[], &[], None, None)
            .unwrap();

        assert_eq!(
            store.empty_documents().unwrap(),
            vec!["inbox/scan.pdf".to_string()]
        );
        assert_eq!(store.chunk_count().unwrap(), 0);
    }

    #[test]
    fn lexical_search_finds_a_matching_term() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        let chunk = sample_chunk(
            "inbox/letter.pdf#p1#s1",
            "inbox/letter.pdf",
            "HbA1c a 6.8 pourcent chez Camille Exemple",
        );
        store
            .replace_document(
                "inbox/letter.pdf",
                "hash1",
                false,
                &[chunk],
                &[vec![0.1, 0.2]],
                None,
                None,
            )
            .unwrap();

        let hits = store
            .search_lexical_terms_in(&["HbA1c".to_string()], 5, &["inbox/letter.pdf".to_string()])
            .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk_id, "inbox/letter.pdf#p1#s1");
    }

    fn store_with_three_files() -> (tempfile::TempDir, IndexStore) {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        for (path, text) in [
            (
                "Archive \u{e9}t\u{e9}/it's \"a\" name.pdf",
                "Cephalees episodiques depuis deux semaines",
            ),
            (
                "b.pdf",
                "Cephalees episodiques et nausees matinales persistantes",
            ),
            ("c.pdf", "Courrier sans rapport avec ceci"),
        ] {
            let chunk = sample_chunk(&format!("{path}#p1#s1"), path, text);
            store
                .replace_document(path, "hash", false, &[chunk], &[vec![0.1, 0.2]], None, None)
                .unwrap();
        }
        (dir, store)
    }

    #[test]
    fn a_scoped_entity_search_only_returns_chunks_of_the_given_files() {
        let (_dir, store) = store_with_three_files();
        let form = ["cephalees".to_string()];

        let hits = store
            .search_lexical_terms_in(&form, 6, &["b.pdf".to_string()])
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk_id, "b.pdf#p1#s1");

        let none = store
            .search_lexical_terms_in(&form, 6, &["c.pdf".to_string()])
            .unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn a_scoped_entity_search_over_no_files_allows_nothing() {
        let (_dir, store) = store_with_three_files();

        assert!(store
            .search_lexical_terms_in(&["cephalees".to_string()], 6, &[])
            .unwrap()
            .is_empty());
    }

    /// Folders keep their accents (only file names are made plain), and a path may hold a quote:
    /// the set travels as a JSON value, never spliced into the SQL.
    #[test]
    fn a_scoped_entity_search_accepts_accents_and_quotes_in_a_path() {
        let (_dir, store) = store_with_three_files();
        let awkward = "Archive \u{e9}t\u{e9}/it's \"a\" name.pdf".to_string();

        let hits = store
            .search_lexical_terms_in(
                &["cephalees".to_string()],
                6,
                std::slice::from_ref(&awkward),
            )
            .unwrap();

        assert_eq!(hits.len(), 1);
        assert!(hits[0].chunk_id.starts_with(&awkward));
    }

    #[test]
    fn a_scoped_entity_search_spends_its_limit_inside_the_scope() {
        let (_dir, store) = store_with_three_files();

        let hits = store
            .search_lexical_terms_in(
                &["cephalees episodiques".to_string()],
                1,
                &["b.pdf".to_string()],
            )
            .unwrap();

        assert_eq!(hits.len(), 1, "the limit applies after the restriction");
        assert_eq!(hits[0].chunk_id, "b.pdf#p1#s1");
    }

    fn sample_inventory(relative_path: &str, workbook_id: &str) -> TabularInventory {
        use crate::tabular::inventory::{
            ColumnInventory, ColumnType, SheetInventory, TabularFormat,
        };

        TabularInventory {
            workbook_id: workbook_id.to_string(),
            relative_path: relative_path.to_string(),
            format: TabularFormat::Csv,
            sheets: vec![SheetInventory {
                name: relative_path.to_string(),
                header_row: Some(0),
                row_count: 1,
                column_count: 1,
                columns: vec![ColumnInventory {
                    name: "montant".to_string(),
                    index: 0,
                    inferred_type: ColumnType::Numeric,
                    has_formulas: false,
                    year_like: false,
                    unit: None,
                    unparsed_count: 0,
                    ambiguous_date: false,
                }],
                has_formulas: false,
                formula_cells: 0,
            }],
        }
    }

    #[test]
    fn a_tabular_inventory_is_found_again_by_its_workbook_hash() {
        let dir = tempfile::tempdir().unwrap();
        let store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        let inventory = sample_inventory("data/montants.csv", "hash-1");

        store.put_tabular_inventory(&inventory).unwrap();
        let found = store.tabular_inventory_by_hash("hash-1").unwrap();

        assert_eq!(found, Some(inventory));
        assert_eq!(
            store.tabular_inventory_by_hash("hash-absent").unwrap(),
            None
        );
    }

    #[test]
    fn a_changed_file_is_no_longer_found_under_its_old_hash() {
        let dir = tempfile::tempdir().unwrap();
        let store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        let before = sample_inventory("data/montants.csv", "hash-old");
        store.put_tabular_inventory(&before).unwrap();

        // Same path, new content: the pass that re-reads a changed file stores its new hash at
        // the same relative path, exactly as `put_tabular_inventory`'s own contract promises.
        let after = sample_inventory("data/montants.csv", "hash-new");
        store.put_tabular_inventory(&after).unwrap();

        assert_eq!(store.tabular_inventory_by_hash("hash-old").unwrap(), None);
        assert_eq!(
            store.tabular_inventory_by_hash("hash-new").unwrap(),
            Some(after)
        );
    }

    #[test]
    fn invalidating_a_path_forgets_whatever_was_cached_there() {
        let dir = tempfile::tempdir().unwrap();
        let store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        let inventory = sample_inventory("data/montants.csv", "hash-1");
        store.put_tabular_inventory(&inventory).unwrap();

        store
            .invalidate_tabular_inventory("data/montants.csv")
            .unwrap();

        assert_eq!(store.tabular_inventory_by_hash("hash-1").unwrap(), None);
    }

    #[test]
    fn every_cached_workbook_is_listed_by_path_and_hash() {
        let dir = tempfile::tempdir().unwrap();
        let store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        store
            .put_tabular_inventory(&sample_inventory("b.csv", "hash-b"))
            .unwrap();
        store
            .put_tabular_inventory(&sample_inventory("a.csv", "hash-a"))
            .unwrap();

        assert_eq!(
            store.all_tabular_inventories().unwrap(),
            vec![
                TabularInventoryRow {
                    relative_path: "a.csv".into(),
                    workbook_id: "hash-a".into()
                },
                TabularInventoryRow {
                    relative_path: "b.csv".into(),
                    workbook_id: "hash-b".into()
                },
            ]
        );
    }

    #[test]
    fn a_workbook_gone_from_the_folder_is_dropped_and_named() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        store
            .put_tabular_inventory(&sample_inventory("kept.csv", "hash-k"))
            .unwrap();
        store
            .put_tabular_inventory(&sample_inventory("gone.csv", "hash-g"))
            .unwrap();

        let dropped = store
            .retain_tabular_inventories(&["kept.csv".to_string()])
            .unwrap();

        assert_eq!(dropped, vec!["gone.csv".to_string()]);
        assert_eq!(store.tabular_inventory_by_hash("hash-g").unwrap(), None);
        assert!(store.tabular_inventory_by_hash("hash-k").unwrap().is_some());
    }

    #[test]
    fn the_two_resets_leave_each_others_tables_alone() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        store
            .replace_document(
                "letter.txt",
                "sha-l",
                false,
                &[sample_chunk("letter.txt#p1#s1", "letter.txt", "Bonjour")],
                &[vec![1.0]],
                None,
                None,
            )
            .unwrap();
        store
            .put_tabular_inventory(&sample_inventory("data.csv", "hash-d"))
            .unwrap();

        store.clear_tabular_inventories().unwrap();
        assert_eq!(
            store.chunk_count().unwrap(),
            1,
            "the Data reset kept the documents"
        );
        assert!(store.all_tabular_inventories().unwrap().is_empty());

        store
            .put_tabular_inventory(&sample_inventory("data.csv", "hash-d"))
            .unwrap();
        store.clear().unwrap();
        assert_eq!(store.chunk_count().unwrap(), 0);
        assert_eq!(
            store.all_tabular_inventories().unwrap().len(),
            1,
            "the Documents reset kept the workbooks"
        );
    }

    fn sample_workbook() -> Workbook {
        use crate::tabular::{CellValue, SheetData};

        Workbook {
            sheets: vec![SheetData {
                name: "Feuille1".to_string(),
                rows: vec![
                    vec![CellValue::Text("montant".to_string())],
                    vec![CellValue::Number(10.0)],
                ],
            }],
        }
    }

    #[test]
    fn a_typed_workbook_is_found_again_by_its_content_hash() {
        let dir = tempfile::tempdir().unwrap();
        let store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        let workbook = sample_workbook();

        store.put_tabular_workbook("hash-1", &workbook).unwrap();

        assert_eq!(
            store.tabular_workbook_by_hash("hash-1").unwrap(),
            Some(workbook)
        );
        assert_eq!(store.tabular_workbook_by_hash("hash-absent").unwrap(), None);
    }

    #[test]
    fn writing_the_same_hash_twice_replaces_rather_than_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        let workbook = sample_workbook();

        store.put_tabular_workbook("hash-1", &workbook).unwrap();
        store.put_tabular_workbook("hash-1", &workbook).unwrap();

        assert_eq!(
            store.tabular_workbook_by_hash("hash-1").unwrap(),
            Some(workbook)
        );
    }

    #[test]
    fn clearing_typed_workbooks_leaves_the_inventories_and_documents_alone() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        store
            .put_tabular_workbook("hash-1", &sample_workbook())
            .unwrap();
        store
            .put_tabular_inventory(&sample_inventory("data.csv", "hash-1"))
            .unwrap();
        store
            .replace_document(
                "letter.txt",
                "sha-l",
                false,
                &[sample_chunk("letter.txt#p1#s1", "letter.txt", "Bonjour")],
                &[vec![1.0]],
                None,
                None,
            )
            .unwrap();

        store.clear_tabular_workbooks().unwrap();

        assert_eq!(store.tabular_workbook_by_hash("hash-1").unwrap(), None);
        assert!(
            store.tabular_inventory_by_hash("hash-1").unwrap().is_some(),
            "clearing the cell cache must not touch the structural inventory"
        );
        assert_eq!(
            store.chunk_count().unwrap(),
            1,
            "clearing the cell cache must not touch the documents"
        );
    }

    #[test]
    fn syntax_characters_in_a_name_or_a_code_do_not_break_the_search() {
        let dir = tempfile::tempdir().unwrap();
        let store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
        let paths = ["a.pdf".to_string()];

        let forms = ["Camille's \"HbA1c\" level?".to_string(), "NEAR(".to_string()];
        assert!(store.search_lexical_terms_in(&forms, 5, &paths).unwrap().is_empty());
        assert!(store
            .search_identifier("What is FAC-2026-014\"? OR (", 5, Some(&paths))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn the_router_asks_the_passages_whether_a_word_is_in_the_corpus() {
        use crate::folder_questions::CorpusWords;
        let (_dir, store) = store_with_three_files();

        assert!(store.contains("cephalees"));
        assert!(store.contains("Nausees"));
        assert!(!store.contains("zirconium"));
        assert!(!store.contains("ces"), "a word of fewer than four letters says nothing");
        assert!(!store.contains("\"(\""));
    }
}
