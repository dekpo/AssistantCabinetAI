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
//! when exactly one is reachable or the question names one unambiguously. Since the tabular UI
//! follow-up it also reads two group questions, because both name only real columns and no value:
//! "which agency costs the most" (`Operation::LargestGroup`) and "total amount per agency"
//! (`Operation::GroupSum`). The column to total is the one the question names, or else the one
//! column that reads as an amount (`choose_measure`); when several do, it asks rather than picks.
//! It still does **not** parse a filter clause ("... where category is X"): a misread filter value
//! would be exactly the guess this pipeline exists to refuse, so `Operation::Filter` and a filtered
//! `Operation::Count` are reached only by constructing the `Operation` directly.

use serde::Deserialize;

use super::engine::Operation;
use super::inventory::{ColumnInventory, ColumnType, TabularInventory};
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
    groups: GroupWords,
    column_names: ColumnNameWords,
    descending: Vec<String>,
    /// Words that carry no vocabulary of their own - articles, prepositions, question words,
    /// polite forms - read only by `residual_words` (gap A, `docs/DECISIONS.md`, session 7's
    /// D1): a leftover word the classifier does not recognise may still be a filter value, and a
    /// filler word must never be mistaken for one.
    filler: Vec<String>,
    /// Session 11's filter vocabulary (`docs/DECISIONS.md`): comparison words for a numeric
    /// filter, a weekday and a month name for a date filter, and the word that introduces a
    /// date range. Read from the question's own locale pack only (`filter_words`), the same
    /// pack `residual_words` already used to find the question's leftover words.
    comparisons: ComparisonWords,
    /// Keyed `"1"` (Monday) to `"7"` (Sunday), ISO order.
    weekdays: std::collections::BTreeMap<String, Vec<String>>,
    /// Keyed `"1"` (January) to `"12"` (December).
    months: std::collections::BTreeMap<String, Vec<String>>,
    between: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ComparisonWords {
    greater_than: Vec<String>,
    less_than: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GroupWords {
    which: Vec<String>,
    most: Vec<String>,
    per: Vec<String>,
}

/// Words found inside a column's own name, not in a question. Read from every pack at once
/// (`column_words`), since a workbook's headers are in the language it was exported in.
#[derive(Debug, Clone, Deserialize)]
struct ColumnNameWords {
    measures: Vec<String>,
    not_measures: Vec<String>,
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
    list: Vec<String>,
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

/// Session 11's filter vocabulary, folded and keyed by weekday (1 Monday - 7 Sunday) or month
/// (1-12): what `tabular_answer::detect_filters` reads to turn a residual word into a weekday,
/// month, year or numeric-comparison filter. Built from the question's own locale pack, the
/// fallback chain `pack_for` already uses - not merged across every shipped pack the way
/// `column_names` are, because a filter word is asked in the question's own language, not read
/// off a workbook header.
pub(crate) struct FilterWords {
    pub greater_than: Vec<String>,
    pub less_than: Vec<String>,
    pub between: Vec<String>,
    pub weekdays: Vec<(u8, Vec<String>)>,
    pub months: Vec<(u8, Vec<String>)>,
}

pub(crate) fn filter_words(locale: &str) -> Option<FilterWords> {
    let pack = pack_for(locale)?;
    let fold_all = |words: &[String]| words.iter().map(|word| fold_text(word)).collect();
    let fold_keyed = |map: &std::collections::BTreeMap<String, Vec<String>>| -> Vec<(u8, Vec<String>)> {
        map.iter()
            .filter_map(|(key, words)| {
                key.parse::<u8>().ok().map(|n| (n, fold_all(words)))
            })
            .collect()
    };
    Some(FilterWords {
        greater_than: fold_all(&pack.comparisons.greater_than),
        less_than: fold_all(&pack.comparisons.less_than),
        between: fold_all(&pack.between),
        weekdays: fold_keyed(&pack.weekdays),
        months: fold_keyed(&pack.months),
    })
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
    /// A group question whose column to total could be any of `candidates`. Asked, never picked.
    WhichMeasure {
        group_by: String,
        candidates: Vec<String>,
    },
    /// The question could not be confidently read as either. Never a guess at which operation or
    /// which column was meant.
    NotRecognised,
}

/// Classify one question against `inventory`, restricted to `allowed_sheets` exactly as
/// `tabular::engine::execute` and `tabular::structural::answer` are - a sheet outside the scope
/// is never matched by name here either, so a question cannot even be classified against
/// evidence it could not go on to read.
///
/// The interface language's words are tried first, then every other pack's: a question typed in
/// English on a French interface - or about a workbook with English headers - is still read. Each
/// pack still only ever points at columns and sheets the workbook really has.
pub fn classify(
    question: &str,
    inventory: &TabularInventory,
    allowed_sheets: Option<&[String]>,
    locale: &str,
) -> TabularRoute {
    classify_traced(question, inventory, allowed_sheets, locale).0
}

/// `classify`, plus the question's own words left over once every recognised route's own
/// vocabulary - operation, structural and group words, the matched sheet and column names, and
/// this pack's filler words - is removed, plus the column the question itself already named
/// (`find_column_name`/`named_columns`), whether or not the recognised operation reads it. A
/// residual word is not itself proof of anything: this module never reads a cell value, so it
/// cannot tell a genuine filter ("Alpha") from ordinary prose the pack's filler list does not
/// happen to name. `tabular_answer`, which does hold the workbook's real cell values, is the one
/// place that turns a residual word into a filter or a refusal (gap A, `docs/DECISIONS.md`,
/// session 7's D1 and session 11). The residual list is empty whenever `route` is not
/// `TabularRoute::Operation`: a structural question is exempt (naming a sheet is not a filter),
/// and nothing else reaches the engine at all.
pub fn classify_traced(
    question: &str,
    inventory: &TabularInventory,
    allowed_sheets: Option<&[String]>,
    locale: &str,
) -> (TabularRoute, Vec<String>, Option<String>) {
    let Some(first) = pack_for(locale) else {
        return (TabularRoute::NotRecognised, Vec::new(), None);
    };
    let others = PACKS
        .iter()
        .filter_map(|(_, body)| serde_json::from_str::<PatternPack>(body).ok());
    for pack in std::iter::once(first).chain(others) {
        let outcome = classify_with(question, inventory, allowed_sheets, &pack);
        if outcome.0 != TabularRoute::NotRecognised {
            return outcome;
        }
    }
    (TabularRoute::NotRecognised, Vec::new(), None)
}

fn classify_with(
    question: &str,
    inventory: &TabularInventory,
    allowed_sheets: Option<&[String]>,
    pack: &PatternPack,
) -> (TabularRoute, Vec<String>, Option<String>) {
    let tokens = tokenise(question);
    if tokens.is_empty() {
        return (TabularRoute::NotRecognised, Vec::new(), None);
    }

    let folded_question = fold_text(question);
    let sheet_name = find_sheet_name(inventory, allowed_sheets, &folded_question);
    let columns = columns_for_matching(inventory, allowed_sheets, sheet_name.as_deref());
    // The folded-substring match first, as before; then whole words with a regular plural, which
    // is what reaches `agency` from "agencies" - but only when exactly one column is named.
    let column_name = find_column_name(&columns, &folded_question).or_else(|| {
        match named_columns(&columns, question).as_slice() {
            [only] => Some(only.name.clone()),
            _ => None,
        }
    });

    // Gap H: a token that is one of the matched column's or sheet's own words must never be
    // read as the question's operation or group word - "duree_min" must not let "min" outrank
    // "sort" in "Sort by duree_min descending". Narrow on purpose: only tokens that are part of a
    // name this question already resolved are excluded, and only from operation/group
    // detection - `detect_structural` still reads every token.
    let mut excluded_parts: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(name) = &sheet_name {
        excluded_parts.extend(name_words(name));
    }
    if let Some(name) = &column_name {
        excluded_parts.extend(name_words(name));
    }
    let detection_tokens: Vec<String> = tokens
        .iter()
        .filter(|token| !excluded_parts.contains(&fold_text(token)))
        .cloned()
        .collect();

    let route = if let Some(structural) =
        detect_structural(question, &tokens, pack, &sheet_name, &column_name)
    {
        TabularRoute::Structural(structural)
    } else if let Some(route) = detect_group(&detection_tokens, pack, question, &columns, &sheet_name)
    {
        route
    } else if let Some(operation) = detect_operation(&detection_tokens, pack, &column_name) {
        TabularRoute::Operation {
            sheet: sheet_name.clone(),
            operation,
        }
    } else {
        TabularRoute::NotRecognised
    };

    let residual = match &route {
        TabularRoute::Operation { operation, .. } => {
            residual_words(question, pack, &sheet_name, &column_name, operation)
        }
        _ => Vec::new(),
    };
    (route, residual, column_name)
}

/// Every column name an `Operation` actually reads, whether it was found through
/// `find_column_name`, `named_columns` or `choose_measure` - so a group question's automatically
/// chosen measure column ("amount", never named in "which agency costs the most") is excluded
/// from the residual check exactly as a directly named column is.
fn operation_column_names(operation: &Operation) -> Vec<&str> {
    match operation {
        Operation::Count { filters } => filters.iter().map(|spec| spec.column.as_str()).collect(),
        Operation::Distinct { column, filters }
        | Operation::Sum { column, filters }
        | Operation::Min { column, filters }
        | Operation::Max { column, filters }
        | Operation::Sort { column, filters, .. } => std::iter::once(column.as_str())
            .chain(filters.iter().map(|spec| spec.column.as_str()))
            .collect(),
        Operation::LargestRow { by_column, filters } => std::iter::once(by_column.as_str())
            .chain(filters.iter().map(|spec| spec.column.as_str()))
            .collect(),
        Operation::GroupSum {
            group_by,
            sum_column,
            filters,
        }
        | Operation::LargestGroup {
            group_by,
            sum_column,
            filters,
        } => [group_by.as_str(), sum_column.as_str()]
            .into_iter()
            .chain(filters.iter().map(|spec| spec.column.as_str()))
            .collect(),
        Operation::Filter { filter } => vec![filter.column.as_str()],
        Operation::RowAt { .. } => Vec::new(),
    }
}

/// The question's own words, in the order typed, with none of: this pack's structural,
/// operation, group, descending and filler vocabulary; the matched sheet's own words; and every
/// column the resolved operation actually reads. What remains may still be nothing - most
/// recognised questions leave no residue - or ordinary prose this pack's `filler` list does not
/// happen to name; neither is treated as a filter here, only by `tabular_answer`, which can read
/// the workbook's real cell values.
fn residual_words(
    question: &str,
    pack: &PatternPack,
    sheet_name: &Option<String>,
    column_name: &Option<String>,
    operation: &Operation,
) -> Vec<String> {
    let mut consumed: std::collections::HashSet<String> = std::collections::HashSet::new();
    fold_into(&mut consumed, &pack.filler);
    fold_into(&mut consumed, &pack.structural.sheets);
    fold_into(&mut consumed, &pack.structural.rows);
    fold_into(&mut consumed, &pack.structural.columns);
    fold_into(&mut consumed, &pack.structural.numeric);
    fold_into(&mut consumed, &pack.structural.formulas);
    fold_into(&mut consumed, &pack.operations.count);
    fold_into(&mut consumed, &pack.operations.distinct);
    fold_into(&mut consumed, &pack.operations.list);
    fold_into(&mut consumed, &pack.operations.sum);
    fold_into(&mut consumed, &pack.operations.min);
    fold_into(&mut consumed, &pack.operations.max);
    fold_into(&mut consumed, &pack.operations.largest_row);
    fold_into(&mut consumed, &pack.operations.sort);
    fold_into(&mut consumed, &pack.groups.which);
    fold_into(&mut consumed, &pack.groups.most);
    fold_into(&mut consumed, &pack.groups.per);
    fold_into(&mut consumed, &pack.descending);
    if let Some(name) = sheet_name {
        consumed.extend(name_words(name));
    }
    if let Some(name) = column_name {
        consumed.extend(name_words(name));
    }
    for name in operation_column_names(operation) {
        consumed.extend(name_words(name));
    }

    words_with_original(question)
        .into_iter()
        .filter(|(_, folded)| !consumed.contains(folded))
        .map(|(original, _)| original)
        .collect()
}

fn fold_into(consumed: &mut std::collections::HashSet<String>, words: &[String]) {
    consumed.extend(words.iter().map(|word| fold_text(word)));
}

/// A text's words, each paired with its folded form: like `name_words`, but keeping the original
/// spelling too, since a filter nudge should name what she typed ("Alpha"), not a folded form of
/// it.
fn words_with_original(text: &str) -> Vec<(String, String)> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| (word.to_string(), fold_text(word)))
        .collect()
}

fn detect_structural(
    question: &str,
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
        // "How many rows does Harbor have?" is a filtered count, not the sheet's row count
        // (`docs/DECISIONS.md`, session 11, gap A's row-count case): this module never reads a
        // cell value, so it cannot tell a real filter from ordinary prose, but it can tell
        // whether anything is left over once every recognised word, the sheet and the column are
        // accounted for - and defers to the ordinary `Count` operation when there is, letting
        // `tabular_answer::detect_filters`, which does hold the real data, decide what the
        // leftover word means.
        let leftover = residual_words(
            question,
            pack,
            sheet_name,
            column_name,
            &Operation::Count {
                filters: Vec::new(),
            },
        );
        if leftover.is_empty() {
            return Some(StructuralQuestion::RowCount {
                sheet: sheet_name.clone(),
            });
        }
    } else if has(&pack.structural.columns) {
        return Some(StructuralQuestion::ColumnNames {
            sheet: sheet_name.clone(),
        });
    }
    None
}

/// "Which agency costs the most" and "total amount per agency": one group column the question
/// names, and a column to total - the one it names, or the one that reads as an amount. `None`
/// when the question is not shaped like either, so the ordinary operations still get their turn.
fn detect_group(
    tokens: &[String],
    pack: &PatternPack,
    question: &str,
    columns: &[&ColumnInventory],
    sheet_name: &Option<String>,
) -> Option<TabularRoute> {
    let has = |words: &[String]| tokens.iter().any(|token| contains(words, token));
    let largest = has(&pack.groups.which) && has(&pack.groups.most);
    let per_group = has(&pack.operations.sum) && has(&pack.groups.per);
    if !largest && !per_group {
        return None;
    }

    let named = named_columns(columns, question);
    let groups: Vec<&ColumnInventory> = named
        .iter()
        .copied()
        .filter(|column| is_group_column(column))
        .collect();
    let [group] = groups.as_slice() else {
        return None;
    };
    let named_measures: Vec<&ColumnInventory> = named
        .iter()
        .copied()
        .filter(|column| {
            column.index != group.index && is_numeric_value(column) && !column.year_like
        })
        .collect();
    let sum_column = match named_measures.as_slice() {
        [one] => one.name.clone(),
        [] => {
            let others: Vec<&ColumnInventory> = columns
                .iter()
                .copied()
                .filter(|column| column.index != group.index)
                .collect();
            match choose_measure(&others) {
                MeasureChoice::One(name) => name,
                MeasureChoice::Several(candidates) => {
                    return Some(TabularRoute::WhichMeasure {
                        group_by: group.name.clone(),
                        candidates,
                    })
                }
                MeasureChoice::Nothing => return None,
            }
        }
        several => {
            return Some(TabularRoute::WhichMeasure {
                group_by: group.name.clone(),
                candidates: several.iter().map(|column| column.name.clone()).collect(),
            })
        }
    };
    let group_by = group.name.clone();
    Some(TabularRoute::Operation {
        sheet: sheet_name.clone(),
        operation: if largest {
            Operation::LargestGroup {
                group_by,
                sum_column,
                filters: Vec::new(),
            }
        } else {
            Operation::GroupSum {
                group_by,
                sum_column,
                filters: Vec::new(),
            }
        },
    })
}

/// A column rows can be grouped by: text, or a year. Never a formula column.
fn is_group_column(column: &ColumnInventory) -> bool {
    !column.has_formulas && (column.inferred_type == ColumnType::Categorical || column.year_like)
}

fn is_numeric_value(column: &ColumnInventory) -> bool {
    column.inferred_type == ColumnType::Numeric && !column.has_formulas
}

enum MeasureChoice {
    One(String),
    Several(Vec<String>),
    Nothing,
}

/// The column to total when the question names none. Only a numeric, formula-free column that is
/// not a year and whose name is not an identifier, a code or a date part can be; among those, the
/// ones whose name reads as an amount are preferred. Several equally good ones are returned, never
/// ranked.
fn choose_measure(columns: &[&ColumnInventory]) -> MeasureChoice {
    let words = column_words();
    let candidates: Vec<&ColumnInventory> = columns
        .iter()
        .copied()
        .filter(|column| {
            is_numeric_value(column)
                && !column.year_like
                && !name_words(&column.name)
                    .iter()
                    .any(|word| words.not_measures.contains(word))
        })
        .collect();
    let preferred: Vec<&ColumnInventory> = candidates
        .iter()
        .copied()
        .filter(|column| {
            name_words(&column.name)
                .iter()
                .any(|word| words.measures.contains(word))
        })
        .collect();
    let pool = if preferred.is_empty() {
        candidates
    } else {
        preferred
    };
    match pool.as_slice() {
        [] => MeasureChoice::Nothing,
        [one] => MeasureChoice::One(one.name.clone()),
        several => {
            MeasureChoice::Several(several.iter().map(|column| column.name.clone()).collect())
        }
    }
}

/// A column worth totalling and a column worth grouping by, for an example question the engine
/// will answer - the nudge's "for example". `None` for either when the sheet has none.
pub fn example_columns(columns: &[ColumnInventory]) -> (Option<String>, Option<String>) {
    let all: Vec<&ColumnInventory> = columns.iter().collect();
    let measure = match choose_measure(&all) {
        MeasureChoice::One(name) => Some(name),
        MeasureChoice::Several(names) => names.into_iter().next(),
        MeasureChoice::Nothing => None,
    };
    let words = column_words();
    let group = columns
        .iter()
        .find(|column| {
            column.inferred_type == ColumnType::Categorical
                && !column.has_formulas
                && !name_words(&column.name)
                    .iter()
                    .any(|word| words.not_measures.contains(word))
        })
        .map(|column| column.name.clone());
    (measure, group)
}

/// Every pack's column-name words, folded, in one list: a header's language is the export's, not
/// the interface's.
fn column_words() -> ColumnNameWords {
    let mut merged = ColumnNameWords {
        measures: Vec::new(),
        not_measures: Vec::new(),
    };
    for pack in PACKS
        .iter()
        .filter_map(|(_, body)| serde_json::from_str::<PatternPack>(body).ok())
    {
        merged
            .measures
            .extend(pack.column_names.measures.iter().map(|word| fold_text(word)));
        merged
            .not_measures
            .extend(pack.column_names.not_measures.iter().map(|word| fold_text(word)));
    }
    merged
}

/// A name's words, folded: `calendar_year` is `calendar` and `year`. `pub(crate)` so
/// `tabular_answer`'s gap-A filter check can fold a workbook's own cell text by the same rule a
/// residual question word was folded by.
pub(crate) fn name_words(text: &str) -> Vec<String> {
    fold_text(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

/// Every column the question names, as whole words: `sub_agency` is named by "sub agency" or
/// "sub_agency", never by "agency" alone, which names `agency`. A plural on the last word still
/// counts. The longest names are matched first and each question word serves one column only. A
/// group column may also be named by its last word alone ("year" for `calendar_year`) when that
/// word is long enough and no other group column ends with it.
fn named_columns<'a>(columns: &[&'a ColumnInventory], question: &str) -> Vec<&'a ColumnInventory> {
    let words = name_words(question);
    let mut used = vec![false; words.len()];
    let mut found: Vec<&ColumnInventory> = Vec::new();

    let mut by_length: Vec<(&ColumnInventory, Vec<String>)> = columns
        .iter()
        .map(|column| (*column, name_words(&column.name)))
        .filter(|(_, parts)| !parts.is_empty())
        .collect();
    by_length.sort_by_key(|(_, parts)| std::cmp::Reverse(parts.len()));

    let same = |question_word: &str, name_word: &str, last: bool| {
        question_word == name_word || (last && plural_of(question_word, name_word))
    };
    for (column, parts) in &by_length {
        let span = parts.len();
        if span > words.len() {
            continue;
        }
        for start in 0..=words.len() - span {
            let matches = (0..span).all(|offset| {
                !used[start + offset]
                    && same(&words[start + offset], &parts[offset], offset + 1 == span)
            });
            if matches {
                (start..start + span).for_each(|index| used[index] = true);
                found.push(column);
                break;
            }
        }
    }

    for (index, word) in words.iter().enumerate() {
        if used[index] || word.chars().count() < 4 {
            continue;
        }
        let tails: Vec<&ColumnInventory> = by_length
            .iter()
            .filter(|(column, parts)| {
                parts.len() > 1
                    && is_group_column(column)
                    && parts.last().is_some_and(|last| same(word, last, true))
                    && !found.iter().any(|seen| seen.index == column.index)
            })
            .map(|(column, _)| *column)
            .collect();
        if let [only] = tails.as_slice() {
            used[index] = true;
            found.push(only);
        }
    }
    found
}

/// Whether `word` is a plural of `name`, by the regular endings of the two shipped languages:
/// `montants`, `prix`/`travaux` (`-x`, `-aux` for `-al`), and `agencies` for `agency`. Spelling
/// only, never meaning: a plural that is not regular is simply not matched.
fn plural_of(word: &str, name: &str) -> bool {
    word.strip_suffix('s') == Some(name)
        || word.strip_suffix('x') == Some(name)
        || word
            .strip_suffix("ies")
            .is_some_and(|stem| name.strip_suffix('y') == Some(stem))
        || word
            .strip_suffix("aux")
            .is_some_and(|stem| name.strip_suffix("al") == Some(stem))
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
        return column_name.clone().map(|column| Operation::Sum {
            column,
            filters: Vec::new(),
        });
    }
    if has(&pack.operations.min) {
        return column_name.clone().map(|column| Operation::Min {
            column,
            filters: Vec::new(),
        });
    }
    if has(&pack.operations.max) {
        return column_name.clone().map(|column| Operation::Max {
            column,
            filters: Vec::new(),
        });
    }
    if has(&pack.operations.largest_row) {
        return column_name.clone().map(|column| Operation::LargestRow {
            by_column: column,
            filters: Vec::new(),
        });
    }
    // "List the agencies" is the column's different values, the same operation "which distinct
    // agencies" already reaches.
    if has(&pack.operations.distinct) || has(&pack.operations.list) {
        return column_name.clone().map(|column| Operation::Distinct {
            column,
            filters: Vec::new(),
        });
    }
    if has(&pack.operations.sort) {
        return column_name.clone().map(|column| Operation::Sort {
            column,
            descending,
            filters: Vec::new(),
        });
    }
    if has(&pack.operations.count) {
        return Some(Operation::Count { filters: Vec::new() });
    }
    // No operation word at all, but a comparison word ("over", "plus de") beside an actual
    // number is still a real question - "count", the same default a plain "how many" reaches,
    // with the comparison itself resolved afterward as a filter against real data
    // (`tabular_answer::detect_filters`, `docs/DECISIONS.md`, session 11). The number check
    // matters: "moins"/"plus" alone, with no number in sight, is a superlative group phrase
    // (which room has the least duree_min) this classifier still does not read (gap E,
    // session 12), not a numeric filter to guess at.
    if (has(&pack.comparisons.greater_than) || has(&pack.comparisons.less_than))
        && tokens.iter().any(|token| token.parse::<f64>().is_ok())
    {
        return Some(Operation::Count { filters: Vec::new() });
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

    /// Test questions in every shipped language, read as data: a French sentence may not sit in a
    /// `.rs` file (`src/guards/sources.test.ts`), so both languages live in the fixtures.
    const QUESTIONS: &[(&str, &str)] = &[
        (
            "en-US",
            include_str!("../../tests/fixtures/tabular-questions/en-US.json"),
        ),
        (
            "fr-FR",
            include_str!("../../tests/fixtures/tabular-questions/fr-FR.json"),
        ),
    ];

    /// The question stored under `key` for `locale`. Panics on a missing file or key, so a
    /// renamed fixture fails loudly rather than testing an empty string.
    fn question(locale: &str, key: &str) -> String {
        let (_, body) = QUESTIONS
            .iter()
            .find(|(tag, _)| *tag == locale)
            .unwrap_or_else(|| panic!("no question fixture for {locale}"));
        let fixture: serde_json::Value = serde_json::from_str(body).expect("fixture parses");
        fixture["questions"][key]
            .as_str()
            .unwrap_or_else(|| panic!("no question {key} for {locale}"))
            .to_string()
    }

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
        TabularInventory::build("factures.csv", "hash-1", TabularFormat::Csv, &workbook, "fr-FR")
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
        TabularInventory::build("classeur.xlsx", "hash-2", TabularFormat::Xlsx, &workbook, "fr-FR")
    }

    #[test]
    fn every_question_fixture_asks_the_same_questions() {
        let keys = |body: &str| -> Vec<String> {
            let fixture: serde_json::Value = serde_json::from_str(body).expect("fixture parses");
            let mut keys: Vec<String> = fixture["questions"]
                .as_object()
                .expect("a questions object")
                .keys()
                .cloned()
                .collect();
            keys.sort();
            keys
        };
        let (_, first) = QUESTIONS[0];
        for (tag, body) in QUESTIONS {
            assert_eq!(keys(body), keys(first), "{tag} must ask the same questions");
        }
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
        for locale in ["en-US", "fr-FR"] {
            let question = question(locale, "sheet_names");
            assert_eq!(
                classify(&question, &inventory, None, locale),
                TabularRoute::Structural(StructuralQuestion::SheetNames),
                "{question}"
            );
        }
    }

    #[test]
    fn row_count_is_structural_even_when_a_sheet_is_named() {
        let inventory = two_sheets();
        for locale in ["en-US", "fr-FR"] {
            let question = question(locale, "row_count_in_named_sheet");
            assert_eq!(
                classify(&question, &inventory, None, locale),
                TabularRoute::Structural(StructuralQuestion::RowCount {
                    sheet: Some("Facturation".to_string())
                }),
                "{question}"
            );
        }
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
        for locale in ["en-US", "fr-FR"] {
            let question = question(locale, "sum_of_named_column");
            assert_eq!(
                classify(&question, &inventory, None, locale),
                TabularRoute::Operation {
                    sheet: None,
                    operation: Operation::Sum {
                        column: "montant".to_string(),
                        filters: Vec::new(),
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
                    column: "fournisseur".to_string(),
                    filters: Vec::new(),
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
                    filters: Vec::new(),
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
                    filters: Vec::new(),
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
                operation: Operation::Count { filters: Vec::new() }
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
    fn a_filter_clause_is_still_not_parsed_but_its_words_are_left_residual() {
        let inventory = invoices();
        // A filter clause is not parsed from free text - only structural facts and single-column
        // operations are. This is a documented scope limit, not a bug: guessing a filter value
        // wrongly would be exactly the failure this pipeline exists to refuse. What changed
        // (session 9, superseding this test's old name and the unconditional Count it used to
        // assert): the classifier now also reports "Alpha" as residual, so `tabular_answer` can
        // recognise it names real data and refuse rather than silently return the unfiltered
        // count (`docs/DECISIONS.md`, session 7's D1).
        let (route, residual, _column_name) = classify_traced(
            "How many invoices where fournisseur is Alpha?",
            &inventory,
            None,
            "en-US",
        );
        assert_eq!(
            route,
            TabularRoute::Operation {
                sheet: None,
                operation: Operation::Count { filters: Vec::new() }
            },
            "the filter clause is still ignored rather than mis-parsed; the plain count still resolves"
        );
        // "invoices" is left over too - this pack's filler list is grammar, not a domain
        // dictionary - but only "Alpha" happens to name real data, which is for
        // `tabular_answer` to decide.
        assert_eq!(
            residual,
            vec!["invoices".to_string(), "Alpha".to_string()]
        );
    }

    #[test]
    fn a_column_names_own_word_does_not_outrank_the_real_operation() {
        // Gap H: "duree_min" splits into "duree" and "min" on the underscore, and "min" is also
        // this pack's minimum operation word. Once the column is matched, "min" must not be read
        // as the question's operation - "sort" must win, not the minimum.
        let workbook = Workbook {
            sheets: vec![SheetData {
                name: "Planning".to_string(),
                rows: vec![
                    text_row(&["salle", "duree_min"]),
                    text_row(&["Azur", "20"]),
                    text_row(&["Lotus", "60"]),
                ],
            }],
        };
        let inventory =
            TabularInventory::build("rendez-vous.xlsx", "hash-3", TabularFormat::Xlsx, &workbook, "fr-FR");

        assert_eq!(
            classify("Sort by duree_min descending", &inventory, None, "en-US"),
            TabularRoute::Operation {
                sheet: None,
                operation: Operation::Sort {
                    column: "duree_min".to_string(),
                    descending: true,
                    filters: Vec::new(),
                }
            }
        );
        // The fix excludes a matched column's own words from operation detection - it does not
        // disable "min" as an operation word everywhere: naming the minimum still reaches it.
        assert_eq!(
            classify("What is the minimum duree_min?", &inventory, None, "en-US"),
            TabularRoute::Operation {
                sheet: None,
                operation: Operation::Min {
                    column: "duree_min".to_string(),
                    filters: Vec::new(),
                }
            }
        );
    }
}
