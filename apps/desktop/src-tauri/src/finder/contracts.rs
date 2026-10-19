//! What the finder takes and gives (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 6). Data and machine codes only:
//! no sentence and no chat Markdown, so the left panel, the chat and an entity chip all render the same
//! result in their own way and the interface localises every code.

use serde::{Deserialize, Serialize};

use crate::discovery::DiscoveredFile;
use crate::file_profile::build::Domain;
use crate::file_record::FileRecord;

/// The most files a page of results carries when the caller does not say.
pub const DEFAULT_LIMIT: usize = 50;
/// A ceiling against a degenerate request; the total is always stated, whatever the page.
pub const MAX_LIMIT: usize = 100_000;

/// A file the folder holds, as the file system knows it. The finder is given this list and never walks
/// the folder itself, so a test or a cache can supply it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryFile {
    /// Relative to the folder, forward slashes.
    pub relative_path: String,
    /// Lower case, without the dot.
    pub extension: String,
}

impl From<&DiscoveredFile> for InventoryFile {
    fn from(file: &DiscoveredFile) -> Self {
        Self {
            relative_path: file.relative_path.clone(),
            extension: file.extension.clone(),
        }
    }
}

impl From<&FileRecord> for InventoryFile {
    fn from(file: &FileRecord) -> Self {
        Self {
            relative_path: file.relative_path.clone(),
            extension: file.extension.clone(),
        }
    }
}

/// One entity to narrow by. With a `role` only the profile is read, which holds roles with their evidence;
/// without one the knowledge base's mentions count too.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityFilter {
    pub entity_id: i64,
    #[serde(default)]
    pub role: Option<String>,
}

/// A range of dates, each end `YYYY`, `YYYY-MM` or `YYYY-MM-DD`. An end written as a month or a year
/// includes the whole month or year.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DateRange {
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FinderQuery {
    /// Words typed by the user: matched as words (the last one may be a start), never as meaning.
    pub text: Option<String>,
    /// The typed words are a finished phrase (a sentence), not a field being typed: the last word is then
    /// a whole word and never the start of one.
    pub whole_words: bool,
    /// Identifiers in their compact form (lower case, no separator), searched in the profile's stored
    /// forms so every spelling of one number meets. The caller reads them from what the user typed with
    /// the active packs (`finder::set_search::identifier_forms`).
    pub identifiers: Vec<String>,
    pub entities: Vec<EntityFilter>,
    /// Business type codes.
    pub kinds: Vec<String>,
    pub date: Option<DateRange>,
    /// A day, month or year typed in the left panel's field: the files dated so (by their profile or by
    /// the date their name starts with) are found **in addition to** the files the typed words find, and
    /// the passages are not searched for it. Set by the `find_files` command from what was typed
    /// (`finder::typed`), never by the webview.
    #[serde(skip)]
    pub typed_date: Option<DateRange>,
    /// Folders, relative to the work folder: every file under one of them.
    pub folders: Vec<String>,
    /// Extensions, lower case.
    pub formats: Vec<String>,
    /// State codes: `analysed`, `empty`, `no_text`, `cells_missing`, `unanalysed`, `unsupported`.
    pub states: Vec<String>,
    pub limit: Option<usize>,
    pub offset: usize,
    /// The caller needs the files and their state, not their cards or excerpts (the left panel selects
    /// the files found): nothing is read for the page beyond the index's one row per file.
    pub paths_only: bool,
}

/// Where to look. The finder never changes the selection: a result is only a list.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", content = "paths", rename_all = "camelCase")]
pub enum FinderScope {
    WholeFolder,
    Selection(Vec<String>),
}

/// Why a file was found. `kind` is `word` (a typed word met a text of the file; `detail` says which:
/// `name`, `title`, `author`, `subject`, `entities`, `headers`, `cells`, `identifiers`, `profile` when the
/// words are spread over several texts, `passage`), `identifier` (an identifier typed in any spelling met
/// the same number stored in the file), `date` (a date typed in the field met the file's date; `detail`
/// is `profile` or `name`) or `entity` (`detail` is the role, or `mention`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reason {
    pub kind: String,
    pub detail: Option<String>,
}

/// A person or an organisation linked to a file. `entity_id` is `None` for a surface the base does not
/// know: it is shown as possible and is never a filter.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardEntity {
    pub entity_id: Option<i64>,
    pub surface: String,
    pub role: String,
    pub confidence: f64,
}

/// What the panel shows of a file besides its name.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCard {
    pub title: Option<String>,
    pub business_type: Option<String>,
    pub document_date: Option<String>,
    pub format: String,
    pub state: String,
    pub entities: Vec<CardEntity>,
}

/// Where in a file the shown passage stands. `section` is empty until the structure of long documents is
/// indexed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Locator {
    pub page: Option<i64>,
    /// The passage's rank inside its page (what this field used to be called "section").
    pub ordinal: Option<i64>,
    /// The title of the section of the document the passage starts in, when the document has a
    /// structure and the section has a title (a slice of pages has none).
    pub section_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHit {
    pub domain: Domain,
    pub relative_path: String,
    pub card: FileCard,
    pub reasons: Vec<Reason>,
    /// The passage that matched, for the files of the page only.
    pub excerpt: Option<String>,
    pub locator: Option<Locator>,
    /// Every place where the words matched in this file, in reading order, at most `MAX_LOCATORS`: a long
    /// document is one hit with all its places, not one hit per passage. For the files of the page only.
    pub locators: Vec<Locator>,
    /// How many passages of the file matched, counting those beyond the places kept.
    pub locators_total: usize,
}

/// The most places kept for one file.
pub const MAX_LOCATORS: usize = 20;

/// One value of a filter control and how many files of the result carry it. `label` is the entity's
/// canonical name for the entity facet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Facet {
    pub value: String,
    pub count: usize,
    pub label: Option<String>,
}

/// Counts over the whole result, not over the page, so a filter control can say `courriers (41)` with no
/// second query.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Facets {
    pub by_type: Vec<Facet>,
    pub by_year: Vec<Facet>,
    pub by_entity: Vec<Facet>,
    pub by_folder: Vec<Facet>,
    pub by_format: Vec<Facet>,
    pub by_state: Vec<Facet>,
}

/// What the search could and could not look into, over the files of the scope (before the filters).
/// `searched` files were analysed; `unreadable` ones hold no text (empty, no text recovered, workbook not
/// at rest) or are a format the product does not read; `unanalysed` ones are readable formats the Analyse
/// has not processed yet, findable by name only; `partial` is the part of `searched` whose words were cut
/// at a cap.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    pub searched: usize,
    pub unreadable: usize,
    pub unanalysed: usize,
    pub partial: usize,
}

/// The words a search read, so the interface can say that it is a word search: a synonym or another form
/// of a word is not matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSearch {
    pub terms: Vec<String>,
    /// Whether the last word was also read as the start of a word.
    pub last_is_prefix: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinderResult {
    pub domain: Domain,
    /// Every file that answers, whatever the page.
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub files: Vec<FileHit>,
    pub facets: Facets,
    pub coverage: Coverage,
    pub text_search: Option<TextSearch>,
}

/// One file of a set answer, with the passage that matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentListing {
    pub relative_path: String,
    pub title: Option<String>,
    pub business_type: Option<String>,
    pub excerpt: Option<String>,
    pub page: Option<i64>,
    /// The pages where the words matched, ascending, without repeats (a long document has several).
    pub pages: Vec<i64>,
    /// How many passages of the file matched.
    pub matches_total: usize,
}
