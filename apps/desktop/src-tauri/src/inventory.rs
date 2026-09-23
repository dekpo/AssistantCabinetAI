//! The Work Folder inventory: what is actually on disk, joined with what the index did about it.
//!
//! This module is the single authority for filesystem facts (`docs/WORK-FOLDER-INVENTORY.md`).
//! How many files there are, what they are called, which extension each one carries, whether one
//! was read and how - all of that is answered here, from `std::fs` and from `IndexStore`, and
//! never from a retrieved passage or from a model. A document says whatever its author typed;
//! the filesystem says what is true.
//!
//! Two rules keep it honest. It holds **metadata only**: no page text ever enters a
//! `FileRecord`, so an inventory cannot leak a document. And it reuses the one walk in
//! `discovery` and the one index in `index_store` rather than opening a second scanner or a
//! second database, so there is nothing for the two to disagree about.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::discovery::{self, DiscoveredFile};
use crate::error::AppError;
use crate::file_record::{
    mime_type_for, path_identity, split_name, ExtractionMethod, FileKind, FileRecord,
    IndexMetadata, ProcessingStatus, Readability,
};
use crate::index_store::{DocumentIndexRow, IndexStore};

/// Counts, and nothing else. What the Work Folder panel shows, and what a "how many files?"
/// question is answered from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorySummary {
    pub total_files: usize,
    pub indexed_files: usize,
    pub unreadable_files: usize,
    /// Files no pass has tried to read yet, plus the kinds this pipeline does not own.
    pub not_assessed_files: usize,
    pub folder_count: usize,
    /// Lowercase extension without the dot, in alphabetical order. The empty string covers files
    /// with no extension at all.
    pub by_extension: BTreeMap<String, usize>,
    pub by_kind: BTreeMap<String, usize>,
}

/// One node of the folder hierarchy, for a "show me the folder structure" answer. Names only,
/// never absolute paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderNode {
    /// The folder's own name. Empty for the root, which the interface labels itself.
    pub name: String,
    /// Relative to the Work Folder. Empty for the root.
    pub relative_path: String,
    pub folders: Vec<FolderNode>,
    /// File names directly inside this folder, in canonical order.
    pub files: Vec<String>,
}

/// The authoritative current state of the configured Work Folder.
///
/// Built by `discover`, then read. It is a snapshot on purpose: a value that cannot change under
/// a caller is a value two answers in the same breath cannot disagree about. Rebuild it when the
/// folder may have changed.
#[derive(Debug, Clone)]
pub struct WorkFolderInventory {
    /// The absolute root. Kept so a caller can open a file it resolved; deliberately **not**
    /// serialised and never put in front of the model.
    root: PathBuf,
    files: Vec<FileRecord>,
}

impl WorkFolderInventory {
    /// Walk the folder and join the index's view onto it. `index` is optional: with no index
    /// open, every file is `Discovered` and `NotAssessed`, which is exactly what a folder that
    /// has never been analysed should report.
    pub fn discover(root: &Path, index: Option<&IndexStore>) -> Result<Self, AppError> {
        let rows = match index {
            Some(store) => store.all_documents()?,
            None => Vec::new(),
        };
        let by_path: BTreeMap<&str, &DocumentIndexRow> = rows
            .iter()
            .map(|row| (row.relative_path.as_str(), row))
            .collect();

        let files = discovery::discover_all(root)
            .into_iter()
            .map(|file| {
                let indexed = by_path.get(file.relative_path.as_str()).copied();
                record_for(&file, indexed)
            })
            .collect();

        Ok(Self {
            root: root.to_path_buf(),
            files,
        })
    }

    /// Build one from records directly. For tests, and for a caller that already holds them.
    pub fn from_records(root: &Path, mut files: Vec<FileRecord>) -> Self {
        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Self {
            root: root.to_path_buf(),
            files,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// What to call the folder in front of the user, and in front of the model: its own name,
    /// never the absolute path, which says who the user is and where they keep their files.
    pub fn root_identifier(&self) -> String {
        self.root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    /// Resolve a record back to a path on disk. The only place a `FileRecord` becomes an
    /// absolute path, and it refuses anything that is not in this inventory, so a fabricated
    /// record cannot reach the filesystem.
    pub fn absolute_path(&self, record: &FileRecord) -> Option<PathBuf> {
        if !self.contains(&record.relative_path) {
            return None;
        }
        Some(
            record
                .relative_path
                .split('/')
                .fold(self.root.clone(), |path, part| path.join(part)),
        )
    }

    /// Every file, in canonical relative-path order. The same folder state always produces the
    /// same order, on Windows and on macOS alike.
    pub fn all_files(&self) -> &[FileRecord] {
        &self.files
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn files_by_kind(&self) -> BTreeMap<FileKind, Vec<&FileRecord>> {
        let mut grouped: BTreeMap<FileKind, Vec<&FileRecord>> = BTreeMap::new();
        for file in &self.files {
            grouped.entry(file.kind).or_default().push(file);
        }
        grouped
    }

    pub fn files_of_kind(&self, kind: FileKind) -> Vec<&FileRecord> {
        self.files.iter().filter(|file| file.kind == kind).collect()
    }

    pub fn files_by_extension(&self) -> BTreeMap<String, Vec<&FileRecord>> {
        let mut grouped: BTreeMap<String, Vec<&FileRecord>> = BTreeMap::new();
        for file in &self.files {
            grouped
                .entry(file.extension.clone())
                .or_default()
                .push(file);
        }
        grouped
    }

    /// `extension` without the dot; case is ignored, because a user types `PDF` as readily as
    /// `pdf` and the answer must not depend on which.
    pub fn files_with_extension(&self, extension: &str) -> Vec<&FileRecord> {
        let wanted = extension.trim_start_matches('.').to_lowercase();
        self.files
            .iter()
            .filter(|file| file.extension == wanted)
            .collect()
    }

    pub fn readable_files(&self) -> Vec<&FileRecord> {
        self.files
            .iter()
            .filter(|file| file.readability == Readability::Readable)
            .collect()
    }

    pub fn unreadable_files(&self) -> Vec<&FileRecord> {
        self.files
            .iter()
            .filter(|file| file.readability == Readability::Unreadable)
            .collect()
    }

    pub fn indexed_files(&self) -> Vec<&FileRecord> {
        self.files.iter().filter(|file| file.indexed).collect()
    }

    /// Every record carrying this identity. More than one means two byte-identical copies are in
    /// the folder, which is a fact worth reporting rather than a tie to break silently.
    pub fn find_by_id(&self, id: &str) -> Vec<&FileRecord> {
        self.files.iter().filter(|file| file.id == id).collect()
    }

    pub fn find_by_relative_path(&self, relative_path: &str) -> Option<&FileRecord> {
        self.files
            .iter()
            .find(|file| file.relative_path == relative_path)
    }

    pub fn find_by_name(&self, name: &str) -> Vec<&FileRecord> {
        self.files.iter().filter(|file| file.name == name).collect()
    }

    pub fn find_by_stem(&self, stem: &str) -> Vec<&FileRecord> {
        self.files.iter().filter(|file| file.stem == stem).collect()
    }

    pub fn contains(&self, relative_path: &str) -> bool {
        self.find_by_relative_path(relative_path).is_some()
    }

    pub fn summary(&self) -> InventorySummary {
        let mut by_extension: BTreeMap<String, usize> = BTreeMap::new();
        let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
        for file in &self.files {
            *by_extension.entry(file.extension.clone()).or_insert(0) += 1;
            *by_kind.entry(file.kind.as_code().to_string()).or_insert(0) += 1;
        }
        InventorySummary {
            total_files: self.files.len(),
            indexed_files: self.indexed_files().len(),
            unreadable_files: self.unreadable_files().len(),
            not_assessed_files: self
                .files
                .iter()
                .filter(|file| file.readability == Readability::NotAssessed)
                .count(),
            folder_count: self.folders().len(),
            by_extension,
            by_kind,
        }
    }

    /// Every folder that holds at least one file, relative to the root, in canonical order.
    pub fn folders(&self) -> Vec<String> {
        let mut folders: Vec<String> = Vec::new();
        for file in &self.files {
            let mut prefix = String::new();
            for part in file
                .parent_path()
                .split('/')
                .filter(|part| !part.is_empty())
            {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(part);
                if !folders.contains(&prefix) {
                    folders.push(prefix.clone());
                }
            }
        }
        folders.sort();
        folders
    }

    /// The complete hierarchy as a tree. Names and relative paths only.
    pub fn hierarchy(&self) -> FolderNode {
        let mut root = FolderNode {
            name: String::new(),
            relative_path: String::new(),
            folders: Vec::new(),
            files: Vec::new(),
        };
        for file in &self.files {
            let parts: Vec<&str> = file
                .parent_path()
                .split('/')
                .filter(|part| !part.is_empty())
                .collect();
            let mut node = &mut root;
            let mut prefix = String::new();
            for part in parts {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(part);
                let position = match node.folders.iter().position(|child| child.name == part) {
                    Some(found) => found,
                    None => {
                        node.folders.push(FolderNode {
                            name: part.to_string(),
                            relative_path: prefix.clone(),
                            folders: Vec::new(),
                            files: Vec::new(),
                        });
                        node.folders.len() - 1
                    }
                };
                node = &mut node.folders[position];
            }
            node.files.push(file.name.clone());
        }
        sort_node(&mut root);
        root
    }
}

fn sort_node(node: &mut FolderNode) {
    node.folders.sort_by(|a, b| a.name.cmp(&b.name));
    node.files.sort();
    for child in &mut node.folders {
        sort_node(child);
    }
}

/// Filesystem truth plus index truth, for one file.
fn record_for(file: &DiscoveredFile, indexed: Option<&DocumentIndexRow>) -> FileRecord {
    let name = file
        .relative_path
        .rsplit('/')
        .next()
        .unwrap_or(&file.relative_path)
        .to_string();
    // From the name the filesystem reports, never from what the file says about itself.
    let (stem, extension) = split_name(&name);
    let kind = FileKind::from_extension(&extension);
    let sha256 = hash_file(Path::new(&file.absolute_path));

    let (readability, processing_status, extraction_method) = match indexed {
        None => (
            Readability::NotAssessed,
            ProcessingStatus::Discovered,
            ExtractionMethod::None,
        ),
        Some(row) if row.chunk_count > 0 => {
            let method = if row.ocr_chunk_count > 0 {
                ExtractionMethod::RecognisedOcr
            } else {
                ExtractionMethod::NativeText
            };
            // A file whose bytes have moved on since the pass that indexed it is still readable;
            // what is stored about it is simply out of date.
            let status = match &sha256 {
                Some(current) if current != &row.sha256 => ProcessingStatus::Pending,
                _ => ProcessingStatus::Indexed,
            };
            (Readability::Readable, status, method)
        }
        Some(_) => (
            Readability::Unreadable,
            ProcessingStatus::Failed,
            ExtractionMethod::None,
        ),
    };

    FileRecord {
        id: sha256
            .clone()
            .unwrap_or_else(|| path_identity(&file.relative_path)),
        relative_path: file.relative_path.clone(),
        name,
        stem,
        mime_type: mime_type_for(&extension).to_string(),
        extension,
        kind,
        size_bytes: file.size_bytes,
        modified_at: file.modified_at,
        sha256,
        readability,
        processing_status,
        extraction_method,
        indexed: indexed.is_some_and(|row| row.chunk_count > 0),
        index_metadata: indexed.map(|row| IndexMetadata {
            indexed_sha256: row.sha256.clone(),
            chunk_count: row.chunk_count,
            ocr_engine: row.ocr_engine.clone(),
            ocr_engine_version: row.ocr_engine_version.clone(),
        }),
    }
}

/// The same SHA-256 over the same bytes that `indexing` computes, so one identity serves both
/// the inventory and `Source.origin.sha256`. A file that cannot be read gets `None` rather than
/// a guessed value; the record then falls back to a path identity.
fn hash_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temp dir");
        for (relative, content) in files {
            let path = relative
                .split('/')
                .fold(root.path().to_path_buf(), |path, part| path.join(part));
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, content.as_bytes()).unwrap();
        }
        root
    }

    #[test]
    fn every_file_is_counted_whatever_the_pipeline_can_do_with_it() {
        let root = folder_with(&[
            ("letter.pdf", "pdf"),
            ("planning.csv", "csv"),
            ("archive.zip", "zip"),
            ("NOTICE", "text"),
        ]);

        let inventory = WorkFolderInventory::discover(root.path(), None).unwrap();

        assert_eq!(inventory.file_count(), 4);
        assert_eq!(inventory.summary().total_files, 4);
    }

    #[test]
    fn the_extension_comes_from_the_filesystem_and_not_from_the_content() {
        let root = folder_with(&[("report.txt", "This is a PDF report.")]);

        let inventory = WorkFolderInventory::discover(root.path(), None).unwrap();
        let record = inventory.find_by_name("report.txt").pop().unwrap();

        assert_eq!(record.extension, "txt");
        assert_eq!(record.kind, FileKind::DocumentText);
        assert_eq!(record.name, "report.txt");
        assert_eq!(record.stem, "report");
    }

    #[test]
    fn nested_folders_keep_their_relative_paths_and_hierarchy() {
        let root = folder_with(&[
            ("2026/mars/neurologie.pdf", "one"),
            ("2026/janvier/neurologie.pdf", "two"),
            ("administratif/assurance.txt", "three"),
        ]);

        let inventory = WorkFolderInventory::discover(root.path(), None).unwrap();
        let paths: Vec<&str> = inventory
            .all_files()
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect();

        assert_eq!(
            paths,
            vec![
                "2026/janvier/neurologie.pdf",
                "2026/mars/neurologie.pdf",
                "administratif/assurance.txt"
            ]
        );
        assert_eq!(
            inventory.folders(),
            vec!["2026", "2026/janvier", "2026/mars", "administratif"]
        );

        let tree = inventory.hierarchy();
        assert_eq!(tree.folders.len(), 2);
        assert_eq!(tree.folders[0].name, "2026");
        assert_eq!(tree.folders[0].folders[0].name, "janvier");
        assert_eq!(tree.folders[0].folders[0].files, vec!["neurologie.pdf"]);
        assert_eq!(tree.folders[1].files, vec!["assurance.txt"]);
    }

    #[test]
    fn two_runs_over_the_same_folder_produce_the_same_ordered_inventory() {
        let root = folder_with(&[
            ("b.txt", "b"),
            ("a.txt", "a"),
            ("sub/c.txt", "c"),
            ("sub/a.txt", "a-again"),
        ]);

        let first = WorkFolderInventory::discover(root.path(), None).unwrap();
        let second = WorkFolderInventory::discover(root.path(), None).unwrap();

        assert_eq!(first.all_files(), second.all_files());
    }

    #[test]
    fn an_identity_follows_the_content_rather_than_the_name() {
        let root = folder_with(&[("first.txt", "same bytes"), ("second.txt", "same bytes")]);

        let inventory = WorkFolderInventory::discover(root.path(), None).unwrap();
        let first = inventory.find_by_name("first.txt").pop().unwrap();
        let second = inventory.find_by_name("second.txt").pop().unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(inventory.find_by_id(&first.id).len(), 2);
        assert_eq!(first.sha256.as_ref().unwrap().len(), 64);
    }

    #[test]
    fn a_folder_that_was_never_analysed_reports_nothing_as_unreadable() {
        let root = folder_with(&[("a.pdf", "a"), ("b.png", "b")]);

        let inventory = WorkFolderInventory::discover(root.path(), None).unwrap();

        assert_eq!(inventory.unreadable_files().len(), 0);
        assert_eq!(inventory.indexed_files().len(), 0);
        assert_eq!(inventory.summary().not_assessed_files, 2);
        assert!(inventory
            .all_files()
            .iter()
            .all(|file| file.processing_status == ProcessingStatus::Discovered));
    }

    #[test]
    fn the_root_identifier_is_a_folder_name_and_not_a_machine_path() {
        let root = folder_with(&[("a.txt", "a")]);
        let inventory = WorkFolderInventory::discover(root.path(), None).unwrap();

        let identifier = inventory.root_identifier();

        assert!(!identifier.contains('/') && !identifier.contains('\\'));
        assert!(!identifier.is_empty());
    }

    #[test]
    fn an_absolute_path_is_only_produced_for_a_file_the_inventory_holds() {
        let root = folder_with(&[("sub/a.txt", "a")]);
        let inventory = WorkFolderInventory::discover(root.path(), None).unwrap();
        let record = inventory
            .find_by_relative_path("sub/a.txt")
            .unwrap()
            .clone();

        assert!(inventory.absolute_path(&record).is_some());

        let mut forged = record;
        forged.relative_path = "sub/elsewhere.txt".into();
        assert!(inventory.absolute_path(&forged).is_none());
    }

    #[test]
    fn no_record_carries_document_text() {
        let root = folder_with(&[("a.txt", "a confidential sentence")]);

        let inventory = WorkFolderInventory::discover(root.path(), None).unwrap();
        let serialised = serde_json::to_string(inventory.all_files()).unwrap();

        assert!(!serialised.contains("confidential"));
    }
}
