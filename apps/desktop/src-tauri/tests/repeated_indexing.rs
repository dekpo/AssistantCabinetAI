//! What happens on the second, third and fourth "Analyse" of a session.
//!
//! The pilot's workflow is not "index everything once". It is "drop a document in the folder,
//! press Analyse, ask a question", over and over, without restarting the application and without
//! deleting the local index. A scanned PDF added after the first pass has to be read like one
//! that was there from the start.
//!
//! This is the regression suite for the bug in `docs/TROUBLESHOOTING.md`: the rasteriser was
//! rebuilt for every pass, pdfium refused to bind a second time, and from the second pass onwards
//! every scanned PDF was silently reported unreadable while JPEG and PNG kept working. These
//! tests drive the pass itself, with the ports faked; the construction that actually broke is
//! covered by `the_shared_rasterizer_answers_every_pass_not_only_the_first` in `src/raster.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::indexing::{
    self, IndexSummary, CAPABILITY_OCR_ENGINE, CAPABILITY_PAGE_RASTERIZER,
};
use assistant_cabinet_ai_lib::ocr::fake::FakeOcrProvider;
use assistant_cabinet_ai_lib::ocr::{OcrProvider, OcrStatus};
use assistant_cabinet_ai_lib::raster::{FakeRasterizer, PageRasterizer};
use serde_json::{json, Value};

/// A scanned letter: an image-only PDF, so it only reaches OCR through the rasteriser.
const SCAN: &str = "2026-03-22_courrier-rhumatologie-scan.pdf";
/// A photograph of a prescription: reaches OCR without any rasteriser at all. Its job here is to
/// be the control that kept working while scanned PDFs were failing.
const PHOTO: &str = "2026-03-26_ordonnance-scan.png";
/// Born-digital text, never any business of OCR.
const LETTER: &str = "2026-03-10_courrier-specialiste.txt";

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("fixtures")
        .join("gp-sandbox")
        .join("inbox")
}

/// Copies one sandbox fixture into the work folder, the way she drops a document into it.
fn add_document(work_folder: &Path, name: &str) {
    let source = fixtures().join(name);
    assert!(source.exists(), "fixture missing: {}", source.display());
    std::fs::copy(&source, work_folder.join(name)).expect("copies the fixture");
}

/// `/v1/embeddings` only. These tests never ask a question, so the chat route is not needed; the
/// vector just has to be the right shape and the right count.
fn start_fake_embeddings() -> String {
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
    let url = format!("http://{}", server.server_addr());

    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let payload: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let count = match &payload["input"] {
                Value::Array(items) => items.len(),
                Value::String(_) => 1,
                _ => 0,
            };
            let data: Vec<Value> = (0..count)
                .map(|index| json!({ "index": index, "embedding": vec![0.5_f32; 8] }))
                .collect();
            let response = tiny_http::Response::from_string(json!({ "data": data }).to_string())
                .with_status_code(200);
            let _ = request.respond(response);
        }
    });

    url
}

/// An engine that reads both scans and nothing else, so "was this file read" is unambiguous.
fn fake_engine() -> FakeOcrProvider {
    let ocr = FakeOcrProvider::new();
    ocr.set_fallback_status(OcrStatus::NoTextFound);
    ocr.set_response(
        SCAN,
        1,
        Ok(FakeOcrProvider::recognised_page(
            SCAN,
            1,
            "Cabinet de rhumatologie. Douleurs articulaires bilaterales des mains.",
        )),
    );
    ocr.set_response(
        PHOTO,
        1,
        Ok(FakeOcrProvider::recognised_page(
            PHOTO,
            1,
            "Ordonnance. Paracetamol 1000 mg, une prise si douleur.",
        )),
    );
    ocr
}

/// One press of "Analyse".
async fn pass(
    index: &mut IndexStore,
    gateway: &GatewayClient,
    server_url: &str,
    work_folder: &Path,
    ocr: Option<&dyn OcrProvider>,
    rasterizer: Option<&dyn PageRasterizer>,
) -> IndexSummary {
    indexing::run(
        index,
        gateway,
        server_url,
        "cabinet-embed",
        work_folder,
        ocr,
        rasterizer,
        "fr-FR",
        &|_| {},
    )
    .await
    .expect("the pass completes")
}

fn lists(paths: &[String], name: &str) -> bool {
    paths.iter().any(|path| path == name)
}

#[tokio::test]
async fn a_scan_added_after_the_first_pass_is_read_by_the_second() {
    let server_url = start_fake_embeddings();
    let gateway = GatewayClient::new().expect("builds a client");
    let work = tempfile::tempdir().expect("temp work folder");
    let index_dir = tempfile::tempdir().expect("temp index dir");
    let mut index =
        IndexStore::open_at(&index_dir.path().join("index.sqlite3")).expect("opens the index");
    let ocr = fake_engine();
    let rasterizer = FakeRasterizer::with_page_count(1);

    add_document(work.path(), LETTER);
    let first = pass(
        &mut index,
        &gateway,
        &server_url,
        work.path(),
        Some(&ocr),
        Some(&rasterizer),
    )
    .await;
    assert_eq!(first.scanned_files, 1);
    assert!(first.ocr_files.is_empty(), "no scan in the folder yet");

    // She drops the scanned letter in and presses Analyse again. No restart, no deleted index.
    add_document(work.path(), SCAN);
    let second = pass(
        &mut index,
        &gateway,
        &server_url,
        work.path(),
        Some(&ocr),
        Some(&rasterizer),
    )
    .await;

    assert_eq!(
        second.indexed_files, 1,
        "only the new document is read again"
    );
    assert_eq!(second.unchanged_files, 1, "the text letter is left alone");
    assert!(
        lists(&second.ocr_files, SCAN),
        "the scan added after the first pass must be read by OCR, not reported unreadable"
    );
    assert!(!lists(&second.empty_files, SCAN));
    assert!(second.unavailable_capabilities.is_empty());
}

#[tokio::test]
async fn a_fourth_pass_still_reads_scans() {
    let server_url = start_fake_embeddings();
    let gateway = GatewayClient::new().expect("builds a client");
    let work = tempfile::tempdir().expect("temp work folder");
    let index_dir = tempfile::tempdir().expect("temp index dir");
    let mut index =
        IndexStore::open_at(&index_dir.path().join("index.sqlite3")).expect("opens the index");
    let ocr = fake_engine();
    let rasterizer = FakeRasterizer::with_page_count(1);

    add_document(work.path(), LETTER);
    for _ in 0..3 {
        pass(
            &mut index,
            &gateway,
            &server_url,
            work.path(),
            Some(&ocr),
            Some(&rasterizer),
        )
        .await;
    }

    add_document(work.path(), SCAN);
    add_document(work.path(), PHOTO);
    let fourth = pass(
        &mut index,
        &gateway,
        &server_url,
        work.path(),
        Some(&ocr),
        Some(&rasterizer),
    )
    .await;

    assert!(
        lists(&fourth.ocr_files, SCAN),
        "the scanned PDF must survive as many passes as she presses the button"
    );
    assert!(
        lists(&fourth.ocr_files, PHOTO),
        "the photograph never needed the rasteriser and must still be read"
    );
    assert!(fourth.unavailable_capabilities.is_empty());
}

#[tokio::test]
async fn a_missing_rasterizer_is_reported_rather_than_blamed_on_the_document() {
    let server_url = start_fake_embeddings();
    let gateway = GatewayClient::new().expect("builds a client");
    let work = tempfile::tempdir().expect("temp work folder");
    let index_dir = tempfile::tempdir().expect("temp index dir");
    let mut index =
        IndexStore::open_at(&index_dir.path().join("index.sqlite3")).expect("opens the index");
    let ocr = fake_engine();

    add_document(work.path(), SCAN);
    add_document(work.path(), PHOTO);
    let summary = pass(
        &mut index,
        &gateway,
        &server_url,
        work.path(),
        Some(&ocr),
        None,
    )
    .await;

    assert!(
        lists(&summary.empty_files, SCAN),
        "without a rasteriser the scanned PDF still reads as empty"
    );
    assert!(
        lists(&summary.ocr_files, PHOTO),
        "the photograph is the tell: images keep working while scanned PDFs do not"
    );
    assert_eq!(
        summary.unavailable_capabilities,
        vec![CAPABILITY_PAGE_RASTERIZER],
        "the pass must say the capability is missing instead of letting the scan look illegible"
    );
}

#[tokio::test]
async fn a_scan_read_as_empty_without_an_engine_is_retried_once_the_engine_is_back() {
    let server_url = start_fake_embeddings();
    let gateway = GatewayClient::new().expect("builds a client");
    let work = tempfile::tempdir().expect("temp work folder");
    let index_dir = tempfile::tempdir().expect("temp index dir");
    let mut index =
        IndexStore::open_at(&index_dir.path().join("index.sqlite3")).expect("opens the index");

    add_document(work.path(), SCAN);
    let without = pass(&mut index, &gateway, &server_url, work.path(), None, None).await;

    assert!(lists(&without.empty_files, SCAN));
    assert_eq!(
        without.unavailable_capabilities,
        vec![CAPABILITY_OCR_ENGINE, CAPABILITY_PAGE_RASTERIZER]
    );

    // The engine comes back - a restart, or a staged resource - and the same index is reused.
    // Nothing about the file changed, so only the stored "empty, read by no engine" mark can
    // make the pass look at it again. She must never have to delete the index by hand.
    let ocr = fake_engine();
    let rasterizer = FakeRasterizer::with_page_count(1);
    let with = pass(
        &mut index,
        &gateway,
        &server_url,
        work.path(),
        Some(&ocr),
        Some(&rasterizer),
    )
    .await;

    assert!(
        lists(&with.ocr_files, SCAN),
        "a scan stored empty before the engine existed must be retried, not skipped for ever"
    );
    assert_eq!(with.unchanged_files, 0);
    assert!(with.unavailable_capabilities.is_empty());
}
