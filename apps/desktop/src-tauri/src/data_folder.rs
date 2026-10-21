//! The Data Folder as the interface shows it: which files are there, and which of them hold a
//! table that can be asked about.
//!
//! Two questions, asked separately and joined only here (`docs/DECISIONS.md`, "the two
//! inventories stay separate"). "What files exist" is `WorkFolderInventory`, unchanged and built
//! with no index at all, so no document row can ever be joined onto a workbook by a shared
//! relative path. "Which of them has a current tabular analysis" is
//! `IndexStore::all_tabular_inventories`. This module combines the two answers into one state per
//! file and nothing more: it teaches neither side about the other.
//!
//! Three states, the same three the Documents Folder listing shows (`docs/SESSION-DATA-05-TABULAR-UI.md`
//! section 5a): not analysed yet (orange), analysed with at least one sheet that looks like a real
//! table (green), analysed with nothing usable in it (red). A file in this folder that is not a
//! spreadsheet at all is red too: nothing here will ever read it as a table.
//!
//! Local parsing only. Nothing in this module calls the gateway or the network.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use serde::Serialize;

use crate::discovery;
use crate::error::AppError;
use crate::file_key::Root;
use crate::file_profile::build::Domain;
use crate::file_record::{FileKind, FileRecord, ProcessingStatus, Readability};
use crate::index_store::IndexStore;
use crate::indexing::{IndexProgress, IndexSummary};
use crate::inventory::{FileHashCache, WorkFolderInventory};
use crate::knowledge::backfill;
use crate::knowledge::diagnostics::{
    elapsed_ms, timed_ms, AnalysisPass, AnalysisTimings, FileTiming, StageTimer,
};
use crate::knowledge::ingest::KnowledgeContext;
use crate::tabular;
use crate::tabular::inventory::{TabularFormat, TabularInventory};

/// The three counts the card prints, in the same order and with the same meaning as the
/// Documents Folder card's: files, analysed (green), unreadable (red).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataFolderSummary {
    pub total_files: usize,
    pub analysed_files: usize,
    pub unreadable_files: usize,
    pub not_assessed_files: usize,
}

/// The Data Folder panel. Metadata only: no sheet, column or cell crosses the bridge here.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataFolderReport {
    pub root_identifier: String,
    pub summary: DataFolderSummary,
    pub files: Vec<FileRecord>,
}

/// The Data Folder with each file's tabular state joined on.
///
/// `files` carries `FileRecord`s whose `readability` and `processing_status` describe the tabular
/// analysis rather than the document one - the same fields, read by the same listing, so the
/// Documents Folder's dots and checkboxes work unchanged. `indexed` stays false: nothing here is in
/// the document index, and nothing may claim it is.
#[derive(Debug, Clone)]
pub struct DataFolder {
    files: WorkFolderInventory,
    /// The cached inventory of every **green** workbook, by relative path. Only a green one is
    /// ever answered from, so only a green one is kept.
    usable: BTreeMap<String, TabularInventory>,
}

impl DataFolder {
    /// The Data folder as a physical folder: every file of it, each joined to what its own family made
    /// of it (`WorkFolderInventory::discover_in`). A workbook is green, orange or red by the tabular
    /// analysis; a document sitting here is joined to the document index under its key
    /// (`file_key`), and never to a document of the Documents folder that shares its path.
    pub fn discover(
        root: &Path,
        index: Option<&IndexStore>,
        cache: &FileHashCache,
    ) -> Result<Self, AppError> {
        let (files, usable) = WorkFolderInventory::discover_in(Root::Data, root, index, cache)?;
        Ok(Self { files, usable })
    }

    /// Built from parts that were already joined: the tabular family across both folders
    /// (`family_view`).
    pub(crate) fn from_parts(
        files: WorkFolderInventory,
        usable: BTreeMap<String, TabularInventory>,
    ) -> Self {
        Self { files, usable }
    }

    /// The files, with their tabular state. A `WorkFolderInventory` so the file-reference
    /// resolver, which is already generic over any `FileKind`, is reused as-is.
    pub fn files(&self) -> &WorkFolderInventory {
        &self.files
    }

    /// The cached inventory of a green workbook; `None` for any other file.
    pub fn usable_inventory(&self, relative_path: &str) -> Option<&TabularInventory> {
        self.usable.get(relative_path)
    }

    pub fn report(&self) -> DataFolderReport {
        let files = self.files.all_files();
        let count = |status: ProcessingStatus| {
            files
                .iter()
                .filter(|file| file.processing_status == status)
                .count()
        };
        DataFolderReport {
            root_identifier: self.files.root_identifier(),
            summary: DataFolderSummary {
                total_files: files.len(),
                analysed_files: count(ProcessingStatus::Indexed),
                unreadable_files: count(ProcessingStatus::Failed),
                not_assessed_files: files
                    .iter()
                    .filter(|file| file.readability == Readability::NotAssessed)
                    .count(),
            },
            files: files.to_vec(),
        }
    }
}

/// One Analyse pass over the Data Folder: every CSV/XLS/XLSX/XLSM file is parsed and its
/// inventory cached, and inventories of files no longer present are dropped. No embedding, no
/// gateway call: local parsing only, so it finishes in a fraction of the Documents Folder's time.
///
/// Every workbook is rebuilt on every pass rather than skipped when unchanged: parsing is cheap,
/// and it means an inventory cached by an older build never outlives the next pass.
///
/// Reported in the Documents pass's own shape (`IndexSummary`) so the card writes both the same
/// way: `indexed_files` counts green workbooks, `empty_files` names the red ones, and
/// `removed_files` names what was forgotten. The renames are filled in by the caller, which ran
/// them before this pass read anything - exactly as for the Documents Folder.
pub fn analyse(
    root: &Path,
    index: &mut IndexStore,
    locale: &str,
    on_progress: &dyn Fn(IndexProgress),
) -> Result<IndexSummary, AppError> {
    analyse_with_knowledge(root, index, locale, on_progress, None)
}

/// [`analyse`], with the knowledge base taking part. After a workbook's inventory is built, its
/// columns are read for what they mean and its values for the names, organisations and identifiers
/// they hold; the inventory and that knowledge are written in one transaction
/// (`IndexStore::put_tabular_inventory_with_knowledge`). A workbook with no usable sheet, or a pass
/// with extraction off, writes no knowledge and drops what an earlier version taught. The engine's
/// inventory is the one `analyse` has always built: the knowledge never alters it.
///
/// When the workbooks taught the base names the documents were not read against, the documents are
/// read again from the text the index already stores, with no request to the gateway
/// (`knowledge::backfill`), and the progress says so. `knowledge: None` is the pass as it was.
pub fn analyse_with_knowledge(
    root: &Path,
    index: &mut IndexStore,
    locale: &str,
    on_progress: &dyn Fn(IndexProgress),
    knowledge: Option<&mut KnowledgeContext>,
) -> Result<IndexSummary, AppError> {
    analyse_cancellable(root, None, index, locale, on_progress, knowledge, &|| false)
}

/// [`analyse_with_knowledge`] that stops, between two workbooks, when `cancelled()` is true; the
/// summary then says `cancelled`. **What a stopped pass leaves** (`docs/DECISIONS.md`, "Progress of
/// an Analyse"): every workbook read before the stop is analysed and stays so; each other workbook
/// keeps the inventory an earlier pass gave it, so it stays green when its content is unchanged and
/// goes back to "to analyse" when it changed or was never read. The typed cell cache, cleared up
/// front exactly as in a complete pass, is only a cache: a question re-reads the file on a miss, so
/// it cannot serve a stale cell. A stopped pass does not spend time on what comes after the loop
/// (forgetting vanished workbooks, re-reading documents for the new names, the file profiles): the
/// next pass does, as for the Documents pass.
pub fn analyse_cancellable(
    root: &Path,
    // The other folder (the Documents folder) when it is chosen: the workbooks found there are read
    // too, in place, under their own key (`file_key`). `None` reads the Data folder alone.
    foreign_root: Option<&Path>,
    index: &mut IndexStore,
    locale: &str,
    on_progress: &dyn Fn(IndexProgress),
    mut knowledge: Option<&mut KnowledgeContext>,
    cancelled: &dyn Fn() -> bool,
) -> Result<IndexSummary, AppError> {
    if let Some(context) = knowledge.as_mut() {
        if context.mode.reads_files() {
            context.begin_pass(index.connection());
        }
    }
    // The typed workbook cache (`docs/SESSION-DATA-13-Column-Cache.md`) is forgotten up front,
    // not selectively: a pass rebuilds every inventory below regardless of whether a file
    // changed, so a cell cache that survived it could serve bytes this very pass never looked at
    // again.
    let pass_timer = StageTimer::start();
    let (cleared, clear_ms) = timed_ms(|| index.clear_tabular_workbooks());
    cleared?;

    let mut workbooks: Vec<discovery::DiscoveredFile> = discovery::discover_all(root)
        .into_iter()
        .filter(|file| FileKind::from_extension(&file.extension) == FileKind::TabularCandidate)
        .collect();
    if let Some(foreign) = foreign_root {
        workbooks.extend(discovery::discover_foreign(
            foreign,
            Root::Documents,
            Domain::Data,
        ));
        workbooks.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    }
    let total_files = workbooks.len();
    // `build_inventory_ms` is parsing every workbook; `write_ms` is storing the inventories (and
    // forgetting the typed cache up front). Per workbook, only its position is kept.
    let mut timings = AnalysisTimings::new(AnalysisPass::Data, total_files);
    timings.write_ms += clear_ms;
    on_progress(IndexProgress {
        processed_files: 0,
        total_files,
        batch_index: 0,
        batch_total: 0,
        reading_names: false,
        vectors_done: 0,
        vectors_total: 0,
    });

    let mut usable = 0;
    let mut unusable = Vec::new();
    let mut was_cancelled = false;
    let mut read_files = 0;
    for (position, file) in workbooks.iter().enumerate() {
        if cancelled() {
            was_cancelled = true;
            break;
        }
        read_files = position + 1;
        let path = Path::new(&file.absolute_path);
        let workbook_started = Instant::now();
        let mut workbook_timing = FileTiming {
            position,
            ..FileTiming::default()
        };
        let (built, build_ms) = timed_ms(|| {
            tabular::build_inventory_with_source_rows(path, &file.relative_path, locale)
        });
        workbook_timing.extract_ms = build_ms;
        timings.build_inventory_ms += build_ms;
        match built {
            Ok((inventory, workbook, source_rows)) => {
                if inventory.has_a_usable_sheet() {
                    usable += 1;
                } else {
                    unusable.push(file.relative_path.clone());
                }
                let (stored, write_ms) = timed_ms(|| match knowledge.as_mut() {
                    Some(context) => {
                        context.store_workbook(index, &inventory, &workbook, &source_rows)
                    }
                    // No knowledge base in this pass: the inventory, and whatever an earlier
                    // version of the workbook taught the base is dropped with it.
                    None => index
                        .put_tabular_inventory_with_knowledge(&inventory, None)
                        .map(|_| ()),
                });
                stored?;
                workbook_timing.write_ms = write_ms;
                timings.write_ms += write_ms;
            }
            // The adapters report a file they cannot open as `ReadFailed` or `ParseFailed` alike
            // (`calamine` does not tell a locked file from a fake `.xlsx`), so the difference is
            // decided here, from whether the bytes themselves can be read.
            Err(_) => match std::fs::read(path) {
                // Read, but not a workbook: remembered as analysed with no sheet at all, so it
                // shows red - the same outcome as a document extraction that failed - instead of
                // staying "not analysed yet" for ever.
                Ok(bytes) => {
                    let (stored, write_ms) = timed_ms(|| {
                        index
                            .put_tabular_inventory_with_knowledge(
                                &TabularInventory {
                                    workbook_id: tabular::hash_bytes(&bytes),
                                    relative_path: file.relative_path.clone(),
                                    format: format_of(&file.extension),
                                    sheets: Vec::new(),
                                },
                                None,
                            )
                            .map(|_| ())
                    });
                    stored?;
                    workbook_timing.write_ms = write_ms;
                    timings.write_ms += write_ms;
                    unusable.push(file.relative_path.clone());
                }
                // Could not even be read: a file still being written, or locked. Nothing is
                // stored, so it stays "not analysed yet" and the next pass tries again.
                Err(_) => {}
            },
        }
        workbook_timing.total_ms = elapsed_ms(workbook_started.elapsed());
        timings.note_file(workbook_timing);
        on_progress(IndexProgress {
            processed_files: position + 1,
            total_files,
            batch_index: 0,
            batch_total: 0,
            reading_names: false,
            vectors_done: 0,
            vectors_total: 0,
        });
    }

    let mut removed_files = Vec::new();
    if !was_cancelled {
        let present: Vec<String> = workbooks
            .iter()
            .map(|file| file.relative_path.clone())
            .collect();
        let (retained, retain_ms) = timed_ms(|| index.retain_tabular_inventories(&present));
        removed_files = retained?;
        timings.write_ms += retain_ms;
    }

    // The names the workbooks taught: documents read before them are read again from their stored
    // text. Local, no request, and announced on the progress channel so the card shows that this is
    // what is running.
    let mut announced_names = false;
    let knowledge_summary = match knowledge.as_mut() {
        Some(context) if context.mode.reads_files() && !was_cancelled => {
            if context.settle_epoch(index.connection()).unwrap_or(false) {
                announced_names = true;
                let announce = |progress: backfill::RefreshProgress| {
                    on_progress(IndexProgress {
                        processed_files: progress.done,
                        total_files: progress.total,
                        batch_index: 0,
                        batch_total: 0,
                        reading_names: true,
                        vectors_done: 0,
                        vectors_total: 0,
                    })
                };
                if backfill::refresh_documents(index, context, &|| false, &announce).is_err() {
                    context.note_failed("");
                }
            }
            Some(context.data_summary(index.connection()))
        }
        _ => None,
    };
    if announced_names {
        // The progress was about documents for a while: say it is back to the workbooks, finished.
        on_progress(IndexProgress {
            processed_files: total_files,
            total_files,
            batch_index: 0,
            batch_total: 0,
            reading_names: false,
            vectors_done: 0,
            vectors_total: 0,
        });
    }
    timings.files_processed = read_files;
    timings.total_ms = pass_timer.total_ms();

    // The profile of every analysed workbook: reads the inventories this pass just wrote, never
    // writes to them.
    let profile = if was_cancelled {
        None
    } else {
        crate::file_profile::store::run_workbooks_across(
            index,
            root,
            foreign_root,
            knowledge.as_deref(),
            locale,
        )
    };

    Ok(IndexSummary {
        scanned_files: total_files,
        indexed_files: usable,
        unchanged_files: 0,
        empty_files: unusable,
        ocr_files: Vec::new(),
        low_confidence_files: Vec::new(),
        removed_files,
        renamed_files: Vec::new(),
        rename_failed_files: Vec::new(),
        failed_files: Vec::new(),
        unavailable_capabilities: Vec::new(),
        // Every workbook format that is listed here is read (CSV, XLSX, ODS); only the documents
        // pass reports its unread formats.
        unread_formats: Vec::new(),
        chunk_count: 0,
        timings: Some(timings),
        knowledge: knowledge_summary,
        profile,
        cancelled: was_cancelled,
        vectors_pending: 0,
    })
}

fn format_of(extension: &str) -> TabularFormat {
    match extension {
        "csv" => TabularFormat::Csv,
        "ods" => TabularFormat::Ods,
        _ => TabularFormat::Xlsx,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn invoice_csv(rows: usize) -> String {
        let mut csv = String::from("date;fournisseur;montant\n");
        for day in 1..=rows {
            csv.push_str(&format!(
                "{:02}/03/2026;Fournisseur {day};{day},50\n",
                day % 28 + 1
            ));
        }
        csv
    }

    pub(crate) fn write(root: &Path, relative: &str, content: &[u8]) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    fn state(folder: &DataFolder, relative: &str) -> (Readability, ProcessingStatus) {
        let record = folder.files().find_by_relative_path(relative).unwrap();
        (record.readability, record.processing_status)
    }

    fn open(root: &Path, index: &IndexStore) -> DataFolder {
        DataFolder::discover(root, Some(index), &FileHashCache::new()).unwrap()
    }

    fn setup() -> (tempfile::TempDir, tempfile::TempDir, IndexStore) {
        let data = tempfile::tempdir().unwrap();
        let app = tempfile::tempdir().unwrap();
        let index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
        (data, app, index)
    }

    const ORANGE: (Readability, ProcessingStatus) =
        (Readability::NotAssessed, ProcessingStatus::Discovered);
    const GREEN: (Readability, ProcessingStatus) =
        (Readability::Readable, ProcessingStatus::Indexed);
    const RED: (Readability, ProcessingStatus) =
        (Readability::Unreadable, ProcessingStatus::Failed);

    #[test]
    fn before_any_pass_every_workbook_is_orange() {
        let (data, _app, index) = setup();
        write(data.path(), "factures.csv", invoice_csv(12).as_bytes());

        let folder = open(data.path(), &index);

        assert_eq!(state(&folder, "factures.csv"), ORANGE);
        assert_eq!(folder.report().summary.not_assessed_files, 1);
    }

    #[test]
    fn a_pass_turns_a_real_table_green_and_a_note_red() {
        let (data, _app, mut index) = setup();
        write(data.path(), "factures.csv", invoice_csv(12).as_bytes());
        write(
            data.path(),
            "notes.csv",
            b"a short note\nwith no table in it\n",
        );
        write(data.path(), "faux.xlsx", b"not a zip archive at all");

        let summary = analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();
        let folder = open(data.path(), &index);

        assert_eq!(state(&folder, "factures.csv"), GREEN);
        assert_eq!(state(&folder, "notes.csv"), RED);
        assert_eq!(
            state(&folder, "faux.xlsx"),
            RED,
            "a broken workbook is red, not orange"
        );
        assert_eq!(summary.indexed_files, 1);
        assert_eq!(summary.empty_files, vec!["faux.xlsx", "notes.csv"]);
        let report = folder.report();
        assert_eq!(
            (
                report.summary.total_files,
                report.summary.analysed_files,
                report.summary.unreadable_files
            ),
            (3, 1, 2)
        );
        assert!(folder.usable_inventory("factures.csv").is_some());
        assert!(folder.usable_inventory("notes.csv").is_none());
    }

    #[test]
    fn a_document_in_the_data_folder_waits_for_the_document_pass_and_is_never_parsed_as_a_table() {
        let (data, _app, mut index) = setup();
        write(data.path(), "courrier.pdf", b"%PDF-1.4");

        let summary = analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();
        let folder = open(data.path(), &index);

        assert_eq!(
            summary.scanned_files, 0,
            "the tabular pass reads no document"
        );
        // Its own family reads it (`indexing::run_cancellable` with this folder as the other one): until
        // then it is simply not analysed, not a failure of this folder.
        assert_eq!(state(&folder, "courrier.pdf"), ORANGE);
    }

    #[test]
    fn a_file_no_pipeline_reads_is_red_in_the_data_folder_and_never_parsed() {
        let (data, _app, mut index) = setup();
        write(data.path(), "archive.zip", b"PK");

        let summary = analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();
        let folder = open(data.path(), &index);

        assert_eq!(summary.scanned_files, 0);
        assert_eq!(state(&folder, "archive.zip"), RED);
    }

    #[test]
    fn a_changed_workbook_goes_back_to_orange_until_the_next_pass() {
        let (data, _app, mut index) = setup();
        write(data.path(), "factures.csv", invoice_csv(12).as_bytes());
        analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();

        write(data.path(), "factures.csv", invoice_csv(13).as_bytes());
        let folder = open(data.path(), &index);

        assert_eq!(
            state(&folder, "factures.csv"),
            (Readability::NotAssessed, ProcessingStatus::Pending)
        );
        assert!(folder.usable_inventory("factures.csv").is_none());

        analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();
        assert_eq!(state(&open(data.path(), &index), "factures.csv"), GREEN);
    }

    #[test]
    fn a_removed_workbook_is_forgotten_by_the_next_pass() {
        let (data, _app, mut index) = setup();
        write(data.path(), "factures.csv", invoice_csv(12).as_bytes());
        write(data.path(), "ancien.csv", invoice_csv(10).as_bytes());
        analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();

        std::fs::remove_file(data.path().join("ancien.csv")).unwrap();
        let summary = analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();

        assert_eq!(summary.removed_files, vec!["ancien.csv"]);
        let paths: Vec<String> = index
            .all_tabular_inventories()
            .unwrap()
            .into_iter()
            .map(|row| row.relative_path)
            .collect();
        assert_eq!(paths, vec!["factures.csv"]);
        assert!(open(data.path(), &index)
            .files()
            .find_by_relative_path("ancien.csv")
            .is_none());
    }

    #[test]
    fn a_document_row_with_the_same_path_is_never_joined_onto_a_data_file() {
        // The Documents Folder and the Data Folder share one index file. A document indexed at
        // `notes.txt` must not make the Data Folder's own `notes.txt` look analysed.
        let (data, _app, mut index) = setup();
        write(data.path(), "notes.txt", b"hello");
        index
            .replace_document("notes.txt", "sha", true, &[], &[], None, None)
            .unwrap();

        let folder = open(data.path(), &index);

        let record = folder.files().find_by_relative_path("notes.txt").unwrap();
        assert!(!record.indexed);
        assert!(record.index_metadata.is_none());
        assert_eq!(
            state(&folder, "notes.txt"),
            ORANGE,
            "a document of this folder is not the document of the other one"
        );
    }

    #[test]
    fn progress_reports_counts_only_and_reaches_the_total() {
        let (data, _app, mut index) = setup();
        write(data.path(), "a.csv", invoice_csv(9).as_bytes());
        write(data.path(), "b.csv", invoice_csv(9).as_bytes());
        let seen = std::sync::Mutex::new(Vec::new());

        analyse(data.path(), &mut index, "fr-FR", &|progress| {
            seen.lock()
                .unwrap()
                .push((progress.processed_files, progress.total_files))
        })
        .unwrap();

        assert_eq!(*seen.lock().unwrap(), vec![(0, 2), (1, 2), (2, 2)]);
    }

    /// Runs a pass over `a.csv`, `b.csv` and `c.csv` that is asked to stop once `stop_after`
    /// workbooks have been read (the progress line is sent before the first and after each one).
    fn pass_stopped_after(data: &Path, index: &mut IndexStore, stop_after: usize) -> IndexSummary {
        let sent = std::sync::atomic::AtomicUsize::new(0);
        analyse_cancellable(
            data,
            None,
            index,
            "fr-FR",
            &|_| {
                sent.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            },
            None,
            &|| sent.load(std::sync::atomic::Ordering::SeqCst) > stop_after,
        )
        .unwrap()
    }

    #[test]
    fn a_pass_stopped_between_two_workbooks_keeps_what_it_wrote() {
        let (data, _app, mut index) = setup();
        for name in ["a.csv", "b.csv", "c.csv"] {
            write(data.path(), name, invoice_csv(9).as_bytes());
        }

        let summary = pass_stopped_after(data.path(), &mut index, 1);
        let folder = open(data.path(), &index);

        assert!(summary.cancelled);
        assert_eq!(summary.scanned_files, 3);
        assert_eq!(summary.indexed_files, 1);
        assert_eq!(summary.timings.as_ref().unwrap().files_processed, 1);
        assert_eq!(state(&folder, "a.csv"), GREEN, "read before the stop");
        assert_eq!(state(&folder, "b.csv"), ORANGE, "left to the next pass");
        assert_eq!(state(&folder, "c.csv"), ORANGE, "left to the next pass");
    }

    #[test]
    fn a_stopped_pass_leaves_an_unread_workbook_as_the_last_complete_pass_left_it() {
        let (data, _app, mut index) = setup();
        for name in ["a.csv", "b.csv", "c.csv"] {
            write(data.path(), name, invoice_csv(9).as_bytes());
        }
        analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();
        write(data.path(), "c.csv", invoice_csv(10).as_bytes());
        std::fs::remove_file(data.path().join("b.csv")).unwrap();
        write(data.path(), "d.csv", invoice_csv(11).as_bytes());

        let summary = pass_stopped_after(data.path(), &mut index, 1);
        let folder = open(data.path(), &index);

        assert!(summary.cancelled);
        assert!(
            summary.removed_files.is_empty(),
            "a stopped pass forgets nothing; the next one does"
        );
        assert_eq!(state(&folder, "a.csv"), GREEN);
        assert_eq!(
            state(&folder, "c.csv"),
            (Readability::NotAssessed, ProcessingStatus::Pending),
            "changed since it was read, not read again by this pass"
        );
        assert_eq!(state(&folder, "d.csv"), ORANGE, "never read");
        assert!(
            summary.profile.is_none(),
            "profiles are the next pass's work"
        );
    }

    #[test]
    fn a_pass_that_is_never_stopped_is_the_complete_pass() {
        let (data, _app, mut index) = setup();
        let (other, _other_app, mut other_index) = setup();
        for root in [data.path(), other.path()] {
            write(root, "factures.csv", invoice_csv(12).as_bytes());
            write(
                root,
                "notes.csv",
                b"a short note
with no table in it
",
            );
        }

        let plain = analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();
        let guarded = analyse_cancellable(
            other.path(),
            None,
            &mut other_index,
            "fr-FR",
            &|_| {},
            None,
            &|| false,
        )
        .unwrap();

        assert!(!plain.cancelled && !guarded.cancelled);
        assert_eq!(
            (plain.scanned_files, plain.indexed_files, &plain.empty_files),
            (
                guarded.scanned_files,
                guarded.indexed_files,
                &guarded.empty_files
            )
        );
        assert_eq!(
            plain.timings.as_ref().unwrap().files_processed,
            guarded.timings.as_ref().unwrap().files_processed
        );
        assert_eq!(plain.timings.unwrap().files_processed, 2);
    }
}
