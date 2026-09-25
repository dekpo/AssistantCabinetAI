//! Which files a conversation is about: an optional narrowing of the Work Folder allow-list.
//!
//! The default is the whole folder, which is today's exact behaviour expressed as a case of this
//! type rather than a special path around it. A user opts into narrowing it, and a narrowing only
//! ever picks among files the inventory already holds: it copies nothing and reaches outside
//! nothing (`docs/SPRINT-2-ASSESSMENT.md` section E).
//!
//! Deliberately absent, so they cannot drift from real membership or from what consumes them: a
//! scope id (nothing persists a session yet), the allowed domains (derived on demand from each
//! member's `FileRecord::kind`), a purpose label, a status enum, and sheet restrictions (Sprint 2b
//! adds `sheet_names` to `ScopeEntry` when a tabular engine exists to read them).

use serde::Serialize;

/// How wide a conversation may look.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "entries")]
pub enum ScopeMode {
    /// Default. Every retrieval or tabular operation may use the whole inventory.
    WholeFolder,
    /// Only these entries. Retrieval and the folder-question router must not look past this set.
    Explicit(Vec<ScopeEntry>),
}

/// One file a narrowed scope keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeEntry {
    /// The stable handle into `WorkFolderInventory`. Never an absolute path.
    pub relative_path: String,
    /// `FileRecord::id` (content SHA-256, or the path-identity fallback) pinned at the moment
    /// this entry was added, so a silent file replacement mid-conversation can be detected.
    pub pinned_id: String,
    pub added_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisScope {
    pub mode: ScopeMode,
    pub created_at: i64,
    pub updated_at: i64,
}

impl AnalysisScope {
    /// The default scope: nothing is narrowed.
    pub fn whole_folder(now: i64) -> Self {
        Self {
            mode: ScopeMode::WholeFolder,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_folder_is_the_default_shape() {
        let scope = AnalysisScope::whole_folder(1_000);
        assert_eq!(scope.mode, ScopeMode::WholeFolder);
        assert_eq!(scope.created_at, 1_000);
        assert_eq!(scope.updated_at, 1_000);
    }

    #[test]
    fn an_explicit_scope_holds_relative_handles_with_their_pinned_ids() {
        let scope = AnalysisScope {
            mode: ScopeMode::Explicit(vec![ScopeEntry {
                relative_path: "2026/mars/bilan.pdf".to_string(),
                pinned_id: "abc123".to_string(),
                added_at: 2_000,
            }]),
            created_at: 1_000,
            updated_at: 2_000,
        };
        let ScopeMode::Explicit(entries) = &scope.mode else {
            panic!("expected an explicit scope");
        };
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].relative_path, "2026/mars/bilan.pdf");
        assert_eq!(entries[0].pinned_id, "abc123");
        assert!(scope.updated_at >= scope.created_at);
    }

    #[test]
    fn the_wire_form_is_camel_case_with_a_tagged_mode() {
        let json = serde_json::to_value(AnalysisScope::whole_folder(5)).unwrap();
        assert_eq!(json["mode"]["kind"], "whole_folder");
        assert_eq!(json["createdAt"], 5);
        assert_eq!(json["updatedAt"], 5);
    }
}
