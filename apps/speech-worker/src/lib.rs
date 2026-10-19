//! The shared parts of the speech workers: the protocol the application speaks (the very file the
//! application compiles, included here by path) and the reader of the canonical clip.
//!
//! No network code, no log and no file written except the one audio file a synthesis is asked to
//! produce: a worker reads one clip or speaks one text, answers on its standard output and keeps
//! nothing.

#[path = "../../desktop/src-tauri/src/speech/worker_protocol.rs"]
pub mod protocol;

pub mod wav;

#[cfg(feature = "stt")]
pub mod engine;

#[cfg(feature = "tts")]
pub mod voice;
