//! Lot 3 of the long-documents programme, through the Analyse: a long document is cut by the chunker for
//! long documents and a short one by today's; the context prefix is embedded and searched but never cited;
//! the chunker that cut a document is recorded, and a document cut by an older one is read again.

mod common;

use assistant_cabinet_ai_lib::chunking::LONG_DOCUMENT_CHARS;
use assistant_cabinet_ai_lib::chunking_long::{set_long_options_for_measurement, LongOptions};
use common::knowledge::Lab;
use std::sync::OnceLock;
use tokio::sync::{Mutex, MutexGuard};

/// The chunker options are process-wide: the tests of this file take turns, each setting the ones it needs
/// (the defaults, or the defaults with the context prefix on).
async fn with_options(prefix: bool) -> MutexGuard<'static, ()> {
    static TURN: OnceLock<Mutex<()>> = OnceLock::new();
    let guard = TURN.get_or_init(|| Mutex::new(())).lock().await;
    set_long_options_for_measurement(Some(LongOptions {
        prefix,
        ..LongOptions::default()
    }));
    guard
}

fn line(tag: &str, n: usize) -> String {
    format!("the ordinary sentence number {n} of the part {tag} goes on with the usual words and ends here.")
}

/// A Markdown book above the long threshold, with a table that sits where a plain cut would split it.
fn long_book() -> String {
    let mut text = String::new();
    for (chapter, tag) in ["alpha", "beta", "gamma"].iter().enumerate() {
        text.push_str(&format!("# Chapter {} Zebrafish{}\n", chapter + 1, tag));
        for n in 0..260 {
            text.push_str(&line(tag, n));
            text.push('\n');
        }
        if chapter == 1 {
            text.push_str("Site | Delai (jours) | Tarif (CHF)\n");
            for n in 1..=6 {
                text.push_str(&format!("Site{n} | {} | {}\n", n * 3, n * 40));
            }
        }
    }
    assert!(text.chars().count() > LONG_DOCUMENT_CHARS);
    text
}

fn chunker_of(lab: &Lab, path: &str) -> i64 {
    lab.side()
        .query_row(
            "SELECT chunker FROM doc_structure WHERE relative_path = ?1",
            [path],
            |row| row.get(0),
        )
        .unwrap()
}

#[tokio::test]
async fn a_long_document_is_cut_by_the_long_chunker_and_a_short_one_by_the_old_one() {
    let _turn = with_options(false).await;
    let mut lab = Lab::new();
    lab.write("book.md", &long_book());
    lab.write(
        "note.md",
        "# A note\nTwo lines only.\n## Another\nand a third.",
    );
    lab.analyse().await;
    assert_eq!(chunker_of(&lab, "book.md"), 2);
    assert_eq!(chunker_of(&lab, "note.md"), 1);

    let side = lab.side();
    let table_chunks: i64 = side
        .query_row(
            "SELECT COUNT(*) FROM chunks WHERE relative_path = 'book.md' AND text LIKE '%Site | Delai%' AND text LIKE '%Site6 |%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(table_chunks, 1, "the table is whole in one passage");
    let headings_not_first: i64 = side
        .query_row(
            "SELECT COUNT(*) FROM chunks WHERE relative_path = 'book.md' AND text LIKE '%' || char(10) || '# Chapter%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(headings_not_first, 0, "a heading always starts its passage");
}

#[tokio::test]
async fn the_prefix_is_embedded_and_searched_and_never_cited() {
    let _turn = with_options(true).await;
    let mut lab = Lab::new();
    lab.write("book.md", &long_book());
    lab.write(
        "note.md",
        "# A note\nTwo lines only.\n## Another\nand a third.",
    );
    lab.analyse().await;

    // Embedded: some input sent to the model starts with the title and carries the section path.
    let sent: Vec<String> = lab
        .gateway
        .requests()
        .into_iter()
        .flat_map(|seen| seen.inputs)
        .collect();
    assert!(
        sent.iter()
            .any(|input| input.starts_with("book\n") && input.contains("Zebrafishbeta")),
        "the model sees the path"
    );

    // Searched: a word that only the heading carries finds passages of its section.
    let side = lab.side();
    let by_word: Vec<(String, String)> = side
        .prepare(
            "SELECT chunks.chunk_id, chunks.text FROM chunks_fts JOIN chunks ON chunks.chunk_id = chunks_fts.chunk_id
             WHERE chunks_fts MATCH 'Zebrafishgamma' AND chunks.relative_path = 'book.md'",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .flatten()
        .collect();
    assert!(
        by_word.len() > 2,
        "every passage of the chapter answers to the heading's word: {}",
        by_word.len()
    );
    assert!(
        by_word
            .iter()
            .any(|(_, text)| !text.contains("Zebrafishgamma")),
        "though the cited text does not carry it"
    );

    // Never cited: no stored text starts with the title.
    let cited_with_prefix: i64 = side
        .query_row(
            "SELECT COUNT(*) FROM chunks WHERE text LIKE 'book' || char(10) || '%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cited_with_prefix, 0);
    // The short document has no prefix at all.
    let note_prefixed: i64 = side
        .query_row(
            "SELECT COUNT(*) FROM chunk_index_text WHERE relative_path = 'note.md'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(note_prefixed, 0);
}

#[tokio::test]
async fn nothing_is_read_or_embedded_again_when_nothing_changed() {
    let _turn = with_options(false).await;
    let mut lab = Lab::new();
    lab.write("book.md", &long_book());
    lab.analyse().await;
    let requests = lab.gateway.requests().len();
    let summary = lab.analyse().await;
    assert_eq!(summary.unchanged_files, 1);
    assert_eq!(lab.gateway.requests().len(), requests);
}

#[tokio::test]
async fn a_long_document_cut_by_the_old_chunker_is_read_again_and_a_short_one_is_not() {
    let _turn = with_options(false).await;
    let mut lab = Lab::new();
    lab.write("book.md", &long_book());
    lab.write(
        "note.md",
        "# A note\nTwo lines only.\n## Another\nand a third.",
    );
    lab.analyse().await;

    // What an index from before the long chunker looks like: both documents were cut by chunker 1.
    lab.side()
        .execute("UPDATE doc_structure SET chunker = 1", [])
        .unwrap();
    let summary = lab.analyse().await;
    assert_eq!(summary.unchanged_files, 1, "the short note is left alone");
    assert_eq!(chunker_of(&lab, "book.md"), 2);
    assert_eq!(chunker_of(&lab, "note.md"), 1);
}

#[tokio::test]
async fn a_passage_whose_prefix_changed_is_embedded_again() {
    let _turn = with_options(true).await;
    let mut lab = Lab::new();
    lab.write("book.md", &long_book());
    lab.analyse().await;
    let first = lab.gateway.requests().len();

    // Same text, a heading renamed: the cited text of the passages below it is unchanged, their prefix is not.
    let renamed = long_book().replace("Zebrafishgamma", "Zebrafishdelta");
    lab.write("book.md", &renamed);
    lab.analyse().await;
    let sent: Vec<String> = lab
        .gateway
        .requests()
        .into_iter()
        .skip(first)
        .flat_map(|s| s.inputs)
        .collect();
    assert!(
        sent.iter()
            .any(|input| input.contains("Zebrafishdelta") && !input.starts_with('#')),
        "{}",
        sent.len()
    );
    assert!(
        sent.len() > 2,
        "the passages of the renamed chapter are embedded again"
    );
    assert!(
        sent.len() < 30,
        "and only those (78 in the first pass): {}",
        sent.len()
    );
}
