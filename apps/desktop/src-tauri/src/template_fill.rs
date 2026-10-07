//! Filling a template (a letter) with the rows of a table: the mail merge (HAP-1, lot E, steps E4
//! and E5; `docs/test-reports/human-acceptance-pass-1/11-lot-e-plan.md`).
//!
//! **Deterministic and local.** The rows of a table never go to a model (session 16), so the letter is
//! made by substituting cell values for placeholders here, in Rust, on the workstation. Nothing in this
//! module reads or writes a file, calls a model or knows about the interface: it takes a template (bytes),
//! a table (text) and a mapping, and returns the filled bytes. The command that writes the file after
//! the user approved it lives in `commands.rs`.
//!
//! What it understands:
//!
//! - **Placeholders** written `\u{ab}Name\u{bb}`, `[Name]` or `{{Name}}` (a name is letters, digits, `_`, `-`,
//!   `.` or a space; a bare number such as the citation `[1]` is not one).
//! - **A mapping** from each placeholder to a column. An exact name (case, accents, `_`/space/`-`
//!   ignored) is exact; a unique column the placeholder is a prefix of (`Prix_U` for `Prix_Unitaire`) is
//!   only **proposed**; anything else is unresolved. A proposal or an unresolved placeholder is never
//!   filled unless the user confirms it, and an unresolved one is left as written.
//! - **Several lines under one letter.** Rows are grouped by a key column (the order number). A column
//!   whose value differs between the rows of some group is a *line* column; a template line (text) or a
//!   table row (Word) that holds a line column is repeated once per row of the group; everything else
//!   is filled from the first row (the client, the order).
//!
//! Word files: the document part is edited as text and every other part of the file is copied
//! untouched. A placeholder split across several runs by Word is handled (the value goes in the first
//! run, the rest of the placeholder is removed).

use std::collections::HashSet;
use std::io::{Cursor, Read, Write};

use crate::file_reference::fold_text;

/// What kind of file a template is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateKind {
    Text,
    Docx,
}

impl TemplateKind {
    /// By extension, folded: `txt` and `md` are text, `docx` is Word. Anything else is not a template.
    pub fn of_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "txt" | "md" => Some(Self::Text),
            "docx" => Some(Self::Docx),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FillError {
    /// The template is not a readable Word file.
    UnreadableDocx,
    /// The template text is not valid UTF-8.
    UnreadableText,
    /// The group has no row to fill from.
    NoRow,
}

// ------------------------------------------------------------------------------------ placeholders

/// A placeholder found in a text: where it is (bytes of the text) and the name inside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub start: usize,
    pub end: usize,
    pub name: String,
}

fn is_name_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.' | ' ')
}

/// A name between guillemets or square brackets starts and ends with a character of the name: a
/// typographic quotation ("\u{ab} word \u{bb}", written with spaces inside) is not a field. Between double
/// braces, spaces around the name are customary and allowed.
fn valid_name(name: &str, spaces_allowed: bool) -> bool {
    if !spaces_allowed && (name.starts_with(char::is_whitespace) || name.ends_with(char::is_whitespace)) {
        return false;
    }
    let trimmed = name.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() <= 60
        && trimmed.chars().all(is_name_char)
        && !trimmed.chars().all(|ch| ch.is_ascii_digit())
}

/// Every placeholder of `text`, in order, with its position.
pub fn scan_placeholders(text: &str) -> Vec<Found> {
    let mut found = Vec::new();
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        let (open, close): (&str, &str) = if rest.starts_with('\u{ab}') {
            ("\u{ab}", "\u{bb}")
        } else if rest.starts_with("{{") {
            ("{{", "}}")
        } else if rest.starts_with('[') {
            ("[", "]")
        } else {
            index += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        };
        let inner_start = index + open.len();
        match text[inner_start..].find(close) {
            Some(length) if valid_name(&text[inner_start..inner_start + length], open == "{{") => {
                let end = inner_start + length + close.len();
                found.push(Found {
                    start: index,
                    end,
                    name: text[inner_start..inner_start + length].trim().to_string(),
                });
                index = end;
            }
            _ => index += open.len(),
        }
    }
    found
}

/// The distinct placeholder names of a text, in the order they first appear.
pub fn placeholder_names(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for item in scan_placeholders(text) {
        if !names.contains(&item.name) {
            names.push(item.name);
        }
    }
    names
}

// ---------------------------------------------------------------------------------------- mapping

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingStatus {
    /// The placeholder is the column's name.
    Exact,
    /// The placeholder is the beginning of one column's name: to be confirmed, never filled silently.
    Proposed,
    /// No column: left as written.
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingEntry {
    pub placeholder: String,
    pub column: Option<String>,
    pub status: MappingStatus,
}

/// A name with case, accents and every separator removed, so that `Prix_U`, `prix u` and `PRIX-U`
/// are one name.
fn compact(name: &str) -> String {
    fold_text(name).chars().filter(|ch| ch.is_alphanumeric()).collect()
}

/// A column for each placeholder: the exact one, a unique proposal, or none.
pub fn propose_mapping(placeholders: &[String], columns: &[String]) -> Vec<MappingEntry> {
    placeholders
        .iter()
        .map(|placeholder| {
            let wanted = compact(placeholder);
            if let Some(column) = columns.iter().find(|column| compact(column) == wanted) {
                return MappingEntry {
                    placeholder: placeholder.clone(),
                    column: Some(column.clone()),
                    status: MappingStatus::Exact,
                };
            }
            let beginning: Vec<&String> = columns
                .iter()
                .filter(|column| !wanted.is_empty() && compact(column).starts_with(&wanted))
                .collect();
            match beginning.as_slice() {
                [only] => MappingEntry {
                    placeholder: placeholder.clone(),
                    column: Some((*only).clone()),
                    status: MappingStatus::Proposed,
                },
                _ => MappingEntry {
                    placeholder: placeholder.clone(),
                    column: None,
                    status: MappingStatus::Unresolved,
                },
            }
        })
        .collect()
}

// ------------------------------------------------------------------------------------------ table

/// The rows to fill from, as the text a person sees in each cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|column| column == name)
    }
}

/// The rows sharing one value of the key column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub key: String,
    pub rows: Vec<usize>,
}

/// Rows grouped by the value of `key_column`, in the order the values first appear. With no key
/// (`None`), one group per row, keyed by its position.
pub fn groups(table: &Table, key_column: Option<&str>) -> Vec<Group> {
    let Some(key_index) = key_column.and_then(|name| table.column_index(name)) else {
        return (0..table.rows.len())
            .map(|row| Group {
                key: (row + 1).to_string(),
                rows: vec![row],
            })
            .collect();
    };
    let mut result: Vec<Group> = Vec::new();
    for (row, cells) in table.rows.iter().enumerate() {
        let key = cells.get(key_index).map(|cell| cell.trim().to_string()).unwrap_or_default();
        match result.iter_mut().find(|group| group.key == key) {
            Some(group) => group.rows.push(row),
            None => result.push(Group { key, rows: vec![row] }),
        }
    }
    result
}

/// The columns whose value differs between the rows of at least one group: the *line* columns
/// (article, quantity, price), as opposed to the client and order columns that never do.
pub fn line_columns(table: &Table, groups: &[Group]) -> HashSet<String> {
    let mut lines = HashSet::new();
    for (index, column) in table.columns.iter().enumerate() {
        let differs = groups.iter().any(|group| {
            let mut values = group
                .rows
                .iter()
                .map(|row| table.rows[*row].get(index).map(String::as_str).unwrap_or(""));
            match values.next() {
                Some(first) => values.any(|value| value != first),
                None => false,
            }
        });
        if differs {
            lines.insert(column.clone());
        }
    }
    lines
}

// ------------------------------------------------------------------------------------- the output

/// A letter made from one group of rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filled {
    pub bytes: Vec<u8>,
    /// What the user reads before approving: the text of the letter.
    pub preview: String,
    /// Placeholders left as written because no column was confirmed for them.
    pub unresolved: Vec<String>,
}

/// What to fill and from what, for one letter.
pub struct FillContext<'a> {
    pub table: &'a Table,
    pub mapping: &'a [MappingEntry],
    pub line_columns: &'a HashSet<String>,
    /// The rows of the group, first row first.
    pub rows: &'a [usize],
}

impl FillContext<'_> {
    fn column_of(&self, placeholder: &str) -> Option<usize> {
        let entry = self.mapping.iter().find(|entry| entry.placeholder == placeholder)?;
        self.table.column_index(entry.column.as_deref()?)
    }

    /// The value of `placeholder` in `row` (a position in `self.rows`), when a column is mapped.
    fn value(&self, placeholder: &str, row: usize) -> Option<String> {
        let column = self.column_of(placeholder)?;
        let table_row = *self.rows.get(row)?;
        Some(self.table.rows[table_row].get(column).cloned().unwrap_or_default())
    }

    /// Whether a placeholder reads a line column: its line (or table row) is repeated per row.
    fn is_line(&self, placeholder: &str) -> bool {
        self.mapping
            .iter()
            .find(|entry| entry.placeholder == placeholder)
            .and_then(|entry| entry.column.as_ref())
            .is_some_and(|column| self.line_columns.contains(column))
    }

    fn unresolved_in(&self, text: &str) -> Vec<String> {
        placeholder_names(text)
            .into_iter()
            .filter(|name| self.column_of(name).is_none())
            .collect()
    }
}

/// `text` with every placeholder the lookup knows replaced; the others stay as written.
fn substitute(text: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for item in scan_placeholders(text) {
        out.push_str(&text[last..item.start]);
        match lookup(&item.name) {
            Some(value) => out.push_str(&value),
            None => out.push_str(&text[item.start..item.end]),
        }
        last = item.end;
    }
    out.push_str(&text[last..]);
    out
}

/// A text template filled for one group. A line holding a line column is written once per row.
pub fn fill_text(template: &str, context: &FillContext<'_>) -> Result<Filled, FillError> {
    if context.rows.is_empty() {
        return Err(FillError::NoRow);
    }
    let mut out = String::new();
    for line in template.split_inclusive('\n') {
        let names = placeholder_names(line);
        if names.iter().any(|name| context.is_line(name)) {
            for row in 0..context.rows.len() {
                out.push_str(&substitute(line, &|name| context.value(name, row)));
                if !line.ends_with('\n') && row + 1 < context.rows.len() {
                    out.push('\n');
                }
            }
        } else {
            out.push_str(&substitute(line, &|name| context.value(name, 0)));
        }
    }
    Ok(Filled {
        preview: out.clone(),
        bytes: out.into_bytes(),
        unresolved: context.unresolved_in(template),
    })
}

// ------------------------------------------------------------------------------------- Word files

fn unescape_xml(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// One text run of a paragraph: where its text sits in the XML, and the text.
struct Run {
    /// Byte range of the text inside the XML (between `>` and `</w:t>`).
    start: usize,
    end: usize,
    text: String,
}

/// The `<w:t>` runs of a fragment, in order. A tag such as `<w:tab/>` or `<w:tbl>` is not one.
fn runs_of(xml: &str) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut from = 0;
    while let Some(offset) = xml[from..].find("<w:t") {
        let tag_start = from + offset;
        let after = xml[tag_start + 4..].chars().next();
        if !matches!(after, Some('>') | Some(' ')) {
            from = tag_start + 4;
            continue;
        }
        let Some(tag_end) = xml[tag_start..].find('>').map(|at| tag_start + at) else {
            break;
        };
        if xml[..tag_end].ends_with('/') {
            from = tag_end + 1;
            continue;
        }
        let Some(close) = xml[tag_end + 1..].find("</w:t>").map(|at| tag_end + 1 + at) else {
            break;
        };
        runs.push(Run {
            start: tag_end + 1,
            end: close,
            text: unescape_xml(&xml[tag_end + 1..close]),
        });
        from = close + 6;
    }
    runs
}

/// A paragraph (or any fragment) with its placeholders filled, a placeholder split across runs
/// included: the value goes in the run where the placeholder starts, the rest of it is removed.
fn fill_fragment(xml: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    let runs = runs_of(xml);
    if runs.is_empty() {
        return xml.to_string();
    }
    let full: String = runs.iter().map(|run| run.text.as_str()).collect();
    let found = scan_placeholders(&full);
    if found.is_empty() {
        return xml.to_string();
    }
    // Byte offset in `full` where each run starts.
    let mut run_starts = Vec::with_capacity(runs.len());
    let mut total = 0;
    for run in &runs {
        run_starts.push(total);
        total += run.text.len();
    }
    let mut texts: Vec<String> = runs.iter().map(|run| run.text.clone()).collect();
    // Work from the last placeholder to the first, so earlier offsets stay valid.
    for item in found.iter().rev() {
        let Some(value) = lookup(&item.name) else {
            continue;
        };
        let run_of = |offset: usize| {
            run_starts
                .iter()
                .rposition(|start| *start <= offset)
                .unwrap_or(0)
        };
        let first = run_of(item.start);
        let last = run_of(item.end.saturating_sub(1));
        for run in first..=last {
            let from = item.start.max(run_starts[run]) - run_starts[run];
            let to = item.end.min(run_starts[run] + runs[run].text.len()) - run_starts[run];
            if run == first {
                texts[run].replace_range(from..to, &value);
            } else {
                texts[run].replace_range(from..to, "");
            }
        }
    }
    let mut out = String::with_capacity(xml.len());
    let mut last = 0;
    for (run, text) in runs.iter().zip(&texts) {
        out.push_str(&xml[last..run.start]);
        // Keep leading and trailing spaces: Word drops them unless the run says to preserve them.
        let tag_start = out.rfind("<w:t").unwrap_or(0);
        if (text.starts_with(' ') || text.ends_with(' ')) && !out[tag_start..].contains("xml:space") {
            out.insert_str(tag_start + 4, " xml:space=\"preserve\"");
        }
        out.push_str(&escape_xml(text));
        last = run.end;
    }
    out.push_str(&xml[last..]);
    out
}

/// The text of a fragment of the document part: paragraphs on their own lines, cells of a row joined
/// by " | ". For the preview and for tests.
fn xml_text(xml: &str) -> String {
    let mut out = String::new();
    let mut index = 0;
    while index < xml.len() {
        let Some(open) = xml[index..].find('<').map(|at| index + at) else {
            break;
        };
        let Some(close) = xml[open..].find('>').map(|at| open + at) else {
            break;
        };
        let tag = &xml[open + 1..close];
        if tag == "/w:p" || tag == "w:br/" || tag.starts_with("w:br ") {
            out.push('\n');
        } else if tag == "w:tab/" {
            out.push('\t');
        } else if tag == "/w:tc" {
            out.push_str(" | ");
        } else if tag == "w:t" || tag.starts_with("w:t ") {
            if !tag.ends_with('/') {
                if let Some(end) = xml[close + 1..].find("</w:t>") {
                    out.push_str(&unescape_xml(&xml[close + 1..close + 1 + end]));
                    index = close + 1 + end + 6;
                    continue;
                }
            }
        }
        index = close + 1;
    }
    out.replace(" | \n", "\n").replace("\n\n", "\n")
}

/// The document part split into table rows and what is between them.
fn split_rows(xml: &str) -> Vec<(bool, &str)> {
    let mut parts = Vec::new();
    let mut from = 0;
    let mut search = 0;
    while let Some(offset) = xml[search..].find("<w:tr") {
        let start = search + offset;
        let after = xml[start + 5..].chars().next();
        if !matches!(after, Some('>') | Some(' ')) {
            search = start + 5;
            continue;
        }
        let Some(end) = xml[start..].find("</w:tr>").map(|at| start + at + 7) else {
            break;
        };
        parts.push((false, &xml[from..start]));
        parts.push((true, &xml[start..end]));
        from = end;
        search = end;
    }
    parts.push((false, &xml[from..]));
    parts
}

/// The placeholder names of a Word template, in order.
pub fn docx_placeholder_names(bytes: &[u8]) -> Result<Vec<String>, FillError> {
    let xml = read_document_part(bytes)?;
    // Per paragraph, so that a placeholder split across runs is still found.
    let text = xml
        .split("</w:p>")
        .map(|paragraph| runs_of(paragraph).into_iter().map(|run| run.text).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(placeholder_names(&text))
}

fn read_document_part(bytes: &[u8]) -> Result<String, FillError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| FillError::UnreadableDocx)?;
    let mut part = archive
        .by_name("word/document.xml")
        .map_err(|_| FillError::UnreadableDocx)?;
    let mut xml = String::new();
    part.read_to_string(&mut xml).map_err(|_| FillError::UnreadableDocx)?;
    Ok(xml)
}

/// A Word template filled for one group. A table row holding a line column is written once per row of
/// the group; every other part of the file is copied as it is.
pub fn fill_docx(template: &[u8], context: &FillContext<'_>) -> Result<Filled, FillError> {
    if context.rows.is_empty() {
        return Err(FillError::NoRow);
    }
    let xml = read_document_part(template)?;
    let mut output = String::with_capacity(xml.len());
    for (is_row, part) in split_rows(&xml) {
        if !is_row {
            output.push_str(&fill_fragment(part, &|name| context.value(name, 0)));
            continue;
        }
        let row_text: String = runs_of(part).into_iter().map(|run| run.text).collect();
        let repeated = placeholder_names(&row_text).iter().any(|name| context.is_line(name));
        if repeated {
            for row in 0..context.rows.len() {
                output.push_str(&fill_fragment(part, &|name| context.value(name, row)));
            }
        } else {
            output.push_str(&fill_fragment(part, &|name| context.value(name, 0)));
        }
    }

    let mut archive = zip::ZipArchive::new(Cursor::new(template)).map_err(|_| FillError::UnreadableDocx)?;
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| FillError::UnreadableDocx)?;
        let name = entry.name().to_string();
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(entry.compression())
            .unix_permissions(entry.unix_mode().unwrap_or(0o644));
        if entry.is_dir() {
            writer.add_directory(name, options).map_err(|_| FillError::UnreadableDocx)?;
            continue;
        }
        writer.start_file(name.clone(), options).map_err(|_| FillError::UnreadableDocx)?;
        if name == "word/document.xml" {
            writer.write_all(output.as_bytes()).map_err(|_| FillError::UnreadableDocx)?;
        } else {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|_| FillError::UnreadableDocx)?;
            writer.write_all(&bytes).map_err(|_| FillError::UnreadableDocx)?;
        }
    }
    let bytes = writer.finish().map_err(|_| FillError::UnreadableDocx)?.into_inner();
    let template_names = docx_placeholder_names(template)?;
    Ok(Filled {
        bytes,
        preview: xml_text(&output),
        unresolved: template_names
            .into_iter()
            .filter(|name| context.column_of(name).is_none())
            .collect(),
    })
}

/// The text of a Word file, for a preview or a test: paragraphs on their own lines.
pub fn docx_text(bytes: &[u8]) -> Result<String, FillError> {
    Ok(xml_text(&read_document_part(bytes)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        let columns = ["Civilite", "Nom", "No_Commande", "Article", "Quantite", "Prix_Unitaire"];
        let rows = [
            ["M.", "Dupont", "CMD-1", "Ordinateur", "1", "899"],
            ["M.", "Dupont", "CMD-1", "Souris", "1", "25.5"],
            ["Mme", "Martin", "CMD-2", "Chaise", "1", "189"],
            ["M.", "Rousseau", "CMD-3", "Ecran", "2", "199.99"],
            ["M.", "Rousseau", "CMD-3", "Clavier", "1", "79.5"],
        ];
        Table {
            columns: columns.iter().map(|c| c.to_string()).collect(),
            rows: rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect(),
        }
    }

    #[test]
    fn the_three_syntaxes_are_placeholders_and_a_citation_is_not() {
        assert_eq!(
            placeholder_names("A \u{ab}Nom\u{bb}, B [Prenom], C {{No_Commande}}, D [1], E [Prix U]"),
            vec!["Nom", "Prenom", "No_Commande", "Prix U"]
        );
        assert!(placeholder_names("see [1] and [2025] and [] and [a very long ......................................................... name]").is_empty());
    }

    #[test]
    fn an_exact_name_is_exact_a_unique_beginning_is_proposed_and_the_rest_is_unresolved() {
        let columns: Vec<String> = ["Prix_Unitaire", "Total_Ligne", "Nom", "Prenom"].iter().map(|c| c.to_string()).collect();
        let placeholders: Vec<String> = ["nom", "Prix_U", "Total", "Pr", "Nothing"].iter().map(|c| c.to_string()).collect();
        let mapping = propose_mapping(&placeholders, &columns);
        assert_eq!(mapping[0].status, MappingStatus::Exact);
        assert_eq!(mapping[0].column.as_deref(), Some("Nom"));
        assert_eq!(mapping[1].status, MappingStatus::Proposed);
        assert_eq!(mapping[1].column.as_deref(), Some("Prix_Unitaire"));
        assert_eq!(mapping[2].status, MappingStatus::Proposed);
        assert_eq!(mapping[2].column.as_deref(), Some("Total_Ligne"));
        // "Pr" begins two columns: never guessed.
        assert_eq!(mapping[3].status, MappingStatus::Unresolved);
        assert_eq!(mapping[4].status, MappingStatus::Unresolved);
    }

    #[test]
    fn rows_are_grouped_by_the_key_and_the_line_columns_are_those_that_vary() {
        let table = table();
        let groups = groups(&table, Some("No_Commande"));
        assert_eq!(groups.iter().map(|g| g.key.as_str()).collect::<Vec<_>>(), vec!["CMD-1", "CMD-2", "CMD-3"]);
        assert_eq!(groups[0].rows, vec![0, 1]);
        let lines = line_columns(&table, &groups);
        for column in ["Article", "Prix_Unitaire", "Quantite"] {
            assert!(lines.contains(column), "{column}");
        }
        for column in ["Civilite", "Nom", "No_Commande"] {
            assert!(!lines.contains(column), "{column}");
        }
    }

    #[test]
    fn with_no_key_every_row_is_its_own_letter() {
        assert_eq!(groups(&table(), None).len(), 5);
    }

    fn mapping_for(table: &Table, placeholders: &[&str]) -> Vec<MappingEntry> {
        let names: Vec<String> = placeholders.iter().map(|p| p.to_string()).collect();
        propose_mapping(&names, &table.columns)
            .into_iter()
            .map(|mut entry| {
                // Everything proposed is confirmed in these tests.
                if entry.status == MappingStatus::Proposed {
                    entry.status = MappingStatus::Exact;
                }
                entry
            })
            .collect()
    }

    #[test]
    fn a_text_letter_repeats_the_line_once_per_row_and_fills_the_rest_once() {
        let table = table();
        let grouped = groups(&table, Some("No_Commande"));
        let lines = line_columns(&table, &grouped);
        let mapping = mapping_for(&table, &["Civilite", "Nom", "No_Commande", "Article", "Quantite", "Prix_U", "Nothing"]);
        let template = "Order [No_Commande]\nDear [Civilite] [Nom],\n[Article] | [Quantite] | [Prix_U] EUR\nBye [Nothing]\n";
        let filled = fill_text(
            template,
            &FillContext { table: &table, mapping: &mapping, line_columns: &lines, rows: &grouped[0].rows },
        )
        .unwrap();
        assert_eq!(
            filled.preview,
            "Order CMD-1\nDear M. Dupont,\nOrdinateur | 1 | 899 EUR\nSouris | 1 | 25.5 EUR\nBye [Nothing]\n"
        );
        assert_eq!(filled.unresolved, vec!["Nothing".to_string()]);
    }

    #[test]
    fn a_single_line_order_gives_one_line() {
        let table = table();
        let grouped = groups(&table, Some("No_Commande"));
        let lines = line_columns(&table, &grouped);
        let mapping = mapping_for(&table, &["Article"]);
        let filled = fill_text(
            "[Article]\n",
            &FillContext { table: &table, mapping: &mapping, line_columns: &lines, rows: &grouped[1].rows },
        )
        .unwrap();
        assert_eq!(filled.preview, "Chaise\n");
    }

    #[test]
    fn a_placeholder_split_across_word_runs_is_filled_in_the_first_run() {
        let xml = "<w:p><w:r><w:t>Dear [No</w:t></w:r><w:r><w:t>m], and</w:t></w:r><w:r><w:t> more</w:t></w:r></w:p>";
        let filled = fill_fragment(xml, &|name| (name == "Nom").then(|| "Dupont & Fils".to_string()));
        assert_eq!(
            filled,
            "<w:p><w:r><w:t>Dear Dupont &amp; Fils</w:t></w:r><w:r><w:t>, and</w:t></w:r><w:r><w:t xml:space=\"preserve\"> more</w:t></w:r></w:p>"
        );
    }

    #[test]
    fn a_typographic_quotation_is_not_a_field() {
        // Written with spaces inside, as typographic quotation marks are.
        assert!(placeholder_names("the rule \u{ab} Next if \u{bb} of Word, or [ a ]").is_empty());
        assert_eq!(placeholder_names("{{ Nom }} et \u{ab}Nom\u{bb}"), vec!["Nom"]);
    }

    #[test]
    fn a_tab_or_a_table_tag_is_not_a_text_run() {
        let xml = "<w:p><w:r><w:tab/><w:t>[A]</w:t></w:r></w:p><w:tbl></w:tbl>";
        assert_eq!(runs_of(xml).len(), 1);
    }
}
