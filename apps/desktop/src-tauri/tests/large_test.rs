//! The large reproducible test, the part that runs the application's own code (the rest is Python:
//! `scripts/gen-knowledge-sandbox.py --profile demo` writes the library and the book,
//! `scripts/large_test/` scores). Protocol: `docs/test-reports/large-test/README.md`.
//!
//! Three entry points, all `#[ignore]` (they read a generated library and, but for `fake`, the AI gateway):
//!
//! - `analyse`: the Analyse of the library on the real stack (embeddings, the bundled OCR), into an index kept
//!   under `ACAI_LT_INDEX`, with its summary lines and its time written to `<out>/analyse.json`;
//! - `routes`: every question of the book through the application's routing and retrieval, **with no chat
//!   model**: the route taken, the passages and files found, what the deterministic engines answer. Needs the
//!   embedding model (the question is embedded) and the index, never an answering model;
//! - `run`: the same, and the answer written by `ACAI_MODEL` exactly where the application writes it.
//!
//! The run never touches the application's data: the index is copied to a temporary folder and the library is
//! only read. What is replayed is `commands::sourced_answer` and its tiers, step for step, from the same public
//! modules (the dispatch itself lives in the window's command layer and cannot be called without a window).
//!
//! ```text
//! ACAI_LT_LIBRARY=data/large-test/library   ACAI_LT_INDEX=data/large-test/index/assistant-embed
//! ACAI_LT_OUT=data/large-test/2026-10-16-ministral   ACAI_GATEWAY=http://192.168.1.83:8080
//! ACAI_EMBED_ALIAS=assistant-embed   ACAI_MODEL=ministral-3:3b
//! cargo test --release --test large_test analyse -- --ignored --nocapture
//! cargo test --release --test large_test routes  -- --ignored --nocapture
//! cargo test --release --test large_test run     -- --ignored --nocapture
//! ```
//!
//! `ACAI_LT_FAKE=1` replaces the gateway with the tests' own fake (vectors with no meaning, no answer): enough to
//! check that a library and a book hold together, never to judge a result.

mod common;

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use assistant_cabinet_ai_lib::analysis_scope::{
    AnalysisScope, GroundingTier, ScopeEntry, ScopeMode,
};
use assistant_cabinet_ai_lib::conversation::{self, ContextBudget, ModelBudgets};
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::error::AppError;
use assistant_cabinet_ai_lib::file_profile::vocabulary::ProfileVocabulary;
use assistant_cabinet_ai_lib::finder::{self, InventoryFile};
use assistant_cabinet_ai_lib::folder_questions::{self, FolderAnswer, QuestionRoute};
use assistant_cabinet_ai_lib::gateway::{ChatTurn, GatewayClient, GROUNDED_TEMPERATURE};
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::indexing;
use assistant_cabinet_ai_lib::inventory::{FileHashCache, WorkFolderInventory};
use assistant_cabinet_ai_lib::knowledge::ingest::KnowledgeContext;
use assistant_cabinet_ai_lib::knowledge::packs::PackSet;
use assistant_cabinet_ai_lib::knowledge::query::SelectedFile;
use assistant_cabinet_ai_lib::knowledge::reduce::{SideInput, SkipReason, TargetingPolicy};
use assistant_cabinet_ai_lib::knowledge::secret::IdentifierKey;
use assistant_cabinet_ai_lib::knowledge::targeting::{
    self, DocumentPlan, QueryVocabulary, TargetingPlan, TargetingRequest, TargetingTools,
};
use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;
use assistant_cabinet_ai_lib::mixed_answer::{self, MixedContext, MixedPartUnavailable};
use assistant_cabinet_ai_lib::number_check;
use assistant_cabinet_ai_lib::ocr::tesseract::TesseractProvider;
use assistant_cabinet_ai_lib::raster::{self, PageRasterizer};
use assistant_cabinet_ai_lib::retrieval::{self, Evidence};
use assistant_cabinet_ai_lib::tabular_answer::{self, ModelAssist, PendingAnswer, TabularAnswer};
use assistant_cabinet_ai_lib::work_folder_context::{self, ContextView};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const LOCALE: &str = "fr-FR";
/// Both countries are active together, as for a cross-border worker (`docs/DECISIONS.md`).
const PACKS: [&str; 2] = ["health-fr", "health-ch"];
/// The identifier key of a test run: the same for every run, so a stored index stays readable.
const KEY: [u8; 32] = [7; 32];

fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

fn manifest(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

// ------------------------------------------------------------------------------------------------ the book

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Pick {
    Word(String),
    Paths { paths: Vec<String> },
}

impl Default for Pick {
    fn default() -> Self {
        Pick::Word("all".to_string())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Selection {
    #[serde(default)]
    documents: Pick,
    #[serde(default = "no_workbooks")]
    workbooks: Pick,
}

fn no_workbooks() -> Pick {
    Pick::Word("none".to_string())
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            documents: Pick::default(),
            workbooks: no_workbooks(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct BookQuestion {
    id: String,
    #[serde(default)]
    block: String,
    text: String,
    #[serde(default)]
    follow_up_of: Option<String>,
    #[serde(default)]
    selection: Selection,
}

fn load_book(library: &Path) -> Vec<BookQuestion> {
    let path = std::env::var("ACAI_LT_BOOK")
        .map(PathBuf::from)
        .unwrap_or_else(|_| library.join("questions.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("reads the book {}: {error}", path.display()));
    let mut book: Vec<BookQuestion> =
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("parses the book: {error}"));
    if let Ok(only) = std::env::var("ACAI_LT_ONLY") {
        let wanted: Vec<&str> = only.split(',').map(str::trim).collect();
        // A follow-up needs the question it follows: keep it with its parent.
        let parents: Vec<String> = book
            .iter()
            .filter(|q| wanted.contains(&q.id.as_str()))
            .filter_map(|q| q.follow_up_of.clone())
            .collect();
        book.retain(|q| wanted.contains(&q.id.as_str()) || parents.contains(&q.id));
    }
    if let Ok(limit) = std::env::var("ACAI_LT_LIMIT") {
        if let Ok(limit) = limit.parse::<usize>() {
            book.truncate(limit);
        }
    }
    book
}

// ------------------------------------------------------------------------------------------------ the stack

struct Stack {
    client: GatewayClient,
    url: String,
    embed_alias: String,
    model: String,
    /// Kept alive for the whole run when the fake gateway is used.
    _fake: Option<common::fake_gateway::FakeGateway>,
}

fn stack() -> Stack {
    if env("ACAI_LT_FAKE", "0") == "1" {
        let fake =
            common::fake_gateway::FakeGateway::start(|_| common::fake_gateway::Reply::vectors());
        return Stack {
            client: common::knowledge::gateway_client(),
            url: fake.url.clone(),
            embed_alias: "cabinet-embed".to_string(),
            model: "fake".to_string(),
            _fake: Some(fake),
        };
    }
    Stack {
        client: GatewayClient::new().expect("builds the gateway client"),
        url: env("ACAI_GATEWAY", "http://127.0.0.1:8080"),
        embed_alias: env("ACAI_EMBED_ALIAS", "assistant-embed"),
        model: env("ACAI_MODEL", "ministral-3:3b"),
        _fake: None,
    }
}

/// The bundled OCR engine and page rasteriser, when they are staged (`scripts/fetch-ocr-resources.ps1`).
fn real_ocr() -> (
    Option<TesseractProvider>,
    Option<&'static raster::Rasterizer>,
) {
    let binary = std::env::var("ACAI_TESSERACT")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            [
                "binaries/tesseract-x86_64-pc-windows-msvc.exe",
                "binaries/tesseract-aarch64-apple-darwin",
                "binaries/tesseract-x86_64-apple-darwin",
                "tesseract.exe",
            ]
            .iter()
            .map(|p| manifest(p))
            .find(|p| p.exists())
        });
    let tessdata = std::env::var("ACAI_TESSDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest("resources/tessdata"));
    let engine = binary.and_then(|b| TesseractProvider::new(b, tessdata).ok());
    let library = std::env::var("ACAI_PDFIUM")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            [
                "resources/pdfium/pdfium.dll",
                "resources/pdfium/libpdfium.dylib",
                "resources/pdfium/libpdfium.so",
            ]
            .iter()
            .map(|p| manifest(p))
            .find(|p| p.exists())
        });
    let rasterizer = library.and_then(|l| raster::shared(&l));
    (engine, rasterizer)
}

fn out_dir() -> PathBuf {
    let dir = PathBuf::from(env("ACAI_LT_OUT", "target/large-test-out"));
    std::fs::create_dir_all(&dir).expect("creates the result folder");
    dir
}

fn library_dir() -> PathBuf {
    PathBuf::from(env("ACAI_LT_LIBRARY", "data/large-test/library"))
}

fn index_dir() -> PathBuf {
    PathBuf::from(env("ACAI_LT_INDEX", "data/large-test/index"))
}

fn pick_context(index: &IndexStore) -> KnowledgeContext {
    KnowledgeContext::for_index(
        index,
        LOCALE,
        &PACKS,
        KnowledgeMode::Suggest,
        IdentifierKey::from_bytes(KEY),
    )
    .expect("builds the knowledge context")
}

// ---------------------------------------------------------------------------------------------- analyse

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn analyse() {
    let library = library_dir();
    let (work, data) = (library.join("documents"), library.join("data"));
    let stack = stack();
    let index_folder = index_dir();
    if std::env::var("ACAI_LT_FRESH").is_ok() && index_folder.exists() {
        std::fs::remove_dir_all(&index_folder).expect("clears the index folder");
    }
    std::fs::create_dir_all(&index_folder).expect("creates the index folder");
    let mut index =
        IndexStore::open_at(&index_folder.join("index.sqlite3")).expect("opens the index");
    let (ocr, rasterizer) = real_ocr();
    let ocr_ref: Option<&dyn assistant_cabinet_ai_lib::ocr::OcrProvider> = ocr
        .as_ref()
        .map(|e| e as &dyn assistant_cabinet_ai_lib::ocr::OcrProvider);
    let raster_ref: Option<&dyn PageRasterizer> = rasterizer.map(|r| r as &dyn PageRasterizer);
    eprintln!(
        "OCR engine: {}, page rasteriser: {}",
        ocr.is_some(),
        rasterizer.is_some()
    );

    let started = Instant::now();
    let last = std::sync::Mutex::new(Instant::now());
    let mut context = pick_context(&index);
    let documents = indexing::run_with_knowledge(
        &mut index,
        &stack.client,
        &stack.url,
        &stack.embed_alias,
        &work,
        ocr_ref,
        raster_ref,
        LOCALE,
        &|progress| {
            let mut last = last.lock().unwrap();
            if last.elapsed() > Duration::from_secs(20) {
                *last = Instant::now();
                eprintln!(
                    "  documents {}/{} (vectors {}/{})",
                    progress.processed_files,
                    progress.total_files,
                    progress.vectors_done,
                    progress.vectors_total
                );
            }
        },
        Some(&mut context),
    )
    .await
    .expect("the documents pass completes");
    let documents_seconds = started.elapsed().as_secs_f64();

    let started_data = Instant::now();
    let mut context = pick_context(&index);
    let tables =
        data_folder::analyse_with_knowledge(&data, &mut index, LOCALE, &|_| {}, Some(&mut context))
            .expect("the data pass completes");
    let data_seconds = started_data.elapsed().as_secs_f64();
    // The index is copied file by file for each run: leave nothing in the write-ahead log.
    if let Ok(connection) = rusqlite::Connection::open(index_folder.join("index.sqlite3")) {
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .ok();
    }

    let summary = |s: &indexing::IndexSummary| {
        json!({
            "scanned_files": s.scanned_files, "indexed_files": s.indexed_files, "unchanged_files": s.unchanged_files,
            "empty_files": s.empty_files, "ocr_files": s.ocr_files.len(), "low_confidence_files": s.low_confidence_files.len(),
            "removed_files": s.removed_files, "renamed_files": s.renamed_files.len(), "failed_files": s.failed_files.len(),
            "unavailable_capabilities": s.unavailable_capabilities,
            "unread_formats": s.unread_formats.iter().map(|f| json!({"extension": f.extension, "count": f.count})).collect::<Vec<_>>(),
            "chunk_count": s.chunk_count, "vectors_pending": s.vectors_pending, "cancelled": s.cancelled,
            "timings": s.timings.as_ref().map(|t| serde_json::to_value(t).unwrap_or(Value::Null)),
            "knowledge": s.knowledge.as_ref().map(|k| serde_json::to_value(k).unwrap_or(Value::Null)),
        })
    };
    let report = json!({
        "library": library.display().to_string(),
        "embed_alias": stack.embed_alias,
        "ocr_engine": ocr.is_some(),
        "documents": summary(&documents), "documents_seconds": documents_seconds,
        "data": summary(&tables), "data_seconds": data_seconds,
        "failed_files": documents.failed_files.iter().map(|f| f.path.clone()).collect::<Vec<_>>(),
        "renamed": documents.renamed_files.len() + tables.renamed_files.len(),
    });
    let text = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(out_dir().join("analyse.json"), &text).expect("writes analyse.json");
    println!("{text}");
}

// -------------------------------------------------------------------------------------------------- replay

#[derive(Debug, Serialize, Default)]
struct Source {
    path: String,
    page: u32,
    ordinal: u32,
    section: Option<String>,
}

#[derive(Debug, Serialize, Default)]
struct Replayed {
    id: String,
    block: String,
    /// `documents`, `tables` or `mixed`: the tier the selection puts the question on.
    tier: String,
    /// What the application did: `folder_answer`, `set_search`, `named_file`, `every_document`,
    /// `global_retrieval`, `without_documents`, `tabular:<kind>`, `fill_plan`, `mixed_folder`,
    /// `mixed_data_only:<kind>`, `mixed_tier`, or `error`.
    route: String,
    /// The model's words, or empty when the application answered without it.
    answer: String,
    /// Every string and number of the deterministic answer, flattened, for the scorer.
    structured_text: String,
    structured: Option<Value>,
    sources: Vec<Source>,
    /// The files the search was held to when the knowledge base narrowed it.
    narrowed_to: Option<usize>,
    selected_documents: usize,
    chunks_selected: usize,
    evidence_chars: usize,
    unverified_numbers: Vec<String>,
    error: Option<String>,
    generated: bool,
    embed_ms: f64,
    search_ms: f64,
    generate_ms: f64,
    total_ms: f64,
}

fn flatten(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Number(n) => out.push(n.to_string()),
        Value::Array(items) => items.iter().for_each(|v| flatten(v, out)),
        Value::Object(map) => map
            .iter()
            .filter(|(key, _)| key.as_str() != "kind")
            .for_each(|(_, v)| flatten(v, out)),
        _ => {}
    }
}

fn kind_of(value: &Value) -> String {
    value
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn sources_of(evidence: &[Evidence]) -> Vec<Source> {
    evidence
        .iter()
        .map(|e| Source {
            path: e.relative_path.clone(),
            page: e.page_number,
            ordinal: e.ordinal,
            section: e.section.as_ref().map(|s| s.title.clone()),
        })
        .collect()
}

struct Replay<'a> {
    stack: &'a Stack,
    index: IndexStore,
    /// A second connection to the same file: the finder reads through a plain connection.
    side: rusqlite::Connection,
    work: PathBuf,
    data: PathBuf,
    vocabulary: QueryVocabulary,
    budget: ContextBudget,
    key: IdentifierKey,
    data_hashes: FileHashCache,
}

impl<'a> Replay<'a> {
    async fn open(stack: &'a Stack, library: &Path, index_copy: &Path) -> Replay<'a> {
        let index = IndexStore::open_at(index_copy).expect("opens the copy of the index");
        let side = rusqlite::Connection::open(index_copy).expect("opens a second connection");
        assistant_cabinet_ai_lib::index_store::configure_connection(&side).expect("configures it");
        let mut budgets = ModelBudgets::default();
        if let Ok(health) = stack.client.health(&stack.url).await {
            budgets.update(
                health.context_windows.clone(),
                health.max_output_tokens,
                health.max_context_chars,
            );
        }
        Replay {
            stack,
            index,
            side,
            work: library.join("documents"),
            data: library.join("data"),
            vocabulary: QueryVocabulary::load(LOCALE, &PACKS).expect("loads the vocabulary"),
            budget: budgets.for_alias(&stack.model),
            key: IdentifierKey::from_bytes(KEY),
            data_hashes: FileHashCache::new(),
        }
    }

    fn plan(
        &self,
        request: &TargetingRequest,
        question: &str,
        documents: SideInput<'_>,
        data: SideInput<'_>,
    ) -> TargetingPlan {
        targeting::plan(
            &self.index,
            &TargetingTools {
                vocabulary: &self.vocabulary,
                key: Some(&self.key),
                policy: TargetingPolicy::default(),
            },
            KnowledgeMode::Suggest,
            request,
            question,
            documents,
            data,
        )
    }

    fn scope_of(&self, selection: &Selection) -> AnalysisScope {
        let documents = WorkFolderInventory::discover(&self.work, Some(&self.index))
            .expect("discovers the documents");
        let folder = DataFolder::discover(&self.data, Some(&self.index), &self.data_hashes)
            .expect("discovers the data folder");
        let entries = |pick: &Pick, find: &dyn Fn(&str) -> Option<(String, String)>| -> ScopeMode {
            match pick {
                Pick::Word(word) if word == "all" => ScopeMode::WholeFolder,
                Pick::Word(_) => ScopeMode::nothing(),
                Pick::Paths { paths } => ScopeMode::Explicit(
                    paths
                        .iter()
                        .filter_map(|path| find(path))
                        .map(|(relative_path, pinned_id)| ScopeEntry {
                            relative_path,
                            pinned_id,
                            added_at: 0,
                            sheet_names: Vec::new(),
                        })
                        .collect(),
                ),
            }
        };
        let mode = entries(&selection.documents, &|path| {
            documents
                .find_by_relative_path(path)
                .map(|r| (r.relative_path.clone(), r.id.clone()))
        });
        let data_mode = entries(&selection.workbooks, &|path| {
            folder
                .files()
                .find_by_relative_path(path)
                .map(|r| (r.relative_path.clone(), r.id.clone()))
        });
        AnalysisScope {
            mode,
            data_mode,
            created_at: 0,
            updated_at: 0,
        }
    }

    async fn ask(&self, q: &BookQuestion, history: &[ChatTurn], generate: bool) -> Replayed {
        let started = Instant::now();
        let scope = self.scope_of(&q.selection);
        let mut out = Replayed {
            id: q.id.clone(),
            block: q.block.clone(),
            ..Replayed::default()
        };
        let request = TargetingRequest::default();
        let result = match scope.tier() {
            GroundingTier::Documents => {
                out.tier = "documents".into();
                self.documents_tier(&q.text, &scope, history, &request, generate, &mut out)
                    .await
            }
            GroundingTier::TablesOnly => {
                out.tier = "tables".into();
                self.tabular_tier(&q.text, &scope, &request, generate, &mut out)
                    .await
            }
            GroundingTier::DocumentsAndTables => {
                out.tier = "mixed".into();
                self.mixed(&q.text, &scope, history, &request, generate, &mut out)
                    .await
            }
        };
        if let Err(error) = result {
            out.error = Some(error.code().to_string());
            if out.route.is_empty() {
                out.route = "error".into();
            }
        }
        if let Some(structured) = &out.structured {
            let mut leaves = Vec::new();
            flatten(structured, &mut leaves);
            out.structured_text = leaves.join(" ; ");
        }
        out.total_ms = started.elapsed().as_secs_f64() * 1000.0;
        out
    }

    // ------------------------------------------------------------------------------- documents tier

    async fn documents_tier(
        &self,
        question: &str,
        scope: &AnalysisScope,
        history: &[ChatTurn],
        request: &TargetingRequest,
        generate: bool,
        out: &mut Replayed,
    ) -> Result<(), AppError> {
        let full = WorkFolderInventory::discover(&self.work, Some(&self.index))?;
        let resolution = scope.resolve(&full);
        let narrowed = resolution.narrowed;
        let no_documents_chosen = resolution.no_documents_chosen;
        let scope_outdated = !resolution.missing.is_empty() || !resolution.changed.is_empty();
        let inventory = resolution.inventory;
        let scoped_paths: Vec<String> = inventory
            .indexed_files()
            .into_iter()
            .map(|f| f.relative_path.clone())
            .collect();
        out.selected_documents = scoped_paths.len();
        let selection = folder_questions::Selection {
            full: &full,
            scoped: &inventory,
            narrowed,
            no_documents_chosen,
        };
        let route = folder_questions::route_in_selection(&selection, &self.index, question, LOCALE);

        enum Plan {
            OneFile(String),
            EveryDocument(Vec<String>),
            WholeFolder,
        }
        let plan = match route {
            QuestionRoute::Deterministic(answer) => {
                out.route = "folder_answer".into();
                out.structured = serde_json::to_value(&answer).ok();
                if let Some(v) = &out.structured {
                    out.route = format!("folder_answer:{}", kind_of(v));
                }
                return Ok(());
            }
            QuestionRoute::SetSearch(set) => {
                let (identifiers, names_vocabulary) = self.set_search_tools(&set.text);
                let files: Vec<InventoryFile> = inventory
                    .all_files()
                    .iter()
                    .map(InventoryFile::from)
                    .collect();
                let names =
                    names_vocabulary
                        .as_ref()
                        .map(|vocabulary| finder::names::KnowledgeNames {
                            connection: &self.side,
                            vocabulary,
                        });
                let answer = match &names {
                    Some(names) => finder::set_search::answer_with_names(
                        &self.side,
                        &files,
                        &set,
                        identifiers,
                        names,
                    )?,
                    None => finder::set_search::answer(&self.side, &files, &set, identifiers)?,
                };
                out.route = "set_search".into();
                out.structured = serde_json::to_value(&answer).ok();
                return Ok(());
            }
            QuestionRoute::TargetedRetrieval { file } => Plan::OneFile(file.relative_path.clone()),
            QuestionRoute::PerDocumentRetrieval { files } => {
                Plan::EveryDocument(files.into_iter().map(|f| f.relative_path).collect())
            }
            QuestionRoute::GlobalRetrieval if no_documents_chosen => {
                out.route = "without_documents".into();
                if generate {
                    let started = Instant::now();
                    let text = self
                        .write(
                            conversation::NO_DOCUMENTS_INSTRUCTION.to_string(),
                            history,
                            question,
                            None,
                        )
                        .await?;
                    out.generate_ms = started.elapsed().as_secs_f64() * 1000.0;
                    out.answer = text;
                    out.generated = true;
                }
                return Ok(());
            }
            QuestionRoute::GlobalRetrieval => Plan::WholeFolder,
        };
        out.route = match &plan {
            Plan::OneFile(_) => "named_file",
            Plan::EveryDocument(_) => "every_document",
            Plan::WholeFolder => "global_retrieval",
        }
        .into();
        if narrowed && scoped_paths.is_empty() && scope_outdated {
            return Err(AppError::ScopeUnavailable);
        }
        let view = match &plan {
            Plan::OneFile(path) => ContextView::Targeted {
                relative_path: path.clone(),
            },
            _ => ContextView::Summary,
        };
        if self.index.chunk_count()? == 0 {
            return Err(AppError::InsufficientEvidence);
        }
        let plan_files: Vec<String> = match &plan {
            Plan::OneFile(path) => vec![path.clone()],
            Plan::EveryDocument(paths) => paths.clone(),
            Plan::WholeFolder => scoped_paths.clone(),
        };
        let selected = selected_documents(&inventory, &plan_files);
        let documents_side = match &plan {
            Plan::OneFile(_) => SideInput::new(&selected).blocked_by(SkipReason::NamedFile),
            _ => SideInput::new(&selected),
        };
        let targeting_plan = self.plan(request, question, documents_side, SideInput::new(&[]));
        if let Some(applied) = targeting_plan.applied_documents() {
            out.narrowed_to = Some(applied.len());
        }
        let search_text = conversation::retrieval_query(question, history);
        let started = Instant::now();
        let vectors = self
            .stack
            .client
            .embed(
                &self.stack.url,
                &self.stack.embed_alias,
                std::slice::from_ref(&search_text),
            )
            .await?;
        out.embed_ms = started.elapsed().as_secs_f64() * 1000.0;
        let embedding = vectors.into_iter().next().unwrap_or_default();
        let present: Vec<String> = inventory
            .all_files()
            .iter()
            .map(|f| f.relative_path.clone())
            .collect();
        let started = Instant::now();
        let searched = targeting::search_documents_within(
            &self.index,
            &targeting_plan,
            match &plan {
                Plan::OneFile(path) => DocumentPlan::OneFile(path.as_str()),
                Plan::EveryDocument(paths) => DocumentPlan::EveryDocument(paths),
                Plan::WholeFolder => DocumentPlan::WholeFolder {
                    narrowed,
                    scoped_paths: &scoped_paths,
                    present_paths: &present,
                },
            },
            &search_text,
            &embedding,
            retrieval::RetrievalLimits::for_budget(&self.budget),
        )?;
        out.search_ms = started.elapsed().as_secs_f64() * 1000.0;
        let evidence = searched.outcome.evidence;
        out.chunks_selected = evidence.len();
        out.evidence_chars = evidence.iter().map(|e| e.text.chars().count()).sum();
        out.sources = sources_of(&evidence);
        if evidence.is_empty() {
            return Err(if narrowed && scope_outdated {
                AppError::ScopeUnavailable
            } else {
                AppError::InsufficientEvidence
            });
        }
        if !generate {
            return Ok(());
        }
        let folder_context = work_folder_context::build(&inventory, &view);
        let grounding = format!(
            "{}\n{}",
            work_folder_context::build_system_turn(
                retrieval::RETRIEVAL_INSTRUCTION,
                &folder_context
            ),
            retrieval::format_evidence(&evidence)
        );
        let history = conversation::history_beside_sources(history, self.budget);
        let started = Instant::now();
        let answer = self
            .write(grounding, &history, question, Some(GROUNDED_TEMPERATURE))
            .await?;
        out.generate_ms = started.elapsed().as_secs_f64() * 1000.0;
        let mut known: Vec<&str> = evidence.iter().map(|e| e.text.as_str()).collect();
        known.extend(evidence.iter().map(|e| e.relative_path.as_str()));
        known.push(question);
        let pages: Vec<u32> = evidence.iter().map(|e| e.page_number).collect();
        out.unverified_numbers =
            number_check::unsupported_numbers_with_pages(&answer, &known, &pages, 2);
        out.answer = answer;
        out.generated = true;
        Ok(())
    }

    fn set_search_tools(&self, text: &str) -> (Vec<String>, Option<QueryVocabulary>) {
        let ids: Vec<&str> = PACKS.to_vec();
        let identifiers = match (PackSet::load(LOCALE, &ids), ProfileVocabulary::load(&ids)) {
            (Ok(packs), Ok(vocabulary)) => {
                finder::set_search::identifier_forms(text, &packs, &vocabulary)
            }
            _ => Vec::new(),
        };
        (identifiers, QueryVocabulary::load(LOCALE, &ids).ok())
    }

    /// `Writer::write` of the window's command layer: the history that fits, then one turn carrying the
    /// instruction, the grounding and the question.
    async fn write(
        &self,
        instruction: String,
        history: &[ChatTurn],
        question: &str,
        temperature: Option<f32>,
    ) -> Result<String, AppError> {
        let final_turn = format!("{instruction}\n\nQuestion: {question}");
        let mut turns = conversation::fit_history(
            history,
            self.budget.history_chars(final_turn.chars().count()),
        );
        turns.push(ChatTurn {
            role: "user".to_string(),
            content: final_turn,
        });
        self.stack
            .client
            .chat_at(
                &self.stack.url,
                &self.stack.model,
                Some(LOCALE),
                &turns,
                Duration::from_secs(240),
                temperature,
                |_| {},
            )
            .await
    }

    // --------------------------------------------------------------------------------- tables tier

    fn assist(&self) -> ModelAssist<'_> {
        ModelAssist {
            gateway: &self.stack.client,
            server_url: &self.stack.url,
            model_alias: &self.stack.model,
            idle_timeout: Duration::from_secs(240),
        }
    }

    async fn tabular_tier(
        &self,
        question: &str,
        scope: &AnalysisScope,
        request: &TargetingRequest,
        generate: bool,
        out: &mut Replayed,
    ) -> Result<(), AppError> {
        let folder = DataFolder::discover(&self.data, Some(&self.index), &self.data_hashes)?;
        let files = selected_workbook_files(&folder, &scope.data_mode);
        let side = match tabular_answer::names_a_workbook(question, &folder, &scope.data_mode) {
            true => SideInput::new(&files).blocked_by(SkipReason::NamedFile),
            false => SideInput::new(&files),
        };
        let plan = self.plan(request, question, SideInput::new(&[]), side);
        let selection = match plan.applied_workbooks() {
            Some(candidates) => {
                targeting::narrowed_data_selection(&folder, &scope.data_mode, candidates)
            }
            None => scope.data_mode.clone(),
        };
        let pending =
            tabular_answer::prepare(question, &folder, &selection, LOCALE, &self.index, false)?;
        let pending = match pending {
            PendingAnswer::Done(answer) => {
                PendingAnswer::Done(targeting::decorate_which_workbook(answer, &plan))
            }
            other => other,
        };
        if !generate && matches!(pending, PendingAnswer::TryModel { .. }) {
            out.route = "tabular:model_plan".into();
            return Ok(());
        }
        let started = Instant::now();
        let answer = tabular_answer::resolve(pending, question, LOCALE, &self.assist()).await;
        out.generate_ms = started.elapsed().as_secs_f64() * 1000.0;
        self.note_tabular(&answer, out);
        Ok(())
    }

    fn note_tabular(&self, answer: &TabularAnswer, out: &mut Replayed) {
        let value = serde_json::to_value(answer).unwrap_or(Value::Null);
        out.route = format!("tabular:{}", kind_of(&value));
        out.structured = Some(value);
    }

    // ----------------------------------------------------------------------------------- mixed tier

    async fn mixed(
        &self,
        question: &str,
        scope: &AnalysisScope,
        history: &[ChatTurn],
        request: &TargetingRequest,
        generate: bool,
        out: &mut Replayed,
    ) -> Result<(), AppError> {
        let full = WorkFolderInventory::discover(&self.work, Some(&self.index))?;
        let folder = DataFolder::discover(&self.data, Some(&self.index), &self.data_hashes)?;
        let resolution = scope.resolve(&full);
        let document_paths: Vec<String> = resolution
            .inventory
            .indexed_files()
            .into_iter()
            .map(|f| f.relative_path.clone())
            .collect();
        out.selected_documents = document_paths.len();

        // The request to write something: a plan to approve, nothing written (`fill_tier`).
        if assistant_cabinet_ai_lib::tabular::question::has_writing_intent(question, LOCALE) {
            let every_row = assistant_cabinet_ai_lib::tabular::question::has_distributive_word(
                question, LOCALE,
            );
            if let Some(source) = tabular_answer::locate_fill_source(
                question,
                &folder,
                &scope.data_mode,
                LOCALE,
                &self.index,
                every_row,
            )? {
                let templates = assistant_cabinet_ai_lib::fill_plan::candidate_templates(
                    &resolution.inventory,
                    &source.table.columns,
                );
                if !templates.is_empty() {
                    let plan = assistant_cabinet_ai_lib::fill_plan::build_plan(
                        templates,
                        &source.file,
                        &source.sheet,
                        &source.table,
                        source.key,
                    )?;
                    out.route = "fill_plan".into();
                    out.structured = serde_json::to_value(&plan).ok();
                    return Ok(());
                }
            }
        }

        // A question about the documents themselves, answered from the inventory (or the word search).
        if folder_questions::names_the_documents(question, LOCALE) {
            let selection = folder_questions::Selection {
                full: &full,
                scoped: &resolution.inventory,
                narrowed: resolution.narrowed,
                no_documents_chosen: resolution.no_documents_chosen,
            };
            let set = match folder_questions::route_in_selection(
                &selection,
                &self.index,
                question,
                LOCALE,
            ) {
                QuestionRoute::SetSearch(set) => Some(set),
                _ => None,
            };
            let answer = if let Some(set) = set {
                let workbooks: Vec<InventoryFile> =
                    tabular_answer::selected_workbook_paths(&folder, &scope.data_mode)
                        .iter()
                        .filter_map(|path| folder.files().find_by_relative_path(path))
                        .map(InventoryFile::from)
                        .collect();
                let documents: Vec<InventoryFile> = resolution
                    .inventory
                    .all_files()
                    .iter()
                    .map(InventoryFile::from)
                    .collect();
                let (identifiers, vocabulary) = self.set_search_tools(&set.text);
                let names = vocabulary
                    .as_ref()
                    .map(|vocabulary| finder::names::KnowledgeNames {
                        connection: &self.side,
                        vocabulary,
                    });
                let no_names = finder::names::NoNames;
                let resolver: &dyn finder::names::NameResolver = match &names {
                    Some(names) => names,
                    None => &no_names,
                };
                Some(finder::set_search::answer_both(
                    &self.side,
                    &documents,
                    &workbooks,
                    &set,
                    identifiers,
                    resolver,
                )?)
            } else {
                folder_questions::answer_in_mixed_selection(
                    &selection,
                    &self.index,
                    question,
                    LOCALE,
                )
            };
            if let Some(answer) = answer {
                out.route = match &answer {
                    FolderAnswer::DocumentSearchBoth { .. } => "mixed_set".into(),
                    _ => "mixed_folder".into(),
                };
                out.structured = serde_json::to_value(&answer).ok();
                return Ok(());
            }
        }

        // A question that is clearly and only about the data.
        let workbook_files = selected_workbook_files(&folder, &scope.data_mode);
        let data_side = match tabular_answer::names_a_workbook(question, &folder, &scope.data_mode)
        {
            true => SideInput::new(&workbook_files).blocked_by(SkipReason::NamedFile),
            false => SideInput::new(&workbook_files),
        };
        let plan = self.plan(request, question, SideInput::new(&[]), data_side);
        let effective = match plan.applied_workbooks() {
            Some(candidates) => {
                targeting::narrowed_data_selection(&folder, &scope.data_mode, candidates)
            }
            None => scope.data_mode.clone(),
        };
        if let Some(pending) = tabular_answer::prepare_if_data_only(
            question,
            &folder,
            &effective,
            LOCALE,
            &self.index,
        )? {
            let pending = match pending {
                PendingAnswer::Done(answer) => {
                    PendingAnswer::Done(targeting::decorate_which_workbook(answer, &plan))
                }
                other => other,
            };
            if !generate && matches!(pending, PendingAnswer::TryModel { .. }) {
                out.route = "mixed_data_only:model_plan".into();
                return Ok(());
            }
            let answer = tabular_answer::resolve(pending, question, LOCALE, &self.assist()).await;
            let value = serde_json::to_value(&answer).unwrap_or(Value::Null);
            out.route = format!("mixed_data_only:{}", kind_of(&value));
            out.structured = Some(value);
            return Ok(());
        }

        // The mixed tier proper: the table part, the documents' excerpts, one answer.
        out.route = "mixed_tier".into();
        if resolution.narrowed && document_paths.is_empty() {
            return Err(AppError::ScopeUnavailable);
        }
        let document_files = selected_documents(&resolution.inventory, &document_paths);
        let names_a_document =
            assistant_cabinet_ai_lib::file_reference::FileReferenceResolver::new(
                &resolution.inventory,
            )
            .resolve_in_question(question)
            .is_some();
        let documents_side = match names_a_document {
            true => SideInput::new(&document_files).blocked_by(SkipReason::NamedFile),
            false => SideInput::new(&document_files),
        };
        let data_side = match tabular_answer::names_a_workbook(question, &folder, &scope.data_mode)
        {
            true => SideInput::new(&workbook_files).blocked_by(SkipReason::NamedFile),
            false => SideInput::new(&workbook_files),
        };
        let plan = self.plan(request, question, documents_side, data_side);
        let effective = match plan.applied_workbooks() {
            Some(candidates) => {
                targeting::narrowed_data_selection(&folder, &scope.data_mode, candidates)
            }
            None => scope.data_mode.clone(),
        };
        let pending = tabular_answer::prepare_for_mixed(
            question,
            &folder,
            &effective,
            LOCALE,
            &self.index,
            &document_paths,
        )?;
        let (document_sources, unavailable) =
            if document_paths.is_empty() || self.index.chunk_count().unwrap_or(0) == 0 {
                (Vec::new(), Some(MixedPartUnavailable::NoEvidence))
            } else {
                let search_text = conversation::retrieval_query(question, history);
                let started = Instant::now();
                let embedded = self
                    .stack
                    .client
                    .embed(
                        &self.stack.url,
                        &self.stack.embed_alias,
                        std::slice::from_ref(&search_text),
                    )
                    .await;
                out.embed_ms = started.elapsed().as_secs_f64() * 1000.0;
                match embedded {
                    Ok(vectors) => {
                        let embedding = vectors.into_iter().next().unwrap_or_default();
                        let started = Instant::now();
                        let found = targeting::search_documents_within(
                            &self.index,
                            &plan,
                            DocumentPlan::WholeFolder {
                                narrowed: true,
                                scoped_paths: &document_paths,
                                present_paths: &document_paths,
                            },
                            &search_text,
                            &embedding,
                            retrieval::RetrievalLimits::for_budget(&self.budget),
                        );
                        out.search_ms = started.elapsed().as_secs_f64() * 1000.0;
                        match found {
                            Ok(found) if !found.outcome.evidence.is_empty() => {
                                (found.outcome.evidence, None)
                            }
                            _ => (Vec::new(), Some(MixedPartUnavailable::NoEvidence)),
                        }
                    }
                    Err(_) => (Vec::new(), Some(MixedPartUnavailable::GatewayUnavailable)),
                }
            };
        out.chunks_selected = document_sources.len();
        out.evidence_chars = document_sources
            .iter()
            .map(|e| e.text.chars().count())
            .sum();
        out.sources = sources_of(&document_sources);
        if !generate {
            // The table part is deterministic when the engine answers it; record what it came to.
            if let PendingAnswer::Done(answer) = &pending {
                out.structured = serde_json::to_value(answer).ok();
            } else {
                out.structured = Some(json!({"kind": "model_plan_pending"}));
            }
            return Ok(());
        }
        let context = MixedContext {
            locale: LOCALE,
            gateway: &self.stack.client,
            server_url: &self.stack.url,
            model_alias: &self.stack.model,
            idle_timeout: Duration::from_secs(240),
            history,
            budget: self.budget,
            // The replay does not send workbook cards: it stays the replay of step 24 (a later step
            // that wants them calls `tabular_answer::question_cards`, as `commands::mixed_tier` does).
            cards_text: "",
        };
        let started = Instant::now();
        let (result, generated) = mixed_answer::answer(
            question,
            pending,
            document_sources,
            unavailable,
            &context,
            |_| {},
            |_| {},
        )
        .await?;
        out.generate_ms = started.elapsed().as_secs_f64() * 1000.0;
        out.generated = generated;
        out.answer = result.answer.clone();
        out.unverified_numbers = result.unverified_numbers.clone();
        out.sources = sources_of(&result.document_sources);
        out.structured = serde_json::to_value(&result.table).ok();
        Ok(())
    }
}

fn selected_documents(inventory: &WorkFolderInventory, paths: &[String]) -> Vec<SelectedFile> {
    paths
        .iter()
        .filter_map(|path| inventory.find_by_relative_path(path))
        .map(|record| SelectedFile::new(&record.relative_path, &record.id))
        .collect()
}

fn selected_workbook_files(folder: &DataFolder, mode: &ScopeMode) -> Vec<SelectedFile> {
    tabular_answer::selected_workbook_files(folder, mode)
        .into_iter()
        .map(|(path, id)| SelectedFile::new(&path, &id))
        .collect()
}

// ------------------------------------------------------------------------------------------ the two loops

async fn replay_book(generate: bool, file_name: &str) {
    let library = library_dir();
    let stack = stack();
    let book = load_book(&library);
    let work = tempfile::tempdir().expect("temporary folder");
    let copy = work.path().join("index.sqlite3");
    std::fs::copy(index_dir().join("index.sqlite3"), &copy).expect("copies the analysed index");
    let replay = Replay::open(&stack, &library, &copy).await;

    let out = out_dir();
    let mut answers: HashMap<String, String> = HashMap::new();
    // `ACAI_LT_RESUME=1`: keep the answers already written, drop the ones that failed for want of a server, and go
    // on with the questions that are left (the AI machine can go away in the middle of a long run).
    let mut kept: Vec<String> = Vec::new();
    if env("ACAI_LT_RESUME", "0") == "1" {
        let transient = [
            "embedding_timeout",
            "server_timeout",
            "server_unreachable",
            "provider_error",
        ];
        if let Ok(text) = std::fs::read_to_string(out.join(file_name)) {
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                let row: Value = serde_json::from_str(line).expect("reads a result line");
                let failed = row["error"]
                    .as_str()
                    .is_some_and(|code| transient.contains(&code));
                if !failed {
                    let id = row["id"].as_str().unwrap_or_default().to_string();
                    let said = row["answer"]
                        .as_str()
                        .filter(|a| !a.is_empty())
                        .or(row["structured_text"].as_str());
                    answers.insert(id, said.unwrap_or_default().to_string());
                    kept.push(line.to_string());
                }
            }
        }
    }
    let mut file = std::fs::File::create(out.join(file_name)).expect("creates the result file");
    for line in &kept {
        writeln!(file, "{line}").expect("rewrites a kept result");
    }
    let started = Instant::now();
    for (position, q) in book.iter().enumerate() {
        if answers.contains_key(&q.id) {
            continue;
        }
        let history: Vec<ChatTurn> = match q.follow_up_of.as_ref().and_then(|parent| {
            book.iter()
                .find(|p| &p.id == parent)
                .map(|p| (p, answers.get(parent).cloned().unwrap_or_default()))
        }) {
            Some((parent, answer)) => vec![
                ChatTurn {
                    role: "user".into(),
                    content: parent.text.clone(),
                },
                ChatTurn {
                    role: "assistant".into(),
                    content: if answer.trim().is_empty() {
                        "Voici ce que disent vos fichiers.".into()
                    } else {
                        answer
                    },
                },
            ],
            None => Vec::new(),
        };
        let replayed = replay.ask(q, &history, generate).await;
        let remembered = if replayed.answer.is_empty() {
            replayed.structured_text.clone()
        } else {
            replayed.answer.clone()
        };
        answers.insert(q.id.clone(), remembered);
        writeln!(file, "{}", serde_json::to_string(&replayed).unwrap()).expect("writes a result");
        file.flush().ok();
        if position % 10 == 0 {
            eprintln!(
                "  {}/{} {} {} ({:.0} ms)",
                position + 1,
                book.len(),
                q.id,
                replayed.route,
                replayed.total_ms
            );
        }
    }
    let meta = json!({
        "questions": book.len(), "generate": generate, "model": stack.model, "embed_alias": stack.embed_alias,
        "gateway": stack.url, "temperature": GROUNDED_TEMPERATURE, "seconds": started.elapsed().as_secs_f64(),
        "library": library.display().to_string(),
    });
    std::fs::write(
        out.join(format!("{file_name}.meta.json")),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .expect("writes the run description");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn routes() {
    replay_book(false, "routes.jsonl").await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn run() {
    replay_book(true, "answers.jsonl").await;
}
