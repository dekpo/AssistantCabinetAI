//! The readers of the other text formats (ODT, ODP, PPTX, RTF, EML, VTT) go through the Analyse like a
//! DOCX does: the file is read, its text is cut into passages with their pages (a slide is a page), it is
//! searchable, it is no longer counted as a format nobody reads, and the file itself is never touched.

mod common;

use std::io::Write;

use assistant_cabinet_ai_lib::discovery::discover_all;
use common::knowledge::Lab;

fn zip_with(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }
    buffer.into_inner()
}

const ODF_HEAD: &str = r#"<office:document-content xmlns:office="o" xmlns:text="t" xmlns:draw="d" xmlns:presentation="p">"#;

fn odt() -> Vec<u8> {
    zip_with(&[(
        "content.xml",
        &format!("{ODF_HEAD}<office:body><office:text><text:h>Supplier agreement</text:h><text:p>The monthly fee is CHF 640 and the term is 24 months.</text:p></office:text></office:body></office:document-content>"),
    )])
}

fn odp() -> Vec<u8> {
    zip_with(&[(
        "content.xml",
        &format!("{ODF_HEAD}<office:body><office:presentation><draw:page><text:p>Opening remarks</text:p></draw:page><draw:page><text:p>Autoclave budget CHF 9 800</text:p></draw:page></office:presentation></office:body></office:document-content>"),
    )])
}

fn pptx() -> Vec<u8> {
    let slide = |text: &str| {
        format!(
            "<p:sld xmlns:a=\"a\" xmlns:p=\"p\"><a:p><a:r><a:t>{text}</a:t></a:r></a:p></p:sld>"
        )
    };
    zip_with(&[
        ("ppt/slides/slide1.xml", &slide("Training day agenda")),
        (
            "ppt/slides/slide2.xml",
            &slide("Hand hygiene session on 9 October 2026"),
        ),
    ])
}

const RTF: &str = r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0 Arial;}}\f0 Visit report\par The patient file number is 4471 and the total is CHF 385.\par}";

const EML: &str = "From: Anna Martin <anna@example.test>\nTo: office@example.test\nSubject: Quote D-5521\nDate: Thu, 12 Mar 2026 09:30:00 +0100\n\nThe quote total is CHF 3 420.00, valid for three months.\n";

const VTT: &str = "WEBVTT\n\n00:00:03.000 --> 00:00:06.000\n<v Anna>The cleaning contract renewal is on the agenda.\n\n00:00:07.000 --> 00:00:09.000\nWe keep the same provider.\n";

fn pages_of(lab: &Lab, path: &str) -> Vec<u32> {
    let mut pages: Vec<u32> = lab
        .index
        .chunks_for_document(path)
        .unwrap()
        .iter()
        .map(|chunk| chunk.page_number)
        .collect();
    pages.sort_unstable();
    pages.dedup();
    pages
}

#[tokio::test]
async fn each_new_format_is_analysed_searchable_and_left_untouched() {
    let mut lab = Lab::new();
    lab.write_bytes("agreement.odt", &odt());
    lab.write_bytes("meeting.odp", &odp());
    lab.write_bytes("training.pptx", &pptx());
    lab.write("report.rtf", RTF);
    lab.write("quote.eml", EML);
    lab.write("call.vtt", VTT);
    let before: Vec<(String, u64)> = discover_all(lab.work_path())
        .iter()
        .map(|file| (file.relative_path.clone(), file.size_bytes))
        .collect();

    let summary = lab.analyse().await;
    assert_eq!(summary.indexed_files, 6, "{summary:?}");
    assert!(
        summary.failed_files.is_empty(),
        "{:?}",
        summary.failed_files
    );
    assert!(
        summary.unread_formats.is_empty(),
        "no format is left unread: {:?}",
        summary.unread_formats
    );

    // A slide is a page.
    assert_eq!(pages_of(&lab, "meeting.odp"), vec![1, 2]);
    assert_eq!(pages_of(&lab, "training.pptx"), vec![1, 2]);
    assert_eq!(pages_of(&lab, "agreement.odt"), vec![1]);

    // Each is found by a word of its text.
    for (word, path) in [
        ("monthly fee", "agreement.odt"),
        ("autoclave budget", "meeting.odp"),
        ("hygiene session", "training.pptx"),
        ("patient file number", "report.rtf"),
        ("quote total", "quote.eml"),
        ("cleaning contract", "call.vtt"),
    ] {
        let paths: Vec<String> = lab
            .index
            .all_documents()
            .unwrap()
            .into_iter()
            .map(|d| d.relative_path)
            .collect();
        let first_word = word.split(' ').next().unwrap().to_string();
        let hits = lab
            .index
            .search_lexical_terms_in(&[first_word], 20, &paths)
            .unwrap();
        let found: Vec<String> = hits
            .iter()
            .filter_map(|hit| hit.chunk_id.split('#').next().map(str::to_string))
            .collect();
        assert!(found.contains(&path.to_string()), "{word}: {found:?}");
    }

    // The files are exactly as they were.
    let after: Vec<(String, u64)> = discover_all(lab.work_path())
        .iter()
        .map(|file| (file.relative_path.clone(), file.size_bytes))
        .collect();
    assert_eq!(before, after);
}

#[tokio::test]
async fn a_file_that_cannot_be_read_is_named_and_does_not_stop_the_others() {
    let mut lab = Lab::new();
    lab.write_bytes("broken.pptx", b"this is not a zip");
    lab.write("fine.vtt", VTT);
    let summary = lab.analyse().await;
    assert_eq!(summary.indexed_files, 1);
    assert_eq!(summary.failed_files.len(), 1);
    assert_eq!(summary.failed_files[0].path, "broken.pptx");
}

const ODS_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2">
<manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.spreadsheet"/>
<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
</manifest:manifest>"#;

const ODS_CONTENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.2">
<office:body><office:spreadsheet><table:table table:name="Invoices">
<table:table-row><table:table-cell office:value-type="string"><text:p>supplier</text:p></table:table-cell><table:table-cell office:value-type="string"><text:p>amount</text:p></table:table-cell></table:table-row>
<table:table-row><table:table-cell office:value-type="string"><text:p>MedSupply</text:p></table:table-cell><table:table-cell office:value-type="float" office:value="1450"><text:p>1450</text:p></table:table-cell></table:table-row>
<table:table-row><table:table-cell office:value-type="string"><text:p>OfficeCo</text:p></table:table-cell><table:table-cell office:value-type="float" office:value="320.5"><text:p>320.5</text:p></table:table-cell></table:table-row>
</table:table></office:spreadsheet></office:body></office:document-content>"#;

#[tokio::test]
async fn an_ods_workbook_joins_the_tabular_side_and_its_cells_are_read() {
    let mut lab = Lab::new();
    lab.write_data_bytes(
        "invoices.ods",
        &zip_with(&[
            ("mimetype", "application/vnd.oasis.opendocument.spreadsheet"),
            // A real spreadsheet always carries its manifest; the reader refuses an archive without it.
            ("META-INF/manifest.xml", ODS_MANIFEST),
            ("content.xml", ODS_CONTENT),
        ]),
    );
    let summary = lab.analyse_data();
    assert_eq!(summary.indexed_files, 1, "{summary:?}");
    let inventories = lab.index.all_tabular_inventories().unwrap();
    assert_eq!(inventories.len(), 1);
    let inventory = lab
        .index
        .tabular_inventory_by_hash(&inventories[0].workbook_id)
        .unwrap()
        .expect("the inventory is cached");
    assert_eq!(inventory.relative_path, "invoices.ods");
    assert_eq!(
        serde_json::to_value(inventory.format).unwrap(),
        serde_json::json!("ods")
    );
    let columns: Vec<&str> = inventory.sheets[0]
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect();
    assert_eq!(columns, vec!["supplier", "amount"]);
    // It is a workbook, not a document: the document pass does not read it.
    assert!(!assistant_cabinet_ai_lib::discovery::is_supported("ods"));
}
