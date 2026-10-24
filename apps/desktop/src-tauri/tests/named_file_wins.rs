//! TABULAR step 08 (`docs/SESSION-TABULAR/SESSION-TABULAR-08-named-file-wins.md`): the workbook a
//! question names wins when a letter is asked for each row, the program never lets a model improvise
//! the letters it could not prepare, and the sheet a plan was built from is read again by the
//! preview and the generation.
//!
//! Exercised one level below the Tauri command (`fill_tier` needs an `AppHandle`), at the calls it
//! makes: `named_workbooks`, `locate_fill_source`, `fill_plan::decide` and `load_fill_table`. None of
//! them has a gateway to call: a letter the program cannot prepare is a deterministic answer. Every
//! workbook is written at test time into a temporary folder (`tests/common/tabular_fixtures.rs`). Case I
//! (a document named in a mixed selection is targeted) is tested through `mixed_answer::named_document`
//! and `folder_facts_block`, which `commands::mixed_tier` calls.

mod common;

use assistant_cabinet_ai_lib::analysis_scope::{ScopeEntry, ScopeMode};
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::fill_plan::{decide, FillDecision};
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::{FileHashCache, WorkFolderInventory};
use assistant_cabinet_ai_lib::knowledge::diagnostics::take_workbook_loads;
use assistant_cabinet_ai_lib::mixed_answer::{folder_facts_block, named_document};
use assistant_cabinet_ai_lib::tabular::question::has_distributive_word;
use assistant_cabinet_ai_lib::tabular_answer::{self, FillLocation, FillSheet, TabularAnswer};
use common::tabular_fixtures::{
    self, AGENCIES, DEPENSES, FACTURES, MIXTE, MIXTE_MONTANTS, RENDEZ_VOUS,
};

const LOCALE: &str = "en-US";
const LETTER_FOR_INVOICES: &str = "lettre-factures.txt";
const ARCHIVE: &str = "archive/factures.csv";

struct Folders {
    data: tempfile::TempDir,
    documents: tempfile::TempDir,
    _app: tempfile::TempDir,
    index: IndexStore,
}

impl Folders {
    fn build() -> Self {
        let data = tempfile::tempdir().unwrap();
        let documents = tempfile::tempdir().unwrap();
        let app = tempfile::tempdir().unwrap();
        tabular_fixtures::write_all(data.path());
        // The same invoices kept in a second folder: `factures.csv` is then a name two files carry.
        std::fs::create_dir_all(data.path().join("archive")).unwrap();
        std::fs::copy(data.path().join(FACTURES), data.path().join(ARCHIVE)).unwrap();
        // Two templates: the first fits the columns of `factures.csv`, the second those of the
        // `Donnees` sheet of `mixte.xlsx`.
        std::fs::write(
            documents.path().join(LETTER_FOR_INVOICES),
            "Hello [fournisseur], your invoice of [montant] is due.\n",
        )
        .unwrap();
        std::fs::write(
            documents.path().join("lettre-articles.txt"),
            "Order of [article], quantity [quantite].\n",
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

    fn templates(&self) -> WorkFolderInventory {
        WorkFolderInventory::discover(self.documents.path(), None).unwrap()
    }

    fn no_templates(&self) -> WorkFolderInventory {
        WorkFolderInventory::from_records(self.documents.path(), Vec::new())
    }

    /// The workbooks ticked on purpose, each narrowed to the sheets given (none: every sheet).
    fn ticked(&self, entries: &[(&str, &[&str])]) -> ScopeMode {
        let folder = self.open();
        ScopeMode::Explicit(
            entries
                .iter()
                .map(|(name, sheets)| {
                    let record = folder.files().find_by_relative_path(name).unwrap();
                    ScopeEntry {
                        relative_path: name.to_string(),
                        pinned_id: record.id.clone(),
                        added_at: 1,
                        sheet_names: sheets.iter().map(|sheet| sheet.to_string()).collect(),
                    }
                })
                .collect(),
        )
    }

    fn every_workbook(&self) -> ScopeMode {
        self.ticked(&[
            (FACTURES, &[]),
            (AGENCIES, &[]),
            (RENDEZ_VOUS, &[]),
            (MIXTE, &[]),
            (DEPENSES, &[]),
            (MIXTE_MONTANTS, &[]),
        ])
    }

    /// Door 1's lookup exactly as `commands::fill_tier` makes it, and the names it used.
    fn locate(
        &self,
        question: &str,
        selection: &ScopeMode,
    ) -> (FillLocation, tabular_answer::NamedWorkbooks, bool) {
        let data = self.open();
        let every_row = has_distributive_word(question, LOCALE);
        let named = tabular_answer::named_workbooks(question, &data, selection);
        let located = tabular_answer::locate_fill_source(
            question,
            &data,
            selection,
            LOCALE,
            &self.index,
            every_row,
            &named,
        )
        .unwrap();
        (located, named, every_row)
    }
}

#[test]
fn a_workbook_narrowed_to_one_sheet_is_read_again_from_that_sheet() {
    // Reproduced before the fix: the plan is built under the sheet restriction of the selection, then
    // the preview re-read the workbook with no restriction, and a workbook of two sheets has no single
    // sheet to read: the card appeared and its preview failed with `FillSourceUnavailable`.
    let folders = Folders::build();
    let selection = folders.ticked(&[(MIXTE, &["Donnees"])]);
    let (located, _, _) = folders.locate("Write a letter for each row of mixte.xlsx", &selection);
    let FillLocation::Found(source) = located else {
        panic!("narrowed to one sheet, the workbook is a source: {located:?}");
    };
    assert_eq!(source.sheet, "Donnees");

    let data = folders.open();
    let (sheet, table) = tabular_answer::load_fill_table(
        &data,
        &source.file,
        Some(&source.sheet),
        LOCALE,
        &folders.index,
    )
    .expect("the preview reads the sheet the plan was built from");
    assert_eq!(sheet, "Donnees");
    assert_eq!(table.columns, source.table.columns);

    // The sheet of a request is looked up in the workbook, never trusted: one the workbook does not
    // hold is refused, and a request with no sheet still has no single sheet to read.
    assert!(
        tabular_answer::load_fill_table(&data, MIXTE, Some("Absente"), LOCALE, &folders.index)
            .is_none()
    );
    assert!(tabular_answer::load_fill_table(&data, MIXTE, None, LOCALE, &folders.index).is_none());
    assert_eq!(
        tabular_answer::load_fill_table(&data, MIXTE, Some("donnees"), LOCALE, &folders.index)
            .map(|(name, _)| name),
        Some("Donnees".to_string()),
        "the sheet is matched as the engine matches it, accents and case ignored"
    );
}

#[test]
fn the_named_workbook_beats_every_other_single_sheet_workbook() {
    // Case H of the plan: "a letter for each invoice of factures.csv" with everything ticked. Every
    // single-sheet workbook used to be a candidate because "each" sets `every_row`; the name decides
    // now, and only the named workbook is opened.
    let folders = Folders::build();
    let selection = folders.every_workbook();
    let question = "Write a letter for each invoice of factures.csv";
    take_workbook_loads();
    let (located, named, every_row) = folders.locate(question, &selection);
    let opened = take_workbook_loads().loads;

    assert!(every_row);
    assert_eq!(named.exact, vec![FACTURES.to_string()]);
    assert_eq!(opened, 1, "the others are ignored, not loaded for nothing");
    let FillDecision::Plan(plan) =
        decide(located, &named, every_row, &folders.templates()).unwrap()
    else {
        panic!("a plan is prepared from the named workbook");
    };
    assert_eq!(plan.data_file, FACTURES);
    assert_eq!(plan.template, LETTER_FOR_INVOICES);
    assert_eq!(plan.key_value, None, "every row, no identifier");

    // The same request with no name keeps today's behaviour: several workbooks qualify, none is picked.
    let (located, _, _) = folders.locate("Write a letter for each row", &selection);
    assert!(matches!(located, FillLocation::NoSource), "{located:?}");
}

#[test]
fn two_named_workbooks_for_a_merge_are_asked_about_and_never_picked() {
    let folders = Folders::build();
    let selection = folders.every_workbook();
    let question = "Write a letter for each row of agencies.csv and factures.csv";
    let (located, named, every_row) = folders.locate(question, &selection);
    let FillLocation::SeveralSources(candidates) = &located else {
        panic!("two names for a merge are asked about: {located:?}");
    };
    assert_eq!(
        candidates,
        &vec![AGENCIES.to_string(), FACTURES.to_string()],
        "in the order written"
    );

    let FillDecision::Ask(TabularAnswer::FillSeveralSources { candidates, query }) =
        decide(located, &named, every_row, &folders.templates()).unwrap()
    else {
        panic!("the decision asks which workbook");
    };
    assert_eq!(candidates.len(), 2);
    assert_eq!(
        query, None,
        "two different names: no single button settles it"
    );

    // Two names in a free writing request are not a merge request: step 05's advisory reading and the
    // writer keep them.
    let (located, _, _) = folders.locate(
        "Write a mail comparing factures.csv and agencies.csv",
        &selection,
    );
    assert!(matches!(located, FillLocation::NoSource), "{located:?}");
}

#[test]
fn a_named_workbook_with_several_sheets_asks_which_one_and_lists_them_all() {
    let folders = Folders::build();
    let selection = folders.every_workbook();
    let question = "Write a letter for each row of mixte.xlsx";
    let (located, named, every_row) = folders.locate(question, &selection);
    let FillLocation::SheetToChoose { file, sheets } = &located else {
        panic!("a letter is never filled from a sheet picked for her: {located:?}");
    };
    assert_eq!(file, MIXTE);
    assert_eq!(
        sheets,
        &vec![
            FillSheet {
                name: "Donnees".to_string(),
                looks_tabular: true
            },
            FillSheet {
                name: "Calculs".to_string(),
                looks_tabular: false
            },
        ],
        "the sheet that holds no data table is listed too, marked as such"
    );
    let FillDecision::Ask(TabularAnswer::FillNeedsSheet { file, .. }) =
        decide(located, &named, every_row, &folders.templates()).unwrap()
    else {
        panic!("the decision asks which sheet");
    };
    assert_eq!(file, MIXTE);

    // The sheet she names in the question is her choice, not one picked for her; the file name alone
    // names no sheet.
    let (located, _, _) = folders.locate(
        "Write a letter for each row of the sheet Donnees of mixte.xlsx",
        &selection,
    );
    let FillLocation::Found(source) = located else {
        panic!("a named sheet is the source: {located:?}");
    };
    assert_eq!(
        (source.file.as_str(), source.sheet.as_str()),
        (MIXTE, "Donnees")
    );
    // A sheet that holds no data table is not filled from, even when it is named.
    let (located, _, _) = folders.locate(
        "Write a letter for each row of the sheet Calculs of mixte.xlsx",
        &selection,
    );
    assert!(
        matches!(located, FillLocation::SheetToChoose { .. }),
        "{located:?}"
    );

    // An editorial request about the same workbook is not a merge request.
    let (located, _, _) = folders.locate("Write a mail about mixte.xlsx", &selection);
    assert!(matches!(located, FillLocation::NoSource), "{located:?}");
}

#[test]
fn a_merge_with_no_template_has_one_drafted_and_never_reaches_the_writer() {
    let folders = Folders::build();
    let selection = folders.every_workbook();
    let question = "Write a reminder letter for each invoice of depenses.csv";
    let (located, named, every_row) = folders.locate(question, &selection);
    // No template among the documents: the decision is made without a gateway at all, and it hands the
    // source over to the draft (TABULAR step 09) instead of to the writer.
    let FillDecision::NeedsDraft(source) =
        decide(located, &named, every_row, &folders.no_templates()).unwrap()
    else {
        panic!("a named workbook and no template that fits: a template is drafted");
    };
    assert_eq!(source.file, DEPENSES);

    // The workbook is only implied (named nowhere): no template keeps the question editorial, as before.
    let single = folders.ticked(&[(FACTURES, &[])]);
    let (located, named, every_row) = folders.locate("Write a letter for each invoice", &single);
    assert!(named.is_empty());
    assert!(matches!(located, FillLocation::Found(_)), "{located:?}");
    assert!(matches!(
        decide(located, &named, every_row, &folders.no_templates()).unwrap(),
        FillDecision::Editorial
    ));
}

#[test]
fn a_named_workbook_that_is_not_ticked_is_never_replaced_by_another() {
    let folders = Folders::build();
    let selection = folders.ticked(&[(AGENCIES, &[])]);
    let question = "Write a letter for each invoice of factures.csv";
    let (located, named, every_row) = folders.locate(question, &selection);
    assert_eq!(named.not_selected, vec!["factures.csv".to_string()]);
    assert!(
        matches!(located, FillLocation::NoSource),
        "agencies.csv must not stand in: {located:?}"
    );
    let FillDecision::Ask(TabularAnswer::FileNotSelected { query }) =
        decide(located, &named, every_row, &folders.templates()).unwrap()
    else {
        panic!("the engine's own sentence says to tick it");
    };
    assert_eq!(query, "factures.csv");
}

#[test]
fn a_file_written_in_full_is_not_made_ambiguous_by_its_own_name() {
    // Two folders hold `factures.csv`. Written without its extension, the name is asked about; written
    // with its folder, it is that file (the same words also read as the ambiguous name, which must not
    // count twice).
    let folders = Folders::build();
    let selection = folders.ticked(&[(FACTURES, &[]), (ARCHIVE, &[]), (AGENCIES, &[])]);
    let (located, named, every_row) = folders.locate(
        "Write a letter for each invoice of archive/factures.csv",
        &selection,
    );
    assert!(
        !named.ambiguous.is_empty(),
        "the stem of the path reads as the ambiguous name too"
    );
    let FillLocation::Found(source) = located else {
        panic!("the path written in full names one file: {located:?}");
    };
    assert_eq!(source.file, ARCHIVE);
    assert!(every_row);

    let (located, named, every_row) =
        folders.locate("Write a letter for each invoice of factures", &selection);
    let FillLocation::SeveralSources(candidates) = &located else {
        panic!("the name two files carry is asked about: {located:?}");
    };
    assert_eq!(candidates.len(), 2, "{candidates:?}");
    assert!(
        candidates.contains(&FACTURES.to_string()) && candidates.contains(&ARCHIVE.to_string())
    );
    // One name two files carry: the interface replaces it by the file she picks.
    let FillDecision::Ask(TabularAnswer::FillSeveralSources { query, .. }) =
        decide(located, &named, every_row, &folders.templates()).unwrap()
    else {
        panic!("the decision asks which workbook");
    };
    assert_eq!(query.as_deref(), Some("factures"));
}

#[test]
fn a_writing_request_without_a_merge_sign_is_left_to_the_writer() {
    // Case L stays editorial: one name, no distributive word, no identifier.
    let folders = Folders::build();
    let selection = folders.every_workbook();
    let question = "Write a mail to Alpha about his unpaid invoices in factures.csv";
    let (located, named, every_row) = folders.locate(question, &selection);
    assert!(!every_row);
    assert!(matches!(located, FillLocation::NoSource), "{located:?}");
    assert!(matches!(
        decide(located, &named, every_row, &folders.templates()).unwrap(),
        FillDecision::Editorial
    ));
}

// --- Case I: a document named in a mixed selection is targeted ----------------------------------

/// Documents of a work folder, written at test time: one name that is unique, one that two folders
/// carry. Nothing is read from `fixtures/`.
fn documents_folder() -> (tempfile::TempDir, WorkFolderInventory, Vec<String>) {
    let root = tempfile::tempdir().unwrap();
    for (path, text) in [
        ("notes.txt", "The rent is paid on the first of the month."),
        ("Contrats/2025/contrat.txt", "Signed in 2025."),
        ("Contrats/2026/contrat.txt", "Signed in 2026."),
    ] {
        let file = root.path().join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
    let inventory = WorkFolderInventory::discover(root.path(), None).unwrap();
    let readable = inventory
        .all_files()
        .iter()
        .map(|file| file.relative_path.clone())
        .collect();
    (root, inventory, readable)
}

#[test]
fn a_document_named_exactly_is_targeted_and_the_folder_facts_carry_only_that_file() {
    let (_root, inventory, readable) = documents_folder();
    let named = named_document(
        "What does notes.txt say about the rent?",
        &inventory,
        &readable,
    );
    assert_eq!(named.targeted.as_deref(), Some("notes.txt"));
    assert!(named.names_a_document);

    let facts = folder_facts_block(&inventory, "notes.txt");
    assert!(
        facts.starts_with("Folder facts:\nWORK_FOLDER_CONTEXT\n"),
        "{facts}"
    );
    assert!(
        facts.contains("\"view\":\"targeted\"") && facts.contains("notes.txt"),
        "{facts}"
    );
    // The other documents are absent, and so is the text of the targeted one: facts, not content.
    assert!(
        !facts.contains("contrat.txt") && !facts.contains("2025"),
        "{facts}"
    );
    assert!(!facts.contains("rent is paid"), "{facts}");
}

#[test]
fn an_ambiguous_name_or_one_outside_the_selection_keeps_the_whole_selection() {
    let (_root, inventory, readable) = documents_folder();
    // Two folders carry `contrat.txt`: asked about elsewhere, never picked, and the search stays wide.
    let ambiguous = named_document("What does contrat.txt say?", &inventory, &readable);
    assert!(ambiguous.names_a_document);
    assert_eq!(ambiguous.targeted, None);
    // A file name no selected document carries is still a name (never reduced by the knowledge base),
    // and it targets nothing.
    let outside = named_document("What does autre.txt say?", &inventory, &readable);
    assert!(outside.names_a_document);
    assert_eq!(outside.targeted, None);
    // A question that names no file at all is searched as a whole, as before.
    let none = named_document("What is the rent?", &inventory, &readable);
    assert!(!none.names_a_document);
    assert_eq!(none.targeted, None);
    // An exact name whose document could not be read has nothing to search.
    let unread = named_document("What does notes.txt say?", &inventory, &[]);
    assert!(unread.names_a_document);
    assert_eq!(unread.targeted, None);
}

#[test]
fn a_workbook_written_in_full_is_advisory_even_when_a_same_named_one_is_ticked() {
    // S12-1 (found by the wave-1 regression walk): with `factures.csv` and `archive/factures.csv` both
    // ticked, a description of `archive/factures.csv` is a question about that one file. The bare name
    // `factures.csv` that the path also reads as is not a second, ambiguous name.
    let folders = Folders::build();
    let both = folders.ticked(&[(FACTURES, &[]), (ARCHIVE, &[])]);
    let advisory = |question: &str| {
        tabular_answer::advisory_target(question, &folders.open(), &both, LOCALE, &[])
    };

    let target = advisory("Describe archive/factures.csv").expect("the file written in full wins");
    assert_eq!(target.files, vec![ARCHIVE.to_string()]);
}
