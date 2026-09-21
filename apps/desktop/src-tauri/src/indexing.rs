//! Orchestrates the chain: discovery -> extraction -> chunking -> embeddings -> local index.
//!
//! Kept out of `commands.rs` so the pipeline can be exercised without Tauri, and so no piece of
//! it depends on the webview being present.

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::chunking;
use crate::discovery::{self, DiscoveredFile};
use crate::error::AppError;
use crate::extraction::{self, ExtractionError, PageOrigin};
use crate::gateway::{GatewayClient, MAX_EMBEDDING_CHARS, MAX_EMBEDDING_INPUTS};
use crate::index_store::IndexStore;
use crate::ocr::OcrProvider;
use crate::raster::PageRasterizer;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexSummary {
    pub scanned_files: usize,
    pub indexed_files: usize,
    pub unchanged_files: usize,
    /// Files that read as empty: a PDF with no text layer, most often a scan. Reported by name,
    /// never silently dropped.
    pub empty_files: Vec<String>,
    /// Files that produced at least one chunk from OCR, so the interface can say which ones a
    /// machine read rather than copied (`docs/SPRINT-2.5-ASSESSMENT.md` section D).
    pub ocr_files: Vec<String>,
    /// Files where at least one page came back below the OCR confidence floor. No raw score.
    pub low_confidence_files: Vec<String>,
    pub chunk_count: u64,
}

/// One pass over the work folder. Files whose content hash has not changed since the last pass
/// are skipped rather than re-extracted and re-embedded, unless the OCR engine that produced
/// their stored chunks is no longer the one configured.
pub async fn run(
    index: &mut IndexStore,
    gateway: &GatewayClient,
    server_url: &str,
    embedding_alias: &str,
    work_folder: &std::path::Path,
    ocr: Option<&dyn OcrProvider>,
    rasterizer: Option<&dyn PageRasterizer>,
    locale: &str,
) -> Result<IndexSummary, AppError> {
    let files = discovery::discover(work_folder);
    let mut indexed_files = 0usize;
    let mut unchanged_files = 0usize;
    let mut empty_files = Vec::new();
    let mut ocr_files = Vec::new();
    let mut low_confidence_files = Vec::new();

    for file in &files {
        let sha256 = match hash_file(file) {
            Ok(hash) => hash,
            Err(_) => continue,
        };
        if should_skip(index, file, &sha256, ocr)? {
            unchanged_files += 1;
            continue;
        }

        match extraction::extract(file, ocr, rasterizer, locale) {
            Ok(document) if document.empty => {
                let (engine, version) = ocr_identity(ocr, document.used_ocr);
                index.replace_document(
                    &file.relative_path,
                    &sha256,
                    true,
                    &[],
                    &[],
                    engine,
                    version,
                )?;
                empty_files.push(file.relative_path.clone());
                if document.low_confidence {
                    low_confidence_files.push(file.relative_path.clone());
                }
            }
            Ok(document) => {
                if document.low_confidence {
                    low_confidence_files.push(file.relative_path.clone());
                }
                let chunks = chunking::chunk(&document);
                if chunks.is_empty() {
                    let (engine, version) = ocr_identity(ocr, document.used_ocr);
                    index.replace_document(
                        &file.relative_path,
                        &sha256,
                        true,
                        &[],
                        &[],
                        engine,
                        version,
                    )?;
                    empty_files.push(file.relative_path.clone());
                } else {
                    let embeddings =
                        embed_chunks(gateway, server_url, embedding_alias, &chunks).await?;
                    let (engine, version) = ocr_identity(ocr, document.used_ocr);
                    index.replace_document(
                        &file.relative_path,
                        &sha256,
                        false,
                        &chunks,
                        &embeddings,
                        engine,
                        version,
                    )?;
                    indexed_files += 1;
                    if chunks.iter().any(|chunk| chunk.origin == PageOrigin::Ocr) {
                        ocr_files.push(file.relative_path.clone());
                    }
                }
            }
            Err(ExtractionError::UnsupportedExtension) => continue,
            Err(_) => {
                return Err(AppError::ExtractionFailed {
                    path: file.relative_path.clone(),
                })
            }
        }
    }

    Ok(IndexSummary {
        scanned_files: files.len(),
        indexed_files,
        unchanged_files,
        empty_files,
        ocr_files,
        low_confidence_files,
        chunk_count: index.chunk_count()?,
    })
}

fn ocr_identity<'a>(
    ocr: Option<&'a dyn OcrProvider>,
    used_ocr: bool,
) -> (Option<&'a str>, Option<&'a str>) {
    if used_ocr {
        ocr.map(|provider| (Some(provider.id()), Some(provider.version())))
            .unwrap_or((None, None))
    } else {
        (None, None)
    }
}

fn should_skip(
    index: &IndexStore,
    file: &DiscoveredFile,
    sha256: &str,
    ocr: Option<&dyn OcrProvider>,
) -> Result<bool, AppError> {
    let Some(stored) = index.stored_document(&file.relative_path)? else {
        return Ok(false);
    };
    if stored.sha256 != sha256 {
        return Ok(false);
    }

    let engine_changed = match (
        stored.ocr_engine.as_deref(),
        stored.ocr_engine_version.as_deref(),
        ocr,
    ) {
        (Some(stored_id), Some(stored_version), Some(provider)) => {
            stored_id != provider.id() || stored_version != provider.version()
        }
        _ => false,
    };
    if engine_changed {
        return Ok(false);
    }

    // An empty scan stored before the engine existed should be retried now that we have one.
    let retry_empty = stored.empty && ocr.is_some() && stored.ocr_engine.is_none();
    Ok(!retry_empty)
}

fn hash_file(file: &DiscoveredFile) -> std::io::Result<String> {
    let bytes = std::fs::read(&file.absolute_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Send chunk text to the gateway in batches that respect the embedding caps client-side too
/// (`docs/RETRIEVAL.md`): the gateway does not trust a caller, but a client that never sends an
/// oversized batch in the first place fails faster and closer to the cause.
async fn embed_chunks(
    gateway: &GatewayClient,
    server_url: &str,
    embedding_alias: &str,
    chunks: &[chunking::Chunk],
) -> Result<Vec<Vec<f32>>, AppError> {
    let mut vectors = Vec::with_capacity(chunks.len());
    let mut batch: Vec<String> = Vec::new();
    let mut batch_chars = 0usize;

    async fn flush(
        gateway: &GatewayClient,
        server_url: &str,
        embedding_alias: &str,
        batch: &mut Vec<String>,
        vectors: &mut Vec<Vec<f32>>,
    ) -> Result<(), AppError> {
        if batch.is_empty() {
            return Ok(());
        }
        let embedded = gateway.embed(server_url, embedding_alias, batch).await?;
        vectors.extend(embedded);
        batch.clear();
        Ok(())
    }

    for chunk in chunks {
        let chunk_chars = chunk.text.chars().count();
        if !batch.is_empty()
            && (batch.len() + 1 > MAX_EMBEDDING_INPUTS
                || batch_chars + chunk_chars > MAX_EMBEDDING_CHARS)
        {
            flush(
                gateway,
                server_url,
                embedding_alias,
                &mut batch,
                &mut vectors,
            )
            .await?;
            batch_chars = 0;
        }
        batch.push(chunk.text.clone());
        batch_chars += chunk_chars;
    }
    flush(
        gateway,
        server_url,
        embedding_alias,
        &mut batch,
        &mut vectors,
    )
    .await?;

    Ok(vectors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashing_the_same_bytes_twice_is_stable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, b"contenu").unwrap();
        let file = DiscoveredFile {
            relative_path: "a.txt".into(),
            absolute_path: path.display().to_string(),
            extension: "txt".into(),
            size_bytes: 7,
            modified_at: None,
        };

        let first = hash_file(&file).unwrap();
        let second = hash_file(&file).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn changing_the_content_changes_the_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, b"contenu un").unwrap();
        let file = DiscoveredFile {
            relative_path: "a.txt".into(),
            absolute_path: path.display().to_string(),
            extension: "txt".into(),
            size_bytes: 0,
            modified_at: None,
        };
        let before = hash_file(&file).unwrap();

        std::fs::write(&path, b"contenu deux").unwrap();
        let after = hash_file(&file).unwrap();

        assert_ne!(before, after);
    }
}
