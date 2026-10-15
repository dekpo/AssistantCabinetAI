//! Lot 7 of the long-documents programme: the ranking no longer loads the text of every passage for every
//! question, and it must return exactly what it returned when it did. This test keeps the old algorithm as
//! a reference (every passage loaded, cosine plus the lexical bonus, one stable sort, the two caps) and
//! compares the passages, their order, their scores and the counts on a library, in every scope and at two
//! sets of limits.

use assistant_cabinet_ai_lib::chunking::Chunk;
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::index_store::{cosine_similarity, IndexStore, StoredChunk};
use assistant_cabinet_ai_lib::retrieval::{
    search_scoped_counted_with_terms, RetrievalLimits, RetrievalScope, MAX_PER_DOCUMENT_CHARS,
};

/// What `retrieval` uses as its floor (the reference must use the same one).
const MIN_VECTOR_SCORE: f32 = 0.15;
const WORDS: [&str; 24] = [
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
    "meeting",
    "report",
    "procedure",
    "inventory",
    "request",
    "approval",
    "notice",
    "summary",
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

    fn vector(&mut self) -> Vec<f32> {
        (0..16).map(|_| self.unit()).collect()
    }
}

fn library() -> (IndexStore, tempfile::TempDir, Vec<String>) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    let mut random = Lcg(7);
    let mut paths = Vec::new();
    for file in 0..12 {
        let path = format!("f{file:02}.txt");
        let mut chunks = Vec::new();
        let mut vectors = Vec::new();
        for n in 0..25 {
            let words = 12 + (random.next() % 90) as usize;
            let text: String = (0..words)
                .map(|_| WORDS[(random.next() % WORDS.len() as u64) as usize])
                .collect::<Vec<_>>()
                .join(" ");
            chunks.push(Chunk {
                chunk_id: format!("{path}#p{}#s{}", n / 5 + 1, n % 5 + 1),
                relative_path: path.clone(),
                page_number: n as u32 / 5 + 1,
                section: n as u32 % 5 + 1,
                text,
                index_text: None,
                origin: PageOrigin::TextLayer,
                confidence: None,
            });
            vectors.push(random.vector());
        }
        store
            .replace_document(&path, "hash", false, &chunks, &vectors, None, None)
            .unwrap();
        paths.push(path);
    }
    (store, dir, paths)
}

struct Expected {
    ids: Vec<String>,
    scores: Vec<f32>,
    considered: usize,
    best: f32,
}

fn reference(
    index: &IndexStore,
    query: &str,
    embedding: &[f32],
    scope: RetrievalScope<'_>,
    limits: RetrievalLimits,
) -> Expected {
    let (chunks, lexical): (Vec<StoredChunk>, Vec<String>) = match scope {
        RetrievalScope::WholeFolder => (
            index.all_chunks().unwrap(),
            index
                .search_lexical(query, limits.max_chunks)
                .unwrap()
                .into_iter()
                .map(|h| h.chunk_id)
                .collect(),
        ),
        RetrievalScope::File(path) => (
            index.chunks_for_document(path).unwrap(),
            index
                .search_lexical_in(query, limits.max_chunks, &[path.to_string()])
                .unwrap()
                .into_iter()
                .map(|h| h.chunk_id)
                .collect(),
        ),
        RetrievalScope::Files(paths) => (
            paths
                .iter()
                .flat_map(|p| index.chunks_for_document(p).unwrap())
                .collect(),
            index
                .search_lexical_in(query, limits.max_chunks, paths)
                .unwrap()
                .into_iter()
                .map(|h| h.chunk_id)
                .collect(),
        ),
        RetrievalScope::CurrentFolder(paths) => (
            index
                .all_chunks()
                .unwrap()
                .into_iter()
                .filter(|c| paths.contains(&c.relative_path))
                .collect(),
            index
                .search_lexical_in(query, limits.max_chunks, paths)
                .unwrap()
                .into_iter()
                .map(|h| h.chunk_id)
                .collect(),
        ),
    };
    let mut best = f32::NEG_INFINITY;
    let mut scored: Vec<(String, f32, usize)> = Vec::new();
    for chunk in &chunks {
        let vector_score = cosine_similarity(&chunk.embedding, embedding);
        best = best.max(vector_score);
        let bonus = if lexical.contains(&chunk.chunk_id) {
            0.2
        } else {
            0.0
        };
        if vector_score < MIN_VECTOR_SCORE && bonus == 0.0 {
            continue;
        }
        scored.push((
            chunk.chunk_id.clone(),
            vector_score + bonus,
            chunk.text.len(),
        ));
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut ids = Vec::new();
    let mut scores = Vec::new();
    let mut total = 0usize;
    for (id, score, bytes) in scored {
        if ids.len() >= limits.max_chunks {
            break;
        }
        if total + bytes > limits.max_chars {
            continue;
        }
        total += bytes;
        ids.push(id);
        scores.push(score);
    }
    Expected {
        ids,
        scores,
        considered: chunks.len(),
        best: if best.is_finite() { best } else { 0.0 },
    }
}

#[test]
fn the_lean_ranking_returns_what_the_loading_one_returned_in_every_scope() {
    let (index, _dir, paths) = library();
    let some: Vec<String> = vec![paths[2].clone(), paths[5].clone(), paths[9].clone()];
    let folder: Vec<String> = paths[..5].to_vec();
    let narrow = RetrievalLimits::BASELINE;
    let wide = RetrievalLimits {
        max_chunks: 30,
        max_chars: usize::MAX / 4,
        max_per_document_chars: MAX_PER_DOCUMENT_CHARS,
    };
    let tight = RetrievalLimits {
        max_chunks: 12,
        max_chars: 1_500,
        max_per_document_chars: MAX_PER_DOCUMENT_CHARS,
    };

    let mut random = Lcg(99);
    let mut compared = 0;
    for _ in 0..30 {
        let text: String = (0..3)
            .map(|_| WORDS[(random.next() % WORDS.len() as u64) as usize])
            .collect::<Vec<_>>()
            .join(" ");
        let embedding = random.vector();
        for limits in [narrow, wide, tight] {
            for scope in [
                RetrievalScope::WholeFolder,
                RetrievalScope::File("f03.txt"),
                RetrievalScope::Files(&some),
                RetrievalScope::CurrentFolder(&folder),
            ] {
                let expected = reference(&index, &text, &embedding, scope, limits);
                let found =
                    search_scoped_counted_with_terms(&index, &text, &[], &embedding, scope, limits)
                        .unwrap();
                let ids: Vec<String> = found.evidence.iter().map(|e| e.chunk_id.clone()).collect();
                let scores: Vec<f32> = found.evidence.iter().map(|e| e.score).collect();
                assert_eq!(ids, expected.ids, "{text:?} {scope:?}");
                assert_eq!(scores, expected.scores, "scores, {text:?} {scope:?}");
                assert_eq!(found.chunks_considered, expected.considered);
                assert_eq!(found.best_vector_score, expected.best);
                assert_eq!(found.unvectorised_chunks, 0);
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 360);
}

#[test]
fn a_passage_without_a_vector_is_ranked_by_its_words_alone_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let mut index = IndexStore::open_at(&dir.path().join("index.sqlite3")).unwrap();
    let make = |id: &str, text: &str| Chunk {
        chunk_id: id.into(),
        relative_path: "extra.txt".into(),
        page_number: 1,
        section: 1,
        text: text.into(),
        index_text: None,
        origin: PageOrigin::TextLayer,
        confidence: None,
    };
    let mut second = make("extra.txt#p1#s2", "something unrelated about invoices");
    second.section = 2;
    index
        .replace_document_pending(
            "extra.txt",
            "h",
            false,
            &[
                make("extra.txt#p1#s1", "zyxophone quartz without a vector yet"),
                second,
            ],
            &[None, Some(vec![1.0, 0.0])],
            None,
            None,
        )
        .unwrap();
    let found = search_scoped_counted_with_terms(
        &index,
        "zyxophone quartz",
        &[],
        &[0.0, 1.0],
        RetrievalScope::WholeFolder,
        RetrievalLimits::BASELINE,
    )
    .unwrap();
    assert_eq!(found.unvectorised_chunks, 1);
    assert!(
        found
            .evidence
            .iter()
            .any(|e| e.chunk_id == "extra.txt#p1#s1"),
        "found by its words alone"
    );
}
