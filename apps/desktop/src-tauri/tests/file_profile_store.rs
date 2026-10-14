//! Step 4 of the file profile work (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 4.8, 5, 8): the profile tables, the
//! pass that fills them inside the Analyse, and the guarantees around them.
//!
//! Every document is invented. Names and documents are plain ASCII French as the pilot writes them.

mod common;

use std::io::Write;

use assistant_cabinet_ai_lib::file_profile::build::Domain;
use assistant_cabinet_ai_lib::file_profile::store::{
    index_status, record_original_names, refresh_documents, FolderProfileStatus, ProfileSettings,
};
use assistant_cabinet_ai_lib::file_profile::vocabulary::ProfileVocabulary;
use assistant_cabinet_ai_lib::filename_sanitizer::Renamed;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::knowledge::packs::PackSet;
use assistant_cabinet_ai_lib::knowledge::KnowledgeMode;
use common::knowledge::{count, knowledge_rows, profile_snapshot, snapshot, tabular_snapshot, Lab};
use rusqlite::Connection;

const INVOICE: &str = "Facture FAC-2026-014\nMedSupply SA\nFacture n FAC-2026-014\nDate : 14 avril 2026\nClient : Cabinet medical des Voirons\nTotal HT : CHF 326.30\n";
const PRESCRIPTION: &str = "Ordonnance du 20 mai 2026\nCabinet medical des Voirons\nPatient : Marie Martin\nOmeprazole, un comprime matin et soir.\n";
const FIRST_DOCUMENT: &str =
    "Madame Marie Martin a ete recue aux HUG pour un controle de routine.\n";

fn docx_with_properties(creator: &str, title: &str) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options: zip::write::FileOptions<()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let parts = [
        (
            "word/document.xml",
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:r><w:t>Compte rendu de la reunion de service.</w:t></w:r></w:p></w:body></w:document>".to_string(),
        ),
        (
            "docProps/core.xml",
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\"><dc:title>{title}</dc:title><dc:creator>{creator}</dc:creator><dcterms:created>2026-03-02T09:00:00Z</dcterms:created></cp:coreProperties>"),
        ),
    ];
    for (name, body) in parts {
        zip.start_file(name, options).expect("starts a part");
        zip.write_all(body.as_bytes()).expect("writes a part");
    }
    zip.finish().expect("finishes the archive").into_inner()
}

fn one_string(connection: &Connection, sql: &str, parameter: &str) -> Option<String> {
    connection
        .query_row(sql, [parameter], |row| row.get::<_, Option<String>>(0))
        .ok()
        .flatten()
}

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

#[tokio::test]
async fn an_analyse_writes_a_profile_per_document_and_a_second_one_writes_nothing() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write("2026-04-14_facture-medsupply-fac-2026-014.txt", INVOICE);
    lab.write("2026-05-20_ordonnance.txt", PRESCRIPTION);
    lab.write_bytes(
        "compte-rendu.docx",
        &docx_with_properties("Zorglub Metadatus", "Titre de metadonnee"),
    );
    let summary = lab.analyse().await;
    let profile = summary.profile.expect("the profile pass ran");
    assert_eq!(
        (profile.built, profile.errors, profile.complete),
        (3, 0, true),
        "{profile:?}"
    );

    let connection = lab.side();
    let invoice = "2026-04-14_facture-medsupply-fac-2026-014.txt";
    let ask = |column: &str, path: &str| {
        one_string(
            &connection,
            &format!("SELECT {column} FROM file_profiles WHERE domain = 'documents' AND relative_path = ?1"),
            path,
        )
    };
    assert_eq!(ask("business_type", invoice).as_deref(), Some("invoice"));
    assert_eq!(
        ask("title", invoice).as_deref(),
        Some("Facture FAC-2026-014")
    );
    assert_eq!(ask("document_date", invoice).as_deref(), Some("2026-04-14"));
    assert_eq!(
        ask("document_date_source", invoice).as_deref(),
        Some("header_label")
    );
    assert_eq!(
        ask("business_type", "2026-05-20_ordonnance.txt").as_deref(),
        Some("prescription")
    );
    assert_eq!(
        ask("title", "compte-rendu.docx").as_deref(),
        Some("Titre de metadonnee")
    );
    assert_eq!(
        ask("business_type", "compte-rendu.docx"),
        None,
        "no evidence, no type"
    );
    let created: Option<String> = connection
        .query_row(
            "SELECT value FROM file_profile_fields WHERE relative_path = 'compte-rendu.docx' AND kind = 'metadata' AND detail = 'created'",
            [],
            |row| row.get(0),
        )
        .ok();
    assert_eq!(created.as_deref(), Some("2026-03-02"));

    // The words of the name, the title, the author and the identifiers are findable, in their own columns.
    assert_eq!(
        fts_paths(&connection, "name:fac*"),
        vec![invoice.to_string()]
    );
    assert_eq!(
        fts_paths(&connection, "title:titre"),
        vec!["compte-rendu.docx".to_string()]
    );
    assert_eq!(
        fts_paths(&connection, "author:zorglub"),
        vec!["compte-rendu.docx".to_string()]
    );
    assert_eq!(
        fts_paths(&connection, "identifiers:fac2026014"),
        vec![invoice.to_string()]
    );
    assert_eq!(fts_paths(&connection, "name:ordo*").len(), 1);
    assert!(
        fts_paths(&connection, "name:zorglub").is_empty(),
        "the author is not in the name column"
    );

    // A second Analyse over unchanged files writes nothing.
    let before = profile_snapshot(&connection);
    let second = lab.analyse().await;
    let again = second.profile.expect("the profile pass ran");
    assert_eq!(
        (again.built, again.relinked, again.unchanged, again.errors),
        (0, 0, 3, 0),
        "{again:?}"
    );
    assert_eq!(
        before,
        profile_snapshot(&lab.side()),
        "the second pass changed the profile"
    );
    assert_eq!(second.indexed_files, 0);
}

#[tokio::test]
async fn a_changed_file_is_rebuilt_alone_and_a_removed_file_loses_its_profile() {
    let mut lab = Lab::new();
    lab.write("2026-05-20_ordonnance.txt", PRESCRIPTION);
    lab.write("2026-04-14_facture-medsupply-fac-2026-014.txt", INVOICE);
    lab.analyse().await;

    lab.write(
        "2026-05-20_ordonnance.txt",
        "Certificat du 20 mai 2026\nSigne\n",
    );
    lab.remove("2026-04-14_facture-medsupply-fac-2026-014.txt");
    let summary = lab.analyse().await;
    let profile = summary.profile.expect("the profile pass ran");
    assert_eq!(
        (profile.built, profile.removed, profile.unchanged),
        (1, 1, 0),
        "{profile:?}"
    );

    let connection = lab.side();
    assert_eq!(
        one_string(
            &connection,
            "SELECT business_type FROM file_profiles WHERE relative_path = ?1",
            "2026-05-20_ordonnance.txt"
        )
        .as_deref(),
        Some("certificate"),
        "the type follows the content, and the file name's own word is outvoted by the title"
    );
    assert_eq!(count(&connection, "file_profiles"), 1);
    assert_eq!(count(&connection, "file_profile_fts"), 1);
    assert!(count(&connection, "file_profile_fields") > 0);
    assert!(
        fts_paths(&connection, "name:facture").is_empty()
            && fts_paths(&connection, "identifiers:fac2026014").is_empty(),
        "nothing of the removed file stays findable"
    );
}

#[tokio::test]
async fn names_the_base_holds_are_linked_in_the_profile_and_never_written_to_the_knowledge_base() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write("dossier-initial.txt", FIRST_DOCUMENT);
    lab.analyse().await;
    lab.write("hug-martin-marie.txt", "Autre note sur les fournitures.\n");
    lab.write("2026-05-20_ordonnance.txt", PRESCRIPTION);
    lab.analyse().await;

    let connection = lab.side();
    let person = one_string(
        &connection,
        "SELECT CAST(entity_id AS TEXT) FROM kb_entities WHERE type_id = 'person' AND canonical_name LIKE ?1",
        "%Martin%",
    )
    .expect("the base knows Marie Martin");
    let organisation = one_string(
        &connection,
        "SELECT CAST(entity_id AS TEXT) FROM kb_entities WHERE type_id = 'organization' AND canonical_name LIKE ?1",
        "%universitaires%",
    )
    .expect("the base knows the HUG");

    let linked = |path: &str| -> Vec<(String, String, String, String)> {
        let mut statement = connection
            .prepare(
                "SELECT CAST(entity_id AS TEXT), role, method, source FROM file_profile_entities
                 WHERE relative_path = ?1 AND entity_id IS NOT NULL ORDER BY entity_id",
            )
            .expect("prepares");
        statement
            .query_map([path], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .expect("queries")
            .collect::<Result<_, _>>()
            .expect("reads")
    };
    // G9b: an organisation and a person in one name, both linked, as a mention of the name.
    let by_name = linked("hug-martin-marie.txt");
    let ids: Vec<&str> = by_name.iter().map(|row| row.0.as_str()).collect();
    assert!(
        ids.contains(&person.as_str()) && ids.contains(&organisation.as_str()),
        "{by_name:?}"
    );
    assert!(by_name
        .iter()
        .all(|row| row.1 == "mentioned" && row.2 == "filename_token" && row.3 == "filename"));
    // A label gives a role: `Patient : Marie Martin` makes her the subject of the prescription.
    let by_label = linked("2026-05-20_ordonnance.txt");
    assert!(
        by_label
            .iter()
            .any(|row| row.0 == person && row.1 == "subject" && row.2 == "header_label"),
        "{by_label:?}"
    );
    assert_eq!(
        fts_paths(&connection, "entities:martin").len(),
        2,
        "both files are findable by her name"
    );

    // Nothing of this reached the knowledge base: forgetting every profile and building them again
    // leaves every `kb_*` row as it was, and no mention was created by the profile.
    let knowledge = snapshot(&connection);
    let mut index = IndexStore::open_at(&lab.index_path()).expect("opens the index");
    index
        .clear_profiles(Domain::Documents)
        .expect("clears the profiles");
    drop(index);
    lab.analyse().await;
    assert_eq!(
        knowledge,
        snapshot(&lab.side()),
        "the profile pass wrote to the knowledge base"
    );
}

#[tokio::test]
async fn links_follow_the_base_when_it_learns_a_name_later() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write("2026-05-20_ordonnance.txt", PRESCRIPTION);
    lab.analyse().await;
    {
        let connection = lab.side();
        let unlinked: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM file_profile_entities WHERE role = 'subject' AND entity_id IS NULL AND surface = 'Marie Martin'",
                [],
                |row| row.get(0),
            )
            .expect("counts");
        // The same pass that read the label also taught the base her name from the text, so she is linked
        // at once; the surface is kept either way.
        let any: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM file_profile_entities WHERE role = 'subject'",
                [],
                |row| row.get(0),
            )
            .expect("counts");
        assert_eq!(any, 1);
        assert!(unlinked <= 1);
    }
    // A new document names a second person; the base's epoch moves; the prescription is relinked from
    // the stored surfaces without reading its file again.
    lab.write(
        "compte-rendu.txt",
        "Compte rendu de consultation\nDr Paul Roy a recu Madame Julie Perrin.\n",
    );
    let summary = lab.analyse().await;
    let profile = summary.profile.expect("ran");
    assert_eq!(profile.built, 1, "{profile:?}");
    assert!(profile.relinked + profile.unchanged >= 1);
}

#[tokio::test]
async fn with_the_knowledge_mode_off_layer_one_is_complete_and_links_are_surfaces_only() {
    let mut lab = Lab::new();
    lab.mode = KnowledgeMode::Off;
    lab.packs = vec!["health-fr"];
    lab.write("2026-05-20_ordonnance.txt", PRESCRIPTION);
    lab.write("hug-martin-marie.txt", "Note.\n");
    let summary = lab.analyse().await;
    let profile = summary
        .profile
        .expect("the profile pass ran even with the knowledge base off");
    assert_eq!(profile.built, 2);

    let connection = lab.side();
    // The type needs the health pack, which the user switched on whatever the knowledge mode; the date
    // and the title need nothing.
    assert_eq!(
        one_string(
            &connection,
            "SELECT business_type FROM file_profiles WHERE relative_path = ?1",
            "2026-05-20_ordonnance.txt"
        )
        .as_deref(),
        Some("prescription")
    );
    assert_eq!(
        one_string(
            &connection,
            "SELECT document_date FROM file_profiles WHERE relative_path = ?1",
            "2026-05-20_ordonnance.txt"
        )
        .as_deref(),
        Some("2026-05-20")
    );
    assert_eq!(
        one_string(
            &connection,
            "SELECT title FROM file_profiles WHERE relative_path = ?1",
            "2026-05-20_ordonnance.txt"
        )
        .as_deref(),
        Some("Ordonnance du 20 mai 2026")
    );
    let with_ids: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM file_profile_entities WHERE entity_id IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .expect("counts");
    assert_eq!(with_ids, 0, "no knowledge base, no link");
    let surfaces: i64 = connection
        .query_row("SELECT COUNT(*) FROM file_profile_entities WHERE role = 'subject' AND surface = 'Marie Martin'", [], |row| row.get(0))
        .expect("counts");
    assert_eq!(surfaces, 1, "the label's surface is kept");
    assert_eq!(
        knowledge_rows(&connection),
        0,
        "the knowledge base stayed empty"
    );
}

#[tokio::test]
async fn an_old_index_is_detected_at_launch_and_left_untouched_until_the_user_analyses() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write("2026-05-20_ordonnance.txt", PRESCRIPTION);
    lab.write_data(
        "rendez-vous.csv",
        "Patient;Date du rendez-vous\nMarie Martin;2026-05-20\n",
    );
    lab.analyse().await;
    lab.analyse_data();
    assert_eq!(
        index_status(&lab.side()).documents,
        FolderProfileStatus::Current
    );
    assert_eq!(index_status(&lab.side()).data, FolderProfileStatus::Current);

    // An index written before the profile existed: the profile tables are not there at all.
    {
        let connection = lab.side();
        for table in [
            "file_profile_fts",
            "file_profile_fields",
            "file_profile_entities",
            "file_profile_sections",
            "file_profile_original_names",
            "file_profiles",
            "file_profile_meta",
        ] {
            connection
                .execute(&format!("DROP TABLE {table}"), [])
                .expect("drops");
        }
    }
    let old_snapshot = |connection: &Connection| {
        let mut all = snapshot(connection);
        for (table, rows) in tabular_snapshot(connection) {
            all.insert(table, rows);
        }
        for (table, columns) in [
            (
                "documents",
                "relative_path || '|' || sha256 || '|' || empty",
            ),
            ("chunks", "chunk_id || '|' || relative_path || '|' || text"),
        ] {
            let mut statement = connection
                .prepare(&format!("SELECT {columns} FROM {table}"))
                .expect("prepares");
            let mut rows: Vec<String> = statement
                .query_map([], |row| row.get::<_, String>(0))
                .expect("queries")
                .collect::<Result<_, _>>()
                .expect("reads");
            rows.sort();
            all.insert(table.to_string(), rows);
        }
        all
    };
    let before = old_snapshot(&lab.side());

    // Opening it adds the empty tables and changes nothing else; the status says what to do.
    let reopened = IndexStore::open_at(&lab.index_path()).expect("an old index opens");
    drop(reopened);
    let connection = lab.side();
    let status = index_status(&connection);
    assert_eq!(status.documents, FolderProfileStatus::NeedsAnalyse);
    assert_eq!(status.data, FolderProfileStatus::NeedsAnalyse);
    assert_eq!(
        count(&connection, "file_profiles"),
        0,
        "never rebuilt silently"
    );
    assert_eq!(
        before,
        old_snapshot(&connection),
        "opening an old index must change nothing it held"
    );
    drop(connection);

    // Only an Analyse the user presses fills it, and it does not re-read or re-embed the documents.
    lab.index = IndexStore::open_at(&lab.index_path()).expect("opens");
    let summary = lab.analyse().await;
    assert_eq!(
        (summary.indexed_files, summary.unchanged_files),
        (0, 1),
        "{summary:?}"
    );
    assert_eq!(summary.profile.expect("ran").built, 1);
    lab.analyse_data();
    let status = index_status(&lab.side());
    assert_eq!(
        (status.documents, status.data),
        (FolderProfileStatus::Current, FolderProfileStatus::Current)
    );
    // An empty index has nothing to rebuild.
    let fresh = Lab::new();
    assert_eq!(
        index_status(&fresh.side()).documents,
        FolderProfileStatus::Empty
    );
    assert_eq!(index_status(&fresh.side()).data, FolderProfileStatus::Empty);
}

#[tokio::test]
async fn the_profile_pass_leaves_the_spreadsheet_tables_byte_identical() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data(
        "rendez-vous-2026.csv",
        "Patient;Medecin traitant;Date du rendez-vous\nJean Dupont;Dr Martin;2026-05-20\nMarie Martin;Dr Roy;2026-05-21\n",
    );
    lab.analyse_data();
    let before = tabular_snapshot(&lab.side());
    assert!(!before["tabular_inventories"].is_empty());

    // The profile of the workbook now exists, typed from its name and its headers.
    let connection = lab.side();
    assert_eq!(
        one_string(
            &connection,
            "SELECT business_type FROM file_profiles WHERE domain = 'data' AND relative_path = ?1",
            "rendez-vous-2026.csv"
        )
        .as_deref(),
        Some("schedule")
    );
    assert!(
        !fts_paths(&connection, "headers:medecin").is_empty(),
        "the header words are findable"
    );

    // Forgetting and rebuilding the profile, and a second Analyse, change none of the three tables.
    let mut index = IndexStore::open_at(&lab.index_path()).expect("opens the index");
    index.clear_profiles(Domain::Data).expect("clears");
    drop(index);
    assert!(index_status(&lab.side()).data == FolderProfileStatus::NeedsAnalyse);
    lab.analyse_data();
    assert_eq!(
        before,
        tabular_snapshot(&lab.side()),
        "a profile pass changed a spreadsheet table"
    );
    assert_eq!(count(&lab.side(), "file_profiles"), 1);
}

#[tokio::test]
async fn the_pass_is_resumable_and_cancellable() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    for index in 0..6 {
        lab.write(
            &format!("2026-05-0{}_ordonnance-{index}.txt", index + 1),
            PRESCRIPTION,
        );
    }
    // Analyse first with the profile pass emptied, to run the pass by hand.
    lab.analyse().await;
    let mut index = IndexStore::open_at(&lab.index_path()).expect("opens the index");
    index.clear_profiles(Domain::Documents).expect("clears");
    assert_eq!(
        index_status(&lab.side()).documents,
        FolderProfileStatus::NeedsAnalyse
    );

    let vocabulary = ProfileVocabulary::load(&["health-fr"]).expect("vocabulary");
    let packs = PackSet::load("fr-FR", &["health-fr"]).expect("packs");
    let settings = ProfileSettings {
        vocabulary: &vocabulary,
        packs: &packs,
        gazetteer: None,
    };

    // Cancelled after three files: three profiles exist, the folder is not marked as profiled.
    let calls = std::cell::Cell::new(0);
    let cancel_after_three = || {
        calls.set(calls.get() + 1);
        calls.get() > 3
    };
    let partial = refresh_documents(&mut index, lab.work_path(), &settings, &cancel_after_three)
        .expect("runs");
    assert_eq!((partial.built, partial.complete), (3, false), "{partial:?}");
    assert_eq!(
        index_status(&lab.side()).documents,
        FolderProfileStatus::NeedsAnalyse
    );
    assert_eq!(count(&lab.side(), "file_profiles"), 3);

    // The next pass completes it, building only what is missing, and the result is the full one.
    let rest = refresh_documents(&mut index, lab.work_path(), &settings, &|| false).expect("runs");
    assert_eq!(
        (rest.built, rest.unchanged, rest.complete),
        (3, 3, true),
        "{rest:?}"
    );
    assert_eq!(
        index_status(&lab.side()).documents,
        FolderProfileStatus::Current
    );
    let resumed = profile_snapshot(&lab.side());

    index.clear_profiles(Domain::Documents).expect("clears");
    refresh_documents(&mut index, lab.work_path(), &settings, &|| false).expect("runs");
    // Row ids differ between the two builds; what must agree is the content of each profile.
    let strip = |snapshot: std::collections::BTreeMap<String, Vec<String>>| {
        let mut texts: Vec<String> = snapshot["file_profile_fields"].clone();
        texts.extend(snapshot["file_profile_entities"].clone());
        texts.sort();
        texts
    };
    assert_eq!(strip(resumed), strip(profile_snapshot(&lab.side())));
}

#[tokio::test]
async fn the_name_before_cleaning_is_recorded_and_findable() {
    let mut lab = Lab::new();
    lab.write("2026-10-01_ordonnance-pour-esaie.txt", PRESCRIPTION);
    record_original_names(
        &lab.side(),
        Domain::Documents,
        &[Renamed {
            from: "Ordonnance du mercredi.txt".to_string(),
            to: "2026-10-01_ordonnance-pour-esaie.txt".to_string(),
        }],
    )
    .expect("records");
    lab.analyse().await;

    let connection = lab.side();
    assert_eq!(
        fts_paths(&connection, "name:mercredi"),
        vec!["2026-10-01_ordonnance-pour-esaie.txt".to_string()]
    );
    assert_eq!(
        fts_paths(&connection, "name:esaie"),
        vec!["2026-10-01_ordonnance-pour-esaie.txt".to_string()]
    );

    // Renamed a second time, the earliest name is the one kept.
    record_original_names(
        &connection,
        Domain::Documents,
        &[Renamed {
            from: "2026-10-01_ordonnance-pour-esaie.txt".to_string(),
            to: "2026-10-01_ordonnance-esaie.txt".to_string(),
        }],
    )
    .expect("records");
    let kept = one_string(
        &connection,
        "SELECT original_name FROM file_profile_original_names WHERE relative_path = ?1",
        "2026-10-01_ordonnance-esaie.txt",
    );
    assert_eq!(kept.as_deref(), Some("Ordonnance du mercredi.txt"));
}
