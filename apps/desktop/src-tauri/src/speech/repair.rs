//! Name suggestions after a dictation: the words of a transcript that probably meant a name of the
//! selection, as data for the window. A thin layer over `knowledge::repair::propose`, which does the
//! reading; this file only shapes the answer for the webview and applies the hidden-names rule.
//!
//! Rules that do not move (`docs/DECISIONS.md`, "Speech", D7):
//!
//! - **Read only.** Nothing here writes: not the knowledge base, not the text. The window shows
//!   `heard -> display` and the user accepts; a confirmed correction teaches the base nothing.
//! - **Only the selection.** The candidates are names of the selected files, as in `propose`.
//! - **Hidden names stay hidden.** While the setting hides names and the user has not asked to see
//!   them, the answer holds a count and no name: the canonical names never reach the window.
//! - **Generic.** A candidate is an entity id and its display name. Nothing here reads a type, so a
//!   person, an organisation or a type added later behave alike.
//! - **Any failure gives an empty answer.** The dictation then works as a plain dictation.
//!
//! The spans are counted in UTF-16 units of the transcript, which is how the window slices a
//! string; `propose` counts bytes.

use rusqlite::Connection;
use serde::Serialize;

use crate::error::AppError;
use crate::knowledge::query::QuerySelection;
use crate::knowledge::repair::{self, RepairProposal};
use crate::knowledge::targeting::QueryVocabulary;

/// A name offered in place of the words heard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairChoice {
    pub entity_id: i64,
    pub display: String,
}

/// Words of the transcript and the names they may have meant, best first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairItem {
    /// UTF-16 offsets of the words heard in the transcript that was read.
    pub start: usize,
    pub end: usize,
    pub heard: String,
    /// `suggest` (one strong candidate) or `ask` (the user picks).
    pub action: String,
    pub candidates: Vec<RepairChoice>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairSet {
    pub proposals: Vec<RepairItem>,
    /// How many proposals were left out because names are hidden. Never a name.
    pub hidden_count: usize,
}

/// The number of UTF-16 units of `text[..byte]`.
fn utf16_offset(text: &str, byte: usize) -> usize {
    text.get(..byte)
        .map_or(0, |head| head.encode_utf16().count())
}

fn item_of(transcript: &str, proposal: RepairProposal) -> RepairItem {
    RepairItem {
        start: utf16_offset(transcript, proposal.span.start),
        end: utf16_offset(transcript, proposal.span.end),
        heard: proposal.heard,
        action: proposal.action.as_code().to_string(),
        candidates: proposal
            .candidates
            .into_iter()
            .map(|candidate| RepairChoice {
                entity_id: candidate.entity_id,
                display: candidate.display,
            })
            .collect(),
    }
}

/// The suggestions for a transcript, read against the files of the selection.
/// `names_allowed` is false while the setting hides names and the user has not revealed them: the
/// answer then carries the count only.
pub fn propose_for(
    connection: &Connection,
    vocabulary: &QueryVocabulary,
    transcript: &str,
    selection: Option<&QuerySelection<'_>>,
    case_reliable: bool,
    names_allowed: bool,
) -> Result<RepairSet, AppError> {
    let proposals = repair::propose(connection, vocabulary, transcript, selection, case_reliable)?;
    if !names_allowed {
        return Ok(RepairSet {
            proposals: Vec::new(),
            hidden_count: proposals.len(),
        });
    }
    Ok(RepairSet {
        proposals: proposals
            .into_iter()
            .map(|proposal| item_of(transcript, proposal))
            .collect(),
        hidden_count: 0,
    })
}

/// Any failure of the reading is an empty answer: the transcript stays as it was heard.
pub fn or_nothing(result: Result<RepairSet, AppError>) -> RepairSet {
    result.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_count_utf16_units_not_bytes() {
        let text = "Meet L\u{e8}vre then Dupond";
        let byte = text.find("then").expect("found");

        assert_eq!(
            utf16_offset(text, byte),
            byte - 1,
            "one accent, one byte more"
        );
        assert_eq!(utf16_offset(text, text.len()), text.encode_utf16().count());
    }

    #[test]
    fn an_offset_inside_a_character_or_past_the_end_is_zero() {
        assert_eq!(utf16_offset("\u{e8}a", 1), 0);
        assert_eq!(utf16_offset("ab", 9), 0);
    }

    #[test]
    fn a_failure_is_an_empty_answer() {
        assert_eq!(
            or_nothing(Err(AppError::KnowledgeUnavailable)),
            RepairSet::default()
        );
    }

    #[test]
    fn the_answer_is_camel_case_data_without_a_sentence() {
        let set = RepairSet {
            proposals: vec![RepairItem {
                start: 3,
                end: 9,
                heard: "Dupond".into(),
                action: "suggest".into(),
                candidates: vec![RepairChoice {
                    entity_id: 7,
                    display: "Jean Dupont".into(),
                }],
            }],
            hidden_count: 0,
        };
        let json = serde_json::to_value(&set).expect("serialises");

        assert_eq!(json["proposals"][0]["candidates"][0]["entityId"], 7);
        assert_eq!(json["hiddenCount"], 0);
    }
}
