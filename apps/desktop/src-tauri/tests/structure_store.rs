//! Lot 2 of the long-documents programme: the structure is stored by the Analyse, read again when its
//! version changes (the vectors are reused, not computed again), dropped with its document, and an index
//! from before it is reported and left as it is until the user presses Analyse.

mod common;

use assistant_cabinet_ai_lib::file_profile::store::{index_status, FolderProfileStatus};
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::structure::STRUCTURE_VERSION;
use common::knowledge::Lab;
use sha2::{Digest, Sha256};

fn line(tag: &str, n: usize) -> String {
    format!("the ordinary sentence number {n} of the part {tag} goes on with the usual words and ends here.")
}

fn part(tag: &str, lines: usize) -> String {
    (0..lines)
        .map(|n| line(tag, n))
        .collect::<Vec<_>>()
        .join("\n")
}

fn book() -> String {
    format!(
        "# First chapter\n{}\n## A sub part\n{}\n# Second chapter\n{}\n",
        part("one", 14),
        part("two", 14),
        part("three", 14)
    )
}

fn embedded_chars(lab: &Lab) -> usize {
    lab.gateway.requests().iter().map(|seen| seen.chars()).sum()
}

#[tokio::test]
async fn an_analysed_document_has_its_sections_and_its_passages_know_theirs() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.analyse().await;

    let sections = lab.index.sections_of("book.md").unwrap();
    let titles: Vec<&str> = sections.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["First chapter", "A sub part", "Second chapter"]
    );
    assert_eq!(sections[1].parent, Some(0));
    assert_eq!(sections[0].source, "markdown");

    let ids: Vec<String> = lab
        .index
        .chunks_for_document("book.md")
        .unwrap()
        .into_iter()
        .map(|c| c.chunk_id)
        .collect();
    let owners = lab.index.sections_of_chunks(&ids).unwrap();
    assert_eq!(
        owners.len(),
        ids.len(),
        "every passage starts inside a section here"
    );
    assert_eq!(
        lab.index.structure_version("book.md").unwrap(),
        Some(STRUCTURE_VERSION)
    );
}

#[tokio::test]
async fn a_short_note_has_no_sections_and_is_not_read_again_for_that() {
    let mut lab = Lab::new();
    lab.write("note.txt", "A short note.\nTwo lines only.");
    lab.analyse().await;
    assert!(lab.index.sections_of("note.txt").unwrap().is_empty());
    assert_eq!(
        lab.index.structure_version("note.txt").unwrap(),
        Some(STRUCTURE_VERSION)
    );

    let requests = lab.gateway.requests().len();
    let summary = lab.analyse().await;
    assert_eq!(summary.unchanged_files, 1);
    assert_eq!(lab.gateway.requests().len(), requests);
}

#[tokio::test]
async fn a_document_stored_before_the_structure_existed_is_read_again_and_its_vectors_are_reused() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.analyse().await;
    let before = lab.gateway.requests().len();
    assert!(before > 0);

    // What an index from before the structure looks like: the document is there, its structure is not.
    let side = lab.side();
    side.execute_batch(
        "DELETE FROM doc_structure; DELETE FROM doc_sections; DELETE FROM chunk_sections;",
    )
    .unwrap();
    assert!(lab.index.sections_of("book.md").unwrap().is_empty());

    let summary = lab.analyse().await;
    assert_eq!(summary.unchanged_files, 0, "the document is read again");
    assert_eq!(
        lab.gateway.requests().len(),
        before,
        "and nothing is embedded again"
    );
    assert_eq!(lab.index.sections_of("book.md").unwrap().len(), 3);
    assert_eq!(
        lab.index.structure_version("book.md").unwrap(),
        Some(STRUCTURE_VERSION)
    );
}

#[tokio::test]
async fn only_the_passages_whose_text_changed_are_embedded_again() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.analyse().await;
    let first_pass = embedded_chars(&lab);

    let edited = book().replace(
        &line("three", 13),
        "a sentence that was rewritten after the first pass.",
    );
    assert_ne!(edited, book());
    lab.write("book.md", &edited);
    lab.analyse().await;

    let second_pass = embedded_chars(&lab) - first_pass;
    assert!(second_pass > 0, "the changed passage is embedded");
    assert!(
        second_pass * 2 < first_pass,
        "{second_pass} chars again against {first_pass} the first time"
    );
}

#[tokio::test]
async fn replacing_or_removing_a_document_drops_its_sections() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.write("other.md", &book());
    lab.analyse().await;
    assert!(!lab.index.sections_of("book.md").unwrap().is_empty());

    lab.write("book.md", "Now a plain note, no heading at all.");
    lab.remove("other.md");
    lab.analyse().await;

    assert!(lab.index.sections_of("book.md").unwrap().is_empty());
    assert!(lab.index.sections_of("other.md").unwrap().is_empty());
    let side = lab.side();
    let orphans: i64 = side
        .query_row("SELECT COUNT(*) FROM chunk_sections", [], |row| row.get(0))
        .unwrap();
    assert_eq!(orphans, 0);
}

fn dump(connection: &rusqlite::Connection) -> String {
    let mut hasher = Sha256::new();
    for sql in [
        "SELECT relative_path, sha256, empty FROM documents ORDER BY relative_path",
        "SELECT chunk_id, page_number, section, text FROM chunks ORDER BY chunk_id",
    ] {
        let mut statement = connection.prepare(sql).unwrap();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            for i in 0..row.as_ref().column_count() {
                hasher.update(format!("{:?}|", row.get_ref(i).unwrap()));
            }
            hasher.update("\n");
        }
    }
    format!("{:x}", hasher.finalize())
}

#[tokio::test]
async fn an_index_from_before_the_structure_is_reported_and_left_untouched_until_analyse() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.analyse().await;

    // Make it an old index: the profile of the previous layout, no structure.
    let side = lab.side();
    side.execute(
        "UPDATE file_profile_meta SET value = '1' WHERE key = 'profile_schema.documents'",
        [],
    )
    .unwrap();
    side.execute_batch(
        "DELETE FROM doc_structure; DELETE FROM doc_sections; DELETE FROM chunk_sections;",
    )
    .unwrap();
    let before = dump(&side);

    // Opening it again, and asking its status, change nothing.
    let reopened = IndexStore::open_at(&lab.index_path()).unwrap();
    drop(reopened);
    let status = index_status(&side);
    assert_eq!(status.documents, FolderProfileStatus::NeedsAnalyse);
    assert_eq!(
        dump(&side),
        before,
        "documents and passages are byte-identical"
    );

    // Analyse brings it to the current layout.
    lab.analyse().await;
    let side = lab.side();
    assert_eq!(index_status(&side).documents, FolderProfileStatus::Current);
    assert!(!lab.index.sections_of("book.md").unwrap().is_empty());
}

// ------------------------------------------------------------------------------------ the locator

use assistant_cabinet_ai_lib::discovery::discover_all;
use assistant_cabinet_ai_lib::file_profile::build::Domain;
use assistant_cabinet_ai_lib::finder::{find, FinderQuery, FinderScope, InventoryFile};
use assistant_cabinet_ai_lib::retrieval::{format_evidence, search};
use common::fake_gateway::vector_for;

#[tokio::test]
async fn a_retrieved_passage_carries_the_section_it_sits_in_and_the_model_is_told() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.write(
        "note.txt",
        "A short note about the sentence of the part two.",
    );
    lab.analyse().await;

    let query = "ordinary sentence number 7 of the part two goes on";
    let evidence = search(&lab.index, query, &vector_for(query)).unwrap();
    let in_part_two = evidence
        .iter()
        .find(|e| e.relative_path == "book.md" && e.text.contains("of the part two"))
        .expect("a passage of the second part is found");
    let section = in_part_two.section.as_ref().expect("it knows its section");
    assert!(
        ["A sub part", "First chapter"].contains(&section.title.as_str()),
        "{section:?}"
    );
    assert!(
        section.first_page <= in_part_two.page_number
            && in_part_two.page_number <= section.last_page
    );
    assert!(
        in_part_two.ordinal >= 1,
        "the ordinal inside the page is still there"
    );

    let note = evidence.iter().find(|e| e.relative_path == "note.txt");
    assert!(
        note.is_none_or(|n| n.section.is_none()),
        "a document with no structure has no section"
    );

    let text = format_evidence(&evidence);
    assert!(
        text.contains(&format!("section \"{}\"", section.title)),
        "{text}"
    );
}

#[tokio::test]
async fn the_finder_names_the_section_of_the_passage_it_shows() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.analyse().await;
    let files: Vec<InventoryFile> = discover_all(lab.work_path())
        .iter()
        .map(InventoryFile::from)
        .collect();
    let connection = lab.side();
    let query = FinderQuery {
        text: Some("three".to_string()),
        limit: Some(10),
        ..FinderQuery::default()
    };
    let result = find(
        &connection,
        Domain::Documents,
        &files,
        &query,
        &FinderScope::WholeFolder,
    )
    .unwrap();
    let locator = result.files[0].locator.as_ref().expect("a passage matched");
    assert_eq!(locator.page, Some(1));
    assert!(
        locator.ordinal.is_some(),
        "the ordinal inside the page keeps its meaning"
    );
    assert!(
        locator.section_label.is_some(),
        "and the section is named: {locator:?}"
    );
}

// ------------------------------------------------------------------------------------ the profile

#[tokio::test]
async fn the_profile_holds_the_table_of_contents_with_the_entities_and_dates_of_each_section() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    let text = format!(
        "# First chapter\n{}\nMadame Marie Martin a ete recue aux HUG le 12 mars 2026.\n## Second part\n{}\nLe rendez-vous suivant est fixe au 3 juin 2026.\n# Third chapter\n{}\n",
        part("one", 14),
        part("two", 14),
        part("three", 14)
    );
    lab.write("book.md", &text);
    lab.analyse().await;

    let side = lab.side();
    let titles: Vec<(i64, String, i64)> = side
        .prepare("SELECT position, title, level FROM file_profile_sections WHERE relative_path = 'book.md' ORDER BY position")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(
        titles,
        vec![
            (0, "First chapter".to_string(), 1),
            (1, "Second part".to_string(), 2),
            (2, "Third chapter".to_string(), 1)
        ]
    );

    let dates: Vec<(i64, String)> = side
        .prepare("SELECT position, iso FROM file_profile_section_dates WHERE relative_path = 'book.md' ORDER BY position, iso")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .flatten()
        .collect();
    assert!(dates.contains(&(0, "2026-03-12".to_string())), "{dates:?}");
    assert!(dates.contains(&(1, "2026-06-03".to_string())), "{dates:?}");
    assert!(!dates.iter().any(|(position, _)| *position == 2));

    let people: Vec<i64> = side
        .prepare(
            "SELECT s.position FROM file_profile_section_entities s
             JOIN kb_entities e ON e.entity_id = s.entity_id
             WHERE s.relative_path = 'book.md' AND e.canonical_name LIKE '%Martin%'",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .flatten()
        .collect();
    assert!(
        people.contains(&0),
        "the person named in the first chapter: {people:?}"
    );
    assert!(!people.contains(&2), "and not in the third");
}
