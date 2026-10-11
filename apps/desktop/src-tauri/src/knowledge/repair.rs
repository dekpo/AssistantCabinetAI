//! Proposals to repair the names in a transcript (lot 9). Pure text in, proposals out.
//!
//! A speech engine writes "Dupond" for a patient called Dupont, or "Du pont". This reads the
//! transcript against the names of the user's **selection** and says which words probably meant a
//! name the base holds. It does not depend on any speech engine: the input is a string, the output
//! is data, and the contract file `tests/fixtures/knowledge/repair-fr.json` (pairs that must be
//! proposed, pairs that must not) is meant to be reused by the speech work.
//!
//! Rules that do not move:
//!
//! - **It never rewrites the text.** A proposal says `heard` and the candidates; the caller shows
//!   `heard -> display` and the user accepts. Nothing here edits the composer, and no confirmed
//!   correction is ever written back (master invariant I3: a dictation does not teach the base).
//! - **Only the selection (invariant I7).** A candidate is a name mentioned in the selected files.
//!   A name known only from other files is never proposed, so a repair cannot reveal it.
//! - **Only what is not already a name.** A word or a run that is, as written, a name the selection
//!   holds is left alone, whatever it sounds like.
//! - **Only likely names.** A word is looked at when it is capitalised (if the engine's case can be
//!   trusted) or follows a title, is not a stop-word or a title itself, and has at least four
//!   letters. A run of up to three words is looked at as one when its first word qualifies, which
//!   is how "Du pont" is read as one name.
//! - **One strong candidate is a suggestion, anything less is a question.** Several candidates, or
//!   a candidate found by spelling alone, give an `Ask`: the user picks, nothing is chosen for them.
//!   When no candidate is close enough there is no proposal at all.
//! - Unconfirmed names (candidates) are not targets, as for the speech hints.
//!
//! Voice never confirms an outward action (`docs/DECISIONS.md`); this module has nothing to
//! confirm.

use std::collections::BTreeMap;
use std::collections::HashSet;

use rusqlite::Connection;

use super::normalize::{fold_word, normalize_name};
use super::phonetic::bounded_damerau_levenshtein;
use super::query::{words_of, QuerySelection, Word};
use super::surface::known_entities;
use super::targeting::QueryVocabulary;
use crate::error::AppError;

/// The shortest word looked at on its own. Shorter words are too often ordinary words.
pub const MIN_HEARD_LETTERS: usize = 4;

/// The longest run of words read as one name.
pub const MAX_RUN_WORDS: usize = 3;

/// The most candidates one proposal lists.
pub const MAX_CANDIDATES: usize = 5;

/// A spelling-only match (the sounds differ) needs a word at least this long, one edit, and the
/// same first letter.
const MIN_EDIT_ONLY_LETTERS: usize = 6;

const SCORE_RESPELLED: f32 = 0.95;
const SCORE_SAME_SOUND: f32 = 0.9;
const SCORE_SAME_SOUND_PER_EDIT: f32 = 0.1;
const SCORE_SPELLING_ONLY: f32 = 0.6;

/// Why a name is proposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairReason {
    /// The same sound, within a small spelling distance (`Dupond` for `Dupont`).
    Phonetic,
    /// A spelling one edit away from a long name, whatever the sound.
    Edit,
    /// The same letters as a name or an alias, written with different spacing (`Du pont`).
    Alias,
}

impl RepairReason {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Phonetic => "phonetic",
            Self::Edit => "edit",
            Self::Alias => "alias",
        }
    }
}

/// What the caller should do with a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairAction {
    /// One strong candidate: show `heard -> display` for the user to accept.
    Suggest,
    /// Several candidates, or a weak one: let the user choose, or leave the word.
    Ask,
}

impl RepairAction {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Suggest => "suggest",
            Self::Ask => "ask",
        }
    }
}

/// A byte range of the transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepairSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RepairCandidate {
    pub entity_id: i64,
    /// The canonical name.
    pub display: String,
    pub score: f32,
    pub reason: RepairReason,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RepairProposal {
    pub span: RepairSpan,
    /// The words as the engine wrote them.
    pub heard: String,
    /// Best first. Never empty.
    pub candidates: Vec<RepairCandidate>,
    pub action: RepairAction,
}

/// One spelling an entity of the selection can be written with.
struct Spelling {
    entity: usize,
    compact: String,
    compact_length: usize,
    first: Option<char>,
    key: String,
}

fn looks_like_a_name_word(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_alphabetic() || c == '-' || c == '\'' || c == '\u{2019}')
}

/// The proposals for a transcript, in the order of the transcript.
pub fn propose(
    connection: &Connection,
    vocabulary: &QueryVocabulary,
    transcript: &str,
    selection: Option<&QuerySelection<'_>>,
    case_reliable: bool,
) -> Result<Vec<RepairProposal>, AppError> {
    let words = words_of(transcript);
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let entities: Vec<_> = known_entities(connection, selection)?
        .into_iter()
        .filter(|known| !known.is_unconfirmed())
        .collect();
    if entities.is_empty() {
        return Ok(Vec::new());
    }

    // What the selection already calls a name, as written: such a word is never repaired.
    let mut exact: HashSet<String> = HashSet::new();
    let mut spellings: Vec<Spelling> = Vec::new();
    for (index, known) in entities.iter().enumerate() {
        exact.insert(known.entity.normalized_name.clone());
        let mut seen: HashSet<String> = HashSet::new();
        let mut forms: Vec<(String, String)> = known
            .aliases
            .iter()
            .map(|alias| (alias.normalized.clone(), alias.phonetic_key.clone()))
            .collect();
        forms.push((known.entity.normalized_name.clone(), String::new()));
        for (normalized, key) in forms {
            exact.insert(normalized.clone());
            let compact: String = normalized.chars().filter(|c| *c != ' ').collect();
            if compact.is_empty() || !seen.insert(compact.clone()) {
                continue;
            }
            spellings.push(Spelling {
                entity: index,
                compact_length: compact.chars().count(),
                first: compact.chars().next(),
                compact,
                key,
            });
        }
    }

    let titles = &vocabulary.titles;
    let is_title = |word: &Word| {
        let folded = fold_word(&word.text);
        titles.is_title(&folded) || (titles.is_weak_title(&folded) && word.is_capitalised())
    };

    let mut proposals: Vec<RepairProposal> = Vec::new();
    let mut at = 0;
    while at < words.len() {
        let first = &words[at];
        let follows_title = at > 0 && is_title(&words[at - 1]);
        let eligible = (case_reliable && first.is_capitalised()) || follows_title;
        if !eligible
            || !looks_like_a_name_word(&first.text)
            || is_title(first)
            || vocabulary.packs.is_stop_word(&first.text)
        {
            at += 1;
            continue;
        }
        // Every run that starts here, longest first: the words, their folded forms, and the text
        // as heard.
        let mut runs: Vec<(usize, String, String, String)> = Vec::new();
        for length in (1..=MAX_RUN_WORDS.min(words.len() - at)).rev() {
            let run = &words[at..at + length];
            if run.iter().any(|word| {
                !looks_like_a_name_word(&word.text)
                    || is_title(word)
                    || vocabulary.packs.is_stop_word(&word.text)
            }) {
                continue;
            }
            let folded: Vec<String> = run.iter().map(|word| fold_word(&word.text)).collect();
            let compact: String = folded.concat();
            if compact.chars().count() < MIN_HEARD_LETTERS {
                continue;
            }
            let heard = transcript[run[0].start..run[length - 1].end].to_string();
            runs.push((length, folded.join(" "), compact, heard));
        }

        // A run that is already a name the selection holds is left alone, and its parts are not
        // read again (the longest such run wins).
        if let Some((length, ..)) = runs.iter().find(|(_, joined, ..)| exact.contains(joined)) {
            at += length;
            continue;
        }

        // Otherwise the run whose best candidate is the strongest; on a tie the shortest, so that a
        // word that follows a name is not swallowed into it ("Duran a" is "Duran").
        let mut best: Option<(usize, Vec<RepairCandidate>, String)> = None;
        for (length, _, compact, heard) in &runs {
            let key = vocabulary
                .encoder
                .encode_name(&normalize_name(heard, titles));
            let candidates = candidates_for(compact, &key, &spellings, &entities);
            let Some(top) = candidates.first().map(|candidate| candidate.score) else {
                continue;
            };
            let better = match &best {
                None => true,
                Some((known_length, known, _)) => {
                    let known_top = known[0].score;
                    top > known_top || (top == known_top && length < known_length)
                }
            };
            if better {
                best = Some((*length, candidates, heard.clone()));
            }
        }
        match best {
            Some((length, candidates, heard)) => {
                let run = &words[at..at + length];
                let action = if candidates.len() == 1 && candidates[0].reason != RepairReason::Edit
                {
                    RepairAction::Suggest
                } else {
                    RepairAction::Ask
                };
                proposals.push(RepairProposal {
                    span: RepairSpan {
                        start: run[0].start,
                        end: run[length - 1].end,
                    },
                    heard,
                    candidates,
                    action,
                });
                at += length;
            }
            None => at += 1,
        }
    }
    Ok(proposals)
}

/// The names of the selection a heard run could be, best first.
fn candidates_for(
    compact: &str,
    key: &str,
    spellings: &[Spelling],
    entities: &[super::surface::KnownEntity],
) -> Vec<RepairCandidate> {
    let length = compact.chars().count();
    let bound = if length <= 6 { 1 } else { 2 };
    let first = compact.chars().next();
    let mut best: BTreeMap<usize, (f32, RepairReason)> = BTreeMap::new();
    for spelling in spellings {
        if spelling.compact_length.abs_diff(length) > bound {
            continue;
        }
        let Some(distance) = bounded_damerau_levenshtein(compact, &spelling.compact, bound) else {
            continue;
        };
        let same_sound = !key.is_empty() && !spelling.key.is_empty() && key == spelling.key;
        let (score, reason) = if distance == 0 {
            (SCORE_RESPELLED, RepairReason::Alias)
        } else if same_sound {
            (
                SCORE_SAME_SOUND - SCORE_SAME_SOUND_PER_EDIT * (distance as f32 - 1.0),
                RepairReason::Phonetic,
            )
        } else if length >= MIN_EDIT_ONLY_LETTERS
            && distance == 1
            && spelling.first.is_some()
            && spelling.first == first
        {
            (SCORE_SPELLING_ONLY, RepairReason::Edit)
        } else {
            continue;
        };
        let entry = best.entry(spelling.entity).or_insert((score, reason));
        if score > entry.0 {
            *entry = (score, reason);
        }
    }
    let mut candidates: Vec<RepairCandidate> = best
        .into_iter()
        .map(|(index, (score, reason))| RepairCandidate {
            entity_id: entities[index].entity.entity_id,
            display: entities[index].entity.canonical_name.clone(),
            score,
            reason,
        })
        .collect();
    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then(left.display.cmp(&right.display))
            .then(left.entity_id.cmp(&right.entity_id))
    });
    candidates.truncate(MAX_CANDIDATES);
    candidates
}
