//! File discovery over the work folder.
//!
//! Extension-driven, generic across professions: nothing here assumes a GP inbox. Symbolic
//! links are refused rather than followed, because a link can point outside the allow-listed
//! work folder and this module has no opinion on where it leads.

use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;
use walkdir::WalkDir;

use crate::file_key::{family_of_extension, key_under, physical_identity};
use crate::file_profile::build::Domain;

/// Extensions this session's extractor understands: the four it reads itself, the pictures it sends to OCR, and
/// the ones `readers` opens (subtitles, RTF, e-mail, OpenDocument, PowerPoint). Workbooks (`.csv`, `.xlsx`,
/// `.ods`) are the tabular pipeline's (`docs/RETRIEVAL.md`), not this one, so they are deliberately absent here.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "txt", "md", "jpg", "jpeg", "png", "vtt", "srt", "rtf", "eml", "odt", "odp", "pptx",
];

/// Whether the document pipeline can extract this extension. Lowercase, without the dot.
pub fn is_supported(extension: &str) -> bool {
    SUPPORTED_EXTENSIONS.contains(&extension)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredFile {
    /// Relative to the work folder, forward-slash separated so the value is stable across
    /// platforms and can be stored and compared without caring which machine indexed it.
    pub relative_path: String,
    pub absolute_path: String,
    pub extension: String,
    pub size_bytes: u64,
    pub modified_at: Option<i64>,
}

/// Walk `work_folder` and return every file whose extension we can extract, skipping symbolic
/// links (files and directories both) and anything unreadable rather than failing the whole
/// discovery over one bad entry.
pub fn discover(work_folder: &Path) -> Vec<DiscoveredFile> {
    let mut found = discover_all(work_folder);
    found.retain(|file| is_supported(&file.extension));
    found
}

/// Every regular file under `work_folder`, whatever its extension, in canonical relative-path
/// order. This is what the Work Folder inventory counts: the filesystem holds files, and only
/// some of them are documents this pipeline can read (`docs/WORK-FOLDER-INVENTORY.md`). The
/// same walk, the same symbolic-link refusal and the same relative paths as `discover`, so the
/// two can never disagree about what is in the folder.
pub fn discover_all(work_folder: &Path) -> Vec<DiscoveredFile> {
    let mut found = Vec::new();

    let walker = WalkDir::new(work_folder)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_symlink(entry.path()));

    for entry in walker.filter_map(Result::ok) {
        let path = entry.path();
        if is_symlink(path) || !path.is_file() {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Some(relative) = relative_slash_path(work_folder, path) else {
            continue;
        };
        found.push(DiscoveredFile {
            relative_path: relative,
            absolute_path: path.display().to_string(),
            extension: extension_of(path).unwrap_or_default(),
            size_bytes: metadata.len(),
            modified_at: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64),
        });
    }

    found.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    found
}

/// Every file of `family` in both folders: the family's own folder (`native`) and the other one
/// (`foreign`, when it is chosen). Each file is given its key (`file_key::key_under`, the one rule) as
/// `relative_path`; the absolute path stays the real one, so reading, hashing and extraction need no
/// change, and nothing is copied or moved. Same walk and same refusals as `discover_all`. A file reachable
/// from both folders (equal or nested folders) is listed **once**, whichever walk met it first: the list
/// is de-duplicated on the physical file before any key is written. What a search or a pass over one
/// family reads; a file no pipeline reads is in neither.
pub fn discover_family(
    native: &Path,
    foreign: Option<&Path>,
    family: Domain,
) -> Vec<DiscoveredFile> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for folder in std::iter::once(native).chain(foreign) {
        for file in discover_all(folder) {
            if family_of_extension(&file.extension) != Some(family) {
                continue;
            }
            let absolute = Path::new(&file.absolute_path);
            if !seen.insert(physical_identity(absolute)) {
                continue;
            }
            let Some(key) = key_under(family, Some(native), foreign, absolute) else {
                continue;
            };
            found.push(DiscoveredFile {
                relative_path: key,
                ..file
            });
        }
    }
    found.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    found
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn extension_of(path: &Path) -> Option<String> {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
}

fn relative_slash_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    Some(
        relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_supported_extensions_are_returned() {
        let root = tempfile::tempdir().expect("temp dir");
        std::fs::write(root.path().join("letter.pdf"), b"pdf").unwrap();
        std::fs::write(root.path().join("notes.docx"), b"docx").unwrap();
        std::fs::write(root.path().join("readme.txt"), b"txt").unwrap();
        std::fs::write(root.path().join("summary.md"), b"md").unwrap();
        std::fs::write(root.path().join("image.png"), b"png").unwrap();
        std::fs::write(root.path().join("photo.jpg"), b"jpg").unwrap();
        std::fs::write(root.path().join("data.csv"), b"csv").unwrap();

        let files = discover(root.path());
        let names: Vec<_> = files.iter().map(|f| f.relative_path.clone()).collect();

        assert_eq!(
            names,
            vec![
                "image.png",
                "letter.pdf",
                "notes.docx",
                "photo.jpg",
                "readme.txt",
                "summary.md"
            ]
        );
    }

    #[test]
    fn every_extension_a_reader_opens_is_discovered() {
        for extension in crate::readers::EXTENSIONS {
            assert!(is_supported(extension), "{extension}");
        }
    }

    #[test]
    fn nested_folders_are_walked_with_forward_slash_relative_paths() {
        let root = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(root.path().join("inbox")).unwrap();
        std::fs::write(root.path().join("inbox").join("letter.txt"), b"content").unwrap();

        let files = discover(root.path());

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "inbox/letter.txt");
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_is_refused_even_when_it_points_at_a_supported_extension() {
        let root = tempfile::tempdir().expect("temp dir");
        let target = root.path().join("real.txt");
        std::fs::write(&target, b"content").unwrap();
        let link = root.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let files = discover(root.path());
        let names: Vec<_> = files.iter().map(|f| f.relative_path.clone()).collect();

        assert_eq!(names, vec!["real.txt"]);
    }

    #[test]
    fn discover_all_keeps_every_file_including_the_ones_the_pipeline_cannot_read() {
        let root = tempfile::tempdir().expect("temp dir");
        std::fs::write(root.path().join("letter.pdf"), b"pdf").unwrap();
        std::fs::write(root.path().join("planning.csv"), b"csv").unwrap();
        std::fs::write(root.path().join("archive.zip"), b"zip").unwrap();
        std::fs::write(root.path().join("NOTICE"), b"no extension").unwrap();

        let every = discover_all(root.path());
        let names: Vec<_> = every.iter().map(|f| f.relative_path.clone()).collect();

        assert_eq!(names, vec!["NOTICE", "archive.zip", "letter.pdf", "planning.csv"]);
        assert_eq!(discover(root.path()).len(), 1);
    }

    #[test]
    fn files_of_the_other_folder_are_found_where_they_sit_and_keyed_by_their_folder() {
        let documents = tempfile::tempdir().expect("temp dir");
        let data = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(documents.path().join("Factures")).unwrap();
        std::fs::write(documents.path().join("Factures").join("2025.xlsx"), b"x").unwrap();
        std::fs::write(documents.path().join("letter.pdf"), b"pdf").unwrap();
        std::fs::write(documents.path().join("planning.csv"), b"csv").unwrap();
        std::fs::write(data.path().join("model.txt"), b"txt").unwrap();

        let tables = discover_family(data.path(), Some(documents.path()), Domain::Data);
        let keys: Vec<_> = tables.iter().map(|f| f.relative_path.clone()).collect();
        assert_eq!(
            keys,
            vec!["/documents/Factures/2025.xlsx", "/documents/planning.csv"]
        );
        // The real file is still where it was found.
        assert!(Path::new(&tables[0].absolute_path).is_file());

        let letters = discover_family(documents.path(), Some(data.path()), Domain::Documents);
        let keys: Vec<_> = letters.iter().map(|f| f.relative_path.clone()).collect();
        assert_eq!(keys, vec!["/data/model.txt", "letter.pdf"]);
    }

    #[test]
    fn a_file_reachable_from_both_folders_is_listed_once() {
        let root = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(root.path().join("Tables")).unwrap();
        std::fs::write(root.path().join("Tables").join("a.csv"), b"csv").unwrap();
        std::fs::write(root.path().join("Tables").join("a.pdf"), b"pdf").unwrap();
        std::fs::write(root.path().join("b.csv"), b"csv").unwrap();

        let tables = root.path().join("Tables");
        let keys = |native: &Path, foreign: &Path, family| -> Vec<String> {
            discover_family(native, Some(foreign), family)
                .into_iter()
                .map(|file| file.relative_path)
                .collect()
        };
        // The same folder twice.
        assert_eq!(
            keys(root.path(), root.path(), Domain::Data),
            vec!["Tables/a.csv", "b.csv"]
        );
        // Tables inside documents.
        assert_eq!(
            keys(&tables, root.path(), Domain::Data),
            vec!["/documents/b.csv", "a.csv"]
        );
        assert_eq!(
            keys(root.path(), &tables, Domain::Documents),
            vec!["Tables/a.pdf"]
        );
        // Documents inside tables.
        assert_eq!(
            keys(root.path(), &tables, Domain::Data),
            vec!["Tables/a.csv", "b.csv"]
        );
        assert_eq!(keys(&tables, root.path(), Domain::Documents), vec!["a.pdf"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_in_the_other_folder_is_refused_too() {
        let root = tempfile::tempdir().expect("temp dir");
        let own = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("temp dir");
        std::fs::write(outside.path().join("secret.xlsx"), b"x").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.xlsx"),
            root.path().join("link.xlsx"),
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("folder")).unwrap();

        assert!(discover_family(own.path(), Some(root.path()), Domain::Data).is_empty());
    }

    #[test]
    fn an_empty_folder_returns_nothing() {
        let root = tempfile::tempdir().expect("temp dir");

        assert!(discover(root.path()).is_empty());
    }
}
