//! Lot 6 of the long-documents programme: the knowledge base on a book. The caps on names and mentions per
//! source grow with the number of passages, and a source that still hits its cap says so in the sheet of
//! every entity it feeds. Every name is invented.

mod common;

use assistant_cabinet_ai_lib::knowledge::extract::text::caps_for;
use assistant_cabinet_ai_lib::knowledge::manage;
use common::knowledge::{entity_named, Lab};

/// A pronounceable capitalised word that is different for every number.
fn word(number: usize) -> String {
    let letters = ['b', 'd', 'f', 'g', 'k', 'l', 'm', 'n', 'p', 'r', 's', 't'];
    let vowels = ['a', 'e', 'i', 'o', 'u'];
    let mut n = number;
    let mut word = String::new();
    for round in 0..4 {
        word.push(letters[n % letters.len()]);
        n /= letters.len();
        word.push(vowels[(n + round) % vowels.len()]);
        n /= vowels.len();
    }
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn names(count: usize) -> String {
    (0..count)
        .map(|number| format!("seen {} {}, ", word(number), word(number + 1_000)))
        .collect()
}

#[test]
fn the_caps_grow_with_the_passages_and_never_fall_below_what_a_short_document_had() {
    assert_eq!(caps_for(1), (500, 5_000));
    assert_eq!(caps_for(1_000), (500, 5_000));
    assert_eq!(caps_for(4_000), (2_000, 20_000));
    assert_eq!(caps_for(100_000), (20_000, 100_000), "and a ceiling");
}

fn english_lab() -> Lab {
    let mut lab = Lab::new();
    lab.locale = "en-US";
    lab.packs = Vec::new();
    lab
}

fn first_entity(lab: &Lab) -> i64 {
    // Any name the base kept: the first one of the text.
    let name = format!("{} {}", word(0), word(1_000));
    entity_named(&lab.side(), &name)
        .unwrap_or_else(|| panic!("the base knows {name}"))
        .entity_id
}

#[tokio::test]
async fn a_source_over_its_cap_says_so_in_the_sheet_of_its_entities() {
    let mut lab = english_lab();
    lab.write("register.txt", &names(620));
    lab.write("small.txt", "seen Zabo Kemi, and nothing else.");
    lab.analyse().await;

    let sheet = manage::entity_detail(&lab.side(), first_entity(&lab)).unwrap();
    let source = sheet
        .sources
        .iter()
        .find(|s| s.relative_path == "register.txt")
        .expect("the source");
    assert!(
        source.truncated,
        "620 names in a few passages is over the cap of 500"
    );
}

#[tokio::test]
async fn the_same_names_in_a_book_of_many_passages_are_all_kept() {
    let mut lab = english_lab();
    // 520 names, then about 1 100 passages of filler: the cap is half the passages, 550.
    let filler: String = (0..1_100)
        .map(|n| format!("Filler paragraph {n}. {}\n\n", "x".repeat(1_150)))
        .collect();
    lab.write("book.txt", &format!("{}\n\n{filler}", names(520)));
    lab.analyse().await;

    let sheet = manage::entity_detail(&lab.side(), first_entity(&lab)).unwrap();
    let source = sheet
        .sources
        .iter()
        .find(|s| s.relative_path == "book.txt")
        .expect("the source");
    assert!(
        !source.truncated,
        "a book of 1 100 passages keeps 520 names"
    );
}
