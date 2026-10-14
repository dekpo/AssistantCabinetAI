//! KB lot 11: the validation harness shared by the automated quality run, the benchmarks and the real-stack
//! run.
//!
//! It reads the ground truth of the knowledge sandbox (`fixtures/knowledge-sandbox`, written by
//! `scripts/gen-knowledge-sandbox.py`), builds a laboratory of the library at a given size (a *tier*: the first N
//! documents of the stream), and replays what `commands::sourced_answer` does with a question before the model
//! is called: the folder router, the selection, the knowledge step, the reduced set.
//!
//! Nothing is judged by eye. Every figure is a comparison between what the base did and what the generator knows,
//! because it wrote the text. Every name is invented.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use assistant_cabinet_ai_lib::analysis_scope::{AnalysisScope, ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::discovery::is_supported;
use assistant_cabinet_ai_lib::folder_questions::{
    route_in_selection, QuestionRoute, Selection as RouterSelection,
};
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::indexing::{self, IndexSummary};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use assistant_cabinet_ai_lib::knowledge::normalize::{normalize_name, TitleSet};
use assistant_cabinet_ai_lib::knowledge::query::SelectedFile;
use assistant_cabinet_ai_lib::knowledge::reduce::{SideInput, SkipReason, TargetingPolicy};
use assistant_cabinet_ai_lib::knowledge::targeting::{
    self, DocumentPlan, DocumentSearch, QueryVocabulary, TargetingPlan, TargetingRequest,
};
use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;
use assistant_cabinet_ai_lib::ocr::fake::FakeOcrProvider;
use assistant_cabinet_ai_lib::ocr::OcrProvider;
use assistant_cabinet_ai_lib::raster::{FakeRasterizer, PageRasterizer};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::Value;

use super::knowledge::Lab;

pub const TIERS: [usize; 5] = [30, 100, 200, 500, 1000];

// ----------------------------------------------------------------------------------------- the ground truth

#[derive(Debug, Clone, Deserialize)]
pub struct Mention {
    pub entity: String,
    pub text: String,
    pub form: String,
    #[serde(rename = "where")]
    pub place: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DocTruth {
    pub index: usize,
    pub path: String,
    pub format: String,
    pub extension: String,
    pub kind: String,
    pub readable: bool,
    pub ocr: bool,
    pub pages: usize,
    pub tier: usize,
    #[serde(default)]
    pub special: String,
    #[serde(default)]
    pub about: Vec<String>,
    #[serde(default)]
    pub mentions: Vec<Mention>,
    #[serde(default)]
    pub terms: Vec<String>,
    #[serde(default)]
    pub ocr_pages: Vec<String>,
    #[serde(default)]
    pub facts: BTreeMap<String, String>,
}

impl DocTruth {
    /// What the product reads: an extension its extractor knows.
    pub fn product_reads(&self) -> bool {
        is_supported(&self.extension)
    }

    pub fn text_entities(&self) -> BTreeSet<String> {
        self.mentions
            .iter()
            .filter(|m| m.place == "text")
            .map(|m| m.entity.clone())
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntityTruth {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub canonical: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub forms: BTreeMap<String, String>,
    #[serde(default)]
    pub homonym_group: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookMention {
    pub entity: String,
    pub form: String,
    pub sheet: String,
    pub column: String,
    pub rows: Vec<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookTruth {
    pub path: String,
    pub format: String,
    pub readable: bool,
    #[serde(default)]
    pub mentions: Vec<BookMention>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Pick {
    Word(String),
    Paths { paths: Vec<String> },
}

#[derive(Debug, Clone, Deserialize)]
pub struct SelectionTruth {
    pub documents: Pick,
    pub workbooks: Pick,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Expect {
    pub resolution: String,
    #[serde(default)]
    pub between: Vec<String>,
    #[serde(default)]
    pub contains: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuestionTruth {
    pub id: String,
    pub category: String,
    pub text: String,
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub name_entities: Vec<String>,
    #[serde(default)]
    pub relevant_documents: Vec<String>,
    #[serde(default)]
    pub relevant_readable: Vec<String>,
    #[serde(default)]
    pub relevant_workbooks: Vec<String>,
    pub selection: SelectionTruth,
    pub expect: Expect,
    pub min_tier: usize,
    #[serde(default)]
    pub follow_up_of: Option<String>,
    #[serde(default)]
    pub carry_over: Vec<String>,
    #[serde(default)]
    pub left_out: Vec<String>,
    #[serde(default)]
    pub risk: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub expected_value: Option<i64>,
}

pub struct Truth {
    pub root: PathBuf,
    pub docs: Vec<DocTruth>,
    pub entities: BTreeMap<String, EntityTruth>,
    pub books: Vec<BookTruth>,
    pub questions: Vec<QuestionTruth>,
    pub manifest: Value,
}

pub fn sandbox_root() -> PathBuf {
    if let Ok(path) = std::env::var("ACAI_SANDBOX_DIR") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("fixtures")
        .join("knowledge-sandbox")
}

fn read<T: for<'de> Deserialize<'de>>(root: &Path, name: &str) -> T {
    let text = std::fs::read_to_string(root.join(name))
        .unwrap_or_else(|error| panic!("reads {name} of the sandbox: {error}"));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("parses {name}: {error}"))
}

impl Truth {
    pub fn load() -> Self {
        let root = sandbox_root();
        let entities: Vec<EntityTruth> = read(&root, "entities.json");
        Self {
            docs: read(&root, "documents.json"),
            entities: entities.into_iter().map(|e| (e.id.clone(), e)).collect(),
            books: read(&root, "workbooks.json"),
            questions: read(&root, "questions.json"),
            manifest: read(&root, "manifest.json"),
            root,
        }
    }

    pub fn docs_in(&self, tier: usize) -> impl Iterator<Item = &DocTruth> {
        self.docs.iter().filter(move |d| d.index <= tier)
    }

    pub fn doc(&self, path: &str) -> Option<&DocTruth> {
        self.docs.iter().find(|d| d.path == path)
    }

    pub fn tier_size(&self, wanted: usize) -> usize {
        wanted.min(self.docs.len())
    }
}

// ----------------------------------------------------------------------------------------- the laboratory

pub struct Library {
    pub lab: Lab,
    pub tier: usize,
    /// Relative paths of the documents the product read at this tier.
    pub product_files: Vec<String>,
    pub analyse_ms: u128,
    /// The last pass, with its timings (hash, extract, OCR, chunk, embed, write).
    pub summary: Option<IndexSummary>,
    /// Every pass so far: the tier it brought the library to, the wall time, the timings line.
    pub passes: Vec<(usize, u128, String)>,
}

/// What a pass runs on: the gateway that embeds (the lab's fake one by default), and the OCR engine and the
/// page rasteriser (perfect fakes by default, the bundled Tesseract and pdfium for a real run).
pub struct Stack<'a> {
    pub real: Option<Real>,
    pub ocr: Option<&'a dyn OcrProvider>,
    pub rasterizer: Option<&'a dyn PageRasterizer>,
}

/// A real gateway: the address, the embedding alias, and a client with the application's own deadlines.
pub struct Real {
    pub client: GatewayClient,
    pub url: String,
    pub alias: String,
}

impl Stack<'_> {
    pub fn fakes() -> Stack<'static> {
        Stack {
            real: None,
            ocr: None,
            rasterizer: None,
        }
    }
}

/// The OCR fake that answers every scan with the text the page really shows, and a rasteriser that knows each
/// scanned PDF's page count.
pub fn perfect_ocr(truth: &Truth, tier: usize) -> (FakeOcrProvider, FakeRasterizer) {
    let ocr = FakeOcrProvider::new();
    let rasterizer = FakeRasterizer::new();
    for doc in truth.docs_in(tier).filter(|d| d.ocr && d.product_reads()) {
        for (position, page) in doc.ocr_pages.iter().enumerate() {
            ocr.set_response(
                &doc.path,
                (position + 1) as u32,
                Ok(FakeOcrProvider::recognised_page(
                    &doc.path,
                    (position + 1) as u32,
                    page,
                )),
            );
        }
        if let Some(name) = Path::new(&doc.path).file_name().and_then(|n| n.to_str()) {
            rasterizer.set_page_count(name, doc.ocr_pages.len().max(1) as u32);
        }
    }
    (ocr, rasterizer)
}

fn copy_documents(
    truth: &Truth,
    lab: &Lab,
    from_exclusive: usize,
    to_inclusive: usize,
    into: &mut Vec<String>,
) {
    for doc in truth
        .docs
        .iter()
        .filter(|d| d.index > from_exclusive && d.index <= to_inclusive)
    {
        if !doc.product_reads() {
            continue;
        }
        let from = truth.root.join("documents").join(&doc.path);
        let to = lab.work_path().join(&doc.path);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).expect("creates the folder");
        }
        std::fs::copy(&from, &to).unwrap_or_else(|e| panic!("copies {}: {e}", from.display()));
        into.push(doc.path.clone());
    }
}

fn copy_workbooks(truth: &Truth, lab: &Lab) {
    for book in &truth.books {
        let extension = Path::new(&book.path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if book.readable && matches!(extension, "csv" | "xlsx") {
            let from = truth.root.join("data").join(&book.path);
            lab.write_data_bytes(
                &book.path,
                &std::fs::read(from).expect("reads the workbook"),
            );
        }
    }
}

/// A laboratory holding the first `tier` documents the product can read, OCR answered with the text the page
/// really shows (a perfect OCR; the real engine is measured in the real-stack run), and the workbooks it can
/// read. Both folders are analysed with a fresh knowledge context, as the application does.
pub async fn library(truth: &Truth, tier: usize, packs: &[&'static str]) -> Library {
    let (ocr, rasterizer) = perfect_ocr(truth, tier);
    library_on(
        truth,
        tier,
        packs,
        &Stack {
            real: None,
            ocr: Some(&ocr),
            rasterizer: Some(&rasterizer),
        },
    )
    .await
}

pub async fn library_on(
    truth: &Truth,
    tier: usize,
    packs: &[&'static str],
    stack: &Stack<'_>,
) -> Library {
    let mut lab = Lab::new();
    lab.packs = packs.to_vec();
    lab.mode = KnowledgeMode::Suggest;
    let mut product_files = Vec::new();
    copy_documents(truth, &lab, 0, tier, &mut product_files);
    copy_workbooks(truth, &lab);
    let mut library = Library {
        lab,
        tier: 0,
        product_files,
        analyse_ms: 0,
        summary: None,
        passes: Vec::new(),
    };
    library.pass(tier, stack).await;
    library.lab.analyse_data();
    library
}

impl Library {
    /// One Analyse over what the folders hold now, with a fresh knowledge context.
    pub async fn pass(&mut self, tier: usize, stack: &Stack<'_>) {
        let started = std::time::Instant::now();
        let mut context = self.lab.context();
        let summary = match &stack.real {
            Some(real) => indexing::run_with_knowledge(
                &mut self.lab.index,
                &real.client,
                &real.url,
                &real.alias,
                self.lab.work.path(),
                stack.ocr,
                stack.rasterizer,
                self.lab.locale,
                &|_| {},
                Some(&mut context),
            )
            .await
            .expect("the pass completes"),
            None => {
                let url = self.lab.gateway.url.clone();
                self.lab
                    .analyse_through(&mut context, &url, stack.ocr, stack.rasterizer)
                    .await
            }
        };
        self.analyse_ms = started.elapsed().as_millis();
        self.tier = tier;
        let line = summary
            .timings
            .as_ref()
            .map(|t| serde_json::to_string(t).unwrap_or_default())
            .unwrap_or_default();
        self.passes.push((tier, self.analyse_ms, line));
        self.summary = Some(summary);
    }

    /// The library grows: the documents of the next tier are added to the folder and a pass reads only them.
    pub async fn grow_to(&mut self, truth: &Truth, tier: usize, stack: &Stack<'_>) {
        let from = self.tier;
        let mut added = Vec::new();
        copy_documents(truth, &self.lab, from, tier, &mut added);
        self.product_files.extend(added);
        self.pass(tier, stack).await;
        self.lab.analyse_data();
    }
}

// ------------------------------------------------------------------------------------------- reading the base

#[derive(Debug, Clone)]
pub struct KbEntity {
    pub id: i64,
    pub type_id: String,
    pub canonical: String,
    pub normalized: String,
    pub status: String,
    pub keys: BTreeSet<String>,
    pub documents: BTreeSet<String>,
    pub workbooks: BTreeSet<String>,
}

/// Every candidate and active entity with the identity keys of its names and the files that mention it.
pub fn read_base(connection: &Connection, titles: &TitleSet) -> Vec<KbEntity> {
    let mut entities: BTreeMap<i64, KbEntity> = BTreeMap::new();
    let mut statement = connection
        .prepare(
            "SELECT entity_id, type_id, canonical_name, normalized_name, status
             FROM kb_entities WHERE status IN ('candidate','active')",
        )
        .expect("prepares");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .expect("queries");
    for row in rows {
        let (id, type_id, canonical, normalized, status) = row.expect("reads");
        let mut keys = BTreeSet::new();
        keys.insert(normalize_name(&canonical, titles).token_set_key);
        entities.insert(
            id,
            KbEntity {
                id,
                type_id,
                canonical,
                normalized,
                status,
                keys,
                documents: BTreeSet::new(),
                workbooks: BTreeSet::new(),
            },
        );
    }
    let mut statement = connection
        .prepare("SELECT entity_id, display, kind FROM kb_aliases")
        .expect("prepares");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .expect("queries");
    for row in rows {
        let (id, display, kind) = row.expect("reads");
        // Initials and the surname alone are not the identity of a person.
        if matches!(kind.as_str(), "initial" | "partial_surname") {
            continue;
        }
        if let Some(entity) = entities.get_mut(&id) {
            entity
                .keys
                .insert(normalize_name(&display, titles).token_set_key);
        }
    }
    let mut statement = connection
        .prepare(
            "SELECT m.entity_id, s.domain, s.relative_path FROM kb_mentions m
             JOIN kb_sources s ON s.source_id = m.source_id",
        )
        .expect("prepares");
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .expect("queries");
    for row in rows {
        let (id, domain, path) = row.expect("reads");
        if let Some(entity) = entities.get_mut(&id) {
            if domain == "data" {
                entity.workbooks.insert(path);
            } else {
                entity.documents.insert(path);
            }
        }
    }
    entities.into_values().collect()
}

/// The key a truth person is known by: the words that carry identity, sorted.
pub fn truth_key(entity: &EntityTruth, titles: &TitleSet) -> String {
    normalize_name(&entity.canonical, titles).token_set_key
}

// ------------------------------------------------------------------------------------------ replaying a question

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    Folder,
    NamedFile,
    EveryDocument,
    Global,
    NothingSelected,
}

impl RouteKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::NamedFile => "named_file",
            Self::EveryDocument => "every_document",
            Self::Global => "global",
            Self::NothingSelected => "nothing_selected",
        }
    }
}

pub struct Asked {
    pub route: RouteKind,
    /// The documents the question's selection covers (what retrieval is held to).
    pub selected: Vec<String>,
    pub workbooks_selected: Vec<String>,
    /// The knowledge base's decision. `None` when the router answered without the documents.
    pub plan: Option<TargetingPlan>,
    /// The files the plan covers (one named file, every document, or the whole selection).
    pub plan_files: Vec<String>,
    /// Every analysed file of the folder, selected or not.
    pub present: Vec<String>,
    pub narrowed: bool,
}

impl Asked {
    /// The files the search would be held to on the documents side: the reduced set when one applies, the whole
    /// selection otherwise.
    pub fn searched(&self) -> Vec<String> {
        match &self.plan {
            Some(plan) => match plan.applied_documents() {
                Some(candidates) => candidates.to_vec(),
                None => self.selected.clone(),
            },
            None => self.selected.clone(),
        }
    }

    pub fn applied(&self) -> bool {
        self.plan
            .as_ref()
            .is_some_and(|plan| plan.reduction.documents.is_applied())
    }

    pub fn skip_code(&self) -> String {
        match &self.plan {
            Some(plan) => plan
                .reduction
                .documents
                .skipped
                .map(SkipReason::as_code)
                .unwrap_or("-")
                .to_string(),
            None => "router".to_string(),
        }
    }
}

pub struct Replay<'a> {
    pub library: &'a Library,
    pub vocabulary: &'a QueryVocabulary,
    pub policy: TargetingPolicy,
}

impl<'a> Replay<'a> {
    pub fn new(library: &'a Library, vocabulary: &'a QueryVocabulary) -> Self {
        Self {
            library,
            vocabulary,
            policy: TargetingPolicy::default(),
        }
    }

    /// One question, the way `sourced_answer` treats the document side: inventory, selection, router, then the
    /// knowledge step. `selection_paths` is `None` for "tous".
    pub fn ask(
        &self,
        text: &str,
        mode: KnowledgeMode,
        request: &TargetingRequest,
        selection_paths: Option<&[String]>,
        with_workbooks: bool,
    ) -> Asked {
        let lab = &self.library.lab;
        let full = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index))
            .expect("discovers the folder");
        let scope = AnalysisScope {
            mode: match selection_paths {
                None => ScopeMode::WholeFolder,
                Some(paths) => ScopeMode::Explicit(
                    paths
                        .iter()
                        .filter_map(|path| full.find_by_relative_path(path))
                        .map(|record| ScopeEntry {
                            relative_path: record.relative_path.clone(),
                            pinned_id: record.id.clone(),
                            added_at: 0,
                            sheet_names: Vec::new(),
                        })
                        .collect(),
                ),
            },
            data_mode: ScopeMode::nothing(),
            created_at: 0,
            updated_at: 0,
        };
        let resolution = scope.resolve(&full);
        let narrowed = resolution.narrowed;
        let no_documents_chosen = resolution.no_documents_chosen;
        let inventory = resolution.inventory;
        let scoped: Vec<String> = inventory
            .indexed_files()
            .into_iter()
            .map(|f| f.relative_path.clone())
            .collect();
        let selection = RouterSelection {
            full: &full,
            scoped: &inventory,
            narrowed,
            no_documents_chosen,
        };
        let route = route_in_selection(&selection, &lab.index, text, "fr-FR");
        let present: Vec<String> = full
            .indexed_files()
            .into_iter()
            .map(|f| f.relative_path.clone())
            .collect();
        let workbooks: Vec<SelectedFile> = if with_workbooks {
            lab.workbooks()
        } else {
            Vec::new()
        };
        let workbook_paths: Vec<String> =
            workbooks.iter().map(|f| f.relative_path.clone()).collect();
        let (kind, plan_files, blocked) = match route {
            // The finder answers a set sentence from the index with no retrieval, as the inventory answers
            // a folder question: for the replay both are "answered without the documents".
            QuestionRoute::Deterministic(_) | QuestionRoute::SetSearch(_) => {
                return Asked {
                    route: if no_documents_chosen {
                        RouteKind::NothingSelected
                    } else {
                        RouteKind::Folder
                    },
                    selected: scoped,
                    workbooks_selected: workbook_paths,
                    plan: None,
                    plan_files: Vec::new(),
                    present,
                    narrowed,
                };
            }
            QuestionRoute::TargetedRetrieval { file } => (
                RouteKind::NamedFile,
                vec![file.relative_path.clone()],
                Some(SkipReason::NamedFile),
            ),
            QuestionRoute::PerDocumentRetrieval { files } => (
                RouteKind::EveryDocument,
                files
                    .into_iter()
                    .map(|f| f.relative_path)
                    .collect::<Vec<_>>(),
                None,
            ),
            QuestionRoute::GlobalRetrieval => (RouteKind::Global, scoped.clone(), None),
        };
        let selected_files: Vec<SelectedFile> = plan_files
            .iter()
            .filter_map(|path| inventory.find_by_relative_path(path))
            .map(|record| SelectedFile::new(&record.relative_path, &record.id))
            .collect();
        let side = match blocked {
            Some(reason) => SideInput::new(&selected_files).blocked_by(reason),
            None => SideInput::new(&selected_files),
        };
        let plan = lab.plan(
            self.vocabulary,
            mode,
            request,
            text,
            side,
            SideInput::new(&workbooks),
            self.policy,
        );
        Asked {
            route: kind,
            selected: scoped,
            workbooks_selected: workbook_paths,
            plan: Some(plan),
            plan_files,
            present,
            narrowed,
        }
    }

    /// The document search the question would run once it is embedded, exactly as the application chooses it.
    pub fn search(&self, asked: &Asked, text: &str, embedding: &[f32]) -> Option<DocumentSearch> {
        let plan = asked.plan.as_ref()?;
        let kind = match asked.route {
            RouteKind::NamedFile => DocumentPlan::OneFile(asked.plan_files.first()?),
            RouteKind::EveryDocument => DocumentPlan::EveryDocument(&asked.plan_files),
            RouteKind::Global => DocumentPlan::WholeFolder {
                narrowed: asked.narrowed,
                scoped_paths: &asked.selected,
                present_paths: &asked.present,
            },
            _ => return None,
        };
        targeting::search_documents(&self.library.lab.index, plan, kind, text, embedding).ok()
    }
}

// ----------------------------------------------------------------------------------------------- formatting

pub fn pct(part: usize, whole: usize) -> String {
    if whole == 0 {
        "-".to_string()
    } else {
        format!("{:.1} %", 100.0 * part as f64 / whole as f64)
    }
}

pub fn ratio(part: usize, whole: usize) -> Option<f64> {
    (whole > 0).then(|| part as f64 / whole as f64)
}

pub fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(values[values.len() / 2])
}

pub fn percentile(mut values: Vec<f64>, p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let index = ((values.len() as f64 - 1.0) * p).round() as usize;
    Some(values[index])
}

/// Where a run writes its reports when asked to: `ACAI_VALIDATION_OUT`, else nowhere.
pub fn out_dir() -> Option<PathBuf> {
    std::env::var("ACAI_VALIDATION_OUT").ok().map(|p| {
        let path = PathBuf::from(p);
        std::fs::create_dir_all(&path).expect("creates the report folder");
        path
    })
}

pub fn write_report(name: &str, text: &str) {
    if let Some(dir) = out_dir() {
        std::fs::write(dir.join(name), text).expect("writes the report");
    }
}
