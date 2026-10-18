//! Saving notes as files (`docs/SPEECH.md`, notes). No speech is involved: a note is text the user
//! typed or dictated, reviewed on screen, and saved by a click on the exact files shown.
//!
//! The rules follow the mail merge (`fill_plan`), which is the other place the product writes new
//! files after an approval:
//!
//! * **New files only.** A name that is taken gets `-2`, `-3`; nothing is ever overwritten, moved or
//!   deleted (the only removal is of a file this very call created, when a later write of the same
//!   save fails, so a save is whole or nothing).
//! * **Nothing the webview sends is trusted.** The destination is derived here: the documents folder
//!   is revalidated by the work folder policy, the subfolder name is cleaned once more, and a
//!   symbolic link or a path that leaves the documents folder is refused.
//! * **The exact text.** UTF-8 without a byte order mark, `\n` line ends, no header added.
//! * **A log without text.** One line per file: when, the relative path, the counts.
//!
//! Analysing the saved notes is not done here: the interface runs the existing Analyse pass.
//!
//! Every function takes plain arguments (a policy, a folder path, a clock), so the tests drive them
//! in temporary folders without any Tauri state.

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use super::settings::DEFAULT_NOTES_FOLDER_NAME;
use crate::error::AppError;
use crate::filename_sanitizer::sanitize_file_name;
use crate::work_folder::WorkFolderPolicy;

/// The most notes one save may write.
pub const MAX_NOTES: usize = 20;
/// The largest note, in bytes of UTF-8 (1 MiB).
pub const MAX_NOTE_BYTES: usize = 1024 * 1024;
/// The longest user name kept in a file name, in characters.
const MAX_NAME_CHARS: usize = 80;
/// Local time as it prefixes a file name: sortable, in the clean-name alphabet.
const TIMESTAMP_FORMAT: &str = "%Y-%m-%d_%H-%M-%S";
/// Name of the log in the application data folder.
pub const LOG_FILE_NAME: &str = "saved-notes.jsonl";

/// The two extensions a note may take.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteFormat {
    #[default]
    Md,
    Txt,
}

impl NoteFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Md => "md",
            Self::Txt => "txt",
        }
    }
}

/// One note as the interface sends it: what the user decided, never a path.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDraft {
    /// The name the user typed; may be empty (the file is then named by its time alone).
    #[serde(default)]
    pub name: String,
    pub text: String,
    #[serde(default)]
    pub format: NoteFormat,
}

/// A note file, as it would be written (preview) or was written (save).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteFile {
    /// Relative to the documents folder, forward-slash separated: the key the index uses.
    pub relative_path: String,
    pub file_name: String,
    pub bytes: usize,
    pub chars: usize,
    pub lines: usize,
}

/// Where notes go: the documents folder the user chose, the rules it must pass, and the name of the
/// subfolder (a setting, cleaned again here).
pub struct Target<'a> {
    pub policy: &'a WorkFolderPolicy,
    pub work_folder: &'a Path,
    pub folder_name: &'a str,
}

/// Where the log goes and the time to write in it.
pub struct SaveLog<'a> {
    pub path: &'a Path,
    pub at_seconds: i64,
}

/// A draft after checking: its final bytes and the file name it asks for.
struct Prepared {
    file_name: String,
    bytes: Vec<u8>,
    chars: usize,
    lines: usize,
}

// ---------------------------------------------------------------------------------------- names

/// The part of a file name that comes from the user, or an empty string for no name.
///
/// A name that holds nothing once cleaned (only punctuation, or characters with no ASCII form) is
/// refused rather than silently replaced by a generic word. The test for "nothing left" appends a
/// letter: the sanitiser trims separators at the ends, so the letter comes back alone exactly when
/// the typed part contributed nothing.
fn clean_user_name(typed: &str) -> Result<String, AppError> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Ok(String::new());
    }
    if sanitize_file_name(&format!("{typed}a.x")) == "a.x" {
        return Err(AppError::NotesNameInvalid);
    }
    let cleaned = sanitize_file_name(&format!("{typed}.x"));
    let stem = cleaned.strip_suffix(".x").unwrap_or(&cleaned);
    // A dot inside a name would read as an extension; the extension is chosen by the format.
    let mut name = String::new();
    for character in stem.chars().take(MAX_NAME_CHARS) {
        let character = if character == '.' { '-' } else { character };
        if character == '-' && name.ends_with('-') {
            continue;
        }
        name.push(character);
    }
    let name = name.trim_matches('-');
    if name.is_empty() {
        return Err(AppError::NotesNameInvalid);
    }
    Ok(name.to_string())
}

/// `<timestamp>` or `<timestamp>-<name>`, then the extension.
fn file_name_of(now: NaiveDateTime, name: &str, format: NoteFormat) -> String {
    let stamp = now.format(TIMESTAMP_FORMAT).to_string();
    let base = if name.is_empty() {
        stamp
    } else {
        format!("{stamp}-{name}")
    };
    format!("{base}.{}", format.extension())
}

/// The text exactly as it will be on disk: no byte order mark, `\n` line ends.
fn normalise(text: &str) -> String {
    text.trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

/// Every draft checked before anything is written, so a refusal leaves the folder untouched.
fn prepare(drafts: &[NoteDraft], now: NaiveDateTime) -> Result<Vec<Prepared>, AppError> {
    if drafts.len() > MAX_NOTES {
        return Err(AppError::NotesTooMany);
    }
    drafts
        .iter()
        .map(|draft| {
            let name = clean_user_name(&draft.name)?;
            let text = normalise(&draft.text);
            if text.len() > MAX_NOTE_BYTES {
                return Err(AppError::NotesTooLarge);
            }
            Ok(Prepared {
                file_name: file_name_of(now, &name, draft.format),
                chars: text.chars().count(),
                lines: text.lines().count(),
                bytes: text.into_bytes(),
            })
        })
        .collect()
}

/// The `attempt`-th candidate for `name`: the name itself, then `stem-2.ext`, `stem-3.ext`.
fn candidate(name: &str, attempt: usize) -> String {
    if attempt <= 1 {
        return name.to_string();
    }
    let (stem, extension) = match name.rfind('.') {
        Some(position) if position > 0 => (&name[..position], &name[position..]),
        _ => (name, ""),
    };
    format!("{stem}-{attempt}{extension}")
}

// ------------------------------------------------------------------------------------ the folder

/// The notes folder, revalidated. With `create` off a missing folder is not an error and is not
/// created (a preview writes nothing, not even a folder). The second value is the cleaned name of
/// the subfolder.
fn locate(target: &Target, create: bool) -> Result<(PathBuf, String), AppError> {
    fn unavailable<E>(_: E) -> AppError {
        AppError::NotesFolderUnavailable
    }
    let root = target
        .policy
        .validate(target.work_folder)
        .map_err(unavailable)?;
    let folder =
        crate::settings::clean_folder_name_or(target.folder_name, DEFAULT_NOTES_FOLDER_NAME);
    let directory = root.join(&folder);

    let mut metadata = std::fs::symlink_metadata(&directory);
    if create && matches!(&metadata, Err(error) if error.kind() == std::io::ErrorKind::NotFound) {
        match std::fs::create_dir(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(AppError::NotesFolderUnavailable),
        }
        metadata = std::fs::symlink_metadata(&directory);
    }
    match metadata {
        Ok(found) if found.file_type().is_symlink() || !found.is_dir() => {
            Err(AppError::NotesFolderUnavailable)
        }
        Ok(_) => {
            // Resolved and checked again: the subfolder must still sit inside the documents folder
            // and pass the same rules.
            let resolved = dunce::canonicalize(&directory).map_err(unavailable)?;
            if !resolved.starts_with(&root) {
                return Err(AppError::NotesFolderUnavailable);
            }
            target.policy.check_path(&resolved).map_err(unavailable)?;
            // The name as the disk spells it, so the relative path matches the index key even when
            // the folder was created earlier with another capitalisation.
            let folder = resolved
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or(folder);
            Ok((resolved, folder))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => {
            Ok((directory, folder))
        }
        Err(_) => Err(AppError::NotesFolderUnavailable),
    }
}

// ------------------------------------------------------------------------------------- preview

/// The files a save would write, with the names they would get now. Writes nothing.
pub fn preview(
    target: &Target,
    drafts: &[NoteDraft],
    now: NaiveDateTime,
) -> Result<Vec<NoteFile>, AppError> {
    let prepared = prepare(drafts, now)?;
    let (directory, folder) = locate(target, false)?;
    let mut reserved: Vec<String> = Vec::new();
    let mut files = Vec::with_capacity(prepared.len());
    for note in prepared {
        let mut attempt = 1;
        let name = loop {
            let name = candidate(&note.file_name, attempt);
            let taken = reserved.contains(&name.to_lowercase())
                || std::fs::symlink_metadata(directory.join(&name)).is_ok();
            if !taken {
                break name;
            }
            attempt += 1;
        };
        reserved.push(name.to_lowercase());
        files.push(NoteFile {
            relative_path: format!("{folder}/{name}"),
            file_name: name,
            bytes: note.bytes.len(),
            chars: note.chars,
            lines: note.lines,
        });
    }
    Ok(files)
}

// --------------------------------------------------------------------------------------- save

/// A name not yet taken in `directory`, and the file created under it. `create_new` makes the check
/// and the creation one step, so a name taken between the two is never overwritten.
fn create_unique(directory: &Path, name: &str) -> std::io::Result<(PathBuf, std::fs::File)> {
    for attempt in 1..=10_000 {
        let path = directory.join(candidate(name, attempt));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "no free name",
    ))
}

/// Removes the files this save created. Only ever paths returned by `create_unique` in this call.
pub(super) fn remove_created(created: &[PathBuf]) {
    for path in created {
        let _ = std::fs::remove_file(path);
    }
}

/// Writes the notes the user reviewed, as new files in the notes folder. All or nothing: when one
/// write fails, the files this call already created are removed and `notes_write_failed` is
/// returned. A refusal before the first write (too many, too large, a bad name, an unusable folder)
/// leaves the disk as it was.
pub fn save(
    target: &Target,
    drafts: &[NoteDraft],
    now: NaiveDateTime,
    log: Option<&SaveLog>,
) -> Result<Vec<NoteFile>, AppError> {
    let prepared = prepare(drafts, now)?;
    if prepared.is_empty() {
        return Ok(Vec::new());
    }
    let (directory, folder) = locate(target, true)?;

    let mut created: Vec<PathBuf> = Vec::new();
    let mut files = Vec::with_capacity(prepared.len());
    for note in &prepared {
        let Ok((path, mut file)) = create_unique(&directory, &note.file_name) else {
            remove_created(&created);
            return Err(AppError::NotesWriteFailed);
        };
        created.push(path.clone());
        if file
            .write_all(&note.bytes)
            .and_then(|()| file.sync_all())
            .is_err()
        {
            drop(file);
            remove_created(&created);
            return Err(AppError::NotesWriteFailed);
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        files.push(NoteFile {
            relative_path: format!("{folder}/{name}"),
            file_name: name,
            bytes: note.bytes.len(),
            chars: note.chars,
            lines: note.lines,
        });
    }
    if let Some(log) = log {
        for file in &files {
            append_log(log, file);
        }
    }
    Ok(files)
}

/// One line per file written: when, the relative path and the counts. Never the text, never the
/// user's own name apart from what the path already carries. Best effort: a log that cannot be
/// written never undoes a note, which is already on disk.
fn append_log(log: &SaveLog, file: &NoteFile) {
    let line = serde_json::json!({
        "at": log.at_seconds,
        "output": file.relative_path,
        "bytes": file.bytes,
        "chars": file.chars,
        "lines": file.lines,
    });
    if let Some(parent) = log.path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut out) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log.path)
    {
        let _ = writeln!(out, "{line}");
    }
}
