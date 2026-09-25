//! The bridge the webview is allowed to use.
//!
//! Each command is a narrow door: read the settings, store validated settings, pick a folder
//! through the system dialog, ask the gateway how it is, send one conversation. The address, the
//! alias, the locale and the allow-list stay on this side of the door.

use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::analysis_scope::AnalysisScope;
use crate::cancellation::{until_stopped, Cancellation};
use crate::error::AppError;
use crate::file_record::FileRecord;
use crate::folder_questions::{self, FolderAnswer, QuestionRoute};
use crate::gateway::{ChatTurn, GatewayClient, HealthSnapshot};
use crate::index_store::IndexStore;
use crate::indexing::{self, IndexProgress, IndexSummary};
use crate::inventory::{FileHashCache, FolderNode, InventorySummary, WorkFolderInventory};
use crate::ocr::tesseract::TesseractProvider;
use crate::ocr::OcrProvider;
use crate::raster::{self, PageRasterizer, Rasterizer};
use crate::retrieval::{self, Evidence, EvidenceCoverage, RetrievalScope};
use crate::reveal;
use crate::settings::{self, Settings};
use crate::work_folder::{self, display, suggested_work_folder, WorkFolderPolicy};
use crate::work_folder_context::{self, ContextView};

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub gateway: GatewayClient,
    /// Lets `cancel_chat` reach the question being worked on. One at a time.
    pub cancellation: Cancellation,
    /// What each file in the work folder last hashed to. Lives as long as the application so the
    /// folder panel can be rebuilt whenever she comes back to the window without re-reading every
    /// scan in the folder (`inventory::FileHashCache`).
    pub file_hashes: FileHashCache,
}

impl AppState {
    pub fn new() -> Result<Self, AppError> {
        Ok(Self {
            settings: Mutex::new(Settings::default()),
            gateway: GatewayClient::new()?,
            cancellation: Cancellation::default(),
            file_hashes: FileHashCache::new(),
        })
    }

    fn read<T>(&self, extract: impl FnOnce(&Settings) -> T) -> Result<T, AppError> {
        let guard = self.settings.lock().map_err(|_| AppError::Internal)?;
        Ok(extract(&guard))
    }

    fn replace(&self, settings: Settings) -> Result<(), AppError> {
        *self.settings.lock().map_err(|_| AppError::Internal)? = settings;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    settings: Settings,
    system_locale: String,
    settings_path: String,
    /// What to propose when no folder has been chosen: `~/AssistantCabinetAI`, outside Documents
    /// so that no cloud client mirrors it.
    suggested_work_folder: Option<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ChatStreamEvent {
    Delta {
        text: String,
    },
    Completed {
        text: String,
    },
    Sources {
        sources: Vec<Evidence>,
        /// Present when the question asked about every document, so the interface can say how
        /// much of the folder the answer really rests on.
        coverage: Option<EvidenceCoverage>,
    },
    /// A question the Work Folder inventory answered on its own. Machine codes plus data: the
    /// interface writes the sentence, in the practice's language, and no gateway call was made
    /// (`docs/WORK-FOLDER-INVENTORY.md`).
    FolderAnswer {
        answer: FolderAnswer,
    },
}

/// The Work Folder as it actually is, for the panel that shows it. Metadata only.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryReport {
    /// The folder's own name, never the machine path, which the panel already shows separately.
    root_identifier: String,
    summary: InventorySummary,
    files: Vec<FileRecord>,
    hierarchy: FolderNode,
}

fn open_index(app: &AppHandle) -> Result<IndexStore, AppError> {
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| AppError::IndexUnavailable)?;
    IndexStore::open_in_app_data(&directory)
}

/// Built from what the platform reports rather than from written paths, so the same rules hold on
/// Windows and on macOS.
fn work_folder_policy(app: &AppHandle) -> WorkFolderPolicy {
    let paths = app.path();
    let personal = [
        paths.document_dir(),
        paths.desktop_dir(),
        paths.download_dir(),
    ]
    .into_iter()
    .flatten()
    .collect();
    WorkFolderPolicy::for_machine(paths.home_dir().ok(), personal)
}

#[tauri::command]
pub fn load_app_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshot, AppError> {
    let (mut stored, mut warnings) = settings::load(&app);

    // The rules are re-applied to a folder chosen on an earlier run, because the machine changes
    // underneath us: switching OneDrive on redirects Documents into the cloud without asking the
    // software. A folder that no longer passes is dropped rather than kept and written into. The
    // cost is that an unplugged external disk also loses the setting, which is the safe side.
    if let Some(chosen) = stored.work_folder.clone() {
        let policy = work_folder_policy(&app);
        if policy.validate(Path::new(&chosen)).is_err() {
            stored.work_folder = None;
            warnings.push(AppError::WorkFolderNoLongerAllowed.code().to_string());
        }
    }

    state.replace(stored.clone())?;
    Ok(AppSnapshot {
        settings: stored,
        system_locale: sys_locale::get_locale().unwrap_or_default(),
        settings_path: display(&settings::settings_path(&app)?),
        suggested_work_folder: suggested_work_folder(app.path().home_dir().ok().as_deref())
            .as_deref()
            .map(display),
        warnings,
    })
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<Settings, AppError> {
    let stored = settings::save(&app, &work_folder_policy(&app), settings)?;
    state.replace(stored.clone())?;
    Ok(stored)
}

/// Put every setting back to what a first launch would have written, the documents folder
/// included.
///
/// Deliberately whole rather than "the easy ones". A reset that leaves the folder behind is not a
/// reset, and the folder is the setting most likely to be part of whatever went wrong - a path on
/// a disk that is no longer there, or a folder a sync client has since taken over. The defaults
/// come from Rust because Rust is what decides them: `DEFAULT_SERVER_URL` can be an environment
/// variable, so a default is read, never assumed by the interface.
///
/// The local index is **not** touched. Documents stay analysed, and choosing the same folder
/// again finds them exactly as they were - the index is keyed by path relative to the folder, not
/// by the setting. Emptying it is the other button, in the folder card, on purpose.
#[tauri::command]
pub fn reset_settings(app: AppHandle, state: State<'_, AppState>) -> Result<Settings, AppError> {
    let stored = settings::save(&app, &work_folder_policy(&app), Settings::default())?;
    state.replace(stored.clone())?;
    Ok(stored)
}

/// Opens the system dialog, then applies the allow-list. Nothing is stored until the interface
/// saves the settings, and nothing is read from the folder in this sprint.
#[tauri::command]
pub async fn choose_work_folder(app: AppHandle) -> Result<String, AppError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |chosen| {
        let _ = sender.send(chosen);
    });

    let Some(chosen) = receiver.await.map_err(|_| AppError::Internal)? else {
        return Err(AppError::WorkFolderSelectionCancelled);
    };
    let path = chosen.into_path().map_err(|_| AppError::Internal)?;
    let accepted = work_folder_policy(&app).validate(&path)?;
    Ok(display(&accepted))
}

/// Create `~/AssistantCabinetAI` if needed, then return the accepted path. The interface still
/// has to save the settings; this command does not write `settings.json` on its own.
#[tauri::command]
pub fn ensure_suggested_work_folder(app: AppHandle) -> Result<String, AppError> {
    let home = app.path().home_dir().ok();
    let accepted = work_folder::ensure_suggested(&work_folder_policy(&app), home.as_deref())?;
    Ok(display(&accepted))
}

#[tauri::command]
pub async fn check_server_health(state: State<'_, AppState>) -> Result<HealthSnapshot, AppError> {
    let server_url = state.read(|settings| settings.server_url.clone())?;
    state.gateway.health(&server_url).await
}

#[tauri::command]
pub async fn send_chat_message(
    state: State<'_, AppState>,
    turns: Vec<ChatTurn>,
    on_event: Channel<ChatStreamEvent>,
) -> Result<String, AppError> {
    let mut stopped = state.cancellation.begin();
    until_stopped(
        &mut stopped,
        conversation_answer(state.inner(), &turns, &on_event),
    )
    .await
}

/// Stop the question being worked on, and keep nothing of it.
///
/// The interface only offers this while the answer has not started, so there is never a
/// half-written summary to decide about: a summary of a specialist letter that looks complete but
/// stops mid-sentence is exactly the risk the disclaimer exists for.
#[tauri::command]
pub fn cancel_chat(state: State<'_, AppState>) {
    state.cancellation.cancel();
}

async fn conversation_answer(
    state: &AppState,
    turns: &[ChatTurn],
    on_event: &Channel<ChatStreamEvent>,
) -> Result<String, AppError> {
    let (server_url, model_alias, locale, idle_timeout) = state.read(|settings| {
        (
            settings.server_url.clone(),
            settings.model_alias.clone(),
            settings.locale.clone(),
            settings.answer_idle_timeout(),
        )
    })?;

    let answer = state
        .gateway
        .chat(
            &server_url,
            &model_alias,
            locale.as_deref(),
            turns,
            idle_timeout,
            |delta| {
                // A closed window is not a failure worth reporting.
                let _ = on_event.send(ChatStreamEvent::Delta {
                    text: delta.to_string(),
                });
            },
        )
        .await?;

    let _ = on_event.send(ChatStreamEvent::Completed {
        text: answer.clone(),
    });
    Ok(answer)
}

/// Discovery -> extraction -> chunking -> embeddings -> local index, one pass over the current
/// work folder. Nothing leaves the machine except the chunk text sent for embedding, batched
/// and capped (`docs/RETRIEVAL.md`).
#[tauri::command]
pub async fn index_work_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    on_progress: Channel<IndexProgress>,
) -> Result<IndexSummary, AppError> {
    let (work_folder, server_url, embedding_alias, locale) = state.read(|settings| {
        (
            settings.work_folder.clone(),
            settings.server_url.clone(),
            settings.embedding_alias.clone(),
            settings.locale.clone(),
        )
    })?;
    let Some(work_folder) = work_folder else {
        return Err(AppError::NoWorkFolderSet);
    };

    let tesseract = try_tesseract(&app);
    let rasterizer = try_rasterizer(&app);
    let ocr: Option<&dyn OcrProvider> = tesseract
        .as_ref()
        .map(|provider| provider as &dyn OcrProvider);
    let raster: Option<&dyn PageRasterizer> =
        rasterizer.map(|provider| provider as &dyn PageRasterizer);
    let locale = locale.as_deref().unwrap_or(settings::DEFAULT_LOCALE);

    let mut index = open_index(&app)?;
    indexing::run(
        &mut index,
        &state.gateway,
        &server_url,
        &embedding_alias,
        Path::new(&work_folder),
        ocr,
        raster,
        locale,
        // A closed window is not a failure worth reporting, exactly as for a chat delta.
        &|progress| {
            let _ = on_progress.send(progress);
        },
    )
    .await
}

/// Whether the local index has anything to search yet. The chat panel checks this before
/// spending a round trip on a question the retrieval path is guaranteed to refuse
/// (`AppError::InsufficientEvidence`) - a document has been chosen but never analysed.
#[tauri::command]
pub async fn has_indexed_documents(app: AppHandle) -> Result<bool, AppError> {
    let index = open_index(&app)?;
    Ok(index.chunk_count()? > 0)
}

/// What is in the Work Folder right now: the filesystem, joined with what the index made of it.
///
/// Answered from `std::fs` and the local index, never from the model, so the counts the panel
/// shows are the same ones a question about the folder gets (`docs/WORK-FOLDER-INVENTORY.md`).
#[tauri::command]
pub fn work_folder_inventory(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<InventoryReport, AppError> {
    let work_folder = state.read(|settings| settings.work_folder.clone())?;
    let Some(work_folder) = work_folder else {
        return Err(AppError::NoWorkFolderSet);
    };
    // A folder that has never been analysed still has an inventory; it simply has no index to
    // join onto, which is exactly what "nothing has been read yet" should look like.
    let index = open_index(&app).ok();
    let inventory = WorkFolderInventory::discover_with_cache(
        Path::new(&work_folder),
        index.as_ref(),
        &state.file_hashes,
    )?;

    Ok(InventoryReport {
        root_identifier: inventory.root_identifier(),
        summary: inventory.summary(),
        files: inventory.all_files().to_vec(),
        hierarchy: inventory.hierarchy(),
    })
}

/// Show the work folder in the system's own file manager.
///
/// Takes no path. The folder comes from settings, on this side of the bridge, so the webview can
/// ask for "the work folder" and never for a path of its own choosing - the allow-list holds
/// because there is nothing to allow-list (`docs/PRIVACY-AND-SECURITY.md`). Which file manager
/// opens is `reveal`'s business alone; nothing here knows the platform.
#[tauri::command]
pub fn reveal_work_folder(state: State<'_, AppState>) -> Result<(), AppError> {
    let work_folder = state.read(|settings| settings.work_folder.clone())?;
    let Some(work_folder) = work_folder else {
        return Err(AppError::NoWorkFolderSet);
    };
    reveal::folder(Path::new(&work_folder))
}

/// Forget everything the index holds, leaving every document in the work folder untouched.
///
/// This is the deliberate way back to "nothing has been analysed yet", and it exists because the
/// alternative she was reduced to was deleting `%LOCALAPPDATA%` by hand
/// (`docs/TROUBLESHOOTING.md`). It empties the index and the hash cache built from it; it never
/// reads, moves or deletes a file in the folder. The interface confirms before calling it - Rust
/// does not ask questions, it does what it was told.
#[tauri::command]
pub fn reset_index(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    let mut index = open_index(&app)?;
    index.clear()?;
    // Otherwise the next inventory would answer from hashes taken before the reset and show
    // files as still indexed when nothing is.
    state.file_hashes.forget_all();
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AskAnswer {
    pub answer: String,
    pub sources: Vec<Evidence>,
    /// Present when the question was about the Work Folder itself and was answered from the
    /// inventory. `answer` is then empty: Rust does not write prose.
    pub folder_answer: Option<FolderAnswer>,
    /// Present when the question asked about every document, so the interface can say how much of
    /// the folder the answer rests on rather than let it imply everything.
    pub coverage: Option<EvidenceCoverage>,
    /// How many documents in the folder this answer could not have used: never analysed, or
    /// changed since they were. Counted from the inventory the answer was planned against, so it
    /// describes the same moment as the answer itself.
    ///
    /// The failure it guards against is the quiet one. She drops a document in, forgets to press
    /// Analyse, asks about it, and gets a confident answer drawn from everything except the file
    /// she had in mind. Retrieval refuses when it has too little evidence; this is the other half,
    /// for when there is plenty of evidence and it is simply the wrong evidence.
    pub unanalysed_files: usize,
    /// Files the conversation's scope named that are gone, or no longer hold the content that was
    /// pinned. They were left out of the answer, and the interface says so rather than let a
    /// narrower answer read as the one she asked for.
    pub scope_outdated: Vec<String>,
}

/// Retrieval, then a sourced chat answer. Refuses rather than answers when the index does not
/// carry enough evidence for the question (`docs/ARCHITECTURE.md`).
///
/// The whole chain is stoppable, not only the chat leg: embedding the question is what runs while
/// the spinner shows, which is precisely the moment she wants the button to answer.
#[tauri::command]
pub async fn ask_with_sources(
    app: AppHandle,
    state: State<'_, AppState>,
    question: String,
    // Ask the model even when the Work Folder could answer on its own. Her choice, made on an
    // answer she has already seen, so the deterministic path stays the default and the model
    // stays reachable (`docs/WORK-FOLDER-INVENTORY.md`).
    skip_deterministic: Option<bool>,
    // The files this conversation is about. Absent means the whole folder, which is what every
    // caller did before scopes existed.
    scope: Option<AnalysisScope>,
    on_event: Channel<ChatStreamEvent>,
) -> Result<AskAnswer, AppError> {
    let mut stopped = state.cancellation.begin();
    until_stopped(
        &mut stopped,
        sourced_answer(
            &app,
            state.inner(),
            question,
            skip_deterministic.unwrap_or(false),
            scope.unwrap_or_else(|| AnalysisScope::whole_folder(0)),
            &on_event,
        ),
    )
    .await
}

async fn sourced_answer(
    app: &AppHandle,
    state: &AppState,
    question: String,
    skip_deterministic: bool,
    scope: AnalysisScope,
    on_event: &Channel<ChatStreamEvent>,
) -> Result<AskAnswer, AppError> {
    let (work_folder, server_url, model_alias, embedding_alias, locale, idle_timeout) = state
        .read(|settings| {
            (
                settings.work_folder.clone(),
                settings.server_url.clone(),
                settings.model_alias.clone(),
                settings.embedding_alias.clone(),
                settings.locale.clone(),
                settings.answer_idle_timeout(),
            )
        })?;
    let Some(work_folder) = work_folder else {
        return Err(AppError::NoWorkFolderSet);
    };

    let index = open_index(app)?;
    let full_inventory = WorkFolderInventory::discover(Path::new(&work_folder), Some(&index))?;
    // From here on the conversation sees the scope's inventory and nothing wider: the router, the
    // per-document list, the counts and the context all read it, and retrieval is held to its
    // members below.
    let resolution = scope.resolve(&full_inventory);
    let narrowed = resolution.narrowed;
    let scope_outdated: Vec<String> = resolution
        .missing
        .into_iter()
        .chain(resolution.changed)
        .collect();
    let inventory = resolution.inventory;
    let scoped_paths: Vec<String> = inventory
        .indexed_files()
        .into_iter()
        .map(|file| file.relative_path.clone())
        .collect();
    let locale = locale.as_deref();

    // Deterministic before generative (`docs/ARCHITECTURE.md`). Routing happens before the
    // question is embedded, so a question the folder itself answers costs no gateway call at
    // all - not even the embedding one - and still answers with the server stopped.
    /// What retrieval was asked to cover, decided before the question is embedded.
    enum Plan {
        /// One named file.
        OneFile(String),
        /// Every indexed file, because the question asked about every document.
        EveryDocument(Vec<String>),
        /// Whatever ranks best in the folder.
        WholeFolder,
    }

    let plan = match folder_questions::route(
        &inventory,
        &index,
        &question,
        locale.unwrap_or(settings::DEFAULT_LOCALE),
    ) {
        // Asked for again, with the model this time. A question the folder answers is still a
        // question about the folder, so the model gets every document rather than the handful
        // that rank highest - the same treatment "summarise each document" gets.
        _ if skip_deterministic => Plan::EveryDocument(
            inventory
                .indexed_files()
                .into_iter()
                .map(|file| file.relative_path.clone())
                .collect(),
        ),
        QuestionRoute::Deterministic(answer) => {
            let _ = on_event.send(ChatStreamEvent::FolderAnswer {
                answer: answer.clone(),
            });
            return Ok(AskAnswer {
                answer: String::new(),
                sources: Vec::new(),
                folder_answer: Some(answer),
                coverage: None,
                // A folder answer is read from the filesystem, not from the index, so it already
                // accounts for every file there is. Nothing about it is waiting on a pass.
                unanalysed_files: 0,
                scope_outdated,
            });
        }
        QuestionRoute::TargetedRetrieval { file } => Plan::OneFile(file.relative_path.clone()),
        QuestionRoute::PerDocumentRetrieval { files } => {
            Plan::EveryDocument(files.into_iter().map(|file| file.relative_path).collect())
        }
        QuestionRoute::GlobalRetrieval => Plan::WholeFolder,
    };

    // A question about one named file gets that file's record; anything else gets counts only.
    // A per-document question deliberately gets counts rather than the whole listing: the
    // excerpts already carry one header per file, so a listing would repeat every path in a
    // second format and push the first excerpts into the middle of a long prompt, which is where
    // small models were observed losing them (`docs/WORK-FOLDER-INVENTORY.md`).
    let view = match &plan {
        Plan::OneFile(relative_path) => ContextView::Targeted {
            relative_path: relative_path.clone(),
        },
        Plan::EveryDocument(_) | Plan::WholeFolder => ContextView::Summary,
    };

    // Nothing has been read yet: retrieval is certain to find nothing, so say so here rather
    // than spend a round trip discovering it.
    if index.chunk_count()? == 0 {
        return Err(AppError::InsufficientEvidence);
    }

    let query_vectors = state
        .gateway
        .embed(
            &server_url,
            &embedding_alias,
            std::slice::from_ref(&question),
        )
        .await?;
    let query_embedding = query_vectors.into_iter().next().unwrap_or_default();

    let evidence = match &plan {
        Plan::OneFile(relative_path) => retrieval::search_scoped(
            &index,
            &question,
            &query_embedding,
            RetrievalScope::File(relative_path.as_str()),
        )?,
        Plan::EveryDocument(relative_paths) => {
            retrieval::search_per_document(&index, &question, &query_embedding, relative_paths)?
        }
        Plan::WholeFolder => retrieval::search_scoped(
            &index,
            &question,
            &query_embedding,
            match narrowed {
                true => RetrievalScope::Files(&scoped_paths),
                false => RetrievalScope::WholeFolder,
            },
        )?,
    };
    if evidence.is_empty() {
        return Err(AppError::InsufficientEvidence);
    }

    // How much of the corpus this answer really rests on. Reported only when the question asked
    // about every document, because that is the only shape that claims completeness: a question
    // about one fact is properly answered from one passage, and saying "1 of 10" there would be
    // noise. Computed from the evidence and the inventory, never from the model.
    let coverage = match &plan {
        Plan::EveryDocument(_) => Some(retrieval::EvidenceCoverage::of(
            &evidence,
            inventory.indexed_files().len(),
            inventory.unreadable_files().len(),
        )),
        _ => None,
    };

    // Sent before the answer rather than after it. Retrieval has already finished at this point, so
    // there is nothing to wait for - and an answer she stops halfway still shows which documents it
    // was being written from, which an answer left on screen has to do.
    let _ = on_event.send(ChatStreamEvent::Sources {
        sources: evidence.clone(),
        coverage,
    });

    // Two kinds of evidence in one turn, kept apart on purpose: the excerpts say what the
    // documents state, the Work Folder context says which files exist and what was done to them,
    // and the contract between them forbids using either as a substitute for the other.
    let folder_context = work_folder_context::build(&inventory, &view);
    let context_turn = format!(
        "{}\n{}",
        work_folder_context::build_system_turn(retrieval::RETRIEVAL_INSTRUCTION, &folder_context),
        retrieval::format_evidence(&evidence)
    );
    let turns = vec![
        ChatTurn {
            role: "system".to_string(),
            content: context_turn,
        },
        ChatTurn {
            role: "user".to_string(),
            content: question,
        },
    ];

    let answer = state
        .gateway
        .chat(
            &server_url,
            &model_alias,
            locale,
            &turns,
            idle_timeout,
            |delta| {
                let _ = on_event.send(ChatStreamEvent::Delta {
                    text: delta.to_string(),
                });
            },
        )
        .await?;

    let _ = on_event.send(ChatStreamEvent::Completed {
        text: answer.clone(),
    });

    Ok(AskAnswer {
        answer,
        sources: evidence,
        folder_answer: None,
        coverage,
        unanalysed_files: inventory.unanalysed_documents(),
        scope_outdated,
    })
}

/// Sidecar discovery: look next to the executable, a few ancestors up (so `tauri dev` can see
/// copies left in `src-tauri/`), and under the resource directory. Never a hardcoded install
/// path. A missing engine is `None`, and indexing degrades to Sprint 2a.
fn search_roots(app: &AppHandle) -> Vec<std::path::PathBuf> {
    sidecar_search_roots(
        app.path().resource_dir().ok(),
        std::env::current_exe().ok(),
        std::env::current_dir().ok(),
    )
}

fn sidecar_search_roots(
    resource_dir: Option<std::path::PathBuf>,
    exe: Option<std::path::PathBuf>,
    cwd: Option<std::path::PathBuf>,
) -> Vec<std::path::PathBuf> {
    let mut roots = Vec::new();
    let mut push = |path: std::path::PathBuf| {
        if path.as_os_str().is_empty() {
            return;
        }
        if !roots.iter().any(|existing| existing == &path) {
            roots.push(path);
        }
    };
    if let Some(dir) = resource_dir {
        push(dir);
    }
    if let Some(exe_path) = exe {
        if let Some(parent) = exe_path.parent() {
            for ancestor in parent.ancestors().take(6) {
                push(ancestor.to_path_buf());
                push(ancestor.join("binaries"));
                push(ancestor.join("resources"));
            }
        }
    }
    if let Some(cwd_path) = cwd {
        push(cwd_path.clone());
        push(cwd_path.join("binaries"));
        push(cwd_path.join("resources"));
    }
    roots
}

fn first_existing(
    candidates: impl IntoIterator<Item = std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    candidates.into_iter().find(|path| path.exists())
}

fn try_tesseract(app: &AppHandle) -> Option<TesseractProvider> {
    let roots = search_roots(app);
    let binary_names = [
        "tesseract.exe",
        "tesseract",
        "tesseract-x86_64-pc-windows-msvc.exe",
        "tesseract-aarch64-apple-darwin",
        "tesseract-x86_64-apple-darwin",
    ];
    let mut binaries = Vec::new();
    let mut tessdata_dirs = Vec::new();
    for root in &roots {
        for name in binary_names {
            binaries.push(root.join(name));
        }
        tessdata_dirs.push(root.join("resources").join("tessdata"));
        tessdata_dirs.push(root.join("tessdata"));
    }
    let binary = first_existing(binaries)?;
    let tessdata = first_existing(tessdata_dirs)?;
    TesseractProvider::new(binary, tessdata).ok()
}

/// The process's rasteriser, not a new one. Discovery still runs on every pass - it is a handful
/// of `exists()` calls, and it means a library staged mid-session is picked up on the next pass -
/// but the instance itself comes from `raster::shared`, because pdfium can only be bound once per
/// process and a second binding would silently disable OCR for every scanned PDF
/// (`docs/TROUBLESHOOTING.md`).
fn try_rasterizer(app: &AppHandle) -> Option<&'static Rasterizer> {
    let roots = search_roots(app);
    let library_names = ["pdfium.dll", "libpdfium.dylib", "libpdfium.so", "pdfium"];
    let mut candidates = Vec::new();
    for root in &roots {
        for name in library_names {
            candidates.push(root.join("resources").join("pdfium").join(name));
            candidates.push(root.join("pdfium").join(name));
            candidates.push(root.join(name));
        }
    }
    let library = first_existing(candidates)?;
    raster::shared(&library)
}

#[cfg(test)]
mod sidecar_discovery_tests {
    use super::sidecar_search_roots;
    use std::path::PathBuf;

    #[test]
    fn tauri_dev_walks_up_from_the_debug_executable_to_the_crate_root() {
        let exe = PathBuf::from("repo/apps/desktop/src-tauri/target/debug/app");
        let roots = sidecar_search_roots(None, Some(exe), None);

        assert!(
            roots.iter().any(|path| path.ends_with("src-tauri")),
            "expected a src-tauri ancestor among {roots:?}"
        );
        assert!(
            roots.iter().any(|path| path.ends_with("binaries")),
            "expected a binaries folder among {roots:?}"
        );
    }
}
