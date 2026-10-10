//! The deterministic text extractor: names, organisations and identifiers in the text of a document.
//!
//! It reads the chunks the index just stored (or is about to store), never the original file, and
//! returns **candidates**: a surface, a type, how it was found, where. Whether a candidate is a
//! person the knowledge base already holds is the resolver's decision (`knowledge::resolve`); this
//! file only reads.
//!
//! Sources of candidates, in the order they win when two of them claim the same words:
//!
//! ```text
//! 0  gazetteer, a full name     a name the knowledge base already holds, in full        0.90
//! 1  organisation suffix        "Dupont SARL"                                           0.80
//! 2  organisation prefix        "Association Friends Of Music"                          0.70 (candidate)
//! 3  title and name             "Dr Jean Dupont", "Dr Martin"                           0.85 / 0.70
//! 5  untitled capitalised run   two or three capitalised words, not opening a sentence  0.40 (candidate)
//! 6  gazetteer, a short form    a surname or an initial the same file also names in full 0.70
//! -  identifiers                an invoice number, an e-mail address, an IBAN           1.00
//! -  file name                  words of the file's own name that match a known name    0.50
//! ```
//!
//! Guards that keep this honest: a month, a weekday or a document word is never a name (the packs'
//! stop-words); a capital letter that merely opens a sentence is not evidence of a name; a hard cap
//! on entities and mentions per source; a chunk a machine read with little confidence cannot create
//! anything (it may only repeat a name the base already knows, with its confidence scaled down);
//! text that is empty, digits only or in another script yields nothing, never an error.
//!
//! Every word that depends on a language or a profession comes from the packs. There is no literal
//! title, month or suffix in this file.

use std::collections::{BTreeMap, BTreeSet};

use unicode_normalization::char::is_combining_mark;

use super::{
    Candidate, CandidateIdentifier, DocumentInput, EntityExtractor, ExtractContext,
    ExtractedKnowledge, Span,
};
use crate::knowledge::gazetteer::{Gazetteer, GazetteerEntry};
use crate::knowledge::normalize::{filename_name_candidates, fold_word, normalize_name, TitleSet};
use crate::knowledge::packs::PackSet;
use crate::knowledge::{AliasKind, EntityTypeId, MentionLocator, Method};

pub const EXTRACTOR_ID: &str = "deterministic-text";

/// Raised whenever this extractor's output for some text changes, so the next pass reads the
/// sources again (`CURRENT_KB_VERSION` is what decides that; this one is recorded beside it).
pub const EXTRACTOR_VERSION: u32 = 1;

/// A source with more distinct names than this keeps the most confident ones.
pub const MAX_ENTITIES_PER_SOURCE: usize = 500;
/// A source with more mentions than this keeps the most confident ones.
pub const MAX_MENTIONS_PER_SOURCE: usize = 5_000;

/// A page the OCR engine rated below this cannot create an entity. The engine itself already
/// refuses a page below 0.6 (`ocr::tesseract::CONFIDENCE_FLOOR`); between that and this number a
/// page is readable but one wrong letter makes a new, wrong name. Tuned in lot 11.
pub const MIN_OCR_CONFIDENCE_TO_CREATE: f32 = 0.80;

/// A name of a person is two or three capitalised words; more is a heading or a sentence.
const MAX_PERSON_UNITS: usize = 3;
/// An organisation's own words, besides its marker.
const MAX_ORGANIZATION_UNITS: usize = 4;

/// Whether a word of a gazetteer phrase names the whole entity (a full name, an alias a person
/// typed) or only part of it (a surname, an initial and a surname, a title and a surname).
fn names_the_whole_entity(kind: AliasKind) -> bool {
    matches!(
        kind,
        AliasKind::CanonicalVariant
            | AliasKind::Reordered
            | AliasKind::Abbreviation
            | AliasKind::FilenameForm
            | AliasKind::Manual
    )
}

// ---------------------------------------------------------------------------------------------
// Reading a text as units
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TitleKind {
    No,
    Strong,
    Weak,
}

/// One word as a person would see it: letters, with a hyphen or an apostrophe inside (`Jean-Pierre`,
/// `d'Aubigne`). The unit is what a name is made of; the sub-words are what the gazetteer reads.
#[derive(Debug, Clone)]
struct Unit {
    /// Character offsets in the text that was read.
    start: usize,
    end: usize,
    written: String,
    /// The folded sub-words, split at hyphens and apostrophes the way `normalize_name` splits.
    words: Vec<String>,
    /// First letter upper case, after an elided one-letter article ("d'Aubigne").
    capitalised: bool,
    /// A single capital letter followed by a dot.
    initial: bool,
    /// A lowercase word that sits inside a name ("de", "van").
    particle: bool,
    stop: bool,
    title: TitleKind,
    /// Units with nothing but spaces between them share a segment; a comma, a colon, a number or a
    /// full stop starts another. A name never spans two.
    segment: usize,
    /// The first unit of a sentence or of a line: a capital letter there proves nothing.
    sentence_start: bool,
    letters: usize,
}

impl Unit {
    /// A word that can be part of a name: capitalised, not a stop-word, not a title.
    fn name_like(&self) -> bool {
        self.capitalised
            && !self.particle
            && !self.stop
            && self.title == TitleKind::No
            && (self.letters >= 2 || self.initial)
    }

    fn rendered(&self) -> String {
        if self.initial {
            format!("{}.", self.written)
        } else {
            self.written.clone()
        }
    }
}

fn is_apostrophe(c: char) -> bool {
    matches!(
        c,
        '\'' | '\u{2019}' | '\u{2018}' | '\u{2032}' | '\u{2bc}' | '`' | '\u{b4}'
    )
}

fn is_letter(c: char) -> bool {
    c.is_alphabetic() || is_combining_mark(c)
}

/// What the characters between two tokens do: end the run of words, and perhaps start a sentence.
fn gap_effect(gap: &str, previous_may_be_abbreviated: bool) -> (bool, bool) {
    if gap.is_empty() {
        return (false, false);
    }
    if gap.contains('\n') || gap.contains('\r') {
        return (true, true);
    }
    let marks: Vec<char> = gap.chars().filter(|c| !c.is_whitespace()).collect();
    if marks.is_empty() {
        return (false, false);
    }
    // "Dr. Martin", "J. Dupont": the dot belongs to the abbreviation, not to the sentence.
    if marks == ['.'] && previous_may_be_abbreviated {
        return (false, false);
    }
    let ends_sentence = marks
        .iter()
        .any(|c| matches!(c, '.' | '!' | '?' | '\u{2026}'));
    (true, ends_sentence)
}

fn scan_units(text: &str, titles: &TitleSet) -> Vec<Unit> {
    let chars: Vec<char> = text.chars().collect();
    let mut units: Vec<Unit> = Vec::new();
    let mut segment = 0usize;
    let mut sentence_start = true;
    let mut gap = String::new();
    // Whether the token before the gap is a word that a dot may abbreviate.
    let mut abbreviable = false;
    let mut seen_token = false;
    let mut index = 0usize;

    while index < chars.len() {
        let c = chars[index];
        let starts_word = c.is_alphabetic();
        let starts_number = c.is_numeric();
        if !starts_word && !starts_number {
            gap.push(c);
            index += 1;
            continue;
        }

        if seen_token {
            let (breaks, new_sentence) = gap_effect(&gap, abbreviable);
            if breaks {
                segment += 1;
            }
            if new_sentence {
                sentence_start = true;
            }
        }
        gap.clear();
        seen_token = true;

        let start = index;
        let mut end = index;
        if starts_word {
            while end < chars.len() {
                let d = chars[end];
                // A hyphen or an apostrophe belongs to the word when a letter follows it.
                let joins = (d == '-' || is_apostrophe(d))
                    && end > start
                    && chars.get(end + 1).is_some_and(|n| n.is_alphabetic());
                if is_letter(d) || joins {
                    end += 1;
                } else {
                    break;
                }
            }
        }
        // Letters glued to digits (a reference, a model number) or a number: not a word, and a
        // wall between the words on either side of it.
        let glued = !starts_word || chars.get(end).is_some_and(|d| d.is_numeric() || *d == '_');
        if glued {
            while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            segment += 1;
            sentence_start = false;
            abbreviable = false;
            index = end;
            continue;
        }

        let written: String = chars[start..end].iter().collect();
        let mut words: Vec<String> = Vec::new();
        let mut current = String::new();
        for &d in &chars[start..end] {
            if d == '-' || is_apostrophe(d) {
                if !current.is_empty() {
                    words.push(fold_word(&current));
                    current.clear();
                }
            } else {
                current.push(d);
            }
        }
        if !current.is_empty() {
            words.push(fold_word(&current));
        }
        words.retain(|word| !word.is_empty());
        if words.is_empty() {
            index = end;
            continue;
        }

        let letters = chars[start..end]
            .iter()
            .filter(|d| d.is_alphabetic())
            .count();
        // The letter that carries the capital: the first one, or the one after a one-letter elided
        // article ("d'Aubigne" is capitalised by its A).
        let body = &chars[start..end];
        let elided_article = body.len() > 2 && body[0].is_alphabetic() && is_apostrophe(body[1]);
        let carrier = if elided_article { &body[2..] } else { body };
        let capitalised = carrier
            .iter()
            .find(|d| d.is_alphabetic())
            .is_some_and(|d| d.is_uppercase());
        let followed_by_dot = chars.get(end) == Some(&'.');
        let initial = letters == 1 && words.len() == 1 && capitalised && followed_by_dot;
        let single = words.len() == 1;
        let kind = if single && titles.is_title(&words[0]) {
            TitleKind::Strong
        } else if single && titles.is_weak_title(&words[0]) {
            TitleKind::Weak
        } else {
            TitleKind::No
        };
        // A small word inside a name is lower case ("de"), or written in capitals in a heading
        // ("DU"): either way it carries no identity of its own.
        let shouting = letters >= 2
            && carrier
                .iter()
                .filter(|d| d.is_alphabetic())
                .all(|d| d.is_uppercase());
        let particle = single && (!capitalised || shouting) && titles.is_particle(&words[0]);
        let stop = words.iter().any(|word| titles.is_stop_word(word));

        units.push(Unit {
            start,
            end,
            written,
            words,
            capitalised,
            initial: initial && kind == TitleKind::No,
            particle,
            stop,
            title: kind,
            segment,
            sentence_start,
            letters,
        });
        sentence_start = false;
        abbreviable = initial || (kind != TitleKind::No && capitalised);
        index = end;
    }

    // A weak title ("Me", "M") is a title only when it is written with a capital and a capitalised
    // word follows it.
    for position in 0..units.len() {
        if units[position].title != TitleKind::Weak {
            continue;
        }
        let qualifies = units[position].capitalised
            && units.get(position + 1).is_some_and(|next| {
                next.segment == units[position].segment && next.capitalised && !next.stop
            });
        if !qualifies {
            units[position].title = TitleKind::No;
        }
    }
    units
}

/// The units as maximal runs that share a segment: `[start, end)` positions.
fn segments_of(units: &[Unit]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut start = 0usize;
    for position in 1..=units.len() {
        if position == units.len() || units[position].segment != units[start].segment {
            if position > start {
                found.push((start, position));
            }
            start = position;
        }
    }
    found
}

// ---------------------------------------------------------------------------------------------
// Finding names
// ---------------------------------------------------------------------------------------------

/// A name found in the units of one chunk, before overlaps are settled.
#[derive(Debug, Clone)]
struct Found {
    /// Positions in the unit list, both included.
    first: usize,
    last: usize,
    /// Lower wins when two finds overlap (the table at the top of this file).
    priority: u8,
    surface: String,
    type_id: EntityTypeId,
    method: Method,
    titles_seen: Vec<String>,
}

fn render(units: &[Unit]) -> String {
    units
        .iter()
        .map(Unit::rendered)
        .collect::<Vec<_>>()
        .join(" ")
}

/// A title the unit at `position` stands under, when one stands right before it.
fn title_before(units: &[Unit], position: usize) -> Option<String> {
    let previous = position.checked_sub(1).and_then(|p| units.get(p))?;
    (previous.segment == units[position].segment && previous.title != TitleKind::No)
        .then(|| previous.words[0].clone())
}

fn gazetteer_finds(units: &[Unit], range: (usize, usize), gazetteer: &Gazetteer) -> Vec<Found> {
    let (from, to) = range;
    let mut words: Vec<String> = Vec::new();
    let mut word_unit: Vec<usize> = Vec::new();
    let mut first_word: Vec<usize> = Vec::new();
    for (offset, unit) in units[from..to].iter().enumerate() {
        first_word.push(words.len());
        for word in &unit.words {
            words.push(word.clone());
            word_unit.push(offset);
        }
    }

    let mut found = Vec::new();
    for hit in gazetteer.matches_in(&words) {
        let first = word_unit[hit.start];
        let last = word_unit[hit.start + hit.len - 1];
        // A phrase must start and end where a word does: "jean dupont" is not a part of
        // "Jean-Dupont-Martin".
        if first_word[first] != hit.start
            || first_word[last] + units[from + last].words.len() != hit.start + hit.len
        {
            continue;
        }
        // One short word of lower case text is likelier an ordinary word than a name.
        if first == last && !units[from + first].capitalised {
            continue;
        }
        let entries: Vec<&GazetteerEntry> = hit
            .entries
            .iter()
            .filter(|entry| entry.type_id != EntityTypeId::IDENTIFIER)
            .collect();
        if entries.is_empty() {
            continue;
        }
        let whole = entries
            .iter()
            .copied()
            .find(|entry| names_the_whole_entity(entry.kind));
        let (chosen, method, priority) = match whole {
            Some(entry) => (entry, Method::GazetteerHit, 0),
            // After the untitled runs on purpose: "Alice Archer" is a new person even where
            // "Archer" alone is a known one, and "P. Dupont" is not "Dupont".
            None => (entries[0], Method::CorefInDocument, 6),
        };
        let Some(type_id) = EntityTypeId::new(&chosen.type_id) else {
            continue;
        };
        let (a, b) = (from + first, from + last);
        found.push(Found {
            first: a,
            last: b,
            priority,
            surface: render(&units[a..=b]),
            type_id,
            method,
            titles_seen: title_before(units, a).into_iter().collect(),
        });
    }
    found
}

fn title_finds(units: &[Unit], range: (usize, usize)) -> Vec<Found> {
    let (from, to) = range;
    let mut found = Vec::new();
    for position in from..to {
        if units[position].title == TitleKind::No {
            continue;
        }
        let mut cursor = position + 1;
        let mut names = 0usize;
        let mut last = None;
        while cursor < to && names < MAX_PERSON_UNITS {
            if units[cursor].name_like() {
                names += 1;
                last = Some(cursor);
                cursor += 1;
            } else if names > 0
                && units[cursor].particle
                && cursor + 1 < to
                && units[cursor + 1].name_like()
            {
                cursor += 1;
            } else {
                break;
            }
        }
        let Some(last) = last else { continue };
        if units[last].initial {
            continue;
        }
        let first = position + 1;
        found.push(Found {
            first,
            last,
            priority: 3,
            surface: render(&units[first..=last]),
            type_id: EntityTypeId::person(),
            method: if names >= 2 {
                Method::TitleFullName
            } else {
                Method::TitleSurname
            },
            titles_seen: vec![units[position].words[0].clone()],
        });
    }
    found
}

/// The positions where the words of `marker` stand one after the other, as whole units.
fn marker_positions(units: &[Unit], range: (usize, usize), marker: &[String]) -> Vec<usize> {
    let (from, to) = range;
    if marker.is_empty() || to < from + marker.len() {
        return Vec::new();
    }
    (from..=to - marker.len())
        .filter(|&position| {
            marker.iter().enumerate().all(|(offset, word)| {
                let unit = &units[position + offset];
                unit.words.len() == 1 && unit.words[0] == *word
            })
        })
        .collect()
}

fn organization_finds(units: &[Unit], range: (usize, usize), packs: &PackSet) -> Vec<Found> {
    let (from, to) = range;
    let mut found = Vec::new();

    for marker in &packs.org_markers().prefix {
        for position in marker_positions(units, range, marker) {
            let after = position + marker.len();
            let mut cursor = after;
            let mut names = 0usize;
            let mut last = None;
            while cursor < to && names < MAX_ORGANIZATION_UNITS {
                if units[cursor].name_like() {
                    names += 1;
                    last = Some(cursor);
                    cursor += 1;
                } else if (names > 0 || cursor == after)
                    && units[cursor].particle
                    && cursor + 1 < to
                    && units[cursor + 1].name_like()
                {
                    cursor += 1;
                } else {
                    break;
                }
            }
            let Some(last) = last else { continue };
            found.push(Found {
                first: position,
                last,
                priority: 2,
                surface: render(&units[position..=last]),
                type_id: EntityTypeId::organization(),
                method: Method::OrganizationPrefix,
                titles_seen: Vec::new(),
            });
        }
    }

    for marker in &packs.org_markers().suffix {
        for position in marker_positions(units, range, marker) {
            // A legal form is written as one: "SARL", "Inc". A word that merely looks like one
            // ("Sa", "Co") is not.
            let written = &units[position].written;
            let all_caps = written
                .chars()
                .all(|c| !c.is_alphabetic() || c.is_uppercase());
            let proper = units[position].capitalised && units[position].letters >= 3;
            if !(all_caps || proper) {
                continue;
            }
            let mut cursor = position;
            let mut names = 0usize;
            let mut first = None;
            while cursor > from && names < MAX_ORGANIZATION_UNITS {
                let before = cursor - 1;
                if units[before].name_like() {
                    names += 1;
                    first = Some(before);
                    cursor = before;
                } else if names > 0
                    && units[before].particle
                    && before > from
                    && units[before - 1].name_like()
                {
                    cursor = before;
                } else {
                    break;
                }
            }
            let Some(first) = first else { continue };
            let last = position + marker.len() - 1;
            found.push(Found {
                first,
                last,
                priority: 1,
                surface: render(&units[first..=last]),
                type_id: EntityTypeId::organization(),
                method: Method::OrganizationMarker,
                titles_seen: Vec::new(),
            });
        }
    }
    found
}

fn untitled_finds(units: &[Unit], range: (usize, usize)) -> Vec<Found> {
    let (from, to) = range;
    let mut found = Vec::new();
    let mut position = from;
    while position < to {
        if !units[position].name_like() {
            position += 1;
            continue;
        }
        let first = position;
        let mut last = position;
        let mut names = 1usize;
        loop {
            let next = last + 1;
            if next >= to {
                break;
            }
            if units[next].name_like() {
                last = next;
                names += 1;
            } else if units[next].particle && next + 1 < to && units[next + 1].name_like() {
                last = next + 1;
                names += 1;
            } else {
                break;
            }
        }
        position = last + 1;
        let usable = (2..=MAX_PERSON_UNITS).contains(&names)
            && !units[first].sentence_start
            && !units[last].initial;
        if usable {
            found.push(Found {
                first,
                last,
                priority: 5,
                surface: render(&units[first..=last]),
                type_id: EntityTypeId::person(),
                method: Method::CapitalisedName,
                titles_seen: Vec::new(),
            });
        }
    }
    found
}

/// Keep the finds that do not overlap a better one: the lowest priority number first, then the
/// longest, then the leftmost.
fn settle(mut finds: Vec<Found>) -> Vec<Found> {
    finds.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then((b.last - b.first).cmp(&(a.last - a.first)))
            .then(a.first.cmp(&b.first))
    });
    let mut taken: Vec<(usize, usize)> = Vec::new();
    let mut kept = Vec::new();
    for find in finds {
        if taken
            .iter()
            .any(|&(first, last)| find.first <= last && first <= find.last)
        {
            continue;
        }
        taken.push((find.first, find.last));
        kept.push(find);
    }
    kept.sort_by_key(|find| find.first);
    kept
}

// ---------------------------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------------------------

struct FoundIdentifier {
    /// Byte range in the chunk text.
    range: std::ops::Range<usize>,
    scheme: String,
    kind: String,
    normalized: String,
}

/// Every identifier of every scheme, the longest first when two overlap (an IBAN contains digit
/// runs that read as a phone number).
fn identifier_finds(text: &str, packs: &PackSet) -> Vec<FoundIdentifier> {
    let mut all: Vec<FoundIdentifier> = Vec::new();
    for scheme in packs.identifier_schemes() {
        for found in scheme.scan(text) {
            all.push(FoundIdentifier {
                range: found.range,
                scheme: scheme.id.clone(),
                kind: scheme.kind.clone(),
                normalized: found.normalized,
            });
        }
    }
    all.sort_by(|a, b| {
        (b.range.end - b.range.start)
            .cmp(&(a.range.end - a.range.start))
            .then(a.range.start.cmp(&b.range.start))
    });
    let mut kept: Vec<FoundIdentifier> = Vec::new();
    for found in all {
        if kept
            .iter()
            .any(|other| found.range.start < other.range.end && other.range.start < found.range.end)
        {
            continue;
        }
        kept.push(found);
    }
    kept.sort_by_key(|found| found.range.start);
    kept
}

/// The text with every identifier blanked out, same length in characters, so the words around an
/// identifier keep their place and nothing of it is read as a name.
fn blank_out(text: &str, found: &[FoundIdentifier]) -> String {
    if found.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for item in found {
        out.push_str(&text[cursor..item.range.start]);
        out.extend(text[item.range.clone()].chars().map(|_| ' '));
        cursor = item.range.end;
    }
    out.push_str(&text[cursor..]);
    out
}

// ---------------------------------------------------------------------------------------------
// The extractor
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicTextExtractor;

struct Reading<'a> {
    packs: &'a PackSet,
    titles: TitleSet,
    gazetteer: &'a Gazetteer,
}

impl Reading<'_> {
    fn chunk(&self, chunk_id: &str, text: &str, ocr_confidence: Option<f32>) -> Vec<Candidate> {
        let weak_scan = ocr_confidence.is_some_and(|value| value < MIN_OCR_CONFIDENCE_TO_CREATE);
        let scale = if weak_scan {
            ocr_confidence.unwrap_or(0.0).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let locator = MentionLocator::Chunk {
            chunk_id: chunk_id.to_string(),
        };
        let mut candidates: Vec<Candidate> = Vec::new();

        // A chunk read badly creates nothing: no identifier (a misread digit is a new, wrong
        // number) and no name from a pattern. It may still repeat a name the base already knows.
        let identifiers = if weak_scan {
            Vec::new()
        } else {
            identifier_finds(text, self.packs)
        };
        for item in &identifiers {
            let written = text[item.range.clone()].trim().to_string();
            let start = text[..item.range.start].chars().count();
            let length = text[item.range.clone()].chars().count();
            let mut candidate = Candidate::new(
                &written,
                EntityTypeId::new(EntityTypeId::IDENTIFIER).expect("a built-in type"),
                Method::Identifier,
                locator.clone(),
            );
            candidate.subtype = Some(item.kind.clone());
            candidate.span = Some(Span {
                start,
                end: start + length,
            });
            candidate.identifier = Some(CandidateIdentifier {
                scheme: item.scheme.clone(),
                normalized: item.normalized.clone(),
            });
            candidates.push(candidate);
        }

        let blanked = blank_out(text, &identifiers);
        let units = scan_units(&blanked, &self.titles);
        let mut finds: Vec<Found> = Vec::new();
        for range in segments_of(&units) {
            finds.extend(gazetteer_finds(&units, range, self.gazetteer));
            if !weak_scan {
                finds.extend(title_finds(&units, range));
                finds.extend(organization_finds(&units, range, self.packs));
                finds.extend(untitled_finds(&units, range));
            }
        }

        for find in settle(finds) {
            let mut candidate =
                Candidate::new(&find.surface, find.type_id, find.method, locator.clone());
            candidate.span = Some(Span {
                start: units[find.first].start,
                end: units[find.last].end,
            });
            candidate.titles_seen = find.titles_seen;
            if weak_scan {
                candidate.confidence =
                    crate::knowledge::Confidence::new(candidate.confidence.value() * scale);
            }
            candidates.push(candidate);
        }
        candidates
    }

    /// Words of the file's own name that match a name the base holds, in full. Never a new name.
    fn file_name(&self, relative_path: &str) -> Vec<Candidate> {
        let name = relative_path.rsplit('/').next().unwrap_or(relative_path);
        let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
        let mut found = Vec::new();
        for run in filename_name_candidates(stem, &self.titles) {
            let Some(hit) = self.gazetteer.longest_match_at(&run, 0) else {
                continue;
            };
            if hit.len != run.len() {
                continue;
            }
            let whole = hit.entries.iter().find(|entry| {
                names_the_whole_entity(entry.kind) && entry.type_id != EntityTypeId::IDENTIFIER
            });
            let Some(entry) = whole else { continue };
            let Some(type_id) = EntityTypeId::new(&entry.type_id) else {
                continue;
            };
            found.push(Candidate::new(
                &run.join(" "),
                type_id,
                Method::FilenameToken,
                MentionLocator::Filename,
            ));
        }
        found
    }
}

/// What makes two candidates one name: the type and the name as the resolver reads it, or the
/// identifier itself.
fn group_key(candidate: &Candidate, titles: &TitleSet) -> String {
    match &candidate.identifier {
        Some(identifier) => format!("identifier:{}:{}", identifier.scheme, identifier.normalized),
        None => format!(
            "{}:{}",
            candidate.type_id.as_str(),
            normalize_name(&candidate.surface, titles).joined
        ),
    }
}

/// One candidate per name per chunk, with the number of times it appears there.
fn merge_within_chunks(candidates: Vec<Candidate>, titles: &TitleSet) -> Vec<Candidate> {
    let mut order: Vec<String> = Vec::new();
    let mut merged: BTreeMap<String, Candidate> = BTreeMap::new();
    for candidate in candidates {
        let key = format!("{:?}|{}", candidate.locator, group_key(&candidate, titles));
        match merged.get_mut(&key) {
            Some(known) => {
                known.occurrence_count += candidate.occurrence_count;
                for title in candidate.titles_seen {
                    if !known.titles_seen.contains(&title) {
                        known.titles_seen.push(title);
                    }
                }
                if candidate.confidence > known.confidence {
                    known.method = candidate.method;
                    known.confidence = candidate.confidence;
                    known.surface = candidate.surface;
                }
            }
            None => {
                order.push(key.clone());
                merged.insert(key, candidate);
            }
        }
    }
    order
        .into_iter()
        .filter_map(|key| merged.remove(&key))
        .collect()
}

/// Apply the per-source caps. Returns whether anything was dropped.
fn cap(candidates: Vec<Candidate>, titles: &TitleSet) -> (Vec<Candidate>, bool) {
    let mut truncated = false;

    // Distinct names first: keep the most confident, then the most frequent.
    let mut standing: BTreeMap<String, (f32, u32)> = BTreeMap::new();
    for candidate in &candidates {
        let entry = standing
            .entry(group_key(candidate, titles))
            .or_insert((0.0, 0));
        entry.0 = entry.0.max(candidate.confidence.value());
        entry.1 += candidate.occurrence_count;
    }
    let mut candidates = candidates;
    if standing.len() > MAX_ENTITIES_PER_SOURCE {
        truncated = true;
        let mut ranked: Vec<(&String, &(f32, u32))> = standing.iter().collect();
        ranked.sort_by(|a, b| {
            b.1 .0
                .partial_cmp(&a.1 .0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.1 .1.cmp(&a.1 .1))
                .then(a.0.cmp(b.0))
        });
        let keep: BTreeSet<String> = ranked
            .into_iter()
            .take(MAX_ENTITIES_PER_SOURCE)
            .map(|(key, _)| key.clone())
            .collect();
        candidates.retain(|candidate| keep.contains(&group_key(candidate, titles)));
    }

    if candidates.len() > MAX_MENTIONS_PER_SOURCE {
        truncated = true;
        let mut ranked: Vec<usize> = (0..candidates.len()).collect();
        ranked.sort_by(|&a, &b| {
            candidates[b]
                .confidence
                .partial_cmp(&candidates[a].confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.cmp(&b))
        });
        let keep: BTreeSet<usize> = ranked.into_iter().take(MAX_MENTIONS_PER_SOURCE).collect();
        let mut position = 0usize;
        candidates.retain(|_| {
            let kept = keep.contains(&position);
            position += 1;
            kept
        });
    }
    (candidates, truncated)
}

impl EntityExtractor for DeterministicTextExtractor {
    fn id(&self) -> &str {
        EXTRACTOR_ID
    }

    fn version(&self) -> u32 {
        EXTRACTOR_VERSION
    }

    fn extract_document(
        &self,
        input: &DocumentInput,
        context: &ExtractContext<'_>,
    ) -> ExtractedKnowledge {
        let reading = Reading {
            packs: context.packs,
            titles: context.packs.title_set(),
            gazetteer: context.gazetteer,
        };
        let mut candidates: Vec<Candidate> = Vec::new();
        for chunk in &input.chunks {
            candidates.extend(merge_within_chunks(
                reading.chunk(&chunk.chunk_id, &chunk.text, chunk.ocr_confidence),
                &reading.titles,
            ));
        }
        candidates.extend(merge_within_chunks(
            reading.file_name(&input.source.relative_path),
            &reading.titles,
        ));
        let (candidates, truncated) = cap(candidates, &reading.titles);
        ExtractedKnowledge {
            candidates,
            truncated,
            ..ExtractedKnowledge::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::extract::ChunkInput;
    use crate::knowledge::phonetic::FrenchPhonetic;
    use crate::knowledge::{Domain, Origin, SourceRef};

    fn source(path: &str) -> SourceRef {
        SourceRef {
            domain: Domain::Documents,
            relative_path: path.to_string(),
            content_id: "sha".to_string(),
        }
    }

    fn read_with(
        path: &str,
        chunks: &[(&str, Option<f32>)],
        gazetteer: &Gazetteer,
        packs: &PackSet,
    ) -> ExtractedKnowledge {
        let input = DocumentInput {
            source: source(path),
            chunks: chunks
                .iter()
                .enumerate()
                .map(|(position, (text, confidence))| ChunkInput {
                    chunk_id: format!("{path}#p1#s{}", position + 1),
                    text: text.to_string(),
                    ocr_confidence: *confidence,
                })
                .collect(),
        };
        let encoder = FrenchPhonetic;
        DeterministicTextExtractor.extract_document(
            &input,
            &ExtractContext {
                packs,
                locale: "en-US",
                gazetteer,
                encoder: &encoder,
            },
        )
    }

    fn read(text: &str) -> ExtractedKnowledge {
        let packs = PackSet::load("en-US", &[]).unwrap();
        read_with("doc.txt", &[(text, None)], &Gazetteer::new(), &packs)
    }

    fn surfaces(found: &ExtractedKnowledge) -> Vec<(String, Method)> {
        found
            .candidates
            .iter()
            .map(|c| (c.surface.clone(), c.method))
            .collect()
    }

    fn names(found: &ExtractedKnowledge) -> Vec<String> {
        found.candidates.iter().map(|c| c.surface.clone()).collect()
    }

    fn known(list: &mut Gazetteer, phrase: &str, entity: i64, kind: AliasKind) {
        list.insert(
            phrase,
            GazetteerEntry {
                entity_id: entity,
                type_id: "person".to_string(),
                kind,
                origin: Origin::Automatic,
            },
        );
    }

    #[test]
    fn empty_and_meaningless_text_yields_nothing() {
        for text in [
            "",
            "   \n\n ",
            "12345 67890",
            "!!! ??? ...",
            "----",
            "a b c",
        ] {
            assert!(read(text).candidates.is_empty(), "{text:?}");
        }
    }

    #[test]
    fn a_title_and_a_full_name_make_a_person_with_the_title_kept_aside() {
        let found = read("We saw Dr Jean Dupont yesterday.");
        assert_eq!(
            surfaces(&found),
            [("Jean Dupont".to_string(), Method::TitleFullName)]
        );
        assert_eq!(found.candidates[0].titles_seen, ["dr"]);
        assert_eq!(found.candidates[0].type_id, EntityTypeId::person());
    }

    #[test]
    fn a_title_with_a_dot_and_a_hyphenated_given_name() {
        let found = read("Ask Dr. Jean-Pierre Martin about it.");
        assert_eq!(names(&found), ["Jean-Pierre Martin"]);
        assert_eq!(found.candidates[0].method, Method::TitleFullName);
    }

    #[test]
    fn a_title_and_a_surname_alone_is_a_weak_find() {
        let found = read("Please call Dr Martin tomorrow.");
        assert_eq!(
            surfaces(&found),
            [("Martin".to_string(), Method::TitleSurname)]
        );
    }

    #[test]
    fn a_title_before_a_lowercase_word_names_nobody() {
        assert!(read("The Dr will see you now.").candidates.is_empty());
    }

    #[test]
    fn two_capitalised_words_inside_a_sentence_are_a_low_confidence_guess() {
        let found = read("The report was written by Alice Archer last week.");
        assert_eq!(
            surfaces(&found),
            [("Alice Archer".to_string(), Method::CapitalisedName)]
        );
        assert!((found.candidates[0].confidence.value() - 0.40).abs() < 1e-6);
    }

    #[test]
    fn a_capital_that_only_opens_a_sentence_proves_nothing() {
        assert!(read("Alice Archer wrote this.").candidates.is_empty());
        assert!(read("First line\nAlice Archer\nthird line")
            .candidates
            .is_empty());
        assert!(read("It rained. Alice Archer left.").candidates.is_empty());
    }

    #[test]
    fn a_label_and_a_colon_do_not_hide_a_name() {
        let found = read("Client: Alice Archer, address follows.");
        assert_eq!(names(&found), ["Alice Archer"]);
    }

    #[test]
    fn months_and_weekdays_never_become_names() {
        for text in [
            "We meet on Monday March 3 again.",
            "See you in March April or May.",
            "Closed Saturday Sunday.",
            "Sent on Friday June by mail",
        ] {
            assert!(read(text).candidates.is_empty(), "{text}");
        }
    }

    #[test]
    fn a_run_of_four_capitalised_words_is_a_heading_not_a_person() {
        assert!(read("He read the Annual Review Of Results today.")
            .candidates
            .is_empty());
    }

    #[test]
    fn a_legal_form_makes_an_organization_that_is_trusted() {
        let found = read("The contract with Acme Widgets Ltd was signed.");
        assert_eq!(
            surfaces(&found),
            [("Acme Widgets Ltd".to_string(), Method::OrganizationMarker)]
        );
        assert_eq!(found.candidates[0].type_id, EntityTypeId::organization());
    }

    #[test]
    fn a_word_that_only_looks_like_a_legal_form_is_not_one() {
        assert!(read("Seen with Alice Co yesterday.")
            .candidates
            .iter()
            .all(|c| c.method != Method::OrganizationMarker));
    }

    #[test]
    fn an_organization_prefix_makes_a_candidate_organization() {
        let found = read("We joined the Association Friends Of Music last year.");
        let organizations: Vec<_> = found
            .candidates
            .iter()
            .filter(|c| c.type_id == EntityTypeId::organization())
            .collect();
        assert_eq!(organizations.len(), 1);
        assert_eq!(organizations[0].method, Method::OrganizationPrefix);
        assert!(organizations[0].surface.starts_with("Association"));
    }

    #[test]
    fn an_identifier_is_read_and_its_digits_are_not_taken_for_words() {
        let found = read(
            "Invoice INV-2026-0042 is due, see Alice Archer or mail alice.archer@example.org.",
        );
        let kinds: Vec<_> = found
            .candidates
            .iter()
            .map(|c| (c.method, c.subtype.clone()))
            .collect();
        assert!(kinds.contains(&(Method::Identifier, Some("invoice".to_string()))));
        assert!(kinds.contains(&(Method::Identifier, Some("email".to_string()))));
        let invoice = found
            .candidates
            .iter()
            .find(|c| c.subtype.as_deref() == Some("invoice"))
            .unwrap();
        assert_eq!(
            invoice.identifier.as_ref().unwrap().normalized,
            "INV20260042"
        );
        assert!(found
            .candidates
            .iter()
            .any(|c| c.surface == "Alice Archer" && c.method == Method::CapitalisedName));
    }

    #[test]
    fn a_name_found_twice_in_a_chunk_is_one_candidate_counted_twice() {
        let found = read("We asked Dr Jean Dupont, and later Dr Jean Dupont answered.");
        assert_eq!(found.candidates.len(), 1);
        assert_eq!(found.candidates[0].occurrence_count, 2);
    }

    #[test]
    fn the_same_name_in_two_chunks_is_two_candidates_with_their_own_chunk() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let found = read_with(
            "doc.txt",
            &[
                ("Ask Dr Jean Dupont today.", None),
                ("Dr Jean Dupont agreed.", None),
            ],
            &Gazetteer::new(),
            &packs,
        );
        let chunks: Vec<_> = found
            .candidates
            .iter()
            .map(|c| match &c.locator {
                MentionLocator::Chunk { chunk_id } => chunk_id.clone(),
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(chunks, ["doc.txt#p1#s1", "doc.txt#p1#s2"]);
    }

    #[test]
    fn a_known_name_is_found_even_where_a_capital_opens_the_sentence() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let mut list = Gazetteer::new();
        known(&mut list, "alice archer", 1, AliasKind::CanonicalVariant);
        let found = read_with(
            "doc.txt",
            &[("Alice Archer wrote this.", None)],
            &list,
            &packs,
        );
        assert_eq!(
            surfaces(&found),
            [("Alice Archer".to_string(), Method::GazetteerHit)]
        );
        assert!((found.candidates[0].confidence.value() - 0.90).abs() < 1e-6);
    }

    #[test]
    fn a_known_name_written_with_a_hyphen_is_still_found() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let mut list = Gazetteer::new();
        known(&mut list, "jean dupont", 1, AliasKind::CanonicalVariant);
        let found = read_with(
            "doc.txt",
            &[("We met Jean-Dupont there.", None)],
            &list,
            &packs,
        );
        assert_eq!(names(&found), ["Jean-Dupont"]);
    }

    #[test]
    fn a_phrase_must_start_and_end_where_a_word_does() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let mut list = Gazetteer::new();
        known(&mut list, "jean dupont", 1, AliasKind::CanonicalVariant);
        let found = read_with(
            "doc.txt",
            &[("We met Jean-Dupont-Martin there.", None)],
            &list,
            &packs,
        );
        assert!(found.candidates.is_empty());
    }

    #[test]
    fn a_surname_of_a_known_person_is_a_coref_find_not_a_new_name() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let mut list = Gazetteer::new();
        known(&mut list, "dupont", 1, AliasKind::PartialSurname);
        let found = read_with(
            "doc.txt",
            &[("The letter from Dupont arrived.", None)],
            &list,
            &packs,
        );
        assert_eq!(
            surfaces(&found),
            [("Dupont".to_string(), Method::CorefInDocument)]
        );
    }

    #[test]
    fn a_lowercase_ordinary_word_that_is_also_a_surname_is_left_alone() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let mut list = Gazetteer::new();
        known(&mut list, "baker", 1, AliasKind::PartialSurname);
        let found = read_with(
            "doc.txt",
            &[("the baker opened early", None)],
            &list,
            &packs,
        );
        assert!(found.candidates.is_empty());
    }

    #[test]
    fn an_identifier_entity_is_not_looked_for_by_its_name() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let mut list = Gazetteer::new();
        list.insert(
            "acme widget",
            GazetteerEntry {
                entity_id: 9,
                type_id: "identifier".to_string(),
                kind: AliasKind::CanonicalVariant,
                origin: Origin::Automatic,
            },
        );
        let found = read_with(
            "doc.txt",
            &[("Please settle with Acme Widget soon.", None)],
            &list,
            &packs,
        );
        // Not by the gazetteer: an identifier is found by its pattern and nothing else. (The words
        // are also not read as a person here, being two capitalised words, so look at the method.)
        assert!(found
            .candidates
            .iter()
            .all(|c| c.method != Method::GazetteerHit));
    }

    #[test]
    fn a_title_before_a_known_name_is_kept_as_a_title_seen() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let mut list = Gazetteer::new();
        known(&mut list, "alice archer", 1, AliasKind::CanonicalVariant);
        let found = read_with(
            "doc.txt",
            &[("Please thank Dr Alice Archer today.", None)],
            &list,
            &packs,
        );
        assert_eq!(found.candidates.len(), 1);
        assert_eq!(found.candidates[0].method, Method::GazetteerHit);
        assert_eq!(found.candidates[0].titles_seen, ["dr"]);
    }

    #[test]
    fn a_file_name_only_matches_a_name_that_is_already_known() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let mut list = Gazetteer::new();
        known(&mut list, "alice archer", 1, AliasKind::CanonicalVariant);
        let named = read_with(
            "inbox/Archer_Alice_2026-03.txt",
            &[("plain", None)],
            &list,
            &packs,
        );
        assert_eq!(named.candidates.len(), 1);
        assert_eq!(named.candidates[0].method, Method::FilenameToken);
        assert_eq!(named.candidates[0].locator, MentionLocator::Filename);
        assert!((named.candidates[0].confidence.value() - 0.50).abs() < 1e-6);

        let unknown = read_with(
            "inbox/Archer_Alice_2026-03.txt",
            &[("plain", None)],
            &Gazetteer::new(),
            &packs,
        );
        assert!(
            unknown.candidates.is_empty(),
            "names are never created from file names"
        );
    }

    #[test]
    fn a_page_read_badly_creates_nothing_and_scales_what_it_repeats() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let mut list = Gazetteer::new();
        known(&mut list, "alice archer", 1, AliasKind::CanonicalVariant);
        let text = "Seen: Dr Bruno Baker and Alice Archer and Cara Clark, INV-2026-0042.";
        let weak = read_with("scan.png", &[(text, Some(0.65))], &list, &packs);
        assert_eq!(
            surfaces(&weak),
            [("Alice Archer".to_string(), Method::GazetteerHit)]
        );
        assert!((weak.candidates[0].confidence.value() - 0.90 * 0.65).abs() < 1e-6);

        let good = read_with("scan.png", &[(text, Some(0.92))], &list, &packs);
        assert!(
            good.candidates.len() > 1,
            "above the floor a scan reads like text"
        );
    }

    #[test]
    fn a_source_with_too_many_names_keeps_the_most_confident_and_says_so() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let mut text = String::new();
        for number in 0..620 {
            // Distinct, two-word, never at a sentence start.
            let a = to_word(number);
            let b = to_word(number + 1_000);
            text.push_str(&format!("seen {a} {b}, "));
        }
        let found = read_with("big.txt", &[(&text, None)], &Gazetteer::new(), &packs);
        assert!(found.truncated);
        let distinct: BTreeSet<_> = found.candidates.iter().map(|c| c.surface.clone()).collect();
        assert!(distinct.len() <= MAX_ENTITIES_PER_SOURCE);
        assert!(distinct.len() >= 400);
    }

    /// A pronounceable capitalised word that is different for every number.
    fn to_word(number: usize) -> String {
        let letters = ['b', 'd', 'f', 'g', 'k', 'l', 'm', 'n', 'p', 'r', 's', 't'];
        let vowels = ['a', 'e', 'i', 'o', 'u'];
        let mut n = number;
        let mut word = String::new();
        for round in 0..4 {
            word.push(letters[n % letters.len()]);
            n /= letters.len();
            word.push(vowels[(n + round) % vowels.len()]);
            n /= vowels.len();
        }
        let mut chars = word.chars();
        chars
            .next()
            .map(|first| first.to_uppercase().chain(chars).collect())
            .unwrap_or_default()
    }

    #[test]
    fn a_page_of_one_repeated_name_is_capped_by_mentions_not_entities() {
        let packs = PackSet::load("en-US", &[]).unwrap();
        let chunks: Vec<(String, Option<f32>)> = (0..MAX_MENTIONS_PER_SOURCE + 50)
            .map(|_| ("Ask Dr Jean Dupont now.".to_string(), None))
            .collect();
        let refs: Vec<(&str, Option<f32>)> = chunks.iter().map(|(t, c)| (t.as_str(), *c)).collect();
        let found = read_with("loop.txt", &refs, &Gazetteer::new(), &packs);
        assert!(found.truncated);
        assert_eq!(found.candidates.len(), MAX_MENTIONS_PER_SOURCE);
    }

    #[test]
    fn mixed_scripts_and_odd_input_never_fail() {
        for text in [
            "Zhang Wei and Li Na met Dr Zhang Wei",
            "\u{4e2d}\u{6587}\u{6587}\u{5b57}",
            "Dr \u{1f600} Smith",
            "\u{0000}\u{0001}",
            "A.B.C.D.E.F.",
            "Dr. . . .",
        ] {
            let _ = read(text);
        }
    }

    #[test]
    fn a_small_word_written_in_capitals_in_a_heading_is_still_a_small_word() {
        let titles = TitleSet::new().with_particles(["du", "de"]);
        let units = scan_units("voir CAISSE DU RHONE puis Marie de Fontaine", &titles);
        let flags: Vec<(String, bool)> = units
            .iter()
            .map(|u| (u.written.clone(), u.particle))
            .collect();
        assert_eq!(
            flags,
            [
                ("voir".to_string(), false),
                ("CAISSE".to_string(), false),
                ("DU".to_string(), true),
                ("RHONE".to_string(), false),
                ("puis".to_string(), false),
                ("Marie".to_string(), false),
                ("de".to_string(), true),
                ("Fontaine".to_string(), false),
            ]
        );
    }

    #[test]
    fn units_know_where_a_sentence_starts() {
        let titles = TitleSet::new();
        let units = scan_units("One two. Three four\nFive", &titles);
        let starts: Vec<_> = units.iter().map(|u| u.sentence_start).collect();
        assert_eq!(starts, [true, false, true, false, true]);
    }

    #[test]
    fn digits_wall_off_the_words_around_them() {
        let titles = TitleSet::new();
        let units = scan_units("Alice 42 Archer and A4 Baker", &titles);
        let segments: Vec<_> = units.iter().map(|u| u.segment).collect();
        assert_ne!(segments[0], segments[1]);
        assert!(units.iter().all(|u| u.written != "A4"));
    }
}
