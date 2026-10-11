//! The invented clinic and the small helpers the question-time tests share: a corpus of documents
//! whose first words choose their vector, a lab analysed over it, and one-line ways to plan and to
//! search. Every name here is invented.

#![allow(dead_code)]

use std::collections::BTreeSet;

use assistant_cabinet_ai_lib::knowledge::query::SelectedFile;
use assistant_cabinet_ai_lib::knowledge::reduce::{SideInput, TargetingPolicy};
use assistant_cabinet_ai_lib::knowledge::targeting::{
    self, DocumentPlan, DocumentSearch, TargetingPlan, TargetingRequest,
};
use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;

use super::fake_gateway::vector_for;
use super::knowledge::{entity_named, small_world_policy, Lab};

/// A document whose first words say which vector it gets (`para-0007` is the vector `[7, length]`),
/// so a test can ask for exactly that excerpt.
pub fn doc(marker: u32, body: &str) -> String {
    format!("para-{marker:04} {body}\n")
}

pub fn filler(index: usize) -> (String, String) {
    (
        format!("note-{index:02}.txt"),
        doc(
            5000 + index as u32,
            &format!("Note interne numero {index} sur le materiel de bureau et les fournitures."),
        ),
    )
}

/// Twelve documents: two about Dr Jean Dupont, one about Mrs Claire Martin, nine with no name.
pub fn clinic() -> Vec<(String, String)> {
    let mut files = vec![
        (
            "dupont-bilan.txt".to_string(),
            doc(
                0,
                "Docteur Jean Dupont recommande un bilan sanguin complet pour le suivi annuel.",
            ),
        ),
        (
            "dupont-lettre.txt".to_string(),
            doc(
                0,
                "Courrier du Docteur Jean Dupont au sujet du traitement en cours.",
            ),
        ),
        (
            "martin-controle.txt".to_string(),
            doc(
                9000,
                "Madame Claire Martin a ete recue pour une consultation de controle.",
            ),
        ),
    ];
    files.extend((1..=9).map(filler));
    files
}

pub async fn world(files: Vec<(String, String)>) -> Lab {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    for (name, text) in &files {
        lab.write(name, text);
    }
    lab.analyse().await;
    lab
}

pub fn auto() -> TargetingRequest {
    TargetingRequest::default()
}

pub fn chips(ids: &[i64]) -> TargetingRequest {
    TargetingRequest {
        entity_ids: ids.to_vec(),
        ..TargetingRequest::default()
    }
}

pub fn plan_with(
    lab: &Lab,
    mode: KnowledgeMode,
    request: &TargetingRequest,
    question: &str,
    policy: TargetingPolicy,
) -> TargetingPlan {
    let vocabulary = lab.vocabulary();
    let documents = lab.documents();
    lab.plan(
        &vocabulary,
        mode,
        request,
        question,
        SideInput::new(&documents),
        SideInput::new(&[]),
        policy,
    )
}

pub fn plan_for(lab: &Lab, mode: KnowledgeMode, question: &str) -> TargetingPlan {
    plan_with(lab, mode, &auto(), question, small_world_policy())
}

pub fn names(files: &[String]) -> BTreeSet<String> {
    files.iter().cloned().collect()
}

pub fn paths_of(files: &[SelectedFile]) -> Vec<String> {
    files
        .iter()
        .map(|file| file.relative_path.clone())
        .collect()
}

pub fn clean(lab: &Lab) {
    assert_eq!(
        assistant_cabinet_ai_lib::knowledge::store::integrity_check(&lab.side()).expect("checks"),
        assistant_cabinet_ai_lib::knowledge::IntegrityReport::default()
    );
}

pub fn entity_id(lab: &Lab, canonical: &str) -> i64 {
    entity_named(&lab.side(), canonical)
        .unwrap_or_else(|| panic!("no entity named {canonical:?}"))
        .entity_id
}

pub fn search(lab: &Lab, plan: &TargetingPlan, text: &str, embedding: &[f32]) -> DocumentSearch {
    let documents = paths_of(&lab.documents());
    targeting::search_documents(
        &lab.index,
        plan,
        DocumentPlan::WholeFolder {
            narrowed: true,
            scoped_paths: &documents,
            present_paths: &documents,
        },
        text,
        embedding,
    )
    .expect("searches")
}

pub fn found_in(searched: &DocumentSearch) -> BTreeSet<String> {
    searched
        .outcome
        .evidence
        .iter()
        .map(|item| item.relative_path.clone())
        .collect()
}

/// The question's embedding when it is about the chunk `text` (a vector equal to the chunk's).
pub fn embedding_of(chunk_text: &str) -> Vec<f32> {
    vector_for(chunk_text)
}
