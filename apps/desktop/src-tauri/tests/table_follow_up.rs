//! TABULAR step 11 (`docs/SESSION-TABULAR/SESSION-TABULAR-11-follow-up-memory.md`): a short follow-up on
//! a table answer ("and for March?", "and for Beta?", "and the count?", "by supplier", "sort them by
//! date") is understood from the state the previous answer left, which the interface sends back and
//! the program distrusts.
//!
//! Exercised one level below the Tauri command (`tabular_tier` needs an `AppHandle`), at the calls the
//! three doors make: `prepare_with_carry_over`, `prepare_for_mixed_with_carry_over` and
//! `prepare_if_data_only_with_carry_over`. Every workbook is written at test time
//! (`tests/common/tabular_fixtures.rs`, or inline for the two-measure shape). The invoices of
//! `factures.csv` hold, per row `i` (0-based) of 60: date 05/01/2026 + `i` days, supplier `i % 4` of
//! Alpha, Beta, Gamma, Delta, amount 1 000,50 + 100 x `i` EUR. So March is `i` = 55..=59 (5 rows, total
//! 5 x 1 000,50 + 100 x 285 = **33 502,50**), and Beta in March is `i` = 57 alone (**6 700,50**), Gamma
//! `i` = 58 (**6 800,50**).

mod common;

use std::time::Duration;

use assistant_cabinet_ai_lib::analysis_scope::{ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::FileHashCache;
use assistant_cabinet_ai_lib::tabular::carry_over::{
    CarryDrop, CarryReport, OperationKind, TableCarryOver,
};
use assistant_cabinet_ai_lib::tabular::engine::{AppliedFilter, TabularValue};
use assistant_cabinet_ai_lib::tabular_answer::{self, ModelAssist, TabularAnswer};
use common::counting_gateway::start_counting_gateway;
use common::tabular_fixtures::{self, AGENCIES, DEPENSES, FACTURES};

const FR: &str = "fr-FR";
const EN: &str = "en-US";
const TWO_MEASURES: &str = "commandes.csv";

struct Folder {
    data: tempfile::TempDir,
    _app: tempfile::TempDir,
    index: IndexStore,
}

impl Folder {
    fn build() -> Self {
        let data = tempfile::tempdir().unwrap();
        let app = tempfile::tempdir().unwrap();
        tabular_fixtures::write_all(data.path());
        // Two numeric columns: a total could read either of them.
        std::fs::write(
            data.path().join(TWO_MEASURES),
            "fournisseur;montant;quantite\n\
             Alpha;100;1\nBeta;650;2\nGamma;300;3\nDelta;120;4\n\
             Alpha;130;5\nBeta;140;6\nGamma;150;7\nDelta;250;8\n",
        )
        .unwrap();
        let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
        data_folder::analyse(data.path(), &mut index, FR, &|_| {}).unwrap();
        Self {
            data,
            _app: app,
            index,
        }
    }

    fn open(&self) -> DataFolder {
        DataFolder::discover(self.data.path(), Some(&self.index), &FileHashCache::new()).unwrap()
    }

    fn ticked(&self, names: &[&str]) -> ScopeMode {
        let folder = self.open();
        ScopeMode::Explicit(
            names
                .iter()
                .map(|name| ScopeEntry {
                    relative_path: name.to_string(),
                    pinned_id: folder
                        .files()
                        .find_by_relative_path(name)
                        .unwrap()
                        .id
                        .clone(),
                    added_at: 1,
                    sheet_names: Vec::new(),
                })
                .collect(),
        )
    }

    /// One question of a dialogue on the tables-only door: the answer the engine gives without any
    /// model, and what the carry-over came to.
    fn ask(
        &self,
        question: &str,
        selection: &ScopeMode,
        locale: &str,
        carry: Option<&TableCarryOver>,
    ) -> (TabularAnswer, CarryReport) {
        let (pending, report) = tabular_answer::prepare_with_carry_over(
            question,
            &self.open(),
            selection,
            locale,
            &self.index,
            false,
            carry,
        )
        .unwrap();
        (tabular_answer::without_the_model(pending), report)
    }
}

fn number(answer: &TabularAnswer) -> f64 {
    match answer {
        TabularAnswer::Value {
            value: TabularValue::Sum(aggregate),
            ..
        } => aggregate.value,
        TabularAnswer::Value {
            value: TabularValue::Count(count),
            ..
        } => *count as f64,
        other => panic!("expected a sum or a count, got {other:?}"),
    }
}

fn filters_of(answer: &TabularAnswer) -> Vec<AppliedFilter> {
    match answer {
        TabularAnswer::Value { locator, .. } => locator.filters.clone(),
        other => panic!("expected a computed value, got {other:?}"),
    }
}

fn equals(column: &str, value: &str) -> AppliedFilter {
    AppliedFilter::Equals {
        column: column.into(),
        value: value.into(),
    }
}

fn month(column: &str, month: u8) -> AppliedFilter {
    AppliedFilter::Month {
        column: column.into(),
        month,
    }
}

/// The dialogue's first answer: the total of the invoices, and the state it leaves.
fn first_total(
    folder: &Folder,
    selection: &ScopeMode,
    locale: &str,
    question: &str,
) -> TableCarryOver {
    let (answer, report) = folder.ask(question, selection, locale, None);
    assert_eq!(number(&answer), 237_030.0, "the whole column");
    let next = report.next.expect("a figure leaves a state");
    assert_eq!(next.workbook, FACTURES);
    assert_eq!(next.column.as_deref(), Some("montant"));
    assert_eq!(next.operation, OperationKind::Sum);
    assert!(next.filters.is_empty());
    next
}

#[test]
fn a_new_month_a_new_supplier_and_a_new_operation_continue_the_total_in_french() {
    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES]);
    let total = first_total(&folder, &selection, FR, "Quel est le total montant ?");

    // "And for March?": the period is added to the same sum.
    let (answer, report) = folder.ask("Et pour mars ?", &selection, FR, Some(&total));
    assert_eq!(number(&answer), 33_502.5);
    assert_eq!(filters_of(&answer), vec![month("date", 3)]);
    let inherited = report.inherited.clone().expect("the follow-up was applied");
    assert!(inherited.workbook);
    assert_eq!(inherited.column.as_deref(), Some("montant"));
    assert_eq!(inherited.operation, Some(OperationKind::Sum));
    assert!(
        inherited.filters.is_empty(),
        "nothing to inherit but the sum"
    );
    let in_march = report.next.expect("the state moves on");

    // "And for Beta?": the supplier is added, the month stays.
    let (answer, report) = folder.ask(
        "Et pour le fournisseur Beta ?",
        &selection,
        FR,
        Some(&in_march),
    );
    assert_eq!(number(&answer), 6_700.5);
    assert_eq!(
        filters_of(&answer),
        vec![month("date", 3), equals("fournisseur", "Beta")]
    );
    assert_eq!(report.inherited.unwrap().filters, vec![month("date", 3)]);
    let beta = report.next.unwrap();

    // "And for Gamma?": the supplier is replaced, not added.
    let (answer, report) = folder.ask("Et pour Gamma ?", &selection, FR, Some(&beta));
    assert_eq!(number(&answer), 6_800.5);
    assert_eq!(
        filters_of(&answer),
        vec![month("date", 3), equals("fournisseur", "Gamma")]
    );
    let gamma = report.next.unwrap();

    // "And the count?": the operation changes, the filters stay.
    let (answer, report) = folder.ask("Et le nombre ?", &selection, FR, Some(&gamma));
    assert_eq!(number(&answer), 1.0);
    assert_eq!(
        filters_of(&answer),
        vec![month("date", 3), equals("fournisseur", "Gamma")]
    );
    let inherited = report.inherited.clone().unwrap();
    assert_eq!(
        inherited.operation, None,
        "the operation was changed by the question"
    );
    assert_eq!(
        report.next.as_ref().unwrap().operation,
        OperationKind::Count
    );

    // "By supplier": a grouping added to the count. The filter on the supplier gives way to the grouping,
    // the month stays: March holds Delta twice, and Alpha, Beta and Gamma once.
    let count = report.next.unwrap();
    let (answer, report) = folder.ask("Et par fournisseur ?", &selection, FR, Some(&count));
    let TabularAnswer::Value {
        value: TabularValue::CountPerGroup(groups),
        locator,
        ..
    } = &answer
    else {
        panic!("expected a count per group, got {answer:?}");
    };
    assert_eq!(groups.len(), 4);
    assert_eq!(groups[0].count, 2, "the largest group first");
    assert_eq!(locator.filters, vec![month("date", 3)]);
    assert_eq!(
        report.next.unwrap().group_by.as_deref(),
        Some("fournisseur")
    );
}

#[test]
fn the_same_dialogue_is_understood_in_english() {
    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES]);
    let total = first_total(&folder, &selection, EN, "What is the total montant?");

    let (answer, report) = folder.ask("And for March?", &selection, EN, Some(&total));
    assert_eq!(number(&answer), 33_502.5);
    let in_march = report.next.unwrap();

    let (answer, report) = folder.ask(
        "What about the fournisseur Beta?",
        &selection,
        EN,
        Some(&in_march),
    );
    assert_eq!(number(&answer), 6_700.5);
    assert_eq!(report.inherited.unwrap().filters, vec![month("date", 3)]);

    let (answer, _) = folder.ask(
        "Same for Gamma",
        &selection,
        EN,
        Some(&report.next.unwrap()),
    );
    assert_eq!(
        number(&answer),
        6_800.5,
        "the supplier is replaced, the month stays"
    );
}

#[test]
fn a_total_by_supplier_is_a_grouping_added_to_the_same_sum() {
    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES]);
    let total = first_total(&folder, &selection, FR, "Quel est le total montant ?");

    let (answer, report) = folder.ask("Et par fournisseur ?", &selection, FR, Some(&total));
    let TabularAnswer::Value {
        value: TabularValue::GroupSums(groups),
        ..
    } = &answer
    else {
        panic!("expected totals per group, got {answer:?}");
    };
    assert_eq!(groups.len(), 4);
    let next = report.next.unwrap();
    assert_eq!(next.operation, OperationKind::GroupSum);
    assert_eq!(next.group_by.as_deref(), Some("fournisseur"));
    assert_eq!(next.column.as_deref(), Some("montant"));
}

#[test]
fn sorting_keeps_the_filtered_selection() {
    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES]);
    let (answer, report) = folder.ask("Trier par montant pour Beta", &selection, FR, None);
    let TabularAnswer::Value {
        value: TabularValue::Rows(rows),
        ..
    } = &answer
    else {
        panic!("expected rows, got {answer:?}");
    };
    assert_eq!(rows.len(), 15, "Beta holds 15 rows");
    let sorted = report.next.expect("a sort leaves a state");

    let (answer, report) = folder.ask("Trie-les par date", &selection, FR, Some(&sorted));
    let TabularAnswer::Value {
        value: TabularValue::Rows(rows),
        locator,
        ..
    } = &answer
    else {
        panic!("expected rows, got {answer:?}");
    };
    assert_eq!(rows.len(), 15, "the same selection, in another order");
    assert_eq!(locator.column.as_deref(), Some("date"));
    assert_eq!(locator.filters, vec![equals("fournisseur", "Beta")]);
    assert_eq!(report.next.unwrap().column.as_deref(), Some("date"));
}

#[test]
fn a_question_that_stands_alone_never_inherits() {
    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES, DEPENSES]);
    let total = first_total(
        &folder,
        &folder.ticked(&[FACTURES]),
        FR,
        "Quel est le total montant ?",
    );
    let mut in_march = total.clone();
    in_march.filters = vec![month("date", 3)];

    // It names a workbook.
    let (_, report) = folder.ask(
        "Et pour mars dans depenses.csv ?",
        &selection,
        FR,
        Some(&in_march),
    );
    assert!(report.inherited.is_none());
    assert!(report.dropped.is_none(), "a reset is silent");

    // It carries a complete operation of its own: the figure is the question's, not the dialogue's.
    let (answer, report) = folder.ask(
        "Et la somme des montants pour Beta ?",
        &folder.ticked(&[FACTURES]),
        FR,
        Some(&in_march),
    );
    assert!(report.inherited.is_none());
    assert_eq!(
        number(&answer),
        58_507.5,
        "Beta over the whole year, March is not carried"
    );

    // It is long.
    let (_, report) = folder.ask(
        "Et pour mars, avril, mai, juin, juillet, aout, septembre ou octobre ?",
        &folder.ticked(&[FACTURES]),
        FR,
        Some(&in_march),
    );
    assert!(report.inherited.is_none());

    // It points at another file.
    let (_, report) = folder.ask(
        "Meme chose pour l'autre fichier",
        &selection,
        FR,
        Some(&in_march),
    );
    assert!(report.inherited.is_none());

    // It does not open like a follow-up.
    let (_, report) = folder.ask(
        "Combien de lignes ?",
        &folder.ticked(&[FACTURES]),
        FR,
        Some(&in_march),
    );
    assert!(report.inherited.is_none());

    // There is no previous table answer.
    let (_, report) = folder.ask("Et pour mars ?", &folder.ticked(&[FACTURES]), FR, None);
    assert!(report.inherited.is_none());
    assert!(report.dropped.is_none());
}

#[test]
fn a_state_that_no_longer_holds_is_dropped_and_said() {
    let folder = Folder::build();
    let total = first_total(
        &folder,
        &folder.ticked(&[FACTURES]),
        FR,
        "Quel est le total montant ?",
    );

    // The workbook is not ticked any more.
    let (_, report) = folder.ask(
        "Et pour mars ?",
        &folder.ticked(&[DEPENSES]),
        FR,
        Some(&total),
    );
    assert!(report.inherited.is_none());
    assert_eq!(
        report.dropped,
        Some((FACTURES.to_string(), CarryDrop::WorkbookNotSelected))
    );

    // A forged state naming a workbook of the folder that was never ticked is ignored the same way.
    let mut forged = total.clone();
    forged.workbook = AGENCIES.to_string();
    let (_, report) = folder.ask(
        "Et pour mars ?",
        &folder.ticked(&[FACTURES]),
        FR,
        Some(&forged),
    );
    assert_eq!(
        report.dropped,
        Some((AGENCIES.to_string(), CarryDrop::WorkbookNotSelected))
    );

    // A path outside the folder is nothing the selection holds.
    let mut outside = total.clone();
    outside.workbook = "../../secret.xlsx".to_string();
    let (_, report) = folder.ask(
        "Et pour mars ?",
        &folder.ticked(&[FACTURES]),
        FR,
        Some(&outside),
    );
    assert_eq!(
        report.dropped.as_ref().map(|(_, why)| *why),
        Some(CarryDrop::WorkbookNotSelected)
    );

    // The sheet is gone.
    let mut sheet_gone = total.clone();
    sheet_gone.sheet = Some("Ghost".to_string());
    let (_, report) = folder.ask(
        "Et pour mars ?",
        &folder.ticked(&[FACTURES]),
        FR,
        Some(&sheet_gone),
    );
    assert_eq!(
        report.dropped.map(|(_, why)| why),
        Some(CarryDrop::SheetGone)
    );

    // A column is gone.
    let mut column_gone = total.clone();
    column_gone.column = Some("prix_unitaire".to_string());
    let (_, report) = folder.ask(
        "Et pour mars ?",
        &folder.ticked(&[FACTURES]),
        FR,
        Some(&column_gone),
    );
    assert_eq!(
        report.dropped.map(|(_, why)| why),
        Some(CarryDrop::ColumnGone)
    );

    // A filter on a column that is gone, and a date that is not a date.
    let mut filter_gone = total.clone();
    filter_gone.filters = vec![equals("service", "X")];
    let (_, report) = folder.ask(
        "Et pour mars ?",
        &folder.ticked(&[FACTURES]),
        FR,
        Some(&filter_gone),
    );
    assert_eq!(
        report.dropped.map(|(_, why)| why),
        Some(CarryDrop::ColumnGone)
    );
    let mut not_a_date = total;
    not_a_date.filters = vec![AppliedFilter::DateRange {
        column: "date".into(),
        start: "not a date".into(),
        end: "2026-03-31".into(),
    }];
    let (_, report) = folder.ask(
        "Et pour mars ?",
        &folder.ticked(&[FACTURES]),
        FR,
        Some(&not_a_date),
    );
    assert_eq!(
        report.dropped.map(|(_, why)| why),
        Some(CarryDrop::Malformed)
    );
}

#[test]
fn a_follow_up_that_fits_two_columns_asks_instead_of_guessing() {
    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES]);
    let total = first_total(&folder, &selection, FR, "Quel est le total montant ?");

    // Alpha is a supplier and a client (the invoices hold it in two columns on purpose).
    let (answer, _) = folder.ask("Et pour Alpha ?", &selection, FR, Some(&total));
    let TabularAnswer::WhichColumn { candidates, .. } = &answer else {
        panic!("expected a question about the column, got {answer:?}");
    };
    assert!(
        candidates.contains(&"fournisseur".to_string())
            && candidates.contains(&"client".to_string())
    );

    // "And the total?" after a count, where two columns could be added: asked.
    let two = folder.ticked(&[TWO_MEASURES]);
    let (count, report) = folder.ask("Combien de lignes pour Beta ?", &two, FR, None);
    assert!(matches!(count, TabularAnswer::Value { .. }), "{count:?}");
    let (answer, _) = folder.ask("Et le total ?", &two, FR, Some(&report.next.unwrap()));
    let TabularAnswer::WhichFollowUpColumn { candidates, .. } = &answer else {
        panic!("expected a question about the column, got {answer:?}");
    };
    assert_eq!(
        candidates,
        &vec!["montant".to_string(), "quantite".to_string()]
    );
}

#[test]
fn a_follow_up_that_finds_no_value_is_refused_not_repeated() {
    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES]);
    let total = first_total(&folder, &selection, FR, "Quel est le total montant ?");

    let (answer, report) = folder.ask("Et pour Zorglub ?", &selection, FR, Some(&total));
    assert!(
        matches!(answer, TabularAnswer::Nudge { .. }),
        "the figure of the previous question is never repeated: {answer:?}"
    );
    assert!(report.next.is_none());
}

#[test]
fn the_mixed_doors_continue_the_table_too() {
    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES]);
    let total = first_total(&folder, &selection, FR, "Quel est le total montant ?");
    let documents = vec!["note.txt".to_string()];

    let (pending, report) = tabular_answer::prepare_for_mixed_with_carry_over(
        "Et pour mars ?",
        &folder.open(),
        &selection,
        FR,
        &folder.index,
        &documents,
        Some(&total),
    )
    .unwrap();
    assert_eq!(
        number(&tabular_answer::without_the_model(pending)),
        33_502.5
    );
    assert!(report.inherited.is_some());

    let (pending, report) = tabular_answer::prepare_if_data_only_with_carry_over(
        "Et pour mars ?",
        &folder.open(),
        &selection,
        FR,
        &folder.index,
        Some(&total),
    )
    .unwrap();
    let pending = pending.expect("a follow-up on the data alone is answered by the engine alone");
    assert_eq!(
        number(&tabular_answer::without_the_model(pending)),
        33_502.5
    );
    assert!(report.inherited.is_some());
}

#[test]
fn a_deterministic_follow_up_makes_no_gateway_call() {
    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES]);
    let total = first_total(&folder, &selection, FR, "Quel est le total montant ?");
    let gateway = start_counting_gateway();
    let client = GatewayClient::new().expect("builds a client");
    let assist = ModelAssist {
        gateway: &client,
        server_url: &gateway.url,
        model_alias: "cabinet-chat",
        idle_timeout: Duration::from_millis(500),
    };

    for question in ["Et pour mars ?", "Et le nombre ?", "Et par fournisseur ?"] {
        let (pending, _) = tabular_answer::prepare_with_carry_over(
            question,
            &folder.open(),
            &selection,
            FR,
            &folder.index,
            false,
            Some(&total),
        )
        .unwrap();
        let answer = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(tabular_answer::resolve(pending, question, FR, &assist));
        assert!(
            matches!(answer, TabularAnswer::Value { .. }),
            "{question}: {answer:?}"
        );
    }
    assert_eq!(gateway.calls(), 0, "the model was never dialled");
}

#[test]
fn the_state_it_sends_back_is_the_one_each_answer_left() {
    // Regenerating the second answer reads the first answer's state, not the third's: the state is a
    // pure function of the answer that left it, so the interface can read it off the entry before.
    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES]);
    let first = first_total(&folder, &selection, FR, "Quel est le total montant ?");
    let (_, second) = folder.ask("Et pour mars ?", &selection, FR, Some(&first));
    let second = second.next.unwrap();
    let (_, third) = folder.ask("Et pour Beta ?", &selection, FR, Some(&second));
    let third = third.next.unwrap();

    let (again, _) = folder.ask("Et pour mars ?", &selection, FR, Some(&first));
    assert_eq!(number(&again), 33_502.5);
    assert_ne!(first, second);
    assert_ne!(second, third);
    let (not_this_one, _) = folder.ask("Et pour mars ?", &selection, FR, Some(&third));
    assert_ne!(
        filters_of(&not_this_one),
        filters_of(&again),
        "the third answer's state carries Beta, which the second question must not see"
    );
}

#[test]
fn explaining_a_follow_up_recomputes_it_with_the_state_it_was_asked_with() {
    use assistant_cabinet_ai_lib::calculation_follow_up::{
        recompute, recompute_with_carry_over, ShownCalculation,
    };
    use assistant_cabinet_ai_lib::error::AppError;

    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES]);
    let total = first_total(&folder, &selection, FR, "Quel est le total montant ?");
    let question = "Et pour mars ?";
    let (answer, _) = folder.ask(question, &selection, FR, Some(&total));
    let TabularAnswer::Value {
        file,
        value,
        derivation,
        ..
    } = &answer
    else {
        panic!("expected a figure, got {answer:?}");
    };
    let shown = ShownCalculation {
        file: file.clone(),
        value: value.clone(),
        derivation: derivation.clone(),
    };
    let data = folder.open();

    // With the state the question was asked with, the figure comes out again, and says what it inherited.
    let (figure, report) = recompute_with_carry_over(
        question,
        &data,
        &selection,
        FR,
        &folder.index,
        &shown,
        None,
        Some(&total),
    )
    .unwrap();
    assert_eq!(figure, answer);
    let sizes = tabular_answer::sheet_sizes(&data, &selection);
    let record = tabular_answer::calculation_record_inheriting(
        Some(question),
        &figure,
        &sizes,
        None,
        report.inherited.as_ref(),
    )
    .expect("a record");
    assert!(
        record.contains(
            "Inherited from the previous question, not written in this one: the workbook;"
        ),
        "{record}"
    );
    assert!(record.contains("column \"montant\""), "{record}");

    // Without it the short question gives no figure: refused, never explained.
    let refused = recompute(question, &data, &selection, FR, &folder.index, &shown, None);
    assert!(matches!(refused, Err(AppError::ScopeUnavailable)));
}

#[test]
fn the_workbook_a_follow_up_continued_is_implied_before_any_other_rule() {
    use assistant_cabinet_ai_lib::tabular_answer::{implied_workbooks_carrying, ImpliedReason};

    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES, DEPENSES]);
    let data = folder.open();

    // Two ticked, a question that names nothing: no sign points at either of them on its own.
    let alone = implied_workbooks_carrying("Et pour mars ?", &data, &selection, FR, &[], None);
    assert!(alone.is_empty(), "{alone:?}");

    let carried =
        implied_workbooks_carrying("Et pour mars ?", &data, &selection, FR, &[], Some(FACTURES));
    assert_eq!(carried.len(), 1);
    assert_eq!(carried[0].file, FACTURES);
    assert_eq!(carried[0].reason, ImpliedReason::CarriedOver);
    assert_eq!(carried[0].reason.as_code(), "carried_over");

    // A workbook that is not ticked cannot be implied by a forged state.
    let forged =
        implied_workbooks_carrying("Et pour mars ?", &data, &selection, FR, &[], Some(AGENCIES));
    assert!(forged.is_empty(), "{forged:?}");
}

#[test]
fn a_filter_the_question_repeats_is_not_inherited() {
    let folder = Folder::build();
    let selection = folder.ticked(&[FACTURES]);
    let total = first_total(&folder, &selection, FR, "Quel est le total montant ?");
    let (_, in_march) = folder.ask("Et pour mars ?", &selection, FR, Some(&total));
    let (_, beta) = folder.ask("Et pour Beta ?", &selection, FR, in_march.next.as_ref());
    // The same supplier asked again: the month is inherited, the supplier is written by the question.
    let (answer, again) = folder.ask("Et pour Beta ?", &selection, FR, beta.next.as_ref());
    assert_eq!(number(&answer), 6_700.5);
    assert_eq!(again.inherited.unwrap().filters, vec![month("date", 3)]);
}
