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

/// The OCR engine did not start. Every scan reads as unreadable, however clear it is.
pub const CAPABILITY_OCR_ENGINE: &str = "ocrEngine";

/// The PDF page rasteriser did not start. A scanned PDF cannot reach OCR at all; a JPEG or PNG
/// still can, which is what made the failure in `docs/TROUBLESHOOTING.md` look like a bad
/// document rather than a missing capability.
pub const CAPABILITY_PAGE_RASTERIZER: &str = "pageRasterizer";

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
    /// Documents dropped from the index because they are no longer in the work folder. Reported
    /// by name: forgetting a document is as much a result of a pass as reading one, and she is
    /// the only one who can tell a deliberate deletion from a folder that failed to mount.
    pub removed_files: Vec<String>,
    /// Machine codes for an ingestion capability that did not start for this pass, in a stable
    /// order. Empty on a healthy installation. Reported so an unreadable scan can be explained
    /// by the interface instead of being blamed on the document: the engine being absent and the
    /// document being illegible used to be the same silent outcome
    /// (`docs/TROUBLESHOOTING.md`). English codes; the interface localises them
    /// (`docs/LANGUAGE-AND-LOCALE.md`).
    pub unavailable_capabilities: Vec<&'static str>,
    pub chunk_count: u64,
}

/// How far one pass has got, sent while it runs so a long analysis shows its progress rather than
/// an unbroken spinner. Counts only: no document text and not even a file name travels here.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    /// Files already dealt with, whether indexed, skipped as unchanged, or unreadable.
    pub processed_files: usize,
    pub total_files: usize,
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
    // `on_progress` is called once before the first file and once after the last. `Sync` rather
    // than a plain closure so the future stays `Send` and can be awaited from a Tauri command.
    on_progress: &(dyn Fn(IndexProgress) + Sync),
) -> Result<IndexSummary, AppError> {
    let unavailable_capabilities = unavailable_capabilities(ocr, rasterizer);
    let files = discovery::discover(work_folder);
    // Before reading anything: a document she deleted must stop being citable, and that is true
    // whether or not the rest of the pass succeeds. Keyed on the same walk the pass itself uses,
    // so the index and the folder panel cannot disagree about what is there.
    let removed_files = index.retain_documents(
        &files
            .iter()
            .map(|file| file.relative_path.clone())
            .collect::<Vec<_>>(),
    )?;
    let total_files = files.len();
    // Sent before any work, so the bar appears at zero rather than only once the first file is
    // done - on a folder of scans that first file can take a while on its own.
    on_progress(IndexProgress {
        processed_files: 0,
        total_files,
    });
    let mut indexed_files = 0usize;
    let mut unchanged_files = 0usize;
    let mut empty_files = Vec::new();
    let mut ocr_files = Vec::new();
    let mut low_confidence_files = Vec::new();

    for (position, file) in files.iter().enumerate() {
        // Reported as this file starts rather than as it ends, because several branches below
        // `continue`, and a count some paths forget to advance is worse than one that is a
        // single file behind. The final call after the loop closes the gap.
        on_progress(IndexProgress {
            processed_files: position,
            total_files,
        });
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

    on_progress(IndexProgress {
        processed_files: total_files,
        total_files,
    });

    Ok(IndexSummary {
        scanned_files: total_files,
        indexed_files,
        unchanged_files,
        empty_files,
        ocr_files,
        low_confidence_files,
        removed_files,
        unavailable_capabilities,
        chunk_count: index.chunk_count()?,
    })
}

/// Which ingestion capabilities are missing for this pass. Read from the ports themselves rather
/// than from what the pass happened to encounter, so a folder that holds no scan today still
/// reports an engine that will fail the first scan added tomorrow.
fn unavailable_capabilities(
    ocr: Option<&dyn OcrProvider>,
    rasterizer: Option<&dyn PageRasterizer>,
) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if ocr.is_none() {
        missing.push(CAPABILITY_OCR_ENGINE);
    }
    if rasterizer.is_none() {
        missing.push(CAPABILITY_PAGE_RASTERIZER);
    }
    missing
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
    use crate::ocr::fake::FakeOcrProvider;
    use crate::raster::FakeRasterizer;

    #[test]
    fn a_healthy_pass_reports_no_missing_capability() {
        let ocr = FakeOcrProvider::new();
        let rasterizer = FakeRasterizer::new();

        let missing = unavailable_capabilities(Some(&ocr), Some(&rasterizer));

        assert!(missing.is_empty());
    }

    #[test]
    fn a_missing_rasterizer_is_named_even_though_images_still_work() {
        let ocr = FakeOcrProvider::new();

        let missing = unavailable_capabilities(Some(&ocr), None);

        assert_eq!(missing, vec![CAPABILITY_PAGE_RASTERIZER]);
    }

    #[test]
    fn a_missing_engine_is_named() {
        let rasterizer = FakeRasterizer::new();

        let missing = unavailable_capabilities(None, Some(&rasterizer));

        assert_eq!(missing, vec![CAPABILITY_OCR_ENGINE]);
    }

    #[test]
    fn both_missing_engines_are_named_in_a_stable_order() {
        let missing = unavailable_capabilities(None, None);

        assert_eq!(
            missing,
            vec![CAPABILITY_OCR_ENGINE, CAPABILITY_PAGE_RASTERIZER]
        );
    }

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
