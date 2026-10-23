//! The formula checker (TABULAR step 07, plan decision 13): every formula a model writes is read by
//! the program and compared with what the program knows, and the result is shown beside the answer,
//! never edited into it.
//!
//! What it knows: the function names of the spreadsheet application in the user's language and in the
//! other shipped languages (`resources/formula-functions/<application>/<locale>.json`), the argument
//! separator of that language, and the structure of the workbooks the question concerned (the cards:
//! sheet names, column positions, header names). It never knows a cell's content, so it never says a
//! formula is right: only that what it can look up exists.
//!
//! **Grammar supported.** `=` then function calls `NAME(arg; arg)`, operators, numbers, text between
//! double quotes (`""` is a quote), cell and range references (`A1`, `$B$2:$C$9`, `D:D`), a sheet
//! prefix (`Sheet1!A1`, `'Sheet 1'!A1`), an external workbook prefix (`[Book.xlsx]Sheet1!A1`) and a
//! header name in brackets (`[Amount]`). **Deliberately skipped, and silent:** array constants between
//! braces, structured references (`Table1[Column]`, `[@Column]`), defined names, row ranges, R1C1
//! notation, a decimal mark mistaken for an argument separator (`1,5` in a language whose decimal mark
//! is the comma), and every unknown function name of a formula that defines its own functions (`LET`,
//! `LAMBDA`). A formula the checker cannot read to the end is checked as far as it goes.
//!
//! Pure: no `tauri::` import, no file, no network. Machine codes only; the interface writes the
//! sentences in the user's language.

use std::collections::HashSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::card::{column_letter, SheetShape, WorkbookCard, WorkbookShape};
use crate::file_reference::fold_text;

/// The application a dictionary is for; the setting `spreadsheet_application` chooses one.
pub const EXCEL: &str = "excel";

/// The most formulas read from one answer.
const MAX_FORMULAS: usize = 20;

/// The last column a sheet can have (`XFD`), 0-indexed.
const LAST_COLUMN: usize = 16_383;

const SPREADSHEET_EXTENSIONS: [&str; 5] = [".xlsx", ".xlsm", ".xls", ".ods", ".csv"];

// --- The dictionaries -----------------------------------------------------------------------------

const DICTIONARY_FILES: [(&str, &str, &str); 2] = [
    (
        EXCEL,
        "fr-FR",
        include_str!("../../resources/formula-functions/excel/fr-FR.json"),
    ),
    (
        EXCEL,
        "en-US",
        include_str!("../../resources/formula-functions/excel/en-US.json"),
    ),
];

#[derive(Deserialize)]
struct DictionaryFile {
    application: String,
    locale: String,
    argument_separator: String,
    decimal_separator: String,
    /// `[canonical English name, name in this language]`.
    functions: Vec<(String, String)>,
    /// Canonical names of the functions that read or call something outside the file.
    reaches_outside: Vec<String>,
}

/// The function names of one application in one language.
#[derive(Debug)]
pub struct Dictionary {
    pub application: String,
    pub locale: String,
    pub argument_separator: char,
    pub decimal_separator: char,
    functions: Vec<(String, String)>,
    outside: HashSet<String>,
}

impl Dictionary {
    fn canonical_of(&self, written: &str) -> Option<&str> {
        self.functions
            .iter()
            .find(|(_, local)| local.eq_ignore_ascii_case(written))
            .map(|(canonical, _)| canonical.as_str())
    }

    fn local_name_of(&self, canonical: &str) -> Option<&str> {
        self.functions
            .iter()
            .find(|(each, _)| each == canonical)
            .map(|(_, local)| local.as_str())
    }

    /// The canonical English names this dictionary covers, in file order.
    pub fn canonical_names(&self) -> Vec<&str> {
        self.functions
            .iter()
            .map(|(canonical, _)| canonical.as_str())
            .collect()
    }

    pub fn reaches_outside(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.outside.iter().map(String::as_str).collect();
        names.sort_unstable();
        names
    }
}

fn parsed_dictionaries() -> &'static Vec<Dictionary> {
    static ALL: OnceLock<Vec<Dictionary>> = OnceLock::new();
    ALL.get_or_init(|| {
        DICTIONARY_FILES
            .iter()
            .filter_map(|(_, _, body)| {
                let file: DictionaryFile = serde_json::from_str(body).ok()?;
                Some(Dictionary {
                    application: file.application,
                    locale: file.locale,
                    argument_separator: file.argument_separator.chars().next()?,
                    decimal_separator: file.decimal_separator.chars().next()?,
                    functions: file.functions,
                    outside: file.reaches_outside.into_iter().collect(),
                })
            })
            .collect()
    })
}

/// Every shipped dictionary of an application.
pub fn dictionaries(application: &str) -> Vec<&'static Dictionary> {
    parsed_dictionaries()
        .iter()
        .filter(|each| each.application == application)
        .collect()
}

/// The dictionary for a locale: the exact tag, then any dictionary of the same language. `None` when
/// the application has none for that language (a missing combination makes the checker silent).
fn dictionary_for(application: &str, locale: &str) -> Option<&'static Dictionary> {
    let all = dictionaries(application);
    let language = locale.split('-').next().unwrap_or(locale);
    all.iter()
        .copied()
        .find(|each| each.locale.eq_ignore_ascii_case(locale))
        .or_else(|| {
            all.iter().copied().find(|each| {
                each.locale
                    .split('-')
                    .next()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(language))
            })
        })
}

// --- What a check returns ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FormulaIssueCode {
    /// A function name that is in no shipped language.
    UnknownFunction,
    /// A function of the other language: `expected` is its name in the user's.
    FunctionInOtherLanguage,
    /// A comma where the language writes a semicolon, or the reverse: `expected` is the right one.
    WrongSeparator,
    UnbalancedParentheses,
    UnbalancedQuotes,
    /// A reference to a column beyond the last of the sheet: `expected` is the last letter.
    UnknownColumnLetter,
    /// A header name in brackets that no column of the cards has.
    UnknownColumnName,
    /// A sheet name that no workbook of the cards has.
    UnknownSheet,
    /// A workbook file name used where a sheet name goes (`file.csv!B2`).
    FileUsedAsSheet,
    /// A workbook file name that is not one of the cards.
    UnknownWorkbook,
    /// A function or a reference that reads or calls something outside the file.
    ReachesOutside,
}

impl FormulaIssueCode {
    /// The problems a second request to the model can fix: a name or a letter it got wrong.
    pub fn is_repairable(self) -> bool {
        matches!(
            self,
            Self::UnknownFunction | Self::FunctionInOtherLanguage | Self::UnknownColumnLetter
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnknownFunction => "unknown_function",
            Self::FunctionInOtherLanguage => "function_in_other_language",
            Self::WrongSeparator => "wrong_separator",
            Self::UnbalancedParentheses => "unbalanced_parentheses",
            Self::UnbalancedQuotes => "unbalanced_quotes",
            Self::UnknownColumnLetter => "unknown_column_letter",
            Self::UnknownColumnName => "unknown_column_name",
            Self::UnknownSheet => "unknown_sheet",
            Self::FileUsedAsSheet => "file_used_as_sheet",
            Self::UnknownWorkbook => "unknown_workbook",
            Self::ReachesOutside => "reaches_outside",
        }
    }
}

/// One problem found in one formula: what was written, what was expected instead when the program
/// knows, and where (a sheet or a workbook) when that matters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormulaIssue {
    pub code: FormulaIssueCode,
    pub found: String,
    pub expected: Option<String>,
    pub place: Option<String>,
}

/// A problem, with the formula it was found in, as it goes beside the answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormulaNote {
    pub formula: String,
    pub issue: FormulaIssue,
}

/// What `check_answer` found in a whole answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormulaReport {
    pub notes: Vec<FormulaNote>,
    /// How many formulas had no problem the program could find.
    pub verified: usize,
}

impl FormulaReport {
    /// Whether a second request could fix at least one of the problems.
    pub fn has_repairable(&self) -> bool {
        self.notes
            .iter()
            .any(|note| note.issue.code.is_repairable())
    }
}

// --- The context ---------------------------------------------------------------------------------------

/// What a formula is compared with: the dictionaries and the structure of the workbooks the question
/// concerned. Built once per turn from the cards that were sent.
#[derive(Debug)]
pub struct FormulaContext {
    active: &'static Dictionary,
    others: Vec<&'static Dictionary>,
    workbooks: Vec<WorkbookShape>,
}

impl FormulaContext {
    /// `None` when the application has no dictionary for the locale: nothing is then checked.
    pub fn new(application: &str, locale: &str, cards: &[WorkbookCard]) -> Option<Self> {
        let active = dictionary_for(application, locale)?;
        let others = dictionaries(application)
            .into_iter()
            .filter(|each| !std::ptr::eq(*each, active))
            .collect();
        Some(Self {
            active,
            others,
            workbooks: cards.iter().map(|card| card.shape.clone()).collect(),
        })
    }

    /// The workbooks and sheets of the cards, which a reference can be compared with.
    fn has_structure(&self) -> bool {
        self.workbooks.iter().any(|each| !each.sheets.is_empty())
    }
}

// --- Extraction ------------------------------------------------------------------------------------

/// The formulas written in an answer: the lines of a fenced block that start with `=` (a formula that
/// goes on over the next lines is joined while its brackets are open), inline code that starts with
/// `=`, and plain lines that start with `=` (a list marker before it is ignored, a full stop or a comma
/// after it is not part of it). Each formula once, in the order written, at most `MAX_FORMULAS`.
pub fn extract_formulas(answer: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut push = |formula: String| {
        if looks_like_a_formula(&formula) && !found.contains(&formula) && found.len() < MAX_FORMULAS
        {
            found.push(formula);
        }
    };
    let lines: Vec<&str> = answer.lines().collect();
    let mut fenced = false;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        index += 1;
        if line.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            if line.starts_with('=') {
                let mut formula = line.to_string();
                while open_depth(&formula) > 0 && index < lines.len() {
                    let next = lines[index].trim();
                    if next.starts_with('=') || next.starts_with("```") {
                        break;
                    }
                    formula.push(' ');
                    formula.push_str(next);
                    index += 1;
                }
                push(formula);
            }
            continue;
        }
        for code in inline_code(line) {
            if code.starts_with('=') {
                push(code);
            }
        }
        let bare = line
            .trim_start_matches(['-', '*', '>', ' '])
            .trim_start_matches(|c: char| c.is_ascii_digit())
            .trim_start_matches(". ")
            .trim_start();
        if bare.starts_with('=') && !line.contains('`') {
            push(bare.trim_end_matches(['.', ',', ';', ' ']).to_string());
        }
    }
    found
}

/// What sits between single backticks on a line.
fn inline_code(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut parts = line.split('`');
    parts.next();
    while let Some(code) = parts.next() {
        if parts.next().is_none() && !line.ends_with('`') {
            break;
        }
        let code = code.trim();
        if !code.is_empty() {
            out.push(code.to_string());
        }
    }
    out
}

/// `=` followed by something a formula can start with; `==` or a bare `=` is not one.
fn looks_like_a_formula(text: &str) -> bool {
    let Some(rest) = text.strip_prefix('=') else {
        return false;
    };
    let rest = rest.trim_start();
    rest.chars().next().is_some_and(|c| {
        c.is_alphanumeric()
            || matches!(
                c,
                '(' | '-' | '+' | '"' | '\'' | '$' | '_' | '[' | '{' | '@'
            )
    })
}

/// How many brackets are still open at the end of the text (text between quotes ignored).
fn open_depth(text: &str) -> i32 {
    let mut depth = 0;
    let mut in_text = false;
    for c in text.chars() {
        match c {
            '"' => in_text = !in_text,
            '(' if !in_text => depth += 1,
            ')' if !in_text => depth -= 1,
            _ => {}
        }
    }
    depth
}

// --- The checker ----------------------------------------------------------------------------------------

/// Every formula of `answer`, checked: the notes in the order the formulas were written, and how many
/// formulas had nothing to report.
pub fn check_answer(answer: &str, context: &FormulaContext) -> FormulaReport {
    let mut report = FormulaReport::default();
    for formula in extract_formulas(answer) {
        let issues = check_formula(&formula, context);
        if issues.is_empty() {
            report.verified += 1;
        }
        report
            .notes
            .extend(issues.into_iter().map(|issue| FormulaNote {
                formula: formula.clone(),
                issue,
            }));
    }
    report
}

/// What a reference is read against.
enum Scope {
    /// No prefix: any sheet of the cards.
    Any,
    /// A sheet named in the cards (several workbooks may have one of that name).
    Sheets(Vec<SheetShape>),
    /// A workbook written where a sheet goes: its own sheets.
    Workbook(WorkbookShape),
    /// A prefix the program cannot place (already reported): no column is checked.
    Unplaced,
}

/// The problems the program can find in one formula, each once. Empty means: nothing it can look up is
/// wrong (never that the formula is right).
pub fn check_formula(formula: &str, context: &FormulaContext) -> Vec<FormulaIssue> {
    let text = formula.trim();
    let text = text.strip_prefix('=').unwrap_or(text);
    let chars: Vec<char> = text.chars().collect();
    let mut checker = Checker {
        context,
        chars: &chars,
        issues: Vec::new(),
        defines_names: {
            let upper = text.to_uppercase();
            upper.contains("LET(") || upper.contains("LAMBDA(")
        },
        separator_reported: false,
    };
    checker.run();
    checker.issues
}

struct Checker<'a> {
    context: &'a FormulaContext,
    chars: &'a [char],
    issues: Vec<FormulaIssue>,
    /// `LET` or `LAMBDA` is used: a name that is not a function may be one the formula defines.
    defines_names: bool,
    separator_reported: bool,
}

impl Checker<'_> {
    fn report(
        &mut self,
        code: FormulaIssueCode,
        found: &str,
        expected: Option<String>,
        place: Option<String>,
    ) {
        let issue = FormulaIssue {
            code,
            found: found.to_string(),
            expected,
            place,
        };
        if !self.issues.contains(&issue) {
            self.issues.push(issue);
        }
    }

    fn at(&self, index: usize) -> Option<char> {
        self.chars.get(index).copied()
    }

    fn run(&mut self) {
        let mut depth: i32 = 0;
        let mut unbalanced = false;
        let mut stopped = false;
        let mut index = 0;
        let separator = self.context.active.argument_separator;
        while index < self.chars.len() {
            let c = self.chars[index];
            match c {
                '"' => match self.skip_text(index, '"') {
                    Some(next) => index = next,
                    None => {
                        self.report(FormulaIssueCode::UnbalancedQuotes, "\"", None, None);
                        stopped = true;
                        break;
                    }
                },
                '\'' => {
                    let Some(next) = self.skip_text(index, '\'') else {
                        self.report(FormulaIssueCode::UnbalancedQuotes, "'", None, None);
                        stopped = true;
                        break;
                    };
                    if self.at(next) == Some('!') {
                        let name: String = self.chars[index + 1..next - 1].iter().collect();
                        let name = name.replace("''", "'");
                        index = self.reference_after_prefix(&name, next + 1);
                    } else {
                        index = next;
                    }
                }
                '{' => {
                    index = self.skip_braces(index);
                }
                '[' => {
                    index = self.bracket(index);
                }
                '(' => {
                    depth += 1;
                    index += 1;
                }
                ')' => {
                    depth -= 1;
                    if depth < 0 {
                        unbalanced = true;
                        depth = 0;
                    }
                    index += 1;
                }
                ',' | ';' => {
                    if depth > 0 && c != separator && !self.is_decimal_mark(index) {
                        self.separator_once(separator);
                    }
                    index += 1;
                }
                '|' => {
                    if index > 0 && self.chars[index - 1].is_alphanumeric() {
                        self.report(FormulaIssueCode::ReachesOutside, "DDE", None, None);
                        stopped = true;
                        break;
                    }
                    index += 1;
                }
                _ if c.is_ascii_digit() => index = self.skip_number(index),
                _ if c.is_alphabetic() || c == '_' || c == '$' => {
                    index = self.word(index);
                }
                _ => index += 1,
            }
        }
        if !stopped && (depth > 0 || unbalanced) {
            self.report(FormulaIssueCode::UnbalancedParentheses, "(", None, None);
        }
    }

    fn separator_once(&mut self, expected: char) {
        if !self.separator_reported {
            self.separator_reported = true;
            let written = if expected == ';' { "," } else { ";" };
            self.report(
                FormulaIssueCode::WrongSeparator,
                written,
                Some(expected.to_string()),
                None,
            );
        }
    }

    /// A comma between two digits in a language whose decimal mark is the comma.
    fn is_decimal_mark(&self, index: usize) -> bool {
        self.chars[index] == self.context.active.decimal_separator
            && index > 0
            && self.chars[index - 1].is_ascii_digit()
            && self.at(index + 1).is_some_and(|c| c.is_ascii_digit())
    }

    /// From the opening quote at `start` to just after the closing one; a doubled quote is a quote.
    fn skip_text(&self, start: usize, quote: char) -> Option<usize> {
        let mut index = start + 1;
        while index < self.chars.len() {
            if self.chars[index] == quote {
                if self.at(index + 1) == Some(quote) {
                    index += 2;
                    continue;
                }
                return Some(index + 1);
            }
            index += 1;
        }
        None
    }

    /// An array constant between braces is skipped whole.
    fn skip_braces(&self, start: usize) -> usize {
        let mut index = start + 1;
        while index < self.chars.len() && self.chars[index] != '}' {
            index += 1;
        }
        (index + 1).min(self.chars.len())
    }

    fn skip_number(&self, start: usize) -> usize {
        let mut index = start;
        while index < self.chars.len() {
            let c = self.chars[index];
            let decimal = (c == '.' || c == ',')
                && c == self.context.active.decimal_separator
                && self.at(index + 1).is_some_and(|next| next.is_ascii_digit());
            if c.is_ascii_alphanumeric() || decimal {
                index += 1;
            } else {
                break;
            }
        }
        index
    }

    /// `[...]` at `start`: an external workbook prefix, a header name, or the column of a structured
    /// reference (silent). Returns the index to go on from.
    fn bracket(&mut self, start: usize) -> usize {
        let mut end = start + 1;
        while end < self.chars.len() && self.chars[end] != ']' {
            end += 1;
        }
        if end >= self.chars.len() {
            return self.chars.len();
        }
        let inner: String = self.chars[start + 1..end].iter().collect();
        let after = end + 1;
        // `Table1[Column]`: a word right before the bracket makes it a structured reference.
        let structured = start > 0
            && (self.chars[start - 1].is_alphanumeric()
                || matches!(self.chars[start - 1], '_' | ']' | ')'));
        if structured || inner.starts_with('@') || inner.starts_with('#') {
            return after;
        }
        if self
            .at(after)
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '\'')
        {
            // `[Book.xlsx]Sheet1!A1`
            let mut stop = after;
            while stop < self.chars.len() && self.chars[stop] != '!' && self.chars[stop] != '(' {
                stop += 1;
            }
            if self.at(stop) == Some('!') {
                self.external_workbook(&inner);
                return self.skip_reference_span(stop + 1);
            }
            return after;
        }
        self.header_name(&inner);
        after
    }

    fn external_workbook(&mut self, book: &str) {
        self.report(FormulaIssueCode::ReachesOutside, book, None, None);
        if self.context.has_structure() && self.workbook_named(book).is_none() {
            self.report(FormulaIssueCode::UnknownWorkbook, book, None, None);
        }
    }

    /// A header name written in brackets: reported when no column of the cards has it.
    fn header_name(&mut self, name: &str) {
        let folded = fold_text(name);
        let headers: Vec<&SheetShape> = self
            .context
            .workbooks
            .iter()
            .flat_map(|each| each.sheets.iter())
            .filter(|sheet| sheet.has_header)
            .collect();
        if headers.is_empty() || folded.is_empty() {
            return;
        }
        let known = headers.iter().any(|sheet| {
            sheet
                .columns
                .iter()
                .any(|column| !column.is_empty() && fold_text(column) == folded)
        });
        if !known {
            self.report(FormulaIssueCode::UnknownColumnName, name, None, None);
        }
    }

    /// A word at `start`: a function name when a bracket follows, a sheet prefix when `!` follows, a
    /// reference or a name otherwise. Returns the index to go on from.
    fn word(&mut self, start: usize) -> usize {
        let mut end = start;
        while end < self.chars.len() {
            let c = self.chars[end];
            if c.is_alphanumeric() || matches!(c, '_' | '.' | '$') {
                end += 1;
            } else {
                break;
            }
        }
        let word: String = self.chars[start..end].iter().collect();
        match self.at(end) {
            Some('(') => {
                self.function(&word);
                end
            }
            Some('!') => self.reference_after_prefix(&word, end + 1),
            Some(':') if column_position(&word).is_some() => {
                let stop = self.skip_reference_span(end);
                let span: String = self.chars[start..stop].iter().collect();
                self.reference(&span, &Scope::Any, None);
                stop
            }
            _ => {
                if let Some(true) = column_position(&word).map(|(_, has_row)| has_row) {
                    self.reference(&word, &Scope::Any, None);
                }
                end
            }
        }
    }

    /// The span of a reference from `start`: letters, digits, `$` and `:`.
    fn skip_reference_span(&self, start: usize) -> usize {
        let mut index = start;
        while index < self.chars.len()
            && (self.chars[index].is_ascii_alphanumeric() || matches!(self.chars[index], '$' | ':'))
        {
            index += 1;
        }
        index
    }

    /// A prefix then `!` then a reference, the prefix being `name`. Returns where to go on from.
    fn reference_after_prefix(&mut self, name: &str, span_start: usize) -> usize {
        let stop = self.skip_reference_span(span_start);
        let span: String = self.chars[span_start..stop].iter().collect();
        let (scope, label) = self.scope_of(name);
        self.reference(&span, &scope, label.as_deref());
        stop
    }

    /// Finds the workbook a name stands for: its plain path, its file name, or the name of a CSV
    /// without its extension (which is how a spreadsheet application names the sheet of a CSV).
    fn workbook_named(&self, name: &str) -> Option<&WorkbookShape> {
        let folded = fold_text(name);
        self.context.workbooks.iter().find(|each| {
            let path = fold_text(&each.path);
            let file = path.rsplit('/').next().unwrap_or(&path).to_string();
            folded == path || folded == file || (!each.is_csv && folded == stem_of(&file))
        })
    }

    /// What a sheet prefix stands for, reporting what is wrong with it. The label is the workbook or
    /// sheet to name when a column is beyond the end.
    fn scope_of(&mut self, name: &str) -> (Scope, Option<String>) {
        if !self.context.has_structure() {
            return (Scope::Any, None);
        }
        let folded = fold_text(name);
        let context = self.context;
        let wanted = &folded;
        let sheets: Vec<SheetShape> = context
            .workbooks
            .iter()
            .flat_map(|workbook| {
                workbook.sheets.iter().filter(move |sheet| {
                    fold_text(&sheet.name) == *wanted
                        || (workbook.is_csv && stem_of(&file_name(&workbook.path)) == *wanted)
                })
            })
            .cloned()
            .collect();
        if !sheets.is_empty() {
            let label = Some(name.to_string());
            return (Scope::Sheets(sheets), label);
        }
        if let Some(workbook) = self.workbook_named(name).cloned() {
            self.report(FormulaIssueCode::FileUsedAsSheet, name, None, None);
            let label = Some(workbook.path.clone());
            return (Scope::Workbook(workbook), label);
        }
        if SPREADSHEET_EXTENSIONS
            .iter()
            .any(|extension| folded.ends_with(extension))
        {
            self.report(FormulaIssueCode::UnknownWorkbook, name, None, None);
        } else {
            self.report(FormulaIssueCode::UnknownSheet, name, None, None);
        }
        (Scope::Unplaced, None)
    }

    /// Checks the columns of a reference (`A1`, `B:B`, `$C$2:$D$9`) against `scope`.
    fn reference(&mut self, span: &str, scope: &Scope, label: Option<&str>) {
        for part in span.split(':') {
            let Some((position, _)) = column_position(part) else {
                continue;
            };
            let width = match scope {
                Scope::Unplaced => continue,
                Scope::Any => widest(self.context.workbooks.iter().flat_map(|w| &w.sheets)),
                Scope::Sheets(sheets) => widest(sheets.iter()),
                Scope::Workbook(workbook) => widest(workbook.sheets.iter()),
            };
            if width == 0 || position < width {
                continue;
            }
            self.report(
                FormulaIssueCode::UnknownColumnLetter,
                &column_letter(position),
                Some(column_letter(width - 1)),
                label.map(str::to_string),
            );
        }
    }

    /// A function name followed by a bracket.
    fn function(&mut self, written: &str) {
        let name = written.to_uppercase();
        let name = name
            .trim_start_matches("_XLFN.")
            .trim_start_matches("_XLWS.")
            .to_string();
        let active = self.context.active;
        let canonical = match active.canonical_of(&name) {
            Some(canonical) => Some(canonical.to_string()),
            None => {
                let other = self
                    .context
                    .others
                    .iter()
                    .find_map(|each| each.canonical_of(&name).map(str::to_string));
                match other {
                    Some(canonical) => {
                        let expected = active.local_name_of(&canonical).map(str::to_string);
                        self.report(
                            FormulaIssueCode::FunctionInOtherLanguage,
                            &name,
                            expected,
                            None,
                        );
                        Some(canonical)
                    }
                    None => {
                        if !self.defines_names {
                            self.report(FormulaIssueCode::UnknownFunction, &name, None, None);
                        }
                        None
                    }
                }
            }
        };
        if canonical.is_some_and(|canonical| active.outside.contains(&canonical)) {
            self.report(FormulaIssueCode::ReachesOutside, &name, None, None);
        }
    }
}

/// The most columns any of `sheets` has.
fn widest<'s>(sheets: impl Iterator<Item = &'s SheetShape>) -> usize {
    sheets.map(|sheet| sheet.columns.len()).max().unwrap_or(0)
}

/// The file name of a plain path, folded.
fn file_name(path: &str) -> String {
    let folded = fold_text(path);
    folded.rsplit('/').next().unwrap_or(&folded).to_string()
}

/// A file name without its extension.
fn stem_of(file: &str) -> String {
    match file.rfind('.') {
        Some(dot) if dot > 0 => file[..dot].to_string(),
        _ => file.to_string(),
    }
}

/// The column a reference part stands for, 0-indexed, and whether it carries a row number: `B2` is
/// `(1, true)`, `$D` is `(3, false)`. `None` for anything that is not a cell or a column.
fn column_position(part: &str) -> Option<(usize, bool)> {
    let plain: String = part.chars().filter(|c| *c != '$').collect();
    let letters: String = plain
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect();
    let rest = &plain[letters.len()..];
    if letters.is_empty() || letters.len() > 3 || !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let position = letters
        .to_ascii_uppercase()
        .bytes()
        .fold(0usize, |total, byte| {
            total * 26 + usize::from(byte - b'A' + 1)
        })
        - 1;
    (position <= LAST_COLUMN).then_some((position, !rest.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabular::card::{SheetShape, WorkbookShape};

    fn sheet(name: &str, columns: &[&str]) -> SheetShape {
        SheetShape {
            name: name.to_string(),
            columns: columns.iter().map(|each| each.to_string()).collect(),
            has_header: true,
        }
    }

    fn card(path: &str, is_csv: bool, sheets: Vec<SheetShape>) -> WorkbookCard {
        WorkbookCard {
            file: path.to_string(),
            text: String::new(),
            truncated: false,
            shape: WorkbookShape {
                path: path.to_string(),
                is_csv,
                sheets,
            },
        }
    }

    /// The two workbooks of the owner's test of 22 October 2026: five columns each.
    fn cards() -> Vec<WorkbookCard> {
        vec![
            card(
                "fournisseurs.csv",
                true,
                vec![sheet(
                    "fournisseurs",
                    &["date", "fournisseur", "tva", "echeance", "montant"],
                )],
            ),
            card(
                "reglements.csv",
                true,
                vec![sheet(
                    "reglements",
                    &["date", "fournisseur", "acompte", "reglement", "montant"],
                )],
            ),
            card(
                "ventes 2025.xlsx",
                false,
                vec![
                    sheet("Janvier", &["jour", "client", "total"]),
                    sheet("Data sheet", &["a", "b"]),
                ],
            ),
        ]
    }

    fn french() -> FormulaContext {
        FormulaContext::new(EXCEL, "fr-FR", &cards()).expect("a dictionary")
    }

    fn english() -> FormulaContext {
        FormulaContext::new(EXCEL, "en-US", &cards()).expect("a dictionary")
    }

    fn codes(formula: &str, context: &FormulaContext) -> Vec<&'static str> {
        check_formula(formula, context)
            .iter()
            .map(|issue| issue.code.as_str())
            .collect()
    }

    // --- dictionaries --------------------------------------------------------------------------------

    #[test]
    fn both_dictionaries_cover_the_same_canonical_functions() {
        let all = dictionaries(EXCEL);
        assert_eq!(all.len(), 2);
        let first: Vec<&str> = all[0].canonical_names();
        for each in &all[1..] {
            assert_eq!(first, each.canonical_names(), "{} differs", each.locale);
            assert_eq!(all[0].reaches_outside(), each.reaches_outside());
        }
        let mut seen = HashSet::new();
        for name in &first {
            assert!(seen.insert(*name), "{name} is listed twice");
        }
        for dictionary in &all {
            let mut locals = HashSet::new();
            for (canonical, local) in &dictionary.functions {
                assert!(
                    locals.insert(local.to_uppercase()),
                    "{local} is listed twice in {}",
                    dictionary.locale
                );
                assert!(!canonical.is_empty() && !local.is_empty());
            }
            for name in dictionary.reaches_outside() {
                assert!(
                    first.contains(&name),
                    "{name} reaches outside but is unknown"
                );
            }
        }
    }

    #[test]
    fn well_known_names_map_both_ways() {
        let french = dictionary_for(EXCEL, "fr-FR").unwrap();
        let english = dictionary_for(EXCEL, "en-US").unwrap();
        for (canonical, local) in [
            ("SUMIF", "SOMME.SI"),
            ("VLOOKUP", "RECHERCHEV"),
            ("IF", "SI"),
            ("SUMIFS", "SOMME.SI.ENS"),
            ("IFERROR", "SIERREUR"),
        ] {
            assert_eq!(french.local_name_of(canonical), Some(local));
            assert_eq!(french.canonical_of(local), Some(canonical));
            assert_eq!(english.local_name_of(canonical), Some(canonical));
        }
        assert_eq!(french.argument_separator, ';');
        assert_eq!(english.argument_separator, ',');
    }

    #[test]
    fn a_locale_falls_back_to_its_language_and_a_missing_one_is_silent() {
        assert_eq!(dictionary_for(EXCEL, "fr-CH").unwrap().locale, "fr-FR");
        assert_eq!(dictionary_for(EXCEL, "en-GB").unwrap().locale, "en-US");
        assert!(dictionary_for(EXCEL, "de-DE").is_none());
        assert!(dictionary_for("libreoffice", "fr-FR").is_none());
        assert!(FormulaContext::new("libreoffice", "fr-FR", &cards()).is_none());
    }

    // --- extraction ----------------------------------------------------------------------------------

    #[test]
    fn a_fenced_block_an_inline_code_and_a_bare_line_are_all_found() {
        let answer = "Try this:\n```\n=SOMME(A1:A9)\n=MOYENNE(B1:B9)\n```\n\
                      Or `=MAX(C1:C9)` for the maximum.\n\
                      - =MIN(D1:D9).\n\
                      3. =SOMME(E1:E9),\n\
                      Not a formula: a = b.";
        assert_eq!(
            extract_formulas(answer),
            [
                "=SOMME(A1:A9)",
                "=MOYENNE(B1:B9)",
                "=MAX(C1:C9)",
                "=MIN(D1:D9)",
                "=SOMME(E1:E9)"
            ]
        );
    }

    #[test]
    fn an_answer_with_no_formula_has_none() {
        assert!(
            extract_formulas("Nothing to compute.\n`SOMME(A1:A3)` without an equals sign")
                .is_empty()
        );
        assert!(extract_formulas("== titre ==\n=").is_empty());
        assert!(extract_formulas("").is_empty());
    }

    #[test]
    fn a_formula_is_listed_once_and_one_over_several_lines_is_joined() {
        let answer = "```\n=SI(A1>0;\n  B1;\n  C1)\n=SI(A1>0;B1;C1)\n=SI(A1>0; B1; C1)\n```";
        let found = extract_formulas(answer);
        assert_eq!(found[0], "=SI(A1>0; B1; C1)");
        assert_eq!(found.len(), 2);
    }

    // --- the codes, one positive and one negative each ----------------------------------------------

    #[test]
    fn a_function_in_no_language_is_unknown() {
        assert_eq!(
            codes("=JOIN(A2:B2; \" \")", &french()),
            ["unknown_function"]
        );
        assert!(codes("=SOMME(A2:B2)", &french()).is_empty());
    }

    #[test]
    fn a_function_of_the_other_language_names_the_expected_one() {
        let issues = check_formula("=SUMIF(B:B; \"x\"; E:E)", &french());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, FormulaIssueCode::FunctionInOtherLanguage);
        assert_eq!(issues[0].found, "SUMIF");
        assert_eq!(issues[0].expected.as_deref(), Some("SOMME.SI"));
        let issues = check_formula("=SOMME.SI(B:B, \"x\", E:E)", &english());
        assert!(issues.iter().any(
            |issue| issue.code == FormulaIssueCode::FunctionInOtherLanguage
                && issue.expected.as_deref() == Some("SUMIF")
        ));
        // The name both languages share is neither.
        assert!(codes("=INDEX(A:A; 2)", &french()).is_empty());
        assert!(codes("=INDEX(A:A, 2)", &english()).is_empty());
    }

    #[test]
    fn a_function_name_is_read_whatever_its_case_and_prefix() {
        assert!(codes("=somme(A1:A3)", &french()).is_empty());
        assert!(codes("=_xlfn.XLOOKUP(A1, B:B, C:C)", &english()).is_empty());
    }

    #[test]
    fn the_wrong_separator_is_reported_once_with_the_right_one() {
        let issues = check_formula("=SOMME.SI(B:B, \"x\", E:E)", &french());
        let separators: Vec<_> = issues
            .iter()
            .filter(|issue| issue.code == FormulaIssueCode::WrongSeparator)
            .collect();
        assert_eq!(separators.len(), 1);
        assert_eq!(separators[0].expected.as_deref(), Some(";"));
        assert_eq!(codes("=SUM(A1; A2)", &english()), ["wrong_separator"]);
        assert!(codes("=SOMME(A1; A2)", &french()).is_empty());
        assert!(codes("=SUM(A1, A2)", &english()).is_empty());
    }

    #[test]
    fn a_decimal_comma_and_a_comma_in_text_are_not_separators() {
        assert!(codes("=SI(A1>1,5; \"a, b\"; 2,25)", &french()).is_empty());
        assert!(codes("=SUM(A1:A3)*1.5", &english()).is_empty());
        // Outside any bracket a comma is the union operator, not an argument separator.
        assert!(codes("=A1,B1", &french()).is_empty());
    }

    #[test]
    fn brackets_and_quotes_must_balance() {
        assert_eq!(codes("=SOMME(A1:A3", &french()), ["unbalanced_parentheses"]);
        assert_eq!(
            codes("=SOMME(A1:A3))", &french()),
            ["unbalanced_parentheses"]
        );
        assert_eq!(
            codes("=SI(A1>0; \"oui; 2)", &french()),
            ["unbalanced_quotes"]
        );
        assert!(codes("=SI(A1>0; \"un \"\"mot\"\"\"; 2)", &french()).is_empty());
        assert!(codes("=SOMME((A1+A2)*2)", &french()).is_empty());
    }

    #[test]
    fn a_column_beyond_the_sheet_is_reported_with_the_last_one() {
        // `ventes 2025.xlsx` has sheets of 3 and 2 columns, the CSVs 5: the widest is 5 (E).
        let issues = check_formula("=SOMME(F2:F9)", &french());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, FormulaIssueCode::UnknownColumnLetter);
        assert_eq!(issues[0].found, "F");
        assert_eq!(issues[0].expected.as_deref(), Some("E"));
        assert!(codes("=SOMME(E2:E9)", &french()).is_empty());
        assert!(codes("=SOMME(D:D)", &french()).is_empty());
        let issues = check_formula("=SOMME($H$2:$H$9)", &french());
        assert_eq!(issues[0].found, "H");
    }

    #[test]
    fn a_column_is_checked_against_the_sheet_it_names() {
        let issues = check_formula("=SOMME(Janvier!D2:D9)", &french());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].found, "D");
        assert_eq!(issues[0].expected.as_deref(), Some("C"));
        assert_eq!(issues[0].place.as_deref(), Some("Janvier"));
        assert!(codes("=SOMME(Janvier!C2:C9)", &french()).is_empty());
        // Quoted, with a space and a doubled quote.
        assert!(codes("=SOMME('Data sheet'!B2:B9)", &french()).is_empty());
        assert_eq!(
            codes("=SOMME('Data sheet'!C2:C9)", &french()),
            ["unknown_column_letter"]
        );
        assert_eq!(
            codes("=SOMME('Data sheet!B2)", &french()),
            ["unbalanced_quotes"]
        );
    }

    #[test]
    fn a_header_name_in_brackets_must_be_a_column() {
        let issues = check_formula("=SOMME([Prix])", &french());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, FormulaIssueCode::UnknownColumnName);
        assert_eq!(issues[0].found, "Prix");
        assert!(codes("=SOMME([montant])", &french()).is_empty());
        assert!(codes("=SOMME([Fournisseur])", &french()).is_empty());
        // A structured reference is out of grammar and silent.
        assert!(codes("=SOMME(Tableau1[Prix])", &french()).is_empty());
        assert!(codes("=SOMME([@Prix])", &french()).is_empty());
    }

    #[test]
    fn an_unknown_sheet_is_reported() {
        let issues = check_formula("=SOMME(Fevrier!B2:B9)", &french());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, FormulaIssueCode::UnknownSheet);
        assert_eq!(issues[0].found, "Fevrier");
        assert!(codes("=SOMME(Janvier!B2:B9)", &french()).is_empty());
    }

    #[test]
    fn a_csv_is_a_sheet_named_like_its_file_and_a_file_name_is_not() {
        assert!(codes("=SOMME(reglements!D:D)", &french()).is_empty());
        let issues = check_formula("=SOMME(reglements.csv!D:D)", &french());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, FormulaIssueCode::FileUsedAsSheet);
        assert_eq!(issues[0].found, "reglements.csv");
        // A workbook with its own sheets: its name is not a sheet.
        assert_eq!(
            codes("=SOMME('ventes 2025'!C2:C9)", &french()),
            ["file_used_as_sheet"]
        );
    }

    #[test]
    fn a_workbook_that_is_not_among_the_cards_is_unknown() {
        let issues = check_formula("=SOMME(budget.xlsx!B2)", &french());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, FormulaIssueCode::UnknownWorkbook);
        assert_eq!(issues[0].found, "budget.xlsx");
    }

    #[test]
    fn a_reference_to_another_workbook_reaches_outside_the_file() {
        let found = codes("=SOMME([Budget.xlsx]Feuil1!B2:B9)", &french());
        assert_eq!(found, ["reaches_outside", "unknown_workbook"]);
        let known = codes("=SOMME([reglements.csv]reglements!B2:B9)", &french());
        assert_eq!(known, ["reaches_outside"]);
        assert!(codes("=SOMME(B2:B9)", &french())
            .iter()
            .all(|c| *c != "reaches_outside"));
    }

    #[test]
    fn a_function_that_calls_outside_the_file_is_reported_in_both_languages() {
        let found = codes("=WEBSERVICE(\"https://example.org/cours\")", &english());
        assert_eq!(found, ["reaches_outside"]);
        let found = codes("=SERVICEWEB(\"https://example.org/cours\")", &french());
        assert_eq!(found, ["reaches_outside"]);
        let found = codes("=WEBSERVICE(\"https://example.org\")", &french());
        assert_eq!(found, ["function_in_other_language", "reaches_outside"]);
        assert_eq!(
            codes("=RTD(\"srv\"; \"\"; \"x\")", &french()),
            ["reaches_outside"]
        );
        assert!(codes("=SOMME(A1:A3)", &french()).is_empty());
        assert_eq!(codes("=cmd|'/c calc'!A1", &french()), ["reaches_outside"]);
    }

    // --- the cases of the owner's tests -------------------------------------------------------------

    #[test]
    fn the_formulas_small_models_wrote_on_her_folders_are_caught() {
        // A function in neither language, files written as sheets, a range as criterion.
        let found = codes(
            "=SOMMEIFS(reglements.csv!D:D; reglements.csv!B:B; fournisseurs.csv!B:B)",
            &french(),
        );
        assert!(found.contains(&"unknown_function"));
        assert!(found.contains(&"file_used_as_sheet"));
        // A lookup in a reversed range on a file written as a sheet.
        let found = codes(
            "=SI(RECHERCHEV(B2; reglements!B:A; 2; FAUX); reglements!D2; \"\")",
            &french(),
        );
        assert!(found.is_empty(), "{found:?}");
        // `JOIN` is not a spreadsheet function.
        let found = codes(
            "=JOIN(fournisseurs.csv!B2:B4, reglements.csv!B2:B4)",
            &french(),
        );
        assert_eq!(
            found,
            [
                "unknown_function",
                "file_used_as_sheet",
                "wrong_separator",
                "file_used_as_sheet"
            ]
        );
        assert_eq!(
            codes("=JOIN(A2:B2, \" \")", &french()),
            ["unknown_function", "wrong_separator"]
        );
    }

    // --- silence -----------------------------------------------------------------------------------------

    #[test]
    fn what_the_program_cannot_know_stays_silent() {
        // Array constants, a name the formula defines, a defined name, numbers with exponents.
        assert!(codes("=SOMME({1;2;3})", &french()).is_empty());
        assert!(codes("=LET(x; A1*2; calcul(x))", &french()).is_empty());
        assert!(codes("=SOMME(TauxTVA)*1E3", &french()).is_empty());
        // No cards at all: names, separators and brackets are still read, columns are not.
        let bare = FormulaContext::new(EXCEL, "fr-FR", &[]).unwrap();
        assert!(check_formula("=SOMME(Z9:Z90; Autre!A1)", &bare).is_empty());
        assert_eq!(
            check_formula("=SOMMEIFS(A1)", &bare)[0].code,
            FormulaIssueCode::UnknownFunction
        );
    }

    #[test]
    fn a_whole_answer_counts_what_it_verified() {
        let answer = "Try:\n=SOMME(E:E)\nou:\n=SOMME.SI(B:B, \"x\", E:E)";
        let report = check_answer(answer, &french());
        assert_eq!(report.verified, 1);
        assert_eq!(report.notes.len(), 1);
        assert_eq!(report.notes[0].formula, "=SOMME.SI(B:B, \"x\", E:E)");
        assert!(!report.has_repairable());
        let report = check_answer("=SUMIF(B:B; \"x\"; E:E)", &french());
        assert!(report.has_repairable());
        assert_eq!(
            check_answer("no formula here", &french()),
            FormulaReport::default()
        );
    }
}
