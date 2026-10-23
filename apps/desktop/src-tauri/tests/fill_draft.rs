//! TABULAR step 09 (`docs/SESSION-TABULAR/SESSION-TABULAR-09-mail-merge-drafted-template.md`): a letter
//! is asked for each row of a workbook the question names and no template fits, so the model drafts
//! the template from the column names alone and the program fills it.
//!
//! Exercised one level below the Tauri command (`fill_tier` needs an `AppHandle`), at the calls it
//! makes: `named_workbooks`, `locate_fill_source`, `decide`, `draft_decision`, then the preview and the
//! generation. The workbook is written at test time with distinctive cell values, so that "no value
//! reaches the model" is asserted on the request itself. A test that opens a socket runs alone:
//! `cargo test --test fill_draft -- --test-threads=1`.

mod common;

use std::path::Path;
use std::time::Duration;

use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
use assistant_cabinet_ai_lib::data_folder::{self, DataFolder};
use assistant_cabinet_ai_lib::error::AppError;
use assistant_cabinet_ai_lib::fill_draft::{DraftContext, MAX_DRAFT_BYTES};
use assistant_cabinet_ai_lib::fill_plan::{
    decide, draft_decision, generate, preview, without_output_folder, FillDecision, FillPlan,
    FillRequest, RequestMapping, TemplateFile,
};
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::index_store::IndexStore;
use assistant_cabinet_ai_lib::inventory::{FileHashCache, WorkFolderInventory};
use assistant_cabinet_ai_lib::tabular::question::has_distributive_word;
use assistant_cabinet_ai_lib::tabular_answer::{self, FillLocation, FillSource, TabularAnswer};
use common::counting_gateway::{
    start_counting_gateway, start_gateway_replying, start_gateway_replying_in_turn, CountingGateway,
};
use common::tabular_fixtures::{write_xlsx, Cell};

const LOCALE: &str = "en-US";
const WORKBOOK: &str = "invoices.xlsx";
const QUESTION: &str = "Write a friendly reminder letter for each invoice of invoices.xlsx";

/// Values nothing else in the test spells: each is asserted absent from what the model is sent.
const ROWS: [[&str; 4]; 3] = [
    ["FAC-Z9Q-001", "Zorglub Industries", "7731,92", "impayee-qx"],
    ["FAC-Z9Q-002", "Blorbo Textiles", "412,08", "relancee-qx"],
    ["FAC-Z9Q-003", "Quuxville Marine", "1990,45", "impayee-qx"],
];

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
        let mut sheet: Vec<Vec<Cell>> =
            vec![
                ["N\u{b0} facture", "Fournisseur", "Montant (TTC)", "Statut"]
                    .iter()
                    .map(|header| Cell::text(header))
                    .collect(),
            ];
        for row in ROWS {
            sheet.push(row.iter().map(|cell| Cell::text(cell)).collect());
        }
        write_xlsx(
            &data.path().join(WORKBOOK),
            &[("Factures".to_string(), sheet)],
        );
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

    fn no_templates(&self) -> WorkFolderInventory {
        WorkFolderInventory::from_records(self.documents.path(), Vec::new())
    }

    /// Door 1 as `commands::fill_tier` makes it, up to the decision to draft.
    fn source(&self, question: &str) -> FillSource {
        let data = self.open();
        let every_row = has_distributive_word(question, LOCALE);
        let named = tabular_answer::named_workbooks(question, &data, &ScopeMode::WholeFolder);
        let located = tabular_answer::locate_fill_source(
            question,
            &data,
            &ScopeMode::WholeFolder,
            LOCALE,
            &self.index,
            every_row,
            &named,
        )
        .unwrap();
        assert!(matches!(located, FillLocation::Found(_)), "{located:?}");
        match decide(located, &named, every_row, &self.no_templates()).unwrap() {
            FillDecision::NeedsDraft(source) => source,
            _ => panic!("a named workbook and no template that fits: a template is drafted"),
        }
    }
}

async fn drafted(gateway_url: &str, source: FillSource, question: &str) -> FillDecision {
    let client = GatewayClient::new().unwrap();
    let context = DraftContext {
        locale: "fr-FR",
        gateway: &client,
        server_url: gateway_url,
        model_alias: "model",
        idle_timeout: Duration::from_secs(10),
    };
    draft_decision(&context, &[], question, source)
        .await
        .unwrap()
}

fn plan_of(decision: FillDecision) -> FillPlan {
    match decision {
        FillDecision::Plan(plan) => plan,
        FillDecision::Ask(answer) => panic!("a plan was expected, not {answer:?}"),
        _ => panic!("a plan was expected"),
    }
}

fn hanging_up_gateway_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("binds a free port");
    let url = format!("http://{}", listener.local_addr().expect("a local address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            drop(stream);
        }
    });
    url
}

const DRAFT: &str =
    "Dear {{Fournisseur}},\n\nInvoice {{N facture}} of {{Montant TTC}} is {{Statut}}.\n";

fn request_of(plan: &FillPlan, all: bool) -> FillRequest {
    FillRequest {
        template: plan.template.clone(),
        draft_text: plan.draft_text.clone(),
        data_file: plan.data_file.clone(),
        sheet: Some(plan.sheet.clone()),
        key_column: plan.key_column.clone(),
        key_value: plan.key_value.clone(),
        all,
        mapping: plan
            .placeholders
            .iter()
            .map(|placeholder| RequestMapping {
                placeholder: placeholder.name.clone(),
                column: placeholder.column.clone(),
            })
            .collect(),
    }
}

fn files_in(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[tokio::test]
async fn a_named_workbook_with_no_template_gets_a_drafted_plan_and_nothing_is_written() {
    let folders = Folders::build();
    let source = folders.source(QUESTION);
    let table = source.table.clone();
    let gateway = start_gateway_replying(DRAFT);

    let plan = plan_of(drafted(&gateway.url, source, QUESTION).await);

    assert_eq!(gateway.calls(), 1, "one gateway call drafts the template");
    assert_eq!(plan.template_origin, "drafted");
    assert_eq!(plan.draft_text.as_deref(), Some(DRAFT.trim()));
    assert_eq!(plan.template, "");
    assert!(plan.templates.is_empty());
    assert_eq!(plan.data_file, WORKBOOK);
    assert_eq!(plan.sheet, "Factures");
    // Every field is one of the program's own names for a column: exact, no confirmation to wait for.
    let columns: Vec<(&str, Option<&str>, &str)> = plan
        .placeholders
        .iter()
        .map(|each| (each.name.as_str(), each.column.as_deref(), each.status))
        .collect();
    assert_eq!(
        columns,
        [
            ("Fournisseur", Some("Fournisseur"), "exact"),
            ("N facture", Some("N\u{b0} facture"), "exact"),
            ("Montant TTC", Some("Montant (TTC)"), "exact"),
            ("Statut", Some("Statut"), "exact"),
        ]
    );
    // The program filled the first letter from the first row; one letter per row was asked.
    assert_eq!(plan.preview.letters, ROWS.len());
    assert!(
        plan.preview.text.contains("Dear Zorglub Industries")
            && plan
                .preview
                .text
                .contains("Invoice FAC-Z9Q-001 of 7731,92 is impayee-qx"),
        "{}",
        plan.preview.text
    );
    assert!(plan.preview.unresolved.is_empty());
    assert_eq!(plan.preview.file_name, "invoices-letter-1.txt");
    assert!(
        files_in(folders.documents.path()).is_empty(),
        "a question writes nothing: {:?}",
        files_in(folders.documents.path())
    );

    // What the model was sent: the field names, the user's request, and no value of the table.
    let sent = gateway.requests().remove(0);
    for name in [
        "{{Fournisseur}}",
        "{{N facture}}",
        "{{Montant TTC}}",
        "{{Statut}}",
    ] {
        assert!(sent.contains(name), "{name} is listed: {sent}");
    }
    assert!(sent.contains(QUESTION), "{sent}");
    for cell in table.rows.iter().flatten() {
        assert!(
            !sent.contains(cell.as_str()),
            "the value {cell:?} reached the model:\n{sent}"
        );
    }
}

#[tokio::test]
async fn an_altered_name_is_a_proposal_and_an_unknown_one_stays_unresolved_and_shown() {
    let folders = Folders::build();
    let gateway = start_gateway_replying(
        "Dear {{fournisseur}}, {{Montant TTC}} ({{Statu}}) ref {{Invented}}",
    );
    let plan = plan_of(drafted(&gateway.url, folders.source(QUESTION), QUESTION).await);

    let status = |name: &str| {
        plan.placeholders
            .iter()
            .find(|each| each.name == name)
            .map(|each| (each.column.clone(), each.status))
            .unwrap()
    };
    assert_eq!(
        status("fournisseur"),
        (Some("Fournisseur".to_string()), "exact")
    );
    assert_eq!(status("Statu"), (Some("Statut".to_string()), "proposed"));
    assert_eq!(status("Invented"), (None, "unresolved"));
    // Neither the proposal nor the unknown name is filled; both are said, and left as written.
    assert_eq!(plan.preview.unresolved, ["Statu", "Invented"]);
    assert!(plan.preview.text.contains("{{Statu}}") && plan.preview.text.contains("{{Invented}}"));
}

#[tokio::test]
async fn a_code_fenced_answer_with_a_line_of_commentary_is_cleaned() {
    let folders = Folders::build();
    let gateway = start_gateway_replying(
        "Here is the template:\n\n```text\nDear {{Fournisseur}}, please pay {{Montant TTC}}.\n```\nHope it helps!",
    );
    let plan = plan_of(drafted(&gateway.url, folders.source(QUESTION), QUESTION).await);
    assert_eq!(
        plan.draft_text.as_deref(),
        Some("Dear {{Fournisseur}}, please pay {{Montant TTC}}.")
    );
}

#[tokio::test]
async fn an_answer_with_no_placeholder_is_asked_for_again_once() {
    let folders = Folders::build();
    let gateway: CountingGateway = start_gateway_replying_in_turn(&[
        Some("Dear customer, please pay."),
        Some("Dear {{Fournisseur}}, please pay."),
    ]);
    let plan = plan_of(drafted(&gateway.url, folders.source(QUESTION), QUESTION).await);

    assert_eq!(gateway.calls(), 2);
    assert_eq!(
        plan.draft_text.as_deref(),
        Some("Dear {{Fournisseur}}, please pay.")
    );
    let requests = gateway.requests();
    assert!(!requests[0].contains("no placeholder"), "{}", requests[0]);
    assert!(
        requests[1].contains("held no placeholder"),
        "the second request is stricter: {}",
        requests[1]
    );
}

#[tokio::test]
async fn twice_no_placeholder_is_the_honest_answer_and_no_letter() {
    let folders = Folders::build();
    let gateway = start_gateway_replying("Dear customer, please pay.");
    let decision = drafted(&gateway.url, folders.source(QUESTION), QUESTION).await;

    assert_eq!(gateway.calls(), 2, "one retry, not more");
    let FillDecision::Ask(TabularAnswer::FillNeedsTemplate { file, draft }) = decision else {
        panic!("no template could be drafted: the answer says so");
    };
    assert_eq!(file, WORKBOOK);
    assert_eq!(draft.as_deref(), Some("unusable"));
    assert!(files_in(folders.documents.path()).is_empty());
}

#[tokio::test]
async fn a_gateway_that_cannot_be_reached_is_the_honest_answer_and_no_letter() {
    let folders = Folders::build();
    let decision = drafted(
        &hanging_up_gateway_url(),
        folders.source(QUESTION),
        QUESTION,
    )
    .await;

    let FillDecision::Ask(TabularAnswer::FillNeedsTemplate { file, draft }) = decision else {
        panic!("the AI machine is stopped: the answer says so");
    };
    assert_eq!(file, WORKBOOK);
    assert_eq!(draft.as_deref(), Some("unreachable"));
    assert!(files_in(folders.documents.path()).is_empty());
}

#[tokio::test]
async fn a_gateway_that_answers_nothing_usable_is_not_a_plan() {
    let folders = Folders::build();
    let gateway = start_counting_gateway();
    let decision = drafted(&gateway.url, folders.source(QUESTION), QUESTION).await;
    assert!(
        matches!(
            decision,
            FillDecision::Ask(TabularAnswer::FillNeedsTemplate { .. })
        ),
        "a body that is not a stream never becomes a letter"
    );
}

#[tokio::test]
async fn approving_writes_new_letters_and_the_model_and_never_overwrites() {
    let folders = Folders::build();
    let source = folders.source(QUESTION);
    let table = source.table.clone();
    let gateway = start_gateway_replying(DRAFT);
    let plan = plan_of(drafted(&gateway.url, source, QUESTION).await);
    let request = request_of(&plan, true);
    let template =
        TemplateFile::drafted(request.draft_text.as_deref().unwrap(), &request.data_file).unwrap();
    let log = folders.documents.path().join("log").join("generated.jsonl");

    let first = generate(
        folders.documents.path(),
        "Generated",
        &log,
        1,
        &template,
        &table,
        &request,
    )
    .unwrap();
    assert_eq!(
        first
            .written
            .iter()
            .map(|each| each.relative_path.as_str())
            .collect::<Vec<_>>(),
        [
            "Generated/invoices-letter-1.txt",
            "Generated/invoices-letter-2.txt",
            "Generated/invoices-letter-3.txt"
        ]
    );
    assert_eq!(
        first.model_file.as_deref(),
        Some("Generated/invoices-letter-model.txt")
    );
    let generated = folders.documents.path().join("Generated");
    let letter = std::fs::read_to_string(generated.join("invoices-letter-2.txt")).unwrap();
    assert!(
        letter.contains("Dear Blorbo Textiles") && letter.contains("412,08"),
        "{letter}"
    );
    assert_eq!(
        std::fs::read_to_string(generated.join("invoices-letter-model.txt")).unwrap(),
        DRAFT.trim(),
        "the approved text is kept as it was approved, placeholders and all"
    );

    // The same approval again: the names are taken, so new ones are made and nothing is touched.
    let second = generate(
        folders.documents.path(),
        "Generated",
        &log,
        2,
        &template,
        &table,
        &request,
    )
    .unwrap();
    assert_eq!(
        second.model_file.as_deref(),
        Some("Generated/invoices-letter-model-2.txt")
    );
    assert_eq!(
        second.written[0].relative_path,
        "Generated/invoices-letter-1-2.txt"
    );
    assert_eq!(
        std::fs::read_to_string(generated.join("invoices-letter-2.txt")).unwrap(),
        letter,
        "the first letters are unchanged"
    );
    assert_eq!(files_in(&generated).len(), 8);

    // The log says the template was drafted and holds no cell and no text.
    let logged = std::fs::read_to_string(&log).unwrap();
    assert!(logged.contains("\"drafted\":true"), "{logged}");
    for cell in table.rows.iter().flatten() {
        assert!(!logged.contains(cell.as_str()), "{logged}");
    }
    assert!(!logged.contains("Dear"), "{logged}");
    // The data was not touched and nothing was written outside `Generated`.
    assert_eq!(files_in(folders.documents.path()), ["Generated", "log"]);
}

#[tokio::test]
async fn a_saved_model_in_the_output_folder_is_not_offered_as_a_template_next_time() {
    let folders = Folders::build();
    let source = folders.source(QUESTION);
    let table = source.table.clone();
    let gateway = start_gateway_replying(DRAFT);
    let plan = plan_of(drafted(&gateway.url, source, QUESTION).await);
    let request = request_of(&plan, true);
    let template =
        TemplateFile::drafted(request.draft_text.as_deref().unwrap(), &request.data_file).unwrap();
    let log = folders.documents.path().join("log").join("generated.jsonl");
    generate(
        folders.documents.path(),
        "Generated",
        &log,
        1,
        &template,
        &table,
        &request,
    )
    .unwrap();

    // The documents now hold the saved model; asked again, the folder it sits in offers no template.
    let documents = WorkFolderInventory::discover(folders.documents.path(), None).unwrap();
    let data = folders.open();
    let named = tabular_answer::named_workbooks(QUESTION, &data, &ScopeMode::WholeFolder);
    let locate = || {
        tabular_answer::locate_fill_source(
            QUESTION,
            &data,
            &ScopeMode::WholeFolder,
            LOCALE,
            &folders.index,
            true,
            &named,
        )
        .unwrap()
    };
    assert!(
        matches!(
            decide(locate(), &named, true, &documents).unwrap(),
            FillDecision::Plan(_)
        ),
        "without the rule the saved model would be taken as a template"
    );
    let kept = without_output_folder(documents, "Generated");
    assert!(matches!(
        decide(locate(), &named, true, &kept).unwrap(),
        FillDecision::NeedsDraft(_)
    ));
}

#[tokio::test]
async fn an_edited_draft_is_checked_again_and_a_tampered_one_is_refused_or_left_unresolved() {
    let folders = Folders::build();
    let source = folders.source(QUESTION);
    let table = source.table.clone();
    let gateway = start_gateway_replying(DRAFT);
    let plan = plan_of(drafted(&gateway.url, source, QUESTION).await);

    // Edited by the user: the preview follows the new text.
    let mut edited = request_of(&plan, false);
    edited.draft_text = Some("Hello {{Fournisseur}}, about {{N facture}}.".to_string());
    let template =
        TemplateFile::drafted(edited.draft_text.as_deref().unwrap(), &edited.data_file).unwrap();
    let shown = preview(&template, &table, &edited).unwrap();
    assert!(
        shown
            .text
            .starts_with("Hello Zorglub Industries, about FAC-Z9Q-001."),
        "{}",
        shown.text
    );
    assert_eq!(
        shown
            .placeholders
            .iter()
            .map(|each| each.name.as_str())
            .collect::<Vec<_>>(),
        ["Fournisseur", "N facture"]
    );

    // A placeholder of a column the table lacks, mapped by a tampered request, stays as written.
    let mut tampered = request_of(&plan, false);
    tampered.draft_text = Some("Secret {{Password}} for {{Fournisseur}}".to_string());
    tampered.mapping = vec![
        RequestMapping {
            placeholder: "Password".into(),
            column: Some("Password".into()),
        },
        RequestMapping {
            placeholder: "Fournisseur".into(),
            column: Some("Fournisseur".into()),
        },
    ];
    let template =
        TemplateFile::drafted(tampered.draft_text.as_deref().unwrap(), &tampered.data_file)
            .unwrap();
    let shown = preview(&template, &table, &tampered).unwrap();
    assert_eq!(shown.text, "Secret {{Password}} for Zorglub Industries");
    assert_eq!(shown.unresolved, ["Password"]);

    // Too large: refused, nothing is written.
    let huge = "x".repeat(MAX_DRAFT_BYTES + 1);
    assert!(matches!(
        TemplateFile::drafted(&huge, WORKBOOK),
        Err(AppError::FillDraftTooLarge { limit }) if limit == MAX_DRAFT_BYTES
    ));
    assert!(matches!(
        TemplateFile::drafted("   \n", WORKBOOK),
        Err(AppError::FillNothingToFill)
    ));
    assert!(files_in(folders.documents.path()).is_empty());
}
