//! KB lot 11 - what the knowledge base costs and what it saves, as the library grows from 30 to 1 000 documents
//! of every format, with deterministic fake vectors (the real stack, with the real embedding model and OCR, is
//! `knowledge_validation_real.rs`).
//!
//! `#[ignore]`: it writes up to a thousand files and takes minutes. It **prints** numbers and asserts none.
//!
//! ```text
//! python scripts/gen-knowledge-sandbox.py --documents 1000 --extra-rows 40 --out data/knowledge-sandbox-1000
//! ACAI_SANDBOX_DIR=data/knowledge-sandbox-1000 ACAI_TIERS=30,100,200,500,1000 \
//!   cargo test --release --test knowledge_validation_bench -- --ignored --nocapture
//! ```
//!
//! What it measures, per tier:
//!
//! - an Analyse with the knowledge base off, and the same with it on: the difference is the cost of reading
//!   names (the embedding is a fake, so the figure is the work done on the workstation: reading, chunking,
//!   writing, extracting);
//! - the size of the base: entities, aliases, mentions, rows in `kb_*`, the index file;
//! - a question, in each mode: reading the question and deciding (the knowledge lookup), then the search, over
//!   the whole folder ("tous") and over an explicit selection of every file but one (the shape that
//!   `chunks_for_document` makes quadratic, KBD-16).
//!
//! The generation of the answer, which dominates what the user waits for, is not here: the evidence is capped at
//! six passages whatever the mode, so the prompt is the same size.

mod common;

use std::collections::BTreeSet;
use std::time::Instant;

use assistant_cabinet_ai_lib::knowledge::reduce::TargetingPolicy;
use assistant_cabinet_ai_lib::knowledge::targeting::{QueryVocabulary, TargetingRequest};
use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;

use common::fake_gateway::vector_for;
use common::validation::*;

const PACKS: [&str; 2] = ["health-fr", "health-ch"];

fn fmt(v: Option<f64>) -> String {
    v.map(|v| format!("{v:.1}")).unwrap_or_else(|| "-".into())
}

fn count(connection: &rusqlite::Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap_or(0)
}

#[tokio::test]
#[ignore]
async fn the_library_grows_from_thirty_to_a_thousand_documents() {
    let truth = Truth::load();
    let tiers: Vec<usize> = std::env::var("ACAI_TIERS")
        .unwrap_or_else(|_| "30,100,200".to_string())
        .split(',')
        .filter_map(|t| t.trim().parse().ok())
        .collect();
    let vocabulary = QueryVocabulary::load("fr-FR", &PACKS).expect("loads the vocabulary");
    let mut report = String::from("### Cost and gain by size of the library (fake vectors; the generation is not measured)\n\n");
    report.push_str("| Documents | Read by the product | Analyse, base off (ms) | Analyse, base on (ms) | Cost of the base | Entities | Aliases | Mentions | Index file (MB) | Mode | Lookup median / p95 (ms) | Search 'tous' median / p95 (ms) | Search all-but-one median / p95 (ms) | Files searched, median |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");

    // two laboratories grown in step: one with the knowledge base off, one with it on
    let mut with_base: Option<Library> = None;
    let mut without_base: Option<Library> = None;
    for wanted in tiers {
        let tier = truth.tier_size(wanted);
        let (ocr, rasterizer) = perfect_ocr(&truth, tier);
        let stack = Stack {
            real: None,
            ocr: Some(&ocr),
            rasterizer: Some(&rasterizer),
        };
        match with_base.as_mut() {
            None => with_base = Some(library_on(&truth, tier, &PACKS, &stack).await),
            Some(lib) => lib.grow_to(&truth, tier, &stack).await,
        }
        match without_base.as_mut() {
            None => {
                let mut lab_off = library_off(&truth, tier, &stack).await;
                lab_off.lab.mode = KnowledgeMode::Off;
                without_base = Some(lab_off);
            }
            Some(lib) => lib.grow_to(&truth, tier, &stack).await,
        }
        let lib = with_base.as_ref().expect("a library");
        let off_ms = without_base.as_ref().map(|l| l.analyse_ms).unwrap_or(0);
        let on_ms = lib.analyse_ms;
        let connection = lib.lab.side();
        let (entities, aliases, mentions) = (
            count(&connection, "kb_entities"),
            count(&connection, "kb_aliases"),
            count(&connection, "kb_mentions"),
        );
        let index_mb = std::fs::metadata(lib.lab.index_path())
            .map(|m| m.len() as f64 / 1_048_576.0)
            .unwrap_or(0.0);

        // the questions: every third one that can be asked at this size and searches the documents
        let sample: Vec<&QuestionTruth> = truth
            .questions
            .iter()
            .filter(|q| {
                q.min_tier <= tier
                    && !matches!(
                        q.category.as_str(),
                        "data" | "leak" | "small_scope" | "follow_up"
                    )
            })
            .enumerate()
            .filter(|(i, _)| i % 3 == 0)
            .map(|(_, q)| q)
            .collect();
        let mut replay = Replay::new(lib, &vocabulary);
        // the synthetic vectors say nothing about meaning: never widen on a weak score, or every reduction is repeated
        replay.policy = TargetingPolicy {
            widen_below_score: 0.0,
            ..TargetingPolicy::default()
        };
        let everything_but_one: Vec<String> = {
            let all = lib.product_files.clone();
            all.into_iter().skip(1).collect()
        };
        for mode in [
            KnowledgeMode::Off,
            KnowledgeMode::Suggest,
            KnowledgeMode::Auto,
        ] {
            let (mut lookup, mut tous, mut but_one, mut searched) =
                (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for q in &sample {
                let embedding = vector_for(&q.text);
                let request = TargetingRequest::default();
                for (selection, bucket) in [
                    (None, &mut tous),
                    (Some(everything_but_one.as_slice()), &mut but_one),
                ] {
                    let asked = replay.ask(&q.text, mode, &request, selection, true);
                    if asked.plan.is_none() {
                        continue;
                    }
                    let started = Instant::now();
                    let found = replay.search(&asked, &q.text, &embedding);
                    let ms = started.elapsed().as_secs_f64() * 1000.0;
                    if found.is_some() {
                        bucket.push(ms);
                        if selection.is_none() {
                            lookup.push(
                                asked
                                    .plan
                                    .as_ref()
                                    .map(|p| p.knowledge_ms as f64)
                                    .unwrap_or(0.0),
                            );
                            searched.push(asked.searched().len() as f64);
                        }
                    }
                }
            }
            report.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {:.1} | {:?} | {} / {} | {} / {} | {} / {} | {} |\n",
                tier, lib.product_files.len(), off_ms, on_ms,
                if mode == KnowledgeMode::Off { format!("{:+.0} %", 100.0 * (on_ms as f64 - off_ms as f64) / off_ms.max(1) as f64) } else { String::new() },
                entities, aliases, mentions, index_mb, mode,
                fmt(median(lookup.clone())), fmt(percentile(lookup, 0.95)),
                fmt(median(tous.clone())), fmt(percentile(tous, 0.95)),
                fmt(median(but_one.clone())), fmt(percentile(but_one, 0.95)),
                fmt(median(searched)),
            ));
        }
        let unique: BTreeSet<&str> = lib.product_files.iter().map(String::as_str).collect();
        println!(
            "tier {tier}: {} documents, analyse off {off_ms} ms, on {on_ms} ms, {} entities",
            unique.len(),
            entities
        );
    }
    println!("{report}");
    write_report("bench.md", &report);
}

/// A laboratory analysed with the base switched off (the mode is read when the context is built).
async fn library_off(truth: &Truth, tier: usize, stack: &Stack<'_>) -> Library {
    use common::knowledge::Lab;
    let mut lab = Lab::new();
    lab.packs = PACKS.to_vec();
    lab.mode = KnowledgeMode::Off;
    let mut product_files = Vec::new();
    for doc in truth.docs_in(tier).filter(|d| d.product_reads()) {
        let from = truth.root.join("documents").join(&doc.path);
        let to = lab.work_path().join(&doc.path);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(from, to).unwrap();
        product_files.push(doc.path.clone());
    }
    let mut library = Library {
        lab,
        tier: 0,
        product_files,
        analyse_ms: 0,
        summary: None,
        passes: Vec::new(),
    };
    library.pass(tier, stack).await;
    library
}
