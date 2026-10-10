//! The table extractor: what the columns of a workbook mean, and which entities their values name.
//!
//! It reads a workbook the Data Analyse has already parsed (headers, typed columns, the cells as
//! text), never the file, and returns **candidates** like the text extractor does: a surface, a type,
//! a role, a locator that points at a column and a few rows. Whether a value is an entity the base
//! already holds is the resolver's decision (`knowledge::ingest`); this file only reads.
//!
//! ```text
//! header words --- pack column entries ---> a type, a subtype, a role, a confidence
//!      |                                        |
//!      +--- value shape (a sample of cells) ----+--> keep, lower, or reject the column
//! distinct values of a kept column ---> candidates (one per value, never one per cell)
//! two role columns of one sheet     ---> a relation, once per distinct pair of values
//! ```
//!
//! **A module never changes a type** (`docs/DECISIONS.md`, "Knowledge packs are named
//! domain-country"). A header that only one module's vocabulary reads gives its type through that
//! module and through nothing else: with the module off the column is simply not classified, it is
//! never read as something else. Two checks make that true. A header is looked up in the active packs
//! *and* in the vocabulary of every pack the build ships, and if the two read it as different types
//! the column is left alone. And a header that merely contains a vocabulary word counts only when
//! its other words belong to the same kind of thing (`patient_name_full`: "name" and "full" are
//! words of person headers; `nom caisse`: "nom" is a word of person headers, but "caisse" is not, so
//! the person reading is refused and the organisation one wins or nothing does).
//!
//! What it never does: touch the engine's results, write a cell as itself when a pack marks the
//! scheme as a personal key (the builder hashes it), read a number or a date as a name, or trust a
//! formula's cached result.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use super::{
    Candidate, CandidateIdentifier, CandidateRelation, ColumnInput, ColumnKind, EntityExtractor,
    ExtractContext, ExtractedKnowledge, SheetInput, WorkbookInput,
};
use crate::error::AppError;
use crate::knowledge::normalize::{fold_word, normalize_name, TitleSet};
use crate::knowledge::packs::{
    available_pack_ids, bundled_packs, header_words, ColumnRule, IdentifierScheme, PackSet,
    BASE_PACK_ID,
};
use crate::knowledge::{Confidence, EntityTypeId, MentionLocator, Method, RoleId};

pub const EXTRACTOR_ID: &str = "deterministic-table";

/// Raised whenever this extractor's output for some table changes. Recorded beside
/// `CURRENT_KB_VERSION`, which is what actually makes the next pass read a workbook again.
pub const EXTRACTOR_VERSION: u32 = 1;

/// A column read with less confidence than this is not a column of entities.
pub const MIN_COLUMN_CONFIDENCE: f32 = crate::knowledge::ACCEPTED_COLUMN_CONFIDENCE;
/// How many of a column's cells are looked at to decide whether its values look like what the header
/// says.
pub const SAMPLE_SIZE: usize = 200;
/// The share of the sample that must look like names for a column read as a person.
pub const PERSON_SHAPE_SHARE: f32 = 0.70;
/// The share of the sample that must match an identifier scheme for an identifier column.
pub const IDENTIFIER_SHAPE_SHARE: f32 = 0.80;
/// The share of the sample that must be short text for any other column of entities.
pub const TEXT_SHAPE_SHARE: f32 = 0.70;
/// A header that holds a vocabulary entry plus other words of the same kind is read at this fraction
/// of the entry's confidence.
pub const CONTAINED_FACTOR: f32 = 0.90;
/// Two independent signals (a partial header and a strong share of matching values) raise an
/// identifier column to this.
pub const COMBINED_CONFIDENCE: f32 = 0.80;
/// The confidence of an identifier column no header explains but whose values all match a scheme.
pub const VALUE_SHAPE_CONFIDENCE: f32 = 0.65;
/// The share of values that must match a scheme for a column with no usable header.
pub const VALUE_SHAPE_SHARE: f32 = 0.90;
/// The fewest values a column with no usable header needs before its shape is trusted.
pub const VALUE_SHAPE_MIN_SAMPLE: usize = 5;

/// Distinct values read from one column. Beyond it the rest are not read and the source is reported
/// as truncated.
pub const MAX_DISTINCT_VALUES_PER_COLUMN: usize = 20_000;
/// Candidates kept for one workbook.
pub const MAX_CANDIDATES_PER_SOURCE: usize = 50_000;
/// Relations kept for one workbook.
pub const MAX_RELATIONS_PER_SOURCE: usize = 10_000;

/// A cell longer than this is a sentence, not a name.
const MAX_VALUE_CHARS: usize = 80;
/// A cell with more words than this is a sentence, not a name.
const MAX_VALUE_WORDS: usize = 6;
/// A person is at most a title, a first name, a particle and a surname.
const MAX_NAME_WORDS: usize = 4;
/// The most row numbers kept with a mention.
const SAMPLE_ROWS: usize = 3;

// ---------------------------------------------------------------------------------------------
// What the analyzer reports
// ---------------------------------------------------------------------------------------------

/// Why a column was read the way it was, or why it was not (`kb_column_semantics.reason`). The
/// first four are a column that was read; the others say why a column carries no entity, so that
/// "why was this column not read?" has an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticReason {
    /// The header is a vocabulary entry word for word.
    HeaderExact,
    /// The header holds a vocabulary entry and other words of the same kind.
    HeaderPartial,
    /// No header explains the column; its values all match an identifier scheme.
    ValueShape,
    /// A partial header and a strong share of matching values agree.
    Combined,
    /// No pack of any language has a word for this header (`salle`, `montant`).
    NoHeaderMatch,
    /// A module that is off would read this header (`patient` without a health module). Kept as a
    /// hint with no confidence, never as a reading.
    ModuleOff,
    /// Two modules or two languages read the header as different kinds of thing: left alone.
    AmbiguousHeader,
    /// The header means a name, an organisation, a place or an item, but the column holds numbers or
    /// dates.
    NumberOrDate,
    /// The column has a header and no usable value.
    NoValues,
    /// The header means something and the values do not look like it (codes under `Nom`, notes
    /// under `Patient`).
    ValuesMismatch,
}

impl SemanticReason {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::HeaderExact => "header_exact",
            Self::HeaderPartial => "header_partial",
            Self::ValueShape => "value_shape",
            Self::Combined => "combined",
            Self::NoHeaderMatch => "no_header_match",
            Self::ModuleOff => "module_off",
            Self::AmbiguousHeader => "ambiguous_header",
            Self::NumberOrDate => "number_or_date",
            Self::NoValues => "no_values",
            Self::ValuesMismatch => "values_mismatch",
        }
    }

    /// Whether this reason says a column was **not** read.
    pub fn is_rejection(self) -> bool {
        !matches!(
            self,
            Self::HeaderExact | Self::HeaderPartial | Self::ValueShape | Self::Combined
        )
    }
}

/// What one column of one sheet was read as, or why it was not read.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSemantics {
    pub sheet: String,
    /// The header as written.
    pub column: String,
    /// Position in the sheet, from 0.
    pub index: usize,
    /// `unknown` when the header means nothing to any pack. For a column that was not read, the type
    /// a header would have if its module were on is only a hint.
    pub entity_type: EntityTypeId,
    pub subtype: Option<String>,
    pub role: Option<RoleId>,
    pub confidence: Confidence,
    pub reason: SemanticReason,
}

impl ColumnSemantics {
    /// Whether the column's values are read as entities: a reading, with the confidence the
    /// acceptance line asks for.
    pub fn is_read(&self) -> bool {
        !self.reason.is_rejection() && self.confidence.value() >= MIN_COLUMN_CONFIDENCE
    }
}

/// A personal identifier found on the same row as a person: an attribute of that person, stated by
/// the table's own layout. The value is the identifier reduced by its scheme; the builder stores it
/// as a keyed hash, never as itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateAttribute {
    /// Index into [`ExtractedKnowledge::candidates`].
    pub candidate: usize,
    pub scheme: String,
    pub normalized: String,
}

/// Counts of what the reading saw, for the Analyse summary. Numbers only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableStats {
    pub sheets: usize,
    /// Sheets that have a header row, the only ones whose columns are classified.
    pub tables: usize,
    pub semantic_columns: usize,
    /// Cells that hold a formula: counted, never read.
    pub formula_cells: usize,
}

/// Everything one workbook gave.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableReading {
    pub knowledge: ExtractedKnowledge,
    pub columns: Vec<ColumnSemantics>,
    pub attributes: Vec<CandidateAttribute>,
    pub stats: TableStats,
}

// ---------------------------------------------------------------------------------------------
// The vocabulary every shipped pack contributes, for the "a module never changes a type" check
// ---------------------------------------------------------------------------------------------

/// The column vocabulary of **every** pack the build ships, in **every** language it ships,
/// whichever are active: what a header would mean if all modules were on. It is never used to
/// classify a column; it answers "would another module, or another language, read this header as a
/// different type?" and "which words belong to headers of this kind?".
pub struct Vocabulary {
    rules: Vec<ColumnRule>,
    /// The locale tags of the bundled packs (`fr-FR`, `en-US`).
    languages: Vec<String>,
    /// Per entity type, the words of its header entries. A header that contains a person entry may
    /// carry other words of person headers (`name`, `full`), never words of something else.
    words_by_type: BTreeMap<String, BTreeSet<String>>,
}

impl Vocabulary {
    /// The vocabulary of every pack this build ships, in every language.
    pub fn shipped() -> Result<Self, AppError> {
        let ids = available_pack_ids();
        let mut languages: Vec<String> = bundled_packs()
            .into_iter()
            .map(|(_, locale)| locale.to_string())
            .collect();
        languages.sort();
        languages.dedup();
        let mut rules = Vec::new();
        for language in &languages {
            rules.extend(
                PackSet::load(language, &ids)?
                    .column_rules()
                    .iter()
                    .cloned(),
            );
        }
        Ok(Self::from_rules(rules, languages))
    }

    pub fn from_rules(rules: Vec<ColumnRule>, languages: Vec<String>) -> Self {
        let mut words_by_type: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for rule in &rules {
            let words = words_by_type
                .entry(rule.entity_type.as_str().to_string())
                .or_default();
            for header in &rule.headers {
                words.extend(header.iter().cloned());
            }
        }
        Self {
            rules,
            languages,
            words_by_type,
        }
    }
}

/// A header read against a set of rules.
#[derive(Debug, Clone)]
struct Finding {
    entity_type: EntityTypeId,
    subtype: Option<String>,
    role: Option<RoleId>,
    confidence: f32,
    exact: bool,
}

/// What a header says against a set of rules.
enum HeaderReading {
    /// No entry reads it.
    Nothing,
    Found(Finding),
    /// Entries of different types read it equally well: not a reading.
    Conflict,
}

/// The best reading of a header: an exact entry first, otherwise the longest entry the header
/// contains whose other words are particles or words of headers of the same type. Entries of
/// different types at the same rank are a conflict, never a pick.
fn find_header(
    rules: &[ColumnRule],
    words: &[String],
    words_by_type: &BTreeMap<String, BTreeSet<String>>,
    titles: &TitleSet,
) -> HeaderReading {
    let mut exact: Vec<Finding> = Vec::new();
    for rule in rules {
        if rule.headers.iter().any(|entry| entry.as_slice() == words) {
            exact.push(Finding {
                entity_type: rule.entity_type.clone(),
                subtype: rule.subtype.clone(),
                role: rule.role.clone(),
                confidence: rule.confidence.value(),
                exact: true,
            });
        }
    }
    if !exact.is_empty() {
        return settle(exact);
    }

    let mut contained: Vec<(usize, Finding)> = Vec::new();
    for rule in rules {
        let allowed = words_by_type.get(rule.entity_type.as_str());
        for entry in &rule.headers {
            if entry.is_empty() || !entry.iter().all(|word| words.contains(word)) {
                continue;
            }
            let others_belong = words
                .iter()
                .filter(|word| !entry.contains(word))
                .all(|word| {
                    titles.is_particle(word) || allowed.is_some_and(|set| set.contains(word))
                });
            if others_belong {
                contained.push((
                    entry.len(),
                    Finding {
                        entity_type: rule.entity_type.clone(),
                        subtype: rule.subtype.clone(),
                        role: rule.role.clone(),
                        confidence: rule.confidence.value() * CONTAINED_FACTOR,
                        exact: false,
                    },
                ));
            }
        }
    }
    let Some(longest) = contained.iter().map(|(length, _)| *length).max() else {
        return HeaderReading::Nothing;
    };
    settle(
        contained
            .into_iter()
            .filter(|(length, _)| *length == longest)
            .map(|(_, finding)| finding)
            .collect(),
    )
}

/// One finding of the best rank, or a conflict when they are not all of one type. Among findings of
/// one type the most confident wins (a header two packs list at two confidences).
fn settle(findings: Vec<Finding>) -> HeaderReading {
    let first_type = findings[0].entity_type.clone();
    if findings.iter().any(|f| f.entity_type != first_type) {
        return HeaderReading::Conflict;
    }
    let best = findings
        .into_iter()
        .reduce(|best, next| {
            if next.confidence > best.confidence {
                next
            } else {
                best
            }
        })
        .expect("at least one finding");
    HeaderReading::Found(best)
}

// ---------------------------------------------------------------------------------------------
// What a value looks like
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Name,
    ShortText,
    Identifier,
}

fn shape_of(entity_type: &EntityTypeId) -> Shape {
    match entity_type.as_str() {
        EntityTypeId::PERSON => Shape::Name,
        EntityTypeId::IDENTIFIER => Shape::Identifier,
        _ => Shape::ShortText,
    }
}

/// Digits and the marks that sit between them: a quantity, a postal code, a date written as text.
fn is_pure_number(value: &str) -> bool {
    let mut digits = 0;
    for c in value.chars() {
        if c.is_ascii_digit() {
            digits += 1;
        } else if !matches!(
            c,
            '.' | ',' | ' ' | '-' | '+' | '%' | '/' | ':' | '\u{a0}' | '\u{20ac}' | '$'
        ) {
            return false;
        }
    }
    digits > 0
}

fn words_in(value: &str) -> usize {
    value.split_whitespace().count()
}

/// One entry of a column after the value was judged.
struct Entry {
    /// What two spellings of the same value share.
    key: String,
    surface: String,
    /// The titles that stood before a name (folded), so a title-and-surname alias can be made.
    titles: Vec<String>,
    identifier: Option<CandidateIdentifier>,
    personal: bool,
}

/// Judges the cells of columns against one active set of packs.
struct Reader<'a> {
    packs: &'a PackSet,
    titles: TitleSet,
    vocabulary: &'a Vocabulary,
    /// The column rules of the active modules, in every language.
    rules: Arc<Vec<ColumnRule>>,
}

impl Reader<'_> {
    /// "Nothing there": no letter or digit at all (`-`, `?`), or one of the pack's null-like words
    /// (`N/A`, `unknown`, `none`).
    fn is_null_like(&self, value: &str) -> bool {
        !value.chars().any(char::is_alphanumeric) || self.packs.is_stop_word(value)
    }

    fn has_org_marker(&self, value: &str) -> bool {
        let words = normalize_name(value, &TitleSet::new()).tokens;
        let markers = self.packs.org_markers();
        markers.prefix.iter().any(|marker| {
            !marker.is_empty() && words.len() > marker.len() && words.starts_with(marker)
        }) || markers.suffix.iter().any(|marker| {
            !marker.is_empty() && words.len() > marker.len() && words.ends_with(marker)
        })
    }

    /// One to four words made of letters, with a hyphen, an apostrophe, a period or one comma inside
    /// ("DUPONT, Jean"), and no organisation marker.
    fn looks_like_name(&self, value: &str) -> bool {
        if value.chars().filter(|c| *c == ',').count() > 1 {
            return false;
        }
        let words: Vec<&str> = value
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|word| !word.is_empty())
            .collect();
        if words.is_empty() || words.len() > MAX_NAME_WORDS {
            return false;
        }
        let only_name_characters = value.chars().all(|c| {
            c.is_alphabetic()
                || c.is_whitespace()
                || matches!(c, '-' | '\'' | '\u{2019}' | '.' | ',')
        });
        only_name_characters
            && words
                .iter()
                .any(|word| word.chars().filter(|c| c.is_alphabetic()).count() >= 2)
            && !self.has_org_marker(value)
    }

    /// A person's cell without the titles in front of the name: "Dr Jean Dupont" is Jean Dupont,
    /// who was addressed as a doctor. Returns the name as written and the titles, folded.
    fn without_titles(&self, value: &str) -> (String, Vec<String>) {
        let words: Vec<&str> = value.split_whitespace().collect();
        let mut titles = Vec::new();
        let mut start = 0;
        while start + 1 < words.len() {
            let folded = fold_word(words[start]);
            if self.titles.is_title(&folded) || self.titles.is_weak_title(&folded) {
                titles.push(folded);
                start += 1;
            } else {
                break;
            }
        }
        (words[start..].join(" "), titles)
    }

    fn is_short_text(&self, value: &str) -> bool {
        value.chars().count() <= MAX_VALUE_CHARS
            && words_in(value) <= MAX_VALUE_WORDS
            && value.chars().any(char::is_alphabetic)
    }

    /// The scheme that reads the whole cell as one identifier, schemes named like `preferred` first.
    fn identifier_in(
        &self,
        preferred: Option<&str>,
        value: &str,
    ) -> Option<(&IdentifierScheme, String)> {
        let schemes = self.packs.identifier_schemes();
        let wanted = |scheme: &&IdentifierScheme| {
            preferred.is_some_and(|name| scheme.id == name || scheme.kind == name)
        };
        let ordered = schemes
            .iter()
            .filter(wanted)
            .chain(schemes.iter().filter(|scheme| !wanted(scheme)));
        for scheme in ordered {
            for found in scheme.scan(value) {
                if found.range.start == 0 && found.range.end == value.len() {
                    return Some((scheme, found.normalized));
                }
            }
        }
        None
    }

    /// The entry a trimmed, non-empty cell stands for in a column of this shape, or `None` when the
    /// cell is not an entity (empty, null-like, a number, a sentence, not shaped like the column).
    fn judge(&self, shape: Shape, preferred: Option<&str>, value: &str) -> Option<Entry> {
        match shape {
            Shape::Identifier => {
                let (scheme, normalized) = self.identifier_in(preferred, value)?;
                Some(Entry {
                    key: format!("{}:{}", scheme.id, normalized),
                    surface: value.to_string(),
                    titles: Vec::new(),
                    personal: scheme.personal_key,
                    identifier: Some(CandidateIdentifier {
                        scheme: scheme.id.clone(),
                        normalized,
                    }),
                })
            }
            Shape::Name | Shape::ShortText => {
                if self.is_null_like(value) || is_pure_number(value) || !self.is_short_text(value) {
                    return None;
                }
                let (surface, titles) = if shape == Shape::Name {
                    self.without_titles(value)
                } else {
                    (
                        value.split_whitespace().collect::<Vec<_>>().join(" "),
                        Vec::new(),
                    )
                };
                if shape == Shape::Name && !self.looks_like_name(&surface) {
                    return None;
                }
                let key = normalize_name(&surface, &TitleSet::new()).joined;
                if key.is_empty() {
                    return None;
                }
                Some(Entry {
                    key,
                    surface,
                    titles,
                    identifier: None,
                    personal: false,
                })
            }
        }
    }

    /// The cells of a column that are worth looking at to decide what it holds.
    fn sample<'c>(&self, column: &'c ColumnInput) -> Vec<&'c str> {
        column
            .values
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty() && !self.is_null_like(value))
            .take(SAMPLE_SIZE)
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// The extractor
// ---------------------------------------------------------------------------------------------

/// Reads workbooks for the entities their columns hold. The packs of the pass come with each call
/// (`ExtractContext::packs`); the extractor itself holds the shipped vocabulary and, per set of
/// active modules, the column rules of those modules in the other languages.
pub struct TableEntityExtractor {
    vocabulary: Vocabulary,
    rules: Mutex<HashMap<String, Arc<Vec<ColumnRule>>>>,
}

impl TableEntityExtractor {
    pub fn new(vocabulary: Vocabulary) -> Self {
        Self {
            vocabulary,
            rules: Mutex::new(HashMap::new()),
        }
    }

    /// An extractor that knows the vocabulary of every pack this build ships, in every language. A
    /// header is read in the words of every language: an export from an English program is read in
    /// a French session, and the result is the same type either way.
    pub fn for_locale(_locale: &str) -> Result<Self, AppError> {
        Ok(Self::new(Vocabulary::shipped()?))
    }

    /// The column rules of the active modules, in the session's language (the set itself) and in
    /// every other language the build ships. Made once per set of modules.
    fn rules_for(&self, packs: &PackSet) -> Arc<Vec<ColumnRule>> {
        let ids: Vec<String> = packs
            .loaded()
            .iter()
            .map(|(id, _)| id.clone())
            .filter(|id| id != BASE_PACK_ID)
            .collect();
        let key = format!("{}|{}", packs.locale(), ids.join(","));
        let mut cache = self
            .rules
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(known) = cache.get(&key) {
            return Arc::clone(known);
        }
        let mut rules: Vec<ColumnRule> = packs.column_rules().to_vec();
        let wanted: Vec<&str> = ids.iter().map(String::as_str).collect();
        for language in &self.vocabulary.languages {
            if language != packs.locale() {
                if let Ok(other) = PackSet::load(language, &wanted) {
                    rules.extend(other.column_rules().iter().cloned());
                }
            }
        }
        let rules = Arc::new(rules);
        cache.insert(key, Arc::clone(&rules));
        rules
    }

    fn reader<'a>(&'a self, packs: &'a PackSet) -> Reader<'a> {
        Reader {
            packs,
            titles: packs.title_set(),
            vocabulary: &self.vocabulary,
            rules: self.rules_for(packs),
        }
    }

    /// How one column would be read, `None` when it is not read as entities. For tests.
    pub fn classify(
        &self,
        packs: &PackSet,
        sheet: &str,
        index: usize,
        column: &ColumnInput,
    ) -> Option<ColumnSemantics> {
        Some(self.classify_any(packs, sheet, index, column)).filter(ColumnSemantics::is_read)
    }

    /// How one column would be read, or why not: always an answer.
    pub fn classify_any(
        &self,
        packs: &PackSet,
        sheet: &str,
        index: usize,
        column: &ColumnInput,
    ) -> ColumnSemantics {
        classify_column(&self.reader(packs), sheet, index, column)
    }

    /// Everything a workbook gives: how its columns were read, the candidates its values name, the
    /// relations its layout states, the personal identifiers that sit beside a person.
    pub fn read(&self, input: &WorkbookInput, context: &ExtractContext<'_>) -> TableReading {
        let reader = self.reader(context.packs);
        let mut reading = Accumulator::default();
        for sheet in &input.sheets {
            read_sheet(&reader, sheet, &mut reading);
        }
        reading.finish()
    }
}

impl EntityExtractor for TableEntityExtractor {
    fn id(&self) -> &str {
        EXTRACTOR_ID
    }

    fn version(&self) -> u32 {
        EXTRACTOR_VERSION
    }

    fn extract_workbook(
        &self,
        input: &WorkbookInput,
        context: &ExtractContext<'_>,
    ) -> ExtractedKnowledge {
        self.read(input, context).knowledge
    }
}

// ---------------------------------------------------------------------------------------------
// Classifying a column
// ---------------------------------------------------------------------------------------------

fn classify_column(
    reader: &Reader<'_>,
    sheet: &str,
    index: usize,
    column: &ColumnInput,
) -> ColumnSemantics {
    // A column that was not read: what a hint says, if there is one, and why.
    let described =
        |finding: Option<&Finding>, confidence: f32, reason: SemanticReason| ColumnSemantics {
            sheet: sheet.to_string(),
            column: column.header.clone(),
            index,
            entity_type: finding
                .map(|found| found.entity_type.clone())
                .or_else(|| EntityTypeId::new("unknown"))
                .expect("a machine id"),
            subtype: finding.and_then(|found| found.subtype.clone()),
            role: finding.and_then(|found| found.role.clone()),
            confidence: Confidence::new(confidence),
            reason,
        };
    let words = header_words(&column.header);
    if words.is_empty() {
        return described(None, 0.0, SemanticReason::NoHeaderMatch);
    }
    let vocabulary = reader.vocabulary;
    let active = find_header(
        &reader.rules,
        &words,
        &vocabulary.words_by_type,
        &reader.titles,
    );
    let everything = find_header(
        &vocabulary.rules,
        &words,
        &vocabulary.words_by_type,
        &reader.titles,
    );

    // A module never changes a type. If the modules that are off read this header, there is nothing
    // to read (a hint is kept); if they read it as another type, or two languages disagree, the
    // column is left alone rather than read the way only the active modules see it.
    let finding = match (active, everything) {
        (HeaderReading::Conflict, _) | (_, HeaderReading::Conflict) => {
            return described(None, 0.0, SemanticReason::AmbiguousHeader);
        }
        (HeaderReading::Found(active), HeaderReading::Found(everything))
            if active.entity_type != everything.entity_type =>
        {
            return described(Some(&everything), 0.0, SemanticReason::AmbiguousHeader);
        }
        (HeaderReading::Nothing, HeaderReading::Found(everything)) => {
            return described(Some(&everything), 0.0, SemanticReason::ModuleOff);
        }
        (HeaderReading::Found(active), _) => Some(active),
        (HeaderReading::Nothing, HeaderReading::Nothing) => None,
    };

    let sample = reader.sample(column);
    let read = |finding: &Finding, confidence: f32, reason: SemanticReason| ColumnSemantics {
        sheet: sheet.to_string(),
        column: column.header.clone(),
        index,
        entity_type: finding.entity_type.clone(),
        subtype: finding.subtype.clone(),
        role: finding.role.clone(),
        confidence: Confidence::new(confidence),
        reason,
    };

    let Some(finding) = finding else {
        // No header explains it. Only a column whose values all match an identifier scheme is read.
        if sample.len() < VALUE_SHAPE_MIN_SAMPLE {
            return described(None, 0.0, SemanticReason::NoHeaderMatch);
        }
        let share = share_of(&sample, |value| reader.identifier_in(None, value).is_some());
        if share < VALUE_SHAPE_SHARE {
            return described(None, 0.0, SemanticReason::NoHeaderMatch);
        }
        let finding = Finding {
            entity_type: EntityTypeId::new(EntityTypeId::IDENTIFIER).expect("a machine id"),
            subtype: None,
            role: None,
            confidence: VALUE_SHAPE_CONFIDENCE,
            exact: false,
        };
        return read(&finding, VALUE_SHAPE_CONFIDENCE, SemanticReason::ValueShape);
    };

    let shape = shape_of(&finding.entity_type);
    // A number or a date is never a name; an identifier column may be numeric.
    if shape != Shape::Identifier && column.kind != ColumnKind::Text {
        return described(Some(&finding), 0.0, SemanticReason::NumberOrDate);
    }
    if sample.is_empty() {
        return described(Some(&finding), 0.0, SemanticReason::NoValues);
    }

    let preferred = finding.subtype.as_deref();
    let (share, wanted) = match shape {
        Shape::Identifier => (
            share_of(&sample, |value| {
                reader.identifier_in(preferred, value).is_some()
            }),
            IDENTIFIER_SHAPE_SHARE,
        ),
        Shape::Name => (
            share_of(&sample, |value| {
                reader.judge(Shape::Name, None, value).is_some()
            }),
            PERSON_SHAPE_SHARE,
        ),
        Shape::ShortText => (
            share_of(&sample, |value| {
                reader.judge(Shape::ShortText, None, value).is_some()
            }),
            TEXT_SHAPE_SHARE,
        ),
    };

    let mut confidence = finding.confidence;
    let mut reason = if finding.exact {
        SemanticReason::HeaderExact
    } else {
        SemanticReason::HeaderPartial
    };
    if share >= wanted {
        // Two independent signals agree: a header that only partly matched, and values that all
        // have the shape of the identifier it names.
        if shape == Shape::Identifier && !finding.exact && share >= VALUE_SHAPE_SHARE {
            confidence = confidence.max(COMBINED_CONFIDENCE);
            reason = SemanticReason::Combined;
        }
    } else {
        // The values do not look like what the header says: the further under, the less sure.
        confidence *= share / wanted;
    }
    if confidence < MIN_COLUMN_CONFIDENCE {
        return described(Some(&finding), confidence, SemanticReason::ValuesMismatch);
    }
    read(&finding, confidence, reason)
}

fn share_of(sample: &[&str], mut accepts: impl FnMut(&str) -> bool) -> f32 {
    if sample.is_empty() {
        return 0.0;
    }
    let accepted = sample.iter().filter(|value| accepts(value)).count();
    accepted as f32 / sample.len() as f32
}

// ---------------------------------------------------------------------------------------------
// Reading a sheet
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Accumulator {
    candidates: Vec<Candidate>,
    relations: BTreeMap<(usize, usize, String), (u32, f32)>,
    attributes: BTreeSet<(usize, String, String)>,
    columns: Vec<ColumnSemantics>,
    stats: TableStats,
    truncated: bool,
}

impl Accumulator {
    fn finish(self) -> TableReading {
        let relations = self
            .relations
            .into_iter()
            .map(
                |((subject, object, predicate), (occurrence_count, confidence))| {
                    CandidateRelation {
                        subject,
                        predicate,
                        object,
                        confidence: Confidence::new(confidence),
                        occurrence_count,
                    }
                },
            )
            .collect();
        TableReading {
            knowledge: ExtractedKnowledge {
                candidates: self.candidates,
                relations,
                signals: Vec::new(),
                truncated: self.truncated,
            },
            columns: self.columns,
            attributes: self
                .attributes
                .into_iter()
                .map(|(candidate, scheme, normalized)| CandidateAttribute {
                    candidate,
                    scheme,
                    normalized,
                })
                .collect(),
            stats: self.stats,
        }
    }
}

/// What a classified column produced, row by row: the candidate each row's cell stands for, and for
/// an identifier column the identifier it holds.
struct ColumnRows {
    semantics: ColumnSemantics,
    candidates: Vec<Option<usize>>,
    identifiers: Vec<Option<CandidateIdentifier>>,
    personal: Vec<bool>,
}

/// The candidates of one column, built from the entry of each row.
struct Interned {
    candidates: Vec<Option<usize>>,
    identifiers: Vec<Option<CandidateIdentifier>>,
    personal: Vec<bool>,
}

struct Seen {
    surface: String,
    titles: Vec<String>,
    identifier: Option<CandidateIdentifier>,
    count: u32,
    first_row: u32,
    sample_rows: Vec<u32>,
}

/// Turn the entries of a column into candidates: one per distinct value, with where it was first
/// seen and how often. Rows are numbered from 1 among the data rows.
fn intern_column(
    out: &mut Accumulator,
    semantics: &ColumnSemantics,
    locator_column: &str,
    entity_type: &EntityTypeId,
    subtype: Option<&str>,
    row_numbers: &[u32],
    entries: Vec<Option<Entry>>,
) -> Interned {
    let mut distinct: Vec<Seen> = Vec::new();
    let mut index_of: HashMap<String, usize> = HashMap::new();
    let mut local: Vec<Option<usize>> = Vec::with_capacity(entries.len());
    let mut identifiers: Vec<Option<CandidateIdentifier>> = Vec::with_capacity(entries.len());
    let mut personal: Vec<bool> = Vec::with_capacity(entries.len());

    for (position, entry) in entries.into_iter().enumerate() {
        // The row of the file, as the spreadsheet program numbers it; from 1 among the data rows
        // when the file's own numbers are not known.
        let row = row_numbers
            .get(position)
            .copied()
            .unwrap_or(position as u32 + 1);
        let Some(entry) = entry else {
            local.push(None);
            identifiers.push(None);
            personal.push(false);
            continue;
        };
        identifiers.push(entry.identifier.clone());
        personal.push(entry.personal);
        let slot = match index_of.get(&entry.key) {
            Some(slot) => *slot,
            None if distinct.len() >= MAX_DISTINCT_VALUES_PER_COLUMN => {
                out.truncated = true;
                local.push(None);
                continue;
            }
            None => {
                distinct.push(Seen {
                    surface: entry.surface,
                    titles: entry.titles,
                    identifier: entry.identifier,
                    count: 0,
                    first_row: row,
                    sample_rows: Vec::new(),
                });
                index_of.insert(entry.key, distinct.len() - 1);
                distinct.len() - 1
            }
        };
        let known = &mut distinct[slot];
        known.count += 1;
        if known.sample_rows.len() < SAMPLE_ROWS {
            known.sample_rows.push(row);
        }
        local.push(Some(slot));
    }

    let base = out.candidates.len();
    let room = MAX_CANDIDATES_PER_SOURCE.saturating_sub(base);
    let kept = distinct.len().min(room);
    if kept < distinct.len() {
        out.truncated = true;
    }
    for known in distinct.into_iter().take(kept) {
        let mut candidate = Candidate::new(
            &known.surface,
            entity_type.clone(),
            Method::ColumnValue,
            MentionLocator::Cells {
                sheet: semantics.sheet.clone(),
                column_name: locator_column.to_string(),
                first_row: Some(known.first_row),
                sample_rows: known.sample_rows,
            },
        );
        candidate.subtype = subtype.map(str::to_string);
        candidate.role = semantics.role.clone();
        candidate.confidence = semantics.confidence;
        candidate.occurrence_count = known.count;
        candidate.identifier = known.identifier;
        candidate.titles_seen = known.titles;
        out.candidates.push(candidate);
    }

    Interned {
        candidates: local
            .into_iter()
            .map(|slot| slot.filter(|slot| *slot < kept).map(|slot| base + slot))
            .collect(),
        identifiers,
        personal,
    }
}

fn read_sheet(reader: &Reader<'_>, sheet: &SheetInput, out: &mut Accumulator) {
    out.stats.sheets += 1;
    out.stats.formula_cells += sheet
        .columns
        .iter()
        .map(|column| column.formula_cells)
        .sum::<usize>();
    if !sheet.has_header {
        return;
    }
    out.stats.tables += 1;

    // Every column gets an answer: what it was read as, or why it was not.
    let answers: Vec<ColumnSemantics> = sheet
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| classify_column(reader, &sheet.name, index, column))
        .collect();
    out.columns.extend(answers.iter().cloned());
    let classified: Vec<(usize, ColumnSemantics)> = answers
        .into_iter()
        .filter(ColumnSemantics::is_read)
        .map(|semantics| (semantics.index, semantics))
        .collect();
    out.stats.semantic_columns += classified.len();

    // A family name and a given name in two columns are one person per row, not two kinds of
    // entity: the first of each is paired, and neither is read on its own.
    let family = classified
        .iter()
        .find(|(_, s)| is_person_part(s, "family_name"))
        .map(|(index, _)| *index);
    let given = classified
        .iter()
        .find(|(_, s)| is_person_part(s, "given_name"))
        .map(|(index, _)| *index);
    let paired = family.zip(given);

    let mut read: Vec<ColumnRows> = Vec::new();
    // Person columns that stand for one person per row, for attaching a row's identifier to it.
    let mut people: Vec<Vec<Option<usize>>> = Vec::new();

    if let Some((family_index, given_index)) = paired {
        let semantics_of = |index: usize| {
            classified
                .iter()
                .find(|(i, _)| *i == index)
                .map(|(_, s)| s.clone())
        };
        if let (Some(family_semantics), Some(given_semantics)) =
            (semantics_of(family_index), semantics_of(given_index))
        {
            let family_column = &sheet.columns[family_index];
            let given_column = &sheet.columns[given_index];
            let rows = family_column.values.len().max(given_column.values.len());
            let mut entries = Vec::with_capacity(rows);
            for row in 0..rows {
                let family_value = cell(family_column, row);
                let given_value = cell(given_column, row);
                let entry = if family_value.is_empty() || given_value.is_empty() {
                    None
                } else {
                    let whole = format!("{given_value} {family_value}");
                    reader.judge(Shape::Name, None, &whole).filter(|_| {
                        reader.judge(Shape::Name, None, family_value).is_some()
                            && reader.judge(Shape::Name, None, given_value).is_some()
                    })
                };
                entries.push(entry);
            }
            let mut combined = family_semantics.clone();
            combined.confidence = Confidence::new(
                family_semantics
                    .confidence
                    .value()
                    .min(given_semantics.confidence.value()),
            );
            let interned = intern_column(
                out,
                &combined,
                &family_column.header,
                &EntityTypeId::person(),
                None,
                &sheet.row_numbers,
                entries,
            );
            people.push(interned.candidates.clone());
            read.push(ColumnRows {
                semantics: combined,
                candidates: interned.candidates,
                identifiers: interned.identifiers,
                personal: interned.personal,
            });
        }
    }

    for (index, semantics) in &classified {
        if let Some((family_index, given_index)) = paired {
            if *index == family_index || *index == given_index {
                continue;
            }
        } else if is_person_part(semantics, "given_name") {
            // A first name alone is not a name worth remembering.
            continue;
        }
        let column = &sheet.columns[*index];
        let shape = shape_of(&semantics.entity_type);
        let preferred = semantics.subtype.as_deref();
        let entries: Vec<Option<Entry>> = judged_entries(reader, shape, preferred, column);
        // The subtype of a column of names says which part of a name it holds, which says nothing
        // of the entity: only the other types carry theirs through.
        let entity_subtype = if semantics.entity_type.as_str() == EntityTypeId::PERSON {
            None
        } else {
            semantics.subtype.as_deref()
        };
        let interned = intern_column(
            out,
            semantics,
            &column.header,
            &semantics.entity_type,
            entity_subtype,
            &sheet.row_numbers,
            entries,
        );
        if semantics.entity_type.as_str() == EntityTypeId::PERSON {
            people.push(interned.candidates.clone());
        }
        read.push(ColumnRows {
            semantics: semantics.clone(),
            candidates: interned.candidates,
            identifiers: interned.identifiers,
            personal: interned.personal,
        });
    }

    state_relations(reader, &read, out);
    attach_identifiers(&read, &people, out);
}

fn is_person_part(semantics: &ColumnSemantics, part: &str) -> bool {
    semantics.entity_type.as_str() == EntityTypeId::PERSON
        && semantics.subtype.as_deref() == Some(part)
}

/// The trimmed text of a cell, empty when the row has none.
fn cell(column: &ColumnInput, row: usize) -> &str {
    column
        .values
        .get(row)
        .map(|value| value.trim())
        .unwrap_or("")
}

/// One entry per row of the column. Each distinct text is judged once.
fn judged_entries(
    reader: &Reader<'_>,
    shape: Shape,
    preferred: Option<&str>,
    column: &ColumnInput,
) -> Vec<Option<Entry>> {
    let mut judged: HashMap<&str, Option<Entry>> = HashMap::new();
    let mut entries = Vec::with_capacity(column.values.len());
    for raw in &column.values {
        let value = raw.trim();
        if value.is_empty() {
            entries.push(None);
            continue;
        }
        let entry = judged
            .entry(value)
            .or_insert_with(|| reader.judge(shape, preferred, value));
        entries.push(entry.as_ref().map(|entry| Entry {
            key: entry.key.clone(),
            surface: entry.surface.clone(),
            titles: entry.titles.clone(),
            identifier: entry.identifier.clone(),
            personal: entry.personal,
        }));
    }
    entries
}

/// A relation between two role columns of one sheet, once per distinct pair of values, as the packs'
/// `relations` list names the role pairs. Nothing is inferred from two names merely being in the same
/// file.
fn state_relations(reader: &Reader<'_>, read: &[ColumnRows], out: &mut Accumulator) {
    for rule in reader.packs.relations() {
        for from in read {
            if from.semantics.role.as_ref() != Some(&rule.from_role) {
                continue;
            }
            for to in read {
                if to.semantics.role.as_ref() != Some(&rule.to_role)
                    || to.semantics.index == from.semantics.index
                {
                    continue;
                }
                let confidence = from
                    .semantics
                    .confidence
                    .value()
                    .min(to.semantics.confidence.value());
                for (subject, object) in from.candidates.iter().zip(&to.candidates) {
                    let (Some(subject), Some(object)) = (subject, object) else {
                        continue;
                    };
                    let key = (*subject, *object, rule.predicate.clone());
                    if !out.relations.contains_key(&key)
                        && out.relations.len() >= MAX_RELATIONS_PER_SOURCE
                    {
                        out.truncated = true;
                        continue;
                    }
                    let entry = out.relations.entry(key).or_insert((0, confidence));
                    entry.0 += 1;
                }
            }
        }
    }
}

/// A personal identifier (a pack scheme marked `personal_key`) on the same row as a person becomes an
/// attribute of that person. Only when the sheet has exactly one column of people: with two (a client
/// and a provider) the table does not say whose number it is.
fn attach_identifiers(read: &[ColumnRows], people: &[Vec<Option<usize>>], out: &mut Accumulator) {
    let [only] = people else {
        return;
    };
    for column in read {
        if column.semantics.entity_type.as_str() != EntityTypeId::IDENTIFIER {
            continue;
        }
        for (row, person) in only.iter().enumerate() {
            let Some(person) = person else {
                continue;
            };
            let (Some(Some(found)), Some(true)) =
                (column.identifiers.get(row), column.personal.get(row))
            else {
                continue;
            };
            out.attributes
                .insert((*person, found.scheme.clone(), found.normalized.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::packs::PackSet;

    const LOCALE: &str = "fr-FR";

    fn extractor() -> TableEntityExtractor {
        TableEntityExtractor::for_locale(LOCALE).expect("loads the shipped vocabulary")
    }

    fn names(count: usize) -> Vec<String> {
        const PEOPLE: [&str; 6] = [
            "Alice Archer",
            "Bruno Baker",
            "Carole Clark",
            "Denis Dumas",
            "Emma Evans",
            "Felix Fabre",
        ];
        (0..count)
            .map(|i| PEOPLE[i % PEOPLE.len()].to_string())
            .collect()
    }

    fn column(header: &str, values: Vec<String>) -> ColumnInput {
        ColumnInput {
            header: header.to_string(),
            values,
            kind: ColumnKind::Text,
            formula_cells: 0,
        }
    }

    fn read_as(packs: &[&str], header: &str, values: Vec<String>) -> Option<ColumnSemantics> {
        let packs = PackSet::load_with_type_vocabulary(LOCALE, packs).unwrap();
        extractor().classify(&packs, "Sheet", 0, &column(header, values))
    }

    #[test]
    fn a_header_is_read_in_snake_camel_and_spaced_forms_alike() {
        for header in [
            "Patient",
            "patient",
            "PATIENT",
            "nom_patient",
            "NomPatient",
            "nom patient",
        ] {
            let found = read_as(&["health-fr"], header, names(6)).unwrap_or_else(|| {
                panic!("{header} was not classified");
            });
            assert_eq!(found.entity_type.as_str(), "person", "{header}");
            assert_eq!(
                found.role.as_ref().map(RoleId::as_str),
                Some("client"),
                "{header}"
            );
        }
    }

    #[test]
    fn a_header_that_only_a_module_reads_is_left_alone_when_the_module_is_off() {
        assert!(read_as(&[], "patient", names(6)).is_none());
        assert!(read_as(&["legal-fr"], "patient", names(6)).is_none());
        assert!(read_as(&["health-fr"], "patient", names(6)).is_some());
    }

    #[test]
    fn words_that_do_not_belong_to_the_kind_do_not_make_a_match() {
        // "client" is a person word, "status" is not a word of any person header.
        assert!(read_as(&[], "statut client", names(6)).is_none());
        assert!(read_as(&[], "client", names(6)).is_some());
    }

    #[test]
    fn a_column_of_numbers_or_dates_is_never_a_column_of_names() {
        let mut numbers = column("client", vec!["1".into(), "2".into(), "3".into()]);
        numbers.kind = ColumnKind::Numeric;
        let packs = PackSet::load_with_type_vocabulary(LOCALE, &[]).unwrap();
        assert!(extractor().classify(&packs, "S", 0, &numbers).is_none());
    }

    #[test]
    fn values_that_do_not_look_like_names_lower_and_reject_a_person_column() {
        let notes: Vec<String> = (0..10)
            .map(|i| format!("routine check number {i} after the morning visit, nothing to report"))
            .collect();
        assert!(read_as(&["health-fr"], "patient", notes).is_none());
        let codes: Vec<String> = (0..10).map(|i| format!("A{i}7")).collect();
        assert!(read_as(&[], "nom", codes).is_none());
    }

    #[test]
    fn the_rows_of_a_value_are_counted_from_one_and_kept_to_three() {
        let packs = PackSet::load_with_type_vocabulary(LOCALE, &["health-fr"]).unwrap();
        let sheet = SheetInput {
            name: "S".to_string(),
            has_header: true,
            row_numbers: Vec::new(),
            columns: vec![column(
                "Patient",
                vec![
                    "Alice Archer".into(),
                    "Bruno Baker".into(),
                    "Alice Archer".into(),
                    "".into(),
                    "Alice Archer".into(),
                    "Alice Archer".into(),
                ],
            )],
        };
        let reading = extractor().read(
            &WorkbookInput {
                source: crate::knowledge::SourceRef {
                    domain: crate::knowledge::Domain::Data,
                    relative_path: "a.csv".into(),
                    content_id: "x".into(),
                },
                sheets: vec![sheet],
            },
            &ExtractContext {
                packs: &packs,
                locale: LOCALE,
                gazetteer: &crate::knowledge::gazetteer::Gazetteer::new(),
                encoder: &crate::knowledge::phonetic::FrenchPhonetic,
            },
        );
        let alice = &reading.knowledge.candidates[0];
        assert_eq!(alice.occurrence_count, 4);
        assert_eq!(
            alice.locator,
            MentionLocator::Cells {
                sheet: "S".into(),
                column_name: "Patient".into(),
                first_row: Some(1),
                sample_rows: vec![1, 3, 5],
            }
        );
    }
}
