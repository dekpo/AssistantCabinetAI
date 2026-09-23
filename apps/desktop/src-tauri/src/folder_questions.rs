//! Deterministic before generative, applied to the Work Folder (`docs/ARCHITECTURE.md`).
//!
//! "How many files are there?" has one right answer and the filesystem already holds it. Sending
//! that question to a model would spend a round trip to get a guess, and a guess assembled from
//! whichever passages retrieval happened to return - which is how a folder of fifteen files comes
//! to be described as a folder of six. Questions like it are answered here, from the inventory,
//! with no gateway call at all.
//!
//! The vocabulary lives in a locale pattern pack loaded as **data**, never as literals in Rust
//! (`docs/DECISIONS.md`): a French sentence in this crate would fail the language guard, and a
//! question pack is not code.
//!
//! Three things have to hold before a question is answered from the folder. It needs an
//! **intent** and a **subject** - something to do, and something to do it to. It must carry no
//! **content word**: "give me a summary of each document" is not a listing request however much
//! of its grammar it shares with one. And no word the pack does not know may be a word the
//! **documents** contain: that is what separates "how many documents mention metformin" from
//! "can you list all the available documents", without a dictionary and without demanding that
//! every polite phrasing be enumerated in advance.
//!
//! What this module returns is a machine code plus data. It never writes a sentence: the React
//! catalogues do that, in the practice's own language (`docs/LANGUAGE-AND-LOCALE.md`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::file_record::{FileRecord, Readability};
use crate::file_reference::{FileReferenceResolver, ReferenceStatus};
use crate::inventory::WorkFolderInventory;

/// The shipped packs. `include_str!` rather than a runtime resource lookup: the vocabulary is
/// part of the build, so it cannot go missing on a machine and cannot be edited into something
/// else beside the executable.
const PACKS: &[(&str, &str)] = &[
    (
        "en-US",
        include_str!("../resources/work-folder-questions/en-US.json"),
    ),
    (
        "fr-FR",
        include_str!("../resources/work-folder-questions/fr-FR.json"),
    ),
];

const FALLBACK_LOCALE: &str = "fr-FR";

#[derive(Debug, Clone, Deserialize)]
struct PatternPack {
    filler: Vec<String>,
    /// Words that mean "tell me what is inside". They disqualify the deterministic path outright,
    /// whatever else the sentence contains: "give me a summary of each document" shares every
    /// other word with a listing request, and answering it with a list of names would be a
    /// confident answer to a question nobody asked. A short denylist rather than a long
    /// allowlist, because the ways of asking for content are few and the ways of asking politely
    /// are not.
    content: Vec<String>,
    operations: Operations,
    /// Words that put every document in scope: "each", "every", "chaque", "tous". They are what
    /// turns a content question into a question about the whole corpus, and the corpus is the
    /// inventory's business rather than the ranker's.
    distributive: Vec<String>,
    subjects: Subjects,
    extensions: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
struct Operations {
    count: Vec<String>,
    list: Vec<String>,
    status: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct Subjects {
    structure: Vec<String>,
    unreadable: Vec<String>,
    indexed: Vec<String>,
    images: Vec<String>,
    /// Anything on disk.
    files: Vec<String>,
    /// A file something could be read from. Her word, and the one that matters: "all my
    /// documents" means the ones the software can actually answer from, not the images it failed
    /// to read (`docs/WORK-FOLDER-INVENTORY.md`).
    documents: Vec<String>,
    folders: Vec<String>,
}

/// The pack for a locale, falling back the way the interface does: the exact tag, then any pack
/// for the same language, then the pilot's own.
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

/// The words the indexed documents actually contain.
///
/// This is how a question is told apart from a question about the folder without a dictionary.
/// "How many documents mention metformin" and "can you list all the available documents" share
/// their shape; what separates them is that `metformin` is a word in the corpus and `available`
/// is not. The index already knows this, so nothing new has to be maintained, and it is a port
/// rather than a call into SQLite so the routing stays testable without a database.
pub trait CorpusWords {
    /// Whether `word` appears in any indexed document. Lowercase, already tokenised.
    fn contains(&self, word: &str) -> bool;
}

/// A corpus with nothing in it: a folder that has never been analysed. Every question then reads
/// as a question about the folder, which is the right reading when there is nothing to ask about.
pub struct NoCorpus;

impl CorpusWords for NoCorpus {
    fn contains(&self, _word: &str) -> bool {
        false
    }
}

/// What the interface is told, in machine codes. Every variant carries exactly the facts the
/// sentence needs, and no document text.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FolderAnswer {
    FileCount {
        total: usize,
    },
    FileList {
        files: Vec<FileRecord>,
    },
    ExtensionCount {
        extension: String,
        count: usize,
    },
    ExtensionList {
        extension: String,
        files: Vec<FileRecord>,
    },
    ImageList {
        files: Vec<FileRecord>,
    },
    UnreadableCount {
        count: usize,
    },
    UnreadableList {
        files: Vec<FileRecord>,
    },
    IndexedCount {
        count: usize,
    },
    IndexedList {
        files: Vec<FileRecord>,
    },
    FolderList {
        folders: Vec<String>,
    },
    FolderTree {
        root: String,
        /// One indented line per entry, deepest last. The interface renders it as it stands.
        lines: Vec<String>,
    },
    FileDetails {
        /// Boxed so one large variant does not set the size of every answer. Serde sees through
        /// the box, so the wire form is unchanged.
        file: Box<FileRecord>,
    },
    /// Several files carry the name the question used. The interface asks which one; nothing is
    /// retrieved and nothing is chosen (`docs/WORK-FOLDER-INVENTORY.md`).
    AmbiguousReference {
        query: String,
        candidates: Vec<FileRecord>,
    },
    /// The question named a file the Work Folder does not hold.
    NoMatchingFile {
        query: String,
    },
    /// The question named a file nothing could be read from. Its content is not guessed at.
    FileUnreadable {
        file: Box<FileRecord>,
    },
}

/// Where a question goes.
#[derive(Debug, Clone, PartialEq)]
pub enum QuestionRoute {
    /// Answered from the inventory. No gateway call, no retrieval, no model.
    Deterministic(FolderAnswer),
    /// The question names one readable file: retrieve inside it, then answer.
    TargetedRetrieval { file: Box<FileRecord> },
    /// "Summarise **each** document": a content question whose scope is the whole corpus.
    ///
    /// Ordinary retrieval ranks by similarity and stops at a handful of excerpts, so a folder of
    /// ten indexed documents is answered from six of them and the other four are never sent. The
    /// question asked about all of them, so the **inventory** supplies the list and retrieval runs
    /// once inside each file (`docs/WORK-FOLDER-INVENTORY.md`).
    PerDocumentRetrieval {
        /// Every indexed file, in canonical order. The authoritative list, not a ranking.
        files: Vec<FileRecord>,
    },
    /// Ordinary content question: the existing whole-folder retrieval, unchanged.
    GlobalRetrieval,
}

/// Decide what to do with one question, from the inventory alone.
///
/// Order matters. A named file is looked at first, because "what does neurologie.pdf say?" is a
/// content question whose scope is already settled and whose failure modes - two files with that
/// name, no file with that name, a file nothing could be read from - must be reported rather than
/// papered over with whatever retrieval finds. Only then is the question tested against the
/// filesystem vocabulary, and only then does it fall through to ordinary retrieval.
pub fn route(
    inventory: &WorkFolderInventory,
    corpus: &dyn CorpusWords,
    question: &str,
    locale: &str,
) -> QuestionRoute {
    let pack = pack_for(locale);
    let resolver = FileReferenceResolver::new(inventory);

    if let Some(resolution) = resolver.resolve_in_question(question) {
        return match resolution.status {
            ReferenceStatus::MultipleMatches => {
                QuestionRoute::Deterministic(FolderAnswer::AmbiguousReference {
                    query: resolution.query,
                    candidates: resolution.candidates,
                })
            }
            ReferenceStatus::NoMatch => {
                QuestionRoute::Deterministic(FolderAnswer::NoMatchingFile {
                    query: resolution.query,
                })
            }
            ReferenceStatus::Exact => {
                let file = resolution
                    .exact_match
                    .expect("an exact match carries a record");
                // "What is the status of X?" and "is X indexed?" are filesystem questions that
                // happen to name a file, not content questions: they are answered from the
                // record. Asking whether a file was read is not asking what it says.
                let asks_for_status = pack.as_ref().is_some_and(|pack| {
                    mentions(question, &pack.operations.status)
                        || mentions(question, &pack.subjects.indexed)
                        || mentions(question, &pack.subjects.unreadable)
                });
                if asks_for_status {
                    return QuestionRoute::Deterministic(FolderAnswer::FileDetails {
                        file: Box::new(file),
                    });
                }
                if file.readability == Readability::Unreadable {
                    return QuestionRoute::Deterministic(FolderAnswer::FileUnreadable {
                        file: Box::new(file),
                    });
                }
                if !file.indexed {
                    // Named, present, and nothing has been read from it yet. Saying so beats
                    // answering from the rest of the folder as though it had been.
                    return QuestionRoute::Deterministic(FolderAnswer::FileDetails {
                        file: Box::new(file),
                    });
                }
                QuestionRoute::TargetedRetrieval {
                    file: Box::new(file),
                }
            }
        };
    }

    if let Some(pack) = pack {
        // The filesystem questions come first, so "list all files" stays a listing rather than
        // becoming a request to read all of them.
        if let Some(answer) = folder_answer(inventory, corpus, question, &pack) {
            return QuestionRoute::Deterministic(answer);
        }
        if puts_every_document_in_scope(question, &pack) {
            return QuestionRoute::PerDocumentRetrieval {
                files: inventory.indexed_files().into_iter().cloned().collect(),
            };
        }
    }

    QuestionRoute::GlobalRetrieval
}

/// Whether the question asks about **every** document rather than about whatever is most
/// relevant: a distributive word plus a word for the documents themselves.
///
/// Both halves are required. "Each" on its own is ordinary English ("each time"), and "documents"
/// on its own is the common case that relevance ranking already serves well.
fn puts_every_document_in_scope(question: &str, pack: &PatternPack) -> bool {
    let tokens = tokenise(question);
    let distributive = tokens
        .iter()
        .any(|token| contains(&pack.distributive, token));
    let documents = tokens.iter().any(|token| {
        contains(&pack.subjects.files, token) || contains(&pack.subjects.documents, token)
    });
    distributive && documents
}

/// The whole-folder questions. `None` when the question is not one of them, which is the common
/// case and must stay cheap.
fn folder_answer(
    inventory: &WorkFolderInventory,
    corpus: &dyn CorpusWords,
    question: &str,
    pack: &PatternPack,
) -> Option<FolderAnswer> {
    let tokens = tokenise(question);
    if tokens.len() < 2 {
        return None;
    }
    if tokens.iter().any(|token| contains(&pack.content, token)) {
        return None;
    }

    let mut counts = false;
    let mut subject: Option<Subject> = None;
    let mut extension: Option<String> = None;

    for token in &tokens {
        if contains(&pack.operations.count, token) {
            counts = true;
            continue;
        }
        if contains(&pack.operations.list, token) || contains(&pack.operations.status, token) {
            continue;
        }
        if let Some(found) = extension_for(pack, token) {
            extension = Some(found);
            continue;
        }
        if let Some(found) = subject_for(&pack.subjects, token) {
            subject = Some(match (subject, found) {
                (Some(existing), candidate) => existing.max(candidate),
                (None, candidate) => candidate,
            });
            continue;
        }
        if contains(&pack.filler, token) {
            continue;
        }
        // A word the pack does not know. Whether that disqualifies the question depends on where
        // the word comes from: one that appears in the documents is what the question is really
        // about ("metformine"), and one that does not is ordinary phrasing the vocabulary simply
        // has not been taught ("disponibles"). Requiring every word to be known was the stricter
        // rule, and a politely phrased listing request went to a model that then had to
        // transcribe sixteen paths by hand. The exact sentence is in
        // `tests/work_folder_inventory.rs`, where a French example is allowed.
        if corpus.contains(token) {
            return None;
        }
    }

    // An extension on its own ("how many PDFs?") is a subject; with one named, it wins over a
    // bare "files", which is only there to make the sentence grammatical.
    if let Some(extension) = extension {
        let files: Vec<FileRecord> = inventory
            .files_with_extension(&extension)
            .into_iter()
            .cloned()
            .collect();
        return Some(if counts {
            FolderAnswer::ExtensionCount {
                extension,
                count: files.len(),
            }
        } else {
            FolderAnswer::ExtensionList { extension, files }
        });
    }

    match subject? {
        Subject::Folders => Some(FolderAnswer::FolderList {
            folders: inventory.folders(),
        }),
        Subject::Files => Some(if counts {
            FolderAnswer::FileCount {
                total: inventory.file_count(),
            }
        } else {
            FolderAnswer::FileList {
                files: inventory.all_files().to_vec(),
            }
        }),
        // "How many documents" is the same question as "how many were analysed": a file nothing
        // could be read from is not a document she can ask anything about.
        Subject::Documents => {
            let files: Vec<FileRecord> = inventory.indexed_files().into_iter().cloned().collect();
            Some(if counts {
                FolderAnswer::IndexedCount { count: files.len() }
            } else {
                FolderAnswer::IndexedList { files }
            })
        }
        Subject::Images => Some(FolderAnswer::ImageList {
            files: inventory
                .files_of_kind(crate::file_record::FileKind::Image)
                .into_iter()
                .cloned()
                .collect(),
        }),
        Subject::Indexed => {
            let files: Vec<FileRecord> = inventory.indexed_files().into_iter().cloned().collect();
            Some(if counts {
                FolderAnswer::IndexedCount { count: files.len() }
            } else {
                FolderAnswer::IndexedList { files }
            })
        }
        Subject::Unreadable => {
            let files: Vec<FileRecord> =
                inventory.unreadable_files().into_iter().cloned().collect();
            Some(if counts {
                FolderAnswer::UnreadableCount { count: files.len() }
            } else {
                FolderAnswer::UnreadableList { files }
            })
        }
        Subject::Structure => Some(FolderAnswer::FolderTree {
            root: inventory.root_identifier(),
            lines: tree_lines(inventory),
        }),
    }
}

/// What a question is about. The order is the priority: a question mentioning both files and the
/// folder structure is about the structure, and one mentioning both files and unreadability is
/// about the unreadable ones. `Ord` is derived from this declaration order, and `max` applies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Subject {
    Folders,
    Files,
    Documents,
    Images,
    Indexed,
    Unreadable,
    Structure,
}

fn subject_for(subjects: &Subjects, token: &str) -> Option<Subject> {
    if contains(&subjects.structure, token) {
        return Some(Subject::Structure);
    }
    if contains(&subjects.unreadable, token) {
        return Some(Subject::Unreadable);
    }
    if contains(&subjects.indexed, token) {
        return Some(Subject::Indexed);
    }
    if contains(&subjects.images, token) {
        return Some(Subject::Images);
    }
    if contains(&subjects.documents, token) {
        return Some(Subject::Documents);
    }
    if contains(&subjects.files, token) {
        return Some(Subject::Files);
    }
    if contains(&subjects.folders, token) {
        return Some(Subject::Folders);
    }
    None
}

fn extension_for(pack: &PatternPack, token: &str) -> Option<String> {
    pack.extensions
        .iter()
        .find(|(_, words)| contains(words, token))
        .map(|(extension, _)| extension.clone())
}

fn contains(words: &[String], token: &str) -> bool {
    words.iter().any(|word| word == token)
}

fn mentions(question: &str, words: &[String]) -> bool {
    tokenise(question)
        .iter()
        .any(|token| contains(words, token))
}

/// Words, lowercased. Splitting on everything that is not a letter or a digit keeps the rule the
/// same in both languages and means punctuation cannot change an answer.
fn tokenise(question: &str) -> Vec<String> {
    question
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_lowercase())
        .collect()
}

fn tree_lines(inventory: &WorkFolderInventory) -> Vec<String> {
    let mut lines = Vec::new();
    collect(&inventory.hierarchy(), 0, &mut lines);
    lines
}

fn collect(node: &crate::inventory::FolderNode, depth: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    for name in &node.files {
        lines.push(format!("{indent}{name}"));
    }
    for child in &node.folders {
        lines.push(format!("{indent}{}/", child.name));
        collect(child, depth + 1, lines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_record::{
        mime_type_for, split_name, ExtractionMethod, FileKind, ProcessingStatus,
    };
    use std::path::Path;

    fn record(relative_path: &str, readability: Readability) -> FileRecord {
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
            size_bytes: 1,
            modified_at: None,
            sha256: None,
            readability,
            processing_status: match readability {
                Readability::Readable => ProcessingStatus::Indexed,
                Readability::Unreadable => ProcessingStatus::Failed,
                Readability::NotAssessed => ProcessingStatus::Discovered,
            },
            extraction_method: match readability {
                Readability::Readable => ExtractionMethod::NativeText,
                _ => ExtractionMethod::None,
            },
            indexed: readability == Readability::Readable,
            index_metadata: None,
        }
    }

    fn folder() -> WorkFolderInventory {
        WorkFolderInventory::from_records(
            Path::new("cabinet"),
            vec![
                record("biologie.pdf", Readability::Readable),
                record("neurologie.pdf", Readability::Readable),
                record("notes.txt", Readability::Readable),
                record("scan.png", Readability::Unreadable),
                record("photo.jpg", Readability::Unreadable),
            ],
        )
    }

    /// A corpus double: the words a set of documents would contain.
    struct Words(&'static [&'static str]);

    impl CorpusWords for Words {
        fn contains(&self, word: &str) -> bool {
            self.0.contains(&word)
        }
    }

    fn answer(question: &str, locale: &str) -> Option<FolderAnswer> {
        match route(&folder(), &NoCorpus, question, locale) {
            QuestionRoute::Deterministic(answer) => Some(answer),
            _ => None,
        }
    }

    #[test]
    fn counting_files_is_answered_from_the_inventory_in_both_languages() {
        assert_eq!(
            answer("How many files are in the work folder?", "en-US"),
            Some(FolderAnswer::FileCount { total: 5 })
        );
        assert_eq!(
            answer("Combien de fichiers au total ?", "fr-FR"),
            Some(FolderAnswer::FileCount { total: 5 })
        );
    }

    #[test]
    fn listing_files_returns_every_one_of_them() {
        let Some(FolderAnswer::FileList { files }) = answer("List all files.", "en-US") else {
            panic!("expected a file listing");
        };

        assert_eq!(files.len(), 5);
    }

    #[test]
    fn counting_by_extension_uses_the_filesystem_extension() {
        assert_eq!(
            answer("How many PDF files are there?", "en-US"),
            Some(FolderAnswer::ExtensionCount {
                extension: "pdf".into(),
                count: 2
            })
        );
        assert_eq!(
            answer("Combien de fichiers PDF ?", "fr-FR"),
            Some(FolderAnswer::ExtensionCount {
                extension: "pdf".into(),
                count: 2
            })
        );
    }

    #[test]
    fn unreadable_and_indexed_are_different_questions() {
        assert_eq!(
            answer("How many files are unreadable?", "en-US"),
            Some(FolderAnswer::UnreadableCount { count: 2 })
        );
        let Some(FolderAnswer::IndexedList { files }) = answer("Which files are indexed?", "en-US")
        else {
            panic!("expected an indexed listing");
        };
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn the_folder_structure_is_its_own_answer() {
        let Some(FolderAnswer::FolderTree { root, lines }) =
            answer("Show the folder structure.", "en-US")
        else {
            panic!("expected a tree");
        };

        assert_eq!(root, "cabinet");
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn listing_folders_is_its_own_question() {
        let inventory = WorkFolderInventory::from_records(
            Path::new("cabinet"),
            vec![
                record("2026/mars/biologie.pdf", Readability::Readable),
                record("administratif/assurance.txt", Readability::Readable),
            ],
        );

        let answer = match route(&inventory, &NoCorpus, "List the folders.", "en-US") {
            QuestionRoute::Deterministic(answer) => answer,
            other => panic!("expected a folder listing, got {other:?}"),
        };

        assert_eq!(
            answer,
            FolderAnswer::FolderList {
                folders: vec!["2026".into(), "2026/mars".into(), "administratif".into()]
            }
        );
    }

    #[test]
    fn asking_whether_a_named_file_was_read_is_answered_from_its_record() {
        // Both of these name a file, so they reach the resolver first; neither is a question
        // about what that file says, so neither may become a retrieval.
        for (question, locale) in [
            ("What is the status of biologie.pdf?", "en-US"),
            ("Is biologie.pdf indexed?", "en-US"),
            ("Quel statut pour biologie.pdf ?", "fr-FR"),
        ] {
            match route(&folder(), &NoCorpus, question, locale) {
                QuestionRoute::Deterministic(FolderAnswer::FileDetails { file }) => {
                    assert_eq!(file.name, "biologie.pdf", "{question}");
                    assert!(file.indexed);
                }
                other => panic!("expected file details for {question:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_question_about_every_document_covers_every_indexed_file() {
        // Five files, four of them readable. A summary question asks about all four, so the
        // inventory supplies the list rather than the ranker picking a favourite few.
        for (question, locale) in [
            ("Give me a summary of each document", "en-US"),
            ("Summarise every file", "en-US"),
            ("Un resume de chaque document", "fr-FR"),
            ("Resume tous mes documents", "fr-FR"),
        ] {
            match route(&folder(), &NoCorpus, question, locale) {
                QuestionRoute::PerDocumentRetrieval { files } => {
                    assert_eq!(files.len(), 3, "{question}");
                    assert!(files.iter().all(|file| file.indexed), "{question}");
                }
                other => panic!("expected per-document retrieval for {question:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn listing_files_is_still_a_listing_rather_than_a_request_to_read_them_all() {
        // "all" is a distributive word, so this is the case the ordering has to get right: a
        // filesystem question is recognised first and never becomes a retrieval.
        assert!(matches!(
            route(&folder(), &NoCorpus, "List all files.", "en-US"),
            QuestionRoute::Deterministic(FolderAnswer::FileList { .. })
        ));
        assert!(matches!(
            route(
                &folder(),
                &NoCorpus,
                "How many files are in the work folder?",
                "en-US"
            ),
            QuestionRoute::Deterministic(FolderAnswer::FileCount { .. })
        ));
    }

    #[test]
    fn a_distributive_word_without_a_document_word_changes_nothing() {
        // "each" appears in ordinary sentences. On its own it must not put the whole corpus in
        // scope, or every question would become an expensive one.
        assert_eq!(
            route(
                &folder(),
                &NoCorpus,
                "What was measured at each consultation?",
                "en-US"
            ),
            QuestionRoute::GlobalRetrieval
        );
    }

    #[test]
    fn a_content_question_is_left_to_retrieval() {
        // "metformin" is a word the documents contain, which is what makes this a question about
        // their contents rather than about the folder. Nothing else in the sentence says so.
        let corpus = Words(&["metformin", "glycemie", "camille"]);

        assert_eq!(
            route(
                &folder(),
                &corpus,
                "How many documents mention metformin?",
                "en-US"
            ),
            QuestionRoute::GlobalRetrieval
        );
        assert_eq!(
            route(
                &folder(),
                &corpus,
                "Quel taux de glycemie pour Camille ?",
                "fr-FR"
            ),
            QuestionRoute::GlobalRetrieval
        );
    }

    #[test]
    fn an_ordinary_phrasing_still_reaches_the_folder() {
        // Politely phrased, the way a person actually asks. None of the courtesy words were in
        // the pack before 23 September, and one of them was enough to send the question to a
        // model. The sentence reported from the workstation is exercised verbatim in
        // `tests/work_folder_inventory.rs`.
        let corpus = Words(&["metformine", "glycemie", "cephalees"]);

        assert!(matches!(
            route(
                &folder(),
                &corpus,
                "Peux-tu me donner tous mes documents disponibles ?",
                "fr-FR"
            ),
            QuestionRoute::Deterministic(FolderAnswer::IndexedList { .. })
        ));
        assert!(matches!(
            route(
                &folder(),
                &corpus,
                "Can you please give me a list of all the available files?",
                "en-US"
            ),
            QuestionRoute::Deterministic(FolderAnswer::FileList { .. })
        ));
    }

    #[test]
    fn a_word_that_means_read_this_never_becomes_a_listing() {
        // The failure the content list exists for: every other word in this sentence belongs to a
        // listing request, and answering it with a list of names would answer a question nobody
        // asked.
        for (question, locale) in [
            ("Give me a summary of each document", "en-US"),
            ("Resume tous mes documents", "fr-FR"),
            ("Que disent tous mes documents ?", "fr-FR"),
        ] {
            assert!(
                !matches!(
                    route(&folder(), &NoCorpus, question, locale),
                    QuestionRoute::Deterministic(FolderAnswer::IndexedList { .. })
                        | QuestionRoute::Deterministic(FolderAnswer::FileList { .. })
                ),
                "{question}"
            );
        }
    }

    #[test]
    fn a_file_is_anything_on_disk_and_a_document_is_one_that_could_be_read() {
        // Her own distinction, and the one a person means: five files, three of them readable.
        assert_eq!(
            answer("How many files are there?", "en-US"),
            Some(FolderAnswer::FileCount { total: 5 })
        );
        assert_eq!(
            answer("How many documents are there?", "en-US"),
            Some(FolderAnswer::IndexedCount { count: 3 })
        );
        assert_eq!(
            answer("Combien de fichiers ?", "fr-FR"),
            Some(FolderAnswer::FileCount { total: 5 })
        );
        assert_eq!(
            answer("Combien de documents ?", "fr-FR"),
            Some(FolderAnswer::IndexedCount { count: 3 })
        );
    }

    #[test]
    fn a_question_naming_a_readable_file_is_retrieved_inside_that_file() {
        match route(&folder(), &NoCorpus, "Que dit biologie.pdf ?", "fr-FR") {
            QuestionRoute::TargetedRetrieval { file } => {
                assert_eq!(file.relative_path, "biologie.pdf");
            }
            other => panic!("expected targeted retrieval, got {other:?}"),
        }
    }

    #[test]
    fn a_question_naming_an_unreadable_file_never_guesses_at_its_content() {
        assert!(matches!(
            route(&folder(), &NoCorpus, "What does scan.png say?", "en-US"),
            QuestionRoute::Deterministic(FolderAnswer::FileUnreadable { .. })
        ));
    }

    #[test]
    fn a_question_naming_a_file_that_is_absent_reports_that_and_retrieves_nothing() {
        assert_eq!(
            route(
                &folder(),
                &NoCorpus,
                "What does secret-report.pdf say?",
                "en-US"
            ),
            QuestionRoute::Deterministic(FolderAnswer::NoMatchingFile {
                query: "secret-report.pdf".into()
            })
        );
    }

    #[test]
    fn an_unknown_locale_still_answers_through_the_fallback_pack() {
        assert_eq!(
            answer("Combien de fichiers ?", "de-DE"),
            Some(FolderAnswer::FileCount { total: 5 })
        );
    }

    #[test]
    fn every_shipped_pack_parses() {
        for (tag, _) in PACKS {
            assert!(pack_for(tag).is_some(), "pack {tag} must parse");
        }
    }

    #[test]
    fn an_answer_carries_machine_codes_and_never_a_sentence() {
        let answer = answer("How many files are in the work folder?", "en-US").unwrap();
        let payload = serde_json::to_value(&answer).unwrap();

        assert_eq!(payload["kind"], "file_count");
        assert_eq!(payload["total"], 5);
    }
}
