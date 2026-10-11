//! Measures what the knowledge base adds to a question, and what it can save: the lookup (reading
//! the question, resolving its names, deciding), and the search with and without the reduction, on
//! a synthetic corpus of 10 to 1 000 documents with deterministic fake vectors, plus the lookup alone
//! against a base of 10 000 entities.
//!
//! `#[ignore]`: it writes a thousand files and is slow by design. Run explicitly:
//!
//! ```text
//! cargo test --release --test knowledge_targeting_bench -- --ignored --nocapture
//! ```
//!
//! It **prints** numbers and asserts none. The band to read is 20 to 50 files, the size of a real
//! selection; the larger sizes show how it scales. The vectors are fake, so what the search time
//! measures is the work of scoring and sorting the chunks of the scope, which is exactly what a
//! reduction removes. The generation time, which dominates a real answer, is not measured here.

mod common;

use std::time::Instant;

use assistant_cabinet_ai_lib::knowledge::reduce::{SideInput, TargetingPolicy};
use assistant_cabinet_ai_lib::knowledge::store as kb;
use assistant_cabinet_ai_lib::knowledge::targeting::{self, DocumentPlan, TargetingRequest};
use assistant_cabinet_ai_lib::knowledge::{
    EntityStatus, KnowledgeMode, NewAlias, NewEntity, Origin,
};

use common::corpus::{doc, embedding_of, paths_of};
use common::knowledge::Lab;

const SIZES: [usize; 7] = [10, 20, 30, 50, 100, 500, 1000];
const ROUNDS: usize = 15;

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values[values.len() / 2]
}

/// `files` documents: one in twenty names Dr Jean Dupont, the rest hold no name. Each has three
/// chunks' worth of text so the search has something to score.
fn corpus(files: usize) -> Vec<(String, String)> {
    (0..files)
        .map(|index| {
            let body = if index % 20 == 3 {
                "Docteur Jean Dupont recommande un suivi regulier. "
            } else {
                "Note interne sur le materiel de bureau et les fournitures. "
            };
            (
                format!("fichier-{index:04}.txt"),
                doc(index as u32 % 9000, &body.repeat(30 + index % 7)),
            )
        })
        .collect()
}

#[tokio::test]
#[ignore]
async fn lookup_and_search_with_and_without_the_reduction() {
    println!(
        "{:>6} {:>10} {:>12} {:>12} {:>16} {:>16}",
        "files", "lookup ms", "search full", "search cut", "chunks full", "chunks cut"
    );
    for size in SIZES {
        let mut lab = Lab::new();
        lab.packs = vec!["health-fr"];
        for (name, text) in corpus(size) {
            lab.write(&name, &text);
        }
        lab.analyse().await;
        let vocabulary = lab.vocabulary();
        let documents = lab.documents();
        let paths = paths_of(&documents);
        let policy = TargetingPolicy {
            min_scope_files: 1,
            max_candidates: 200,
            ..TargetingPolicy::default()
        };
        let question = "Que dit Jean Dupont du suivi ?";
        let embedding = embedding_of("para-0003 Docteur Jean Dupont");

        let (mut lookups, mut full, mut cut) = (Vec::new(), Vec::new(), Vec::new());
        let (mut chunks_full, mut chunks_cut) = (0, 0);
        for _ in 0..ROUNDS {
            let started = Instant::now();
            let plan = lab.plan(
                &vocabulary,
                KnowledgeMode::Auto,
                &TargetingRequest::default(),
                question,
                SideInput::new(&documents),
                SideInput::new(&[]),
                policy,
            );
            lookups.push(started.elapsed().as_secs_f64() * 1000.0);

            let kind = DocumentPlan::WholeFolder {
                narrowed: true,
                scoped_paths: &paths,
                present_paths: &paths,
            };
            let started = Instant::now();
            let whole = lab.plan(
                &vocabulary,
                KnowledgeMode::Off,
                &TargetingRequest::default(),
                question,
                SideInput::new(&documents),
                SideInput::new(&[]),
                policy,
            );
            let all = targeting::search_documents(&lab.index, &whole, kind, question, &embedding)
                .expect("searches");
            full.push(started.elapsed().as_secs_f64() * 1000.0);
            chunks_full = all.outcome.chunks_considered;

            let started = Instant::now();
            let reduced =
                targeting::search_documents(&lab.index, &plan, kind, question, &embedding)
                    .expect("searches");
            cut.push(started.elapsed().as_secs_f64() * 1000.0);
            chunks_cut = reduced.outcome.chunks_considered;
        }
        println!(
            "{:>6} {:>10.2} {:>12.2} {:>12.2} {:>16} {:>16}",
            size,
            median(lookups),
            median(full),
            median(cut),
            chunks_full,
            chunks_cut
        );
    }
}

#[tokio::test]
#[ignore]
async fn the_lookup_against_a_base_of_ten_thousand_entities() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    for index in 0..30 {
        lab.write(
            &format!("fichier-{index:02}.txt"),
            &doc(index, "Docteur Jean Dupont recommande un suivi regulier."),
        );
    }
    lab.analyse().await;

    // 10 000 more people, each with its canonical form and its surname alone.
    let connection = lab.side();
    connection.execute_batch("BEGIN").expect("begins");
    for index in 0..10_000 {
        let given = format!("Prenom{}", index % 97);
        let family = format!("Famille{index}");
        let name = format!("{given} {family}");
        let normalized = name.to_lowercase();
        let id = kb::create_entity(
            &connection,
            &NewEntity {
                type_id: "person".into(),
                subtype: None,
                canonical_name: name.clone(),
                normalized_name: normalized.clone(),
                disambiguator: String::new(),
                status: EntityStatus::Active,
                origin: Origin::Automatic,
            },
        )
        .expect("creates");
        for (display, form, kind) in [
            (
                name.clone(),
                normalized.clone(),
                assistant_cabinet_ai_lib::knowledge::AliasKind::CanonicalVariant,
            ),
            (
                family.clone(),
                family.to_lowercase(),
                assistant_cabinet_ai_lib::knowledge::AliasKind::PartialSurname,
            ),
        ] {
            kb::add_alias(
                &connection,
                id,
                &NewAlias {
                    display,
                    normalized: form,
                    phonetic_key: format!("k{index}"),
                    kind,
                    confidence: 0.9,
                },
                Origin::Automatic,
            )
            .expect("adds");
        }
    }
    connection.execute_batch("COMMIT").expect("commits");

    let vocabulary = lab.vocabulary();
    let documents = lab.documents();
    let mut timings = Vec::new();
    for _ in 0..ROUNDS {
        let started = Instant::now();
        let _ = lab.plan(
            &vocabulary,
            KnowledgeMode::Auto,
            &TargetingRequest::default(),
            "Que dit le Docteur Jean Dupont du suivi de Prenom5 Famille77 ?",
            SideInput::new(&documents),
            SideInput::new(&[]),
            TargetingPolicy {
                min_scope_files: 1,
                ..TargetingPolicy::default()
            },
        );
        timings.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    println!(
        "lookup against 10 000 more entities: median {:.2} ms (budget: 20 ms)",
        median(timings)
    );
}
