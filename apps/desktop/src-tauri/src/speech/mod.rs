//! Local voice: dictation, read-aloud and voice notes (`docs/SPEECH.md`).
//!
//! This module is the base every later speech phase stands on, and it does no work of its own: no
//! engine, no audio, no model, no network, no thread (the one file it writes is a saved note, see below). Nothing here runs at launch (the
//! application has no `.setup()` hook, and `speech_commands::SpeechState::new` only builds empty
//! values). The engines live in separate worker executables and arrive in later phases; the words of
//! a language live in data packs under `resources/speech/`, never in code.
//!
//! * [`settings`]: the nested `speech` field of `settings.json`, clamped on the way in and out.
//! * [`languages`]: the one table that maps an interface locale to an engine language code, rule
//!   packs, capabilities and default voices. No other file names a language.
//! * [`status`]: a pure function that says, per feature, whether it can run and, if not, why - as a
//!   machine code the interface localises.
//! * [`notes`]: saves reviewed notes as new files in the notes folder. The only file of this module
//!   that survives a job, written only on the user's click, and it needs no engine and no audio.
//! * [`stt`], [`service`], [`wav`], [`temp_audio`], [`output_guard`]: the speech-to-text contract
//!   and everything around the engine. The service runs a job (one at a time, cancellable, under a
//!   time limit); the clip lives in a private temporary folder for that job only. Without a worker
//!   executable the service answers `speech_engine_missing`; the scripted fake of the tests is
//!   never built by a command.
//! * [`worker_protocol`], [`worker_stt`], [`console_window`]: the speech-to-text engine, out of
//!   process. The protocol is one file shared by source with the worker crate
//!   (`apps/speech-worker`); the provider starts the worker on the first dictation, ends it after an
//!   idle delay and kills it on a cancel; the console window module owns the one platform detail.
//! * [`repair`]: shapes the name suggestions that follow a dictation (read only, selection only, names
//!   withheld while the setting hides them) over `knowledge::repair`.
//! * [`dictation_commands`]: the spoken commands of a note dictation (punctuation, title, new line, new
//!   note), applied by code to a transcribed passage; the words are data in the rule packs.
//! * [`models`], [`model_download`]: the compiled-in manifest of the models, what is on disk, and the
//!   one file of the module that touches the network, on an explicit user action only.
//! * [`speakable`] and its `speakable_*` files: turns an answer into segments a voice can speak,
//!   in Rust, by code, never by the model: Markdown reader, number, date, time and unit words from
//!   the rule packs of `resources/speech/rules/`, titles and identifiers from the knowledge packs.
//!   No engine, no audio and no network: a pure function of its arguments.

pub mod console_window;
pub mod dictation_commands;
pub mod languages;
pub mod model_download;
pub mod models;
pub mod notes;
pub mod output_guard;
pub mod repair;
pub mod service;
pub mod settings;
pub mod speakable;
pub mod speakable_markdown;
pub mod speakable_numbers;
pub mod speakable_rules;
pub mod speakable_scan;
pub mod speakable_sentences;
pub mod status;
pub mod stt;
pub mod temp_audio;
pub mod timing_log;
pub mod wav;
pub mod worker_protocol;
pub mod worker_stt;

#[cfg(test)]
mod dictation_commands_tests;
#[cfg(test)]
mod fake;
#[cfg(test)]
mod model_download_tests;
#[cfg(test)]
mod models_tests;
#[cfg(test)]
mod notes_tests;
#[cfg(test)]
mod speakable_tests;
#[cfg(test)]
mod stt_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod worker_stt_tests;

/// Shortest cap of one chat dictation, in seconds. Below this a recording is hardly a sentence.
pub const MIN_DICTATION_SECONDS: u32 = 10;
/// Longest cap of one chat dictation, in seconds (10 minutes). The text lands in a text box that
/// the user reads before sending, so a longer cap only delays the moment she can check it.
pub const MAX_DICTATION_SECONDS: u32 = 600;
/// Shortest cap of one note dictation, in seconds.
pub const MIN_NOTE_SECONDS: u32 = 60;
/// Longest cap of one note dictation, in seconds (1 hour).
pub const MAX_NOTE_SECONDS: u32 = 3_600;

/// Slowest read-aloud speed, as a multiple of the voice's natural rate. Passed to the synthesiser
/// as its own speed, never as a playback rate, which would change the pitch.
pub const MIN_TTS_RATE: f32 = 0.5;
/// Fastest read-aloud speed, as a multiple of the voice's natural rate.
pub const MAX_TTS_RATE: f32 = 2.0;
/// Natural speed of a voice.
pub const DEFAULT_TTS_RATE: f32 = 1.0;

/// Longest identifier kept for a voice, in characters. Manifest ids are short ASCII slugs.
pub const MAX_VOICE_ID_CHARS: usize = 64;
/// Longest locale tag kept for the language override, in characters (BCP 47 tags are short).
pub const MAX_LOCALE_CHARS: usize = 16;
