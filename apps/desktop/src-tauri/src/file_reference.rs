//! Turning what a person calls a file into the file itself.
//!
//! The resolver reads the `WorkFolderInventory` and nothing else. It never touches the
//! filesystem, so a reference like `../../secret.pdf` has nowhere to go: there is no path to
//! escape from, only a list of records to fail to match. A file name is data, never an
//! instruction (`docs/WORK-FOLDER-INVENTORY.md`).
//!
//! Three outcomes, and the middle one matters most. When several files match, the resolver says
//! so and hands back the candidates; it never breaks the tie itself. Opening the wrong
//! `neurologie.pdf` because it sorted first is the kind of quiet mistake this product exists to
//! avoid.

use serde::Serialize;

use crate::file_record::{split_name, FileRecord};
use crate::inventory::WorkFolderInventory;

/// Shorter tokens are articles, prepositions and noise in French and in English alike. The same
/// cutoff the lexical index uses, for the same reason.
const MIN_TOKEN_CHARS: usize = 4;

/// How much of a name, beside its extension, a shortened reference has to carry before the end of
/// it is matched against the folder.
const MIN_FRAGMENT_STEM_CHARS: usize = 3;
/// The longest extension a file name carries here (`docx`, `xlsx`, `pptx`, `html`, `jpeg`).
const MAX_EXTENSION_CHARS: usize = 5;
/// A multi-word file name shorter than this is too easily a phrase in an ordinary sentence.
const MIN_PHRASE_CHARS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceStatus {
    Exact,
    MultipleMatches,
    NoMatch,
}

/// Why the resolver answered as it did. A machine code: the interface writes the sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionReason {
    /// Matched the canonical relative path.
    RelativePath,
    /// Matched one file name.
    FileName,
    /// Matched one file name once case was ignored.
    FileNameCaseInsensitive,
    /// Matched one name with the extension left off.
    Stem,
    /// Matched the end of one file name: a reference that dropped a prefix, most often a date.
    FileNameSuffix,
    /// A word in the reference is one file's name without its extension.
    StemToken,
    /// The reference is the end of one file's path, from a folder name on (`mars/neurologie.pdf`).
    FolderSuffix,
    /// Several files carry the reference. The caller must ask which.
    Ambiguous,
    /// Nothing in the Work Folder carries it.
    NotFound,
    /// The reference tried to leave the Work Folder, or was not a path at all. Refused before
    /// any lookup, and without touching the filesystem.
    OutsideWorkFolder,
}

/// What a reference resolved to. `exact_match` is present only for `Exact`; `candidates` carries
/// every record worth showing for `MultipleMatches`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileReferenceResolution {
    /// The reference as it was given, kept so the interface can quote it back.
    pub query: String,
    pub status: ReferenceStatus,
    pub exact_match: Option<FileRecord>,
    pub candidates: Vec<FileRecord>,
    pub reason: ResolutionReason,
}

impl FileReferenceResolution {
    fn exact(query: &str, record: &FileRecord, reason: ResolutionReason) -> Self {
        Self {
            query: query.to_string(),
            status: ReferenceStatus::Exact,
            exact_match: Some(record.clone()),
            candidates: vec![record.clone()],
            reason,
        }
    }

    fn several(query: &str, records: Vec<&FileRecord>) -> Self {
        Self {
            query: query.to_string(),
            status: ReferenceStatus::MultipleMatches,
            exact_match: None,
            candidates: records.into_iter().cloned().collect(),
            reason: ResolutionReason::Ambiguous,
        }
    }

    fn none(query: &str, reason: ResolutionReason) -> Self {
        Self {
            query: query.to_string(),
            status: ReferenceStatus::NoMatch,
            exact_match: None,
            candidates: Vec::new(),
            reason,
        }
    }
}

/// One reference found in a question by `resolve_all_in_question`, with where it was written.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionedResolution {
    /// How many of the question's words (folded: case, accents and punctuation set aside) come
    /// before the reference. Only the order is meant: two references never share a position.
    pub position: usize,
    pub resolution: FileReferenceResolution,
}

/// A reference and the run of the question's words it was read from.
struct Hit {
    start: usize,
    end: usize,
    resolution: FileReferenceResolution,
}

impl Hit {
    /// Whether this run holds the other one, which is then a piece of it.
    fn covers(&self, other: &Hit) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

impl FileReferenceResolution {
    /// What makes two resolutions the same reference: the file for an exact one, the set of
    /// candidates for an ambiguous one, the folded query for one that matched nothing.
    fn identity(&self) -> String {
        match (&self.status, &self.exact_match) {
            (ReferenceStatus::Exact, Some(record)) => format!("file:{}", record.relative_path),
            (ReferenceStatus::MultipleMatches, _) => {
                let mut paths: Vec<&str> = self
                    .candidates
                    .iter()
                    .map(|record| record.relative_path.as_str())
                    .collect();
                paths.sort_unstable();
                format!("several:{}", paths.join("|"))
            }
            _ => format!("none:{}", fold_words(&self.query)),
        }
    }
}

/// Resolves references against one inventory snapshot.
///
/// Deterministic metadata matching only, in a fixed order: canonical path, file name, stem. That
/// is all this sprint needs and all it does. A semantic step - "the March biology report" with no
/// name in it - would slot in behind this same interface and return the same
/// `FileReferenceResolution`, which is why the type exists rather than a bare function.
pub struct FileReferenceResolver<'a> {
    inventory: &'a WorkFolderInventory,
}

impl<'a> FileReferenceResolver<'a> {
    pub fn new(inventory: &'a WorkFolderInventory) -> Self {
        Self { inventory }
    }

    /// Resolve one reference: a relative path, a file name, or a name without its extension.
    pub fn resolve(&self, query: &str) -> FileReferenceResolution {
        let trimmed = query.trim().trim_matches(|c| c == '"' || c == '\'');
        if trimmed.is_empty() {
            return FileReferenceResolution::none(query, ResolutionReason::NotFound);
        }
        let normalised = trimmed.replace('\\', "/");
        if escapes_work_folder(&normalised) {
            return FileReferenceResolution::none(query, ResolutionReason::OutsideWorkFolder);
        }
        let normalised = normalised.trim_start_matches("./").to_string();

        // 1. The canonical relative path, exactly as the inventory stores it.
        if let Some(record) = self.inventory.find_by_relative_path(&normalised) {
            return FileReferenceResolution::exact(query, record, ResolutionReason::RelativePath);
        }
        if normalised.contains('/') {
            let folded = self.case_insensitive_paths(&normalised);
            if folded.is_empty() {
                // A person names a file by its last folder and its name (`mars/neurologie.pdf`), not by
                // the whole path. The reference must end a path at a folder boundary, so `ars/x.pdf`
                // reaches nothing and `janvier/x.pdf` never reaches the file under `mars/`.
                let by_folder = self.paths_ending_with(&normalised);
                return match by_folder.len() {
                    1 => FileReferenceResolution::exact(
                        query,
                        by_folder[0],
                        ResolutionReason::FolderSuffix,
                    ),
                    0 => FileReferenceResolution::none(query, ResolutionReason::NotFound),
                    _ => FileReferenceResolution::several(query, by_folder),
                };
            }
            return match folded.len() {
                1 => {
                    FileReferenceResolution::exact(query, folded[0], ResolutionReason::RelativePath)
                }
                _ => FileReferenceResolution::several(query, folded),
            };
        }

        // 2. The file name on its own.
        let by_name = self.inventory.find_by_name(&normalised);
        match by_name.len() {
            1 => {
                return FileReferenceResolution::exact(
                    query,
                    by_name[0],
                    ResolutionReason::FileName,
                )
            }
            0 => {}
            _ => return FileReferenceResolution::several(query, by_name),
        }
        let folded_names = self.case_insensitive_names(&normalised);
        match folded_names.len() {
            1 => {
                return FileReferenceResolution::exact(
                    query,
                    folded_names[0],
                    ResolutionReason::FileNameCaseInsensitive,
                )
            }
            0 => {}
            _ => return FileReferenceResolution::several(query, folded_names),
        }

        // 3. The name with its extension left off. Only when the reference itself carries no
        //    extension: `report.pdf` must never quietly resolve to `report.txt`.
        let (_, extension) = split_name(&normalised);
        if extension.is_empty() {
            let by_stem = self.case_insensitive_stems(&normalised);
            match by_stem.len() {
                1 => {
                    return FileReferenceResolution::exact(
                        query,
                        by_stem[0],
                        ResolutionReason::Stem,
                    )
                }
                0 => {}
                _ => return FileReferenceResolution::several(query, by_stem),
            }
        }

        // 4. The end of a name. People shorten a file name far more often than they lengthen it,
        //    and a date prefix is the first thing to go: `12_compte-rendu-biologie.pdf` for
        //    `2026-03-12_compte-rendu-biologie.pdf`. Matching the tail is what makes that work,
        //    and it changes nothing about the guarantee that matters - several tails matching
        //    is an ambiguity to report, never a tie to break.
        if let Some(fragment) = usable_fragment(&normalised) {
            let by_tail = self.names_ending_with(&fragment);
            match by_tail.len() {
                1 => {
                    return FileReferenceResolution::exact(
                        query,
                        by_tail[0],
                        ResolutionReason::FileNameSuffix,
                    )
                }
                0 => {}
                _ => return FileReferenceResolution::several(query, by_tail),
            }
        }

        FileReferenceResolution::none(query, ResolutionReason::NotFound)
    }

    /// Find the file a question names, if it names one at all.
    ///
    /// Conservative on purpose. A question that mentions no file returns `None`, and the caller
    /// keeps its ordinary behaviour - searching the whole corpus - untouched. Only three things
    /// count as naming a file: something shaped like a path, something shaped like a file name
    /// with an extension, and a word that is exactly one file's name without its extension.
    /// Anything looser would let an ordinary sentence silently narrow retrieval to one document.
    pub fn resolve_in_question(&self, question: &str) -> Option<FileReferenceResolution> {
        for token in path_like_tokens(question) {
            let resolution = self.resolve(&token);
            if resolution.status != ReferenceStatus::NoMatch
                || resolution.reason == ResolutionReason::OutsideWorkFolder
            {
                return Some(resolution);
            }
            // A token that looks like a file name but matches nothing is still a reference: the
            // user named a file, and being told it does not exist beats an answer assembled from
            // whatever else the folder happens to contain.
            if looks_like_a_file_name(&token) {
                return Some(resolution);
            }
        }

        for token in word_tokens(question) {
            let matches = self.case_insensitive_stems(&token);
            match matches.len() {
                1 => {
                    return Some(FileReferenceResolution::exact(
                        &token,
                        matches[0],
                        ResolutionReason::StemToken,
                    ))
                }
                0 => {}
                _ => return Some(FileReferenceResolution::several(&token, matches)),
            }
        }

        // A file whose name is several words ("ordonnance-pour-esaie") is written by a person as
        // several words. Matched only as a whole run of words, folded the same way, and only for
        // names long enough and multi-word enough that a chance match is not a concern; the
        // longest name wins, so naming the longer of two overlapping files names that one.
        let question_words = format!("-{}-", fold_words(question));
        let mut phrase_matches: Vec<(usize, &'a FileRecord)> = self
            .inventory
            .all_files()
            .iter()
            .filter_map(|file| {
                let words = fold_words(&file.stem);
                let is_phrase = words.contains('-') && words.chars().count() >= MIN_PHRASE_CHARS;
                (is_phrase && question_words.contains(&format!("-{words}-")))
                    .then(|| (words.chars().count(), file))
            })
            .collect();
        let longest = phrase_matches.iter().map(|(length, _)| *length).max()?;
        phrase_matches.retain(|(length, _)| *length == longest);
        match phrase_matches.len() {
            1 => Some(FileReferenceResolution::exact(
                &phrase_matches[0].1.stem,
                phrase_matches[0].1,
                ResolutionReason::StemToken,
            )),
            _ => Some(FileReferenceResolution::several(
                &phrase_matches[0].1.stem,
                phrase_matches.into_iter().map(|(_, file)| file).collect(),
            )),
        }
    }

    /// Every file a question names, in the order the question writes them.
    ///
    /// `resolve_in_question` stops at the first hit, which is right for a document search and wrong
    /// for a question that crosses two workbooks. This keeps its three routes (a token shaped like
    /// a path or a file name, a word equal to a stem, a multi-word stem) and adds a fourth that
    /// reads a name **as a whole run of words**, because a name that holds spaces
    /// (`Ventes 2025 (final).xlsx`, `Mes ventes.xlsx`) is split by the first route and reached only
    /// by its last token: `ventes 2025.xlsx` came back ambiguous with every other file ending in
    /// `2025.xlsx`. The whole-name route matches, folded like every other route (case, accents,
    /// punctuation):
    ///
    /// - the full name with its extension, whatever the length of the name;
    /// - the name without its extension when it holds several words and at least
    ///   `MIN_PHRASE_CHARS` characters, as before;
    /// - the name without its extension, whatever its length, when it is quoted.
    ///
    /// The longest run wins: a token that lies inside a longer name that matched is that name's
    /// token, not a second reference. The same file named twice is returned once, at its first
    /// position. The existing function is unchanged.
    pub fn resolve_all_in_question(&self, question: &str) -> Vec<PositionedResolution> {
        let folded_question = format!("-{}-", fold_words(question));
        let word_start = |byte_offset: usize| {
            fold_words(&question[..byte_offset])
                .split('-')
                .filter(|word| !word.is_empty())
                .count()
        };
        let word_count = |text: &str| {
            fold_words(text)
                .split('-')
                .filter(|word| !word.is_empty())
                .count()
        };

        let mut whole_names = self.whole_name_hits(question, &folded_question);
        whole_names.sort_by_key(|hit| std::cmp::Reverse(hit.end - hit.start));
        let mut kept_names: Vec<Hit> = Vec::new();
        for hit in whole_names {
            if !kept_names.iter().any(|kept| kept.covers(&hit)) {
                kept_names.push(hit);
            }
        }

        let mut hits: Vec<Hit> = Vec::new();
        for (offset, token) in tokens_with_offsets(question) {
            let trimmed = token.trim_matches(is_reference_edge);
            if trimmed.is_empty() || !(trimmed.contains(['/', '\\', '.'])) {
                continue;
            }
            let lead = token.len() - token.trim_start_matches(is_reference_edge).len();
            let resolution = self.resolve(trimmed);
            if resolution.status == ReferenceStatus::NoMatch
                && resolution.reason != ResolutionReason::OutsideWorkFolder
                && !looks_like_a_file_name(trimmed)
            {
                continue;
            }
            let start = word_start(offset + lead);
            hits.push(Hit {
                start,
                end: start + word_count(trimmed).max(1),
                resolution,
            });
        }
        for (offset, token) in word_spans(question) {
            let matches = self.case_insensitive_stems(token);
            let resolution = match matches.len() {
                0 => continue,
                1 => FileReferenceResolution::exact(token, matches[0], ResolutionReason::StemToken),
                _ => FileReferenceResolution::several(token, matches),
            };
            let start = word_start(offset);
            hits.push(Hit {
                start,
                end: start + word_count(token).max(1),
                resolution,
            });
        }
        // A token that is a piece of a longer name that matched is not a reference of its own.
        hits.retain(|hit| !kept_names.iter().any(|name| name.covers(hit)));
        hits.extend(kept_names);

        hits.sort_by_key(|hit| (hit.start, std::cmp::Reverse(hit.end)));
        let mut seen: Vec<String> = Vec::new();
        let mut out: Vec<PositionedResolution> = Vec::new();
        for hit in hits {
            let identity = hit.resolution.identity();
            if seen.contains(&identity) {
                continue;
            }
            seen.push(identity);
            out.push(PositionedResolution {
                position: hit.start,
                resolution: hit.resolution,
            });
        }
        out
    }

    /// The files whose whole name is written in the question, as runs of folded words (see
    /// `resolve_all_in_question`). Files that carry the same name are one hit, and ambiguous.
    fn whole_name_hits(&self, question: &str, folded_question: &str) -> Vec<Hit> {
        let quoted = quoted_phrases(question);
        let mut by_phrase: Vec<(String, ResolutionReason, Vec<&'a FileRecord>)> = Vec::new();
        for file in self.inventory.all_files() {
            let stem = fold_words(&file.stem);
            if stem.is_empty() {
                continue;
            }
            let multi_word = stem.contains('-');
            let mut phrases: Vec<(String, ResolutionReason)> = Vec::new();
            if multi_word {
                phrases.push((fold_words(&file.name), ResolutionReason::FileName));
                if stem.chars().count() >= MIN_PHRASE_CHARS || quoted.contains(&stem) {
                    phrases.push((stem.clone(), ResolutionReason::StemToken));
                }
            } else if quoted.contains(&stem) {
                phrases.push((stem.clone(), ResolutionReason::StemToken));
            }
            for (phrase, reason) in phrases {
                if !folded_question.contains(&format!("-{phrase}-")) {
                    continue;
                }
                match by_phrase.iter_mut().find(|(known, _, _)| *known == phrase) {
                    Some((_, _, files)) => files.push(file),
                    None => by_phrase.push((phrase, reason, vec![file])),
                }
            }
        }
        // Every place the name is written, so a second mention is not read word by word.
        by_phrase
            .into_iter()
            .flat_map(|(phrase, reason, files)| {
                let query = files[0].stem.clone();
                let resolution = match files.len() {
                    1 => FileReferenceResolution::exact(&query, files[0], reason),
                    _ => FileReferenceResolution::several(&query, files),
                };
                let width = phrase.matches('-').count() + 1;
                folded_question
                    .match_indices(&format!("-{phrase}-"))
                    .map(|(at, _)| {
                        let start = folded_question[..=at]
                            .matches('-')
                            .count()
                            .saturating_sub(1);
                        Hit {
                            start,
                            end: start + width,
                            resolution: resolution.clone(),
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn case_insensitive_paths(&self, path: &str) -> Vec<&'a FileRecord> {
        let wanted = fold_text(&path);
        self.inventory
            .all_files()
            .iter()
            .filter(|file| fold_text(&file.relative_path) == wanted)
            .collect()
    }

    /// Every file whose relative path ends with `reference` at a folder boundary, once case is set
    /// aside: `mars/neurologie.pdf` ends `2026/mars/neurologie.pdf` and nothing else.
    fn paths_ending_with(&self, reference: &str) -> Vec<&'a FileRecord> {
        let wanted = format!("/{}", fold_text(reference.trim_matches('/')));
        self.inventory
            .all_files()
            .iter()
            .filter(|file| format!("/{}", fold_text(&file.relative_path)).ends_with(&wanted))
            .collect()
    }

    fn case_insensitive_names(&self, name: &str) -> Vec<&'a FileRecord> {
        let wanted = fold_text(&name);
        self.inventory
            .all_files()
            .iter()
            .filter(|file| fold_text(&file.name) == wanted)
            .collect()
    }

    /// Every file whose name ends with `fragment`, once case is set aside. The comparison is on
    /// the name alone, never the folder, so a fragment cannot reach across the folder structure.
    fn names_ending_with(&self, fragment: &str) -> Vec<&'a FileRecord> {
        let wanted = fold_text(&fragment);
        self.inventory
            .all_files()
            .iter()
            .filter(|file| {
                let name = fold_text(&file.name);
                name.len() > wanted.len() && name.ends_with(&wanted)
            })
            .collect()
    }

    fn case_insensitive_stems(&self, stem: &str) -> Vec<&'a FileRecord> {
        let wanted = fold_text(&stem);
        self.inventory
            .all_files()
            .iter()
            .filter(|file| fold_text(&file.stem) == wanted)
            .collect()
    }
}

/// Anything that would take the reference out of the Work Folder. Checked on the string, before
/// any lookup and without a filesystem call, so the answer is the same on every platform.
fn escapes_work_folder(path: &str) -> bool {
    if path.starts_with('/') || path.starts_with("//") {
        return true;
    }
    // A Windows drive or UNC prefix: `C:/...`, `\\server\share` (already slash-normalised).
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' {
        return true;
    }
    path.split('/').any(|part| part == "..")
}

/// The characters a reference is not written with at either end: quotes and sentence punctuation.
fn is_reference_edge(c: char) -> bool {
    matches!(
        c,
        '"' | '\'' | ',' | ';' | ':' | '?' | '!' | '(' | ')' | '.'
    )
}

/// The whitespace-separated tokens of a text, each with the byte offset it starts at.
fn tokens_with_offsets(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (index, c) in text.char_indices() {
        if c.is_whitespace() {
            if let Some(from) = start.take() {
                out.push((from, &text[from..index]));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(from) = start {
        out.push((from, &text[from..]));
    }
    out
}

/// `word_tokens`, with the byte offset each word starts at.
fn word_spans(question: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut push = |from: usize, to: usize| {
        let token = &question[from..to];
        if token.chars().count() >= MIN_TOKEN_CHARS && token.chars().any(|c| c.is_alphabetic()) {
            out.push((from, token));
        }
    };
    for (index, c) in question.char_indices() {
        let in_word = c.is_alphanumeric() || c == '-' || c == '_';
        match (in_word, start) {
            (true, None) => start = Some(index),
            (false, Some(from)) => {
                push(from, index);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        push(from, question.len());
    }
    out
}

/// What the question puts between quotes, as folded words (`fold_words`): `"Mes ventes"`,
/// `\u{ab} Mes ventes \u{bb}`, a typographic pair, or single quotes that open a word and are
/// not an apostrophe (`l'annee` opens nothing).
fn quoted_phrases(question: &str) -> Vec<String> {
    let chars: Vec<char> = question.chars().collect();
    let mut out = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let closer = match chars[index] {
            '"' => Some('"'),
            '\u{201c}' => Some('\u{201d}'),
            '\u{ab}' => Some('\u{bb}'),
            '\u{2018}' => Some('\u{2019}'),
            '\'' if index == 0 || !chars[index - 1].is_alphanumeric() => Some('\''),
            _ => None,
        };
        let Some(closer) = closer else {
            index += 1;
            continue;
        };
        let end = (index + 1..chars.len()).find(|&at| {
            chars[at] == closer
                && (closer != '\'' || !chars.get(at + 1).is_some_and(|c| c.is_alphanumeric()))
        });
        match end {
            Some(end) => {
                let phrase = fold_words(&chars[index + 1..end].iter().collect::<String>());
                if !phrase.is_empty() {
                    out.push(phrase);
                }
                index = end + 1;
            }
            None => index += 1,
        }
    }
    out
}

/// Tokens from a sentence that could be a path or a file name: they carry a separator or a dot.
fn path_like_tokens(question: &str) -> Vec<String> {
    question
        .split_whitespace()
        .map(|token| {
            token
                .trim_matches(|c: char| {
                    c == '"'
                        || c == '\''
                        || c == ','
                        || c == ';'
                        || c == ':'
                        || c == '?'
                        || c == '!'
                        || c == '('
                        || c == ')'
                        || c == '.'
                })
                .to_string()
        })
        .filter(|token| !token.is_empty())
        .filter(|token| token.contains('/') || token.contains('\\') || token.contains('.'))
        .collect()
}

/// A fragment worth matching the end of a name against.
///
/// `.pdf` is not one: it would match every PDF in the folder and turn an ordinary question into a
/// disambiguation prompt. What makes a fragment a reference is the part that is not the
/// extension, so that part has to be long enough to have been chosen rather than typed by
/// accident. A fragment carrying a folder separator is not one either - a path is matched as a
/// path, above, and matching part of one would let `mars/neurologie.pdf` reach a file in
/// `janvier/`.
/// Lowercase, accents removed, one canonical form for every spelling of an accent. What she types
/// and what the folder holds are compared through this, so an accented "Esaie", "Esaie" and "ESAIE" are one
/// name, and a Mac's two-code-point accent is the same as Windows's one.
///
/// The body lives in `knowledge::normalize::fold`, the one place the product turns text into a
/// comparable form; this path stays because a dozen modules already import it.
pub(crate) fn fold_text(text: &str) -> String {
    crate::knowledge::normalize::fold(text)
}

/// `fold_text`, with every run of anything that is not a letter or digit reduced to one `-`, so a
/// name written with spaces and the same name written with hyphens compare equal.
fn fold_words(text: &str) -> String {
    let mut out = String::new();
    for c in fold_text(text).chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn usable_fragment(reference: &str) -> Option<String> {
    if reference.contains('/') {
        return None;
    }
    let (stem, _) = split_name(reference);
    (stem.chars().count() >= MIN_FRAGMENT_STEM_CHARS).then(|| reference.to_string())
}

/// Whether a token that carries a dot is written the way a file name is: a stem, then an
/// extension that is a short word with at least one letter (`.pdf`, `.7z`, `.p7m`). A number with
/// points in it (`756.3052.5778.43`, an AVS number; `04.50.12.34.56`, a phone number; `1.5`), a
/// number with a hyphen after the last point (`6.126.452-9`, a policy number) and an e-mail
/// address (`jean.dupont@email.com`) are identifiers, and an identifier is looked up in the
/// documents, never reported as "no such file" (defect KBD-35).
fn looks_like_a_file_name(token: &str) -> bool {
    if token.contains('@') {
        return false;
    }
    let (stem, extension) = split_name(token);
    !stem.is_empty()
        && !extension.is_empty()
        && extension.chars().count() <= MAX_EXTENSION_CHARS
        && extension.chars().all(|c| c.is_ascii_alphanumeric())
        && extension.chars().any(|c| c.is_ascii_alphabetic())
}

/// Words long enough to be a file's name rather than a preposition. Purely numeric words are
/// dropped: a year in a question would otherwise reach for every file filed under it.
fn word_tokens(question: &str) -> Vec<String> {
    question
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|token| token.chars().count() >= MIN_TOKEN_CHARS)
        .filter(|token| token.chars().any(|c| c.is_alphabetic()))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_record::{
        mime_type_for, ExtractionMethod, FileKind, IndexMetadata, ProcessingStatus, Readability,
    };
    use std::path::Path;

    fn record(relative_path: &str) -> FileRecord {
        let name = relative_path
            .rsplit('/')
            .next()
            .unwrap_or(relative_path)
            .to_string();
        let (stem, extension) = split_name(&name);
        FileRecord {
            id: format!("id-{relative_path}"),
            relative_path: relative_path.to_string(),
            name,
            stem,
            mime_type: mime_type_for(&extension).to_string(),
            kind: FileKind::from_extension(&extension),
            extension,
            size_bytes: 10,
            modified_at: Some(0),
            sha256: Some("f".repeat(64)),
            readability: Readability::Readable,
            processing_status: ProcessingStatus::Indexed,
            extraction_method: ExtractionMethod::NativeText,
            indexed: true,
            index_metadata: Some(IndexMetadata {
                indexed_sha256: "f".repeat(64),
                chunk_count: 2,
                ocr_engine: None,
                ocr_engine_version: None,
            }),
        }
    }

    fn nested() -> WorkFolderInventory {
        WorkFolderInventory::from_records(
            Path::new("root"),
            vec![
                record("2026/mars/neurologie.pdf"),
                record("2026/mars/biologie.pdf"),
                record("2026/janvier/neurologie.pdf"),
                record("administratif/assurance.txt"),
            ],
        )
    }

    #[test]
    fn an_exact_relative_path_resolves() {
        let inventory = nested();
        let resolution = FileReferenceResolver::new(&inventory).resolve("2026/mars/neurologie.pdf");

        assert_eq!(resolution.status, ReferenceStatus::Exact);
        assert_eq!(resolution.reason, ResolutionReason::RelativePath);
        assert_eq!(
            resolution.exact_match.unwrap().relative_path,
            "2026/mars/neurologie.pdf"
        );
    }

    #[test]
    fn a_windows_style_separator_resolves_to_the_same_file() {
        let inventory = nested();
        let resolution = FileReferenceResolver::new(&inventory).resolve("2026\\mars\\biologie.pdf");

        assert_eq!(resolution.status, ReferenceStatus::Exact);
    }

    #[test]
    fn a_unique_file_name_resolves() {
        let inventory = nested();
        let resolution = FileReferenceResolver::new(&inventory).resolve("biologie.pdf");

        assert_eq!(resolution.status, ReferenceStatus::Exact);
        assert_eq!(resolution.reason, ResolutionReason::FileName);
    }

    #[test]
    fn a_name_without_its_extension_resolves_when_it_is_unique() {
        let inventory = nested();
        let resolution = FileReferenceResolver::new(&inventory).resolve("biologie");

        assert_eq!(resolution.status, ReferenceStatus::Exact);
        assert_eq!(resolution.reason, ResolutionReason::Stem);
    }

    #[test]
    fn a_name_carried_by_two_files_is_never_chosen_between() {
        let inventory = nested();
        let resolution = FileReferenceResolver::new(&inventory).resolve("neurologie.pdf");

        assert_eq!(resolution.status, ReferenceStatus::MultipleMatches);
        assert!(resolution.exact_match.is_none());
        assert_eq!(resolution.candidates.len(), 2);
        let paths: Vec<&str> = resolution
            .candidates
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect();
        assert_eq!(
            paths,
            vec!["2026/janvier/neurologie.pdf", "2026/mars/neurologie.pdf"]
        );
    }

    #[test]
    fn a_file_that_does_not_exist_resolves_to_nothing() {
        let inventory = nested();
        let resolution = FileReferenceResolver::new(&inventory).resolve("nonexistent.pdf");

        assert_eq!(resolution.status, ReferenceStatus::NoMatch);
        assert_eq!(resolution.reason, ResolutionReason::NotFound);
        assert!(resolution.candidates.is_empty());
    }

    #[test]
    fn a_traversal_attempt_is_refused_rather_than_looked_up() {
        let inventory = nested();
        let resolver = FileReferenceResolver::new(&inventory);

        for attempt in [
            "../../secret.pdf",
            "..\\..\\secret.pdf",
            "/etc/passwd",
            "C:/Windows/system32/config",
            "2026/../../secret.pdf",
        ] {
            let resolution = resolver.resolve(attempt);
            assert_eq!(
                resolution.status,
                ReferenceStatus::NoMatch,
                "attempt: {attempt}"
            );
            assert_eq!(
                resolution.reason,
                ResolutionReason::OutsideWorkFolder,
                "attempt: {attempt}"
            );
        }
    }

    #[test]
    fn a_resolved_file_carries_its_whole_record_and_not_a_name() {
        let inventory = nested();
        let resolution =
            FileReferenceResolver::new(&inventory).resolve("administratif/assurance.txt");
        let resolved = resolution.exact_match.expect("an exact match");

        assert_eq!(resolved.id, "id-administratif/assurance.txt");
        assert_eq!(resolved.extension, "txt");
        assert_eq!(resolved.kind, FileKind::DocumentText);
        assert!(resolved.indexed);
    }

    #[test]
    fn an_extension_in_the_reference_is_never_swapped_for_another() {
        let inventory =
            WorkFolderInventory::from_records(Path::new("root"), vec![record("report.txt")]);

        let resolution = FileReferenceResolver::new(&inventory).resolve("report.pdf");

        assert_eq!(resolution.status, ReferenceStatus::NoMatch);
    }

    #[test]
    fn a_shortened_name_finds_the_file_it_is_the_end_of() {
        // How people actually refer to a dated file: they drop the date, not the subject.
        let inventory = WorkFolderInventory::from_records(
            Path::new("root"),
            vec![
                record("inbox/2026-03-12_compte-rendu-biologie.pdf"),
                record("inbox/2026-03-14_courrier-endocrinologie.docx"),
            ],
        );
        let resolver = FileReferenceResolver::new(&inventory);

        for reference in [
            "12_compte-rendu-biologie.pdf",
            "compte-rendu-biologie.pdf",
            "rendu-biologie.pdf",
        ] {
            let resolution = resolver.resolve(reference);
            assert_eq!(resolution.status, ReferenceStatus::Exact, "{reference}");
            assert_eq!(
                resolution.reason,
                ResolutionReason::FileNameSuffix,
                "{reference}"
            );
            assert_eq!(
                resolution.exact_match.unwrap().relative_path,
                "inbox/2026-03-12_compte-rendu-biologie.pdf",
                "{reference}"
            );
        }
    }

    #[test]
    fn a_shortened_name_carried_by_two_files_is_still_never_chosen_between() {
        let inventory = WorkFolderInventory::from_records(
            Path::new("root"),
            vec![
                record("2026/mars/courrier-neurologie.pdf"),
                record("2026/janvier/compte-rendu-neurologie.pdf"),
            ],
        );

        let resolution = FileReferenceResolver::new(&inventory).resolve("neurologie.pdf");

        assert_eq!(resolution.status, ReferenceStatus::MultipleMatches);
        assert_eq!(resolution.candidates.len(), 2);
    }

    #[test]
    fn a_file_actually_called_that_wins_over_one_that_merely_ends_with_it() {
        // Shortening is a convenience, not a licence to overrule what a file is called.
        let inventory = WorkFolderInventory::from_records(
            Path::new("root"),
            vec![
                record("2026/mars/neurologie.pdf"),
                record("2026/janvier/courrier-neurologie.pdf"),
            ],
        );

        let resolution = FileReferenceResolver::new(&inventory).resolve("neurologie.pdf");

        assert_eq!(resolution.status, ReferenceStatus::Exact);
        assert_eq!(resolution.reason, ResolutionReason::FileName);
        assert_eq!(
            resolution.exact_match.unwrap().relative_path,
            "2026/mars/neurologie.pdf"
        );
    }

    #[test]
    fn a_bare_extension_is_not_a_reference_to_anything() {
        let inventory = nested();

        for reference in [".pdf", "pdf", "f.pdf"] {
            assert_ne!(
                FileReferenceResolver::new(&inventory)
                    .resolve(reference)
                    .status,
                ReferenceStatus::Exact,
                "{reference}"
            );
        }
    }

    #[test]
    fn a_shortened_name_still_never_reaches_across_folders() {
        // A fragment is matched against names, never against paths: `janvier/neurologie.pdf` is
        // not a way to reach the file of that name under `mars/`.
        let inventory = nested();

        let resolution = FileReferenceResolver::new(&inventory).resolve("vier/neurologie.pdf");

        assert_eq!(resolution.status, ReferenceStatus::NoMatch);
    }

    #[test]
    fn a_shortened_name_in_a_question_finds_the_file() {
        let inventory = WorkFolderInventory::from_records(
            Path::new("root"),
            vec![record("inbox/2026-03-12_compte-rendu-biologie.pdf")],
        );

        let resolution = FileReferenceResolver::new(&inventory)
            .resolve_in_question("Que dit 12_compte-rendu-biologie.pdf ?")
            .expect("the question names a file");

        assert_eq!(resolution.status, ReferenceStatus::Exact);
        assert_eq!(
            resolution.exact_match.unwrap().relative_path,
            "inbox/2026-03-12_compte-rendu-biologie.pdf"
        );
    }

    #[test]
    fn a_question_naming_a_file_finds_it() {
        let inventory = nested();
        let resolver = FileReferenceResolver::new(&inventory);

        let resolution = resolver
            .resolve_in_question("Que contient biologie.pdf ?")
            .expect("the question names a file");

        assert_eq!(resolution.status, ReferenceStatus::Exact);
    }

    #[test]
    fn a_question_naming_an_ambiguous_file_reports_both() {
        let inventory = nested();
        let resolver = FileReferenceResolver::new(&inventory);

        let resolution = resolver
            .resolve_in_question("Que contient neurologie.pdf ?")
            .expect("the question names a file");

        assert_eq!(resolution.status, ReferenceStatus::MultipleMatches);
        assert_eq!(resolution.candidates.len(), 2);
    }

    #[test]
    fn a_question_naming_a_file_that_is_absent_says_so_rather_than_searching() {
        let inventory = nested();
        let resolver = FileReferenceResolver::new(&inventory);

        let resolution = resolver
            .resolve_in_question("Que contient secret-report.pdf ?")
            .expect("the question names a file");

        assert_eq!(resolution.status, ReferenceStatus::NoMatch);
    }

    #[test]
    fn a_question_about_content_with_no_file_in_it_narrows_nothing() {
        let inventory = nested();
        let resolver = FileReferenceResolver::new(&inventory);

        assert!(resolver
            .resolve_in_question("Quel taux de glycemie pour Camille ?")
            .is_none());
        assert!(resolver
            .resolve_in_question("What did the specialist recommend?")
            .is_none());
    }

    #[test]
    fn a_year_in_a_question_does_not_reach_for_every_file_filed_under_it() {
        let inventory = nested();
        let resolver = FileReferenceResolver::new(&inventory);

        assert!(resolver
            .resolve_in_question("Quoi de neuf en 2026 ?")
            .is_none());
    }

    #[test]
    fn a_bare_stem_in_a_question_still_finds_a_unique_file() {
        let inventory = nested();
        let resolver = FileReferenceResolver::new(&inventory);

        let resolution = resolver
            .resolve_in_question("Que dit biologie ?")
            .expect("the question names a stem");

        assert_eq!(resolution.status, ReferenceStatus::Exact);
        assert_eq!(resolution.reason, ResolutionReason::StemToken);
    }

    fn esaie() -> WorkFolderInventory {
        WorkFolderInventory::from_records(
            Path::new("root"),
            vec![
                record("Ordonnance-pour-Esaie.pdf"),
                record("Ordonnance-pour-Esaie-2.pdf"),
                record("Absence-pour-Esaie.pdf"),
            ],
        )
    }

    #[test]
    fn a_name_typed_with_accents_finds_the_accent_free_file() {
        let inventory = esaie();
        let resolver = FileReferenceResolver::new(&inventory);

        let resolution = resolver.resolve("Absence-pour-Esa\u{ef}e.pdf");

        assert_eq!(resolution.status, ReferenceStatus::Exact);
        assert_eq!(resolution.exact_match.unwrap().name, "Absence-pour-Esaie.pdf");
    }

    #[test]
    fn both_spellings_of_an_accent_match_the_same_file() {
        let inventory = esaie();
        let resolver = FileReferenceResolver::new(&inventory);

        for typed in ["Absence-pour-Esa\u{ef}e.pdf", "Absence-pour-Esai\u{308}e.pdf"] {
            assert_eq!(resolver.resolve(typed).status, ReferenceStatus::Exact, "{typed}");
        }
    }

    #[test]
    fn a_multi_word_name_typed_with_spaces_is_recognised() {
        let inventory = esaie();
        let resolver = FileReferenceResolver::new(&inventory);

        let resolution = resolver
            .resolve_in_question("Que dit Absence pour Esa\u{ef}e ?")
            .expect("the question names a file");

        assert_eq!(resolution.status, ReferenceStatus::Exact);
        assert_eq!(resolution.exact_match.unwrap().name, "Absence-pour-Esaie.pdf");
    }

    #[test]
    fn naming_the_longer_of_two_overlapping_files_names_that_one() {
        let inventory = esaie();
        let resolver = FileReferenceResolver::new(&inventory);

        let resolution = resolver
            .resolve_in_question("Resume Ordonnance pour Esaie 2 s'il te plait")
            .expect("the question names a file");

        assert_eq!(resolution.exact_match.unwrap().name, "Ordonnance-pour-Esaie-2.pdf");
    }

    #[test]
    fn an_ordinary_sentence_names_no_file() {
        let inventory = esaie();
        let resolver = FileReferenceResolver::new(&inventory);

        assert!(resolver
            .resolve_in_question("What dose was given to Esaie ?")
            .is_none());
    }

    /// Names that hold spaces, accents, parentheses, plus a plain neighbour for each, and a
    /// workbook of the other folder (a foreign key).
    fn spaced() -> WorkFolderInventory {
        WorkFolderInventory::from_records(
            Path::new("root"),
            vec![
                record("Ventes 2025 (final).xlsx"),
                record("Mes ventes.xlsx"),
                record("ventes 2025.xlsx"),
                record("Ventes.xlsx"),
                record("Bilan \u{e9}t\u{e9} 2025.xlsx"),
                record("/documents/Suivi commandes clients.xlsx"),
                record("Q1 24.xlsx"),
            ],
        )
    }

    fn named(inventory: &WorkFolderInventory, question: &str) -> Vec<String> {
        FileReferenceResolver::new(inventory)
            .resolve_all_in_question(question)
            .into_iter()
            .map(|found| match found.resolution.exact_match {
                Some(record) => record.relative_path,
                None => format!("?{}", found.resolution.candidates.len()),
            })
            .collect()
    }

    #[test]
    fn a_name_with_spaces_resolves_in_every_spelling_a_person_types() {
        let inventory = spaced();
        let cases: [(&str, &str); 22] = [
            (
                "total of Ventes 2025 (final).xlsx",
                "Ventes 2025 (final).xlsx",
            ),
            (
                "total of \"Ventes 2025 (final).xlsx\"",
                "Ventes 2025 (final).xlsx",
            ),
            ("total of Ventes 2025 (final)", "Ventes 2025 (final).xlsx"),
            (
                "total of \"Ventes 2025 (final)\"",
                "Ventes 2025 (final).xlsx",
            ),
            (
                "total of ventes 2025 (final) please",
                "Ventes 2025 (final).xlsx",
            ),
            (
                "TOTAL OF VENTES 2025 (FINAL).XLSX",
                "Ventes 2025 (final).xlsx",
            ),
            ("total of Mes ventes.xlsx", "Mes ventes.xlsx"),
            ("total of \"Mes ventes.xlsx\"", "Mes ventes.xlsx"),
            ("total of Mes ventes", "Mes ventes.xlsx"),
            ("total of \"Mes ventes\"", "Mes ventes.xlsx"),
            ("TOTAL OF MES VENTES", "Mes ventes.xlsx"),
            ("total of ventes 2025.xlsx", "ventes 2025.xlsx"),
            ("total of ventes 2025", "ventes 2025.xlsx"),
            ("total of \"ventes 2025\"", "ventes 2025.xlsx"),
            (
                "total of Bilan \u{e9}t\u{e9} 2025.xlsx",
                "Bilan \u{e9}t\u{e9} 2025.xlsx",
            ),
            (
                "total of bilan ete 2025.xlsx",
                "Bilan \u{e9}t\u{e9} 2025.xlsx",
            ),
            (
                "total of BILAN \u{c9}T\u{c9} 2025",
                "Bilan \u{e9}t\u{e9} 2025.xlsx",
            ),
            (
                "total of Suivi commandes clients.xlsx",
                "/documents/Suivi commandes clients.xlsx",
            ),
            (
                "total of suivi commandes clients",
                "/documents/Suivi commandes clients.xlsx",
            ),
            (
                "total of \"Suivi commandes clients\"",
                "/documents/Suivi commandes clients.xlsx",
            ),
            ("total of \u{ab} Mes ventes \u{bb}", "Mes ventes.xlsx"),
            ("total of \"Q1 24\"", "Q1 24.xlsx"),
        ];
        for (question, expected) in cases {
            assert_eq!(
                named(&inventory, question),
                vec![expected.to_string()],
                "{question}"
            );
        }
    }

    #[test]
    fn the_longest_name_in_a_run_wins_and_a_plain_neighbour_is_not_named() {
        let inventory = spaced();
        // `Ventes.xlsx` ends the text of `Mes ventes.xlsx` and `ventes 2025.xlsx` is the start of
        // `Ventes 2025 (final).xlsx`: each question names exactly one file.
        assert_eq!(
            named(&inventory, "sum Mes ventes.xlsx"),
            vec!["Mes ventes.xlsx"]
        );
        assert_eq!(named(&inventory, "sum Ventes.xlsx"), vec!["Ventes.xlsx"]);
        assert_eq!(
            named(&inventory, "sum Ventes 2025 (final).xlsx"),
            vec!["Ventes 2025 (final).xlsx"]
        );
    }

    #[test]
    fn several_names_come_back_in_the_order_the_question_writes_them() {
        let inventory = spaced();
        let resolver = FileReferenceResolver::new(&inventory);

        let found = resolver.resolve_all_in_question(
            "compare Mes ventes.xlsx with ventes 2025.xlsx and Ventes.xlsx",
        );
        let files: Vec<_> = found
            .iter()
            .map(|each| each.resolution.exact_match.clone().unwrap().relative_path)
            .collect();
        assert_eq!(
            files,
            ["Mes ventes.xlsx", "ventes 2025.xlsx", "Ventes.xlsx"]
        );
        assert!(found
            .windows(2)
            .all(|pair| pair[0].position < pair[1].position));

        assert_eq!(
            named(&inventory, "compare ventes 2025.xlsx with Mes ventes.xlsx"),
            ["ventes 2025.xlsx", "Mes ventes.xlsx"]
        );
        // The same file twice is one reference, at its first place.
        assert_eq!(
            named(&inventory, "Mes ventes.xlsx against Mes ventes"),
            ["Mes ventes.xlsx"]
        );
    }

    #[test]
    fn a_stem_word_and_a_path_are_still_named_beside_a_spaced_name() {
        let inventory = WorkFolderInventory::from_records(
            Path::new("root"),
            vec![
                record("2026/mars/neurologie.pdf"),
                record("Mes ventes.xlsx"),
                record("montants.csv"),
            ],
        );
        assert_eq!(
            named(
                &inventory,
                "compare montants with Mes ventes.xlsx and 2026/mars/neurologie.pdf"
            ),
            [
                "montants.csv",
                "Mes ventes.xlsx",
                "2026/mars/neurologie.pdf"
            ]
        );
    }

    #[test]
    fn two_files_with_the_same_name_are_one_ambiguous_reference() {
        let inventory = WorkFolderInventory::from_records(
            Path::new("root"),
            vec![
                record("2025/Mes ventes.xlsx"),
                record("2026/Mes ventes.xlsx"),
            ],
        );
        let found = FileReferenceResolver::new(&inventory)
            .resolve_all_in_question("total of Mes ventes.xlsx");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].resolution.status, ReferenceStatus::MultipleMatches);
        assert_eq!(found[0].resolution.candidates.len(), 2);
    }

    #[test]
    fn a_name_that_exists_nowhere_is_reported_and_an_ordinary_sentence_names_nothing() {
        let inventory = spaced();
        let resolver = FileReferenceResolver::new(&inventory);

        let found = resolver.resolve_all_in_question("total of Absent.xlsx");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].resolution.status, ReferenceStatus::NoMatch);

        assert!(resolver
            .resolve_all_in_question("what is the total of the year")
            .is_empty());
        assert!(resolver
            .resolve_all_in_question("cost is 1.5 per unit")
            .is_empty());
    }

    #[test]
    fn the_known_limits_of_a_name_with_spaces_stay_unresolved() {
        let inventory = spaced();
        // Too short to be told from a phrase of the sentence unless it is quoted or written with
        // its extension (`Q1 24.xlsx` and `"Q1 24"` resolve, see above).
        assert!(named(&inventory, "total of q1 24").is_empty());
        // A part of a name, or its words in another order, is not the name.
        assert!(named(&inventory, "total of final 2025").is_empty());
        assert!(named(&inventory, "total of 2025 final").is_empty());
    }

    #[test]
    fn the_first_reference_is_the_one_resolve_in_question_returns() {
        let inventory = esaie();
        let resolver = FileReferenceResolver::new(&inventory);
        for question in [
            "Que dit Absence pour Esa\u{ef}e ?",
            "Resume Ordonnance pour Esaie 2 s'il te plait",
            "What dose was given to Esaie ?",
        ] {
            let first = resolver.resolve_in_question(question);
            let all = resolver.resolve_all_in_question(question);
            assert_eq!(
                first.as_ref().map(|each| each.exact_match.clone()),
                all.first().map(|each| each.resolution.exact_match.clone()),
                "{question}"
            );
        }
    }

    #[test]
    fn quoted_phrases_ignore_apostrophes() {
        assert_eq!(
            quoted_phrases("sum \"Mes ventes\" and 'Q1 24' ok"),
            ["mes-ventes", "q1-24"]
        );
        assert_eq!(
            quoted_phrases("sum \u{ab} Mes ventes \u{bb}"),
            ["mes-ventes"]
        );
        assert!(quoted_phrases("the customer's total and l'annee").is_empty());
    }

    /// The body `fold_text` had before it moved to `knowledge::normalize::fold`, kept verbatim so
    /// the move is proved rather than believed.
    fn fold_text_before_the_move(text: &str) -> String {
        use unicode_normalization::char::is_combining_mark;
        use unicode_normalization::UnicodeNormalization;
        text.nfd()
            .filter(|c| !is_combining_mark(*c))
            .flat_map(char::to_lowercase)
            .collect()
    }

    #[test]
    fn fold_text_gives_the_same_output_on_the_names_of_every_fixture_folder() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let roots = [
            manifest.join("../../../fixtures"),
            manifest.join("../../../docs/test-reports/human-acceptance-pass-1/fixtures"),
            manifest.join("tests/fixtures"),
        ];
        let mut names: Vec<String> = Vec::new();
        for root in roots {
            for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        // A Mac stores an accent as two code points; a Windows folder as one. Both must keep
        // going through the same door.
        names.extend(
            [
                "Compte-rendu \u{c9}sa\u{ef}e.pdf",
                "Compte-rendu E\u{301}sai\u{308}e.pdf",
                "\u{152}UVRE-d\u{2019}Aubign\u{e9}.docx",
                "ma\u{ee}tre_Fran\u{e7}ois_2026-03.txt",
            ]
            .map(String::from),
        );

        assert!(names.len() > 20, "the fixture folders were not found");
        for name in names {
            assert_eq!(fold_text(&name), fold_text_before_the_move(&name), "{name}");
        }
    }
}
