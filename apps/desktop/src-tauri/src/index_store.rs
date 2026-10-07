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

use rusqlite::Connection;

use crate::chunking::Chunk;
use crate::error::AppError;
use crate::extraction::PageOrigin;
use crate::tabular::inventory::TabularInventory;
use crate::tabular::Workbook;

const INDEX_FILE_NAME: &str = "index.sqlite3";

pub struct IndexStore {
    connection: Connection,
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

impl IndexStore {
    pub fn open_at(path: &std::path::Path) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| AppError::IndexUnavailable)?;
        }
        let connection = Connection::open(path).map_err(|_| AppError::IndexUnavailable)?;
        Self::migrate(&connection)?;
        Ok(Self { connection })
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
                    embedding BLOB NOT NULL
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
        Ok(())
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
        let tx = self
            .connection
            .transaction()
            .map_err(|_| AppError::IndexUnavailable)?;
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
            let bytes = encode_vector(embedding);
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
            tx.execute(
                "INSERT INTO chunks_fts (chunk_id, text) VALUES (?1, ?2)",
                rusqlite::params![chunk.chunk_id, chunk.text],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
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

        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
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
                let embedding_bytes: Vec<u8> = row.get(5)?;
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

    /// Forget every document and every chunk, keeping the file and its schema.
    ///
    /// The index is emptied rather than deleted: the connection is open, and on Windows a file
    /// held open cannot be removed underneath it. One transaction, so a failure halfway leaves a
    /// whole index rather than a half-erased one. Nothing in the work folder is touched - this
    /// forgets what was read, never what was read *from* (`docs/PRIVACY-AND-SECURITY.md`).
    pub fn clear(&mut self) -> Result<(), AppError> {
        let tx = self
            .connection
            .transaction()
            .map_err(|_| AppError::IndexUnavailable)?;
        tx.execute_batch(
            "DELETE FROM chunks_fts; DELETE FROM chunks; DELETE FROM documents;",
        )
        .map_err(|_| AppError::IndexUnavailable)?;
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(())
    }

    /// Drop every document whose relative path is not in `present`, and its chunks with it.
    ///
    /// A file she deleted from the work folder must stop being citable. Until this existed, its
    /// rows outlived it: the folder panel stopped showing it, because that reads the filesystem,
    /// while retrieval went on offering its passages as evidence for an answer. Returns the paths
    /// dropped, so the pass can report them by name rather than as a count.
    pub fn retain_documents(&mut self, present: &[String]) -> Result<Vec<String>, AppError> {
        let known: std::collections::BTreeSet<&str> =
            present.iter().map(String::as_str).collect();
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

        let tx = self
            .connection
            .transaction()
            .map_err(|_| AppError::IndexUnavailable)?;
        for path in &gone {
            // The FTS table mirrors `chunks` by `chunk_id`, so it is cleared from the same list
            // rather than from a second query that could disagree with it.
            tx.execute(
                "DELETE FROM chunks_fts WHERE chunk_id IN
                     (SELECT chunk_id FROM chunks WHERE relative_path = ?1)",
                [path],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
            tx.execute("DELETE FROM chunks WHERE relative_path = ?1", [path])
                .map_err(|_| AppError::IndexUnavailable)?;
            tx.execute("DELETE FROM documents WHERE relative_path = ?1", [path])
                .map_err(|_| AppError::IndexUnavailable)?;
        }
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

    /// FTS5 lexical search, escaping the query to a simple `AND`-of-terms match so punctuation
    /// in a question cannot break the FTS syntax.
    pub fn search_lexical(&self, query: &str, limit: usize) -> Result<Vec<LexicalHit>, AppError> {
        let match_expression = fts_match_expression(query);
        if match_expression.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement = self
            .connection
            .prepare(
                "SELECT chunk_id, bm25(chunks_fts) FROM chunks_fts
                 WHERE chunks_fts MATCH ?1 ORDER BY bm25(chunks_fts) LIMIT ?2",
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map(rusqlite::params![match_expression, limit as i64], |row| {
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
                let embedding_bytes: Vec<u8> = row.get(5)?;
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
                    let embedding_bytes: Vec<u8> = row.get(5)?;
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
        let json = serde_json::to_string(inventory).map_err(|_| AppError::IndexUnavailable)?;
        self.connection
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
        let known: std::collections::BTreeSet<&str> =
            present.iter().map(String::as_str).collect();
        let gone: Vec<String> = self
            .all_tabular_inventories()?
            .into_iter()
            .map(|row| row.relative_path)
            .filter(|path| !known.contains(path.as_str()))
            .collect();
        if gone.is_empty() {
            return Ok(gone);
        }
        let tx = self
            .connection
            .transaction()
            .map_err(|_| AppError::IndexUnavailable)?;
        for path in &gone {
            tx.execute(
                "DELETE FROM tabular_inventories WHERE relative_path = ?1",
                [path],
            )
            .map_err(|_| AppError::IndexUnavailable)?;
        }
        tx.commit().map_err(|_| AppError::IndexUnavailable)?;
        Ok(gone)
    }

    /// Forget every cached workbook inventory, and nothing else. The Data Folder card's Reset: the
    /// two folders are reset from two cards, independently, so this must never touch `documents`
    /// or `chunks` - exactly as `clear` never touches this table.
    pub fn clear_tabular_inventories(&mut self) -> Result<(), AppError> {
        self.connection
            .execute("DELETE FROM tabular_inventories", [])
            .map_err(|_| AppError::IndexUnavailable)?;
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
        !self
            .search_lexical(word, 1)
            .map(|hits| hits.is_empty())
            .unwrap_or(true)
    }
}

fn encode_vector(vector: &[f32]) -> Vec<u8> {
    vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn decode_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// A plain `OR`-of-terms match: strip FTS5 operator characters and quote each term, so a
/// question containing `?`, `"` or `-` cannot be read as query syntax. `OR` rather than `AND`
/// because a natural-language question shares only its content words with the passage that
/// answers it, rarely every word in the sentence.
///
/// Terms shorter than four characters are dropped: a length cutoff, not a language-specific
/// stopword list, but it removes most short function words in French and in English alike, so
/// a lexical hit means a real content word matched rather than an article or a preposition
/// that recurs in every document in the folder.
fn fts_match_expression(query: &str) -> String {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|term| term.chars().count() >= 4)
        .map(|term| format!("\"{term}\""))
        .collect::<Vec<_>>()
        .join(" OR ")
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
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
            origin: crate::extraction::PageOrigin::TextLayer,
            confidence: None,
        }
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
        assert!(store
            .search_lexical("Camille", 10)
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

        let hits = store.search_lexical("HbA1c", 5).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk_id, "inbox/letter.pdf#p1#s1");
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
        assert_eq!(store.chunk_count().unwrap(), 1, "the Data reset kept the documents");
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
    fn a_question_mark_in_the_query_does_not_break_the_search() {
        let dir = tempfile::tempdir().unwrap();
        let store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();

        let hits = store
            .search_lexical("What is Camille's HbA1c level?", 5)
            .unwrap();

        assert!(hits.is_empty());
    }
}
