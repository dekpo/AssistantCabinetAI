//! Read-aloud text preparation (phase 7, decision D8): turns what is on screen into segments a voice
//! can speak, in Rust, by code, never by the model. Nothing here reaches a model or a server: a
//! source scan proves it.
//!
//! The pipeline, per part:
//!
//! 1. [`speakable_markdown::blocks`]: Markdown into lines, notes for what is not read;
//! 2. [`speakable_markdown::clean_inline`]: marks, addresses, emoji and paths removed or reduced;
//! 3. [`speakable_sentences::split_sentences`]: abbreviation-aware sentences, with a length cap;
//! 4. identifiers of the knowledge packs (health insurance number, IBAN, invoice, e-mail) read in
//!    groups, then [`speakable_scan::spoken_form`]: numbers, dates, times, units, titles.
//!
//! The words of a language come from its rule pack (`resources/speech/rules/`), the titles and the
//! identifier patterns from the knowledge packs. Nothing here names a language, and the text on
//! screen is never changed: the segments are a copy.

use std::ops::Range;

use serde::{Deserialize, Serialize};

use super::languages::{self, Feature};
use super::speakable_markdown::{blocks, clean_inline, Block};
use super::speakable_numbers::{digit_word, digits_one_by_one};
use super::speakable_rules::{rules_for, IdentifierReading, Rules};
use super::speakable_scan::spoken_form;
use super::speakable_sentences::split_sentences;
use crate::error::AppError;
use crate::knowledge::packs::PackSet;
use crate::settings::Settings;

pub use super::speakable_markdown::NoteCode;

/// How a part is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PartKind {
    Markdown,
    Plain,
}

/// One piece of what is read: the answer text, the computed block, a caution line.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Part {
    pub kind: PartKind,
    pub text: String,
}

/// How long the voice rests after a segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Pause {
    /// The sentence goes on in the next segment (it was cut for its length).
    Clause,
    Sentence,
    Paragraph,
}

/// One item of a reading: a segment to speak, or a note standing for what is not read. A note is a
/// machine code and a count; the interface says it in the user's language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Segment {
    Speech {
        text: String,
        #[serde(rename = "pauseAfter")]
        pause_after: Pause,
    },
    Note {
        code: NoteCode,
        count: usize,
    },
}

/// Read-aloud runs only when the voice master switch and the read-aloud switch are on.
pub fn reading_allowed(settings: &Settings) -> Result<(), AppError> {
    if settings.speech.enabled && settings.speech.read_aloud_enabled {
        Ok(())
    } else {
        Err(AppError::SpeechUnavailable)
    }
}

/// The segments of `parts` for `locale`, with the knowledge packs `pack_ids` active. A locale the
/// language table does not ship for read-aloud answers `speech_locale_unsupported`.
pub fn prepare_for_locale(
    parts: &[Part],
    locale: &str,
    pack_ids: &[&str],
) -> Result<Vec<Segment>, AppError> {
    let row = languages::resolve(locale, Feature::ReadAloud)?;
    let rules = rules_for(row.id)?;
    let packs = PackSet::load(row.id, pack_ids)?;
    Ok(prepare(parts, rules, &packs))
}

/// The segments of `parts`: every part in order, a pause between parts.
pub fn prepare(parts: &[Part], rules: &Rules, packs: &PackSet) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for part in parts {
        for block in blocks(&part.text, part.kind == PartKind::Markdown) {
            match block {
                Block::Note { code, count } => out.push(Segment::Note { code, count }),
                Block::Text {
                    text,
                    paragraph_end,
                } => push_text(&mut out, &text, paragraph_end, rules, packs),
            }
        }
        raise_pause(&mut out, Pause::Paragraph);
    }
    out
}

fn push_text(
    out: &mut Vec<Segment>,
    text: &str,
    paragraph_end: bool,
    rules: &Rules,
    packs: &PackSet,
) {
    let cleaned = clean_inline(text);
    let sentences = split_sentences(&cleaned, rules, packs);
    let count = sentences.len();
    for (index, sentence) in sentences.into_iter().enumerate() {
        let last = index + 1 == count;
        let pause = if sentence.continues {
            Pause::Clause
        } else if sentence.paragraph_end || (last && paragraph_end) {
            Pause::Paragraph
        } else {
            Pause::Sentence
        };
        let mut spoken = speak_sentence(&sentence.text, rules, packs);
        if !spoken.chars().any(char::is_alphanumeric) {
            raise_pause(out, pause);
            continue;
        }
        // A line with no end mark (a heading, a list item) gets one, so the voice closes it.
        if !sentence.continues && spoken.ends_with(char::is_alphanumeric) {
            spoken.push('.');
        }
        out.push(Segment::Speech {
            text: spoken,
            pause_after: pause,
        });
    }
    if paragraph_end {
        raise_pause(out, Pause::Paragraph);
    }
}

/// Makes the pause after the last spoken segment at least `pause`.
fn raise_pause(out: &mut [Segment], pause: Pause) {
    if let Some(Segment::Speech { pause_after, .. }) = out.last_mut() {
        if *pause_after < pause {
            *pause_after = pause;
        }
    }
}

// -------------------------------------------------------------------------------------------------
// One sentence
// -------------------------------------------------------------------------------------------------

/// One sentence as words: identifiers of the knowledge packs in groups, the rest through the scanner.
pub fn speak_sentence(text: &str, rules: &Rules, packs: &PackSet) -> String {
    let found = identifier_spans(text, rules, packs);
    if found.is_empty() {
        return spoken_form(text, rules, packs);
    }
    let mut out = String::new();
    let mut cursor = 0usize;
    let mut after_identifier = false;
    for (range, spoken) in found {
        append_gap(
            &mut out,
            &text[cursor..range.start],
            &mut after_identifier,
            rules,
            packs,
        );
        if out.ends_with(char::is_alphanumeric) {
            out.push(' ');
        }
        out.push_str(&spoken);
        after_identifier = true;
        cursor = range.end;
    }
    append_gap(
        &mut out,
        &text[cursor..],
        &mut after_identifier,
        rules,
        packs,
    );
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn append_gap(
    out: &mut String,
    gap: &str,
    after_identifier: &mut bool,
    rules: &Rules,
    packs: &PackSet,
) {
    if gap.is_empty() {
        return;
    }
    let spoken = spoken_form(gap, rules, packs);
    if gap.starts_with(char::is_whitespace)
        || (*after_identifier && spoken.starts_with(char::is_alphanumeric))
    {
        out.push(' ');
    }
    out.push_str(&spoken);
    if gap.ends_with(char::is_whitespace) {
        out.push(' ');
    }
    *after_identifier = false;
}

/// The identifiers the active knowledge packs find in `text` that the rule pack knows how to read,
/// without overlaps, with their spoken form.
fn identifier_spans(text: &str, rules: &Rules, packs: &PackSet) -> Vec<(Range<usize>, String)> {
    let mut found: Vec<(Range<usize>, String)> = Vec::new();
    for scheme in packs.identifier_schemes() {
        let reading = rules
            .identifiers
            .readings
            .get(&scheme.id)
            .or_else(|| rules.identifiers.readings.get(&scheme.kind));
        let is_email = scheme.kind == "email";
        if reading.is_none() && !is_email {
            continue;
        }
        for matched in scheme.scan(text) {
            let written = &text[matched.range.clone()];
            let spoken = match reading {
                Some(reading) => read_in_groups(written, reading, rules),
                None => read_address(written, rules),
            };
            found.push((matched.range, spoken));
        }
    }
    found.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(b.0.end.cmp(&a.0.end)));
    let mut kept: Vec<(Range<usize>, String)> = Vec::new();
    for candidate in found {
        if kept
            .last()
            .is_none_or(|last| candidate.0.start >= last.0.end)
        {
            kept.push(candidate);
        }
    }
    kept
}

/// Groups of characters, each said one by one: the groups the text is written in, or else cut by
/// the lengths the pack gives.
fn read_in_groups(written: &str, reading: &IdentifierReading, rules: &Rules) -> String {
    let written_groups: Vec<&str> = written
        .split([' ', '.', '-', '/', '\u{a0}', '\u{202f}'])
        .filter(|group| !group.is_empty())
        .collect();
    let groups: Vec<String> = if written_groups.len() >= 2 {
        written_groups
            .iter()
            .flat_map(|group| cut_by_size(group, reading.size))
            .collect()
    } else {
        let compact: String = written.chars().filter(|c| c.is_alphanumeric()).collect();
        cut_by_chunks(&compact, reading)
    };
    groups
        .iter()
        .map(|group| spell_group(group, rules))
        .collect::<Vec<_>>()
        .join(", ")
}

fn cut_by_size(group: &str, size: usize) -> Vec<String> {
    let chars: Vec<char> = group.chars().collect();
    if size == 0 || chars.len() <= size {
        return vec![group.to_string()];
    }
    chars
        .chunks(size)
        .map(|piece| piece.iter().collect())
        .collect()
}

fn cut_by_chunks(compact: &str, reading: &IdentifierReading) -> Vec<String> {
    let chars: Vec<char> = compact.chars().collect();
    let mut groups: Vec<String> = Vec::new();
    let mut position = 0usize;
    for length in &reading.chunks {
        if position >= chars.len() {
            break;
        }
        let end = (position + length).min(chars.len());
        groups.push(chars[position..end].iter().collect());
        position = end;
    }
    if position < chars.len() {
        let rest: String = chars[position..].iter().collect();
        groups.extend(cut_by_size(&rest, reading.size));
    }
    groups
}

/// A group said character by character: digits as digit words, letters by the names of the pack.
fn spell_group(group: &str, rules: &Rules) -> String {
    group
        .chars()
        .filter_map(|c| {
            if c.is_ascii_digit() {
                Some(digit_word(rules, c).to_string())
            } else if c.is_alphabetic() {
                let key = c.to_lowercase().to_string();
                Some(rules.letters.get(&key).cloned().unwrap_or(key))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// An e-mail address: its words as they are, its marks said with the words of the pack.
fn read_address(written: &str, rules: &Rules) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut word = String::new();
    for c in written.chars() {
        if c.is_alphanumeric() {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            parts.push(read_address_word(&word, rules));
            word.clear();
        }
        if let Some(spoken) = rules.identifiers.email_symbols.get(&c.to_string()) {
            parts.push(spoken.clone());
        }
    }
    if !word.is_empty() {
        parts.push(read_address_word(&word, rules));
    }
    parts.join(" ")
}

/// A word of an address keeps its letters; a run of digits is said digit by digit.
fn read_address_word(word: &str, rules: &Rules) -> String {
    if word.chars().all(|c| c.is_ascii_digit()) {
        digits_one_by_one(rules, word)
    } else {
        word.to_string()
    }
}
