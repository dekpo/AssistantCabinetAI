//! Lot C of the retrieval-quality programme: « quelles sections parlent de X ? » is answered from the index,
//! grouped by section, with no model and no embedding; the same sentence about documents is unchanged, and
//! « le dernier chapitre » is not a ranking of the inventory.

mod common;

use assistant_cabinet_ai_lib::discovery::discover_all;
use assistant_cabinet_ai_lib::finder::set_search::answer;
use assistant_cabinet_ai_lib::finder::InventoryFile;
use assistant_cabinet_ai_lib::folder_questions::{
    route, set_question, FolderAnswer, NoCorpus, QuestionRoute,
};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use common::knowledge::Lab;

const LOCALE: &str = "fr-FR";

fn line(tag: &str, n: usize) -> String {
    format!("the ordinary sentence number {n} of the part {tag} goes on with the usual words and ends here.")
}

/// Three chapters, long enough (more than 60 000 characters) to be cut at its headings; the word `kingfisher` is in
/// the second and the third.
fn book() -> String {
    let mut text = String::new();
    for (chapter, tag) in ["alpha", "beta", "gamma"].iter().enumerate() {
        text.push_str(&format!("# Chapter {} {}\n", chapter + 1, tag));
        for n in 0..260 {
            if chapter > 0 && n % 60 == 0 {
                text.push_str(&format!("A kingfisher was seen near the pond ({tag}, {n}).\n"));
            }
            text.push_str(&line(tag, n));
            text.push('\n');
        }
    }
    text
}

fn inventory(lab: &Lab) -> Vec<InventoryFile> {
    discover_all(lab.work_path())
        .iter()
        .map(InventoryFile::from)
        .collect()
}

#[tokio::test]
async fn the_sections_question_lists_the_sections_where_the_word_is_with_no_model() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.write("note.txt", "A note about the kingfisher and nothing else.");
    lab.analyse().await;
    let requests = lab.gateway.requests().len();

    let set = set_question("Quelles sections parlent de kingfisher ?", LOCALE)
        .expect("a set question about sections");
    let found = answer(&lab.side(), &inventory(&lab), &set, Vec::new()).expect("answers");
    let FolderAnswer::DocumentSearch {
        by_section,
        business_type,
        listed,
        total,
        ..
    } = &found
    else {
        panic!("expected a document search, got {found:?}");
    };
    assert!(by_section);
    assert_eq!(*business_type, None, "the noun is no business type");
    assert_eq!(*total, 2, "two files hold the word");

    let book = listed.iter().find(|d| d.relative_path == "book.md").unwrap();
    let labels: Vec<&str> = book.sections.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, vec!["Chapter 2 beta", "Chapter 3 gamma"], "in reading order");
    assert!(book.sections.iter().all(|s| !s.pages.is_empty()));

    let note = listed.iter().find(|d| d.relative_path == "note.txt").unwrap();
    assert!(note.sections.is_empty(), "a note has no structure");

    assert_eq!(lab.gateway.requests().len(), requests, "no model, no embedding");
}

#[tokio::test]
async fn the_same_sentence_about_documents_is_not_by_section() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.analyse().await;
    let set = set_question("Quels documents parlent de kingfisher ?", LOCALE).expect("a set question");
    let found = answer(&lab.side(), &inventory(&lab), &set, Vec::new()).expect("answers");
    let FolderAnswer::DocumentSearch { by_section, .. } = &found else {
        panic!("expected a document search, got {found:?}");
    };
    assert!(!by_section);
}

#[tokio::test]
async fn the_nouns_of_sections_are_read_in_both_languages_and_a_chapter_is_not_a_ranking() {
    for question in [
        "Quels chapitres parlent de la franchise ?",
        "Quelles sections mentionnent Nyon ?",
    ] {
        let set = set_question(question, LOCALE).unwrap_or_else(|| panic!("{question}"));
        assert_eq!(set.kind.as_deref(), Some("section"), "{question}");
    }
    let english = set_question("Which chapters mention Nyon?", "en-US").expect("a set question");
    assert_eq!(english.kind.as_deref(), Some("section"));

    // « le dernier chapitre » takes no kind of paper for a ranking of the inventory
    let mut lab = Lab::new();
    lab.write("2026-10-01_courrier.txt", "Un courrier.
");
    lab.analyse().await;
    let inventory = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index)).unwrap();
    let routed = route(&inventory, &NoCorpus, "Quel est le dernier chapitre ?", LOCALE);
    assert!(
        !matches!(
            routed,
            QuestionRoute::Deterministic(FolderAnswer::RankedFile { .. })
        ),
        "a chapter is not a file of the inventory: {routed:?}"
    );
}
