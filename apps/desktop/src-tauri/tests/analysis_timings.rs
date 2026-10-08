//! Where an Analyse pass spends its time, as numbers (`knowledge::diagnostics`).
//!
//! The measurement the owner reads before anything is optimised
//! (`docs/test-reports/knowledge-base-pass-1/00-baseline.md`): so it must be right about what it
//! counts, and it must never hold a name or a passage. Both are asserted here against a real pass
//! with a scripted gateway.

mod common;

use std::path::Path;
use std::time::Duration;

use assistant_cabinet_ai_lib::data_folder;
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::indexing::{self, IndexSummary};
use assistant_cabinet_ai_lib::knowledge::diagnostics::{AnalysisPass, SLOWEST_FILES_KEPT};

use common::fake_gateway::{FakeGateway, Reply};

/// The same two lines in every file, as a letterhead and a signature are.
const LETTERHEAD: &str =
    "Cabinet exemple, 12 rue des Lilas, 75000 Ville. Ouvert du lundi au vendredi.";

async fn pass(work: &Path, index: &mut IndexStore, server_url: &str) -> IndexSummary {
    let gateway = GatewayClient::new().expect("builds a client");
    indexing::run(
        index,
        &gateway,
        server_url,
        "cabinet-embed",
        work,
        None,
        None,
        "fr-FR",
        &|_| {},
    )
    .await
    .expect("the pass completes")
}

fn open_index() -> (IndexStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp index dir");
    let index = IndexStore::open_at(&dir.path().join("index.sqlite3")).expect("opens the index");
    (index, dir)
}

fn write_letters(work: &Path, count: usize) {
    for number in 0..count {
        let body = format!(
            "{LETTERHEAD}\n\nCourrier numero {number}. Le sujet traite ici est le dossier {number}, \
             avec ses pieces et ses observations.\n\n{LETTERHEAD}"
        );
        std::fs::write(work.join(format!("courrier-{number}.txt")), body).expect("writes");
    }
}

#[tokio::test]
async fn a_pass_reports_what_it_read_embedded_and_wrote() {
    let gateway = FakeGateway::start(|_| Reply::slow(Duration::from_millis(40)));
    let work = tempfile::tempdir().expect("temp work folder");
    write_letters(work.path(), 7);
    let (mut index, _dir) = open_index();

    let summary = pass(work.path(), &mut index, &gateway.url).await;
    let timings = summary.timings.expect("a pass always measures itself");

    assert_eq!(timings.pass, AnalysisPass::Documents);
    assert_eq!(timings.files_scanned, 7);
    assert_eq!(timings.files_processed, 7);
    assert!(timings.embed_batches >= 7, "one request at least per file");
    assert!(
        timings.embed_ms >= 7 * 40,
        "seven requests of at least 40 ms were measured as {} ms",
        timings.embed_ms
    );
    assert!(timings.embed_ms <= timings.total_ms);
    assert!(timings.embedded_chars > 0);
    assert!(timings.chunks_total >= 7);
    assert_eq!(timings.ocr_ms, 0, "no scan, no engine, no OCR time");
    assert_eq!(
        timings.slowest_files.len(),
        SLOWEST_FILES_KEPT,
        "seven files processed, the five slowest kept"
    );
    assert!(timings
        .slowest_files
        .windows(2)
        .all(|pair| pair[0].total_ms >= pair[1].total_ms));
    assert!(timings
        .slowest_files
        .iter()
        .all(|file| file.position < 7 && file.embed_ms >= 40));
}

#[tokio::test]
async fn a_second_pass_over_unchanged_files_only_hashes() {
    let gateway = FakeGateway::start(|_| Reply::vectors());
    let work = tempfile::tempdir().expect("temp work folder");
    write_letters(work.path(), 3);
    let (mut index, _dir) = open_index();
    pass(work.path(), &mut index, &gateway.url).await;
    let requests_after_first = gateway.requests().len();

    let second = pass(work.path(), &mut index, &gateway.url).await;
    let timings = second.timings.expect("measured");

    assert_eq!(timings.files_scanned, 3);
    assert_eq!(timings.files_processed, 0);
    assert_eq!(timings.embed_batches, 0);
    assert_eq!(timings.chunks_total, 0);
    assert!(timings.slowest_files.is_empty());
    assert_eq!(
        gateway.requests().len(),
        requests_after_first,
        "nothing re-embedded"
    );
}

#[tokio::test]
async fn repeated_text_across_files_is_counted() {
    let gateway = FakeGateway::start(|_| Reply::vectors());
    let work = tempfile::tempdir().expect("temp work folder");
    // Three files holding exactly the same paragraph, and one that does not.
    for name in ["a.txt", "b.txt", "c.txt"] {
        std::fs::write(work.path().join(name), LETTERHEAD).expect("writes");
    }
    std::fs::write(
        work.path().join("d.txt"),
        "Un paragraphe qui ne ressemble a aucun autre.",
    )
    .expect("writes");
    let (mut index, _dir) = open_index();

    let summary = pass(work.path(), &mut index, &gateway.url).await;
    let timings = summary.timings.expect("measured");

    assert_eq!(timings.chunks_total, 4);
    assert_eq!(timings.chunks_repeating_earlier_text, 2);
}

/// The acceptance criterion: a pass's own numbers, serialised, carry no file name and no text.
#[tokio::test]
async fn the_timings_of_a_pass_carry_no_file_name_and_no_text() {
    let gateway = FakeGateway::start(|_| Reply::vectors());
    let work = tempfile::tempdir().expect("temp work folder");
    std::fs::create_dir(work.path().join("Dossier-Dupont")).expect("creates");
    std::fs::write(
        work.path()
            .join("Dossier-Dupont")
            .join("bilan-martine-roy.txt"),
        "Madame Martine Roy, nee le 3 mars 1961, telephone 0612345678.",
    )
    .expect("writes");
    let (mut index, _dir) = open_index();

    let summary = pass(work.path(), &mut index, &gateway.url).await;
    let line = serde_json::to_string(&summary.timings.expect("measured")).expect("serialises");

    for planted in [
        "Dupont",
        "martine",
        "Martine",
        "Roy",
        "roy",
        "0612345678",
        ".txt",
        "bilan",
    ] {
        assert!(!line.contains(planted), "{planted:?} found in {line}");
    }
    let value: serde_json::Value = serde_json::from_str(&line).expect("parses");
    assert_eq!(value["pass"], "documents");
    for (key, field) in value.as_object().expect("an object") {
        assert!(
            field.is_number() || field.is_array() || key == "pass",
            "{key} is neither a number, a list of timings nor the pass code: {field}"
        );
    }
}

#[test]
fn a_data_pass_reports_parsing_and_writing_per_workbook() {
    let work = tempfile::tempdir().expect("temp data folder");
    std::fs::write(
        work.path().join("factures.csv"),
        "date;fournisseur;montant\n01/03/2026;Alpha;12,50\n02/03/2026;Beta;8,00\n",
    )
    .expect("writes");
    std::fs::write(
        work.path().join("clients.csv"),
        "nom;ville\nMartin;Lyon\nDurand;Nice\n",
    )
    .expect("writes");
    let (mut index, _dir) = open_index();

    let summary = data_folder::analyse(work.path(), &mut index, "fr-FR", &|_| {})
        .expect("the pass completes");
    let timings = summary.timings.expect("a pass always measures itself");

    assert_eq!(timings.pass, AnalysisPass::Data);
    assert_eq!(timings.files_scanned, 2);
    assert_eq!(timings.files_processed, 2);
    assert_eq!(timings.embed_batches, 0, "tables are never embedded");
    assert_eq!(timings.slowest_files.len(), 2);
    assert!(timings.build_inventory_ms <= timings.total_ms);
    let line = serde_json::to_string(&timings).expect("serialises");
    assert!(
        !line.contains("factures") && !line.contains("Martin"),
        "{line}"
    );
}
