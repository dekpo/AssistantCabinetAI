//! The hidden interpreter: gap G (`docs/SESSION-DATA-REFERENCE-report.md` section 4), decision D6
//! (`docs/DECISIONS.md`, "Settled for the tabular continuation, session 7"). When a question does
//! not classify deterministically, a model reads the workbook's **schema alone** - sheet names,
//! row counts, each column's name, type, unit and whether it holds formulas; never a cell value,
//! never a distinct value, never a row, never a file path - and writes back one JSON query plan in
//! a closed vocabulary. Rust parses it strictly (`deny_unknown_fields`), resolves every filter
//! value against the real workbook (the same folded-equality rule session 11 already uses), maps
//! the plan to an existing `tabular::engine::Operation`, and only then runs it through
//! `engine::execute` - the unchanged engine, no new arithmetic. The number in the answer is always
//! the engine's; the model only ever chose which question to ask it.
//!
//! This module does no network I/O itself: `tabular_answer` owns the one gateway call (building
//! the message from `build_schema_message`, parsing the reply with `parse_response`) so that every
//! place this product speaks to the gateway stays in a module whose name says so.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::engine::{self, Comparison, FilterSpec, Operation};
use super::inventory::{data_rows, text_value, ColumnInventory, TabularInventory};
use super::Workbook;
use crate::file_reference::fold_text;

/// English, profession-neutral (`docs/DECISIONS.md`, "profession-neutral model-facing text"),
/// short for the reason every model-facing instruction is short - a small local model echoes a
/// long rule list back into its answer. Spells out the wire shape precisely: a small model cannot
/// be expected to infer a JSON schema from a description alone, the way a careful human reader
/// could from the Rust vocabulary comment below.
///
/// The closing sentence on `filters` was added after a live model (`gemma2:2b`, session 14's
/// manual validation pass, `docs/DECISIONS.md`) repeatedly echoed `aggregate`'s own `op` back as
/// a spurious `filters` entry with no real value - as if the aggregate also had to be restated
/// there. `tabular::query_plan`'s own parsing already drops an entry that shapeless on its own
/// (`lenient_filters`, a `null` value), so this sentence is a second line of defence, not the only
/// one: it exists to make the confusion rarer, not because the code depends on it.
pub(crate) const QUERY_PLAN_INSTRUCTION: &str =
    "You are given the structure of a data table - its sheets, and for each column its name, \
     type and whether it holds formulas - never a value, a row or a file path. Read the question \
     and reply with exactly one JSON object describing how to answer it, matching this shape and \
     nothing else: {\"sheet\": string or null, \"filters\": [{\"column\": string, \"op\": one of \
     \"eq\", \"in\", \"gt\", \"lt\", \"between\", \"contains\", \"weekday\", \"month\", \"year\", \
     \"date_range\", \"value\": the comparison value}], \"group_by\": string or null, \
     \"aggregate\": {\"op\": one of \"count\", \"sum\", \"mean\", \"median\", \"min\", \"max\", \
     \"distinct\", \"column\": string or null} or null, \"sort\": {\"column\": string, \
     \"descending\": boolean} or null, \"limit\": number or null, \"unsupported\": boolean}. \
     Every sheet and column name must be copied exactly from what you were given. A filter's \
     value must be a value you believe the data genuinely holds - never a guess at one. Set \
     \"unsupported\" to true and leave every other field at its default when the question cannot \
     be expressed this way. Reply with the JSON object alone: no explanation, no code fence. \
     Leave \"filters\" empty unless the question names a specific condition to filter rows by - \
     never repeat \"aggregate\"'s own op there.";

// --- The schema sent to the model: structure only, never a value ---------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SchemaSheet {
    name: String,
    row_count: usize,
    columns: Vec<SchemaColumn>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SchemaColumn {
    name: String,
    #[serde(rename = "type")]
    kind: &'static str,
    unit: Option<String>,
    has_formulas: bool,
}

fn build_schema(inventory: &TabularInventory, allowed: Option<&[String]>) -> Vec<SchemaSheet> {
    engine::reachable_sheets(inventory, allowed)
        .into_iter()
        .map(|sheet| SchemaSheet {
            name: sheet.name.clone(),
            row_count: sheet.row_count,
            columns: sheet
                .columns
                .iter()
                .map(|column| SchemaColumn {
                    name: column.name.clone(),
                    kind: column.inferred_type.as_code(),
                    unit: column.unit.clone(),
                    has_formulas: column.has_formulas,
                })
                .collect(),
        })
        .collect()
}

/// The one message sent to the model: the instruction, the schema as JSON, then the question.
/// A test (`a_built_message_never_carries_a_fixture_cell_value`) asserts no fixture cell value
/// ever appears in what this returns.
pub(crate) fn build_schema_message(
    question: &str,
    inventory: &TabularInventory,
    allowed: Option<&[String]>,
) -> String {
    let schema = build_schema(inventory, allowed);
    let schema_json = serde_json::to_string(&schema).unwrap_or_default();
    format!("{QUERY_PLAN_INSTRUCTION}\n\nSchema:\n{schema_json}\n\nQuestion: {question}")
}

// --- The plan the model writes back ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PlanFilterOp {
    Eq,
    In,
    Gt,
    Lt,
    Between,
    Contains,
    Weekday,
    Month,
    Year,
    DateRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PlanAggregateOp {
    Count,
    Sum,
    Mean,
    Median,
    Min,
    Max,
    Distinct,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanFilter {
    pub column: String,
    pub op: PlanFilterOp,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanAggregate {
    pub op: PlanAggregateOp,
    #[serde(default)]
    pub column: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanSort {
    /// `None` when the model fills `sort` in (often with just `descending`) for a question that
    /// never asked for a column-level sort at all - observed live from gemma2:2b during session
    /// 14's manual validation, which wrote `"sort": {"column": null, "descending": null}` beside
    /// a `group_by` + `aggregate` plan where `sort.descending` alone is the only field
    /// `build_operation` ever reads (see its own comment). A plain column sort with no column
    /// named is meaningless and maps to `Unsupported` there, same as before.
    #[serde(default)]
    pub column: Option<String>,
    /// Same `null`-tolerance as `QueryPlan.filters`, for the same observed reason: gemma2:2b sent
    /// `"descending": null` in the capture `column`'s own doc comment quotes in full.
    #[serde(default, deserialize_with = "null_as_default")]
    pub descending: bool,
}

/// The model's own JSON, after `deny_unknown_fields` - anything the vocabulary does not name is
/// rejected rather than ignored, so a model that invents a field fails loudly instead of having
/// that field silently dropped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QueryPlan {
    #[serde(default)]
    pub sheet: Option<String>,
    /// Two kinds of noise tolerated, both observed live (session 14's manual validation pass,
    /// `docs/DECISIONS.md`): `"filters": null` for "no filters" (`ministral-3:3b`), handled the
    /// same way every other `null`-tolerant field here is; and one array entry that is not a
    /// real filter at all - `gemma2:2b` repeatedly wrote `{"column": "montant", "op": "sum",
    /// "value": null}`, restating the aggregate's own `op` (not a valid filter `op` at all,
    /// `"sum"`) as if it also belonged in `filters`. `deserialize_with = "lenient_filters"`
    /// parses each array entry on its own and keeps only the ones that are a real `PlanFilter` -
    /// an entry this loose is not a wrong guess to refuse, it is noise with nothing to guess at.
    #[serde(default, deserialize_with = "lenient_filters")]
    pub filters: Vec<PlanFilter>,
    #[serde(default)]
    pub group_by: Option<String>,
    #[serde(default)]
    pub aggregate: Option<PlanAggregate>,
    #[serde(default)]
    pub sort: Option<PlanSort>,
    #[serde(default)]
    pub limit: Option<usize>,
    /// Same `null`-tolerance as `filters` above, defensively: not yet observed for this
    /// particular field, but every other plain (non-`Option`) field with a default has been
    /// caught sending `null` by at least one of the three models probed live, so there is no
    /// reason to expect this one is exempt.
    #[serde(default, deserialize_with = "null_as_default")]
    pub unsupported: bool,
}

/// `#[serde(default)]` alone only covers a key that is *absent*; a real small model fills in a
/// field it has nothing to say with an explicit `null` at least as often as it omits the key, for
/// every plain (non-`Option`) default field this module has - observed live against the gateway
/// during session 14's manual validation (`docs/DECISIONS.md`, "the hidden interpreter session").
/// Shared by every field that needs both spellings to mean the same thing.
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::deserialize(deserializer)?.unwrap_or_default())
}

/// `filters: null` reads as empty, the same as every other `null`-tolerant field
/// (`null_as_default`); each remaining array entry is then parsed **on its own**, as a bare
/// `serde_json::Value` first, and kept only if it then parses as a real `PlanFilter` -
/// `deny_unknown_fields` and the closed `op` vocabulary still apply to each entry individually,
/// only the *whole array* no longer fails because of one bad one. A plan that is still full of
/// real filters loses nothing; the one entry that was never a filter at all just is not kept as
/// one, exactly as if the model had not written it.
fn lenient_filters<'de, D>(deserializer: D) -> Result<Vec<PlanFilter>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: Option<Vec<serde_json::Value>> = Option::deserialize(deserializer)?;
    Ok(raw
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| serde_json::from_value::<PlanFilter>(entry).ok())
        .collect())
}

/// `raw` is whatever the model streamed back - real small local models routinely wrap JSON in a
/// code fence or a sentence of prose, so this takes the substring between the first `{` and the
/// last `}` before parsing. It is **not** lenient about the JSON itself once found:
/// `deny_unknown_fields` and the closed `op`/aggregate-`op` vocabularies still reject anything
/// outside the schema this module owns.
pub(crate) fn parse_response(raw: &str) -> Option<QueryPlan> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end < start {
        return None;
    }
    serde_json::from_str(&raw[start..=end]).ok()
}

// --- Resolving a plan against the real workbook ----------------------------------------------

/// What a plan, once read, came to.
#[derive(Debug)]
pub(crate) enum PlanResolution {
    /// A plan Rust could map to an existing operation - still subject to `engine::execute`'s own
    /// refusals (a column that does not exist, a formula column, the wrong type), which the
    /// caller reads from the `TabularOutcome` it gets back.
    Operation(Operation),
    /// An `eq`/`in` filter named a value this column's real data does not hold - never run as a
    /// silent zero (`docs/DECISIONS.md`, session 11's rule, applied here to a model-written
    /// filter exactly as it already applies to a residual question word).
    ValueNotFound { value: String, close: Vec<String> },
    /// A value whose JSON shape does not match its `op`, or a group/aggregate/sort combination no
    /// existing operation expresses - including a bare `unsupported: true` with nothing else
    /// filled in. Degrades to the ordinary nudge, exactly as an unrecognised question already
    /// does.
    Unsupported,
}

/// Validate and map one plan. `inventory`/`workbook` are the same pair `engine::execute` is about
/// to be called with, so a filter value is checked against the exact rows execution will read.
///
/// `plan.unsupported` is read as **advisory, not a gate**: probed live against the gateway during
/// session 14's manual validation, every shipped model (gemma2:2b, llama3.2:3b, ministral-3:3b)
/// sets it `true` at least sometimes *alongside* an otherwise complete, correct plan - the
/// instruction's own wording ("set unsupported to true... when the question cannot be expressed")
/// is evidently read by a small model as a confidence hedge to fill in regardless, not a strict
/// either/or choice. The concrete fields are strictly more specific evidence of what the model
/// actually meant than one ambiguous boolean, and trusting them costs nothing in safety: a plan
/// that is genuinely empty (no aggregate, no group_by, no sort) still resolves to `Unsupported`
/// below on its own, through `build_operation` finding nothing to map, exactly as a bare
/// `{"unsupported": true}` always has.
pub(crate) fn resolve(
    plan: &QueryPlan,
    inventory: &TabularInventory,
    workbook: &Workbook,
    allowed: Option<&[String]>,
) -> PlanResolution {
    let filters = match resolve_filters(plan, inventory, workbook, allowed) {
        Ok(filters) => filters,
        Err(outcome) => return outcome,
    };
    match build_operation(plan, filters) {
        Some(operation) => PlanResolution::Operation(operation),
        None => PlanResolution::Unsupported,
    }
}

/// Every filter's `FilterSpec`, or the first `eq`/`in` value that the named column's real data
/// does not hold. Column existence and type are deliberately **not** checked here: that is
/// `engine::execute`'s own job (`resolve_value_column`/`resolve_filter_column`), so this never
/// duplicates a refusal reason the engine already owns. When the sheet itself cannot be resolved
/// (not named unambiguously, or does not exist), every filter is passed through unexamined and
/// `engine::execute` refuses the whole operation with the correct, more specific reason.
fn resolve_filters(
    plan: &QueryPlan,
    inventory: &TabularInventory,
    workbook: &Workbook,
    allowed: Option<&[String]>,
) -> Result<Vec<FilterSpec>, PlanResolution> {
    let sheet_data = engine::resolve_sheet_inventory(inventory, plan.sheet.as_deref(), allowed)
        .ok()
        .and_then(|sheet_inventory| {
            workbook
                .sheets
                .iter()
                .find(|sheet| sheet.name == sheet_inventory.name)
                .map(|sheet_data| (sheet_inventory, sheet_data))
        });

    let mut resolved = Vec::with_capacity(plan.filters.len());
    for filter in &plan.filters {
        if let Some(spec) = resolve_one_filter(filter, sheet_data)? {
            resolved.push(spec);
        }
    }
    Ok(resolved)
}

/// `Ok(None)` - dropped, not refused - for a filter whose `value` is a literal JSON `null`: a
/// well-formed `op` and column with nothing after it is not a wrong guess to catch, it is the
/// model leaving a slot empty the way `gemma2:2b` repeatedly did live (session 14's manual
/// validation pass, `docs/DECISIONS.md`) rather than omitting the entry outright. Any other
/// unresolvable shape - a real but absent value, a type mismatch - still refuses as `Unsupported`
/// or `ValueNotFound`, because *those* do carry a guess worth catching.
fn resolve_one_filter(
    filter: &PlanFilter,
    sheet: Option<(&super::inventory::SheetInventory, &super::SheetData)>,
) -> Result<Option<FilterSpec>, PlanResolution> {
    if filter.value.is_null() {
        return Ok(None);
    }
    let needs_real_data = matches!(filter.op, PlanFilterOp::Eq | PlanFilterOp::In);
    if needs_real_data {
        if let Some((sheet_inventory, sheet_data)) = sheet {
            if let Some(column) = find_column(&sheet_inventory.columns, &filter.column) {
                return resolve_equality(filter, column, sheet_inventory, sheet_data).map(Some);
            }
        }
        // No sheet, or no such column: pass the raw value through so `engine::execute` can
        // refuse with `SheetNotFound`/`ColumnNotFound`, the more specific reason.
    }
    comparison_from(filter.op, &filter.value)
        .map(|comparison| {
            Some(FilterSpec {
                column: filter.column.clone(),
                comparison,
            })
        })
        .ok_or(PlanResolution::Unsupported)
}

fn find_column<'a>(columns: &'a [ColumnInventory], name: &str) -> Option<&'a ColumnInventory> {
    columns
        .iter()
        .find(|column| column.name == name)
        .or_else(|| columns.iter().find(|column| fold_text(&column.name) == fold_text(name)))
}

/// `eq`/`in`, resolved against the named column's own real values (session 11's rule: folded
/// equality, close values on a miss). The requested value is canonicalised to the real text found
/// - so "Understood as" shows the value as the workbook actually spells it, not as the model typed
/// it - while still matching case- and accent-insensitively.
fn resolve_equality(
    filter: &PlanFilter,
    column: &ColumnInventory,
    sheet_inventory: &super::inventory::SheetInventory,
    sheet_data: &super::SheetData,
) -> Result<FilterSpec, PlanResolution> {
    let rows = data_rows(sheet_inventory, sheet_data);
    let pool: BTreeSet<String> = rows
        .iter()
        .filter_map(|row| row.get(column.index))
        .map(text_value)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();

    let requested: Vec<String> = match filter.op {
        PlanFilterOp::Eq => match filter.value.as_str() {
            Some(value) => vec![value.to_string()],
            None => return Err(PlanResolution::Unsupported),
        },
        // A bare string is read the same as a one-element array: a live capture (`gemma2:2b`,
        // session 14's manual validation pass) wrote `"op": "in", "value": "Beta"` for a plain
        // single-value filter - `in` and `eq` meaning the same thing for one value is exactly the
        // confusion this tolerates, without changing what either op means for a real list.
        PlanFilterOp::In => match filter.value.as_array().cloned().or_else(|| {
            filter.value.as_str().map(|value| vec![serde_json::Value::String(value.to_string())])
        }) {
            Some(items) => {
                let values: Option<Vec<String>> =
                    items.iter().map(|item| item.as_str().map(str::to_string)).collect();
                match values {
                    Some(values) if !values.is_empty() => values,
                    _ => return Err(PlanResolution::Unsupported),
                }
            }
            None => return Err(PlanResolution::Unsupported),
        },
        _ => unreachable!("resolve_equality is only called for eq/in"),
    };

    let mut canonical = Vec::with_capacity(requested.len());
    for value in &requested {
        let folded = fold_text(value);
        match pool.iter().find(|real| fold_text(real) == folded) {
            Some(real) => canonical.push(real.clone()),
            None => {
                return Err(PlanResolution::ValueNotFound {
                    close: close_values(value, &pool),
                    value: value.clone(),
                })
            }
        }
    }

    let comparison = match filter.op {
        PlanFilterOp::Eq => Comparison::Equals(canonical.into_iter().next().unwrap()),
        PlanFilterOp::In => Comparison::In(canonical),
        _ => unreachable!("resolve_equality is only called for eq/in"),
    };
    Ok(FilterSpec {
        column: column.name.clone(),
        comparison,
    })
}

/// Up to five of `pool`'s values close to `word` - folded prefix match first, then edit distance
/// at most two - mirroring `tabular_answer::close_values` for a model-written filter value. Kept
/// as its own small copy rather than shared: the two pools differ (every reachable column's
/// values there, one named column's values here), and the whole routine is a dozen lines.
fn close_values(word: &str, pool: &BTreeSet<String>) -> Vec<String> {
    let folded_word = fold_text(word);
    let mut scored: Vec<(usize, &String)> = pool
        .iter()
        .filter_map(|value| {
            let folded_value = fold_text(value);
            if folded_value.starts_with(&folded_word) || folded_word.starts_with(&folded_value) {
                Some((0, value))
            } else {
                let distance = levenshtein(&folded_word, &folded_value);
                (distance <= 2).then_some((distance + 1, value))
            }
        })
        .collect();
    scored.sort_by_key(|(distance, _)| *distance);
    scored.into_iter().take(5).map(|(_, value)| value.clone()).collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, &cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            current.push((previous[j + 1] + 1).min(current[j] + 1).min(previous[j] + cost));
        }
        previous = current;
    }
    previous[b.len()]
}

/// A comparison that needs no real-data resolution: `gt`/`lt`/`between` (numbers),
/// `contains` (any text - substring semantics are fuzzy by nature, so a miss is an honest empty
/// result rather than evidence of a wrong value), `weekday`/`month`/`year`/`date_range`. Type
/// checking (numeric vs. date column) is `engine::execute`'s own job, exactly as it is for a
/// deterministically classified filter. `None` when the JSON shape does not match what the op
/// needs - a plan Rust will not guess at, degrading to the ordinary nudge.
fn comparison_from(op: PlanFilterOp, value: &serde_json::Value) -> Option<Comparison> {
    let as_number_pair = |value: &serde_json::Value| -> Option<(f64, f64)> {
        let items = value.as_array()?;
        let [a, b] = items.as_slice() else { return None };
        Some((a.as_f64()?, b.as_f64()?))
    };
    match op {
        PlanFilterOp::Eq | PlanFilterOp::In => {
            unreachable!("eq/in are resolved against real data, not reached here")
        }
        PlanFilterOp::Contains => value.as_str().map(|text| Comparison::Contains(text.to_string())),
        PlanFilterOp::Gt => value.as_f64().map(Comparison::GreaterThan),
        PlanFilterOp::Lt => value.as_f64().map(Comparison::LessThan),
        PlanFilterOp::Between => {
            let (a, b) = as_number_pair(value)?;
            Some(Comparison::Between(a.min(b), a.max(b)))
        }
        PlanFilterOp::Weekday => value
            .as_u64()
            .filter(|n| (1..=7).contains(n))
            .map(|n| Comparison::Weekday(n as u8)),
        PlanFilterOp::Month => value
            .as_u64()
            .filter(|n| (1..=12).contains(n))
            .map(|n| Comparison::Month(n as u8)),
        PlanFilterOp::Year => value.as_i64().map(|n| Comparison::Year(n as i32)),
        PlanFilterOp::DateRange => {
            let items = value.as_array()?;
            let [a, b] = items.as_slice() else { return None };
            let start = super::inventory::parse_date_value(a.as_str()?)?;
            let end = super::inventory::parse_date_value(b.as_str()?)?;
            Some(Comparison::DateRange(start.min(end), start.max(end)))
        }
    }
}

/// `group_by` and `aggregate` together pick which existing `Operation` a plan maps to; `sort` and
/// `limit` only ever refine a `group_by` + `sum` plan into the right ranking shape - every other
/// combination reads `sort`/`limit` as meaningless and ignores them, rather than inventing a new
/// operation to honour a field the engine has no way to compute. `None` for a combination no
/// existing operation expresses (a per-group mean, say): degrades to the ordinary nudge.
fn build_operation(plan: &QueryPlan, filters: Vec<FilterSpec>) -> Option<Operation> {
    match (&plan.group_by, &plan.aggregate) {
        (None, Some(aggregate)) => match aggregate.op {
            PlanAggregateOp::Count => Some(Operation::Count { filters }),
            PlanAggregateOp::Sum => Some(Operation::Sum { column: aggregate.column.clone()?, filters }),
            PlanAggregateOp::Mean => Some(Operation::Mean { column: aggregate.column.clone()?, filters }),
            PlanAggregateOp::Median => {
                Some(Operation::Median { column: aggregate.column.clone()?, filters })
            }
            PlanAggregateOp::Min => Some(Operation::Min { column: aggregate.column.clone()?, filters }),
            PlanAggregateOp::Max => Some(Operation::Max { column: aggregate.column.clone()?, filters }),
            PlanAggregateOp::Distinct => {
                Some(Operation::Distinct { column: aggregate.column.clone()?, filters })
            }
        },
        // `sort.column` is `None` whenever the model filled `sort` in without one (common even
        // for a group ranking, where this branch is never reached at all) - meaningless on its
        // own here, since a plain column sort has nothing to sort by without it.
        (None, None) => plan
            .sort
            .as_ref()
            .and_then(|sort| sort.column.clone())
            .map(|column| Operation::Sort {
                column,
                descending: plan.sort.as_ref().is_some_and(|sort| sort.descending),
                filters,
            }),
        (Some(group_by), Some(aggregate)) => match aggregate.op {
            PlanAggregateOp::Count => {
                Some(Operation::CountPerGroup { group_by: group_by.clone(), filters })
            }
            PlanAggregateOp::Sum => {
                let sum_column = aggregate.column.clone()?;
                match (&plan.sort, plan.limit) {
                    (None, _) => Some(Operation::GroupSum {
                        group_by: group_by.clone(),
                        sum_column,
                        filters,
                    }),
                    (Some(sort), None) if sort.descending => Some(Operation::LargestGroup {
                        group_by: group_by.clone(),
                        sum_column,
                        filters,
                    }),
                    (Some(sort), Some(n)) if sort.descending => {
                        if n == 0 {
                            None
                        } else {
                            // `n` is passed through uncapped: `engine::execute` caps it at
                            // `MAX_TOP_GROUPS` and reports both the request and the cap itself
                            // (`TopGroups::requested`/`capped`), so capping it again here would
                            // only lose the honest "you asked for 80" the engine already keeps.
                            Some(Operation::TopGroups {
                                group_by: group_by.clone(),
                                sum_column,
                                n,
                                filters,
                            })
                        }
                    }
                    (Some(_ascending), _) => Some(Operation::LeastGroup {
                        group_by: group_by.clone(),
                        sum_column,
                        filters,
                    }),
                }
            }
            // The engine has no per-group mean, median, min, max or distinct - a plan asking for
            // one is honestly unsupported rather than approximated.
            PlanAggregateOp::Mean
            | PlanAggregateOp::Median
            | PlanAggregateOp::Min
            | PlanAggregateOp::Max
            | PlanAggregateOp::Distinct => None,
        },
        // `group_by` with no `aggregate`: ambiguous (group by what measure?) - never guessed.
        (Some(_), None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabular::engine::TabularValue;
    use crate::tabular::inventory::{TabularFormat, TabularInventory};
    use crate::tabular::{CellValue, SheetData, Workbook};

    #[test]
    fn the_query_plan_instruction_stays_neutral_about_who_the_user_is() {
        let lower = QUERY_PLAN_INSTRUCTION.to_lowercase();
        for word in ["patient", "doctor", "practitioner", "practice", "gp"] {
            assert!(!lower.contains(word), "{word:?} found in QUERY_PLAN_INSTRUCTION");
        }
    }

    fn text_row(values: &[&str]) -> Vec<CellValue> {
        values.iter().map(|v| CellValue::Text(v.to_string())).collect()
    }

    fn fixture() -> (Workbook, TabularInventory) {
        let workbook = Workbook {
            sheets: vec![SheetData {
                name: "factures".to_string(),
                rows: vec![
                    text_row(&["fournisseur", "montant"]),
                    text_row(&["Alpha", "120,50"]),
                    text_row(&["Beta", "75,00"]),
                    text_row(&["Alpha", "30,00"]),
                ],
            }],
        };
        let inventory =
            TabularInventory::build("factures.csv", "hash-1", TabularFormat::Csv, &workbook, "fr-FR");
        (workbook, inventory)
    }

    #[test]
    fn a_built_message_never_carries_a_fixture_cell_value() {
        // The question itself is free text and may legitimately echo a cell value back (she can
        // type "Alpha" in her own question); what must never happen is the *schema* this function
        // builds leaking one on its own. Checked by using a question that names none of them, so
        // any of these strings found in the message can only have come from the schema.
        let (_, inventory) = fixture();
        let message = build_schema_message("What is the biggest supplier's total?", &inventory, None);
        for value in ["Alpha", "Beta", "120,50", "75,00", "30,00"] {
            assert!(!message.contains(value), "{value:?} leaked into the model-facing message");
        }
        assert!(message.contains("factures"), "the sheet name is schema, not a value");
        assert!(message.contains("fournisseur") && message.contains("montant"));
    }

    #[test]
    fn a_fenced_reply_with_prose_around_it_still_parses() {
        let raw = "Sure, here it is:\n```json\n{\"aggregate\": {\"op\": \"sum\", \"column\": \"montant\"}}\n```\nHope that helps!";
        let plan = parse_response(raw).expect("extracts the JSON object");
        assert_eq!(plan.aggregate.unwrap().op, PlanAggregateOp::Sum);
    }

    #[test]
    fn an_unknown_field_is_rejected_not_ignored() {
        let raw = r#"{"aggregate": {"op": "sum", "column": "montant"}, "made_up_field": true}"#;
        assert!(parse_response(raw).is_none());
    }

    /// The exact workbook session 14's manual validation pass used by hand: `fournisseur`,
    /// `montant`, Alpha/Beta/Gamma cycling, 10..90 by tens. Total 450.
    fn factures_test_fixture() -> (Workbook, TabularInventory) {
        let workbook = Workbook {
            sheets: vec![SheetData {
                name: "factures-test".to_string(),
                rows: {
                    let mut rows = vec![text_row(&["fournisseur", "montant"])];
                    for (supplier, amount) in [
                        ("Alpha", "10"), ("Beta", "20"), ("Gamma", "30"),
                        ("Alpha", "40"), ("Beta", "50"), ("Gamma", "60"),
                        ("Alpha", "70"), ("Beta", "80"), ("Gamma", "90"),
                    ] {
                        rows.push(text_row(&[supplier, amount]));
                    }
                    rows
                },
            }],
        };
        let inventory = TabularInventory::build(
            "factures-test.csv", "hash-live-probe", TabularFormat::Csv, &workbook, "en-US",
        );
        (workbook, inventory)
    }

    /// Asserts `plan` resolves and executes to the sum 450 over `factures_test_fixture`'s rows -
    /// shared by every live-captured-reply regression test below, which otherwise differ only in
    /// the raw JSON a real model actually sent back.
    fn assert_resolves_to_the_grand_total(plan: &QueryPlan) {
        let (workbook, inventory) = factures_test_fixture();
        match resolve(plan, &inventory, &workbook, None) {
            PlanResolution::Operation(operation) => {
                let outcome = engine::execute(&workbook, &inventory, plan.sheet.as_deref(), None, &operation, "en-US");
                match outcome {
                    crate::tabular::engine::TabularOutcome::Value {
                        value: TabularValue::Sum(total), ..
                    } => assert!((total.value - 450.0).abs() < 1e-9, "expected 450, got {total:?}"),
                    other => panic!("expected a sum, got {other:?}"),
                }
            }
            _ => panic!("expected the plan to resolve to an operation"),
        }
    }

    #[test]
    fn a_real_gemma2_2b_reply_with_a_null_sort_column_still_resolves_to_the_right_sum() {
        // Captured verbatim (minus the ```json fence) from a live probe of gemma2:2b against the
        // real gateway during session 14's manual validation, for "What's the grand total we've
        // taken in altogether?". The model filled `sort` in even though nothing asked it to sort,
        // with `"column": null` inside it - and set `unsupported: true` beside a plan that is
        // otherwise exactly right.
        let raw = r#"{"sheet": "factures-test", "filters": [], "group_by": null, "aggregate": {"op": "sum", "column": "montant"}, "sort": {"column": null, "descending": null}, "limit": null, "unsupported": true}"#;
        let plan = parse_response(raw).expect("a real model's null-sort-column reply still parses");
        assert_resolves_to_the_grand_total(&plan);
    }

    #[test]
    fn a_real_gemma2_2b_reply_with_a_redundant_null_filter_still_resolves_to_the_right_sum() {
        // Captured verbatim from a second live probe of gemma2:2b, same question, later the same
        // day: a recurring pattern distinct from the one above - a single `filters` entry that
        // restates the aggregate's own `op` ("sum", not a valid filter `op` at all) with a `null`
        // value, as if the aggregate also had to be echoed into `filters`. Two defences meet here
        // at once: `lenient_filters` drops the entry at parse time because `"op": "sum"` is not a
        // real `PlanFilterOp`, so the array still parses; `resolve_one_filter`'s own null-value
        // check would have dropped it anyway had the op been valid. Either alone would have been
        // enough for this exact capture; both exist because each has been the one that mattered
        // for a different real reply.
        let raw = r#"{"sheet": "factures-test", "filters": [{"column": "montant", "op": "sum", "value": null}], "group_by": null, "aggregate": {"op": "sum", "column": "montant"}, "sort": {"column": null, "descending": null}, "limit": null, "unsupported": true}"#;
        let plan = parse_response(raw).expect("a real model's redundant-filter reply still parses");
        assert!(plan.filters.is_empty(), "the bogus filter entry must not survive parsing");
        assert_resolves_to_the_grand_total(&plan);
    }

    #[test]
    fn a_real_gemma2_2b_reply_with_a_redundant_filter_carrying_a_real_number_still_resolves() {
        // A third live capture, same recurring pattern, this time with a plausible-looking number
        // (1000) instead of `null` after the still-invalid `"op": "sum"` - confirming the fix is
        // about the invalid `op` itself, not merely about spotting a `null`: `lenient_filters`
        // drops this entry at parse time regardless of what its `value` was.
        let raw = r#"{"sheet": "factures-test", "filters": [{"column": "montant", "op": "sum", "value": 1000}], "group_by": null, "aggregate": {"op": "sum", "column": "montant"}, "sort": {"column": null, "descending": null}, "limit": null, "unsupported": true}"#;
        let plan = parse_response(raw).expect("a real model's redundant-filter reply still parses");
        assert!(plan.filters.is_empty(), "the bogus filter entry must not survive parsing");
        assert_resolves_to_the_grand_total(&plan);
    }

    #[test]
    fn a_real_gemma2_2b_reply_using_in_with_a_bare_string_still_resolves_to_betas_total() {
        // Captured verbatim from a live probe of gemma2:2b for the French phrasing of "how much
        // have we taken in from Beta specifically?": `"op": "in"` with a bare string value
        // (`"Beta"`), not the array `in` otherwise expects - the same confusion as writing `eq`,
        // just with the other op's name. Beta's rows (10, 40, 70 in the fixture's indexing) sum to
        // 150.
        let raw = r#"{"sheet": "factures-test", "filters": [{"column": "fournisseur", "op": "in", "value": "Beta"}], "group_by": null, "aggregate": {"op": "sum", "column": "montant"}, "sort": {"column": null, "descending": null}, "limit": null, "unsupported": true}"#;
        let plan = parse_response(raw).expect("a real model's bare-string in-filter reply still parses");

        let (workbook, inventory) = factures_test_fixture();
        match resolve(&plan, &inventory, &workbook, None) {
            PlanResolution::Operation(operation) => {
                let outcome = engine::execute(&workbook, &inventory, plan.sheet.as_deref(), None, &operation, "en-US");
                match outcome {
                    crate::tabular::engine::TabularOutcome::Value {
                        value: TabularValue::Sum(total), ..
                    } => assert!((total.value - 150.0).abs() < 1e-9, "expected 150, got {total:?}"),
                    other => panic!("expected a sum, got {other:?}"),
                }
            }
            other => panic!("expected the plan to resolve to an operation, got {other:?}"),
        }
    }

    #[test]
    fn an_explicit_null_for_filters_is_read_as_empty_not_a_parse_failure() {
        // A real small model (ministral-3:3b, probed live against the gateway) wrote this exact
        // shape for a perfectly good plan - `"filters": null` rather than omitting the key or
        // writing `[]`. `#[serde(default)]` alone only covers a *missing* key, not an explicit
        // `null`, so this plan must not be silently discarded.
        let raw = r#"{"filters": null, "group_by": "fournisseur", "aggregate": {"op": "sum", "column": "montant"}, "sort": {"column": "montant", "descending": true}, "unsupported": false}"#;
        let plan = parse_response(raw).expect("a plan with an explicit null filters list still parses");
        assert!(plan.filters.is_empty());
    }

    #[test]
    fn an_unknown_op_is_rejected() {
        let raw = r#"{"aggregate": {"op": "total", "column": "montant"}}"#;
        assert!(parse_response(raw).is_none());
    }

    fn plan_sum(column: &str) -> QueryPlan {
        QueryPlan {
            sheet: None,
            filters: Vec::new(),
            group_by: None,
            aggregate: Some(PlanAggregate { op: PlanAggregateOp::Sum, column: Some(column.to_string()) }),
            sort: None,
            limit: None,
            unsupported: false,
        }
    }

    #[test]
    fn a_plain_sum_plan_maps_to_the_sum_operation() {
        let (workbook, inventory) = fixture();
        let plan = plan_sum("montant");
        match resolve(&plan, &inventory, &workbook, None) {
            PlanResolution::Operation(Operation::Sum { column, filters }) => {
                assert_eq!(column, "montant");
                assert!(filters.is_empty());
            }
            _ => panic!("expected a sum operation"),
        }
    }

    #[test]
    fn a_filter_with_a_null_value_is_dropped_not_refused() {
        // A well-formed `op` and column with a literal JSON `null` for `value` is not a guess to
        // catch - it is the model leaving a slot empty (observed live, `gemma2:2b`, session 14's
        // manual validation pass). Dropped, so the rest of the plan - here, a plain unfiltered
        // sum - still runs.
        let (workbook, inventory) = fixture();
        let mut plan = plan_sum("montant");
        plan.filters.push(PlanFilter {
            column: "fournisseur".to_string(),
            op: PlanFilterOp::Eq,
            value: serde_json::Value::Null,
        });
        match resolve(&plan, &inventory, &workbook, None) {
            PlanResolution::Operation(Operation::Sum { filters, .. }) => {
                assert!(filters.is_empty(), "the null-valued filter must not survive resolution");
            }
            other => panic!("expected the sum to still resolve, dropping the empty filter: {other:?}"),
        }
    }

    #[test]
    fn an_eq_filter_is_resolved_against_real_data_and_canonicalised() {
        let (workbook, inventory) = fixture();
        let mut plan = plan_sum("montant");
        plan.filters.push(PlanFilter {
            column: "fournisseur".to_string(),
            op: PlanFilterOp::Eq,
            value: serde_json::json!("alpha"),
        });
        match resolve(&plan, &inventory, &workbook, None) {
            PlanResolution::Operation(Operation::Sum { filters, .. }) => {
                assert_eq!(filters.len(), 1);
                assert_eq!(filters[0].column, "fournisseur");
                assert_eq!(filters[0].comparison, Comparison::Equals("Alpha".to_string()));
            }
            _ => panic!("expected a resolved operation"),
        }
    }

    #[test]
    fn an_eq_filter_naming_an_absent_value_is_value_not_found_with_close_values() {
        let (workbook, inventory) = fixture();
        let mut plan = plan_sum("montant");
        plan.filters.push(PlanFilter {
            column: "fournisseur".to_string(),
            op: PlanFilterOp::Eq,
            value: serde_json::json!("Alfa"),
        });
        match resolve(&plan, &inventory, &workbook, None) {
            PlanResolution::ValueNotFound { value, close } => {
                assert_eq!(value, "Alfa");
                assert!(close.contains(&"Alpha".to_string()));
            }
            _ => panic!("expected ValueNotFound"),
        }
    }

    #[test]
    fn a_bare_unsupported_true_with_nothing_else_filled_in_is_unsupported() {
        let plan = QueryPlan {
            sheet: None,
            filters: Vec::new(),
            group_by: None,
            aggregate: None,
            sort: None,
            limit: None,
            unsupported: true,
        };
        let (workbook, inventory) = fixture();
        assert!(matches!(resolve(&plan, &inventory, &workbook, None), PlanResolution::Unsupported));
    }

    #[test]
    fn unsupported_true_does_not_veto_an_otherwise_complete_plan() {
        // A real small model probed live against the gateway (session 14's manual validation)
        // reliably set `unsupported: true` *alongside* a correct, complete plan - evidently
        // reading the instruction's hedge as something to fill in regardless. The concrete fields
        // are trusted over the ambiguous flag.
        let mut plan = plan_sum("montant");
        plan.unsupported = true;
        let (workbook, inventory) = fixture();
        match resolve(&plan, &inventory, &workbook, None) {
            PlanResolution::Operation(Operation::Sum { column, filters }) => {
                assert_eq!(column, "montant");
                assert!(filters.is_empty());
            }
            _ => panic!("expected the sum operation to still run"),
        }
    }

    #[test]
    fn group_by_with_no_aggregate_is_unsupported() {
        let plan = QueryPlan {
            sheet: None,
            filters: Vec::new(),
            group_by: Some("fournisseur".to_string()),
            aggregate: None,
            sort: None,
            limit: None,
            unsupported: false,
        };
        let (workbook, inventory) = fixture();
        assert!(matches!(resolve(&plan, &inventory, &workbook, None), PlanResolution::Unsupported));
    }

    #[test]
    fn group_by_sum_with_descending_sort_and_a_limit_is_top_groups() {
        let plan = QueryPlan {
            sheet: None,
            filters: Vec::new(),
            group_by: Some("fournisseur".to_string()),
            aggregate: Some(PlanAggregate { op: PlanAggregateOp::Sum, column: Some("montant".to_string()) }),
            sort: Some(PlanSort { column: Some("montant".to_string()), descending: true }),
            limit: Some(2),
            unsupported: false,
        };
        let (workbook, inventory) = fixture();
        match resolve(&plan, &inventory, &workbook, None) {
            PlanResolution::Operation(Operation::TopGroups { n, .. }) => assert_eq!(n, 2),
            _ => panic!("expected a top-groups operation"),
        }
    }

    #[test]
    fn group_by_mean_is_unsupported_since_no_operation_computes_it() {
        let plan = QueryPlan {
            sheet: None,
            filters: Vec::new(),
            group_by: Some("fournisseur".to_string()),
            aggregate: Some(PlanAggregate { op: PlanAggregateOp::Mean, column: Some("montant".to_string()) }),
            sort: None,
            limit: None,
            unsupported: false,
        };
        let (workbook, inventory) = fixture();
        assert!(matches!(resolve(&plan, &inventory, &workbook, None), PlanResolution::Unsupported));
    }
}
