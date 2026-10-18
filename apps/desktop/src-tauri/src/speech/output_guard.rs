//! A last look at what an engine returned, before it reaches a text box.
//!
//! Engines fail quietly: on silence they invent a sentence, on noise they loop on one word, on a
//! broken decode they return nothing or a wall of text. This guard rejects those outputs with
//! `speech_output_rejected`, so the interface says "nothing was heard" instead of showing text the
//! user never said. SpeechLab never built it (its issue I-035 is open).
//!
//! Every threshold is a named constant with its unit. They are provisional: phase 3b calibrates
//! them on real clips before they are trusted.
//!
//! Pure functions over a string and two numbers: no file, no thread, no network.

use crate::error::AppError;

/// Fewest words per second of audio that still reads as speech (one word in ten seconds).
pub const MIN_WORDS_PER_SECOND: f32 = 0.1;
/// Most words per second of audio that a person can say; above it the engine is looping or lying.
pub const MAX_WORDS_PER_SECOND: f32 = 8.0;
/// An output shorter than this many words is not checked for repetition: "no no" is an answer.
pub const MIN_WORDS_FOR_REPETITION_CHECK: usize = 8;
/// Largest share of the words that one single word may make up (0 to 1).
pub const MAX_SINGLE_WORD_SHARE: f32 = 0.5;
/// How many times in a row one group of up to [`MAX_REPEATED_GROUP_WORDS`] words may repeat.
pub const MAX_CONSECUTIVE_REPEATS: usize = 4;
/// Longest group of words looked for in a repetition loop.
pub const MAX_REPEATED_GROUP_WORDS: usize = 4;

/// Words of a text, lower case, without the punctuation around them.
fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| {
            word.trim_matches(|character: char| !character.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect()
}

/// The longest run of one group of `size` words repeated back to back, counted in repeats.
fn longest_run(words: &[String], size: usize) -> usize {
    let mut best = 1;
    for start in 0..words.len().saturating_sub(size) {
        let mut repeats = 1;
        let mut next = start + size;
        while next + size <= words.len() && words[start..start + size] == words[next..next + size] {
            repeats += 1;
            next += size;
        }
        best = best.max(repeats);
    }
    best
}

fn is_runaway_repetition(words: &[String]) -> bool {
    if words.len() < MIN_WORDS_FOR_REPETITION_CHECK {
        return false;
    }
    let mut counts = std::collections::HashMap::new();
    for word in words {
        *counts.entry(word.as_str()).or_insert(0usize) += 1;
    }
    let most = counts.values().copied().max().unwrap_or(0);
    if most as f32 / words.len() as f32 > MAX_SINGLE_WORD_SHARE {
        return true;
    }
    (1..=MAX_REPEATED_GROUP_WORDS).any(|size| longest_run(words, size) >= MAX_CONSECUTIVE_REPEATS)
}

/// Accepts or rejects an engine's output. `audio_ms` is the length of the clip as measured from its
/// samples; `quiet` is whether those samples were flagged too quiet (any text for such audio is
/// invented, even when the service already refuses quiet clips before the engine runs).
pub fn check(text: &str, audio_ms: u64, quiet: bool) -> Result<(), AppError> {
    if quiet || text.trim().is_empty() {
        return Err(AppError::SpeechOutputRejected);
    }
    let words = words(text);
    if words.is_empty() || audio_ms == 0 {
        return Err(AppError::SpeechOutputRejected);
    }
    let per_second = words.len() as f32 / (audio_ms as f32 / 1_000.0);
    if !(MIN_WORDS_PER_SECOND..=MAX_WORDS_PER_SECOND).contains(&per_second) {
        return Err(AppError::SpeechOutputRejected);
    }
    if is_runaway_repetition(&words) {
        return Err(AppError::SpeechOutputRejected);
    }
    Ok(())
}

/// The number of words the guard counts, for the timing line (never the words themselves).
pub fn word_count(text: &str) -> usize {
    words(text).len()
}
