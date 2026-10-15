//! Orchestrates the chain: discovery -> extraction -> chunking -> embeddings -> local index.
//!
//! Kept out of `commands.rs` so the pipeline can be exercised without Tauri, and so no piece of
//! it depends on the webview being present.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::chunking;
use crate::chunking_long;
use crate::cleaning;
use crate::discovery::{self, DiscoveredFile};
use crate::error::AppError;
use crate::extraction::{self, ExtractionError, PageOrigin};
use crate::gateway::GatewayClient;
use crate::index_store::{self, IndexStore};
use crate::knowledge::backfill;
use crate::knowledge::diagnostics::{
    elapsed_ms, timed_ms, AnalysisPass, AnalysisTimings, FileTiming, StageTimer,
};
use crate::knowledge::ingest::KnowledgeContext;
use crate::knowledge::KnowledgeSummary;
use crate::ocr::{ImageFormat, OcrError, OcrInput, OcrPage, OcrProvider};
use crate::ocr_resume;
use crate::raster::PageRasterizer;
use crate::structure;

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
    /// Files of the folder that no reader of this product opens, counted by extension, most
    /// numerous first: the summary says how many were not read and why (their format), instead of
    /// letting a library with a tenth of its files in a format nobody reads answer from the rest
    /// without a word (defect KBD-30). A count and an extension, never a name.
    pub unread_formats: Vec<UnreadFormat>,
    /// What the pass learned about names, as counts. `None` when the knowledge base took no part
    /// (`knowledge_mode = off`, or a pass that never had a context) and for the Data Folder's pass
    /// until lot 5. Never a name.
    pub knowledge: Option<KnowledgeSummary>,
    /// What the file profile pass did, as counts (`file_profile::store`). `None` when it could not
    /// start; it never blocks a pass.
    pub profile: Option<crate::file_profile::store::ProfileSummary>,
    /// The user stopped the pass. Everything written stays; the next pass goes on from there.
    pub cancelled: bool,
    /// Passages that still have no vector at the end of the pass (stopped, or refused by the server).
    /// They are found by their words; the next pass computes their vectors.
    pub vectors_pending: usize,
}

/// How many files of one format the pass could not read. The format is the extension, lowercase and
/// without the dot; the interface says that these formats are not read yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnreadFormat {
    pub extension: String,
    pub count: usize,
}

/// Files the operating system keeps in a folder for its own use: not documents, never reported.
const OPERATING_SYSTEM_FILES: [&str; 2] = ["thumbs.db", "desktop.ini"];

/// The formats of `everything` (the walk of the whole folder) that no reader opens: a known or
/// unknown extension that is neither a document the pipeline reads nor a workbook, which belongs
/// to the Data Folder. Counted by extension, most numerous first, then by extension.
fn unread_formats(everything: &[DiscoveredFile]) -> Vec<UnreadFormat> {
    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for file in everything {
        let extension = file.extension.to_lowercase();
        let name = file
            .relative_path
            .rsplit('/')
            .next()
            .unwrap_or(&file.relative_path)
            .to_lowercase();
        let unread = crate::file_record::FileKind::from_extension(&extension)
            == crate::file_record::FileKind::Unsupported
            && !OPERATING_SYSTEM_FILES.contains(&name.as_str());
        if unread {
            *counts.entry(extension).or_insert(0) += 1;
        }
    }
    let mut formats: Vec<UnreadFormat> = counts
        .into_iter()
        .map(|(extension, count)| UnreadFormat { extension, count })
        .collect();
    formats.sort_by(|a, b| b.count.cmp(&a.count).then(a.extension.cmp(&b.extension)));
    formats
}

/// How far one pass has got, sent while it runs so a long analysis shows its progress rather than
/// an unbroken spinner. Counts only: no document text and not even a file name travels here.
#[derive(Debug, Clone, Copy, Default, Serialize)]
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
    /// The pass has read every file and is now reading the names in documents analysed earlier
    /// (`knowledge::backfill`): `processed_files` and `total_files` then count those documents. The
    /// interface says so instead of showing a bar that seems to start again.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reading_names: bool,
    /// Phase 2: passages that have their vector, and passages in all. Both 0 while the files are being
    /// read (phase 1). The interface shows "meaning search: x %" from them.
    #[serde(skip_serializing_if = "is_zero")]
    pub vectors_done: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub vectors_total: usize,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

/// One pass over the work folder. Files whose content hash has not changed since the last pass
/// are skipped rather than re-extracted and re-embedded, unless the OCR engine that produced
/// their stored chunks is no longer the one configured.
///
/// The pass as it was before the knowledge base: no names are read. Kept with its exact signature
/// because the tests and the callers that predate lot 4 use it; the application calls
/// [`run_with_knowledge`].
pub async fn run(
    index: &mut IndexStore,
    gateway: &GatewayClient,
    server_url: &str,
    embedding_alias: &str,
    work_folder: &std::path::Path,
    ocr: Option<&dyn OcrProvider>,
    rasterizer: Option<&dyn PageRasterizer>,
    locale: &str,
    on_progress: &(dyn Fn(IndexProgress) + Sync),
) -> Result<IndexSummary, AppError> {
    run_with_knowledge(
        index,
        gateway,
        server_url,
        embedding_alias,
        work_folder,
        ocr,
        rasterizer,
        locale,
        on_progress,
        None,
    )
    .await
}

/// Write one document: through the knowledge context when there is one (names learned from the
/// chunks, in the same transaction), as the pass has always done when there is not.
#[allow(clippy::too_many_arguments)]
fn write_document(
    index: &mut IndexStore,
    knowledge: &mut Option<&mut KnowledgeContext>,
    relative_path: &str,
    sha256: &str,
    empty: bool,
    chunks: &[crate::chunking::Chunk],
    embeddings: &[Vec<f32>],
    ocr_engine: Option<&str>,
    ocr_engine_version: Option<&str>,
) -> Result<(), AppError> {
    match knowledge {
        Some(context) => context.store_document(
            index,
            relative_path,
            sha256,
            empty,
            chunks,
            embeddings,
            ocr_engine,
            ocr_engine_version,
        ),
        None => index.replace_document(
            relative_path,
            sha256,
            empty,
            chunks,
            embeddings,
            ocr_engine,
            ocr_engine_version,
        ),
    }
}

/// Write the text of one document whose vectors may still be missing (`None`): through the knowledge
/// context when there is one, as `write_document` does.
#[allow(clippy::too_many_arguments)]
fn write_document_pending(
    index: &mut IndexStore,
    knowledge: &mut Option<&mut KnowledgeContext>,
    relative_path: &str,
    sha256: &str,
    chunks: &[crate::chunking::Chunk],
    vectors: &[Option<Vec<f32>>],
    ocr_engine: Option<&str>,
    ocr_engine_version: Option<&str>,
) -> Result<(), AppError> {
    match knowledge {
        Some(context) => context.store_document_pending(
            index,
            relative_path,
            sha256,
            false,
            chunks,
            vectors,
            ocr_engine,
            ocr_engine_version,
        ),
        None => index.replace_document_pending(
            relative_path,
            sha256,
            false,
            chunks,
            vectors,
            ocr_engine,
            ocr_engine_version,
        ),
    }
}

/// Run blocking work (parsing a PDF, recognising a scan) without holding the executor thread: on the
/// application's multi-threaded runtime it moves off the worker; elsewhere (a current-thread test
/// runtime) it simply runs.
fn run_blocking<T>(work: impl FnOnce() -> T) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(work)
        }
        _ => work(),
    }
}

/// [`run`], with the knowledge base taking part, in **two speeds**.
///
/// Phase 1, file by file: read, clean, cut, and write the text, the word index, the structure and the
/// names, with the vector of a passage only when it is already known (an unchanged passage keeps its
/// stored one). When phase 1 is over for every file, the names of documents analysed earlier are read
/// (`knowledge::backfill`) and the file profiles are built: the word search, the finder and the entity
/// base work on everything from that moment on.
///
/// Phase 2, batch by batch: the passages that have no vector, in reading order, 16 at a time and 20 000
/// characters at most, each batch stored in its own transaction. A pass that stops, by `cancelled` or by
/// an error, keeps everything written; the next pass reads only what changed and computes only the
/// vectors that are missing.
///
/// A file that cannot be read, or whose passages the server keeps refusing, is reported by name and the
/// pass goes on with the others (`FailedFile`); the pass stops only when the server itself is gone
/// (three files in a row), and then too with everything kept.
///
/// `knowledge: None` is the pass as it was before the knowledge base. A context whose mode is
/// `Off` writes the documents without touching a `kb_*` row.
#[allow(clippy::too_many_arguments)]
pub async fn run_with_knowledge(
    index: &mut IndexStore,
    gateway: &GatewayClient,
    server_url: &str,
    embedding_alias: &str,
    work_folder: &std::path::Path,
    ocr: Option<&dyn OcrProvider>,
    rasterizer: Option<&dyn PageRasterizer>,
    locale: &str,
    on_progress: &(dyn Fn(IndexProgress) + Sync),
    knowledge: Option<&mut KnowledgeContext>,
) -> Result<IndexSummary, AppError> {
    run_cancellable(
        index,
        gateway,
        server_url,
        embedding_alias,
        work_folder,
        ocr,
        rasterizer,
        locale,
        on_progress,
        knowledge,
        &|| false,
    )
    .await
}

/// [`run_with_knowledge`] that stops, between two files or two batches, when `cancelled()` is true. The
/// summary then says `cancelled`; what was written stays.
#[allow(clippy::too_many_arguments)]
pub async fn run_cancellable(
    index: &mut IndexStore,
    gateway: &GatewayClient,
    server_url: &str,
    embedding_alias: &str,
    work_folder: &std::path::Path,
    ocr: Option<&dyn OcrProvider>,
    rasterizer: Option<&dyn PageRasterizer>,
    locale: &str,
    // `on_progress` is called before the first file, as each file starts, after each batch of vectors
    // and once at the end. `Sync` rather than a plain closure so the future stays `Send` and can be
    // awaited from a Tauri command.
    on_progress: &(dyn Fn(IndexProgress) + Sync),
    mut knowledge: Option<&mut KnowledgeContext>,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<IndexSummary, AppError> {
    // The Analyse is the moment an index from before is brought to the layout where a vector may be
    // absent: passages and vectors are copied as they are.
    index.upgrade_layout()?;
    // The tables of the word search (stems, trigrams) are made to match the switches that are on, from the
    // text already stored: seconds, no file read and no vector computed. Nothing is done while they match.
    index.ensure_lexical_layout()?;
    // Vectors of two models cannot be compared. An index made with another model than the one asked for
    // now has its vectors computed again, here, in the pass the user started (nothing at launch).
    if let Some((model, _)) = index.embedding_info()? {
        if model != embedding_alias {
            index.reset_vectors()?;
            index.set_embedding_info(embedding_alias, 0)?;
        }
    }
    if let Some(context) = knowledge.as_mut() {
        if context.mode.reads_files() {
            context.begin_pass(index.connection());
        }
    }
    let unavailable_capabilities = unavailable_capabilities(ocr, rasterizer);
    let pass_timer = StageTimer::start();
    // The engine itself is timed from inside, so the time it takes is not counted as extraction.
    let timed_ocr = ocr.map(TimedOcr::new);
    let ocr: Option<&dyn OcrProvider> = timed_ocr.as_ref().map(|engine| engine as &dyn OcrProvider);
    let everything = discovery::discover_all(work_folder);
    let unread_formats = unread_formats(&everything);
    let files: Vec<DiscoveredFile> = everything
        .into_iter()
        .filter(|file| discovery::is_supported(&file.extension))
        .collect();
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
        ..IndexProgress::default()
    });
    let mut indexed_files = 0usize;
    let mut unchanged_files = 0usize;
    let mut empty_files = Vec::new();
    let mut ocr_files = Vec::new();
    let mut low_confidence_files = Vec::new();
    let mut failed_files: Vec<FailedFile> = Vec::new();
    let mut was_cancelled = false;

    // ---------------------------------------------------------------- phase 1: the text of every file
    for (position, file) in files.iter().enumerate() {
        finish_file(
            &mut timings,
            &mut file_active,
            &mut file_timing,
            file_started,
        );
        if cancelled() {
            was_cancelled = true;
            break;
        }
        file_started = Instant::now();
        // Reported as this file starts rather than as it ends, because several branches below
        // `continue`, and a count some paths forget to advance is worse than one that is a
        // single file behind. The final call after the loop closes the gap.
        on_progress(IndexProgress {
            processed_files: position,
            total_files,
            ..IndexProgress::default()
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

        // A scan is read page by page and each recognised page is kept as it is recognised, so a stop
        // or a failure in the middle of a thousand pages loses none of them.
        let page_cache = (ocr.is_some()
            && matches!(file.extension.as_str(), "pdf" | "jpg" | "jpeg" | "png"))
        .then(|| ocr_resume::PageCache::open(index.path()))
        .flatten();
        if let Some(cache) = &page_cache {
            cache.forget_other_versions(&file.relative_path, &sha256);
        }
        let cached_ocr = page_cache
            .as_ref()
            .zip(ocr)
            .map(|(cache, inner)| ocr_resume::CachedOcr {
                inner,
                cache,
                sha256: &sha256,
            });
        let cached_rasterizer = page_cache.as_ref().zip(rasterizer).map(|(cache, inner)| {
            ocr_resume::CachedRasterizer {
                inner,
                cache,
                relative_path: &file.relative_path,
                sha256: &sha256,
                cancelled,
            }
        });
        let file_ocr: Option<&dyn OcrProvider> = match &cached_ocr {
            Some(cached) => Some(cached as &dyn OcrProvider),
            None => ocr,
        };
        let file_rasterizer: Option<&dyn PageRasterizer> = match &cached_rasterizer {
            Some(cached) => Some(cached as &dyn PageRasterizer),
            None => rasterizer,
        };
        let ocr_before = timed_ocr.as_ref().map_or(0, TimedOcr::spent_ms);
        let (extracted, extraction_ms) = timed_ms(|| {
            run_blocking(|| extraction::extract(file, file_ocr, file_rasterizer, locale))
        });
        let ocr_ms = timed_ocr.as_ref().map_or(0, TimedOcr::spent_ms) - ocr_before;
        file_timing.ocr_ms = ocr_ms;
        file_timing.extract_ms = extraction_ms.saturating_sub(ocr_ms);
        timings.ocr_ms += file_timing.ocr_ms;
        timings.extract_ms += file_timing.extract_ms;
        if cancelled() {
            // What was just read is dropped: the file is read again by the next pass.
            was_cancelled = true;
            break;
        }

        match extracted {
            Ok(document) if document.empty => {
                let (engine, version) = ocr_identity(ocr, document.used_ocr);
                let (written, write_ms) = timed_ms(|| {
                    write_document(
                        index,
                        &mut knowledge,
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
                index.replace_structure(
                    &file.relative_path,
                    structure::STRUCTURE_VERSION,
                    chunking_long::CHUNKER_SHORT,
                    &structure::Structure::default(),
                )?;
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
                // A long document is cleaned (repeated headers, cut words, odd characters) before it
                // is cut into passages; a short one comes back unchanged. Counted with the chunking.
                //
                // Whether it is long is decided on the text as extracted, before the cleaning removes
                // anything from it. A long one is cut by the chunker for long documents (headings,
                // tables and lists whole, optional overlap and prefix); a short one by today's.
                let long = chunking::is_long(&document);
                let (document, clean_ms) = timed_ms(|| cleaning::clean(document));
                let ((chunks, built), chunk_ms) = timed_ms(|| {
                    let sections = structure::sections_for(
                        std::path::Path::new(&file.absolute_path),
                        &file.extension,
                        &document,
                    );
                    if long {
                        let cut = chunking_long::chunk_long(
                            &document,
                            &sections,
                            &chunking_long::long_options(),
                        );
                        (
                            cut.chunks,
                            structure::Structure {
                                sections,
                                chunk_sections: cut.owners,
                                chunk_ends: cut.ends,
                            },
                        )
                    } else {
                        let chunks = chunking::chunk(&document);
                        let chunk_sections = structure::assign(&sections, &document, &chunks);
                        (
                            chunks,
                            structure::Structure {
                                sections,
                                chunk_sections,
                                chunk_ends: Vec::new(),
                            },
                        )
                    }
                });
                let chunk_ms = chunk_ms + clean_ms;
                let chunker = if long {
                    chunking_long::CHUNKER_LONG
                } else {
                    chunking_long::CHUNKER_SHORT
                };
                file_timing.chunk_ms = chunk_ms;
                file_timing.chunks = chunks.len();
                timings.chunk_ms += chunk_ms;
                for chunk in &chunks {
                    timings.note_chunk_text(&chunk.text);
                }
                let (engine, version) = ocr_identity(ocr, document.used_ocr);
                if chunks.is_empty() {
                    let (written, write_ms) = timed_ms(|| {
                        write_document(
                            index,
                            &mut knowledge,
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
                    index.replace_structure(
                        &file.relative_path,
                        structure::STRUCTURE_VERSION,
                        chunking_long::CHUNKER_SHORT,
                        &structure::Structure::default(),
                    )?;
                    file_timing.write_ms += write_ms;
                    timings.write_ms += write_ms;
                    empty_files.push(file.relative_path.clone());
                } else {
                    // A passage whose id and text are exactly those already stored for this file
                    // keeps its stored vector: a file read again for another reason (its structure
                    // changed version, one paragraph was edited) is not embedded again as a whole.
                    // The others are stored without a vector; phase 2 computes it.
                    let stored = index.chunks_for_document(&file.relative_path)?;
                    let stored_prefixes = index.index_prefixes_of(&file.relative_path)?;
                    let reusable: HashMap<&str, &index_store::StoredChunk> = stored
                        .iter()
                        .filter(|old| !old.embedding.is_empty())
                        .map(|old| (old.chunk_id.as_str(), old))
                        .collect();
                    let vectors: Vec<Option<Vec<f32>>> = chunks
                        .iter()
                        .map(|chunk| {
                            reusable
                                .get(chunk.chunk_id.as_str())
                                .filter(|old| {
                                    old.text == chunk.text
                                        && chunk.index_text
                                            == stored_prefixes
                                                .get(&chunk.chunk_id)
                                                .map(|prefix| format!("{prefix}\n{}", chunk.text))
                                })
                                .map(|old| old.embedding.clone())
                        })
                        .collect();
                    let (written, write_ms) = timed_ms(|| {
                        write_document_pending(
                            index,
                            &mut knowledge,
                            &file.relative_path,
                            &sha256,
                            &chunks,
                            &vectors,
                            engine,
                            version,
                        )
                    });
                    written?;
                    index.replace_structure(
                        &file.relative_path,
                        structure::STRUCTURE_VERSION,
                        chunker,
                        &built,
                    )?;
                    file_timing.write_ms += write_ms;
                    timings.write_ms += write_ms;
                    indexed_files += 1;
                    if chunks.iter().any(|chunk| chunk.origin == PageOrigin::Ocr) {
                        ocr_files.push(file.relative_path.clone());
                    }
                }
            }
            Err(ExtractionError::UnsupportedExtension) => continue,
            // A file that cannot be read does not stop the others: it is named, and the next pass
            // tries it again.
            Err(_) => {
                let error = AppError::ExtractionFailed {
                    path: file.relative_path.clone(),
                };
                failed_files.push(FailedFile {
                    path: file.relative_path.clone(),
                    code: error.code().to_string(),
                    data: error.data(),
                });
            }
        }
        // The document is stored: the pages kept for it are no longer needed.
        if let Some(cache) = &page_cache {
            if index.stored_hash(&file.relative_path)?.as_deref() == Some(sha256.as_str()) {
                cache.forget(&file.relative_path);
            }
        }
    }

    finish_file(
        &mut timings,
        &mut file_active,
        &mut file_timing,
        file_started,
    );

    // The names of documents analysed earlier and the profile of every document, from the text that is
    // stored now: local, no request. A pass stopped by the user does not spend time on them; the next
    // one does. Failing here costs nothing but a note in the summary.
    let mut knowledge_summary = None;
    let mut profile = None;
    if !was_cancelled {
        knowledge_summary = match knowledge.as_mut() {
            Some(context) if context.mode.reads_files() => {
                let announce = |progress: backfill::RefreshProgress| {
                    on_progress(IndexProgress {
                        processed_files: progress.done,
                        total_files: progress.total,
                        reading_names: true,
                        ..IndexProgress::default()
                    })
                };
                if backfill::refresh_documents(index, context, &|| false, &announce).is_err() {
                    context.note_failed("");
                }
                Some(context.summary(index.connection()))
            }
            _ => None,
        };
        profile = crate::file_profile::store::run_documents(
            index,
            work_folder,
            knowledge.as_deref(),
            locale,
        );
        // The card of each document, from the text, structure and profile just stored. A failure costs
        // the cards of this pass, never the pass.
        let _ = crate::cards::build_missing(index);
    }

    // ------------------------------------------------------------ phase 2: the vectors, batch by batch
    let mut model_warm = false;
    let mut consecutive_failures = 0usize;
    let mut refused: Vec<String> = Vec::new();
    let mut phase_two_error: Option<AppError> = None;
    let mut vectors_started = false;
    if !was_cancelled {
        let counts = index.vector_counts()?;
        let mut done = counts.done;
        let total = counts.total;
        if done < total {
            let deadlines = gateway.embedding_deadlines();
            let announce = |done: usize, total: usize| {
                on_progress(IndexProgress {
                    processed_files: total_files,
                    total_files,
                    vectors_done: done,
                    vectors_total: total,
                    ..IndexProgress::default()
                })
            };
            vectors_started = true;
            announce(done, total);
            loop {
                if cancelled() {
                    was_cancelled = true;
                    break;
                }
                let mut batch = index.pending_chunks_excluding(EMBEDDING_BATCH_INPUTS, &refused)?;
                if batch.is_empty() {
                    break;
                }
                let lengths: Vec<usize> = batch
                    .iter()
                    .map(|pending| pending.text.chars().count())
                    .collect();
                let keep = batch_len(&lengths);
                let characters: usize = lengths[..keep].iter().sum();
                batch.truncate(keep);
                let texts: Vec<String> = batch.iter().map(|pending| pending.text.clone()).collect();
                let deadline = if model_warm {
                    deadlines.batch
                } else {
                    deadlines.first_batch
                };
                let started = Instant::now();
                let embedded = gateway
                    .embed_batch(server_url, embedding_alias, &texts, deadline)
                    .await;
                timings.embed_ms += elapsed_ms(started.elapsed());
                match embedded {
                    Ok(vectors) => {
                        if !model_warm {
                            // The first vectors of the pass: they say what the model returns now. If
                            // the stored ones have another length, the model behind the name changed.
                            let dimension = vectors.first().map_or(0, Vec::len);
                            if dimension > 0 {
                                if index
                                    .vector_dimension_in_use()?
                                    .is_some_and(|stored| stored != dimension)
                                {
                                    index.reset_vectors()?;
                                    done = index.vector_counts()?.done;
                                }
                                index.set_embedding_info(embedding_alias, dimension)?;
                            }
                        }
                        model_warm = true;
                        consecutive_failures = 0;
                        let stored: Vec<(String, Vec<f32>)> = batch
                            .iter()
                            .map(|pending| pending.chunk_id.clone())
                            .zip(vectors)
                            .collect();
                        index.store_vectors(&stored)?;
                        timings.embed_batches += 1;
                        timings.embedded_chars += characters;
                        done += stored.len();
                        announce(done, total);
                    }
                    Err(error) if is_about_this_file(&error) => {
                        // A batch mixes files, so the one that made it fail is not known: each file
                        // of the batch is tried on its own, and only the file the server keeps
                        // refusing is set aside. Its text stays in the index and is searchable by
                        // words; the next pass tries its vectors again.
                        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
                        for (position, pending) in batch.iter().enumerate() {
                            match groups
                                .iter_mut()
                                .find(|(path, _)| *path == pending.relative_path)
                            {
                                Some((_, members)) => members.push(position),
                                None => {
                                    groups.push((pending.relative_path.clone(), vec![position]))
                                }
                            }
                        }
                        let mut first_error = Some(error);
                        for (path, members) in groups {
                            let outcome = match first_error.take() {
                                // A batch of one file already failed as a whole: that is its answer.
                                Some(error) if batch.iter().all(|p| p.relative_path == path) => {
                                    Err(error)
                                }
                                _ => {
                                    let group: Vec<String> =
                                        members.iter().map(|m| texts[*m].clone()).collect();
                                    let started = Instant::now();
                                    let result = gateway
                                        .embed_batch(server_url, embedding_alias, &group, deadline)
                                        .await;
                                    timings.embed_ms += elapsed_ms(started.elapsed());
                                    result
                                }
                            };
                            match outcome {
                                Ok(vectors) => {
                                    consecutive_failures = 0;
                                    let stored: Vec<(String, Vec<f32>)> = members
                                        .iter()
                                        .map(|m| batch[*m].chunk_id.clone())
                                        .zip(vectors)
                                        .collect();
                                    index.store_vectors(&stored)?;
                                    timings.embed_batches += 1;
                                    timings.embedded_chars += members
                                        .iter()
                                        .map(|m| texts[*m].chars().count())
                                        .sum::<usize>();
                                    done += stored.len();
                                    announce(done, total);
                                }
                                Err(error) if is_about_this_file(&error) => {
                                    consecutive_failures += 1;
                                    failed_files.push(FailedFile {
                                        path: path.clone(),
                                        code: error.code().to_string(),
                                        data: error.data(),
                                    });
                                    refused.push(path);
                                    if consecutive_failures >= MAX_CONSECUTIVE_FAILED_FILES {
                                        phase_two_error = Some(error);
                                        break;
                                    }
                                }
                                Err(error) => {
                                    phase_two_error = Some(error);
                                    break;
                                }
                            }
                        }
                        if phase_two_error.is_some() {
                            break;
                        }
                    }
                    Err(error) => {
                        phase_two_error = Some(error);
                        break;
                    }
                }
            }
        }
    }

    let remaining = index.vector_counts()?;
    // The last report keeps the share of vectors the pass reached, so the bar never goes back.
    on_progress(IndexProgress {
        processed_files: total_files,
        total_files,
        vectors_done: if vectors_started { remaining.done } else { 0 },
        vectors_total: if vectors_started { remaining.total } else { 0 },
        ..IndexProgress::default()
    });
    timings.total_ms = pass_timer.total_ms();

    // A server that is gone stops the pass with an error, as it always did; everything written stays.
    if let Some(error) = phase_two_error {
        return Err(error);
    }

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
        unread_formats,
        chunk_count: index.chunk_count()?,
        timings: Some(timings),
        knowledge: knowledge_summary,
        profile,
        cancelled: was_cancelled,
        vectors_pending: remaining.total - remaining.done,
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

fn ocr_identity(ocr: Option<&dyn OcrProvider>, used_ocr: bool) -> (Option<&str>, Option<&str>) {
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
    if retry_empty {
        return Ok(false);
    }
    // A document whose structure was never read, or was read by an older detection, is read again;
    // its vectors are reused, so this costs the extraction and not the embedding.
    let structure_current = index
        .structure_version(&file.relative_path)?
        .is_some_and(|version| version >= structure::STRUCTURE_VERSION);
    if !structure_current {
        return Ok(false);
    }
    // A document stored with more text than the long threshold must have been cut by the chunker for
    // long documents; one cut by the old chunker is read again (a short one never is for this).
    let expected = if index.stored_chars(&file.relative_path)? > chunking::LONG_DOCUMENT_CHARS {
        chunking_long::CHUNKER_LONG
    } else {
        chunking_long::CHUNKER_SHORT
    };
    Ok(index.stored_chunker(&file.relative_path)?.unwrap_or(0) >= expected)
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

/// How many of the passages waiting, taken in order from the front, make one request: at most
/// `EMBEDDING_BATCH_INPUTS` of them and `EMBEDDING_BATCH_CHARS` characters (`docs/RETRIEVAL.md`: the
/// latency of a request grows with its size and the request has a deadline). A passage longer than
/// the character cap still travels, alone. `lengths` are the characters of each waiting passage.
fn batch_len(lengths: &[usize]) -> usize {
    let mut taken = 0usize;
    let mut chars = 0usize;
    for length in lengths {
        if taken > 0
            && (taken + 1 > EMBEDDING_BATCH_INPUTS || chars + length > EMBEDDING_BATCH_CHARS)
        {
            break;
        }
        chars += length;
        taken += 1;
    }
    taken
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
    fn short_passages_are_cut_by_count() {
        let lengths = vec![100usize; 40];
        assert_eq!(batch_len(&lengths), 16);
        assert_eq!(batch_len(&lengths[16..]), 16);
        assert_eq!(batch_len(&lengths[32..]), 8);
    }

    #[test]
    fn long_passages_are_cut_by_characters_before_the_count_is_reached() {
        let lengths = vec![6_000usize; 10];
        assert_eq!(batch_len(&lengths), 3);
    }

    #[test]
    fn a_passage_over_the_character_cap_travels_alone_rather_than_being_dropped() {
        let lengths = [EMBEDDING_BATCH_CHARS + 1, 100, 100];
        assert_eq!(batch_len(&lengths), 1);
        assert_eq!(batch_len(&lengths[1..]), 2);
    }

    #[test]
    fn nothing_waiting_means_no_request() {
        assert_eq!(batch_len(&[]), 0);
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
