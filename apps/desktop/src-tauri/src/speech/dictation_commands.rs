//! Voice commands of a note dictation: spoken punctuation, title, subtitle, new line, new
//! paragraph and new note (`docs/SPEECH.md`, phase 12).
//!
//! The words of a language are data (`dictation_commands` in `resources/speech/rules/<locale>.json`);
//! this file names no language. [`apply`] is a pure function of one transcribed passage, the rules
//! and the heading left open by the previous passage: no engine, no file, no network.
//!
//! Rules that do not move:
//!
//! * Matching is on folded text (accents, case and the engine's own punctuation around a word are
//!   ignored), longest phrase first, and only on whole words of the transcript.
//! * A punctuation command replaces the punctuation the engine already wrote next to it, so a mark
//!   is never doubled.
//! * A heading runs to the end of the sentence, to a new-line command or to the end of the passage,
//!   whichever comes first; a heading command with nothing after it in the passage stays open and
//!   is handed to the next one.
//! * A new line is a Markdown hard break (two blanks, then the newline) so the preview draws it; a
//!   new paragraph is a blank line.
//! * "New note" cuts the passage in two: the text before it belongs to the note being written, the
//!   text after it opens a new one.
//! * Nothing here changes a note on its own: the result is text for the editor, which appends it.

use serde::{Deserialize, Serialize};

use super::languages::Feature;
use super::speakable_rules::rules_for_feature;
use super::status::effective_locale;
use crate::error::AppError;
use crate::knowledge::normalize::fold;
use crate::settings::Settings;

/// Where a mark sits against its neighbours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Attach {
    /// Spaces on both sides.
    #[default]
    Neither,
    /// Glued to the word before it ("comma", "period", "closing bracket").
    Previous,
    /// Glued to the word after it ("opening bracket", "opening quote").
    Next,
    /// Glued on both sides.
    Both,
}

/// What a spoken phrase does.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    /// Writes a mark. `capitalise_next` starts the next word with a capital (after a full stop).
    Mark {
        text: String,
        #[serde(default)]
        attach: Attach,
        #[serde(default)]
        capitalise_next: bool,
    },
    /// Starts a Markdown heading of this level (1 to 6).
    Heading {
        level: u8,
    },
    Line,
    Paragraph,
    /// Closes the note being written and opens a new one.
    Note,
}

/// One spoken command: the phrases that say it and what it does.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Command {
    pub say: Vec<String>,
    #[serde(flatten)]
    pub action: Action,
}

/// The `dictation_commands` entry of a rule pack.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CommandRules {
    pub commands: Vec<Command>,
}

impl CommandRules {
    /// Structural checks that a hand-edited pack could break.
    pub fn validate(&self) -> Result<(), String> {
        for command in &self.commands {
            if command.say.is_empty()
                || command.say.iter().any(|phrase| words_of(phrase).is_empty())
            {
                return Err("a dictation command has no phrase".into());
            }
            if let Action::Heading { level } = command.action {
                if !(1..=6).contains(&level) {
                    return Err("a heading level must be between 1 and 6".into());
                }
            }
        }
        Ok(())
    }
}

/// What one passage becomes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    /// The text for the note being written, then one text for each new note asked for. Never empty.
    /// A text may start or end with line breaks: the editor that appends it joins them to its own.
    pub pieces: Vec<String>,
    /// The level of a heading command that ended the passage with nothing after it, 0 for none.
    /// The caller hands it back with the next passage.
    pub pending_heading: u8,
}

/// The passage as it is, for a dictation with the commands switched off.
pub fn untouched(text: &str) -> Applied {
    Applied {
        pieces: vec![text.trim().to_string()],
        pending_heading: 0,
    }
}

/// Punctuation that may sit around a word of the transcript and never counts for matching.
fn is_mark(character: char) -> bool {
    matches!(
        character,
        ',' | '.'
            | ';'
            | ':'
            | '!'
            | '?'
            | '"'
            | '('
            | ')'
            | '['
            | ']'
            | '\u{ab}'
            | '\u{bb}'
            | '\u{2026}'
    )
}

/// The punctuation an engine writes at the end of a word, replaced when a command writes its own.
fn is_trailing_mark(character: char) -> bool {
    matches!(character, ',' | '.' | ';' | ':' | '!' | '?' | '\u{2026}')
}

/// The folded words of a phrase: apostrophes of any shape are one, a hyphen separates words.
fn words_of(text: &str) -> Vec<String> {
    fold(&text.replace('\u{2019}', "'").replace('-', " "))
        .split_whitespace()
        .map(|word| word.trim_matches(is_mark).to_string())
        .filter(|word| !word.is_empty())
        .collect()
}

/// One word of the transcript, as matching sees it.
struct Piece {
    /// Index of the transcript word (a split on blanks) it belongs to.
    word: usize,
    folded: String,
    /// First and last piece of its transcript word: a command covers whole words only.
    first: bool,
    last: bool,
}

fn pieces_of(words: &[&str]) -> Vec<Piece> {
    let mut pieces = Vec::new();
    for (word, raw) in words.iter().enumerate() {
        let parts = words_of(raw);
        let total = parts.len();
        for (index, folded) in parts.into_iter().enumerate() {
            pieces.push(Piece {
                word,
                folded,
                first: index == 0,
                last: index + 1 == total,
            });
        }
    }
    pieces
}

/// A command found in the passage: the transcript words it covers, both included.
struct Found<'a> {
    from: usize,
    to: usize,
    action: &'a Action,
}

/// The commands of the passage, left to right, longest phrase first at each place.
fn find<'a>(words: &[&str], rules: &'a CommandRules) -> Vec<Found<'a>> {
    let pieces = pieces_of(words);
    let mut phrases: Vec<(Vec<String>, &Action)> = rules
        .commands
        .iter()
        .flat_map(|command| {
            command
                .say
                .iter()
                .map(move |phrase| (words_of(phrase), &command.action))
        })
        .collect();
    phrases.sort_by_key(|(phrase, _)| std::cmp::Reverse(phrase.len()));
    let mut found = Vec::new();
    let mut at = 0;
    while at < pieces.len() {
        let matched = phrases.iter().find(|(phrase, _)| {
            let end = at + phrase.len();
            end <= pieces.len()
                && pieces[at].first
                && pieces[end - 1].last
                && phrase
                    .iter()
                    .zip(&pieces[at..end])
                    .all(|(want, piece)| *want == piece.folded)
        });
        match matched {
            Some((phrase, action)) => {
                let end = at + phrase.len();
                found.push(Found {
                    from: pieces[at].word,
                    to: pieces[end - 1].word,
                    action,
                });
                at = end;
            }
            None => at += 1,
        }
    }
    found
}

/// The word with its first letter in capitals.
fn capitalised(word: &str) -> String {
    let mut characters = word.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => String::new(),
    }
}

/// Whether the engine ended a sentence with this word.
fn ends_sentence(word: &str) -> bool {
    word.trim_end_matches(['"', ')', '\u{bb}'])
        .ends_with(['.', '?', '!', '\u{2026}'])
}

/// A growing text with the spacing rules of the marks. Line breaks may open the text: the editor
/// that appends it knows what comes before, and joins the two.
struct Writer {
    text: String,
    /// The next word follows an opening mark: no blank in between.
    glued: bool,
    capitalise: bool,
    /// Level of the heading being written, 0 for none.
    heading: u8,
    /// Where the blank lines in front of that heading begin in `text`.
    heading_from: usize,
}

impl Writer {
    fn new() -> Self {
        Self {
            text: String::new(),
            glued: false,
            capitalise: false,
            heading: 0,
            heading_from: 0,
        }
    }

    fn at_line_start(&self) -> bool {
        self.text.is_empty() || self.text.ends_with('\n')
    }

    fn separator(&self) -> &'static str {
        if self.at_line_start() || self.glued {
            ""
        } else {
            " "
        }
    }

    fn word(&mut self, raw: &str) {
        let word = if self.capitalise {
            capitalised(raw)
        } else {
            raw.to_string()
        };
        self.text.push_str(self.separator());
        self.text.push_str(&word);
        self.glued = false;
        self.capitalise = false;
    }

    fn mark(&mut self, text: &str, attach: Attach, capitalise_next: bool) {
        match attach {
            Attach::Previous | Attach::Both => {
                // The engine's own punctuation at this place is replaced, never doubled.
                let kept = self.text.trim_end_matches(is_trailing_mark).len();
                self.text.truncate(kept);
                self.text.push_str(text);
            }
            Attach::Neither | Attach::Next => {
                self.text.push_str(self.separator());
                self.text.push_str(text);
            }
        }
        self.glued = matches!(attach, Attach::Next | Attach::Both);
        self.capitalise = capitalise_next;
    }

    /// Trailing blanks of the line removed, then newlines added until `count` of them end the text.
    fn newlines_to(&mut self, count: usize) {
        let kept = self.text.trim_end_matches([' ', '\t']).len();
        self.text.truncate(kept);
        let present = self.text.len() - self.text.trim_end_matches('\n').len();
        for _ in present..count {
            self.text.push('\n');
        }
        self.glued = false;
        self.capitalise = true;
    }

    /// Ends the heading in progress, dropping the full stop that closes it.
    fn close_heading(&mut self) {
        if self.heading != 0 {
            if self.text.trim_end().ends_with('#') {
                // A heading with no word: nothing of it is kept.
                self.text.truncate(self.heading_from);
            } else {
                let kept = self.text.trim_end_matches(['.', ' ']).len();
                self.text.truncate(kept);
            }
            self.heading = 0;
        }
    }

    /// The heading ends where its sentence ends: the line is closed, the next word starts below.
    fn end_heading_line(&mut self) {
        self.close_heading();
        self.newlines_to(1);
    }

    /// A line break inside a paragraph: Markdown only draws it when two blanks come before the
    /// newline (a hard break), so that is what is written, except after a heading (whose line ends
    /// by itself), at the start of a text and when a line already ends there.
    fn line_break(&mut self) {
        let after_heading = self.heading != 0;
        self.close_heading();
        self.newlines_to(1);
        let body = self.text.strip_suffix('\n').unwrap_or("");
        if !after_heading && !body.is_empty() && !body.ends_with('\n') {
            let keep = body.len();
            self.text.truncate(keep);
            self.text.push_str("  \n");
        }
    }

    /// Blank lines: `count` newlines end the text.
    fn paragraph_break(&mut self, count: usize) {
        self.close_heading();
        self.newlines_to(count);
    }

    fn start_heading(&mut self, level: u8) {
        self.close_heading();
        self.newlines_to(2);
        self.heading_from = self.text.trim_end_matches('\n').len();
        for _ in 0..level {
            self.text.push('#');
        }
        self.text.push(' ');
        self.glued = true;
        self.capitalise = true;
        self.heading = level;
    }

    /// The text, and the level of a heading that was started and has no word yet (whose marker is
    /// then removed: the next passage opens it again).
    fn finish(mut self) -> (String, u8) {
        let empty = self.heading != 0 && self.text.trim_end().ends_with('#');
        if empty {
            let level = self.heading;
            self.text.truncate(self.heading_from);
            return (self.text.trim_end().to_string(), level);
        }
        if self.heading != 0 {
            self.close_heading();
            self.text.push('\n');
        }
        (self.text.trim_end_matches([' ', '\t']).to_string(), 0)
    }
}

/// Applies the commands of `rules` to one passage. `pending_heading` is the level a heading command
/// left open at the end of the previous passage, 0 for none.
pub fn apply(text: &str, rules: &CommandRules, pending_heading: u8) -> Applied {
    let words: Vec<&str> = text.split_whitespace().collect();
    let found = find(&words, rules);
    let mut pieces: Vec<String> = Vec::new();
    let mut writer = Writer::new();
    if (1..=6).contains(&pending_heading) {
        writer.start_heading(pending_heading);
    }
    let mut cursor = 0;
    let mut index = 0;
    while index < words.len() {
        if cursor < found.len() && found[cursor].from == index {
            let command = &found[cursor];
            cursor += 1;
            index = command.to + 1;
            match command.action {
                Action::Mark {
                    text,
                    attach,
                    capitalise_next,
                } => {
                    writer.mark(text, *attach, *capitalise_next);
                    if *capitalise_next && writer.heading != 0 {
                        writer.end_heading_line();
                    }
                }
                Action::Heading { level } => writer.start_heading(*level),
                Action::Line => writer.line_break(),
                Action::Paragraph => writer.paragraph_break(2),
                Action::Note => {
                    let finished = std::mem::replace(&mut writer, Writer::new());
                    let (piece, _) = finished.finish();
                    pieces.push(piece.trim_end().to_string());
                }
            }
            continue;
        }
        let word = words[index];
        let in_heading = writer.heading != 0;
        writer.word(word);
        index += 1;
        if in_heading && ends_sentence(word) {
            writer.end_heading_line();
        }
    }
    let (last, open) = writer.finish();
    pieces.push(last);
    Applied {
        pieces,
        pending_heading: open,
    }
}

/// The commands for one passage under the user's settings: the language is the one dictation works
/// in (the override, else the interface language), never one the window names. With the setting
/// off, or a language whose pack has no commands, the passage comes back as it is. A language the
/// table does not ship for dictation answers `speech_locale_unsupported`.
pub fn apply_for_settings(
    settings: &Settings,
    text: &str,
    pending_heading: u8,
) -> Result<Applied, AppError> {
    if !settings.speech.notes_voice_commands {
        return Ok(untouched(text));
    }
    let rules = rules_for_feature(&effective_locale(settings), Feature::Dictation)?;
    Ok(match &rules.dictation_commands {
        Some(commands) => apply(text, commands, pending_heading),
        None => untouched(text),
    })
}
