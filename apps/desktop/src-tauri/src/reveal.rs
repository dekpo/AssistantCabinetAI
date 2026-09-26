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
//! work-folder allow-list: it is a door opener, not a door keeper, and the command that shows the
//! folder reads it from settings rather than from the webview.
//!
//! Showing one file is the exception, because the webview names the file. So `file` takes the work
//! folder and a path relative to it, and refuses anything that could point elsewhere before any
//! process starts.

use std::path::{Component, Path, PathBuf};
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

/// Show one file of the work folder in the platform's file manager, selected in its folder.
///
/// The file is only pointed at, never opened: nothing is executed, read or changed, and she decides
/// what to do with it from the file manager she already uses. `relative` comes from the webview, so
/// it is treated as untrusted: it must stay inside `root`, and any failure is the same machine code
/// as a file manager that would not start, because to her the outcome is the same.
pub fn file(root: &Path, relative: &str) -> Result<(), AppError> {
    let target = resolve_inside(root, relative)?;
    spawn_file_manager_selecting(&target)
}

/// The file `relative` names under `root`, or a refusal. Rebuilt component by component, which
/// both rejects `..`, drive letters and absolute paths and gives Windows the separators Explorer
/// expects, whichever ones the inventory used.
fn resolve_inside(root: &Path, relative: &str) -> Result<PathBuf, AppError> {
    let mut target = root.to_path_buf();
    let mut any = false;
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => {
                target.push(part);
                any = true;
            }
            _ => return Err(AppError::WorkFolderRevealFailed),
        }
    }
    // A link is refused, not followed: it could lead out of the folder (`discovery`).
    let is_plain_file = std::fs::symlink_metadata(&target)
        .map(|metadata| metadata.is_file())
        .unwrap_or(false);
    if !any || !is_plain_file {
        return Err(AppError::WorkFolderRevealFailed);
    }
    Ok(target)
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

/// Explorer opened on the folder with the file selected. The argument is passed raw because
/// Explorer parses `/select,` itself and mishandles the quoting Rust would add around a path with
/// spaces; a Windows file name cannot contain a quote, so quoting it here is safe.
#[cfg(windows)]
fn spawn_file_manager_selecting(path: &Path) -> Result<(), AppError> {
    use std::os::windows::process::CommandExt;

    Command::new("explorer.exe")
        .raw_arg(format!("/select,\"{}\"", path.display()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|_| AppError::WorkFolderRevealFailed)
}

/// Finder with the file selected: `open -R` reveals rather than opens.
#[cfg(target_os = "macos")]
fn spawn_file_manager_selecting(path: &Path) -> Result<(), AppError> {
    Command::new("open")
        .arg("-R")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|_| AppError::WorkFolderRevealFailed)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn spawn_file_manager_selecting(_path: &Path) -> Result<(), AppError> {
    Err(AppError::WorkFolderRevealFailed)
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
    fn a_file_inside_the_folder_resolves_whichever_separator_the_inventory_used() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("inbox")).expect("creates");
        std::fs::write(dir.path().join("inbox").join("letter.pdf"), b"pdf").expect("writes");

        let resolved = resolve_inside(dir.path(), "inbox/letter.pdf").expect("resolves");

        assert_eq!(resolved, dir.path().join("inbox").join("letter.pdf"));
    }

    #[test]
    fn a_path_that_could_leave_the_folder_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("temp dir");
        std::fs::write(outside.path().join("secret.txt"), b"x").expect("writes");
        let absolute = outside.path().join("secret.txt").display().to_string();

        for relative in [
            "../secret.txt",
            "inbox/../../secret.txt",
            absolute.as_str(),
            "",
            ".",
        ] {
            assert!(
                matches!(
                    resolve_inside(dir.path(), relative),
                    Err(AppError::WorkFolderRevealFailed)
                ),
                "{relative}"
            );
        }
    }

    #[test]
    fn a_file_that_is_absent_or_a_folder_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("inbox")).expect("creates");

        assert!(resolve_inside(dir.path(), "absent.pdf").is_err());
        assert!(resolve_inside(dir.path(), "inbox").is_err());
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
