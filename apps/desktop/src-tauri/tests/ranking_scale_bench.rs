//! Lot 7 of the long-documents programme: the cost of one question at 5 000, 20 000 and 100 000 passages
//! of 768-number vectors, the way the ranking loaded them (`old`: every passage with its text and vector,
//! as `all_chunks` did) and the way it reads them now (`new`: the vectors only, then the text of the few
//! kept). A measure, not a test: `#[ignore]`.
//!
//! ```text
//! ACAI_BENCH_DB=<file> ACAI_BENCH_CHUNKS=100000 ACAI_BENCH_MODE=build cargo test --release --test ranking_scale_bench -- --ignored --nocapture
//! ACAI_BENCH_DB=<file> ACAI_BENCH_MODE=old python scripts/peak_memory.py -- <the test executable> --ignored --nocapture
//! ACAI_BENCH_DB=<file> ACAI_BENCH_MODE=new python scripts/peak_memory.py -- <the test executable> --ignored --nocapture
//! ```
//!
//! Building and querying are separate runs so the memory peak of a run is the question's, not the build's.

use std::time::Instant;

use assistant_cabinet_ai_lib::index_store::{cosine_similarity, IndexStore};
use assistant_cabinet_ai_lib::retrieval::{search_scoped_counted, RetrievalScope};

const DIMENSION: usize = 768;
const WORDS: [&str; 16] = [
    "invoice",
    "registry",
    "nightingale",
    "cabinet",
    "weekly",
    "supplier",
    "contract",
    "payment",
    "archive",
    "schedule",
    "hygiene",
    "waste",
    "storage",
    "delivery",
    "reminder",
    "balance",
];

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn unit(&mut self) -> f32 {
        (self.next() % 20_001) as f32 / 10_000.0 - 1.0
    }
}

fn env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

fn build(path: &std::path::Path, chunks: usize) {
    let _ = std::fs::remove_file(path);
    let store = IndexStore::open_at(path).expect("opens");
    let connection = rusqlite::Connection::open(store.path()).expect("second connection");
    let mut random = Lcg(11);
    connection.execute_batch("BEGIN").unwrap();
    for n in 0..chunks {
        let file = n / 40;
        let words = 120 + (random.next() % 40) as usize;
        let text: String = (0..words)
            .map(|_| WORDS[(random.next() % WORDS.len() as u64) as usize])
            .collect::<Vec<_>>()
            .join(" ");
        let vector: Vec<u8> = (0..DIMENSION)
            .flat_map(|_| random.unit().to_le_bytes())
            .collect();
        let id = format!("f{file:05}.txt#p{}#s{}", n % 40 / 5 + 1, n % 5 + 1);
        connection
            .execute(
                "INSERT INTO chunks (chunk_id, relative_path, page_number, section, text, embedding, origin, confidence)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'text_layer', NULL)",
                rusqlite::params![id, format!("f{file:05}.txt"), (n % 40 / 5 + 1) as i64, (n % 5 + 1) as i64, text, vector],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO chunks_fts (chunk_id, text) VALUES (?1, ?2)",
                rusqlite::params![id, text],
            )
            .unwrap();
    }
    connection.execute_batch("COMMIT").unwrap();
}

#[test]
#[ignore = "a measure: see the header"]
fn the_cost_of_one_question_at_scale() {
    let path = std::path::PathBuf::from(env("ACAI_BENCH_DB", "bench-ranking.sqlite3"));
    let mode = env("ACAI_BENCH_MODE", "new");
    if mode == "build" {
        let chunks: usize = env("ACAI_BENCH_CHUNKS", "5000").parse().unwrap();
        let started = Instant::now();
        build(&path, chunks);
        println!(
            "{{\"built\": {chunks}, \"seconds\": {:.1}}}",
            started.elapsed().as_secs_f64()
        );
        return;
    }
    let store = IndexStore::open_at(&path).expect("opens the built index");
    let mut random = Lcg(3);
    let mut times: Vec<f64> = Vec::new();
    let mut considered = 0;
    for _ in 0..12 {
        let text: String = (0..3)
            .map(|_| WORDS[(random.next() % WORDS.len() as u64) as usize])
            .collect::<Vec<_>>()
            .join(" ");
        let embedding: Vec<f32> = (0..DIMENSION).map(|_| random.unit()).collect();
        let started = Instant::now();
        if mode == "old" {
            let all = store.all_chunks().unwrap();
            considered = all.len();
            let mut scored: Vec<(usize, f32)> = Vec::new();
            for (position, chunk) in all.iter().enumerate() {
                let vector = cosine_similarity(&chunk.embedding, &embedding);
                if vector < 0.15 {
                    continue;
                }
                scored.push((position, vector));
            }
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let _kept: Vec<&str> = scored
                .iter()
                .take(6)
                .map(|(p, _)| all[*p].text.as_str())
                .collect();
        } else {
            let found =
                search_scoped_counted(&store, &text, &embedding, RetrievalScope::WholeFolder)
                    .unwrap();
            considered = found.chunks_considered;
        }
        times.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "{{\"mode\": \"{mode}\", \"chunks\": {considered}, \"median_ms\": {:.1}, \"max_ms\": {:.1}}}",
        times[times.len() / 2],
        times[times.len() - 1]
    );
}
