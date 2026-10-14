//! Retrieval lab, phase 0: replay the exam without the generation model and write what each retrieval channel
//! returns, so that `scripts/retrieval_lab_score.py` can score it. Measurement code, not product code: it reads
//! the product through its public functions and changes nothing.
//!
//! `#[ignore]`: the real run embeds the library through the gateway and takes minutes.
//!
//! ```text
//! # plumbing check, fake vectors (lexical channels are real, vector channels meaningless)
//! ACAI_FAKE=1 ACAI_TIER=30 ACAI_LAB_LABEL=plumbing \
//!   cargo test --release --test retrieval_lab -- --ignored --nocapture
//! # the real baseline, through the gateway's embedding model
//! ACAI_GATEWAY=http://192.168.1.83:8080 ACAI_EMBED_ALIAS=assistant-embed ACAI_TIER=200 ACAI_LAB_LABEL=baseline \
//!   cargo test --release --test retrieval_lab -- --ignored --nocapture
//! ```
//!
//! Variables: `ACAI_GATEWAY`, `ACAI_EMBED_ALIAS` (default `assistant-embed`), `ACAI_TIER` (default 200),
//! `ACAI_FAKE=1`, `ACAI_SANDBOX_DIR`, `ACAI_LAB_LABEL` (default `run`), `ACAI_LAB_OUT` (default
//! `<repo>/data/retrieval-lab/<label>`, which git ignores).
//!
//! It writes, in the output folder: `analyse.json` (the pass, its split, the index size, the embedding model and
//! its dimension) and `run.jsonl` (one line per question and channel: the route, the time, and the ranked
//! passages with whether each one carries the planted fact).
//!
//! Channels:
//!
//! - `pipeline`: what the product sends today for an ordinary question (the folder router, the knowledge step
//!   with the base off, the search with the baseline limits): six passages at most.
//! - `hybrid30`: the same ranking (cosine plus the flat lexical bonus) read to a depth of 30 passages, with no
//!   routing, so recall@10 and recall@30 can be read.
//! - `vector30`: cosine alone, depth 30.
//! - `lexical30`: the application's own full-text query (an OR of every word of four letters or more), depth 30.
//! - `term_all`: the plain phrase search of `chunks_fts` for the six set sentences of the final test, every
//!   matching file in rank order (the offline check of `07-record-study-fixes-and-freeze.md`, section 5).

mod common;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::{cosine_similarity, StoredChunk};
use assistant_cabinet_ai_lib::knowledge::normalize::TitleSet;
use assistant_cabinet_ai_lib::knowledge::targeting::{QueryVocabulary, TargetingRequest};
use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;
use assistant_cabinet_ai_lib::retrieval::{
    self, Evidence, RetrievalLimits, RetrievalScope, MAX_PER_DOCUMENT_CHARS,
};
use common::fake_gateway::vector_for;
use common::validation::*;
use serde_json::{json, Value};

const PACKS: [&str; 2] = ["health-fr", "health-ch"];
const DEPTH: usize = 30;

/// The six set sentences of the final test and the phrase a plain word search looks for.
const SET_TERMS: [(&str, &str); 6] = [
    ("LAMal", "LAMal"),
    ("franchise annuelle", "franchise annuelle"),
    ("Genève", "Genève"),
    ("Lausanne", "Lausanne"),
    ("Nyon", "Nyon"),
    ("Annecy", "Annecy"),
];

fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

fn output_folder(label: &str) -> PathBuf {
    match std::env::var("ACAI_LAB_OUT") {
        Ok(path) => PathBuf::from(path),
        Err(_) => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("data")
            .join("retrieval-lab")
            .join(label),
    }
}

/// Lower case, accents and punctuation removed, white space collapsed: the way a planted fact is looked for in a
/// passage, so a line break or an accent never hides it.
fn squash(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let folded: String = text
        .nfd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .collect();
    folded
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn carries_fact(facts: &[String], text: &str) -> bool {
    !facts.is_empty() && {
        let text = squash(text);
        facts.iter().all(|fact| text.contains(&squash(fact)))
    }
}

struct Passage {
    path: String,
    page: u32,
    chunk_id: String,
    score: f64,
    fact: bool,
}

fn passage_json(passage: &Passage) -> Value {
    json!({
        "path": passage.path,
        "page": passage.page,
        "chunk_id": passage.chunk_id,
        "score": passage.score,
        "fact": passage.fact,
    })
}

fn from_evidence(evidence: &[Evidence], facts: &[String]) -> Vec<Passage> {
    evidence
        .iter()
        .map(|e| Passage {
            path: e.relative_path.clone(),
            page: e.page_number,
            chunk_id: e.chunk_id.clone(),
            score: f64::from(e.score),
            fact: carries_fact(facts, &e.text),
        })
        .collect()
}

fn line(id: &str, channel: &str, route: &str, ms: f64, passages: &[Passage]) -> String {
    json!({
        "id": id,
        "channel": channel,
        "route": route,
        "ms": (ms * 1000.0).round() / 1000.0,
        "results": passages.iter().map(passage_json).collect::<Vec<_>>(),
    })
    .to_string()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn replay_the_exam_without_the_model() {
    let truth = Truth::load();
    let label = env("ACAI_LAB_LABEL", "run");
    let fake = env("ACAI_FAKE", "0") == "1";
    let url = env("ACAI_GATEWAY", "http://127.0.0.1:8080");
    let alias = env("ACAI_EMBED_ALIAS", "assistant-embed");
    let tier = truth.tier_size(env("ACAI_TIER", "200").parse().unwrap_or(200));
    let out = output_folder(&label);
    std::fs::create_dir_all(&out).expect("creates the output folder");

    let client = GatewayClient::new().expect("a client");
    if !fake {
        let health = client
            .health(&url)
            .await
            .expect("the gateway answers; check the address");
        println!("gateway {url}: {}", health.status);
    }

    // --- the Analyse, timed and split
    let (ocr, rasterizer) = perfect_ocr(&truth, tier);
    let stack = Stack {
        real: if fake {
            None
        } else {
            Some(Real {
                client: GatewayClient::new().expect("a client"),
                url: url.clone(),
                alias: alias.clone(),
            })
        },
        ocr: Some(&ocr),
        rasterizer: Some(&rasterizer),
    };
    let library = library_on(&truth, tier, &PACKS, &stack).await;
    let index = &library.lab.index;
    let chunks = index.all_chunks().expect("reads the chunks");
    let by_id: HashMap<&str, &StoredChunk> =
        chunks.iter().map(|c| (c.chunk_id.as_str(), c)).collect();
    let dimension = chunks.first().map(|c| c.embedding.len()).unwrap_or(0);
    let index_bytes = std::fs::metadata(library.lab.index_path())
        .map(|m| m.len())
        .unwrap_or(0);
    let timings = library
        .summary
        .as_ref()
        .and_then(|s| s.timings.as_ref())
        .and_then(|t| serde_json::to_value(t).ok())
        .unwrap_or(Value::Null);
    let analyse = json!({
        "label": label,
        "fake_vectors": fake,
        "embedding_alias": if fake { "fake" } else { alias.as_str() },
        "dimension": dimension,
        "tier": tier,
        "documents_read": library.product_files.len(),
        "chunks": chunks.len(),
        "chunk_chars_total": chunks.iter().map(|c| c.text.chars().count()).sum::<usize>(),
        "index_bytes": index_bytes,
        "analyse_ms": library.analyse_ms,
        "timings": timings,
    });
    std::fs::write(
        out.join("analyse.json"),
        serde_json::to_string_pretty(&analyse).unwrap(),
    )
    .expect("writes analyse.json");
    println!("{}", serde_json::to_string_pretty(&analyse).unwrap());

    // --- the questions
    let vocabulary = QueryVocabulary::load("fr-FR", &PACKS).expect("loads the vocabulary");
    let _titles: TitleSet = vocabulary.titles.clone();
    let replay = Replay::new(&library, &vocabulary);
    let wide = RetrievalLimits {
        max_chunks: DEPTH,
        max_chars: usize::MAX / 4,
        max_per_document_chars: MAX_PER_DOCUMENT_CHARS,
    };
    let mut lines: Vec<String> = Vec::new();
    let mut skipped: BTreeMap<String, usize> = BTreeMap::new();
    let connection = library.lab.side();

    for q in truth
        .questions
        .iter()
        .filter(|q| q.min_tier <= tier && q.category != "data")
    {
        let selection: Option<Vec<String>> = match &q.selection.documents {
            Pick::Word(_) => None,
            Pick::Paths { paths } => Some(
                paths
                    .iter()
                    .filter(|p| library.product_files.contains(p))
                    .cloned()
                    .collect(),
            ),
        };
        let with_workbooks = !matches!(&q.selection.workbooks, Pick::Word(w) if w == "none");
        let request = TargetingRequest::default();
        let first = replay.ask(
            &q.text,
            KnowledgeMode::Off,
            &request,
            selection.as_deref(),
            with_workbooks,
        );
        if matches!(first.route, RouteKind::Folder | RouteKind::NothingSelected) {
            *skipped.entry(first.route.code().to_string()).or_default() += 1;
            continue;
        }
        let route = first.route.code();
        let facts = &q.expect.contains;

        // the question is embedded once, for every channel
        let embed_started = Instant::now();
        let embedding = if fake {
            vector_for(&q.text)
        } else {
            match client
                .embed(&url, &alias, std::slice::from_ref(&q.text))
                .await
            {
                Ok(vectors) => vectors.into_iter().next().unwrap_or_default(),
                Err(error) => {
                    println!("embedding failed for {}: {error:?}", q.id);
                    continue;
                }
            }
        };
        let embed_ms = embed_started.elapsed().as_secs_f64() * 1000.0;
        lines.push(
            json!({"id": q.id, "channel": "embed", "route": route, "ms": embed_ms, "results": []})
                .to_string(),
        );

        let allowed: Option<HashSet<&str>> = selection
            .as_ref()
            .map(|paths| paths.iter().map(String::as_str).collect());
        let permitted = |path: &str| allowed.as_ref().map_or(true, |set| set.contains(path));

        // pipeline: the product's own path, six passages at most
        let started = Instant::now();
        let searched = replay.search(&first, &q.text, &embedding);
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let pipeline = searched
            .as_ref()
            .map(|s| from_evidence(&s.outcome.evidence, facts))
            .unwrap_or_default();
        lines.push(line(&q.id, "pipeline", route, ms, &pipeline));

        // hybrid30: the same ranking, deeper
        let started = Instant::now();
        let scope = match &selection {
            Some(paths) => RetrievalScope::Files(paths),
            None => RetrievalScope::WholeFolder,
        };
        let outcome = retrieval::search_scoped_counted_with_terms(
            index,
            &q.text,
            &[],
            &embedding,
            scope,
            wide,
        )
        .expect("searches");
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        lines.push(line(
            &q.id,
            "hybrid30",
            route,
            ms,
            &from_evidence(&outcome.evidence, facts),
        ));

        // vector30: cosine alone
        let started = Instant::now();
        let mut scored: Vec<(f32, &StoredChunk)> = chunks
            .iter()
            .filter(|c| permitted(&c.relative_path))
            .map(|c| (cosine_similarity(&c.embedding, &embedding), c))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let vector: Vec<Passage> = scored
            .iter()
            .take(DEPTH)
            .map(|(score, c)| Passage {
                path: c.relative_path.clone(),
                page: c.page_number,
                chunk_id: c.chunk_id.clone(),
                score: f64::from(*score),
                fact: carries_fact(facts, &c.text),
            })
            .collect();
        lines.push(line(&q.id, "vector30", route, ms, &vector));

        // lexical30: the application's own full-text query
        let started = Instant::now();
        let hits = index
            .search_lexical(&q.text, chunks.len().max(DEPTH))
            .expect("searches the words");
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let lexical: Vec<Passage> = hits
            .iter()
            .filter_map(|hit| by_id.get(hit.chunk_id.as_str()).map(|c| (hit.rank, *c)))
            .filter(|(_, c)| permitted(&c.relative_path))
            .take(DEPTH)
            .map(|(rank, c)| Passage {
                path: c.relative_path.clone(),
                page: c.page_number,
                chunk_id: c.chunk_id.clone(),
                score: -rank,
                fact: carries_fact(facts, &c.text),
            })
            .collect();
        lines.push(line(&q.id, "lexical30", route, ms, &lexical));

        // term_all: the plain phrase search, every matching passage, for the six set sentences
        let set_term = SET_TERMS
            .iter()
            .find(|(word, _)| q.category == "term" && q.text.contains(word));
        if let Some((_, phrase)) = set_term {
            let started = Instant::now();
            let expression = format!("\"{}\"", phrase.replace('"', ""));
            let mut statement = connection
                .prepare(
                    "SELECT chunk_id FROM chunks_fts WHERE chunks_fts MATCH ?1 ORDER BY bm25(chunks_fts)",
                )
                .expect("prepares the phrase search");
            let ids: Vec<String> = statement
                .query_map([expression], |row| row.get::<_, String>(0))
                .expect("runs the phrase search")
                .filter_map(Result::ok)
                .collect();
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            let phrase_hits: Vec<Passage> = ids
                .iter()
                .filter_map(|id| by_id.get(id.as_str()))
                .filter(|c| permitted(&c.relative_path))
                .map(|c| Passage {
                    path: c.relative_path.clone(),
                    page: c.page_number,
                    chunk_id: c.chunk_id.clone(),
                    score: 0.0,
                    fact: false,
                })
                .collect();
            lines.push(line(&q.id, "term_all", route, ms, &phrase_hits));
        }
    }

    std::fs::write(out.join("run.jsonl"), lines.join("\n")).expect("writes run.jsonl");
    println!(
        "{} lines written to {}; questions answered by the folder router and left out: {:?}",
        lines.len(),
        out.display(),
        skipped
    );
}
