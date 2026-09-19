//! Extraction against the real sandbox fixtures rather than in-memory stand-ins.
//!
//! Confirms the boundary this session cares about most: a PDF with a text layer reads back its
//! text, a DOCX reads back its paragraphs, and a PDF with no text layer is reported empty rather
//! than silently dropped (`docs/SESSION-DOCUMENT-EXTRACTION.md`).

use std::path::PathBuf;

use assistant_cabinet_ai_lib::discovery::DiscoveredFile;
use assistant_cabinet_ai_lib::extraction::extract;

fn sandbox_file(name: &str) -> DiscoveredFile {
    let path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("fixtures")
        .join("gp-sandbox")
        .join("inbox")
        .join(name);
    assert!(path.exists(), "fixture missing: {}", path.display());
    let extension = path.extension().unwrap().to_string_lossy().to_lowercase();
    DiscoveredFile {
        relative_path: format!("inbox/{name}"),
        absolute_path: path.display().to_string(),
        extension,
        size_bytes: 0,
        modified_at: None,
    }
}

#[test]
fn a_native_pdf_extracts_its_text_layer() {
    let document =
        extract(&sandbox_file("2026-03-12_compte-rendu-biologie.pdf")).expect("extracts");

    assert!(!document.empty);
    assert!(document.pages[0].text.contains("HbA1c"));
    assert!(document.pages[0].text.contains("6.8"));
}

#[test]
fn a_multi_page_pdf_keeps_pages_separate() {
    let document = extract(&sandbox_file("2026-03-18_courrier-neurologie.pdf")).expect("extracts");

    assert_eq!(document.pages.len(), 2);
    assert!(document.pages[1].text.contains("Cephalees"));
}

#[test]
fn a_docx_extracts_its_paragraphs() {
    let document =
        extract(&sandbox_file("2026-03-14_courrier-endocrinologie.docx")).expect("extracts");

    assert!(!document.empty);
    assert!(document.pages[0].text.contains("metformine"));
}

#[test]
fn a_scanned_pdf_with_no_text_layer_is_reported_empty_not_dropped() {
    let document = extract(&sandbox_file("2026-03-20_radiographie-scan.pdf")).expect("extracts");

    assert!(document.empty);
}
