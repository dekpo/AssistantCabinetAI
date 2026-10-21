//! Each family seen across both folders: what a question, a selection or a pass reads.
//!
//! The explorer lists **folders** (`WorkFolderInventory::discover_in`, one per root, paths inside the
//! root). The conversation reads **families**: the documents wherever they sit, the workbooks wherever
//! they sit (`docs/DECISIONS.md`, "root and family"). A family's view is its own folder's inventory,
//! unchanged, plus the files of that family found in the other folder, each carrying the key it has in
//! the index (`file_key`) instead of its path, so a selection, a retrieval or a table question meets one
//! list and never two files of the same path. Nothing is copied or moved: the other folder's files
//! are resolved against that folder, and only that one.

use std::path::{Path, PathBuf};

use crate::data_folder::DataFolder;
use crate::error::AppError;
use crate::file_key::{index_key, Root};
use crate::file_profile::build::Domain;
use crate::file_record::{FileKind, FileRecord};
use crate::index_store::IndexStore;
use crate::inventory::{FileHashCache, WorkFolderInventory};

/// The two families, each across both folders. A family is absent when its own folder is not chosen.
#[derive(Debug, Clone, Default)]
pub struct FamilyViews {
    pub documents: Option<WorkFolderInventory>,
    pub data: Option<DataFolder>,
}

/// `record`, as the family `family` sees it when it sits in `root` (not the family's own folder).
fn rekeyed(record: &FileRecord, root: Root, family: Domain) -> FileRecord {
    let mut record = record.clone();
    record.relative_path = index_key(root, family, &record.relative_path);
    record
}

/// Both families, across the folders that are chosen. Two walks (one per folder), whatever the number
/// of families asked for; the hash caches are one per folder.
pub fn family_views(
    documents_root: Option<&Path>,
    data_root: Option<&Path>,
    index: Option<&IndexStore>,
    documents_cache: &FileHashCache,
    data_cache: &FileHashCache,
) -> Result<FamilyViews, AppError> {
    let physical_documents = documents_root
        .map(|root| WorkFolderInventory::discover_in(Root::Documents, root, index, documents_cache))
        .transpose()?;
    let physical_data = data_root
        .map(|root| WorkFolderInventory::discover_in(Root::Data, root, index, data_cache))
        .transpose()?;

    let documents = physical_documents.as_ref().map(|(own, _)| {
        let mut files: Vec<FileRecord> = own.all_files().to_vec();
        if let Some((other, _)) = &physical_data {
            files.extend(
                other
                    .all_files()
                    .iter()
                    .filter(|record| record.kind.is_document())
                    .map(|record| rekeyed(record, Root::Data, Domain::Documents)),
            );
        }
        WorkFolderInventory::from_records(own.root(), files).with_foreign_root(
            physical_data
                .as_ref()
                .map(|(other, _)| other.root().to_path_buf()),
        )
    });

    let data = physical_data.as_ref().map(|(own, own_usable)| {
        let mut files: Vec<FileRecord> = own.all_files().to_vec();
        let mut usable = own_usable.clone();
        if let Some((other, other_usable)) = &physical_documents {
            for record in other
                .all_files()
                .iter()
                .filter(|record| record.kind == FileKind::TabularCandidate)
            {
                let foreign = rekeyed(record, Root::Documents, Domain::Data);
                if let Some(inventory) = other_usable.get(&record.relative_path) {
                    usable.insert(foreign.relative_path.clone(), inventory.clone());
                }
                files.push(foreign);
            }
        }
        DataFolder::from_parts(
            WorkFolderInventory::from_records(own.root(), files).with_foreign_root(
                physical_documents
                    .as_ref()
                    .map(|(other, _)| PathBuf::from(other.root())),
            ),
            usable,
        )
    });

    Ok(FamilyViews { documents, data })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_record::{ProcessingStatus, Readability};

    fn write(root: &Path, relative: &str, content: &[u8]) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    fn views(documents: &Path, data: &Path, index: Option<&IndexStore>) -> FamilyViews {
        family_views(
            Some(documents),
            Some(data),
            index,
            &FileHashCache::new(),
            &FileHashCache::new(),
        )
        .unwrap()
    }

    #[test]
    fn a_family_meets_the_other_folders_files_under_their_key() {
        let documents = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        write(documents.path(), "lettre.txt", b"bonjour");
        write(documents.path(), "suivi.csv", b"a;b\n1;2\n");
        write(data.path(), "tableau.csv", b"a;b\n1;2\n");
        write(data.path(), "note.txt", b"hello");

        let seen = views(documents.path(), data.path(), None);

        let documents_view = seen.documents.unwrap();
        let paths: Vec<_> = documents_view
            .all_files()
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect();
        assert!(paths.contains(&"lettre.txt"));
        assert!(paths.contains(&"/data/note.txt"));
        assert!(
            !paths.contains(&"/data/tableau.csv"),
            "a workbook is not a document"
        );

        let data_view = seen.data.unwrap();
        let paths: Vec<_> = data_view
            .files()
            .all_files()
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect();
        assert!(paths.contains(&"tableau.csv"));
        assert!(paths.contains(&"/documents/suivi.csv"));
    }

    #[test]
    fn a_key_resolves_to_the_file_in_the_folder_it_names() {
        let documents = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        write(data.path(), "Courriers/note.txt", b"hello");
        write(documents.path(), "suivi.csv", b"a;b\n1;2\n");

        let seen = views(documents.path(), data.path(), None);
        let documents_view = seen.documents.unwrap();
        let record = documents_view
            .find_by_relative_path("/data/Courriers/note.txt")
            .unwrap();
        assert_eq!(
            documents_view.absolute_path(record),
            Some(data.path().join("Courriers").join("note.txt"))
        );
        let data_view = seen.data.unwrap();
        let workbook = data_view
            .files()
            .find_by_relative_path("/documents/suivi.csv")
            .unwrap();
        assert_eq!(
            data_view.files().absolute_path(workbook),
            Some(documents.path().join("suivi.csv"))
        );
    }

    #[test]
    fn the_same_path_in_both_folders_stays_two_files() {
        let documents = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        write(documents.path(), "rapport.txt", b"one");
        write(data.path(), "rapport.txt", b"two");

        let documents_view = views(documents.path(), data.path(), None)
            .documents
            .unwrap();

        assert!(documents_view
            .find_by_relative_path("rapport.txt")
            .is_some());
        assert!(documents_view
            .find_by_relative_path("/data/rapport.txt")
            .is_some());
        assert_eq!(documents_view.file_count(), 2);
    }

    #[test]
    fn a_foreign_workbook_is_green_once_the_tabular_pass_has_read_it() {
        let documents = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let app = tempfile::tempdir().unwrap();
        let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
        let csv = crate::data_folder::tests::invoice_csv(12);
        write(documents.path(), "factures.csv", csv.as_bytes());

        let before = views(documents.path(), data.path(), Some(&index));
        let record = before
            .data
            .as_ref()
            .unwrap()
            .files()
            .find_by_relative_path("/documents/factures.csv")
            .unwrap()
            .clone();
        assert_eq!(
            (record.readability, record.processing_status),
            (Readability::NotAssessed, ProcessingStatus::Discovered)
        );

        crate::data_folder::analyse_cancellable(
            data.path(),
            Some(documents.path()),
            &mut index,
            "fr-FR",
            &|_| {},
            None,
            &|| false,
        )
        .unwrap();

        let after = views(documents.path(), data.path(), Some(&index));
        let data_view = after.data.unwrap();
        let record = data_view
            .files()
            .find_by_relative_path("/documents/factures.csv")
            .unwrap();
        assert_eq!(
            (record.readability, record.processing_status),
            (Readability::Readable, ProcessingStatus::Indexed)
        );
        assert!(data_view
            .usable_inventory("/documents/factures.csv")
            .is_some());
        // The physical Documents folder says the same about the same file.
        let (physical, _) = WorkFolderInventory::discover_in(
            Root::Documents,
            documents.path(),
            Some(&index),
            &FileHashCache::new(),
        )
        .unwrap();
        let record = physical.find_by_relative_path("factures.csv").unwrap();
        assert_eq!(record.processing_status, ProcessingStatus::Indexed);
        // Nothing was written next to the workbook, and the Data folder gained no file.
        assert_eq!(std::fs::read_dir(documents.path()).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(data.path()).unwrap().count(), 0);
    }
}
