//! Tier 2 of the grounding priority chain (`docs/SELECTION-AND-MEMORY.md`): tables are selected
//! and no document is. Every question on this tier is answered by `tabular::structural` or
//! `tabular::engine`, and nothing else - no gateway call, no model, no fallback to open chat.
//!
//! When the engine cannot answer exactly, the answer is still useful: a `Nudge` naming the real
//! sheets and columns of the workbook, so the interface can suggest a question the engine does
//! answer (`docs/SESSION-DATA-05-TABULAR-UI.md` section 5b). It is built from what the engine and
//! the cached inventory already hold. An unclassified question, or a
//! `NOT_DETERMINISTICALLY_ANSWERABLE` outcome, is never sent to a model "to be helpful".
//!
//! Machine codes and data only, exactly like `folder_questions::FolderAnswer`: the interface
//! writes every sentence, in her language.
//!
//! `answer` takes no gateway, no model alias and no network client: that the tabular tier cannot
//! reach a model is a property of its signature, not a promise.

use serde::Serialize;

use crate::analysis_scope::ScopeMode;
use crate::data_folder::DataFolder;
use crate::error::AppError;
use crate::file_record::{FileRecord, ProcessingStatus};
use crate::file_reference::{FileReferenceResolver, ReferenceStatus};
use crate::inventory::WorkFolderInventory;
use crate::tabular::engine::{
    self, NotAnswerableReason, TabularDerivation, TabularLocator, TabularOutcome, TabularValue,
};
use crate::tabular::inventory::{SheetInventory, TabularInventory};
use crate::tabular::question::{self, TabularRoute};
use crate::tabular::structural::{self, StructuralAnswer};
use crate::tabular::{self, TabularError};

/// What a question about the selected tables came to. `file` is always the workbook's path
/// relative to the Data Folder, so the interface can cite it beside the sheet.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum TabularAnswer {
    /// A value the engine computed. `locator.sheet` is the citation: where a document answer says
    /// "page 3", this says which sheet.
    Value {
        file: String,
        value: TabularValue,
        locator: TabularLocator,
        derivation: TabularDerivation,
    },
    /// A structural fact read from the inventory: sheets, rows, columns, formulas.
    Structural {
        file: String,
        answer: StructuralAnswer,
    },
    /// The engine could not answer this exactly. What the workbook does hold, so the interface can
    /// say which question would work. `reason` is `None` when the question was not recognised at
    /// all, and the engine's own reason otherwise.
    Nudge {
        file: String,
        reason: Option<NotAnswerableReason>,
        available_sheets: Vec<String>,
        available_columns: Vec<String>,
        /// A column the engine would total as it stands - an amount, never a year or an
        /// identifier (`question::example_columns`) - for a concrete example ("somme de
        /// montant"). `None` when the sheet has none.
        example_column: Option<String>,
        /// A text column to group by, for a second example ("which agency has the most
        /// amount"). `None` when the sheet has none, or no column to total.
        example_group: Option<String>,
    },
    /// A group question with several columns it could total, none named. Asked, never picked.
    WhichMeasure {
        file: String,
        group_column: String,
        candidates: Vec<String>,
    },
    /// Analysed, and nothing in it looks like a table (red on the listing). The same honesty an
    /// unreadable scan gets.
    WorkbookUnreadable { file: String },
    /// Chosen, but its analysis is gone (the Data Folder was reset) or it was never analysed.
    WorkbookNotAnalysed { file: String },
    /// Several workbooks are selected and the question names none of them. Never picked between.
    WhichWorkbook { candidates: Vec<String> },
    /// The question names a file that more than one selected workbook matches.
    AmbiguousReference {
        query: String,
        candidates: Vec<String>,
    },
    /// The question names a workbook the Data Folder holds but this conversation did not select.
    FileNotSelected { query: String },
    /// The question names a file the Data Folder does not hold.
    NoMatchingFile { query: String },
    /// "Tous" was chosen, and no workbook in the folder is usable.
    NoUsableTable,
}

/// One selected workbook, with the sheets its entry was narrowed to.
struct Chosen<'a> {
    record: &'a FileRecord,
    sheets: Option<Vec<String>>,
}

/// Answer one question from the tables `selection` chose in `data`.
///
/// Refuses with `ScopeUnavailable` exactly as the document path does when every chosen file is
/// gone or has changed since it was ticked. A red or unanalysed workbook is never answered from,
/// whatever the selection says: the interface offers it no checkbox, and this refuses it again.
pub fn answer(
    question: &str,
    data: &DataFolder,
    selection: &ScopeMode,
    locale: &str,
) -> Result<TabularAnswer, AppError> {
    let chosen = chosen_workbooks(data, selection)?;
    if chosen.is_empty() {
        return Ok(TabularAnswer::NoUsableTable);
    }

    let target = match pick_target(question, data, &chosen) {
        Ok(target) => target,
        Err(answer) => return Ok(answer),
    };
    let file = target.record.relative_path.clone();
    match target.record.processing_status {
        ProcessingStatus::Indexed => {}
        ProcessingStatus::Failed => return Ok(TabularAnswer::WorkbookUnreadable { file }),
        _ => return Ok(TabularAnswer::WorkbookNotAnalysed { file }),
    }
    let Some(inventory) = data.usable_inventory(&file) else {
        return Ok(TabularAnswer::WorkbookNotAnalysed { file });
    };
    let allowed = target.sheets.as_deref();

    match question::classify(question, inventory, allowed, locale) {
        TabularRoute::Structural(structural_question) => {
            match structural::answer(inventory, allowed, &structural_question) {
                StructuralAnswer::NotAnswerable {
                    reason,
                    available_sheets,
                } => Ok(nudge(
                    file,
                    inventory,
                    allowed,
                    Some(reason),
                    available_sheets,
                    Vec::new(),
                )),
                answer => Ok(TabularAnswer::Structural { file, answer }),
            }
        }
        TabularRoute::Operation { sheet, operation } => {
            // Re-read and re-hashed at the moment of computing: a workbook that changed since
            // its analysis is refused, never computed against a stale inventory.
            let path = data
                .files()
                .absolute_path(target.record)
                .ok_or(AppError::ScopeUnavailable)?;
            let (workbook, fresh) =
                match tabular::load_current(&path, &file, &inventory.workbook_id) {
                    Ok(pair) => pair,
                    Err(TabularError::WorkbookChanged) => return Err(AppError::ScopeUnavailable),
                    Err(_) => return Ok(TabularAnswer::WorkbookUnreadable { file }),
                };
            match engine::execute(&workbook, &fresh, sheet.as_deref(), allowed, &operation) {
                TabularOutcome::Value {
                    value,
                    locator,
                    derivation,
                } => Ok(TabularAnswer::Value {
                    file,
                    value,
                    locator,
                    derivation,
                }),
                TabularOutcome::NotDeterministicallyAnswerable {
                    reason,
                    available_sheets,
                    available_columns,
                } => Ok(nudge(
                    file,
                    &fresh,
                    allowed,
                    Some(reason),
                    available_sheets,
                    available_columns,
                )),
            }
        }
        TabularRoute::WhichMeasure {
            group_by,
            candidates,
        } => Ok(TabularAnswer::WhichMeasure {
            file,
            group_column: group_by,
            candidates,
        }),
        TabularRoute::NotRecognised => {
            let sheets = sheet_names(engine::reachable_sheets(inventory, allowed));
            Ok(nudge(file, inventory, allowed, None, sheets, Vec::new()))
        }
    }
}

/// The workbooks the selection keeps. "Tous" keeps every green workbook, so an empty result
/// there means the folder has nothing usable; an explicit selection keeps whatever it pinned that
/// is still there unchanged, green or not, so that `answer` can say what is wrong with it.
fn chosen_workbooks<'a>(
    data: &'a DataFolder,
    selection: &ScopeMode,
) -> Result<Vec<Chosen<'a>>, AppError> {
    match selection {
        ScopeMode::WholeFolder => Ok(data
            .files()
            .all_files()
            .iter()
            .filter(|record| record.processing_status == ProcessingStatus::Indexed)
            .map(|record| Chosen {
                record,
                sheets: None,
            })
            .collect()),
        ScopeMode::Explicit(entries) => {
            let chosen: Vec<Chosen> = entries
                .iter()
                .filter_map(|entry| {
                    let record = data.files().find_by_relative_path(&entry.relative_path)?;
                    (record.id == entry.pinned_id).then(|| Chosen {
                        record,
                        sheets: (!entry.sheet_names.is_empty()).then(|| entry.sheet_names.clone()),
                    })
                })
                .collect();
            // She chose workbooks and none of them is there as she chose it: say so rather than
            // answer from nothing, the same refusal as a document selection gets.
            if chosen.is_empty() {
                return Err(AppError::ScopeUnavailable);
            }
            Ok(chosen)
        }
    }
}

/// Which chosen workbook the question is about: the one it names, or the only usable one. Never
/// a guess between two.
fn pick_target<'c, 'a>(
    question: &str,
    data: &'a DataFolder,
    chosen: &'c [Chosen<'a>],
) -> Result<&'c Chosen<'a>, TabularAnswer> {
    let scoped = WorkFolderInventory::from_records(
        data.files().root(),
        chosen.iter().map(|each| each.record.clone()).collect(),
    );
    let find = |relative_path: &str| {
        chosen
            .iter()
            .find(|each| each.record.relative_path == relative_path)
    };

    match FileReferenceResolver::new(&scoped).resolve_in_question(question) {
        Some(resolution) => match resolution.status {
            ReferenceStatus::Exact => resolution
                .exact_match
                .as_ref()
                .and_then(|record| find(&record.relative_path))
                .ok_or(TabularAnswer::NoMatchingFile {
                    query: resolution.query.clone(),
                }),
            ReferenceStatus::MultipleMatches => Err(TabularAnswer::AmbiguousReference {
                query: resolution.query,
                candidates: resolution
                    .candidates
                    .into_iter()
                    .map(|record| record.relative_path)
                    .collect(),
            }),
            ReferenceStatus::NoMatch => {
                let elsewhere = FileReferenceResolver::new(data.files()).resolve(&resolution.query);
                Err(match elsewhere.exact_match {
                    Some(record) if record.processing_status == ProcessingStatus::Failed => {
                        TabularAnswer::WorkbookUnreadable {
                            file: record.relative_path,
                        }
                    }
                    _ if elsewhere.status != ReferenceStatus::NoMatch => {
                        TabularAnswer::FileNotSelected {
                            query: resolution.query,
                        }
                    }
                    _ => TabularAnswer::NoMatchingFile {
                        query: resolution.query,
                    },
                })
            }
        },
        None => {
            let usable: Vec<&Chosen> = chosen
                .iter()
                .filter(|each| each.record.processing_status == ProcessingStatus::Indexed)
                .collect();
            match usable.len() {
                1 => Ok(usable[0]),
                // Nothing chosen is usable: the first one tells her why.
                0 => Ok(&chosen[0]),
                _ => Err(TabularAnswer::WhichWorkbook {
                    candidates: usable
                        .iter()
                        .map(|each| each.record.relative_path.clone())
                        .collect(),
                }),
            }
        }
    }
}

/// A refusal turned into a next step. The columns come from the engine when it resolved a sheet,
/// and from the inventory when exactly one sheet is reachable - never from a sheet outside the
/// scope, and never invented.
fn nudge(
    file: String,
    inventory: &TabularInventory,
    allowed: Option<&[String]>,
    reason: Option<NotAnswerableReason>,
    available_sheets: Vec<String>,
    available_columns: Vec<String>,
) -> TabularAnswer {
    let reachable = engine::reachable_sheets(inventory, allowed);
    let sheet: Option<&SheetInventory> = if available_columns.is_empty() {
        match reachable.as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    } else {
        reachable
            .iter()
            .copied()
            .find(|sheet| column_names(sheet) == available_columns)
    };
    let available_columns = match (available_columns.is_empty(), sheet) {
        (true, Some(sheet)) => column_names(sheet),
        _ => available_columns,
    };
    let (example_column, example_group) = match sheet {
        Some(sheet) => question::example_columns(&sheet.columns),
        None => (None, None),
    };
    // A group example needs a column to total as well.
    let example_group = example_group.filter(|_| example_column.is_some());
    let available_sheets = if available_sheets.is_empty() {
        sheet_names(reachable)
    } else {
        available_sheets
    };
    TabularAnswer::Nudge {
        file,
        reason,
        available_sheets,
        available_columns,
        example_column,
        example_group,
    }
}

fn column_names(sheet: &SheetInventory) -> Vec<String> {
    sheet
        .columns
        .iter()
        .map(|column| column.name.clone())
        .collect()
}

fn sheet_names(sheets: Vec<&SheetInventory>) -> Vec<String> {
    sheets.into_iter().map(|sheet| sheet.name.clone()).collect()
}

/// This module never imports `crate::gateway`, `crate::retrieval` or `crate::conversation`: every
/// test below runs with no server at all, which is the point.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_scope::ScopeEntry;
    use crate::data_folder::tests::{invoice_csv, write};
    use crate::data_folder::{self};
    use crate::index_store::IndexStore;
    use crate::inventory::FileHashCache;

    struct Folder {
        data: tempfile::TempDir,
        _app: tempfile::TempDir,
        index: IndexStore,
    }

    impl Folder {
        fn with(files: &[(&str, &str)]) -> Self {
            let data = tempfile::tempdir().unwrap();
            let app = tempfile::tempdir().unwrap();
            let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
            for (path, content) in files {
                write(data.path(), path, content.as_bytes());
            }
            data_folder::analyse(data.path(), &mut index, &|_| {}).unwrap();
            Self {
                data,
                _app: app,
                index,
            }
        }

        fn open(&self) -> DataFolder {
            DataFolder::discover(self.data.path(), Some(&self.index), &FileHashCache::new())
                .unwrap()
        }

        /// An explicit selection of these files, pinned to what they hold now.
        fn ticked(&self, paths: &[&str]) -> ScopeMode {
            let folder = self.open();
            ScopeMode::Explicit(
                paths
                    .iter()
                    .map(|path| ScopeEntry {
                        relative_path: path.to_string(),
                        pinned_id: folder
                            .files()
                            .find_by_relative_path(path)
                            .unwrap()
                            .id
                            .clone(),
                        added_at: 1,
                        sheet_names: Vec::new(),
                    })
                    .collect(),
            )
        }

        fn ask(&self, question: &str, selection: &ScopeMode) -> Result<TabularAnswer, AppError> {
            answer(question, &self.open(), selection, "en-US")
        }
    }

    fn invoices() -> String {
        invoice_csv(12)
    }

    #[test]
    fn a_table_only_question_is_computed_with_its_sheet_as_the_citation() {
        let folder = Folder::with(&[("factures.csv", &invoices())]);

        let result = folder
            .ask(
                "What is the total montant?",
                &folder.ticked(&["factures.csv"]),
            )
            .unwrap();

        let TabularAnswer::Value {
            file,
            value,
            locator,
            ..
        } = result
        else {
            panic!("expected a computed value, got {result:?}");
        };
        assert_eq!(file, "factures.csv");
        // 1,50 + 2,50 + ... + 12,50
        assert_eq!(
            value,
            TabularValue::Sum((1..=12).map(|n| n as f64 + 0.5).sum())
        );
        assert_eq!(
            locator.sheet, "factures",
            "a CSV's one sheet is named after the file"
        );
        assert_eq!(locator.column.as_deref(), Some("montant"));
    }

    #[test]
    fn a_structural_question_is_answered_from_the_inventory() {
        let folder = Folder::with(&[("factures.csv", &invoices())]);

        let result = folder
            .ask("How many rows?", &folder.ticked(&["factures.csv"]))
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::Structural {
                file: "factures.csv".into(),
                answer: StructuralAnswer::RowCount {
                    sheet: "factures".into(),
                    rows: 12
                }
            }
        );
    }

    #[test]
    fn an_unclassified_question_gets_the_real_columns_not_a_generic_refusal() {
        let folder = Folder::with(&[("factures.csv", &invoices())]);

        let result = folder
            .ask(
                "What did the specialist recommend?",
                &folder.ticked(&["factures.csv"]),
            )
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::Nudge {
                file: "factures.csv".into(),
                reason: None,
                available_sheets: vec!["factures".into()],
                available_columns: vec!["date".into(), "fournisseur".into(), "montant".into()],
                example_column: Some("montant".into()),
                example_group: Some("fournisseur".into()),
            }
        );
    }

    #[test]
    fn an_engine_refusal_becomes_a_nudge_carrying_its_reason_and_the_real_columns() {
        let folder = Folder::with(&[("factures.csv", &invoices())]);

        // `fournisseur` is text: a sum over it is NOT_DETERMINISTICALLY_ANSWERABLE.
        let result = folder
            .ask(
                "What is the total fournisseur?",
                &folder.ticked(&["factures.csv"]),
            )
            .unwrap();

        let TabularAnswer::Nudge {
            reason,
            available_columns,
            example_column,
            ..
        } = result
        else {
            panic!("expected a nudge, got {result:?}");
        };
        assert_eq!(reason, Some(NotAnswerableReason::NonNumericColumn));
        assert_eq!(available_columns, vec!["date", "fournisseur", "montant"]);
        assert_eq!(example_column.as_deref(), Some("montant"));
    }

    #[test]
    fn a_red_workbook_is_never_answered_from_even_when_the_selection_holds_it() {
        // The interface offers it no checkbox; this is the second line of defence.
        let folder = Folder::with(&[("notes.csv", "a short note\nwith no table\n")]);

        let result = folder
            .ask("How many rows?", &folder.ticked(&["notes.csv"]))
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::WorkbookUnreadable {
                file: "notes.csv".into()
            }
        );
    }

    #[test]
    fn a_red_workbook_named_in_the_question_is_called_unreadable_not_missing() {
        let folder = Folder::with(&[
            ("factures.csv", &invoices()),
            ("notes.csv", "a short note\nwith no table\n"),
        ]);

        let result = folder
            .ask("How many rows in notes.csv?", &ScopeMode::WholeFolder)
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::WorkbookUnreadable {
                file: "notes.csv".into()
            }
        );
    }

    #[test]
    fn two_selected_workbooks_and_no_name_asks_which_one() {
        let folder = Folder::with(&[("achats.csv", &invoices()), ("ventes.csv", &invoices())]);

        let result = folder
            .ask("What is the total montant?", &ScopeMode::WholeFolder)
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::WhichWorkbook {
                candidates: vec!["achats.csv".into(), "ventes.csv".into()]
            }
        );
    }

    #[test]
    fn two_selected_workbooks_and_one_named_answers_from_that_one() {
        let folder = Folder::with(&[("achats.csv", &invoices()), ("ventes.csv", &invoice_csv(9))]);

        let result = folder
            .ask("How many rows in ventes?", &ScopeMode::WholeFolder)
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::Structural {
                file: "ventes.csv".into(),
                answer: StructuralAnswer::RowCount {
                    sheet: "ventes".into(),
                    rows: 9
                }
            }
        );
    }

    #[test]
    fn a_workbook_outside_the_selection_is_named_as_not_selected() {
        let folder = Folder::with(&[("achats.csv", &invoices()), ("ventes.csv", &invoices())]);

        let result = folder
            .ask(
                "How many rows in ventes.csv?",
                &folder.ticked(&["achats.csv"]),
            )
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::FileNotSelected {
                query: "ventes.csv".into()
            }
        );
    }

    #[test]
    fn every_workbook_red_under_tous_says_there_is_no_usable_table() {
        let folder = Folder::with(&[("notes.csv", "a short note\n")]);

        let result = folder
            .ask("How many rows?", &ScopeMode::WholeFolder)
            .unwrap();

        assert_eq!(result, TabularAnswer::NoUsableTable);
    }

    #[test]
    fn a_ticked_workbook_that_changed_since_is_refused_like_a_changed_document() {
        let folder = Folder::with(&[("factures.csv", &invoices())]);
        let selection = folder.ticked(&["factures.csv"]);
        write(
            folder.data.path(),
            "factures.csv",
            invoice_csv(13).as_bytes(),
        );

        let result = folder.ask("How many rows?", &selection);

        assert!(matches!(result, Err(AppError::ScopeUnavailable)));
    }

    #[test]
    fn a_ticked_workbook_whose_analysis_was_reset_asks_for_an_analysis() {
        let mut folder = Folder::with(&[("factures.csv", &invoices())]);
        let selection = folder.ticked(&["factures.csv"]);
        folder.index.clear_tabular_inventories().unwrap();

        let result = folder.ask("How many rows?", &selection).unwrap();

        assert_eq!(
            result,
            TabularAnswer::WorkbookNotAnalysed {
                file: "factures.csv".into()
            }
        );
    }

    // --- Group questions, on the layout of a real public-revenue export ----------------------

    /// `revenue_sub_agency.csv`'s columns, with made-up rows: two text columns to group by, a
    /// year, the amount, and a numeric code that must never be mistaken for the amount.
    fn revenue() -> String {
        let mut csv = String::from(
            "agency,sub_agency,calendar_year,amount,sub_type_name,rev_type,sub_type\n",
        );
        let rows = [
            ("Parks", "Parks North", 2019, "1200.50", 3),
            ("Parks", "Parks South", 2020, "800", 3),
            ("Transit", "Transit Bus", 2019, "5000", 7),
            ("Transit", "Transit Rail", 2020, "2500.25", 7),
            ("Health", "Health Clinics", 2019, "300", 1),
            ("Health", "Health Labs", 2020, "450", 1),
            ("Water", "Water Supply", 2019, "900", 5),
            ("Water", "Water Waste", 2020, "100", 5),
            ("Transit", "Transit Bus", 2021, "1000", 7),
        ];
        for (agency, sub, year, amount, code) in rows {
            csv.push_str(&format!(
                "{agency},{sub},{year},{amount},Fees,Other,{code}\n"
            ));
        }
        csv
    }

    fn ranking(result: &TabularAnswer) -> &crate::tabular::engine::GroupRanking {
        match result {
            TabularAnswer::Value {
                value: TabularValue::LargestGroup(ranking),
                ..
            } => ranking,
            other => panic!("expected a group ranking, got {other:?}"),
        }
    }

    #[test]
    fn which_agency_costs_the_most_is_answered_with_the_total_per_agency() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask(
                "Which agency costs the most? And how much?",
                &folder.ticked(&["revenue_sub_agency.csv"]),
            )
            .unwrap();

        let top = ranking(&result);
        assert_eq!(top.group_column, "agency");
        assert_eq!(top.group_count, 4);
        assert_eq!(top.top[0].group, "Transit");
        assert!((top.top[0].sum - 8500.25).abs() < 1e-9);
        let TabularAnswer::Value { locator, .. } = &result else {
            unreachable!()
        };
        assert_eq!(
            locator.column.as_deref(),
            Some("amount"),
            "the amount, never the year or the code"
        );
    }

    #[test]
    fn please_list_agencies_lists_the_agencies() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Please list agencies", &folder.ticked(&["revenue_sub_agency.csv"]))
            .unwrap();

        let TabularAnswer::Value {
            value: TabularValue::Distinct(values),
            locator,
            ..
        } = result
        else {
            panic!("expected the agencies, got {result:?}");
        };
        assert_eq!(values, vec!["Health", "Parks", "Transit", "Water"]);
        assert_eq!(locator.column.as_deref(), Some("agency"));
    }

    #[test]
    fn a_plural_two_word_column_is_listed_by_its_own_values() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Show the sub agencies", &ScopeMode::WholeFolder)
            .unwrap();

        let TabularAnswer::Value { locator, .. } = &result else {
            panic!("expected a list, got {result:?}");
        };
        assert_eq!(locator.column.as_deref(), Some("sub_agency"));
    }

    #[test]
    fn a_plural_group_is_still_ranked() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Which agencies cost the most?", &ScopeMode::WholeFolder)
            .unwrap();

        assert_eq!(ranking(&result).group_column, "agency");
    }

    #[test]
    fn the_same_question_typed_in_english_on_a_french_interface_is_still_read() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = answer(
            "Which agency costs the most?",
            &folder.open(),
            &ScopeMode::WholeFolder,
            "fr-FR",
        )
        .unwrap();

        assert_eq!(ranking(&result).top[0].group, "Transit");
    }

    #[test]
    fn a_two_word_column_is_named_by_both_words_not_by_its_last_one() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Which sub agency has the highest amount?", &ScopeMode::WholeFolder)
            .unwrap();

        let top = ranking(&result);
        assert_eq!(top.group_column, "sub_agency");
        assert_eq!(top.top[0].group, "Transit Bus");
    }

    #[test]
    fn a_year_column_can_be_grouped_by_when_the_question_names_the_year() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Which year had the highest amount?", &ScopeMode::WholeFolder)
            .unwrap();

        let top = ranking(&result);
        assert_eq!(top.group_column, "calendar_year");
        assert_eq!(top.top[0].group, "2019");
    }

    #[test]
    fn a_total_per_group_lists_every_group() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("What is the total amount per agency?", &ScopeMode::WholeFolder)
            .unwrap();

        let TabularAnswer::Value {
            value: TabularValue::GroupSums(sums),
            ..
        } = result
        else {
            panic!("expected every group's total, got {result:?}");
        };
        let groups: Vec<&str> = sums.iter().map(|group| group.group.as_str()).collect();
        assert_eq!(groups, vec!["Health", "Parks", "Transit", "Water"]);
    }

    #[test]
    fn two_columns_that_could_both_be_the_total_are_asked_about_not_picked() {
        let mut csv = String::from("agency,amount,fee\n");
        for index in 0..9 {
            csv.push_str(&format!("A{},{}0,{}\n", index % 3, index + 1, index + 2));
        }
        let folder = Folder::with(&[("costs.csv", &csv)]);

        let result = folder
            .ask("Which agency costs the most?", &ScopeMode::WholeFolder)
            .unwrap();

        assert_eq!(
            result,
            TabularAnswer::WhichMeasure {
                file: "costs.csv".into(),
                group_column: "agency".into(),
                candidates: vec!["amount".into(), "fee".into()],
            }
        );
    }

    #[test]
    fn naming_the_column_to_total_settles_it() {
        let mut csv = String::from("agency,amount,fee\n");
        for index in 0..9 {
            csv.push_str(&format!("A{},{}0,{}\n", index % 3, index + 1, index + 2));
        }
        let folder = Folder::with(&[("costs.csv", &csv)]);

        let result = folder
            .ask("Which agency has the highest fee?", &ScopeMode::WholeFolder)
            .unwrap();

        let TabularAnswer::Value { locator, .. } = &result else {
            panic!("expected a value, got {result:?}");
        };
        assert_eq!(locator.column.as_deref(), Some("fee"));
    }

    #[test]
    fn the_nudge_suggests_the_amount_never_the_year() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);

        let result = folder
            .ask("Tell me something interesting", &ScopeMode::WholeFolder)
            .unwrap();

        let TabularAnswer::Nudge {
            example_column,
            example_group,
            ..
        } = result
        else {
            panic!("expected a nudge, got {result:?}");
        };
        assert_eq!(example_column.as_deref(), Some("amount"));
        assert_eq!(example_group.as_deref(), Some("agency"));
    }

    /// The nudge's examples are sentences in the interface catalogues. Each one, filled with the
    /// columns the nudge would give, must be a question this engine answers - an example that
    /// earns a second nudge would be worse than none.
    #[test]
    fn every_example_the_nudge_offers_is_a_question_the_engine_answers() {
        let folder = Folder::with(&[("revenue_sub_agency.csv", &revenue())]);
        let catalogues = [
            ("en-US", include_str!("../../src/locales/en-US.json")),
            ("fr-FR", include_str!("../../src/locales/fr-FR.json")),
        ];
        for (locale, body) in catalogues {
            let catalogue: serde_json::Value = serde_json::from_str(body).unwrap();
            for key in ["nudgeExample", "nudgeExampleGroup"] {
                let template = catalogue["tabularAnswer"][key]
                    .as_str()
                    .unwrap_or_else(|| panic!("{locale} has no tabularAnswer.{key}"));
                let question = quoted(template)
                    .replace("{column}", "amount")
                    .replace("{group}", "agency");

                let result = answer(&question, &folder.open(), &ScopeMode::WholeFolder, locale)
                    .unwrap();

                assert!(
                    matches!(result, TabularAnswer::Value { .. }),
                    "{locale} {key}: {question:?} gave {result:?}"
                );
            }
        }
    }

    /// The part of an example sentence between its quotation marks, whichever the language uses.
    fn quoted(template: &str) -> String {
        let open = template
            .find(['\u{201c}', '\u{ab}'])
            .expect("an example is quoted");
        let rest = &template[open..];
        let body: String = rest.chars().skip(1).collect();
        let close = body
            .find(['\u{201d}', '\u{bb}'])
            .expect("an example is closed");
        body[..close].to_string()
    }

    #[test]
    fn every_computed_value_crosses_the_bridge() {
        // The interface reads these; a value that cannot be serialised would fail the whole
        // question at the last step, after the engine had already answered it.
        let row = crate::tabular::engine::RowValue {
            row_index: 0,
            cells: vec![crate::tabular::engine::CellText {
                column: "montant".into(),
                text: "10".into(),
            }],
        };
        for value in [
            TabularValue::Count(3),
            TabularValue::Sum(1.5),
            TabularValue::Min(1.0),
            TabularValue::Max(2.0),
            TabularValue::Distinct(vec!["a".into()]),
            TabularValue::GroupSums(vec![crate::tabular::engine::GroupSum {
                group: "a".into(),
                sum: 1.0,
            }]),
            TabularValue::LargestRow(row.clone()),
            TabularValue::Row(row.clone()),
            TabularValue::Rows(vec![row]),
        ] {
            let answer = TabularAnswer::Value {
                file: "f.csv".into(),
                value: value.clone(),
                locator: TabularLocator {
                    sheet: "Facturation".into(),
                    header_row: Some(0),
                    column: Some("montant".into()),
                    row_range: Some((0, 0)),
                },
                derivation: TabularDerivation::Computed {
                    operation: "sum".into(),
                    row_count: 1,
                },
            };
            let json = serde_json::to_value(&answer)
                .unwrap_or_else(|error| panic!("{value:?} did not serialise: {error}"));
            assert_eq!(json["kind"], "value");
            assert_eq!(json["locator"]["sheet"], "Facturation");
        }
        let json = serde_json::to_value(TabularValue::Sum(1.5)).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "sum", "value": 1.5 }));
    }

    #[test]
    fn the_wire_form_is_tagged_with_camel_case_fields() {
        let json = serde_json::to_value(TabularAnswer::Nudge {
            file: "f.csv".into(),
            reason: Some(NotAnswerableReason::ColumnNotFound),
            available_sheets: vec!["f".into()],
            available_columns: vec!["a".into()],
            example_column: None,
            example_group: None,
        })
        .unwrap();

        assert_eq!(json["kind"], "nudge");
        assert_eq!(json["reason"], "column_not_found");
        assert_eq!(json["availableColumns"], serde_json::json!(["a"]));
        assert_eq!(json["exampleColumn"], serde_json::Value::Null);
    }
}
