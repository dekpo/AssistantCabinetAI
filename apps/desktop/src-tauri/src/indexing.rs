//! Orchestrates the chain: discovery -> extraction -> chunking -> embeddings -> local index.
//!
//! Kept out of `commands.rs` so the pipeline can be exercised without Tauri, and so no piece of
//! it depends on the webview being present.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::chunking;
use crate::discovery::{self, DiscoveredFile};
use crate::error::AppError;
use crate::extraction::{self, ExtractionError, PageOrigin};
use crate::gateway::GatewayClient;
use crate::index_store::IndexStore;
use crate::knowledge::diagnostics::{
    elapsed_ms, timed_ms, AnalysisPass, AnalysisTimings, FileTiming, StageTimer,
};
use crate::ocr::{ImageFormat, OcrError, OcrInput, OcrPage, OcrProvider};
use crate::raster::PageRasterizer;

/// How much one embeddings request carries. The gateway's own ceilings (`MAX_EMBEDDING_*`) stay
/// as the hard limit; these are what a request is sized to *in practice*, because the latency of
/// one call grows with its size and the call has a deadline.
///
/// Measured on the CPU-only reference stack (`docs/TROUBLESHOOTING.md`, embedding timeout): about
/// 0.7 s per 1 000-character chunk whatever the batch size, so 16 chunks take about 11 s. That
/// leaves the 120 s deadline ten times the batch, a retry that wastes seconds rather than minutes,
/// and a progress step every few seconds. The character cap is the same bound for chunks that
/// are longer than usual: 16 chunks of the 1 200-character ceiling are 19 200.
pub const EMBEDDING_BATCH_INPUTS: usize = 16;
pub const EMBEDDING_BATCH_CHARS: usize = 20_000;

/// This many files in a row failing to embed means the server is the problem, not the files:
/// the pass stops with the last error instead of spending two deadlines on every file left.
pub const MAX_CONSECUTIVE_FAILED_FILES: usize = 3;

/// The OCR engine did not start. Every scan reads as unreadable, however clear it is.
pub const CAPABILITY_OCR_ENGINE: &str = "ocrEngine";

/// The PDF page rasteriser did not start. A scanned PDF cannot reach OCR at all; a JPEG or PNG
/// still can, which is what made the failure in `docs/TROUBLESHOOTING.md` look like a bad
/// document rather than a missing capability.
pub const CAPABILITY_PAGE_RASTERIZER: &str = "pageRasterizer";

/// A file whose vectors could not all be made. Never written to the index, so it is neither
/// half-analysed nor skipped as unchanged next time. A machine code, not a sentence: the
/// interface localises it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedFile {
    pub path: String,
    pub code: String,
    /// What the sentence for `code` interpolates (a status, say). Never document text.
    pub data: serde_json::Value,
}

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
    /// Files renamed to a clean name before this pass read anything (`filename_sanitizer`), old
    /// and new. Filled by the caller that ran the rename, so a pass that fails halfway still
    /// leaves the rename recorded elsewhere; empty here.
    pub renamed_files: Vec<crate::filename_sanitizer::Renamed>,
    /// Files that could not be embedded, with the reason as a machine code. The pass went on
    /// without them; the next pass tries them again.
    pub failed_files: Vec<FailedFile>,
    /// Files that needed a clean name and could not be renamed. Left as they were.
    pub rename_failed_files: Vec<String>,
    /// Machine codes for an ingestion capability that did not start for this pass, in a stable
    /// order. Empty on a healthy installation. Reported so an unreadable scan can be explained
    /// by the interface instead of being blamed on the document: the engine being absent and the
    /// document being illegible used to be the same silent outcome
    /// (`docs/TROUBLESHOOTING.md`). English codes; the interface localises them
    /// (`docs/LANGUAGE-AND-LOCALE.md`).
    pub unavailable_capabilities: Vec<&'static str>,
    pub chunk_count: u64,
    /// Where the pass spent its time: numbers only (`knowledge::diagnostics`). Always measured,
    /// because it costs a few clock reads; written to `analysis-timings.jsonl` only when
    /// `write_timing_log` is on.
    pub timings: Option<AnalysisTimings>,
}

/// How far one pass has got, sent while it runs so a long analysis shows its progress rather than
/// an unbroken spinner. Counts only: no document text and not even a file name travels here.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    /// Files already dealt with, whether indexed, skipped as unchanged, or unreadable.
    pub processed_files: usize,
    pub total_files: usize,
    /// Inside the file being embedded: batches done, and batches in all. Both 0 when the file is
    /// not being embedded (read, skipped, or a pass over spreadsheets), so a bar can place itself
    /// between two files instead of standing still for the minutes one long document takes.
    pub batch_index: usize,
    pub batch_total: usize,
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
    let pass_timer = StageTimer::start();
    // The engine itself is timed from inside, so the time it takes is not counted as extraction.
    let timed_ocr = ocr.map(TimedOcr::new);
    let ocr: Option<&dyn OcrProvider> = timed_ocr.as_ref().map(|engine| engine as &dyn OcrProvider);
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
    let mut timings = AnalysisTimings::new(AnalysisPass::Documents, total_files);
    // The file being worked on: its stages are written into `file_timing` as they finish, and it
    // is closed at the top of the next iteration, so every early `continue` below still reports.
    let mut file_timing = FileTiming::default();
    let mut file_started = Instant::now();
    let mut file_active = false;
    // Sent before any work, so the bar appears at zero rather than only once the first file is
    // done - on a folder of scans that first file can take a while on its own.
    on_progress(IndexProgress {
        processed_files: 0,
        total_files,
        batch_index: 0,
        batch_total: 0,
    });
    let mut indexed_files = 0usize;
    let mut unchanged_files = 0usize;
    let mut empty_files = Vec::new();
    let mut ocr_files = Vec::new();
    let mut low_confidence_files = Vec::new();
    let mut failed_files: Vec<FailedFile> = Vec::new();
    let mut consecutive_failures = 0usize;
    // The model may still have to be loaded for the first request of a pass: it gets the longer
    // deadline until one request has succeeded.
    let mut model_warm = false;

    for (position, file) in files.iter().enumerate() {
        finish_file(
            &mut timings,
            &mut file_active,
            &mut file_timing,
            file_started,
        );
        file_started = Instant::now();
        // Reported as this file starts rather than as it ends, because several branches below
        // `continue`, and a count some paths forget to advance is worse than one that is a
        // single file behind. The final call after the loop closes the gap.
        on_progress(IndexProgress {
            processed_files: position,
            total_files,
            batch_index: 0,
            batch_total: 0,
        });
        let (hashed, hash_ms) = timed_ms(|| hash_file(file));
        timings.hash_ms += hash_ms;
        let sha256 = match hashed {
            Ok(hash) => hash,
            Err(_) => continue,
        };
        if should_skip(index, file, &sha256, ocr)? {
            unchanged_files += 1;
            continue;
        }
        timings.files_processed += 1;
        file_timing = FileTiming {
            position,
            ..FileTiming::default()
        };
        file_active = true;

        let ocr_before = timed_ocr.as_ref().map_or(0, TimedOcr::spent_ms);
        let (extracted, extraction_ms) =
            timed_ms(|| extraction::extract(file, ocr, rasterizer, locale));
        let ocr_ms = timed_ocr.as_ref().map_or(0, TimedOcr::spent_ms) - ocr_before;
        file_timing.ocr_ms = ocr_ms;
        file_timing.extract_ms = extraction_ms.saturating_sub(ocr_ms);
        timings.ocr_ms += file_timing.ocr_ms;
        timings.extract_ms += file_timing.extract_ms;

        match extracted {
            Ok(document) if document.empty => {
                let (engine, version) = ocr_identity(ocr, document.used_ocr);
                let (written, write_ms) = timed_ms(|| {
                    index.replace_document(
                        &file.relative_path,
                        &sha256,
                        true,
                        &[],
                        &[],
                        engine,
                        version,
                    )
                });
                written?;
                file_timing.write_ms += write_ms;
                timings.write_ms += write_ms;
                empty_files.push(file.relative_path.clone());
                if document.low_confidence {
                    low_confidence_files.push(file.relative_path.clone());
                }
            }
            Ok(document) => {
                if document.low_confidence {
                    low_confidence_files.push(file.relative_path.clone());
                }
                let (chunks, chunk_ms) = timed_ms(|| chunking::chunk(&document));
                file_timing.chunk_ms = chunk_ms;
                file_timing.chunks = chunks.len();
                timings.chunk_ms += chunk_ms;
                for chunk in &chunks {
                    timings.note_chunk_text(&chunk.text);
                }
                if chunks.is_empty() {
                    let (engine, version) = ocr_identity(ocr, document.used_ocr);
                    let (written, write_ms) = timed_ms(|| {
                        index.replace_document(
                            &file.relative_path,
                            &sha256,
                            true,
                            &[],
                            &[],
                            engine,
                            version,
                        )
                    });
                    written?;
                    file_timing.write_ms += write_ms;
                    timings.write_ms += write_ms;
                    empty_files.push(file.relative_path.clone());
                } else {
                    // Nothing is written until every vector exists: a file that fails halfway
                    // leaves no trace in the index, so it is never mistaken for an analysed one.
                    let embed_started = Instant::now();
                    let embedded = embed_chunks(
                        gateway,
                        server_url,
                        embedding_alias,
                        &chunks,
                        &mut model_warm,
                        &|batch_index, batch_total| {
                            on_progress(IndexProgress {
                                processed_files: position,
                                total_files,
                                batch_index,
                                batch_total,
                            })
                        },
                    )
                    .await;
                    // Counted whether or not it worked: a request that ran into its deadline is
                    // exactly the time worth knowing about.
                    file_timing.embed_ms = elapsed_ms(embed_started.elapsed());
                    timings.embed_ms += file_timing.embed_ms;
                    let embeddings = match embedded {
                        Ok(vectors) => {
                            consecutive_failures = 0;
                            timings.embed_batches += plan_batches(&chunks).len();
                            timings.embedded_chars += chunks
                                .iter()
                                .map(|chunk| chunk.text.chars().count())
                                .sum::<usize>();
                            vectors
                        }
                        Err(error) if is_about_this_file(&error) => {
                            consecutive_failures += 1;
                            if consecutive_failures >= MAX_CONSECUTIVE_FAILED_FILES {
                                return Err(error);
                            }
                            failed_files.push(FailedFile {
                                path: file.relative_path.clone(),
                                code: error.code().to_string(),
                                data: error.data(),
                            });
                            continue;
                        }
                        Err(error) => return Err(error),
                    };
                    let (engine, version) = ocr_identity(ocr, document.used_ocr);
                    let (written, write_ms) = timed_ms(|| {
                        index.replace_document(
                            &file.relative_path,
                            &sha256,
                            false,
                            &chunks,
                            &embeddings,
                            engine,
                            version,
                        )
                    });
                    written?;
                    file_timing.write_ms += write_ms;
                    timings.write_ms += write_ms;
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

    finish_file(
        &mut timings,
        &mut file_active,
        &mut file_timing,
        file_started,
    );
    on_progress(IndexProgress {
        processed_files: total_files,
        total_files,
        batch_index: 0,
        batch_total: 0,
    });
    timings.total_ms = pass_timer.total_ms();

    Ok(IndexSummary {
        scanned_files: total_files,
        indexed_files,
        unchanged_files,
        empty_files,
        ocr_files,
        low_confidence_files,
        removed_files,
        renamed_files: Vec::new(),
        rename_failed_files: Vec::new(),
        failed_files,
        unavailable_capabilities,
        chunk_count: index.chunk_count()?,
        timings: Some(timings),
    })
}

/// Close the file being worked on: its total, then into the slowest-files list.
fn finish_file(
    timings: &mut AnalysisTimings,
    active: &mut bool,
    timing: &mut FileTiming,
    started: Instant,
) {
    if *active {
        timing.total_ms = elapsed_ms(started.elapsed());
        timings.note_file(*timing);
        *active = false;
    }
}

/// An OCR engine that adds up the time it spends, so the pass can tell reading a scan from
/// extracting a text layer. It decorates the real port and changes nothing else: same identity,
/// same version, same answers.
struct TimedOcr<'a> {
    inner: &'a dyn OcrProvider,
    nanoseconds: AtomicU64,
}

impl<'a> TimedOcr<'a> {
    fn new(inner: &'a dyn OcrProvider) -> Self {
        Self {
            inner,
            nanoseconds: AtomicU64::new(0),
        }
    }

    fn spent_ms(&self) -> u64 {
        self.nanoseconds.load(Ordering::Relaxed) / 1_000_000
    }
}

impl OcrProvider for TimedOcr<'_> {
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
        let started = Instant::now();
        let page = self.inner.recognise(input);
        let spent = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.nanoseconds.fetch_add(spent, Ordering::Relaxed);
        page
    }
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

/// Whether an embeddings failure belongs to this file rather than to the server: a deadline that
/// passed twice, or a gateway that answered with an error of its own making. Anything else - the
/// server unreachable, the model not there, a refused request - would fail the next file the same
/// way, so the pass stops instead of reporting every remaining file as broken.
fn is_about_this_file(error: &AppError) -> bool {
    match error {
        AppError::EmbeddingTimeout | AppError::ServerError { .. } => true,
        AppError::Gateway { code, .. } => code == "provider_error",
        _ => false,
    }
}

/// Cut the chunks into the requests that will carry them: at most `EMBEDDING_BATCH_INPUTS`
/// chunks and `EMBEDDING_BATCH_CHARS` characters each, in order, none empty. A chunk longer than
/// the character cap still travels, alone.
fn plan_batches(chunks: &[chunking::Chunk]) -> Vec<std::ops::Range<usize>> {
    let mut batches = Vec::new();
    let mut start = 0usize;
    let mut chars = 0usize;
    for (position, chunk) in chunks.iter().enumerate() {
        let chunk_chars = chunk.text.chars().count();
        let in_batch = position - start;
        if in_batch > 0
            && (in_batch + 1 > EMBEDDING_BATCH_INPUTS
                || chars + chunk_chars > EMBEDDING_BATCH_CHARS)
        {
            batches.push(start..position);
            start = position;
            chars = 0;
        }
        chars += chunk_chars;
    }
    if start < chunks.len() {
        batches.push(start..chunks.len());
    }
    batches
}

/// Send chunk text to the gateway in small batches (`docs/RETRIEVAL.md`): the latency of one
/// request grows with its size and the request has a deadline, so a long document is many short
/// calls rather than one that can run past it. The vectors come back in chunk order.
///
/// `on_batch` receives (batches done, batches in all): once before the first, then after each.
async fn embed_chunks(
    gateway: &GatewayClient,
    server_url: &str,
    embedding_alias: &str,
    chunks: &[chunking::Chunk],
    model_warm: &mut bool,
    on_batch: &(dyn Fn(usize, usize) + Sync),
) -> Result<Vec<Vec<f32>>, AppError> {
    let batches = plan_batches(chunks);
    let deadlines = gateway.embedding_deadlines();
    let mut vectors = Vec::with_capacity(chunks.len());

    on_batch(0, batches.len());
    for (done, range) in batches.iter().enumerate() {
        let texts: Vec<String> = chunks[range.clone()]
            .iter()
            .map(|chunk| chunk.text.clone())
            .collect();
        let deadline = if *model_warm {
            deadlines.batch
        } else {
            deadlines.first_batch
        };
        vectors.extend(
            gateway
                .embed_batch(server_url, embedding_alias, &texts, deadline)
                .await?,
        );
        *model_warm = true;
        on_batch(done + 1, batches.len());
    }

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

    fn chunk_of(chars: usize) -> chunking::Chunk {
        chunking::Chunk {
            chunk_id: "a.txt#p1#s1".into(),
            relative_path: "a.txt".into(),
            page_number: 1,
            section: 1,
            text: "x".repeat(chars),
            origin: PageOrigin::TextLayer,
            confidence: None,
        }
    }

    #[test]
    fn short_chunks_are_cut_by_count() {
        let chunks: Vec<_> = (0..40).map(|_| chunk_of(100)).collect();

        let batches = plan_batches(&chunks);

        assert_eq!(batches, vec![0..16, 16..32, 32..40]);
    }

    #[test]
    fn long_chunks_are_cut_by_characters_before_the_count_is_reached() {
        let chunks: Vec<_> = (0..10).map(|_| chunk_of(6_000)).collect();

        let batches = plan_batches(&chunks);

        assert!(batches.iter().all(|range| range.len() <= 3));
        assert_eq!(batches.iter().map(|range| range.len()).sum::<usize>(), 10);
    }

    #[test]
    fn a_chunk_over_the_character_cap_travels_alone_rather_than_being_dropped() {
        let chunks = vec![
            chunk_of(100),
            chunk_of(EMBEDDING_BATCH_CHARS + 1),
            chunk_of(100),
        ];

        let batches = plan_batches(&chunks);

        assert_eq!(batches, vec![0..1, 1..2, 2..3]);
    }

    #[test]
    fn no_chunks_means_no_request() {
        assert!(plan_batches(&[]).is_empty());
    }

    #[test]
    fn only_failures_about_the_file_let_the_pass_go_on() {
        assert!(is_about_this_file(&AppError::EmbeddingTimeout));
        assert!(is_about_this_file(&AppError::ServerError { status: 500 }));
        assert!(is_about_this_file(&AppError::Gateway {
            code: "provider_error".into(),
            data: serde_json::json!({}),
        }));

        assert!(!is_about_this_file(&AppError::ServerUnreachable {
            url: "http://example.test".into(),
        }));
        assert!(!is_about_this_file(&AppError::Gateway {
            code: "provider_unreachable".into(),
            data: serde_json::json!({}),
        }));
        assert!(!is_about_this_file(&AppError::Gateway {
            code: "model_alias_not_allowed".into(),
            data: serde_json::json!({}),
        }));
        assert!(!is_about_this_file(&AppError::ServerTimeout {
            url: "http://example.test".into(),
        }));
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
