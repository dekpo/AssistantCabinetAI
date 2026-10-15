//! Retrieval lab, phase 0: replay the exam without the generation model and write what each retrieval channel
//! returns, so that `scripts/retrieval_lab_score.py` can score it. Measurement code, not product code: it reads
//! the product through its public functions and changes nothing.
//!
//! `#[ignore]`: the real run embeds the library through the gateway and takes minutes.
//!
//! ```text
//! # plumbing check, fake vectors (the vector channels are meaningless)
//! ACAI_FAKE=1 ACAI_TIER=30 ACAI_LAB_LABEL=plumbing \
//!   cargo test --release --test retrieval_lab -- --ignored --nocapture
//! # the real baseline, through the gateway's embedding model
//! ACAI_GATEWAY=http://192.168.1.83:8080 ACAI_EMBED_ALIAS=assistant-embed ACAI_TIER=200 ACAI_LAB_LABEL=baseline \
//!   cargo test --release --test retrieval_lab -- --ignored --nocapture
//! ```
//!
//! Variables: `ACAI_GATEWAY`, `ACAI_EMBED_ALIAS` (default `assistant-embed`), `ACAI_TIER` (default 200),
//! `ACAI_FAKE=1`, `ACAI_SANDBOX_DIR`, `ACAI_LAB_LABEL` (default `run`), `ACAI_LAB_OUT` (default
//! `<repo>/data/retrieval-lab/<label>`, which git ignores), `ACAI_LAB_ANALYSE_ONLY=1` (write `analyse.json`
//! and `finder.json`, skip the questions: the long-document baselines of `docs/LONG-DOCUMENTS.md`).
//!
//! It writes, in the output folder: `analyse.json` (the pass, its split, the index size, the embedding model and
//! its dimension) and `run.jsonl` (one line per question and channel: the route, the time, and the ranked
//! passages with whether each one carries the planted fact).
//!
//! Channels:
//!
//! - `pipeline`: what the product sends today for an ordinary question (the folder router, the knowledge step
//!   with the base off, the search with the baseline limits): six passages at most.
//! - `hybrid30`: the same ranking (the meaning, the named entities, the identifiers and facts) read to a depth
//!   of 30 passages, with no routing, so recall@10 and recall@30 can be read.
//! - `vector30`: cosine alone, depth 30.
//! - `term_all`: the plain phrase search of `chunks_fts` for the six set sentences of the final test, every
//!   matching file in rank order (the offline check of `07-record-study-fixes-and-freeze.md`, section 5).

mod common;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use assistant_cabinet_ai_lib::discovery::discover_all;
use assistant_cabinet_ai_lib::file_profile::build::Domain;
use assistant_cabinet_ai_lib::finder::{find, FinderQuery, FinderScope, InventoryFile};
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

fn line(
    index: &assistant_cabinet_ai_lib::index_store::IndexStore,
    id: &str,
    channel: &str,
    route: &str,
    ms: f64,
    passages: &[Passage],
) -> String {
    // The title of the section each passage starts in, when the document has a structure: scored as
    // `section@10` by `scripts/retrieval_lab_score.py`.
    let ids: Vec<String> = passages.iter().map(|p| p.chunk_id.clone()).collect();
    let labels = index.sections_of_chunks(&ids).unwrap_or_default();
    // The page where each passage ends: past its first page when it runs over a page break, scored as
    // `page@10` on the whole range.
    let ends = index.last_pages_of_chunks(&ids).unwrap_or_default();
    let results: Vec<Value> = passages
        .iter()
        .map(|p| {
            let mut value = passage_json(p);
            value["last_page"] =
                json!(ends.get(&p.chunk_id).copied().unwrap_or(p.page).max(p.page));
            if let Some(section) = labels.get(&p.chunk_id).filter(|s| !s.title.is_empty()) {
                value["section_label"] = json!(section.title);
            }
            value
        })
        .collect();
    json!({
        "id": id,
        "channel": channel,
        "route": route,
        "ms": (ms * 1000.0).round() / 1000.0,
        "results": results,
    })
    .to_string()
}

/// The assembly switches of one replay: `cap1`, `cap2`, `neighbours`, `edges` (any of them).
fn assembly_options_from(flags: &str) -> assistant_cabinet_ai_lib::assembly::AssemblyOptions {
    let mut options = assistant_cabinet_ai_lib::assembly::AssemblyOptions::OFF;
    for flag in flags.split(',').map(str::trim) {
        match flag {
            "cap1" => options.per_section_cap = Some(1),
            "cap2" => options.per_section_cap = Some(2),
            "neighbours" => options.neighbours = true,
            "edges" => options.edges_first = true,
            "pin" => options.pin_identifier = true,
            "facts" => options.facts = true,
            _ => {}
        }
    }
    options
}

/// `ACAI_ASSEMBLY_SETS="off;pin;pin,facts"` replays the questions once per set over the same analysed index and
/// writes each to `<label>-<set>`; without it one replay runs with `ACAI_ASSEMBLY`.
fn assembly_sets() -> Vec<(String, assistant_cabinet_ai_lib::assembly::AssemblyOptions)> {
    match std::env::var("ACAI_ASSEMBLY_SETS") {
        Ok(sets) if !sets.trim().is_empty() => sets
            .split(';')
            .map(|set| (set.trim().replace(',', "+"), assembly_options_from(set)))
            .collect(),
        _ => vec![(String::new(), assembly_options_from(&env("ACAI_ASSEMBLY", "off")))],
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn replay_the_exam_without_the_model() {
    let truth = Truth::load();
    // The chunker for long documents is measured at other sizes by setting these (measurement only).
    if ["ACAI_CHUNK_MAX", "ACAI_CHUNK_OVERLAP", "ACAI_CHUNK_PREFIX"]
        .iter()
        .any(|name| std::env::var(name).is_ok())
    {
        use assistant_cabinet_ai_lib::chunking_long::{
            set_long_options_for_measurement, LongOptions,
        };
        let defaults = LongOptions::default();
        set_long_options_for_measurement(Some(LongOptions {
            max_chars: env("ACAI_CHUNK_MAX", &defaults.max_chars.to_string())
                .parse()
                .unwrap_or(defaults.max_chars),
            overlap_percent: env("ACAI_CHUNK_OVERLAP", &defaults.overlap_percent.to_string())
                .parse()
                .unwrap_or(defaults.overlap_percent),
            prefix: env("ACAI_CHUNK_PREFIX", "1") != "0",
        }));
    }
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
    let profile_ms = library
        .summary
        .as_ref()
        .and_then(|s| s.profile.as_ref())
        .map(|p| p.elapsed_ms);
    // The dates and amounts the Analyse wrote as facts, and what writing them all again costs (the one
    // transaction the profile pass uses), to compare with the Analyse itself.
    let (facts_count, facts_ms) = {
        let side = library.lab.side();
        let vocabulary = assistant_cabinet_ai_lib::facts::shipped_vocabulary();
        let started = Instant::now();
        side.execute_batch("BEGIN").expect("begins");
        for path in &library.product_files {
            assistant_cabinet_ai_lib::facts::write(&side, path, vocabulary).expect("writes the facts");
        }
        side.execute_batch("COMMIT").expect("commits");
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let count: i64 = side
            .query_row("SELECT COUNT(*) FROM ld_facts", [], |row| row.get(0))
            .unwrap_or(0);
        (count, ms)
    };
    let analyse = json!({
        "facts": facts_count,
        "facts_ms": facts_ms,
        "profile_ms": profile_ms,
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

    // --- the finder over the profile tables, timed: a word and the whole folder, five runs each
    let inventory: Vec<InventoryFile> = discover_all(library.lab.work_path())
        .iter()
        .map(InventoryFile::from)
        .collect();
    let side = library.lab.side();
    let mut probes = Vec::new();
    for text in SET_TERMS.iter().map(|(_, term)| *term).chain([""]) {
        let query = FinderQuery {
            text: Some(text.to_string()),
            limit: Some(10_000),
            ..FinderQuery::default()
        };
        let mut times = Vec::new();
        let mut files = 0;
        for _ in 0..5 {
            let started = Instant::now();
            let result = find(
                &side,
                Domain::Documents,
                &inventory,
                &query,
                &FinderScope::WholeFolder,
            )
            .expect("the finder answers");
            times.push(started.elapsed().as_secs_f64() * 1000.0);
            files = result.total;
        }
        probes.push(json!({
            "query": if text.is_empty() { "(all files)" } else { text },
            "files": files,
            "ms_median": median(times),
        }));
    }
    std::fs::write(
        out.join("finder.json"),
        serde_json::to_string_pretty(&json!({"inventory": inventory.len(), "probes": probes}))
            .unwrap(),
    )
    .expect("writes finder.json");

    // `ACAI_LAB_ANALYSE_ONLY=1`: stop here, so the memory peak measured around this process is the Analyse's
    // (plus loading the chunks above), not the questions'.
    if env("ACAI_LAB_ANALYSE_ONLY", "0") == "1" {
        return;
    }

    // --- the questions
    let vocabulary = QueryVocabulary::load("fr-FR", &PACKS).expect("loads the vocabulary");
    let _titles: TitleSet = vocabulary.titles.clone();
    let replay = Replay::new(&library, &vocabulary);
    let wide = RetrievalLimits {
        max_chunks: DEPTH,
        max_chars: usize::MAX / 4,
        max_per_document_chars: MAX_PER_DOCUMENT_CHARS,
    };
    let connection = library.lab.side();
    let mut embeddings: HashMap<String, Vec<f32>> = HashMap::new();
    let rerank_wanted = env("ACAI_RERANK", "0") == "1";

    for (set_name, assembly) in assembly_sets() {
    assistant_cabinet_ai_lib::assembly::set_assembly_options_for_measurement(Some(assembly));
    let out = if set_name.is_empty() {
        out.clone()
    } else {
        let folder = output_folder(&format!("{label}-{set_name}"));
        std::fs::create_dir_all(&folder).expect("creates the output folder");
        std::fs::copy(out.join("analyse.json"), folder.join("analyse.json")).ok();
        folder
    };
    let mut lines: Vec<String> = Vec::new();
    let mut skipped: BTreeMap<String, usize> = BTreeMap::new();

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
        } else if let Some(known) = embeddings.get(&q.id) {
            known.clone()
        } else {
            match client
                .embed(&url, &alias, std::slice::from_ref(&q.text))
                .await
            {
                Ok(vectors) => {
                    let vector = vectors.into_iter().next().unwrap_or_default();
                    embeddings.insert(q.id.clone(), vector.clone());
                    vector
                }
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
        let permitted = |path: &str| allowed.as_ref().is_none_or(|set| set.contains(path));

        // pipeline: the product's own path, six passages at most
        let started = Instant::now();
        let searched = replay.search(&first, &q.text, &embedding);
        let ms = started.elapsed().as_secs_f64() * 1000.0;
        let pipeline = searched
            .as_ref()
            .map(|s| from_evidence(&s.outcome.evidence, facts))
            .unwrap_or_default();
        lines.push(line(index, &q.id, "pipeline", route, ms, &pipeline));

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
        let hybrid = from_evidence(&outcome.evidence, facts);
        lines.push(line(index, &q.id, "hybrid30", route, ms, &hybrid));

        // rerank6 / rerank30: the first thirty candidates put in order by the gateway's reranker
        // (`ACAI_RERANK=1`). `rerank6` is what the model would read; `rerank30` the whole new order.
        // A question that names a file, or asks about every document, is not reranked (the product's rule,
        // `commands.rs`): the passage does not hold the file's name, so the reranker ranks it worse. Its
        // lines are the unreranked ones, so every family is scored on the same questions.
        let reranked = !matches!(first.route, RouteKind::NamedFile | RouteKind::EveryDocument);
        if rerank_wanted && !fake && !reranked {
            lines.push(line(index, &q.id, "rerank6", route, 0.0, &pipeline));
            lines.push(line(index, &q.id, "rerank30", route, 0.0, &hybrid));
        }
        if rerank_wanted && !fake && reranked {
            let candidates = retrieval::search_scoped_counted_with_terms(
                index,
                &q.text,
                &[],
                &embedding,
                scope,
                retrieval::limits_for_rerank(RetrievalLimits::BASELINE),
            )
            .expect("searches")
            .evidence;
            let texts: Vec<String> = candidates.iter().map(|c| c.text.clone()).collect();
            let started = Instant::now();
            match client.rerank(&url, &q.text, &texts).await {
                Ok(ranking) => {
                    let ms = started.elapsed().as_secs_f64() * 1000.0;
                    let order: Vec<(usize, f32)> =
                        ranking.iter().map(|r| (r.index, r.score)).collect();
                    let six = retrieval::cut_after_rerank(
                        candidates.clone(),
                        &order,
                        RetrievalLimits::BASELINE,
                    );
                    lines.push(line(index, &q.id, "rerank6", route, ms, &from_evidence(&six, facts)));
                    let all = retrieval::cut_after_rerank(
                        candidates,
                        &order,
                        RetrievalLimits {
                            max_chunks: DEPTH,
                            max_chars: usize::MAX / 4,
                            max_per_document_chars: MAX_PER_DOCUMENT_CHARS,
                        },
                    );
                    lines.push(line(index, &q.id, "rerank30", route, ms, &from_evidence(&all, facts)));
                }
                Err(error) => println!("rerank failed for {}: {error:?}", q.id),
            }
        }

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
        lines.push(line(index, &q.id, "vector30", route, ms, &vector));

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
            lines.push(line(index, &q.id, "term_all", route, ms, &phrase_hits));
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
    assistant_cabinet_ai_lib::assembly::set_assembly_options_for_measurement(None);
}
