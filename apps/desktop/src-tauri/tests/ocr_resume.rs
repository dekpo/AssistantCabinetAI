//! Lot 4 of the long-documents programme, the scans: a scanned document of many pages is read page by page,
//! each recognised page is kept as it is recognised, a stop is honoured between two pages, and the next
//! pass recognises only the pages that were not done.

mod common;

use std::sync::Mutex;

use assistant_cabinet_ai_lib::indexing::{self, IndexSummary};
use assistant_cabinet_ai_lib::ocr::fake::FakeOcrProvider;
use assistant_cabinet_ai_lib::ocr::OcrProvider;
use assistant_cabinet_ai_lib::raster::{FakeRasterizer, PageRasterizer};
use common::knowledge::{gateway_client, Lab};
use rusqlite::Connection;

const PAGES: u32 = 6;

fn scan(lab: &Lab, bytes: &[u8]) -> (FakeOcrProvider, FakeRasterizer) {
    lab.write_bytes("scan.pdf", bytes);
    let ocr = FakeOcrProvider::new();
    for page in 1..=PAGES {
        ocr.set_response(
            "scan.pdf",
            page,
            Ok(FakeOcrProvider::recognised_page(
                "scan.pdf",
                page,
                &format!(
                    "Scanned page {page} says something about the registry number {}.",
                    page * 111
                ),
            )),
        );
    }
    let raster = FakeRasterizer::new();
    raster.set_page_count("scan.pdf", PAGES);
    (ocr, raster)
}

async fn pass(
    lab: &mut Lab,
    ocr: &FakeOcrProvider,
    raster: &FakeRasterizer,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> IndexSummary {
    let mut context = lab.context();
    let url = lab.gateway.url.clone();
    let progress = Mutex::new(Vec::new());
    indexing::run_cancellable(
        &mut lab.index,
        &gateway_client(),
        &url,
        "cabinet-embed",
        lab.work.path(),
        None,
        Some(ocr as &dyn OcrProvider),
        Some(raster as &dyn PageRasterizer),
        lab.locale,
        &|step| progress.lock().unwrap().push(step),
        Some(&mut context),
        cancelled,
    )
    .await
    .expect("the pass completes")
}

fn cached_pages(lab: &Lab) -> i64 {
    Connection::open(lab.index_path())
        .unwrap()
        .query_row("SELECT COUNT(*) FROM ocr_page_cache", [], |r| r.get(0))
        .unwrap()
}

#[tokio::test]
async fn a_stopped_scan_keeps_its_recognised_pages_and_the_next_pass_recognises_only_the_rest() {
    let mut lab = Lab::new();
    let (ocr, raster) = scan(&lab, b"%PDF-1.4 a scan with no text layer");

    let first = pass(&mut lab, &ocr, &raster, &|| ocr.call_count() >= 3).await;
    assert!(first.cancelled);
    assert_eq!(
        ocr.call_count(),
        3,
        "the stop is honoured between two pages"
    );
    assert_eq!(
        lab.index.chunk_count().unwrap(),
        0,
        "a half-read scan is not stored as a document"
    );
    assert_eq!(cached_pages(&lab), 3, "but the three pages are kept");

    let second = pass(&mut lab, &ocr, &raster, &|| false).await;
    assert!(!second.cancelled);
    assert_eq!(
        ocr.call_count(),
        6,
        "only the three pages that were not done are recognised"
    );
    let pages: Vec<u32> = ocr.calls().iter().map(|(_, page)| *page).collect();
    assert_eq!(pages, vec![1, 2, 3, 4, 5, 6], "each page once, in order");
    assert_eq!(second.indexed_files, 1);
    assert_eq!(lab.index.chunk_count().unwrap(), 6);
    assert!(
        !lab.index
            .search_lexical("registry 333", 5)
            .unwrap()
            .is_empty(),
        "a page recognised before the stop is in the document"
    );
    assert_eq!(
        cached_pages(&lab),
        0,
        "the kept pages are dropped once the document is stored"
    );
}

#[tokio::test]
async fn pages_kept_for_a_file_that_changed_are_not_used() {
    let mut lab = Lab::new();
    let (ocr, raster) = scan(&lab, b"%PDF-1.4 first version of the scan");
    let first = pass(&mut lab, &ocr, &raster, &|| ocr.call_count() >= 2).await;
    assert!(first.cancelled);

    lab.write_bytes(
        "scan.pdf",
        b"%PDF-1.4 second version of the scan, different bytes",
    );
    pass(&mut lab, &ocr, &raster, &|| false).await;
    assert_eq!(
        ocr.call_count(),
        2 + 6,
        "a changed file is recognised again from its first page"
    );
}

#[tokio::test]
async fn a_scan_read_in_one_go_leaves_nothing_behind() {
    let mut lab = Lab::new();
    let (ocr, raster) = scan(&lab, b"%PDF-1.4 a complete pass");
    let summary = pass(&mut lab, &ocr, &raster, &|| false).await;
    assert!(!summary.cancelled);
    assert_eq!(ocr.call_count(), 6);
    assert_eq!(cached_pages(&lab), 0);
}
