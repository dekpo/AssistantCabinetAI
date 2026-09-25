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

use serde::{Deserialize, Serialize};

use crate::inventory::WorkFolderInventory;

/// How wide a conversation may look.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "entries")]
pub enum ScopeMode {
    /// Default. Every retrieval or tabular operation may use the whole inventory.
    WholeFolder,
    /// Only these entries. Retrieval and the folder-question router must not look past this set.
    Explicit(Vec<ScopeEntry>),
}

/// One file a narrowed scope keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeEntry {
    /// The stable handle into `WorkFolderInventory`. Never an absolute path.
    pub relative_path: String,
    /// `FileRecord::id` (content SHA-256, or the path-identity fallback) pinned at the moment
    /// this entry was added, so a silent file replacement mid-conversation can be detected.
    pub pinned_id: String,
    pub added_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    /// The inventory this scope allows, and what could not be honoured.
    ///
    /// A narrowed scope yields an inventory holding only its members, so everything built on an
    /// inventory (the router, the file-reference resolver, the counts, the Work Folder context)
    /// cannot look past the scope without a line of scope-specific code. An entry is kept only
    /// while the file it pinned is still there: one that vanished is `missing`, and one whose
    /// content changed since it was added is `changed`. Neither is silently answered from, since
    /// the user chose a file that no longer exists in that form.
    pub fn resolve(&self, full: &WorkFolderInventory) -> ScopeResolution {
        let ScopeMode::Explicit(entries) = &self.mode else {
            return ScopeResolution {
                inventory: WorkFolderInventory::from_records(
                    full.root(),
                    full.all_files().to_vec(),
                ),
                narrowed: false,
                missing: Vec::new(),
                changed: Vec::new(),
            };
        };

        let mut members = Vec::new();
        let mut missing = Vec::new();
        let mut changed = Vec::new();
        for entry in entries {
            match full.find_by_relative_path(&entry.relative_path) {
                None => missing.push(entry.relative_path.clone()),
                Some(record) if record.id != entry.pinned_id => {
                    changed.push(entry.relative_path.clone())
                }
                Some(record) => members.push(record.clone()),
            }
        }
        ScopeResolution {
            inventory: WorkFolderInventory::from_records(full.root(), members),
            narrowed: true,
            missing,
            changed,
        }
    }
}

/// A scope applied to the inventory as it is now.
#[derive(Debug)]
pub struct ScopeResolution {
    /// Everything for a whole-folder scope; the surviving members for an explicit one.
    pub inventory: WorkFolderInventory,
    /// Whether retrieval must be held to `inventory` rather than to the whole index. An
    /// explicit scope with no survivors is still narrowed: it allows nothing, not everything.
    pub narrowed: bool,
    /// Entries whose file is no longer in the folder.
    pub missing: Vec<String>,
    /// Entries whose file is there but no longer has the content that was pinned.
    pub changed: Vec<String>,
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

    fn record(relative_path: &str, id: &str) -> crate::file_record::FileRecord {
        crate::file_record::FileRecord {
            id: id.to_string(),
            relative_path: relative_path.to_string(),
            name: relative_path.rsplit('/').next().unwrap().to_string(),
            stem: "x".to_string(),
            extension: "txt".to_string(),
            kind: crate::file_record::FileKind::DocumentText,
            mime_type: "text/plain".to_string(),
            size_bytes: 1,
            modified_at: None,
            sha256: Some(id.to_string()),
            readability: crate::file_record::Readability::Readable,
            processing_status: crate::file_record::ProcessingStatus::Pending,
            extraction_method: crate::file_record::ExtractionMethod::None,
            indexed: false,
            index_metadata: None,
        }
    }

    fn folder() -> WorkFolderInventory {
        WorkFolderInventory::from_records(
            std::path::Path::new("root"),
            vec![
                record("a.txt", "id-a"),
                record("b.txt", "id-b"),
                record("c.txt", "id-c"),
            ],
        )
    }

    fn explicit(entries: &[(&str, &str)]) -> AnalysisScope {
        AnalysisScope {
            mode: ScopeMode::Explicit(
                entries
                    .iter()
                    .map(|(path, id)| ScopeEntry {
                        relative_path: path.to_string(),
                        pinned_id: id.to_string(),
                        added_at: 1,
                    })
                    .collect(),
            ),
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn whole_folder_resolves_to_the_whole_inventory() {
        let resolution = AnalysisScope::whole_folder(1).resolve(&folder());
        assert!(!resolution.narrowed);
        assert_eq!(resolution.inventory.file_count(), 3);
        assert!(resolution.missing.is_empty() && resolution.changed.is_empty());
    }

    #[test]
    fn an_explicit_scope_keeps_only_its_members() {
        let resolution = explicit(&[("b.txt", "id-b")]).resolve(&folder());
        assert!(resolution.narrowed);
        assert_eq!(resolution.inventory.file_count(), 1);
        assert!(resolution.inventory.contains("b.txt"));
        assert!(!resolution.inventory.contains("a.txt"));
    }

    #[test]
    fn a_vanished_or_replaced_file_is_reported_and_not_kept() {
        let resolution = explicit(&[("a.txt", "old-id"), ("gone.txt", "id-x"), ("c.txt", "id-c")])
            .resolve(&folder());
        assert_eq!(resolution.changed, vec!["a.txt".to_string()]);
        assert_eq!(resolution.missing, vec!["gone.txt".to_string()]);
        assert_eq!(resolution.inventory.file_count(), 1);
        assert!(resolution.inventory.contains("c.txt"));
    }

    #[test]
    fn an_explicit_scope_with_no_survivors_allows_nothing() {
        let resolution = explicit(&[]).resolve(&folder());
        assert!(resolution.narrowed);
        assert!(resolution.inventory.is_empty());
    }

    #[test]
    fn the_wire_form_is_camel_case_with_a_tagged_mode() {
        let json = serde_json::to_value(AnalysisScope::whole_folder(5)).unwrap();
        assert_eq!(json["mode"]["kind"], "whole_folder");
        assert_eq!(json["createdAt"], 5);
        assert_eq!(json["updatedAt"], 5);
        let back: AnalysisScope = serde_json::from_value(json).unwrap();
        assert_eq!(back, AnalysisScope::whole_folder(5));
    }
}
