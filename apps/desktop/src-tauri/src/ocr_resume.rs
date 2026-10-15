//! Reading a long scan in several passes (long-documents programme, lot 4).
//!
//! Recognising a page costs seconds; a scan of a thousand pages costs hours. A pass that is stopped, or
//! that fails, must not lose the pages it already recognised. Each recognised page is kept in the index
//! (`ocr_page_cache`, keyed by the file and its hash) as soon as it is recognised, and the next pass
//! takes it from there instead of rasterising and recognising it again. The kept pages are dropped when
//! the document is stored, and when the file changes (another hash).
//!
//! It sits beside the two ports, as decorators: `CachedRasterizer` answers a kept page with a marker
//! instead of rendering it and honours a stop between two pages; `CachedOcr` turns the marker back into
//! the kept page and keeps every page the engine recognises. Neither the extraction nor the engine knows.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::ocr::{ImageFormat, OcrError, OcrInput, OcrPage, OcrProvider, OcrStatus};
use crate::raster::PageRasterizer;

/// What the rasterizer returns for a page that is already kept.
const KEPT_PAGE: &[u8] = b"acai-kept-page";

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS ocr_page_cache (
    relative_path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    page_number INTEGER NOT NULL,
    status TEXT NOT NULL,
    text TEXT NOT NULL,
    confidence REAL,
    engine TEXT NOT NULL,
    engine_version TEXT NOT NULL,
    PRIMARY KEY (relative_path, sha256, page_number)
);
";

/// Pages are written to the index in groups of this many: one transaction (one disk sync) per group
/// instead of per page. A stop or the end of the file writes whatever is waiting; only a hard kill of
/// the application can lose the pages of the last group, a few minutes of recognition at most.
const PAGES_PER_WRITE: usize = 10;

struct Inner {
    connection: Connection,
    waiting: Vec<(String, OcrPage)>,
}

pub struct PageCache {
    inner: Mutex<Inner>,
}

fn status_code(status: OcrStatus) -> &'static str {
    match status {
        OcrStatus::Recognised => "recognised",
        OcrStatus::BelowThreshold => "below_threshold",
        OcrStatus::NoTextFound => "no_text",
    }
}

fn status_of(code: &str) -> OcrStatus {
    match code {
        "recognised" => OcrStatus::Recognised,
        "below_threshold" => OcrStatus::BelowThreshold,
        _ => OcrStatus::NoTextFound,
    }
}

impl PageCache {
    /// A connection of its own to the index file: the recognition runs while the pass holds the index.
    /// `None` when it cannot be opened; the pass then reads the scan as it always did.
    pub fn open(index_path: &Path) -> Option<Self> {
        let connection = Connection::open(index_path).ok()?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .ok()?;
        connection.execute_batch(SCHEMA).ok()?;
        Some(Self {
            inner: Mutex::new(Inner {
                connection,
                waiting: Vec::new(),
            }),
        })
    }

    fn get(&self, relative_path: &str, sha256: &str, page_number: u32) -> Option<OcrPage> {
        let inner = self.inner.lock().ok()?;
        if let Some((_, page)) = inner.waiting.iter().find(|(hash, page)| {
            hash == sha256 && page.relative_path == relative_path && page.page_number == page_number
        }) {
            return Some(page.clone());
        }
        inner
            .connection
            .query_row(
                "SELECT status, text, confidence, engine, engine_version FROM ocr_page_cache
                 WHERE relative_path = ?1 AND sha256 = ?2 AND page_number = ?3",
                params![relative_path, sha256, page_number],
                |row| {
                    Ok(OcrPage {
                        relative_path: relative_path.to_string(),
                        page_number,
                        status: status_of(&row.get::<_, String>(0)?),
                        text: row.get(1)?,
                        confidence: row.get(2)?,
                        engine: row.get(3)?,
                        engine_version: row.get(4)?,
                    })
                },
            )
            .optional()
            .ok()
            .flatten()
    }

    fn put(&self, sha256: &str, page: &OcrPage) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.waiting.push((sha256.to_string(), page.clone()));
            if inner.waiting.len() >= PAGES_PER_WRITE {
                Self::flush(&mut inner);
            }
        }
    }

    fn flush(inner: &mut Inner) {
        if inner.waiting.is_empty() {
            return;
        }
        let waiting = std::mem::take(&mut inner.waiting);
        let Ok(tx) = inner.connection.transaction() else {
            return;
        };
        for (sha256, page) in &waiting {
            let _ = tx.execute(
                "INSERT OR REPLACE INTO ocr_page_cache
                    (relative_path, sha256, page_number, status, text, confidence, engine, engine_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    page.relative_path,
                    sha256,
                    page.page_number,
                    status_code(page.status),
                    page.text,
                    page.confidence,
                    page.engine,
                    page.engine_version,
                ],
            );
        }
        let _ = tx.commit();
    }

    /// Drop what was kept for earlier versions of this file.
    pub fn forget_other_versions(&self, relative_path: &str, sha256: &str) {
        if let Ok(inner) = self.inner.lock() {
            let _ = inner.connection.execute(
                "DELETE FROM ocr_page_cache WHERE relative_path = ?1 AND sha256 <> ?2",
                params![relative_path, sha256],
            );
        }
    }

    /// Drop everything kept for this file: its document is stored.
    pub fn forget(&self, relative_path: &str) {
        if let Ok(mut inner) = self.inner.lock() {
            inner
                .waiting
                .retain(|(_, page)| page.relative_path != relative_path);
            let _ = inner.connection.execute(
                "DELETE FROM ocr_page_cache WHERE relative_path = ?1",
                [relative_path],
            );
        }
    }
}

impl Drop for PageCache {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.inner.lock() {
            Self::flush(&mut inner);
        }
    }
}

pub struct CachedRasterizer<'a> {
    pub inner: &'a dyn PageRasterizer,
    pub cache: &'a PageCache,
    pub relative_path: &'a str,
    pub sha256: &'a str,
    pub cancelled: &'a (dyn Fn() -> bool + Sync),
}

impl PageRasterizer for CachedRasterizer<'_> {
    fn page_count(&self, pdf_path: &Path) -> Result<u32, OcrError> {
        self.inner.page_count(pdf_path)
    }

    fn rasterize_page(&self, pdf_path: &Path, page_number: u32) -> Result<Vec<u8>, OcrError> {
        if (self.cancelled)() {
            return Err(OcrError::Stopped);
        }
        if self
            .cache
            .get(self.relative_path, self.sha256, page_number)
            .is_some()
        {
            return Ok(KEPT_PAGE.to_vec());
        }
        self.inner.rasterize_page(pdf_path, page_number)
    }
}

pub struct CachedOcr<'a> {
    pub inner: &'a dyn OcrProvider,
    pub cache: &'a PageCache,
    pub sha256: &'a str,
}

impl OcrProvider for CachedOcr<'_> {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn version(&self) -> &str {
        self.inner.version()
    }

    fn supports(&self, format: ImageFormat) -> bool {
        self.inner.supports(format)
    }

    fn recognise(&self, input: &OcrInput<'_>) -> Result<OcrPage, OcrError> {
        if input.image == KEPT_PAGE {
            return self
                .cache
                .get(input.relative_path, self.sha256, input.page_number)
                .ok_or(OcrError::UnreadableImage);
        }
        let page = self.inner.recognise(input)?;
        self.cache.put(self.sha256, &page);
        Ok(page)
    }
}
