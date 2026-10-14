//! The user's own business types: storage and engine (`docs/LEXICAL-RETRIEVAL-DESIGN.md` 5.1).
//!
//! A type is a **rule as data**: a code, a label, and the phrases that are evidence for it in each place a
//! phrase can be found (the file name, the title, the header lines, the first words of the text, a
//! workbook's sheet names and column headers). The shipped rules live in the packs; the user's own are
//! **manual rows** of the local index (`file_profile_user_types`), which no Analyse, no Reset of a folder and no
//! rebuild of a profile ever writes or deletes, like the manual rows of the knowledge base.
//!
//! A user code always starts with [`USER_PREFIX`], so it can never meet a shipped code, and its label is the
//! row's, not a catalogue entry. A user type is **specific** (never the generic catch-all), and the rules of
//! `types.rs` weigh it like any other: assigned only with enough evidence, an ambiguity with another
//! specific type leaves the file with none.
//!
//! **Re-evaluating** after a change ([`reevaluate`]) reads the **stored profile only, never a file**: the file
//! name, the stored title, the stored header words, a workbook's stored sheet and column names. It is a cheap
//! pass the user can start at once; it cannot see the body of the text (the profile does not keep it), so the
//! next Analyse, which builds every profile again because the vocabulary changed, completes the reading. A
//! type already assigned is only changed by a user type that beats it under the same rules.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::name::{words_of, NameProfile};
use super::types::{classify_among, combine, TypeAssignment, TypeInput};
use super::vocabulary::{DocumentTypeRule, Phrase, ProfileVocabulary, TypeScope};
use crate::error::AppError;

/// Every code of a user type starts with this, whatever the user typed.
pub const USER_PREFIX: &str = "user_";
/// The longest code, prefix included.
const MAX_CODE_CHARS: usize = 48;
const MAX_LABEL_CHARS: usize = 80;
const MAX_PHRASE_CHARS: usize = 80;
/// The most phrases in one place, and in the whole rule.
const MAX_PHRASES: usize = 40;

/// One business type of the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserType {
    /// `user_` and lower case letters, digits and `_`.
    pub code: String,
    /// `documents` or `data`.
    pub scope: String,
    /// The name the user gave it, shown as it is (user data, not a catalogue entry).
    pub label: String,
    #[serde(default)]
    pub name: Vec<String>,
    #[serde(default)]
    pub title: Vec<String>,
    #[serde(default)]
    pub header: Vec<String>,
    #[serde(default)]
    pub body: Vec<String>,
    #[serde(default)]
    pub sheet: Vec<String>,
}

fn refused() -> AppError {
    AppError::KnowledgeOperationRefused
}

fn scope_of(value: &str) -> Option<TypeScope> {
    match value {
        "documents" => Some(TypeScope::Documents),
        "data" => Some(TypeScope::Data),
        _ => None,
    }
}

impl UserType {
    /// Refuse a definition that cannot be a rule. Nothing is kept of it.
    pub fn validate(&self) -> Result<(), AppError> {
        let code_ok = self.code.starts_with(USER_PREFIX)
            && self.code.len() > USER_PREFIX.len()
            && self.code.chars().count() <= MAX_CODE_CHARS
            && self
                .code
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        let label = self.label.trim();
        if !code_ok
            || scope_of(&self.scope).is_none()
            || label.is_empty()
            || label.chars().count() > MAX_LABEL_CHARS
        {
            return Err(refused());
        }
        let mut count = 0;
        for list in self.places() {
            if list.len() > MAX_PHRASES {
                return Err(refused());
            }
            for phrase in list {
                if phrase.chars().count() > MAX_PHRASE_CHARS || words_of(phrase).is_empty() {
                    return Err(refused());
                }
                count += 1;
            }
        }
        // A rule with no phrase could never be evidence for anything.
        if count == 0 || count > MAX_PHRASES * 5 {
            return Err(refused());
        }
        Ok(())
    }

    fn places(&self) -> [&Vec<String>; 5] {
        [
            &self.name,
            &self.title,
            &self.header,
            &self.body,
            &self.sheet,
        ]
    }

    /// The rule the engine weighs. `None` for a scope that is not one (never, after `validate`).
    fn rule(&self) -> Option<DocumentTypeRule> {
        let phrases =
            |list: &[String]| -> Vec<Phrase> { list.iter().map(|p| words_of(p)).collect() };
        Some(DocumentTypeRule {
            code: self.code.clone(),
            scope: scope_of(&self.scope)?,
            generic: false,
            name: phrases(&self.name),
            title: phrases(&self.title),
            header: phrases(&self.header),
            body: phrases(&self.body),
            sheet: phrases(&self.sheet),
        })
    }
}

/// Keep `user_type`, replacing the one with the same code. The Analyse never touches the rows.
pub fn save(connection: &Connection, user_type: &UserType) -> Result<(), AppError> {
    user_type.validate()?;
    let mut kept = user_type.clone();
    kept.label = kept.label.trim().to_string();
    let json = serde_json::to_string(&kept).map_err(|_| AppError::IndexUnavailable)?;
    connection
        .execute(
            "INSERT INTO file_profile_user_types (code, scope, label, rule_json) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(code) DO UPDATE SET scope = excluded.scope, label = excluded.label, rule_json = excluded.rule_json",
            params![kept.code, kept.scope, kept.label, json],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    Ok(())
}

/// Forget one type of the user. The profiles that carried it keep the code until the next Analyse or
/// re-evaluation reads them again.
pub fn delete(connection: &Connection, code: &str) -> Result<bool, AppError> {
    let removed = connection
        .execute(
            "DELETE FROM file_profile_user_types WHERE code = ?1",
            [code],
        )
        .map_err(|_| AppError::IndexUnavailable)?;
    Ok(removed > 0)
}

/// Every type of the user, in code order. A row that no longer reads as a rule is left out.
pub fn list(connection: &Connection) -> Result<Vec<UserType>, AppError> {
    let mut statement = connection
        .prepare("SELECT rule_json FROM file_profile_user_types ORDER BY code")
        .map_err(|_| AppError::IndexUnavailable)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|_| AppError::IndexUnavailable)?;
    let mut found = Vec::new();
    for json in rows.flatten() {
        if let Ok(user_type) = serde_json::from_str::<UserType>(&json) {
            if user_type.validate().is_ok() {
                found.push(user_type);
            }
        }
    }
    Ok(found)
}

/// The vocabulary with the user's types added. Their phrases are part of its hash, so the next Analyse builds
/// the profiles again and none is built again while the types stay the same.
pub fn with_stored_types(
    vocabulary: ProfileVocabulary,
    connection: &Connection,
) -> ProfileVocabulary {
    match list(connection) {
        Ok(types) => vocabulary.with_user_types(types.iter().filter_map(UserType::rule).collect()),
        Err(_) => vocabulary,
    }
}

/// One profile as the re-evaluation reads it: folder, path, stored type, its confidence, stored title.
type StoredType = (String, String, Option<String>, Option<f64>, Option<String>);

/// What a re-evaluation did, in counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reevaluation {
    /// Profiles read.
    pub examined: usize,
    /// Profiles whose business type changed.
    pub changed: usize,
}

/// Weigh the user's types against the **stored profiles**, with no file read. `vocabulary` already holds the
/// user's types ([`with_stored_types`]).
pub fn reevaluate(
    connection: &Connection,
    vocabulary: &ProfileVocabulary,
) -> Result<Reevaluation, AppError> {
    let db = |_| AppError::IndexUnavailable;
    let mut summary = Reevaluation::default();
    let rows: Vec<StoredType> = {
        let mut statement = connection
            .prepare(
                "SELECT domain, relative_path, business_type, business_type_confidence, title
                 FROM file_profiles ORDER BY domain, relative_path",
            )
            .map_err(db)?;
        let found = statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(db)?;
        found.collect::<Result<_, _>>().map_err(db)?
    };
    let transaction = connection.unchecked_transaction().map_err(db)?;
    for (domain, path, stored_type, stored_confidence, title) in rows {
        summary.examined += 1;
        let Some(scope) = scope_of(&domain) else {
            continue;
        };
        let original: Option<String> = transaction
            .query_row(
                "SELECT original_name FROM file_profile_original_names WHERE domain = ?1 AND relative_path = ?2",
                params![domain, path],
                |row| row.get(0),
            )
            .optional()
            .map_err(db)?;
        let name = NameProfile::build(&path, original.as_deref());
        let title_words = title.as_deref().map(words_of).unwrap_or_default();
        let header_words: Vec<String> = transaction
            .query_row(
                "SELECT value FROM file_profile_fields WHERE domain = ?1 AND relative_path = ?2 AND kind = 'header_words'",
                params![domain, path],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(db)?
            .map(|words| words.split(' ').map(str::to_string).collect())
            .unwrap_or_default();
        let stored_places = |kind: &str| -> Result<Vec<Vec<String>>, AppError> {
            let mut statement = transaction
                .prepare(
                    "SELECT value FROM file_profile_fields WHERE domain = ?1 AND relative_path = ?2 AND kind = ?3 ORDER BY position",
                )
                .map_err(db)?;
            let found = statement
                .query_map(params![domain, path, kind], |row| row.get::<_, String>(0))
                .map_err(db)?;
            Ok(found.flatten().map(|value| words_of(&value)).collect())
        };
        let sheet_names = stored_places("sheet_name")?;
        let column_headers = stored_places("column_header")?;
        let input = TypeInput {
            scope,
            name_words: &name.name_words,
            title_words: &title_words,
            header_words: &header_words,
            // The body of the text is not part of the stored profile.
            body_words: &[],
            sheet_names: &sheet_names,
            column_headers: &column_headers,
        };
        let user = classify_among(vocabulary, &input, &|rule| {
            rule.code.starts_with(USER_PREFIX)
        });
        // A type of the user is decided again from the stored evidence (a rule may have changed or gone);
        // a shipped type stays unless a user type beats it.
        let shipped = stored_type
            .as_ref()
            .filter(|code| !code.starts_with(USER_PREFIX))
            .map(|code| TypeAssignment {
                code: code.clone(),
                confidence: stored_confidence.unwrap_or(0.0) as f32,
                sources: Vec::new(),
            });
        let existing_generic = shipped.as_ref().is_some_and(|assigned| {
            vocabulary
                .document_types()
                .any(|rule| rule.code == assigned.code && rule.generic)
        });
        let decided = combine(shipped, user, existing_generic);
        if decided.as_ref().map(|d| &d.code) == stored_type.as_ref() {
            continue;
        }
        transaction
            .execute(
                "UPDATE file_profiles SET business_type = ?3, business_type_confidence = ?4, business_type_sources = ?5
                 WHERE domain = ?1 AND relative_path = ?2",
                params![
                    domain,
                    path,
                    decided.as_ref().map(|d| d.code.clone()),
                    decided.as_ref().map(|d| f64::from(d.confidence)),
                    decided.as_ref().map(|d| {
                        d.sources
                            .iter()
                            .map(|source| source.as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    }),
                ],
            )
            .map_err(db)?;
        summary.changed += 1;
    }
    transaction.commit().map_err(db)?;
    Ok(summary)
}
