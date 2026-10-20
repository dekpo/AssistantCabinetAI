//! Explorer 7: what the knowledge base and the file profile know of each analysed file, read once per folder
//! for the explorer's filters (`finder::facts`). Every document and every name is invented.

mod common;

use assistant_cabinet_ai_lib::file_profile::build::Domain;
use assistant_cabinet_ai_lib::finder::facts::{folder_facts, FolderFacts};
use common::knowledge::Lab;
use rusqlite::Connection;

const PRESCRIPTION: &str = "Ordonnance du 20 mai 2026\nCabinet medical des Voirons\nPatient : Marie Martin\nOmeprazole, un comprime matin et soir.\n";
const INVOICE: &str = "Facture FAC-2026-014\nMedSupply SA\nFacture n FAC-2026-014\nDate : 14 avril 2026\nClient : Cabinet medical des Voirons\nTotal HT : CHF 326.30\n";
const LETTER: &str = "Courrier du 3 juin 2025\nHopitaux universitaires de Geneve\nMadame Marie Martin a ete recue aux HUG pour une consultation de dermatologie.\nElle presente une lesion cutanee du bras gauche.\n";

async fn analysed_lab() -> Lab {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write("2026-05-20_ordonnance.txt", PRESCRIPTION);
    lab.write("2026-04-14_facture-medsupply-fac-2026-014.txt", INVOICE);
    lab.write("courriers/courrier-dermatologie.txt", LETTER);
    lab.analyse().await;
    lab
}

fn facts_of<'a>(
    facts: &'a FolderFacts,
    path: &str,
) -> &'a assistant_cabinet_ai_lib::finder::facts::FileFacts {
    facts
        .files
        .iter()
        .find(|file| file.relative_path == path)
        .unwrap_or_else(|| panic!("no facts for {path}"))
}

fn entity_id(connection: &Connection, name: &str) -> i64 {
    connection
        .query_row(
            "SELECT entity_id FROM kb_entities WHERE canonical_name LIKE ?1",
            [name],
            |row| row.get(0),
        )
        .expect("the base knows the entity")
}

#[tokio::test]
async fn each_analysed_file_carries_its_type_its_years_and_its_people() {
    let lab = analysed_lab().await;
    let connection = lab.side();

    let facts = folder_facts(&connection, Domain::Documents).expect("the facts are read");

    // A year comes from the document's date, or from the date the name starts with.
    let invoice = facts_of(&facts, "2026-04-14_facture-medsupply-fac-2026-014.txt");
    assert_eq!(invoice.years, vec!["2026".to_string()]);
    let letter = facts_of(&facts, "courriers/courrier-dermatologie.txt");
    assert_eq!(letter.years, vec!["2025".to_string()]);
    assert!(invoice.business_type.is_some() || letter.business_type.is_some());

    // The same person is linked to the two files that name her, under one id.
    let marie = entity_id(&connection, "%Marie Martin%");
    let linked: Vec<&str> = facts
        .files
        .iter()
        .filter(|file| file.entities.iter().any(|link| link.entity_id == marie))
        .map(|file| file.relative_path.as_str())
        .collect();
    assert!(linked.contains(&"2026-05-20_ordonnance.txt"), "{linked:?}");
    assert!(
        linked.contains(&"courriers/courrier-dermatologie.txt"),
        "{linked:?}"
    );

    // A person stays a person and an organisation an organisation, whatever packs are active.
    let name = facts
        .entities
        .iter()
        .find(|entity| entity.entity_id == marie)
        .expect("the person is named");
    assert_eq!(name.type_id, "person");
    assert!(facts
        .entities
        .iter()
        .any(|entity| entity.type_id == "organization"));
    assert!(facts
        .entities
        .iter()
        .all(|entity| entity.type_id == "person" || entity.type_id == "organization"));

    // Every role is a machine code, sorted, once each.
    for file in &facts.files {
        for link in &file.entities {
            let mut sorted = link.roles.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(link.roles, sorted);
            assert!(!link.roles.is_empty());
        }
    }
}

#[tokio::test]
async fn a_file_never_analysed_and_another_folder_carry_no_facts() {
    let lab = analysed_lab().await;
    lab.write("never-analysed.txt", "x");
    let connection = lab.side();

    let documents = folder_facts(&connection, Domain::Documents).expect("the facts are read");
    assert!(documents
        .files
        .iter()
        .all(|file| file.relative_path != "never-analysed.txt"));
    let data = folder_facts(&connection, Domain::Data).expect("the facts are read");
    assert!(data.files.is_empty());
    assert!(data.entities.is_empty());
}

#[tokio::test]
async fn a_merged_entity_answers_as_the_one_it_was_merged_into_and_a_deleted_one_links_nothing() {
    let lab = analysed_lab().await;
    let connection = lab.side();
    let organisations: Vec<(i64, String)> = {
        let mut statement = connection
            .prepare("SELECT entity_id, canonical_name FROM kb_entities WHERE type_id = 'organization' ORDER BY entity_id")
            .expect("prepares");
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("queries")
            .flatten()
            .collect()
    };
    assert!(organisations.len() >= 2, "{organisations:?}");
    let (absorbed, survivor) = (organisations[0].0, organisations[1].0);
    let before = folder_facts(&connection, Domain::Documents).expect("the facts are read");
    assert!(before
        .entities
        .iter()
        .any(|entity| entity.entity_id == absorbed));

    connection
        .execute(
            "UPDATE kb_entities SET merged_into = ?1, status = 'merged' WHERE entity_id = ?2",
            [survivor, absorbed],
        )
        .expect("merges");
    let merged = folder_facts(&connection, Domain::Documents).expect("the facts are read");
    assert!(merged
        .entities
        .iter()
        .all(|entity| entity.entity_id != absorbed));
    assert!(merged
        .files
        .iter()
        .all(|file| file.entities.iter().all(|link| link.entity_id != absorbed)));
    assert!(merged
        .entities
        .iter()
        .any(|entity| entity.entity_id == survivor));

    connection
        .execute(
            "UPDATE kb_entities SET status = 'deleted' WHERE entity_id = ?1",
            [survivor],
        )
        .expect("deletes");
    let deleted = folder_facts(&connection, Domain::Documents).expect("the facts are read");
    assert!(deleted
        .entities
        .iter()
        .all(|entity| entity.entity_id != survivor));
    assert!(deleted
        .files
        .iter()
        .all(|file| file.entities.iter().all(|link| link.entity_id != survivor)));
}

#[tokio::test]
async fn the_facts_come_out_in_a_stable_order() {
    let lab = analysed_lab().await;
    let connection = lab.side();

    let first = folder_facts(&connection, Domain::Documents).expect("the facts are read");
    let second = folder_facts(&connection, Domain::Documents).expect("the facts are read");

    assert_eq!(first, second);
    let paths: Vec<&str> = first
        .files
        .iter()
        .map(|f| f.relative_path.as_str())
        .collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted);
}
