//! The private folder where a clip lives for the length of one transcription (decision D4, "audio
//! never kept").
//!
//! The folder is under the application's local data directory, never in the documents folder. A
//! clip is a [`TempAudioFile`] guard: the file is removed when the guard is dropped, which covers
//! success, error, cancel, timeout and a panic. [`sweep`] clears what a crash left behind; it lists
//! this one small folder and nothing else, and the service runs it lazily at the first speech
//! command, never at launch.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::AppError;

/// Name of the folder under the application's local data directory.
pub const TEMP_FOLDER_NAME: &str = "speech-temp";
/// Every clip file starts with this, so the sweep touches only files this module created.
const FILE_PREFIX: &str = "clip-";
const FILE_EXTENSION: &str = "wav";

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A clip on disk. Dropping it removes the file.
#[derive(Debug)]
pub struct TempAudioFile {
    path: PathBuf,
}

impl TempAudioFile {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempAudioFile {
    fn drop(&mut self) {
        // Nothing to report: a file that is already gone is the goal, and one that cannot be removed
        // is swept at the next first speech command.
        let _ = fs::remove_file(&self.path);
    }
}

/// A name for a file the engine will create inside `directory`, with the guard that removes it. The
/// folder is made, the file is not: a worker that writes audio (text to speech) creates it new.
pub fn reserve(directory: &Path) -> Result<TempAudioFile, AppError> {
    fs::create_dir_all(directory).map_err(|_| AppError::TtsFailed)?;
    let name = format!(
        "{FILE_PREFIX}{}-{}.{FILE_EXTENSION}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    Ok(TempAudioFile {
        path: directory.join(name),
    })
}

/// Writes a clip as a new file in `directory`, creating the folder if needed. The name carries a
/// process id and a counter, so two clips never meet, and the file is created new, never opened over.
pub fn write(directory: &Path, bytes: &[u8]) -> Result<TempAudioFile, AppError> {
    fs::create_dir_all(directory).map_err(|_| AppError::SpeechEngineFailed)?;
    let name = format!(
        "{FILE_PREFIX}{}-{}.{FILE_EXTENSION}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let path = directory.join(name);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| AppError::SpeechEngineFailed)?;
    // The guard exists before the bytes are written, so a failed write leaves nothing behind.
    let guard = TempAudioFile { path };
    file.write_all(bytes)
        .map_err(|_| AppError::SpeechEngineFailed)?;
    Ok(guard)
}

/// Removes the clips a crash left in `directory` and returns how many. Lists only this folder, one
/// level, and only files named like a clip; a missing folder is nothing to do.
pub fn sweep(directory: &Path) -> usize {
    let Ok(entries) = fs::read_dir(directory) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_clip = path.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with(FILE_PREFIX) && name.ends_with(&format!(".{FILE_EXTENSION}"))
                });
        if is_clip && fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}
