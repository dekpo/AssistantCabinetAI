//! The readers of the other text formats (VTT, SRT, RTF, EML, ODT, ODP, PPTX) go through the Analyse like a DOCX
//! does: the file is read, its text is cut into passages with their pages (a slide is a page), it is searchable
//! by a word of its text, it is no longer counted as a format nobody reads, and the file itself is never touched.
//! An OpenDocument spreadsheet joins the tabular side (the last test).

mod common;

use std::io::Write;

use assistant_cabinet_ai_lib::discovery::{discover, is_supported};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use common::knowledge::Lab;
use sha2::{Digest, Sha256};

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

const RTF: &str = r"{\rtf1\ansi\ansicpg1252{\fonttbl{\f0 Arial;}}\f0 Visit report\par The file number is 4471 and the total is CHF 385.\par}";

const EML: &str = "From: Anna Martin <anna@example.test>\nTo: office@example.test\nSubject: Quote D-5521\nDate: Thu, 12 Mar 2026 09:30:00 +0100\n\nThe quote total is CHF 3 420.00, valid for three months.\n";

const VTT: &str = "WEBVTT\n\n00:00:03.000 --> 00:00:06.000\n<v Anna>The cleaning contract renewal is on the agenda.\n\n00:00:07.000 --> 00:00:09.000\nWe keep the same provider.\n";

const SRT: &str = "1\n00:00:03,000 --> 00:00:06,000\nThe boiler maintenance visit is booked.\n\n2\n00:00:07,000 --> 00:00:09,000\n<i>The technician</i> comes on Tuesday.\n";

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

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

/// Analyses a folder holding the one file `name`, then proves the reader's contract: indexed, nothing failed,
/// no unread format, found by `query`, the pages in `pages`, the file byte for byte as it was.
async fn check_reader(name: &str, bytes: &[u8], query: &str, pages: &[u32]) {
    let mut lab = Lab::new();
    lab.write_bytes(name, bytes);
    let before = sha256(bytes);

    let summary = lab.analyse().await;

    assert_eq!(summary.indexed_files, 1, "{name}: {summary:?}");
    assert!(
        summary.failed_files.is_empty(),
        "{name}: {:?}",
        summary.failed_files
    );
    assert!(
        summary.empty_files.is_empty(),
        "{name}: {:?}",
        summary.empty_files
    );
    assert!(
        summary.unread_formats.is_empty(),
        "{name}: no format is left unread: {:?}",
        summary.unread_formats
    );
    assert_eq!(pages_of(&lab, name), pages, "{name}");

    let hits = lab.index.search_lexical(query, 5).unwrap();
    assert!(
        hits.iter().any(|hit| hit.chunk_id.starts_with(name)),
        "{name}: `{query}` should find it: {hits:?}"
    );

    // The folder card counts it as analysed: indexed, not unreadable, and the pass does not rename it.
    let inventory = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index)).unwrap();
    let card = inventory.summary();
    assert_eq!(
        (card.total_files, card.indexed_files, card.unreadable_files),
        (1, 1, 0),
        "{name}: {card:?}"
    );

    let on_disk = std::fs::read(lab.work_path().join(name)).unwrap();
    assert_eq!(sha256(&on_disk), before, "{name}: the file is untouched");
    assert_eq!(discover(lab.work_path()).len(), 1, "{name}");
}

#[tokio::test]
async fn a_vtt_transcript_is_analysed_searchable_and_left_untouched() {
    check_reader("call.vtt", VTT.as_bytes(), "cleaning contract", &[1]).await;
}

#[tokio::test]
async fn an_srt_subtitle_file_is_analysed_searchable_and_left_untouched() {
    check_reader("visit.srt", SRT.as_bytes(), "boiler maintenance", &[1]).await;
}

#[tokio::test]
async fn an_rtf_document_is_analysed_searchable_and_left_untouched() {
    check_reader("report.rtf", RTF.as_bytes(), "file number", &[1]).await;
}

#[tokio::test]
async fn an_eml_message_is_analysed_searchable_and_left_untouched() {
    check_reader("quote.eml", EML.as_bytes(), "quote total", &[1]).await;
}

#[tokio::test]
async fn an_odt_document_is_analysed_searchable_and_left_untouched() {
    check_reader("agreement.odt", &odt(), "monthly fee", &[1]).await;
}

#[tokio::test]
async fn an_odp_presentation_has_one_page_per_slide() {
    check_reader("meeting.odp", &odp(), "autoclave budget", &[1, 2]).await;
}

#[tokio::test]
async fn a_pptx_presentation_has_one_page_per_slide() {
    check_reader("training.pptx", &pptx(), "hygiene session", &[1, 2]).await;
}

#[tokio::test]
async fn a_file_that_cannot_be_read_is_named_and_does_not_stop_the_others() {
    let mut lab = Lab::new();
    lab.write_bytes("broken.pptx", b"this is not a zip");
    lab.write("fine.vtt", VTT);

    let summary = lab.analyse().await;

    assert_eq!(summary.indexed_files, 1, "{summary:?}");
    assert_eq!(summary.failed_files.len(), 1, "{summary:?}");
    assert_eq!(summary.failed_files[0].path, "broken.pptx");
}

#[tokio::test]
async fn a_file_with_no_text_is_reported_empty_never_guessed_at() {
    let mut lab = Lab::new();
    lab.write("silence.srt", "1\n00:00:01,000 --> 00:00:02,000\n<i></i>\n");
    lab.write("blank.rtf", r"{\rtf1\ansi{\fonttbl{\f0 Arial;}}\f0 \par}");

    let summary = lab.analyse().await;

    assert_eq!(summary.indexed_files, 0, "{summary:?}");
    let mut empty = summary.empty_files.clone();
    empty.sort();
    assert_eq!(empty, vec!["blank.rtf", "silence.srt"], "{summary:?}");
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
async fn an_ods_workbook_joins_the_tabular_side_and_the_document_pass_does_not_read_it() {
    let mut lab = Lab::new();
    let bytes = zip_with(&[
        ("mimetype", "application/vnd.oasis.opendocument.spreadsheet"),
        // A real spreadsheet always carries its manifest; the reader refuses an archive without it.
        ("META-INF/manifest.xml", ODS_MANIFEST),
        ("content.xml", ODS_CONTENT),
    ]);
    lab.write_data_bytes("invoices.ods", &bytes);

    let summary = lab.analyse_data();

    assert_eq!(summary.indexed_files, 1, "{summary:?}");
    assert!(summary.empty_files.is_empty(), "{summary:?}");
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

    // It is a workbook, not a document: the document pass does not read it, and the file is as it was.
    assert!(!is_supported("ods"));
    let on_disk = std::fs::read(lab.data.path().join("invoices.ods")).unwrap();
    assert_eq!(sha256(&on_disk), sha256(&bytes));
}
