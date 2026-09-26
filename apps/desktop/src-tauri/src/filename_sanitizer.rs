//! Clean file names, applied when the user presses Analyse.
//!
//! A French practice saves documents as "Ordonnance pour Esaie.pdf" with an accent on the i: spaces, accents, an apostrophe
//! now and then, and on a Mac the accent may be stored as two code points where Windows stores one.
//! Nothing downstream breaks on those characters as such, but a name that can be typed three ways
//! is a name a question cannot reliably match, and a path that differs between two machines is a
//! path the index cannot trust. So the names are made plain once, at the door: ASCII letters,
//! digits, `-`, `_` and `.` (`docs/DECISIONS.md`).
//!
//! **This renames the user's files without asking**, which is a deliberate exception to the rule
//! that a file action is a plan the user approves (`docs/DECISIONS.md`). What keeps it safe is what
//! it refuses to do: it never overwrites (a taken name gets a numeric suffix), never touches
//! content, never leaves the file's own folder, never renames a folder, and records every change
//! in a log so the original name can always be recovered.
//!
//! No `tauri::` import: this module takes a folder and a log path and knows nothing else.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::discovery;

/// The name a file should have. Idempotent: a clean name comes back unchanged.
///
/// Accents are removed rather than transliterated to anything else (an accented "Esaie" becomes "Esaie"), the
/// ligatures French keeps are spelled out, and every other character - spaces, apostrophes,
/// brackets, anything outside ASCII - becomes one `-`, with runs collapsed and the ends trimmed.
/// The extension is kept as it is. A stem that has nothing left becomes `document`.
pub fn sanitize_file_name(name: &str) -> String {
    let (stem, extension) = match name.rfind('.') {
        Some(position) if position > 0 => (&name[..position], &name[position..]),
        _ => (name, ""),
    };

    let mut clean = String::with_capacity(stem.len());
    for character in stem.nfd() {
        if is_combining_mark(character) {
            continue;
        }
        match character {
            '\u{153}' => clean.push_str("oe"),
            '\u{152}' => clean.push_str("OE"),
            '\u{e6}' => clean.push_str("ae"),
            '\u{c6}' => clean.push_str("AE"),
            '\u{df}' => clean.push_str("ss"),
            c if c.is_ascii_alphanumeric() || c == '_' || c == '.' => clean.push(c),
            _ => clean.push('-'),
        }
    }

    let mut collapsed = String::with_capacity(clean.len());
    for character in clean.chars() {
        if character == '-' && collapsed.ends_with('-') {
            continue;
        }
        collapsed.push(character);
    }
    let trimmed = collapsed.trim_matches(|c| c == '-' || c == '.');
    let stem = if trimmed.is_empty() { "document" } else { trimmed };
    format!("{stem}{extension}")
}

/// A file that was renamed. Relative to the work folder, forward-slash separated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Renamed {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Default)]
pub struct SanitizeReport {
    pub renamed: Vec<Renamed>,
    /// Files that needed a new name and could not have one: open in another program, read-only,
    /// or on a volume that refused. Left exactly as they were, and named so nothing is silent.
    pub failed: Vec<String>,
}

/// Rename every document in `work_folder` whose name is not clean.
///
/// Only files the pipeline reads are considered (`discovery::discover`), so an unrelated file in
/// the folder keeps its name. Hidden files and Office lock files (`~$...`) are left alone: the
/// second is Word's own bookkeeping and renaming it would confuse Word. Folder names are never
/// changed.
pub fn sanitize_folder(work_folder: &Path) -> SanitizeReport {
    let mut report = SanitizeReport::default();
    // Names already in use, per folder and lowercase, because Windows and a default Mac disk both
    // treat `A.pdf` and `a.pdf` as one file. Seeded from the real directory, not just from the
    // documents, so a clean name never lands on a file this module does not manage.
    let mut taken: std::collections::HashMap<PathBuf, HashSet<String>> = Default::default();

    for file in discovery::discover(work_folder) {
        let source = PathBuf::from(&file.absolute_path);
        let Some(parent) = source.parent() else {
            continue;
        };
        let Some(name) = source.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        if name.starts_with('.') || name.starts_with("~$") {
            continue;
        }
        let wanted = sanitize_file_name(&name);
        if wanted == name {
            continue;
        }

        let names = taken
            .entry(parent.to_path_buf())
            .or_insert_with(|| directory_names(parent));
        let target_name = free_name(&wanted, names);
        let target = parent.join(&target_name);

        match std::fs::rename(&source, &target) {
            Ok(()) => {
                names.remove(&name.to_lowercase());
                names.insert(target_name.to_lowercase());
                report.renamed.push(Renamed {
                    to: replace_last_segment(&file.relative_path, &target_name),
                    from: file.relative_path,
                });
            }
            Err(_) => report.failed.push(file.relative_path),
        }
    }
    report
}

fn directory_names(directory: &Path) -> HashSet<String> {
    std::fs::read_dir(directory)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().to_lowercase())
                .collect()
        })
        .unwrap_or_default()
}

/// `wanted`, or `wanted` with `-2`, `-3`, ... before the extension until nothing else has it.
fn free_name(wanted: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(&wanted.to_lowercase()) {
        return wanted.to_string();
    }
    let (stem, extension) = match wanted.rfind('.') {
        Some(position) if position > 0 => (&wanted[..position], &wanted[position..]),
        _ => (wanted, ""),
    };
    (2..)
        .map(|n| format!("{stem}-{n}{extension}"))
        .find(|candidate| !taken.contains(&candidate.to_lowercase()))
        .expect("an unbounded counter always finds a free name")
}

fn replace_last_segment(relative_path: &str, name: &str) -> String {
    match relative_path.rfind('/') {
        Some(position) => format!("{}/{name}", &relative_path[..position]),
        None => name.to_string(),
    }
}

/// Append what was renamed to a JSON-lines log, one object per line, so the original name of any
/// file can be found again. Names only, never document text. A log that cannot be written is not
/// a reason to undo a rename that already happened, so the caller may ignore the error.
pub fn append_log(log_path: &Path, renamed: &[Renamed], at_seconds: i64) -> std::io::Result<()> {
    if renamed.is_empty() {
        return Ok(());
    }
    if let Some(directory) = log_path.parent() {
        std::fs::create_dir_all(directory)?;
    }
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    for entry in renamed {
        let line = serde_json::json!({ "at": at_seconds, "from": entry.from, "to": entry.to });
        writeln!(log, "{line}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaces_and_accents_are_made_plain() {
        assert_eq!(
            sanitize_file_name("Ordonnance pour Esa\u{ef}e.pdf"),
            "Ordonnance-pour-Esaie.pdf"
        );
        assert_eq!(sanitize_file_name("Bilan sanguin \u{e9}t\u{e9}.docx"), "Bilan-sanguin-ete.docx");
    }

    #[test]
    fn a_clean_name_is_left_alone() {
        for name in [
            "2026-03-02_document-sans-intitule.txt",
            "Absence-pour-Esaie.pdf",
            "v1.2_final.md",
        ] {
            assert_eq!(sanitize_file_name(name), name);
        }
    }

    #[test]
    fn sanitising_twice_changes_nothing() {
        for name in ["L'\u{153}uvre  de  \u{c7}a (copie 2).pdf", "  .pdf", "\u{e9}\u{e9}.png", "a - b.txt"] {
            let once = sanitize_file_name(name);
            assert_eq!(sanitize_file_name(&once), once, "{name}");
        }
    }

    #[test]
    fn both_spellings_of_an_accent_give_the_same_name() {
        // One code point (Windows) and two (a Mac): the same word, and the same file name.
        assert_eq!(
            sanitize_file_name("Esa\u{ef}e.pdf"),
            sanitize_file_name("Esai\u{308}e.pdf")
        );
    }

    #[test]
    fn ligatures_apostrophes_and_symbols_are_handled() {
        assert_eq!(sanitize_file_name("L'\u{153}uvre.txt"), "L-oeuvre.txt");
        assert_eq!(sanitize_file_name("a & b (2).txt"), "a-b-2.txt");
    }

    #[test]
    fn a_name_with_nothing_left_gets_a_placeholder() {
        assert_eq!(sanitize_file_name("()[].pdf"), "document.pdf");
    }

    fn folder_with(names: &[&str]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for name in names {
            let path = root.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, name.as_bytes()).unwrap();
        }
        root
    }

    #[test]
    fn a_folder_is_renamed_in_place_and_reported() {
        let root = folder_with(&["Ordonnance pour Esa\u{ef}e.pdf", "inbox/Compte rendu.txt", "ok.txt"]);

        let report = sanitize_folder(root.path());

        assert!(root.path().join("Ordonnance-pour-Esaie.pdf").is_file());
        assert!(root.path().join("inbox/Compte-rendu.txt").is_file());
        assert!(root.path().join("ok.txt").is_file());
        let mut renamed = report.renamed.clone();
        renamed.sort_by(|a, b| a.from.cmp(&b.from));
        assert_eq!(
            renamed,
            vec![
                Renamed {
                    from: "Ordonnance pour Esa\u{ef}e.pdf".into(),
                    to: "Ordonnance-pour-Esaie.pdf".into()
                },
                Renamed {
                    from: "inbox/Compte rendu.txt".into(),
                    to: "inbox/Compte-rendu.txt".into()
                },
            ]
        );
        assert!(report.failed.is_empty());
    }

    #[test]
    fn content_is_untouched() {
        let root = folder_with(&["My report.txt"]);
        sanitize_folder(root.path());

        let body = std::fs::read(root.path().join("My-report.txt")).unwrap();
        assert_eq!(body, b"My report.txt");
    }

    #[test]
    fn an_existing_file_is_never_overwritten() {
        let root = folder_with(&["My report.txt", "My-report.txt"]);

        let report = sanitize_folder(root.path());

        assert_eq!(report.renamed.len(), 1);
        assert_eq!(report.renamed[0].to, "My-report-2.txt");
        assert_eq!(
            std::fs::read(root.path().join("My-report.txt")).unwrap(),
            b"My-report.txt"
        );
        assert_eq!(
            std::fs::read(root.path().join("My-report-2.txt")).unwrap(),
            b"My report.txt"
        );
    }

    #[test]
    fn two_names_that_clean_to_the_same_one_both_survive() {
        let root = folder_with(&["a b.txt", "a  b.txt"]);

        let report = sanitize_folder(root.path());

        let mut targets: Vec<_> = report.renamed.iter().map(|r| r.to.clone()).collect();
        targets.sort();
        assert_eq!(targets, vec!["a-b-2.txt", "a-b.txt"]);
    }

    #[test]
    fn unrelated_hidden_and_lock_files_keep_their_names() {
        let root = folder_with(&["notes perso.odt", ".cache file.txt", "~$Mon doc.docx"]);

        let report = sanitize_folder(root.path());

        assert!(report.renamed.is_empty());
        assert!(root.path().join("notes perso.odt").is_file());
        assert!(root.path().join(".cache file.txt").is_file());
        assert!(root.path().join("~$Mon doc.docx").is_file());
    }

    #[test]
    fn folder_names_are_never_changed() {
        let root = folder_with(&["Patient folder/My report.txt"]);

        sanitize_folder(root.path());

        assert!(root.path().join("Patient folder/My-report.txt").is_file());
    }

    #[test]
    fn a_second_pass_finds_nothing_to_do() {
        let root = folder_with(&["Ordonnance pour Esa\u{ef}e.pdf"]);
        sanitize_folder(root.path());

        assert!(sanitize_folder(root.path()).renamed.is_empty());
    }

    #[test]
    fn the_log_records_each_rename_and_appends() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("logs/renames.jsonl");
        let renamed = vec![Renamed {
            from: "a b.pdf".into(),
            to: "a-b.pdf".into(),
        }];

        append_log(&log, &renamed, 100).unwrap();
        append_log(&log, &renamed, 200).unwrap();

        let text = std::fs::read_to_string(&log).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["from"], "a b.pdf");
        assert_eq!(first["to"], "a-b.pdf");
        assert_eq!(first["at"], 100);
    }

    #[test]
    fn an_empty_report_writes_no_log() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("renames.jsonl");

        append_log(&log, &[], 1).unwrap();

        assert!(!log.exists());
    }
}
