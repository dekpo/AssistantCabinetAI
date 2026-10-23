//! TABULAR step 04 (`docs/SESSION-TABULAR/SESSION-TABULAR-04-usage-trace.md`): what each door of
//! `commands::sourced_answer` records about the workbooks of a question. The doors need an
//! `AppHandle` no test builds, so each test calls the same functions the door calls, with the same
//! inputs (`selection_facts`, `question_cards`, `prepare*`, `locate_fill_source`), and then
//! `workbook_use::trace`, the one function every door ends with.
//!
//! No gateway and no socket: a card is "sent" by passing `model_wrote: true`, exactly what
//! `commands::mixed_tier` passes after a model answer. Every workbook is written at test time.

mod common;

use assistant_cabinet_ai_lib::analysis_scope::{ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::conversation::ContextBudget;
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::FileHashCache;
use assistant_cabinet_ai_lib::tabular::card::card_budget_chars;
use assistant_cabinet_ai_lib::tabular_answer::{self, PendingAnswer, TabularAnswer};
use assistant_cabinet_ai_lib::workbook_use::{
    self, Answered, UseReason, WorkbookUse, WorkbookUseHow,
};
use common::tabular_fixtures::{self, AGENCIES, FACTURES};

const COMMANDES: &str = "commandes.csv";

struct Folders {
    data: tempfile::TempDir,
    _app: tempfile::TempDir,
    index: IndexStore,
}

impl Folders {
    fn build() -> Self {
        let data = tempfile::tempdir().unwrap();
        let app = tempfile::tempdir().unwrap();
        std::fs::write(data.path().join(FACTURES), tabular_fixtures::factures_csv()).unwrap();
        std::fs::write(data.path().join(AGENCIES), tabular_fixtures::agencies_csv()).unwrap();
        std::fs::write(
            data.path().join(COMMANDES),
            "reference;client;montant\nCMD-2026-001;Client 1;110\nCMD-2026-002;Client 2;120\n",
        )
        .unwrap();
        for year in ["2025", "2026"] {
            std::fs::create_dir_all(data.path().join(year)).unwrap();
            std::fs::write(
                data.path().join(year).join("ventes.csv"),
                "mois;montant\njanvier;10\nfevrier;20\n",
            )
            .unwrap();
        }
        let mut index = IndexStore::open_at(&app.path().join("index.sqlite3")).unwrap();
        data_folder::analyse(data.path(), &mut index, "fr-FR", &|_| {}).unwrap();
        Self {
            data,
            _app: app,
            index,
        }
    }

    fn open(&self) -> DataFolder {
        DataFolder::discover(self.data.path(), Some(&self.index), &FileHashCache::new()).unwrap()
    }

    /// The named workbooks, ticked on purpose.
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

    /// Door 4, as `commands::mixed_tier` calls it: the cards of the question, from what is ticked,
    /// then the trace of an answer the model wrote.
    fn door_four(&self, question: &str, locale: &str, selection: &ScopeMode) -> Vec<WorkbookUse> {
        let folder = self.open();
        let cards = tabular_answer::question_cards(
            question,
            &folder,
            selection,
            locale,
            &[],
            card_budget_chars(&ContextBudget::DEFAULT),
        );
        workbook_use::trace(
            &tabular_answer::selection_facts(question, &folder, selection),
            Answered::Mixed {
                cards: &cards.block.cards,
                implied: &cards.implied,
                table: &None,
                model_wrote: true,
                samples: &[],
            },
        )
    }
}

fn hows(trace: &[WorkbookUse]) -> Vec<(String, WorkbookUseHow, Option<UseReason>)> {
    trace
        .iter()
        .map(|entry| (entry.file.clone(), entry.how, entry.reason))
        .collect()
}

#[test]
fn door_four_a_named_workbook_gets_its_card_and_the_others_are_counted() {
    let folders = Folders::build();
    let selection = folders.ticked(&[FACTURES, AGENCIES, COMMANDES]);
    let trace = folders.door_four("Que contient factures.csv ?", "fr-FR", &selection);
    assert_eq!(
        hows(&trace),
        vec![
            (FACTURES.to_string(), WorkbookUseHow::CardSent, None),
            (
                String::new(),
                WorkbookUseHow::NotUsed,
                Some(UseReason::NotRelevant)
            ),
        ]
    );
    assert_eq!(trace[1].count, Some(2));
}

#[test]
fn door_four_a_table_word_implies_every_ticked_workbook_and_nothing_is_left_out() {
    let folders = Folders::build();
    let selection = folders.ticked(&[FACTURES, COMMANDES]);
    let trace = folders.door_four("Que peux-tu me dire de ce tableur ?", "fr-FR", &selection);
    assert_eq!(
        hows(&trace),
        vec![
            (
                FACTURES.to_string(),
                WorkbookUseHow::CardSent,
                Some(UseReason::ImpliedByWord)
            ),
            (
                COMMANDES.to_string(),
                WorkbookUseHow::CardSent,
                Some(UseReason::ImpliedByWord)
            ),
        ]
    );
}

#[test]
fn door_four_a_question_that_concerns_no_ticked_workbook_says_how_many_were_not_used() {
    let folders = Folders::build();
    let selection = folders.ticked(&[FACTURES, AGENCIES, COMMANDES]);
    let trace = folders.door_four("Quand le contrat a-t-il ete signe ?", "fr-FR", &selection);
    assert_eq!(trace.len(), 1);
    assert_eq!(trace[0].reason, Some(UseReason::NotRelevant));
    assert_eq!(trace[0].count, Some(3));
}

#[test]
fn door_four_a_single_ticked_workbook_is_implied_and_has_no_aggregate() {
    let folders = Folders::build();
    let selection = folders.ticked(&[FACTURES]);
    let trace = folders.door_four("Quand le contrat a-t-il ete signe ?", "fr-FR", &selection);
    assert_eq!(
        hows(&trace),
        vec![(
            FACTURES.to_string(),
            WorkbookUseHow::CardSent,
            Some(UseReason::ImpliedSingle)
        )]
    );
}

#[test]
fn door_three_a_value_computed_from_the_named_workbook_is_traced() {
    let folders = Folders::build();
    let selection = folders.ticked(&[FACTURES, AGENCIES]);
    let question = "Quel est le total du montant de factures.csv ?";
    let folder = folders.open();
    let pending = tabular_answer::prepare_if_data_only(
        question,
        &folder,
        &selection,
        "fr-FR",
        &folders.index,
    )
    .unwrap()
    .expect("door 3 answers a data-only question");
    let PendingAnswer::Done(answer) = pending else {
        panic!("a recognised operation is a finished answer");
    };
    assert!(matches!(answer, TabularAnswer::Value { .. }));
    let trace = workbook_use::trace(
        &tabular_answer::selection_facts(question, &folder, &selection),
        Answered::Engine(&answer),
    );
    assert_eq!(
        hows(&trace),
        vec![
            (FACTURES.to_string(), WorkbookUseHow::Computed, None),
            (
                String::new(),
                WorkbookUseHow::NotUsed,
                Some(UseReason::NotRelevant)
            ),
        ]
    );
    // `agencies.csv` is ticked and was not used.
    assert_eq!(trace[1].count, Some(1));
}

#[test]
fn the_tables_only_tier_says_a_named_workbook_that_is_not_ticked_was_not_used() {
    let folders = Folders::build();
    let selection = folders.ticked(&[AGENCIES, COMMANDES]);
    let question = "Quel est le total du montant de factures.csv ?";
    let folder = folders.open();
    let pending = tabular_answer::prepare(
        question,
        &folder,
        &selection,
        "fr-FR",
        &folders.index,
        false,
    )
    .unwrap();
    let PendingAnswer::Done(answer) = pending else {
        panic!("a name the selection does not hold is a finished answer");
    };
    assert!(matches!(answer, TabularAnswer::FileNotSelected { .. }));
    let trace = workbook_use::trace(
        &tabular_answer::selection_facts(question, &folder, &selection),
        Answered::Engine(&answer),
    );
    // The name as the user typed it; the two ticked workbooks the question has nothing to do with
    // are counted after it.
    assert_eq!(trace[0].file, FACTURES);
    assert_eq!(trace[0].reason, Some(UseReason::NamedNotSelected));
    assert_eq!(trace[1].reason, Some(UseReason::NotRelevant));
    assert_eq!(trace[1].count, Some(2));
}

#[test]
fn a_name_that_two_ticked_workbooks_carry_lists_both_and_uses_neither() {
    let folders = Folders::build();
    let selection = folders.ticked(&["2025/ventes.csv", "2026/ventes.csv", COMMANDES]);
    let question = "Quel est le total du montant de ventes.csv ?";
    let folder = folders.open();
    let facts = tabular_answer::selection_facts(question, &folder, &selection);
    assert_eq!(facts.named.ambiguous.len(), 1, "{facts:?}");
    let pending = tabular_answer::prepare(
        question,
        &folder,
        &selection,
        "fr-FR",
        &folders.index,
        false,
    )
    .unwrap();
    let PendingAnswer::Done(answer) = pending else {
        panic!("an ambiguous name is a finished answer");
    };
    assert!(matches!(answer, TabularAnswer::AmbiguousReference { .. }));
    let trace = workbook_use::trace(&facts, Answered::Engine(&answer));
    assert_eq!(trace[0].reason, Some(UseReason::AmbiguousName));
    assert_eq!(trace[0].candidates.len(), 2);
    // Only `commandes.csv` was left out.
    assert_eq!(trace[1].count, Some(1));
}

#[test]
fn door_one_traces_the_workbook_the_plan_was_prepared_from() {
    let folders = Folders::build();
    let selection = folders.ticked(&[FACTURES]);
    let question = "Ecris une lettre pour chaque facture";
    let folder = folders.open();
    let source = tabular_answer::locate_fill_source(
        question,
        &folder,
        &selection,
        "fr-FR",
        &folders.index,
        true,
    )
    .unwrap()
    .expect("the one ticked workbook is the source");
    let trace = workbook_use::trace(
        &tabular_answer::selection_facts(question, &folder, &selection),
        Answered::Plan(&source.file),
    );
    assert_eq!(
        hows(&trace),
        vec![(FACTURES.to_string(), WorkbookUseHow::PlanPrepared, None)]
    );
}

#[test]
fn door_two_still_says_a_named_workbook_that_is_not_ticked() {
    let folders = Folders::build();
    let selection = folders.ticked(&[AGENCIES, COMMANDES]);
    let question = "Combien de documents parlent de factures.csv ?";
    let trace = workbook_use::trace(
        &tabular_answer::selection_facts(question, &folders.open(), &selection),
        Answered::FolderSearch,
    );
    assert_eq!(
        hows(&trace),
        vec![(
            FACTURES.to_string(),
            WorkbookUseHow::NotUsed,
            Some(UseReason::NamedNotSelected)
        )]
    );
}

#[test]
fn a_workbook_that_changed_or_vanished_since_it_was_ticked_is_reported_not_skipped() {
    let folders = Folders::build();
    let mut selection = folders.ticked(&[FACTURES, AGENCIES]);
    let ScopeMode::Explicit(entries) = &mut selection else {
        panic!("an explicit selection");
    };
    // The content `agencies.csv` had when it was ticked is no longer there.
    entries[1].pinned_id = "another-content".to_string();
    // A ticked workbook whose file is gone.
    entries.push(ScopeEntry {
        relative_path: "disparu.csv".to_string(),
        pinned_id: "nothing-has-this-content".to_string(),
        added_at: 1,
        sheet_names: Vec::new(),
    });
    let facts =
        tabular_answer::selection_facts("Que contient factures.csv ?", &folders.open(), &selection);
    assert_eq!(facts.usable, vec![FACTURES.to_string()]);
    let trace = workbook_use::trace(&facts, Answered::FolderSearch);
    assert_eq!(
        hows(&trace),
        vec![
            (
                AGENCIES.to_string(),
                WorkbookUseHow::NotUsed,
                Some(UseReason::ChangedSinceAnalysis)
            ),
            (
                "disparu.csv".to_string(),
                WorkbookUseHow::NotUsed,
                Some(UseReason::ChangedSinceAnalysis)
            ),
        ]
    );
}

#[test]
fn a_ticked_workbook_that_was_never_analysed_is_reported() {
    let folders = Folders::build();
    // Dropped in after the analysis, then ticked as it is: the file is there, nothing is known of it.
    std::fs::write(folders.data.path().join("nouveau.csv"), "a;b\n1;2\n").unwrap();
    let selection = folders.ticked(&[FACTURES, "nouveau.csv"]);
    let facts =
        tabular_answer::selection_facts("Que contient factures.csv ?", &folders.open(), &selection);
    assert_eq!(facts.usable, vec![FACTURES.to_string()]);
    assert_eq!(
        facts.unusable,
        vec![(
            "nouveau.csv".to_string(),
            tabular_answer::Unusable::NotAnalysed
        )]
    );
}

#[test]
fn a_ticked_workbook_that_changed_is_not_also_reported_as_not_ticked_when_the_question_names_it() {
    let folders = Folders::build();
    let mut selection = folders.ticked(&[FACTURES, AGENCIES]);
    let ScopeMode::Explicit(entries) = &mut selection else {
        panic!("an explicit selection");
    };
    entries[1].pinned_id = "another-content".to_string();
    let question = "Quel est le total du montant de agencies.csv ?";
    let folder = folders.open();
    // Without the correction `named_workbooks` calls it a workbook that is not ticked.
    assert_eq!(
        tabular_answer::named_workbooks(question, &folder, &selection).not_selected,
        vec![AGENCIES.to_string()]
    );
    let facts = tabular_answer::selection_facts(question, &folder, &selection);
    assert!(facts.named.not_selected.is_empty(), "{facts:?}");
    let trace = workbook_use::trace(&facts, Answered::FolderSearch);
    assert_eq!(
        hows(&trace),
        vec![(
            AGENCIES.to_string(),
            WorkbookUseHow::NotUsed,
            Some(UseReason::ChangedSinceAnalysis)
        )]
    );
}
