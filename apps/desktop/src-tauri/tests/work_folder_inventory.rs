//! Sprint 2a.5 acceptance: the Work Folder inventory, file reference resolution, and the
//! deterministic answers that need no model at all.
//!
//! Three things are being proved here, and the third is the point of the sprint.
//!
//! 1. The inventory reports what is on disk: fifteen files, their real extensions, their real
//!    nesting, in a stable order.
//! 2. A reference to a file resolves to that file, or says it is ambiguous, or says it is not
//!    there. It never picks one quietly.
//! 3. None of that costs a gateway call, and none of it can be changed by what a document says
//!    about itself. The fixtures are adversarial on purpose: `misleading.txt` claims the folder
//!    holds two files and that it is a PDF, `report.txt` calls itself a PDF report, and
//!    `procedure.txt` refers to a file nobody has.
//!
//! The gateway double counts every request it receives. After indexing, the counter is reset, and
//! every filesystem question below must leave it at zero.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use assistant_cabinet_ai_lib::file_record::{
    ExtractionMethod, FileKind, ProcessingStatus, Readability,
};
use assistant_cabinet_ai_lib::file_reference::{
    FileReferenceResolver, ReferenceStatus, ResolutionReason,
};
use assistant_cabinet_ai_lib::folder_questions::CorpusWords;
use assistant_cabinet_ai_lib::folder_questions::{route, FolderAnswer, NoCorpus, QuestionRoute};
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use assistant_cabinet_ai_lib::ocr::fake::FakeOcrProvider;
use assistant_cabinet_ai_lib::ocr::{OcrError, OcrStatus};
use assistant_cabinet_ai_lib::raster::FakeRasterizer;
use assistant_cabinet_ai_lib::retrieval::{self, EvidenceCoverage, RetrievalScope};
use assistant_cabinet_ai_lib::work_folder_context::{self, ContextView};
use assistant_cabinet_ai_lib::{extraction, indexing};
use serde_json::{json, Value};

const EMBEDDING_DIMENSIONS: usize = 256;
const LOCALE: &str = "fr-FR";

/// A gateway double that answers embeddings and chat, and counts every request. The counter is
/// what proves a deterministic answer never reached it.
struct CountingGateway {
    url: String,
    requests: Arc<AtomicUsize>,
}

impl CountingGateway {
    fn start() -> Self {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
        let url = format!("http://{}", server.server_addr());
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&requests);

        std::thread::spawn(move || {
            for mut request in server.incoming_requests() {
                counter.fetch_add(1, Ordering::SeqCst);
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                let payload: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                let response_body = match request.url() {
                    "/v1/embeddings" => embeddings_response(&payload),
                    "/v1/chat/completions" => chat_response(),
                    _ => json!({}).to_string(),
                };
                let _ = request
                    .respond(tiny_http::Response::from_string(response_body).with_status_code(200));
            }
        });

        Self { url, requests }
    }

    fn reset(&self) {
        self.requests.store(0, Ordering::SeqCst);
    }

    fn count(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

fn embeddings_response(payload: &Value) -> String {
    let inputs: Vec<String> = match &payload["input"] {
        Value::String(text) => vec![text.clone()],
        Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    };
    let data: Vec<Value> = inputs
        .iter()
        .enumerate()
        .map(|(index, text)| json!({ "index": index, "embedding": bag_of_words(text) }))
        .collect();
    json!({ "data": data, "model": "cabinet-embed", "usage": {} }).to_string()
}

fn chat_response() -> String {
    "data: {\"choices\":[{\"delta\":{\"content\":\"Reponse.\"}}]}\n\ndata: [DONE]\n\n".to_string()
}

fn bag_of_words(text: &str) -> Vec<f32> {
    let mut buckets = vec![0.0_f32; EMBEDDING_DIMENSIONS];
    for word in text.split(|c: char| !c.is_alphanumeric()) {
        if word.is_empty() {
            continue;
        }
        let mut hasher = DefaultHasher::new();
        word.to_lowercase().hash(&mut hasher);
        buckets[(hasher.finish() as usize) % EMBEDDING_DIMENSIONS] += 1.0;
    }
    let norm: f32 = buckets.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut buckets {
            *value /= norm;
        }
    }
    buckets
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("fixtures")
        .join("inventory-sandbox")
        .join(name)
}

fn gp_sandbox() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("fixtures")
        .join("gp-sandbox")
}

/// One indexed Work Folder, and the inventory built over it.
struct Indexed {
    _dir: tempfile::TempDir,
    index: IndexStore,
    gateway: CountingGateway,
}

/// Index a fixture folder. No rasteriser, and an OCR engine that finds nothing: the images and
/// the text-layer-free PDFs in `flat/` therefore fail for the reason a real unreadable file fails,
/// which is what makes the "five unreadable" count mean something.
async fn index_fixture(folder: &str) -> Indexed {
    let gateway_double = CountingGateway::start();
    let gateway = GatewayClient::new().expect("builds a client");
    let dir = tempfile::tempdir().expect("temp index dir");
    let mut index = IndexStore::open_at(&dir.path().join("index.sqlite3")).expect("opens");

    let ocr = FakeOcrProvider::new();
    ocr.set_fallback_status(OcrStatus::NoTextFound);

    indexing::run(
        &mut index,
        &gateway,
        &gateway_double.url,
        "cabinet-embed",
        &fixture(folder),
        Some(&ocr),
        None,
        LOCALE,
        &|_| {},
    )
    .await
    .expect("indexes the fixture");

    gateway_double.reset();
    Indexed {
        _dir: dir,
        index,
        gateway: gateway_double,
    }
}

fn inventory_of(folder: &str, indexed: &Indexed) -> WorkFolderInventory {
    WorkFolderInventory::discover(&fixture(folder), Some(&indexed.index)).expect("an inventory")
}

/// `locale` picks the pattern pack the question is written in: the vocabulary is per language,
/// and the product ships one pack per catalogue (`docs/WORK-FOLDER-INVENTORY.md`).
fn deterministic(inventory: &WorkFolderInventory, question: &str, locale: &str) -> FolderAnswer {
    match route(inventory, &NoCorpus, question, locale) {
        QuestionRoute::Deterministic(answer) => answer,
        other => panic!("expected a deterministic answer for {question:?}, got {other:?}"),
    }
}

fn names(files: &[assistant_cabinet_ai_lib::file_record::FileRecord]) -> Vec<String> {
    files.iter().map(|file| file.name.clone()).collect()
}

// ---------------------------------------------------------------------------------------------
// A - the inventory
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn a1_to_a6_the_flat_folder_is_reported_exactly_as_it_is() {
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    // A1 - every physical file, not only the ones that could be read.
    assert_eq!(inventory.file_count(), 15);

    // A2 - extension counts, from the filesystem.
    assert_eq!(inventory.files_with_extension("txt").len(), 6);
    assert_eq!(inventory.files_with_extension("pdf").len(), 5);
    assert_eq!(inventory.files_with_extension("docx").len(), 1);
    assert_eq!(inventory.files_with_extension("png").len(), 2);
    assert_eq!(inventory.files_with_extension("jpg").len(), 1);

    // A3 and A4 - ten readable and indexed, five that genuinely could not be read.
    assert_eq!(inventory.indexed_files().len(), 10);
    assert_eq!(inventory.unreadable_files().len(), 5);
    assert_eq!(inventory.summary().total_files, 15);
    assert_eq!(inventory.summary().indexed_files, 10);
    assert_eq!(inventory.summary().unreadable_files, 5);

    // A5 - every expected name is present, exactly as the filesystem spells it.
    let mut present = names(inventory.all_files());
    present.sort();
    assert_eq!(
        present,
        vec![
            "assurance.txt",
            "biologie.pdf",
            "convocation.txt",
            "courrier-cardiologie.pdf",
            "courrier-endocrinologie.docx",
            "echographie-scan.pdf",
            "horaires.txt",
            "illisible.png",
            "misleading.txt",
            "neurologie.pdf",
            "ordonnance-illisible.jpg",
            "patient-report.png",
            "procedure.txt",
            "radiographie-scan.pdf",
            "report.txt",
        ]
    );

    // A6 - every record's extension is the one on disk, never one inferred from content.
    for file in inventory.all_files() {
        let on_disk = std::path::Path::new(&file.name)
            .extension()
            .map(|value| value.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        assert_eq!(file.extension, on_disk, "{}", file.relative_path);
        assert!(file.name.ends_with(&format!(".{}", file.extension)));
    }

    // The five unreadable ones are the two PDFs with no text layer and the three images, and
    // none of them is presented as having been read by any method.
    let mut unreadable = names(
        &inventory
            .unreadable_files()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>(),
    );
    unreadable.sort();
    assert_eq!(
        unreadable,
        vec![
            "echographie-scan.pdf",
            "illisible.png",
            "ordonnance-illisible.jpg",
            "patient-report.png",
            "radiographie-scan.pdf",
        ]
    );
    for file in inventory.unreadable_files() {
        assert_eq!(file.extraction_method, ExtractionMethod::None);
        assert_eq!(file.processing_status, ProcessingStatus::Failed);
        assert!(!file.indexed);
    }
}

#[tokio::test]
async fn a7_the_nested_folder_keeps_every_relative_path() {
    let indexed = index_fixture("nested").await;
    let inventory = inventory_of("nested", &indexed);

    let paths: Vec<&str> = inventory
        .all_files()
        .iter()
        .map(|file| file.relative_path.as_str())
        .collect();

    assert_eq!(
        paths,
        vec![
            "2026/janvier/neurologie.pdf",
            "2026/mars/biologie.pdf",
            "2026/mars/neurologie.pdf",
            "administratif/assurance.txt",
        ]
    );
    assert_eq!(
        inventory.folders(),
        vec!["2026", "2026/janvier", "2026/mars", "administratif"]
    );
}

#[tokio::test]
async fn a8_two_discoveries_of_the_same_folder_are_identical() {
    let indexed = index_fixture("flat").await;

    let first = inventory_of("flat", &indexed);
    let second = inventory_of("flat", &indexed);

    assert_eq!(first.all_files(), second.all_files());
    assert_eq!(first.summary(), second.summary());
}

// ---------------------------------------------------------------------------------------------
// B - file reference resolution
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn b1_to_b7_references_resolve_or_say_why_they_could_not() {
    let indexed = index_fixture("nested").await;
    let inventory = inventory_of("nested", &indexed);
    let resolver = FileReferenceResolver::new(&inventory);

    // B1 - an exact relative path.
    let exact_path = resolver.resolve("2026/mars/neurologie.pdf");
    assert_eq!(exact_path.status, ReferenceStatus::Exact);
    assert_eq!(exact_path.reason, ResolutionReason::RelativePath);

    // B2 - a file name that only one file carries.
    let by_name = resolver.resolve("biologie.pdf");
    assert_eq!(by_name.status, ReferenceStatus::Exact);
    assert_eq!(
        by_name.exact_match.as_ref().unwrap().relative_path,
        "2026/mars/biologie.pdf"
    );

    // B3 - the same name with the extension left off.
    assert_eq!(resolver.resolve("biologie").status, ReferenceStatus::Exact);

    // B4 - a name two files carry. Both are returned; neither is chosen.
    let ambiguous = resolver.resolve("neurologie.pdf");
    assert_eq!(ambiguous.status, ReferenceStatus::MultipleMatches);
    assert!(ambiguous.exact_match.is_none());
    assert_eq!(
        ambiguous
            .candidates
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect::<Vec<_>>(),
        vec!["2026/janvier/neurologie.pdf", "2026/mars/neurologie.pdf"]
    );

    // B5 - a file that is not there.
    let missing = resolver.resolve("nonexistent.pdf");
    assert_eq!(missing.status, ReferenceStatus::NoMatch);
    assert_eq!(missing.reason, ResolutionReason::NotFound);

    // B6 - an attempt to leave the Work Folder. Refused on the string, so no path outside the
    // folder is ever built, opened or stat-ed.
    for attempt in ["../../secret.pdf", "2026/../../secret.pdf", "/etc/passwd"] {
        let escaped = resolver.resolve(attempt);
        assert_eq!(escaped.status, ReferenceStatus::NoMatch, "{attempt}");
        assert_eq!(
            escaped.reason,
            ResolutionReason::OutsideWorkFolder,
            "{attempt}"
        );
        assert!(escaped.candidates.is_empty(), "{attempt}");
    }

    // B7 - what comes back is a record, not a name.
    let resolved = exact_path.exact_match.expect("a record");
    assert!(!resolved.id.is_empty());
    assert_eq!(resolved.id, resolved.sha256.clone().unwrap());
    assert_eq!(resolved.kind, FileKind::DocumentPdf);
    assert_eq!(resolved.extraction_method, ExtractionMethod::NativeText);
    assert!(resolved.indexed);
    assert!(resolved.index_metadata.is_some());
}

// ---------------------------------------------------------------------------------------------
// C - deterministic questions, with the gateway double asserting it was never called
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn c1_to_c7_filesystem_questions_are_answered_without_the_gateway() {
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    // C1 - all fifteen, not the six a retrieval pass might have surfaced.
    let FolderAnswer::FileList { files } = deterministic(&inventory, "List all files.", "en-US")
    else {
        panic!("expected a file listing");
    };
    assert_eq!(files.len(), 15);

    // C2
    assert_eq!(
        deterministic(
            &inventory,
            "How many files are in the work folder?",
            "en-US"
        ),
        FolderAnswer::FileCount { total: 15 }
    );

    // C3 and C4
    assert_eq!(
        deterministic(&inventory, "How many PDF files are there?", "en-US"),
        FolderAnswer::ExtensionCount {
            extension: "pdf".into(),
            count: 5
        }
    );
    assert_eq!(
        deterministic(&inventory, "How many TXT files are there?", "en-US"),
        FolderAnswer::ExtensionCount {
            extension: "txt".into(),
            count: 6
        }
    );

    // C5
    let FolderAnswer::ExtensionList { files, .. } =
        deterministic(&inventory, "List the PNG files.", "en-US")
    else {
        panic!("expected a PNG listing");
    };
    let mut png = names(&files);
    png.sort();
    assert_eq!(png, vec!["illisible.png", "patient-report.png"]);

    // C6
    assert_eq!(
        deterministic(&inventory, "How many files are unreadable?", "en-US"),
        FolderAnswer::UnreadableCount { count: 5 }
    );

    // C7
    let FolderAnswer::IndexedList { files } =
        deterministic(&inventory, "Which files are indexed?", "en-US")
    else {
        panic!("expected an indexed listing");
    };
    assert_eq!(files.len(), 10);
    assert!(files.iter().all(|file| file.indexed));

    // The same questions in the pilot's own language take the same path.
    assert_eq!(
        deterministic(&inventory, "Combien de fichiers au total ?", LOCALE),
        FolderAnswer::FileCount { total: 15 }
    );
    assert_eq!(
        deterministic(&inventory, "Combien de fichiers PDF ?", LOCALE),
        FolderAnswer::ExtensionCount {
            extension: "pdf".into(),
            count: 5
        }
    );

    assert_eq!(
        indexed.gateway.count(),
        0,
        "a filesystem question must not reach the gateway"
    );
}

#[tokio::test]
async fn c8_the_folder_structure_matches_the_fixture_exactly() {
    let indexed = index_fixture("nested").await;
    let inventory = inventory_of("nested", &indexed);

    let FolderAnswer::FolderTree { root, lines } =
        deterministic(&inventory, "Show the folder structure.", "en-US")
    else {
        panic!("expected a tree");
    };

    assert_eq!(root, "nested");
    assert_eq!(
        lines,
        vec![
            "2026/",
            "  janvier/",
            "    neurologie.pdf",
            "  mars/",
            "    biologie.pdf",
            "    neurologie.pdf",
            "administratif/",
            "  assurance.txt",
        ]
    );
    assert_eq!(indexed.gateway.count(), 0);
}

// ---------------------------------------------------------------------------------------------
// D - anti-hallucination
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn d1_and_d4_what_a_document_says_about_itself_changes_nothing() {
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    // D1 - the file claims the folder holds two files and that it is a PDF.
    let misleading = inventory
        .find_by_relative_path("misleading.txt")
        .expect("the misleading fixture");
    assert_eq!(misleading.name, "misleading.txt");
    assert_eq!(misleading.extension, "txt");
    assert_eq!(misleading.kind, FileKind::DocumentText);
    assert_eq!(inventory.file_count(), 15);
    // Its content really does say otherwise; the inventory simply does not read it.
    let claim = std::fs::read_to_string(fixture("flat").join("misleading.txt")).unwrap();
    assert!(claim.contains("only 2 files"));
    assert!(claim.contains(".pdf"));

    // D4 - a TXT file that calls itself a PDF report.
    let report = inventory
        .find_by_relative_path("report.txt")
        .expect("the report fixture");
    assert_eq!(report.extension, "txt");
    assert!(std::fs::read_to_string(fixture("flat").join("report.txt"))
        .unwrap()
        .contains("This is a PDF report."));

    assert_eq!(indexed.gateway.count(), 0);
}

#[tokio::test]
async fn d2_and_d3_a_listing_comes_from_the_filesystem_and_not_from_retrieval() {
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    // D2 - `procedure.txt` names a file that does not exist, and its text is genuinely in the
    // index, so a listing assembled from retrieved passages could pick it up.
    let indexed_text: Vec<String> = indexed
        .index
        .all_chunks()
        .unwrap()
        .into_iter()
        .map(|chunk| chunk.text)
        .collect();
    assert!(
        indexed_text
            .iter()
            .any(|text| text.contains("fake-document.pdf")),
        "the misleading reference must really be retrievable"
    );

    let FolderAnswer::FileList { files } =
        deterministic(&inventory, "List the files in the work folder.", "en-US")
    else {
        panic!("expected a file listing");
    };

    assert_eq!(files.len(), 15);
    assert!(
        !names(&files).iter().any(|name| name == "fake-document.pdf"),
        "a file named only inside a document must not appear in the inventory"
    );

    // D3 - retrieval, asked the same thing, returns a handful of passages from a handful of
    // files. The listing above is fifteen regardless, which is the whole point.
    let query = bag_of_words("fichiers du cabinet");
    let retrieved = retrieval::search(&indexed.index, "fichiers du cabinet", &query).unwrap();
    let distinct: std::collections::BTreeSet<&str> = retrieved
        .iter()
        .map(|item| item.relative_path.as_str())
        .collect();
    assert!(
        distinct.len() < files.len(),
        "retrieval saw {} files, the inventory holds {}",
        distinct.len(),
        files.len()
    );

    assert_eq!(indexed.gateway.count(), 0);
}

#[tokio::test]
async fn d5_an_unreadable_file_is_reported_as_such_and_never_described() {
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    let answer = deterministic(&inventory, "What does patient-report.png say?", "en-US");
    let FolderAnswer::FileUnreadable { file } = answer else {
        panic!("expected an unreadable answer, got {answer:?}");
    };

    assert_eq!(file.name, "patient-report.png");
    assert_eq!(file.readability, Readability::Unreadable);
    assert_eq!(file.extraction_method, ExtractionMethod::None);
    assert!(!file.indexed);
    // Nothing was stored for it, so there is nothing a citation could point at.
    assert!(indexed
        .index
        .chunks_for_document("patient-report.png")
        .unwrap()
        .is_empty());
    assert_eq!(indexed.gateway.count(), 0);
}

#[tokio::test]
async fn d6_a_file_that_does_not_exist_is_not_answered_from_the_rest_of_the_folder() {
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    assert_eq!(
        deterministic(&inventory, "What does secret-report.pdf say?", "en-US"),
        FolderAnswer::NoMatchingFile {
            query: "secret-report.pdf".into()
        }
    );
    assert_eq!(indexed.gateway.count(), 0);
}

#[tokio::test]
async fn d7_an_ambiguous_name_asks_which_one_and_retrieves_nothing() {
    let indexed = index_fixture("nested").await;
    let inventory = inventory_of("nested", &indexed);

    let answer = deterministic(&inventory, "What does neurologie.pdf say?", "en-US");
    let FolderAnswer::AmbiguousReference { query, candidates } = answer else {
        panic!("expected an ambiguity, got {answer:?}");
    };

    assert_eq!(query, "neurologie.pdf");
    assert_eq!(
        candidates
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect::<Vec<_>>(),
        vec!["2026/janvier/neurologie.pdf", "2026/mars/neurologie.pdf"]
    );
    assert_eq!(indexed.gateway.count(), 0);
}

#[tokio::test]
async fn d8_every_path_the_model_is_given_exists_in_the_inventory() {
    let indexed = index_fixture("nested").await;
    let inventory = inventory_of("nested", &indexed);

    for view in [
        ContextView::Summary,
        ContextView::Tree,
        ContextView::FileList,
        ContextView::Targeted {
            relative_path: "2026/mars/biologie.pdf".into(),
        },
    ] {
        let block = work_folder_context::build(&inventory, &view).to_json();
        for file in block["files"].as_array().unwrap_or(&Vec::new()) {
            let path = file["path"].as_str().expect("a path");
            assert!(inventory.contains(path), "invented path: {path}");
        }
    }

    // The contract the model is held to travels with every block that needs one.
    let turn = work_folder_context::build_system_turn(
        retrieval::RETRIEVAL_INSTRUCTION,
        &work_folder_context::build(&inventory, &ContextView::Summary),
    );
    assert!(turn.contains("WORK FOLDER KNOWLEDGE CONTRACT"));
    assert!(turn.contains("Never invent a file"));
}

// ---------------------------------------------------------------------------------------------
// E - document retrieval
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn e1_and_e3_an_explicit_file_narrows_retrieval_and_nothing_else_changes() {
    let indexed = index_fixture("nested").await;
    let inventory = inventory_of("nested", &indexed);

    // E1 - the question names one file, so retrieval happens inside it.
    let QuestionRoute::TargetedRetrieval { file } = route(
        &inventory,
        &NoCorpus,
        "Que dit 2026/mars/neurologie.pdf ?",
        LOCALE,
    ) else {
        panic!("expected targeted retrieval");
    };
    assert_eq!(file.relative_path, "2026/mars/neurologie.pdf");

    let query = bag_of_words("consultation neurologie mars");
    let scoped = retrieval::search_scoped(
        &indexed.index,
        "consultation neurologie mars",
        &query,
        RetrievalScope::File(&file.relative_path),
    )
    .unwrap();
    assert!(
        !scoped.is_empty(),
        "the named file must still carry evidence"
    );
    assert!(scoped
        .iter()
        .all(|item| item.relative_path == "2026/mars/neurologie.pdf"));
    // The citation contract is untouched: file, page and passage.
    assert!(scoped[0].page_number >= 1);
    assert!(!scoped[0].chunk_id.is_empty());

    // The same question over the whole folder reaches the January letter too, which is exactly
    // the contamination the scope exists to prevent.
    let unscoped =
        retrieval::search(&indexed.index, "consultation neurologie mars", &query).unwrap();
    assert!(unscoped
        .iter()
        .any(|item| item.relative_path == "2026/janvier/neurologie.pdf"));

    // E3 - a content question naming no file keeps the existing behaviour.
    assert_eq!(
        route(
            &inventory,
            &NoCorpus,
            "Quel patient a eu une consultation ?",
            LOCALE
        ),
        QuestionRoute::GlobalRetrieval
    );
}

#[tokio::test]
async fn e2_an_ambiguous_reference_retrieves_nothing_until_it_is_settled() {
    let indexed = index_fixture("nested").await;
    let inventory = inventory_of("nested", &indexed);

    assert!(matches!(
        route(&inventory, &NoCorpus, "Que dit neurologie.pdf ?", LOCALE),
        QuestionRoute::Deterministic(FolderAnswer::AmbiguousReference { .. })
    ));
    assert_eq!(indexed.gateway.count(), 0);

    // Naming the folder settles it, and retrieval then runs inside that one file.
    let QuestionRoute::TargetedRetrieval { file } = route(
        &inventory,
        &NoCorpus,
        "Que dit 2026/janvier/neurologie.pdf ?",
        LOCALE,
    ) else {
        panic!("expected targeted retrieval once the reference is unambiguous");
    };
    assert_eq!(file.relative_path, "2026/janvier/neurologie.pdf");
}

#[tokio::test]
async fn e4_an_ocr_read_file_resolves_and_keeps_its_recognised_provenance() {
    let gateway_double = CountingGateway::start();
    let gateway = GatewayClient::new().expect("builds a client");
    let dir = tempfile::tempdir().expect("temp index dir");
    let mut index = IndexStore::open_at(&dir.path().join("index.sqlite3")).expect("opens");

    let ocr = FakeOcrProvider::new();
    ocr.set_fallback_status(OcrStatus::NoTextFound);
    ocr.set_response(
        "inbox/2026-03-22_courrier-rhumatologie-scan.pdf",
        1,
        Ok(FakeOcrProvider::recognised_page(
            "inbox/2026-03-22_courrier-rhumatologie-scan.pdf",
            1,
            "Cabinet de rhumatologie. Douleurs articulaires bilaterales des mains. \
             Bilan: CRP, facteur rhumatoide, anticorps anti-CCP.",
        )),
    );
    ocr.set_response(
        "inbox/2026-03-28_illisible.png",
        1,
        Err(OcrError::UnreadableImage),
    );
    let rasterizer = FakeRasterizer::with_page_count(1);
    rasterizer.set_page_count("2026-03-24_compte-rendu-mixte.pdf", 2);

    indexing::run(
        &mut index,
        &gateway,
        &gateway_double.url,
        "cabinet-embed",
        &gp_sandbox(),
        Some(&ocr),
        Some(&rasterizer),
        LOCALE,
        &|_| {},
    )
    .await
    .expect("indexes the gp sandbox");

    let inventory = WorkFolderInventory::discover(&gp_sandbox(), Some(&index)).expect("inventory");
    let resolver = FileReferenceResolver::new(&inventory);

    let resolution = resolver.resolve("2026-03-22_courrier-rhumatologie-scan.pdf");
    assert_eq!(resolution.status, ReferenceStatus::Exact);
    let scan = resolution.exact_match.expect("a record");

    // What a machine read stays distinguishable from what a file carries, all the way into the
    // record (`docs/ARCHITECTURE.md`, the Source model).
    assert_eq!(scan.extraction_method, ExtractionMethod::RecognisedOcr);
    assert_eq!(
        scan.index_metadata
            .as_ref()
            .and_then(|meta| meta.ocr_engine.as_deref()),
        Some("fake")
    );
    assert_eq!(scan.readability, Readability::Readable);

    // A born-digital file indexed in the same pass is not marked as recognised.
    let native = inventory
        .find_by_name("2026-03-12_compte-rendu-biologie.pdf")
        .pop()
        .expect("the native lab report")
        .clone();
    assert_eq!(native.extraction_method, ExtractionMethod::NativeText);

    // And the OCR-derived evidence is still retrievable, inside that file.
    let query = bag_of_words("anticorps anti-CCP rhumatoide");
    let scoped = retrieval::search_scoped(
        &index,
        "anticorps anti-CCP rhumatoide",
        &query,
        RetrievalScope::File(&scan.relative_path),
    )
    .unwrap();
    assert!(!scoped.is_empty());
    assert!(scoped
        .iter()
        .all(|item| item.origin == extraction::PageOrigin::Ocr));
}

// ---------------------------------------------------------------------------------------------
// F - a question about every document
// ---------------------------------------------------------------------------------------------

/// The case reported from the pilot workstation on 23 September: a folder of 15 files, 10 of them
/// indexed, and "give me a summary of each document" answered from six of them.
///
/// Relevance ranking stops at `MAX_EVIDENCE_CHUNKS`, so four indexed documents were never sent to
/// the model at all - it was not hiding them, it never had them. A question that says "each
/// document" is a question about the corpus, and the corpus is the inventory's to enumerate.
#[tokio::test]
async fn f1_a_summary_of_each_document_reaches_every_indexed_file() {
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    assert_eq!(inventory.indexed_files().len(), 10);

    let question = "Give me a summary of each document";
    let QuestionRoute::PerDocumentRetrieval { files } =
        route(&inventory, &NoCorpus, question, "en-US")
    else {
        panic!("expected per-document retrieval");
    };
    assert_eq!(
        files.len(),
        10,
        "the inventory supplies the list, not the ranker"
    );

    let paths: Vec<String> = files
        .iter()
        .map(|file| file.relative_path.clone())
        .collect();
    let evidence =
        retrieval::search_per_document(&indexed.index, question, &bag_of_words(question), &paths)
            .unwrap();

    let covered: std::collections::BTreeSet<&str> = evidence
        .iter()
        .map(|item| item.relative_path.as_str())
        .collect();
    assert_eq!(
        covered.len(),
        10,
        "every indexed document must reach the model, not only the ones that ranked well"
    );
    for file in inventory.indexed_files() {
        assert!(
            covered.contains(file.relative_path.as_str()),
            "{} was left out",
            file.relative_path
        );
    }

    // The old path, for contrast: ranking alone sees a handful.
    let ranked = retrieval::search(&indexed.index, question, &bag_of_words(question)).unwrap();
    let ranked_files: std::collections::BTreeSet<&str> = ranked
        .iter()
        .map(|item| item.relative_path.as_str())
        .collect();
    assert!(
        ranked_files.len() < covered.len(),
        "ranking saw {} files, the per-document pass saw {}",
        ranked_files.len(),
        covered.len()
    );

    // Every excerpt is a real stored chunk: nothing was truncated to make it fit, so a citation
    // still points at a passage that exists.
    for item in &evidence {
        let stored = indexed
            .index
            .chunks_for_document(&item.relative_path)
            .unwrap();
        assert!(
            stored
                .iter()
                .any(|chunk| chunk.chunk_id == item.chunk_id && chunk.text == item.text),
            "excerpt for {} is not a stored chunk",
            item.relative_path
        );
    }

    assert_eq!(
        indexed.gateway.count(),
        0,
        "retrieval itself costs no gateway call"
    );
}

#[tokio::test]
async fn f2_coverage_is_computed_from_the_evidence_and_the_inventory() {
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    let question = "Give me a summary of each document";
    let paths: Vec<String> = inventory
        .indexed_files()
        .iter()
        .map(|file| file.relative_path.clone())
        .collect();
    let evidence =
        retrieval::search_per_document(&indexed.index, question, &bag_of_words(question), &paths)
            .unwrap();

    let coverage = EvidenceCoverage::of(
        &evidence,
        inventory.indexed_files().len(),
        inventory.unreadable_files().len(),
    );

    assert_eq!(
        coverage,
        EvidenceCoverage {
            files_covered: 10,
            indexed_files: 10,
            unreadable_files: 5,
        }
    );

    // The same fact for an ordinary ranked answer, which covers only part of the folder. The
    // interface is what refuses to let that pass as completeness.
    let ranked = retrieval::search(&indexed.index, question, &bag_of_words(question)).unwrap();
    let partial = EvidenceCoverage::of(&ranked, 10, 5);
    assert!(partial.files_covered < partial.indexed_files);
}

#[tokio::test]
async fn f3_a_document_that_ranks_badly_is_still_represented() {
    // A summary question shares almost no words with a lab report, so ranking inside that file
    // can come back empty. Coverage must not depend on that: the file gets its opening passage.
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);
    let paths: Vec<String> = inventory
        .indexed_files()
        .iter()
        .map(|file| file.relative_path.clone())
        .collect();

    let evidence = retrieval::search_per_document(
        &indexed.index,
        "zzzz nothing in this folder matches this word",
        &bag_of_words("zzzz nothing in this folder matches this word"),
        &paths,
    )
    .unwrap();

    assert_eq!(
        evidence.len(),
        10,
        "coverage does not depend on the ranking"
    );
}

// ---------------------------------------------------------------------------------------------
// G - ordinary phrasing, and shortened file names
// ---------------------------------------------------------------------------------------------

/// The questions the pilot actually typed on 23 September, against the real index rather than a
/// double, so the corpus rule is exercised with the words the documents really contain.
#[tokio::test]
async fn g1_an_ordinary_phrasing_is_answered_from_the_folder_with_no_gateway_call() {
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    for (question, locale) in [
        (
            "Peux-tu faire une liste de tous les documents disponibles ?",
            "fr-FR",
        ),
        ("Peux-tu me donner la liste des fichiers ?", "fr-FR"),
        (
            "Can you please give me a list of all the available documents?",
            "en-US",
        ),
        ("Could you show me every file in my work folder?", "en-US"),
    ] {
        match route(&inventory, &indexed.index, question, locale) {
            QuestionRoute::Deterministic(
                FolderAnswer::FileList { .. } | FolderAnswer::IndexedList { .. },
            ) => {}
            other => panic!("expected a listing for {question:?}, got {other:?}"),
        }
    }

    assert_eq!(
        indexed.gateway.count(),
        0,
        "a listing must not reach the gateway"
    );
}

#[tokio::test]
async fn g2_a_word_from_the_documents_still_sends_the_question_to_retrieval() {
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    // "glycemie" is in the lab report, so this is a question about what the documents say - which
    // no amount of shared grammar with a counting question may override.
    assert!(
        indexed.index.contains("glycemie"),
        "the fixture must carry the word"
    );
    assert_eq!(
        route(
            &inventory,
            &indexed.index,
            "Combien de documents parlent de glycemie ?",
            "fr-FR"
        ),
        QuestionRoute::GlobalRetrieval
    );
}

#[tokio::test]
async fn g3_a_file_is_everything_and_a_document_is_what_could_be_read() {
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    assert_eq!(
        deterministic(&inventory, "Combien de fichiers ?", LOCALE),
        FolderAnswer::FileCount { total: 15 }
    );
    assert_eq!(
        deterministic(&inventory, "Combien de documents ?", LOCALE),
        FolderAnswer::IndexedCount { count: 10 }
    );
    assert_eq!(indexed.gateway.count(), 0);
}

#[tokio::test]
async fn g4_a_shortened_file_name_reaches_the_file_it_names() {
    let indexed = index_fixture("nested").await;
    let inventory = inventory_of("nested", &indexed);

    // `2026/mars/biologie.pdf`, referred to the way a person refers to it.
    let QuestionRoute::TargetedRetrieval { file } =
        route(&inventory, &indexed.index, "Que dit biologie.pdf ?", LOCALE)
    else {
        panic!("expected targeted retrieval");
    };
    assert_eq!(file.relative_path, "2026/mars/biologie.pdf");
    assert_eq!(
        indexed.gateway.count(),
        0,
        "resolution costs no gateway call"
    );
}

// ---------------------------------------------------------------------------------------------
// Privacy
// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_inventory_holds_metadata_and_never_document_text() {
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    let serialised = serde_json::to_string(inventory.all_files()).expect("serialises");
    for sentence in [
        "This is a PDF report.",
        "only 2 files",
        "fake-document.pdf",
        "SANDBOX-0001",
    ] {
        assert!(
            !serialised.contains(sentence),
            "the inventory must not carry document text: {sentence}"
        );
    }

    // Nor does anything the gateway would be sent.
    let block = work_folder_context::build(&inventory, &ContextView::FileList).to_prompt_block();
    assert!(!block.contains("SANDBOX-0001"));
    assert!(!block.contains("only 2 files"));
    assert!(!block.contains("sha256"));
    for file in inventory.all_files() {
        if let Some(hash) = &file.sha256 {
            assert!(
                !block.contains(hash.as_str()),
                "no content hash reaches the model"
            );
        }
    }
}

#[tokio::test]
async fn the_same_folder_answers_the_same_way_whatever_model_is_configured() {
    // Filesystem facts come from the filesystem, so nothing about them can depend on an alias.
    // The route below takes no model, no alias and no gateway: that is the proof, and this test
    // keeps it from being quietly undone.
    let indexed = index_fixture("flat").await;
    let inventory = inventory_of("flat", &indexed);

    let first = deterministic(
        &inventory,
        "How many files are in the work folder?",
        "en-US",
    );
    let second = deterministic(&inventory, "Combien de fichiers au total ?", LOCALE);

    assert_eq!(first, FolderAnswer::FileCount { total: 15 });
    assert_eq!(second, FolderAnswer::FileCount { total: 15 });
    assert_eq!(indexed.gateway.count(), 0);
}
