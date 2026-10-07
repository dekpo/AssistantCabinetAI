//! Session 15 (`docs/SESSION-DATA-15-Mixed-Routing.md`), D7 step 1 (`docs/DECISIONS.md`): with
//! documents and tables both selected (`GroundingTier::DocumentsAndTables`), a question that is
//! clearly and only about the data - the tabular classifier recognises it and nothing is left
//! over but filler and the filter words it applied - is answered by the tabular engine alone,
//! before any file is read for retrieval. Every other question keeps the existing
//! `documents_and_tables_together` refusal, unchanged.
//!
//! Exercised at `tabular_answer::prepare_if_data_only`/`resolve` - the exact two calls
//! `commands::sourced_answer`'s own mixed-selection router makes - rather than at
//! `commands::ask_with_sources` itself, which needs a real Tauri `AppHandle` nothing in this
//! crate's tests constructs (the same reason `tests/tabular_query_plan.rs` tests the
//! model-assisted path one level below the Tauri command, too).
//!
//! French test data, per this session's own scope.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use assistant_cabinet_ai_lib::analysis_scope::{ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::FileHashCache;
use assistant_cabinet_ai_lib::tabular::engine::TabularValue;
use assistant_cabinet_ai_lib::tabular_answer::{self, ModelAssist, TabularAnswer};

const IDLE_TIMEOUT: Duration = Duration::from_millis(500);

/// A fake gateway that counts every request it receives and never answers one usefully. Any call
/// reaching it at all is already the failure this file's tests exist to catch: a mixed selection
/// must never dial the gateway, whatever the question.
struct CountingGateway {
    url: String,
    calls: Arc<AtomicUsize>,
}

fn start_counting_gateway() -> CountingGateway {
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
    let url = format!("http://{}", server.server_addr());
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            counted.fetch_add(1, Ordering::SeqCst);
            let _ = request.respond(tiny_http::Response::from_string(
                r#"{"unsupported": true}"#,
            ));
        }
    });
    CountingGateway { url, calls }
}

/// `date, fournisseur, montant`: ten rows (`SheetInventory::looks_tabular` needs at least eight),
/// three of them in March 2026 (the 2nd, the 10th and the 20th), so "how many invoices in March
/// 2026" has a hand-checkable answer of 3. `Omega` never appears as a `fournisseur`, so a question
/// naming it is a residual value no reachable column holds.
fn factures_csv() -> String {
    "date,fournisseur,montant\n\
     2026-01-05,Alpha,100\n\
     2026-01-15,Beta,150\n\
     2026-02-01,Gamma,200\n\
     2026-02-10,Alpha,250\n\
     2026-02-20,Beta,300\n\
     2026-03-02,Gamma,300\n\
     2026-03-10,Alpha,400\n\
     2026-03-20,Beta,500\n\
     2026-04-01,Gamma,600\n\
     2026-04-15,Alpha,650\n"
        .to_string()
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

    /// The one workbook, ticked - standing in for "documents and tables both selected": this
    /// file's own tests are only ever about the data side of that selection, since the document
    /// side is never read on this path at all (`tabular_answer::prepare_if_data_only` has no
    /// retrieval dependency to call even by accident).
    fn ticked(&self) -> ScopeMode {
        let folder = self.open();
        let record = folder.files().find_by_relative_path("factures.csv").unwrap();
        ScopeMode::Explicit(vec![ScopeEntry {
            relative_path: "factures.csv".to_string(),
            pinned_id: record.id.clone(),
            added_at: 1,
            sheet_names: Vec::new(),
        }])
    }

    /// Mirrors `commands::sourced_answer`'s own mixed-selection router exactly: `None` from
    /// `prepare_if_data_only` is returned untouched - the caller keeps its mixed-selection refusal
    /// - and only `Some` is ever handed to `resolve`, with a real `ModelAssist` pointed at
    /// `gateway_url`.
    async fn ask(&self, question: &str, locale: &str, gateway_url: &str) -> Option<TabularAnswer> {
        let pending = tabular_answer::prepare_if_data_only(
            question,
            &self.open(),
            &self.ticked(),
            locale,
            &self.index,
        )
        .unwrap();
        let pending = pending?;
        let gateway = GatewayClient::new().expect("builds a client");
        let assist = ModelAssist {
            gateway: &gateway,
            server_url: gateway_url,
            model_alias: "cabinet-chat",
            idle_timeout: IDLE_TIMEOUT,
        };
        Some(tabular_answer::resolve(pending, question, locale, &assist).await)
    }
}

// --- A question clearly and only about the data is answered by the engine alone -------------

#[tokio::test]
async fn a_clearly_data_only_question_is_answered_by_the_engine_alone() {
    let folder = Folder::build();
    for (question, locale) in [
        ("Combien de factures en mars 2026 ?", "fr-FR"),
        ("How many invoices in March 2026?", "en-US"),
    ] {
        let gateway = start_counting_gateway();
        let result = folder.ask(question, locale, &gateway.url).await;

        let Some(TabularAnswer::Value { value, .. }) = result else {
            panic!("{question:?}: expected a computed value, got {result:?}");
        };
        assert_eq!(value, TabularValue::Count(3), "{question:?}");
        assert_eq!(
            gateway.calls.load(Ordering::SeqCst),
            0,
            "{question:?}: a recognised, residual-clean question must never dial the gateway"
        );
    }
}

// --- A pure content question keeps the mixed-selection refusal, unchanged --------------------

#[tokio::test]
async fn a_pure_content_question_is_not_answered_from_the_data_alone() {
    let folder = Folder::build();
    for (question, locale) in [
        ("Que dit le contrat sur Alpha ?", "fr-FR"),
        ("What does the contract say about Alpha?", "en-US"),
    ] {
        let gateway = start_counting_gateway();
        let result = folder.ask(question, locale, &gateway.url).await;

        assert_eq!(
            result, None,
            "{question:?}: not a data question at all, so the caller must keep its own refusal"
        );
        assert_eq!(gateway.calls.load(Ordering::SeqCst), 0, "{question:?}");
    }
}

// --- A question mixing the table and a document keeps the refusal too (session 16's own case) -

#[tokio::test]
async fn a_question_mixing_the_table_and_a_document_is_not_answered_from_the_data_alone() {
    let folder = Folder::build();
    let gateway = start_counting_gateway();

    let result = folder
        .ask(
            "Is the amount for Alpha in the table the same as in the contract?",
            "en-US",
            &gateway.url,
        )
        .await;

    assert_eq!(result, None, "combining both sides is session 16's own design, not this one's");
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 0);
}

// --- Gap G's automatic escalation does not fire for a mixed selection, by construction --------

#[tokio::test]
async fn an_unrecognised_data_question_never_reaches_the_model_either() {
    // On tier 2 alone (tables selected, no document) this exact question would escalate to the
    // model-assisted path (`tests/tabular_query_plan.rs`); on a mixed selection it must not -
    // `prepare_if_data_only` returns `None` before `resolve`, the only place this tier could dial
    // a gateway, is ever called at all. The counting gateway proves it rather than only asserting
    // it (`docs/SESSION-DATA-15-Mixed-Routing.md`, section 6).
    let folder = Folder::build();
    let gateway = start_counting_gateway();

    let result = folder
        .ask("Tell me something interesting about this data.", "en-US", &gateway.url)
        .await;

    assert_eq!(result, None);
    assert_eq!(
        gateway.calls.load(Ordering::SeqCst),
        0,
        "gap G's escalation must not fire on a mixed selection"
    );
}

// --- A recognised operation with a residual value no reachable column holds also refuses ------

#[tokio::test]
async fn a_residual_word_naming_no_real_data_also_keeps_the_refusal() {
    // "Omega" is not a real `fournisseur`: the classifier resolves `Sum` over `montant`, same as
    // it would for a clean question, but the leftover word is exactly `FilterDetection::ValueNotFound`
    // - gap G's other trigger, alongside `TabularRoute::NotRecognised` - so "recognised" alone is
    // not enough; nothing may be left over, filler and applied filters excepted.
    let folder = Folder::build();
    let gateway = start_counting_gateway();

    let result = folder
        .ask("What is the total montant for Omega?", "en-US", &gateway.url)
        .await;

    assert_eq!(result, None, "a residual value matching no real data must not be guessed at");
    assert_eq!(gateway.calls.load(Ordering::SeqCst), 0);
}
