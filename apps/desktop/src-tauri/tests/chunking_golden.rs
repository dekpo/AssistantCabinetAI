//! Golden test for short documents (long-documents programme, tab 2, lot 0; owner rule: below the "long"
//! threshold the chunks, ids, texts and order are byte-identical to the ones that exist today).
//!
//! It reads every short, readable, text-layer document of the versioned sandbox through the product's own
//! extraction and chunker, and compares a hash of its chunks (id, page, ordinal, text, in order) with the
//! recorded one in `tests/golden/short-chunks.golden`. Any later change to the extraction, the cleaning or the
//! chunker that moves a single character of a short document fails here, and the owner decides whether the
//! change is accepted (then: `ACAI_BLESS_GOLDEN=1 cargo test --test chunking_golden`, and say so in the report).
//!
//! The documents are the 200 of `fixtures/knowledge-sandbox`; the long ones (`special = "long"`) and the scans
//! (OCR is fake in the laboratory, measured elsewhere) are left out.

use std::fmt::Write as _;
use std::path::PathBuf;

use assistant_cabinet_ai_lib::chunking::chunk;
use assistant_cabinet_ai_lib::discovery::DiscoveredFile;
use assistant_cabinet_ai_lib::extraction::extract;
use serde_json::Value;
use sha2::{Digest, Sha256};

fn sandbox() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("fixtures")
        .join("knowledge-sandbox")
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("short-chunks.golden")
}

fn current() -> String {
    let root = sandbox();
    let documents: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("documents.json")).expect("reads documents.json"),
    )
    .expect("parses documents.json");
    let mut lines = String::new();
    for record in documents.as_array().expect("an array") {
        let path = record["path"].as_str().expect("a path");
        let extension = record["extension"].as_str().unwrap_or("");
        let short_text = record["readable"].as_bool().unwrap_or(false)
            && !record["ocr"].as_bool().unwrap_or(false)
            && record["special"].as_str().unwrap_or("") != "long"
            && matches!(extension, "pdf" | "docx" | "txt" | "md");
        if !short_text {
            continue;
        }
        let absolute = root.join("documents").join(path);
        let file = DiscoveredFile {
            relative_path: path.to_string(),
            absolute_path: absolute.to_string_lossy().to_string(),
            extension: extension.to_string(),
            size_bytes: 0,
            modified_at: None,
        };
        let extracted = extract(&file, None, None, "fr-FR")
            .unwrap_or_else(|error| panic!("extracts {path}: {error}"));
        let chunks = chunk(&extracted);
        let mut hasher = Sha256::new();
        for c in &chunks {
            hasher.update(format!(
                "{}\0{}\0{}\0{}\n",
                c.chunk_id, c.page_number, c.section, c.text
            ));
        }
        let digest = hasher.finalize();
        let mut hex = String::new();
        for byte in digest.iter() {
            write!(hex, "{byte:02x}").expect("writes to a string");
        }
        writeln!(lines, "{path}\t{}\t{hex}", chunks.len()).expect("writes to a string");
    }
    lines
}

#[test]
fn the_chunks_of_short_documents_are_byte_identical_to_the_recorded_ones() {
    let now = current();
    assert!(
        now.lines().count() > 100,
        "the sandbox should hold well over a hundred short text documents, found {}",
        now.lines().count()
    );
    if std::env::var("ACAI_BLESS_GOLDEN").is_ok() {
        std::fs::create_dir_all(golden_path().parent().expect("a folder"))
            .expect("creates the folder");
        std::fs::write(golden_path(), &now).expect("writes the golden file");
        return;
    }
    let recorded = std::fs::read_to_string(golden_path()).unwrap_or_else(|_| {
        panic!(
            "no golden file at {}: record it once with ACAI_BLESS_GOLDEN=1",
            golden_path().display()
        )
    });
    let differing: Vec<&str> = now
        .lines()
        .filter(|line| !recorded.lines().any(|r| r == *line))
        .collect();
    assert!(
        differing.is_empty() && now.lines().count() == recorded.lines().count(),
        "the chunks of {} short document(s) changed, first: {:?}",
        differing.len(),
        differing.first()
    );
}
