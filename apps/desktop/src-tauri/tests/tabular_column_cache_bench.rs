//! Measures the cost `docs/SESSION-DATA-06-Findings.md` section 10 first put a number on, before
//! any cache is built for it (`docs/SESSION-DATA-13-Column-Cache.md`): what one question actually
//! costs today, when it re-reads and re-hashes the whole workbook (`tabular::load_current`,
//! called from `tabular_answer::answer`'s `Operation` route).
//!
//! `#[ignore]`: this writes a 100,000-row CSV and a 50,000-row XLSX to a temp directory and is
//! slow by design. Run explicitly:
//!
//! ```text
//! cargo test --release --test tabular_column_cache_bench -- --ignored --nocapture
//! ```
//!
//! The release profile matters - these are the numbers a shipped build sees, not a debug one.
//! Run it on the machine whose number decides anything: the development PC first, then the
//! pilot's own 2019 practice PC before deciding whether a cache is worth building at all.

mod common;

use std::time::{Duration, Instant};

use assistant_cabinet_ai_lib::tabular::engine::{self, FilterSpec, Operation, TabularOutcome};
use assistant_cabinet_ai_lib::tabular::{self, TabularError};
use common::tabular_fixtures::{write_xlsx, Cell};

const LOCALE: &str = "en-US";
const CSV_ROWS: usize = 100_000;
const XLSX_ROWS: usize = 50_000;
const REPEATED_QUESTIONS: usize = 10;

const REGIONS: [&str; 4] = ["North", "South", "East", "West"];
const CATEGORIES: [&str; 5] = ["Office", "Transport", "Energy", "Software", "Other"];

/// `id,date,category,region,amount,quantity,unit_price,notes` - 8 columns, a mix of numeric,
/// date and text so building the inventory does the same classification work a real export
/// would, not just a single uniform column.
fn large_csv(rows: usize) -> String {
    let mut text = String::from("id,date,category,region,amount,quantity,unit_price,notes\n");
    for i in 0..rows {
        let day = i % 28 + 1;
        let month = i % 12 + 1;
        let year = 2020 + i % 6;
        let category = CATEGORIES[i % CATEGORIES.len()];
        let region = REGIONS[i % REGIONS.len()];
        let quantity = i % 50 + 1;
        let unit_price = 1.0 + (i % 997) as f64 / 100.0;
        let amount = quantity as f64 * unit_price;
        text.push_str(&format!(
            "{i},{day:02}/{month:02}/{year},{category},{region},{amount:.2},{quantity},{unit_price:.2},note-{i}\n"
        ));
    }
    text
}

fn large_xlsx_rows(rows: usize) -> Vec<Vec<Cell>> {
    let mut data = vec![vec![
        Cell::text("id"),
        Cell::text("date"),
        Cell::text("category"),
        Cell::text("region"),
        Cell::text("amount"),
        Cell::text("quantity"),
        Cell::text("unit_price"),
        Cell::text("notes"),
    ]];
    // 01/01/2020 is Excel serial 43831; stepping by one calendar day per row keeps a real
    // `Date`-typed column without the fixture having to reimplement a calendar.
    const FIRST_DAY_2020: u32 = 43831;
    for i in 0..rows {
        let category = CATEGORIES[i % CATEGORIES.len()];
        let region = REGIONS[i % REGIONS.len()];
        let quantity = (i % 50 + 1) as f64;
        let unit_price = 1.0 + (i % 997) as f64 / 100.0;
        let amount = quantity * unit_price;
        data.push(vec![
            Cell::Number(i as f64),
            Cell::Date(FIRST_DAY_2020 + (i as u32 % 2000)),
            Cell::text(category),
            Cell::text(region),
            Cell::Number(amount),
            Cell::Number(quantity),
            Cell::Number(unit_price),
            Cell::text(&format!("note-{i}")),
        ]);
    }
    data
}

struct Timing {
    first_analysis: Duration,
    one_question: Duration,
    ten_questions: Duration,
}

/// `first_analysis` is `tabular::build_inventory` alone, the per-file cost of an Analyse pass
/// (`data_folder::analyse`). `one_question` and `ten_questions` are `tabular::load_current` plus
/// `engine::execute`, the exact pair `tabular_answer::answer`'s `Operation` route runs on every
/// question today - the cost gap F names.
fn measure(path: &std::path::Path, relative_path: &str, sum_column: &str) -> Timing {
    let analysis_start = Instant::now();
    let inventory =
        tabular::build_inventory(path, relative_path, LOCALE).expect("the fixture parses");
    let first_analysis = analysis_start.elapsed();
    assert!(
        inventory.has_a_usable_sheet(),
        "the fixture must be a usable table, or the measurement is meaningless"
    );
    let workbook_id = inventory.workbook_id.clone();

    let operation = Operation::Sum {
        column: sum_column.to_string(),
        filters: Vec::<FilterSpec>::new(),
    };

    let ask_once = || -> Duration {
        let start = Instant::now();
        let (workbook, fresh) =
            match tabular::load_current(path, relative_path, &workbook_id, LOCALE) {
                Ok(pair) => pair,
                Err(TabularError::WorkbookChanged) => {
                    panic!("the fixture must not change under its own benchmark")
                }
                Err(error) => panic!("load_current failed: {error:?}"),
            };
        let outcome = engine::execute(&workbook, &fresh, None, None, &operation, LOCALE);
        let elapsed = start.elapsed();
        match outcome {
            TabularOutcome::Value { .. } => {}
            TabularOutcome::NotDeterministicallyAnswerable { reason, .. } => {
                panic!("expected a computed sum, got a refusal: {reason:?}")
            }
        }
        elapsed
    };

    let one_question = ask_once();
    let mut ten_questions = Duration::ZERO;
    for _ in 0..REPEATED_QUESTIONS {
        ten_questions += ask_once();
    }

    Timing {
        first_analysis,
        one_question,
        ten_questions,
    }
}

fn report(label: &str, bytes: u64, rows: usize, timing: &Timing) {
    println!("--- {label}: {rows} rows, {bytes} bytes ---");
    println!(
        "  first analysis (build_inventory):      {:?}",
        timing.first_analysis
    );
    println!(
        "  one question (load_current + execute):  {:?}",
        timing.one_question
    );
    println!(
        "  ten questions, total:                   {:?} ({:?} average)",
        timing.ten_questions,
        timing.ten_questions / REPEATED_QUESTIONS as u32
    );
}

#[test]
#[ignore]
fn measures_the_cost_of_rereading_large_workbooks() {
    let dir = tempfile::tempdir().unwrap();

    let csv_path = dir.path().join("large.csv");
    std::fs::write(&csv_path, large_csv(CSV_ROWS)).unwrap();
    let csv_bytes = std::fs::metadata(&csv_path).unwrap().len();
    let csv_timing = measure(&csv_path, "large.csv", "amount");
    report("CSV", csv_bytes, CSV_ROWS, &csv_timing);

    let xlsx_path = dir.path().join("large.xlsx");
    write_xlsx(
        &xlsx_path,
        &[("Feuille1".to_string(), large_xlsx_rows(XLSX_ROWS))],
    );
    let xlsx_bytes = std::fs::metadata(&xlsx_path).unwrap().len();
    let xlsx_timing = measure(&xlsx_path, "large.xlsx", "amount");
    report("XLSX", xlsx_bytes, XLSX_ROWS, &xlsx_timing);

    // Session 13's gate: record the numbers above in `docs/HARDWARE.md`, run the same binary on
    // the 2019 practice PC, and only build the typed column cache
    // (`docs/SESSION-DATA-13-Column-Cache.md`) if one question there does not already land
    // comfortably under the budget retrieval already accepts for a similarly sized folder.
}
