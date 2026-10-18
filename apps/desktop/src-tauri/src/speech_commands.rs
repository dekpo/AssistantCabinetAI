//! The Tauri shell of the speech module: its own managed state and thin commands over the pure
//! functions of `speech/`. It touches neither `AppState`'s fields nor `commands.rs`, only
//! `AppState.settings`, which is public.
//!
//! Nothing here runs at launch: `SpeechState::new` builds empty values (no worker is started, no
//! file is read), and a command runs only when the interface asks.

use std::sync::{Arc, Mutex};

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::commands::{search_roots, work_folder_policy, AppState};
use crate::error::AppError;
use crate::speech::model_download::{self, Progress, Sink};
use crate::speech::models::{self, JobSlot};
use crate::speech::notes::{self, NoteDraft, NoteFile, SaveLog, Target};
use crate::speech::service::{self, Purpose, SpeechService, PURPOSE_HEADER};
use crate::speech::speakable::{self, Part, Segment};
use crate::speech::status::{self, Probes, SpeechStatus};
use crate::speech::stt::SttResult;
use crate::speech::temp_audio;
use crate::speech::worker_stt::{self, WorkerSttProvider};

/// What the speech module keeps between two commands: what was found on this machine, the worker
/// provider (a process that exists only after the first dictation) behind the transcription service,
/// and the slot of the one install or verify that may run.
pub struct SpeechState {
    probes: Mutex<Probes>,
    worker: Option<Arc<WorkerSttProvider>>,
    service: SpeechService,
    model_job: JobSlot,
}

impl Default for SpeechState {
    fn default() -> Self {
        // The manifest is compiled in and a test proves it parses, so `None` is not expected; the
        // state then simply has no engine and every job answers `speech_engine_missing`.
        let worker = models::stt_model().map(|spec| Arc::new(WorkerSttProvider::new(spec.clone())));
        let service = match &worker {
            Some(provider) => SpeechService::with_provider(provider.clone()),
            None => SpeechService::default(),
        };
        Self {
            probes: Mutex::new(Probes::default()),
            worker,
            service,
            model_job: JobSlot::default(),
        }
    }
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

    /// Finds the worker and the model folder on this machine, tells the provider, and records what
    /// was found. A handful of `stat` calls; no hashing and no process.
    pub(crate) fn refresh(&self, app: &AppHandle) -> Result<Probes, AppError> {
        let (Some(worker), Some(spec)) = (&self.worker, models::stt_model()) else {
            return self.probes();
        };
        let local = app
            .path()
            .app_local_data_dir()
            .map_err(|_| AppError::Internal)?;
        worker.configure(worker_stt::setup_for(&search_roots(app), &local, spec));
        let found = Probes {
            stt_engine: worker.engine_found(),
            stt_model: worker.model_state(),
            tts_voice: false,
        };
        *self.probes.lock().map_err(|_| AppError::Internal)? = found;
        Ok(found)
    }
}

/// Per feature (dictation, read-aloud, notes): can it run, and if not the machine code of the
/// first reason; plus the language table. Reads the settings and looks at the disk for the worker and
/// the model files; starts nothing.
#[tauri::command]
pub fn speech_status(
    app: AppHandle,
    state: State<'_, AppState>,
    speech: State<'_, SpeechState>,
) -> Result<SpeechStatus, AppError> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Internal)?
        .clone();
    Ok(status::compute(&settings, &speech.refresh(&app)?))
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
/// Without a worker this answers `speech_engine_missing`; the worker starts here, on the first call.
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
    if speech.model_job.is_running() {
        return Err(AppError::SpeechBusy);
    }
    speech.refresh(&app)?;
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

/// Where the models live and the address they come from, read from Rust state and settings: the
/// webview names neither.
fn model_places(
    app: &AppHandle,
    state: &AppState,
) -> Result<(std::path::PathBuf, String), AppError> {
    let local = app
        .path()
        .app_local_data_dir()
        .map_err(|_| AppError::Internal)?;
    let address = state
        .settings
        .lock()
        .map_err(|_| AppError::Internal)?
        .speech
        .speech_models_url
        .clone();
    Ok((worker_stt::models_root(&local), address))
}

/// Downloads and installs the speech model. Only ever called by the Install button: nothing starts
/// it by itself. Progress goes to the channel; `speech_model_cancel` stops it.
#[tauri::command]
pub async fn speech_model_install(
    app: AppHandle,
    state: State<'_, AppState>,
    speech: State<'_, SpeechState>,
    on_progress: Channel<Progress>,
) -> Result<(), AppError> {
    let spec = models::stt_model().ok_or(AppError::Internal)?;
    let (root, address) = model_places(&app, &state)?;
    if speech.service().is_busy() {
        return Err(AppError::SpeechBusy);
    }
    let job = speech.model_job.begin()?;
    // A loaded model keeps its files open, and Windows will not replace a file that is open.
    if let Some(worker) = &speech.worker {
        worker.shutdown();
    }
    let sink: Sink = Arc::new(move |progress| {
        let _ = on_progress.send(progress);
    });
    model_download::install(spec, &address, &root, job.cancel_token(), &sink).await?;
    speech.refresh(&app)?;
    Ok(())
}

/// Hashes the files in the model folder against the manifest and, if they match, marks the model
/// verified. This is also how a model copied by hand into the folder becomes usable.
#[tauri::command]
pub async fn speech_model_verify(
    app: AppHandle,
    state: State<'_, AppState>,
    speech: State<'_, SpeechState>,
) -> Result<(), AppError> {
    let spec = models::stt_model().ok_or(AppError::Internal)?;
    let (root, _) = model_places(&app, &state)?;
    if speech.service().is_busy() {
        return Err(AppError::SpeechBusy);
    }
    let job = speech.model_job.begin()?;
    if let Some(worker) = &speech.worker {
        worker.shutdown();
    }
    let folder = spec.folder(&root);
    let token = job.cancel_token().clone();
    tauri::async_runtime::spawn_blocking(move || models::verify(spec, &folder, &token))
        .await
        .map_err(|_| AppError::Internal)??;
    speech.refresh(&app)?;
    Ok(())
}

/// Stops the install or the verify in progress, if any. Returns whether there was one.
#[tauri::command]
pub fn speech_model_cancel(speech: State<'_, SpeechState>) -> bool {
    speech.model_job.cancel()
}

/// The segments a voice speaks for these parts of an answer, and the notes that stand for what is
/// not read. Derived here, by code, from the text the interface sends; no model, no engine and no
/// network. Reads the settings only to refuse when read-aloud is off. The optional step that says
/// the names of the selection as the knowledge base spells them is not part of this command.
#[tauri::command]
pub fn speech_prepare_reading(
    state: State<'_, AppState>,
    parts: Vec<Part>,
    locale: String,
) -> Result<Vec<Segment>, AppError> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| AppError::Internal)?
        .clone();
    speakable::reading_allowed(&settings)?;
    let packs = settings.knowledge_pack_ids();
    let ids: Vec<&str> = packs.iter().map(String::as_str).collect();
    speakable::prepare_for_locale(&parts, &locale, &ids)
}
