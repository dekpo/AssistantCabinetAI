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
pub mod inventory;
pub mod xlsx_adapter;

use std::path::Path;

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum TabularError {
    #[error("read_failed")]
    ReadFailed,
    #[error("parse_failed")]
    ParseFailed,
    #[error("unsupported_extension")]
    UnsupportedExtension,
}

/// One cell, exactly as the adapter read it. A formula's cached value and its expression are
/// kept apart from each other and from a plain value, because a formula's cached result is a
/// claim the file makes and not a verified fact (`docs/DECISIONS.md`, "a formula's cached
/// result"). Nothing here evaluates a formula; the adapters only report what the file already
/// stored.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Empty,
    /// A CSV field, or an XLSX text cell. A CSV number is deliberately **not** represented any
    /// other way: locale rules (decimal comma, `dd/mm/yyyy`) are applied once, in
    /// `inventory::TabularInventory::build`, rather than guessed twice by two adapters.
    Text(String),
    /// An XLSX numeric cell, already typed by the file format itself.
    Number(f64),
    /// An XLSX date/datetime cell, already typed by the file format itself. Kept as the text the
    /// format gives rather than a calendar type this crate does not otherwise depend on.
    Date(String),
    /// A formula cell: the expression the file stores, and the value it last cached, if the
    /// adapter could read one. `cached_value` is never presented as a verified fact.
    Formula {
        expression: String,
        cached_value: Option<Box<CellValue>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SheetData {
    pub name: String,
    /// Row-major. Rows are not padded to a common width: a padded empty cell would be
    /// indistinguishable from a cell that was genuinely written blank.
    pub rows: Vec<Vec<CellValue>>,
}

/// What `TabularDataSource::open` returns: every sheet's cells, nothing inferred yet. A CSV file
/// becomes a single-sheet workbook; an XLSX file keeps every sheet it defines.
#[derive(Debug, Clone, PartialEq)]
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
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let result = build_inventory(&path, "archive.zip");

        assert_eq!(result.unwrap_err(), TabularError::UnsupportedExtension);
    }
}
