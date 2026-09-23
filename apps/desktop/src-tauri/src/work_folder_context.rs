//! What the model is told about the Work Folder, and what it is told never to do with it.
//!
//! Two separate things travel to the gateway when a question needs a model at all. Retrieved
//! excerpts are **document evidence**: what the files say. This context is **filesystem
//! evidence**: which files exist and what happened to each one. The contract below exists to stop
//! the model using either as a substitute for the other, because a document that claims to be a
//! PDF, or that names a file nobody has, is exactly the input that produces a confident wrong
//! answer.
//!
//! The block is generated from the inventory, never handwritten, and it is compact on purpose:
//! no absolute paths, no hashes, no document text, and only the view the question actually
//! needs. An inventory is metadata, and sending more of it than the question calls for spends
//! context for nothing.

use serde::Serialize;
use serde_json::{json, Value};

use crate::file_record::FileRecord;
use crate::inventory::{FolderNode, InventorySummary, WorkFolderInventory};

/// How much of the inventory one question needs. The builder produces a different block for
/// each, so a question about one file does not carry a listing of the whole folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextView {
    /// Counts only.
    Summary,
    /// Counts plus the folder hierarchy.
    Tree,
    /// Counts plus every file.
    FileList,
    /// Counts plus one file, in full.
    FileDetails { relative_path: String },
    /// Counts plus one file, as the subject of a content question whose evidence is retrieved
    /// separately.
    Targeted { relative_path: String },
}

/// The document side of the Work Folder, ready to be rendered into a prompt block.
///
/// `DocumentWorkFolderContext` is deliberately one of a family. A future `TabularWorkFolderContext`
/// will be built from the same `WorkFolderInventory` over the same `FileRecord`s and will carry
/// workbook structure instead of document structure (`docs/WORK-FOLDER-INVENTORY.md`). Nothing in
/// this type is shared with it except the inventory it reads.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentWorkFolderContext {
    /// The folder's own name. Never the absolute path: where the user keeps their files is not
    /// the model's business, and a home directory carries a person's name.
    pub root_identifier: String,
    pub summary: InventorySummary,
    pub hierarchy: Option<FolderNode>,
    pub files: Vec<FileRecord>,
    pub view: &'static str,
}

/// Build the context for one view.
pub fn build(inventory: &WorkFolderInventory, view: &ContextView) -> DocumentWorkFolderContext {
    let (hierarchy, files, name) = match view {
        ContextView::Summary => (None, Vec::new(), "summary"),
        ContextView::Tree => (Some(inventory.hierarchy()), Vec::new(), "tree"),
        ContextView::FileList => (None, inventory.all_files().to_vec(), "file_list"),
        ContextView::FileDetails { relative_path } => (
            None,
            inventory
                .find_by_relative_path(relative_path)
                .cloned()
                .into_iter()
                .collect(),
            "file_details",
        ),
        ContextView::Targeted { relative_path } => (
            None,
            inventory
                .find_by_relative_path(relative_path)
                .cloned()
                .into_iter()
                .collect(),
            "targeted",
        ),
    };

    DocumentWorkFolderContext {
        root_identifier: inventory.root_identifier(),
        summary: inventory.summary(),
        hierarchy,
        files,
        view: name,
    }
}

impl DocumentWorkFolderContext {
    /// The compact JSON the model receives. One file is one small object: identity, path, name,
    /// extension, kind, and what happened to it. No hash, no size, no absolute path, and never
    /// a line of the document itself.
    pub fn to_json(&self) -> Value {
        let mut body = json!({
            "root": self.root_identifier,
            "view": self.view,
            "summary": {
                "total_files": self.summary.total_files,
                "indexed_files": self.summary.indexed_files,
                "unreadable_files": self.summary.unreadable_files,
                "by_extension": self.summary.by_extension,
            },
        });
        if let Some(tree) = &self.hierarchy {
            body["hierarchy"] = json!(tree_lines(tree));
        }
        if !self.files.is_empty() {
            body["files"] = Value::Array(self.files.iter().map(file_entry).collect());
        }
        body
    }

    /// The block as it is appended to the system turn.
    pub fn to_prompt_block(&self) -> String {
        format!(
            "WORK_FOLDER_CONTEXT\n{}\n",
            serde_json::to_string(&self.to_json()).unwrap_or_else(|_| "{}".to_string())
        )
    }
}

/// One file, as the model sees it.
///
/// `FileRecord::id` is deliberately absent. It is the content SHA-256, and a document's hash is a
/// fingerprint that confirms which document it is to anyone holding a copy; the gateway has no
/// operation that needs one, and the relative path already identifies a file uniquely inside the
/// Work Folder. Identity stays on the workstation, where the resolver uses it.
fn file_entry(file: &FileRecord) -> Value {
    json!({
        "path": file.relative_path,
        "name": file.name,
        "extension": if file.extension.is_empty() {
            String::new()
        } else {
            format!(".{}", file.extension)
        },
        "kind": file.kind.as_code(),
        "readability": file.readability.as_code(),
        "processing": file.processing_status.as_code(),
        "extraction": file.extraction_method.as_code(),
    })
}

/// The hierarchy flattened to one indented line per folder, which costs a fraction of the
/// characters a nested object does and reads the same to a model.
fn tree_lines(node: &FolderNode) -> Vec<String> {
    let mut lines = Vec::new();
    collect_tree(node, 0, &mut lines);
    lines
}

fn collect_tree(node: &FolderNode, depth: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    for name in &node.files {
        lines.push(format!("{indent}{name}"));
    }
    for child in &node.folders {
        lines.push(format!("{indent}{}/", child.name));
        collect_tree(child, depth + 1, lines);
    }
}

/// The invariant the model is held to, in English like every other prompt body
/// (`docs/LANGUAGE-AND-LOCALE.md`). It says nothing about which language to answer in: the
/// gateway appends the output-language directive from `locale`, and duplicating that here would
/// give the model two masters.
///
/// It is appended to the existing retrieval instruction rather than replacing it, because the
/// two govern different evidence.
pub const WORK_FOLDER_KNOWLEDGE_CONTRACT: &str = "\
WORK FOLDER KNOWLEDGE CONTRACT\n\
The WORK_FOLDER_CONTEXT block below is authoritative for filesystem facts. Retrieved document \
excerpts are authoritative for what a document says. Never use one as a substitute for the other.\n\
- Never invent a file, a file name, a file extension or a path.\n\
- Never alter a file name or an extension, whatever a document's own text claims about itself.\n\
- Never infer how many files exist, or which files exist, from retrieved excerpts.\n\
- Never infer whether a file is indexed, readable or unreadable from its content.\n\
- Never claim an unreadable file was read, and never describe what it might contain.\n\
- Never present text obtained by optical character recognition as text the file itself carries.\n\
- When the user refers to a file, use the resolved file from the context block.\n\
- If several files match a reference, ask which one instead of choosing.\n\
- If no file matches, say that no matching file was found.\n";

/// The system turn for a question that needs the model: the retrieval instruction, the knowledge
/// contract, and the filesystem context.
pub fn build_system_turn(
    retrieval_instruction: &str,
    context: &DocumentWorkFolderContext,
) -> String {
    format!(
        "{retrieval_instruction}\n{WORK_FOLDER_KNOWLEDGE_CONTRACT}\n{}",
        context.to_prompt_block()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_record::{
        mime_type_for, split_name, ExtractionMethod, FileKind, ProcessingStatus, Readability,
    };
    use std::path::Path;

    fn record(relative_path: &str, readability: Readability) -> FileRecord {
        let name = relative_path
            .rsplit('/')
            .next()
            .unwrap_or(relative_path)
            .to_string();
        let (stem, extension) = split_name(&name);
        FileRecord {
            id: "a".repeat(64),
            relative_path: relative_path.to_string(),
            name,
            stem,
            mime_type: mime_type_for(&extension).to_string(),
            kind: FileKind::from_extension(&extension),
            extension,
            size_bytes: 1,
            modified_at: None,
            sha256: Some("a".repeat(64)),
            readability,
            processing_status: match readability {
                Readability::Readable => ProcessingStatus::Indexed,
                Readability::Unreadable => ProcessingStatus::Failed,
                Readability::NotAssessed => ProcessingStatus::Discovered,
            },
            extraction_method: match readability {
                Readability::Readable => ExtractionMethod::NativeText,
                _ => ExtractionMethod::None,
            },
            indexed: readability == Readability::Readable,
            index_metadata: None,
        }
    }

    fn inventory() -> WorkFolderInventory {
        WorkFolderInventory::from_records(
            Path::new("root").join("cabinet").as_path(),
            vec![
                record("2026/rapport.pdf", Readability::Readable),
                record("2026/scan.png", Readability::Unreadable),
                record("notes.txt", Readability::Readable),
            ],
        )
    }

    #[test]
    fn the_summary_view_carries_counts_and_no_file_listing() {
        let inventory = inventory();
        let context = build(&inventory, &ContextView::Summary);
        let body = context.to_json();

        assert_eq!(body["summary"]["total_files"], 3);
        assert_eq!(body["summary"]["indexed_files"], 2);
        assert_eq!(body["summary"]["unreadable_files"], 1);
        assert!(body.get("files").is_none());
        assert!(body.get("hierarchy").is_none());
    }

    #[test]
    fn the_file_list_view_carries_every_file_with_its_own_extension() {
        let inventory = inventory();
        let context = build(&inventory, &ContextView::FileList);
        let body = context.to_json();
        let files = body["files"].as_array().expect("a file listing");

        assert_eq!(files.len(), 3);
        assert_eq!(files[0]["path"], "2026/rapport.pdf");
        assert_eq!(files[0]["extension"], ".pdf");
        assert_eq!(files[1]["extension"], ".png");
        assert_eq!(files[1]["readability"], "unreadable");
        assert_eq!(files[2]["extraction"], "native_text");
    }

    #[test]
    fn a_targeted_view_carries_one_file_only() {
        let inventory = inventory();
        let context = build(
            &inventory,
            &ContextView::Targeted {
                relative_path: "notes.txt".into(),
            },
        );

        assert_eq!(context.files.len(), 1);
        assert_eq!(context.files[0].name, "notes.txt");
    }

    #[test]
    fn the_tree_view_carries_the_hierarchy() {
        let inventory = inventory();
        let context = build(&inventory, &ContextView::Tree);
        let body = context.to_json();
        let lines: Vec<String> = body["hierarchy"]
            .as_array()
            .expect("a hierarchy")
            .iter()
            .map(|line| line.as_str().unwrap_or_default().to_string())
            .collect();

        assert_eq!(
            lines,
            vec!["notes.txt", "2026/", "  rapport.pdf", "  scan.png"]
        );
    }

    #[test]
    fn the_block_carries_no_absolute_path_no_hash_and_no_document_text() {
        let inventory = inventory();
        let block = build(&inventory, &ContextView::FileList).to_prompt_block();

        assert!(block.starts_with("WORK_FOLDER_CONTEXT"));
        // The root travels as a folder name, so no separator and no home directory can appear.
        assert!(block.contains("\"root\":\"cabinet\""), "block: {block}");
        assert!(
            !block.contains(std::path::MAIN_SEPARATOR),
            "no machine path: {block}"
        );
        assert!(!block.contains(&"a".repeat(64)), "no content hash");
        assert!(!block.contains("sha256"));
        assert!(!block.contains("sizeBytes"));
        assert!(
            !block.contains("\"id\""),
            "identity stays on the workstation"
        );
    }

    #[test]
    fn the_contract_forbids_inventing_and_forbids_choosing_between_matches() {
        assert!(WORK_FOLDER_KNOWLEDGE_CONTRACT.contains("Never invent a file"));
        assert!(WORK_FOLDER_KNOWLEDGE_CONTRACT.contains("ask which one instead of choosing"));
        assert!(WORK_FOLDER_KNOWLEDGE_CONTRACT.contains("no matching file was found"));
        // The output language belongs to the gateway's own directive, not to this block.
        assert!(!WORK_FOLDER_KNOWLEDGE_CONTRACT
            .to_lowercase()
            .contains("french"));
        assert!(!WORK_FOLDER_KNOWLEDGE_CONTRACT
            .to_lowercase()
            .contains("answer in"));
    }

    #[test]
    fn the_system_turn_keeps_the_existing_retrieval_instruction() {
        let inventory = inventory();
        let context = build(&inventory, &ContextView::Summary);
        let turn = build_system_turn(crate::retrieval::RETRIEVAL_INSTRUCTION, &context);

        assert!(turn.contains("Answer only from"));
        assert!(turn.contains("WORK FOLDER KNOWLEDGE CONTRACT"));
        assert!(turn.contains("WORK_FOLDER_CONTEXT"));
    }
}
