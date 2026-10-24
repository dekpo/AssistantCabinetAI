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

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::discovery::{self, DiscoveredFile};
use crate::error::AppError;
use crate::file_key::{family_of_extension, key_for, FamilyRoots, Root};
use crate::file_profile::build::Domain;
use crate::file_record::{
    mime_type_for, path_identity, split_name, ExtractionMethod, FileKind, FileRecord,
    IndexMetadata, ProcessingStatus, Readability,
};
use crate::index_store::{DocumentIndexRow, IndexStore};
use crate::tabular::inventory::TabularInventory;

/// The key a discovered file has in the index (`file_key::key_for`): decided from its absolute path and
/// the two folders. A file under neither folder cannot come from a walk of one of them; its path inside
/// the folder it was found in stands in, so the listing never loses it.
fn key_of(
    family: Domain,
    documents: Option<&Path>,
    data: Option<&Path>,
    file: &DiscoveredFile,
) -> String {
    key_for(family, documents, data, Path::new(&file.absolute_path))
        .unwrap_or_else(|| file.relative_path.clone())
}

/// What one file's bytes hashed to, and the two cheap facts that say whether they still do.
#[derive(Debug, Clone)]
struct CachedHash {
    size_bytes: u64,
    modified_at: Option<i64>,
    sha256: String,
}

/// Remembers what each file hashed to, so an inventory built often does not re-read the folder
/// every time.
///
/// The inventory decides "indexed" against "changed since it was indexed" by comparing content
/// hashes, which means reading every byte in the work folder. That was affordable while the panel
/// was rebuilt on startup and after a pass. It is not affordable once it is also rebuilt every
/// time she comes back to the window, on a folder of scans that is hundreds of megabytes.
///
/// A cached hash is reused only when **both** the size and the modification time are unchanged.
/// The failure this accepts is a file rewritten within the same second to exactly the same length:
/// the panel would go on calling it indexed until the next pass. It is a hint that would be
/// briefly stale, never a wrong index - `indexing::run` hashes the bytes itself and remains the
/// only thing that decides what is stored.
///
/// Owned by the caller rather than kept in a global, so a test builds its own and two work
/// folders cannot share one. Keyed by absolute path for the same reason.
#[derive(Debug, Default)]
pub struct FileHashCache {
    entries: Mutex<HashMap<String, CachedHash>>,
}

impl FileHashCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// The file's content hash, read only when the cheap facts say it may have moved on. `None`
    /// when the file could not be read at all, which is never cached: an unreadable file is a
    /// transient state (a scanner still writing, a locked document), not an identity.
    fn hash_of(&self, file: &DiscoveredFile) -> Option<String> {
        if let Ok(entries) = self.entries.lock() {
            if let Some(cached) = entries.get(&file.absolute_path) {
                if cached.size_bytes == file.size_bytes && cached.modified_at == file.modified_at {
                    return Some(cached.sha256.clone());
                }
            }
        }
        let sha256 = hash_file(Path::new(&file.absolute_path))?;
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(
                file.absolute_path.clone(),
                CachedHash {
                    size_bytes: file.size_bytes,
                    modified_at: file.modified_at,
                    sha256: sha256.clone(),
                },
            );
        }
        Some(sha256)
    }

    /// Drop everything remembered. Used when the index is emptied, so the next inventory is built
    /// from the folder as it is rather than from what a previous session believed about it.
    pub fn forget_all(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}

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
    /// The other folder, when this inventory is a family's view across both (`family_view`): the files
    /// of the family found there carry a key (`file_key`) instead of a path, and are resolved against
    /// this folder. `None` for an inventory of one folder.
    foreign_root: Option<PathBuf>,
    files: Vec<FileRecord>,
}

impl WorkFolderInventory {
    /// Walk the folder and join the index's view onto it. `index` is optional: with no index
    /// open, every file is `Discovered` and `NotAssessed`, which is exactly what a folder that
    /// has never been analysed should report.
    pub fn discover(root: &Path, index: Option<&IndexStore>) -> Result<Self, AppError> {
        Self::discover_with_cache(root, index, &FileHashCache::new())
    }

    /// The same walk, reusing what `cache` already knows about each file's bytes. The command
    /// that answers the folder panel passes a cache that lives as long as the application, so
    /// rebuilding the panel is cheap enough to do whenever the window is looked at again.
    pub fn discover_with_cache(
        root: &Path,
        index: Option<&IndexStore>,
        cache: &FileHashCache,
    ) -> Result<Self, AppError> {
        Self::discover_alongside(root, None, index, cache)
    }

    /// [`discover_with_cache`](Self::discover_with_cache) with the Data folder given when it is chosen:
    /// it decides the key of a file the two folders share (equal or nested folders), so both listings
    /// agree on it.
    pub fn discover_alongside(
        root: &Path,
        data: Option<&Path>,
        index: Option<&IndexStore>,
        cache: &FileHashCache,
    ) -> Result<Self, AppError> {
        Ok(Self::discover_in(Root::Documents, root, data, index, cache)?.0)
    }

    /// The physical folder `root` is, whichever of the two it is: every file listed once, each joined to
    /// what **its own family** made of it. A document is joined to the document index, a workbook to the
    /// tabular inventories, by the key the file has there (`file_key`): a file of the family that sits
    /// in the folder of the other family is read in place and stored under a key that names this
    /// folder, so it is found here and the same path in the other folder is not mistaken for it. `other`
    /// is the other folder when it is chosen: the key of a file is decided from its absolute path and the
    /// two folders (`file_key::key_for`), so a file the two share has the same key, and the same state,
    /// in both listings. The second value holds the cached inventory of every workbook that reads as a
    /// table, by its path inside this folder.
    ///
    /// A file no pipeline reads is `NotAssessed` in the Documents folder and unreadable in the Data
    /// folder, as it has always been.
    pub fn discover_in(
        role: Root,
        root: &Path,
        other: Option<&Path>,
        index: Option<&IndexStore>,
        cache: &FileHashCache,
    ) -> Result<(Self, BTreeMap<String, TabularInventory>), AppError> {
        let (documents, data) = match role {
            Root::Documents => (Some(root), other),
            Root::Data => (other, Some(root)),
        };
        let rows = match index {
            Some(store) => store.all_documents()?,
            None => Vec::new(),
        };
        let by_key: BTreeMap<&str, &DocumentIndexRow> = rows
            .iter()
            .map(|row| (row.relative_path.as_str(), row))
            .collect();
        let workbooks: BTreeMap<String, String> = match index {
            Some(store) => store
                .all_tabular_inventories()?
                .into_iter()
                .map(|row| (row.relative_path, row.workbook_id))
                .collect(),
            None => BTreeMap::new(),
        };

        let mut usable = BTreeMap::new();
        let mut files = Vec::new();
        for file in discovery::discover_all(root) {
            let name = file.relative_path.rsplit('/').next().unwrap_or("");
            let family = family_of_extension(&split_name(name).1);
            let record = match family {
                Some(Domain::Documents) => {
                    let key = key_of(Domain::Documents, documents, data, &file);
                    record_for(&file, by_key.get(key.as_str()).copied(), cache)
                }
                Some(Domain::Data) => {
                    let mut record = record_for(&file, None, cache);
                    let key = key_of(Domain::Data, documents, data, &file);
                    let (readability, status, inventory) = tabular_state(
                        record.sha256.as_deref(),
                        workbooks.get(&key).map(String::as_str),
                        index,
                    )?;
                    record.readability = readability;
                    record.processing_status = status;
                    // The document index holds nothing about a workbook, and nothing may claim it does.
                    record.indexed = false;
                    record.index_metadata = None;
                    if let Some(inventory) = inventory {
                        record.layout_caveats = inventory
                            .caveats(&record.extension)
                            .into_iter()
                            .map(str::to_string)
                            .collect();
                        usable.insert(file.relative_path.clone(), inventory);
                    }
                    record
                }
                None => {
                    let mut record = record_for(&file, None, cache);
                    if role == Root::Data {
                        record.readability = Readability::Unreadable;
                        record.processing_status = ProcessingStatus::Failed;
                    }
                    record
                }
            };
            files.push(record);
        }

        Ok((
            Self {
                root: root.to_path_buf(),
                foreign_root: None,
                files,
            },
            usable,
        ))
    }

    /// Build one from records directly. For tests, and for a caller that already holds them.
    pub fn from_records(root: &Path, mut files: Vec<FileRecord>) -> Self {
        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Self {
            root: root.to_path_buf(),
            foreign_root: None,
            files,
        }
    }

    /// The same inventory, resolving the keys of the other folder's files against `foreign`.
    pub fn with_foreign_root(mut self, foreign: Option<PathBuf>) -> Self {
        self.foreign_root = foreign;
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The other folder of a family's view across both, when it has one.
    pub fn foreign_root(&self) -> Option<&Path> {
        self.foreign_root.as_deref()
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
        if crate::file_key::is_foreign_key(&record.relative_path) {
            // A file of the other folder: rebuilt from plain names under that folder, or nothing.
            let family = family_of_extension(&record.extension)?;
            return FamilyRoots::new(family, &self.root, self.foreign_root.as_deref())
                .resolve(&record.relative_path);
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

    /// How many documents one more analysis pass would still change: never read, or read and
    /// changed since. A file that was read and produced nothing is **not** one of them - it will
    /// not become readable by being analysed again, and counting it would leave a warning on
    /// screen for ever, which teaches her to stop reading warnings. The same two states
    /// `analysisPending` lights the Analyse button for, so the button and an answer can never
    /// disagree about whether the folder is up to date.
    pub fn unanalysed_documents(&self) -> usize {
        self.files
            .iter()
            .filter(|file| {
                file.kind.is_document()
                    && matches!(
                        file.processing_status,
                        ProcessingStatus::Discovered | ProcessingStatus::Pending
                    )
            })
            .count()
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
            // Analysed means read **and current**, the same word the Data folder's report uses: a document
            // whose bytes changed since it was read is out of date, not analysed.
            indexed_files: self
                .files
                .iter()
                .filter(|file| file.processing_status == ProcessingStatus::Indexed)
                .count(),
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

/// What the tabular analysis made of one workbook: not analysed yet, changed since, readable as a
/// table, or analysed with nothing usable in it. `analysed_id` is the content id the last pass stored
/// for it (`None` when it was never read); the third value is the cached inventory of a workbook that
/// reads as a table. The one place this is decided, for a workbook of either folder.
pub(crate) fn tabular_state(
    sha256: Option<&str>,
    analysed_id: Option<&str>,
    index: Option<&IndexStore>,
) -> Result<(Readability, ProcessingStatus, Option<TabularInventory>), AppError> {
    Ok(match analysed_id {
        None => (Readability::NotAssessed, ProcessingStatus::Discovered, None),
        Some(workbook_id) if sha256 != Some(workbook_id) => {
            (Readability::NotAssessed, ProcessingStatus::Pending, None)
        }
        Some(workbook_id) => {
            let cached = match index {
                Some(store) => store.tabular_inventory_by_hash(workbook_id)?,
                None => None,
            };
            match cached {
                Some(inventory) if inventory.has_a_usable_sheet() => (
                    Readability::Readable,
                    ProcessingStatus::Indexed,
                    Some(inventory),
                ),
                Some(_) => (Readability::Unreadable, ProcessingStatus::Failed, None),
                None => (Readability::NotAssessed, ProcessingStatus::Discovered, None),
            }
        }
    })
}

/// Filesystem truth plus index truth, for one file.
fn record_for(
    file: &DiscoveredFile,
    indexed: Option<&DocumentIndexRow>,
    cache: &FileHashCache,
) -> FileRecord {
    let name = file
        .relative_path
        .rsplit('/')
        .next()
        .unwrap_or(&file.relative_path)
        .to_string();
    // From the name the filesystem reports, never from what the file says about itself.
    let (stem, extension) = split_name(&name);
    let kind = FileKind::from_extension(&extension);
    let sha256 = cache.hash_of(file);

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
        layout_caveats: Vec::new(),
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

    fn discovered(path: &std::path::Path, relative: &str) -> DiscoveredFile {
        let metadata = std::fs::metadata(path).expect("reads metadata");
        DiscoveredFile {
            relative_path: relative.to_string(),
            absolute_path: path.display().to_string(),
            extension: "txt".into(),
            size_bytes: metadata.len(),
            modified_at: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64),
        }
    }

    #[test]
    fn a_cached_hash_is_reused_while_size_and_modification_time_hold() {
        let root = folder_with(&[("note.txt", "Bonjour Camille")]);
        let path = root.path().join("note.txt");
        let file = discovered(&path, "note.txt");
        let cache = FileHashCache::new();

        let first = cache.hash_of(&file).expect("hashes");
        // The bytes change underneath, but the record the cache was given still claims the old
        // size and time - which is exactly the situation a cache hit must survive unchanged.
        std::fs::write(&path, b"Something else entirely").unwrap();
        let second = cache.hash_of(&file).expect("hashes");

        assert_eq!(first, second, "an unchanged size and time must not re-read");
    }

    #[test]
    fn a_changed_size_sends_the_cache_back_to_the_bytes() {
        let root = folder_with(&[("note.txt", "Bonjour")]);
        let path = root.path().join("note.txt");
        let cache = FileHashCache::new();
        let before = cache.hash_of(&discovered(&path, "note.txt")).expect("hashes");

        std::fs::write(&path, b"Bonjour Camille, bien plus long").unwrap();
        let after = cache.hash_of(&discovered(&path, "note.txt")).expect("hashes");

        assert_ne!(before, after);
    }

    #[test]
    fn forgetting_everything_makes_the_next_hash_a_real_read() {
        let root = folder_with(&[("note.txt", "Bonjour")]);
        let path = root.path().join("note.txt");
        let file = discovered(&path, "note.txt");
        let cache = FileHashCache::new();
        let before = cache.hash_of(&file).expect("hashes");

        std::fs::write(&path, b"Autre").unwrap();
        cache.forget_all();
        let after = cache.hash_of(&file).expect("hashes");

        assert_ne!(before, after);
    }

    #[test]
    fn an_unreadable_file_is_not_remembered_as_an_identity() {
        let root = folder_with(&[("note.txt", "Bonjour")]);
        let missing = root.path().join("absent.txt");
        let cache = FileHashCache::new();

        let file = DiscoveredFile {
            relative_path: "absent.txt".into(),
            absolute_path: missing.display().to_string(),
            extension: "txt".into(),
            size_bytes: 7,
            modified_at: Some(1),
        };

        assert_eq!(cache.hash_of(&file), None);
        std::fs::write(&missing, b"Bonjour").unwrap();
        assert!(
            cache.hash_of(&file).is_some(),
            "a file that appears later must be read, not remembered as unreadable"
        );
    }

    #[test]
    fn a_folder_never_analysed_has_every_document_waiting() {
        let root = folder_with(&[
            ("inbox/letter.txt", "Bonjour"),
            ("inbox/note.md", "Note"),
            ("inbox/planning.csv", "a,b"),
        ]);

        let inventory = WorkFolderInventory::discover(root.path(), None).expect("discovers");

        // The spreadsheet is not a document this pipeline owns, so analysing again would not
        // change it and it must not be counted as waiting.
        assert_eq!(inventory.unanalysed_documents(), 2);
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

    #[test]
    fn the_summary_counts_a_document_as_analysed_only_while_it_is_current() {
        let root = folder_with(&[("a.txt", "one"), ("b.txt", "two")]);
        let app = tempfile::tempdir().unwrap();
        let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
        let cache = FileHashCache::new();
        let chunk = |path: &str| crate::chunking::Chunk {
            chunk_id: format!("{path}#p1#s1"),
            relative_path: path.to_string(),
            page_number: 1,
            section: 1,
            text: "text".into(),
            index_text: None,
            origin: crate::extraction::PageOrigin::TextLayer,
            confidence: None,
        };
        for (name, content) in [("a.txt", "one"), ("b.txt", "two")] {
            let sha256 = hash_file(&root.path().join(name)).unwrap();
            assert!(!sha256.is_empty() && !content.is_empty());
            index
                .replace_document(
                    name,
                    &sha256,
                    false,
                    &[chunk(name)],
                    &[vec![0.5]],
                    None,
                    None,
                )
                .unwrap();
        }
        let current =
            WorkFolderInventory::discover_with_cache(root.path(), Some(&index), &cache).unwrap();
        assert_eq!(current.summary().indexed_files, 2);

        // b.txt changes after it was read: still indexed, no longer current.
        std::fs::write(root.path().join("b.txt"), "two, changed").unwrap();
        let after = WorkFolderInventory::discover_with_cache(
            root.path(),
            Some(&index),
            &FileHashCache::new(),
        )
        .unwrap();
        assert_eq!(after.indexed_files().len(), 2, "both still have passages");
        assert_eq!(after.summary().indexed_files, 1, "only a.txt is current");
        assert_eq!(after.unanalysed_documents(), 1);
    }
}
