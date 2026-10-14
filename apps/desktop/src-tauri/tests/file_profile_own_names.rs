//! Tab 1 of the long-documents chain, item 7: the user's own names, read from the settings, are never the
//! author of a file. A word processor stamps the licence holder on every document; without this list she would
//! be the author of everything. Every name is invented.

mod common;

use std::io::Write;

use common::knowledge::{profile_snapshot, Lab};
use rusqlite::Connection;

fn docx_with_creator(creator: &str) -> Vec<u8> {
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
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\"><dc:title>Reunion</dc:title><dc:creator>{creator}</dc:creator></cp:coreProperties>"),
        ),
    ];
    for (name, body) in parts {
        zip.start_file(name, options).expect("starts a part");
        zip.write_all(body.as_bytes()).expect("writes a part");
    }
    zip.finish().expect("finishes the archive").into_inner()
}

fn authors(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare("SELECT relative_path FROM file_profile_fts WHERE file_profile_fts MATCH 'author:zorglub' ORDER BY relative_path")
        .expect("prepares");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads")
}

#[tokio::test]
async fn a_property_that_holds_the_users_own_name_does_not_make_her_the_author() {
    let mut lab = Lab::new();
    lab.write_bytes("compte-rendu.docx", &docx_with_creator("Zorglub Metadatus"));
    lab.write_bytes("autre.docx", &docx_with_creator("Ylva Reviewer"));

    // Without her names, the creator of the properties is the author.
    let first = lab.analyse().await.profile.expect("the profile pass ran");
    assert_eq!(first.built, 2);
    assert_eq!(authors(&lab.side()), vec!["compte-rendu.docx".to_string()]);

    // With her name in the settings, the next Analyse reads that file again and no longer lists her.
    lab.own_names = vec!["Zorglub Metadatus".to_string()];
    let second = lab.analyse().await.profile.expect("the profile pass ran");
    assert_eq!(
        second.built, 2,
        "a change of her names builds the profiles again"
    );
    assert!(
        authors(&lab.side()).is_empty(),
        "she is the author of nothing"
    );
    let other: Option<String> = lab
        .side()
        .query_row(
            "SELECT value FROM file_profile_fields WHERE relative_path = 'autre.docx' AND kind = 'metadata' AND detail = 'author'",
            [],
            |row| row.get(0),
        )
        .ok();
    assert_eq!(
        other.as_deref(),
        Some("Ylva Reviewer"),
        "another author is kept"
    );
    let ignored: i64 = lab
        .side()
        .query_row(
            "SELECT COUNT(*) FROM file_profile_fields WHERE relative_path = 'compte-rendu.docx' AND kind = 'metadata_ignored'",
            [],
            |row| row.get(0),
        )
        .expect("counts");
    assert_eq!(
        ignored, 1,
        "the property is kept as ignored, with its reason"
    );

    // The same names change nothing the next time.
    let before = profile_snapshot(&lab.side());
    let third = lab.analyse().await.profile.expect("the profile pass ran");
    assert_eq!((third.built, third.unchanged), (0, 2));
    assert_eq!(before, profile_snapshot(&lab.side()));
}
