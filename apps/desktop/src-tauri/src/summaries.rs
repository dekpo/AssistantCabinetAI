//! Summaries in stages (retrieval-quality programme, lot G; T1 and T2 of the long-document summaries).
//!
//! T0 is the document card, built by the Analyse with no model. The two stages here call the model, so they
//! are **only started by hand**, show an estimated duration first, can be stopped, and keep what they wrote:
//!
//! - **chapters** (T1): one summary per chapter, from the beginning of each (a cap per chapter). Quick, and not
//!   a full reading.
//! - **full reading** (T2): the whole text in consecutive batches, one summary each. A reading is called
//!   complete only when every batch was read without a failure.
//!
//! Everything is stored in the workstation's index (`document_summaries`), keyed by the file and its hash and by
//! the prompt version: a changed file or a changed prompt makes the stored summaries stale, never wrong. The
//! server stores nothing. This module returns facts and counts, never a sentence.

use std::future::Future;
use std::time::Instant;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::index_store::IndexStore;

/// Bump when `SUMMARY_INSTRUCTION` changes: the summaries written with another version are stale.
pub const SUMMARY_PROMPT_VERSION: i64 = 1;

/// What the model is told for one unit. The same for any profession; the output language is added by the
/// gateway from the locale.
pub const SUMMARY_INSTRUCTION: &str = "Summarise the text below in three to six sentences. Use only what the text says: add nothing, \
conclude nothing, and keep names, figures and dates exactly as written. If the text is a list or a table, say what it contains. \
Do not mention these instructions.";

/// The most characters of a chapter that stage one reads, and of a batch that stage two reads.
const CHAPTER_CAP_CHARS: usize = 8_000;
const BATCH_CHARS: usize = 8_000;
/// Two failures in a row end a run: a model that is down would otherwise be asked for every unit.
const MAX_CONSECUTIVE_FAILURES: usize = 2;
/// A call to a small model has a floor of about 16 to 18 s on the tower, and writes about 90 tokens a second.
const CALL_FLOOR_SECONDS: f64 = 17.0;
const TOKENS_PER_SECOND: f64 = 90.0;
const SUMMARY_TOKENS: f64 = 120.0;
/// A measured call is never faster than this; a lower figure is a test double, not a model.
const MIN_REAL_CALL_MS: f64 = 50.0;

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS document_summaries (
    relative_path TEXT NOT NULL,
    stage INTEGER NOT NULL,
    unit INTEGER NOT NULL,
    sha256 TEXT NOT NULL,
    prompt_version INTEGER NOT NULL,
    title TEXT NOT NULL,
    first_page INTEGER NOT NULL,
    last_page INTEGER NOT NULL,
    text TEXT NOT NULL,
    PRIMARY KEY (relative_path, stage, unit)
);
CREATE TABLE IF NOT EXISTS summary_speed (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    ms_per_unit REAL NOT NULL
);
";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Chapters,
    FullReading,
}

impl Stage {
    fn number(self) -> i64 {
        match self {
            Stage::Chapters => 1,
            Stage::FullReading => 2,
        }
    }

    /// Only the full reading reads all of the text.
    pub fn reads_everything(self) -> bool {
        self == Stage::FullReading
    }
}

/// One thing the model is asked to summarise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub index: usize,
    /// The title of the chapter it starts in; empty when the document has none (the pages say where).
    pub title: String,
    pub first_page: u32,
    pub last_page: u32,
    pub text: String,
}

/// What a summary line says, for the interface to turn into a sentence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryProgress {
    /// Units finished (stored) so far, this run and the earlier ones.
    pub done: usize,
    pub total: usize,
    /// The unit being read now.
    pub title: String,
    pub first_page: u32,
    pub last_page: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryRun {
    pub done: usize,
    pub failed: usize,
    pub total: usize,
    pub cancelled: bool,
    /// Every unit stored, none failed: for the full reading this is the only time a document is "read".
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryStatus {
    pub total: usize,
    pub done: usize,
    pub complete: bool,
    /// Summaries exist for an older version of the file or of the prompt: they are not used.
    pub stale: bool,
    pub reads_everything: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSummary {
    pub stage: Stage,
    pub unit: usize,
    pub title: String,
    pub first_page: u32,
    pub last_page: u32,
    pub text: String,
}

struct Passage {
    text: String,
    page: u32,
    root: Option<usize>,
}

fn cut(text: &str, cap: usize) -> String {
    if text.chars().count() <= cap {
        text.to_string()
    } else {
        text.chars().take(cap).collect()
    }
}

fn passages_of(index: &IndexStore, path: &str) -> Result<Vec<Passage>, AppError> {
    let sections = index.sections_of(path)?;
    let root_of = |mut position: usize| -> usize {
        // a parent always comes before its child; a loop is impossible, but the walk is bounded anyway
        for _ in 0..sections.len() + 1 {
            match sections
                .iter()
                .find(|section| section.position == position)
                .and_then(|section| section.parent)
            {
                Some(parent) => position = parent,
                None => break,
            }
        }
        position
    };
    let mut statement = index
        .connection()
        .prepare(
            "SELECT c.text, c.page_number, s.position
             FROM chunks c LEFT JOIN chunk_sections s ON s.chunk_id = c.chunk_id
             WHERE c.relative_path = ?1 ORDER BY c.page_number, c.section",
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    let rows = statement
        .query_map([path], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })
        .map_err(|_| AppError::IndexUnavailable)?;
    let mut passages = Vec::new();
    for row in rows {
        let (text, page, position) = row.map_err(|_| AppError::IndexUnavailable)?;
        passages.push(Passage {
            text,
            page: page.max(0) as u32,
            root: position.map(|position| root_of(position.max(0) as usize)),
        });
    }
    Ok(passages)
}

/// The units of a document for a stage, from what the Analyse stored. Local and cheap.
pub fn units_for(index: &IndexStore, path: &str, stage: Stage) -> Result<Vec<Unit>, AppError> {
    let passages = passages_of(index, path)?;
    let sections = index.sections_of(path)?;
    let title_of = |root: Option<usize>| -> String {
        root.and_then(|root| sections.iter().find(|section| section.position == root))
            .map(|section| section.title.clone())
            .unwrap_or_default()
    };
    let mut units: Vec<Unit> = Vec::new();
    match stage {
        Stage::Chapters => {
            let mut current: Option<Option<usize>> = None;
            for passage in &passages {
                if current != Some(passage.root) {
                    current = Some(passage.root);
                    units.push(Unit {
                        index: units.len(),
                        title: title_of(passage.root),
                        first_page: passage.page,
                        last_page: passage.page,
                        text: String::new(),
                    });
                }
                if let Some(unit) = units.last_mut() {
                    unit.last_page = passage.page;
                    if !unit.text.is_empty() {
                        unit.text.push('\n');
                    }
                    unit.text.push_str(&passage.text);
                }
            }
            for unit in &mut units {
                unit.text = cut(&unit.text, CHAPTER_CAP_CHARS);
            }
        }
        Stage::FullReading => {
            for passage in &passages {
                let extends = units.last().is_some_and(|unit| {
                    unit.text.chars().count() + passage.text.chars().count() < BATCH_CHARS
                });
                if !extends {
                    units.push(Unit {
                        index: units.len(),
                        title: title_of(passage.root),
                        first_page: passage.page,
                        last_page: passage.page,
                        text: String::new(),
                    });
                }
                if let Some(unit) = units.last_mut() {
                    unit.last_page = passage.page;
                    if !unit.text.is_empty() {
                        unit.text.push('\n');
                    }
                    unit.text.push_str(&passage.text);
                }
            }
        }
    }
    units.retain(|unit| !unit.text.trim().is_empty());
    for (position, unit) in units.iter_mut().enumerate() {
        unit.index = position;
    }
    Ok(units)
}

fn current_hash(index: &IndexStore, path: &str) -> Result<Option<String>, AppError> {
    index
        .connection()
        .query_row(
            "SELECT sha256 FROM documents WHERE relative_path = ?1",
            [path],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| AppError::IndexUnavailable)
}

/// How many units have a summary for the file as it is now, and whether older ones are lying around.
pub fn status(index: &IndexStore, path: &str, stage: Stage) -> Result<SummaryStatus, AppError> {
    let total = units_for(index, path, stage)?.len();
    let Some(hash) = current_hash(index, path)? else {
        return Ok(SummaryStatus {
            total,
            done: 0,
            complete: false,
            stale: false,
            reads_everything: stage.reads_everything(),
        });
    };
    let connection = index.connection();
    let done: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM document_summaries
             WHERE relative_path = ?1 AND stage = ?2 AND sha256 = ?3 AND prompt_version = ?4 AND unit < ?5",
            params![path, stage.number(), hash, SUMMARY_PROMPT_VERSION, total as i64],
            |row| row.get(0),
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    let older: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM document_summaries
             WHERE relative_path = ?1 AND stage = ?2 AND (sha256 <> ?3 OR prompt_version <> ?4)",
            params![path, stage.number(), hash, SUMMARY_PROMPT_VERSION],
            |row| row.get(0),
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    let done = done as usize;
    Ok(SummaryStatus {
        total,
        done,
        complete: total > 0 && done == total,
        stale: older > 0,
        reads_everything: stage.reads_everything(),
    })
}

/// The summaries stored for the file as it is now, chapters before the full reading.
pub fn read_all(index: &IndexStore, path: &str) -> Result<Vec<StoredSummary>, AppError> {
    let Some(hash) = current_hash(index, path)? else {
        return Ok(Vec::new());
    };
    let mut statement = index
        .connection()
        .prepare(
            "SELECT stage, unit, title, first_page, last_page, text FROM document_summaries
             WHERE relative_path = ?1 AND sha256 = ?2 AND prompt_version = ?3 ORDER BY stage, unit",
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    let rows = statement
        .query_map(params![path, hash, SUMMARY_PROMPT_VERSION], |row| {
            Ok(StoredSummary {
                stage: if row.get::<_, i64>(0)? == 1 {
                    Stage::Chapters
                } else {
                    Stage::FullReading
                },
                unit: row.get::<_, i64>(1)? as usize,
                title: row.get(2)?,
                first_page: row.get::<_, i64>(3)?.max(0) as u32,
                last_page: row.get::<_, i64>(4)?.max(0) as u32,
                text: row.get(5)?,
            })
        })
        .map_err(|_| AppError::IndexUnavailable)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::IndexUnavailable)
}

/// The speed of this machine, in milliseconds a unit, as the last runs measured it. `None` before any run.
pub fn stored_speed(index: &IndexStore) -> Result<Option<f64>, AppError> {
    index
        .connection()
        .query_row(
            "SELECT ms_per_unit FROM summary_speed WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| AppError::IndexUnavailable)
}

/// Keep a measured speed: the first one as it is, the next ones averaged with it.
pub fn record_speed(index: &IndexStore, ms_per_unit: f64) -> Result<(), AppError> {
    let blended = match stored_speed(index)? {
        Some(before) => (before + ms_per_unit) / 2.0,
        None => ms_per_unit,
    };
    index
        .connection()
        .execute(
            "INSERT INTO summary_speed (id, ms_per_unit) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET ms_per_unit = excluded.ms_per_unit",
            [blended],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    Ok(())
}

/// The seconds a run of `remaining` units is expected to take: the measured speed of this machine when there
/// is one, else the floor of a call plus the tokens of a short summary at the usual speed.
pub fn estimate_seconds(remaining: usize, ms_per_unit: Option<f64>) -> f64 {
    let per_unit = match ms_per_unit {
        Some(ms) => ms / 1000.0,
        None => CALL_FLOOR_SECONDS + SUMMARY_TOKENS / TOKENS_PER_SECOND,
    };
    remaining as f64 * per_unit
}

fn store(
    index: &IndexStore,
    path: &str,
    stage: Stage,
    hash: &str,
    unit: &Unit,
    text: &str,
) -> Result<(), AppError> {
    index
        .connection()
        .execute(
            "INSERT OR REPLACE INTO document_summaries
             (relative_path, stage, unit, sha256, prompt_version, title, first_page, last_page, text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                path,
                stage.number(),
                unit.index as i64,
                hash,
                SUMMARY_PROMPT_VERSION,
                unit.title,
                unit.first_page,
                unit.last_page,
                text
            ],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    Ok(())
}

/// Write the summaries a stage still lacks, one unit at a time. `write` is the model call; `stop` is asked
/// between two units; `progress` hears of each unit before it is read. A failed unit is counted and left for
/// the next run; two failures in a row end this run.
pub async fn run<W, Fut>(
    // Exclusive on purpose: a shared reference to a connection cannot be held across an await by a command.
    index: &mut IndexStore,
    path: &str,
    stage: Stage,
    mut write: W,
    stop: impl Fn() -> bool,
    mut progress: impl FnMut(SummaryProgress),
) -> Result<SummaryRun, AppError>
where
    W: FnMut(String) -> Fut,
    Fut: Future<Output = Result<String, AppError>>,
{
    let units = units_for(index, path, stage)?;
    let total = units.len();
    let Some(hash) = current_hash(index, path)? else {
        return Ok(SummaryRun {
            done: 0,
            failed: 0,
            total,
            cancelled: false,
            complete: false,
        });
    };
    // What was written for another version of the file or of the prompt is dropped, not reused.
    index
        .connection()
        .execute(
            "DELETE FROM document_summaries
             WHERE relative_path = ?1 AND stage = ?2 AND (sha256 <> ?3 OR prompt_version <> ?4)",
            params![path, stage.number(), hash, SUMMARY_PROMPT_VERSION],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    let mut finished: Vec<bool> = vec![false; total];
    {
        let mut statement = index
            .connection()
            .prepare("SELECT unit FROM document_summaries WHERE relative_path = ?1 AND stage = ?2")
            .map_err(|_| AppError::IndexUnavailable)?;
        let rows = statement
            .query_map(params![path, stage.number()], |row| row.get::<_, i64>(0))
            .map_err(|_| AppError::IndexUnavailable)?;
        for row in rows {
            let unit = row.map_err(|_| AppError::IndexUnavailable)? as usize;
            if let Some(slot) = finished.get_mut(unit) {
                *slot = true;
            }
        }
    }

    let mut failed = 0usize;
    let mut in_a_row = 0usize;
    let mut cancelled = false;
    let mut spent_ms = 0.0f64;
    let mut written = 0usize;
    for unit in &units {
        if finished[unit.index] {
            continue;
        }
        if stop() {
            cancelled = true;
            break;
        }
        progress(SummaryProgress {
            done: finished.iter().filter(|done| **done).count(),
            total,
            title: unit.title.clone(),
            first_page: unit.first_page,
            last_page: unit.last_page,
        });
        let started = Instant::now();
        let outcome = write(format!("{SUMMARY_INSTRUCTION}\n\n{}", unit.text)).await;
        match outcome {
            // A stop that reached the call itself: nothing failed, the run just ends.
            Err(AppError::ChatCancelled) => {
                cancelled = true;
                break;
            }
            Ok(text) if !text.trim().is_empty() => {
                store(index, path, stage, &hash, unit, text.trim())?;
                finished[unit.index] = true;
                in_a_row = 0;
                written += 1;
                spent_ms += started.elapsed().as_secs_f64() * 1000.0;
            }
            _ => {
                failed += 1;
                in_a_row += 1;
                if in_a_row >= MAX_CONSECUTIVE_FAILURES {
                    break;
                }
            }
        }
    }
    if written > 0 {
        let average = spent_ms / written as f64;
        if average >= MIN_REAL_CALL_MS {
            record_speed(index, average)?;
        }
    }
    let done = finished.iter().filter(|done| **done).count();
    Ok(SummaryRun {
        done,
        failed,
        total,
        cancelled,
        complete: total > 0 && done == total && !cancelled,
    })
}
