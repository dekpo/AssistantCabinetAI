//! Which files a conversation is about: a narrowing of the Work Folder allow-list.
//!
//! Two shapes: the whole folder, or an explicit set she ticked. A narrowing only ever picks among
//! files the inventory already holds: it copies nothing and reaches outside nothing
//! (`docs/SPRINT-2-ASSESSMENT.md` section E).
//!
//! An explicit set with **no document in it** is a choice too, and since 27 September 2026 the
//! interface's default: the conversation is answered without excerpts rather than refused
//! (`docs/SELECTION-AND-MEMORY.md`). It still reads no document. What keeps that safe is telling
//! "she chose nothing" apart from "she chose files that can no longer be used", which is why
//! `ScopeResolution` reports the first on its own rather than leaving it to an empty inventory.
//!
//! Deliberately absent, so they cannot drift from real membership or from what consumes them: a
//! scope id (nothing persists a session yet), the allowed domains (derived on demand from each
//! member's `FileRecord::kind`), and a purpose label or status enum.
//!
//! Sprint 2b adds `sheet_names` to `ScopeEntry`: a selected workbook may be narrowed to some of
//! its sheets. `FileRecord` stays sheet-agnostic (`docs/ARCHITECTURE.md`'s rule that a domain
//! fact lives with the domain that produced it), so the restriction cannot live on the resolved
//! inventory the way the rest of a scope does. `ScopeResolution::sheet_restrictions` carries it
//! instead, keyed by relative path, and the tabular engine is the only reader of it: retrieval
//! and the document router never look at it.

use serde::{Deserialize, Serialize};

use crate::inventory::WorkFolderInventory;

/// How wide a conversation may look.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "entries")]
pub enum ScopeMode {
    /// Every retrieval or tabular operation may use the whole inventory.
    WholeFolder,
    /// Only these entries. Retrieval and the folder-question router must not look past this set.
    /// No entries at all means she chose no document: answered without excerpts.
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
    /// Restricts a selected workbook to these sheets. Empty means every sheet it holds -
    /// meaningless for a document entry, and never consulted for one. Sheet names, not indices,
    /// because they are what a workbook's own inventory names them by and what survives a column
    /// being inserted elsewhere in the file.
    #[serde(default)]
    pub sheet_names: Vec<String>,
}

/// One conversation's scope, built from both selection lists before every question: the
/// documents she ticked in the Documents Folder card (`mode`) and the workbooks she ticked in the
/// Data Folder card (`data_mode`). Two modes rather than one mixed list, because each is relative
/// to its own folder - the same relative path can exist in both - and because "tous" means the
/// whole of one folder, never of the other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisScope {
    /// The Documents Folder selection. Everything in this module's `resolve` reads this one only.
    pub mode: ScopeMode,
    /// The Data Folder selection. Absent on the wire means no table, which is what every caller
    /// sent before the Data Folder could be selected from. `WholeFolder` is every green workbook
    /// in the Data Folder; `Explicit` holds entries relative to the Data Folder.
    #[serde(default = "ScopeMode::nothing")]
    pub data_mode: ScopeMode,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ScopeMode {
    /// An explicit selection with no entry: "aucun".
    pub fn nothing() -> Self {
        Self::Explicit(Vec::new())
    }

    /// Whether this selection chose anything at all. "Tous" did, even over an empty folder.
    pub fn chose_something(&self) -> bool {
        match self {
            Self::WholeFolder => true,
            Self::Explicit(entries) => !entries.is_empty(),
        }
    }
}

/// Which grounding tier a scope asks for (`docs/SELECTION-AND-MEMORY.md`, the grounding priority
/// chain), decided from the two selections alone, before any file is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundingTier {
    /// Documents chosen, no table: tier 1, retrieval. Also "neither", tier 3, which the document
    /// path already tells apart through `ScopeResolution::no_documents_chosen`.
    Documents,
    /// At least one table chosen and no document: tier 2, the tabular engine, no model.
    TablesOnly,
    /// Both. Refused explicitly (`AppError::DocumentsAndTablesTogether`) until combining the two
    /// is designed; never silently routed to one side.
    DocumentsAndTables,
}

impl AnalysisScope {
    /// The default scope: nothing is narrowed.
    pub fn whole_folder(now: i64) -> Self {
        Self {
            mode: ScopeMode::WholeFolder,
            data_mode: ScopeMode::nothing(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn tier(&self) -> GroundingTier {
        match (self.mode.chose_something(), self.data_mode.chose_something()) {
            (true, true) => GroundingTier::DocumentsAndTables,
            (false, true) => GroundingTier::TablesOnly,
            (_, false) => GroundingTier::Documents,
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
                no_documents_chosen: false,
                missing: Vec::new(),
                changed: Vec::new(),
                sheet_restrictions: std::collections::BTreeMap::new(),
            };
        };

        let mut members = Vec::new();
        let mut missing = Vec::new();
        let mut changed = Vec::new();
        let mut sheet_restrictions = std::collections::BTreeMap::new();
        for entry in entries {
            match full.find_by_relative_path(&entry.relative_path) {
                None => missing.push(entry.relative_path.clone()),
                Some(record) if record.id != entry.pinned_id => {
                    changed.push(entry.relative_path.clone())
                }
                Some(record) => {
                    if !entry.sheet_names.is_empty() {
                        sheet_restrictions
                            .insert(entry.relative_path.clone(), entry.sheet_names.clone());
                    }
                    members.push(record.clone());
                }
            }
        }
        ScopeResolution {
            inventory: WorkFolderInventory::from_records(full.root(), members),
            narrowed: true,
            // Documents only: a table-only choice never reaches this function's caller on the
            // document path, because `tier` routes it to the tabular engine first.
            no_documents_chosen: entries.is_empty(),
            missing,
            changed,
            sheet_restrictions,
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
    /// She chose no document at all. The one case answered without excerpts. Never true for a
    /// selection whose files vanished or changed: that is `missing` or `changed`, and refused.
    pub no_documents_chosen: bool,
    /// Entries whose file is no longer in the folder.
    pub missing: Vec<String>,
    /// Entries whose file is there but no longer has the content that was pinned.
    pub changed: Vec<String>,
    /// Sheet names a surviving entry was narrowed to, keyed by relative path. A path absent here
    /// carries no restriction: every sheet the workbook holds is in scope. Read only by the
    /// tabular engine (`tabular::engine::execute`), which must treat a sheet outside this list
    /// exactly as it treats a sheet that does not exist - never distinguishing the two, or a
    /// question could probe which sheets exist outside the scope she chose.
    pub sheet_restrictions: std::collections::BTreeMap<String, Vec<String>>,
}

impl ScopeResolution {
    /// The sheets a workbook at `relative_path` is restricted to, if any. `None` means
    /// unrestricted - every sheet the workbook holds.
    pub fn sheets_allowed(&self, relative_path: &str) -> Option<&[String]> {
        self.sheet_restrictions
            .get(relative_path)
            .map(Vec::as_slice)
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
                sheet_names: Vec::new(),
            }]),
            data_mode: ScopeMode::nothing(),
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
        explicit_with_sheets(
            &entries
                .iter()
                .map(|(path, id)| (*path, *id, &[][..]))
                .collect::<Vec<_>>(),
        )
    }

    fn explicit_with_sheets(entries: &[(&str, &str, &[&str])]) -> AnalysisScope {
        AnalysisScope {
            mode: ScopeMode::Explicit(
                entries
                    .iter()
                    .map(|(path, id, sheets)| ScopeEntry {
                        relative_path: path.to_string(),
                        pinned_id: id.to_string(),
                        added_at: 1,
                        sheet_names: sheets.iter().map(|s| s.to_string()).collect(),
                    })
                    .collect(),
            ),
            data_mode: ScopeMode::nothing(),
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
    fn choosing_nothing_is_reported_as_such() {
        let resolution = explicit(&[]).resolve(&folder());
        assert!(resolution.no_documents_chosen);
        assert!(resolution.missing.is_empty() && resolution.changed.is_empty());
    }

    #[test]
    fn choosing_files_that_are_gone_is_not_choosing_nothing() {
        // The distinction the whole rule rests on: she named documents, so answering without them
        // would be the quiet failure. This one is refused upstream, never answered without files.
        let resolution = explicit(&[("gone.txt", "id-x"), ("a.txt", "old-id")]).resolve(&folder());
        assert!(!resolution.no_documents_chosen);
        assert!(resolution.inventory.is_empty());
        assert_eq!(resolution.missing, vec!["gone.txt".to_string()]);
        assert_eq!(resolution.changed, vec!["a.txt".to_string()]);
    }

    #[test]
    fn the_whole_folder_and_a_real_choice_are_not_choosing_nothing() {
        assert!(
            !AnalysisScope::whole_folder(1)
                .resolve(&folder())
                .no_documents_chosen
        );
        assert!(
            !explicit(&[("b.txt", "id-b")])
                .resolve(&folder())
                .no_documents_chosen
        );
    }

    #[test]
    fn a_sheet_restriction_is_reported_only_for_the_entry_that_carries_one() {
        let resolution = explicit_with_sheets(&[
            ("a.txt", "id-a", &["Facturation"]),
            ("b.txt", "id-b", &[]),
        ])
        .resolve(&folder());

        assert_eq!(
            resolution.sheets_allowed("a.txt"),
            Some(&["Facturation".to_string()][..])
        );
        assert_eq!(resolution.sheets_allowed("b.txt"), None);
        assert_eq!(resolution.sheets_allowed("c.txt"), None);
    }

    #[test]
    fn an_empty_sheet_names_list_means_unrestricted() {
        let resolution = explicit_with_sheets(&[("a.txt", "id-a", &[])]).resolve(&folder());

        assert_eq!(resolution.sheets_allowed("a.txt"), None);
    }

    #[test]
    fn a_sheet_restriction_on_a_changed_or_missing_entry_is_not_reported() {
        // The entry did not survive, so nothing at all should be said about its sheets - a
        // restriction on a workbook that was rejected would be a stray fact nobody can act on.
        let resolution = explicit_with_sheets(&[
            ("a.txt", "old-id", &["Facturation"]),
            ("gone.txt", "id-x", &["Consultations"]),
        ])
        .resolve(&folder());

        assert!(resolution.sheet_restrictions.is_empty());
    }

    #[test]
    fn the_whole_folder_scope_never_carries_a_sheet_restriction() {
        let resolution = AnalysisScope::whole_folder(1).resolve(&folder());

        assert!(resolution.sheet_restrictions.is_empty());
    }

    #[test]
    fn sheet_names_round_trip_through_the_wire_form() {
        let scope = explicit_with_sheets(&[("data.csv", "id-1", &["Facturation", "Stock"])]);
        let json = serde_json::to_value(&scope).unwrap();

        let ScopeMode::Explicit(entries) = &scope.mode else {
            panic!("expected an explicit scope");
        };
        assert_eq!(
            json["mode"]["entries"][0]["sheetNames"],
            serde_json::json!(["Facturation", "Stock"])
        );
        let back: AnalysisScope = serde_json::from_value(json).unwrap();
        assert_eq!(back, scope);
        assert_eq!(entries[0].sheet_names, vec!["Facturation", "Stock"]);
    }

    #[test]
    fn a_scope_entry_with_no_sheet_names_field_still_deserialises() {
        // Written before this field existed, or written by a caller that only ever means "every
        // sheet". `#[serde(default)]` is what keeps that reading as "unrestricted" rather than a
        // parse failure.
        let json = serde_json::json!({
            "mode": {
                "kind": "explicit",
                "entries": [
                    { "relativePath": "a.csv", "pinnedId": "id-a", "addedAt": 1 }
                ]
            },
            "createdAt": 1,
            "updatedAt": 1
        });

        let scope: AnalysisScope = serde_json::from_value(json).unwrap();
        let ScopeMode::Explicit(entries) = &scope.mode else {
            panic!("expected an explicit scope");
        };
        assert!(entries[0].sheet_names.is_empty());
    }

    fn with_data(documents: ScopeMode, data: ScopeMode) -> AnalysisScope {
        AnalysisScope {
            mode: documents,
            data_mode: data,
            created_at: 1,
            updated_at: 1,
        }
    }

    fn one_entry(path: &str) -> ScopeMode {
        ScopeMode::Explicit(vec![ScopeEntry {
            relative_path: path.to_string(),
            pinned_id: "id".to_string(),
            added_at: 1,
            sheet_names: Vec::new(),
        }])
    }

    #[test]
    fn the_tier_is_decided_from_the_two_selections_alone() {
        use GroundingTier::*;
        let none = ScopeMode::nothing;
        assert_eq!(with_data(none(), none()).tier(), Documents);
        assert_eq!(with_data(one_entry("a.pdf"), none()).tier(), Documents);
        assert_eq!(with_data(ScopeMode::WholeFolder, none()).tier(), Documents);
        assert_eq!(with_data(none(), one_entry("a.csv")).tier(), TablesOnly);
        assert_eq!(with_data(none(), ScopeMode::WholeFolder).tier(), TablesOnly);
        assert_eq!(
            with_data(one_entry("a.pdf"), one_entry("a.csv")).tier(),
            DocumentsAndTables
        );
        assert_eq!(
            with_data(ScopeMode::WholeFolder, ScopeMode::WholeFolder).tier(),
            DocumentsAndTables
        );
    }

    #[test]
    fn a_scope_sent_before_the_data_folder_existed_chooses_no_table() {
        let json = serde_json::json!({
            "mode": { "kind": "explicit", "entries": [] },
            "createdAt": 1,
            "updatedAt": 1
        });

        let scope: AnalysisScope = serde_json::from_value(json).unwrap();

        assert_eq!(scope.data_mode, ScopeMode::nothing());
        assert_eq!(scope.tier(), GroundingTier::Documents);
    }

    #[test]
    fn the_data_selection_never_leaks_into_the_document_resolution() {
        // A workbook ticked in the Data Folder is relative to that folder. Resolving the document
        // selection must not see it at all - not as a member, and not as a missing file either.
        let resolution =
            with_data(ScopeMode::nothing(), one_entry("a.txt")).resolve(&folder());

        assert!(resolution.no_documents_chosen);
        assert!(resolution.inventory.is_empty());
        assert!(resolution.missing.is_empty());
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
