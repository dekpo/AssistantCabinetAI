//! The chain end to end on the sandbox: work folder -> index -> retrieval -> sourced answer.
//!
//! A tiny local stand-in for the gateway answers `/v1/embeddings` (a crude bag-of-words vector,
//! good enough combined with the real FTS5 lexical search) and `/v1/chat/completions` (a canned
//! streamed reply), so this test exercises the client pipeline without needing Ollama running.
//! Acceptance for milestone A (`docs/ROADMAP.md`): three questions, three sourced answers; a
//! fourth, off-topic question gets the refusal.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::{indexing, retrieval};
use serde_json::{json, Value};

const EMBEDDING_DIMENSIONS: usize = 256;

/// Starts a background thread serving `/v1/embeddings` and `/v1/chat/completions`, and returns
/// its base URL. Dropped automatically when the test ends (the thread is detached, which is
/// fine: the process exits with the test binary).
fn start_fake_gateway() -> String {
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
    let address = server.server_addr();
    let url = format!("http://{address}");

    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let payload: Value = serde_json::from_str(&body).unwrap_or(Value::Null);

            let response_body = match request.url() {
                "/v1/embeddings" => embeddings_response(&payload),
                "/v1/chat/completions" => chat_response(),
                _ => json!({}).to_string(),
            };
            let response = tiny_http::Response::from_string(response_body).with_status_code(200);
            let _ = request.respond(response);
        }
    });

    url
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
        .map(|(index, text)| json!({ "index": index, "embedding": bag_of_words_embedding(text) }))
        .collect();
    json!({
        "data": data,
        "model": "cabinet-embed",
        "usage": { "prompt_tokens": 0, "total_tokens": 0 }
    })
    .to_string()
}

fn chat_response() -> String {
    // One SSE delta, then the terminator - just enough for `GatewayClient::chat` to read a
    // complete answer.
    "data: {\"choices\":[{\"delta\":{\"content\":\"Reponse fondee sur les extraits fournis.\"}}]}\n\ndata: [DONE]\n\n".to_string()
}

/// A crude, deterministic embedding: hash each lowercase word into one of `EMBEDDING_DIMENSIONS`
/// buckets and count it. Not a real model, but real cosine similarity on top of it, combined
/// with the genuine FTS5 lexical search in `IndexStore`, is enough to tell an on-topic passage
/// from an off-topic one - which is all this fake stands in for.
const STOPWORDS: &[&str] = &[
    "le", "la", "les", "de", "des", "du", "un", "une", "et", "est", "a", "au", "aux", "en", "pour",
    "sur", "avec", "que", "qui", "quel", "quelle", "quels", "quelles", "ce", "ci", "depuis",
    "combien", "temps", "il", "elle", "d", "l", "s", "n",
];

fn bag_of_words_embedding(text: &str) -> Vec<f32> {
    let mut buckets = vec![0.0_f32; EMBEDDING_DIMENSIONS];
    for word in text.split(|c: char| !c.is_alphanumeric()) {
        let word = word.to_lowercase();
        if word.is_empty() || STOPWORDS.contains(&word.as_str()) {
            continue;
        }
        let mut hasher = DefaultHasher::new();
        word.hash(&mut hasher);
        let bucket = (hasher.finish() as usize) % EMBEDDING_DIMENSIONS;
        buckets[bucket] += 1.0;
    }
    let norm: f32 = buckets.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut buckets {
            *value /= norm;
        }
    }
    buckets
}

fn sandbox_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("fixtures")
        .join("gp-sandbox")
}

async fn ask(
    index: &IndexStore,
    gateway: &GatewayClient,
    server_url: &str,
    question: &str,
) -> Vec<retrieval::Evidence> {
    let query_vector = gateway
        .embed(server_url, "cabinet-embed", &[question.to_string()])
        .await
        .expect("embeds the question")
        .into_iter()
        .next()
        .unwrap_or_default();
    retrieval::search(index, question, &query_vector).expect("searches")
}

#[tokio::test]
async fn three_questions_get_sourced_answers_and_an_off_topic_one_is_refused() {
    let server_url = start_fake_gateway();
    let gateway = GatewayClient::new().expect("builds a client");
    let index_dir = tempfile::tempdir().expect("temp index dir");
    let mut index = IndexStore::open_at(&index_dir.path().join("index.sqlite3")).expect("opens");

    let summary = indexing::run(
        &mut index,
        &gateway,
        &server_url,
        "cabinet-embed",
        &sandbox_dir(),
    )
    .await
    .expect("indexes the sandbox");

    assert!(summary.indexed_files > 0, "at least one file was indexed");
    assert!(
        summary
            .empty_files
            .iter()
            .any(|path| path.contains("radiographie-scan")),
        "the scanned PDF is reported empty, not silently dropped"
    );

    // Three on-topic questions, each answerable from a distinct fictional document.
    let biology = ask(
        &index,
        &gateway,
        &server_url,
        "Quel est le taux d'HbA1c de Camille ?",
    )
    .await;
    assert!(
        !biology.is_empty(),
        "expected sourced evidence for the biology question"
    );
    assert!(biology
        .iter()
        .any(|item| item.relative_path.contains("biologie")));

    let endocrinology = ask(
        &index,
        &gateway,
        &server_url,
        "Quelle est la dose de metformine prescrite ?",
    )
    .await;
    assert!(
        !endocrinology.is_empty(),
        "expected sourced evidence for the endocrinology question"
    );
    assert!(endocrinology
        .iter()
        .any(|item| item.relative_path.contains("endocrinologie")));

    let neurology = ask(
        &index,
        &gateway,
        &server_url,
        "Depuis combien de temps Hugo a-t-il des cephalees ?",
    )
    .await;
    assert!(
        !neurology.is_empty(),
        "expected sourced evidence for the neurology question"
    );
    assert!(neurology
        .iter()
        .any(|item| item.relative_path.contains("neurologie")));

    // A fourth, off-topic question: nothing in the sandbox answers it, so retrieval must return
    // nothing rather than let the model guess.
    let off_topic = ask(
        &index,
        &gateway,
        &server_url,
        "Quelle est la capitale de l'Australie ?",
    )
    .await;
    assert!(
        off_topic.is_empty(),
        "an off-topic question must find no evidence"
    );
}
