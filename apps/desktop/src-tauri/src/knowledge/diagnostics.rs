//! Where the seconds go: the timings of one question and of one Analyse pass.
//!
//! Written before anything is optimised, because the slowness the owner saw was not obviously the
//! kind a Knowledge Base cures. At question time the only embedding call is the question's, the
//! chunk vectors are computed at Analyse and the prompt is capped, so the cost may well sit in
//! reading a workbook, in the model's first token, or in the embedding of a long document. This
//! module makes each of those a number (`docs/test-reports/knowledge-base-pass-1/00-baseline.md`).
//!
//! **Numbers and machine codes only.** Nothing here can hold a question, a file name, an entity or
//! an excerpt: the structs have no string field that is not a code, and the slowest files of a
//! pass are identified by their position in the pass, never by name (`AGENTS.md`: a log is not a
//! second place where a document's content lives; `docs/PRIVACY-AND-SECURITY.md`). The timing log
//! is off by default (`Settings::write_timing_log`).

use std::cell::Cell;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;

/// How many of the slowest files a pass keeps, by position in the pass.
pub const SLOWEST_FILES_KEPT: usize = 5;

/// A duration in whole milliseconds, saturating rather than wrapping.
pub fn elapsed_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// A stopwatch that reads either the time since it started or the time since its last lap.
///
/// `std::time::Instant` and nothing else: no logging crate, no global. One per question and one
/// per pass; each stage takes a lap, so the stages add up to the total by construction.
pub struct StageTimer {
    started: Instant,
    last_lap: Instant,
}

impl StageTimer {
    pub fn start() -> Self {
        let now = Instant::now();
        Self {
            started: now,
            last_lap: now,
        }
    }

    /// Milliseconds since the previous lap (or the start), and the next lap begins now.
    pub fn lap_ms(&mut self) -> u64 {
        let now = Instant::now();
        let lap = elapsed_ms(now.duration_since(self.last_lap));
        self.last_lap = now;
        lap
    }

    /// Milliseconds since the timer started. Does not move the lap.
    pub fn total_ms(&self) -> u64 {
        elapsed_ms(self.started.elapsed())
    }
}

/// Run `work` and return its result with the milliseconds it took.
pub fn timed_ms<T>(work: impl FnOnce() -> T) -> (T, u64) {
    let started = Instant::now();
    let result = work();
    (result, elapsed_ms(started.elapsed()))
}

/// Which way a question was answered. Machine codes, serialised as `snake_case`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalPath {
    /// Documents selected: retrieval, then a sourced answer.
    #[default]
    Documents,
    /// Tables selected, no document: the tabular engine alone.
    Tabular,
    /// Documents and tables selected, answered across both or by the engine alone.
    Mixed,
    /// A question about the folder itself, answered from the inventory.
    FolderAnswer,
    /// No document selected: answered from the conversation alone.
    WithoutDocuments,
}

/// What retrieval was asked to cover (`Plan` in `commands::sourced_answer`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalPlan {
    OneFile,
    EveryDocument,
    WholeFolder,
}

/// The timings and counts of one question. Returned with the answer and, when
/// `write_timing_log` is on, appended to `retrieval-timings.jsonl`.
///
/// Every `*_ms` is the wall time of one stage; a stage the question did not go through stays 0. A
/// stage that is **not** a separate step on a path says so on its field (the tabular path has no
/// embedding, for instance). `generation_first_token_ms` is `None` when no model wrote anything.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalDiagnostics {
    pub path: RetrievalPath,
    pub plan: Option<RetrievalPlan>,
    /// Documents in the selection that are analysed and usable.
    pub selected_documents: usize,
    /// Workbooks in the selection.
    pub selected_workbooks: usize,
    /// Stored chunks the search scored (the scope's chunks, before the cut).
    pub chunks_considered: usize,
    /// Excerpts that reached the prompt.
    pub chunks_selected: usize,
    /// Characters of excerpts sent to the model, a stand-in for the prompt's weight. Never the
    /// text itself.
    pub estimated_input_chars: usize,
    /// Discovering the folder and joining it onto the index.
    pub inventory_ms: u64,
    /// Resolving the selection against the inventory.
    pub scope_ms: u64,
    /// Deciding what the question is: the folder router on the documents path, the classifier and
    /// the computation on the tabular and mixed paths (`prepare`).
    pub routing_ms: u64,
    /// The one embedding call for the question.
    pub embedding_ms: u64,
    /// Lexical and vector search, the cut to the caps.
    pub search_ms: u64,
    /// Reading the selected workbooks: from the typed cache, or from the file on a miss.
    pub workbook_load_ms: u64,
    /// Every workbook this question read came from the typed cache. `false` when none was read.
    pub workbook_cache_hit: bool,
    /// From sending the prompt to the first word of the answer. `None` without generation.
    pub generation_first_token_ms: Option<u64>,
    /// From sending the prompt to the end of the answer (the tabular path: the model-assist step,
    /// which only spends time when a model is asked).
    pub generation_total_ms: u64,
    pub total_ms: u64,
}

impl RetrievalDiagnostics {
    pub fn new(path: RetrievalPath) -> Self {
        Self {
            path,
            ..Self::default()
        }
    }

    /// Fold in the workbooks a synchronous stretch of the question just read.
    pub fn absorb_workbook_loads(&mut self, loads: WorkbookLoads) {
        self.workbook_load_ms += loads.total_ms;
        if loads.loads > 0 {
            self.workbook_cache_hit = loads.cache_hits == loads.loads;
        }
    }
}

/// What the typed workbook cache did during a stretch of work: how many workbooks were read, how
/// many of those came from the cache, and the time all of them took.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkbookLoads {
    pub loads: usize,
    pub cache_hits: usize,
    pub total_ms: u64,
}

thread_local! {
    static WORKBOOK_LOADS: Cell<WorkbookLoads> = const {
        Cell::new(WorkbookLoads { loads: 0, cache_hits: 0, total_ms: 0 })
    };
}

/// Called by the workbook loader each time it returns a workbook.
///
/// A thread-local rather than a parameter, on purpose: the loader sits several calls below the
/// code that owns a question's diagnostics, and every one of those calls is synchronous, so the
/// load and the `take_workbook_loads` that follows run in the same poll on the same thread. The
/// counter is therefore read straight after the stretch that filled it, never across an `.await`.
pub fn record_workbook_load(elapsed: Duration, cache_hit: bool) {
    WORKBOOK_LOADS.with(|cell| {
        let mut loads = cell.get();
        loads.loads += 1;
        loads.cache_hits += usize::from(cache_hit);
        loads.total_ms += elapsed_ms(elapsed);
        cell.set(loads);
    });
}

/// Read and clear what this thread's loader has recorded since the last call.
pub fn take_workbook_loads() -> WorkbookLoads {
    WORKBOOK_LOADS.with(|cell| cell.replace(WorkbookLoads::default()))
}

/// Which folder a pass analysed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisPass {
    #[default]
    Documents,
    Data,
}

/// One of the slowest files of a pass. Identified by its **position** in the pass, never by name:
/// the position is enough to find it in a folder the owner knows, and a log must not become a list
/// of file names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTiming {
    pub position: usize,
    pub total_ms: u64,
    pub extract_ms: u64,
    pub ocr_ms: u64,
    pub chunk_ms: u64,
    pub embed_ms: u64,
    pub write_ms: u64,
    pub chunks: usize,
}

/// The timings of one Analyse pass: stage totals over every file, plus the slowest few.
///
/// The Documents pass fills `hash_ms` to `write_ms` and the chunk counts; the Data pass fills
/// `build_inventory_ms` and `write_ms`. A stage a pass does not have stays 0. `extract_ms`
/// excludes `ocr_ms`, which is measured around the engine itself, so the two add up to the time
/// spent reading files.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisTimings {
    pub pass: AnalysisPass,
    pub files_scanned: usize,
    /// Files actually read in this pass: not skipped as unchanged.
    pub files_processed: usize,
    pub hash_ms: u64,
    pub extract_ms: u64,
    pub ocr_ms: u64,
    pub chunk_ms: u64,
    pub embed_ms: u64,
    pub embed_batches: usize,
    /// Characters sent to the embedding model in this pass.
    pub embedded_chars: usize,
    pub write_ms: u64,
    pub build_inventory_ms: u64,
    /// Chunks produced in this pass (unchanged files are not chunked again).
    pub chunks_total: usize,
    /// Of those, chunks whose text is byte-identical to an earlier chunk of the same pass: the
    /// letterheads, signatures and boilerplate a vector cache would save embedding twice.
    pub chunks_repeating_earlier_text: usize,
    pub total_ms: u64,
    /// The slowest files, slowest first. Position in the pass, not a name.
    pub slowest_files: Vec<FileTiming>,
    #[serde(skip)]
    seen_text: HashSet<u64>,
}

impl AnalysisTimings {
    pub fn new(pass: AnalysisPass, files_scanned: usize) -> Self {
        Self {
            pass,
            files_scanned,
            ..Self::default()
        }
    }

    /// Count a chunk, and whether its text was already seen earlier in this pass. Only a hash of
    /// the text is kept, for the length of the pass, and nothing of it is serialised.
    pub fn note_chunk_text(&mut self, text: &str) {
        self.chunks_total += 1;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        if !self.seen_text.insert(hasher.finish()) {
            self.chunks_repeating_earlier_text += 1;
        }
    }

    /// Offer one processed file to the slowest list.
    pub fn note_file(&mut self, file: FileTiming) {
        self.slowest_files.push(file);
        self.slowest_files.sort_by(|a, b| {
            b.total_ms
                .cmp(&a.total_ms)
                .then(a.position.cmp(&b.position))
        });
        self.slowest_files.truncate(SLOWEST_FILES_KEPT);
    }
}

/// A timing line as it is written: the timestamp, then the entry's own fields.
#[derive(Serialize)]
struct TimingLine<'a, T: Serialize> {
    at: i64,
    #[serde(flatten)]
    entry: &'a T,
}

/// Append one entry to a JSON-lines timing log, creating the folder and the file if needed.
///
/// The same pattern as `filename_sanitizer::append_log` for `renamed-files.jsonl`. A log that
/// cannot be written never fails the question or the pass it describes, so the caller ignores the
/// error. The file is not rotated: a line is a few hundred bytes and the switch is off by default,
/// so it is the owner's to delete when a measurement is done.
pub fn append_timing_line<T: Serialize>(
    log_path: &Path,
    entry: &T,
    at_seconds: i64,
) -> std::io::Result<()> {
    if let Some(directory) = log_path.parent() {
        std::fs::create_dir_all(directory)?;
    }
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let line = serde_json::to_string(&TimingLine {
        at: at_seconds,
        entry,
    })
    .map_err(std::io::Error::other)?;
    writeln!(log, "{line}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::Chunk;
    use crate::extraction::PageOrigin;
    use crate::index_store::IndexStore;
    use crate::retrieval;

    #[test]
    fn laps_add_up_and_never_exceed_the_total() {
        let mut timer = StageTimer::start();
        std::thread::sleep(Duration::from_millis(15));
        let first = timer.lap_ms();
        std::thread::sleep(Duration::from_millis(15));
        let second = timer.lap_ms();
        let total = timer.total_ms();

        assert!(first >= 10, "the first lap measured {first} ms");
        assert!(second >= 10, "the second lap measured {second} ms");
        assert!(first + second <= total, "{first} + {second} > {total}");
    }

    #[test]
    fn timed_ms_returns_the_result_and_the_time() {
        let (value, elapsed) = timed_ms(|| {
            std::thread::sleep(Duration::from_millis(12));
            41 + 1
        });

        assert_eq!(value, 42);
        assert!(elapsed >= 10);
    }

    #[test]
    fn a_huge_duration_saturates_instead_of_wrapping() {
        assert_eq!(elapsed_ms(Duration::MAX), u64::MAX);
    }

    #[test]
    fn diagnostics_serialise_in_camel_case_with_snake_case_codes() {
        let mut diagnostics = RetrievalDiagnostics::new(RetrievalPath::FolderAnswer);
        diagnostics.plan = Some(RetrievalPlan::EveryDocument);
        diagnostics.selected_documents = 3;
        diagnostics.generation_first_token_ms = Some(1_250);

        let json = serde_json::to_value(&diagnostics).unwrap();

        assert_eq!(json["path"], "folder_answer");
        assert_eq!(json["plan"], "every_document");
        assert_eq!(json["selectedDocuments"], 3);
        assert_eq!(json["generationFirstTokenMs"], 1_250);
        assert_eq!(json["workbookCacheHit"], false);
        assert!(json["totalMs"].is_number());
        assert!(json.get("selected_documents").is_none());
    }

    #[test]
    fn a_question_without_generation_has_no_first_token() {
        let json = serde_json::to_value(RetrievalDiagnostics::default()).unwrap();

        assert!(json["generationFirstTokenMs"].is_null());
        assert!(json["plan"].is_null());
    }

    #[test]
    fn workbook_loads_are_read_once_and_cleared() {
        let _ = take_workbook_loads();
        record_workbook_load(Duration::from_millis(30), true);
        record_workbook_load(Duration::from_millis(50), false);

        let loads = take_workbook_loads();

        assert_eq!(loads.loads, 2);
        assert_eq!(loads.cache_hits, 1);
        assert!(loads.total_ms >= 80);
        assert_eq!(take_workbook_loads(), WorkbookLoads::default());
    }

    #[test]
    fn the_cache_hit_flag_means_every_workbook_came_from_the_cache() {
        let mut diagnostics = RetrievalDiagnostics::default();
        diagnostics.absorb_workbook_loads(WorkbookLoads {
            loads: 2,
            cache_hits: 2,
            total_ms: 4,
        });
        assert!(diagnostics.workbook_cache_hit);

        let mut mixed = RetrievalDiagnostics::default();
        mixed.absorb_workbook_loads(WorkbookLoads {
            loads: 2,
            cache_hits: 1,
            total_ms: 900,
        });
        assert!(!mixed.workbook_cache_hit);
        assert_eq!(mixed.workbook_load_ms, 900);

        let mut none = RetrievalDiagnostics::default();
        none.absorb_workbook_loads(WorkbookLoads::default());
        assert!(!none.workbook_cache_hit, "no workbook read is not a hit");
    }

    #[test]
    fn the_slowest_files_are_kept_slowest_first_and_capped() {
        let mut timings = AnalysisTimings::new(AnalysisPass::Documents, 9);
        for (position, total_ms) in [40, 900, 10, 700, 700, 5, 300, 800, 60]
            .into_iter()
            .enumerate()
        {
            timings.note_file(FileTiming {
                position,
                total_ms,
                ..FileTiming::default()
            });
        }

        let kept: Vec<(usize, u64)> = timings
            .slowest_files
            .iter()
            .map(|file| (file.position, file.total_ms))
            .collect();

        assert_eq!(
            kept,
            vec![(1, 900), (7, 800), (3, 700), (4, 700), (6, 300)],
            "slowest first, the earlier position first on a tie"
        );
    }

    #[test]
    fn repeated_chunk_text_is_counted_and_never_kept() {
        let mut timings = AnalysisTimings::new(AnalysisPass::Documents, 3);
        timings.note_chunk_text("Practice letterhead, 12 Lilac Street");
        timings.note_chunk_text("Resultats du bilan sanguin");
        timings.note_chunk_text("Practice letterhead, 12 Lilac Street");
        timings.note_chunk_text("Practice letterhead, 12 Lilac Street");

        assert_eq!(timings.chunks_total, 4);
        assert_eq!(timings.chunks_repeating_earlier_text, 2);

        let json = serde_json::to_string(&timings).unwrap();
        assert!(!json.contains("Lilas"), "no chunk text in {json}");
        assert!(!json.contains("seenText"), "the hash set is not serialised");
    }

    /// The acceptance criterion of lot 0: a diagnostics value built from a populated store holds
    /// numbers and codes and nothing else. File names and passages are planted that no field could
    /// legitimately hold, and the serialised line is searched for them.
    #[test]
    fn a_timing_line_built_from_a_populated_store_holds_only_numbers_and_codes() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = IndexStore::open_at(&directory.path().join("index.sqlite3")).unwrap();
        for (path, text) in [
            (
                "clients/Martine-Dupont-bilan.pdf",
                "Madame Martine Dupont, 0612345678",
            ),
            (
                "clients/lettre-secrete.pdf",
                "Rendez-vous confidentiel avec Monsieur Roy",
            ),
        ] {
            let chunk = Chunk {
                chunk_id: format!("{path}#p1#s1"),
                relative_path: path.to_string(),
                page_number: 1,
                section: 1,
                text: text.to_string(),
                origin: PageOrigin::TextLayer,
                confidence: None,
            };
            store
                .replace_document(path, "hash", false, &[chunk], &[vec![1.0, 0.0]], None, None)
                .unwrap();
        }
        let evidence = retrieval::search(&store, "Martine Dupont", &[1.0, 0.0]).unwrap();
        assert!(!evidence.is_empty(), "the fixture must retrieve something");

        let mut diagnostics = RetrievalDiagnostics::new(RetrievalPath::Documents);
        diagnostics.plan = Some(RetrievalPlan::WholeFolder);
        diagnostics.selected_documents = 2;
        diagnostics.chunks_selected = evidence.len();
        diagnostics.estimated_input_chars = evidence.iter().map(|item| item.text.len()).sum();
        diagnostics.total_ms = 1_800;
        let mut analysis = AnalysisTimings::new(AnalysisPass::Documents, 2);
        analysis.note_chunk_text(&evidence[0].text);
        analysis.note_file(FileTiming {
            position: 1,
            total_ms: 700,
            ..FileTiming::default()
        });

        let log = directory.path().join("timings.jsonl");
        append_timing_line(&log, &diagnostics, 1_760_000_000).unwrap();
        append_timing_line(&log, &analysis, 1_760_000_001).unwrap();
        let written = std::fs::read_to_string(&log).unwrap();

        for planted in [
            "Martine",
            "Dupont",
            "Roy",
            "0612345678",
            "lettre-secrete",
            "clients/",
            ".pdf",
        ] {
            assert!(
                !written.contains(planted),
                "{planted:?} leaked into {written}"
            );
        }
        let codes = ["documents", "whole_folder", "data"];
        for line in written.lines() {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_only_numbers_and_codes(&value, &codes);
        }
        assert_eq!(written.lines().count(), 2);
    }

    fn assert_only_numbers_and_codes(value: &serde_json::Value, codes: &[&str]) {
        match value {
            serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            }
            serde_json::Value::String(code) => {
                assert!(codes.contains(&code.as_str()), "unexpected string {code:?}")
            }
            serde_json::Value::Array(items) => items
                .iter()
                .for_each(|item| assert_only_numbers_and_codes(item, codes)),
            serde_json::Value::Object(fields) => fields
                .values()
                .for_each(|field| assert_only_numbers_and_codes(field, codes)),
        }
    }

    #[test]
    fn a_timing_line_starts_with_its_timestamp_and_appends() {
        let directory = tempfile::tempdir().unwrap();
        let log = directory
            .path()
            .join("nested")
            .join("retrieval-timings.jsonl");
        let mut diagnostics = RetrievalDiagnostics::new(RetrievalPath::Tabular);

        diagnostics.total_ms = 5;
        append_timing_line(&log, &diagnostics, 100).unwrap();
        diagnostics.total_ms = 9;
        append_timing_line(&log, &diagnostics, 200).unwrap();

        let lines: Vec<serde_json::Value> = std::fs::read_to_string(&log)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["at"], 100);
        assert_eq!(lines[0]["path"], "tabular");
        assert_eq!(lines[0]["totalMs"], 5);
        assert_eq!(lines[1]["at"], 200);
        assert_eq!(lines[1]["totalMs"], 9);
    }
}
