//! Structural questions about a workbook - "what sheets exist", "how many rows", "what columns
//! exist", "is this column numeric", "does this workbook contain formulas" - answered directly
//! from `TabularInventory`, which Session 3 already built by a full pass over the file. No cell
//! is read again, and no gateway call is made: these are exactly the kind of fact the engine's
//! deterministic operations are not needed for, and natural-language phrasing of one is never a
//! reason to run it through `tabular::engine` or escalate to a model
//! (`docs/SESSION-DATA-04-TABULAR-Engine.md`).
//!
//! Deliberately not folded into `tabular::engine::Operation`: a structural question needs no
//! `Workbook`, only the `TabularInventory` Session 3 already produced, and keeping the two apart
//! is what lets a caller answer "how many sheets does this have" without opening the file a
//! second time.

use serde::Serialize;

use super::engine::{resolve_sheet_inventory, NotAnswerableReason};
use super::inventory::TabularInventory;

#[derive(Debug, Clone, PartialEq)]
pub enum StructuralQuestion {
    SheetNames,
    RowCount { sheet: Option<String> },
    ColumnNames { sheet: Option<String> },
    IsColumnNumeric { sheet: Option<String>, column: String },
    HasFormulas { sheet: Option<String> },
}

/// Machine codes plus data, exactly as `folder_questions::FolderAnswer` writes no sentence: the
/// interface localises these.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StructuralAnswer {
    SheetNames { sheets: Vec<String> },
    RowCount { sheet: String, rows: usize },
    ColumnNames { sheet: String, columns: Vec<String> },
    IsColumnNumeric {
        sheet: String,
        column: String,
        numeric: bool,
    },
    HasFormulas { sheet: String, has_formulas: bool },
    NotAnswerable {
        reason: NotAnswerableReason,
        available_sheets: Vec<String>,
    },
}

/// Answer one structural question, restricted to `allowed_sheets` exactly as
/// `tabular::engine::execute` is - a structural question about a sheet outside the scope must
/// refuse identically to one about a sheet that does not exist.
pub fn answer(
    inventory: &TabularInventory,
    allowed_sheets: Option<&[String]>,
    question: &StructuralQuestion,
) -> StructuralAnswer {
    if let StructuralQuestion::SheetNames = question {
        let sheets: Vec<String> = super::engine::reachable_sheets(inventory, allowed_sheets)
            .into_iter()
            .map(|sheet| sheet.name.clone())
            .collect();
        return StructuralAnswer::SheetNames { sheets };
    }

    let requested = match question {
        StructuralQuestion::RowCount { sheet }
        | StructuralQuestion::ColumnNames { sheet }
        | StructuralQuestion::HasFormulas { sheet } => sheet.as_deref(),
        StructuralQuestion::IsColumnNumeric { sheet, .. } => sheet.as_deref(),
        StructuralQuestion::SheetNames => unreachable!(),
    };

    let sheet = match resolve_sheet_inventory(inventory, requested, allowed_sheets) {
        Ok(sheet) => sheet,
        Err(reason) => {
            let available_sheets: Vec<String> =
                super::engine::reachable_sheets(inventory, allowed_sheets)
                    .into_iter()
                    .map(|sheet| sheet.name.clone())
                    .collect();
            return StructuralAnswer::NotAnswerable {
                reason,
                available_sheets,
            };
        }
    };

    match question {
        StructuralQuestion::SheetNames => unreachable!(),
        StructuralQuestion::RowCount { .. } => StructuralAnswer::RowCount {
            sheet: sheet.name.clone(),
            rows: sheet.row_count,
        },
        StructuralQuestion::ColumnNames { .. } => StructuralAnswer::ColumnNames {
            sheet: sheet.name.clone(),
            columns: sheet.columns.iter().map(|c| c.name.clone()).collect(),
        },
        StructuralQuestion::HasFormulas { .. } => StructuralAnswer::HasFormulas {
            sheet: sheet.name.clone(),
            has_formulas: sheet.has_formulas,
        },
        StructuralQuestion::IsColumnNumeric { column, .. } => {
            match sheet.columns.iter().find(|c| c.name == *column) {
                Some(found) => StructuralAnswer::IsColumnNumeric {
                    sheet: sheet.name.clone(),
                    column: found.name.clone(),
                    numeric: found.inferred_type == super::inventory::ColumnType::Numeric,
                },
                None => StructuralAnswer::NotAnswerable {
                    reason: NotAnswerableReason::ColumnNotFound,
                    available_sheets: vec![sheet.name.clone()],
                },
            }
        }
    }
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

    fn build(sheets: Vec<(&str, Vec<Vec<CellValue>>)>) -> TabularInventory {
        let workbook = Workbook {
            sheets: sheets
                .into_iter()
                .map(|(name, rows)| SheetData {
                    name: name.to_string(),
                    rows,
                })
                .collect(),
        };
        TabularInventory::build("data.xlsx", "hash-1", TabularFormat::Xlsx, &workbook, "fr-FR")
    }

    #[test]
    fn sheet_names_lists_every_reachable_sheet() {
        let inventory = build(vec![
            ("Consultations", vec![text_row(&["a"]), text_row(&["1"])]),
            ("Facturation", vec![text_row(&["b"]), text_row(&["2"])]),
        ]);

        let answer = answer(&inventory, None, &StructuralQuestion::SheetNames);

        assert_eq!(
            answer,
            StructuralAnswer::SheetNames {
                sheets: vec!["Consultations".into(), "Facturation".into()]
            }
        );
    }

    #[test]
    fn sheet_names_respects_a_scope_restriction() {
        let inventory = build(vec![
            ("Consultations", vec![text_row(&["a"]), text_row(&["1"])]),
            ("Facturation", vec![text_row(&["b"]), text_row(&["2"])]),
        ]);
        let allowed = vec!["Consultations".to_string()];

        let answer = answer(&inventory, Some(&allowed), &StructuralQuestion::SheetNames);

        assert_eq!(
            answer,
            StructuralAnswer::SheetNames {
                sheets: vec!["Consultations".into()]
            }
        );
    }

    #[test]
    fn row_count_and_column_names_read_the_named_sheet() {
        let inventory = build(vec![(
            "Feuille1",
            vec![
                text_row(&["nom", "montant"]),
                text_row(&["Camille", "10"]),
                text_row(&["Esaie", "20"]),
            ],
        )]);

        assert_eq!(
            answer(
                &inventory,
                None,
                &StructuralQuestion::RowCount { sheet: None }
            ),
            StructuralAnswer::RowCount {
                sheet: "Feuille1".into(),
                rows: 2
            }
        );
        assert_eq!(
            answer(
                &inventory,
                None,
                &StructuralQuestion::ColumnNames { sheet: None }
            ),
            StructuralAnswer::ColumnNames {
                sheet: "Feuille1".into(),
                columns: vec!["nom".into(), "montant".into()]
            }
        );
    }

    #[test]
    fn is_column_numeric_reports_the_inferred_type() {
        let inventory = build(vec![(
            "Feuille1",
            vec![
                text_row(&["nom", "montant"]),
                text_row(&["Camille", "10"]),
            ],
        )]);

        assert_eq!(
            answer(
                &inventory,
                None,
                &StructuralQuestion::IsColumnNumeric {
                    sheet: None,
                    column: "montant".into()
                }
            ),
            StructuralAnswer::IsColumnNumeric {
                sheet: "Feuille1".into(),
                column: "montant".into(),
                numeric: true
            }
        );
        assert_eq!(
            answer(
                &inventory,
                None,
                &StructuralQuestion::IsColumnNumeric {
                    sheet: None,
                    column: "nom".into()
                }
            ),
            StructuralAnswer::IsColumnNumeric {
                sheet: "Feuille1".into(),
                column: "nom".into(),
                numeric: false
            }
        );
    }

    #[test]
    fn has_formulas_reports_honestly_without_evaluating_anything() {
        let inventory = build(vec![(
            "Feuille1",
            vec![
                text_row(&["a", "total"]),
                vec![
                    CellValue::Number(1.0),
                    CellValue::Formula {
                        expression: "A2*2".into(),
                        cached_value: Some(Box::new(CellValue::Number(2.0))),
                    },
                ],
            ],
        )]);

        assert_eq!(
            answer(
                &inventory,
                None,
                &StructuralQuestion::HasFormulas { sheet: None }
            ),
            StructuralAnswer::HasFormulas {
                sheet: "Feuille1".into(),
                has_formulas: true
            }
        );
    }

    #[test]
    fn a_structural_question_about_a_sheet_outside_the_scope_refuses_like_a_missing_one() {
        let inventory = build(vec![
            ("Consultations", vec![text_row(&["a"]), text_row(&["1"])]),
            ("Facturation", vec![text_row(&["b"]), text_row(&["2"])]),
        ]);
        let allowed = vec!["Consultations".to_string()];

        let answer = answer(
            &inventory,
            Some(&allowed),
            &StructuralQuestion::RowCount {
                sheet: Some("Facturation".into()),
            },
        );

        assert_eq!(
            answer,
            StructuralAnswer::NotAnswerable {
                reason: NotAnswerableReason::SheetNotFound,
                available_sheets: vec!["Consultations".into()],
            }
        );
    }

    #[test]
    fn a_column_that_does_not_exist_is_refused_on_a_structural_question_too() {
        let inventory = build(vec![("Feuille1", vec![text_row(&["nom"]), text_row(&["x"])])]);

        let answer = answer(
            &inventory,
            None,
            &StructuralQuestion::IsColumnNumeric {
                sheet: None,
                column: "absent".into(),
            },
        );

        assert!(matches!(
            answer,
            StructuralAnswer::NotAnswerable {
                reason: NotAnswerableReason::ColumnNotFound,
                ..
            }
        ));
    }
}
