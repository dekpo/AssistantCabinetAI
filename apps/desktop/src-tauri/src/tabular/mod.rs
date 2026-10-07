//! The tabular pipeline's own boundary between a file on disk and structured data
//! (`docs/ARCHITECTURE.md`):
//!
//! ```text
//! filesystem -> TabularDataSource -> Workbook -> TabularInventory -> (deterministic analysis, later)
//! ```
//!
//! CSV and XLSX are a first-class source with their own pipeline, never document chunks: this
//! module and its adapters never flatten a workbook into text and never touch the document
//! embedding pipeline. `TabularDataSource` is the only thing the rest of the application knows
//! about reading a workbook; the CSV and XLSX adapters both implement it and agree on one
//! `Workbook` shape, which `inventory::TabularInventory::build` turns into structural facts.
//!
//! This module reads files only. It never sends anything to the gateway or the network.

pub mod csv_adapter;
pub mod engine;
pub mod escalation;
pub mod inventory;
pub mod question;
pub mod query_plan;
pub mod structural;
pub mod xlsx_adapter;

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum TabularError {
    #[error("read_failed")]
    ReadFailed,
    #[error("parse_failed")]
    ParseFailed,
    #[error("unsupported_extension")]
    UnsupportedExtension,
    /// The bytes on disk no longer hash to the identity the caller expected - the workbook
    /// changed, or was replaced, since whatever pinned that identity (a cached inventory, a
    /// selected `ScopeEntry`) was built. The engine must reject it explicitly rather than run
    /// against a workbook that is silently no longer the one that was chosen
    /// (`docs/SESSION-DATA-04-TABULAR-Engine.md` section 5).
    #[error("workbook_changed")]
    WorkbookChanged,
}

/// One cell, exactly as the adapter read it. A formula's cached value and its expression are
/// kept apart from each other and from a plain value, because a formula's cached result is a
/// claim the file makes and not a verified fact (`docs/DECISIONS.md`, "a formula's cached
/// result"). Nothing here evaluates a formula; the adapters only report what the file already
/// stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CellValue {
    Empty,
    /// A CSV field, or an XLSX text cell. A CSV number is deliberately **not** represented any
    /// other way: locale rules (decimal comma, `dd/mm/yyyy`) are applied once, in
    /// `inventory::TabularInventory::build`, rather than guessed twice by two adapters.
    Text(String),
    /// An XLSX numeric cell, already typed by the file format itself.
    Number(f64),
    /// An XLSX date/datetime cell, already typed by the file format itself, read to a real
    /// calendar date (`calamine`'s `dates` feature, D3) - never the raw serial number the file
    /// stores, and never a second parse: the adapter is the only place that produces this
    /// variant, so a comparison against a text date parsed elsewhere reads the same calendar day.
    Date(chrono::NaiveDate),
    /// A formula cell: the expression the file stores, and the value it last cached, if the
    /// adapter could read one. `cached_value` is never presented as a verified fact.
    Formula {
        expression: String,
        cached_value: Option<Box<CellValue>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetData {
    pub name: String,
    /// Row-major. Rows are not padded to a common width: a padded empty cell would be
    /// indistinguishable from a cell that was genuinely written blank.
    pub rows: Vec<Vec<CellValue>>,
}

/// What `TabularDataSource::open` returns: every sheet's cells, nothing inferred yet. A CSV file
/// becomes a single-sheet workbook; an XLSX file keeps every sheet it defines.
///
/// `Serialize`/`Deserialize`: the shape the typed workbook cache stores
/// (`docs/SESSION-DATA-13-Column-Cache.md`) - cell values already typed by the adapter (numbers,
/// real calendar dates), exactly what `IndexStore::put_tabular_workbook` persists keyed by the
/// content hash, and what a cache hit hands back with no file read at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workbook {
    pub sheets: Vec<SheetData>,
}

/// The adapter boundary between a file on disk and a `Workbook`. A CSV adapter and an XLSX
/// adapter both implement this and nothing else in the application talks to `csv` or `calamine`
/// directly.
pub trait TabularDataSource {
    fn open(&self, path: &Path) -> Result<Workbook, TabularError>;
}

/// The content SHA-256, the same identity `FileRecord::id` and `Source.origin.sha256` carry
/// (`docs/WORK-FOLDER-INVENTORY.md`): no second hashing scheme for a workbook.
pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Build a `TabularInventory` for one file: read its bytes once for identity, open it through
/// the adapter its extension selects, and take a full pass over every sheet. The one place
/// identity, adapter choice and inventory-building meet, so a caller working from a
/// `FileRecord` does not re-derive any of the three separately.
pub fn build_inventory(
    path: &Path,
    relative_path: &str,
    locale: &str,
) -> Result<inventory::TabularInventory, TabularError> {
    let bytes = std::fs::read(path).map_err(|_| TabularError::ReadFailed)?;
    let workbook_id = hash_bytes(&bytes);
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let (format, workbook) = match extension.as_str() {
        "csv" => (
            inventory::TabularFormat::Csv,
            csv_adapter::CsvDataSource.open(path)?,
        ),
        "xls" | "xlsx" | "xlsm" => (
            inventory::TabularFormat::Xlsx,
            xlsx_adapter::XlsxDataSource.open(path)?,
        ),
        _ => return Err(TabularError::UnsupportedExtension),
    };

    Ok(inventory::TabularInventory::build(
        relative_path,
        &workbook_id,
        format,
        &workbook,
        locale,
    ))
}

/// Re-read a workbook and confirm it still hashes to `expected_workbook_id` before handing back
/// fresh cells and a fresh inventory to run an operation against.
///
/// This is what `AnalysisScope::resolve`'s `changed`/`missing` reporting does for a *selected*
/// workbook, applied at the moment the engine actually reads one: a workbook can change between
/// the pass that built the cached inventory the scope pinned and the question that reads it, and
/// silently running the engine against the old inventory but the new bytes - or the new bytes
/// under the old inventory's stale header/column assumptions - would be exactly the quiet failure
/// this pipeline exists to avoid (`docs/SESSION-DATA-04-TABULAR-Engine.md` section 5).
pub fn load_current(
    path: &Path,
    relative_path: &str,
    expected_workbook_id: &str,
    locale: &str,
) -> Result<(Workbook, inventory::TabularInventory), TabularError> {
    let inventory = build_inventory(path, relative_path, locale)?;
    if inventory.workbook_id != expected_workbook_id {
        return Err(TabularError::WorkbookChanged);
    }
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let workbook = match extension.as_str() {
        "csv" => csv_adapter::CsvDataSource.open(path)?,
        "xls" | "xlsx" | "xlsm" => xlsx_adapter::XlsxDataSource.open(path)?,
        _ => return Err(TabularError::UnsupportedExtension),
    };
    Ok((workbook, inventory))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_LOCALE: &str = "fr-FR";

    #[test]
    fn the_same_bytes_hash_to_the_same_identity() {
        assert_eq!(hash_bytes(b"a,b\n1,2\n"), hash_bytes(b"a,b\n1,2\n"));
        assert_ne!(hash_bytes(b"a,b\n1,2\n"), hash_bytes(b"a,b\n1,3\n"));
    }

    #[test]
    fn an_unsupported_extension_is_refused_before_any_adapter_runs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("archive.zip");
        std::fs::write(&path, b"not tabular").unwrap();

        let result = build_inventory(&path, "archive.zip", TEST_LOCALE);

        assert_eq!(result.unwrap_err(), TabularError::UnsupportedExtension);
    }

    // --- Adversarial cases (docs/SESSION-DATA-04-TABULAR-Engine.md section 7) ------------

    #[test]
    fn a_workbook_that_changed_since_the_inventory_was_built_is_rejected_explicitly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("montants.csv");
        std::fs::write(&path, b"nom,montant\nCamille,10\n").unwrap();
        let original = build_inventory(&path, "montants.csv", TEST_LOCALE).unwrap();

        // The file changes underneath the pinned identity - a new export overwriting the old one,
        // for instance - before the next question reads it.
        std::fs::write(&path, b"nom,montant\nCamille,10\nEsaie,20\n").unwrap();

        let result = load_current(&path, "montants.csv", &original.workbook_id, TEST_LOCALE);

        assert_eq!(result.unwrap_err(), TabularError::WorkbookChanged);
    }

    #[test]
    fn a_workbook_with_an_unchanged_hash_loads_fresh_cells_and_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("montants.csv");
        std::fs::write(&path, b"nom,montant\nCamille,10\n").unwrap();
        let original = build_inventory(&path, "montants.csv", TEST_LOCALE).unwrap();

        let (workbook, inventory) =
            load_current(&path, "montants.csv", &original.workbook_id, TEST_LOCALE).unwrap();

        assert_eq!(inventory, original);
        assert_eq!(workbook.sheets[0].rows.len(), 2);
    }

    #[test]
    fn a_workbook_that_has_disappeared_from_disk_is_rejected_not_guessed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("montants.csv");
        std::fs::write(&path, b"nom,montant\nCamille,10\n").unwrap();
        let original = build_inventory(&path, "montants.csv", TEST_LOCALE).unwrap();

        std::fs::remove_file(&path).unwrap();

        let result = load_current(&path, "montants.csv", &original.workbook_id, TEST_LOCALE);

        assert_eq!(result.unwrap_err(), TabularError::ReadFailed);
    }

    // --- Green or red on the Data Folder listing (docs/SESSION-DATA-05-TABULAR-UI.md 5a) --------

    fn inventory_of(name: &str, content: &str) -> inventory::TabularInventory {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, content.as_bytes()).unwrap();
        build_inventory(&path, name, TEST_LOCALE).expect("a text file always parses as a CSV")
    }

    #[test]
    fn a_note_renamed_to_csv_parses_but_is_not_a_usable_table() {
        let note = "Call the lab back on Monday morning\n\
                    Order paper for the printer\n\
                    Check the July rota\n\
                    Post the waiting letters\n\
                    Update the on-call table\n\
                    Read the maintenance contract again\n\
                    File the quarterly invoices\n\
                    Prepare the September meeting\n\
                    Archive the June folders\n";

        let parsed = inventory_of("notes.csv", note);

        assert_eq!(parsed.sheets.len(), 1, "it did parse, as one sheet");
        assert!(!parsed.has_a_usable_sheet());
    }

    #[test]
    fn a_real_but_tiny_table_is_offered_as_queryable() {
        // A short real export - a handful of clients, a day's invoices - is every bit as real as
        // a thousand-row one (`docs/DECISIONS.md`, "a short real export stays usable"). What kept
        // the note above red was its single column, not its length.
        let parsed = inventory_of(
            "trois.csv",
            "nom;montant\nAlpha;10,50\nBeta;20,00\nGamma;30,00\n",
        );

        assert_eq!(parsed.sheets[0].column_count, 2);
        assert_eq!(parsed.sheets[0].row_count, 3);
        assert!(parsed.has_a_usable_sheet());
    }

    #[test]
    fn a_single_data_row_beside_a_real_header_is_still_a_usable_table() {
        let parsed = inventory_of("one-row.csv", "name;amount\nAlpha;10,50\n");

        assert_eq!(parsed.sheets[0].row_count, 1);
        assert!(parsed.has_a_usable_sheet());
    }

    #[test]
    fn an_invoice_sized_csv_is_a_usable_table() {
        let mut csv = String::from("date;fournisseur;montant\n");
        for day in 1..=24 {
            csv.push_str(&format!("{day:02}/03/2026;Fournisseur {day};{day},50\n"));
        }

        let parsed = inventory_of("factures.csv", &csv);

        assert!(parsed.has_a_usable_sheet());
    }

    #[test]
    fn two_workbooks_with_confusable_file_names_are_never_chosen_between() {
        // The tabular pipeline reuses the same file-reference resolver documents already use -
        // it does not reinvent ambiguity handling for its own file kind.
        use crate::file_reference::{FileReferenceResolver, ReferenceStatus};
        use crate::file_record::{mime_type_for, split_name, FileKind, FileRecord};
        use crate::inventory::WorkFolderInventory;

        fn workbook_record(relative_path: &str) -> FileRecord {
            let name = relative_path.rsplit('/').next().unwrap().to_string();
            let (stem, extension) = split_name(&name);
            FileRecord {
                id: format!("id-{relative_path}"),
                relative_path: relative_path.to_string(),
                name,
                stem,
                mime_type: mime_type_for(&extension).to_string(),
                kind: FileKind::from_extension(&extension),
                extension,
                size_bytes: 1,
                modified_at: None,
                sha256: Some("f".repeat(64)),
                readability: crate::file_record::Readability::Readable,
                processing_status: crate::file_record::ProcessingStatus::Indexed,
                extraction_method: crate::file_record::ExtractionMethod::None,
                indexed: false,
                index_metadata: None,
            }
        }

        let inventory = WorkFolderInventory::from_records(
            Path::new("data"),
            vec![
                workbook_record("2026/janvier/facturation.csv"),
                workbook_record("2026/mars/facturation.csv"),
            ],
        );

        let resolution =
            FileReferenceResolver::new(&inventory).resolve("facturation.csv");

        assert_eq!(resolution.status, ReferenceStatus::MultipleMatches);
        assert_eq!(resolution.candidates.len(), 2);
    }
}
