//! Measurement, not a product test: how much of the exact answer an approximate nearest-neighbour index
//! (HNSW) returns on a library of 100 000 passages and more, how long it takes to build, and how much faster
//! it answers than the exact scan. The vectors are **fake** (random, in clusters, 768 dimensions like the
//! embedding model): they say how the graph behaves on this many points, not how real documents are
//! retrieved, and the recall on real vectors is to be measured again on a real library of that size.
//!
//! Run (release, a few minutes):
//! `ACAI_ANN_N=120000 ACAI_ANN_EF_CONSTRUCTION=100 ACAI_ANN_EF_SEARCH=100 cargo test --release --test ann_recall -- --ignored --nocapture`
//! Variables: `ACAI_ANN_N` (passages, default 120000), `ACAI_ANN_DIM` (default 768), `ACAI_ANN_QUERIES`
//! (default 300), `ACAI_ANN_SPREAD` (how far a passage is from the centre of its group, in tenths of the
//! distance between groups; default 3, so groups are well apart: 20 makes them overlap), `ACAI_ANN_EF_CONSTRUCTION` (default 100), `ACAI_ANN_EF_SEARCH` (default 100).

use std::time::Instant;

use instant_distance::{Builder, Point, Search};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Roughly normal (sum of uniforms), mean 0, standard deviation 1.
    fn normal(&mut self) -> f32 {
        let mut total = 0.0f32;
        for _ in 0..12 {
            total += (self.next() >> 40) as f32 / (1u64 << 24) as f32;
        }
        total - 6.0
    }
}

#[derive(Clone)]
struct Vector(Vec<f32>);

impl Point for Vector {
    /// 1 - cosine, the vectors being unit length.
    fn distance(&self, other: &Self) -> f32 {
        1.0 - dot(&self.0, &other.0)
    }
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn unit(mut values: Vec<f32>) -> Vec<f32> {
    let norm = values.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-9);
    values.iter_mut().for_each(|v| *v /= norm);
    values
}

fn env(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn exact_top(points: &[Vector], query: &[f32], count: usize) -> Vec<usize> {
    let mut scored: Vec<(f32, usize)> = points
        .iter()
        .enumerate()
        .map(|(index, point)| (dot(&point.0, query), index))
        .collect();
    scored.select_nth_unstable_by(count, |a, b| b.0.partial_cmp(&a.0).unwrap());
    scored.truncate(count);
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    scored.into_iter().map(|(_, index)| index).collect()
}

#[test]
#[ignore]
fn recall_of_an_hnsw_index_against_the_exact_search() {
    let n = env("ACAI_ANN_N", 120_000);
    let dim = env("ACAI_ANN_DIM", 768);
    let queries = env("ACAI_ANN_QUERIES", 300);
    let ef_construction = env("ACAI_ANN_EF_CONSTRUCTION", 100);
    let ef_search = env("ACAI_ANN_EF_SEARCH", 100);
    let spread = env("ACAI_ANN_SPREAD", 3) as f32 / 10.0;
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);

    // Passages of a library come in groups (a document, a topic): clusters, with a spread inside each.
    let clusters = n / 60;
    let centres: Vec<Vec<f32>> = (0..clusters)
        .map(|_| unit((0..dim).map(|_| rng.normal()).collect()))
        .collect();
    let points: Vec<Vector> = (0..n)
        .map(|index| {
            let centre = &centres[index % clusters];
            Vector(unit(
                centre
                    .iter()
                    .map(|c| c + spread * rng.normal() / (dim as f32).sqrt())
                    .collect(),
            ))
        })
        .collect();
    // A question is close to some passage of the library, not identical to it.
    let questions: Vec<Vec<f32>> = (0..queries)
        .map(|n| {
            let base = &points[(n * 7919) % points.len()].0;
            unit(
                base.iter()
                    .map(|b| b + 0.25 * rng.normal() / (dim as f32).sqrt())
                    .collect(),
            )
        })
        .collect();

    let started = Instant::now();
    let values: Vec<u32> = (0..n as u32).collect();
    let map = Builder::default()
        .ef_construction(ef_construction)
        .ef_search(ef_search)
        .seed(7)
        .build(points.clone(), values);
    let build_seconds = started.elapsed().as_secs_f64();

    let mut exact_ms = Vec::new();
    let mut ann_ms = Vec::new();
    let mut recall10 = 0.0f64;
    let mut recall30 = 0.0f64;
    let mut search = Search::default();
    for question in &questions {
        let started = Instant::now();
        let truth = exact_top(&points, question, 30);
        exact_ms.push(started.elapsed().as_secs_f64() * 1000.0);

        let query = Vector(question.clone());
        let started = Instant::now();
        let found: Vec<usize> = map
            .search(&query, &mut search)
            .take(30)
            .map(|item| *item.value as usize)
            .collect();
        ann_ms.push(started.elapsed().as_secs_f64() * 1000.0);

        let hits = |count: usize| {
            truth[..count]
                .iter()
                .filter(|index| found.iter().take(count).any(|f| f == *index))
                .count() as f64
                / count as f64
        };
        recall10 += hits(10);
        recall30 += hits(30);
    }
    let median = |values: &mut Vec<f64>| {
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        values[values.len() / 2]
    };
    println!(
        "N={n} dim={dim} queries={queries} spread={spread} ef_construction={ef_construction} ef_search={ef_search} (fake vectors)"
    );
    println!("build: {build_seconds:.1} s");
    println!(
        "recall@10: {:.4}   recall@30: {:.4}",
        recall10 / queries as f64,
        recall30 / queries as f64
    );
    println!(
        "median per question: exact {:.1} ms, approximate {:.2} ms",
        median(&mut exact_ms),
        median(&mut ann_ms)
    );
    println!(
        "vectors alone: {:.0} MB; the graph keeps up to 64 neighbours of 4 bytes per passage on its first layer: about {:.0} MB",
        (n * dim * 4) as f64 / 1e6,
        (n * 64 * 4) as f64 / 1e6
    );
}
