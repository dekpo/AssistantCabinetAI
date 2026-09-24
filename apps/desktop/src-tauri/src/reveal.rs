//! Showing a folder in the system's own file manager.
//!
//! This module owns the platform concept "the file manager the person already uses", and is the
//! only place in the crate that names one. Everything above it asks for a folder to be shown and
//! never learns which program did it, so supporting a third platform is one more arm here rather
//! than a change anywhere else (`AGENTS.md`, `docs/SPRINT-2.5-ASSESSMENT.md` section O).
//!
//! The `#[cfg]` blocks are deliberately paired and adjacent: Windows and macOS are both mandatory
//! targets, and a reader has to be able to see at a glance that neither was forgotten. Nothing in
//! here is allowed to leak upwards - no capability such as extraction, OCR or retrieval may ever
//! contain a `#[cfg]` because of this file.
//!
//! The caller passes a folder it has already validated. This module does not consult the
//! work-folder allow-list: it is a door opener, not a door keeper, and the one command that uses
//! it reads the folder from settings rather than from the webview.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::AppError;

/// Open `path` in the platform's file manager.
///
/// Returns as soon as the manager has been asked, not when a window appears: the file manager is
/// a separate application with its own lifetime, and waiting on it would block a command for as
/// long as the person leaves that window open.
pub fn folder(path: &Path) -> Result<(), AppError> {
    if !path.is_dir() {
        return Err(AppError::WorkFolderNotADirectory {
            path: path.display().to_string(),
        });
    }
    spawn_file_manager(path)
}

/// Windows Explorer.
///
/// Spawned and deliberately never waited on, for two reasons. It detaches from the process that
/// started it, so its exit status describes the launcher rather than the window; and that status
/// is `1` even when the folder opens perfectly. Treating a non-zero status as failure here would
/// report an error over a window that is already on screen.
#[cfg(windows)]
fn spawn_file_manager(path: &Path) -> Result<(), AppError> {
    Command::new("explorer.exe")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|_| AppError::WorkFolderRevealFailed)
}

/// macOS Finder, through `open`, which is the documented way to hand a path to the desktop and
/// does not require Finder to be running already.
#[cfg(target_os = "macos")]
fn spawn_file_manager(path: &Path) -> Result<(), AppError> {
    Command::new("open")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|_| AppError::WorkFolderRevealFailed)
}

/// Anywhere else. The product targets Windows and macOS (`AGENTS.md`), and a platform with no
/// arm above gets an honest machine code rather than a guess at which file manager is installed.
/// The crate still compiles, so a Linux development machine can run the test suite.
#[cfg(not(any(windows, target_os = "macos")))]
fn spawn_file_manager(_path: &Path) -> Result<(), AppError> {
    Err(AppError::WorkFolderRevealFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_that_is_not_a_folder_is_refused_before_any_process_starts() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("letter.pdf");
        std::fs::write(&file, b"pdf").expect("writes");

        assert!(matches!(
            folder(&file),
            Err(AppError::WorkFolderNotADirectory { .. })
        ));
    }

    #[test]
    fn a_folder_that_does_not_exist_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");

        assert!(matches!(
            folder(&dir.path().join("absent")),
            Err(AppError::WorkFolderNotADirectory { .. })
        ));
    }
}
