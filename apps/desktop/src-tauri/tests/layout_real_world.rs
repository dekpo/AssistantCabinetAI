//! TABULAR step 13: what a spreadsheet application shows and the grid of cells does not say.
//!
//! Each workbook is written at test time by `tests/common/tabular_fixtures.rs` (a real `.xlsx`
//! with hidden rows, merged cells, a filter, a table, a hidden sheet), read through the same
//! functions a question uses (`tabular::build_inventory_with_source_rows`, then the raw grid and
//! the stored layout facts, then `tabular::view_of`), and asked through `engine::execute`.
//!
//! Every expected number is derived by hand below, never by running the code under test.

mod common;

use assistant_cabinet_ai_lib::analysis_scope::{ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::FileHashCache;
use assistant_cabinet_ai_lib::tabular::engine::{execute, Operation, TabularOutcome, TabularValue};
use assistant_cabinet_ai_lib::tabular::layout::{RowPolicy, SheetState};
use assistant_cabinet_ai_lib::tabular::{self, inventory::TabularInventory, Workbook};
use assistant_cabinet_ai_lib::tabular_answer::{self, ModelAssist};
use assistant_cabinet_ai_lib::workbook_use::{self, WorkbookUse, WorkbookUseHow};
use common::counting_gateway::start_counting_gateway;
use common::tabular_fixtures::{write_laid_out_xlsx, Cell, LaidOutSheet};

const LOCALE: &str = "fr-FR";

fn t(value: &str) -> Cell {
    Cell::text(value)
}

fn n(value: f64) -> Cell {
    Cell::Number(value)
}

/// The workbook as a question reads it: analysed once, then viewed from the raw grid and the
/// facts the inventory stored (the path of a typed-cache hit).
fn viewed(path: &std::path::Path, policy: RowPolicy) -> (Workbook, TabularInventory) {
    let (analysed, _, _) =
        tabular::build_inventory_with_source_rows(path, "t.xlsx", LOCALE).expect("analyses");
    let raw = tabular::load_current_raw(path, &analysed.workbook_id).expect("reads");
    let (workbook, inventory, _) = tabular::view_of(
        &raw.workbook,
        "t.xlsx",
        &analysed.workbook_id,
        analysed.format,
        &analysed.facts,
        None,
        policy,
        LOCALE,
    );
    (workbook, inventory)
}

fn run(
    book: &(Workbook, TabularInventory),
    sheet: Option<&str>,
    operation: Operation,
) -> TabularOutcome {
    execute(&book.0, &book.1, sheet, None, &operation, LOCALE)
}

fn count_of(outcome: TabularOutcome) -> usize {
    match outcome {
        TabularOutcome::Value {
            value: TabularValue::Count(count),
            ..
        } => count,
        other => panic!("expected a count, got {other:?}"),
    }
}

fn sum_of(outcome: TabularOutcome) -> f64 {
    match outcome {
        TabularOutcome::Value {
            value: TabularValue::Sum(aggregate),
            ..
        } => aggregate.value,
        other => panic!("expected a sum, got {other:?}"),
    }
}

fn sum(column: &str) -> Operation {
    Operation::Sum {
        column: column.to_string(),
        filters: Vec::new(),
    }
}

fn count() -> Operation {
    Operation::Count {
        filters: Vec::new(),
    }
}

/// Rows 2 to 6 of "Data": a Frais 10, b (Frais by the merge B2:B3) 20, c Charges 1 000 (hidden by
/// the filter), d Charges 5, e Frais 1. Sheet "Calculs" is hidden.
fn journal() -> LaidOutSheet {
    LaidOutSheet {
        name: "Data".to_string(),
        state: "visible".to_string(),
        rows: vec![
            vec![t("lib"), t("cat"), t("montant")],
            vec![t("a"), t("Frais"), n(10.0)],
            vec![t("b"), n_empty(), n(20.0)],
            vec![t("c"), t("Charges"), n(1000.0)],
            vec![t("d"), t("Charges"), n(5.0)],
            vec![t("e"), t("Frais"), n(1.0)],
        ],
        hidden_rows: vec![4],
        merges: vec!["B2:B3".to_string()],
        filter: Some(("A1:C6".to_string(), true)),
        ..LaidOutSheet::default()
    }
}

/// A cell with no content: an empty text is read as empty by the adapter.
fn n_empty() -> Cell {
    Cell::Text(String::new())
}

fn calculs() -> LaidOutSheet {
    LaidOutSheet {
        name: "Calculs".to_string(),
        state: "hidden".to_string(),
        rows: vec![vec![t("k"), t("v")], vec![t("z"), n(7.0)]],
        ..LaidOutSheet::default()
    }
}

#[test]
fn a_figure_counts_a_hidden_row_by_default_and_leaves_it_out_when_asked() {
    // Visible rows: 10 + 20 + 5 + 1 = 36 (4 rows). With the hidden row of 1 000: 1 036 (5 rows).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    write_laid_out_xlsx(&path, &[journal(), calculs()]);

    let including = viewed(&path, RowPolicy::Include);
    assert_eq!(count_of(run(&including, Some("Data"), count())), 5);
    assert_eq!(
        sum_of(run(&including, Some("Data"), sum("montant"))),
        1036.0
    );

    let excluding = viewed(&path, RowPolicy::Exclude);
    assert_eq!(count_of(run(&excluding, Some("Data"), count())), 4);
    assert_eq!(sum_of(run(&excluding, Some("Data"), sum("montant"))), 36.0);
}

#[test]
fn the_inventory_counts_the_hidden_rows_and_says_a_filter_is_active() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    write_laid_out_xlsx(&path, &[journal(), calculs()]);

    let (inventory, _, _) =
        tabular::build_inventory_with_source_rows(&path, "t.xlsx", LOCALE).unwrap();
    let data = inventory.sheets.iter().find(|s| s.name == "Data").unwrap();
    let layout = data.layout.as_ref().expect("layout read");
    assert_eq!(layout.hidden_rows, 1);
    assert_eq!(layout.hidden_row_numbers, vec![4]);
    assert!(layout.filter_active);
    assert_eq!(layout.merged_filled, 1);
    assert_eq!(layout.header_source_row, Some(1));
    assert_eq!(inventory.layout_version, 1);

    let excluded = viewed(&path, RowPolicy::Exclude);
    let layout = excluded.1.sheets[0].layout.as_ref().unwrap();
    assert_eq!(layout.hidden_excluded, 1);
}

#[test]
fn a_vertical_merge_gives_its_value_to_every_row_it_covers() {
    // Frais: 10 + 20 (the covered row) + 1 = 31. Charges: 1 000 + 5 = 1 005, or 5 without the hidden row.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    write_laid_out_xlsx(&path, &[journal()]);
    let group = || Operation::GroupSum {
        group_by: "cat".to_string(),
        sum_column: "montant".to_string(),
        filters: Vec::new(),
    };
    let groups_of = |outcome: TabularOutcome| match outcome {
        TabularOutcome::Value {
            value: TabularValue::GroupSums(groups),
            ..
        } => groups
            .into_iter()
            .map(|g| (g.group, g.sum))
            .collect::<Vec<_>>(),
        other => panic!("expected groups, got {other:?}"),
    };

    let including = viewed(&path, RowPolicy::Include);
    assert_eq!(
        groups_of(run(&including, None, group())),
        vec![("Charges".to_string(), 1005.0), ("Frais".to_string(), 31.0)]
    );
    let excluding = viewed(&path, RowPolicy::Exclude);
    assert_eq!(
        groups_of(run(&excluding, None, group())),
        vec![("Charges".to_string(), 5.0), ("Frais".to_string(), 31.0)]
    );
}

#[test]
fn a_hidden_sheet_is_not_offered_and_is_reached_by_naming_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    write_laid_out_xlsx(&path, &[journal(), calculs()]);
    let book = viewed(&path, RowPolicy::Include);

    // No sheet named: the only visible sheet is the one asked about, no "which sheet?".
    assert_eq!(count_of(run(&book, None, count())), 5);
    // Named: the hidden sheet answers (1 data row).
    assert_eq!(count_of(run(&book, Some("Calculs"), count())), 1);
    let hidden = book.1.sheets.iter().find(|s| s.name == "Calculs").unwrap();
    assert_eq!(hidden.layout.as_ref().unwrap().state, SheetState::Hidden);
    assert!(!hidden.looks_tabular());
}

#[test]
fn a_total_row_is_left_out_of_sums_and_counts_and_does_not_spoil_the_dates() {
    // 10 + 25 = 35 over two data rows; the total row (35) must not be added again (it would give 70).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    let sheet = LaidOutSheet::plain(
        "Data",
        vec![
            vec![t("date"), t("lib"), t("montant")],
            vec![Cell::Date(46055), t("a"), n(10.0)],
            vec![Cell::Date(46056), t("b"), n(25.0)],
            vec![
                t("Total"),
                t(""),
                Cell::Formula {
                    expression: "SUM(C2:C3)".to_string(),
                    cached: 35.0,
                },
            ],
        ],
    );
    write_laid_out_xlsx(&path, &[sheet]);

    let book = viewed(&path, RowPolicy::Include);
    assert_eq!(count_of(run(&book, None, count())), 2);
    assert_eq!(sum_of(run(&book, None, sum("montant"))), 35.0);
    let sheet = &book.1.sheets[0];
    assert_eq!(sheet.layout.as_ref().unwrap().total_rows, 1);
    assert_eq!(sheet.layout.as_ref().unwrap().total_row_numbers, vec![4]);
    assert!(
        !sheet.columns[2].has_formulas,
        "the total was the only formula"
    );
    assert_eq!(
        sheet.columns[0].inferred_type.as_code(),
        "date",
        "the word Total no longer makes the date column text"
    );
    assert!(sheet.looks_tabular());
}

#[test]
fn two_tables_on_one_sheet_are_two_sheets_and_a_question_that_names_none_is_asked_which() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    let sheet = LaidOutSheet::plain(
        "Data",
        vec![
            vec![t("date"), t("lib"), t("montant")],
            vec![t("2026-03-01"), t("a"), n(10.0)],
            vec![t("2026-03-02"), t("b"), n(20.0)],
            vec![],
            vec![t("fournisseur"), t("statut"), t("total")],
            vec![t("X"), t("paye"), n(100.0)],
            vec![t("Y"), t("du"), n(200.0)],
        ],
    );
    write_laid_out_xlsx(&path, &[sheet]);
    let book = viewed(&path, RowPolicy::Include);

    let names: Vec<&str> = book.1.sheets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["Data A1:C3", "Data A5:C7"]);
    match run(&book, None, count()) {
        TabularOutcome::NotDeterministicallyAnswerable {
            available_sheets, ..
        } => assert_eq!(available_sheets, vec!["Data A1:C3", "Data A5:C7"]),
        other => panic!("expected the question which table, got {other:?}"),
    }
    assert_eq!(sum_of(run(&book, Some("Data A1:C3"), sum("montant"))), 30.0);
    assert_eq!(sum_of(run(&book, Some("Data A5:C7"), sum("total"))), 300.0);
}

#[test]
fn an_excel_table_is_the_region_whatever_surrounds_it() {
    // The table is B2:D4 (header B2); the title above and the note below are not data; the totals
    // row of the table (row 5) is left out.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    let sheet = LaidOutSheet {
        name: "Data".to_string(),
        state: "visible".to_string(),
        rows: vec![
            vec![t("Quarterly export")],
            vec![t(""), t("lib"), t("cat"), t("montant")],
            vec![t(""), t("a"), t("x"), n(1.0)],
            vec![t(""), t("b"), t("y"), n(2.0)],
            vec![t(""), t("Total"), t(""), n(3.0)],
            vec![t("Source: the accountant")],
        ],
        table: Some(("Factures".to_string(), "B2:D5".to_string(), true)),
        ..LaidOutSheet::default()
    };
    write_laid_out_xlsx(&path, &[sheet]);
    let book = viewed(&path, RowPolicy::Include);

    assert_eq!(count_of(run(&book, None, count())), 2);
    assert_eq!(sum_of(run(&book, None, sum("montant"))), 3.0);
    let layout = book.1.sheets[0].layout.as_ref().unwrap();
    assert_eq!(layout.excel_table.as_deref(), Some("Factures"));
    assert_eq!(layout.range.as_deref(), Some("B2:D4"));
}

#[test]
fn a_blank_row_inside_a_table_and_a_note_after_it_are_not_data() {
    // Rows: 1 + 2 + (blank) + 3 + 4 = 10 over 4 data rows; the note under the table is not a row.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    let sheet = LaidOutSheet::plain(
        "Data",
        vec![
            vec![t("lib"), t("cat"), t("montant")],
            vec![t("a"), t("x"), n(1.0)],
            vec![t("b"), t("y"), n(2.0)],
            vec![],
            vec![t("c"), t("x"), n(3.0)],
            vec![t("d"), t("y"), n(4.0)],
            vec![],
            vec![t("Source: the accountant")],
        ],
    );
    write_laid_out_xlsx(&path, &[sheet]);
    let book = viewed(&path, RowPolicy::Include);

    assert_eq!(count_of(run(&book, None, count())), 4);
    assert_eq!(sum_of(run(&book, None, sum("montant"))), 10.0);
}

#[test]
fn a_plain_workbook_is_read_exactly_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    let sheet = LaidOutSheet::plain(
        "Data",
        vec![
            vec![t("lib"), t("montant")],
            vec![t("a"), n(10.0)],
            vec![t("b"), n(20.0)],
        ],
    );
    write_laid_out_xlsx(&path, &[sheet]);
    for policy in [RowPolicy::Include, RowPolicy::Exclude] {
        let book = viewed(&path, policy);
        assert_eq!(count_of(run(&book, None, count())), 2);
        assert_eq!(sum_of(run(&book, None, sum("montant"))), 30.0);
        assert!(book.1.sheets[0].layout.as_ref().unwrap().is_plain());
    }
}

#[test]
fn an_inventory_analysed_before_the_layout_existed_is_read_with_no_fact() {
    let old = r#"{"workbookId":"id","relativePath":"a.csv","format":"csv","sheets":[]}"#;
    let inventory: TabularInventory = serde_json::from_str(old).expect("an old inventory loads");
    assert_eq!(inventory.layout_version, 0);
    assert!(inventory.facts.is_empty());
}

#[test]
fn header_only_csv_is_an_empty_table_not_one_row_of_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.csv");
    std::fs::write(&path, "nom;prenom;ville\n").unwrap();
    let (inventory, workbook, _) =
        tabular::build_inventory_with_source_rows(&path, "t.csv", LOCALE).unwrap();
    assert_eq!(inventory.sheets[0].header_row, Some(0));
    assert_eq!(inventory.sheets[0].row_count, 0);
    assert_eq!(inventory.sheets[0].columns[0].name, "nom");
    assert_eq!(
        count_of(execute(&workbook, &inventory, None, None, &count(), LOCALE)),
        0
    );
}

#[test]
fn spellings_that_differ_by_space_or_case_are_one_group() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.csv");
    std::fs::write(
        &path,
        "cat;montant\nFrais;10\n Frais;20\nFrais ;30\nfrais;40\nCharges;5\n",
    )
    .unwrap();
    let (inventory, workbook, _) =
        tabular::build_inventory_with_source_rows(&path, "t.csv", LOCALE).unwrap();
    let groups = execute(
        &workbook,
        &inventory,
        None,
        None,
        &Operation::GroupSum {
            group_by: "cat".to_string(),
            sum_column: "montant".to_string(),
            filters: Vec::new(),
        },
        LOCALE,
    );
    match groups {
        TabularOutcome::Value {
            value: TabularValue::GroupSums(groups),
            ..
        } => {
            let pairs: Vec<(String, f64)> = groups.into_iter().map(|g| (g.group, g.sum)).collect();
            // Frais, " Frais", "Frais ", frais = 10 + 20 + 30 + 40 = 100; shown as the commonest
            // spelling once trimmed (`Frais`, twice).
            assert_eq!(
                pairs,
                vec![("Charges".to_string(), 5.0), ("Frais".to_string(), 100.0)]
            );
        }
        other => panic!("expected groups, got {other:?}"),
    }
}

#[test]
fn a_placeholder_in_a_numeric_column_does_not_make_the_column_text() {
    // 1 234,50 + 20,00 + (15,00) = 1 239,50; "n/a" and "-" are empty, not unreadable numbers.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.csv");
    std::fs::write(
        &path,
        "lib;montant\na;1 234,50\nb; 20,00\nc;n/a\nd;-\ne;(15,00)\n",
    )
    .unwrap();
    let (inventory, workbook, _) =
        tabular::build_inventory_with_source_rows(&path, "t.csv", LOCALE).unwrap();
    assert_eq!(
        inventory.sheets[0].columns[1].inferred_type.as_code(),
        "numeric"
    );
    assert_eq!(inventory.sheets[0].columns[1].unparsed_count, 0);
    let total = sum_of(execute(
        &workbook,
        &inventory,
        None,
        None,
        &sum("montant"),
        LOCALE,
    ));
    assert!((total - (1234.50 + 20.0 - 15.0)).abs() < 1e-9, "{total}");
}

#[test]
fn a_notes_sheet_beside_the_table_does_not_make_the_question_ambiguous() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    let table = LaidOutSheet::plain(
        "Journal",
        vec![
            vec![t("lib"), t("montant")],
            vec![t("a"), n(10.0)],
            vec![t("b"), n(20.0)],
        ],
    );
    let notes = LaidOutSheet::plain(
        "Notes",
        vec![vec![t("Notes")], vec![t("A line")], vec![t("Another")]],
    );
    write_laid_out_xlsx(&path, &[table, notes]);
    let book = viewed(&path, RowPolicy::Include);

    assert_eq!(sum_of(run(&book, None, sum("montant"))), 30.0);
    // The structure still lists both sheets: only the default of an operation changes.
    assert_eq!(book.1.sheets.len(), 2);
}

#[test]
fn rows_sent_to_the_writer_keep_the_numbers_the_spreadsheet_shows() {
    // Rows 2, 3, 5 and 6 remain once row 4 is hidden and left out: the third data row is row 5.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    write_laid_out_xlsx(&path, &[journal()]);
    let excluded = viewed(&path, RowPolicy::Exclude);
    let sheet = &excluded.1.sheets[0];
    let numbers: Vec<usize> = (0..4).map(|index| sheet.data_row_number(index)).collect();
    assert_eq!(numbers, vec![2, 3, 5, 6]);
    let included = viewed(&path, RowPolicy::Include);
    let numbers: Vec<usize> = (0..5)
        .map(|index| included.1.sheets[0].data_row_number(index))
        .collect();
    assert_eq!(numbers, vec![2, 3, 4, 5, 6]);
}

#[test]
fn the_explorer_caveats_name_what_the_reader_should_know() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    write_laid_out_xlsx(&path, &[journal(), calculs()]);
    let (inventory, _, _) =
        tabular::build_inventory_with_source_rows(&path, "t.xlsx", LOCALE).unwrap();
    assert_eq!(
        inventory.caveats("xlsx"),
        vec!["filtered_rows", "merged_cells", "hidden_sheets"]
    );
    assert!(inventory.caveats("xls").contains(&"xls"));

    // An inventory analysed before the layout was read asks for a new analysis, by hand.
    let mut old = inventory.clone();
    old.layout_version = 0;
    assert_eq!(old.caveats("xlsx")[0], "layout_unread");
}

/// The question asked as the application asks it: the folder analysed, the workbook ticked, the
/// answer computed under `policy`, then the trace line the layout adds to it.
async fn asked(
    question: &str,
    policy: RowPolicy,
) -> (tabular_answer::TabularAnswer, workbook_use::LayoutNote) {
    let data = tempfile::tempdir().unwrap();
    let app = tempfile::tempdir().unwrap();
    write_laid_out_xlsx(&data.path().join("journal.xlsx"), &[journal(), calculs()]);
    let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
    data_folder::analyse(data.path(), &mut index, LOCALE, &|_| {}).unwrap();
    let folder = DataFolder::discover(data.path(), Some(&index), &FileHashCache::new())
        .unwrap()
        .with_row_policy(policy);
    let record = folder
        .files()
        .find_by_relative_path("journal.xlsx")
        .unwrap();
    let selection = ScopeMode::Explicit(vec![ScopeEntry {
        relative_path: "journal.xlsx".to_string(),
        pinned_id: record.id.clone(),
        added_at: 1,
        sheet_names: Vec::new(),
    }]);
    let gateway = GatewayClient::new().unwrap();
    let assist = ModelAssist {
        gateway: &gateway,
        server_url: "http://127.0.0.1:9",
        model_alias: "cabinet-chat",
        idle_timeout: std::time::Duration::from_millis(200),
    };
    let answer = tabular_answer::answer(question, &folder, &selection, LOCALE, &index, &assist)
        .await
        .expect("answers");
    let mut uses = vec![WorkbookUse {
        file: "journal.xlsx".to_string(),
        how: WorkbookUseHow::Computed,
        reason: None,
        rows_sent: None,
        rows_total: None,
        rows_truncated: false,
        truncated: false,
        count: None,
        candidates: Vec::new(),
        layout: None,
    }];
    workbook_use::note_layout(&mut uses, &folder, Some(&answer));
    (
        answer,
        uses.remove(0).layout.expect("the sheet is not plain"),
    )
}

#[tokio::test]
async fn the_question_path_counts_the_hidden_rows_and_the_trace_says_so() {
    let (answer, note) = asked("Quel est le total des montants ?", RowPolicy::Include).await;
    match answer {
        tabular_answer::TabularAnswer::Value {
            value: TabularValue::Sum(aggregate),
            ..
        } => assert_eq!(aggregate.value, 1036.0),
        other => panic!("expected a sum, got {other:?}"),
    }
    assert_eq!((note.filtered_rows, note.hidden_rows), (1, 0), "{note:?}");
    assert!(!note.rows_left_out);
    // This sheet has no total row; one merge is filled; the workbook hides one sheet.
    assert_eq!(
        (note.total_rows, note.merged_filled, note.hidden_sheets),
        (0, 1, 1)
    );

    let (answer, note) = asked("Quel est le total des montants ?", RowPolicy::Exclude).await;
    match answer {
        tabular_answer::TabularAnswer::Value {
            value: TabularValue::Sum(aggregate),
            ..
        } => assert_eq!(aggregate.value, 36.0),
        other => panic!("expected a sum, got {other:?}"),
    }
    assert!(note.rows_left_out);
}

#[tokio::test]
async fn a_row_count_follows_the_policy_and_is_traced_too() {
    let (answer, note) = asked("Combien de lignes ?", RowPolicy::Exclude).await;
    match answer {
        tabular_answer::TabularAnswer::Structural {
            answer:
                assistant_cabinet_ai_lib::tabular::structural::StructuralAnswer::RowCount {
                    rows, ..
                },
            ..
        } => assert_eq!(rows, 4),
        other => panic!("expected a row count, got {other:?}"),
    }
    assert!(note.rows_left_out);
    let (_, note) = asked("Combien de lignes ?", RowPolicy::Include).await;
    assert!(!note.rows_left_out);
}

#[tokio::test]
async fn a_question_that_names_none_of_two_tables_is_asked_never_guessed_by_the_model() {
    // Found by the owner's run of the human test (24 October 2026): with the AI reachable the
    // hidden interpreter chose `Suivi A1:C3` for her and answered 30. Which table is hers to say.
    let data = tempfile::tempdir().unwrap();
    let app = tempfile::tempdir().unwrap();
    let sheet = LaidOutSheet::plain(
        "Suivi",
        vec![
            vec![t("date"), t("libelle"), t("montant")],
            vec![t("2026-03-01"), t("Loyer"), n(10.0)],
            vec![t("2026-03-02"), t("Eau"), n(20.0)],
            vec![],
            vec![t("fournisseur"), t("statut"), t("reste")],
            vec![t("Alpha"), t("paye"), n(100.0)],
            vec![t("Beta"), t("du"), n(200.0)],
        ],
    );
    write_laid_out_xlsx(&data.path().join("deux.xlsx"), &[sheet]);
    let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
    data_folder::analyse(data.path(), &mut index, LOCALE, &|_| {}).unwrap();
    let folder = DataFolder::discover(data.path(), Some(&index), &FileHashCache::new()).unwrap();
    let record = folder.files().find_by_relative_path("deux.xlsx").unwrap();
    let selection = ScopeMode::Explicit(vec![ScopeEntry {
        relative_path: "deux.xlsx".to_string(),
        pinned_id: record.id.clone(),
        added_at: 1,
        sheet_names: Vec::new(),
    }]);
    let gateway = GatewayClient::new().unwrap();
    let fake = start_counting_gateway();
    let assist = ModelAssist {
        gateway: &gateway,
        server_url: &fake.url,
        model_alias: "cabinet-chat",
        idle_timeout: std::time::Duration::from_millis(500),
    };

    let answer = tabular_answer::answer(
        "Quel est le total des montants ?",
        &folder,
        &selection,
        LOCALE,
        &index,
        &assist,
    )
    .await
    .expect("answers");

    match answer {
        tabular_answer::TabularAnswer::Nudge {
            available_sheets, ..
        } => assert_eq!(available_sheets, vec!["Suivi A1:C3", "Suivi A5:C7"]),
        other => panic!("expected the question which table, got {other:?}"),
    }
    assert_eq!(fake.calls(), 0, "the model was asked to pick a table");
}

#[test]
fn a_hidden_sheet_the_answer_was_read_from_is_not_reported_as_unread() {
    // The answer named `Calculs` and read it: the line must not say it is not read.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.xlsx");
    write_laid_out_xlsx(&path, &[journal(), calculs()]);
    let (inventory, _, _) =
        tabular::build_inventory_with_source_rows(&path, "t.xlsx", LOCALE).unwrap();
    assert_eq!(
        workbook_use::hidden_sheets_unread(&inventory, Some("Calculs")),
        0
    );
    assert_eq!(
        workbook_use::hidden_sheets_unread(&inventory, Some("Data")),
        1
    );
    assert_eq!(workbook_use::hidden_sheets_unread(&inventory, None), 1);
}
