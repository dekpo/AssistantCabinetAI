//! The publipostage finding (`docs/SESSION-DATA-17-Publipostage-Issue.md`, lot E): the owner's own
//! test data, copied verbatim into `docs/test-reports/human-acceptance-pass-1/fixtures/publipostage/`
//! (one row per order line: CMD-2026-001 and CMD-2026-003 have two lines, CMD-2026-002 has one).
//!
//! E2, the row lookup: a question that names an order number found in the data is a lookup of the
//! rows that hold it, shown as they are, with no model and no gateway call. The gateway address
//! given to `ModelAssist` refuses every connection, so a question that unexpectedly reached the
//! model would show up as a nudge instead of the rows.

use std::path::PathBuf;
use std::time::Duration;

use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::FileHashCache;
use assistant_cabinet_ai_lib::tabular::engine::{AppliedFilter, TabularValue};
use assistant_cabinet_ai_lib::tabular_answer::{self, ModelAssist, TabularAnswer};

const DATA: &str = "donnees_publipostage.csv";

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("test-reports")
        .join("human-acceptance-pass-1")
        .join("fixtures")
        .join("publipostage")
        .join(DATA)
}

fn closed_gateway_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("binds a free port");
    let url = format!("http://{}", listener.local_addr().expect("a local address"));
    drop(listener);
    url
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
        std::fs::copy(fixture(), data.path().join(DATA)).expect("copies the fixture");
        let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
        data_folder::analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();
        Self {
            data,
            _app: app,
            index,
            gateway: GatewayClient::new().expect("builds a client"),
        }
    }

    async fn ask(&self, question: &str, locale: &str) -> TabularAnswer {
        let folder =
            DataFolder::discover(self.data.path(), Some(&self.index), &FileHashCache::new()).unwrap();
        let url = closed_gateway_url();
        let assist = ModelAssist {
            gateway: &self.gateway,
            server_url: &url,
            model_alias: "cabinet-chat",
            idle_timeout: Duration::from_millis(200),
        };
        tabular_answer::answer(question, &folder, &ScopeMode::WholeFolder, locale, &self.index, &assist)
            .await
            .unwrap_or_else(|error| panic!("{question:?}: {error:?}"))
    }
}

fn rows_of(answer: &TabularAnswer) -> (usize, Vec<String>, &[AppliedFilter]) {
    let TabularAnswer::Value { value: TabularValue::Rows(rows), locator, .. } = answer else {
        panic!("expected the rows of an order, got {answer:?}");
    };
    let articles = rows
        .iter()
        .map(|row| {
            row.cells
                .iter()
                .find(|cell| cell.column == "Article")
                .map(|cell| cell.text.clone())
                .unwrap_or_default()
        })
        .collect();
    (rows.len(), articles, &locator.filters)
}

#[tokio::test]
async fn an_order_number_alone_shows_the_rows_that_hold_it() {
    let folder = Folder::build();
    for (question, locale) in [
        ("Quelle est la commande CMD-2026-002 ?", "fr-FR"),
        ("Show order CMD-2026-002", "en-US"),
    ] {
        let answer = folder.ask(question, locale).await;
        let (count, articles, filters) = rows_of(&answer);
        assert_eq!(count, 1, "{question:?}");
        assert_eq!(articles, vec!["Chaise Ergonomique".to_string()], "{question:?}");
        assert_eq!(
            filters,
            &[AppliedFilter::Equals {
                column: "No_Commande".to_string(),
                value: "CMD-2026-002".to_string(),
            }],
            "{question:?}: the filter is shown"
        );
    }
}

#[tokio::test]
async fn an_order_with_several_lines_shows_every_line() {
    let folder = Folder::build();
    let answer = folder.ask("Quelle est la commande CMD-2026-001 ?", "fr-FR").await;
    let (count, articles, _) = rows_of(&answer);
    assert_eq!(count, 2);
    assert_eq!(articles, vec!["Ordinateur Portable".to_string(), "Souris Sans Fil".to_string()]);
}

#[tokio::test]
async fn the_owners_own_question_finds_the_order_instead_of_dismissing_the_tables() {
    let folder = Folder::build();
    let answer = folder
        .ask(
            "G\u{e9}n\u{e8}re le courrier de la commande CMD-2026-002 en reprenant les donn\u{e9}es du client et en les ins\u{e9}rant dans la lettre correspondante.",
            "fr-FR",
        )
        .await;
    let (count, _, _) = rows_of(&answer);
    assert_eq!(count, 1);
}

#[tokio::test]
async fn an_order_number_the_data_does_not_hold_is_refused_at_once_with_the_close_ones() {
    let folder = Folder::build();
    let answer = folder.ask("Quelle est la commande CMD-2026-009 ?", "fr-FR").await;
    let TabularAnswer::Nudge { reason, filter_value, close_values, model_attempt, .. } = answer else {
        panic!("expected a refusal, got {answer:?}");
    };
    assert_eq!(
        reason,
        Some(assistant_cabinet_ai_lib::tabular::engine::NotAnswerableReason::ValueNotFound)
    );
    assert_eq!(filter_value.as_deref(), Some("CMD-2026-009"));
    assert!(close_values.contains(&"CMD-2026-001".to_string()), "{close_values:?}");
    assert!(model_attempt.is_none(), "no model wait for a value the data does not hold");
}

#[tokio::test]
async fn a_name_without_a_digit_is_not_looked_up() {
    // An ordinary word can equal a cell; only an identifier is looked up.
    let folder = Folder::build();
    let answer = folder.ask("Quelle est la commande de Sophie ?", "fr-FR").await;
    assert!(!matches!(answer, TabularAnswer::Value { value: TabularValue::Rows(_), .. }), "{answer:?}");
}

#[tokio::test]
async fn an_operation_with_an_order_number_is_still_that_operation() {
    let folder = Folder::build();
    let answer = folder
        .ask("Quelle est la somme de Total_Ligne pour la commande CMD-2026-001 ?", "fr-FR")
        .await;
    let TabularAnswer::Value { value: TabularValue::Sum(total), .. } = answer else {
        panic!("expected a sum, got {answer:?}");
    };
    assert_eq!(total.value, 924.5);
}
