//! Lot 4 of the long-documents programme, the pass: the text of every file is written before the first
//! vector is asked for, the vectors come batch by batch and each batch is kept, a pass that is stopped
//! keeps everything and the next one goes on where it stopped, and a file that cannot be read does not
//! stop the others.

mod common;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use assistant_cabinet_ai_lib::indexing::{self, IndexProgress, IndexSummary};
use common::fake_gateway::{FakeGateway, Reply};
use common::knowledge::{gateway_client, Lab};
use rusqlite::Connection;

fn passage(file: usize, n: usize) -> String {
    format!("file{file:02} sentence {n:03}: the usual words of an ordinary paragraph that fills a passage nicely.")
}

/// A document of `passages` paragraphs, each long enough to be a passage of its own.
fn document(file: usize, passages: usize) -> String {
    (0..passages)
        .map(|n| format!("{} {}", passage(file, n), "x".repeat(1_000)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

async fn pass(
    lab: &mut Lab,
    cancelled: &(dyn Fn() -> bool + Sync),
    progress: &Mutex<Vec<IndexProgress>>,
) -> Result<IndexSummary, assistant_cabinet_ai_lib::error::AppError> {
    let mut context = lab.context();
    let url = lab.gateway.url.clone();
    indexing::run_cancellable(
        &mut lab.index,
        &gateway_client(),
        &url,
        "cabinet-embed",
        lab.work.path(),
        None,
        None,
        lab.locale,
        &|step| progress.lock().unwrap().push(step),
        Some(&mut context),
        cancelled,
    )
    .await
}

fn count(path: &std::path::Path, sql: &str) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row(sql, [], |r| r.get(0))
        .unwrap()
}

#[tokio::test]
async fn every_file_is_written_before_the_first_vector_is_asked_for() {
    let mut lab = Lab::new();
    let seen_at_first_request: Arc<Mutex<Option<i64>>> = Arc::new(Mutex::new(None));
    let index_path = lab.index_path();
    let record = seen_at_first_request.clone();
    lab.gateway = FakeGateway::start(move |_| {
        let mut slot = record.lock().unwrap();
        if slot.is_none() {
            *slot = Some(count(&index_path, "SELECT COUNT(*) FROM documents"));
        }
        Reply::vectors()
    });
    for file in 0..6 {
        lab.write(&format!("f{file}.txt"), &document(file, 4));
    }
    let progress = Mutex::new(Vec::new());
    pass(&mut lab, &|| false, &progress).await.unwrap();
    assert_eq!(
        *seen_at_first_request.lock().unwrap(),
        Some(6),
        "the six texts were already stored"
    );
}

#[tokio::test]
async fn a_stopped_pass_keeps_every_batch_it_finished_and_the_words_of_every_file() {
    let mut lab = Lab::new();
    let stop = Arc::new(AtomicBool::new(false));
    let requests = Arc::new(AtomicUsize::new(0));
    let (flag, counter) = (stop.clone(), requests.clone());
    lab.gateway = FakeGateway::start(move |_| {
        if counter.fetch_add(1, Ordering::SeqCst) + 1 == 2 {
            flag.store(true, Ordering::SeqCst);
        }
        Reply::vectors()
    });
    for file in 0..5 {
        lab.write(&format!("f{file}.txt"), &document(file, 30));
    }
    let progress = Mutex::new(Vec::new());
    let flag = stop.clone();
    let summary = pass(&mut lab, &move || flag.load(Ordering::SeqCst), &progress)
        .await
        .unwrap();

    assert!(summary.cancelled);
    let counts = lab.index.vector_counts().unwrap();
    assert_eq!(counts.total, 155, "150 passages and 5 cards");
    assert_eq!(
        counts.done, 21,
        "the five cards, then one batch of sixteen: two requests were stored, no more"
    );
    assert_eq!(summary.vectors_pending, 134);
    // The last file's words are searchable although none of its vectors exists.
    let hits = lab.index.search_lexical("file04 sentence 029", 5).unwrap();
    assert!(!hits.is_empty(), "the word search works on everything");
    // The profile of the files was built before the vectors: the finder works.
    let profiles = count(&lab.index_path(), "SELECT COUNT(*) FROM file_profiles");
    assert_eq!(profiles, 5);
}

#[tokio::test]
async fn the_next_pass_computes_only_the_vectors_that_are_missing() {
    let mut lab = Lab::new();
    let stop = Arc::new(AtomicBool::new(false));
    let counter = Arc::new(AtomicUsize::new(0));
    let (flag, seen) = (stop.clone(), counter.clone());
    lab.gateway = FakeGateway::start(move |_| {
        if seen.fetch_add(1, Ordering::SeqCst) + 1 == 3 {
            flag.store(true, Ordering::SeqCst);
        }
        Reply::vectors()
    });
    for file in 0..4 {
        lab.write(&format!("f{file}.txt"), &document(file, 30));
    }
    let progress = Mutex::new(Vec::new());
    let flag = stop.clone();
    let first = pass(&mut lab, &move || flag.load(Ordering::SeqCst), &progress)
        .await
        .unwrap();
    assert!(first.cancelled);
    let first_inputs: Vec<String> = lab
        .gateway
        .requests()
        .into_iter()
        .flat_map(|s| s.inputs)
        .collect();
    assert_eq!(
        first_inputs.len(),
        36,
        "the four cards, then two batches of sixteen"
    );

    let second = pass(&mut lab, &|| false, &progress).await.unwrap();
    assert!(!second.cancelled);
    assert_eq!(
        (second.indexed_files, second.unchanged_files),
        (0, 4),
        "no file is read again"
    );
    assert_eq!(second.vectors_pending, 0);
    let all_inputs: Vec<String> = lab
        .gateway
        .requests()
        .into_iter()
        .flat_map(|s| s.inputs)
        .collect();
    assert_eq!(
        all_inputs.len(),
        124,
        "every passage and card was embedded once, across both passes"
    );
    let mut unique = all_inputs.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 124, "and none twice");
}

#[tokio::test]
async fn a_pass_stopped_while_reading_keeps_the_files_it_read_and_the_next_one_reads_the_rest() {
    let mut lab = Lab::new();
    for file in 0..5 {
        lab.write(&format!("f{file}.txt"), &document(file, 3));
    }
    let progress = Mutex::new(Vec::new());
    let path = lab.index_path();
    let first = pass(
        &mut lab,
        &move || count(&path, "SELECT COUNT(*) FROM documents") >= 2,
        &progress,
    )
    .await
    .unwrap();
    assert!(first.cancelled);
    assert_eq!(first.indexed_files, 2);
    assert!(
        lab.gateway.requests().is_empty(),
        "no vector is asked for after a stop during the reading"
    );

    let second = pass(&mut lab, &|| false, &progress).await.unwrap();
    assert_eq!((second.indexed_files, second.unchanged_files), (3, 2));
    assert_eq!(second.vectors_pending, 0);
}

#[tokio::test]
async fn a_file_that_cannot_be_read_does_not_stop_the_others() {
    let mut lab = Lab::new();
    lab.write_bytes("a-broken.docx", b"this is not a zip archive at all");
    lab.write("b-good.txt", &document(1, 3));
    let progress = Mutex::new(Vec::new());
    let summary = pass(&mut lab, &|| false, &progress)
        .await
        .expect("the pass completes");
    assert_eq!(summary.indexed_files, 1);
    assert_eq!(summary.failed_files.len(), 1);
    assert_eq!(summary.failed_files[0].path, "a-broken.docx");
    assert_eq!(summary.failed_files[0].code, "extraction_failed");
    assert_eq!(summary.vectors_pending, 0);
}

#[tokio::test]
async fn the_progress_tells_the_two_phases_apart() {
    let mut lab = Lab::new();
    for file in 0..3 {
        lab.write(&format!("f{file}.txt"), &document(file, 20));
    }
    let progress = Mutex::new(Vec::new());
    pass(&mut lab, &|| false, &progress).await.unwrap();
    let steps = progress.into_inner().unwrap();

    let reading: Vec<&IndexProgress> = steps.iter().filter(|s| s.vectors_total == 0).collect();
    let meaning: Vec<&IndexProgress> = steps.iter().filter(|s| s.vectors_total > 0).collect();
    assert!(!reading.is_empty() && !meaning.is_empty());
    // the reading comes first, then the vectors, never the other way round
    let first_meaning = steps.iter().position(|s| s.vectors_total > 0).unwrap();
    assert!(steps[..first_meaning].iter().all(|s| s.vectors_total == 0));
    // the vectors only go up, from the share already done to all of it
    let done: Vec<usize> = meaning.iter().map(|s| s.vectors_done).collect();
    assert!(done.windows(2).all(|w| w[0] <= w[1]), "{done:?}");
    assert_eq!(*done.last().unwrap(), 63);
    assert!(meaning.iter().all(|s| s.vectors_total == 63));
}

#[tokio::test]
async fn a_second_pass_over_nothing_new_does_no_work() {
    let mut lab = Lab::new();
    lab.write("a.txt", &document(1, 5));
    let progress = Mutex::new(Vec::new());
    pass(&mut lab, &|| false, &progress).await.unwrap();
    let before = lab.gateway.requests().len();
    let second = pass(&mut lab, &|| false, &progress).await.unwrap();
    assert_eq!(
        (
            second.indexed_files,
            second.unchanged_files,
            second.vectors_pending
        ),
        (0, 1, 0)
    );
    assert_eq!(lab.gateway.requests().len(), before);
}
