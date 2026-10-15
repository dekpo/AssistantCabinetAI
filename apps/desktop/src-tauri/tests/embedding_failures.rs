//! What indexing does when the embeddings call is slow, refused or gone.
//!
//! Before this, any `reqwest` timeout became `server_timeout`, whose only sentence tells the user
//! to raise a waiting time that governs chat and nothing else, and one slow file ended the whole
//! pass. These tests hold the replacements: an honest code of its own, one bounded retry, a failed
//! file reported by name without ever reaching the index, and a pass that goes on.

mod common;

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use assistant_cabinet_ai_lib::error::AppError;
use assistant_cabinet_ai_lib::gateway::{EmbeddingDeadlines, GatewayClient};
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::indexing::{self, IndexProgress, IndexSummary};
use serde_json::Value;

use common::fake_gateway::{closed_address, FakeGateway, Reply, Seen};

/// Text that makes the scripted gateway stall.
const POISON: &str = "POISONED";
/// Longer than any deadline below, shorter than a test worth waiting for.
const STALL: Duration = Duration::from_millis(600);

fn quick_deadlines() -> EmbeddingDeadlines {
    EmbeddingDeadlines {
        first_batch: Duration::from_millis(250),
        batch: Duration::from_millis(250),
        retry_pause: Duration::ZERO,
    }
}

/// For tests where nothing listens: Windows takes about two seconds to refuse a connection to a
/// closed port, and the refusal must win over the deadline for the test to mean anything.
fn patient_deadlines() -> EmbeddingDeadlines {
    EmbeddingDeadlines {
        first_batch: Duration::from_secs(10),
        batch: Duration::from_secs(10),
        retry_pause: Duration::ZERO,
    }
}

fn client(deadlines: EmbeddingDeadlines) -> GatewayClient {
    GatewayClient::new()
        .expect("builds a client")
        .with_embedding_deadlines(deadlines)
}

fn texts(count: usize) -> Vec<String> {
    (0..count)
        .map(|number| format!("passage {number}"))
        .collect()
}

fn poisoned(seen: &Seen) -> bool {
    seen.inputs.iter().any(|text| text.contains(POISON))
}

// ---------------------------------------------------------------- the gateway call itself

#[tokio::test]
async fn a_slow_embedding_reports_its_own_code_not_the_chat_one() {
    let gateway = FakeGateway::start(|_| Reply::slow(STALL));

    let error = client(quick_deadlines())
        .embed(&gateway.url, "cabinet-embed", &texts(3))
        .await
        .expect_err("the deadline passes");

    assert_eq!(error.code(), "embedding_timeout");
    assert!(matches!(error, AppError::EmbeddingTimeout));
}

#[tokio::test]
async fn a_timeout_is_retried_exactly_once() {
    let gateway = FakeGateway::start(|seen| {
        if seen.number == 1 {
            Reply::slow(STALL)
        } else {
            Reply::vectors()
        }
    });

    let vectors = client(quick_deadlines())
        .embed(&gateway.url, "cabinet-embed", &texts(3))
        .await
        .expect("the second attempt answers");

    assert_eq!(vectors.len(), 3);
    assert_eq!(gateway.requests().len(), 2);
}

#[tokio::test]
async fn two_timeouts_in_a_row_stop_at_two_requests() {
    let gateway = FakeGateway::start(|_| Reply::slow(STALL));

    let error = client(quick_deadlines())
        .embed(&gateway.url, "cabinet-embed", &texts(2))
        .await
        .expect_err("both attempts time out");

    assert_eq!(error.code(), "embedding_timeout");
    assert_eq!(gateway.requests().len(), 2);
}

#[tokio::test]
async fn a_server_that_is_briefly_unavailable_is_retried() {
    for status in [502u16, 503, 504] {
        let gateway = FakeGateway::start(move |seen| {
            if seen.number == 1 {
                Reply::refusal(status, "provider_error")
            } else {
                Reply::vectors()
            }
        });

        // Every reply is immediate: the deadline is not what is tested, and a short one expires on a loaded
        // machine before the reply is read, which turns the refusal into a timeout.
        let vectors = client(patient_deadlines())
            .embed(&gateway.url, "cabinet-embed", &texts(2))
            .await
            .unwrap_or_else(|error| panic!("status {status} was not retried: {}", error.code()));

        assert_eq!(vectors.len(), 2, "status {status}");
        assert_eq!(gateway.requests().len(), 2, "status {status}");
    }
}

#[tokio::test]
async fn a_refusal_of_the_request_itself_is_never_retried() {
    for (status, code) in [
        (400u16, "invalid_request"),
        (404, "model_alias_not_allowed"),
        (413, "payload_too_large"),
    ] {
        let gateway = FakeGateway::start(move |_| Reply::refusal(status, code));

        // The refusal is immediate: the deadline is not what is tested. With the 250 ms one, a machine under
        // load (the whole suite at once) let it pass before the reply was read, the refusal became a timeout
        // and was retried, which is the failure this test saw once.
        let error = client(patient_deadlines())
            .embed(&gateway.url, "cabinet-embed", &texts(2))
            .await
            .expect_err("the gateway refuses");

        assert_eq!(error.code(), code);
        assert_eq!(
            gateway.requests().len(),
            1,
            "status {status} must not be retried"
        );
    }
}

#[tokio::test]
async fn the_first_request_of_a_pass_gets_the_longer_deadline() {
    // 400 ms is past `batch` (250 ms) but inside `first_batch` (1 s): the model is cold-loading.
    let deadlines = EmbeddingDeadlines {
        first_batch: Duration::from_secs(1),
        batch: Duration::from_millis(250),
        retry_pause: Duration::ZERO,
    };
    let gateway = FakeGateway::start(|_| Reply::slow(Duration::from_millis(400)));
    let gateway_client = client(deadlines);

    gateway_client
        .embed_batch(
            &gateway.url,
            "cabinet-embed",
            &texts(1),
            deadlines.first_batch,
        )
        .await
        .expect("the cold load fits the first deadline");
    let later = gateway_client
        .embed_batch(&gateway.url, "cabinet-embed", &texts(1), deadlines.batch)
        .await
        .expect_err("the same stall does not fit a warm deadline");

    assert_eq!(later.code(), "embedding_timeout");
}

#[tokio::test]
async fn a_server_that_is_off_is_unreachable_not_a_timeout() {
    let error = client(patient_deadlines())
        .embed(&closed_address(), "cabinet-embed", &texts(1))
        .await
        .expect_err("nothing listens");

    assert_eq!(error.code(), "server_unreachable");
}

// ---------------------------------------------------------------- the pass

/// `paragraphs` chunks of 1 000 characters, the last one carrying the poison when asked.
fn write_document(work: &Path, name: &str, paragraphs: usize, poison_last: bool) {
    let body: Vec<String> = (0..paragraphs)
        .map(|number| {
            let mut paragraph = format!("para-{number:04} ");
            if poison_last && number + 1 == paragraphs {
                paragraph.push_str(POISON);
                paragraph.push(' ');
            }
            paragraph.push_str(&"x".repeat(1_000 - paragraph.len()));
            paragraph
        })
        .collect();
    std::fs::write(work.join(name), body.join("\n\n")).expect("writes the document");
}

struct Pass {
    result: Result<IndexSummary, AppError>,
    index: IndexStore,
    progress: Vec<IndexProgress>,
    _dir: tempfile::TempDir,
}

async fn pass(work: &Path, server_url: &str, deadlines: EmbeddingDeadlines) -> Pass {
    let dir = tempfile::tempdir().expect("temp index dir");
    let mut index = IndexStore::open_at(&dir.path().join("index.sqlite3")).expect("opens index");
    let progress = Mutex::new(Vec::new());
    let result = indexing::run(
        &mut index,
        &client(deadlines),
        server_url,
        "cabinet-embed",
        work,
        None,
        None,
        "fr-FR",
        &|step| progress.lock().expect("lock").push(step),
    )
    .await;
    Pass {
        result,
        index,
        progress: progress.into_inner().expect("lock"),
        _dir: dir,
    }
}

#[tokio::test]
async fn a_file_that_cannot_be_embedded_does_not_stop_the_others() {
    let gateway = FakeGateway::start(|seen| {
        if poisoned(seen) {
            Reply::slow(STALL)
        } else {
            Reply::vectors()
        }
    });
    let work = tempfile::tempdir().expect("temp work folder");
    write_document(work.path(), "a-first.txt", 3, false);
    // Forty chunks, and only the last batch stalls: the first two batches embed fine, which is
    // exactly the case where a half-indexed document could be written.
    write_document(work.path(), "b-stalls.txt", 40, true);
    write_document(work.path(), "c-last.txt", 3, false);

    let run = pass(work.path(), &gateway.url, quick_deadlines()).await;

    let summary = run.result.expect("the pass completes");
    // All three texts are stored before any vector is asked for; the server refuses one file's.
    assert_eq!(summary.indexed_files, 3);
    assert_eq!(
        summary.failed_files.len(),
        1,
        "only the file the server refuses is named"
    );
    assert_eq!(summary.failed_files[0].path, "b-stalls.txt");
    assert_eq!(summary.failed_files[0].code, "embedding_timeout");
    for good in ["a-first.txt", "c-last.txt"] {
        let counts = run.index.vector_counts_of(good).unwrap();
        assert_eq!(
            counts.done, counts.total,
            "{good} is fully vectorised, though it shared a batch with the failing file"
        );
    }
    let stalled = run.index.vector_counts_of("b-stalls.txt").unwrap();
    assert_eq!(stalled.total, 41, "its 40 passages and its card");
    assert!(
        stalled.done < stalled.total,
        "the failing file keeps its text and lacks some vectors"
    );
    let paths: Vec<String> = run
        .index
        .all_documents()
        .unwrap()
        .into_iter()
        .map(|d| d.relative_path)
        .collect();
    assert!(
        !run.index
            .search_lexical_terms_in(&["para-0003".to_string()], 5, &paths)
            .unwrap()
            .is_empty(),
        "and its text is still in the full-text index"
    );
    assert_eq!(summary.vectors_pending, stalled.total - stalled.done);
}

#[tokio::test]
async fn a_failed_file_is_tried_again_by_the_next_pass() {
    let work = tempfile::tempdir().expect("temp work folder");
    write_document(work.path(), "b-stalls.txt", 3, true);
    let dir = tempfile::tempdir().expect("temp index dir");
    let mut index = IndexStore::open_at(&dir.path().join("index.sqlite3")).expect("opens index");

    let stalled = FakeGateway::start(|_| Reply::slow(STALL));
    let first = indexing::run(
        &mut index,
        &client(quick_deadlines()),
        &stalled.url,
        "cabinet-embed",
        work.path(),
        None,
        None,
        "fr-FR",
        &|_| {},
    )
    .await
    .expect("the pass completes");
    assert_eq!(first.failed_files.len(), 1);

    let healthy = FakeGateway::start(|_| Reply::vectors());
    let second = indexing::run(
        &mut index,
        &client(quick_deadlines()),
        &healthy.url,
        "cabinet-embed",
        work.path(),
        None,
        None,
        "fr-FR",
        &|_| {},
    )
    .await
    .expect("the pass completes");

    assert_eq!(second.indexed_files, 0, "the text is not read again");
    assert!(second.failed_files.is_empty());
    assert_eq!(second.unchanged_files, 1);
    assert_eq!(
        second.vectors_pending, 0,
        "but its missing vectors are computed now"
    );
}

#[tokio::test]
async fn an_error_that_is_not_about_one_file_stops_the_pass() {
    let work = tempfile::tempdir().expect("temp work folder");
    write_document(work.path(), "a-first.txt", 3, false);
    write_document(work.path(), "b-second.txt", 3, false);

    let run = pass(work.path(), &closed_address(), patient_deadlines()).await;

    let error = run.result.expect_err("nothing answers");
    assert_eq!(error.code(), "server_unreachable");
    // The pass stops, and keeps what it wrote: the text of both files, without their vectors.
    assert_eq!(run.index.chunk_count().unwrap(), 6);
    assert_eq!(run.index.vector_counts().unwrap().done, 0);
}

#[tokio::test]
async fn a_model_that_is_down_stops_the_pass_with_the_gateways_own_code() {
    let gateway = FakeGateway::start(|_| Reply::refusal(503, "provider_unreachable"));
    let work = tempfile::tempdir().expect("temp work folder");
    write_document(work.path(), "a-first.txt", 3, false);
    write_document(work.path(), "b-second.txt", 3, false);

    let run = pass(work.path(), &gateway.url, quick_deadlines()).await;

    let error = run.result.expect_err("the model is down");
    assert_eq!(error.code(), "provider_unreachable");
    // First file: the request and its one retry. The second file is never started.
    assert_eq!(gateway.requests().len(), 2);
}

#[tokio::test]
async fn repeated_failures_end_the_pass_instead_of_grinding_through_the_folder() {
    let gateway = FakeGateway::start(|_| Reply::slow(STALL));
    let work = tempfile::tempdir().expect("temp work folder");
    for number in 0..(indexing::MAX_CONSECUTIVE_FAILED_FILES + 2) {
        write_document(work.path(), &format!("doc-{number}.txt"), 2, false);
    }

    let run = pass(work.path(), &gateway.url, quick_deadlines()).await;

    let error = run.result.expect_err("a server that stalls on everything");
    assert_eq!(error.code(), "embedding_timeout");
    // The shared batch (two attempts), then each file alone (two attempts each) until the limit: the
    // files after it are not tried.
    assert_eq!(
        gateway.requests().len(),
        2 + 2 * indexing::MAX_CONSECUTIVE_FAILED_FILES
    );
    assert_eq!(
        run.index.chunk_count().unwrap(),
        2 * (indexing::MAX_CONSECUTIVE_FAILED_FILES + 2) as u64
    );
}

#[tokio::test]
async fn a_good_file_resets_the_count_of_consecutive_failures() {
    let gateway = FakeGateway::start(|seen| {
        if poisoned(seen) {
            Reply::slow(STALL)
        } else {
            Reply::vectors()
        }
    });
    let work = tempfile::tempdir().expect("temp work folder");
    // Alternating bad and good: more bad files in all than the limit allows in a row.
    let pairs = indexing::MAX_CONSECUTIVE_FAILED_FILES + 1;
    for number in 0..pairs {
        write_document(work.path(), &format!("{number}-bad.txt"), 2, true);
        write_document(work.path(), &format!("{number}-good.txt"), 2, false);
    }

    let run = pass(work.path(), &gateway.url, quick_deadlines()).await;

    let summary = run
        .result
        .expect("failures that are not in a row do not stop the pass");
    assert_eq!(summary.failed_files.len(), pairs);
    assert_eq!(summary.indexed_files, 2 * pairs, "every text is stored");
    for number in 0..pairs {
        let good = run
            .index
            .vector_counts_of(&format!("{number}-good.txt"))
            .unwrap();
        assert_eq!(good.done, good.total, "the good files are fully vectorised");
    }
}

// ---------------------------------------------------------------- progress

#[tokio::test]
async fn progress_moves_inside_a_long_file_and_never_goes_back() {
    let gateway = FakeGateway::start(|_| Reply::vectors());
    let work = tempfile::tempdir().expect("temp work folder");
    write_document(work.path(), "a-long.txt", 40, false);
    write_document(work.path(), "b-short.txt", 3, false);

    let run = pass(work.path(), &gateway.url, quick_deadlines()).await;

    run.result.expect("the pass completes");
    let steps = run.progress;
    // 43 passages and 2 cards: the cards (one batch), then 16 a batch, then the final report.
    let vectors: Vec<(usize, usize)> = steps
        .iter()
        .filter(|step| step.vectors_total > 0)
        .map(|step| (step.vectors_done, step.vectors_total))
        .collect();
    assert_eq!(
        vectors,
        vec![(0, 45), (2, 45), (18, 45), (34, 45), (45, 45), (45, 45)],
        "the two cards first, then the passages, then the last report repeats the end"
    );
    for pair in steps.windows(2) {
        let before = (pair[0].processed_files, pair[0].vectors_done);
        let after = (pair[1].processed_files, pair[1].vectors_done);
        assert!(
            after >= before,
            "progress went back: {before:?} then {after:?}"
        );
    }
    let last = steps.last().expect("at least one report");
    assert_eq!((last.processed_files, last.total_files), (2, 2));
}

#[tokio::test]
async fn progress_carries_counts_and_nothing_else() {
    let gateway = FakeGateway::start(|_| Reply::vectors());
    let work = tempfile::tempdir().expect("temp work folder");
    write_document(work.path(), "dossier-confidentiel.txt", 20, false);

    let run = pass(work.path(), &gateway.url, quick_deadlines()).await;

    run.result.expect("the pass completes");
    assert!(!run.progress.is_empty());
    for step in &run.progress {
        let value = serde_json::to_value(step).expect("serialises");
        let object = value.as_object().expect("an object");
        let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
        keys.sort_unstable();
        // The counts of the pass; while the vectors are computed, theirs too.
        let known = [
            "batchIndex",
            "batchTotal",
            "processedFiles",
            "totalFiles",
            "vectorsDone",
            "vectorsTotal",
        ];
        assert!(
            keys.iter().all(|key| known.contains(key))
                && keys.contains(&"processedFiles")
                && keys.contains(&"totalFiles"),
            "{keys:?}"
        );
        assert!(
            object.values().all(Value::is_number),
            "counts only: {value}"
        );
    }
}
