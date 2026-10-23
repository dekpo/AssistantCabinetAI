//! TABULAR step 07 (`docs/SESSION-TABULAR/SESSION-TABULAR-07-checked-formula-help.md`): the formulas a
//! model writes are checked by the program and the result is appended beside the answer, which is
//! never edited. Exercised at `mixed_answer::answer`, the one writer of the mixed tier and of the
//! tables-only tier, against the counting gateway: how many requests were made and what the last one
//! said. The workbook cards are written by hand here (their structure is what the checker reads); the
//! engine, the classifier and the documents are other tests' business.

mod common;

use std::time::Duration;

use assistant_cabinet_ai_lib::conversation::ContextBudget;
use assistant_cabinet_ai_lib::gateway::GatewayClient;
use assistant_cabinet_ai_lib::mixed_answer::{
    self, MixedAnswer, MixedContext, MixedPartUnavailable, FORMULA_REPAIR_INSTRUCTION,
};
use assistant_cabinet_ai_lib::tabular::card::{SheetShape, WorkbookCard, WorkbookShape};
use assistant_cabinet_ai_lib::tabular::formula_check::{FormulaContext, FormulaIssueCode, EXCEL};
use assistant_cabinet_ai_lib::tabular_answer::{self, PendingAnswer};
use common::counting_gateway::{start_gateway_replying_in_turn, CountingGateway};

const IDLE_TIMEOUT: Duration = Duration::from_millis(500);
const QUESTION: &str = "Write a formula that totals the amount of one supplier";
const CARDS_TEXT: &str = "Workbook cards (structure only, no data values):\n\
     Workbook \"reglements.csv\" (csv, one table)\n  Table: 5 data rows";

/// `reglements.csv`: five columns, A to E.
fn cards() -> Vec<WorkbookCard> {
    vec![WorkbookCard {
        file: "reglements.csv".to_string(),
        text: CARDS_TEXT.to_string(),
        truncated: false,
        shape: WorkbookShape {
            path: "reglements.csv".to_string(),
            is_csv: true,
            sheets: vec![SheetShape {
                name: "reglements".to_string(),
                columns: ["date", "fournisseur", "acompte", "reglement", "montant"]
                    .iter()
                    .map(|name| name.to_string())
                    .collect(),
                has_header: true,
            }],
        },
    }]
}

/// What `commands::advisory_tier` hands the writer: no table part, no document.
async fn ask(gateway: &CountingGateway, locale: &str, checked: bool) -> MixedAnswer {
    let client = GatewayClient::new().expect("builds a client");
    let cards = cards();
    let formulas = FormulaContext::new(EXCEL, locale, &cards);
    let context = MixedContext {
        locale,
        gateway: &client,
        server_url: &gateway.url,
        model_alias: "cabinet-chat",
        idle_timeout: IDLE_TIMEOUT,
        history: &[],
        budget: ContextBudget::DEFAULT,
        cards_text: CARDS_TEXT,
        sample_text: "",
        calculation: Default::default(),
        formulas: if checked { formulas.as_ref() } else { None },
    };
    let (result, generated) = mixed_answer::answer(
        QUESTION,
        PendingAnswer::Done(tabular_answer::no_table_part()),
        Vec::new(),
        Some(MixedPartUnavailable::NotSelected),
        &context,
        |_| {},
        |_| {},
    )
    .await
    .expect("the writer answers");
    assert!(generated);
    result
}

fn codes(result: &MixedAnswer) -> Vec<&'static str> {
    result
        .formula_notes
        .iter()
        .map(|note| note.issue.code.as_str())
        .collect()
}

#[tokio::test]
async fn a_formula_with_nothing_wrong_is_counted_and_costs_no_extra_request() {
    let text = "Use this:\n=SOMME.SI(B:B; \"x\"; E:E)\nI cannot test it.";
    let gateway = start_gateway_replying_in_turn(&[Some(text)]);
    let result = ask(&gateway, "fr-FR", true).await;

    assert_eq!(gateway.calls(), 1);
    assert_eq!(result.answer, text);
    assert!(result.formula_notes.is_empty());
    assert_eq!(result.formulas_verified, 1);
    assert!(!result.formula_repaired);
    assert!(result.formula_repaired_from.is_empty());
}

#[tokio::test]
async fn an_answer_without_a_formula_has_no_note_and_no_count() {
    let gateway = start_gateway_replying_in_turn(&[Some("The file lists payments by supplier.")]);
    let result = ask(&gateway, "en-US", true).await;

    assert_eq!(gateway.calls(), 1);
    assert!(result.formula_notes.is_empty());
    assert_eq!(result.formulas_verified, 0);
}

#[tokio::test]
async fn a_problem_the_model_cannot_fix_is_noted_and_the_text_is_left_as_written() {
    // A comma in a language that writes a semicolon: noted, not worth another request.
    let text = "=SOMME.SI(B:B, \"x\", E:E)";
    let gateway = start_gateway_replying_in_turn(&[Some(text)]);
    let result = ask(&gateway, "fr-FR", true).await;

    assert_eq!(gateway.calls(), 1, "a separator is not repaired");
    assert_eq!(result.answer, text, "the model's text is byte-identical");
    assert_eq!(codes(&result), ["wrong_separator"]);
    assert_eq!(result.formulas_verified, 0);
    assert_eq!(result.formula_notes[0].formula, text);
}

#[tokio::test]
async fn nothing_is_checked_without_a_context() {
    let gateway = start_gateway_replying_in_turn(&[Some("=SUMIF(Z:Z; 1)")]);
    let result = ask(&gateway, "fr-FR", false).await;

    assert_eq!(gateway.calls(), 1);
    assert!(result.formula_notes.is_empty());
    assert_eq!(result.formulas_verified, 0);
}

#[tokio::test]
async fn a_missing_column_and_an_english_function_are_repaired_once() {
    let first = "=SUMIF(B:B; \"x\"; F:F)";
    let second = "=SOMME.SI(B:B; \"x\"; E:E)";
    let gateway = start_gateway_replying_in_turn(&[Some(first), Some(second)]);
    let result = ask(&gateway, "fr-FR", true).await;

    assert_eq!(gateway.calls(), 2, "one answer and one repair, no more");
    assert!(result.formula_repaired);
    assert_eq!(result.answer, second);
    assert!(
        result.formula_notes.is_empty(),
        "{:?}",
        result.formula_notes
    );
    assert_eq!(result.formulas_verified, 1);
    // What was wrong in the first version is kept, with that first formula, to be said beside the
    // corrected answer.
    let before: Vec<(&str, &str)> = result
        .formula_repaired_from
        .iter()
        .map(|note| (note.issue.code.as_str(), note.formula.as_str()))
        .collect();
    assert_eq!(
        before,
        [
            ("function_in_other_language", first),
            ("unknown_column_letter", first)
        ]
    );

    // The second request carries the instruction and one line per problem, in English.
    let requests = gateway.requests();
    let repair = &requests[1];
    assert!(repair.starts_with(FORMULA_REPAIR_INSTRUCTION), "{repair}");
    assert!(repair.contains("function_in_other_language: SUMIF (expected: SOMME.SI)"));
    assert!(repair.contains("unknown_column_letter: F (expected: E)"));
    // The first request is the ordinary turn.
    assert!(requests[0].contains("Question: Write a formula"));
}

#[tokio::test]
async fn a_repair_that_is_still_wrong_is_shown_with_its_notes_and_never_repeated() {
    let first = "=SUMIF(B:B; \"x\"; E:E)";
    let second = "=SUMIFS(B:B; \"x\"; E:E)";
    let gateway = start_gateway_replying_in_turn(&[Some(first), Some(second), Some("=SOMME(E:E)")]);
    let result = ask(&gateway, "fr-FR", true).await;

    assert_eq!(gateway.calls(), 2, "no second repair");
    assert!(result.formula_repaired);
    assert_eq!(result.answer, second);
    assert_eq!(codes(&result), ["function_in_other_language"]);
    assert_eq!(
        result.formula_notes[0].issue.code,
        FormulaIssueCode::FunctionInOtherLanguage
    );
    assert_eq!(
        result.formula_notes[0].issue.expected.as_deref(),
        Some("SOMME.SI.ENS")
    );
}

#[tokio::test]
async fn a_repair_that_fails_keeps_the_first_answer_and_its_notes() {
    let first = "=SUMIF(B:B; \"x\"; E:E)";
    let gateway = start_gateway_replying_in_turn(&[Some(first), None]);
    let result = ask(&gateway, "fr-FR", true).await;

    assert_eq!(gateway.calls(), 2);
    assert!(!result.formula_repaired);
    assert_eq!(result.answer, first);
    assert_eq!(codes(&result), ["function_in_other_language"]);
}

#[tokio::test]
async fn a_repair_that_drops_every_formula_keeps_the_first_answer() {
    let first = "=JOIN(A2:B2; \" \")";
    let gateway =
        start_gateway_replying_in_turn(&[Some(first), Some("There is no formula for this.")]);
    let result = ask(&gateway, "fr-FR", true).await;

    assert_eq!(gateway.calls(), 2);
    assert!(!result.formula_repaired);
    assert_eq!(result.answer, first);
    assert_eq!(codes(&result), ["unknown_function"]);
}

#[tokio::test]
async fn a_function_that_reaches_outside_the_file_is_noted_and_not_repaired() {
    let text = "=SERVICEWEB(\"https://example.org/rate\")";
    let gateway = start_gateway_replying_in_turn(&[Some(text)]);
    let result = ask(&gateway, "fr-FR", true).await;

    assert_eq!(gateway.calls(), 1);
    assert_eq!(codes(&result), ["reaches_outside"]);
    assert_eq!(result.answer, text);
}

#[tokio::test]
async fn the_same_formula_is_read_in_the_language_of_the_interface() {
    let text = "=SUMIF(B:B, \"x\", E:E)";
    let gateway = start_gateway_replying_in_turn(&[Some(text)]);
    let result = ask(&gateway, "en-US", true).await;
    assert_eq!(gateway.calls(), 1);
    assert!(result.formula_notes.is_empty());
    assert_eq!(result.formulas_verified, 1);

    let french = "=SOMME.SI(B:B, \"x\", E:E)";
    let gateway = start_gateway_replying_in_turn(&[Some(french)]);
    let result = ask(&gateway, "en-US", true).await;
    assert_eq!(
        gateway.calls(),
        2,
        "a French name in an English interface is repaired"
    );
    assert_eq!(codes(&result), ["function_in_other_language"]);
    assert_eq!(
        result.formula_notes[0].issue.expected.as_deref(),
        Some("SUMIF")
    );
}
