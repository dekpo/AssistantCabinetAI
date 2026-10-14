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

use assistant_cabinet_ai_lib::file_profile::metadata::{
    read_metadata, MetadataKind, MetadataSource,
};
use assistant_cabinet_ai_lib::file_profile::name::{link_name_words, words_of, NameProfile};
use assistant_cabinet_ai_lib::file_profile::vocabulary::ProfileVocabulary;
use assistant_cabinet_ai_lib::knowledge::gazetteer::Gazetteer;
use common::knowledge::{entities, snapshot, tabular_snapshot, Lab};

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

/// G9a closed (step 3): the words of a file name are searchable through the name profile, the one
/// function the Analyse and the left panel share, for a file that was just analysed and for one that
/// never was. The passages' own index (`chunks_fts`) is unchanged on purpose (design 8): the name's
/// words get their own columns in the profile's full-text table (step 4).
#[tokio::test]
async fn the_words_of_a_file_name_are_searchable_through_the_name_profile() {
    let mut lab = Lab::new();
    lab.write(
        "2026-10-01_courrier-dermatologie.txt",
        "Le patient a ete recu hier pour un controle annuel de routine.
",
    );
    lab.write(
        "note-interne.txt",
        "Note sur le materiel de bureau et les fournitures.
",
    );
    lab.analyse().await;

    let letter = NameProfile::build("2026-10-01_courrier-dermatologie.txt", None);
    let note = NameProfile::build("note-interne.txt", None);
    for query in [
        "dermatologie",
        "derma",
        "dermatologie courrier",
        "2026-10-01_courrier-dermatologie.txt",
    ] {
        assert!(
            letter.matches(query),
            "{query:?} must find the letter by its name"
        );
        assert!(!note.matches(query), "{query:?} must not find the note");
    }
    assert!(
        lab.index
            .search_lexical("dermatologie", 10)
            .expect("searches")
            .is_empty(),
        "the passages' index is left as it was: names get their own columns"
    );
    assert!(
        !lab.index
            .search_lexical("controle", 10)
            .expect("searches")
            .is_empty(),
        "the control word that is in the text must be found"
    );
}

/// G9b closed (step 3): a name of the base found **anywhere** in a file name is linked, not only when
/// the whole stem is one entity. `hug-martin-marie` holds an organisation and a person; both are known
/// from the first document, both are linked, and nothing is created.
#[tokio::test]
async fn a_file_name_that_mixes_an_organisation_and_a_person_links_both() {
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    lab.write(
        "dossier-initial.txt",
        "Madame Marie Martin a ete recue aux HUG pour un controle de routine.
",
    );
    lab.analyse().await;

    let connection = lab.side();
    let known = entities(&connection);
    let person = known
        .iter()
        .find(|row| row.type_id == "person" && row.canonical.contains("Martin"))
        .expect("the base knows Marie Martin from the first document");
    let organisation = known
        .iter()
        .find(|row| row.type_id == "organization")
        .expect("the base knows the HUG from the first document");
    let entities_before = known.len();

    let gazetteer = Gazetteer::rebuild_from_store(&connection).expect("builds the gazetteer");
    let linked = |stem: &str| -> Vec<i64> {
        link_name_words(&words_of(stem), &gazetteer)
            .into_iter()
            .map(|link| link.entity_id)
            .collect()
    };

    let control = linked("martin-marie");
    assert!(
        control.contains(&person.entity_id),
        "control: the person alone is linked: {control:?}"
    );
    let mixed = linked("hug-martin-marie");
    assert!(
        mixed.contains(&organisation.entity_id),
        "the organisation is linked: {mixed:?}"
    );
    assert!(
        mixed.contains(&person.entity_id),
        "the person is linked: {mixed:?}"
    );
    assert!(
        linked("decompte-note-interne").is_empty(),
        "words nobody knows link nothing"
    );
    assert_eq!(
        entities(&connection).len(),
        entities_before,
        "a name never creates an entity"
    );
}

/// G1 closed at the reader level (step 2): the properties of a document are read, with their source and
/// a confidence. G2 stays as designed (design 4.8): they are **not** written as `kb_mentions`, because
/// a metadata mention would change which files targeting keeps; the profile's own tables (step 4) will
/// hold them. The index text is unchanged too: a property is never mixed into the passages.
#[tokio::test]
async fn a_document_property_is_read_by_the_profile_and_reaches_neither_the_knowledge_base_nor_the_passages(
) {
    let mut lab = Lab::new();
    lab.write_bytes(
        "compte-rendu.docx",
        &docx_with_properties("Zorglub Metadatus", "Titre de metadonnee"),
    );
    lab.analyse().await;

    let vocabulary = ProfileVocabulary::load(&[]).expect("loads the profile vocabulary");
    let reading = read_metadata(
        &lab.work_path().join("compte-rendu.docx"),
        "docx",
        &vocabulary,
    );
    let author = reading
        .first(MetadataKind::Author)
        .expect("G1 is closed: the author is read");
    assert_eq!(author.value, "Zorglub Metadatus");
    assert_eq!(author.source, MetadataSource::DocxCore);
    assert_eq!(
        reading
            .first(MetadataKind::Title)
            .map(|field| field.value.as_str()),
        Some("Titre de metadonnee")
    );

    let connection = lab.side();
    let metadata_mentions: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM kb_mentions WHERE locator_kind = 'metadata'",
            [],
            |row| row.get(0),
        )
        .expect("counts");
    assert_eq!(
        metadata_mentions, 0,
        "design 4.8: metadata evidence stays out of kb_mentions"
    );

    let everywhere = lab
        .index
        .all_chunks()
        .expect("reads the chunks")
        .into_iter()
        .map(|chunk| chunk.text)
        .chain(common::knowledge::every_text_value(&connection))
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    assert!(
        !everywhere.contains("Zorglub") && !everywhere.contains("Titre de metadonnee"),
        "a property is never mixed into the passages or the knowledge base"
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
        first
            .iter()
            .map(|(table, rows)| (table.as_str(), rows.len()))
            .collect::<Vec<_>>()
    );
    let knowledge_before = snapshot(&lab.side());

    lab.analyse_data();
    assert_eq!(
        first,
        tabular_snapshot(&lab.side()),
        "tabular tables changed on a second pass"
    );
    assert_eq!(
        knowledge_before,
        snapshot(&lab.side()),
        "knowledge tables changed on a second pass"
    );
}
