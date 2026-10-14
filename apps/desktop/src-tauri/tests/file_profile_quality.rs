//! Quality of the file profile on the knowledge sandbox: dump what `build_profile` fills for every file,
//! for `scripts/file_profile_score.py` to score against the sandbox's own ground truth.
//!
//! ```text
//! cargo test --test file_profile_quality -- --ignored --nocapture
//! python scripts/file_profile_score.py --sandbox fixtures/knowledge-sandbox --profiles <dump>
//! ```
//!
//! The dump is written to `FILE_PROFILE_DUMP` when it is set, else to the temporary folder; the path is
//! printed. Text comes from the product's own extraction with **no text recognition** (a scan is typed
//! from its name only, as it would be without an engine); formats the product cannot read yet (ODT, RTF,
//! EML, VTT, PPTX...) are typed from their name only, as the product would. Ignored by default: it reads
//! the whole sandbox and is a measure, not a regression guard.

use std::path::PathBuf;

mod common;

use assistant_cabinet_ai_lib::discovery::DiscoveredFile;
use assistant_cabinet_ai_lib::extraction::extract;
use assistant_cabinet_ai_lib::file_profile::build::{
    build_profile, Domain, FileProfile, ProfileInput,
};
use assistant_cabinet_ai_lib::file_profile::metadata::read_metadata;
use assistant_cabinet_ai_lib::file_profile::vocabulary::ProfileVocabulary;
use assistant_cabinet_ai_lib::knowledge::packs::PackSet;

fn sandbox() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the repository root")
        .join("fixtures/knowledge-sandbox")
}

/// domain, path, format, state, type, type confidence, date, date source, title.
type StoredRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<f64>,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn dump_entry(profile: &FileProfile, path: &str) -> serde_json::Value {
    serde_json::json!({
        "path": path,
        "business_type": profile.document_type.as_ref().map(|t| t.code.clone()),
        "business_type_confidence": profile.document_type.as_ref().map(|t| t.confidence),
        "document_date": profile.dates.chosen().map(|d| d.iso.clone()),
        "document_date_source": profile.dates.chosen().map(|d| d.source),
        "title": profile.title.as_ref().map(|t| t.text.clone()),
        "roles": profile.roles.iter().map(|r| serde_json::json!({
            "role": r.role, "surface": r.surface, "source": r.source, "confidence": r.confidence,
        })).collect::<Vec<_>>(),
        "identifiers": profile.name_identifiers.iter().map(|i| i.normalized.clone()).collect::<Vec<_>>(),
    })
}

#[test]
#[ignore = "a measure over the whole sandbox, run on demand"]
fn dump_the_sandbox_profiles_for_the_scorer() {
    let root = sandbox();
    let vocabulary = ProfileVocabulary::load(&["health-fr", "health-ch"]).expect("vocabulary");
    let packs = PackSet::load("fr-FR", &["health-fr", "health-ch"]).expect("packs");

    let mut dump = Vec::new();
    for (list, folder, domain) in [
        ("documents.json", "documents", Domain::Documents),
        ("workbooks.json", "data", Domain::Data),
    ] {
        let text = std::fs::read_to_string(root.join(list)).expect("the truth");
        let rows: Vec<serde_json::Value> = serde_json::from_str(&text).expect("JSON");
        for row in rows {
            let path = row["path"].as_str().expect("a path").to_string();
            let absolute = root.join(folder).join(&path);
            let extension = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
            let file = DiscoveredFile {
                relative_path: path.clone(),
                absolute_path: absolute.display().to_string(),
                extension: extension.clone(),
                size_bytes: 0,
                modified_at: None,
            };
            let first_page = match domain {
                Domain::Documents => extract(&file, None, None, "fr-FR")
                    .ok()
                    .and_then(|doc| doc.pages.into_iter().next())
                    .map(|page| page.text)
                    .filter(|text| !text.trim().is_empty()),
                Domain::Data => None,
            };
            let metadata = read_metadata(&absolute, &extension, &vocabulary);
            let profile = build_profile(
                &ProfileInput {
                    relative_path: &path,
                    original_name: None,
                    domain,
                    first_page_text: first_page.as_deref(),
                    metadata: Some(&metadata),
                    workbook: None,
                },
                &vocabulary,
                &packs,
            );
            dump.push(dump_entry(&profile, &path));
        }
    }
    let target = std::env::var_os("FILE_PROFILE_DUMP")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("file-profile-dump.json"));
    std::fs::write(
        &target,
        serde_json::to_string_pretty(&dump).expect("serialises"),
    )
    .expect("writes the dump");
    println!(
        "profile dump of {} files written to {}",
        dump.len(),
        target.display()
    );
}

/// The same measure through the product's own road: the sandbox is copied into a laboratory work folder and
/// data folder, analysed with the knowledge base on and the health packs active, and the **stored** profiles
/// are dumped, links to the base included. Scans are read without a recognition engine, so they are typed
/// from their name only.
#[tokio::test]
#[ignore = "a measure over the whole sandbox, run on demand"]
async fn dump_the_stored_profiles_of_a_real_pass_for_the_scorer() {
    use common::knowledge::Lab;

    let root = sandbox();
    let mut lab = Lab::new();
    lab.packs = vec!["health-fr", "health-ch"];
    for (list, folder, target) in [
        ("documents.json", "documents", lab.work.path().to_path_buf()),
        ("workbooks.json", "data", lab.data.path().to_path_buf()),
    ] {
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(&std::fs::read_to_string(root.join(list)).expect("the truth"))
                .expect("JSON");
        for row in rows {
            let path = row["path"].as_str().expect("a path");
            let destination = target.join(path);
            std::fs::create_dir_all(destination.parent().expect("a folder")).expect("creates it");
            std::fs::copy(root.join(folder).join(path), destination).expect("copies the file");
        }
    }
    let started = std::time::Instant::now();
    let documents = lab.analyse().await;
    let documents_ms = started.elapsed().as_millis();
    let started = std::time::Instant::now();
    let data = lab.analyse_data();
    let data_ms = started.elapsed().as_millis();
    println!(
        "documents pass {documents_ms} ms (profile {:?}); data pass {data_ms} ms (profile {:?})",
        documents.profile, data.profile
    );

    let connection = lab.side();
    let mut statement = connection
        .prepare(
            "SELECT domain, relative_path, format, state, business_type, business_type_confidence,
                    document_date, document_date_source, title FROM file_profiles ORDER BY domain, relative_path",
        )
        .expect("prepares");
    let profiles: Vec<StoredRow> = statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        })
        .expect("queries")
        .collect::<Result<_, _>>()
        .expect("reads");
    let mut dump = Vec::new();
    for (domain, path, format, state, business_type, confidence, date, date_source, title) in
        profiles
    {
        let mut links = connection
            .prepare(
                "SELECT e.role, e.surface, e.entity_id, k.canonical_name, e.method, e.confidence, e.locator
                 FROM file_profile_entities e LEFT JOIN kb_entities k ON k.entity_id = e.entity_id
                 WHERE e.domain = ?1 AND e.relative_path = ?2",
            )
            .expect("prepares");
        let roles: Vec<serde_json::Value> = links
            .query_map([&domain, &path], |row| {
                Ok(serde_json::json!({
                    "role": row.get::<_, String>(0)?, "surface": row.get::<_, String>(1)?,
                    "entity_id": row.get::<_, Option<i64>>(2)?, "canonical": row.get::<_, Option<String>>(3)?,
                    "method": row.get::<_, String>(4)?, "confidence": row.get::<_, f64>(5)?,
                    "locator": row.get::<_, String>(6)?,
                }))
            })
            .expect("queries")
            .collect::<Result<_, _>>()
            .expect("reads");
        dump.push(serde_json::json!({
            "domain": domain, "path": path, "format": format, "state": state,
            "business_type": business_type, "business_type_confidence": confidence,
            "document_date": date, "document_date_source": date_source, "title": title, "roles": roles,
        }));
    }
    let target = std::env::var_os("FILE_PROFILE_DUMP")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("file-profile-dump.json"));
    std::fs::write(
        &target,
        serde_json::to_string_pretty(&dump).expect("serialises"),
    )
    .expect("writes the dump");
    println!(
        "stored profile dump of {} files written to {}",
        dump.len(),
        target.display()
    );
}
