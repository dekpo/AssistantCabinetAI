//! Lot 7 of the long-documents programme: the index records which embedding model and which dimension
//! made its vectors; a pass with another model recomputes them all (vectors of two models cannot be
//! mixed); and a search whose question has another dimension than the stored vectors says so instead of
//! comparing them silently.

mod common;

use std::sync::Mutex;

use assistant_cabinet_ai_lib::indexing;
use assistant_cabinet_ai_lib::retrieval::{search_scoped_counted, RetrievalScope};
use common::knowledge::{gateway_client, Lab};

fn document(file: usize) -> String {
    (0..6)
        .map(|n| {
            format!(
                "file{file} sentence {n}: ordinary words of a paragraph. {}",
                "x".repeat(1_000)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

async fn pass(lab: &mut Lab, alias: &str) {
    let mut context = lab.context();
    let url = lab.gateway.url.clone();
    let progress = Mutex::new(Vec::new());
    indexing::run_cancellable(
        &mut lab.index,
        &gateway_client(),
        &url,
        alias,
        lab.work.path(),
        None,
        None,
        lab.locale,
        &|step| progress.lock().unwrap().push(step),
        Some(&mut context),
        &|| false,
    )
    .await
    .expect("the pass completes");
}

fn inputs(lab: &Lab) -> usize {
    lab.gateway.requests().iter().map(|r| r.inputs.len()).sum()
}

#[tokio::test]
async fn the_index_records_the_model_and_the_dimension_of_its_vectors() {
    let mut lab = Lab::new();
    lab.write("a.txt", &document(1));
    assert_eq!(lab.index.embedding_info().unwrap(), None);
    pass(&mut lab, "model-a").await;
    assert_eq!(
        lab.index.embedding_info().unwrap(),
        Some(("model-a".to_string(), 2))
    );

    let before = inputs(&lab);
    pass(&mut lab, "model-a").await;
    assert_eq!(inputs(&lab), before, "the same model changes nothing");
}

#[tokio::test]
async fn another_model_recomputes_every_vector_in_the_pass_the_user_starts() {
    let mut lab = Lab::new();
    lab.write("a.txt", &document(1));
    lab.write("b.txt", &document(2));
    pass(&mut lab, "model-a").await;
    let total = lab.index.vector_counts().unwrap().total;
    let first = inputs(&lab);
    assert_eq!(first, total);

    pass(&mut lab, "model-b").await;
    assert_eq!(
        inputs(&lab) - first,
        total,
        "every passage and every card is computed again, none twice"
    );
    let counts = lab.index.vector_counts().unwrap();
    assert_eq!((counts.done, counts.total), (total, total));
    assert_eq!(lab.index.embedding_info().unwrap().unwrap().0, "model-b");
}

#[tokio::test]
async fn a_question_of_another_dimension_is_reported_not_compared_silently() {
    let mut lab = Lab::new();
    lab.write("a.txt", &document(1));
    pass(&mut lab, "model-a").await;
    let passages = lab.index.chunk_count().unwrap() as usize;

    let same = search_scoped_counted(
        &lab.index,
        "file1 sentence",
        &[0.5, 0.5],
        RetrievalScope::WholeFolder,
    )
    .unwrap();
    assert_eq!(same.mismatched_chunks, 0);

    let other = search_scoped_counted(
        &lab.index,
        "file1 sentence",
        &[0.1, 0.2, 0.3],
        RetrievalScope::WholeFolder,
    )
    .unwrap();
    assert_eq!(
        other.mismatched_chunks, passages,
        "every stored vector has another length than the question"
    );
    assert!(
        other.evidence.is_empty(),
        "vectors of another length are not compared: nothing is ranked on them"
    );
}
