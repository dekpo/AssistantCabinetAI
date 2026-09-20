//! The local index: SQLite, full-text search plus stored vectors, brute-force cosine.
//!
//! Lives in `%LOCALAPPDATA%` (`app_local_data_dir()`), never the roaming `%APPDATA%` that holds
//! `settings.json` - the index holds document text, and a roaming profile would copy it off the
//! machine (`docs/RETRIEVAL.md`). One file, one work folder: re-choosing the work folder starts a
//! fresh index rather than mixing two corpora.

use rusqlite::Connection;

use crate::chunking::Chunk;
use crate::error::AppError;
use crate::extraction::PageOrigin;

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
            .execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"), [])
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
        Ok(self.stored_document(relative_path)?.map(|document| document.sha256))
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
