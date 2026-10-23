//! A letter template drafted by the model from the column names alone (TABULAR step 09; owner
//! decision of 20 October 2026, `docs/SESSION-TABULAR/SESSION-TABULAR-00-PLAN.md`, decision 3).
//!
//! A letter is asked for each row (or one row) of a workbook the question names and no template file
//! fits. The model is asked for **one template**, written with placeholders; the program fills it
//! for every row exactly as it does for a template file (`fill_plan`, `template_fill`). What the model
//! reads is the names of the fields, the kind of each column and the user's own request: **no cell
//! value, no row, no total**.
//!
//! A placeholder name is made of letters, digits, `_`, `-`, `.` and spaces, at most 60 characters
//! (`template_fill::valid_name`), so a column called `Montant (TTC)` or `N\u{b0} facture` cannot be written
//! as one. The program therefore gives the model a **placeholder name for every column** (the header
//! reduced to that alphabet, made unique when two collapse) and keeps the table `placeholder -> column`
//! itself: the mapping of a draft is known, not guessed. A name the model altered slightly goes through
//! `propose_mapping` and waits for the user; a name that matches nothing stays unresolved and is shown.
//!
//! Tauri-free: the gateway is reached through `GatewayClient`, like the other single calls
//! (`calculation_follow_up::explain`). The user's own stop ends the draft; a gateway that cannot be
//! reached or a reply that holds no placeholder is a failure the caller turns into an honest answer.

use std::time::Duration;

use crate::error::AppError;
use crate::gateway::{ChatTurn, GatewayClient, GROUNDED_TEMPERATURE};
use crate::template_fill::{
    compact, is_name_char, placeholder_names, propose_mapping, MappingEntry, MappingStatus,
};

/// A drafted template larger than this is cut, and an edited one larger than this is refused.
pub const MAX_DRAFT_BYTES: usize = 20_000;

/// The longest placeholder name (`template_fill::valid_name`).
const MAX_NAME_CHARS: usize = 60;

/// English, profession-neutral and short, like every instruction of the product: a small model echoes a
/// long list of rules back into its answer. The language of the letter follows the output-language
/// directive the gateway adds, not the language of this text.
pub const TEMPLATE_DRAFT_INSTRUCTION: &str =
    "You are given the names of the fields of a table and a request for a letter. Write one letter \
     template as plain text. Every personalised part is a placeholder written between double curly \
     braces, like {{Field}}, using the field names exactly as listed and no other placeholder. Write \
     no value, date, amount or name that is not a placeholder. Follow the tone and the purpose of the \
     request. Output only the template: no commentary and no code block.";

/// Added to the second request, when the first answer held no placeholder.
const STRICTER_REMINDER: &str =
    "Your previous answer held no placeholder. Write the template again: it must contain placeholders \
     written exactly like {{Field}}, with the field names as listed, and nothing else around it.";

/// One column of the table, and the placeholder name the model is given for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub placeholder: String,
    /// The column as the table names it.
    pub column: String,
    /// `date`, `numeric` or `text` when the analysis knows it.
    pub kind: Option<String>,
}

/// A placeholder name for each column, in order: the header reduced to the alphabet a placeholder
/// accepts, `Column N` when nothing usable is left, made unique when two names collapse into one
/// (`-2`, `-3`). `kinds` are the types the analysis found, by position; shorter than `columns` is fine.
pub fn fields_of(columns: &[String], kinds: &[Option<String>]) -> Vec<Field> {
    let mut fields: Vec<Field> = Vec::with_capacity(columns.len());
    for (position, column) in columns.iter().enumerate() {
        let base = reduced_name(column, position);
        let mut name = base.clone();
        let mut attempt = 2;
        while fields
            .iter()
            .any(|each| compact(&each.placeholder) == compact(&name))
        {
            let suffix = format!("-{attempt}");
            let room = MAX_NAME_CHARS - suffix.chars().count();
            name = format!("{}{suffix}", base.chars().take(room).collect::<String>());
            attempt += 1;
        }
        fields.push(Field {
            placeholder: name,
            column: column.clone(),
            kind: kinds.get(position).cloned().flatten(),
        });
    }
    fields
}

/// The header with every character a placeholder cannot hold turned into a space, spaces collapsed.
fn reduced_name(column: &str, position: usize) -> String {
    let spaced: String = column
        .chars()
        .map(|ch| if is_name_char(ch) { ch } else { ' ' })
        .collect();
    let joined = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let name: String = joined.chars().take(MAX_NAME_CHARS).collect();
    let name = name.trim().to_string();
    if name.chars().any(char::is_alphanumeric) && !name.chars().all(|ch| ch.is_ascii_digit()) {
        name
    } else {
        format!("Column {}", position + 1)
    }
}

/// The column each placeholder of `text` is filled from. The program's own table comes first (a name
/// that is a field's name, whatever the case, accents and separators); then `propose_mapping`, for a
/// name the model altered slightly: its exact match is a column, its prefix match only a proposal the
/// user confirms. A name that matches nothing is **unresolved**, kept so that it is shown.
pub fn resolve(text: &str, columns: &[String]) -> Vec<MappingEntry> {
    let fields = fields_of(columns, &[]);
    placeholder_names(text)
        .into_iter()
        .map(|name| {
            let wanted = compact(&name);
            if let Some(field) = fields
                .iter()
                .find(|each| compact(&each.placeholder) == wanted)
            {
                return MappingEntry {
                    placeholder: name,
                    column: Some(field.column.clone()),
                    status: MappingStatus::Exact,
                };
            }
            propose_mapping(std::slice::from_ref(&name), columns)
                .pop()
                .unwrap_or(MappingEntry {
                    placeholder: name,
                    column: None,
                    status: MappingStatus::Unresolved,
                })
        })
        .collect()
}

/// What the model is told about the table.
pub struct DraftSource {
    /// The workbook, as the cards name it (plain path, quoted).
    pub label: String,
    pub sheet: String,
    pub fields: Vec<Field>,
}

/// The turn sent to the model: the instruction, the fields, the request. `reminder` is the second
/// attempt's stricter ending. No cell value is part of it.
pub fn draft_turn(source: &DraftSource, request: &str, reminder: bool) -> String {
    let mut turn = String::from(TEMPLATE_DRAFT_INSTRUCTION);
    turn.push_str(&format!(
        "\n\nTable {}, sheet \"{}\". Fields:\n",
        source.label, source.sheet
    ));
    for field in &source.fields {
        turn.push_str(&format!("{{{{{}}}}}", field.placeholder));
        if let Some(kind) = &field.kind {
            turn.push_str(&format!(" ({kind})"));
        }
        turn.push('\n');
    }
    turn.push_str(&format!("\nRequest: {}", request.trim()));
    if reminder {
        turn.push_str("\n\n");
        turn.push_str(STRICTER_REMINDER);
    }
    turn
}

/// The template out of what the model wrote: reasoning tags and a code fence removed, a line of
/// commentary before the template dropped, at most [`MAX_DRAFT_BYTES`] kept.
pub fn clean_draft(raw: &str) -> String {
    let mut text = without_reasoning(raw);
    if let Some(fenced) = fenced_block(&text) {
        text = fenced;
    }
    text = text.trim().to_string();
    // "Here is the template:" before the template itself.
    if let Some((first, rest)) = text.split_once('\n') {
        let first = first.trim();
        if first.ends_with(':') && placeholder_names(first).is_empty() && !rest.trim().is_empty() {
            text = rest.trim().to_string();
        }
    }
    cut_to(&text, MAX_DRAFT_BYTES)
}

fn without_reasoning(raw: &str) -> String {
    let mut out = raw.to_string();
    while let Some(start) = out.find("<think>") {
        match out[start..].find("</think>") {
            Some(length) => out.replace_range(start..start + length + "</think>".len(), ""),
            None => {
                out.truncate(start);
                break;
            }
        }
    }
    out
}

/// The inside of the first ``` block, without its language tag.
fn fenced_block(text: &str) -> Option<String> {
    let start = text.find("```")?;
    let after = &text[start + 3..];
    let body_start = after.find('\n').map_or(0, |position| position + 1);
    let body = &after[body_start..];
    let end = body.find("```").unwrap_or(body.len());
    Some(body[..end].to_string())
}

/// `text` cut at a character boundary so that it holds at most `limit` bytes.
pub fn cut_to(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// The gateway and its settings, as [`draft`] needs them.
pub struct DraftContext<'a> {
    pub locale: &'a str,
    pub gateway: &'a GatewayClient,
    pub server_url: &'a str,
    pub model_alias: &'a str,
    pub idle_timeout: Duration,
}

/// Why no template was drafted. Machine codes: the interface writes every sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftFailure {
    /// The user's own stop.
    Cancelled,
    /// The gateway could not be reached or refused.
    Unreachable,
    /// Twice, an answer with no placeholder.
    Unusable,
}

impl DraftFailure {
    /// The code the answer carries (`TabularAnswer::FillNeedsTemplate.draft`).
    pub fn code(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::Unreachable => "unreachable",
            Self::Unusable => "unusable",
        }
    }
}

/// One gateway call that drafts the template, and one more, stricter, when the answer held no
/// placeholder. A gateway that fails is not tried again: the wait of a server that does not answer
/// is not doubled. The text is returned cleaned; its mapping is read against the real table by
/// [`resolve`].
pub async fn draft(
    context: &DraftContext<'_>,
    source: &DraftSource,
    request: &str,
) -> Result<String, DraftFailure> {
    for attempt in 0..2 {
        let turns = [ChatTurn {
            role: "user".to_string(),
            content: draft_turn(source, request, attempt == 1),
        }];
        let written = context
            .gateway
            .chat_at(
                context.server_url,
                context.model_alias,
                Some(context.locale),
                &turns,
                context.idle_timeout,
                Some(GROUNDED_TEMPERATURE),
                |_| {},
            )
            .await;
        let text = match written {
            Ok(text) => clean_draft(&text),
            Err(AppError::ChatCancelled) => return Err(DraftFailure::Cancelled),
            Err(_) => return Err(DraftFailure::Unreachable),
        };
        if !placeholder_names(&text).is_empty() {
            return Ok(text);
        }
    }
    Err(DraftFailure::Unusable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    /// The same guard every model-facing constant carries (`docs/DECISIONS.md`, "profession-neutral
    /// model-facing text").
    #[test]
    fn the_template_draft_instruction_stays_neutral_about_who_the_user_is() {
        for text in [TEMPLATE_DRAFT_INSTRUCTION, STRICTER_REMINDER] {
            let lower = text.to_lowercase();
            for word in [
                "patient",
                "doctor",
                "practitioner",
                "practice",
                "gp",
                "medical",
                "lawyer",
            ] {
                assert!(!lower.contains(word), "{word:?} found in {text}");
            }
        }
    }

    #[test]
    fn a_column_the_alphabet_cannot_hold_gets_a_name_the_alphabet_holds() {
        let fields = fields_of(
            &columns(&[
                "Montant (TTC)",
                "N\u{b0} facture",
                "Prix/unite",
                "2026",
                "",
                "Date",
            ]),
            &[],
        );
        let names: Vec<&str> = fields
            .iter()
            .map(|field| field.placeholder.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "Montant TTC",
                "N facture",
                "Prix unite",
                "Column 4",
                "Column 5",
                "Date"
            ]
        );
        for name in names {
            assert!(
                placeholder_names(&format!("{{{{{name}}}}}")) == vec![name.to_string()],
                "{name:?} is not a placeholder"
            );
        }
    }

    #[test]
    fn two_columns_that_collapse_get_distinct_names_and_a_name_is_cut_at_sixty() {
        let long = "x".repeat(80);
        let fields = fields_of(
            &columns(&[
                "Montant (TTC)",
                "Montant [TTC]",
                "Montant TTC",
                &long,
                &long,
            ]),
            &[],
        );
        assert_eq!(fields[0].placeholder, "Montant TTC");
        assert_eq!(fields[1].placeholder, "Montant TTC-2");
        assert_eq!(fields[2].placeholder, "Montant TTC-3");
        assert_eq!(fields[3].placeholder.chars().count(), 60);
        assert_eq!(fields[4].placeholder.chars().count(), 60);
        assert_ne!(fields[3].placeholder, fields[4].placeholder);
        // The program knows which column each name stands for.
        assert_eq!(fields[1].column, "Montant [TTC]");
    }

    #[test]
    fn the_turn_carries_the_field_names_the_kinds_and_the_request_only() {
        let source = DraftSource {
            label: "\"factures.xlsx\"".to_string(),
            sheet: "Factures".to_string(),
            fields: fields_of(
                &columns(&["Date", "Montant (TTC)"]),
                &[Some("date".to_string()), Some("numeric".to_string())],
            ),
        };
        let turn = draft_turn(&source, "write a reminder", false);
        assert!(turn.starts_with(TEMPLATE_DRAFT_INSTRUCTION));
        assert!(
            turn.contains("{{Date}} (date)\n{{Montant TTC}} (numeric)\n"),
            "{turn}"
        );
        assert!(turn.ends_with("Request: write a reminder"), "{turn}");
        assert!(!turn.contains(STRICTER_REMINDER));
        assert!(draft_turn(&source, "write a reminder", true).ends_with(STRICTER_REMINDER));
    }

    #[test]
    fn a_fence_commentary_and_reasoning_are_removed() {
        let raw = "<think>I should use the fields.</think>Here is the template:\n\n```text\nDear {{Name}},\nPay {{Amount}}.\n```\nHope it helps.";
        assert_eq!(clean_draft(raw), "Dear {{Name}},\nPay {{Amount}}.");
        assert_eq!(
            clean_draft("Here is the template:\n\nDear {{Name}},\nBye."),
            "Dear {{Name}},\nBye."
        );
        // A first line that holds a placeholder is the template, not commentary.
        assert_eq!(clean_draft("Dear {{Name}}:\nBye."), "Dear {{Name}}:\nBye.");
    }

    #[test]
    fn a_draft_is_cut_at_the_limit_on_a_character_boundary() {
        let long = "\u{e9}".repeat(MAX_DRAFT_BYTES);
        let cut = clean_draft(&long);
        assert!(cut.len() <= MAX_DRAFT_BYTES);
        assert!(cut.chars().all(|ch| ch == '\u{e9}'));
    }

    #[test]
    fn the_program_table_maps_a_name_before_any_guess_and_an_unknown_name_is_kept() {
        let table = columns(&["Montant (TTC)", "Libell\u{e9}", "Statut"]);
        let mapping = resolve(
            "Pay {{Montant TTC}} for {{libelle}} ({{Statu}}), ref {{Invented}}, again {{Montant TTC}}.",
            &table,
        );
        let by = |name: &str| {
            mapping
                .iter()
                .find(|entry| entry.placeholder == name)
                .unwrap()
        };
        assert_eq!(mapping.len(), 4, "a name is listed once");
        assert_eq!(by("Montant TTC").column.as_deref(), Some("Montant (TTC)"));
        assert_eq!(by("Montant TTC").status, MappingStatus::Exact);
        assert_eq!(by("libelle").column.as_deref(), Some("Libell\u{e9}"));
        assert_eq!(by("Statu").status, MappingStatus::Proposed);
        assert_eq!(by("Invented").column, None);
        assert_eq!(by("Invented").status, MappingStatus::Unresolved);
    }
}
