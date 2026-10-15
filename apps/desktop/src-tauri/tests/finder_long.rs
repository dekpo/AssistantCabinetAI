//! Lot 8 of the long-documents programme: the finder returns a long document once, with the passages, pages
//! and sections where the words matched, and a set answer lists the pages. Every text is invented.

mod common;

use assistant_cabinet_ai_lib::discovery::discover_all;
use assistant_cabinet_ai_lib::file_profile::build::Domain;
use assistant_cabinet_ai_lib::finder::{
    find, FinderQuery, FinderResult, FinderScope, InventoryFile,
};
use common::knowledge::Lab;

fn line(tag: &str, n: usize) -> String {
    format!("the ordinary sentence number {n} of the part {tag} goes on with the usual words and ends here.")
}

/// Three chapters; the word `kingfisher` is in the second and the third, a few times each.
fn book(mentions: usize, lines: usize) -> String {
    let mut text = String::new();
    for (chapter, tag) in ["alpha", "beta", "gamma"].iter().enumerate() {
        text.push_str(&format!("# Chapter {} {}\n", chapter + 1, tag));
        for n in 0..lines {
            if chapter > 0 && n % (lines / mentions.max(1)).max(1) == 0 {
                text.push_str(&format!(
                    "A kingfisher was seen near the pond ({tag}, sighting {n}).\n"
                ));
            }
            text.push_str(&line(tag, n));
            text.push('\n');
        }
    }
    text
}

fn ask(lab: &Lab, word: &str) -> FinderResult {
    let files: Vec<InventoryFile> = discover_all(lab.work_path())
        .iter()
        .map(InventoryFile::from)
        .collect();
    let query = FinderQuery {
        text: Some(word.to_string()),
        limit: Some(50),
        ..FinderQuery::default()
    };
    find(
        &lab.side(),
        Domain::Documents,
        &files,
        &query,
        &FinderScope::WholeFolder,
    )
    .unwrap()
}

#[tokio::test]
async fn a_long_document_is_one_hit_with_every_place_where_the_word_matched() {
    let mut lab = Lab::new();
    lab.write("book.md", &book(4, 40));
    lab.write("note.txt", "A note about the kingfisher and nothing else.");
    lab.analyse().await;

    let result = ask(&lab, "kingfisher");
    assert_eq!(result.total, 2, "two files, each once");
    let hit = result
        .files
        .iter()
        .find(|h| h.relative_path == "book.md")
        .unwrap();
    // Four sightings in each of two chapters, a few of them in the same passage: the places are passages.
    assert!(
        (6..=8).contains(&hit.locators_total),
        "{}",
        hit.locators_total
    );
    assert_eq!(hit.locators.len(), hit.locators_total);
    let labels: Vec<Option<&str>> = hit
        .locators
        .iter()
        .map(|l| l.section_label.as_deref())
        .collect();
    assert!(
        labels.contains(&Some("Chapter 2 beta")) && labels.contains(&Some("Chapter 3 gamma")),
        "{labels:?}"
    );
    // reading order, and the first place is the one the hit already shows
    let places: Vec<(i64, i64)> = hit
        .locators
        .iter()
        .map(|l| (l.page.unwrap(), l.ordinal.unwrap()))
        .collect();
    let mut sorted = places.clone();
    sorted.sort_unstable();
    assert_eq!(places, sorted);
    let first = hit.locator.as_ref().unwrap();
    assert_eq!(
        (first.page, first.ordinal),
        (hit.locators[0].page, hit.locators[0].ordinal)
    );

    let note = result
        .files
        .iter()
        .find(|h| h.relative_path == "note.txt")
        .unwrap();
    assert_eq!((note.locators.len(), note.locators_total), (1, 1));
}

#[tokio::test]
async fn the_places_are_capped_and_the_total_still_counts_them_all() {
    let mut lab = Lab::new();
    // Sightings on every line of two chapters: far more than the places kept.
    lab.write("book.md", &book(200, 200));
    lab.analyse().await;
    let hit = ask(&lab, "kingfisher").files.remove(0);
    assert_eq!(
        hit.locators.len(),
        20,
        "the first twenty places, in reading order"
    );
    assert!(hit.locators_total > 20, "{}", hit.locators_total);
}
