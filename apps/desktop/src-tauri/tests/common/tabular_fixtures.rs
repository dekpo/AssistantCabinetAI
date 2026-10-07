//! The six fictional workbooks the tabular reference set (`tests/tabular_reference.rs`) asks its
//! questions about, written at test time into a temporary directory. No binary is committed.
//!
//! Every row comes from a rule simple enough to check by hand: each fixture states its rule and
//! the arithmetic behind every number the reference cases expect, so an expected value is
//! derived from this file, never from running the engine under test. Names and values are
//! fictional and say nothing about any profession.
//!
//! ASCII only: a non-ASCII byte is written as an escape (`\u{20ac}` for the euro sign).

#![allow(dead_code)]

use std::io::Write;
use std::path::Path;

pub const FACTURES: &str = "factures.csv";
pub const AGENCIES: &str = "agencies.csv";
pub const RENDEZ_VOUS: &str = "rendez-vous.xlsx";
pub const MIXTE: &str = "mixte.xlsx";
pub const DEPENSES: &str = "depenses.csv";
pub const MIXTE_MONTANTS: &str = "mixte-montants.csv";

/// Every fixture's file name, in the order `write_all` writes them.
pub const ALL: [&str; 6] = [
    FACTURES,
    AGENCIES,
    RENDEZ_VOUS,
    MIXTE,
    DEPENSES,
    MIXTE_MONTANTS,
];

/// Writes all six fixtures into `root`.
pub fn write_all(root: &Path) {
    std::fs::write(root.join(FACTURES), factures_csv()).unwrap();
    std::fs::write(root.join(AGENCIES), agencies_csv()).unwrap();
    write_xlsx(&root.join(RENDEZ_VOUS), &rendez_vous_sheets());
    write_xlsx(&root.join(MIXTE), &mixte_sheets());
    std::fs::write(root.join(DEPENSES), depenses_csv()).unwrap();
    std::fs::write(root.join(MIXTE_MONTANTS), mixte_montants_csv()).unwrap();
}

// --- factures.csv -------------------------------------------------------------------------------

pub const FACTURES_ROWS: usize = 60;
const SUPPLIERS: [&str; 4] = ["Alpha", "Beta", "Gamma", "Delta"];
const CLIENTS: [&str; 3] = ["Alpha", "Omega", "Sigma"];

/// CP1252, `;`, a one-cell title row above the real header, then 60 rows. Row `i` (0-based):
///
/// - `date`: 05/01/2026 (a Monday) plus `i` days, so 05/01/2026 to 05/03/2026. January holds
///   `i` = 0..=26 (27 rows), February `i` = 27..=54 (**28 rows**), March `i` = 55..=59 (5 rows).
///   From 01/02/2026 to 15/02/2026 is `i` = 27..=41, **15 rows**.
/// - `fournisseur`: `SUPPLIERS[i % 4]`, so Alpha, Beta, Gamma and Delta hold **15 rows** each.
/// - `client`: `CLIENTS[i % 3]`, so Alpha is also a client, on **20 rows**: `Alpha` sits in two
///   columns on purpose.
/// - `montant`: 1 000,50 EUR + 100 x `i`, written `1 000,50 \u{20ac}` with a space as the
///   thousands separator (always present: the smallest is 1 000,50, the largest `i` = 59 is
///   **6 900,50**). Total: 60 x 1 000,50 + 100 x (0 + ... + 59) = 60 030 + 177 000 =
///   **237 030**. Beta's rows (`i` = 1, 5, ..., 57): 15 x 1 000,50 + 100 x (15 x 58 / 2) =
///   15 007,50 + 43 500 = 58 507,50.
///
/// The euro sign is byte 0x80 in CP1252, so the file is not valid UTF-8 and the adapter must
/// take its CP1252 path.
pub fn factures_csv() -> Vec<u8> {
    let mut text = String::from("Export du 12/03/2026\r\ndate;fournisseur;client;montant\r\n");
    for i in 0..FACTURES_ROWS {
        let (day, month, year) = add_days_2026(5, 1, i as u32);
        let euros = 1000 + 100 * i;
        text.push_str(&format!(
            "{day:02}/{month:02}/{year};{};{};{} {:03},50 \u{20ac}\r\n",
            SUPPLIERS[i % 4],
            CLIENTS[i % 3],
            euros / 1000,
            euros % 1000,
        ));
    }
    let (bytes, _, unmappable) = encoding_rs::WINDOWS_1252.encode(&text);
    assert!(
        !unmappable,
        "every character of factures.csv exists in CP1252"
    );
    assert!(
        std::str::from_utf8(&bytes).is_err(),
        "factures.csv must not also be valid UTF-8"
    );
    bytes.into_owned()
}

/// `(day, month, year)` of `day/month/2026` plus `offset` days, within 2026 (not a leap year).
fn add_days_2026(day: u32, month: u32, offset: u32) -> (u32, u32, u32) {
    const LENGTHS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let (mut day, mut month) = (day + offset, month);
    while day > LENGTHS[(month - 1) as usize] {
        day -= LENGTHS[(month - 1) as usize];
        month += 1;
        assert!(month <= 12, "the fixture stays within 2026");
    }
    (day, month, 2026)
}

// --- agencies.csv -------------------------------------------------------------------------------

pub const AGENCIES_ROWS: usize = 2400;
const FIRST_AGENCIES: [&str; 4] = ["North", "South", "East", "West"];
/// The one row far larger than every other: `1234 % 4 == 2`, so it is an East row.
pub const AGENCIES_LARGEST_ROW: usize = 1234;

/// UTF-8, `,`, `agency,amount,year`, 2 400 rows. Row `i` (0-based data row):
///
/// - `i` < 2 000: `agency` is `FIRST_AGENCIES[i % 4]` (500 rows each), `amount` is `100.25`,
///   except row 1 234 (East), whose amount is `9999.75` - the largest single row.
/// - `i` >= 2 000: `agency` is `Harbor` (400 rows), `amount` is `150.50`. The group with the
///   largest **total** appears only after row 2 000.
/// - `year`: 2023 for `i` < 1 200, 2024 from 1 200 on (1 200 rows each).
///
/// Totals per agency: North = South = West = 500 x 100.25 = **50 125**; East = 499 x 100.25 +
/// 9 999.75 = 50 024.75 + 9 999.75 = **60 024.5**; Harbor = 400 x 150.5 = **60 200** (the
/// largest). Whole column: 3 x 50 125 + 60 024.5 + 60 200 = **270 599.5**. Year 2024 (rows
/// 1 200..=2 399): 799 x 100.25 + 9 999.75 + 60 200 = 80 099.75 + 9 999.75 + 60 200 =
/// **150 299.5**; year 2023: 1 200 x 100.25 = 120 300, and 120 300 + 150 299.5 = 270 599.5.
/// Minimum 100.25, maximum 9 999.75; every North row is 100.25, so North's maximum is 100.25.
/// Every amount is a multiple of 0.25, so every sum is exact in binary floating point.
pub fn agencies_csv() -> Vec<u8> {
    let mut text = String::from("agency,amount,year\n");
    for i in 0..AGENCIES_ROWS {
        let (agency, amount) = if i < 2000 {
            let amount = if i == AGENCIES_LARGEST_ROW {
                "9999.75"
            } else {
                "100.25"
            };
            (FIRST_AGENCIES[i % 4], amount)
        } else {
            ("Harbor", "150.50")
        };
        let year = if i < 1200 { 2023 } else { 2024 };
        text.push_str(&format!("{agency},{amount},{year}\n"));
    }
    text.into_bytes()
}

// --- rendez-vous.xlsx ---------------------------------------------------------------------------

pub const RENDEZ_VOUS_ROWS: usize = 40;
/// Excel's serial number for 02/03/2026, a Monday: 2026-01-01 is 46 023, plus 31 (January) and
/// 28 (February) is 01/03/2026 = 46 082, so 02/03/2026 = 46 083.
const MONDAY_2_MARCH_2026: u32 = 46083;
const ROOMS: [&str; 8] = [
    "Azur", "Azur", "Azur", "Azur", "Cedre", "Cedre", "Iris", "Lotus",
];

fn duration_of(room: &str) -> f64 {
    match room {
        "Azur" => 20.0,
        "Cedre" => 25.0,
        "Iris" => 45.0,
        "Lotus" => 60.0,
        _ => unreachable!(),
    }
}

/// One sheet, `Planning`: `date` (real Excel date cells, number format 14), `salle`,
/// `duree_min`, 40 rows. Row `i` (0-based):
///
/// - `date`: 02/03/2026 (a Monday) plus `i / 2` days - two appointments a day over 20 days,
///   02/03/2026 to 21/03/2026, three calendar weeks. Mondays are day offsets 0, 7 and 14, so
///   `i` in {0, 1, 14, 15, 28, 29}: **6 rows**. From 09/03/2026 to 15/03/2026 is day offsets
///   7..=13, `i` = 14..=27: **14 rows**.
/// - `salle`: `ROOMS[i % 8]`, so Azur **20** rows, Cedre **10**, Iris **5**, Lotus **5**.
/// - `duree_min`: fixed by the room - Azur 20, Cedre 25, Iris 45, Lotus 60.
///
/// Totals per room: Azur 20 x 20 = 400, Cedre 10 x 25 = 250, Iris 5 x 45 = **225** (the least),
/// Lotus 5 x 60 = 300; top three Azur, Lotus, Cedre. Whole column: **1 175**; mean 1 175 / 40 =
/// **29.375**; sorted, the values are twenty 20s then ten 25s, so the median (20th and 21st
/// values) is (20 + 25) / 2 = **22.5**. Mondays' rooms (`i % 8` = 0, 1, 6, 7, 4, 5): Azur, Azur,
/// Iris, Lotus, Cedre, Cedre, so 20 + 20 + 45 + 60 + 25 + 25 = **195** minutes. Sorted by
/// `duree_min` descending, the first row is the last Lotus row, `i` = 39.
pub fn rendez_vous_sheets() -> Vec<(String, Vec<Vec<Cell>>)> {
    let mut rows = vec![vec![
        Cell::text("date"),
        Cell::text("salle"),
        Cell::text("duree_min"),
    ]];
    for i in 0..RENDEZ_VOUS_ROWS {
        let room = ROOMS[i % 8];
        rows.push(vec![
            Cell::Date(MONDAY_2_MARCH_2026 + (i / 2) as u32),
            Cell::text(room),
            Cell::Number(duration_of(room)),
        ]);
    }
    vec![("Planning".to_string(), rows)]
}

// --- mixte.xlsx ---------------------------------------------------------------------------------

pub const DONNEES_ROWS: usize = 12;
pub const CALCULS_ROWS: usize = 10;
const ITEMS: [&str; 3] = ["Stylo", "Cahier", "Classeur"];

/// Two sheets side by side.
///
/// - `Donnees`, a clean grid: `article` (`ITEMS[i % 3]`), `quantite` (`i + 1`), `montant`
///   (2.5 x (`i + 1`)), 12 rows, no formula. Total `montant`: 2.5 x (1 + ... + 12) = 2.5 x 78 =
///   **195**.
/// - `Calculs`, formula-heavy: `libelle`, `base` (`i + 1`), `cumul` (`=B{row}*2` on every one of
///   its 10 rows, cached value written as twice the base). Ten formula cells over ten rows is far
///   above the 5 % a usable sheet may hold, so `Calculs` alone would be red; `Donnees` keeps the
///   workbook green (the per-sheet gate).
pub fn mixte_sheets() -> Vec<(String, Vec<Vec<Cell>>)> {
    let mut donnees = vec![vec![
        Cell::text("article"),
        Cell::text("quantite"),
        Cell::text("montant"),
    ]];
    for i in 0..DONNEES_ROWS {
        donnees.push(vec![
            Cell::text(ITEMS[i % 3]),
            Cell::Number((i + 1) as f64),
            Cell::Number(2.5 * (i + 1) as f64),
        ]);
    }
    let mut calculs = vec![vec![
        Cell::text("libelle"),
        Cell::text("base"),
        Cell::text("cumul"),
    ]];
    for i in 0..CALCULS_ROWS {
        let sheet_row = i + 2;
        calculs.push(vec![
            Cell::text(&format!("Ligne {}", i + 1)),
            Cell::Number((i + 1) as f64),
            Cell::Formula {
                expression: format!("B{sheet_row}*2"),
                cached: 2.0 * (i + 1) as f64,
            },
        ]);
    }
    vec![
        ("Donnees".to_string(), donnees),
        ("Calculs".to_string(), calculs),
    ]
}

// --- depenses.csv -------------------------------------------------------------------------------

pub const DEPENSES_ROWS: usize = 20;
const CATEGORIES: [&str; 4] = ["Bureau", "Transport", "Energie", "Logiciel"];

/// UTF-8 with a byte-order mark, `;`, `date;categorie;montant`, 20 rows. Row `i` (0-based):
///
/// - `date`: day `28 - i`, month `i % 12 + 1`, year `2025 + i / 12`, written `dd/mm/yyyy`. So the
///   rows are in calendar order, from 28/01/2025 (`i` = 0) to 09/08/2026 (`i` = 19), while their
///   days fall from 28 to 9: compared as text, the order is exactly reversed and "09/08/2026"
///   comes first.
/// - `categorie`: `CATEGORIES[i % 4]`.
/// - `montant`: `(i + 1),250`, a lone comma followed by exactly three digits in every value
///   (`1,250` to `20,250`) - the English thousands shape. Read as thousands: 1 000 x (1 + ... +
///   20) + 20 x 250 = 210 000 + 5 000 = **215 000**, largest **20 250**. Misread as a decimal
///   comma: 210 + 20 x 0.25 = 215, largest 20.25.
pub fn depenses_csv() -> Vec<u8> {
    let mut text = String::from("date;categorie;montant\n");
    for i in 0..DEPENSES_ROWS {
        text.push_str(&format!(
            "{:02}/{:02}/{};{};{},250\n",
            28 - i,
            i % 12 + 1,
            2025 + i / 12,
            CATEGORIES[i % 4],
            i + 1,
        ));
    }
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

// --- mixte-montants.csv -------------------------------------------------------------------------

pub const MIXTE_MONTANTS_ROWS: usize = 9;

/// UTF-8, `;`, `poste;montant`, 9 rows cycling through three comma shapes (`i % 3`):
///
/// - `12,50`: a decimal comma only - two digits after it, no thousands grouping possible;
/// - `1,234`: either reading - a decimal comma, or a valid three-digit grouping;
/// - `1,234,500`: a thousands grouping only - two commas cannot both be decimal separators.
///
/// Genuinely conflicting evidence in one column: one cell only readable as a decimal, another only
/// as thousands. There is no right number to expect - the reference set expects a refusal.
pub fn mixte_montants_csv() -> Vec<u8> {
    const SHAPES: [&str; 3] = ["12,50", "1,234", "1,234,500"];
    let mut text = String::from("poste;montant\n");
    for i in 0..MIXTE_MONTANTS_ROWS {
        text.push_str(&format!("Poste {};{}\n", i + 1, SHAPES[i % 3]));
    }
    text.into_bytes()
}

// --- A minimal real XLSX writer -----------------------------------------------------------------

/// One cell of a generated sheet.
#[derive(Debug, Clone)]
pub enum Cell {
    Text(String),
    Number(f64),
    /// An Excel date serial, written with style 1 (number format 14, `dd/mm/yyyy`), so `calamine`
    /// reads it as a real date cell rather than a number.
    Date(u32),
    /// A formula with the value the file caches for it.
    Formula {
        expression: String,
        cached: f64,
    },
}

impl Cell {
    pub fn text(value: &str) -> Self {
        Cell::Text(value.to_string())
    }
}

/// Writes a minimal but real `.xlsx` - a stored zip of the OOXML parts `calamine` needs, the same
/// way `tabular::xlsx_adapter`'s own tests do, plus a `styles.xml` whose second cell format is the
/// built-in date format 14, so a `Cell::Date` is a genuine Excel date cell.
pub fn write_xlsx(path: &Path, sheets: &[(String, Vec<Vec<Cell>>)]) {
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
<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
{overrides}
</Types>"#).as_bytes()).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#).unwrap();

    let sheet_rels: String = (1..=sheets.len())
        .map(|n| format!(r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{n}.xml"/>"#))
        .collect();
    let styles_id = sheets.len() + 1;
    zip.start_file("xl/_rels/workbook.xml.rels", options)
        .unwrap();
    zip.write_all(format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{sheet_rels}
<Relationship Id="rId{styles_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#).as_bytes()).unwrap();

    let sheet_entries: String = sheets
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            let n = index + 1;
            format!(r#"<sheet name="{name}" sheetId="{n}" r:id="rId{n}"/>"#)
        })
        .collect();
    zip.start_file("xl/workbook.xml", options).unwrap();
    zip.write_all(format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<sheets>{sheet_entries}</sheets>
</workbook>"#).as_bytes()).unwrap();

    zip.start_file("xl/styles.xml", options).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<fonts count="1"><font/></fonts>
<fills count="1"><fill/></fills>
<borders count="1"><border/></borders>
<cellStyleXfs count="1"><xf numFmtId="0"/></cellStyleXfs>
<cellXfs count="2"><xf numFmtId="0" xfId="0"/><xf numFmtId="14" xfId="0" applyNumberFormat="1"/></cellXfs>
</styleSheet>"#).unwrap();

    for (index, (_, rows)) in sheets.iter().enumerate() {
        let mut sheet_rows = String::new();
        for (row_index, row) in rows.iter().enumerate() {
            let row_number = row_index + 1;
            let mut cells = String::new();
            for (col_index, cell) in row.iter().enumerate() {
                let reference = format!("{}{row_number}", column_letter(col_index));
                cells.push_str(&match cell {
                    Cell::Text(value) => {
                        format!(r#"<c r="{reference}" t="inlineStr"><is><t>{value}</t></is></c>"#)
                    }
                    Cell::Number(value) => format!(r#"<c r="{reference}"><v>{value}</v></c>"#),
                    Cell::Date(serial) => {
                        format!(r#"<c r="{reference}" s="1"><v>{serial}</v></c>"#)
                    }
                    Cell::Formula { expression, cached } => {
                        format!(r#"<c r="{reference}"><f>{expression}</f><v>{cached}</v></c>"#)
                    }
                });
            }
            sheet_rows.push_str(&format!(r#"<row r="{row_number}">{cells}</row>"#));
        }
        zip.start_file(format!("xl/worksheets/sheet{}.xml", index + 1), options)
            .unwrap();
        zip.write_all(format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{sheet_rows}</sheetData></worksheet>"#).as_bytes()).unwrap();
    }

    zip.finish().unwrap();
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
