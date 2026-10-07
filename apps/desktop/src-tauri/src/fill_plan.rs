//! The mail merge as the interface drives it: a plan, its preview, and the files written once the user
//! approved (HAP-1, lot E, steps E4 and E5; `docs/test-reports/human-acceptance-pass-1/11-lot-e-plan.md`).
//!
//! `template_fill` makes one letter from a template, a table and a mapping; this module is what turns
//! that into a file action that follows the rules of the product (`AGENTS.md`, rule 3: a plan the user
//! approves, never a silent write):
//!
//! - **Nothing is written by a question.** A question that asks for a letter returns a [`FillPlan`]:
//!   the template found, the table and the row named, the field-to-column mapping and a preview. The
//!   files are written only by [`generate`], which the user starts from the plan card.
//! - **Nothing the interface sends is trusted.** The preview and the generation re-read the template and
//!   the table from their folders by relative path, through the inventories (a path that is not in the
//!   inventory resolves to nothing), and ignore a mapped column the table does not have.
//! - **New files only.** Letters go in a `Generated` subfolder of the documents folder, are named
//!   `<template>-<key>.<extension>` in the clean-name alphabet, and never overwrite: a taken name gets
//!   `-2`, `-3`. The template and the data are never touched. Each file written is logged without any
//!   cell value.
//!
//! No model is involved anywhere: the rows of a table never go to one.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::filename_sanitizer::sanitize_file_name;
use crate::inventory::WorkFolderInventory;
use crate::template_fill::{
    self, docx_placeholder_names, fill_docx, fill_text, groups, line_columns, placeholder_names,
    propose_mapping, FillContext, MappingEntry, MappingStatus, Table, TemplateKind,
};

/// The subfolder of the documents folder the letters are written in.
pub const OUTPUT_FOLDER: &str = "Generated";

/// The most letters one approval may write.
pub const MAX_LETTERS: usize = 500;

/// A template larger than this is not read.
const MAX_TEMPLATE_BYTES: u64 = 10 * 1024 * 1024;

/// A template file, read.
pub struct TemplateFile {
    /// Relative to the documents folder.
    pub relative_path: String,
    pub kind: TemplateKind,
    pub bytes: Vec<u8>,
    pub stem: String,
    pub extension: String,
}

impl TemplateFile {
    /// The placeholder names the file holds, in order.
    pub fn placeholders(&self) -> Vec<String> {
        match self.kind {
            TemplateKind::Text => String::from_utf8(self.bytes.clone())
                .map(|text| placeholder_names(&text))
                .unwrap_or_default(),
            TemplateKind::Docx => docx_placeholder_names(&self.bytes).unwrap_or_default(),
        }
    }
}

/// One template of the inventory, read. `None` for a file that is not in the inventory, is not a
/// template type, is too large or cannot be read.
pub fn read_template(inventory: &WorkFolderInventory, relative_path: &str) -> Option<TemplateFile> {
    let record = inventory.find_by_relative_path(relative_path)?;
    let kind = TemplateKind::of_extension(&record.extension)?;
    if record.size_bytes > MAX_TEMPLATE_BYTES {
        return None;
    }
    let path = inventory.absolute_path(record)?;
    let bytes = std::fs::read(path).ok()?;
    Some(TemplateFile {
        relative_path: record.relative_path.clone(),
        kind,
        bytes,
        stem: record.stem.clone(),
        extension: record.extension.clone(),
    })
}

/// The templates of the inventory (the documents the conversation may use) that hold at least one
/// placeholder naming, or beginning, a column of `columns`, the best match first.
pub fn candidate_templates(inventory: &WorkFolderInventory, columns: &[String]) -> Vec<TemplateFile> {
    let mut scored: Vec<(usize, TemplateFile)> = inventory
        .all_files()
        .iter()
        .filter_map(|record| read_template(inventory, &record.relative_path))
        .filter_map(|template| {
            let names = template.placeholders();
            let mapping = propose_mapping(&names, columns);
            let exact = mapping.iter().filter(|entry| entry.status == MappingStatus::Exact).count();
            let proposed = mapping.iter().filter(|entry| entry.status == MappingStatus::Proposed).count();
            (exact + proposed > 0).then_some((exact * 2 + proposed, template))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.relative_path.cmp(&b.1.relative_path)));
    scored.into_iter().map(|(_, template)| template).collect()
}

// ------------------------------------------------------------------------------- what is exchanged

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanPlaceholder {
    pub name: String,
    pub column: Option<String>,
    /// "exact", "proposed" or "unresolved": a proposal is only filled once the user confirms it.
    pub status: &'static str,
}

/// What the user sees before anything is written.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FillPlan {
    pub template: String,
    /// Every template that could be used, the chosen one first: the card lets her pick another.
    pub templates: Vec<String>,
    pub data_file: String,
    pub sheet: String,
    pub columns: Vec<String>,
    /// The column and the value the question named; `None` when it asked for every row.
    pub key_column: Option<String>,
    pub key_value: Option<String>,
    pub placeholders: Vec<PlanPlaceholder>,
    pub preview: FillPreview,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FillPreview {
    /// The text of the letter (the first one, when several would be written).
    pub text: String,
    /// How many letters the approval would write.
    pub letters: usize,
    /// How many table rows the previewed letter holds.
    pub lines: usize,
    /// Placeholders left as written, no column being confirmed for them.
    pub unresolved: Vec<String>,
    /// The name the previewed letter would get.
    pub file_name: String,
    /// The fields of the template and the column each would be filled from, as proposed (not as the
    /// request decided): what the card shows when another template is chosen.
    pub placeholders: Vec<PlanPlaceholder>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestMapping {
    pub placeholder: String,
    /// The column the user chose or confirmed; `None` leaves the placeholder as written.
    pub column: Option<String>,
}

/// What the interface asks for: always the user's decisions, never the data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillRequest {
    pub template: String,
    pub data_file: String,
    pub key_column: Option<String>,
    pub key_value: Option<String>,
    /// One letter per value of the key column (one per row with no key), instead of the one named.
    pub all: bool,
    pub mapping: Vec<RequestMapping>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Written {
    /// Relative to the documents folder.
    pub relative_path: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FillReport {
    pub written: Vec<Written>,
    pub unresolved: Vec<String>,
}

// ------------------------------------------------------------------------------------------- plan

fn status_code(status: MappingStatus) -> &'static str {
    match status {
        MappingStatus::Exact => "exact",
        MappingStatus::Proposed => "proposed",
        MappingStatus::Unresolved => "unresolved",
    }
}

/// The mapping the user decided on, checked against the real columns: a column the table does not
/// have is dropped (the placeholder then stays as written).
fn confirmed_mapping(request: &FillRequest, table: &Table) -> Vec<MappingEntry> {
    request
        .mapping
        .iter()
        .map(|entry| {
            let column = entry
                .column
                .as_ref()
                .filter(|column| table.columns.contains(column))
                .cloned();
            MappingEntry {
                placeholder: entry.placeholder.clone(),
                status: if column.is_some() { MappingStatus::Exact } else { MappingStatus::Unresolved },
                column,
            }
        })
        .collect()
}

fn output_name(template: &TemplateFile, key: &str) -> String {
    sanitize_file_name(&format!("{}-{}.{}", template.stem, key, template.extension))
}

/// The groups that would be written and the one previewed.
fn selection<'a>(
    request: &FillRequest,
    all_groups: &'a [template_fill::Group],
) -> Vec<&'a template_fill::Group> {
    if request.all {
        return all_groups.iter().collect();
    }
    let named = request
        .key_value
        .as_ref()
        .and_then(|value| all_groups.iter().find(|group| &group.key == value));
    named.or_else(|| all_groups.first()).into_iter().collect()
}

fn fill_group(
    template: &TemplateFile,
    table: &Table,
    mapping: &[MappingEntry],
    lines: &std::collections::HashSet<String>,
    rows: &[usize],
) -> Result<template_fill::Filled, AppError> {
    let context = FillContext {
        table,
        mapping,
        line_columns: lines,
        rows,
    };
    let unreadable = || AppError::FillTemplateUnreadable {
        path: template.relative_path.clone(),
    };
    match template.kind {
        TemplateKind::Text => {
            let text = String::from_utf8(template.bytes.clone()).map_err(|_| unreadable())?;
            fill_text(&text, &context).map_err(|_| unreadable())
        }
        TemplateKind::Docx => fill_docx(&template.bytes, &context).map_err(|_| unreadable()),
    }
}

/// The preview of what `request` would write.
pub fn preview(template: &TemplateFile, table: &Table, request: &FillRequest) -> Result<FillPreview, AppError> {
    let all_groups = groups(table, request.key_column.as_deref());
    let chosen = selection(request, &all_groups);
    let first = chosen.first().ok_or(AppError::FillNothingToFill)?;
    let mapping = confirmed_mapping(request, table);
    let lines = line_columns(table, &all_groups);
    let filled = fill_group(template, table, &mapping, &lines, &first.rows)?;
    Ok(FillPreview {
        text: filled.preview,
        letters: chosen.len(),
        lines: first.rows.len(),
        unresolved: filled.unresolved,
        file_name: output_name(template, &first.key),
        placeholders: propose_mapping(&template.placeholders(), &table.columns)
            .into_iter()
            .map(|entry| PlanPlaceholder {
                name: entry.placeholder,
                column: entry.column,
                status: status_code(entry.status),
            })
            .collect(),
    })
}

/// The plan for a question that asked for a letter: the best template, the mapping it can propose and
/// the preview with only the exact matches filled.
pub fn build_plan(
    templates: Vec<TemplateFile>,
    data_file: &str,
    sheet: &str,
    table: &Table,
    key: Option<(String, String)>,
) -> Result<FillPlan, AppError> {
    let chosen = templates.first().ok_or(AppError::FillNothingToFill)?;
    let names = chosen.placeholders();
    let proposed = propose_mapping(&names, &table.columns);
    let (key_column, key_value) = match key {
        Some((column, value)) => (Some(column), Some(value)),
        None => (None, None),
    };
    let request = FillRequest {
        template: chosen.relative_path.clone(),
        data_file: data_file.to_string(),
        key_column: key_column.clone(),
        key_value: key_value.clone(),
        all: key_value.is_none(),
        // Only the exact matches are filled in the first preview: a proposal waits for her.
        mapping: proposed
            .iter()
            .map(|entry| RequestMapping {
                placeholder: entry.placeholder.clone(),
                column: (entry.status == MappingStatus::Exact).then(|| entry.column.clone()).flatten(),
            })
            .collect(),
    };
    let preview = preview(chosen, table, &request)?;
    Ok(FillPlan {
        template: chosen.relative_path.clone(),
        templates: templates.iter().map(|template| template.relative_path.clone()).collect(),
        data_file: data_file.to_string(),
        sheet: sheet.to_string(),
        columns: table.columns.clone(),
        key_column,
        key_value,
        placeholders: proposed
            .into_iter()
            .map(|entry| PlanPlaceholder {
                name: entry.placeholder,
                column: entry.column,
                status: status_code(entry.status),
            })
            .collect(),
        preview,
    })
}

// ------------------------------------------------------------------------------------- generation

/// A name not yet taken in `directory`, and the file created under it. `create_new` makes the check
/// and the creation one step, so a name taken between the two is never overwritten.
fn create_unique(directory: &Path, name: &str) -> std::io::Result<(PathBuf, std::fs::File)> {
    let (stem, extension) = match name.rfind('.') {
        Some(position) if position > 0 => (&name[..position], &name[position..]),
        _ => (name, ""),
    };
    for attempt in 1..=10_000 {
        let candidate = if attempt == 1 {
            name.to_string()
        } else {
            format!("{stem}-{attempt}{extension}")
        };
        let path = directory.join(&candidate);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "no free name"))
}

/// Writes the letters `request` describes into `<work_folder>/Generated`, and logs each one to
/// `log_path` (the template, the data file, the key and the output name: no cell value).
pub fn generate(
    work_folder: &Path,
    log_path: &Path,
    now_seconds: i64,
    template: &TemplateFile,
    table: &Table,
    request: &FillRequest,
) -> Result<FillReport, AppError> {
    let all_groups = groups(table, request.key_column.as_deref());
    let chosen = selection(request, &all_groups);
    if chosen.is_empty() {
        return Err(AppError::FillNothingToFill);
    }
    if chosen.len() > MAX_LETTERS {
        return Err(AppError::FillTooMany {
            count: chosen.len(),
            limit: MAX_LETTERS,
        });
    }
    let mapping = confirmed_mapping(request, table);
    let lines = line_columns(table, &all_groups);
    let directory = work_folder.join(OUTPUT_FOLDER);
    std::fs::create_dir_all(&directory).map_err(|_| AppError::FillWriteFailed)?;

    let mut written = Vec::new();
    let mut unresolved: Vec<String> = Vec::new();
    for group in chosen {
        let filled = fill_group(template, table, &mapping, &lines, &group.rows)?;
        for name in &filled.unresolved {
            if !unresolved.contains(name) {
                unresolved.push(name.clone());
            }
        }
        let (path, mut file) =
            create_unique(&directory, &output_name(template, &group.key)).map_err(|_| AppError::FillWriteFailed)?;
        file.write_all(&filled.bytes).map_err(|_| AppError::FillWriteFailed)?;
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let relative = format!("{OUTPUT_FOLDER}/{file_name}");
        append_log(log_path, now_seconds, template, &request.data_file, &group.key, &relative);
        written.push(Written {
            relative_path: relative,
            key: group.key.clone(),
        });
    }
    Ok(FillReport { written, unresolved })
}

/// One line per file written: when, the template, the data file, the key and the output. Best effort:
/// a log that cannot be written never stops the letter, which is already on disk.
fn append_log(log_path: &Path, now_seconds: i64, template: &TemplateFile, data_file: &str, key: &str, output: &str) {
    let line = serde_json::json!({
        "at": now_seconds,
        "template": template.relative_path,
        "dataFile": data_file,
        "key": key,
        "output": output,
    });
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        Table {
            columns: ["Nom", "No_Commande", "Article", "Prix_Unitaire"].iter().map(|c| c.to_string()).collect(),
            rows: [
                ["Dupont", "CMD-1", "Ordinateur", "899"],
                ["Dupont", "CMD-1", "Souris", "25.5"],
                ["Martin", "CMD-2", "Chaise", "189"],
            ]
            .iter()
            .map(|row| row.iter().map(|c| c.to_string()).collect())
            .collect(),
        }
    }

    fn template() -> TemplateFile {
        TemplateFile {
            relative_path: "modele.txt".to_string(),
            kind: TemplateKind::Text,
            bytes: b"Order [No_Commande] for [Nom]\n[Article] [Prix_U]\n".to_vec(),
            stem: "modele".to_string(),
            extension: "txt".to_string(),
        }
    }

    fn request(all: bool, key_value: Option<&str>, prix: Option<&str>) -> FillRequest {
        FillRequest {
            template: "modele.txt".to_string(),
            data_file: "donnees.csv".to_string(),
            key_column: Some("No_Commande".to_string()),
            key_value: key_value.map(str::to_string),
            all,
            mapping: vec![
                RequestMapping { placeholder: "No_Commande".into(), column: Some("No_Commande".into()) },
                RequestMapping { placeholder: "Nom".into(), column: Some("Nom".into()) },
                RequestMapping { placeholder: "Article".into(), column: Some("Article".into()) },
                RequestMapping { placeholder: "Prix_U".into(), column: prix.map(str::to_string) },
            ],
        }
    }

    #[test]
    fn the_first_plan_fills_only_what_is_exact_and_waits_for_the_proposal() {
        let plan = build_plan(vec![template()], "donnees.csv", "feuille", &table(), Some(("No_Commande".into(), "CMD-1".into()))).unwrap();
        let prix = plan.placeholders.iter().find(|p| p.name == "Prix_U").unwrap();
        assert_eq!(prix.status, "proposed");
        assert_eq!(prix.column.as_deref(), Some("Prix_Unitaire"));
        assert!(plan.preview.text.contains("Order CMD-1 for Dupont"), "{}", plan.preview.text);
        assert!(plan.preview.text.contains("[Prix_U]"), "the proposal is not applied: {}", plan.preview.text);
        assert_eq!(plan.preview.letters, 1);
        assert_eq!(plan.preview.lines, 2);
        assert_eq!(plan.preview.unresolved, vec!["Prix_U".to_string()]);
        assert_eq!(plan.preview.file_name, "modele-CMD-1.txt");
    }

    #[test]
    fn a_confirmed_mapping_fills_it_and_a_column_that_does_not_exist_is_ignored() {
        let confirmed = preview(&template(), &table(), &request(false, Some("CMD-1"), Some("Prix_Unitaire"))).unwrap();
        assert!(confirmed.text.contains("Ordinateur 899"), "{}", confirmed.text);
        assert!(confirmed.unresolved.is_empty());
        let invented = preview(&template(), &table(), &request(false, Some("CMD-1"), Some("Secret"))).unwrap();
        assert!(invented.text.contains("[Prix_U]"), "{}", invented.text);
    }

    #[test]
    fn all_means_one_letter_per_order_not_per_line() {
        let all = preview(&template(), &table(), &request(true, None, Some("Prix_Unitaire"))).unwrap();
        assert_eq!(all.letters, 2);
    }

    #[test]
    fn letters_are_new_files_in_the_generated_folder_and_never_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("log").join("generated.jsonl");
        let request = request(true, None, Some("Prix_Unitaire"));
        let first = generate(directory.path(), &log, 1, &template(), &table(), &request).unwrap();
        assert_eq!(
            first.written.iter().map(|w| w.relative_path.as_str()).collect::<Vec<_>>(),
            vec!["Generated/modele-CMD-1.txt", "Generated/modele-CMD-2.txt"]
        );
        let letter = std::fs::read_to_string(directory.path().join("Generated").join("modele-CMD-1.txt")).unwrap();
        assert!(letter.contains("Souris 25.5"), "{letter}");

        // The same request again: the names are taken, so new ones are made and nothing is touched.
        let second = generate(directory.path(), &log, 2, &template(), &table(), &request).unwrap();
        assert_eq!(
            second.written.iter().map(|w| w.relative_path.as_str()).collect::<Vec<_>>(),
            vec!["Generated/modele-CMD-1-2.txt", "Generated/modele-CMD-2-2.txt"]
        );
        assert_eq!(
            std::fs::read_to_string(directory.path().join("Generated").join("modele-CMD-1.txt")).unwrap(),
            letter,
            "the first letter is unchanged"
        );
    }

    #[test]
    fn the_log_names_the_template_and_the_key_and_no_cell() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory.path().join("generated.jsonl");
        generate(directory.path(), &log, 5, &template(), &table(), &request(false, Some("CMD-2"), Some("Prix_Unitaire"))).unwrap();
        let line = std::fs::read_to_string(&log).unwrap();
        assert!(line.contains("modele.txt") && line.contains("CMD-2") && line.contains("Generated/modele-CMD-2.txt"), "{line}");
        for cell in ["Martin", "Chaise", "189"] {
            assert!(!line.contains(cell), "{line}");
        }
    }

    #[test]
    fn too_many_letters_in_one_approval_are_refused_before_anything_is_written() {
        let big = Table {
            columns: vec!["No_Commande".to_string(), "Nom".to_string()],
            rows: (0..MAX_LETTERS + 1).map(|n| vec![format!("C{n}"), "x".to_string()]).collect(),
        };
        let directory = tempfile::tempdir().unwrap();
        let mut ask = request(true, None, None);
        ask.mapping.clear();
        let error = generate(directory.path(), &directory.path().join("l.jsonl"), 0, &template(), &big, &ask).unwrap_err();
        assert!(matches!(error, AppError::FillTooMany { .. }));
        assert!(!directory.path().join(OUTPUT_FOLDER).exists());
    }
}
