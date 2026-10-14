//! Step 5 of the file profile work (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 4.5, 7): the forms of an
//! identifier on the index side, the words of a workbook's cells, and the known-names pass over its
//! free-text columns. Everything here is **additive**: the three spreadsheet tables must be byte-identical
//! after it, and the knowledge base must hold the same rows.
//!
//! Every name and number is invented.

mod common;

use assistant_cabinet_ai_lib::file_profile::build::Domain;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use common::knowledge::{count, snapshot, tabular_snapshot, Lab};
use rusqlite::Connection;

fn fts_paths(connection: &Connection, expression: &str) -> Vec<String> {
    let mut statement = connection
        .prepare("SELECT relative_path FROM file_profile_fts WHERE file_profile_fts MATCH ?1 ORDER BY relative_path")
        .expect("prepares");
    statement
        .query_map([expression], |row| row.get::<_, String>(0))
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads")
}

fn passage_paths(connection: &Connection, expression: &str) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT c.relative_path FROM chunks_fts f JOIN chunks c ON c.chunk_id = f.chunk_id
             WHERE chunks_fts MATCH ?1 ORDER BY c.relative_path",
        )
        .expect("prepares");
    statement
        .query_map([expression], |row| row.get::<_, String>(0))
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads")
}

/// The family the lab measured as 1 of 17: one number, four spellings, the user types the dotless one.
#[tokio::test]
async fn one_number_written_four_ways_is_found_by_its_compact_form_in_every_document() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-ch"];
    let spellings = [
        ("with-dots", "756.3052.5778.43"),
        ("with-spaces", "756 3052 5778 43"),
        ("dotless", "7563052577843"),
        ("mixed", "756.3052 5778.43"),
    ];
    for (index, (name, spelling)) in spellings.iter().enumerate() {
        lab.write(
            &format!("2026-03-0{}_attestation-{name}.txt", index + 1),
            &format!("Attestation d'assurance\nNumero AVS : {spelling}\nValable un an.\n"),
        );
    }
    lab.write(
        "2026-03-09_attestation-other.txt",
        "Attestation d'assurance\nNumero AVS : 756.9999.8888.77\n",
    );
    lab.analyse().await;

    let connection = lab.side();
    // Before: the passages' index holds the number as the document wrote it, so the dotless query finds one.
    let in_passages = passage_paths(&connection, "\"7563052577843\"");
    // After: every spelling meets in the profile's compact form.
    let compact = fts_paths(&connection, "identifiers:7563052577843");
    let grouped = fts_paths(&connection, "identifiers:\"756 3052 5778 43\"");
    println!("AVS family: passages find {} of 4, the profile finds {} of 4 (compact) and {} of 4 (grouped)", in_passages.len(), compact.len(), grouped.len());
    assert_eq!(
        in_passages.len(),
        1,
        "the passages' index finds only the dotless spelling"
    );
    assert_eq!(compact.len(), 4, "{compact:?}");
    assert_eq!(grouped.len(), 4, "{grouped:?}");
    assert!(
        !compact.iter().any(|path| path.contains("other")),
        "another number is not found"
    );
    assert_eq!(
        fts_paths(&connection, "identifiers:7569999888877"),
        vec!["2026-03-09_attestation-other.txt".to_string()]
    );
}

#[tokio::test]
async fn a_pack_that_is_not_active_stores_none_of_its_identifiers() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write(
        "2026-03-01_attestation.txt",
        "Attestation\nNumero AVS : 756.3052.5778.43\n",
    );
    lab.analyse().await;
    assert!(fts_paths(&lab.side(), "identifiers:7563052577843").is_empty());
}

const VISITS: &str = "Patient;Remarques;Date;Montant;Numero AVS\n\
Marie Martin;allergie a la penicilline;2026-03-02;45;756.3052.5778.43\n\
Jean Dupont;a rappeler: Madame Marie Martin sa soeur;2026-05-20;60;7563052577843\n\
Paul Roy;demenagement a Lyon;2026-04-11;30;756.1111.2222.33\n";

#[tokio::test]
async fn a_workbook_is_found_by_what_its_cells_say_and_nothing_spreadsheet_changes() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    // The base learns Marie Martin from a document first.
    lab.write(
        "dossier-initial.txt",
        "Madame Marie Martin a ete recue aux HUG pour un controle.\n",
    );
    lab.analyse().await;
    lab.write_data("visites.csv", VISITS);
    let summary = lab.analyse_data();
    let profile = summary.profile.expect("the profile pass ran");
    assert_eq!(
        (profile.built, profile.errors, profile.complete),
        (1, 0, true),
        "{profile:?}"
    );

    let connection = lab.side();
    let state: String = connection
        .query_row(
            "SELECT state FROM file_profiles WHERE domain = 'data'",
            [],
            |row| row.get(0),
        )
        .expect("a profile");
    assert_eq!(
        state, "analysed",
        "the cells were at rest when the pass ran"
    );

    // Words of every text column, names included; numbers and dates are not words.
    assert_eq!(
        fts_paths(&connection, "cells:penicilline"),
        vec!["visites.csv".to_string()]
    );
    assert_eq!(
        fts_paths(&connection, "cells:penicilline*"),
        vec!["visites.csv".to_string()]
    );
    assert_eq!(
        fts_paths(&connection, "cells:lyon"),
        vec!["visites.csv".to_string()]
    );
    assert_eq!(
        fts_paths(&connection, "cells:dupont"),
        vec!["visites.csv".to_string()]
    );
    assert!(
        fts_paths(&connection, "cells:45").is_empty()
            && fts_paths(&connection, "cells:2026").is_empty()
    );
    // Identifiers are exact tokens, in any spelling, and the document that does not hold them is not found.
    assert_eq!(
        fts_paths(&connection, "identifiers:7563052577843"),
        vec!["visites.csv".to_string()]
    );
    assert_eq!(
        fts_paths(&connection, "identifiers:7561111222233"),
        vec!["visites.csv".to_string()]
    );
    assert!(fts_paths(&connection, "identifiers:7569999888877").is_empty());
    // The date column contributes its range, and the sheet and header words are findable.
    let range: String = connection
        .query_row(
            "SELECT value FROM file_profile_fields WHERE kind = 'date_range'",
            [],
            |row| row.get(0),
        )
        .expect("a range");
    assert_eq!(range, "2026-03-02/2026-05-20");
    assert_eq!(
        fts_paths(&connection, "headers:remarques"),
        vec!["visites.csv".to_string()]
    );

    // The known-names pass: the remark that names Marie Martin links to her; the entity column is not
    // repeated, and a surname alone (Roy) links nothing.
    let person: i64 = connection
        .query_row("SELECT entity_id FROM kb_entities WHERE type_id = 'person' AND canonical_name LIKE '%Martin%'", [], |row| row.get(0))
        .expect("the base knows her");
    let links: Vec<(i64, String, String, String)> = {
        let mut statement = connection
            .prepare("SELECT entity_id, method, locator, role FROM file_profile_entities WHERE domain = 'data' AND source = 'cells'")
            .expect("prepares");
        statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .expect("queries")
            .collect::<Result<_, _>>()
            .expect("reads")
    };
    assert_eq!(
        links,
        vec![(
            person,
            "gazetteer".to_string(),
            "cells:visites!Remarques".to_string(),
            "mentioned".to_string()
        )],
        "{links:?}"
    );

    // Nothing spreadsheet changed, and the base holds the same rows with and without the profile.
    let tabular = tabular_snapshot(&connection);
    let knowledge = snapshot(&connection);
    let mut index = IndexStore::open_at(&lab.index_path()).expect("opens the index");
    index.clear_profiles(Domain::Data).expect("clears");
    drop(index);
    lab.analyse_data();
    assert_eq!(
        tabular,
        tabular_snapshot(&lab.side()),
        "a profile pass changed a spreadsheet table"
    );
    assert_eq!(
        knowledge,
        snapshot(&lab.side()),
        "a profile pass changed the knowledge base"
    );
    // A second Analyse over the unchanged workbook builds nothing.
    let again = lab.analyse_data().profile.expect("ran");
    assert_eq!(
        (again.built, again.relinked, again.unchanged),
        (0, 0, 1),
        "{again:?}"
    );
}

#[tokio::test]
async fn a_free_text_column_past_the_cap_is_recorded_as_truncated() {
    let mut lab = Lab::new();
    let mut csv = String::from("Patient;Remarques\n");
    for index in 0..5_030 {
        csv.push_str(&format!("Person Nom{index}x;remarque numero{index}x\n"));
    }
    lab.write_data("longue-liste.csv", &csv);
    lab.analyse_data();
    let connection = lab.side();
    let truncated: Vec<(String, String)> = {
        let mut statement = connection
            .prepare("SELECT detail, value FROM file_profile_fields WHERE kind = 'cells_truncated'")
            .expect("prepares");
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("queries")
            .collect::<Result<_, _>>()
            .expect("reads")
    };
    assert!(
        !truncated.is_empty(),
        "the cap must be recorded, never silent"
    );
    assert!(
        truncated.iter().all(|(_, cap)| cap == "5000"),
        "{truncated:?}"
    );
    assert!(
        truncated
            .iter()
            .any(|(place, _)| place.ends_with("!Remarques")),
        "{truncated:?}"
    );
    assert!(count(&connection, "file_profile_fts") == 1);
}

#[tokio::test]
async fn a_workbook_that_could_not_be_read_again_is_built_again_by_the_next_pass() {
    let mut lab = Lab::new();
    lab.write_data("visites.csv", VISITS);
    lab.analyse_data();
    // The file goes away between the Analyse and a profile pass over the stored inventory.
    let bytes = std::fs::read(lab.data.path().join("visites.csv")).expect("reads");
    lab.remove_data("visites.csv");
    let mut index = IndexStore::open_at(&lab.index_path()).expect("opens the index");
    index.clear_profiles(Domain::Data).expect("clears");
    let summary = assistant_cabinet_ai_lib::file_profile::store::run_workbooks(
        &mut index,
        lab.data.path(),
        None,
        "fr-FR",
    )
    .expect("ran");
    drop(index);
    assert_eq!(summary.built, 1);
    let state = |lab: &Lab| -> String {
        lab.side()
            .query_row(
                "SELECT state FROM file_profiles WHERE domain = 'data'",
                [],
                |row| row.get(0),
            )
            .expect("a profile")
    };
    assert_eq!(
        state(&lab),
        "cells_missing",
        "its words are missing, and the profile says so"
    );
    assert!(fts_paths(&lab.side(), "cells:lyon").is_empty());

    // The file is back: the next pass builds the profile again, with its words.
    lab.write_data_bytes("visites.csv", &bytes);
    lab.analyse_data();
    assert_eq!(state(&lab), "analysed");
    assert_eq!(
        fts_paths(&lab.side(), "cells:lyon"),
        vec!["visites.csv".to_string()]
    );
}
