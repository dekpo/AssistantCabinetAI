//! The JSON-lines protocol between the application and a speech worker (decision D2), and the loop
//! a worker runs over it.
//!
//! One source, two crates: the application reads this file as a module, and the worker crate
//! (`apps/speech-worker`) includes the very same file with `#[path]`, so the two sides can never
//! drift apart. For that it depends on `serde` and `serde_json` only: no Tauri, no `AppError`, no
//! other module of the application.
//!
//! One request per line on the worker's standard input, one response per line on its standard
//! output, in order, each carrying the `id` of the request it answers. A job is cancelled by
//! killing the worker (a decode cannot be interrupted from inside), so there is no cancel
//! message; closing the standard input ends the worker, which is how it dies with the
//! application. The worker never writes a line of its own to a log: what it heard is the one
//! thing it must not repeat anywhere.

use std::io::{BufRead, Write};

use serde::{Deserialize, Serialize};

/// The four files of a transducer model, as full paths. The application names them from its
/// manifest; the worker does not know a file name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFiles {
    pub encoder: String,
    pub decoder: String,
    pub joiner: String,
    pub tokens: String,
}

/// What the application asks of a worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// Reads the model into memory. Slow (about two seconds), so it is asked once per worker.
    Load {
        id: u64,
        threads: u32,
        files: ModelFiles,
    },
    /// Transcribes the clip at `wav_path`, a canonical WAV the application wrote in its private
    /// temporary folder. The worker reads it and never keeps it.
    Transcribe {
        id: u64,
        wav_path: String,
        language: String,
    },
    /// Ends the worker. It sends nothing back.
    Shutdown { id: u64 },
}

/// Why a request failed, as a kind the application maps to its own machine codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    /// A model file is not there.
    ModelMissing,
    /// The model files are there but the engine cannot load them.
    ModelCorrupt,
    /// The clip cannot be read as audio.
    AudioInvalid,
    /// Anything else the engine did wrong, including a request that came before `load`.
    EngineFailed,
    /// The line was not a request this worker understands.
    BadRequest,
}

/// What a worker answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Loaded {
        id: u64,
        load_ms: u64,
    },
    Transcribed {
        id: u64,
        text: String,
        audio_ms: u64,
        processing_ms: u64,
    },
    Failed {
        id: u64,
        kind: FailureKind,
    },
}

impl Response {
    pub fn id(&self) -> u64 {
        match self {
            Self::Loaded { id, .. } | Self::Transcribed { id, .. } | Self::Failed { id, .. } => *id,
        }
    }
}

/// One clip's result, as an engine hands it to the loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heard {
    pub text: String,
    pub audio_ms: u64,
    pub processing_ms: u64,
}

/// What a worker's engine does. The real one wraps sherpa-onnx; the tests of the application run
/// the same loop over a scripted one.
pub trait Engine {
    /// Loads the model and returns how long it took, in milliseconds.
    fn load(&mut self, threads: u32, files: &ModelFiles) -> Result<u64, FailureKind>;

    /// Transcribes one clip. `language` is the engine code from the application's language table;
    /// an engine that detects the language itself ignores it.
    fn transcribe(&mut self, wav_path: &str, language: &str) -> Result<Heard, FailureKind>;
}

/// Encodes a request as one line, without the newline.
pub fn encode_request(request: &Request) -> String {
    serde_json::to_string(request).unwrap_or_default()
}

/// Encodes a response as one line, without the newline.
pub fn encode_response(response: &Response) -> String {
    serde_json::to_string(response).unwrap_or_default()
}

/// Reads one response line. A line that is not a response (a stray line from a native library) is
/// `None` and is skipped by the caller, never logged: it could hold what was heard.
pub fn decode_response(line: &str) -> Option<Response> {
    let line = line.trim();
    if !line.starts_with('{') {
        return None;
    }
    serde_json::from_str(line).ok()
}

/// Runs the worker loop: reads requests from `input` until `Shutdown` or the end of the input,
/// answers each on `output`, and flushes after every answer. Returns when the worker should exit.
pub fn serve<R: BufRead, W: Write>(
    input: R,
    mut output: W,
    engine: &mut dyn Engine,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(Request::Shutdown { .. }) => return Ok(()),
            Ok(Request::Load { id, threads, files }) => match engine.load(threads, &files) {
                Ok(load_ms) => Response::Loaded { id, load_ms },
                Err(kind) => Response::Failed { id, kind },
            },
            Ok(Request::Transcribe {
                id,
                wav_path,
                language,
            }) => match engine.transcribe(&wav_path, &language) {
                Ok(heard) => Response::Transcribed {
                    id,
                    text: heard.text,
                    audio_ms: heard.audio_ms,
                    processing_ms: heard.processing_ms,
                },
                Err(kind) => Response::Failed { id, kind },
            },
            Err(_) => Response::Failed {
                id: 0,
                kind: FailureKind::BadRequest,
            },
        };
        writeln!(output, "{}", encode_response(&response))?;
        output.flush()?;
    }
    Ok(())
}
