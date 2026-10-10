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
use crate::file_record::{FileKind, FileRecord, ProcessingStatus, Readability};
use crate::index_store::IndexStore;
use crate::indexing::{IndexProgress, IndexSummary};
use crate::inventory::{FileHashCache, WorkFolderInventory};
use crate::knowledge::diagnostics::{
    elapsed_ms, timed_ms, AnalysisPass, AnalysisTimings, FileTiming, StageTimer,
};
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
    pub fn discover(
        root: &Path,
        index: Option<&IndexStore>,
        cache: &FileHashCache,
    ) -> Result<Self, AppError> {
        // No index: a document row must never be joined onto a file of this folder.
        let on_disk = WorkFolderInventory::discover_with_cache(root, None, cache)?;
        let analysed: BTreeMap<String, String> = match index {
            Some(store) => store
                .all_tabular_inventories()?
                .into_iter()
                .map(|row| (row.relative_path, row.workbook_id))
                .collect(),
            None => BTreeMap::new(),
        };

        let mut usable = BTreeMap::new();
        let mut files = Vec::with_capacity(on_disk.file_count());
        for record in on_disk.all_files() {
            let mut record = record.clone();
            let (readability, status) = if record.kind != FileKind::TabularCandidate {
                (Readability::Unreadable, ProcessingStatus::Failed)
            } else {
                match analysed.get(&record.relative_path) {
                    None => (Readability::NotAssessed, ProcessingStatus::Discovered),
                    Some(workbook_id) if record.sha256.as_deref() != Some(workbook_id) => {
                        (Readability::NotAssessed, ProcessingStatus::Pending)
                    }
                    Some(workbook_id) => {
                        let cached = match index {
                            Some(store) => store.tabular_inventory_by_hash(workbook_id)?,
                            None => None,
                        };
                        match cached {
                            Some(inventory) if inventory.has_a_usable_sheet() => {
                                usable.insert(record.relative_path.clone(), inventory);
                                (Readability::Readable, ProcessingStatus::Indexed)
                            }
                            Some(_) => (Readability::Unreadable, ProcessingStatus::Failed),
                            None => (Readability::NotAssessed, ProcessingStatus::Discovered),
                        }
                    }
                }
            };
            record.readability = readability;
            record.processing_status = status;
            record.indexed = false;
            record.index_metadata = None;
            files.push(record);
        }

        Ok(Self {
            files: WorkFolderInventory::from_records(on_disk.root(), files),
            usable,
        })
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
    // The typed workbook cache (`docs/SESSION-DATA-13-Column-Cache.md`) is forgotten up front,
    // not selectively: a pass rebuilds every inventory below regardless of whether a file
    // changed, so a cell cache that survived it could serve bytes this very pass never looked at
    // again.
    let pass_timer = StageTimer::start();
    let (cleared, clear_ms) = timed_ms(|| index.clear_tabular_workbooks());
    cleared?;

    let workbooks: Vec<discovery::DiscoveredFile> = discovery::discover_all(root)
        .into_iter()
        .filter(|file| FileKind::from_extension(&file.extension) == FileKind::TabularCandidate)
        .collect();
    let total_files = workbooks.len();
    // `build_inventory_ms` is parsing every workbook; `write_ms` is storing the inventories (and
    // forgetting the typed cache up front). Per workbook, only its position is kept.
    let mut timings = AnalysisTimings::new(AnalysisPass::Data, total_files);
    timings.files_processed = total_files;
    timings.write_ms += clear_ms;
    on_progress(IndexProgress {
        processed_files: 0,
        total_files,
        batch_index: 0,
        batch_total: 0,
        reading_names: false,
    });

    let mut usable = 0;
    let mut unusable = Vec::new();
    for (position, file) in workbooks.iter().enumerate() {
        let path = Path::new(&file.absolute_path);
        let workbook_started = Instant::now();
        let mut workbook_timing = FileTiming {
            position,
            ..FileTiming::default()
        };
        let (built, build_ms) =
            timed_ms(|| tabular::build_inventory(path, &file.relative_path, locale));
        workbook_timing.extract_ms = build_ms;
        timings.build_inventory_ms += build_ms;
        match built {
            Ok(inventory) => {
                if inventory.has_a_usable_sheet() {
                    usable += 1;
                } else {
                    unusable.push(file.relative_path.clone());
                }
                let (stored, write_ms) = timed_ms(|| index.put_tabular_inventory(&inventory));
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
                        index.put_tabular_inventory(&TabularInventory {
                            workbook_id: tabular::hash_bytes(&bytes),
                            relative_path: file.relative_path.clone(),
                            format: format_of(&file.extension),
                            sheets: Vec::new(),
                        })
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
        });
    }

    let present: Vec<String> = workbooks
        .iter()
        .map(|file| file.relative_path.clone())
        .collect();
    let (retained, retain_ms) = timed_ms(|| index.retain_tabular_inventories(&present));
    let removed_files = retained?;
    timings.write_ms += retain_ms;
    timings.total_ms = pass_timer.total_ms();

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
        chunk_count: 0,
        timings: Some(timings),
        knowledge: None,
    })
}

fn format_of(extension: &str) -> TabularFormat {
    match extension {
        "csv" => TabularFormat::Csv,
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
    fn a_file_that_is_not_a_spreadsheet_is_red_and_never_parsed() {
        let (data, _app, mut index) = setup();
        write(data.path(), "courrier.pdf", b"%PDF-1.4");

        let summary = analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();
        let folder = open(data.path(), &index);

        assert_eq!(summary.scanned_files, 0);
        assert_eq!(state(&folder, "courrier.pdf"), RED);
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
        assert_eq!(state(&folder, "notes.txt"), RED, "not a spreadsheet");
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
}
