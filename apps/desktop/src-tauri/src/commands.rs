//! The bridge the webview is allowed to use.
//!
//! Each command is a narrow door: read the settings, store validated settings, pick a folder
//! through the system dialog, ask the gateway how it is, send one conversation. The address, the
//! alias, the locale and the allow-list stay on this side of the door.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::analysis_scope::{AnalysisScope, GroundingTier, ScopeMode};
use crate::analysis_stop::AnalysisStops;
use crate::calculation_follow_up::{self, CalculationFollowUp, FollowUpKind};
use crate::cancellation::{until_stopped, Cancellation};
use crate::conversation::{self, ContextBudget, ModelBudgets};
use crate::data_folder::{self, DataFolder, DataFolderReport};
use crate::discovery;
use crate::error::AppError;
use crate::family_view::{self, FamilyViews};
use crate::file_profile;
use crate::file_record::{FileRecord, ProcessingStatus};
use crate::file_reference::FileReferenceResolver;
use crate::filename_sanitizer;
use crate::fill_draft;
use crate::fill_plan;
use crate::finder::{self, FinderQuery, FinderResult, FinderScope, InventoryFile};
use crate::folder_questions::{self, FolderAnswer, QuestionRoute};
use crate::gateway::{ChatTurn, GatewayClient, HealthSnapshot, GROUNDED_TEMPERATURE};
use crate::index_store::IndexStore;
use crate::indexing::{self, IndexProgress, IndexSummary};
use crate::inventory::{FileHashCache, FolderNode, InventorySummary, WorkFolderInventory};
use crate::knowledge::diagnostics::{
    self, elapsed_ms, RetrievalDiagnostics, RetrievalPath, RetrievalPlan, StageTimer,
};
use crate::knowledge::ingest::KnowledgeContext;
use crate::knowledge::manage::{self, NameTools};
use crate::knowledge::packs::PackSet;
use crate::knowledge::picker::{self, EntitySearch, PickerFiles};
use crate::knowledge::query::{QuerySelection, SelectedFile};
use crate::knowledge::reduce::{SideInput, SkipReason, TargetingPolicy};
use crate::knowledge::secret::IdentifierKey;
use crate::knowledge::sheet::{self, EntitySheet};
use crate::knowledge::suggest::{self, SuggestionCache, SuggestionInput, SuggestionSet};
use crate::knowledge::targeting::{
    self, DocumentPlan, QueryVocabulary, TargetingPlan, TargetingReport, TargetingRequest,
    TargetingTools,
};
use crate::knowledge::{KnowledgeMode, KnowledgeSummary};
use crate::mixed_answer;
use crate::number_check;
use crate::ocr::tesseract::TesseractProvider;
use crate::ocr::OcrProvider;
use crate::raster::{self, PageRasterizer, Rasterizer};
use crate::retrieval::{self, Evidence, EvidenceCoverage};
use crate::reveal;
use crate::settings::{self, Settings};
use crate::speech::repair;
use crate::tabular::card::card_budget_chars;
use crate::tabular::carry_over::{CarriedParts, CarryReport, TableCarryOver};
use crate::tabular::layout::RowPolicy;
use crate::tabular_answer::{self, TabularAnswer, WorkbookChoice};
use crate::viewer_source;
use crate::work_folder::{
    self, display, suggested_data_folder, suggested_work_folder, WorkFolderPolicy,
};
use crate::work_folder_context::{self, ContextView};
use crate::workbook_use::{self, WorkbookUse};

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub gateway: GatewayClient,
    /// Lets `cancel_chat` reach the question being worked on. One at a time.
    pub cancellation: Cancellation,
    /// One Stop flag per folder, raised by `cancel_analysis`, read between two files and two batches by
    /// the pass of that folder (`indexing::run_cancellable`, `data_folder::analyse_cancellable`) and
    /// lowered when a pass of that folder starts. The two passes can run together.
    pub analysis_stop: AnalysisStops,
    /// What each file in the work folder last hashed to. Lives as long as the application so the
    /// folder panel can be rebuilt whenever she comes back to the window without re-reading every
    /// scan in the folder (`inventory::FileHashCache`).
    pub file_hashes: FileHashCache,
    /// The same, for the Data Folder. Kept apart so resetting one folder's analysis never makes
    /// the other re-read its files.
    pub data_file_hashes: FileHashCache,
    /// Each model's context budget, as the gateway last published it (`conversation`).
    pub model_budgets: Mutex<ModelBudgets>,
    /// The words a question is read with, loaded once for a locale and a set of packs and kept
    /// until either changes (`knowledge::targeting::QueryVocabulary`): loading costs tens of
    /// milliseconds, which is nothing once and far too much per question.
    vocabulary: Mutex<Option<CachedVocabulary>>,
    /// The last few screens of question suggestions (`knowledge::suggest::SuggestionCache`): in
    /// memory only, keyed by the selection and the state of the base, never written anywhere.
    suggestions: SuggestionCache,
    /// What a figure does with the rows a spreadsheet hides or filters out, for the question being
    /// answered (`tabular::layout::RowPolicy`). Set when a question starts and put back to the
    /// default when it ends; `family_views_of` hands it to the Data folder every tier reads.
    row_policy: Mutex<RowPolicy>,
}

struct CachedVocabulary {
    locale: String,
    packs: Vec<String>,
    vocabulary: Arc<QueryVocabulary>,
}

impl AppState {
    pub fn new() -> Result<Self, AppError> {
        Ok(Self {
            settings: Mutex::new(Settings::default()),
            gateway: GatewayClient::new()?,
            cancellation: Cancellation::default(),
            analysis_stop: AnalysisStops::default(),
            file_hashes: FileHashCache::new(),
            data_file_hashes: FileHashCache::new(),
            model_budgets: Mutex::new(ModelBudgets::default()),
            vocabulary: Mutex::new(None),
            suggestions: SuggestionCache::default(),
            row_policy: Mutex::new(RowPolicy::default()),
        })
    }

    /// The vocabulary for this locale and these packs, loaded on first use.
    fn vocabulary(&self, locale: &str, packs: &[String]) -> Result<Arc<QueryVocabulary>, AppError> {
        let mut cache = self.vocabulary.lock().map_err(|_| AppError::Internal)?;
        if let Some(cached) = cache.as_ref() {
            if cached.locale == locale && cached.packs == packs {
                return Ok(Arc::clone(&cached.vocabulary));
            }
        }
        let names: Vec<&str> = packs.iter().map(String::as_str).collect();
        let vocabulary = Arc::new(QueryVocabulary::load(locale, &names)?);
        *cache = Some(CachedVocabulary {
            locale: locale.to_string(),
            packs: packs.to_vec(),
            vocabulary: Arc::clone(&vocabulary),
        });
        Ok(vocabulary)
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
    /// What to propose when no folder has been chosen: `~/AssistantCabinetAI/Docs`, outside
    /// Documents so that no cloud client mirrors it.
    suggested_work_folder: Option<String>,
    /// The Data Folder equivalent: `~/AssistantCabinetAI/Data`, a sibling of `Docs` rather than a
    /// second Documents Folder.
    suggested_data_folder: Option<String>,
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
    /// What the knowledge base did to the search, sent just before `Sources` so an answer that is
    /// interrupted still shows it. Only when there is something to tell: a reduction that was
    /// applied, or names to ask about.
    Targeting {
        report: Box<TargetingReport>,
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

/// The knowledge base's part in a pass: the packs and encoder for the locale, the gazetteer, the
/// key that hashes personal identifiers (kept in the application's local data folder, never in the
/// index). `packs` are the optional lexicon packs the settings switch on: `health` by default,
/// `health-ch` added for a Swiss practice (`Settings::knowledge_pack_ids`).
fn knowledge_context(
    app: &AppHandle,
    index: &IndexStore,
    mode: KnowledgeMode,
    locale: &str,
    packs: &[String],
) -> Result<KnowledgeContext, AppError> {
    if !mode.reads_files() {
        // The knowledge base reads nothing, but the file profile still reads with the packs she chose.
        return KnowledgeContext::off(locale).map(|context| context.with_profile_packs(packs));
    }
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| AppError::KnowledgeUnavailable)?;
    let key = IdentifierKey::load_or_create(&directory)?;
    let packs: Vec<&str> = packs.iter().map(String::as_str).collect();
    KnowledgeContext::standard(index.connection(), locale, &packs, mode, key)
}

/// Built from what the platform reports rather than from written paths, so the same rules hold on
/// Windows and on macOS.
pub(crate) fn work_folder_policy(app: &AppHandle) -> WorkFolderPolicy {
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
    if let Some(chosen) = stored.data_folder.clone() {
        let policy = work_folder_policy(&app);
        if policy.validate(Path::new(&chosen)).is_err() {
            stored.data_folder = None;
            warnings.push(AppError::DataFolderNoLongerAllowed.code().to_string());
        }
    }

    let home = app.path().home_dir().ok();
    state.replace(stored.clone())?;
    Ok(AppSnapshot {
        settings: stored,
        system_locale: sys_locale::get_locale().unwrap_or_default(),
        settings_path: display(&settings::settings_path(&app)?),
        suggested_work_folder: suggested_work_folder(home.as_deref())
            .as_deref()
            .map(display),
        suggested_data_folder: suggested_data_folder(home.as_deref())
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
/// by the setting. Emptying it is the other action, in the folder menu of the explorer, on purpose.
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

/// Create `~/AssistantCabinetAI/Docs` if needed, then return the accepted path. The interface
/// still has to save the settings; this command does not write `settings.json` on its own.
#[tauri::command]
pub fn ensure_suggested_work_folder(app: AppHandle) -> Result<String, AppError> {
    let home = app.path().home_dir().ok();
    let accepted = work_folder::ensure_suggested(&work_folder_policy(&app), home.as_deref())?;
    Ok(display(&accepted))
}

/// Opens the system dialog, then applies the allow-list, for the Data Folder. Nothing is stored
/// until the interface saves the settings, and nothing is read from the folder in this sprint.
#[tauri::command]
pub async fn choose_data_folder(app: AppHandle) -> Result<String, AppError> {
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

/// Create `~/AssistantCabinetAI/Data` if needed, then return the accepted path. The interface
/// still has to save the settings; this command does not write `settings.json` on its own.
#[tauri::command]
pub fn ensure_suggested_data_folder(app: AppHandle) -> Result<String, AppError> {
    let home = app.path().home_dir().ok();
    let accepted = work_folder::ensure_suggested_data(&work_folder_policy(&app), home.as_deref())?;
    Ok(display(&accepted))
}

#[tauri::command]
pub async fn check_server_health(state: State<'_, AppState>) -> Result<HealthSnapshot, AppError> {
    let server_url = state.read(|settings| settings.server_url.clone())?;
    let snapshot = state.gateway.health(&server_url).await?;
    // Kept for every question that follows: the budget of each model is read from here rather
    // than asked for again, so a question costs no extra round trip (`conversation`).
    state
        .model_budgets
        .lock()
        .map_err(|_| AppError::Internal)?
        .update(
            snapshot.context_windows.clone(),
            snapshot.max_output_tokens,
            snapshot.max_context_chars,
        );
    Ok(snapshot)
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

/// Discovery -> extraction -> chunking -> embeddings -> local index, one pass over the current
/// work folder. Nothing leaves the machine except the chunk text sent for embedding, batched
/// and capped (`docs/RETRIEVAL.md`).
#[tauri::command]
pub async fn index_work_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    on_progress: Channel<IndexProgress>,
) -> Result<IndexSummary, AppError> {
    let (work_folder, data_folder, server_url, embedding_alias, locale, mode, packs, own_names) =
        state.read(|settings| {
            (
                settings.work_folder.clone(),
                settings.data_folder.clone(),
                settings.server_url.clone(),
                settings.embedding_alias.clone(),
                settings.locale.clone(),
                settings.knowledge_mode,
                settings.knowledge_pack_ids(),
                settings.own_names.clone(),
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

    // Names are made plain before anything is read, so the pass, the index and every later question
    // agree on one spelling of each file. Logged as it happens: a pass that fails afterwards must
    // not leave a renamed file with no record of what it used to be.
    // The rule is by family: this pass renames a document wherever it sits, in the Data folder too, and
    // no pass renames a workbook (`docs/DECISIONS.md`, "clean file names").
    let sanitised = filename_sanitizer::sanitize_documents(
        Path::new(&work_folder),
        data_folder.as_deref().map(Path::new),
    );
    log_renames(&app, &sanitised.renamed);

    let mut index = open_index(&app)?;
    // The names the files had before the rename: the file profile keeps them searchable. A failure to
    // record them costs a searchable alias, never the pass.
    let _ = file_profile::store::record_original_names(
        index.connection(),
        file_profile::build::Domain::Documents,
        &sanitised.renamed,
    );
    // The knowledge base never blocks a pass: if it cannot start (a damaged pack, a key that cannot
    // be read) the documents are analysed exactly as before and the summary says that part of the
    // analysis did not complete.
    let mut knowledge = knowledge_context(&app, &index, mode, locale, &packs)
        .ok()
        .map(|context| context.with_own_names(&own_names));
    state
        .analysis_stop
        .lower(file_profile::build::Domain::Documents);
    let mut summary = indexing::run_cancellable(
        &mut index,
        &state.gateway,
        &server_url,
        &embedding_alias,
        Path::new(&work_folder),
        // The documents found in the Data folder are read where they sit.
        data_folder.as_deref().map(Path::new),
        ocr,
        raster,
        locale,
        // A closed window is not a failure worth reporting, exactly as for a chat delta.
        &|progress| {
            let _ = on_progress.send(progress);
        },
        knowledge.as_mut(),
        &|| {
            state
                .analysis_stop
                .is_raised(file_profile::build::Domain::Documents)
        },
    )
    .await?;
    if knowledge.is_none() && mode.reads_files() {
        summary.knowledge = Some(KnowledgeSummary {
            errors: summary.scanned_files.max(1),
            ..KnowledgeSummary::default()
        });
    }
    summary.renamed_files = sanitised.renamed;
    summary.rename_failed_files = sanitised.failed;
    log_analysis_timing(&app, &state, &summary);
    Ok(summary)
}

/// Stop the analysis of one folder that is running. It stops between two files, two batches of
/// vectors or two workbooks; what it wrote stays, and the next one goes on from there. The other
/// folder's analysis is not touched. Nothing happens when none is running.
#[tauri::command]
pub fn cancel_analysis(state: State<'_, AppState>, domain: file_profile::build::Domain) {
    state.analysis_stop.raise(domain);
}

/// How many passages have their meaning vector, out of all of them: one cheap count, so the card can
/// say "meaning search: 62 %" while it is below 100. It computes nothing.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorShare {
    pub done: usize,
    pub total: usize,
}

#[tauri::command]
pub fn vector_share(app: AppHandle) -> Result<VectorShare, AppError> {
    let index = open_index(&app)?;
    let counts = index.vector_counts()?;
    Ok(VectorShare {
        done: counts.done,
        total: counts.total,
    })
}

/// Whether each folder's analysis has its file profile, by one cheap lookup, so the interface can say
/// that the index must be redone when it has not (an index written before the profile existed). It
/// reads and changes nothing: only an Analyse the user starts fills the profile.
#[tauri::command]
pub fn profile_index_status(
    app: AppHandle,
) -> Result<file_profile::store::ProfileIndexStatus, AppError> {
    let index = open_index(&app)?;
    Ok(file_profile::store::index_status(index.connection()))
}

/// Find files of one folder by what the index and the file system know, with no model, no embedding and
/// no file read (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 6). Cheap enough to run while the user types: it walks
/// the folder's names, reads the profile tables and starts nothing heavy. It never changes the selection.
#[tauri::command]
pub async fn find_files(
    app: AppHandle,
    state: State<'_, AppState>,
    domain: file_profile::build::Domain,
    mut query: FinderQuery,
    scope: FinderScope,
) -> Result<FinderResult, AppError> {
    let (folder, other_folder, locale, pack_ids) = state.read(|settings| {
        let (own, other) = match domain {
            file_profile::build::Domain::Documents => {
                (settings.work_folder.clone(), settings.data_folder.clone())
            }
            file_profile::build::Domain::Data => {
                (settings.data_folder.clone(), settings.work_folder.clone())
            }
        };
        (
            own,
            other,
            settings
                .locale
                .clone()
                .unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string()),
            settings.knowledge_pack_ids(),
        )
    })?;
    let Some(folder) = folder else {
        return Err(match domain {
            file_profile::build::Domain::Documents => AppError::NoWorkFolderSet,
            file_profile::build::Domain::Data => AppError::NoDataFolderSet,
        });
    };
    // The search is by family, across both folders: the files of the family in its own folder keep their
    // path, those found in the other folder come back under their key (`file_key`), and the client places
    // each by the folder it names. A file of the other family is searched by that family's own request.
    let inventory: Vec<InventoryFile> = discovery::discover_family(
        Path::new(&folder),
        other_folder.as_deref().map(Path::new),
        domain,
    )
    .iter()
    .map(InventoryFile::from)
    .collect();
    // An extension typed in the field is a format filter and a date typed alone also asks for the files
    // dated so (`finder::typed`); the words that are left are what the finder searches.
    let typed_formats = finder::typed::apply_typed_text(&mut query, &inventory);
    // A number typed in any spelling meets the number stored in the profile: the identifiers of the active
    // packs are read from what was typed, here, where the packs are known.
    if query.identifiers.is_empty() {
        if let Some(text) = query.text.as_deref() {
            let ids: Vec<&str> = pack_ids.iter().map(String::as_str).collect();
            if let (Ok(packs), Ok(vocabulary)) = (
                PackSet::load(&locale, &ids),
                file_profile::vocabulary::ProfileVocabulary::load(&ids),
            ) {
                query.identifiers = finder::set_search::identifier_forms(text, &packs, &vocabulary);
            }
        }
    }
    let index = open_index(&app)?;
    let mut result = finder::find(index.connection(), domain, &inventory, &query, &scope)?;
    result.typed_formats = typed_formats;
    Ok(result)
}

/// What the knowledge base and the file profile know of each analysed file of one folder (business type,
/// years, people and organisations with their roles), for the explorer's filters. Two reads of the profile
/// tables and two of the knowledge base: no model, no file read, nothing written. The explorer asks for it when
/// a filter of the knowledge base is shown or in force, never at launch.
#[tauri::command(async)]
pub fn file_knowledge_facts(
    app: AppHandle,
    domain: file_profile::build::Domain,
) -> Result<finder::facts::FolderFacts, AppError> {
    let index = open_index(&app)?;
    finder::facts::folder_facts(index.connection(), domain)
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
///
/// `async` so a first read, which hashes every file of a large folder, never runs on the thread
/// that draws the window: the explorer reads this and `data_folder_inventory` at launch and each
/// time the window gets the focus back (measured, `docs/DECISIONS.md`, "the explorer's unified view").
#[tauri::command(async)]
pub fn work_folder_inventory(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<InventoryReport, AppError> {
    let (work_folder, data_folder) =
        state.read(|settings| (settings.work_folder.clone(), settings.data_folder.clone()))?;
    let Some(work_folder) = work_folder else {
        return Err(AppError::NoWorkFolderSet);
    };
    // A folder that has never been analysed still has an inventory; it simply has no index to
    // join onto, which is exactly what "nothing has been read yet" should look like. The Data folder
    // is given too: it decides the key of a file the two folders share.
    let index = open_index(&app).ok();
    let inventory = WorkFolderInventory::discover_alongside(
        Path::new(&work_folder),
        data_folder.as_deref().map(Path::new),
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

/// Show the Data Folder in the system's own file manager. Same shape as `reveal_work_folder`:
/// takes no path, reads the folder from settings, and reveal itself stays folder-type-agnostic.
#[tauri::command]
pub fn reveal_data_folder(state: State<'_, AppState>) -> Result<(), AppError> {
    let data_folder = state.read(|settings| settings.data_folder.clone())?;
    let Some(data_folder) = data_folder else {
        return Err(AppError::NoDataFolderSet);
    };
    reveal::folder(Path::new(&data_folder))
}

/// Show one file of the work folder, selected in the system's file manager.
///
/// Unlike the command above this one takes a path from the webview, so it takes a path *relative
/// to the work folder* and nothing else: `reveal::file` refuses anything that could lead outside
/// it, and the folder itself still comes from settings. The file is pointed at, never opened.
#[tauri::command]
pub fn reveal_work_file(state: State<'_, AppState>, relative_path: String) -> Result<(), AppError> {
    let work_folder = state.read(|settings| settings.work_folder.clone())?;
    let Some(work_folder) = work_folder else {
        return Err(AppError::NoWorkFolderSet);
    };
    reveal::file(Path::new(&work_folder), &relative_path)
}

/// The bytes of one file of the Documents Folder, for the viewer to show. A path relative to that
/// folder and nothing else; the folder comes from settings. Read-only: nothing is written, hashed
/// or indexed, and the selection of files for the assistant is not touched. Text files come back
/// as UTF-8, every other file byte for byte. `async` so a large read runs off the main thread.
#[tauri::command(async)]
pub fn read_work_file_for_viewer(
    state: State<'_, AppState>,
    relative_path: String,
) -> Result<tauri::ipc::Response, AppError> {
    let work_folder = state.read(|settings| settings.work_folder.clone())?;
    let Some(work_folder) = work_folder else {
        return Err(AppError::NoWorkFolderSet);
    };
    viewer_source::read(Path::new(&work_folder), &relative_path).map(tauri::ipc::Response::new)
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
    index.clear_profiles(file_profile::build::Domain::Documents)?;
    // Otherwise the next inventory would answer from hashes taken before the reset and show
    // files as still indexed when nothing is.
    state.file_hashes.forget_all();
    Ok(())
}

/// The Data Folder's Analyse: clean names first, exactly as for the Documents Folder, then every
/// CSV/XLS/XLSX/XLSM file parsed and its inventory cached (`data_folder::analyse`). No embedding
/// and no gateway call - local parsing only, so it is over in moments where the Documents pass
/// takes minutes. Counts only on the progress channel: no file name, no cell.
///
/// Parsed on a blocking thread, so the window keeps drawing and the Stop of this folder
/// (`cancel_analysis`) can be heard between two workbooks, while the Documents pass runs on its own.
#[tauri::command]
pub async fn index_data_folder(
    app: AppHandle,
    on_progress: Channel<IndexProgress>,
) -> Result<IndexSummary, AppError> {
    tauri::async_runtime::spawn_blocking(move || run_data_pass(&app, &on_progress))
        .await
        .map_err(|_| AppError::Internal)?
}

fn run_data_pass(
    app: &AppHandle,
    on_progress: &Channel<IndexProgress>,
) -> Result<IndexSummary, AppError> {
    let state = app.state::<AppState>();
    let (data_folder, work_folder, locale, mode, packs, own_names) = state.read(|settings| {
        (
            settings.data_folder.clone(),
            settings.work_folder.clone(),
            settings.locale.clone(),
            settings.knowledge_mode,
            settings.knowledge_pack_ids(),
            settings.own_names.clone(),
        )
    })?;
    let Some(data_folder) = data_folder else {
        return Err(AppError::NoDataFolderSet);
    };
    let locale = locale.as_deref().unwrap_or(settings::DEFAULT_LOCALE);

    // This pass renames nothing: a workbook is never renamed, whichever folder it sits in, and the
    // documents are renamed by the document pass (`docs/DECISIONS.md`, "clean file names").
    let mut index = open_index(app)?;
    // The knowledge base never blocks a pass: if it cannot start, the workbooks are analysed exactly
    // as before and the summary says that part of the analysis did not complete.
    let mut knowledge = knowledge_context(app, &index, mode, locale, &packs)
        .ok()
        .map(|context| context.with_own_names(&own_names));
    state.analysis_stop.lower(file_profile::build::Domain::Data);
    let mut summary = data_folder::analyse_cancellable(
        Path::new(&data_folder),
        // The workbooks found in the Documents folder are read where they sit.
        work_folder.as_deref().map(Path::new),
        &mut index,
        locale,
        &|progress| {
            let _ = on_progress.send(progress);
        },
        knowledge.as_mut(),
        &|| {
            state
                .analysis_stop
                .is_raised(file_profile::build::Domain::Data)
        },
    )?;
    if knowledge.is_none() && mode.reads_files() {
        summary.knowledge = Some(KnowledgeSummary {
            errors: summary.scanned_files.max(1),
            ..KnowledgeSummary::default()
        });
    }
    log_analysis_timing(app, &state, &summary);
    Ok(summary)
}

/// What is in the Data Folder right now, and which workbooks hold a usable table: the
/// filesystem, joined with the cached tabular inventories. The one place both are read together
/// (`docs/DECISIONS.md`, "the two inventories stay separate"). `async` for the same reason as
/// `work_folder_inventory`.
#[tauri::command(async)]
pub fn data_folder_inventory(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DataFolderReport, AppError> {
    let (work_folder, data_folder) =
        state.read(|settings| (settings.work_folder.clone(), settings.data_folder.clone()))?;
    let Some(data_folder) = data_folder else {
        return Err(AppError::NoDataFolderSet);
    };
    let index = open_index(&app).ok();
    let folder = DataFolder::discover_alongside(
        Path::new(&data_folder),
        work_folder.as_deref().map(Path::new),
        index.as_ref(),
        &state.data_file_hashes,
    )?;
    Ok(folder.report())
}

/// Show one file of the Data Folder, selected in the system's file manager. A path relative to
/// the Data Folder and nothing else, refused if it could leave it - `reveal_work_file`'s rules.
#[tauri::command]
pub fn reveal_data_file(state: State<'_, AppState>, relative_path: String) -> Result<(), AppError> {
    let data_folder = state.read(|settings| settings.data_folder.clone())?;
    let Some(data_folder) = data_folder else {
        return Err(AppError::NoDataFolderSet);
    };
    reveal::file(Path::new(&data_folder), &relative_path)
}

/// The Data Folder equivalent of `read_work_file_for_viewer`: the bytes of one file of the Data
/// Folder, a relative path and nothing else, read-only.
#[tauri::command(async)]
pub fn read_data_file_for_viewer(
    state: State<'_, AppState>,
    relative_path: String,
) -> Result<tauri::ipc::Response, AppError> {
    let data_folder = state.read(|settings| settings.data_folder.clone())?;
    let Some(data_folder) = data_folder else {
        return Err(AppError::NoDataFolderSet);
    };
    viewer_source::read(Path::new(&data_folder), &relative_path).map(tauri::ipc::Response::new)
}

/// The text of one mail or subtitle file of the Documents Folder as the assistant reads it, as
/// UTF-8, for the viewer's readable mode. A path relative to that folder and nothing else;
/// read-only, like the raw read. A format without a readable text is refused.
#[tauri::command(async)]
pub fn read_work_file_readable_text(
    state: State<'_, AppState>,
    relative_path: String,
) -> Result<tauri::ipc::Response, AppError> {
    let work_folder = state.read(|settings| settings.work_folder.clone())?;
    let Some(work_folder) = work_folder else {
        return Err(AppError::NoWorkFolderSet);
    };
    viewer_source::read_readable_text(Path::new(&work_folder), &relative_path)
        .map(tauri::ipc::Response::new)
}

/// The Data Folder equivalent of `read_work_file_readable_text`.
#[tauri::command(async)]
pub fn read_data_file_readable_text(
    state: State<'_, AppState>,
    relative_path: String,
) -> Result<tauri::ipc::Response, AppError> {
    let data_folder = state.read(|settings| settings.data_folder.clone())?;
    let Some(data_folder) = data_folder else {
        return Err(AppError::NoDataFolderSet);
    };
    viewer_source::read_readable_text(Path::new(&data_folder), &relative_path)
        .map(tauri::ipc::Response::new)
}

/// Forget every workbook analysis, and nothing else: the Data folder's "clear the analysis". The
/// documents index is left exactly as it is, as `reset_index` leaves the workbooks - two folders,
/// two independent clears. No file in either folder is touched.
#[tauri::command]
pub fn reset_data_index(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    let mut index = open_index(&app)?;
    index.clear_tabular_inventories()?;
    index.clear_tabular_workbooks()?;
    index.clear_profiles(file_profile::build::Domain::Data)?;
    state.data_file_hashes.forget_all();
    Ok(())
}

/// Append the renames a pass made to `renamed-files.jsonl`, as they happen: a pass that fails
/// afterwards must not leave a renamed file with no record of what it used to be.
fn log_renames(app: &AppHandle, renamed: &[filename_sanitizer::Renamed]) {
    if let Ok(directory) = app.path().app_local_data_dir() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs() as i64)
            .unwrap_or(0);
        let _ =
            filename_sanitizer::append_log(&directory.join("renamed-files.jsonl"), renamed, now);
    }
}

#[derive(Debug, Serialize, Default)]
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
    /// She selected no document, so this answer rests on none. The interface says so under it.
    pub without_documents: bool,
    /// Present when tables were selected and no document: the tabular engine answered, with no
    /// gateway call. `answer` is then empty, as for `folder_answer`.
    pub tabular_answer: Option<TabularAnswer>,
    /// Documents and tables were both selected, but this question was clearly and only about the
    /// data - the tabular classifier recognised it and nothing was left over - so the tabular
    /// engine answered it alone, exactly as tier 2 (`docs/SESSION-DATA-15-Mixed-Routing.md`). The
    /// documents selected alongside the table were never read: no retrieval, no gateway call.
    /// Always false outside that one path.
    pub documents_not_needed: bool,
    /// Documents and tables were both selected, and this question was neither answered by tier 2
    /// alone nor refused: the mixed tier decomposed it, computed and cited across both sides, and
    /// checked the model's prose against what it was actually given
    /// (`docs/SESSION-DATA-16-Mixed-Tier.md`). `None` on every other path.
    pub mixed_answer: Option<mixed_answer::MixedAnswer>,
    /// The figures a document answer states that appear in none of the excerpts it was written from
    /// and not in the question either, as the model wrote them. Empty when every figure is
    /// supported. Reported beside the answer, never edited into it (`number_check`; approved by the
    /// owner on 5 October 2026 after a small model invented a printer price).
    pub unverified_numbers: Vec<String>,
    /// A letter was asked for and everything it needs was found: the plan the user approves before
    /// anything is written (`fill_plan`). `answer` is then empty; nothing was generated by a model.
    pub fill_plan: Option<fill_plan::FillPlan>,
    /// Where this question spent its time and how much it looked at: numbers and machine codes
    /// only, never a name or a passage (`knowledge::diagnostics`). Always present on an answer;
    /// nothing in the interface shows it yet.
    pub diagnostics: Option<RetrievalDiagnostics>,
    /// What the knowledge base did to the search, when there is something to tell the user: an
    /// applied reduction (and who asked for it), or names that need a question. `None` otherwise;
    /// in `suggest` an unapplied reduction is invisible by design.
    pub targeting: Option<TargetingReport>,
    /// What this answer did with each workbook the question named, implied or the user ticked: a
    /// card sent, a figure computed, a plan prepared, or why a workbook did not serve (TABULAR plan
    /// 4.3, `workbook_use::trace`). Machine codes; the interface writes the lines. Set by every door
    /// of a question with tables selected; empty for a question on documents alone, where no table
    /// is ticked and none can be used.
    pub workbook_use: Vec<WorkbookUse>,
    /// the "Explain the calculation" button: the model's explanation of the method behind a figure the program
    /// computed again, added under the figure on screen (TABULAR step 06). `answer` is then empty and
    /// the figure of the entry is untouched. `None` on every other path.
    pub explanation: Option<calculation_follow_up::Explanation>,
    /// The state a table answer leaves for a short follow-up ("and for March?"): names and filter
    /// values only. The interface keeps it on the entry and sends the last one back with the next
    /// question, where it is untrusted (TABULAR step 11). `None` unless the answer is a figure the
    /// engine computed from an operation that can be continued.
    pub table_carry_over: Option<TableCarryOver>,
    /// What this answer took from the previous question (a follow-up): the "Understood as" line marks it.
    pub carried_over: Option<CarriedParts>,
}

impl AskAnswer {
    /// Records what the carry-over came to for this answer (TABULAR step 11): the state it leaves and what it
    /// inherited, only when the answer carries the figure it describes, and the trace line either way.
    fn with_carry_over(mut self, report: CarryReport) -> Self {
        let figure = match (&self.tabular_answer, &self.mixed_answer) {
            (Some(answer), _) => Some(answer),
            (None, Some(mixed)) => mixed.table.as_ref(),
            _ => None,
        };
        if let (Some(TabularAnswer::Value { file, .. }), Some(next)) = (figure, &report.next) {
            if *file == next.workbook {
                self.table_carry_over = report.next.clone();
                self.carried_over = report.inherited.clone();
            }
        }
        workbook_use::note_carry_over(&mut self.workbook_use, &report);
        self
    }
}

/// Every question, with or without documents. Retrieval, then a sourced chat answer, refusing
/// rather than answering when the index does not carry enough evidence (`docs/ARCHITECTURE.md`);
/// or, when she selected no document, an answer without excerpts that says so
/// (`docs/SELECTION-AND-MEMORY.md`).
///
/// The whole chain is stoppable, not only the chat leg: embedding the question is what runs while
/// the spinner shows, which is precisely the moment she wants the button to answer.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
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
    // The conversation so far, as she saw it. Untrusted: `conversation::fit_history` keeps only
    // answered questions and fits them to the chosen model's budget.
    history: Option<Vec<ChatTurn>>,
    // What the knowledge base is asked to do with this question: entities the user picked as chips,
    // the previous answer's entities for a short follow-up, and `off` for the widen action. Absent
    // means the automatic behaviour of the knowledge mode in the settings.
    targeting: Option<TargetingRequest>,
    // the "Explain the calculation" button or "Demander a l'IA" under a computed figure: the figure is computed
    // again by the program from the original question and the scope it was given under, never taken
    // from the interface (`calculation_follow_up`, TABULAR step 06).
    calculation_follow_up: Option<CalculationFollowUp>,
    // The state the last table answer of this conversation left (TABULAR step 11): untrusted, checked
    // against the real selection and inventory before it is used.
    table_carry_over: Option<TableCarryOver>,
    // What a figure does with the rows a spreadsheet hides or filters out (TABULAR step 13): the
    // user's choice on an answer, otherwise the one a short follow-up inherits from the figure it
    // continues, otherwise the rows are counted and said.
    row_policy: Option<RowPolicy>,
    on_event: Channel<ChatStreamEvent>,
) -> Result<AskAnswer, AppError> {
    let policy = resolve_row_policy(row_policy, table_carry_over.as_ref(), &question);
    if let Ok(mut current) = state.row_policy.lock() {
        *current = policy;
    }
    let mut stopped = state.cancellation.begin();
    // Cleared first: a workbook load recorded on this thread by something else must not be
    // counted as this question's.
    let _ = diagnostics::take_workbook_loads();
    let timer = StageTimer::start();
    let mut measured = RetrievalDiagnostics::new(RetrievalPath::Documents);
    let result = until_stopped(
        &mut stopped,
        sourced_answer(
            &app,
            state.inner(),
            Question {
                text: question,
                skip_deterministic: skip_deterministic.unwrap_or(false),
                scope: scope.unwrap_or_else(|| AnalysisScope::whole_folder(0)),
                history: history.unwrap_or_default(),
                targeting: targeting.unwrap_or_default(),
                calculation_follow_up,
                table_carry_over,
            },
            &on_event,
            &mut measured,
        ),
    )
    .await;
    measured.total_ms = timer.total_ms();
    // A question she stopped is not a timing worth keeping: it measures her patience.
    if !matches!(result, Err(AppError::ChatCancelled)) {
        log_retrieval_timing(&app, state.inner(), &measured);
        record_knowledge_row(&app, &measured);
    }
    let result = result.map(|mut answer| {
        answer.diagnostics = Some(measured);
        note_layout_of(&app, state.inner(), &mut answer);
        // A reading that left rows out is part of the state a short follow-up continues.
        if policy == RowPolicy::Exclude {
            if let Some(carry) = answer.table_carry_over.as_mut() {
                carry.row_policy = Some(RowPolicy::Exclude);
            }
        }
        answer
    });
    if let Ok(mut current) = state.row_policy.lock() {
        *current = RowPolicy::default();
    }
    result
}

/// What a figure does with the rows a spreadsheet hides or filters out: the choice made on an
/// answer, otherwise the reading the previous figure was given when this question is short enough to
/// be a follow-up of it, otherwise the rows are counted and said.
fn resolve_row_policy(
    requested: Option<RowPolicy>,
    carry: Option<&TableCarryOver>,
    question: &str,
) -> RowPolicy {
    requested
        .or_else(|| {
            let short = question.split_whitespace().count() <= conversation::FOLLOW_UP_MAX_WORDS;
            carry.filter(|_| short).and_then(|carry| carry.row_policy)
        })
        .unwrap_or_default()
}

/// What the layout of the sheets did to an answer, written into its trace (`workbook_use::note_layout`).
/// A failure to read the folders only means the lines are missing, never that the answer fails.
fn note_layout_of(app: &AppHandle, state: &AppState, answer: &mut AskAnswer) {
    let Ok(index) = open_index(app) else {
        return;
    };
    let Ok(views) = family_views_of(state, &index) else {
        return;
    };
    let Some(data) = views.data.as_ref() else {
        return;
    };
    let figure = match (&answer.tabular_answer, &answer.mixed_answer) {
        (Some(figure), _) => Some(figure),
        (None, Some(mixed)) => mixed.table.as_ref(),
        _ => None,
    };
    workbook_use::note_layout(&mut answer.workbook_use, data, figure);
}

/// One row of numbers and codes in `kb_diagnostics` for a question that went through the knowledge
/// base. A row that cannot be written never fails the question it describes.
fn record_knowledge_row(app: &AppHandle, measured: &RetrievalDiagnostics) {
    if measured.targeting.is_none() {
        return;
    }
    if let Ok(index) = open_index(app) {
        let _ = crate::knowledge::diagnostics::record_question(
            index.connection(),
            now_seconds(),
            measured,
        );
    }
}

/// What a developer panel reads: the recorded questions added up (`knowledge::diagnostics`).
/// Counts and ratios only; never a name, a question or a file name.
#[tauri::command]
pub fn knowledge_diagnostics(
    app: AppHandle,
    limit: Option<usize>,
) -> Result<crate::knowledge::diagnostics::KnowledgeAggregates, AppError> {
    let index = open_index(&app)?;
    crate::knowledge::diagnostics::aggregate(
        index.connection(),
        limit.unwrap_or(500).clamp(1, 2_000),
    )
}

/// Both families across the two folders the settings name (`family_view`): the documents wherever they
/// sit, the workbooks wherever they sit, each file under the key it has in the index. What a selection,
/// a retrieval or a table question reads; the explorer's own listing stays per folder. A family is
/// absent when its own folder is not chosen.
fn family_views_of(state: &AppState, index: &IndexStore) -> Result<FamilyViews, AppError> {
    let (work_folder, data_folder) =
        state.read(|settings| (settings.work_folder.clone(), settings.data_folder.clone()))?;
    let mut views = family_view::family_views(
        work_folder.as_deref().map(Path::new),
        data_folder.as_deref().map(Path::new),
        Some(index),
        &state.file_hashes,
        &state.data_file_hashes,
    )?;
    // Every tier of the question being answered reads the tables under the same policy.
    let policy = state
        .row_policy
        .lock()
        .map(|policy| *policy)
        .unwrap_or_default();
    views.data = views.data.take().map(|data| data.with_row_policy(policy));
    Ok(views)
}

/// The names of the current selection that begin with `prefix`, for the composer's `@` picker
/// (`knowledge::picker`), and the names that exist only in analysed files she did not select.
///
/// A cheap read the user starts by typing: no model, no file content, no write. Documents and
/// workbooks are resolved exactly as a question resolves them, so the picker offers what the next
/// question could use and nothing else. With the knowledge base off, or nothing selected, it finds
/// nothing.
#[tauri::command]
pub fn knowledge_search_entities(
    app: AppHandle,
    state: State<'_, AppState>,
    prefix: String,
    scope: AnalysisScope,
    limit: Option<usize>,
) -> Result<EntitySearch, AppError> {
    let mode = state.read(|settings| settings.knowledge_mode)?;
    if !mode.reads_files() || prefix.trim().is_empty() {
        return Ok(EntitySearch::default());
    }
    let index = open_index(&app)?;

    let views = family_views_of(&state, &index)?;
    let mut selected_documents_list = Vec::new();
    let mut analysed_documents = Vec::new();
    if let Some(full) = views.documents {
        let as_files = |inventory: &WorkFolderInventory| -> Vec<SelectedFile> {
            inventory
                .indexed_files()
                .into_iter()
                .map(|record| SelectedFile::new(&record.relative_path, &record.id))
                .collect()
        };
        analysed_documents = as_files(&full);
        selected_documents_list = as_files(&scope.resolve(&full).inventory);
    }
    let mut selected_workbooks_list = Vec::new();
    let mut analysed_workbooks = Vec::new();
    if let Some(folder) = views.data {
        analysed_workbooks = selected_workbook_files(&folder, &ScopeMode::WholeFolder);
        selected_workbooks_list = selected_workbook_files(&folder, &scope.data_mode);
    }

    picker::search_entities(
        index.connection(),
        &prefix,
        PickerFiles {
            documents: &selected_documents_list,
            data: &selected_workbooks_list,
        },
        PickerFiles {
            documents: &analysed_documents,
            data: &analysed_workbooks,
        },
        limit.unwrap_or(picker::PICKER_LIMIT),
    )
}

/// The files a selection covers, in the forms the read-only services of the knowledge base take.
struct ResolvedSelection {
    documents: Vec<SelectedFile>,
    /// The documents folder narrowed to the selection, for the "does this read as a file name" test.
    scoped_documents: Option<WorkFolderInventory>,
    data: Option<DataFolder>,
    workbooks: Vec<WorkbookChoice>,
}

impl ResolvedSelection {
    fn workbook_files(&self) -> Vec<SelectedFile> {
        self.workbooks
            .iter()
            .map(|choice| SelectedFile::new(&choice.relative_path, &choice.content_id))
            .collect()
    }

    /// Whether the folder router would take this text for the name of a file of either folder.
    fn names_a_file(&self, text: &str) -> bool {
        let in_documents = self.scoped_documents.as_ref().is_some_and(|inventory| {
            FileReferenceResolver::new(inventory)
                .resolve_in_question(text)
                .is_some()
        });
        let in_data = self.data.as_ref().is_some_and(|folder| {
            FileReferenceResolver::new(folder.files())
                .resolve_in_question(text)
                .is_some()
        });
        in_documents || in_data
    }
}

/// Resolve a selection exactly as a question resolves it (the inventory with the application's hash
/// cache, `scope.resolve`, the usable workbooks), so what a read-only service offers is what the
/// next question could use.
fn resolve_selection(
    state: &State<'_, AppState>,
    index: &IndexStore,
    scope: &AnalysisScope,
) -> Result<ResolvedSelection, AppError> {
    let views = family_views_of(state, index)?;
    let mut resolved = ResolvedSelection {
        documents: Vec::new(),
        scoped_documents: None,
        data: None,
        workbooks: Vec::new(),
    };
    if let Some(full) = views.documents {
        let scoped = scope.resolve(&full).inventory;
        resolved.documents = scoped
            .indexed_files()
            .into_iter()
            .map(|record| SelectedFile::new(&record.relative_path, &record.id))
            .collect();
        resolved.scoped_documents = Some(scoped);
    }
    if let Some(folder) = views.data {
        resolved.workbooks = tabular_answer::selected_workbook_choices(&folder, &scope.data_mode);
        resolved.data = Some(folder);
    }
    Ok(resolved)
}

/// Questions worth asking about the current selection, as data (`knowledge::suggest`): every one
/// has passed a dry run on the user's own files, none contains a sentence. A cheap read the
/// interface starts when it shows an empty conversation or an analysis summary: no model, no
/// request to the AI, no write. With the knowledge base off, or nothing selected, there are none.
///
/// Names are left out while `knowledge_hide_names_in_suggestions` is on, unless the user asked to
/// see them (`reveal_names`, held by the interface for the session): a screen can be seen by
/// someone waiting in the room.
#[tauri::command(async)]
pub fn knowledge_suggest_questions(
    app: AppHandle,
    state: State<'_, AppState>,
    scope: AnalysisScope,
    reveal_names: Option<bool>,
) -> Result<SuggestionSet, AppError> {
    let (mode, hide_names, locale, packs) = state.read(|settings| {
        (
            settings.knowledge_mode,
            settings.knowledge_hide_names_in_suggestions,
            settings.locale.clone(),
            settings.knowledge_pack_ids(),
        )
    })?;
    if !mode.reads_files() {
        return Ok(SuggestionSet::default());
    }
    let locale = locale.unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string());
    let names_allowed = !hide_names || reveal_names.unwrap_or(false);
    let index = open_index(&app)?;
    let selection = resolve_selection(&state, &index, &scope)?;

    let key = suggest::cache_key(
        index.connection(),
        &selection.documents,
        &selection.workbooks,
        &locale,
        names_allowed,
    )?;
    if let Some(cached) = state.suggestions.get(key) {
        return Ok(cached);
    }
    let vocabulary = state.vocabulary(&locale, &packs)?;
    let set = suggest::suggest(&SuggestionInput {
        index: &index,
        vocabulary: &vocabulary,
        data: selection.data.as_ref(),
        documents: &selection.documents,
        workbooks: &selection.workbooks,
        locale: &locale,
        names_allowed,
        names_a_file: &|text| selection.names_a_file(text),
        limit: suggest::MAX_SUGGESTIONS,
        max_dry_runs: suggest::MAX_DRY_RUNS,
        max_cold_read_bytes: suggest::COLD_READ_LIMIT_BYTES,
    })?;
    state.suggestions.put(key, set.clone());
    Ok(set)
}

/// What the knowledge base knows about one name **inside the current selection** (`knowledge::sheet`):
/// the other spellings, the files of the selection that mention it with the pages or the sheet and
/// column, and only a count for everything outside the selection. Opened from a chip or from the
/// name list. A read: no model, no write.
#[tauri::command(async)]
pub fn knowledge_entity_card(
    app: AppHandle,
    state: State<'_, AppState>,
    entity_id: i64,
    scope: AnalysisScope,
) -> Result<Option<EntitySheet>, AppError> {
    let (mode, locale, packs) = state.read(|settings| {
        (
            settings.knowledge_mode,
            settings.locale.clone(),
            settings.knowledge_pack_ids(),
        )
    })?;
    if !mode.reads_files() {
        return Ok(None);
    }
    let locale = locale.unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string());
    let index = open_index(&app)?;
    let selection = resolve_selection(&state, &index, &scope)?;
    let workbook_files = selection.workbook_files();
    let vocabulary = state.vocabulary(&locale, &packs)?;
    sheet::entity_sheet(
        index.connection(),
        &vocabulary.packs,
        entity_id,
        &QuerySelection {
            documents: &selection.documents,
            data: &workbook_files,
        },
    )
}

/// Name suggestions for a dictated transcript (`speech::repair`): the words that probably meant a
/// name of the **current selection**, as `heard`, the span and the candidates. A read: no model, no
/// write to the knowledge base, and the text is not changed. With the knowledge base off, or on any
/// failure, the answer is empty, so the dictation works as a plain dictation.
///
/// While the setting hides names and `reveal_names` is not set, only a count goes back: the names
/// of the selection never reach the window.
#[tauri::command(async)]
pub fn speech_repair_names(
    app: AppHandle,
    state: State<'_, AppState>,
    transcript: String,
    scope: AnalysisScope,
    case_reliable: bool,
    reveal_names: Option<bool>,
) -> Result<repair::RepairSet, AppError> {
    let (mode, hide_names, locale, packs) = state.read(|settings| {
        (
            settings.knowledge_mode,
            settings.knowledge_hide_names_in_suggestions,
            settings.locale.clone(),
            settings.knowledge_pack_ids(),
        )
    })?;
    if !mode.reads_files() {
        return Ok(repair::RepairSet::default());
    }
    let locale = locale.unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string());
    let names_allowed = !hide_names || reveal_names.unwrap_or(false);
    Ok(repair::or_nothing((|| {
        let index = open_index(&app)?;
        let selection = resolve_selection(&state, &index, &scope)?;
        let workbook_files = selection.workbook_files();
        let vocabulary = state.vocabulary(&locale, &packs)?;
        repair::propose_for(
            index.connection(),
            &vocabulary,
            &transcript,
            Some(&QuerySelection {
                documents: &selection.documents,
                data: &workbook_files,
            }),
            case_reliable,
            names_allowed,
        )
    })()))
}

// ---------------------------------------------------------------------------------------------
// The knowledge management dialog (lot 8)
//
// An administrative place, not part of the chat: it is opened from the settings and nothing in the
// conversation can reach it. Every command is one read or one transaction (`knowledge::manage`);
// none reads or writes a file of either folder, and none starts any work on the documents: a
// change that adds or removes a name only raises the generation of the name list, and the
// documents it makes due are read again by the next Analyse the user starts. They run off the
// main thread because a busy database (an Analyse writing) can make one wait a few seconds.
// ---------------------------------------------------------------------------------------------

/// The words a manual name is read with: the packs of the language in use, plus the type
/// vocabulary of every shipped pack (a module never changes a type), and the phonetic encoder.
fn manage_vocabulary(state: &State<'_, AppState>) -> Result<(NameTools, PackSet), AppError> {
    let (locale, packs) =
        state.read(|settings| (settings.locale.clone(), settings.knowledge_pack_ids()))?;
    let locale = locale.unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string());
    let ids: Vec<&str> = packs.iter().map(String::as_str).collect();
    let set = PackSet::load_with_type_vocabulary(&locale, &ids)?;
    Ok((NameTools::from_packs(&set, &locale), set))
}

#[tauri::command(async)]
pub fn knowledge_overview(app: AppHandle) -> Result<manage::Overview, AppError> {
    manage::overview(open_index(&app)?.connection())
}

#[tauri::command(async)]
pub fn knowledge_list_entities(
    app: AppHandle,
    query: manage::ListQuery,
) -> Result<manage::EntityPage, AppError> {
    manage::list_entities(open_index(&app)?.connection(), &query)
}

#[tauri::command(async)]
pub fn knowledge_entity_sheet(
    app: AppHandle,
    entity_id: i64,
) -> Result<manage::EntityDetail, AppError> {
    manage::entity_detail(open_index(&app)?.connection(), entity_id)
}

#[tauri::command(async)]
pub fn knowledge_possible_matches(
    app: AppHandle,
    page: Option<usize>,
) -> Result<manage::PossiblePage, AppError> {
    manage::possible_matches(open_index(&app)?.connection(), page.unwrap_or(0))
}

#[tauri::command(async)]
pub fn knowledge_health(app: AppHandle) -> Result<manage::HealthReport, AppError> {
    manage::health_report(open_index(&app)?.connection())
}

#[tauri::command(async)]
pub fn knowledge_types_and_roles(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<manage::TypesAndRoles, AppError> {
    let (_, packs) = manage_vocabulary(&state)?;
    manage::types_and_roles(open_index(&app)?.connection(), &packs)
}

#[tauri::command(async)]
pub fn knowledge_create_entity(
    app: AppHandle,
    state: State<'_, AppState>,
    input: manage::NewManualEntity,
) -> Result<manage::Outcome, AppError> {
    let (tools, _) = manage_vocabulary(&state)?;
    manage::create_entity(open_index(&app)?.connection(), &tools, &input)
}

#[tauri::command(async)]
pub fn knowledge_edit_entity(
    app: AppHandle,
    state: State<'_, AppState>,
    entity_id: i64,
    edit: manage::EditEntity,
) -> Result<manage::Outcome, AppError> {
    let (tools, _) = manage_vocabulary(&state)?;
    manage::edit_entity(open_index(&app)?.connection(), &tools, entity_id, &edit)
}

#[tauri::command(async)]
pub fn knowledge_add_alias(
    app: AppHandle,
    state: State<'_, AppState>,
    entity_id: i64,
    text: String,
) -> Result<manage::Outcome, AppError> {
    let (tools, _) = manage_vocabulary(&state)?;
    manage::add_alias(open_index(&app)?.connection(), &tools, entity_id, &text)
}

#[tauri::command(async)]
pub fn knowledge_remove_alias(app: AppHandle, alias_id: i64) -> Result<manage::Outcome, AppError> {
    manage::remove_alias(open_index(&app)?.connection(), alias_id)
}

#[tauri::command(async)]
pub fn knowledge_set_attribute(
    app: AppHandle,
    state: State<'_, AppState>,
    entity_id: i64,
    key: String,
    value: String,
) -> Result<manage::Outcome, AppError> {
    let (tools, _) = manage_vocabulary(&state)?;
    manage::set_attribute(
        open_index(&app)?.connection(),
        &tools,
        entity_id,
        &key,
        &value,
    )
}

#[tauri::command(async)]
pub fn knowledge_remove_attribute(
    app: AppHandle,
    entity_id: i64,
    key: String,
    value: String,
) -> Result<manage::Outcome, AppError> {
    manage::remove_attribute(open_index(&app)?.connection(), entity_id, &key, &value)
}

#[tauri::command(async)]
pub fn knowledge_merge(
    app: AppHandle,
    survivor: i64,
    absorbed: i64,
) -> Result<manage::Outcome, AppError> {
    manage::merge(open_index(&app)?.connection(), survivor, absorbed)
}

#[tauri::command(async)]
pub fn knowledge_unmerge(app: AppHandle, entity_id: i64) -> Result<manage::Outcome, AppError> {
    manage::unmerge(open_index(&app)?.connection(), entity_id)
}

#[tauri::command(async)]
pub fn knowledge_split(
    app: AppHandle,
    state: State<'_, AppState>,
    request: manage::SplitRequest,
) -> Result<manage::Outcome, AppError> {
    let (tools, _) = manage_vocabulary(&state)?;
    manage::split(open_index(&app)?.connection(), &tools, &request)
}

#[tauri::command(async)]
pub fn knowledge_delete_entity(
    app: AppHandle,
    entity_id: i64,
) -> Result<manage::Outcome, AppError> {
    manage::delete_entity(open_index(&app)?.connection(), entity_id)
}

#[tauri::command(async)]
pub fn knowledge_restore_entity(
    app: AppHandle,
    entity_id: i64,
) -> Result<manage::Outcome, AppError> {
    manage::restore_entity(open_index(&app)?.connection(), entity_id)
}

#[tauri::command(async)]
pub fn knowledge_accept_match(
    app: AppHandle,
    survivor: i64,
    absorbed: i64,
) -> Result<manage::Outcome, AppError> {
    manage::accept_possible_match(open_index(&app)?.connection(), survivor, absorbed)
}

#[tauri::command(async)]
pub fn knowledge_dismiss_match(
    app: AppHandle,
    one: i64,
    other: i64,
) -> Result<manage::Outcome, AppError> {
    manage::dismiss_possible_match(open_index(&app)?.connection(), one, other)
}

#[tauri::command(async)]
pub fn knowledge_undo(app: AppHandle) -> Result<manage::Outcome, AppError> {
    manage::undo_last_operation(open_index(&app)?.connection())
}

/// "Reset the whole knowledge base". The interface asks first (`ConfirmDialog`); Rust does what it
/// is told. The difference with the two folder Resets is that this one is about names: they remove
/// what a folder's files taught the base, this one can also remove what a person wrote
/// (`include_manual`). Files, chunks and inventories are never touched by any of the three.
#[tauri::command(async)]
pub fn knowledge_reset(app: AppHandle, include_manual: bool) -> Result<manage::Outcome, AppError> {
    manage::reset(open_index(&app)?.connection(), include_manual)
}

/// Seconds since the epoch by the workstation clock, for a log line. Zero if the clock is before
/// 1970, which is a wrong clock and not a reason to lose the line.
fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// Append one question's timings to `retrieval-timings.jsonl` when `write_timing_log` is on.
/// Numbers and machine codes, no question and no name (`knowledge::diagnostics`). A log that
/// cannot be written never fails the question it describes.
fn log_retrieval_timing(app: &AppHandle, state: &AppState, measured: &RetrievalDiagnostics) {
    if !state
        .read(|settings| settings.write_timing_log)
        .unwrap_or(false)
    {
        return;
    }
    if let Ok(directory) = app.path().app_local_data_dir() {
        let _ = diagnostics::append_timing_line(
            &directory.join("retrieval-timings.jsonl"),
            measured,
            now_seconds(),
        );
    }
}

/// The same for an Analyse pass, to `analysis-timings.jsonl`.
fn log_analysis_timing(app: &AppHandle, state: &AppState, summary: &IndexSummary) {
    let Some(timings) = &summary.timings else {
        return;
    };
    if !state
        .read(|settings| settings.write_timing_log)
        .unwrap_or(false)
    {
        return;
    }
    if let Ok(directory) = app.path().app_local_data_dir() {
        let _ = diagnostics::append_timing_line(
            &directory.join("analysis-timings.jsonl"),
            timings,
            now_seconds(),
        );
    }
}

/// How many workbooks the selection covers: the chosen ones, or every analysed one for "tous".
fn selected_workbooks(folder: &DataFolder, mode: &ScopeMode) -> usize {
    match mode {
        ScopeMode::Explicit(entries) => entries.len(),
        ScopeMode::WholeFolder => folder
            .files()
            .all_files()
            .iter()
            .filter(|record| record.processing_status == ProcessingStatus::Indexed)
            .count(),
    }
}

/// One question and everything it arrived with.
struct Question {
    text: String,
    skip_deterministic: bool,
    scope: AnalysisScope,
    history: Vec<ChatTurn>,
    targeting: TargetingRequest,
    calculation_follow_up: Option<CalculationFollowUp>,
    table_carry_over: Option<TableCarryOver>,
}

/// What the knowledge base needs for one question, read once from the settings: the mode, the
/// clamped policy, the words (kept for the session) and the key for personal identifiers. Built
/// without failing: a vocabulary that cannot load makes the question run as if the base were off.
struct TargetingSetup {
    mode: KnowledgeMode,
    policy: TargetingPolicy,
    vocabulary: Option<Arc<QueryVocabulary>>,
    key: Option<IdentifierKey>,
}

impl TargetingSetup {
    fn load(app: &AppHandle, state: &AppState) -> Self {
        let (mode, policy, locale, packs) = state
            .read(|settings| {
                (
                    settings.knowledge_mode,
                    settings.targeting_policy(),
                    settings.locale.clone(),
                    settings.knowledge_pack_ids(),
                )
            })
            .unwrap_or((
                KnowledgeMode::Off,
                TargetingPolicy::default(),
                None,
                Vec::new(),
            ));
        if !mode.reads_files() {
            return Self {
                mode,
                policy,
                vocabulary: None,
                key: None,
            };
        }
        let locale = locale.unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string());
        // A question writes nothing, not even a key file: with no key there is no hashed identifier.
        let key = app
            .path()
            .app_local_data_dir()
            .ok()
            .and_then(|directory| IdentifierKey::load_if_present(&directory));
        Self {
            mode,
            policy,
            vocabulary: state.vocabulary(&locale, &packs).ok(),
            key,
        }
    }

    fn plan(
        &self,
        index: &IndexStore,
        request: &TargetingRequest,
        question: &str,
        documents: SideInput<'_>,
        data: SideInput<'_>,
    ) -> TargetingPlan {
        match (&self.vocabulary, self.mode.reads_files()) {
            (Some(vocabulary), true) => targeting::plan(
                index,
                &TargetingTools {
                    vocabulary: vocabulary.as_ref(),
                    key: self.key.as_ref(),
                    policy: self.policy,
                },
                self.mode,
                request,
                question,
                documents,
                data,
            ),
            _ => TargetingPlan::unplanned(
                self.mode,
                self.policy,
                documents.files.len(),
                data.files.len(),
                match self.mode {
                    KnowledgeMode::Off => SkipReason::Off,
                    _ => SkipReason::LookupError,
                },
            ),
        }
    }
}

/// The selected documents among `paths`, as the knowledge base keys them (path and content id).
fn selected_documents(inventory: &WorkFolderInventory, paths: &[String]) -> Vec<SelectedFile> {
    paths
        .iter()
        .filter_map(|path| inventory.find_by_relative_path(path))
        .map(|record| SelectedFile::new(&record.relative_path, &record.id))
        .collect()
}

/// The usable workbooks the data selection covers, as the knowledge base keys them.
fn selected_workbook_files(folder: &DataFolder, mode: &ScopeMode) -> Vec<SelectedFile> {
    tabular_answer::selected_workbook_files(folder, mode)
        .into_iter()
        .map(|(path, id)| SelectedFile::new(&path, &id))
        .collect()
}

/// An answer as the model wrote it, with how long it took to start and to finish.
struct Generation {
    text: String,
    first_token_ms: Option<u64>,
    total_ms: u64,
}

/// What the gateway needs to write one answer, read once from the settings.
struct Writer<'a> {
    state: &'a AppState,
    server_url: String,
    model_alias: String,
    locale: Option<String>,
    idle_timeout: std::time::Duration,
    budget: ContextBudget,
}

impl Writer<'_> {
    /// As much of the conversation as fits, then one turn carrying everything this question needs
    /// to be answered: the tier's instruction, its grounding material (excerpts, or nothing for
    /// the no-documents tier), and the question itself, in that order. Streams the answer to the
    /// interface and returns it whole.
    ///
    /// Deliberately **not** a leading system turn. A small local model weighs a nearby turn more
    /// than a distant one, and history sitting between the grounding material and the question -
    /// which a system-first layout would produce - is exactly what let a model drift onto a prior
    /// turn's topic instead of the current excerpts (`docs/SELECTION-AND-MEMORY.md`, "why the
    /// grounding material moved next to the question"). The gateway still prepends its own system
    /// turn of stable, tier-independent rules (`prompts.py`); this is what comes right after the
    /// conversation, immediately before generation starts.
    async fn write(
        &self,
        instruction: String,
        history: &[ChatTurn],
        question: String,
        temperature: Option<f32>,
        on_event: &Channel<ChatStreamEvent>,
    ) -> Result<Generation, AppError> {
        let final_turn = format!("{instruction}\n\nQuestion: {question}");
        let mut turns = conversation::fit_history(
            history,
            self.budget.history_chars(final_turn.chars().count()),
        );
        turns.push(ChatTurn {
            role: "user".to_string(),
            content: final_turn,
        });

        // From the moment the prompt is sent: the first word is what she waits for, and on a slow
        // machine it is most of the wait (`docs/HARDWARE.md`).
        let started = std::time::Instant::now();
        let first_token = std::sync::OnceLock::<u64>::new();
        let answer = self
            .state
            .gateway
            .chat_at(
                &self.server_url,
                &self.model_alias,
                self.locale.as_deref(),
                &turns,
                self.idle_timeout,
                temperature,
                |delta| {
                    first_token.get_or_init(|| elapsed_ms(started.elapsed()));
                    let _ = on_event.send(ChatStreamEvent::Delta {
                        text: delta.to_string(),
                    });
                },
            )
            .await?;
        let total_ms = elapsed_ms(started.elapsed());

        let _ = on_event.send(ChatStreamEvent::Completed {
            text: answer.clone(),
        });
        Ok(Generation {
            text: answer,
            first_token_ms: first_token.get().copied(),
            total_ms,
        })
    }
}

async fn sourced_answer(
    app: &AppHandle,
    state: &AppState,
    asked: Question,
    on_event: &Channel<ChatStreamEvent>,
    measured: &mut RetrievalDiagnostics,
) -> Result<AskAnswer, AppError> {
    let Question {
        text: question,
        skip_deterministic,
        scope,
        history,
        targeting: targeting_request,
        calculation_follow_up,
        table_carry_over,
    } = asked;
    let carry = table_carry_over.as_ref();

    // Under a computed figure: the figure is computed again by the program and explained or analysed,
    // whatever else the question could be taken for (TABULAR step 06).
    if let Some(follow_up) = &calculation_follow_up {
        measured.path = match scope.tier() {
            GroundingTier::DocumentsAndTables => RetrievalPath::Mixed,
            _ => RetrievalPath::Tabular,
        };
        return calculation_follow_up_tier(
            app,
            state,
            &question,
            &scope,
            &history,
            &targeting_request,
            follow_up,
            on_event,
            measured,
        )
        .await;
    }

    // Which tier this question is on, decided from the two selections before anything is read
    // (`docs/SELECTION-AND-MEMORY.md`, the grounding priority chain).
    match scope.tier() {
        // D7 step 1 (`docs/DECISIONS.md`, `docs/SESSION-DATA-15-Mixed-Routing.md`): a question
        // that is clearly and only about the data is answered by the tabular engine alone, before
        // any file content is read for retrieval - kept exactly as session 15 left it, and still
        // tried first. Session 16's own mixed tier (`mixed_tier`, `mixed_answer.rs`) starts
        // exactly at its `None`: everything that question did not already answer.
        GroundingTier::DocumentsAndTables => {
            // "Demander a l'IA" (`skip_deterministic`) asks for the model's reading instead of the
            // engine's alone, so the data-only shortcut is skipped and the mixed tier answers; it
            // was ignored here until HAP-1 (BUG-04), which left the button doing nothing.
            measured.path = RetrievalPath::Mixed;
            if skip_deterministic {
                // "Demander a l'IA" under the engine's refusal of a question about the tables (a nudge): the
                // mixed tier hands a refusal back at once, with no model call, so the button did nothing.
                // The model is asked to understand the question as in the tables-only tier, which never
                // reads the documents (found by the owner's test of TABULAR step 06, part F).
                if data_side_is_a_nudge(app, state, &question, &scope, carry) {
                    measured.path = RetrievalPath::Tabular;
                    return tabular_tier(
                        app,
                        state,
                        &question,
                        &scope,
                        true,
                        &history,
                        &targeting_request,
                        carry,
                        on_event,
                        measured,
                    )
                    .await;
                }
                return mixed_tier(
                    app,
                    state,
                    &question,
                    &scope,
                    &history,
                    &targeting_request,
                    on_event,
                    measured,
                    None,
                    carry,
                )
                .await;
            }
            // A request for a letter, with the template and the rows it needs found: a plan to
            // approve, not an answer (`fill_plan`, lot E). Nothing is written here.
            match fill_tier(app, state, &question, &scope, measured)? {
                FillStart::Answer(answer) => return Ok(*answer),
                // No template fits a workbook the question names: the model drafts one from the
                // column names alone (TABULAR step 09).
                FillStart::NeedsDraft(job) => {
                    return fill_draft_tier(state, &question, *job, measured).await
                }
                FillStart::None => {}
            }
            // "How many documents do I have?" is a question about the folder, not a row count of
            // a workbook (KBD-23).
            if let Some(answer) =
                mixed_folder_tier(app, state, &question, &scope, on_event, measured)?
            {
                return Ok(answer);
            }
            match mixed_data_only_tier(
                app,
                state,
                &question,
                &scope,
                &targeting_request,
                carry,
                measured,
            )
            .await?
            {
                Some(answer) => return Ok(answer),
                None => {
                    return mixed_tier(
                        app,
                        state,
                        &question,
                        &scope,
                        &history,
                        &targeting_request,
                        on_event,
                        measured,
                        None,
                        carry,
                    )
                    .await
                }
            }
        }
        GroundingTier::TablesOnly => {
            measured.path = RetrievalPath::Tabular;
            return tabular_tier(
                app,
                state,
                &question,
                &scope,
                skip_deterministic,
                &history,
                &targeting_request,
                carry,
                on_event,
                measured,
            )
            .await;
        }
        GroundingTier::Documents => {}
    }
    let mut stage = StageTimer::start();
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
    if work_folder.is_none() {
        return Err(AppError::NoWorkFolderSet);
    }
    // The budget of the model chosen for this question, from the last health check. Switching
    // models mid-conversation changes this and nothing else.
    let budget = state
        .model_budgets
        .lock()
        .map_err(|_| AppError::Internal)?
        .for_alias(&model_alias);
    let writer = Writer {
        state,
        server_url: server_url.clone(),
        model_alias,
        locale: locale.clone(),
        idle_timeout,
        budget,
    };

    let index = open_index(app)?;
    let full_inventory = family_views_of(state, &index)?
        .documents
        .ok_or(AppError::NoWorkFolderSet)?;
    measured.inventory_ms = stage.lap_ms();
    // From here on the conversation sees the scope's inventory and nothing wider: the router, the
    // per-document list, the counts and the context all read it, and retrieval is held to its
    // members below. The full inventory is kept for one thing: telling a file that is not
    // selected apart from a file that does not exist (`folder_questions::route_in_selection`).
    let resolution = scope.resolve(&full_inventory);
    let narrowed = resolution.narrowed;
    let no_documents_chosen = resolution.no_documents_chosen;
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
    measured.selected_documents = scoped_paths.len();
    measured.scope_ms = stage.lap_ms();

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
        /// She selected no document: nothing is searched and nothing is embedded.
        WithoutDocuments,
    }

    let selection = folder_questions::Selection {
        full: &full_inventory,
        scoped: &inventory,
        narrowed,
        no_documents_chosen,
    };
    let route = folder_questions::route_in_selection(
        &selection,
        &index,
        &question,
        locale.unwrap_or(settings::DEFAULT_LOCALE),
    );
    measured.routing_ms = stage.lap_ms();
    let plan = match route {
        // Asked for again, with the model this time. With no document selected there is nothing
        // to give it but the conversation.
        _ if skip_deterministic && no_documents_chosen => Plan::WithoutDocuments,
        // A question the folder answers is still a question about the folder, so the model gets
        // every document rather than the handful that rank highest - the same treatment
        // "summarise each document" gets.
        _ if skip_deterministic => Plan::EveryDocument(scoped_paths.clone()),
        QuestionRoute::Deterministic(answer) => {
            measured.path = RetrievalPath::FolderAnswer;
            let _ = on_event.send(ChatStreamEvent::FolderAnswer {
                answer: answer.clone(),
            });
            // A folder answer is read from the filesystem, not from the index, so it already
            // accounts for every file there is (`unanalysed_files` stays 0): nothing about it is
            // waiting on a pass.
            return Ok(AskAnswer {
                folder_answer: Some(answer),
                scope_outdated,
                ..AskAnswer::default()
            });
        }
        QuestionRoute::SetSearch(set) => {
            // A word search of the selected files: read from the index, so no embedding call and no
            // generation. Answered the same with the AI machine stopped.
            let (identifiers, names_vocabulary) = set_search_tools(state, &set.text)?;
            let scoped_files: Vec<InventoryFile> = inventory
                .all_files()
                .iter()
                .map(InventoryFile::from)
                .collect();
            // A name that is exactly one entity of the knowledge base also finds the files linked to it. When
            // the vocabulary cannot be loaded the sentence is the word search it always was.
            let names = names_vocabulary
                .as_ref()
                .map(|vocabulary| finder::names::KnowledgeNames {
                    connection: index.connection(),
                    vocabulary,
                });
            let answer = match &names {
                Some(names) => finder::set_search::answer_with_names(
                    index.connection(),
                    &scoped_files,
                    &set,
                    identifiers,
                    names,
                )?,
                None => finder::set_search::answer(
                    index.connection(),
                    &scoped_files,
                    &set,
                    identifiers,
                )?,
            };
            measured.path = RetrievalPath::DocumentSearch;
            let unanalysed = match &answer {
                FolderAnswer::DocumentSearch { coverage, .. } => coverage.unanalysed,
                _ => 0,
            };
            let _ = on_event.send(ChatStreamEvent::FolderAnswer {
                answer: answer.clone(),
            });
            return Ok(AskAnswer {
                folder_answer: Some(answer),
                unanalysed_files: unanalysed,
                scope_outdated,
                ..AskAnswer::default()
            });
        }
        QuestionRoute::TargetedRetrieval { file } => Plan::OneFile(file.relative_path.clone()),
        QuestionRoute::PerDocumentRetrieval { files } => {
            Plan::EveryDocument(files.into_iter().map(|file| file.relative_path).collect())
        }
        QuestionRoute::GlobalRetrieval if no_documents_chosen => Plan::WithoutDocuments,
        QuestionRoute::GlobalRetrieval => Plan::WholeFolder,
    };

    measured.plan = match &plan {
        Plan::OneFile(_) => Some(RetrievalPlan::OneFile),
        Plan::EveryDocument(_) => Some(RetrievalPlan::EveryDocument),
        Plan::WholeFolder => Some(RetrievalPlan::WholeFolder),
        Plan::WithoutDocuments => None,
    };

    // No document selected: answered from the conversation alone, under an instruction that says
    // so. No embedding call and no search - nothing she did not choose can reach this answer.
    if matches!(plan, Plan::WithoutDocuments) {
        measured.path = RetrievalPath::WithoutDocuments;
        let generated = writer
            .write(
                conversation::NO_DOCUMENTS_INSTRUCTION.to_string(),
                &history,
                question,
                // An open conversation rests on no material: the gateway's own default.
                None,
                on_event,
            )
            .await?;
        measured.generation_first_token_ms = generated.first_token_ms;
        measured.generation_total_ms = generated.total_ms;
        return Ok(AskAnswer {
            answer: generated.text,
            without_documents: true,
            ..AskAnswer::default()
        });
    }

    // Everything she chose is gone or has changed: nothing was searched and nothing could be, so
    // say that rather than the generic refusal, and spend no gateway call finding out.
    if narrowed && scoped_paths.is_empty() && !scope_outdated.is_empty() {
        return Err(AppError::ScopeUnavailable);
    }

    // A question about one named file gets that file's record; anything else gets counts only.
    // A per-document question deliberately gets counts rather than the whole listing: the
    // excerpts already carry one header per file, so a listing would repeat every path in a
    // second format and push the first excerpts into the middle of a long prompt, which is where
    // small models were observed losing them (`docs/WORK-FOLDER-INVENTORY.md`).
    let view = match &plan {
        Plan::OneFile(relative_path) => ContextView::Targeted {
            relative_path: relative_path.clone(),
        },
        _ => ContextView::Summary,
    };

    // Nothing has been read yet: retrieval is certain to find nothing, so say so here rather
    // than spend a round trip discovering it.
    if index.chunk_count()? == 0 {
        return Err(AppError::InsufficientEvidence);
    }

    // The knowledge base reads the question before it is embedded: which files of the selection
    // mention what it names, and whether that is worth a smaller search. A named file is never
    // reduced (invariant I2) and the full selection is what runs whenever anything is in doubt
    // (invariant I4).
    let plan_files: Vec<String> = match &plan {
        Plan::OneFile(relative_path) => vec![relative_path.clone()],
        Plan::EveryDocument(relative_paths) => relative_paths.clone(),
        Plan::WholeFolder | Plan::WithoutDocuments => scoped_paths.clone(),
    };
    let selected_files = selected_documents(&inventory, &plan_files);
    let documents_side = match &plan {
        Plan::OneFile(_) => SideInput::new(&selected_files).blocked_by(SkipReason::NamedFile),
        _ => SideInput::new(&selected_files),
    };
    let setup = TargetingSetup::load(app, state);
    let targeting_plan = setup.plan(
        &index,
        &targeting_request,
        &question,
        documents_side,
        SideInput::new(&[]),
    );

    // A short follow-up is searched together with the previous question; the router above and the
    // model below read the question as she wrote it.
    let search_text = conversation::retrieval_query(&question, &history);
    stage.lap_ms();
    let query_vectors = state
        .gateway
        .embed(
            &server_url,
            &embedding_alias,
            std::slice::from_ref(&search_text),
        )
        .await?;
    let query_embedding = query_vectors.into_iter().next().unwrap_or_default();
    measured.embedding_ms = stage.lap_ms();

    // Every file present in the folder right now. "Tous" is held to these, so a previous folder's
    // passages - which the index keeps until the next Analyse - cannot be cited meanwhile.
    let present_paths: Vec<String> = inventory
        .all_files()
        .iter()
        .map(|file| file.relative_path.clone())
        .collect();
    let searched = targeting::search_documents_within(
        &index,
        &targeting_plan,
        match &plan {
            Plan::OneFile(relative_path) => DocumentPlan::OneFile(relative_path.as_str()),
            Plan::EveryDocument(relative_paths) => DocumentPlan::EveryDocument(relative_paths),
            Plan::WholeFolder => DocumentPlan::WholeFolder {
                narrowed,
                scoped_paths: &scoped_paths,
                present_paths: &present_paths,
            },
            // Returned above: nothing is searched when no document is selected.
            Plan::WithoutDocuments => DocumentPlan::EveryDocument(&[]),
        },
        &search_text,
        &query_embedding,
        // How much of the folder this model can read: the excerpts follow its window.
        retrieval::RetrievalLimits::for_budget(&writer.budget),
    )?;
    measured.search_ms = stage.lap_ms();
    targeting::note_documents(&index, &targeting_plan, &searched, measured);
    let targeting_report = targeting_plan.report(searched.widened);
    let found = searched.outcome;
    measured.chunks_considered = found.chunks_considered;
    let evidence = found.evidence;
    measured.chunks_selected = evidence.len();
    measured.estimated_input_chars = evidence.iter().map(|item| item.text.chars().count()).sum();
    if evidence.is_empty() {
        // A file she chose that could not be used may be exactly where the answer was, so the
        // refusal names that rather than blaming the documents that were searched.
        return Err(match narrowed && !scope_outdated.is_empty() {
            true => AppError::ScopeUnavailable,
            false => AppError::InsufficientEvidence,
        });
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
    // was being written from, which an answer left on screen has to do. What the knowledge base did
    // to the search goes first, for the same reason.
    if let Some(report) = &targeting_report {
        let _ = on_event.send(ChatStreamEvent::Targeting {
            report: Box::new(report.clone()),
        });
    }
    let _ = on_event.send(ChatStreamEvent::Sources {
        sources: evidence.clone(),
        coverage,
    });

    // Two kinds of evidence in one instruction, kept apart on purpose: the excerpts say what the
    // documents state, the Work Folder context says which files exist and what was done to them,
    // and the contract between them forbids using either as a substitute for the other. `write`
    // puts this immediately before the question, after any past exchanges; past excerpts
    // themselves are never resent - each question retrieves its own.
    let folder_context = work_folder_context::build(&inventory, &view);
    let grounding = format!(
        "{}\n{}",
        work_folder_context::build_system_turn(retrieval::RETRIEVAL_INSTRUCTION, &folder_context),
        retrieval::format_evidence(&evidence)
    );
    // Sources outrank memory (BUG-09): beside excerpts, a small window remembers fewer and shorter
    // earlier exchanges (`conversation::history_beside_sources`).
    let history = conversation::history_beside_sources(&history, writer.budget);
    let asked = question.clone();
    let generated = writer
        .write(
            grounding,
            &history,
            question,
            Some(GROUNDED_TEMPERATURE),
            on_event,
        )
        .await?;
    measured.generation_first_token_ms = generated.first_token_ms;
    measured.generation_total_ms = generated.total_ms;
    let answer = generated.text;
    // A figure the model states that is in no excerpt and not in the question: said beside the
    // answer. Two digits are enough here (a price such as 80 is the case that mattered).
    // The cited files' own paths are known too: the model names a source by its path, and the date
    // in `2026-03-26_decompte.pdf` is not a figure it invented.
    let mut known: Vec<&str> = evidence.iter().map(|item| item.text.as_str()).collect();
    known.extend(evidence.iter().map(|item| item.relative_path.as_str()));
    known.push(&asked);
    // The pages the excerpts come from are known figures too: a correct citation is no false alarm.
    let pages: Vec<u32> = evidence.iter().map(|item| item.page_number).collect();
    let unverified_numbers =
        number_check::unsupported_numbers_with_pages(&answer, &known, &pages, 2);

    Ok(AskAnswer {
        answer,
        sources: evidence,
        coverage,
        unanalysed_files: inventory.unanalysed_documents(),
        scope_outdated,
        unverified_numbers,
        targeting: targeting_report,
        ..AskAnswer::default()
    })
}

/// Tier 2: tables selected, no document. Answered by the tabular engine alone - structural facts,
/// computed values, or a nudge naming the real columns - with no gateway call and no embedding for
/// a question the classifier reads on its own. Two gateway calls are possible, both narrow and
/// internal to `tabular_answer`, session 14's hidden interpreter, never load-bearing for a
/// number's correctness: automatically, only when a question does not classify deterministically
/// at all (gap G); and now, since the manual validation pass that found a classified answer can
/// still be a *wrong* interpretation (`docs/DECISIONS.md`), on "Demander a l'IA"/"Ask AI"
/// (`skip_deterministic`) - the tier's own form of the regenerate control every other tier already
/// offers, asking the model in addition to a classified answer rather than instead of one. The
/// conversation history is not needed and not read.
#[allow(clippy::too_many_arguments)]
async fn tabular_tier(
    app: &AppHandle,
    state: &AppState,
    question: &str,
    scope: &AnalysisScope,
    skip_deterministic: bool,
    history: &[ChatTurn],
    targeting_request: &TargetingRequest,
    // The state the last table answer left: a short follow-up continues it (TABULAR step 11).
    carry: Option<&TableCarryOver>,
    on_event: &Channel<ChatStreamEvent>,
    measured: &mut RetrievalDiagnostics,
) -> Result<AskAnswer, AppError> {
    let (data_folder, locale, server_url, model_alias, idle_timeout) = state.read(|settings| {
        (
            settings.data_folder.clone(),
            settings.locale.clone(),
            settings.server_url.clone(),
            settings.model_alias.clone(),
            settings.answer_idle_timeout(),
        )
    })?;
    if data_folder.is_none() {
        return Err(AppError::NoDataFolderSet);
    }
    let mut stage = StageTimer::start();
    let index = open_index(app)?;
    let folder = family_views_of(state, &index)?
        .data
        .ok_or(AppError::NoDataFolderSet)?;
    measured.inventory_ms += stage.lap_ms();
    measured.selected_workbooks = selected_workbooks(&folder, &scope.data_mode);
    let locale = locale.as_deref().unwrap_or(settings::DEFAULT_LOCALE);
    // Two steps, not one `tabular_answer::answer(...)` call: `prepare` is the only place `index`
    // is read, entirely synchronously, so the `&index` borrow never has to survive the `.await`
    // on `resolve` - `IndexStore` is not `Sync`, and this command's future must be `Send`
    // (`tabular_answer`'s own module doc explains why).
    // What the question names, among the selected workbooks. A question that names a workbook is
    // never narrowed (invariant I2); in `suggest` the plan only orders and annotates the choice.
    let workbook_files = selected_workbook_files(&folder, &scope.data_mode);
    let data_side = match tabular_answer::names_a_workbook(question, &folder, &scope.data_mode) {
        true => SideInput::new(&workbook_files).blocked_by(SkipReason::NamedFile),
        false => SideInput::new(&workbook_files),
    };
    let setup = TargetingSetup::load(app, state);
    let targeting_plan = setup.plan(
        &index,
        targeting_request,
        question,
        SideInput::new(&[]),
        data_side,
    );
    let effective_selection = match targeting_plan.applied_workbooks() {
        Some(candidates) => {
            targeting::narrowed_data_selection(&folder, &scope.data_mode, candidates)
        }
        None => scope.data_mode.clone(),
    };
    // An explanation, a description, a comparison or a formula request is written by the model from
    // the workbook cards, as in the mixed tier; the hidden interpreter, which can only propose a query
    // plan, is not asked (TABULAR step 05). The decision is cheap and made before `prepare` loads
    // anything. "Demander a l'IA" on such a question asks the writer again.
    if tabular_answer::advisory_target(
        question,
        &folder,
        &scope.data_mode,
        locale,
        &implied_by_entity_candidates(&targeting_plan),
    )
    .is_some()
    {
        measured.routing_ms += stage.lap_ms();
        let locale = locale.to_string();
        return advisory_tier(
            state,
            question,
            scope,
            history,
            index,
            folder,
            locale,
            targeting_plan,
            tabular_answer::PendingAnswer::Done(tabular_answer::no_table_part()),
            on_event,
            measured,
        )
        .await;
    }
    let (pending, carry_report) = tabular_answer::prepare_with_carry_over(
        question,
        &folder,
        selection_for_follow_up(
            carry,
            question,
            locale,
            &scope.data_mode,
            &effective_selection,
        ),
        locale,
        &index,
        skip_deterministic,
        carry,
    )?;
    let pending = decorate_pending(pending, &targeting_plan);
    measured.routing_ms += stage.lap_ms();
    measured.absorb_workbook_loads(diagnostics::take_workbook_loads());
    let assist = tabular_answer::ModelAssist {
        gateway: &state.gateway,
        server_url: &server_url,
        model_alias: &model_alias,
        idle_timeout,
    };
    let answer = tabular_answer::resolve(pending, question, locale, &assist).await;
    measured.generation_total_ms += stage.lap_ms();
    let answered = targeting::answered_file(&answer);
    targeting::note_workbooks(&index, &targeting_plan, answered.as_deref(), measured);
    let workbook_use = workbook_use::trace(
        &tabular_answer::selection_facts(question, &folder, &scope.data_mode),
        workbook_use::Answered::Engine(&answer),
    );
    Ok(AskAnswer {
        tabular_answer: Some(answer),
        targeting: targeting_plan.report(false),
        workbook_use,
        ..AskAnswer::default()
    }
    .with_carry_over(carry_report))
}

/// The selection a follow-up is read against: what the user ticked. The knowledge base may have narrowed
/// the list of workbooks to those a name points at, and the workbook of the previous answer must stay
/// reachable for a question that opens like a follow-up ("and for Beta?"). Any other question keeps the
/// narrowed list.
fn selection_for_follow_up<'s>(
    carry: Option<&TableCarryOver>,
    question: &str,
    locale: &str,
    ticked: &'s ScopeMode,
    narrowed: &'s ScopeMode,
) -> &'s ScopeMode {
    if carry.is_some() && crate::tabular::question::opens_like_a_follow_up(question, locale) {
        ticked
    } else {
        narrowed
    }
}

/// The writer of the tables-only tier (TABULAR step 05): an advisory question (`tabular_answer::
/// advisory_target`) is answered by the model from the workbook cards, with the controlled sample of
/// rows and the engine's column profile when the setting allows, and no document block. It is the
/// mixed tier's writer (`mixed_answer::answer`) with an empty document list, so the interface, the
/// number checks and the usage trace are the ones of the mixed tier; the interface says that no
/// document was selected (`MixedPartUnavailable::NotSelected`).
///
/// `pending` is the table side of the turn: `PendingAnswer::Done(no_table_part())` for an advisory
/// question, and a calculation record for the analysis of step 06, which calls this same function.
///
/// Degrades to today's behaviour: when the model cannot be reached or writes nothing, the answer is
/// the nudge the engine would have given, taken from `prepare` **without** `resolve` (a second call to
/// an unreachable server would only add a wait). `index` is owned, never borrowed across an `.await`
/// (`IndexStore` is not `Sync`, the reason `tabular_answer`'s module doc gives).
#[allow(clippy::too_many_arguments)]
async fn advisory_tier(
    state: &AppState,
    question: &str,
    scope: &AnalysisScope,
    history: &[ChatTurn],
    index: IndexStore,
    folder: DataFolder,
    locale: String,
    targeting_plan: TargetingPlan,
    pending: tabular_answer::PendingAnswer,
    on_event: &Channel<ChatStreamEvent>,
    measured: &mut RetrievalDiagnostics,
) -> Result<AskAnswer, AppError> {
    let (server_url, model_alias, idle_timeout, rows_mode, application) =
        state.read(|settings| {
            (
                settings.server_url.clone(),
                settings.model_alias.clone(),
                settings.answer_idle_timeout(),
                settings.tabular_rows_mode,
                settings.spreadsheet_application,
            )
        })?;
    let budget = state
        .model_budgets
        .lock()
        .map_err(|_| AppError::Internal)?
        .for_alias(&model_alias);
    // A figure on the table side is the analysis of that figure (TABULAR step 06): it is kept whatever
    // the model does, and `ANALYSIS_ADDENDUM` joins the instruction.
    let kept_figure = match &pending {
        tabular_answer::PendingAnswer::Done(figure @ TabularAnswer::Value { .. }) => {
            Some(figure.clone())
        }
        _ => None,
    };
    let sheet_sizes = tabular_answer::sheet_sizes(&folder, &scope.data_mode);
    // The cards and the sample, from what the user ticked and not from the knowledge base's narrowed
    // list: one ticked workbook is implied here, which is right (the engine has no door of its own for
    // an advisory question). Read synchronously, before the first `.await`.
    let cards = tabular_answer::question_cards(
        question,
        &folder,
        &scope.data_mode,
        &locale,
        &implied_by_entity_candidates(&targeting_plan),
        card_budget_chars(&budget),
    );
    let card_block = &cards.block;
    let implied = &cards.implied;
    // `auto` adds the rows the engine chooses with the filters of the question (TABULAR step 10); never
    // for the analysis of a figure, which is final.
    let ask_rows = rows_mode == settings::TabularRowsMode::Auto && kept_figure.is_none();
    let table_data = match rows_mode {
        settings::TabularRowsMode::Off => tabular_answer::TableData::default(),
        settings::TabularRowsMode::Sample | settings::TabularRowsMode::Auto => {
            tabular_answer::question_table_data(
                card_block,
                &folder,
                &scope.data_mode,
                &locale,
                &index,
                &budget,
                &crate::tabular::sample::fresh_token(),
                ask_rows.then_some(tabular_answer::RowsAsk {
                    question,
                    excerpts: &[],
                }),
            )
        }
    };
    let sample_block = &table_data.sample;
    let rows_block = &table_data.rows;
    let trace_uses = crate::tabular::rows::with_chosen_rows(&sample_block.uses, rows_block);
    measured.absorb_workbook_loads(diagnostics::take_workbook_loads());
    let formula_context = crate::tabular::formula_check::FormulaContext::new(
        application.as_str(),
        &locale,
        &card_block.cards,
    );
    let context = mixed_answer::MixedContext {
        locale: &locale,
        gateway: &state.gateway,
        server_url: &server_url,
        model_alias: &model_alias,
        idle_timeout,
        history,
        budget,
        cards_text: &card_block.text,
        sample_text: &sample_block.text,
        calculation: mixed_answer::CalculationSide {
            sheet_sizes: &sheet_sizes,
            analysis: kept_figure.is_some(),
        },
        formulas: formula_context.as_ref(),
        folder_facts: "",
        rows: (!rows_block.is_empty()).then_some(rows_block),
    };
    let targeting_report = targeting_plan.report(false);
    if let Some(report) = &targeting_report {
        let _ = on_event.send(ChatStreamEvent::Targeting {
            report: Box::new(report.clone()),
        });
    }
    let generation_started = std::time::Instant::now();
    let first_token = std::sync::OnceLock::<u64>::new();
    let written = mixed_answer::answer(
        question,
        pending,
        Vec::new(),
        Some(mixed_answer::MixedPartUnavailable::NotSelected),
        &context,
        |sources| {
            let _ = on_event.send(ChatStreamEvent::Sources {
                sources: sources.to_vec(),
                coverage: None,
            });
        },
        |delta| {
            first_token.get_or_init(|| elapsed_ms(generation_started.elapsed()));
            let _ = on_event.send(ChatStreamEvent::Delta {
                text: delta.to_string(),
            });
        },
    )
    .await;
    measured.generation_total_ms = elapsed_ms(generation_started.elapsed());
    measured.generation_first_token_ms = first_token.get().copied();
    let facts = tabular_answer::selection_facts(question, &folder, &scope.data_mode);

    match written {
        // The writer answered, or a computed figure stands without it (step 06's record).
        Ok((result, generated))
            if !(generated && result.answer.is_empty() && result.table.is_none()) =>
        {
            if generated {
                let _ = on_event.send(ChatStreamEvent::Completed {
                    text: result.answer.clone(),
                });
            }
            targeting::note_workbooks(&index, &targeting_plan, None, measured);
            let workbook_use = workbook_use::trace(
                &facts,
                workbook_use::Answered::Mixed {
                    cards: &card_block.cards,
                    implied,
                    table: &result.table,
                    model_wrote: generated && !result.answer.is_empty(),
                    samples: &trace_uses,
                },
            );
            Ok(AskAnswer {
                mixed_answer: Some(result),
                targeting: targeting_report,
                workbook_use,
                ..AskAnswer::default()
            })
        }
        // Stopped by the user: not an answer to replace.
        Err(AppError::ChatCancelled) => Err(AppError::ChatCancelled),
        // The analysis of a figure the model could not write: the figure stays, as when the gateway fails
        // inside the writer (a figure is never replaced by the nudge it was computed instead of).
        Ok(_) | Err(_) if kept_figure.is_some() => {
            targeting::note_workbooks(&index, &targeting_plan, None, measured);
            let table = kept_figure;
            let workbook_use = workbook_use::trace(
                &facts,
                workbook_use::Answered::Mixed {
                    cards: &card_block.cards,
                    implied,
                    table: &table,
                    model_wrote: false,
                    samples: &trace_uses,
                },
            );
            Ok(AskAnswer {
                mixed_answer: Some(mixed_answer::MixedAnswer {
                    answer: String::new(),
                    table,
                    table_unavailable: None,
                    document_sources: Vec::new(),
                    documents_unavailable: Some(
                        mixed_answer::MixedPartUnavailable::GatewayUnavailable,
                    ),
                    corrections: Vec::new(),
                    rejected_citations: Vec::new(),
                    unverified_numbers: Vec::new(),
                    model_arithmetic: Vec::new(),
                    ..mixed_answer::MixedAnswer::default()
                }),
                targeting: targeting_report,
                workbook_use,
                ..AskAnswer::default()
            })
        }
        // The model could not be reached, or wrote nothing: the nudge the engine would have given.
        Ok(_) | Err(_) => {
            let effective_selection = match targeting_plan.applied_workbooks() {
                Some(candidates) => {
                    targeting::narrowed_data_selection(&folder, &scope.data_mode, candidates)
                }
                None => scope.data_mode.clone(),
            };
            let nudge = tabular_answer::without_the_model(tabular_answer::prepare(
                question,
                &folder,
                &effective_selection,
                &locale,
                &index,
                false,
            )?);
            measured.absorb_workbook_loads(diagnostics::take_workbook_loads());
            let answered = targeting::answered_file(&nudge);
            targeting::note_workbooks(&index, &targeting_plan, answered.as_deref(), measured);
            let workbook_use = workbook_use::trace(&facts, workbook_use::Answered::Engine(&nudge));
            Ok(AskAnswer {
                tabular_answer: Some(nudge),
                targeting: targeting_plan.report(false),
                workbook_use,
                ..AskAnswer::default()
            })
        }
    }
}

/// the "Explain the calculation" button and "Demander a l'IA" under a computed figure (TABULAR step 06, owner decision
/// 11). The figure is computed again by the program from the original question and the scope it was
/// given under, without any model call, and compared with the one on screen
/// (`calculation_follow_up::recompute`): a file that changed or a figure that no longer comes out is
/// refused with the existing `scope_unavailable`, never explained.
///
/// **Explain**: one gateway call, the calculation record only - no retrieval, no embedding, no card, no
/// sample, no history. The figure of the entry stays; the explanation is added under it. A model that
/// cannot be reached leaves an `unavailable` explanation and the figure.
///
/// **Analyse**: the writer of the mixed tier with the record in its `Table results` block and
/// `ANALYSIS_ADDENDUM` in its instruction: `advisory_tier` with no document when only tables are
/// ticked, `mixed_tier` when documents are ticked as well.
#[allow(clippy::too_many_arguments)]
async fn calculation_follow_up_tier(
    app: &AppHandle,
    state: &AppState,
    question: &str,
    scope: &AnalysisScope,
    history: &[ChatTurn],
    targeting_request: &TargetingRequest,
    follow_up: &CalculationFollowUp,
    on_event: &Channel<ChatStreamEvent>,
    measured: &mut RetrievalDiagnostics,
) -> Result<AskAnswer, AppError> {
    let tier = scope.tier();
    if matches!(tier, GroundingTier::Documents) {
        return Err(AppError::ScopeUnavailable);
    }
    let (data_folder, locale, server_url, model_alias, idle_timeout) = state.read(|settings| {
        (
            settings.data_folder.clone(),
            settings.locale.clone(),
            settings.server_url.clone(),
            settings.model_alias.clone(),
            settings.answer_idle_timeout(),
        )
    })?;
    if data_folder.is_none() {
        return Err(AppError::NoDataFolderSet);
    }
    let locale = locale.unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string());
    let mut stage = StageTimer::start();
    let index = open_index(app)?;
    let views = family_views_of(state, &index)?;
    let folder = views.data.ok_or(AppError::NoDataFolderSet)?;
    // With documents ticked, the figure may have been read beside their names (the mixed tier).
    let document_names: Option<Vec<String>> = match tier {
        GroundingTier::DocumentsAndTables => {
            let full = views.documents.ok_or(AppError::NoWorkFolderSet)?;
            Some(
                scope
                    .resolve(&full)
                    .inventory
                    .indexed_files()
                    .into_iter()
                    .map(|file| file.relative_path.clone())
                    .collect(),
            )
        }
        _ => None,
    };
    measured.inventory_ms += stage.lap_ms();
    measured.selected_workbooks = selected_workbooks(&folder, &scope.data_mode);
    let (figure, figure_carry) = calculation_follow_up::recompute_with_carry_over(
        question,
        &folder,
        &scope.data_mode,
        &locale,
        &index,
        &follow_up.shown,
        document_names.as_deref(),
        follow_up.table_carry_over.as_ref(),
    )?;
    measured.routing_ms += stage.lap_ms();
    measured.absorb_workbook_loads(diagnostics::take_workbook_loads());

    match follow_up.kind {
        FollowUpKind::Explain => {
            let sizes = tabular_answer::sheet_sizes(&folder, &scope.data_mode);
            let Some(record) = tabular_answer::calculation_record_inheriting(
                Some(question),
                &figure,
                &sizes,
                None,
                figure_carry.inherited.as_ref(),
            ) else {
                return Err(AppError::ScopeUnavailable);
            };
            let file = follow_up.shown.file.clone();
            drop(folder);
            drop(index);
            let context = calculation_follow_up::ExplainContext {
                locale: &locale,
                gateway: &state.gateway,
                server_url: &server_url,
                model_alias: &model_alias,
                idle_timeout,
            };
            let started = std::time::Instant::now();
            let first_token = std::sync::OnceLock::<u64>::new();
            let explanation =
                calculation_follow_up::explain(question, &record, &context, |delta| {
                    first_token.get_or_init(|| elapsed_ms(started.elapsed()));
                    let _ = on_event.send(ChatStreamEvent::Delta {
                        text: delta.to_string(),
                    });
                })
                .await?;
            measured.generation_total_ms = elapsed_ms(started.elapsed());
            measured.generation_first_token_ms = first_token.get().copied();
            if !explanation.unavailable {
                let _ = on_event.send(ChatStreamEvent::Completed {
                    text: explanation.text.clone(),
                });
            }
            // The line says the model explained the calculation: not written when it could not be.
            let workbook_use = match explanation.unavailable {
                true => Vec::new(),
                false => workbook_use::trace(
                    &tabular_answer::SelectionFacts::default(),
                    workbook_use::Answered::Explained(&file),
                ),
            };
            Ok(AskAnswer {
                explanation: Some(explanation),
                workbook_use,
                ..AskAnswer::default()
            })
        }
        FollowUpKind::Analyse => match tier {
            GroundingTier::DocumentsAndTables => {
                drop(folder);
                drop(index);
                mixed_tier(
                    app,
                    state,
                    question,
                    scope,
                    history,
                    targeting_request,
                    on_event,
                    measured,
                    Some(figure),
                    None,
                )
                .await
            }
            _ => {
                let workbook_files = selected_workbook_files(&folder, &scope.data_mode);
                let data_side =
                    match tabular_answer::names_a_workbook(question, &folder, &scope.data_mode) {
                        true => SideInput::new(&workbook_files).blocked_by(SkipReason::NamedFile),
                        false => SideInput::new(&workbook_files),
                    };
                let setup = TargetingSetup::load(app, state);
                let targeting_plan = setup.plan(
                    &index,
                    targeting_request,
                    question,
                    SideInput::new(&[]),
                    data_side,
                );
                advisory_tier(
                    state,
                    question,
                    scope,
                    history,
                    index,
                    folder,
                    locale,
                    targeting_plan,
                    tabular_answer::PendingAnswer::Done(figure),
                    on_event,
                    measured,
                )
                .await
            }
        },
    }
}

/// Whether the engine alone answers this question, in a mixed selection, with a nudge (a refusal that
/// lists what the workbook holds) and no figure: the case where "Demander a l'IA" asks the model to
/// understand the question. Cheap (inventory and the typed cache); `false` on any failure, which keeps the
/// mixed tier's own handling.
fn data_side_is_a_nudge(
    app: &AppHandle,
    state: &AppState,
    question: &str,
    scope: &AnalysisScope,
    carry: Option<&TableCarryOver>,
) -> bool {
    let Ok(locale) = state.read(|settings| settings.locale.clone()) else {
        return false;
    };
    let locale = locale.unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string());
    let Ok(index) = open_index(app) else {
        return false;
    };
    let Ok(views) = family_views_of(state, &index) else {
        return false;
    };
    let Some(folder) = views.data else {
        return false;
    };
    matches!(
        tabular_answer::prepare_if_data_only_with_carry_over(
            question,
            &folder,
            &scope.data_mode,
            &locale,
            &index,
            carry,
        ),
        Ok((
            Some(tabular_answer::PendingAnswer::Done(
                TabularAnswer::Nudge { .. }
            )),
            _
        ))
    )
}

/// A "which workbook?" answer gets the knowledge base's order and notes; any other answer is left
/// exactly as the engine produced it.
fn decorate_pending(
    pending: tabular_answer::PendingAnswer,
    plan: &TargetingPlan,
) -> tabular_answer::PendingAnswer {
    match pending {
        tabular_answer::PendingAnswer::Done(answer) => {
            tabular_answer::PendingAnswer::Done(targeting::decorate_which_workbook(answer, plan))
        }
        other => other,
    }
}

/// The first, low-risk step of D7 (`docs/DECISIONS.md`, `docs/SESSION-DATA-15-Mixed-Routing.md`):
/// documents and tables are both selected. Reads the same settings and opens the same Data Folder
/// as `tabular_tier`, then the same `prepare`/`resolve` split, for the same `IndexStore`-across-
/// `.await` reason (`tabular_answer`'s own module doc): `prepare_if_data_only` is the only place
/// `index` is read, entirely synchronously, and already carries the precondition that keeps gap G
/// from ever reaching `resolve` on this path (`None` here means the caller keeps its own
/// mixed-selection refusal).
async fn mixed_data_only_tier(
    app: &AppHandle,
    state: &AppState,
    question: &str,
    scope: &AnalysisScope,
    targeting_request: &TargetingRequest,
    carry: Option<&TableCarryOver>,
    measured: &mut RetrievalDiagnostics,
) -> Result<Option<AskAnswer>, AppError> {
    let (data_folder, locale, server_url, model_alias, idle_timeout) = state.read(|settings| {
        (
            settings.data_folder.clone(),
            settings.locale.clone(),
            settings.server_url.clone(),
            settings.model_alias.clone(),
            settings.answer_idle_timeout(),
        )
    })?;
    if data_folder.is_none() {
        return Err(AppError::NoDataFolderSet);
    }
    let mut stage = StageTimer::start();
    let index = open_index(app)?;
    let folder = family_views_of(state, &index)?
        .data
        .ok_or(AppError::NoDataFolderSet)?;
    measured.inventory_ms += stage.lap_ms();
    measured.selected_workbooks = selected_workbooks(&folder, &scope.data_mode);
    let locale = locale.as_deref().unwrap_or(settings::DEFAULT_LOCALE);
    // The documents are not read on this path, so only the workbooks can be narrowed.
    let workbook_files = selected_workbook_files(&folder, &scope.data_mode);
    let data_side = match tabular_answer::names_a_workbook(question, &folder, &scope.data_mode) {
        true => SideInput::new(&workbook_files).blocked_by(SkipReason::NamedFile),
        false => SideInput::new(&workbook_files),
    };
    let setup = TargetingSetup::load(app, state);
    let targeting_plan = setup.plan(
        &index,
        targeting_request,
        question,
        SideInput::new(&[]),
        data_side,
    );
    let effective_selection = match targeting_plan.applied_workbooks() {
        Some(candidates) => {
            targeting::narrowed_data_selection(&folder, &scope.data_mode, candidates)
        }
        None => scope.data_mode.clone(),
    };
    let (prepared, carry_report) = tabular_answer::prepare_if_data_only_with_carry_over(
        question,
        &folder,
        selection_for_follow_up(
            carry,
            question,
            locale,
            &scope.data_mode,
            &effective_selection,
        ),
        locale,
        &index,
        carry,
    )?;
    // Counted even when the question turns out not to be data-only: the workbooks were read
    // either way, and the mixed tier that follows reads them from the cache this just filled.
    measured.routing_ms += stage.lap_ms();
    measured.absorb_workbook_loads(diagnostics::take_workbook_loads());
    let Some(pending) = prepared else {
        return Ok(None);
    };
    let pending = decorate_pending(pending, &targeting_plan);
    let assist = tabular_answer::ModelAssist {
        gateway: &state.gateway,
        server_url: &server_url,
        model_alias: &model_alias,
        idle_timeout,
    };
    let answer = tabular_answer::resolve(pending, question, locale, &assist).await;
    measured.generation_total_ms += stage.lap_ms();
    let answered = targeting::answered_file(&answer);
    targeting::note_workbooks(&index, &targeting_plan, answered.as_deref(), measured);
    let workbook_use = workbook_use::trace(
        &tabular_answer::selection_facts(question, &folder, &scope.data_mode),
        workbook_use::Answered::Engine(&answer),
    );
    Ok(Some(
        AskAnswer {
            tabular_answer: Some(answer),
            documents_not_needed: true,
            targeting: targeting_plan.report(false),
            workbook_use,
            ..AskAnswer::default()
        }
        .with_carry_over(carry_report),
    ))
}

/// What a set sentence needs besides the finder: the identifiers the packs read in what the user typed (in
/// every spelling), and the vocabulary that tells whether X is the name of exactly one entity. A pack or a
/// vocabulary that cannot be loaded leaves the sentence the word search it always was.
fn set_search_tools(
    state: &AppState,
    text: &str,
) -> Result<(Vec<String>, Option<Arc<QueryVocabulary>>), AppError> {
    let (locale_tag, pack_ids) = state.read(|settings| {
        (
            settings
                .locale
                .clone()
                .unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string()),
            settings.knowledge_pack_ids(),
        )
    })?;
    let ids: Vec<&str> = pack_ids.iter().map(String::as_str).collect();
    let identifiers = match (
        PackSet::load(&locale_tag, &ids),
        file_profile::vocabulary::ProfileVocabulary::load(&ids),
    ) {
        (Ok(packs), Ok(vocabulary)) => {
            finder::set_search::identifier_forms(text, &packs, &vocabulary)
        }
        _ => Vec::new(),
    };
    Ok((identifiers, state.vocabulary(&locale_tag, &pack_ids).ok()))
}

/// Documents and tables are both selected and the question is about the documents themselves ("how
/// many documents do I have?", "list my files"): answered from the inventory of the selected
/// documents, exactly as the documents tier answers it, with no model and no table. `None` for every
/// other question, which costs one cheap check of its words.
fn mixed_folder_tier(
    app: &AppHandle,
    state: &AppState,
    question: &str,
    scope: &AnalysisScope,
    on_event: &Channel<ChatStreamEvent>,
    measured: &mut RetrievalDiagnostics,
) -> Result<Option<AskAnswer>, AppError> {
    let (work_folder, locale) =
        state.read(|settings| (settings.work_folder.clone(), settings.locale.clone()))?;
    if work_folder.is_none() {
        return Ok(None);
    }
    let locale = locale.unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string());
    if !folder_questions::names_the_documents(question, &locale) {
        return Ok(None);
    }
    let mut stage = StageTimer::start();
    let index = open_index(app)?;
    let full_inventory = family_views_of(state, &index)?
        .documents
        .ok_or(AppError::NoWorkFolderSet)?;
    measured.inventory_ms += stage.lap_ms();
    let resolution = scope.resolve(&full_inventory);
    let selection = folder_questions::Selection {
        full: &full_inventory,
        scoped: &resolution.inventory,
        narrowed: resolution.narrowed,
        no_documents_chosen: resolution.no_documents_chosen,
    };
    // "Which documents talk about X" covers the tables too: one list per folder, each stating its folder,
    // read from the index with no model (`docs/DECISIONS.md`, 14 October 2026).
    let set_sentence =
        match folder_questions::route_in_selection(&selection, &index, question, &locale) {
            QuestionRoute::SetSearch(set) => Some(set),
            _ => None,
        };
    let answer = if let Some(set) = set_sentence {
        measured.path = RetrievalPath::DocumentSearch;
        let Some(data) = family_views_of(state, &index)?.data else {
            return Ok(None);
        };
        let workbooks: Vec<InventoryFile> =
            tabular_answer::selected_workbook_paths(&data, &scope.data_mode)
                .iter()
                .filter_map(|path| data.files().find_by_relative_path(path))
                .map(InventoryFile::from)
                .collect();
        let documents: Vec<InventoryFile> = resolution
            .inventory
            .all_files()
            .iter()
            .map(InventoryFile::from)
            .collect();
        let (identifiers, names_vocabulary) = set_search_tools(state, &set.text)?;
        let names = names_vocabulary
            .as_ref()
            .map(|vocabulary| finder::names::KnowledgeNames {
                connection: index.connection(),
                vocabulary,
            });
        let no_names = finder::names::NoNames;
        let resolver: &dyn finder::names::NameResolver = match &names {
            Some(names) => names,
            None => &no_names,
        };
        finder::set_search::answer_both(
            index.connection(),
            &documents,
            &workbooks,
            &set,
            identifiers,
            resolver,
        )?
    } else {
        let Some(answer) =
            folder_questions::answer_in_mixed_selection(&selection, &index, question, &locale)
        else {
            return Ok(None);
        };
        answer
    };
    measured.routing_ms += stage.lap_ms();
    if !matches!(answer, FolderAnswer::DocumentSearchBoth { .. }) {
        measured.path = RetrievalPath::FolderAnswer;
    }
    let scope_outdated: Vec<String> = resolution
        .missing
        .into_iter()
        .chain(resolution.changed)
        .collect();
    let _ = on_event.send(ChatStreamEvent::FolderAnswer {
        answer: answer.clone(),
    });
    // No workbook was read for a figure; a workbook the question names and the selection does not
    // hold is still said (`workbook_use::Answered::FolderSearch`).
    let workbook_use = match family_views_of(state, &index)?.data {
        Some(data) => workbook_use::trace(
            &tabular_answer::selection_facts(question, &data, &scope.data_mode),
            workbook_use::Answered::FolderSearch,
        ),
        None => Vec::new(),
    };
    Ok(Some(AskAnswer {
        folder_answer: Some(answer),
        scope_outdated,
        workbook_use,
        ..AskAnswer::default()
    }))
}

/// Documents and tables are both selected and the question asks for something to be written. When a
/// selected document is a template whose fields name columns of the one workbook the question points at
/// (an identifier it names, or "each"), the answer is a plan to approve: the template, the rows, the
/// mapping and a preview. Nothing is written and no model is called here; `FillStart::None` for every
/// other question. A named workbook with no template that fits is handed over as a job to draft one
/// (`fill_draft_tier`): it is the only part that needs the gateway, so it happens after the index is
/// let go of.
fn fill_tier(
    app: &AppHandle,
    state: &AppState,
    question: &str,
    scope: &AnalysisScope,
    measured: &mut RetrievalDiagnostics,
) -> Result<FillStart, AppError> {
    let (work_folder, data_folder, locale, output_folder) = state.read(|settings| {
        (
            settings.work_folder.clone(),
            settings.data_folder.clone(),
            settings.locale.clone(),
            settings.generated_folder(),
        )
    })?;
    if work_folder.is_none() || data_folder.is_none() {
        return Ok(FillStart::None);
    }
    let locale = locale.unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string());
    if !crate::tabular::question::has_writing_intent(question, &locale) {
        return Ok(FillStart::None);
    }
    let mut stage = StageTimer::start();
    let index = open_index(app)?;
    let data = family_views_of(state, &index)?
        .data
        .ok_or(AppError::NoDataFolderSet)?;
    measured.inventory_ms += stage.lap_ms();
    let every_row = crate::tabular::question::has_distributive_word(question, &locale);
    // What the question names decides which workbook a letter is filled from, before anything else
    // (TABULAR step 08): computed from what the user ticked, once, for the choice and for the trace.
    let facts = tabular_answer::selection_facts(question, &data, &scope.data_mode);
    let located = tabular_answer::locate_fill_source(
        question,
        &data,
        &scope.data_mode,
        &locale,
        &index,
        every_row,
        &facts.named,
    )?;
    measured.routing_ms += stage.lap_ms();
    measured.absorb_workbook_loads(diagnostics::take_workbook_loads());
    // The templates are looked for among the documents only once a source is found.
    let documents = match &located {
        tabular_answer::FillLocation::Found(_) => {
            let full_inventory = family_views_of(state, &index)?
                .documents
                .ok_or(AppError::NoWorkFolderSet)?;
            // What the program wrote (the letters, a saved model) is not offered as a template.
            fill_plan::without_output_folder(
                scope.resolve(&full_inventory).inventory,
                &output_folder,
            )
        }
        _ => crate::inventory::WorkFolderInventory::from_records(
            std::path::Path::new(""),
            Vec::new(),
        ),
    };
    match fill_plan::decide(located, &facts.named, every_row, &documents)? {
        fill_plan::FillDecision::Plan(plan) => {
            let workbook_use =
                workbook_use::trace(&facts, workbook_use::Answered::Plan(&plan.data_file));
            Ok(FillStart::Answer(Box::new(AskAnswer {
                fill_plan: Some(plan),
                workbook_use,
                ..AskAnswer::default()
            })))
        }
        // The letters the program could not prepare are asked about, never written by a model.
        fill_plan::FillDecision::Ask(answer) => {
            let workbook_use = workbook_use::trace(&facts, workbook_use::Answered::Engine(&answer));
            Ok(FillStart::Answer(Box::new(AskAnswer {
                tabular_answer: Some(answer),
                workbook_use,
                ..AskAnswer::default()
            })))
        }
        fill_plan::FillDecision::NeedsDraft(source) => {
            // What the model is told about each column besides its name: its type, from the analysis.
            let kinds = fill_plan::column_kinds(&data, &source.file, &source.sheet);
            Ok(FillStart::NeedsDraft(Box::new(DraftJob {
                source,
                facts,
                kinds,
            })))
        }
        fill_plan::FillDecision::Editorial => Ok(FillStart::None),
    }
}

/// What door 1 comes to before any gateway call.
enum FillStart {
    /// Not a merge request the program serves: the question goes on to the other doors.
    None,
    /// A plan, or a deterministic answer asking for what is missing.
    Answer(Box<AskAnswer>),
    /// A named workbook and no template that fits: the template is drafted from the column names.
    NeedsDraft(Box<DraftJob>),
}

/// Owned data only (`IndexStore` is not `Sync`): everything the draft and its plan need.
struct DraftJob {
    source: tabular_answer::FillSource,
    facts: tabular_answer::SelectionFacts,
    kinds: Vec<Option<String>>,
}

/// A merge request for a workbook the question names and no template fits: one gateway call drafts
/// the template from the column names, the kind of each column and the user's own request - no cell
/// value, no row. The plan is built from what comes back, with the same card as a template file;
/// nothing is written. When no template can be drafted (the AI machine cannot be reached, or twice an
/// answer with no placeholder) the answer is `FillNeedsTemplate`, with zero letters: a model never
/// improvises the letters. The user's own stop ends the call like any other question.
async fn fill_draft_tier(
    state: &AppState,
    question: &str,
    job: DraftJob,
    measured: &mut RetrievalDiagnostics,
) -> Result<AskAnswer, AppError> {
    let DraftJob {
        source,
        facts,
        kinds,
    } = job;
    let (server_url, model_alias, locale, idle_timeout) = state.read(|settings| {
        (
            settings.server_url.clone(),
            settings.model_alias.clone(),
            settings
                .locale
                .clone()
                .unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string()),
            settings.answer_idle_timeout(),
        )
    })?;
    let context = fill_draft::DraftContext {
        locale: &locale,
        gateway: &state.gateway,
        server_url: &server_url,
        model_alias: &model_alias,
        idle_timeout,
    };
    let started = std::time::Instant::now();
    let decision = fill_plan::draft_decision(&context, &kinds, question, source).await?;
    measured.generation_total_ms = elapsed_ms(started.elapsed());
    Ok(match decision {
        fill_plan::FillDecision::Plan(plan) => AskAnswer {
            workbook_use: workbook_use::trace(
                &facts,
                workbook_use::Answered::Plan(&plan.data_file),
            ),
            fill_plan: Some(plan),
            ..AskAnswer::default()
        },
        fill_plan::FillDecision::Ask(answer) => AskAnswer {
            workbook_use: workbook_use::trace(&facts, workbook_use::Answered::Engine(&answer)),
            tabular_answer: Some(answer),
            ..AskAnswer::default()
        },
        // `draft_decision` answers with one of the two above.
        fill_plan::FillDecision::NeedsDraft(_) | fill_plan::FillDecision::Editorial => {
            return Err(AppError::Internal)
        }
    })
}

/// What the preview and the generation both start from: the template and the table, read again from
/// their folders by relative path - never trusted from the interface.
fn fill_inputs(
    app: &AppHandle,
    state: &AppState,
    request: &fill_plan::FillRequest,
) -> Result<
    (
        std::path::PathBuf,
        fill_plan::TemplateFile,
        crate::template_fill::Table,
    ),
    AppError,
> {
    let (work_folder, data_folder, locale) = state.read(|settings| {
        (
            settings.work_folder.clone(),
            settings.data_folder.clone(),
            settings.locale.clone(),
        )
    })?;
    let work_folder = work_folder.ok_or(AppError::NoWorkFolderSet)?;
    if data_folder.is_none() {
        return Err(AppError::NoDataFolderSet);
    }
    let locale = locale.unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string());
    let index = open_index(app)?;
    // A drafted template is no file: its text comes back from the card and is checked again (size,
    // placeholders against the table's real columns when the letter is filled).
    let template = match &request.draft_text {
        Some(text) => fill_plan::TemplateFile::drafted(text, &request.data_file)?,
        None => {
            let inventory = family_views_of(state, &index)?
                .documents
                .ok_or(AppError::NoWorkFolderSet)?;
            fill_plan::read_template(&inventory, &request.template)
                .ok_or(AppError::FillSourceUnavailable)?
        }
    };
    let data = family_views_of(state, &index)?
        .data
        .ok_or(AppError::NoDataFolderSet)?;
    let (_, table) = tabular_answer::load_fill_table(
        &data,
        &request.data_file,
        request.sheet.as_deref(),
        &locale,
        &index,
    )
    .ok_or(AppError::FillSourceUnavailable)?;
    Ok((std::path::PathBuf::from(work_folder), template, table))
}

/// The preview of a mail merge for the choices she made on the plan card.
#[tauri::command]
pub fn fill_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    request: fill_plan::FillRequest,
) -> Result<fill_plan::FillPreview, AppError> {
    let (_, template, table) = fill_inputs(&app, &state, &request)?;
    fill_plan::preview(&template, &table, &request)
}

/// Writes the letters she approved, into a `Generated` subfolder of the documents folder. The only
/// place a mail merge writes anything, and only ever new files.
#[tauri::command]
pub fn fill_generate(
    app: AppHandle,
    state: State<'_, AppState>,
    request: fill_plan::FillRequest,
) -> Result<fill_plan::FillReport, AppError> {
    let (work_folder, template, table) = fill_inputs(&app, &state, &request)?;
    let output_folder = state.read(|settings| settings.generated_folder())?;
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| AppError::FillWriteFailed)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0);
    fill_plan::generate(
        &work_folder,
        &output_folder,
        &directory.join("generated-files.jsonl"),
        now,
        &template,
        &table,
        &request,
    )
}

/// The workbooks the knowledge base found what the question names in, best first, whether or not
/// its reduction was applied (empty when the mode is `off` or the plan is `unplanned`). A workbook
/// the base never read is left out: it holds no evidence of anything.
fn implied_by_entity_candidates(plan: &TargetingPlan) -> Vec<String> {
    let side = &plan.reduction.data;
    side.candidates
        .iter()
        .filter(|file| !side.kept_unread.contains(file))
        .cloned()
        .collect()
}

/// Session 16 (`docs/SESSION-DATA-16-Mixed-Tier.md`): documents and tables are both selected, and
/// `mixed_data_only_tier` already decided this question is not clearly and only about the data.
/// Reads the same two folders `tabular_tier`/`mixed_data_only_tier` already do, resolves the
/// document selection exactly as tier 1 does, then hands everything to `mixed_answer::answer` -
/// the Tauri-free module that actually decomposes, links an entity, computes, generates and
/// checks. This function is the thin wrapper around it: settings, the local index, and forwarding
/// `mixed_answer::answer`'s two callbacks to the real `Channel`.
#[allow(clippy::too_many_arguments)]
async fn mixed_tier(
    app: &AppHandle,
    state: &AppState,
    question: &str,
    scope: &AnalysisScope,
    history: &[ChatTurn],
    targeting_request: &TargetingRequest,
    on_event: &Channel<ChatStreamEvent>,
    measured: &mut RetrievalDiagnostics,
    // The figure the user is asking to have analysed (TABULAR step 06), already computed again by the
    // program: it is the table side of the turn instead of what the question would classify as, and
    // `ANALYSIS_ADDENDUM` joins the instruction. `None` for every ordinary question.
    known_figure: Option<TabularAnswer>,
    // The state the last table answer left: a short follow-up continues it (TABULAR step 11).
    carry: Option<&TableCarryOver>,
) -> Result<AskAnswer, AppError> {
    let (
        work_folder,
        data_folder,
        server_url,
        model_alias,
        embedding_alias,
        locale,
        idle_timeout,
        rows_mode,
        application,
    ) = state.read(|settings| {
        (
            settings.work_folder.clone(),
            settings.data_folder.clone(),
            settings.server_url.clone(),
            settings.model_alias.clone(),
            settings.embedding_alias.clone(),
            settings.locale.clone(),
            settings.answer_idle_timeout(),
            settings.tabular_rows_mode,
            settings.spreadsheet_application,
        )
    })?;
    if work_folder.is_none() {
        return Err(AppError::NoWorkFolderSet);
    }
    if data_folder.is_none() {
        return Err(AppError::NoDataFolderSet);
    }
    let locale = locale.unwrap_or_else(|| settings::DEFAULT_LOCALE.to_string());

    // `index` stays owned, never borrowed into a struct held across an `.await`, for the same
    // `IndexStore`-is-not-`Sync` reason `tabular_answer`'s own module doc gives - the exact pattern
    // `sourced_answer`'s tier 1 arm already uses below (`open_index`, then a plain local, borrowed
    // only in the synchronous stretches between awaits).
    let mut stage = StageTimer::start();
    let index = open_index(app)?;
    let folder = family_views_of(state, &index)?
        .data
        .ok_or(AppError::NoDataFolderSet)?;

    // The document selection, resolved exactly as tier 1 does: a selection whose files are all
    // gone or changed is refused rather than quietly answered from nothing, the same guarantee
    // `sourced_answer`'s own documents-tier arm already gives.
    let full_inventory = family_views_of(state, &index)?
        .documents
        .ok_or(AppError::NoWorkFolderSet)?;
    measured.inventory_ms += stage.lap_ms();
    let resolution = scope.resolve(&full_inventory);
    let scope_outdated: Vec<String> = resolution
        .missing
        .iter()
        .cloned()
        .chain(resolution.changed.iter().cloned())
        .collect();
    let document_paths: Vec<String> = resolution
        .inventory
        .indexed_files()
        .into_iter()
        .map(|file| file.relative_path.clone())
        .collect();
    measured.selected_documents = document_paths.len();
    measured.selected_workbooks = selected_workbooks(&folder, &scope.data_mode);
    measured.scope_ms += stage.lap_ms();
    if resolution.narrowed && document_paths.is_empty() && !scope_outdated.is_empty() {
        return Err(AppError::ScopeUnavailable);
    }

    // The tabular part, classified against the selected tables alone - entirely synchronous, so
    // `&index` never has to survive an `.await` to produce it. After the document selection is
    // known, because a word of a document's name designates the document, not a table value.
    // What the knowledge base makes of the question, on both sides at once. A document or a workbook
    // the question names is never reduced (invariant I2); the engine only ever receives a shorter
    // list taken from the selection (invariants I1 and I5); `mixed_answer::entity_link` is untouched
    // and still ties a document's entity to a table row after retrieval.
    let document_files = selected_documents(&resolution.inventory, &document_paths);
    let workbook_files = selected_workbook_files(&folder, &scope.data_mode);
    // A selected document the question names exactly is targeted: the search is held to it and the
    // model is told which file it is (TABULAR step 08, case I). An ambiguous name, or one that points
    // outside the selection, keeps the search over the whole selection.
    let named_document =
        mixed_answer::named_document(question, &resolution.inventory, &document_paths);
    let names_a_document = named_document.names_a_document;
    let targeted_document = named_document.targeted;
    let folder_facts = match &targeted_document {
        Some(relative_path) => {
            mixed_answer::folder_facts_block(&resolution.inventory, relative_path)
        }
        None => String::new(),
    };
    let documents_side = match names_a_document {
        true => SideInput::new(&document_files).blocked_by(SkipReason::NamedFile),
        false => SideInput::new(&document_files),
    };
    let data_side = match tabular_answer::names_a_workbook(question, &folder, &scope.data_mode) {
        true => SideInput::new(&workbook_files).blocked_by(SkipReason::NamedFile),
        false => SideInput::new(&workbook_files),
    };
    let setup = TargetingSetup::load(app, state);
    let targeting_plan = setup.plan(
        &index,
        targeting_request,
        question,
        documents_side,
        data_side,
    );
    let effective_selection = match targeting_plan.applied_workbooks() {
        Some(candidates) => {
            targeting::narrowed_data_selection(&folder, &scope.data_mode, candidates)
        }
        None => scope.data_mode.clone(),
    };
    let analysis = known_figure.is_some();
    let (pending, carry_report) = match known_figure {
        Some(figure) => (
            tabular_answer::PendingAnswer::Done(figure),
            CarryReport::default(),
        ),
        None => tabular_answer::prepare_for_mixed_with_carry_over(
            question,
            &folder,
            selection_for_follow_up(
                carry,
                question,
                &locale,
                &scope.data_mode,
                &effective_selection,
            ),
            &locale,
            &index,
            &document_paths,
            carry,
        )?,
    };
    measured.routing_ms += stage.lap_ms();
    measured.absorb_workbook_loads(diagnostics::take_workbook_loads());
    // Recorded now so that a question whose documents turn out to be unavailable still leaves its
    // row; the documents' own comparison replaces it below.
    targeting::note_workbooks(&index, &targeting_plan, None, measured);
    let mut widened = false;

    // The chosen model's budget, read before the search: how much of the documents the search may
    // keep follows the window of the model that will read them.
    let budget = state
        .model_budgets
        .lock()
        .map_err(|_| AppError::Internal)?
        .for_alias(&model_alias);

    // Retrieval, before anything else reads its result (mechanism 2): an embedding call, then a
    // search held to the document selection - `index` borrowed only in the synchronous stretch
    // right after the embed `.await`, exactly as tier 1's own retrieval already is.
    // An explanation, a description, a comparison or a formula request about a workbook, that names no
    // document, is written from the cards alone: the excerpts of an unrelated document made a small
    // model answer from the contract instead of the table (measured 22 October 2026, TABULAR step 05).
    // No embedding, no search, and the excerpt block reads `none`.
    let advisory = tabular_answer::advisory_skips_documents(
        question,
        &folder,
        &scope.data_mode,
        &locale,
        &implied_by_entity_candidates(&targeting_plan),
        names_a_document,
        &document_paths,
    );
    let documents_are_searchable =
        !advisory && matches!(index.chunk_count(), Ok(count) if count > 0);
    let (document_sources, documents_unavailable) =
        if document_paths.is_empty() || !documents_are_searchable {
            (
                Vec::new(),
                Some(match advisory {
                    true => mixed_answer::MixedPartUnavailable::NotAskedAbout,
                    false => mixed_answer::MixedPartUnavailable::NoEvidence,
                }),
            )
        } else {
            let search_text = conversation::retrieval_query(question, history);
            stage.lap_ms();
            let embedded = state
                .gateway
                .embed(
                    &server_url,
                    &embedding_alias,
                    std::slice::from_ref(&search_text),
                )
                .await;
            measured.embedding_ms = stage.lap_ms();
            match embedded {
                Ok(vectors) => {
                    let query_embedding = vectors.into_iter().next().unwrap_or_default();
                    let searched = targeting::search_documents_within(
                        &index,
                        &targeting_plan,
                        match &targeted_document {
                            Some(relative_path) => DocumentPlan::OneFile(relative_path.as_str()),
                            None => DocumentPlan::WholeFolder {
                                narrowed: true,
                                scoped_paths: &document_paths,
                                present_paths: &document_paths,
                            },
                        },
                        &search_text,
                        &query_embedding,
                        retrieval::RetrievalLimits::for_budget(&budget),
                    );
                    measured.search_ms = stage.lap_ms();
                    match searched {
                        Ok(found) => {
                            measured.chunks_considered = found.outcome.chunks_considered;
                            widened = found.widened;
                            targeting::note_documents(&index, &targeting_plan, &found, measured);
                            match found.outcome.evidence.is_empty() {
                                false => (found.outcome.evidence, None),
                                true => (
                                    Vec::new(),
                                    Some(mixed_answer::MixedPartUnavailable::NoEvidence),
                                ),
                            }
                        }
                        Err(_) => (
                            Vec::new(),
                            Some(mixed_answer::MixedPartUnavailable::NoEvidence),
                        ),
                    }
                }
                Err(_) => (
                    Vec::new(),
                    Some(mixed_answer::MixedPartUnavailable::GatewayUnavailable),
                ),
            }
        };

    // The workbooks the question names or concerns reach the model as cards (structure only, no
    // value), computed from what the user ticked and not from the knowledge base's narrowed list
    // (`tabular_answer::question_cards`). Doors 1 to 3 have returned before this function, so
    // `implied_single` needs no filtering here.
    // The workbook a short follow-up continued is the one the question is about, although it does not name it.
    let continued = carry_report
        .inherited
        .as_ref()
        .and(carry_report.next.as_ref())
        .map(|next| next.workbook.as_str());
    let cards = tabular_answer::question_cards_carrying(
        question,
        &folder,
        &scope.data_mode,
        &locale,
        &implied_by_entity_candidates(&targeting_plan),
        card_budget_chars(&budget),
        continued,
    );
    let card_block = &cards.block;
    let implied = &cards.implied;

    // The sample of rows and the column profile of the workbooks that got a card, when the setting
    // allows it: read through the cache the answer path uses, synchronously, before the first
    // `.await` below. With the setting `off` nothing is read and the turn is the turn without it.
    // `auto` adds the rows the engine chooses with the filters of the question or of the excerpts, for a
    // request to write or to analyse (TABULAR step 10); never for the analysis of a figure, never for a
    // calculation. The excerpts are capped as the writer will cap them, so the entity linked is one it
    // is sent.
    let ask_rows = rows_mode == settings::TabularRowsMode::Auto
        && !analysis
        && tabular_answer::wants_rows_for_writer(
            question,
            &folder,
            &scope.data_mode,
            &locale,
            &implied_by_entity_candidates(&targeting_plan),
        );
    let linked_excerpts = mixed_answer::cap_document_share(
        &document_sources,
        budget.scaled_over_baseline(mixed_answer::MIXED_DOCUMENT_SHARE_CHARS),
    );
    let table_data = match rows_mode {
        settings::TabularRowsMode::Off => tabular_answer::TableData::default(),
        settings::TabularRowsMode::Sample | settings::TabularRowsMode::Auto => {
            tabular_answer::question_table_data(
                card_block,
                &folder,
                &scope.data_mode,
                &locale,
                &index,
                &budget,
                &crate::tabular::sample::fresh_token(),
                ask_rows.then_some(tabular_answer::RowsAsk {
                    question,
                    excerpts: &linked_excerpts,
                }),
            )
        }
    };
    let sample_block = &table_data.sample;
    let rows_block = &table_data.rows;
    let trace_uses = crate::tabular::rows::with_chosen_rows(&sample_block.uses, rows_block);

    let sheet_sizes = tabular_answer::sheet_sizes(&folder, &scope.data_mode);
    let formula_context = crate::tabular::formula_check::FormulaContext::new(
        application.as_str(),
        &locale,
        &card_block.cards,
    );
    let context = mixed_answer::MixedContext {
        locale: &locale,
        gateway: &state.gateway,
        server_url: &server_url,
        model_alias: &model_alias,
        idle_timeout,
        history,
        budget,
        cards_text: &card_block.text,
        sample_text: &sample_block.text,
        calculation: mixed_answer::CalculationSide {
            sheet_sizes: &sheet_sizes,
            analysis,
        },
        formulas: formula_context.as_ref(),
        folder_facts: &folder_facts,
        rows: (!rows_block.is_empty()).then_some(rows_block),
    };

    // Nothing below touches `index`, `folder` or the document inventory again: everything either
    // function needs from here on is owned data already extracted above
    // (`mixed_answer::MixedContext`'s own doc explains why that boundary matters).
    measured.chunks_selected = document_sources.len();
    measured.estimated_input_chars = document_sources
        .iter()
        .map(|item| item.text.chars().count())
        .sum();
    // What the knowledge base did to the search, before the answer starts.
    let targeting_report = targeting_plan.report(widened);
    if let Some(report) = &targeting_report {
        let _ = on_event.send(ChatStreamEvent::Targeting {
            report: Box::new(report.clone()),
        });
    }
    // The whole of `mixed_answer::answer`: decomposing, computing, writing and checking. Its first
    // word is the first delta the model sends.
    let generation_started = std::time::Instant::now();
    let first_token = std::sync::OnceLock::<u64>::new();
    let (result, generated) = mixed_answer::answer(
        question,
        pending,
        document_sources,
        documents_unavailable,
        &context,
        |sources| {
            let _ = on_event.send(ChatStreamEvent::Sources {
                sources: sources.to_vec(),
                coverage: None,
            });
        },
        |delta| {
            first_token.get_or_init(|| elapsed_ms(generation_started.elapsed()));
            let _ = on_event.send(ChatStreamEvent::Delta {
                text: delta.to_string(),
            });
        },
    )
    .await?;
    measured.generation_total_ms = elapsed_ms(generation_started.elapsed());
    measured.generation_first_token_ms = first_token.get().copied();
    if generated {
        let _ = on_event.send(ChatStreamEvent::Completed {
            text: result.answer.clone(),
        });
    }

    // What this answer did with each workbook: the model read a card only if it wrote an answer, and
    // a figure was computed only if the table side holds a value.
    let workbook_use = workbook_use::trace(
        &tabular_answer::selection_facts(question, &folder, &scope.data_mode),
        workbook_use::Answered::Mixed {
            cards: &card_block.cards,
            implied,
            table: &result.table,
            model_wrote: generated && !result.answer.is_empty(),
            samples: &trace_uses,
        },
    );

    Ok(AskAnswer {
        scope_outdated,
        mixed_answer: Some(result),
        targeting: targeting_report,
        workbook_use,
        ..AskAnswer::default()
    }
    .with_carry_over(carry_report))
}

/// Sidecar discovery: look next to the executable, a few ancestors up (so `tauri dev` can see
/// copies left in `src-tauri/`), and under the resource directory. Never a hardcoded install
/// path. A missing engine is `None`, and indexing degrades to Sprint 2a.
pub(crate) fn search_roots(app: &AppHandle) -> Vec<std::path::PathBuf> {
    sidecar_search_roots(
        app.path().resource_dir().ok(),
        std::env::current_exe().ok(),
        std::env::current_dir().ok(),
    )
}

pub(crate) fn sidecar_search_roots(
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

#[cfg(test)]
mod answer_shape_tests {
    use super::*;

    fn bare_answer() -> AskAnswer {
        AskAnswer {
            answer: "Reponse".to_string(),
            unanalysed_files: 2,
            scope_outdated: vec!["a.pdf".to_string()],
            ..AskAnswer::default()
        }
    }

    /// `src/lib/ipc.ts` reads exactly these keys. Adding a field must not rename or drop one.
    #[test]
    fn an_answer_still_serialises_every_field_the_interface_reads() {
        let json = serde_json::to_value(bare_answer()).expect("serialises");

        for key in [
            "answer",
            "sources",
            "folderAnswer",
            "coverage",
            "unanalysedFiles",
            "scopeOutdated",
            "withoutDocuments",
            "tabularAnswer",
            "documentsNotNeeded",
            "mixedAnswer",
            "unverifiedNumbers",
            "fillPlan",
            "targeting",
            "workbookUse",
        ] {
            assert!(json.get(key).is_some(), "{key} is missing from {json}");
        }
        assert_eq!(json["answer"], "Reponse");
        assert_eq!(json["unanalysedFiles"], 2);
        assert_eq!(json["scopeOutdated"][0], "a.pdf");
    }

    #[test]
    fn the_diagnostics_travel_with_the_answer_as_numbers_and_codes() {
        let mut answer = bare_answer();
        let mut measured = RetrievalDiagnostics::new(RetrievalPath::Documents);
        measured.plan = Some(RetrievalPlan::WholeFolder);
        measured.chunks_selected = 4;
        measured.total_ms = 2_300;
        answer.diagnostics = Some(measured);

        let json = serde_json::to_value(&answer).expect("serialises");

        assert_eq!(json["diagnostics"]["path"], "documents");
        assert_eq!(json["diagnostics"]["plan"], "whole_folder");
        assert_eq!(json["diagnostics"]["chunksSelected"], 4);
        assert_eq!(json["diagnostics"]["totalMs"], 2_300);
        assert!(serde_json::to_value(bare_answer()).unwrap()["diagnostics"].is_null());
    }
    #[test]
    fn a_short_question_inherits_the_reading_of_the_figure_it_follows() {
        use crate::tabular::engine::Operation;
        let mut carry = TableCarryOver::from_operation(
            "a.xlsx",
            "Data",
            &Operation::Count {
                filters: Vec::new(),
            },
        )
        .unwrap();
        carry.row_policy = Some(RowPolicy::Exclude);

        // The choice made on an answer wins.
        assert_eq!(
            resolve_row_policy(Some(RowPolicy::Include), Some(&carry), "and for March?"),
            RowPolicy::Include
        );
        // A short question after a figure that left the rows out leaves them out too.
        assert_eq!(
            resolve_row_policy(None, Some(&carry), "and for March?"),
            RowPolicy::Exclude
        );
        // A long question stands alone, and so does a question with nothing before it.
        assert_eq!(
            resolve_row_policy(
                None,
                Some(&carry),
                "what is the total of all the invoices of the second quarter for every supplier"
            ),
            RowPolicy::Include
        );
        assert_eq!(
            resolve_row_policy(None, None, "and for March?"),
            RowPolicy::Include
        );
    }
}
