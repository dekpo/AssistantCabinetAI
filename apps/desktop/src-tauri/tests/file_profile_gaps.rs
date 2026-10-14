//! Step 1 of the file profile work (`docs/LEXICAL-RETRIEVAL-DESIGN.md`, sections 3 and 5.3): tests that
//! **prove the gaps on today's code** before anything is changed.
//!
//! Each `today_...` test states what the product does now and fails the day it changes, so closing a
//! gap is a visible act: the test is rewritten to say what the new behaviour is. They are
//! characterisations, not wishes. The last test is the reference of the spreadsheet protection: the
//! helper `tabular_snapshot` sees a workbook and is stable across two passes, so the later steps can
//! compare before and after and require the three tables to be byte-identical.
//!
//! Every name is invented; the documents are ASCII French as the pilot writes them.

mod common;

use std::io::Write;

use common::knowledge::{entities, mentions_of, snapshot, tabular_snapshot, Lab};

/// A minimal DOCX that carries a creator and a title in `docProps/core.xml`, the way a word
/// processor writes them. The text of the document says neither.
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
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?><cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>{title}</dc:title><dc:creator>{creator}</dc:creator></cp:coreProperties>"
            ),
        ),
    ];
    for (name, body) in parts {
        zip.start_file(name, options).expect("starts a part");
        zip.write_all(body.as_bytes()).expect("writes a part");
    }
    zip.finish().expect("finishes the archive").into_inner()
}

/// G9a: the words of a file name are not in the full-text index, so a word that only the name
/// carries finds nothing.
#[tokio::test]
async fn today_the_words_of_a_file_name_are_not_searchable() {
    let mut lab = Lab::new();
    lab.write(
        "2026-10-01_courrier-dermatologie.txt",
        "Le patient a ete recu hier pour un controle annuel de routine.\n",
    );
    lab.write(
        "note-interne.txt",
        "Note sur le materiel de bureau et les fournitures.\n",
    );
    lab.analyse().await;

    assert!(
        lab.index
            .search_lexical("dermatologie", 10)
            .expect("searches")
            .is_empty(),
        "G9a is closed: the file name is now searchable. Rewrite this test as the new behaviour."
    );
    assert!(
        !lab.index.search_lexical("controle", 10).expect("searches").is_empty(),
        "the control word that is in the text must be found"
    );
}

/// G9b: a name of the base found inside a file name is linked only when the whole run of name-like
/// words is one entity. `martin-marie` is linked; `hug-martin-marie` (an organisation and a person in
/// one run) links neither.
#[tokio::test]
async fn today_a_file_name_that_mixes_an_organisation_and_a_person_links_nothing() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write(
        "dossier-initial.txt",
        "Madame Marie Martin a ete recue aux HUG pour un controle de routine.\n",
    );
    lab.analyse().await;

    lab.write("martin-marie.txt", "Note sur le materiel de bureau.\n");
    lab.write("hug-martin-marie.txt", "Autre note sur les fournitures.\n");
    lab.analyse().await;

    let connection = lab.side();
    let person = entities(&connection)
        .into_iter()
        .find(|row| row.type_id == "person" && row.canonical.contains("Martin"))
        .expect("the base knows Marie Martin from the first document");
    let by_name: Vec<String> = mentions_of(&connection, person.entity_id)
        .into_iter()
        .filter(|mention| mention.locator_kind == "filename")
        .map(|mention| mention.path)
        .collect();

    assert!(
        by_name.iter().any(|path| path == "martin-marie.txt"),
        "control: a name that is exactly the person is linked; found {by_name:?}"
    );
    assert!(
        !by_name.iter().any(|path| path == "hug-martin-marie.txt"),
        "G9b is closed: sub-runs of a file name are now linked. Rewrite this test."
    );
}

/// G1 and G2: the properties of a document (author, title) are not read at all, and no mention has
/// the `metadata` locator, although the schema allows it.
#[tokio::test]
async fn today_no_document_property_is_read_and_no_mention_is_a_metadata_mention() {
    let mut lab = Lab::new();
    lab.write_bytes(
        "compte-rendu.docx",
        &docx_with_properties("Zorglub Metadatus", "Titre de metadonnee"),
    );
    lab.analyse().await;

    let connection = lab.side();
    let metadata_mentions: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM kb_mentions WHERE locator_kind = 'metadata'",
            [],
            |row| row.get(0),
        )
        .expect("counts");
    assert_eq!(metadata_mentions, 0, "G2 is closed: something writes metadata mentions");

    let everywhere = lab
        .index
        .all_chunks()
        .expect("reads the chunks")
        .into_iter()
        .map(|chunk| chunk.text)
        .chain(common::knowledge::every_text_value(&connection))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !everywhere.contains("Zorglub") && !everywhere.contains("Titre de metadonnee"),
        "G1 is closed: the properties of a document now reach the index. Rewrite this test."
    );
}

/// The reference of the spreadsheet protection: the three tabular tables hold the workbook, and two
/// passes over unchanged files leave them identical. Later steps compare against this helper.
#[tokio::test]
async fn the_tabular_snapshot_sees_a_workbook_and_is_stable_across_two_passes() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr"];
    lab.write_data(
        "patients.csv",
        "Patient;Medecin traitant;Montant\nJean Dupont;Dr Martin;45\nMarie Martin;Dr Roy;60\n",
    );
    lab.analyse_data();

    let first = tabular_snapshot(&lab.side());
    // `tabular_workbooks` (the cells at rest) is cleared by every Analyse pass by design, so it is
    // compared all the same but only the inventory and the column readings must hold the workbook.
    assert!(
        !first["tabular_inventories"].is_empty(),
        "the workbook must be in the inventory table: {:?}",
        first.iter().map(|(table, rows)| (table.as_str(), rows.len())).collect::<Vec<_>>()
    );
    let knowledge_before = snapshot(&lab.side());

    lab.analyse_data();
    assert_eq!(first, tabular_snapshot(&lab.side()), "tabular tables changed on a second pass");
    assert_eq!(knowledge_before, snapshot(&lab.side()), "knowledge tables changed on a second pass");
}
