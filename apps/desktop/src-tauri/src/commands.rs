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

use crate::error::AppError;
use crate::gateway::{ChatTurn, GatewayClient, HealthSnapshot};
use crate::index_store::IndexStore;
use crate::indexing::{self, IndexSummary};
use crate::retrieval::{self, Evidence};
use crate::settings::{self, Settings};
use crate::work_folder::{self, display, suggested_work_folder, WorkFolderPolicy};

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub gateway: GatewayClient,
}

impl AppState {
    pub fn new() -> Result<Self, AppError> {
        Ok(Self {
            settings: Mutex::new(Settings::default()),
            gateway: GatewayClient::new()?,
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
    Delta { text: String },
    Completed { text: String },
    Sources { sources: Vec<Evidence> },
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
    let (server_url, model_alias, locale) = state.read(|settings| {
        (
            settings.server_url.clone(),
            settings.model_alias.clone(),
            settings.locale.clone(),
        )
    })?;

    let answer = state
        .gateway
        .chat(
            &server_url,
            &model_alias,
            locale.as_deref(),
            &turns,
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
) -> Result<IndexSummary, AppError> {
    let (work_folder, server_url, embedding_alias) = state.read(|settings| {
        (
            settings.work_folder.clone(),
            settings.server_url.clone(),
            settings.embedding_alias.clone(),
        )
    })?;
    let Some(work_folder) = work_folder else {
        return Err(AppError::NoWorkFolderSet);
    };

    let mut index = open_index(&app)?;
    indexing::run(
        &mut index,
        &state.gateway,
        &server_url,
        &embedding_alias,
        Path::new(&work_folder),
    )
    .await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AskAnswer {
    pub answer: String,
    pub sources: Vec<Evidence>,
}

/// Retrieval, then a sourced chat answer. Refuses rather than answers when the index does not
/// carry enough evidence for the question (`docs/ARCHITECTURE.md`).
#[tauri::command]
pub async fn ask_with_sources(
    app: AppHandle,
    state: State<'_, AppState>,
    question: String,
    on_event: Channel<ChatStreamEvent>,
) -> Result<AskAnswer, AppError> {
    let (work_folder, server_url, model_alias, embedding_alias, locale) =
        state.read(|settings| {
            (
                settings.work_folder.clone(),
                settings.server_url.clone(),
                settings.model_alias.clone(),
                settings.embedding_alias.clone(),
                settings.locale.clone(),
            )
        })?;
    if work_folder.is_none() {
        return Err(AppError::NoWorkFolderSet);
    }

    let index = open_index(&app)?;

    let query_vectors = state
        .gateway
        .embed(
            &server_url,
            &embedding_alias,
            std::slice::from_ref(&question),
        )
        .await?;
    let query_embedding = query_vectors.into_iter().next().unwrap_or_default();

    let evidence = retrieval::search(&index, &question, &query_embedding)?;
    if evidence.is_empty() {
        return Err(AppError::InsufficientEvidence);
    }

    let context_turn = retrieval::build_context_turn(&evidence);
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
            locale.as_deref(),
            &turns,
            |delta| {
                let _ = on_event.send(ChatStreamEvent::Delta {
                    text: delta.to_string(),
                });
            },
        )
        .await?;

    let _ = on_event.send(ChatStreamEvent::Sources {
        sources: evidence.clone(),
    });
    let _ = on_event.send(ChatStreamEvent::Completed {
        text: answer.clone(),
    });

    Ok(AskAnswer {
        answer,
        sources: evidence,
    })
}
