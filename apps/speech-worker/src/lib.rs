//! The shared parts of the speech workers: the protocol the application speaks (the very file the
//! application compiles, included here by path) and the reader of the canonical clip.
//!
//! No network code, no file write and no log: a worker reads one clip, answers on its standard
//! output and keeps nothing.

#[path = "../../desktop/src-tauri/src/speech/worker_protocol.rs"]
pub mod protocol;

pub mod wav;

#[cfg(feature = "stt")]
pub mod engine;
