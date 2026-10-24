//! TABULAR step 13: the raw grid of a workbook never reaches a reader except through the view.
//!
//! The cells a file holds are not the cells a question reads: hidden rows, merged cells, notes and
//! total rows are dealt with by `tabular::layout`, and only `tabular::view_of` applies it. A new
//! piece of code that opened a file through an adapter, or took the cached raw grid, would read a
//! sheet the engine, the sample and the mail merge do not see. This test lists the only files that
//! may do so; a new one fails here and has to be added on purpose.

use std::path::{Path, PathBuf};

/// What gives access to the raw grid or to an adapter.
const RAW_ACCESS: [&str; 6] = [
    "tabular_workbook_by_hash(",
    "has_tabular_workbook(",
    "load_current_raw(",
    "open_with_facts(",
    "CsvDataSource",
    "XlsxDataSource",
];

/// The files that may use it, under `src/`, with what they are for.
const ALLOWED: [&str; 4] = [
    // The store of the typed cell cache itself.
    "index_store.rs",
    // Reads the file once and builds the first view (`read_raw`, `view_of`, `load_current`).
    "tabular/mod.rs",
    // `load_workbook_cached`: the cache or the file, then the view, for every question.
    "tabular_answer.rs",
    // The adapters implement the trait.
    "tabular/xlsx_adapter.rs",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn only_the_listed_files_reach_the_raw_grid_or_an_adapter() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);

    let mut offenders = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(&src)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if ALLOWED.contains(&relative.as_str()) || relative == "tabular/csv_adapter.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        // Production code only: what follows the first test module is not a reader.
        let production = text.split("#[cfg(test)]").next().unwrap_or("");
        for token in RAW_ACCESS {
            if production.contains(token) {
                offenders.push(format!("{relative}: {token}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these files reach the raw grid or an adapter without the view: {offenders:?}"
    );
}
