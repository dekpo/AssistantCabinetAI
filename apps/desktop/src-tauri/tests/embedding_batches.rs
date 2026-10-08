//! A long document is embedded in several small requests, never one.
//!
//! The regression behind `docs/TROUBLESHOOTING.md` (embedding timeout): a dense 42-page PDF
//! became one `/v1/embeddings` request of 126 inputs and 114 000 characters, which took 76 s on
//! the CPU-only server while the client gave up at 60 s. The vectors were never stored, and the
//! interface blamed the chat model. Request size is what bounds the latency of one call, so it is
//! what these tests hold down.

mod common;

use std::path::Path;

use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::indexing::{self, IndexSummary};

use common::fake_gateway::{FakeGateway, Reply};

/// One paragraph is one chunk: 1 000 characters is under the 1 200-character chunk ceiling, and
/// two of them never fit together.
const PARAGRAPH_CHARS: usize = 1_000;
const CHUNKS: usize = 300;

/// A document of `count` numbered paragraphs (`para-0000 ...`).
fn numbered_document(count: usize) -> String {
    (0..count)
        .map(|number| {
            let head = format!("para-{number:04} ");
            format!("{head}{}", "x".repeat(PARAGRAPH_CHARS - head.len()))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

async fn analyse(work: &Path, server_url: &str) -> (IndexSummary, IndexStore, tempfile::TempDir) {
    let index_dir = tempfile::tempdir().expect("temp index dir");
    let mut index =
        IndexStore::open_at(&index_dir.path().join("index.sqlite3")).expect("opens the index");
    let gateway = GatewayClient::new().expect("builds a client");
    let summary = indexing::run(
        &mut index,
        &gateway,
        server_url,
        "cabinet-embed",
        work,
        None,
        None,
        "fr-FR",
        &|_| {},
    )
    .await
    .expect("the pass completes");
    (summary, index, index_dir)
}

#[tokio::test]
async fn a_long_document_is_embedded_in_several_bounded_requests() {
    let gateway = FakeGateway::start(|_| Reply::vectors());
    let work = tempfile::tempdir().expect("temp work folder");
    std::fs::write(work.path().join("rapport.txt"), numbered_document(CHUNKS)).expect("writes");

    let (summary, index, _dir) = analyse(work.path(), &gateway.url).await;

    let requests = gateway.requests();
    assert_eq!(summary.indexed_files, 1);
    assert_eq!(index.chunk_count().expect("counts"), CHUNKS as u64);
    assert!(
        requests.len() > 1,
        "one request carried {} inputs",
        requests.first().map_or(0, |seen| seen.inputs.len())
    );
    for seen in &requests {
        assert!(
            seen.inputs.len() <= indexing::EMBEDDING_BATCH_INPUTS,
            "request {} carried {} inputs",
            seen.number,
            seen.inputs.len()
        );
        assert!(
            seen.chars() <= indexing::EMBEDDING_BATCH_CHARS,
            "request {} carried {} characters",
            seen.number,
            seen.chars()
        );
    }
    let sent: usize = requests.iter().map(|seen| seen.inputs.len()).sum();
    assert_eq!(sent, CHUNKS, "every chunk is embedded exactly once");
}

#[tokio::test]
async fn vectors_stay_attached_to_their_chunks_across_batches() {
    let gateway = FakeGateway::start(|_| Reply::vectors());
    let work = tempfile::tempdir().expect("temp work folder");
    std::fs::write(work.path().join("rapport.txt"), numbered_document(CHUNKS)).expect("writes");

    let (_, index, _dir) = analyse(work.path(), &gateway.url).await;

    // The fake gateway answers each batch back to front and writes the paragraph number into
    // the vector, so a vector sitting on the wrong chunk shows as a number that does not match.
    let chunks = index
        .chunks_for_document("rapport.txt")
        .expect("reads chunks");
    assert_eq!(chunks.len(), CHUNKS);
    let mut checked = 0;
    for chunk in chunks {
        let expected = common::fake_gateway::vector_for(&chunk.text);
        assert_eq!(chunk.embedding, expected, "chunk {}", chunk.chunk_id);
        checked += 1;
    }
    assert_eq!(checked, CHUNKS);
}
