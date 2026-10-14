//! Measures the finder's latency on a synthetic index of 500, 5 000 and 20 000 documents: a typed word met
//! in a name, in a title, in a passage, an entity filter, a type and date filter, and the bare listing with
//! its facets. Targets (design 6): under 50 ms at 5 000 files, under 200 ms at 20 000.
//!
//! `#[ignore]`: it writes tens of thousands of rows. Run explicitly:
//!
//! ```text
//! cargo test --release --test file_finder_bench -- --ignored --nocapture
//! ```
//!
//! It **prints** medians and asserts none. Every name is invented; no file is written, the inventory is a
//! list and the index is a temporary file.

#![allow(clippy::field_reassign_with_default)]

mod common;

use std::time::Instant;

use assistant_cabinet_ai_lib::file_profile::build::Domain;
use assistant_cabinet_ai_lib::finder::{
    find, DateRange, EntityFilter, FinderQuery, FinderScope, InventoryFile,
};
use common::knowledge::Lab;
use rusqlite::{params, Connection};

const SIZES: [usize; 3] = [500, 5_000, 20_000];
const ROUNDS: usize = 15;
const PEOPLE: usize = 300;

const KINDS: [&str; 5] = [
    "letter",
    "invoice",
    "lab_result",
    "prescription",
    "certificate",
];
const SURNAMES: [&str; 12] = [
    "martin",
    "dupont",
    "benali",
    "roy",
    "perrin",
    "morel",
    "garcia",
    "lambert",
    "fontaine",
    "chevalier",
    "renard",
    "girard",
];
const GIVEN: [&str; 10] = [
    "marie", "jean", "fatima", "paul", "julie", "luc", "sophie", "hugo", "lea", "adam",
];
const BODY: [&str; 16] = [
    "consultation",
    "resultat",
    "analyse",
    "traitement",
    "controle",
    "rapport",
    "demande",
    "suivi",
    "facture",
    "reglement",
    "ordonnance",
    "bilan",
    "lesion",
    "radiographie",
    "vaccin",
    "examen",
];

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values[values.len() / 2]
}

/// `files` analysed documents, three passages each, two linked entities each, one mention each.
fn populate(connection: &Connection, files: usize) -> Vec<InventoryFile> {
    connection.execute_batch("BEGIN").unwrap();
    for person in 0..PEOPLE {
        let name = format!(
            "{} {}{}",
            GIVEN[person % GIVEN.len()],
            SURNAMES[person % SURNAMES.len()],
            person
        );
        connection
            .execute(
                "INSERT INTO kb_entities (entity_id, type_id, canonical_name, normalized_name, status, origin, created_at, updated_at)
                 VALUES (?1, 'person', ?2, ?2, 'active', 'automatic', 0, 0)",
                params![person as i64 + 1, name],
            )
            .unwrap();
    }
    let mut inventory = Vec::with_capacity(files);
    for n in 0..files {
        let kind = KINDS[n % KINDS.len()];
        let year = 2020 + n % 7;
        let month = 1 + n % 12;
        let day = 1 + n % 28;
        let surname = SURNAMES[(n / 3) % SURNAMES.len()];
        let given = GIVEN[(n / 7) % GIVEN.len()];
        let path = format!(
            "{year}/{year}-{month:02}/{year}-{month:02}-{day:02}_{kind}-{surname}-{given}-{n}.pdf"
        );
        let date = format!("{year}-{month:02}-{day:02}");
        let title = format!("{kind} {surname} {given}");
        connection
            .execute(
                "INSERT INTO file_profiles (domain, relative_path, content_id, format, state, build_version, vocabulary_hash,
                     business_type, title, document_date)
                 VALUES ('documents', ?1, 'c', 'pdf', 'analysed', 1, 'v', ?2, ?3, ?4)",
                params![path, kind, title, date],
            )
            .unwrap();
        let profile_id = connection.last_insert_rowid();
        let first = (n * 7) % PEOPLE + 1;
        let second = (n * 11 + 5) % PEOPLE + 1;
        for (entity, role) in [(first, "subject"), (second, "mentioned")] {
            connection
                .execute(
                    "INSERT INTO file_profile_entities (domain, relative_path, entity_id, surface, role, source, method, confidence)
                     VALUES ('documents', ?1, ?2, 'surface', ?3, 'header_label', 'header_label', 0.85)",
                    params![path, entity as i64, role],
                )
                .unwrap();
        }
        connection
            .execute(
                "INSERT INTO file_profile_fts (rowid, domain, relative_path, name, folder, title, author, subject, entities, headers, cells, identifiers)
                 VALUES (?1, 'documents', ?2, ?3, ?4, ?5, '', '', ?6, 'patient date motif', '', '')",
                params![
                    profile_id,
                    path,
                    format!("{kind} {surname} {given} {n}"),
                    format!("{year}"),
                    title,
                    format!("{given} {surname}")
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO kb_sources (domain, relative_path, content_id) VALUES ('documents', ?1, 'c')",
                [&path],
            )
            .unwrap();
        let source = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO kb_mentions (entity_id, source_id, role, locator_kind, method, origin)
                 VALUES (?1, ?2, NULL, 'chunk', 'text', 'automatic')",
                params![first as i64, source],
            )
            .unwrap();
        for page in 1..=3usize {
            let text = format!(
                "{} {} {} {} {} {} {} {}",
                BODY[(n + page) % BODY.len()],
                BODY[(n * 3 + page) % BODY.len()],
                given,
                surname,
                BODY[(n * 5 + page) % BODY.len()],
                BODY[(n * 7 + page * 2) % BODY.len()],
                if n % 997 == 0 {
                    "zorglubien"
                } else {
                    "courant"
                },
                BODY[(n * 11 + page) % BODY.len()],
            );
            let chunk_id = format!("{path}#{page}");
            connection
                .execute(
                    "INSERT INTO chunks (chunk_id, relative_path, page_number, section, text, embedding)
                     VALUES (?1, ?2, ?3, 0, ?4, x'')",
                    params![chunk_id, path, page as i64, text],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO chunks_fts (chunk_id, text) VALUES (?1, ?2)",
                    params![chunk_id, text],
                )
                .unwrap();
        }
        inventory.push(InventoryFile {
            relative_path: path,
            extension: "pdf".to_string(),
        });
    }
    connection.execute_batch("COMMIT").unwrap();
    inventory
}

#[test]
#[ignore = "a measure: run explicitly with --release"]
fn the_finder_stays_fast_as_the_folder_grows() {
    for files in SIZES {
        let lab = Lab::new();
        let connection = lab.side();
        let started = Instant::now();
        let inventory = populate(&connection, files);
        println!(
            "\n== {files} documents (built in {:.1} s)",
            started.elapsed().as_secs_f64()
        );

        let mut cases: Vec<(&str, FinderQuery)> = Vec::new();
        let word = |text: &str| FinderQuery {
            text: Some(text.to_string()),
            ..FinderQuery::default()
        };
        cases.push(("name word (benali)", word("benali")));
        cases.push(("name start while typing (bena)", word("bena")));
        cases.push(("two words, one start (marie dupo)", word("marie dupo")));
        cases.push(("passage only (zorglubien)", word("zorglubien")));
        cases.push(("common passage word (consultation)", word("consultation")));
        cases.push(("no match (qqqqqq)", word("qqqqqq")));
        let mut by_entity = FinderQuery::default();
        by_entity.entities = vec![EntityFilter {
            entity_id: 7,
            role: None,
        }];
        cases.push(("one entity", by_entity));
        let mut kind_and_date = FinderQuery::default();
        kind_and_date.kinds = vec!["invoice".to_string()];
        kind_and_date.date = Some(DateRange {
            from: Some("2022".to_string()),
            to: Some("2023-06".to_string()),
        });
        cases.push(("type and date range", kind_and_date));
        cases.push(("bare listing and facets", FinderQuery::default()));

        let opened = Instant::now();
        for _ in 0..5 {
            let _ = assistant_cabinet_ai_lib::index_store::IndexStore::open_at(&lab.index_path())
                .expect("opens");
        }
        println!(
            "  opening the index, as the command does on every call: {:.1} ms",
            opened.elapsed().as_secs_f64() * 1000.0 / 5.0
        );

        // The command opens the index afresh on every call: the cache starts cold.
        for (label, query) in [cases[0].clone(), cases[8].clone()] {
            let mut times = Vec::new();
            for _ in 0..ROUNDS {
                let begin = Instant::now();
                let fresh = lab.side();
                find(
                    &fresh,
                    Domain::Documents,
                    &inventory,
                    &query,
                    &FinderScope::WholeFolder,
                )
                .unwrap();
                times.push(begin.elapsed().as_secs_f64() * 1000.0);
            }
            println!("  fresh connection: {label:<26} {:>8.1} ms", median(times));
        }

        for (label, query) in &cases {
            let mut times = Vec::new();
            let mut total = 0;
            for _ in 0..ROUNDS {
                let begin = Instant::now();
                let result = find(
                    &connection,
                    Domain::Documents,
                    &inventory,
                    query,
                    &FinderScope::WholeFolder,
                )
                .unwrap();
                times.push(begin.elapsed().as_secs_f64() * 1000.0);
                total = result.total;
            }
            println!("  {label:<40} {:>8.1} ms   ({total} files)", median(times));
        }
    }
}
