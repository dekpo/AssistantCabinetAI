//! TABULAR step 06 (owner decision 11, 20-21 October 2026): what the user can ask under a computed
//! figure. the "Explain the calculation" button explains the method from a calculation record; "Demander a l'IA"
//! analyses the figure. Both start from the figure the program computes again - never from the model's
//! reading of the question and never from the figure the interface sends back.
//!
//! Exercised one level below the Tauri command, as `tests/mixed_answer.rs` is: `recompute`, `explain`
//! and `mixed_answer::answer` are the Tauri-free functions `commands::calculation_follow_up_tier` calls.
//! The chat fake is `common::counting_gateway` (it counts every request, `/v1/embeddings` included, and
//! records the last turn of each). A test that opens a socket runs alone: `cargo test --test
//! calculation_follow_up -- --test-threads=1`.

mod common;

use std::time::Duration;

use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
use assistant_cabinet_ai_lib::calculation_follow_up::{
    self, ExplainContext, ShownCalculation, EXPLAIN_INSTRUCTION,
};
use assistant_cabinet_ai_lib::conversation::ContextBudget;
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::error::AppError;
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::FileHashCache;
use assistant_cabinet_ai_lib::mixed_answer::{
    self, CalculationSide, MixedContext, MixedPartUnavailable, ANALYSIS_ADDENDUM,
};
use assistant_cabinet_ai_lib::retrieval::Evidence;
use assistant_cabinet_ai_lib::tabular::engine::{
    NumericAggregate, TabularDerivation, TabularValue,
};
use assistant_cabinet_ai_lib::tabular_answer::{self, ModelAssist, PendingAnswer, TabularAnswer};
use common::counting_gateway::start_gateway_replying;

const IDLE_TIMEOUT: Duration = Duration::from_millis(500);
const QUESTION: &str = "What is the total montant?";
const UNRECOGNISED: &str = "What's the grand total across every invoice we have on file?";

/// Eight rows (a sheet needs eight to look like a table), summing to 1840. The supplier names are the
/// distinctive strings the tests look for in what a model is sent: none of them may appear there.
fn factures_csv() -> &'static str {
    "fournisseur,montant\n\
     Alpha-Zorglub,100\n\
     Beta-Zorglub,650\n\
     Gamma-Zorglub,300\n\
     Delta-Zorglub,120\n\
     Epsilon-Zorglub,130\n\
     Zeta-Zorglub,140\n\
     Eta-Zorglub,150\n\
     Theta-Zorglub,250\n"
}

struct Folder {
    data: tempfile::TempDir,
    _app: tempfile::TempDir,
    index: IndexStore,
}

impl Folder {
    fn build() -> Self {
        let data = tempfile::tempdir().unwrap();
        let app = tempfile::tempdir().unwrap();
        std::fs::write(data.path().join("factures.csv"), factures_csv()).unwrap();
        let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
        data_folder::analyse(data.path(), &mut index, "en-US", &|_| {}).unwrap();
        Self {
            data,
            _app: app,
            index,
        }
    }

    fn open(&self) -> DataFolder {
        DataFolder::discover(self.data.path(), Some(&self.index), &FileHashCache::new()).unwrap()
    }

    /// The figure the engine gives for `question` on its own.
    fn figure(&self, question: &str) -> TabularAnswer {
        let pending = tabular_answer::prepare(
            question,
            &self.open(),
            &ScopeMode::WholeFolder,
            "en-US",
            &self.index,
            false,
        )
        .unwrap();
        tabular_answer::without_the_model(pending)
    }

    fn recompute(
        &self,
        question: &str,
        shown: &ShownCalculation,
    ) -> Result<TabularAnswer, AppError> {
        calculation_follow_up::recompute(
            question,
            &self.open(),
            &ScopeMode::WholeFolder,
            "en-US",
            &self.index,
            shown,
            None,
        )
    }

    fn record(&self, question: &str, answer: &TabularAnswer) -> String {
        let sizes = tabular_answer::sheet_sizes(&self.open(), &ScopeMode::WholeFolder);
        tabular_answer::calculation_record(Some(question), answer, &sizes).expect("a record")
    }
}

fn shown_of(answer: &TabularAnswer) -> ShownCalculation {
    let TabularAnswer::Value {
        file,
        value,
        derivation,
        ..
    } = answer
    else {
        panic!("expected a computed value, got {answer:?}");
    };
    ShownCalculation {
        file: file.clone(),
        value: value.clone(),
        derivation: derivation.clone(),
    }
}

fn the_total() -> TabularValue {
    TabularValue::Sum(NumericAggregate {
        value: 1840.0,
        unit: None,
        unparsed: 0,
    })
}

/// A server that accepts every connection and hangs up: "the AI machine is stopped", without the
/// closed port `docs/TROUBLESHOOTING.md` warns about.
fn hanging_up_gateway_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("binds a free port");
    let url = format!("http://{}", listener.local_addr().expect("a local address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            drop(stream);
        }
    });
    url
}

fn evidence(text: &str) -> Evidence {
    Evidence {
        chunk_id: "chunk-1".to_string(),
        relative_path: "contrat.pdf".to_string(),
        page_number: 1,
        ordinal: 1,
        section: None,
        text: text.to_string(),
        score: 1.0,
        origin: PageOrigin::TextLayer,
        confidence: None,
    }
}

// --- A nudge keeps the old meaning of "Demander a l'IA" ----------------------------------------

#[test]
fn ask_ai_on_a_nudge_still_reaches_the_interpreter_and_on_a_figure_it_does_not() {
    let folder = Folder::build();
    let ask = |question: &str| {
        tabular_answer::prepare(
            question,
            &folder.open(),
            &ScopeMode::WholeFolder,
            "en-US",
            &folder.index,
            true,
        )
        .unwrap()
    };

    // The engine refuses to add up a text column: a nudge, and the model may still try to understand.
    assert!(matches!(
        ask("What is the sum of fournisseur?"),
        PendingAnswer::TryModel { .. }
    ));
    // In a mixed selection the same refusal is a data-only answer that is a nudge: the signal
    // `commands::data_side_is_a_nudge` uses to send "Demander a l'IA" to the interpreter instead of the
    // mixed tier, which hands a refusal back with no model call (owner's test of step 06, part F).
    assert!(matches!(
        tabular_answer::prepare_if_data_only(
            "What is the sum of fournisseur?",
            &folder.open(),
            &ScopeMode::WholeFolder,
            "en-US",
            &folder.index,
        )
        .unwrap(),
        Some(PendingAnswer::Done(TabularAnswer::Nudge { .. }))
    ));
    // A figure is final: the model is not asked to read the question again (owner decision 11).
    assert!(matches!(ask(QUESTION), PendingAnswer::Done(TabularAnswer::Value { .. })));
}

// --- The calculation record ---------------------------------------------------------------

#[test]
fn the_record_of_a_real_figure_states_the_method_and_carries_no_cell() {
    let folder = Folder::build();
    let answer = folder.figure(QUESTION);

    let record = folder.record(QUESTION, &answer);

    assert!(record.starts_with("Calculation record:\nQuestion: What is the total montant?\n"));
    assert!(record.contains("sheet \""));
    assert!(record.contains("Operation: sum of column \"montant\""));
    assert!(record.contains("Rows: 8 kept out of 8 data rows"));
    assert!(record.ends_with("Result: 1840"));
    // No cell and no row: the suppliers are in the file, not in the record.
    assert!(!record.contains("Zorglub"), "{record}");
    for row_value in ["650", "300", "120"] {
        assert!(!record.contains(row_value), "{row_value} found in {record}");
    }
}

#[test]
fn the_record_of_a_filtered_figure_states_what_was_understood() {
    let folder = Folder::build();
    let question = "What is the total montant of Beta-Zorglub?";
    let answer = folder.figure(question);
    assert!(matches!(answer, TabularAnswer::Value { .. }), "{answer:?}");

    let record = folder.record(question, &answer);

    assert!(
        record.contains("Understood as: fournisseur = \"Beta-Zorglub\""),
        "{record}"
    );
    assert!(
        record.contains("Rows: 1 kept out of 8 data rows"),
        "{record}"
    );
    assert!(record.ends_with("Result: 650"));
}

// --- The figure is computed again by the program, never trusted ------------------------------

#[test]
fn the_same_figure_comes_back_without_any_model() {
    let folder = Folder::build();
    let answer = folder.figure(QUESTION);

    let again = folder
        .recompute(QUESTION, &shown_of(&answer))
        .expect("the same figure");

    assert_eq!(again, answer);
}

#[test]
fn a_figure_that_is_not_the_one_on_screen_is_refused() {
    let folder = Folder::build();
    let mut shown = shown_of(&folder.figure(QUESTION));
    shown.value = TabularValue::Sum(NumericAggregate {
        value: 1841.0,
        unit: None,
        unparsed: 0,
    });

    let refused = folder.recompute(QUESTION, &shown);

    assert!(
        matches!(refused, Err(AppError::ScopeUnavailable)),
        "{refused:?}"
    );
}

#[test]
fn a_file_that_changed_since_the_answer_is_refused() {
    let folder = Folder::build();
    let shown = shown_of(&folder.figure(QUESTION));
    std::fs::write(
        folder.data.path().join("factures.csv"),
        factures_csv().replace("650", "6500"),
    )
    .unwrap();

    let refused = folder.recompute(QUESTION, &shown);

    assert!(
        matches!(refused, Err(AppError::ScopeUnavailable)),
        "{refused:?}"
    );
}

#[test]
fn a_file_that_is_gone_is_refused() {
    let folder = Folder::build();
    let shown = shown_of(&folder.figure(QUESTION));
    std::fs::remove_file(folder.data.path().join("factures.csv")).unwrap();

    let refused = folder.recompute(QUESTION, &shown);

    assert!(refused.is_err(), "{refused:?}");
}

// --- A figure the hidden interpreter read is replayed from its stored plan ---------------------

async fn interpreted_figure(folder: &Folder) -> (TabularAnswer, usize) {
    let gateway = start_gateway_replying(r#"{"aggregate": {"op": "sum", "column": "montant"}}"#);
    let client = GatewayClient::new().expect("builds a client");
    let assist = ModelAssist {
        gateway: &client,
        server_url: &gateway.url,
        model_alias: "cabinet-chat",
        idle_timeout: IDLE_TIMEOUT,
    };
    let answer = tabular_answer::answer(
        UNRECOGNISED,
        &folder.open(),
        &ScopeMode::WholeFolder,
        "en-US",
        &folder.index,
        &assist,
    )
    .await
    .unwrap();
    (answer, gateway.calls())
}

#[tokio::test]
async fn an_interpreted_figure_is_replayed_from_its_stored_plan_and_the_model_is_not_asked_again() {
    let folder = Folder::build();
    let (answer, plan_requests) = interpreted_figure(&folder).await;
    let TabularAnswer::Value {
        value, derivation, ..
    } = &answer
    else {
        panic!("expected a value, got {answer:?}");
    };
    assert_eq!(*value, the_total());
    assert!(matches!(
        derivation,
        TabularDerivation::InterpretedByModel { .. }
    ));
    assert_eq!(plan_requests, 1, "the first reading asked the model once");

    // `recompute` takes no gateway at all: the replay cannot reach a model, whatever the plan says.
    let again = folder
        .recompute(UNRECOGNISED, &shown_of(&answer))
        .expect("the stored plan gives the same figure");

    assert_eq!(again, answer);
}

#[test]
fn a_tampered_plan_the_engine_rejects_gives_the_existing_refusal() {
    let folder = Folder::build();
    let mut shown = shown_of(&folder.figure(QUESTION));
    shown.derivation = TabularDerivation::InterpretedByModel {
        operation: "sum".to_string(),
        row_count: 8,
        model_alias: "cabinet-chat".to_string(),
        plan: serde_json::json!({"aggregate": {"op": "sum", "column": "no-such-column"}}),
    };

    let refused = folder.recompute(QUESTION, &shown);

    assert!(
        matches!(refused, Err(AppError::ScopeUnavailable)),
        "{refused:?}"
    );

    shown.derivation = TabularDerivation::InterpretedByModel {
        operation: "sum".to_string(),
        row_count: 8,
        model_alias: "cabinet-chat".to_string(),
        plan: serde_json::json!({"drop_table": true}),
    };
    let refused = folder.recompute(QUESTION, &shown);
    assert!(
        matches!(refused, Err(AppError::ScopeUnavailable)),
        "{refused:?}"
    );
}

// --- Explain --------------------------------------------------------------------------------

fn explain_context<'a>(gateway: &'a GatewayClient, url: &'a str) -> ExplainContext<'a> {
    ExplainContext {
        locale: "en-US",
        gateway,
        server_url: url,
        model_alias: "cabinet-chat",
        idle_timeout: IDLE_TIMEOUT,
    }
}

#[tokio::test]
async fn explain_makes_one_call_that_carries_the_record_and_no_cell() {
    let folder = Folder::build();
    let answer = folder.figure(QUESTION);
    let record = folder.record(QUESTION, &answer);
    let model = start_gateway_replying(
        "The program added up the column montant over the 8 rows of the sheet. The total is 1840.",
    );
    let client = GatewayClient::new().expect("builds a client");
    let mut streamed = String::new();

    let explanation = calculation_follow_up::explain(
        QUESTION,
        &record,
        &explain_context(&client, &model.url),
        |delta| streamed.push_str(delta),
    )
    .await
    .expect("the explanation is written");

    // One chat call, no embedding call: every request to the fake is counted.
    assert_eq!(model.calls(), 1);
    let sent = &model.requests()[0];
    assert!(sent.starts_with(EXPLAIN_INSTRUCTION), "{sent}");
    assert!(sent.contains(&record));
    assert!(!sent.contains("Zorglub"), "no cell value reaches the model");
    assert!(!sent.contains("Document excerpts"));
    assert!(!sent.contains("Workbook cards"));
    assert!(!explanation.unavailable);
    assert_eq!(explanation.text, streamed);
    assert!(explanation.unverified_numbers.is_empty(), "{explanation:?}");
    // The figure the explanation sits under is untouched: it is computed again, not written.
    assert_eq!(folder.figure(QUESTION), answer);
}

#[tokio::test]
async fn a_number_the_explanation_adds_is_reported_beside_it() {
    let folder = Folder::build();
    let record = folder.record(QUESTION, &folder.figure(QUESTION));
    let model = start_gateway_replying(
        "The total is 1840, which is about 7777 less than last year, and 100 + 40 = 140.",
    );
    let client = GatewayClient::new().expect("builds a client");

    let explanation = calculation_follow_up::explain(
        QUESTION,
        &record,
        &explain_context(&client, &model.url),
        |_| {},
    )
    .await
    .unwrap();

    assert!(
        explanation.unverified_numbers.contains(&"7777".to_string()),
        "{explanation:?}"
    );
    assert!(!explanation.model_arithmetic.is_empty(), "{explanation:?}");
}

#[tokio::test]
async fn a_stopped_ai_machine_leaves_the_figure_and_an_unavailable_explanation() {
    let folder = Folder::build();
    let record = folder.record(QUESTION, &folder.figure(QUESTION));
    let url = hanging_up_gateway_url();
    let client = GatewayClient::new().expect("builds a client");

    let explanation =
        calculation_follow_up::explain(QUESTION, &record, &explain_context(&client, &url), |_| {})
            .await
            .expect("an unreachable server is not an error");

    assert!(explanation.unavailable);
    assert!(explanation.text.is_empty());
}

// --- Analyse: the writer of the mixed tier, with the record, and the addendum ------------------

async fn analyse(
    folder: &Folder,
    sources: Vec<Evidence>,
    documents_unavailable: Option<MixedPartUnavailable>,
    url: &str,
) -> (mixed_answer::MixedAnswer, bool) {
    let answer = folder.figure(QUESTION);
    let sizes = tabular_answer::sheet_sizes(&folder.open(), &ScopeMode::WholeFolder);
    let client = GatewayClient::new().expect("builds a client");
    let context = MixedContext {
        locale: "en-US",
        gateway: &client,
        server_url: url,
        model_alias: "cabinet-chat",
        idle_timeout: IDLE_TIMEOUT,
        history: &[],
        budget: ContextBudget::DEFAULT,
        cards_text:
            "Workbook cards (structure only, no data values):\nWorkbook \"factures.csv\" (csv)",
        sample_text: "",
        calculation: CalculationSide {
            sheet_sizes: &sizes,
            analysis: true,
        },
        formulas: None,
        folder_facts: "",
        rows: None,
    };
    mixed_answer::answer(
        QUESTION,
        PendingAnswer::Done(answer),
        sources,
        documents_unavailable,
        &context,
        |_| {},
        |_| {},
    )
    .await
    .expect("the writer answers")
}

#[tokio::test]
async fn the_analysis_with_tables_only_sends_the_record_the_cards_and_the_addendum() {
    let folder = Folder::build();
    let model =
        start_gateway_replying("The total of 1840 is steady. Ask which supplier costs the most.");

    let (result, generated) = analyse(
        &folder,
        Vec::new(),
        Some(MixedPartUnavailable::NotSelected),
        &model.url,
    )
    .await;

    assert!(generated);
    assert_eq!(model.calls(), 1, "no embedding call, one writer call");
    let sent = &model.requests()[0];
    assert!(sent.contains(ANALYSIS_ADDENDUM));
    assert!(sent.contains("Table results (workbook \"factures.csv\"):\nCalculation record:"));
    assert!(sent.contains("Rows: 8 kept out of 8 data rows"));
    assert!(sent.contains("Workbook cards (structure only"));
    assert!(sent.contains("Document excerpts: none."));
    assert!(!sent.contains("Zorglub"), "no cell value reaches the model");
    // The figure shown is the engine's.
    assert_eq!(
        result.table.as_ref().map(|table| match table {
            TabularAnswer::Value { value, .. } => value.clone(),
            other => panic!("{other:?}"),
        }),
        Some(the_total())
    );
    assert_eq!(
        result.documents_unavailable,
        Some(MixedPartUnavailable::NotSelected)
    );
}

#[tokio::test]
async fn a_number_the_analysis_adds_is_corrected_against_the_figure() {
    let folder = Folder::build();
    let model = start_gateway_replying("The total is 1480 euros, a little low.");

    let (result, _) = analyse(
        &folder,
        Vec::new(),
        Some(MixedPartUnavailable::NotSelected),
        &model.url,
    )
    .await;

    assert_eq!(result.corrections.len(), 1, "{:?}", result.corrections);
    assert_eq!(result.corrections[0].claimed, "1480");
    assert_eq!(result.corrections[0].correct, 1840.0);
}

#[tokio::test]
async fn a_row_count_quoted_from_the_record_is_not_corrected() {
    let folder = Folder::build();
    let model = start_gateway_replying("The total is 1840 over 8 of the 8 data rows.");

    let (result, _) = analyse(
        &folder,
        Vec::new(),
        Some(MixedPartUnavailable::NotSelected),
        &model.url,
    )
    .await;

    assert!(result.corrections.is_empty(), "{:?}", result.corrections);
}

#[tokio::test]
async fn the_analysis_with_documents_sends_the_record_and_the_excerpts_and_checks_citations() {
    let folder = Folder::build();
    let model = start_gateway_replying(
        "The contract states a total of 1840 euros [1], which matches the table [2].",
    );
    let sources = vec![evidence(
        "The contract states a total of 1840 euros for all suppliers.",
    )];

    let (result, _) = analyse(&folder, sources, None, &model.url).await;

    let sent = &model.requests()[0];
    assert!(sent.contains(ANALYSIS_ADDENDUM));
    assert!(sent.contains("The contract states a total of 1840 euros"));
    assert!(sent.contains("Calculation record:"));
    assert_eq!(result.document_sources.len(), 1);
    assert_eq!(result.rejected_citations, vec!["[2]".to_string()]);
    assert!(matches!(result.table, Some(TabularAnswer::Value { .. })));
}

#[tokio::test]
async fn the_analysis_keeps_the_figure_when_the_ai_machine_is_stopped() {
    let folder = Folder::build();
    let url = hanging_up_gateway_url();

    let (result, generated) = analyse(
        &folder,
        Vec::new(),
        Some(MixedPartUnavailable::NotSelected),
        &url,
    )
    .await;

    assert!(generated, "the writer was dialled and could not be reached");
    assert!(result.answer.is_empty());
    assert_eq!(
        result.documents_unavailable,
        Some(MixedPartUnavailable::GatewayUnavailable)
    );
    assert!(matches!(
        result.table,
        Some(TabularAnswer::Value { value, .. }) if value == the_total()
    ));
}

#[tokio::test]
async fn an_ordinary_figure_without_documents_still_answers_with_no_model_call() {
    // The guarantee of step 03 is kept for the ordinary question: only the analysis reaches the model.
    let folder = Folder::build();
    let model = start_gateway_replying("never read");
    let client = GatewayClient::new().expect("builds a client");
    let context = MixedContext {
        locale: "en-US",
        gateway: &client,
        server_url: &model.url,
        model_alias: "cabinet-chat",
        idle_timeout: IDLE_TIMEOUT,
        history: &[],
        budget: ContextBudget::DEFAULT,
        cards_text: "",
        sample_text: "",
        calculation: Default::default(),
        formulas: None,
        folder_facts: "",
        rows: None,
    };

    let (result, generated) = mixed_answer::answer(
        QUESTION,
        PendingAnswer::Done(folder.figure(QUESTION)),
        Vec::new(),
        Some(MixedPartUnavailable::NotSelected),
        &context,
        |_| {},
        |_| {},
    )
    .await
    .unwrap();

    assert!(!generated);
    assert_eq!(model.calls(), 0);
    assert!(result.answer.is_empty());
}
