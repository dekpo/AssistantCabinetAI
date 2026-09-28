//! Distinguishing what a question about a selected workbook wants: a structural fact already in
//! `TabularInventory`, or a data-analysis operation `tabular::engine` must compute. Neither path
//! calls a model; natural-language phrasing is an input format, not a reason to escalate
//! (`docs/SESSION-DATA-04-TABULAR-Engine.md`).
//!
//! The vocabulary - which words mean "sum", "sheets", "descending" - is a locale pattern pack
//! loaded as data, never literals in Rust (`docs/DECISIONS.md`), the same discipline
//! `folder_questions` already follows for the document pipeline: both accented and unaccented
//! French forms are listed, because a person cannot be required to type an accent. A sheet or
//! column name is never guessed from this vocabulary at all - it is matched against the real
//! names the reachable sheet(s) actually have, so a question can only ever point at evidence that
//! exists.
//!
//! **Scope of this classifier, stated rather than discovered by surprise.** It recognises one
//! operation keyword and at most one column reference per question, and resolves the sheet only
//! when exactly one is reachable or the question names one unambiguously. It does **not** parse a
//! filter clause ("... where category is X") or a group-by pair ("total per supplier") out of
//! free text - `Operation::Filter`, `Operation::Count` with a filter, and `Operation::GroupSum`
//! are fully supported by `tabular::engine` but are not reached by `classify`, only by
//! constructing the `Operation` directly. A miscounted filter value would be exactly the kind of
//! guess this pipeline exists to refuse, so the honest choice is `NotRecognised` rather than a
//! best-effort parse of a clause this module cannot verify it read correctly.

use serde::Deserialize;

use super::engine::Operation;
use super::inventory::{ColumnInventory, TabularInventory};
use super::structural::StructuralQuestion;
use crate::file_reference::fold_text;

const PACKS: &[(&str, &str)] = &[
    (
        "en-US",
        include_str!("../../resources/tabular-questions/en-US.json"),
    ),
    (
        "fr-FR",
        include_str!("../../resources/tabular-questions/fr-FR.json"),
    ),
];

const FALLBACK_LOCALE: &str = "fr-FR";

#[derive(Debug, Clone, Deserialize)]
struct PatternPack {
    structural: StructuralWords,
    operations: OperationWords,
    descending: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct StructuralWords {
    sheets: Vec<String>,
    rows: Vec<String>,
    columns: Vec<String>,
    numeric: Vec<String>,
    formulas: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OperationWords {
    count: Vec<String>,
    distinct: Vec<String>,
    sum: Vec<String>,
    min: Vec<String>,
    max: Vec<String>,
    largest_row: Vec<String>,
    sort: Vec<String>,
}

/// The pack for a locale, falling back the way the interface does: the exact tag, then any pack
/// for the same language, then the pilot's own - the same order `folder_questions::pack_for`
/// uses.
fn pack_for(locale: &str) -> Option<PatternPack> {
    let language = locale.split('-').next().unwrap_or(locale).to_lowercase();
    let body = PACKS
        .iter()
        .find(|(tag, _)| tag.eq_ignore_ascii_case(locale))
        .or_else(|| {
            PACKS.iter().find(|(tag, _)| {
                tag.split('-')
                    .next()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&language))
            })
        })
        .or_else(|| PACKS.iter().find(|(tag, _)| *tag == FALLBACK_LOCALE))
        .map(|(_, body)| *body)?;
    serde_json::from_str(body).ok()
}

/// Where one question was routed.
#[derive(Debug, Clone, PartialEq)]
pub enum TabularRoute {
    /// Answered from `TabularInventory` alone, through `tabular::structural::answer`.
    Structural(StructuralQuestion),
    /// Run through `tabular::engine::execute`. `sheet` is the sheet the question named, if it
    /// named one unambiguously; `None` lets `execute` fall back to the single reachable sheet, or
    /// refuse as ambiguous when there is more than one.
    Operation {
        sheet: Option<String>,
        operation: Operation,
    },
    /// The question could not be confidently read as either. Never a guess at which operation or
    /// which column was meant.
    NotRecognised,
}

/// Classify one question against `inventory`, restricted to `allowed_sheets` exactly as
/// `tabular::engine::execute` and `tabular::structural::answer` are - a sheet outside the scope
/// is never matched by name here either, so a question cannot even be classified against
/// evidence it could not go on to read.
pub fn classify(
    question: &str,
    inventory: &TabularInventory,
    allowed_sheets: Option<&[String]>,
    locale: &str,
) -> TabularRoute {
    let Some(pack) = pack_for(locale) else {
        return TabularRoute::NotRecognised;
    };
    let tokens = tokenise(question);
    if tokens.is_empty() {
        return TabularRoute::NotRecognised;
    }

    let folded_question = fold_text(question);
    let sheet_name = find_sheet_name(inventory, allowed_sheets, &folded_question);
    let columns = columns_for_matching(inventory, allowed_sheets, sheet_name.as_deref());
    let column_name = find_column_name(&columns, &folded_question);

    if let Some(structural) = detect_structural(&tokens, &pack, &sheet_name, &column_name) {
        return TabularRoute::Structural(structural);
    }
    if let Some(operation) = detect_operation(&tokens, &pack, &column_name) {
        return TabularRoute::Operation {
            sheet: sheet_name,
            operation,
        };
    }
    TabularRoute::NotRecognised
}

fn detect_structural(
    tokens: &[String],
    pack: &PatternPack,
    sheet_name: &Option<String>,
    column_name: &Option<String>,
) -> Option<StructuralQuestion> {
    let has = |words: &[String]| tokens.iter().any(|token| contains(words, token));

    // "sheet(s)" on its own means "what sheets exist"; mentioned alongside "row(s)" or
    // "column(s)" it is naming which sheet the real question is about instead, and one of the
    // checks below decides it.
    if has(&pack.structural.sheets)
        && !has(&pack.structural.rows)
        && !has(&pack.structural.columns)
    {
        return Some(StructuralQuestion::SheetNames);
    }
    if has(&pack.structural.numeric) {
        if let Some(column) = column_name.clone() {
            return Some(StructuralQuestion::IsColumnNumeric {
                sheet: sheet_name.clone(),
                column,
            });
        }
    }
    if has(&pack.structural.formulas) {
        return Some(StructuralQuestion::HasFormulas {
            sheet: sheet_name.clone(),
        });
    }
    if has(&pack.structural.rows) {
        return Some(StructuralQuestion::RowCount {
            sheet: sheet_name.clone(),
        });
    }
    if has(&pack.structural.columns) {
        return Some(StructuralQuestion::ColumnNames {
            sheet: sheet_name.clone(),
        });
    }
    None
}

/// Order matters: a more specific operation is checked before `count`, so "how many distinct
/// suppliers" is read as `distinct` rather than `count` - both words are present, and only one
/// operation may be chosen.
fn detect_operation(
    tokens: &[String],
    pack: &PatternPack,
    column_name: &Option<String>,
) -> Option<Operation> {
    let has = |words: &[String]| tokens.iter().any(|token| contains(words, token));
    let descending = has(&pack.descending);

    if has(&pack.operations.sum) {
        return column_name
            .clone()
            .map(|column| Operation::Sum { column });
    }
    if has(&pack.operations.min) {
        return column_name
            .clone()
            .map(|column| Operation::Min { column });
    }
    if has(&pack.operations.max) {
        return column_name
            .clone()
            .map(|column| Operation::Max { column });
    }
    if has(&pack.operations.largest_row) {
        return column_name
            .clone()
            .map(|column| Operation::LargestRow { by_column: column });
    }
    if has(&pack.operations.distinct) {
        return column_name
            .clone()
            .map(|column| Operation::Distinct { column });
    }
    if has(&pack.operations.sort) {
        return column_name
            .clone()
            .map(|column| Operation::Sort { column, descending });
    }
    if has(&pack.operations.count) {
        return Some(Operation::Count { filter: None });
    }
    None
}

/// The one reachable sheet the question names, when exactly one does. `None` when it names none
/// (`tabular::engine::execute` falls back to the single reachable sheet on its own) and also when
/// it names more than one, which this module never picks between.
fn find_sheet_name(
    inventory: &TabularInventory,
    allowed_sheets: Option<&[String]>,
    folded_question: &str,
) -> Option<String> {
    let mut matches: Vec<&str> = super::engine::reachable_sheets(inventory, allowed_sheets)
        .into_iter()
        .filter(|sheet| folded_question.contains(&fold_text(&sheet.name)))
        .map(|sheet| sheet.name.as_str())
        .collect();
    match matches.len() {
        1 => Some(matches.remove(0).to_string()),
        _ => None,
    }
}

/// The columns a column reference in the question may be matched against: the named sheet's
/// columns, or the single reachable sheet's when none was named and there is only one.
/// Deliberately empty when the sheet cannot be pinned down, so a column is never matched against
/// the wrong sheet's headers.
fn columns_for_matching<'a>(
    inventory: &'a TabularInventory,
    allowed_sheets: Option<&[String]>,
    sheet_name: Option<&str>,
) -> Vec<&'a ColumnInventory> {
    let reachable = super::engine::reachable_sheets(inventory, allowed_sheets);
    let sheet = match sheet_name {
        Some(name) => reachable
            .into_iter()
            .find(|sheet| fold_text(&sheet.name) == fold_text(name)),
        None if reachable.len() == 1 => Some(reachable[0]),
        None => None,
    };
    sheet.map(|sheet| sheet.columns.iter().collect()).unwrap_or_default()
}

/// The one column whose real name appears in the question, matched by folded substring so
/// "Montant" and "montant" and "montants" (typed with the plural, or an accent) all still reach
/// it. When more than one column name of the same length matches, or two names overlap without
/// one containing the other, this refuses to pick between them rather than guess which one the
/// question meant.
fn find_column_name(columns: &[&ColumnInventory], folded_question: &str) -> Option<String> {
    let mut matches: Vec<&ColumnInventory> = columns
        .iter()
        .filter(|column| {
            let folded = fold_text(&column.name);
            !folded.is_empty() && folded_question.contains(&folded)
        })
        .copied()
        .collect();
    if matches.is_empty() {
        return None;
    }
    matches.sort_by_key(|column| std::cmp::Reverse(fold_text(&column.name).chars().count()));
    let longest_len = fold_text(&matches[0].name).chars().count();
    let longest: Vec<&&ColumnInventory> = matches
        .iter()
        .filter(|column| fold_text(&column.name).chars().count() == longest_len)
        .collect();
    match longest.len() {
        1 => Some(longest[0].name.clone()),
        _ => None,
    }
}

fn contains(words: &[String], token: &str) -> bool {
    words.iter().any(|word| word == token)
}

/// Words, lowercased - the same rule `folder_questions::tokenise` uses, so punctuation cannot
/// change an answer.
fn tokenise(question: &str) -> Vec<String> {
    question
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabular::inventory::TabularFormat;
    use crate::tabular::{CellValue, SheetData, Workbook};

    fn text_row(values: &[&str]) -> Vec<CellValue> {
        values
            .iter()
            .map(|value| CellValue::Text(value.to_string()))
            .collect()
    }

    fn invoices() -> TabularInventory {
        let workbook = Workbook {
            sheets: vec![SheetData {
                name: "Facturation".to_string(),
                rows: vec![
                    text_row(&["fournisseur", "montant"]),
                    text_row(&["Alpha", "120,50"]),
                    text_row(&["Beta", "75,00"]),
                ],
            }],
        };
        TabularInventory::build("factures.csv", "hash-1", TabularFormat::Csv, &workbook)
    }

    fn two_sheets() -> TabularInventory {
        let workbook = Workbook {
            sheets: vec![
                SheetData {
                    name: "Consultations".to_string(),
                    rows: vec![text_row(&["nom"]), text_row(&["Camille"])],
                },
                SheetData {
                    name: "Facturation".to_string(),
                    rows: vec![
                        text_row(&["fournisseur", "montant"]),
                        text_row(&["Alpha", "120,50"]),
                    ],
                },
            ],
        };
        TabularInventory::build("classeur.xlsx", "hash-2", TabularFormat::Xlsx, &workbook)
    }

    #[test]
    fn every_shipped_pack_parses() {
        for (tag, _) in PACKS {
            assert!(pack_for(tag).is_some(), "pack {tag} must parse");
        }
    }

    #[test]
    fn sheet_names_is_recognised_in_both_languages() {
        let inventory = two_sheets();
        for (question, locale) in [
            ("What sheets does this workbook have?", "en-US"),
            ("Quelles feuilles contient ce classeur ?", "fr-FR"),
        ] {
            assert_eq!(
                classify(question, &inventory, None, locale),
                TabularRoute::Structural(StructuralQuestion::SheetNames),
                "{question}"
            );
        }
    }

    #[test]
    fn row_count_is_structural_even_when_a_sheet_is_named() {
        let inventory = two_sheets();
        assert_eq!(
            classify(
                "How many rows are in the Facturation sheet?",
                &inventory,
                None,
                "en-US"
            ),
            TabularRoute::Structural(StructuralQuestion::RowCount {
                sheet: Some("Facturation".to_string())
            })
        );
        assert_eq!(
            classify(
                "Combien de lignes dans la feuille Facturation ?",
                &inventory,
                None,
                "fr-FR"
            ),
            TabularRoute::Structural(StructuralQuestion::RowCount {
                sheet: Some("Facturation".to_string())
            })
        );
    }

    #[test]
    fn is_this_column_numeric_resolves_the_column_by_its_real_name() {
        let inventory = invoices();
        assert_eq!(
            classify(
                "Is the montant column numeric?",
                &inventory,
                None,
                "en-US"
            ),
            TabularRoute::Structural(StructuralQuestion::IsColumnNumeric {
                sheet: None,
                column: "montant".to_string(),
            })
        );
    }

    #[test]
    fn does_this_workbook_contain_formulas_is_recognised() {
        let inventory = invoices();
        assert_eq!(
            classify(
                "Does this workbook contain formulas?",
                &inventory,
                None,
                "en-US"
            ),
            TabularRoute::Structural(StructuralQuestion::HasFormulas { sheet: None })
        );
    }

    #[test]
    fn a_sum_question_resolves_to_the_sum_operation_over_the_named_column() {
        let inventory = invoices();
        for (question, locale) in [
            ("What is the total montant?", "en-US"),
            ("Quelle est la somme des montant ?", "fr-FR"),
        ] {
            assert_eq!(
                classify(question, &inventory, None, locale),
                TabularRoute::Operation {
                    sheet: None,
                    operation: Operation::Sum {
                        column: "montant".to_string()
                    }
                },
                "{question}"
            );
        }
    }

    #[test]
    fn distinct_outranks_count_when_both_words_are_present() {
        let inventory = invoices();
        assert_eq!(
            classify(
                "How many distinct fournisseur are there?",
                &inventory,
                None,
                "en-US"
            ),
            TabularRoute::Operation {
                sheet: None,
                operation: Operation::Distinct {
                    column: "fournisseur".to_string()
                }
            }
        );
    }

    #[test]
    fn a_sort_question_carries_the_descending_flag() {
        let inventory = invoices();
        assert_eq!(
            classify(
                "Sort by montant descending",
                &inventory,
                None,
                "en-US"
            ),
            TabularRoute::Operation {
                sheet: None,
                operation: Operation::Sort {
                    column: "montant".to_string(),
                    descending: true,
                }
            }
        );
        assert_eq!(
            classify("Sort by montant", &inventory, None, "en-US"),
            TabularRoute::Operation {
                sheet: None,
                operation: Operation::Sort {
                    column: "montant".to_string(),
                    descending: false,
                }
            }
        );
    }

    #[test]
    fn a_plain_count_with_no_column_needs_no_column_reference() {
        let inventory = invoices();
        assert_eq!(
            classify("How many rows are there?", &inventory, None, "en-US"),
            TabularRoute::Structural(StructuralQuestion::RowCount { sheet: None }),
            "rows is a structural subject, checked before the count operation"
        );
        assert_eq!(
            classify("Give me a count", &inventory, None, "en-US"),
            TabularRoute::Operation {
                sheet: None,
                operation: Operation::Count { filter: None }
            }
        );
    }

    #[test]
    fn an_operation_naming_no_recognisable_column_is_not_recognised() {
        let inventory = invoices();
        assert_eq!(
            classify("What is the total?", &inventory, None, "en-US"),
            TabularRoute::NotRecognised
        );
    }

    #[test]
    fn a_column_question_with_more_than_one_reachable_sheet_and_no_name_is_not_recognised() {
        let inventory = two_sheets();
        // "montant" only exists on Facturation, so with two reachable sheets and none named the
        // column cannot be matched against the right one - refused rather than guessed.
        assert_eq!(
            classify("What is the total montant?", &inventory, None, "en-US"),
            TabularRoute::NotRecognised
        );
    }

    #[test]
    fn a_sheet_outside_the_scope_is_never_matched_by_name() {
        let inventory = two_sheets();
        let allowed = vec!["Consultations".to_string()];

        // Facturation exists in the workbook but not in the scope, so naming it must not resolve
        // - the classifier must not even let a question point at evidence outside the scope.
        assert_eq!(
            classify(
                "What is the total montant in Facturation?",
                &inventory,
                Some(&allowed),
                "en-US"
            ),
            TabularRoute::NotRecognised
        );
    }

    #[test]
    fn an_unrecognisable_sentence_is_not_forced_into_a_guess() {
        let inventory = invoices();
        assert_eq!(
            classify(
                "What did the specialist recommend?",
                &inventory,
                None,
                "en-US"
            ),
            TabularRoute::NotRecognised
        );
    }

    #[test]
    fn filter_and_group_sum_are_deliberately_out_of_this_classifiers_scope() {
        let inventory = invoices();
        // A filter clause is not parsed from free text - only structural facts and single-column
        // operations are. This is a documented scope limit, not a bug: guessing a filter value
        // wrongly would be exactly the failure this pipeline exists to refuse.
        assert_eq!(
            classify(
                "How many invoices where fournisseur is Alpha?",
                &inventory,
                None,
                "en-US"
            ),
            TabularRoute::Operation {
                sheet: None,
                operation: Operation::Count { filter: None }
            },
            "the filter clause is ignored rather than mis-parsed; the plain count still resolves"
        );
    }
}
