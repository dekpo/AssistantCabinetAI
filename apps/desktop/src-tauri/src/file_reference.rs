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
            return match folded.len() {
                1 => {
                    FileReferenceResolution::exact(query, folded[0], ResolutionReason::RelativePath)
                }
                0 => FileReferenceResolution::none(query, ResolutionReason::NotFound),
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

    fn case_insensitive_paths(&self, path: &str) -> Vec<&'a FileRecord> {
        let wanted = fold_text(&path);
        self.inventory
            .all_files()
            .iter()
            .filter(|file| fold_text(&file.relative_path) == wanted)
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
fn fold_text(text: &str) -> String {
    use unicode_normalization::char::is_combining_mark;
    use unicode_normalization::UnicodeNormalization;
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
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

fn looks_like_a_file_name(token: &str) -> bool {
    let (stem, extension) = split_name(token);
    !stem.is_empty() && !extension.is_empty()
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
}
