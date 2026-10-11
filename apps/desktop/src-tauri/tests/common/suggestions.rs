//! What the lot 9 tests (and the walk that prints the human test's figures) share: the sentences the
//! interface writes for a suggestion, read from the real catalogues, and the way a refresh is run.

#![allow(dead_code)]

use assistant_cabinet_ai_lib::analysis_scope::ScopeMode;
use assistant_cabinet_ai_lib::file_reference::FileReferenceResolver;
use assistant_cabinet_ai_lib::inventory::WorkFolderInventory;
use assistant_cabinet_ai_lib::knowledge::query::SelectedFile;
use assistant_cabinet_ai_lib::knowledge::suggest::{
    suggest, Suggestion, SuggestionInput, SuggestionSet, COLD_READ_LIMIT_BYTES, MAX_DRY_RUNS,
};
use assistant_cabinet_ai_lib::tabular_answer::{selected_workbook_choices, WorkbookChoice};
use serde_json::Value;

use super::knowledge::Lab;

pub const FRENCH: &str = include_str!("../../../src/locales/fr-FR.json");
pub const ENGLISH: &str = include_str!("../../../src/locales/en-US.json");

pub fn catalogue(locale: &str) -> Value {
    let body = match locale {
        "fr-FR" => FRENCH,
        _ => ENGLISH,
    };
    serde_json::from_str(body).expect("the catalogue is JSON")
}

pub fn month_name(locale: &str, month: u8) -> &'static str {
    const FR: [&str; 12] = [
        "janvier",
        "f\u{e9}vrier",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "ao\u{fb}t",
        "septembre",
        "octobre",
        "novembre",
        "d\u{e9}cembre",
    ];
    const EN: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let names = if locale == "fr-FR" { FR } else { EN };
    names[usize::from(month - 1)]
}

/// The sentence the interface writes for a suggestion: the catalogue entry (the `InFile` variant
/// when the suggestion carries a file), filled with the slots. The same rule as
/// `lib/suggestions.ts`.
pub fn sentence(locale: &str, suggestion: &Suggestion) -> String {
    let catalogue = catalogue(locale);
    let key = match &suggestion.file {
        Some(_) => format!("{}InFile", suggestion.template_id),
        None => suggestion.template_id.to_string(),
    };
    let template = catalogue["suggestions"][key.as_str()]
        .as_str()
        .unwrap_or_else(|| panic!("{locale} has no suggestions.{key}"))
        .to_string();
    let mut text = template;
    let replace = |text: String, slot: &str, value: Option<String>| match value {
        Some(value) => text.replace(&format!("{{{slot}}}"), &value),
        None => text,
    };
    text = replace(
        text,
        "name",
        suggestion
            .entity
            .as_ref()
            .map(|entity| entity.display.clone()),
    );
    text = replace(text, "value", suggestion.value.clone());
    text = replace(text, "column", suggestion.column.clone());
    text = replace(text, "group", suggestion.group.clone());
    text = replace(text, "file", suggestion.file.clone());
    text = replace(
        text,
        "month",
        suggestion
            .month
            .map(|month| month_name(locale, month).to_string()),
    );
    text = replace(text, "year", suggestion.year.map(|year| year.to_string()));
    assert!(
        !text.contains('{'),
        "{locale} {key}: a slot was left empty in {text:?}"
    );
    text
}

pub struct Rig {
    pub documents: Vec<SelectedFile>,
    pub choices: Vec<WorkbookChoice>,
}

pub fn rig_for(lab: &Lab, documents: Vec<SelectedFile>) -> Rig {
    let folder = lab.data_folder();
    Rig {
        documents,
        choices: selected_workbook_choices(&folder, &ScopeMode::WholeFolder),
    }
}

pub fn suggestions(lab: &Lab, rig: &Rig, locale: &str, names_allowed: bool) -> SuggestionSet {
    suggestions_with(lab, rig, locale, names_allowed, MAX_DRY_RUNS)
}

pub fn suggestions_with(
    lab: &Lab,
    rig: &Rig,
    locale: &str,
    names_allowed: bool,
    max_dry_runs: usize,
) -> SuggestionSet {
    let folder = lab.data_folder();
    let inventory = WorkFolderInventory::discover(lab.work_path(), Some(&lab.index))
        .expect("discovers the folder");
    let resolver = FileReferenceResolver::new(&inventory);
    let vocabulary = lab.vocabulary();
    suggest(&SuggestionInput {
        index: &lab.index,
        vocabulary: &vocabulary,
        data: Some(&folder),
        documents: &rig.documents,
        workbooks: &rig.choices,
        locale,
        names_allowed,
        names_a_file: &|text| resolver.resolve_in_question(text).is_some(),
        limit: 8,
        max_dry_runs,
        max_cold_read_bytes: COLD_READ_LIMIT_BYTES,
    })
    .expect("suggests")
}

pub fn templates(set: &SuggestionSet) -> Vec<&'static str> {
    set.suggestions
        .iter()
        .map(|suggestion| suggestion.template_id)
        .collect()
}
