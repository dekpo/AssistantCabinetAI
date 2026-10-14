//! Acceptance of the file-name criterion (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 5.3): **100 %** of the name
//! queries that `scripts/file_profile_score.py` emits from the knowledge sandbox's own ground truth must
//! find the files they are meant to find, through the one function the Analyse and the left panel share
//! (`NameProfile`). The queries come from the script, not from this test, so the owner's list and the
//! product cannot drift apart.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use assistant_cabinet_ai_lib::file_profile::name::NameProfile;
use serde::Deserialize;

#[derive(Deserialize)]
struct Query {
    kind: String,
    query: String,
    expected: Vec<String>,
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the repository root is three levels above src-tauri")
        .to_path_buf()
}

/// Run the script with whichever Python this machine has. A machine with none fails loudly: a skipped
/// acceptance test would hide a regression.
fn emit_queries(out: &std::path::Path) -> Vec<Query> {
    let root = repository_root();
    let mut last_error = String::new();
    for python in ["python", "python3", "py"] {
        let result = Command::new(python)
            .current_dir(&root)
            .args([
                "scripts/file_profile_score.py",
                "--sandbox",
                "fixtures/knowledge-sandbox",
                "--emit-name-queries",
            ])
            .arg(out)
            .output();
        match result {
            Ok(output) if output.status.success() => {
                let text = std::fs::read_to_string(out).expect("the script wrote the queries");
                return serde_json::from_str(&text).expect("the queries are JSON");
            }
            Ok(output) => last_error = String::from_utf8_lossy(&output.stderr).to_string(),
            Err(error) => last_error = error.to_string(),
        }
    }
    panic!("no Python could run scripts/file_profile_score.py: {last_error}");
}

/// Every file of the sandbox, documents and workbooks, with no original name (nothing was cleaned).
fn sandbox_paths() -> Vec<String> {
    let root = repository_root().join("fixtures/knowledge-sandbox");
    let mut paths = Vec::new();
    for file in ["documents.json", "workbooks.json"] {
        let text = std::fs::read_to_string(root.join(file)).expect("reads the truth");
        let rows: Vec<serde_json::Value> = serde_json::from_str(&text).expect("the truth is JSON");
        paths.extend(
            rows.iter()
                .map(|row| row["path"].as_str().expect("a path").to_string()),
        );
    }
    paths
}

#[test]
fn every_name_query_finds_every_file_it_must_through_the_one_name_function() {
    let directory = tempfile::tempdir().expect("temporary folder");
    let queries = emit_queries(&directory.path().join("queries.json"));
    let profiles: Vec<NameProfile> = sandbox_paths()
        .iter()
        .map(|path| NameProfile::build(path, None))
        .collect();

    assert!(
        queries.len() >= 700,
        "the script emits the whole family: {}",
        queries.len()
    );
    let mut missed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut per_kind: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut extra_files = 0usize;
    for query in &queries {
        let found: Vec<&str> = profiles
            .iter()
            .filter(|profile| profile.matches(&query.query))
            .map(|profile| profile.relative_path.as_str())
            .collect();
        let entry = per_kind.entry(query.kind.clone()).or_default();
        entry.0 += 1;
        let lost: Vec<&String> = query
            .expected
            .iter()
            .filter(|path| !found.contains(&path.as_str()))
            .collect();
        if lost.is_empty() {
            entry.1 += 1;
        } else {
            missed
                .entry(format!("{}|{}", query.kind, query.query))
                .or_default()
                .extend(lost.into_iter().cloned());
        }
        extra_files += found
            .iter()
            .filter(|path| !query.expected.iter().any(|e| e == **path))
            .count();
    }
    println!(
        "name queries: {} ; per kind (queries, fully answered): {per_kind:?}",
        queries.len()
    );
    println!("files returned beyond the expected ones (the finder lists, it does not pick): {extra_files}");
    assert!(
        missed.is_empty(),
        "{} of {} name queries missed a file: {:?}",
        missed.len(),
        queries.len(),
        missed.iter().take(10).collect::<Vec<_>>()
    );
}
