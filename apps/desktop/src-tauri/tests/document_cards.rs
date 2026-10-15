//! Lot 5 of the long-documents programme: one short, deterministic card per document, built during the
//! Analyse with no model: its title, type, date and roles, its size, its outline with the first sentence of
//! each section. It is indexed by words and embedded like a passage, and it is never a passage of the
//! document: it is cited as nothing and appears in no excerpt.

mod common;

use assistant_cabinet_ai_lib::retrieval::search;
use common::fake_gateway::vector_for;
use common::knowledge::Lab;

const INVOICE: &str = "Facture FAC-2026-014\nMedSupply SA\nFacture n FAC-2026-014\nDate : 14 avril 2026\nClient : Cabinet medical des Voirons\nTotal HT : CHF 326.30\n";

fn line(tag: &str, n: usize) -> String {
    format!("the ordinary sentence number {n} of the part {tag} goes on with the usual words and ends here.")
}

fn book() -> String {
    let mut text = String::new();
    for (chapter, tag) in ["alpha", "beta", "gamma"].iter().enumerate() {
        text.push_str(&format!("# Chapter {} Zebrafish{}\n", chapter + 1, tag));
        text.push_str(&format!(
            "Opening statement of the chapter {tag}. More words follow it.\n"
        ));
        for n in 0..14 {
            text.push_str(&line(tag, n));
            text.push('\n');
        }
        if chapter == 1 {
            text.push_str("Site | Delai (jours) | Tarif (CHF)\n");
            for n in 1..=4 {
                text.push_str(&format!("Site{n} | {} | {}\n", n * 3, n * 40));
            }
        }
    }
    text
}

fn card_of(lab: &Lab, path: &str) -> String {
    lab.index
        .card_of(path)
        .unwrap()
        .unwrap_or_else(|| panic!("no card for {path}"))
        .text
}

#[tokio::test]
async fn a_document_gets_a_card_with_its_title_outline_and_first_sentences() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.analyse().await;

    let card = card_of(&lab, "book.md");
    assert!(card.starts_with("Title: "), "{card}");
    for chapter in [
        "Chapter 1 Zebrafishalpha",
        "Chapter 2 Zebrafishbeta",
        "Chapter 3 Zebrafishgamma",
    ] {
        assert!(card.contains(chapter), "{chapter} missing in {card}");
    }
    assert!(
        card.contains("Opening statement of the chapter beta."),
        "the first sentence of a section: {card}"
    );
    assert!(
        !card.contains("More words follow it"),
        "and only the first sentence"
    );
    assert!(card.contains("3 sections"), "{card}");
    assert!(card.contains("1 table"), "{card}");
    assert!(card.contains("1 page"), "{card}");
}

#[tokio::test]
async fn the_card_carries_what_the_profile_knows() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write("2026-04-14_facture-medsupply.txt", INVOICE);
    lab.analyse().await;
    let card = card_of(&lab, "2026-04-14_facture-medsupply.txt");
    assert!(card.contains("Type: invoice"), "{card}");
    assert!(card.contains("Date: 2026-04-14"), "{card}");
}

#[tokio::test]
async fn a_document_without_sections_starts_with_its_first_sentence() {
    let mut lab = Lab::new();
    lab.write(
        "note.txt",
        "Reminder for the team. The meeting moves to Thursday.\nBring the report.\n",
    );
    lab.analyse().await;
    let card = card_of(&lab, "note.txt");
    assert!(card.contains("Start: Reminder for the team."), "{card}");
}

#[tokio::test]
async fn a_card_is_never_a_passage() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.analyse().await;
    let card = card_of(&lab, "book.md");

    let side = lab.side();
    let in_chunks: i64 = side
        .query_row(
            "SELECT COUNT(*) FROM chunks WHERE text LIKE 'Title: %'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(in_chunks, 0, "the card is not a row of the passages");
    let query = "Zebrafishbeta outline sections tables";
    let evidence = search(&lab.index, query, &vector_for(query)).unwrap();
    assert!(
        evidence
            .iter()
            .all(|e| !e.text.contains("Title: ") && e.text != card),
        "no excerpt is the card"
    );
}

#[tokio::test]
async fn the_card_is_embedded_with_the_rest() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.write(
        "other.md",
        "# Quiet\nNothing special in this one.\n# Second\nStill nothing.\n",
    );
    lab.analyse().await;

    let counts = lab.index.vector_counts().unwrap();
    assert_eq!(
        counts.done, counts.total,
        "every passage and every card has its vector"
    );
    let passages: i64 = lab
        .side()
        .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        counts.total as i64,
        passages + 2,
        "two cards on top of the passages"
    );
    let sent: Vec<String> = lab
        .gateway
        .requests()
        .into_iter()
        .flat_map(|s| s.inputs)
        .collect();
    assert!(
        sent.iter().any(|input| input.starts_with("Title: ")),
        "the card text goes to the model"
    );
}

#[tokio::test]
async fn a_card_follows_its_document_and_is_not_rebuilt_for_nothing() {
    let mut lab = Lab::new();
    lab.write("book.md", &book());
    lab.write("gone.md", "# Gone\nSoon removed.\n# Also\nAs well.\n");
    lab.analyse().await;
    let before = lab.gateway.requests().len();
    lab.analyse().await;
    assert_eq!(
        lab.gateway.requests().len(),
        before,
        "nothing built or embedded again"
    );

    lab.write(
        "book.md",
        &book().replace("Zebrafishbeta", "Zebrafishdelta"),
    );
    lab.remove("gone.md");
    lab.analyse().await;
    let card = card_of(&lab, "book.md");
    assert!(card.contains("Zebrafishdelta") && !card.contains("Zebrafishbeta"));
    assert!(lab.index.card_of("gone.md").unwrap().is_none());
}
