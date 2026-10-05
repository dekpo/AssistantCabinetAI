//! Regression cases from the first human acceptance pass (`docs/test-reports/human-acceptance-pass-1/`,
//! HAP-1, 3 October 2026): the two workbooks the owner used, copied verbatim into that report's
//! `fixtures/data/`, asked through tier 2 exactly as the app does. Every expectation is the hand
//! computation recorded in the report's `fixtures/README.md`.
//!
//! Cases are asked in French and in English. A deterministic case makes no gateway call; the
//! gateway address given to `ModelAssist` refuses every connection, so a question that unexpectedly
//! reached the model would show up as a nudge instead of the expected value.

use std::path::PathBuf;
use std::time::Duration;

use assistant_cabinet_ai_lib::analysis_scope::{ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::FileHashCache;
use assistant_cabinet_ai_lib::tabular::engine::{AppliedFilter, NotAnswerableReason, TabularValue};
use assistant_cabinet_ai_lib::tabular_answer::{self, ModelAssist, TabularAnswer};

const INVOICES: &str = "factures-fournisseurs-2026.xlsx";
const APPOINTMENTS: &str = "rdv-mars-2026.xlsx";

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("test-reports")
        .join("human-acceptance-pass-1")
        .join("fixtures")
        .join("data")
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
        for name in [INVOICES, APPOINTMENTS] {
            std::fs::copy(fixtures_dir().join(name), data.path().join(name))
                .unwrap_or_else(|error| panic!("copies {name}: {error}"));
        }
        let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
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

    fn ticked(&self, files: &[&str]) -> ScopeMode {
        let folder = self.open();
        ScopeMode::Explicit(
            files
                .iter()
                .map(|file| ScopeEntry {
                    relative_path: file.to_string(),
                    pinned_id: folder.files().find_by_relative_path(file).unwrap().id.clone(),
                    added_at: 1,
                    sheet_names: Vec::new(),
                })
                .collect(),
        )
    }

    async fn ask(&self, question: &str, locale: &str, files: &[&str]) -> TabularAnswer {
        let url = closed_gateway_url();
        let assist = ModelAssist {
            gateway: &self.gateway,
            server_url: &url,
            model_alias: "cabinet-chat",
            idle_timeout: Duration::from_millis(200),
        };
        tabular_answer::answer(question, &self.open(), &self.ticked(files), locale, &self.index, &assist)
            .await
            .unwrap_or_else(|error| panic!("{question:?}: {error:?}"))
    }
}

fn value_of(answer: &TabularAnswer) -> (&TabularValue, &[AppliedFilter]) {
    match answer {
        TabularAnswer::Value { value, locator, .. } => (value, &locator.filters),
        other => panic!("expected a computed value, got {other:?}"),
    }
}

fn sum_of(value: &TabularValue) -> f64 {
    match value {
        TabularValue::Sum(total) => total.value,
        other => panic!("expected a sum, got {other:?}"),
    }
}

// --- BUG-02: the digits of a date are not filters of their own (Q14) -------------------------

#[tokio::test]
async fn a_date_range_counts_the_rows_in_the_range_and_nothing_else() {
    let folder = Folder::build();
    for (question, locale) in [
        ("Combien de rendez-vous entre le 09/03/2026 et le 15/03/2026 ?", "fr-FR"),
        ("How many appointments between 2026-03-09 and 2026-03-15?", "en-US"),
    ] {
        let answer = folder.ask(question, locale, &[APPOINTMENTS]).await;
        let (value, filters) = value_of(&answer);
        assert_eq!(*value, TabularValue::Count(4), "{question:?}: 9, 10, 11 and 12 March");
        assert_eq!(
            filters,
            &[AppliedFilter::DateRange {
                column: "date".to_string(),
                start: "2026-03-09".to_string(),
                end: "2026-03-15".to_string(),
            }],
            "{question:?}: only the range was understood"
        );
    }
}

#[tokio::test]
async fn one_written_date_is_that_day_exactly() {
    let folder = Folder::build();
    let answer = folder
        .ask("Combien de rendez-vous le 09/03/2026 ?", "fr-FR", &[APPOINTMENTS])
        .await;
    let (value, filters) = value_of(&answer);
    assert_eq!(*value, TabularValue::Count(1));
    assert_eq!(filters.len(), 1, "{filters:?}");
}

// --- BUG-02: a particle of the sentence is not a cell value (Q16) ----------------------------

#[tokio::test]
async fn the_room_with_the_least_minutes_is_not_filtered_by_a_patient_named_a() {
    let folder = Folder::build();
    for (question, locale) in [
        ("Quelle salle a le moins de duree_min ?", "fr-FR"),
        ("Which salle has the least duree_min?", "en-US"),
    ] {
        let answer = folder.ask(question, locale, &[APPOINTMENTS]).await;
        let (value, filters) = value_of(&answer);
        let TabularValue::LeastGroup(ranking) = value else {
            panic!("{question:?}: expected the least group, got {value:?}");
        };
        assert_eq!(ranking.top[0].group, "Salle 3", "{question:?}");
        assert_eq!(ranking.top[0].sum, 50.0, "{question:?}");
        assert!(filters.is_empty(), "{question:?}: nothing but the question's own words: {filters:?}");
    }
}

// --- BUG-02: an amount is not a year (Q19 with digits) --------------------------------------

#[tokio::test]
async fn an_amount_over_five_thousand_is_never_read_as_the_year_five_thousand() {
    let folder = Folder::build();
    let answer = folder
        .ask(
            "Est-ce qu'on a dépensé plus de 5000 euros avec un seul fournisseur ce trimestre ?",
            "fr-FR",
            &[INVOICES],
        )
        .await;
    let TabularAnswer::Value { locator, .. } = &answer else {
        return; // a refusal is acceptable here; a wrong year is not
    };
    assert!(
        !locator
            .filters
            .iter()
            .any(|filter| matches!(filter, AppliedFilter::Year { .. })),
        "no year filter may be invented: {:?}",
        locator.filters
    );
}

// --- BUG-05: an aggregate reads the amount, not the longest column name (Q24) ----------------

#[tokio::test]
async fn a_total_names_the_numeric_column_even_when_a_text_column_is_named_too() {
    let folder = Folder::build();
    for (question, locale) in [
        ("Quel est le montant total pour le fournisseur ?", "fr-FR"),
        ("What is the total montant for the fournisseur?", "en-US"),
    ] {
        let answer = folder.ask(question, locale, &[INVOICES]).await;
        let (value, _) = value_of(&answer);
        assert_eq!(sum_of(value), 2215.0, "{question:?}");
    }
}

// --- BUG-15: the workbook that has the named column is the one meant (Q9) -------------------

#[tokio::test]
async fn a_named_column_picks_the_workbook_that_has_it() {
    let folder = Folder::build();
    let both = [INVOICES, APPOINTMENTS];

    let answer = folder.ask("Quelle est la somme des montant ?", "fr-FR", &both).await;
    let (value, _) = value_of(&answer);
    assert_eq!(sum_of(value), 2215.0);
    let TabularAnswer::Value { file, .. } = &answer else { unreachable!() };
    assert_eq!(file, INVOICES);

    let answer = folder.ask("What is the mean of duree_min?", "en-US", &both).await;
    let (value, _) = value_of(&answer);
    let TabularValue::Mean(mean) = value else { panic!("expected a mean, got {value:?}") };
    assert_eq!(mean.value, 24.5);
}

#[tokio::test]
async fn no_named_column_still_asks_which_workbook() {
    let folder = Folder::build();
    let answer = folder
        .ask("Combien de lignes ?", "fr-FR", &[INVOICES, APPOINTMENTS])
        .await;
    assert!(
        matches!(answer, TabularAnswer::WhichWorkbook { .. }),
        "got {answer:?}"
    );
}

// --- The answers HAP-1 already got right stay right -----------------------------------------

#[tokio::test]
async fn the_hap_1_answers_that_passed_still_pass() {
    let folder = Folder::build();

    let answer = folder.ask("Combien de rendez-vous le lundi ?", "fr-FR", &[APPOINTMENTS]).await;
    assert_eq!(*value_of(&answer).0, TabularValue::Count(3));

    let answer = folder.ask("Quelle est la moyenne de duree_min ?", "fr-FR", &[APPOINTMENTS]).await;
    let TabularValue::Mean(mean) = value_of(&answer).0 else { panic!("a mean") };
    assert_eq!(mean.value, 24.5);

    let answer = folder.ask("Quel fournisseur a le plus de montant ?", "fr-FR", &[INVOICES]).await;
    let TabularValue::LargestGroup(ranking) = value_of(&answer).0 else { panic!("a ranking") };
    assert_eq!(ranking.top[0].group, "MedSupply");
    assert_eq!(ranking.top[0].sum, 1450.0);

    let answer = folder
        .ask("Combien de factures pour le fournisseur MedSupply ?", "fr-FR", &[INVOICES])
        .await;
    assert_eq!(*value_of(&answer).0, TabularValue::Count(3));

    let answer = folder.ask("Quel est le maximum de montant ?", "fr-FR", &[INVOICES]).await;
    let TabularValue::Max(max) = value_of(&answer).0 else { panic!("a maximum") };
    assert_eq!(max.value, 620.0);
}

// --- Q24: a question about a document is not answered from the table alone -----------------

#[test]
fn a_question_about_a_document_is_left_to_the_mixed_tier_not_answered_from_the_whole_table() {
    let folder = Folder::build();
    let data = folder.open();
    let scope = folder.ticked(&[INVOICES]);
    let route = |question: &str, locale: &str| {
        tabular_answer::prepare_if_data_only(question, &data, &scope, locale, &folder.index)
            .unwrap()
            .is_some()
    };
    assert!(
        !route("Quel est le montant total pour le fournisseur mentionn\u{e9} dans cette lettre ?", "fr-FR"),
        "the supplier is named by the letter: the table alone cannot answer"
    );
    assert!(!route("What is the total montant for the supplier mentioned in this letter?", "en-US"));
    assert!(route("Quelle est la somme des montant ?", "fr-FR"), "a plain data question stays data-only");
}

// --- BUG-17: "plus de N" / "moins de N" is a threshold, in both languages ------------------

fn threshold(column: &str, value: f64, greater: bool) -> AppliedFilter {
    if greater {
        AppliedFilter::GreaterThan { column: column.to_string(), threshold: value }
    } else {
        AppliedFilter::LessThan { column: column.to_string(), threshold: value }
    }
}

#[tokio::test]
async fn a_written_threshold_filters_the_rows_instead_of_being_dropped() {
    let folder = Folder::build();
    // montant: 450, 180, 620, 95, 210, 380, 120, 160
    for (question, locale, expected, filter) in [
        ("Combien de factures de plus de 400 euros ?", "fr-FR", 2, threshold("montant", 400.0, true)),
        ("Combien de factures de plus de 500 euros ?", "fr-FR", 1, threshold("montant", 500.0, true)),
        ("Combien de factures de moins de 200 euros ?", "fr-FR", 4, threshold("montant", 200.0, false)),
        ("Combien de factures sup\u{e9}rieures \u{e0} 400 euros ?", "fr-FR", 2, threshold("montant", 400.0, true)),
        ("How many invoices with montant over 400?", "en-US", 2, threshold("montant", 400.0, true)),
        ("How many invoices with montant more than 400?", "en-US", 2, threshold("montant", 400.0, true)),
        ("How many invoices with montant under 200?", "en-US", 4, threshold("montant", 200.0, false)),
    ] {
        let answer = folder.ask(question, locale, &[INVOICES]).await;
        let (value, filters) = value_of(&answer);
        assert_eq!(*value, TabularValue::Count(expected), "{question:?}");
        assert_eq!(filters, &[filter], "{question:?}: exactly the threshold was understood");
    }
}

#[tokio::test]
async fn a_threshold_written_with_a_thousands_space_is_read_whole() {
    let folder = Folder::build();
    let answer = folder
        .ask("Combien de factures de plus de 5 000 euros ?", "fr-FR", &[INVOICES])
        .await;
    let (value, filters) = value_of(&answer);
    assert_eq!(*value, TabularValue::Count(0));
    assert_eq!(filters, &[threshold("montant", 5000.0, true)]);
}

#[tokio::test]
async fn a_threshold_the_engine_cannot_apply_as_written_is_never_dropped() {
    let folder = Folder::build();
    for question in [
        "Combien de factures d'au moins 400 euros ?",
        "Combien de factures de pas plus de 400 euros ?",
        "Combien de factures de plus de 1,500 euros ?",
    ] {
        let answer = folder.ask(question, "fr-FR", &[INVOICES]).await;
        assert!(
            matches!(answer, TabularAnswer::Nudge { .. }),
            "{question:?} must not answer an unfiltered or wrongly filtered count: {answer:?}"
        );
    }
}

#[tokio::test]
async fn a_superlative_is_still_a_group_ranking_not_a_threshold() {
    let folder = Folder::build();

    let answer = folder.ask("Quelle salle a le plus de duree_min ?", "fr-FR", &[APPOINTMENTS]).await;
    let (value, filters) = value_of(&answer);
    let TabularValue::LargestGroup(ranking) = value else { panic!("a ranking, got {value:?}") };
    assert_eq!(ranking.top[0].group, "Salle 1");
    assert!(filters.is_empty(), "{filters:?}");

    let answer = folder
        .ask("Quel fournisseur a le plus de montant en 2026 ?", "fr-FR", &[INVOICES])
        .await;
    let (value, filters) = value_of(&answer);
    assert!(matches!(value, TabularValue::LargestGroup(_)), "{value:?}");
    assert!(
        !filters.iter().any(|filter| matches!(filter, AppliedFilter::GreaterThan { .. })),
        "the year after \"le plus de montant\" is not a threshold: {filters:?}"
    );
}

// --- Q19: a group's total against a threshold is refused, not answered as a row count -------

#[tokio::test]
async fn a_threshold_on_a_groups_total_is_refused_not_answered_as_a_row_count() {
    let folder = Folder::build();
    for (question, locale) in [
        ("Est-ce qu'on a d\u{e9}pens\u{e9} plus de 5000 euros avec un seul fournisseur ce trimestre ?", "fr-FR"),
        ("Is there a fournisseur with more than 5000?", "en-US"),
    ] {
        let answer = folder.ask(question, locale, &[INVOICES]).await;
        let TabularAnswer::Nudge { reason, model_attempt, .. } = answer else {
            panic!("{question:?}: expected a refusal, got {answer:?}");
        };
        assert_eq!(reason, Some(NotAnswerableReason::GroupThresholdNotSupported), "{question:?}");
        assert!(model_attempt.is_none(), "{question:?}: the model cannot help, so it is not asked");
    }
}

#[tokio::test]
async fn a_threshold_with_a_value_for_the_column_is_still_a_row_filter() {
    let folder = Folder::build();
    // MedSupply rows: 450, 620, 380 - two are above 400.
    let answer = folder
        .ask("Combien de factures de plus de 400 euros pour le fournisseur MedSupply ?", "fr-FR", &[INVOICES])
        .await;
    let (value, _) = value_of(&answer);
    assert_eq!(*value, TabularValue::Count(2));
}

// --- BUG-12: a name that matches nothing and has no close value is refused at once ---------

#[tokio::test]
async fn a_name_with_no_close_value_is_refused_without_asking_the_model() {
    let folder = Folder::build();
    let answer = folder.ask("Combien de factures pour Zorglub ?", "fr-FR", &[INVOICES]).await;
    let TabularAnswer::Nudge { reason, model_attempt, .. } = answer else {
        panic!("expected a refusal, got {answer:?}");
    };
    assert_eq!(reason, Some(NotAnswerableReason::ValueNotFound));
    assert!(model_attempt.is_none(), "no wait for a refusal the data already settles");
}

// --- BUG-18: pressing "Ask AI" when the model cannot do better says so ----------------------

#[tokio::test]
async fn a_failed_ask_ai_keeps_the_computed_value_and_says_the_model_was_asked() {
    let folder = Folder::build();
    let question = "Quelle est la somme des montant ?";
    let data = folder.open();
    let scope = folder.ticked(&[INVOICES]);
    let pending =
        tabular_answer::prepare(question, &data, &scope, "fr-FR", &folder.index, true).unwrap();
    let url = closed_gateway_url();
    let assist = ModelAssist {
        gateway: &folder.gateway,
        server_url: &url,
        model_alias: "cabinet-chat",
        idle_timeout: Duration::from_millis(200),
    };
    let answer = tabular_answer::resolve(pending, question, "fr-FR", &assist).await;
    let TabularAnswer::Value { value, model_attempt, .. } = &answer else {
        panic!("the computed value is kept, got {answer:?}");
    };
    assert_eq!(sum_of(value), 2215.0);
    assert_eq!(model_attempt.as_ref().map(|attempt| attempt.model_alias.as_str()), Some("cabinet-chat"));
}

// --- BUG-13: a refusal about a column's content names the column ---------------------------

#[tokio::test]
async fn adding_up_a_text_column_is_refused_and_names_that_column() {
    let folder = Folder::build();
    for (question, locale) in [
        ("Quelle est la somme de fournisseur ?", "fr-FR"),
        ("What is the sum of fournisseur?", "en-US"),
    ] {
        let answer = folder.ask(question, locale, &[INVOICES]).await;
        let TabularAnswer::Nudge { reason, filter_column, .. } = answer else {
            panic!("{question:?}: expected a refusal, got {answer:?}");
        };
        assert_eq!(reason, Some(NotAnswerableReason::NonNumericColumn), "{question:?}");
        assert_eq!(filter_column.as_deref(), Some("fournisseur"), "{question:?}");
    }
}

// --- Lot D: a request to write is not a data-only question ---------------------------------

#[test]
fn a_request_to_write_something_is_left_to_the_tier_that_reads_both_sources() {
    let folder = Folder::build();
    let data = folder.open();
    let scope = folder.ticked(&[INVOICES]);
    let data_only = |question: &str, locale: &str| {
        tabular_answer::prepare_if_data_only(question, &data, &scope, locale, &folder.index)
            .unwrap()
            .is_some()
    };
    assert!(!data_only("G\u{e9}n\u{e8}re un courrier avec le montant total des factures", "fr-FR"));
    assert!(!data_only("R\u{e9}dige un r\u{e9}sum\u{e9} de la somme des montant", "fr-FR"));
    assert!(!data_only("Write a letter with the total montant", "en-US"));
    assert!(!data_only("Draft a summary of the sum of montant", "en-US"));
    // A plain question about the data stays instant, with no model.
    assert!(data_only("Quelle est la somme des montant ?", "fr-FR"));
    assert!(data_only("What is the sum of montant?", "en-US"));
}
