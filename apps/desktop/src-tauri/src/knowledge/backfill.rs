//! Bringing documents that were analysed earlier up to date with the knowledge base, from the text
//! the index already stores.
//!
//! Three situations leave a document "due": it was analysed before the knowledge base existed (or
//! while extraction was off), the extractor of this build is newer than the one that read it, or
//! the list of names grew after it was read (a name learned from a later file, a manual alias). In
//! all three the answer is the same: read the stored chunks again and write what they say.
//!
//! **No request is ever made.** The text is in the index, the extractors and the resolver are
//! local, and nothing here holds a gateway client; a test points a gateway that fails on any
//! request at a refresh and checks that it was never reached. The original files are not opened
//! either, so a refresh works on a folder that has since moved.
//!
//! Safe to run at any time and from anywhere: each document is its own transaction, written only
//! if it is still the version that was read (`IndexStore::apply_knowledge_refresh`), and a document
//! that is up to date costs nothing.

use std::collections::BTreeSet;

use super::extract::ChunkInput;
use super::ingest::{stored_chunk_input, KnowledgeContext, MAX_REFRESH_ROUNDS};
use super::store;
use super::{Domain, KnowledgeDelta, SourceRef};
use crate::error::AppError;
use crate::index_store::{IndexStore, RefreshWrite};

/// How far a refresh has got, as counts. Sent while it runs so a long one shows progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshProgress {
    pub done: usize,
    pub total: usize,
}

/// What a refresh did. Counts are of distinct documents: a document read in two rounds is one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RefreshReport {
    /// Documents read again and written.
    pub refreshed: usize,
    /// Documents whose knowledge could not be written; they stay due.
    pub failed: usize,
    /// Documents that changed while they were being read; the newer version has its own pass.
    pub stale: usize,
    /// The caller asked to stop before every due document was done.
    pub stopped: bool,
}

/// Is any document of the index due? One query: what decides whether a lazy refresh is worth
/// starting at all.
pub fn anything_due(index: &IndexStore, context: &KnowledgeContext) -> Result<bool, AppError> {
    Ok(!store::documents_due(index.connection(), context.kb_version, context.epoch())?.is_empty())
}

/// Read every due document again, round after round while learning keeps teaching the gazetteer
/// something (at most [`MAX_REFRESH_ROUNDS`]). `should_stop` is asked between two documents; the
/// document in hand is always finished, so a stop leaves nothing half written.
pub fn refresh_documents(
    index: &mut IndexStore,
    context: &mut KnowledgeContext,
    should_stop: &dyn Fn() -> bool,
    on_progress: &dyn Fn(RefreshProgress),
) -> Result<RefreshReport, AppError> {
    let mut report = RefreshReport::default();
    if !context.mode.reads_files() {
        return Ok(report);
    }
    let mut refreshed: BTreeSet<String> = BTreeSet::new();
    let mut failed: BTreeSet<String> = BTreeSet::new();
    let mut stale: BTreeSet<String> = BTreeSet::new();

    'rounds: for round in 0..MAX_REFRESH_ROUNDS {
        context.settle_epoch(index.connection())?;
        let due = store::documents_due(index.connection(), context.kb_version, context.epoch())?;
        if due.is_empty() {
            break;
        }
        let total = due.len();
        // Only the first round is announced: a bar that starts again looks like a fault.
        let announced = round == 0;
        if announced {
            on_progress(RefreshProgress { done: 0, total });
        }

        for (position, document) in due.iter().enumerate() {
            if should_stop() {
                report.stopped = true;
                break 'rounds;
            }
            let source = SourceRef {
                domain: Domain::Documents,
                relative_path: document.relative_path.clone(),
                content_id: document.sha256.clone(),
            };
            let chunks: Vec<ChunkInput> = if document.empty {
                Vec::new()
            } else {
                index
                    .chunk_texts_for_document(&document.relative_path)?
                    .iter()
                    .map(stored_chunk_input)
                    .collect()
            };

            let built = context.build(index.connection(), source.clone(), chunks);
            let (delta, readable) = match built {
                Ok(built) => (built.delta, true),
                // Nothing could be read: record the document as not extracted, which keeps it due
                // and drops the knowledge of a version of the text that is gone.
                Err(_) => (KnowledgeDelta::empty(source), false),
            };
            match index.apply_knowledge_refresh(&delta)? {
                RefreshWrite::Applied(_) if readable => {
                    refreshed.insert(document.relative_path.clone());
                    context.note_refreshed(&document.relative_path);
                    context.learn(index.connection(), &delta);
                }
                RefreshWrite::Stale => {
                    stale.insert(document.relative_path.clone());
                }
                RefreshWrite::Applied(_) | RefreshWrite::Bypassed => {
                    failed.insert(document.relative_path.clone());
                    context.note_failed(&document.relative_path);
                }
            }
            if announced {
                on_progress(RefreshProgress {
                    done: position + 1,
                    total,
                });
            }
        }
    }
    // Learning in the last round may have moved the list: say so now, so the next pass finds the
    // documents behind it due instead of discovering it by luck.
    context.settle_epoch(index.connection())?;
    report.refreshed = refreshed.len();
    report.failed = failed.len();
    report.stale = stale.len();
    Ok(report)
}
