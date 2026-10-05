//! Lot E, steps E4 and E5, on the owner's own publipostage files (copied verbatim into
//! `docs/test-reports/human-acceptance-pass-1/fixtures/publipostage/`): the order lines, the Word
//! letter whose placeholders are written with guillemets and whose table holds one template row,
//! and the text letter whose placeholders `[Prix_U]` and `[Total]` are abbreviations of columns.

use std::collections::HashSet;
use std::io::Read;
use std::path::PathBuf;

use assistant_cabinet_ai_lib::template_fill::{
    docx_placeholder_names, docx_text, fill_docx, fill_text, groups, line_columns, placeholder_names,
    propose_mapping, FillContext, MappingEntry, MappingStatus, Table,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("test-reports")
        .join("human-acceptance-pass-1")
        .join("fixtures")
        .join("publipostage")
        .join(name)
}

/// The order lines as a table of text (the file has no quoted comma).
fn orders() -> Table {
    let raw = std::fs::read_to_string(fixture("donnees_publipostage.csv")).expect("reads the data");
    let mut lines = raw.trim_start_matches('\u{feff}').lines();
    let columns: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    let rows = lines
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.split(',').map(str::to_string).collect())
        .collect();
    Table { columns, rows }
}

fn confirmed(mut mapping: Vec<MappingEntry>) -> Vec<MappingEntry> {
    for entry in &mut mapping {
        if entry.status == MappingStatus::Proposed {
            entry.status = MappingStatus::Exact;
        }
    }
    mapping
}

fn docx() -> Vec<u8> {
    std::fs::read(fixture("modele_lettre.docx")).expect("reads the Word template")
}

#[test]
fn the_word_template_placeholders_match_the_columns_exactly() {
    let table = orders();
    let names = docx_placeholder_names(&docx()).unwrap();
    // The note at the end of the file quotes a Word rule between guillemets, with spaces: not a field.
    assert!(!names.iter().any(|name| name.contains("Suivant")), "{names:?}");
    for expected in ["No_Commande", "Civilite", "Prenom", "Nom", "Article", "Quantite", "Prix_Unitaire", "Total_Ligne"] {
        assert!(names.contains(&expected.to_string()), "{names:?}");
    }
    let mapping = propose_mapping(&names, &table.columns);
    assert!(mapping.iter().all(|entry| entry.status == MappingStatus::Exact), "{mapping:?}");
}

#[test]
fn an_order_with_two_lines_gives_one_letter_with_two_table_rows() {
    let table = orders();
    let grouped = groups(&table, Some("No_Commande"));
    let lines = line_columns(&table, &grouped);
    let names = docx_placeholder_names(&docx()).unwrap();
    let mapping = confirmed(propose_mapping(&names, &table.columns));
    let group = grouped.iter().find(|group| group.key == "CMD-2026-001").unwrap();

    let filled = fill_docx(
        &docx(),
        &FillContext { table: &table, mapping: &mapping, line_columns: &lines, rows: &group.rows },
    )
    .unwrap();

    let text = docx_text(&filled.bytes).unwrap();
    assert!(text.contains("CMD-2026-001"), "{text}");
    assert!(text.contains("M. Jean Dupont"), "{text}");
    assert!(text.contains("Ordinateur Portable"), "{text}");
    assert!(text.contains("Souris Sans Fil"), "{text}");
    assert!(text.contains("899.0"), "{text}");
    assert!(text.contains("25.5"), "{text}");
    assert!(placeholder_names(&text).is_empty(), "no placeholder left: {:?}", placeholder_names(&text));
    assert!(filled.unresolved.is_empty(), "{:?}", filled.unresolved);
    // One header row, two filled rows.
    assert_eq!(text.matches("Ordinateur Portable").count(), 1);
    assert_eq!(text.matches("Souris Sans Fil").count(), 1);
}

#[test]
fn a_single_line_order_gives_one_row_and_the_other_orders_are_not_in_it() {
    let table = orders();
    let grouped = groups(&table, Some("No_Commande"));
    let lines = line_columns(&table, &grouped);
    let names = docx_placeholder_names(&docx()).unwrap();
    let mapping = confirmed(propose_mapping(&names, &table.columns));
    let group = grouped.iter().find(|group| group.key == "CMD-2026-002").unwrap();

    let filled = fill_docx(
        &docx(),
        &FillContext { table: &table, mapping: &mapping, line_columns: &lines, rows: &group.rows },
    )
    .unwrap();

    let text = docx_text(&filled.bytes).unwrap();
    assert!(text.contains("Mme Sophie Martin"), "{text}");
    assert!(text.contains("Chaise Ergonomique"), "{text}");
    assert!(!text.contains("Ordinateur"), "{text}");
    assert!(!text.contains("Dupont"), "{text}");
}

#[test]
fn every_other_part_of_the_word_file_is_copied_untouched() {
    let table = orders();
    let grouped = groups(&table, Some("No_Commande"));
    let lines = line_columns(&table, &grouped);
    let names = docx_placeholder_names(&docx()).unwrap();
    let mapping = confirmed(propose_mapping(&names, &table.columns));
    let filled = fill_docx(
        &docx(),
        &FillContext { table: &table, mapping: &mapping, line_columns: &lines, rows: &grouped[0].rows },
    )
    .unwrap();

    let read = |bytes: &[u8]| -> Vec<(String, Vec<u8>)> {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
        (0..archive.len())
            .map(|index| {
                let mut entry = archive.by_index(index).unwrap();
                let mut content = Vec::new();
                entry.read_to_end(&mut content).unwrap();
                (entry.name().to_string(), content)
            })
            .collect()
    };
    let before = read(&docx());
    let after = read(&filled.bytes);
    let names_before: Vec<&String> = before.iter().map(|(name, _)| name).collect();
    let names_after: Vec<&String> = after.iter().map(|(name, _)| name).collect();
    assert_eq!(names_before, names_after, "the same parts, in the same order");
    for ((name, old), (_, new)) in before.iter().zip(&after) {
        if name != "word/document.xml" {
            assert_eq!(old, new, "{name} must be untouched");
        }
    }
}

#[test]
fn the_text_template_abbreviations_are_proposed_never_filled_silently() {
    let table = orders();
    let template = std::fs::read_to_string(fixture("modele_lettre.txt")).unwrap();
    let names = placeholder_names(&template);
    let mapping = propose_mapping(&names, &table.columns);
    let status = |name: &str| mapping.iter().find(|entry| entry.placeholder == name).map(|e| e.status);
    assert_eq!(status("No_Commande"), Some(MappingStatus::Exact));
    assert_eq!(status("Prix_U"), Some(MappingStatus::Proposed));
    assert_eq!(status("Total"), Some(MappingStatus::Proposed));
    assert_eq!(
        mapping.iter().find(|entry| entry.placeholder == "Prix_U").and_then(|e| e.column.clone()),
        Some("Prix_Unitaire".to_string())
    );

    // Unconfirmed, a proposed placeholder is not filled: the letter keeps it as written.
    let grouped = groups(&table, Some("No_Commande"));
    let lines = line_columns(&table, &grouped);
    let unconfirmed: Vec<MappingEntry> = mapping
        .iter()
        .cloned()
        .map(|mut entry| {
            if entry.status == MappingStatus::Proposed {
                entry.column = None;
                entry.status = MappingStatus::Unresolved;
            }
            entry
        })
        .collect();
    let filled = fill_text(
        &template,
        &FillContext { table: &table, mapping: &unconfirmed, line_columns: &lines, rows: &grouped[1].rows },
    )
    .unwrap();
    assert!(filled.preview.contains("[Prix_U]"), "{}", filled.preview);
    assert!(filled.unresolved.contains(&"Prix_U".to_string()));
    assert!(filled.preview.contains("Chaise Ergonomique"), "{}", filled.preview);
}

#[test]
fn the_text_letter_is_filled_once_the_proposals_are_confirmed() {
    let table = orders();
    let template = std::fs::read_to_string(fixture("modele_lettre.txt")).unwrap();
    let names = placeholder_names(&template);
    let mapping = confirmed(propose_mapping(&names, &table.columns));
    let grouped = groups(&table, Some("No_Commande"));
    let lines = line_columns(&table, &grouped);
    let filled = fill_text(
        &template,
        &FillContext { table: &table, mapping: &mapping, line_columns: &lines, rows: &grouped[0].rows },
    )
    .unwrap();
    assert!(filled.preview.contains("Confirmation de votre commande n\u{b0} CMD-2026-001")
        || filled.preview.contains("CMD-2026-001"), "{}", filled.preview);
    assert!(filled.preview.contains("Bonjour M. Jean Dupont,"), "{}", filled.preview);
    assert!(filled.preview.contains("Ordinateur Portable"), "{}", filled.preview);
    assert!(filled.preview.contains("Souris Sans Fil"), "{}", filled.preview);
    assert!(filled.unresolved.is_empty(), "{:?}", filled.unresolved);
    let _unused: HashSet<String> = HashSet::new();
}
