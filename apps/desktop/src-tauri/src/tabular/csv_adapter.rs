//! CSV adapter: real French/European exports, not textbook CSV
//! (`docs/SESSION-DATA-03-TABULAR-Core.md`).
//!
//! Three things a naive reader gets wrong on this workstation's real files: the encoding is not
//! always UTF-8, the delimiter is not always a comma, and a comma inside a field is not always a
//! delimiter - it is as often the decimal separator of a French number. All three are decided
//! from the file itself, never assumed.

use std::path::Path;

use super::{CellValue, SheetData, TabularDataSource, TabularError, Workbook};

pub struct CsvDataSource;

impl TabularDataSource for CsvDataSource {
    fn open(&self, path: &Path) -> Result<Workbook, TabularError> {
        let bytes = std::fs::read(path).map_err(|_| TabularError::ReadFailed)?;
        let text = decode(&bytes);
        let delimiter = detect_delimiter(&text);

        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(false)
            .flexible(true)
            .from_reader(text.as_bytes());

        let mut rows = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|_| TabularError::ParseFailed)?;
            let row = record
                .iter()
                .map(|field| {
                    if field.trim().is_empty() {
                        CellValue::Empty
                    } else {
                        CellValue::Text(field.to_string())
                    }
                })
                .collect();
            rows.push(row);
        }

        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "sheet".to_string());

        Ok(Workbook {
            sheets: vec![SheetData { name, rows }],
        })
    }
}

/// UTF-8, with a leading byte-order mark stripped when present, if the bytes decode cleanly;
/// CP1252 otherwise. A CP1252 file with an accented character is never valid UTF-8, so the
/// fallback never fires on a file that genuinely is UTF-8, and there is nothing to detect beyond
/// "did the strict decode succeed".
fn decode(bytes: &[u8]) -> String {
    let without_bom = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(without_bom) {
        Ok(text) => text.to_string(),
        Err(_) => {
            let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(without_bom);
            decoded.into_owned()
        }
    }
}

/// `,` or `;`, decided by which one slices every line into the same field count most
/// consistently - never by which character simply appears more often in the bytes. A French
/// export's decimal commas (`1234,56`) can easily outnumber the file's real semicolon
/// delimiters; counting raw characters would read every such number as extra columns instead of
/// one, so the two candidates are scored by how rectangular a table each one produces.
fn detect_delimiter(text: &str) -> u8 {
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if lines.is_empty() {
        return b',';
    }
    let comma = consistency_score(&lines, b',');
    let semicolon = consistency_score(&lines, b';');
    if semicolon.1 > 1 && semicolon >= comma {
        b';'
    } else {
        b','
    }
}

/// `(how many lines share the most common field count, that field count)`. Compared as a tuple
/// so consistency always outranks width: a delimiter that slices most rows the same way beats
/// one that merely finds more fields in a handful of them.
fn consistency_score(lines: &[&str], delimiter: u8) -> (usize, usize) {
    let mut frequency: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for line in lines {
        let width = count_unquoted(line, delimiter) + 1;
        *frequency.entry(width).or_insert(0) += 1;
    }
    frequency
        .into_iter()
        .map(|(width, freq)| (freq, width))
        .max()
        .unwrap_or((0, 0))
}

/// Delimiter occurrences outside a quoted field, so a quoted value containing the delimiter
/// itself is not mistaken for an extra column.
fn count_unquoted(line: &str, delimiter: u8) -> usize {
    let mut in_quotes = false;
    let mut count = 0;
    for byte in line.bytes() {
        match byte {
            b'"' => in_quotes = !in_quotes,
            value if value == delimiter && !in_quotes => count += 1,
            _ => {}
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_bytes(name: &str, bytes: &[u8]) -> Workbook {
        let dir = tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        CsvDataSource.open(&path).expect("opens")
    }

    fn cell_texts(workbook: &Workbook) -> Vec<Vec<Option<String>>> {
        workbook.sheets[0]
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| match cell {
                        CellValue::Text(text) => Some(text.clone()),
                        CellValue::Empty => None,
                        other => panic!("unexpected cell from a CSV adapter: {other:?}"),
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_plain_utf8_comma_file_parses_verbatim() {
        let workbook = open_bytes("patients.csv", b"nom,age\nCamille,42\nEsaie,7\n");

        assert_eq!(
            cell_texts(&workbook),
            vec![
                vec![Some("nom".into()), Some("age".into())],
                vec![Some("Camille".into()), Some("42".into())],
                vec![Some("Esaie".into()), Some("7".into())],
            ]
        );
    }

    #[test]
    fn a_utf8_bom_is_stripped_before_parsing() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("nom,ville\nEsaie,Nimes\n".as_bytes());
        let workbook = open_bytes("bom.csv", &bytes);

        assert_eq!(workbook.sheets[0].rows[0][0], CellValue::Text("nom".into()));
    }

    #[test]
    fn a_cp1252_file_decodes_its_accents() {
        // "R\xe9sum\xe9" is CP1252 for "Résumé"; not valid UTF-8 on its own.
        let mut bytes = b"nom;ville\n".to_vec();
        bytes.extend_from_slice(&[
            b'R', 0xE9, b's', b'u', b'm', 0xE9, b';', b'N', 0xEE, b'm', b'e', b's',
        ]);
        assert!(
            std::str::from_utf8(&bytes).is_err(),
            "fixture must not already be valid UTF-8"
        );

        let workbook = open_bytes("cp1252.csv", &bytes);

        assert_eq!(
            workbook.sheets[0].rows[1][0],
            CellValue::Text("Résumé".into())
        );
        assert_eq!(
            workbook.sheets[0].rows[1][1],
            CellValue::Text("Nîmes".into())
        );
    }

    #[test]
    fn a_semicolon_delimited_french_file_is_never_read_as_comma_delimited() {
        let workbook = open_bytes(
            "montants.csv",
            "nom;montant;date\nCamille;120,50;12/03/2026\nEsaie;75,00;05/01/2026\n".as_bytes(),
        );

        assert_eq!(
            cell_texts(&workbook),
            vec![
                vec![
                    Some("nom".into()),
                    Some("montant".into()),
                    Some("date".into())
                ],
                vec![
                    Some("Camille".into()),
                    Some("120,50".into()),
                    Some("12/03/2026".into())
                ],
                vec![
                    Some("Esaie".into()),
                    Some("75,00".into()),
                    Some("05/01/2026".into())
                ],
            ]
        );
    }

    #[test]
    fn a_comma_delimited_file_is_not_confused_by_a_semicolon_in_a_free_text_field() {
        let workbook = open_bytes(
            "notes.csv",
            "nom,note\nCamille,\"a vu le Dr X; suivi prevu\"\n".as_bytes(),
        );

        assert_eq!(
            workbook.sheets[0].rows[1][1],
            CellValue::Text("a vu le Dr X; suivi prevu".into())
        );
    }

    #[test]
    fn an_empty_file_produces_no_rows() {
        let workbook = open_bytes("empty.csv", b"");

        assert!(workbook.sheets[0].rows.is_empty());
    }
}
