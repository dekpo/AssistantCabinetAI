//! File discovery over the work folder.
//!
//! Extension-driven, generic across professions: nothing here assumes a GP inbox. Symbolic
//! links are refused rather than followed, because a link can point outside the allow-listed
//! work folder and this module has no opinion on where it leads.

use std::path::Path;

use serde::Serialize;
use walkdir::WalkDir;

/// Extensions this session's extractor understands. `.csv` and `.xlsx` are a later pipeline
/// (`docs/RETRIEVAL.md`), not this one, so they are deliberately absent here.
const SUPPORTED_EXTENSIONS: &[&str] = &["pdf", "docx", "txt", "md", "jpg", "jpeg", "png"];

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
        let Some(extension) = extension_of(path) else {
            continue;
        };
        if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
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
            extension,
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
    fn an_empty_folder_returns_nothing() {
        let root = tempfile::tempdir().expect("temp dir");

        assert!(discover(root.path()).is_empty());
    }
}
