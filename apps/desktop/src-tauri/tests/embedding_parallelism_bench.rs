//! Does the AI machine embed faster with two requests in flight than with one?
//!
//! The Analyse pass sends its batches one after the other (`indexing::embed_chunks`). Whether two
//! requests in flight would finish sooner depends on the machine and on how Ollama is configured
//! (`OLLAMA_NUM_PARALLEL`), and it is not knowable from here: on a CPU-only stack the second
//! request may simply wait for the first. This prints the numbers so the answer is a measurement
//! (`docs/test-reports/knowledge-base-pass-1/00-baseline.md`, "Embedding parallelism").
//!
//! `#[ignore]`: it needs the real gateway and its embedding model, and takes a minute or two. It
//! sends **synthetic text only** - numbered filler sentences, nothing from any document - to the
//! gateway named by `ASSISTANT_CABINET_SERVER_URL` (default `http://127.0.0.1:8080`) and the
//! alias in `ASSISTANT_CABINET_EMBEDDING_ALIAS` (default `cabinet-embed`). It asserts nothing.
//!
//! ```text
//! cargo test --release --test embedding_parallelism_bench -- --ignored --nocapture
//! ```
//!
//! Run it from the workstation against the Mac mini at the practice, which is the pair whose
//! number matters, and once against the development machine for comparison.

use std::time::Instant;

use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::indexing::{EMBEDDING_BATCH_CHARS, EMBEDDING_BATCH_INPUTS};

/// Four batches: enough to see a difference, few enough to finish quickly on a CPU-only server.
const BATCHES: usize = 4;
const CHUNK_CHARS: usize = 1_000;

/// One batch of the size a real pass sends: `EMBEDDING_BATCH_INPUTS` chunks of about 1 000
/// characters, none identical to another batch's so no server-side cache can answer for it.
fn synthetic_batch(batch_number: usize) -> Vec<String> {
    (0..EMBEDDING_BATCH_INPUTS.min(EMBEDDING_BATCH_CHARS / CHUNK_CHARS))
        .map(|chunk| {
            let mut text = String::new();
            let mut sentence = 0usize;
            while text.chars().count() < CHUNK_CHARS {
                text.push_str(&format!(
                    "Phrase {sentence} du paragraphe {chunk} du lot {batch_number}, sans rapport avec \
                     aucun document. "
                ));
                sentence += 1;
            }
            text.chars().take(CHUNK_CHARS).collect()
        })
        .collect()
}

#[tokio::test]
#[ignore]
async fn measures_one_request_in_flight_against_two() {
    let server_url = std::env::var("ASSISTANT_CABINET_SERVER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let alias = std::env::var("ASSISTANT_CABINET_EMBEDDING_ALIAS")
        .unwrap_or_else(|_| "cabinet-embed".to_string());
    let gateway = GatewayClient::new().expect("builds a client");
    let deadlines = gateway.embedding_deadlines();

    // The model may still have to be loaded for the first request: keep that out of the numbers.
    let started = Instant::now();
    gateway
        .embed_batch(
            &server_url,
            &alias,
            &synthetic_batch(99),
            deadlines.first_batch,
        )
        .await
        .expect("the warm-up request succeeds: is the gateway up, and is the alias right?");
    println!("warm-up (model load included): {:?}", started.elapsed());

    let batches: Vec<Vec<String>> = (0..BATCHES).map(synthetic_batch).collect();
    let chunks_per_batch = batches[0].len();
    let chunks = BATCHES * chunks_per_batch;

    let serial_started = Instant::now();
    for batch in &batches {
        gateway
            .embed_batch(&server_url, &alias, batch, deadlines.batch)
            .await
            .expect("a serial request succeeds");
    }
    let serial = serial_started.elapsed();

    // The same batches, with different text so nothing is served twice, two in flight at a time.
    let other: Vec<Vec<String>> = (BATCHES..2 * BATCHES).map(synthetic_batch).collect();
    let parallel_started = Instant::now();
    for pair in other.chunks(2) {
        let first = gateway.embed_batch(&server_url, &alias, &pair[0], deadlines.batch);
        let second = gateway.embed_batch(&server_url, &alias, &pair[1], deadlines.batch);
        let (first, second) = tokio::join!(first, second);
        first.expect("a parallel request succeeds");
        second.expect("a parallel request succeeds");
    }
    let parallel = parallel_started.elapsed();

    println!("gateway {server_url}, alias {alias}");
    println!("{BATCHES} batches of {chunks_per_batch} chunks of about {CHUNK_CHARS} characters");
    println!(
        "  one in flight:  {:?} ({:.0} ms per chunk)",
        serial,
        serial.as_millis() as f64 / chunks as f64
    );
    println!(
        "  two in flight:  {:?} ({:.0} ms per chunk)",
        parallel,
        parallel.as_millis() as f64 / chunks as f64
    );
    println!(
        "  two in flight takes {:.2} times as long as one (below 1.00 is a gain, about 1.00 means \
         the server answers them one after the other)",
        parallel.as_secs_f64() / serial.as_secs_f64()
    );
}
