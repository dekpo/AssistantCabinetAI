//! The tabular reference set: French and English questions over fictional workbooks, each with
//! the answer it should get, run against tier 2 exactly as the app runs it (`docs/DECISIONS.md`,
//! session 7, and `docs/RETRIEVAL.md`, "Tabular reference set").
//!
//! The cases are data, in `tests/tabular_reference_cases.json`; the workbooks are written at test
//! time by `tests/common/tabular_fixtures.rs`, whose comments carry the hand arithmetic behind
//! every expected value. The Data Folder is analysed by `data_folder::analyse` and asked through
//! `tabular_answer::answer`, the same path the tier 2 command takes.
//!
//! Every case here is a gap A-F reference case - a deterministic phrasing the classifier either
//! reads or does not - never gap G (session 14's hidden interpreter, `docs/SESSION-DATA-14-Query-
//! Plan.md`), whose whole point is a question the classifier does *not* read. `ModelAssist` is
//! still required by `answer`'s signature, so it is given an address nothing listens on
//! (`closed_gateway_url`): every case here must still answer exactly as it did before that
//! signature changed, proving the deterministic set makes zero *successful* gateway calls - the
//! strongest version of "no gateway exists here" this signature change still allows.
//!
//! - A `pass` case must match its `expect` exactly.
//! - A `known_failure` case must **not** match it. When it does, the run fails and says so, so a
//!   fix is never left unrecorded: its status becomes `pass`.
//! - A `current_answer`, when recorded, is today's wrong answer, and must still be what comes
//!   back. A change in it without the case turning green is worth a look: the bug changed shape.
//!
//! Matching: an expectation names only the fields it cares about, and each one it names must be
//! equal (numbers within 1e-9, relative), recursively; an array must have the same length. The
//! answer is compared in a flattened form built by `observed`.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

use assistant_cabinet_ai_lib::analysis_scope::{ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::FileHashCache;
use assistant_cabinet_ai_lib::tabular::inventory::ColumnType;
use assistant_cabinet_ai_lib::tabular_answer::{self, ModelAssist};
use common::tabular_fixtures;
use serde::Deserialize;
use serde_json::{json, Map, Value};

/// An address nothing listens on: binds a free port, then immediately drops the listener, so a
/// connection attempt refuses fast and deterministically rather than racing another test for a
/// fixed port or waiting out a timeout.
fn closed_gateway_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("binds a free port");
    let url = format!("http://{}", listener.local_addr().expect("a local address"));
    drop(listener);
    url
}

const CASES: &str = include_str!("tabular_reference_cases.json");
const LOCALES: [&str; 2] = ["fr-FR", "en-US"];
const MIN_SCENARIOS: usize = 40;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    locale: String,
    file: String,
    question: String,
    expect: Value,
    status: Status,
    #[serde(default)]
    gap: Option<String>,
    #[serde(default)]
    fixed_by: Option<u32>,
    #[serde(default)]
    current_answer: Option<Value>,
    /// A stage on the way to `expect` that an earlier session delivers - for gap A, session 9's
    /// `filter_not_supported` nudge before session 11's filtered value.
    #[serde(default)]
    interim: Option<Interim>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Interim {
    expect: Value,
    fixed_by: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Pass,
    KnownFailure,
}

fn cases() -> Vec<Case> {
    serde_json::from_str(CASES).expect("tabular_reference_cases.json parses")
}

/// `A-01` for `A-01-fr`: the scenario both languages share.
fn scenario(id: &str) -> &str {
    id.rsplit_once('-').map(|(base, _)| base).unwrap_or(id)
}

/// The set itself is sound before any question is asked: every scenario in both languages, with
/// the same expectation, status, gap and target session in each; every known failure names its
/// gap and the session expected to fix it.
#[test]
fn every_reference_case_exists_in_both_languages_and_is_well_formed() {
    let cases = cases();
    let mut ids = BTreeSet::new();
    let mut by_scenario: BTreeMap<&str, Vec<&Case>> = BTreeMap::new();
    for case in &cases {
        assert!(
            ids.insert(case.id.as_str()),
            "duplicate case id {}",
            case.id
        );
        assert!(
            LOCALES.contains(&case.locale.as_str()),
            "{}: unknown locale {}",
            case.id,
            case.locale
        );
        let suffix = &case.locale[..2];
        assert!(
            case.id.ends_with(&format!("-{suffix}")),
            "{}: the id must end with -{suffix}",
            case.id
        );
        assert!(
            tabular_fixtures::ALL.contains(&case.file.as_str()),
            "{}: no fixture named {}",
            case.id,
            case.file
        );
        assert!(
            case.expect.get("kind").is_some(),
            "{}: expect needs a kind",
            case.id
        );
        match case.status {
            Status::KnownFailure => {
                assert!(
                    case.gap.is_some(),
                    "{}: a known failure names its gap",
                    case.id
                );
                assert!(
                    case.fixed_by.is_some(),
                    "{}: a known failure names the session expected to fix it",
                    case.id
                );
            }
            Status::Pass => {
                assert!(
                    case.current_answer.is_none() && case.interim.is_none(),
                    "{}: a passing case has no current_answer or interim",
                    case.id
                );
            }
        }
        by_scenario
            .entry(scenario(&case.id))
            .or_default()
            .push(case);
    }

    assert!(
        by_scenario.len() >= MIN_SCENARIOS,
        "at least {MIN_SCENARIOS} scenarios, found {}",
        by_scenario.len()
    );
    for (name, group) in &by_scenario {
        let locales: BTreeSet<&str> = group.iter().map(|case| case.locale.as_str()).collect();
        assert_eq!(
            locales,
            LOCALES.iter().copied().collect(),
            "scenario {name} must exist in French and in English, once each"
        );
        assert_eq!(group.len(), LOCALES.len(), "scenario {name} is duplicated");
        let first = group[0];
        for other in &group[1..] {
            assert_eq!(
                first.file, other.file,
                "scenario {name}: same file in every language"
            );
            assert_eq!(
                first.expect, other.expect,
                "scenario {name}: same expectation"
            );
            assert_eq!(first.status, other.status, "scenario {name}: same status");
            assert_eq!(first.gap, other.gap, "scenario {name}: same gap");
            assert_eq!(
                first.fixed_by, other.fixed_by,
                "scenario {name}: same target session"
            );
            assert_eq!(
                first.interim, other.interim,
                "scenario {name}: same interim target"
            );
        }
    }
}

/// The Data Folder over the six fixtures, analysed the way the Analyse button does it.
struct Folder {
    data: tempfile::TempDir,
    _app: tempfile::TempDir,
    index: IndexStore,
    gateway: GatewayClient,
}

impl Folder {
    fn build() -> Self {
        let data = tempfile::tempdir().unwrap();
        let app = tempfile::tempdir().unwrap();
        tabular_fixtures::write_all(data.path());
        let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
        // The fixtures' own amounts and dates are unambiguous by shape regardless of locale
        // (`docs/DECISIONS.md`, D2/D3); each case's own question still reads in `case.locale`
        // through `tabular_answer::answer`, which re-reads the workbook fresh.
        data_folder::analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();
        Self {
            data,
            _app: app,
            index,
            gateway: GatewayClient::new().expect("builds a client"),
        }
    }

    fn open(&self) -> DataFolder {
        DataFolder::discover(self.data.path(), Some(&self.index), &FileHashCache::new()).unwrap()
    }

    /// A `ModelAssist` pointing nowhere: every case in this reference set is deterministic, so the
    /// one gateway call session 14 added must never be reached for a reason that matters - it may
    /// only be *attempted* and refused, on the same path an unreachable gateway already degrades
    /// to the ordinary nudge.
    fn assist<'a>(&'a self, url: &'a str) -> ModelAssist<'a> {
        ModelAssist {
            gateway: &self.gateway,
            server_url: url,
            model_alias: "cabinet-chat",
            idle_timeout: std::time::Duration::from_millis(200),
        }
    }
}

/// Only the case's own workbook is ticked, pinned to what it holds now: the question is about
/// that file, never a choice between six.
fn selection_of(folder: &DataFolder, file: &str) -> ScopeMode {
    let record = folder
        .files()
        .find_by_relative_path(file)
        .unwrap_or_else(|| panic!("{file} is in the Data Folder"));
    ScopeMode::Explicit(vec![ScopeEntry {
        relative_path: file.to_string(),
        pinned_id: record.id.clone(),
        added_at: 1,
        sheet_names: Vec::new(),
    }])
}

/// The answer in the flattened form expectations are written against.
///
/// - `value`: `{kind, operation, value, sheet, column, rowCount}`, `operation` being the
///   derivation's; a row is `{rowIndex, cells: {column: text}}`, and a row list
///   `{count, first, last}`.
/// - `structural`: `{kind, question, ...the structural answer's own fields}`.
/// - anything else: the answer's own fields, `file` left out.
/// - an error: `{kind: "error", error: "<Debug>"}`.
fn observed(answer: &Result<tabular_answer::TabularAnswer, impl std::fmt::Debug>) -> Value {
    let answer = match answer {
        Ok(answer) => answer,
        Err(error) => return json!({ "kind": "error", "error": format!("{error:?}") }),
    };
    let mut wire = serde_json::to_value(answer).expect("a tabular answer serialises");
    let object = wire.as_object_mut().expect("an answer is an object");
    object.remove("file");
    match object["kind"].as_str() {
        Some("value") => {
            let value = &object["value"];
            json!({
                "kind": "value",
                "operation": object["derivation"]["operation"],
                "value": flatten_value(value["kind"].as_str().unwrap_or_default(), &value["value"]),
                "sheet": object["locator"]["sheet"],
                "column": object["locator"]["column"],
                "rowCount": object["derivation"]["row_count"],
            })
        }
        Some("structural") => {
            let mut flat = Map::new();
            flat.insert("kind".into(), json!("structural"));
            let inner = object["answer"].as_object().cloned().unwrap_or_default();
            for (key, value) in inner {
                let key = if key == "kind" {
                    "question".to_string()
                } else {
                    key
                };
                flat.insert(key, value);
            }
            Value::Object(flat)
        }
        _ => wire,
    }
}

fn flatten_value(kind: &str, value: &Value) -> Value {
    match kind {
        "largest_row" | "row" => flatten_row(value),
        "rows" => {
            let rows = value.as_array().cloned().unwrap_or_default();
            json!({
                "count": rows.len(),
                "first": rows.first().map(flatten_row),
                "last": rows.last().map(flatten_row),
            })
        }
        _ => value.clone(),
    }
}

fn flatten_row(row: &Value) -> Value {
    let cells: Map<String, Value> = row["cells"]
        .as_array()
        .map(|cells| {
            cells
                .iter()
                .map(|cell| {
                    (
                        cell["column"].as_str().unwrap_or_default().to_string(),
                        cell["text"].clone(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    json!({ "rowIndex": row["rowIndex"], "cells": cells })
}

/// Whether `actual` holds everything `expected` names, equal.
fn matches(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Object(want), Value::Object(have)) => want
            .iter()
            .all(|(key, value)| matches(value, have.get(key).unwrap_or(&Value::Null))),
        (Value::Array(want), Value::Array(have)) => {
            want.len() == have.len() && want.iter().zip(have).all(|(w, h)| matches(w, h))
        }
        (Value::Number(want), Value::Number(have)) => {
            let (want, have) = (want.as_f64().unwrap(), have.as_f64().unwrap());
            (want - have).abs() <= 1e-9 * want.abs().max(have.abs()).max(1.0)
        }
        _ => expected == actual,
    }
}

#[derive(Default)]
struct Tally {
    pass: usize,
    known_failures: usize,
}

#[tokio::test]
async fn every_reference_case_gets_its_expected_answer_or_is_a_recorded_known_failure() {
    let fixtures = Folder::build();
    let folder = fixtures.open();
    let url = closed_gateway_url();
    let assist = fixtures.assist(&url);
    for file in tabular_fixtures::ALL {
        assert!(
            folder.usable_inventory(file).is_some(),
            "fixture {file} must be analysed green, or no case on it can be asked"
        );
    }
    // The fixtures are what their comments say: real Excel date cells and dd/mm/yyyy text both
    // reach the inventory as dates, so a gap B case is about dates and not about a number.
    for (file, sheet) in [
        (tabular_fixtures::RENDEZ_VOUS, "Planning"),
        (tabular_fixtures::DEPENSES, "depenses"),
    ] {
        let inventory = folder.usable_inventory(file).unwrap();
        let column = inventory
            .sheets
            .iter()
            .find(|each| each.name == sheet)
            .and_then(|each| each.columns.iter().find(|column| column.name == "date"))
            .unwrap_or_else(|| panic!("{file} has a date column on {sheet}"));
        assert_eq!(
            column.inferred_type,
            ColumnType::Date,
            "{file}: date column"
        );
    }

    let mut problems = Vec::new();
    let mut tally: BTreeMap<String, Tally> = BTreeMap::new();
    for case in cases() {
        let result = tabular_answer::answer(
            &case.question,
            &folder,
            &selection_of(&folder, &case.file),
            &case.locale,
            &fixtures.index,
            &assist,
        )
        .await;
        let actual = observed(&result);
        let met = matches(&case.expect, &actual);
        let note = case
            .note
            .as_deref()
            .map(|note| format!("\n  note: {note}"))
            .unwrap_or_default();
        let gap = case.gap.clone().unwrap_or_else(|| "-".to_string());
        let entry = tally.entry(gap).or_default();
        match case.status {
            Status::Pass => {
                entry.pass += 1;
                if !met {
                    problems.push(format!(
                        "case {} ({:?}) no longer passes:\n  expected {}\n  got      {}{note}",
                        case.id, case.question, case.expect, actual
                    ));
                }
            }
            Status::KnownFailure => {
                entry.known_failures += 1;
                if met {
                    problems.push(format!(
                        "case {} now passes: set its status to pass (and drop its current_answer){note}",
                        case.id
                    ));
                    continue;
                }
                if let Some(current) = &case.current_answer {
                    if !matches(current, &actual) {
                        let interim = case
                            .interim
                            .as_ref()
                            .filter(|interim| matches(&interim.expect, &actual))
                            .map(|interim| {
                                format!(
                                    " - it now matches its interim target (session {}): record it as current_answer",
                                    interim.fixed_by
                                )
                            })
                            .unwrap_or_default();
                        problems.push(format!(
                            "case {} ({:?}) changed shape without passing{interim}:\n  recorded current_answer {}\n  got                      {}{note}",
                            case.id, case.question, current, actual
                        ));
                    }
                }
            }
        }
    }

    // Written to stderr directly rather than through `println!`, so the summary shows in an
    // ordinary `cargo test` run and not only with `--nocapture`.
    let mut summary = String::from("tabular reference set, by gap (passing / known failures):\n");
    for (gap, counts) in &tally {
        summary.push_str(&format!(
            "  gap {gap}: {} passing / {} known failures\n",
            counts.pass, counts.known_failures
        ));
    }
    let _ = std::io::stderr().write_all(summary.as_bytes());

    assert!(
        problems.is_empty(),
        "{} reference case(s) disagree with the set:\n\n{}",
        problems.len(),
        problems.join("\n\n")
    );
}

/// The typed workbook cache (`docs/SESSION-DATA-13-Column-Cache.md`) must change nothing the set
/// expects, whether a case is the first question asked about its file in this run (a cold cache,
/// `tabular_answer::answer` falling back to a full read) or a later one (warm, served from
/// `IndexStore::tabular_workbook_by_hash` with no read at all). Every passing case is asked
/// twice, on the same shared `Folder` the main test leaves warmed by its own earlier cases, so
/// this run alone already exercises every case cold once and warm at least once.
#[tokio::test]
async fn every_passing_case_still_matches_cold_and_warm() {
    let fixtures = Folder::build();
    let folder = fixtures.open();
    let url = closed_gateway_url();
    let assist = fixtures.assist(&url);
    let passing: Vec<Case> = cases()
        .into_iter()
        .filter(|case| case.status == Status::Pass)
        .collect();
    assert!(!passing.is_empty(), "the set has passing cases to check");

    for pass in ["cold or warm from an earlier case", "warm"] {
        for case in &passing {
            let result = tabular_answer::answer(
                &case.question,
                &folder,
                &selection_of(&folder, &case.file),
                &case.locale,
                &fixtures.index,
                &assist,
            )
            .await;
            let actual = observed(&result);
            assert!(
                matches(&case.expect, &actual),
                "case {} ({:?}), {pass} pass: expected {}\n  got {}",
                case.id,
                case.question,
                case.expect,
                actual
            );
        }
    }
}
