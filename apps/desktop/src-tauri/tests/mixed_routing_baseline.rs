//! TABULAR series, step 01 (`docs/SESSION-TABULAR/SESSION-TABULAR-01-baseline-routing-tests.md`):
//! how a question asked with documents and spreadsheets ticked together is routed **today**, frozen
//! so the steps that change the router and the writer cannot break a case that works by accident.
//! Nothing here changes behaviour. A test that records a known defect says so and names the step
//! that flips it on purpose.
//!
//! The cases are the letters of the plan's section 1.3. They are exercised one level below the Tauri
//! command (`commands::sourced_answer` needs an `AppHandle` no test builds), at the calls the doors
//! make: `prepare_if_data_only` (door 3), `locate_fill_source` (door 1), `prepare_for_mixed` followed
//! by `mixed_answer::answer` (door 4, and door 0 which only skips the earlier doors). Case I, the
//! targeting of a document named in a mixed selection, lives in `commands::mixed_tier` and cannot be
//! frozen here: only the table half of it is (see `baseline_case_i_...`).
//!
//! Every workbook is written at test time into a temporary folder (`tests/common/tabular_fixtures.rs`
//! for `factures.csv` and `agencies.csv`, inline for the two others). Nothing is read from `fixtures/`.

mod common;

use std::time::Duration;

use assistant_cabinet_ai_lib::analysis_scope::{ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::conversation::ContextBudget;
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::extraction::PageOrigin;
use assistant_cabinet_ai_lib::fill_plan::candidate_templates;
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::{FileHashCache, WorkFolderInventory};
use assistant_cabinet_ai_lib::mixed_answer::{
    self, MixedAnswer, MixedContext, MixedPartUnavailable,
};
use assistant_cabinet_ai_lib::retrieval::Evidence;
use assistant_cabinet_ai_lib::tabular::card::card_budget_chars;
use assistant_cabinet_ai_lib::tabular::engine::{NotAnswerableReason, TabularValue};
use assistant_cabinet_ai_lib::tabular::question::{has_distributive_word, has_writing_intent};
use assistant_cabinet_ai_lib::tabular::sample::SampleBlock;
use assistant_cabinet_ai_lib::tabular::structural::StructuralAnswer;
use assistant_cabinet_ai_lib::tabular_answer::{
    self, ImpliedReason, ModelAssist, PendingAnswer, TabularAnswer,
};
use assistant_cabinet_ai_lib::TabularRowsMode;
use common::counting_gateway::{start_counting_gateway, start_gateway_replying, CountingGateway};
use common::tabular_fixtures::{self, Cell, AGENCIES, FACTURES};

const IDLE_TIMEOUT: Duration = Duration::from_millis(500);
const COMMANDES: &str = "commandes.csv";
const CALCULS: &str = "calculs.xlsx";
const TEMPLATE: &str = "lettre-type.txt";
const DOCUMENT: &str = "contrat.pdf";
/// The token of the markers in every test that samples rows: fixed, so a test can read the block.
const SAMPLE_TOKEN: &str = "t0k3n";

// --- The folders every case asks its question about ----------------------------------------------

struct Folders {
    data: tempfile::TempDir,
    documents: tempfile::TempDir,
    _app: tempfile::TempDir,
    index: IndexStore,
}

/// `commandes.csv`: ten orders with an identifier column, so a question can name one of them.
fn commandes_csv() -> String {
    let mut text = String::from("reference;client;montant\n");
    for i in 1..=10 {
        text.push_str(&format!("CMD-2026-{i:03};Client {i};{}\n", 100 + 10 * i));
    }
    text
}

/// `calculs.xlsx`: one sheet, sixty rows, `total` holds a formula on its first three rows only, so
/// the sheet stays usable (at most 5 % formula cells) while the workbook does contain formulas.
fn calculs_sheets() -> Vec<(String, Vec<Vec<Cell>>)> {
    let mut rows = vec![vec![
        Cell::text("libelle"),
        Cell::text("base"),
        Cell::text("total"),
    ]];
    for i in 0..60 {
        let sheet_row = i + 2;
        let base = (i + 1) as f64;
        let total = if i < 3 {
            Cell::Formula {
                expression: format!("B{sheet_row}*2"),
                cached: 2.0 * base,
            }
        } else {
            Cell::Number(2.0 * base)
        };
        rows.push(vec![
            Cell::text(&format!("Ligne {}", i + 1)),
            Cell::Number(base),
            total,
        ]);
    }
    vec![("Calculs".to_string(), rows)]
}

impl Folders {
    fn build() -> Self {
        let data = tempfile::tempdir().unwrap();
        let documents = tempfile::tempdir().unwrap();
        let app = tempfile::tempdir().unwrap();
        std::fs::write(data.path().join(FACTURES), tabular_fixtures::factures_csv()).unwrap();
        std::fs::write(data.path().join(AGENCIES), tabular_fixtures::agencies_csv()).unwrap();
        std::fs::write(data.path().join(COMMANDES), commandes_csv()).unwrap();
        tabular_fixtures::write_xlsx(&data.path().join(CALCULS), &calculs_sheets());
        // A mail-merge template: a plain text letter whose fields are columns of `factures.csv`.
        std::fs::write(
            documents.path().join(TEMPLATE),
            "Hello [fournisseur], your invoice of [montant] is due.\n",
        )
        .unwrap();
        let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
        data_folder::analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();
        Self {
            data,
            documents,
            _app: app,
            index,
        }
    }

    fn open(&self) -> DataFolder {
        DataFolder::discover(self.data.path(), Some(&self.index), &FileHashCache::new()).unwrap()
    }

    /// The named workbooks, ticked on purpose (what the user selected in the explorer).
    fn ticked(&self, names: &[&str]) -> ScopeMode {
        let folder = self.open();
        ScopeMode::Explicit(
            names
                .iter()
                .map(|name| {
                    let record = folder.files().find_by_relative_path(name).unwrap();
                    ScopeEntry {
                        relative_path: name.to_string(),
                        pinned_id: record.id.clone(),
                        added_at: 1,
                        sheet_names: Vec::new(),
                    }
                })
                .collect(),
        )
    }

    /// Door 3, exactly as `commands::mixed_data_only_tier` calls it. `None` leaves the question to
    /// the doors after it.
    fn data_only(
        &self,
        question: &str,
        locale: &str,
        selection: &ScopeMode,
    ) -> Option<PendingAnswer> {
        tabular_answer::prepare_if_data_only(question, &self.open(), selection, locale, &self.index)
            .unwrap()
    }

    /// Door 4's first half, exactly as `commands::mixed_tier` calls it (also door 0's, which only
    /// skips the doors before).
    fn for_mixed(&self, question: &str, locale: &str, selection: &ScopeMode) -> PendingAnswer {
        tabular_answer::prepare_for_mixed(
            question,
            &self.open(),
            selection,
            locale,
            &self.index,
            &[DOCUMENT.to_string()],
        )
        .unwrap()
    }

    /// Tables only, exactly as `commands::tabular_tier` calls it without the "Ask AI" button.
    fn tables_only(&self, question: &str, locale: &str, selection: &ScopeMode) -> PendingAnswer {
        tabular_answer::prepare(
            question,
            &self.open(),
            selection,
            locale,
            &self.index,
            false,
        )
        .unwrap()
    }

    /// Door 1's lookup, with the `every_row` flag the door derives from the question.
    fn fill_source(
        &self,
        question: &str,
        locale: &str,
        selection: &ScopeMode,
    ) -> Option<tabular_answer::FillSource> {
        let every_row = has_distributive_word(question, locale);
        tabular_answer::locate_fill_source(
            question,
            &self.open(),
            selection,
            locale,
            &self.index,
            every_row,
        )
        .unwrap()
    }
}

// --- Small readers over the answers ---------------------------------------------------------------

fn finished(pending: PendingAnswer) -> TabularAnswer {
    match pending {
        PendingAnswer::Done(answer) => answer,
        PendingAnswer::TryModel { fallback, .. } => {
            panic!("expected a finished answer, got TryModel with fallback {fallback:?}")
        }
    }
}

/// The fallback of a `TryModel`, and whether `resolve` would skip the model.
fn left_to_the_model(pending: PendingAnswer) -> (TabularAnswer, bool) {
    match pending {
        PendingAnswer::TryModel {
            fallback,
            skip_model,
            ..
        } => (fallback, skip_model),
        PendingAnswer::Done(answer) => {
            panic!("expected TryModel, got the finished answer {answer:?}")
        }
    }
}

fn is_unrecognised_nudge(answer: &TabularAnswer) -> bool {
    matches!(answer, TabularAnswer::Nudge { reason: None, .. })
}

async fn resolve(
    pending: PendingAnswer,
    question: &str,
    locale: &str,
    gateway_url: &str,
) -> TabularAnswer {
    let gateway = GatewayClient::new().expect("builds a client");
    let assist = ModelAssist {
        gateway: &gateway,
        server_url: gateway_url,
        model_alias: "cabinet-chat",
        idle_timeout: IDLE_TIMEOUT,
    };
    tabular_answer::resolve(pending, question, locale, &assist).await
}

fn evidence(text: &str) -> Evidence {
    Evidence {
        chunk_id: "chunk-1".to_string(),
        relative_path: DOCUMENT.to_string(),
        page_number: 1,
        ordinal: 1,
        section: None,
        text: text.to_string(),
        score: 1.0,
        origin: PageOrigin::TextLayer,
        confidence: None,
    }
}

/// The mixed tier from `mixed_answer::answer` on, with one document excerpt retrieved and the
/// workbook cards `commands::mixed_tier` would build (`tabular_answer::question_cards`, from what is
/// ticked).
async fn mixed(
    folders: &Folders,
    selection: &ScopeMode,
    pending: PendingAnswer,
    question: &str,
    locale: &str,
    gateway_url: &str,
) -> (MixedAnswer, bool) {
    mixed_from(
        folders,
        selection,
        pending,
        question,
        locale,
        gateway_url,
        vec![evidence(
            "The contract was signed in January and covers all suppliers.",
        )],
        None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn mixed_from(
    folders: &Folders,
    selection: &ScopeMode,
    pending: PendingAnswer,
    question: &str,
    locale: &str,
    gateway_url: &str,
    document_sources: Vec<Evidence>,
    documents_unavailable: Option<MixedPartUnavailable>,
) -> (MixedAnswer, bool) {
    mixed_with_rows_mode(
        folders,
        selection,
        pending,
        question,
        locale,
        gateway_url,
        document_sources,
        documents_unavailable,
        TabularRowsMode::Off,
    )
    .await
}

/// [`mixed_from`] with the setting `tabular_rows_mode` given: `Off` is the turn of step 03 (the
/// frozen baselines assert the exact blocks), `Sample` adds the sample and the profile exactly as
/// `commands::mixed_tier` builds them (`tabular_answer::question_samples`).
#[allow(clippy::too_many_arguments)]
async fn mixed_with_rows_mode(
    folders: &Folders,
    selection: &ScopeMode,
    pending: PendingAnswer,
    question: &str,
    locale: &str,
    gateway_url: &str,
    document_sources: Vec<Evidence>,
    documents_unavailable: Option<MixedPartUnavailable>,
    rows_mode: TabularRowsMode,
) -> (MixedAnswer, bool) {
    let cards = tabular_answer::question_cards(
        question,
        &folders.open(),
        selection,
        locale,
        &[],
        card_budget_chars(&ContextBudget::DEFAULT),
    );
    let sample = match rows_mode {
        TabularRowsMode::Off => SampleBlock::default(),
        TabularRowsMode::Sample => tabular_answer::question_samples(
            &cards.block,
            &folders.open(),
            selection,
            locale,
            &folders.index,
            &ContextBudget::DEFAULT,
            SAMPLE_TOKEN,
        ),
    };
    let gateway = GatewayClient::new().expect("builds a client");
    let context = MixedContext {
        locale,
        gateway: &gateway,
        server_url: gateway_url,
        model_alias: "cabinet-chat",
        idle_timeout: IDLE_TIMEOUT,
        history: &[],
        budget: ContextBudget::DEFAULT,
        cards_text: &cards.block.text,
        sample_text: &sample.text,
    };
    mixed_answer::answer(
        question,
        pending,
        document_sources,
        documents_unavailable,
        &context,
        |_| {},
        |_| {},
    )
    .await
    .expect("the mixed tier answers")
}

/// What the model was sent: the instruction and both blocks, then the question. Split at the
/// question so a test can look at the sources without the user's own words.
fn sent_sources(gateway: &CountingGateway) -> String {
    let requests = gateway.requests();
    assert_eq!(
        requests.len(),
        1,
        "one request for the writer: {requests:?}"
    );
    requests[0]
        .split_once("\n\nQuestion: ")
        .expect("the turn ends with the question")
        .0
        .to_string()
}

// --- A: a named workbook and an operation: the engine alone -------------------------------------

#[tokio::test]
async fn baseline_case_a_a_named_workbook_and_an_operation_is_answered_by_the_engine_alone() {
    // Frozen 20 October 2026.
    let folders = Folders::build();
    let selection = folders.ticked(&[FACTURES, AGENCIES]);
    for (question, locale, file, expected) in [
        (
            "Quel est le total du montant de factures.csv ?",
            "fr-FR",
            FACTURES,
            237_030.0,
        ),
        (
            "What is the total amount of agencies.csv?",
            "en-US",
            AGENCIES,
            270_599.5,
        ),
    ] {
        let gateway = start_counting_gateway();
        let pending = folders
            .data_only(question, locale, &selection)
            .unwrap_or_else(|| panic!("{question:?}: door 3 must answer a data-only question"));
        let answer = resolve(pending, question, locale, &gateway.url).await;

        let TabularAnswer::Value {
            file: answered_file,
            value: TabularValue::Sum(total),
            ..
        } = &answer
        else {
            panic!("{question:?}: expected a computed sum, got {answer:?}");
        };
        assert_eq!(answered_file, file, "{question:?}");
        assert_eq!(total.value, expected, "{question:?}");
        assert_eq!(
            gateway.calls(),
            0,
            "{question:?}: a computed figure never dials the model"
        );
    }
}

// --- B: the figure sent to the model names its workbook -----------------------------------------

#[tokio::test]
async fn baseline_case_b_the_figure_sent_to_the_model_names_its_workbook() {
    // Frozen 20 October 2026 with the defect that the writer got the figure and no file name.
    // Changed on purpose by step 03: the block names the workbook, and its card is sent too.
    let folders = Folders::build();
    let selection = folders.ticked(&[AGENCIES, FACTURES]);
    let question = "What is the total amount of agencies.csv?";
    let pending = folders.for_mixed(question, "en-US", &selection);
    let gateway = start_gateway_replying("The table gives the total.");

    let (result, generated) = mixed(
        &folders,
        &selection,
        pending,
        question,
        "en-US",
        &gateway.url,
    )
    .await;

    assert!(generated);
    assert!(
        matches!(
            &result.table,
            Some(TabularAnswer::Value {
                value: TabularValue::Sum(_),
                ..
            })
        ),
        "{:?}",
        result.table
    );
    let sources = sent_sources(&gateway);
    let table_block = sources
        .split_once("Table results (workbook \"agencies.csv\"):\n")
        .expect("the table block names the workbook")
        .1;
    assert_eq!(
        table_block,
        "Sheet agencies, column amount: sum = 270599.5 (computed over 2400 rows)"
    );
    // Only the named workbook has a card; the other ticked one is neither named nor implied.
    assert!(sources.contains("Workbook \"agencies.csv\""), "{sources}");
    assert!(!sources.contains("factures.csv"), "{sources}");
}

// --- C: a named workbook, no operation: its card reaches the writer -----------------------------

#[tokio::test]
async fn baseline_case_c_a_named_workbook_without_an_operation_reaches_the_writer_as_a_card() {
    // Frozen 20 October 2026 as "not asked about": the model got `Table results: none.` and the
    // interface said the question concerned none of the tables. Flipped on purpose by step 03.
    let folders = Folders::build();
    let selection = folders.ticked(&[AGENCIES, FACTURES]);
    for (question, locale, file, columns, value) in [
        (
            "Summarise agencies.csv",
            "en-US",
            AGENCIES,
            ["A  agency", "B  amount", "C  year"],
            "Harbor",
        ),
        (
            "R\u{e9}sume factures.csv",
            "fr-FR",
            FACTURES,
            ["A  date", "B  fournisseur", "C  client"],
            "Gamma",
        ),
    ] {
        assert!(
            folders.data_only(question, locale, &selection).is_none(),
            "{question:?}: door 3 has nothing to compute"
        );
        let (fallback, skip_model) =
            left_to_the_model(folders.for_mixed(question, locale, &selection));
        assert!(
            is_unrecognised_nudge(&fallback),
            "{question:?}: {fallback:?}"
        );
        assert!(!skip_model, "{question:?}");

        let gateway = start_gateway_replying("A summary written from the card.");
        let pending = folders.for_mixed(question, locale, &selection);
        let (result, generated) = mixed(
            &folders,
            &selection,
            pending,
            question,
            locale,
            &gateway.url,
        )
        .await;

        assert!(generated, "{question:?}");
        assert_eq!(result.table, None, "{question:?}");
        assert_eq!(result.table_unavailable, None, "{question:?}");
        assert_eq!(
            gateway.calls(),
            1,
            "{question:?}: the writer only, the hidden interpreter is not asked"
        );
        let sources = sent_sources(&gateway);
        assert!(
            sources.contains("Workbook cards (structure only, no data values):"),
            "{question:?}: {sources}"
        );
        assert!(
            sources.contains(&format!("Workbook \"{file}\"")),
            "{sources}"
        );
        for column in columns {
            assert!(
                sources.contains(column),
                "{question:?}: {column} in {sources}"
            );
        }
        // Structure only: no cell value, no row.
        assert!(!sources.contains(value), "{question:?}: {sources}");
        assert!(!sources.contains("9999.75"), "{sources}");
        assert!(sources.ends_with("Table results: none."), "{question:?}");
    }
}

// --- D: a structural question is answered from the inventory -------------------------------------

#[tokio::test]
async fn baseline_case_d_a_structural_question_is_answered_from_the_inventory() {
    // Frozen 20 October 2026.
    let folders = Folders::build();
    let selection = folders.ticked(&[FACTURES, AGENCIES]);
    for (question, locale, expected) in [
        (
            "Which columns does agencies.csv have?",
            "en-US",
            vec!["agency", "amount", "year"],
        ),
        (
            "Quelles sont les colonnes de factures.csv ?",
            "fr-FR",
            vec!["date", "fournisseur", "client", "montant"],
        ),
    ] {
        let gateway = start_counting_gateway();
        let pending = folders
            .data_only(question, locale, &selection)
            .unwrap_or_else(|| panic!("{question:?}: door 3 answers a structural question"));
        let answer = resolve(pending, question, locale, &gateway.url).await;

        let TabularAnswer::Structural {
            answer: StructuralAnswer::ColumnNames { columns, .. },
            ..
        } = &answer
        else {
            panic!("{question:?}: expected the column names, got {answer:?}");
        };
        assert_eq!(columns, &expected, "{question:?}");
        assert_eq!(gateway.calls(), 0, "{question:?}");
    }
}

// --- E: an identifier found in a cell shows its rows ---------------------------------------------

#[tokio::test]
async fn baseline_case_e_an_identifier_found_in_a_cell_shows_its_rows() {
    // Frozen 20 October 2026. No model, and the documents are ignored: the rows are shown as they
    // are, with no link to what the documents say about the order.
    let folders = Folders::build();
    let selection = folders.ticked(&[COMMANDES, AGENCIES]);
    for (question, locale) in [
        ("Show the order CMD-2026-002 of commandes.csv", "en-US"),
        ("Montre la commande CMD-2026-002 de commandes.csv", "fr-FR"),
    ] {
        let gateway = start_counting_gateway();
        let pending = folders
            .data_only(question, locale, &selection)
            .unwrap_or_else(|| panic!("{question:?}: door 3 shows the rows of an identifier"));
        let answer = resolve(pending, question, locale, &gateway.url).await;

        let TabularAnswer::Value {
            file,
            value: TabularValue::Rows(rows),
            ..
        } = &answer
        else {
            panic!("{question:?}: expected the rows of the order, got {answer:?}");
        };
        assert_eq!(file, COMMANDES, "{question:?}");
        assert_eq!(rows.len(), 1, "{question:?}");
        assert_eq!(
            rows[0].row_index, 1,
            "{question:?}: CMD-2026-002 is the second data row"
        );
        assert_eq!(gateway.calls(), 0, "{question:?}");
    }
}

// --- F: an operation with a word that matches no data is left to the model -----------------------

#[tokio::test]
async fn baseline_case_f_an_operation_with_a_word_matching_no_data_is_left_to_the_model() {
    // Frozen 20 October 2026. Door 3 declines; door 4 keeps the refusal (`ValueNotFound`) and, since
    // no real value is close to the word, `skip_model` spares the hidden interpreter.
    let folders = Folders::build();
    let selection = folders.ticked(&[AGENCIES, FACTURES]);
    let question = "What is the total amount of agencies.csv for Nowhere?";
    assert!(folders.data_only(question, "en-US", &selection).is_none());

    let (fallback, skip_model) =
        left_to_the_model(folders.for_mixed(question, "en-US", &selection));
    let TabularAnswer::Nudge {
        reason: Some(NotAnswerableReason::ValueNotFound),
        filter_value: Some(word),
        ..
    } = &fallback
    else {
        panic!("expected a value-not-found nudge, got {fallback:?}");
    };
    assert_eq!(word, "Nowhere");
    assert!(
        skip_model,
        "a word with no close value cannot be helped by the model"
    );

    let gateway = start_counting_gateway();
    let pending = folders.for_mixed(question, "en-US", &selection);
    let (result, generated) = mixed(
        &folders,
        &selection,
        pending,
        question,
        "en-US",
        &gateway.url,
    )
    .await;
    assert!(!generated);
    assert!(
        matches!(
            &result.table,
            Some(TabularAnswer::Nudge {
                reason: Some(NotAnswerableReason::ValueNotFound),
                ..
            })
        ),
        "{:?}",
        result.table
    );
    assert_eq!(gateway.calls(), 0);
}

// --- G: several workbooks, none named: which one? -------------------------------------------------

#[tokio::test]
async fn baseline_case_g_several_workbooks_and_none_named_asks_which() {
    // Frozen 20 October 2026. The choice is the whole answer; nothing is picked for the user.
    let folders = Folders::build();
    let selection = folders.ticked(&[AGENCIES, FACTURES]);
    for (question, locale) in [
        ("How many rows are there?", "en-US"),
        ("Combien de lignes y a-t-il ?", "fr-FR"),
    ] {
        let gateway = start_counting_gateway();
        let pending = folders
            .data_only(question, locale, &selection)
            .unwrap_or_else(|| panic!("{question:?}: door 3 asks which workbook"));
        let answer = resolve(pending, question, locale, &gateway.url).await;

        let TabularAnswer::WhichWorkbook { candidates, .. } = &answer else {
            panic!("{question:?}: expected a which-workbook question, got {answer:?}");
        };
        let mut candidates = candidates.clone();
        candidates.sort();
        assert_eq!(
            candidates,
            vec![AGENCIES.to_string(), FACTURES.to_string()],
            "{question:?}"
        );
        assert_eq!(gateway.calls(), 0, "{question:?}");
    }
}

#[tokio::test]
async fn baseline_case_g_a_column_only_one_ticked_workbook_has_already_chooses_it() {
    // Frozen 20 October 2026. KEPT: the engine already reads "amount" as "the workbook that has an
    // amount column" (HAP-1, BUG-15), without a name. Step 02's "implied by column" rule is the same
    // idea, applied to the workbook card.
    let folders = Folders::build();
    let selection = folders.ticked(&[AGENCIES, FACTURES]);
    let question = "What is the total amount?";
    let gateway = start_counting_gateway();
    let pending = folders
        .data_only(question, "en-US", &selection)
        .expect("door 3 answers");
    let answer = resolve(pending, question, "en-US", &gateway.url).await;

    let TabularAnswer::Value {
        file,
        value: TabularValue::Sum(total),
        ..
    } = &answer
    else {
        panic!("expected a computed sum, got {answer:?}");
    };
    assert_eq!(file, AGENCIES);
    assert_eq!(total.value, 270_599.5);
    assert_eq!(gateway.calls(), 0);
}

// --- H: "a letter for each invoice of X" finds no source -----------------------------------------

#[tokio::test]
async fn baseline_case_h_a_letter_for_each_row_of_a_named_workbook_finds_no_source() {
    // Frozen 20 October 2026. DEFECT, changes in step 08. Two single-sheet workbooks are ticked and
    // the question names one: because "each" sets `every_row`, both qualify, `locate_fill_source`
    // finds two and returns `None`. The name is never used to choose. Door 4 then lets the model
    // write letters from the documents, with `Table results: none.`.
    let folders = Folders::build();
    let both = folders.ticked(&[FACTURES, AGENCIES]);
    let only_factures = folders.ticked(&[FACTURES]);
    for (question, locale) in [
        ("Write a letter for each invoice of factures.csv", "en-US"),
        (
            "R\u{e9}dige une lettre pour chaque facture de factures.csv",
            "fr-FR",
        ),
    ] {
        assert!(has_writing_intent(question, locale), "{question:?}");
        assert!(has_distributive_word(question, locale), "{question:?}");

        assert!(
            folders.fill_source(question, locale, &both).is_none(),
            "{question:?}: the named workbook does not break the tie between two candidates"
        );

        // The control: with the named workbook alone ticked, door 1 does find its source, and the
        // template of the Documents folder fits its columns.
        let source = folders
            .fill_source(question, locale, &only_factures)
            .unwrap_or_else(|| panic!("{question:?}: one candidate is a source"));
        assert_eq!(source.file, FACTURES, "{question:?}");
        assert_eq!(source.key, None, "{question:?}");
        let inventory = WorkFolderInventory::discover(folders.documents.path(), None).unwrap();
        let templates = candidate_templates(&inventory, &source.table.columns);
        assert_eq!(
            templates
                .iter()
                .map(|t| t.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec![TEMPLATE],
            "{question:?}"
        );
    }

    // What door 4 then does with it: the table side is not asked about, the model writes.
    let question = "Write a letter for each invoice of factures.csv";
    let (fallback, _) = left_to_the_model(folders.for_mixed(question, "en-US", &both));
    assert!(is_unrecognised_nudge(&fallback), "{fallback:?}");
    let gateway = start_gateway_replying("Dear supplier, a letter written from the documents.");
    let pending = folders.for_mixed(question, "en-US", &both);
    let (result, generated) =
        mixed(&folders, &both, pending, question, "en-US", &gateway.url).await;
    assert!(generated);
    assert_eq!(result.table, None);
    // The question names factures.csv, so its card reaches the writer: the table side is present as
    // that card and only the table value is absent (flipped by step 03).
    assert_eq!(result.table_unavailable, None);
    let sources = sent_sources(&gateway);
    assert!(sources.contains("Workbook \"factures.csv\""), "{sources}");
    assert!(sources.ends_with("Table results: none."));
}

// --- I (partial): a name that no workbook carries has no table part -------------------------------

#[tokio::test]
async fn baseline_case_i_a_name_no_workbook_carries_is_left_to_the_documents() {
    // Frozen 20 October 2026 (fix 8abb8b5 on `main`). Only the table half of case I is frozen here:
    // the question reaches the documents. Targeting the named document (`Plan::OneFile`, its folder
    // context) happens in `commands::mixed_tier` and is a human test of step 08.
    let folders = Folders::build();
    let selection = folders.ticked(&[AGENCIES]);
    let question = "What does contrat-2026.pdf say about the total amount?";

    assert!(folders.data_only(question, "en-US", &selection).is_none());
    let answer = finished(folders.for_mixed(question, "en-US", &selection));
    assert!(is_unrecognised_nudge(&answer), "{answer:?}");
    let TabularAnswer::Nudge { file, .. } = &answer else {
        unreachable!()
    };
    assert_eq!(file, "", "the no-table-part nudge names no file");

    let gateway = start_gateway_replying("The contract says nothing about it.");
    let pending = folders.for_mixed(question, "en-US", &selection);
    let (result, generated) = mixed(
        &folders,
        &selection,
        pending,
        question,
        "en-US",
        &gateway.url,
    )
    .await;
    assert!(generated);
    // Changed by step 03: the only ticked workbook is implied (`implied_single`, plan 4.2 rule 1), so
    // its card reaches the writer and the table side is no longer "not asked about". With two
    // workbooks ticked and no sign, nothing is implied and the answer stays as it was frozen.
    assert_eq!(result.table_unavailable, None);
    assert!(sent_sources(&gateway).contains("Workbook \"agencies.csv\""));
    assert_eq!(gateway.calls(), 1);
}

// --- K: a formula question is a yes-or-no fact ----------------------------------------------------

#[tokio::test]
async fn baseline_case_k_a_formula_question_is_answered_with_a_yes_or_no() {
    // Frozen 20 October 2026. DEFECT, changes in step 05 (tables only) and step 07 (formula help):
    // the words "formula" and "formule" win over every other reading, so "explain the formula of
    // column total" gets a sentence about whether the sheet holds formulas, and never a writer.
    let folders = Folders::build();
    let selection = folders.ticked(&[CALCULS, AGENCIES]);
    for (question, locale) in [
        (
            "Explain the formula of column total in calculs.xlsx",
            "en-US",
        ),
        (
            "Explique la formule de la colonne total de calculs.xlsx",
            "fr-FR",
        ),
    ] {
        let gateway = start_counting_gateway();
        let pending = folders
            .data_only(question, locale, &selection)
            .unwrap_or_else(|| panic!("{question:?}: door 3 answers the yes-or-no"));
        let answer = resolve(pending, question, locale, &gateway.url).await;

        let TabularAnswer::Structural {
            file,
            answer: StructuralAnswer::HasFormulas { has_formulas, .. },
        } = &answer
        else {
            panic!("{question:?}: expected the has-formulas fact, got {answer:?}");
        };
        assert_eq!(file, CALCULS, "{question:?}");
        assert!(*has_formulas, "{question:?}");
        assert_eq!(gateway.calls(), 0, "{question:?}");
    }

    // A writing intent steps door 3 aside, but door 4 does not get past the same reading either.
    let question = "Write a formula that adds base and total in calculs.xlsx";
    assert!(folders.data_only(question, "en-US", &selection).is_none());
    let pending = folders.for_mixed(question, "en-US", &selection);
    let gateway = start_gateway_replying("=B2+C2");
    let (result, generated) = mixed(
        &folders,
        &selection,
        pending,
        question,
        "en-US",
        &gateway.url,
    )
    .await;
    assert!(
        matches!(
            &result.table,
            Some(TabularAnswer::Structural {
                answer: StructuralAnswer::HasFormulas { .. },
                ..
            })
        ),
        "{:?}",
        result.table
    );
    assert!(
        !generated,
        "the yes-or-no is returned and the model is never asked"
    );
    assert_eq!(result.answer, "");
    assert_eq!(gateway.calls(), 0);
}

// --- L: a writing request about a named workbook that is not a mail merge ------------------------

#[tokio::test]
async fn baseline_case_l_a_writing_request_without_a_merge_sign_stays_editorial() {
    // Frozen 20 October 2026. KEPT: no distributive word and no identifier, so door 1 finds no
    // source and the question goes on to the writer. Step 10 builds on this path (rows chosen by the
    // engine), and step 08 must not capture it.
    let folders = Folders::build();
    let selection = folders.ticked(&[FACTURES, AGENCIES]);
    let question = "Write a mail to Alpha about his unpaid invoices in factures.csv";
    assert!(has_writing_intent(question, "en-US"));
    assert!(!has_distributive_word(question, "en-US"));

    assert!(folders.fill_source(question, "en-US", &selection).is_none());
    assert!(folders.data_only(question, "en-US", &selection).is_none());
    let (fallback, _) = left_to_the_model(folders.for_mixed(question, "en-US", &selection));
    assert!(is_unrecognised_nudge(&fallback), "{fallback:?}");

    let gateway = start_gateway_replying("Dear Alpha, here is the mail.");
    let pending = folders.for_mixed(question, "en-US", &selection);
    let (result, generated) = mixed(
        &folders,
        &selection,
        pending,
        question,
        "en-US",
        &gateway.url,
    )
    .await;
    assert!(generated);
    // The named workbook reaches the writer as a card (flipped by step 03); step 10 adds its rows.
    assert_eq!(result.table_unavailable, None);
    assert!(sent_sources(&gateway).contains("Workbook \"factures.csv\""));
    assert!(sent_sources(&gateway).ends_with("Table results: none."));
}

// --- M: tables only, an unrecognised question: the nudge after one wasted call -------------------

#[tokio::test]
async fn baseline_case_m_a_tables_only_question_without_an_operation_ends_in_the_nudge() {
    // Frozen 20 October 2026. Changes in step 05 for advisory questions. The hidden interpreter is
    // asked once (a wait on a real model) and, when it cannot build a plan, the user gets the nudge
    // listing columns, stamped with the attempt.
    let folders = Folders::build();
    let selection = folders.ticked(&[FACTURES]);
    for (question, locale) in [
        ("Summarise factures.csv", "en-US"),
        ("R\u{e9}sume factures.csv", "fr-FR"),
    ] {
        let (fallback, skip_model) =
            left_to_the_model(folders.tables_only(question, locale, &selection));
        assert!(
            is_unrecognised_nudge(&fallback),
            "{question:?}: {fallback:?}"
        );
        assert!(!skip_model, "{question:?}");

        let gateway = start_gateway_replying(r#"{"unsupported": true}"#);
        let pending = folders.tables_only(question, locale, &selection);
        let answer = resolve(pending, question, locale, &gateway.url).await;

        let TabularAnswer::Nudge {
            reason: None,
            available_columns,
            model_attempt: Some(attempt),
            ..
        } = &answer
        else {
            panic!("{question:?}: expected the stamped nudge, got {answer:?}");
        };
        assert_eq!(attempt.model_alias, "cabinet-chat", "{question:?}");
        let _ = available_columns;
        assert_eq!(
            gateway.calls(),
            1,
            "{question:?}: exactly one call, the hidden interpreter"
        );
    }
}

// --- Step 03: the cards the writer receives ---------------------------------------------------------
//
// Added by TABULAR step 03 beside the frozen cases, because they use the same folders and the same
// calls. They run the real card selection (`tabular_answer::question_cards`, which `commands::
// mixed_tier` calls) and then the real writer, one level below the Tauri command.

#[tokio::test]
async fn step_03_two_named_workbooks_both_get_a_card_in_the_order_of_the_question() {
    let folders = Folders::build();
    let selection = folders.ticked(&[AGENCIES, FACTURES, COMMANDES]);
    // Written in the opposite order to the selection on purpose.
    // No formula word: a formula question is still a yes-or-no fact until steps 05 and 07 (case K).
    let question = "Compare the montant of factures.csv with the amount of agencies.csv";
    let pending = folders.for_mixed(question, "en-US", &selection);
    let gateway = start_gateway_replying("=SUMIF(...)");

    let (result, generated) = mixed(
        &folders,
        &selection,
        pending,
        question,
        "en-US",
        &gateway.url,
    )
    .await;

    assert!(generated);
    assert_eq!(result.table_unavailable, None);
    let sources = sent_sources(&gateway);
    let factures = sources
        .find("Workbook \"factures.csv\"")
        .expect("factures card");
    let agencies = sources
        .find("Workbook \"agencies.csv\"")
        .expect("agencies card");
    assert!(
        factures < agencies,
        "question order, not selection order: {sources}"
    );
    assert!(!sources.contains("commandes.csv"), "{sources}");
    assert_eq!(gateway.calls(), 1);
}

#[test]
fn step_03_implied_workbooks_follow_the_rules_of_the_plan() {
    let folders = Folders::build();
    let budget = card_budget_chars(&ContextBudget::DEFAULT);
    let cards = |question: &str, locale: &str, selection: &ScopeMode, candidates: &[String]| {
        tabular_answer::question_cards(
            question,
            &folders.open(),
            selection,
            locale,
            candidates,
            budget,
        )
    };

    // One ticked workbook, nothing named: it is the one the user means.
    let single = folders.ticked(&[FACTURES]);
    let found = cards(
        "What would a lookup formula look like?",
        "en-US",
        &single,
        &[],
    );
    assert_eq!(found.implied.len(), 1);
    assert_eq!(found.implied[0].reason, ImpliedReason::ImpliedSingle);
    assert_eq!(found.block.cards.len(), 1);
    assert_eq!(found.block.cards[0].file, FACTURES);

    // Two ticked and a table word: both are candidates.
    let two = folders.ticked(&[FACTURES, AGENCIES]);
    let found = cards(
        "Which column should I use in this spreadsheet?",
        "en-US",
        &two,
        &[],
    );
    assert!(found
        .implied
        .iter()
        .all(|each| each.reason == ImpliedReason::ImpliedByWord));
    assert_eq!(found.block.cards.len(), 2);

    // Several ticked, no sign at all: no card, as before.
    let many = folders.ticked(&[FACTURES, AGENCIES, COMMANDES, CALCULS]);
    let found = cards(
        "Write a polite reminder for the supplier",
        "en-US",
        &many,
        &[],
    );
    assert!(found.implied.is_empty());
    assert!(found.block.cards.is_empty());
    assert!(found.block.text.is_empty() || found.block.cards.is_empty());

    // Four ticked and a table word: all four are read, none is picked for the user (the owner's test
    // of step 03b, 22 October 2026: with four ticked, "this spreadsheet" used to read none).
    let found = cards(
        "What can you tell me about this spreadsheet?",
        "en-US",
        &many,
        &[],
    );
    assert_eq!(found.implied.len(), 4);
    assert!(found
        .implied
        .iter()
        .all(|each| each.reason == ImpliedReason::ImpliedByWord));
    assert_eq!(found.block.cards.len(), 4);
}

#[test]
fn step_03_a_narrowed_list_never_hides_a_name_and_never_makes_a_workbook_the_only_one() {
    let folders = Folders::build();
    let budget = card_budget_chars(&ContextBudget::DEFAULT);
    let ticked = folders.ticked(&[AGENCIES, FACTURES, COMMANDES]);

    // The knowledge base found the question's entity in commandes.csv alone. The workbooks the user
    // ticked are still three, so this is "implied by entity" and not "the only ticked workbook".
    let entity = vec![COMMANDES.to_string()];
    let found = tabular_answer::question_cards(
        "Anything about CMD-2026-003?",
        &folders.open(),
        &ticked,
        "en-US",
        &entity,
        budget,
    );
    assert_eq!(found.implied.len(), 1);
    assert_eq!(found.implied[0].file, COMMANDES);
    assert_eq!(found.implied[0].reason, ImpliedReason::ImpliedByEntity);

    // The same list says nothing about a workbook the question names.
    let found = tabular_answer::question_cards(
        "What are the columns of agencies.csv?",
        &folders.open(),
        &ticked,
        "en-US",
        &entity,
        budget,
    );
    let files: Vec<&str> = found
        .block
        .cards
        .iter()
        .map(|card| card.file.as_str())
        .collect();
    assert_eq!(files.first().copied(), Some(AGENCIES), "{files:?}");
}

#[test]
fn step_03_a_small_card_budget_shrinks_the_cards_and_never_drops_a_named_one() {
    let folders = Folders::build();
    let selection = folders.ticked(&[FACTURES, AGENCIES, COMMANDES]);
    let question = "Compare factures.csv, agencies.csv and commandes.csv";
    let open = folders.open();
    let roomy = tabular_answer::question_cards(question, &open, &selection, "en-US", &[], 4_000);
    let tight = tabular_answer::question_cards(question, &open, &selection, "en-US", &[], 300);

    assert_eq!(roomy.block.cards.len(), 3);
    assert!(!roomy.block.truncated);
    assert_eq!(
        tight.block.cards.len(),
        3,
        "a named workbook is shrunk, not dropped"
    );
    assert!(tight.block.truncated);
    assert!(tight.block.text.chars().count() < roomy.block.text.chars().count());
    for card in &tight.block.cards {
        assert!(tight
            .block
            .text
            .contains(&format!("Workbook \"{}\"", card.file)));
    }
}

#[tokio::test]
async fn step_03_cards_without_documents_still_reach_the_model() {
    let folders = Folders::build();
    let selection = folders.ticked(&[FACTURES, AGENCIES]);
    let question = "Draft a short note about factures.csv";
    let pending = folders.for_mixed(question, "en-US", &selection);
    let gateway = start_gateway_replying("A short note.");

    let (result, generated) = mixed_from(
        &folders,
        &selection,
        pending,
        question,
        "en-US",
        &gateway.url,
        Vec::new(),
        Some(MixedPartUnavailable::NoEvidence),
    )
    .await;

    assert!(generated, "documents are empty, the cards are not");
    assert_eq!(gateway.calls(), 1);
    assert_eq!(result.answer, "A short note.");
    assert_eq!(
        result.documents_unavailable,
        Some(MixedPartUnavailable::NoEvidence)
    );
    assert_eq!(result.table_unavailable, None);
    let sources = sent_sources(&gateway);
    assert!(sources.contains("Document excerpts: none."));
    assert!(sources.contains("D  montant"), "{sources}");
}

#[tokio::test]
async fn step_03_no_card_and_no_documents_is_still_insufficient_evidence() {
    let folders = Folders::build();
    // Four ticked, no name, no table word: no card. Documents unavailable: nothing to answer from.
    let selection = folders.ticked(&[FACTURES, AGENCIES, COMMANDES, CALCULS]);
    let question = "Write a polite reminder for the supplier";
    let pending = folders.for_mixed(question, "en-US", &selection);
    let gateway = start_counting_gateway();
    let cards =
        tabular_answer::question_cards(question, &folders.open(), &selection, "en-US", &[], 4_000);
    assert!(cards.block.cards.is_empty());
    let client = GatewayClient::new().expect("builds a client");
    let context = MixedContext {
        locale: "en-US",
        gateway: &client,
        server_url: &gateway.url,
        model_alias: "cabinet-chat",
        idle_timeout: IDLE_TIMEOUT,
        history: &[],
        budget: ContextBudget::DEFAULT,
        cards_text: &cards.block.text,
        sample_text: "",
    };
    let outcome = mixed_answer::answer(
        question,
        pending,
        Vec::new(),
        Some(MixedPartUnavailable::NoEvidence),
        &context,
        |_| {},
        |_| {},
    )
    .await;
    assert!(outcome.is_err());
    assert_eq!(gateway.calls(), 0);
}

#[tokio::test]
async fn step_03_a_figure_the_card_states_is_not_reported_as_unverified() {
    let folders = Folders::build();
    let selection = folders.ticked(&[AGENCIES, FACTURES]);
    let question = "Summarise agencies.csv";
    // agencies.csv has 2 400 data rows, which its card says.
    let supported = start_gateway_replying("The workbook holds 2400 rows over three columns.");
    let pending = folders.for_mixed(question, "en-US", &selection);
    let (result, _) = mixed(
        &folders,
        &selection,
        pending,
        question,
        "en-US",
        &supported.url,
    )
    .await;
    assert!(
        result.unverified_numbers.is_empty(),
        "{:?}",
        result.unverified_numbers
    );

    let invented = start_gateway_replying("The workbook holds 7777 rows over three columns.");
    let pending = folders.for_mixed(question, "en-US", &selection);
    let (result, _) = mixed(
        &folders,
        &selection,
        pending,
        question,
        "en-US",
        &invented.url,
    )
    .await;
    assert_eq!(result.unverified_numbers, vec!["7777".to_string()]);
}
