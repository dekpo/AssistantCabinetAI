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
//! * [`notes`]: saves reviewed notes as new files in the notes folder. The only file this module
//!   writes, only on the user's click, and it needs no engine and no audio.

pub mod languages;
pub mod notes;
pub mod settings;
pub mod status;

#[cfg(test)]
mod notes_tests;
#[cfg(test)]
mod tests;

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
