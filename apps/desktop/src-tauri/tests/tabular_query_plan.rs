//! Session 14's hidden interpreter, gap G (`docs/SESSION-DATA-REFERENCE-report.md` section 4),
//! decision D6 (`docs/DECISIONS.md`): the adversarial tests section 6 of
//! `docs/SESSION-DATA-14-Query-Plan.md` requires, each against a `tiny_http` fake gateway exactly
//! as `tests/chat_cancellation.rs` and `tests/end_to_end_retrieval.rs` already use. The fake
//! gateway stands in for a real model, but never for what it may be shown: every schema this file
//! sends is built the same way production does, from `TabularInventory` alone.
//!
//! Every case that depends on the question actually reaching the model (routing to the
//! model-assisted path, and what comes back once it is there) is asked in French and in English,
//! since the point of this session is that the classifier's own language-specific vocabulary is
//! what sent the question here in the first place - a gap this module exists to close for
//! either language, not only the one this file happens to be written in. The purely mechanical
//! cases (JSON parsing, the idle timeout, the zero-gateway-calls and replay guarantees) are not
//! language-sensitive and are asked once.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use assistant_cabinet_ai_lib::analysis_scope::{ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::FileHashCache;
use assistant_cabinet_ai_lib::tabular::engine::{NotAnswerableReason, TabularDerivation, TabularValue};
use assistant_cabinet_ai_lib::tabular_answer::{self, ModelAssist, TabularAnswer};
use common::tabular_fixtures;

const IDLE_TIMEOUT: Duration = Duration::from_millis(500);

/// A fake gateway serving `/v1/chat/completions` with one canned SSE answer, and recording every
/// request body it received (for the "no cell value leaked" and "zero calls" assertions).
struct FakeGateway {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
}

fn sse_of(content: &str) -> String {
    let escaped = serde_json::to_string(content).expect("a string always serialises");
    format!("data: {{\"choices\":[{{\"delta\":{{\"content\":{escaped}}}}}]}}\n\ndata: [DONE]\n\n")
}

/// `response` is called once per request, with the request body, and its return value is streamed
/// back as the one chat delta.
fn start_fake_gateway(response: impl Fn(&str) -> String + Send + 'static) -> FakeGateway {
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
    let url = format!("http://{}", server.server_addr());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&requests);

    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            // The content sent to the model is nested JSON inside the `messages` field - pull it
            // out so the recorded request is the text a model would actually read, not the wire
            // envelope around it.
            let payload: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
            let content = payload["messages"][0]["content"].as_str().unwrap_or(&body).to_string();
            recorded.lock().unwrap().push(content.clone());
            let reply = sse_of(&response(&content));
            let http_response = tiny_http::Response::from_string(reply).with_status_code(200);
            let _ = request.respond(http_response);
        }
    });

    FakeGateway { url, requests }
}

/// A fake gateway that never answers at all, holding the connection open - the same technique
/// `tests/chat_idle_timeout.rs` uses to prove a bound actually ends a wait.
fn start_silent_gateway() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("binds a free port");
    let url = format!("http://{}", listener.local_addr().expect("a local address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            std::thread::spawn(move || {
                let _held = stream;
                std::thread::sleep(Duration::from_secs(30));
            });
        }
    });
    url
}

/// An address nothing listens on: binds a free port, then drops the listener immediately.
fn closed_gateway_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("binds a free port");
    let url = format!("http://{}", listener.local_addr().expect("a local address"));
    drop(listener);
    url
}

/// One workbook, `factures.csv`: `fournisseur`, `montant`, nine rows, three suppliers cycling -
/// small and easy to hand-check, independent of `tests/tabular_reference.rs`'s six-fixture set.
/// Totals: Alpha (rows 0, 3, 6) = 10 + 40 + 70 = 120; Beta (1, 4, 7) = 20 + 50 + 80 = 150; Gamma
/// (2, 5, 8) = 30 + 60 + 90 = 180; every row's total = 120 + 150 + 180 = 450.
fn factures_csv() -> String {
    let suppliers = ["Alpha", "Beta", "Gamma"];
    let mut csv = String::from("fournisseur,montant\n");
    for i in 0..9 {
        csv.push_str(&format!("{},{}\n", suppliers[i % 3], (i + 1) * 10));
    }
    csv
}

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
        std::fs::write(data.path().join("factures.csv"), factures_csv()).unwrap();
        tabular_fixtures::write_xlsx(
            &data.path().join(tabular_fixtures::MIXTE),
            &tabular_fixtures::mixte_sheets(),
        );
        let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
        data_folder::analyse(data.path(), &mut index, "en-US", &|_| {}).unwrap();
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

    /// An explicit selection of just this one file, pinned to what it holds now - this fixture
    /// keeps two workbooks (`factures.csv`, `mixte.xlsx`), so a question naming no file would
    /// otherwise be refused as ambiguous between them, which is not what any test here means to
    /// exercise.
    fn ticked(&self, path: &str) -> ScopeMode {
        let folder = self.open();
        let record = folder.files().find_by_relative_path(path).unwrap();
        ScopeMode::Explicit(vec![ScopeEntry {
            relative_path: path.to_string(),
            pinned_id: record.id.clone(),
            added_at: 1,
            sheet_names: Vec::new(),
        }])
    }

    async fn ask(
        &self,
        question: &str,
        file: &str,
        url: &str,
        model_alias: &str,
    ) -> Result<TabularAnswer, assistant_cabinet_ai_lib::error::AppError> {
        let assist = ModelAssist {
            gateway: &self.gateway,
            server_url: url,
            model_alias,
            idle_timeout: IDLE_TIMEOUT,
        };
        tabular_answer::answer(question, &self.open(), &self.ticked(file), "en-US", &self.index, &assist).await
    }

    /// "Demander a l'IA"/"Ask AI" on a tabular answer: the exact two-step `prepare`/`resolve`
    /// call `commands::tabular_tier` makes with `skip_deterministic: true`, not the convenience
    /// `answer()` wrapper above (which never forces the model).
    async fn ask_forcing_model(
        &self,
        question: &str,
        file: &str,
        url: &str,
        model_alias: &str,
    ) -> TabularAnswer {
        let pending =
            tabular_answer::prepare(question, &self.open(), &self.ticked(file), "en-US", &self.index, true)
                .unwrap();
        let assist = ModelAssist {
            gateway: &self.gateway,
            server_url: url,
            model_alias,
            idle_timeout: IDLE_TIMEOUT,
        };
        tabular_answer::resolve(pending, question, "en-US", &assist).await
    }
}

fn sum_montant_plan() -> &'static str {
    r#"{"aggregate": {"op": "sum", "column": "montant"}}"#
}

// --- A comparison word beside a spelled-out number escalates, and the model reads the number -

#[tokio::test]
async fn a_spelled_out_number_the_classifier_cannot_parse_is_read_correctly_by_the_model() {
    // Found live (session 14's manual validation pass, `docs/DECISIONS.md`): the deterministic
    // comparison-filter detection only ever looks for a parseable digit beside a comparison word,
    // so "more than fifty" used to silently answer as if unfiltered (all 9 rows, not the 4 whose
    // montant is actually over 50) - a wrong answer with nothing to say it might be wrong. This
    // proves the fix end to end: the word "fifty" that defeated the classifier now escalates
    // (`tabular_answer::detect_filters`'s `unresolved_comparison`), and the model, given the real
    // question text, reads "fifty" as 50 - something the word-for-word scan never could - and the
    // engine computes the honest, correct count from the plan it wrote.
    let folder = Folder::build();
    let gateway = start_fake_gateway(|_| {
        r#"{"aggregate": {"op": "count"}, "filters": [{"column": "montant", "op": "gt", "value": 50}]}"#.to_string()
    });

    let result = folder
        .ask("How many sales were worth more than fifty?", "factures.csv", &gateway.url, "cabinet-chat")
        .await
        .unwrap();

    let TabularAnswer::Value { value, derivation, .. } = result else {
        panic!("expected a computed value, got something else");
    };
    assert_eq!(value, TabularValue::Count(4), "montant 60, 70, 80, 90 - strictly over 50");
    assert!(matches!(derivation, TabularDerivation::InterpretedByModel { .. }));
}

// --- A valid plan: the engine's value, with the interpreted provenance ----------------------

#[tokio::test]
async fn a_valid_plan_is_computed_by_the_engine_with_interpreted_provenance() {
    let folder = Folder::build();
    // Both languages: the model-assisted path must be reachable from a French question exactly as
    // from an English one - the classifier's own vocabulary limits are what sent this question
    // here in the first place, not a language this module reads itself.
    for question in [
        "What's the grand total across every invoice we have on file?",
        "Quel est le total general de toutes les factures que nous avons ?",
    ] {
        let gateway = start_fake_gateway(|_| sum_montant_plan().to_string());
        let result = folder
            .ask(question, "factures.csv", &gateway.url, "cabinet-chat")
            .await
            .unwrap();

        let TabularAnswer::Value { value, derivation, .. } = result else {
            panic!("expected a computed value for {question:?}, got something else");
        };
        assert_eq!(
            value,
            TabularValue::Sum(assistant_cabinet_ai_lib::tabular::engine::NumericAggregate {
                value: 450.0,
                unit: None,
                unparsed: 0,
            })
        );
        let TabularDerivation::InterpretedByModel { operation, row_count, model_alias, .. } = derivation else {
            panic!("expected InterpretedByModel, got {derivation:?}");
        };
        assert_eq!(operation, "sum");
        assert_eq!(row_count, 9);
        assert_eq!(model_alias, "cabinet-chat");
    }
}

// --- A plan naming a column that does not exist -> nudge, no value --------------------------

#[tokio::test]
async fn a_plan_naming_a_missing_column_is_a_nudge_not_a_value() {
    let folder = Folder::build();
    for question in [
        "What is the grand total of everything?",
        "Quel est le total general de tout ?",
    ] {
        let gateway = start_fake_gateway(|_| {
            r#"{"aggregate": {"op": "sum", "column": "does_not_exist"}}"#.to_string()
        });

        let result = folder
            .ask(question, "factures.csv", &gateway.url, "cabinet-chat")
            .await
            .unwrap();

        let TabularAnswer::Nudge { reason, .. } = result else {
            panic!("{question:?}: expected a nudge, got {result:?}");
        };
        assert_eq!(reason, Some(NotAnswerableReason::ColumnNotFound), "{question:?}");
    }
}

// --- A plan filtering on an absent value -> value_not_found with close values ----------------

#[tokio::test]
async fn a_plan_filtering_on_an_absent_value_is_value_not_found_with_close_values() {
    let folder = Folder::build();
    // Neither phrasing names a real column ("fournisseur"/"montant") - naming one would let the
    // classifier itself resolve a `Sum` operation and never reach the model-assisted path at all.
    for question in [
        "What's the total for the supplier whose name I can't quite spell?",
        "Quel est le total pour ce partenaire commercial dont je ne suis pas sur de l'orthographe ?",
    ] {
        let gateway = start_fake_gateway(|_| {
            r#"{"aggregate": {"op": "sum", "column": "montant"},
                "filters": [{"column": "fournisseur", "op": "eq", "value": "Alfa"}]}"#
                .to_string()
        });

        let result = folder
            .ask(question, "factures.csv", &gateway.url, "cabinet-chat")
            .await
            .unwrap();

        let TabularAnswer::Nudge { reason, filter_value, close_values, .. } = result else {
            panic!("{question:?}: expected a value_not_found nudge, got {result:?}");
        };
        assert_eq!(reason, Some(NotAnswerableReason::ValueNotFound), "{question:?}");
        assert_eq!(filter_value.as_deref(), Some("Alfa"), "{question:?}");
        assert!(close_values.contains(&"Alpha".to_string()), "{question:?}");
    }
}

// --- A plan asking to sum a formula column -> formula_cannot_be_verified ---------------------

#[tokio::test]
async fn a_plan_summing_a_formula_column_is_refused_as_unverifiable() {
    let folder = Folder::build();
    // Deliberately avoids any structural word ("sheet"/"feuille", "rows"/"lignes",
    // "columns"/"colonnes"): the point is to reach the model-assisted path, not the deterministic
    // structural one.
    for question in [
        "What's the cumulative figure over there?",
        "Quel est le chiffre cumule la-bas ?",
    ] {
        let gateway = start_fake_gateway(|_| {
            r#"{"sheet": "Calculs", "aggregate": {"op": "sum", "column": "cumul"}}"#.to_string()
        });

        let result = folder
            .ask(question, "mixte.xlsx", &gateway.url, "cabinet-chat")
            .await
            .unwrap();

        let TabularAnswer::Nudge { reason, .. } = result else {
            panic!("{question:?}: expected a nudge, got {result:?}");
        };
        assert_eq!(reason, Some(NotAnswerableReason::FormulaCannotBeVerified), "{question:?}");
    }
}

// --- Invalid JSON, extra fields, unsupported: true -> nudge ---------------------------------

#[tokio::test]
async fn invalid_json_extra_fields_and_unsupported_true_all_degrade_to_a_nudge() {
    let folder = Folder::build();
    for question in [
        "Tell me something interesting about this data.",
        "Dites-moi quelque chose d'interessant sur ces donnees.",
    ] {
        for (label, body) in [
            ("prose with no JSON at all", "I'm not sure how to answer that one, sorry."),
            (
                "an invented field",
                r#"{"aggregate": {"op": "sum", "column": "montant"}, "confidence": 0.9}"#,
            ),
            ("unsupported: true", r#"{"unsupported": true}"#),
        ] {
            let gateway = start_fake_gateway(move |_| body.to_string());
            let result = folder
                .ask(question, "factures.csv", &gateway.url, "cabinet-chat")
                .await
                .unwrap();
            assert!(
                matches!(result, TabularAnswer::Nudge { .. }),
                "{question:?}, {label}: expected a nudge, got {result:?}"
            );
        }
    }
}

// --- The fake gateway stopped -> nudge, within the timeout -----------------------------------

#[tokio::test]
async fn a_gateway_that_never_answers_still_resolves_to_a_nudge_within_the_timeout() {
    let folder = Folder::build();
    for question in [
        "Tell me something interesting about this data.",
        "Dites-moi quelque chose d'interessant sur ces donnees.",
    ] {
        let url = start_silent_gateway();

        let started = std::time::Instant::now();
        let result = folder
            .ask(question, "factures.csv", &url, "cabinet-chat")
            .await
            .unwrap();

        assert!(matches!(result, TabularAnswer::Nudge { .. }), "{question:?}");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{question:?}: the idle bound, not the stand-in's own silence, must end the wait"
        );
    }
}

// --- A deterministically answerable question makes zero gateway requests --------------------

#[tokio::test]
async fn a_deterministic_question_never_reaches_the_gateway() {
    let folder = Folder::build();
    let count = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&count);
    let gateway = start_fake_gateway(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        sum_montant_plan().to_string()
    });

    let result = folder
        .ask("What is the total montant?", "factures.csv", &gateway.url, "cabinet-chat")
        .await
        .unwrap();

    assert!(matches!(result, TabularAnswer::Value { .. }));
    assert_eq!(
        count.load(Ordering::SeqCst),
        0,
        "the classifier reads this question on its own; the gateway must never be dialled"
    );
}

// --- The captured request carries no fixture cell value --------------------------------------

#[tokio::test]
async fn the_captured_request_never_carries_a_fixture_cell_value() {
    let folder = Folder::build();
    let gateway = start_fake_gateway(|_| sum_montant_plan().to_string());

    // A question that does not itself echo a cell value back, so anything found in the captured
    // request can only have leaked from the schema this module built.
    folder
        .ask("What's the grand total across every row?", "factures.csv", &gateway.url, "cabinet-chat")
        .await
        .unwrap();

    let requests = gateway.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    for value in ["Alpha", "Beta", "Gamma", "10", "20", "30", "40", "50", "60", "70", "80", "90"] {
        assert!(
            !requests[0].contains(value),
            "{value:?} leaked into the request sent to the model:\n{}",
            requests[0]
        );
    }
    assert!(requests[0].contains("fournisseur") && requests[0].contains("montant"));
}

// --- The same plan replayed twice gives the same number; a different alias too --------------

#[tokio::test]
async fn the_same_plan_replayed_gives_the_same_number_whatever_the_model_alias() {
    let folder = Folder::build();

    let gateway_a = start_fake_gateway(|_| sum_montant_plan().to_string());
    let first = folder
        .ask("What's the grand total across every row?", "factures.csv", &gateway_a.url, "cabinet-chat")
        .await
        .unwrap();
    let second = folder
        .ask("What's the grand total across every row?", "factures.csv", &gateway_a.url, "cabinet-chat")
        .await
        .unwrap();

    let gateway_b = start_fake_gateway(|_| sum_montant_plan().to_string());
    let third = folder
        .ask("What's the grand total across every row?", "factures.csv", &gateway_b.url, "a-different-model")
        .await
        .unwrap();

    let value_of = |answer: &TabularAnswer| match answer {
        TabularAnswer::Value { value, .. } => value.clone(),
        other => panic!("expected a value, got {other:?}"),
    };
    assert_eq!(value_of(&first), value_of(&second));
    assert_eq!(value_of(&first), value_of(&third));

    let TabularAnswer::Value {
        derivation: TabularDerivation::InterpretedByModel { model_alias: alias_a, .. },
        ..
    } = &first
    else {
        panic!("expected InterpretedByModel");
    };
    let TabularAnswer::Value {
        derivation: TabularDerivation::InterpretedByModel { model_alias: alias_b, .. },
        ..
    } = &third
    else {
        panic!("expected InterpretedByModel");
    };
    assert_eq!(alias_a, "cabinet-chat");
    assert_eq!(alias_b, "a-different-model");
}

// --- An unreachable gateway (no server at all) still degrades to the ordinary nudge ----------

#[tokio::test]
async fn an_unreachable_gateway_degrades_to_the_ordinary_nudge() {
    let folder = Folder::build();
    let url = closed_gateway_url();

    let result = folder
        .ask("Tell me something interesting about this data.", "factures.csv", &url, "cabinet-chat")
        .await
        .unwrap();

    assert!(matches!(result, TabularAnswer::Nudge { reason: None, .. }));
}

// --- "Demander a l'IA" / "Ask AI" on a classified tabular answer -----------------------------

#[tokio::test]
async fn forcing_the_model_on_a_classified_question_asks_it_and_labels_the_result_interpreted() {
    // "What is the total montant?" classifies deterministically on its own (no model, no
    // spinner) - this proves that pressing "Demander a l'IA" on it anyway reaches the model, and
    // that a successful model answer is labelled `InterpretedByModel`, not `Computed`, exactly as
    // an automatically escalated gap-G answer already is.
    let folder = Folder::build();
    let gateway = start_fake_gateway(|_| sum_montant_plan().to_string());

    let result = folder
        .ask_forcing_model("What is the total montant?", "factures.csv", &gateway.url, "cabinet-chat")
        .await;

    let TabularAnswer::Value { value, derivation, .. } = result else {
        panic!("expected a computed value, got something else");
    };
    assert_eq!(
        value,
        TabularValue::Sum(assistant_cabinet_ai_lib::tabular::engine::NumericAggregate {
            value: 450.0,
            unit: None,
            unparsed: 0,
        })
    );
    assert!(matches!(derivation, TabularDerivation::InterpretedByModel { .. }));
}

#[tokio::test]
async fn forcing_the_model_on_a_classified_question_falls_back_to_the_classifiers_own_answer_if_the_model_fails(
) {
    // The same deterministically classified question, "Demander a l'IA" pressed, but the model
    // cannot be reached at all this time: the classifier's own correct answer (450) must still be
    // what she sees - not a bare nudge, which would be a strictly worse outcome than not having
    // pressed the button at all.
    let folder = Folder::build();
    let url = closed_gateway_url();

    let result = folder
        .ask_forcing_model("What is the total montant?", "factures.csv", &url, "cabinet-chat")
        .await;

    let TabularAnswer::Value { value, derivation, .. } = result else {
        panic!("expected the classifier's own value to survive as the fallback, got something else");
    };
    assert_eq!(
        value,
        TabularValue::Sum(assistant_cabinet_ai_lib::tabular::engine::NumericAggregate {
            value: 450.0,
            unit: None,
            unparsed: 0,
        })
    );
    assert!(
        matches!(derivation, TabularDerivation::Computed { .. }),
        "the fallback is the classifier's own Computed answer, not a model-interpreted one"
    );
}

#[tokio::test]
async fn forcing_the_model_never_touches_a_question_the_classifier_could_not_read_at_all() {
    // Gap G already escalates "Tell me something interesting about this data." on its own - this
    // proves `force_model` changes nothing there: it is a no-op for a question the deterministic
    // path never classified, not a second, redundant attempt.
    let folder = Folder::build();
    let count = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&count);
    let gateway = start_fake_gateway(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        r#"{"unsupported": true}"#.to_string()
    });

    let _ = folder
        .ask_forcing_model("Tell me something interesting about this data.", "factures.csv", &gateway.url, "cabinet-chat")
        .await;

    assert_eq!(count.load(Ordering::SeqCst), 1, "exactly one attempt - gap G's own, not a second one for the forced flag");
}

// --- A nudge after a real model attempt names the model and the time spent ------------------

#[tokio::test]
async fn a_nudge_after_a_real_model_attempt_carries_its_model_and_duration() {
    // Session 14's manual validation pass (`docs/DECISIONS.md`, 1 October 2026) found a real
    // local model spend a minute or more before a question it could not interpret quietly became
    // "without the AI", with nothing on screen to say a model had been asked at all. This proves
    // the fix end to end: a nudge that followed a genuine, slow model attempt names the model and
    // roughly how long it took, not just "no".
    let folder = Folder::build();
    let gateway = start_fake_gateway(|_| {
        // A short, deliberate delay so the measured duration is checked against something real
        // rather than a near-zero elapsed time that would pass even if nothing were timed at all.
        std::thread::sleep(Duration::from_millis(120));
        "I'm not sure how to answer that one, sorry.".to_string()
    });

    let result = folder
        .ask("Tell me something interesting about this data.", "factures.csv", &gateway.url, "cabinet-chat")
        .await
        .unwrap();

    let TabularAnswer::Nudge { model_attempt, .. } = result else {
        panic!("expected a nudge, got something else");
    };
    let attempt = model_attempt.expect("a real model attempt must be recorded on the nudge");
    assert_eq!(attempt.model_alias, "cabinet-chat");
    assert!(
        attempt.duration_ms >= 100,
        "expected at least the stand-in's own 120ms delay to be measured, got {}",
        attempt.duration_ms
    );
}

// --- A nudge the classifier gave without ever trying the model carries no model attempt -----

#[tokio::test]
async fn a_purely_deterministic_nudge_carries_no_model_attempt() {
    // "total fournisseur" is classified deterministically (an ordinary `Sum` over a named,
    // existing column) and refused by the engine itself (`fournisseur` is text, not a number) -
    // gap G's escalation never triggers for an already-classified operation's own refusal
    // (`docs/DECISIONS.md`, D6: "never for a question the classifier already answered"), so this
    // nudge must read exactly as it always has, with nothing to say about a model that was never
    // asked.
    let folder = Folder::build();
    let count = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&count);
    let gateway = start_fake_gateway(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
        sum_montant_plan().to_string()
    });

    let result = folder
        .ask("What is the total fournisseur?", "factures.csv", &gateway.url, "cabinet-chat")
        .await
        .unwrap();

    assert_eq!(count.load(Ordering::SeqCst), 0, "a classified operation's own refusal must never dial the gateway");
    let TabularAnswer::Nudge {
        reason,
        model_attempt,
        ..
    } = result
    else {
        panic!("expected a nudge, got something else");
    };
    assert_eq!(reason, Some(NotAnswerableReason::NonNumericColumn));
    assert!(model_attempt.is_none());
}
