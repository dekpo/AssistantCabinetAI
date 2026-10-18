//! The timing line of a transcription, written to `speech-timings.jsonl` in the application's local
//! data folder when `write_timing_log` is on. Engine, purpose and numbers only: never the text that
//! was heard, a name or a path. A log that cannot be written never fails the job it describes.

use std::path::Path;

use serde::Serialize;

use super::output_guard;
use super::service::Purpose;
use super::stt::SttResult;
use crate::knowledge::diagnostics::append_timing_line;

/// The file name, under the application's local data folder.
pub const LOG_FILE_NAME: &str = "speech-timings.jsonl";

/// What one line holds. Every field is a number or a short code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimingEntry {
    pub purpose: &'static str,
    pub engine: String,
    pub audio_ms: u64,
    pub processing_ms: u64,
    pub words: usize,
}

/// The entry of one finished job. The words are counted, not kept.
pub fn entry(purpose: Purpose, result: &SttResult) -> TimingEntry {
    TimingEntry {
        purpose: match purpose {
            Purpose::Dictation => "dictation",
            Purpose::Note => "note",
        },
        engine: result.engine_id.clone(),
        audio_ms: result.audio_ms,
        processing_ms: result.processing_ms,
        words: output_guard::word_count(&result.text),
    }
}

/// Appends the entry of a job to the log in `directory`, creating the folder and the file if needed.
pub fn append(
    directory: &Path,
    purpose: Purpose,
    result: &SttResult,
    at_seconds: i64,
) -> std::io::Result<()> {
    append_timing_line(
        &directory.join(LOG_FILE_NAME),
        &entry(purpose, result),
        at_seconds,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard() -> SttResult {
        SttResult {
            text: "a secret sentence of six words".into(),
            audio_ms: 4_200,
            processing_ms: 610,
            engine_id: "engine-x".into(),
        }
    }

    #[test]
    fn speech_timing_line_holds_no_text() {
        let folder = tempfile::tempdir().expect("temp");

        append(folder.path(), Purpose::Dictation, &heard(), 1_760_000_000).expect("writes");

        let written = std::fs::read_to_string(folder.path().join(LOG_FILE_NAME)).expect("reads");
        assert_eq!(
            written.trim_end(),
            r#"{"at":1760000000,"purpose":"dictation","engine":"engine-x","audioMs":4200,"processingMs":610,"words":6}"#
        );
        assert!(!written.contains("secret"));
        assert!(!written.contains("sentence"));
    }

    #[test]
    fn a_second_job_adds_a_line_and_a_note_says_so() {
        let folder = tempfile::tempdir().expect("temp");

        append(folder.path(), Purpose::Dictation, &heard(), 1).expect("writes");
        append(folder.path(), Purpose::Note, &heard(), 2).expect("writes");

        let written = std::fs::read_to_string(folder.path().join(LOG_FILE_NAME)).expect("reads");
        let lines: Vec<&str> = written.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains(r#""purpose":"note""#));
    }
}
