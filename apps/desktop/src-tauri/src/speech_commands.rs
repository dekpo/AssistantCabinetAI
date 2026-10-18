//! The Tauri shell of the speech module: its own managed state and thin commands over the pure
//! functions of `speech/`. It touches neither `AppState`'s fields nor `commands.rs`, only
//! `AppState.settings`, which is public.
//!
//! Nothing here runs at launch: `SpeechState::new` builds empty values, and a command runs only
//! when the interface asks.

use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

use crate::commands::{work_folder_policy, AppState};
use crate::error::AppError;
use crate::speech::notes::{self, NoteDraft, NoteFile, SaveLog, Target};
use crate::speech::status::{self, Probes, SpeechStatus};

/// What the speech module keeps between two commands. Later phases add the running job and the
/// worker handles here; for now it holds what was found on this machine, which is nothing.
#[derive(Default)]
pub struct SpeechState {
    probes: Mutex<Probes>,
}

impl SpeechState {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn probes(&self) -> Result<Probes, AppError> {
        self.probes
            .lock()
            .map(|probes| *probes)
            .map_err(|_| AppError::Internal)
    }
}

/// Per feature (dictation, read-aloud, notes): can it run, and if not the machine code of the
/// first reason; plus the language table. Reads the settings and the probes, starts nothing.
#[tauri::command]
pub fn speech_status(
    state: State<'_, AppState>,
    speech: State<'_, SpeechState>,
) -> Result<SpeechStatus, AppError> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Internal)?
        .clone();
    Ok(status::compute(&settings, &speech.probes()?))
}

/// The documents folder and the notes subfolder name, read from the settings. Nothing the webview
/// sends decides where a note goes.
fn notes_destination(state: &AppState) -> Result<(String, String), AppError> {
    let settings = state.settings.lock().map_err(|_| AppError::Internal)?;
    let work_folder = settings
        .work_folder
        .clone()
        .filter(|folder| !folder.trim().is_empty())
        .ok_or(AppError::NotesFolderUnavailable)?;
    Ok((work_folder, settings.speech.notes_folder_name.clone()))
}

/// The files a save of these notes would write, with the names they would get. Writes nothing.
#[tauri::command]
pub fn notes_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    drafts: Vec<NoteDraft>,
) -> Result<Vec<NoteFile>, AppError> {
    let (work_folder, folder_name) = notes_destination(&state)?;
    let policy = work_folder_policy(&app);
    let target = Target {
        policy: &policy,
        work_folder: std::path::Path::new(&work_folder),
        folder_name: &folder_name,
    };
    notes::preview(&target, &drafts, chrono::Local::now().naive_local())
}

/// Writes the notes the user reviewed, as new files in the notes folder. The only place a note is
/// written, and only ever new files. Analysing them is the interface's call to the Analyse pass.
#[tauri::command]
pub fn notes_save(
    app: AppHandle,
    state: State<'_, AppState>,
    drafts: Vec<NoteDraft>,
) -> Result<Vec<NoteFile>, AppError> {
    let (work_folder, folder_name) = notes_destination(&state)?;
    let policy = work_folder_policy(&app);
    let target = Target {
        policy: &policy,
        work_folder: std::path::Path::new(&work_folder),
        folder_name: &folder_name,
    };
    let now = chrono::Local::now();
    let log_path = app
        .path()
        .app_local_data_dir()
        .ok()
        .map(|directory| directory.join(notes::LOG_FILE_NAME));
    let log = log_path.as_deref().map(|path| SaveLog {
        path,
        at_seconds: now.timestamp(),
    });
    notes::save(&target, &drafts, now.naive_local(), log.as_ref())
}
