//! XLSX adapter: `calamine` over data-grid sheets only, for Sprint 2b
//! (`docs/DECISIONS.md`, "XLSX scope for the first implementation").
//!
//! No formula evaluation and no Excel engine. A formula cell keeps its expression and whatever
//! value the file last cached for it, both reported honestly rather than one standing in for
//! the other (`docs/ARCHITECTURE.md`'s `derivation`: a cached formula result is a claim the file
//! makes, not a verified fact).

use std::path::Path;

use calamine::{open_workbook_auto, Data, DataType, Range, Reader};

use super::{CellValue, SheetData, TabularDataSource, TabularError, Workbook};

pub struct XlsxDataSource;

impl TabularDataSource for XlsxDataSource {
    fn open(&self, path: &Path) -> Result<Workbook, TabularError> {
        let mut workbook = open_workbook_auto(path).map_err(|_| TabularError::ReadFailed)?;
        let sheet_names = workbook.sheet_names().to_owned();

        let mut sheets = Vec::with_capacity(sheet_names.len());
        for name in sheet_names {
            let range = workbook
                .worksheet_range(&name)
                .map_err(|_| TabularError::ParseFailed)?;
            // Best-effort second pass: a sheet with no formulas at all, or a format calamine
            // cannot read formulas from, degrades to "no formulas" here rather than failing the
            // whole workbook - values are still read either way.
            let formulas = workbook.worksheet_formula(&name).ok();
            let value_start = range.start();
            let formula_start = formulas.as_ref().and_then(Range::start);

            let mut rows = Vec::with_capacity(range.height());
            for (row_index, row) in range.rows().enumerate() {
                let cells = row
                    .iter()
                    .enumerate()
                    .map(|(col_index, cell)| {
                        let expression = formula_expression_at(
                            formulas.as_ref(),
                            value_start,
                            formula_start,
                            row_index,
                            col_index,
                        );
                        convert_cell(cell, expression)
                    })
                    .collect();
                rows.push(cells);
            }
            sheets.push(SheetData { name, rows });
        }

        Ok(Workbook { sheets })
    }
}

/// `worksheet_range` and `worksheet_formula` each return a `Range` whose own coordinates start
/// at its own top-left cell rather than at the sheet's - a lone formula three rows down starts
/// its own range at row three, not row zero (`Range::get`'s own doc calls its argument a
/// "relative_position"). `row_index`/`col_index` here are relative to the value range, so they
/// are raised to absolute sheet coordinates and back down into the formula range's own
/// coordinates before the lookup, rather than reused as if the two ranges shared one origin.
fn formula_expression_at<'a>(
    formulas: Option<&'a Range<String>>,
    value_start: Option<(u32, u32)>,
    formula_start: Option<(u32, u32)>,
    row_index: usize,
    col_index: usize,
) -> Option<&'a String> {
    let formulas = formulas?;
    let (value_row, value_col) = value_start?;
    let (formula_row, formula_col) = formula_start?;
    let absolute_row = value_row as i64 + row_index as i64;
    let absolute_col = value_col as i64 + col_index as i64;
    let relative_row = absolute_row - formula_row as i64;
    let relative_col = absolute_col - formula_col as i64;
    if relative_row < 0 || relative_col < 0 {
        return None;
    }
    formulas
        .get((relative_row as usize, relative_col as usize))
        .filter(|text| !text.is_empty())
}

fn convert_cell(data: &Data, formula: Option<&String>) -> CellValue {
    // A real calendar date, read once by calamine's own `dates` feature (chrono) rather than
    // kept as the raw serial number the file stores or re-parsed from its ISO text a second time
    // (`docs/DECISIONS.md`, D3). `as_date()` covers both `DateTime` and `DateTimeIso` cells.
    let value = if data.is_datetime() || data.is_datetime_iso() {
        match data.as_date() {
            Some(date) => CellValue::Date(date),
            // A date-typed cell calamine cannot resolve to a calendar value: shown as whatever
            // text it does offer rather than silently dropped.
            None => data
                .as_string()
                .map(CellValue::Text)
                .unwrap_or(CellValue::Empty),
        }
    } else {
        match data {
            Data::Empty => CellValue::Empty,
            Data::String(text) if text.trim().is_empty() => CellValue::Empty,
            Data::String(text) => CellValue::Text(text.clone()),
            Data::Float(number) => CellValue::Number(*number),
            Data::Int(number) => CellValue::Number(*number as f64),
            Data::Bool(value) => CellValue::Text(value.to_string()),
            Data::DurationIso(text) => CellValue::Text(text.clone()),
            Data::Error(error) => CellValue::Text(format!("#ERROR:{error:?}")),
            Data::DateTime(_) | Data::DateTimeIso(_) => {
                unreachable!("handled by the is_datetime()/is_datetime_iso() branch above")
            }
        }
    };
    match formula {
        Some(expression) => CellValue::Formula {
            expression: expression.clone(),
            cached_value: Some(Box::new(value)),
        },
        None => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calamine::{Range, Xlsx};
    use std::io::Write;

    /// Writes a minimal but real `.xlsx` (a zip of the required OOXML parts), one or more
    /// sheets, so these tests exercise the actual `calamine` reader rather than a fixture
    /// calamine cannot open. `rows` are raw text; a leading `=` makes a cell a formula,
    /// everything else becomes an inline string (calamine reads inline strings the same way it
    /// reads shared-string cells, and it keeps the fixture self-contained).
    fn write_xlsx(path: &Path, sheets: &[(&str, &[&[&str]])]) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        let overrides: String = (1..=sheets.len())
            .map(|n| format!(r#"<Override PartName="/xl/worksheets/sheet{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#))
            .collect();
        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
{overrides}
</Types>"#).as_bytes()).unwrap();

        zip.start_file("_rels/.rels", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#).unwrap();

        let workbook_rels: String = (1..=sheets.len())
            .map(|n| format!(r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{n}.xml"/>"#))
            .collect();
        zip.start_file("xl/_rels/workbook.xml.rels", options)
            .unwrap();
        zip.write_all(
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{workbook_rels}
</Relationships>"#
            )
            .as_bytes(),
        )
        .unwrap();

        let sheet_entries: String = sheets
            .iter()
            .enumerate()
            .map(|(index, (sheet_name, _))| {
                let n = index + 1;
                format!(r#"<sheet name="{sheet_name}" sheetId="{n}" r:id="rId{n}"/>"#)
            })
            .collect();
        let workbook_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<sheets>{sheet_entries}</sheets>
</workbook>"#
        );
        zip.start_file("xl/workbook.xml", options).unwrap();
        zip.write_all(workbook_xml.as_bytes()).unwrap();

        for (index, (_, rows)) in sheets.iter().enumerate() {
            let mut sheet_rows = String::new();
            for (row_index, row) in rows.iter().enumerate() {
                let row_number = row_index + 1;
                let mut cells = String::new();
                for (col_index, value) in row.iter().enumerate() {
                    let column = column_letter(col_index);
                    let reference = format!("{column}{row_number}");
                    if let Some(expression) = value.strip_prefix('=') {
                        cells.push_str(&format!(
                            r#"<c r="{reference}"><f>{expression}</f><v>0</v></c>"#
                        ));
                    } else if let Some(iso) = value.strip_prefix('@') {
                        // `t="d"` (ISO 8601 date/datetime cell type): calamine reads this as
                        // `Data::DateTimeIso` with no `styles.xml` needed, unlike a numeric-serial
                        // date cell, which a real workbook's cell style decides.
                        cells.push_str(&format!(r#"<c r="{reference}" t="d"><v>{iso}</v></c>"#));
                    } else if value.is_empty() {
                        // No cell element at all: an XLSX omits genuinely blank cells.
                    } else if let Ok(number) = value.parse::<f64>() {
                        cells.push_str(&format!(r#"<c r="{reference}"><v>{number}</v></c>"#));
                    } else {
                        cells.push_str(&format!(
                            r#"<c r="{reference}" t="inlineStr"><is><t>{value}</t></is></c>"#
                        ));
                    }
                }
                sheet_rows.push_str(&format!(r#"<row r="{row_number}">{cells}</row>"#));
            }
            let sheet_xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{sheet_rows}</sheetData></worksheet>"#
            );
            zip.start_file(format!("xl/worksheets/sheet{}.xml", index + 1), options)
                .unwrap();
            zip.write_all(sheet_xml.as_bytes()).unwrap();
        }

        zip.finish().unwrap();
    }

    /// The common case of `write_xlsx`: one sheet.
    fn write_single_sheet_xlsx(path: &Path, sheet_name: &str, rows: &[&[&str]]) {
        write_xlsx(path, &[(sheet_name, rows)]);
    }

    fn column_letter(index: usize) -> String {
        let mut letters = Vec::new();
        let mut n = index;
        loop {
            letters.push((b'A' + (n % 26) as u8) as char);
            if n < 26 {
                break;
            }
            n = n / 26 - 1;
        }
        letters.iter().rev().collect()
    }

    fn open(path: &Path) -> Workbook {
        XlsxDataSource.open(path).expect("opens the fixture xlsx")
    }

    #[test]
    fn calamine_actually_opens_the_generated_fixture() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("check.xlsx");
        write_single_sheet_xlsx(&path, "Feuille1", &[&["a", "b"]]);

        let mut workbook: Xlsx<_> = calamine::open_workbook(&path).expect("calamine opens it");
        let range: Range<Data> = workbook.worksheet_range("Feuille1").expect("reads range");
        assert_eq!(range.get_size(), (1, 2));
    }

    #[test]
    fn every_sheet_is_read_with_its_own_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("patients.xlsx");
        write_single_sheet_xlsx(
            &path,
            "Consultations",
            &[&["nom", "age"], &["Camille", "42"]],
        );

        let workbook = open(&path);

        assert_eq!(workbook.sheets.len(), 1);
        assert_eq!(workbook.sheets[0].name, "Consultations");
        assert_eq!(workbook.sheets[0].rows[0][0], CellValue::Text("nom".into()));
    }

    #[test]
    fn a_workbook_with_several_sheets_keeps_every_one_of_them() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cabinet.xlsx");
        write_xlsx(
            &path,
            &[
                ("Consultations", &[&["nom", "age"], &["Camille", "42"]]),
                ("Facturation", &[&["nom", "montant"], &["Esaie", "75"]]),
            ],
        );

        let workbook = open(&path);

        assert_eq!(workbook.sheets.len(), 2);
        assert_eq!(workbook.sheets[0].name, "Consultations");
        assert_eq!(workbook.sheets[1].name, "Facturation");
        assert_eq!(workbook.sheets[1].rows[1][1], CellValue::Number(75.0));
    }

    #[test]
    fn a_numeric_cell_is_typed_as_a_number_not_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("montants.xlsx");
        write_single_sheet_xlsx(&path, "Feuille1", &[&["montant"], &["120.5"]]);

        let workbook = open(&path);

        assert_eq!(workbook.sheets[0].rows[1][0], CellValue::Number(120.5));
    }

    #[test]
    fn a_formula_keeps_its_expression_and_its_cached_value_apart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("totaux.xlsx");
        write_single_sheet_xlsx(
            &path,
            "Feuille1",
            &[&["a", "b", "total"], &["1", "2", "=A2+B2"]],
        );

        let workbook = open(&path);

        match &workbook.sheets[0].rows[1][2] {
            CellValue::Formula {
                expression,
                cached_value,
            } => {
                assert_eq!(expression, "A2+B2");
                // The fixture's cached <v> is 0: read verbatim, never recomputed.
                assert_eq!(cached_value.as_deref(), Some(&CellValue::Number(0.0)));
            }
            other => panic!("expected a formula cell, got {other:?}"),
        }
    }

    #[test]
    fn a_blank_cell_is_empty_not_an_empty_string() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trous.xlsx");
        write_single_sheet_xlsx(&path, "Feuille1", &[&["nom", "note"], &["Esaie", ""]]);

        let workbook = open(&path);

        assert_eq!(workbook.sheets[0].rows[1][1], CellValue::Empty);
    }

    // --- Dates (D3, docs/SESSION-DATA-10-Locale-Parsing.md section 6) ----------------------

    #[test]
    fn an_iso_date_cell_is_read_to_a_real_calendar_date() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("consultations.xlsx");
        write_single_sheet_xlsx(
            &path,
            "Feuille1",
            &[&["nom", "consultation"], &["Camille", "@2026-03-12"]],
        );

        let workbook = open(&path);

        assert_eq!(
            workbook.sheets[0].rows[1][1],
            CellValue::Date(chrono::NaiveDate::from_ymd_opt(2026, 3, 12).unwrap())
        );
    }

    #[test]
    fn an_xlsx_date_cell_and_a_csv_dd_mm_yyyy_cell_for_the_same_day_compare_equal() {
        use crate::tabular::csv_adapter::CsvDataSource;
        use crate::tabular::TabularDataSource;

        let dir = tempfile::tempdir().unwrap();
        let xlsx_path = dir.path().join("consultations.xlsx");
        write_single_sheet_xlsx(
            &xlsx_path,
            "Feuille1",
            &[&["nom", "consultation"], &["Camille", "@2026-03-12"]],
        );
        let xlsx = open(&xlsx_path);
        let CellValue::Date(xlsx_date) = xlsx.sheets[0].rows[1][1] else {
            panic!("expected a date cell from the xlsx adapter");
        };

        let csv_path = dir.path().join("consultations.csv");
        std::fs::write(&csv_path, "nom,consultation\nCamille,12/03/2026\n").unwrap();
        let csv = CsvDataSource.open(&csv_path).expect("opens the csv fixture");
        let CellValue::Text(csv_text) = &csv.sheets[0].rows[1][1] else {
            panic!("expected a text cell from the csv adapter");
        };
        let csv_date = chrono::NaiveDate::parse_from_str(csv_text, "%d/%m/%Y")
            .expect("the csv adapter's own text still parses as dd/mm/yyyy");

        assert_eq!(xlsx_date, csv_date);
    }
}
