//! Walks the human test of the file finder (`docs/test-reports/file-finder/human-test.md`) on temporary
//! copies, the way the owner will play it: the same 30 invented documents and the workbooks that her
//! `lot11_helper.py prepare 30` writes, analysed in both folders, then every search typed in the field and
//! every sentence typed in the chat, answered by the same code the application runs.
//!
//! `#[ignore]`: it prints what the human test quotes. Run explicitly:
//!
//! ```text
//! cargo test --test file_finder_walk -- --ignored --nocapture
//! ```

mod common;

use assistant_cabinet_ai_lib::discovery::discover_all;
use assistant_cabinet_ai_lib::file_profile::build::Domain;
use assistant_cabinet_ai_lib::file_profile::vocabulary::ProfileVocabulary;
use assistant_cabinet_ai_lib::finder::set_search::{answer, identifier_forms};
use assistant_cabinet_ai_lib::finder::{find, FinderQuery, FinderScope, InventoryFile};
use assistant_cabinet_ai_lib::folder_questions::{route, FolderAnswer, QuestionRoute};
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use assistant_cabinet_ai_lib::knowledge::packs::PackSet;
use common::knowledge::Lab;
use common::validation::Truth;

const LOCALE: &str = "fr-FR";
const PACKS: [&str; 2] = ["health-fr", "health-ch"];

fn inventory(folder: &std::path::Path) -> Vec<InventoryFile> {
    discover_all(folder)
        .iter()
        .map(InventoryFile::from)
        .collect()
}

fn panel(lab: &Lab, domain: Domain, files: &[InventoryFile], typed: &str) {
    let packs = PackSet::load(LOCALE, &PACKS).expect("packs");
    let vocabulary = ProfileVocabulary::load(&PACKS).expect("vocabulary");
    let query = FinderQuery {
        text: Some(typed.to_string()),
        identifiers: identifier_forms(typed, &packs, &vocabulary),
        limit: Some(100_000),
        paths_only: true,
        ..FinderQuery::default()
    };
    let result = find(
        &lab.side(),
        domain,
        files,
        &query,
        &FinderScope::WholeFolder,
    )
    .expect("finds");
    let analysed: Vec<&str> = result
        .files
        .iter()
        .filter(|hit| hit.card.state == "analysed")
        .map(|hit| hit.relative_path.as_str())
        .collect();
    println!(
        "FIELD {domain:?} {typed:?}: found {} selected {} not analysed {}",
        result.total,
        analysed.len(),
        result.total - analysed.len()
    );
    for hit in result.files.iter().take(12) {
        println!("    {} [{}]", hit.relative_path, hit.card.state);
    }
}

#[tokio::test]
#[ignore = "prints what the human test quotes"]
async fn print_the_answers_of_the_human_test() {
    let truth = Truth::load();
    let mut lab = Lab::new();
    lab.packs = PACKS.to_vec();
    for doc in truth.docs.iter().filter(|d| d.index <= 30) {
        let to = lab.work_path().join(&doc.path);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(truth.root.join("documents").join(&doc.path), to).unwrap();
    }
    for book in &truth.books {
        lab.write_data_bytes(
            &book.path,
            &std::fs::read(truth.root.join("data").join(&book.path)).unwrap(),
        );
    }
    lab.analyse().await;
    lab.analyse_data();
    let documents = inventory(lab.work_path());
    let data = inventory(lab.data.path());
    println!(
        "documents folder: {} files, data folder: {} files",
        documents.len(),
        data.len()
    );
    let connection = lab.side();
    let status = assistant_cabinet_ai_lib::file_profile::store::index_status(&connection);
    println!("profile status: {status:?}");

    {
        let pattern = regex::Regex::new(r"\b756[. ]?\d{4}[. ]?\d{4}[. ]?\d{2}\b").expect("pattern");
        let mut statement = connection
            .prepare("SELECT relative_path, text FROM chunks")
            .expect("prepares");
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("reads");
        let mut seen = std::collections::BTreeMap::<String, Vec<String>>::new();
        for (path, text) in rows.flatten() {
            for found in pattern.find_iter(&text) {
                let compact: String = found
                    .as_str()
                    .chars()
                    .filter(char::is_ascii_digit)
                    .collect();
                let holders = seen.entry(compact).or_default();
                if !holders.contains(&path) {
                    holders.push(path.clone());
                }
            }
        }
        for (number, holders) in seen.iter().take(4) {
            println!("AVS {number}: {holders:?}");
        }
    }
    for typed in std::env::var("ACAI_WALK_FIELD")
        .unwrap_or_default()
        .split('|')
        .filter(|t| !t.is_empty())
    {
        panel(&lab, Domain::Documents, &documents, typed);
    }
    for typed in std::env::var("ACAI_WALK_DATA")
        .unwrap_or_default()
        .split('|')
        .filter(|t| !t.is_empty())
    {
        panel(&lab, Domain::Data, &data, typed);
    }

    let inventory_now =
        WorkFolderInventory::discover(lab.work_path(), Some(&lab.index)).expect("discovers");
    for question in std::env::var("ACAI_WALK_CHAT")
        .unwrap_or_default()
        .split('|')
        .filter(|t| !t.is_empty())
    {
        match route(&inventory_now, &lab.index, question, LOCALE) {
            QuestionRoute::SetSearch(set) => {
                let packs = PackSet::load(LOCALE, &PACKS).expect("packs");
                let vocabulary = ProfileVocabulary::load(&PACKS).expect("vocabulary");
                let forms = identifier_forms(&set.text, &packs, &vocabulary);
                let scoped: Vec<InventoryFile> = inventory_now
                    .all_files()
                    .iter()
                    .map(InventoryFile::from)
                    .collect();
                let found = answer(&connection, &scoped, &set, forms).expect("answers");
                if let FolderAnswer::DocumentSearch {
                    total,
                    listed,
                    others,
                    coverage,
                    ..
                } = &found
                {
                    println!("CHAT {question:?}: set search {total} files, coverage {coverage:?}");
                    for item in listed.iter().chain(std::iter::empty()).take(6) {
                        println!(
                            "    {} page {:?}: {:?}",
                            item.relative_path, item.page, item.excerpt
                        );
                    }
                    println!("    others: {}", others.len());
                }
            }
            QuestionRoute::Deterministic(FolderAnswer::RankedFile {
                ranking,
                candidates,
                ..
            }) => {
                println!("CHAT {question:?}: ranking of {candidates}: {ranking:?}");
            }
            other => println!("CHAT {question:?}: {other:?}"),
        }
    }
}
