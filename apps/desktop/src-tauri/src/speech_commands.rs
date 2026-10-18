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
use crate::speech::service::{self, Purpose, SpeechService, PURPOSE_HEADER};
use crate::speech::status::{self, Probes, SpeechStatus};
use crate::speech::stt::SttResult;
use crate::speech::temp_audio;

/// What the speech module keeps between two commands: what was found on this machine, which is
/// nothing yet, and the transcription service, built without an engine until phase 3b.
#[derive(Default)]
pub struct SpeechState {
    probes: Mutex<Probes>,
    service: SpeechService,
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

    pub(crate) fn service(&self) -> &SpeechService {
        &self.service
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

/// Transcribes one clip. The body is the WAV and nothing else (a raw request body, not JSON); the
/// header `purpose` says whether it is a chat dictation or a note, and the language and the cap come
/// from the settings and the language table. The text only fills a text box: nothing is stored.
/// Without an engine this answers `speech_engine_missing`.
#[tauri::command]
pub async fn speech_transcribe(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
    state: State<'_, AppState>,
    speech: State<'_, SpeechState>,
) -> Result<SttResult, AppError> {
    let tauri::ipc::InvokeBody::Raw(wav) = request.body() else {
        return Err(AppError::SpeechAudioInvalid);
    };
    let purpose = Purpose::from_header(
        request
            .headers()
            .get(PURPOSE_HEADER)
            .and_then(|value| value.to_str().ok()),
    );
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Internal)?
        .clone();
    let plan = service::plan(&settings, purpose)?;
    let temp_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| AppError::Internal)?
        .join(temp_audio::TEMP_FOLDER_NAME);
    let result = speech.service().transcribe(wav, &plan, &temp_dir).await?;
    eprintln!("{}", service::timing_line(&result));
    Ok(result)
}

/// Stops the transcription in progress, if any. Returns whether there was one.
#[tauri::command]
pub fn speech_cancel(speech: State<'_, SpeechState>) -> bool {
    speech.service().cancel()
}
