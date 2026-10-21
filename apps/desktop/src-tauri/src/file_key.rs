//! Which file the index means: the key of a file, and the only place that builds or reads it.
//!
//! Two words, never mixed (`docs/DECISIONS.md`, "root and family"). The **root** is the physical
//! folder a file sits in, `documents` or `data`: the two folders of the settings. The **family** is
//! how the file is read, by its extension: the document pipeline or the tabular one. `Domain`
//! (`file_profile::build`) is the family wherever the code or the index says `domain`.
//!
//! A file sitting in the root of its own family (a document in the Documents folder, a workbook in
//! the Data folder) is stored under its path inside that root, exactly as before this module existed:
//! every row written earlier stays valid. A file of the other family sitting in a root (a workbook
//! in the Documents folder, a document in the Data folder) is **foreign** and is stored under the key
//! `/<root>/<path inside that root>`, so it can never collide with a native file of the same path.
//!
//! The leading slash is the point. A native key is a relative path made of plain names, so it never
//! starts with one; and every resolver that turns a path from the webview into a file
//! (`reveal::resolve_plain_file`) refuses a root component, so a foreign key handed to code that does
//! not know about roots fails closed instead of opening another file.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::file_profile::build::Domain;
use crate::file_record::FileKind;
use crate::work_folder::components_for_comparison;

/// The physical folder a file sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Root {
    Documents,
    Data,
}

impl Root {
    /// The word the interface and the key use. Same words as the finder's domains.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Documents => "documents",
            Self::Data => "data",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "documents" => Some(Self::Documents),
            "data" => Some(Self::Data),
            _ => None,
        }
    }

    /// The other folder.
    pub fn other(self) -> Self {
        match self {
            Self::Documents => Self::Data,
            Self::Data => Self::Documents,
        }
    }
}

/// The root that holds a family's own files.
pub fn home_root(family: Domain) -> Root {
    match family {
        Domain::Documents => Root::Documents,
        Domain::Data => Root::Data,
    }
}

/// The family that reads an extension (lowercase, no dot), or `None` when no pipeline of the product
/// does. The one place the two families are told apart.
pub fn family_of_extension(extension: &str) -> Option<Domain> {
    let kind = FileKind::from_extension(extension);
    if kind.is_document() {
        Some(Domain::Documents)
    } else if kind == FileKind::TabularCandidate {
        Some(Domain::Data)
    } else {
        None
    }
}

/// The key of a file of `family` that sits in `root` at `path` (a path inside that root, forward
/// slashes). The path itself when the root is the family's own; `/<root>/<path>` otherwise.
pub fn index_key(root: Root, family: Domain, path: &str) -> String {
    if root == home_root(family) {
        path.to_string()
    } else {
        format!("/{}/{}", root.as_str(), path)
    }
}

/// What is left of `path` once the folder `base` is taken off, as plain names joined by forward
/// slashes. `None` when `path` is not under `base` (compared by components, with the platform's case
/// rule: `C:\Docs` is not `C:\Docs2`), is `base` itself, or would climb out of it.
fn inside(base: &Path, path: &Path) -> Option<String> {
    let base_parts = components_for_comparison(base);
    let path_parts = components_for_comparison(path);
    if base_parts.len() >= path_parts.len() || path_parts[..base_parts.len()] != base_parts[..] {
        return None;
    }
    let mut names = Vec::new();
    for component in path.components().skip(base_parts.len()) {
        match component {
            Component::Normal(name) => names.push(name.to_string_lossy().to_string()),
            _ => return None,
        }
    }
    Some(names.join("/"))
}

/// The one rule that decides the key of a file, from its **absolute path** and the folder of its own
/// family (`own`) and the other one (`other`), each when chosen (`docs/DECISIONS.md`, "any two folders,
/// never a duplicate"):
///
/// 1. under `own`: the path inside it, a plain key;
/// 2. else under `other`: `/<root of other>/<path inside other>`;
/// 3. under neither: `None`, the file is not read.
///
/// A file lying under both folders (equal or nested folders) takes rule 1: its own family's folder wins,
/// so a physical file can never have two keys. Everything that gives a file a key calls this.
pub fn key_under(
    family: Domain,
    own: Option<&Path>,
    other: Option<&Path>,
    absolute: &Path,
) -> Option<String> {
    if let Some(path) = own.and_then(|own| inside(own, absolute)) {
        return Some(path);
    }
    let path = inside(other?, absolute)?;
    Some(index_key(home_root(family).other(), family, &path))
}

/// [`key_under`] with the two folders given by what they are, the Documents folder and the Data folder.
pub fn key_for(
    family: Domain,
    documents: Option<&Path>,
    data: Option<&Path>,
    absolute: &Path,
) -> Option<String> {
    match home_root(family) {
        Root::Documents => key_under(family, documents, data, absolute),
        Root::Data => key_under(family, data, documents, absolute),
    }
}

/// What makes two paths the same physical file, however they were reached: the components, with the
/// platform's case rule. The belt and braces of every list of files: a list is de-duplicated on this
/// before any key is written, so a mistake in the rule could not store a file twice.
pub fn physical_identity(path: &Path) -> String {
    components_for_comparison(path).join("\u{0}")
}

/// The root and the path inside it that a key of `family` stands for. `None` for a key that starts
/// with a slash but names no root or no path: such a key is never produced here and is refused.
pub fn parse_key(family: Domain, key: &str) -> Option<(Root, &str)> {
    let Some(rest) = key.strip_prefix('/') else {
        return Some((home_root(family), key));
    };
    let (name, path) = rest.split_once('/')?;
    let root = Root::from_name(name)?;
    if path.is_empty() || root == home_root(family) {
        return None;
    }
    Some((root, path))
}

/// The path inside its folder that a key stands for, whichever family reads it: the key itself for a native
/// file, the part after the folder's name for a foreign one. What a name is made of.
pub fn plain_path(key: &str) -> &str {
    match key.strip_prefix('/').and_then(|rest| rest.split_once('/')) {
        Some((name, path)) if Root::from_name(name).is_some() && !path.is_empty() => path,
        _ => key,
    }
}

/// Whether the key is a foreign file's.
pub fn is_foreign_key(key: &str) -> bool {
    key.starts_with('/')
}

/// The two folders as one pass of a family sees them: its own, and the other one when it is chosen.
/// The only thing that turns a key back into a place on the disk, and only for a folder the settings
/// name (the allow-list): a key naming a folder that is not chosen resolves to nothing.
#[derive(Debug, Clone, Copy)]
pub struct FamilyRoots<'a> {
    pub family: Domain,
    pub native: &'a Path,
    pub foreign: Option<&'a Path>,
}

impl<'a> FamilyRoots<'a> {
    pub fn new(family: Domain, native: &'a Path, foreign: Option<&'a Path>) -> Self {
        Self {
            family,
            native,
            foreign,
        }
    }

    /// The key of the file at `absolute`, by the one rule (`key_under`).
    pub fn key_of(&self, absolute: &Path) -> Option<String> {
        key_under(self.family, Some(self.native), self.foreign, absolute)
    }

    /// The file a key of this family names, rebuilt from plain names only: `..`, a drive letter or an
    /// absolute path inside it, and a root that is not chosen, all give `None`. Nothing is read here.
    pub fn resolve(&self, key: &str) -> Option<PathBuf> {
        let (root, path) = parse_key(self.family, key)?;
        let base = if root == home_root(self.family) {
            self.native
        } else {
            self.foreign?
        };
        let mut target = base.to_path_buf();
        let mut any = false;
        for component in Path::new(path).components() {
            match component {
                Component::Normal(part) => {
                    target.push(part);
                    any = true;
                }
                _ => return None,
            }
        }
        any.then_some(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_native_file_keeps_its_plain_path() {
        assert_eq!(
            index_key(Root::Documents, Domain::Documents, "Courriers/a.pdf"),
            "Courriers/a.pdf"
        );
        assert_eq!(
            index_key(Root::Data, Domain::Data, "factures.xlsx"),
            "factures.xlsx"
        );
    }

    #[test]
    fn a_foreign_file_is_keyed_by_the_folder_it_sits_in() {
        assert_eq!(
            index_key(Root::Documents, Domain::Data, "Factures/2025.xlsx"),
            "/documents/Factures/2025.xlsx"
        );
        assert_eq!(
            index_key(Root::Data, Domain::Documents, "Courriers/a.pdf"),
            "/data/Courriers/a.pdf"
        );
    }

    #[test]
    fn the_same_path_in_both_roots_gives_two_keys() {
        let own = index_key(Root::Data, Domain::Data, "x.csv");
        let foreign = index_key(Root::Documents, Domain::Data, "x.csv");
        assert_ne!(own, foreign);
        assert_eq!(parse_key(Domain::Data, &own), Some((Root::Data, "x.csv")));
        assert_eq!(
            parse_key(Domain::Data, &foreign),
            Some((Root::Documents, "x.csv"))
        );
    }

    #[test]
    fn a_key_parses_back_to_its_root_and_path() {
        for (root, family, path) in [
            (Root::Documents, Domain::Documents, "a/b c.pdf"),
            (Root::Data, Domain::Documents, "a/b c.pdf"),
            (Root::Documents, Domain::Data, "t.xlsx"),
            (Root::Data, Domain::Data, "t.xlsx"),
        ] {
            let key = index_key(root, family, path);
            assert_eq!(parse_key(family, &key), Some((root, path)));
        }
    }

    #[test]
    fn a_key_that_names_no_root_or_the_family_own_root_is_refused() {
        assert_eq!(parse_key(Domain::Documents, "/etc/passwd"), None);
        assert_eq!(parse_key(Domain::Documents, "/data"), None);
        assert_eq!(parse_key(Domain::Documents, "/data/"), None);
        // The own root is never written as a prefix: that spelling would give a second key to a file.
        assert_eq!(parse_key(Domain::Documents, "/documents/a.pdf"), None);
        assert_eq!(parse_key(Domain::Data, "/data/a.csv"), None);
    }

    #[test]
    fn a_name_with_a_colon_is_never_mistaken_for_a_foreign_file() {
        assert_eq!(
            parse_key(Domain::Data, "data:x.xlsx"),
            Some((Root::Data, "data:x.xlsx"))
        );
        assert!(!is_foreign_key("data:x.xlsx"));
        assert!(is_foreign_key("/data/x.pdf"));
    }

    #[test]
    fn the_plain_path_of_a_key_never_carries_the_folder_it_was_found_in() {
        assert_eq!(plain_path("/data/Courriers/a.pdf"), "Courriers/a.pdf");
        assert_eq!(plain_path("/documents/t.xlsx"), "t.xlsx");
        assert_eq!(plain_path("Courriers/a.pdf"), "Courriers/a.pdf");
        assert_eq!(plain_path("/elsewhere/a.pdf"), "/elsewhere/a.pdf");
    }

    #[test]
    fn the_family_is_decided_by_the_extension_alone() {
        assert_eq!(family_of_extension("pdf"), Some(Domain::Documents));
        assert_eq!(family_of_extension("docx"), Some(Domain::Documents));
        assert_eq!(family_of_extension("xlsx"), Some(Domain::Data));
        assert_eq!(family_of_extension("csv"), Some(Domain::Data));
        assert_eq!(family_of_extension("ods"), Some(Domain::Data));
        assert_eq!(family_of_extension("ppt"), None);
        assert_eq!(family_of_extension(""), None);
    }

    #[test]
    fn a_key_resolves_against_its_own_root_and_nowhere_else() {
        let documents = Path::new("docs-root");
        let data = Path::new("data-root");
        let for_documents = FamilyRoots::new(Domain::Documents, documents, Some(data));
        assert_eq!(
            for_documents.resolve("Courriers/a.pdf"),
            Some(documents.join("Courriers").join("a.pdf"))
        );
        assert_eq!(
            for_documents.resolve("/data/Courriers/a.pdf"),
            Some(data.join("Courriers").join("a.pdf"))
        );
        let for_data = FamilyRoots::new(Domain::Data, data, Some(documents));
        assert_eq!(
            for_data.resolve("/documents/t.xlsx"),
            Some(documents.join("t.xlsx"))
        );
    }

    #[test]
    fn a_key_naming_a_folder_that_is_not_chosen_resolves_to_nothing() {
        let alone = FamilyRoots::new(Domain::Documents, Path::new("docs-root"), None);
        assert_eq!(alone.resolve("/data/a.pdf"), None);
    }

    #[test]
    fn a_key_cannot_climb_out_of_its_root() {
        let roots = FamilyRoots::new(
            Domain::Documents,
            Path::new("docs-root"),
            Some(Path::new("data-root")),
        );
        assert_eq!(roots.resolve("../secret.pdf"), None);
        assert_eq!(roots.resolve("/data/../secret.pdf"), None);
        assert_eq!(roots.resolve("a/../../secret.pdf"), None);
        assert_eq!(roots.resolve(""), None);
        assert_eq!(roots.resolve("/data/"), None);
    }

    /// The key of `absolute` for `family`, the folders given as paths under a made-up parent.
    fn key(family: Domain, documents: &str, data: &str, absolute: &str) -> Option<String> {
        key_for(
            family,
            Some(Path::new(documents)),
            Some(Path::new(data)),
            Path::new(absolute),
        )
    }

    #[test]
    fn separate_folders_key_a_file_of_the_other_family_by_the_folder_it_sits_in() {
        let (docs, data) = ("r/docs", "r/data");
        assert_eq!(
            key(Domain::Documents, docs, data, "r/docs/a/b.pdf").as_deref(),
            Some("a/b.pdf")
        );
        assert_eq!(
            key(Domain::Data, docs, data, "r/data/t.xlsx").as_deref(),
            Some("t.xlsx")
        );
        assert_eq!(
            key(Domain::Data, docs, data, "r/docs/x/t.xlsx").as_deref(),
            Some("/documents/x/t.xlsx")
        );
        assert_eq!(
            key(Domain::Documents, docs, data, "r/data/n.txt").as_deref(),
            Some("/data/n.txt")
        );
        assert_eq!(
            key(Domain::Documents, docs, data, "r/elsewhere/n.txt"),
            None
        );
    }

    #[test]
    fn the_same_folder_has_no_foreign_key_at_all() {
        for family in [Domain::Documents, Domain::Data] {
            let found = key(family, "r/both", "r/both", "r/both/sub/f.ext");
            assert_eq!(found.as_deref(), Some("sub/f.ext"));
        }
    }

    #[test]
    fn tables_inside_documents_key_each_file_once_and_by_its_own_family() {
        let (docs, data) = ("r/docs", "r/docs/Tables");
        // A workbook in the nested folder is native for the tabular family, relative to that folder.
        assert_eq!(
            key(Domain::Data, docs, data, "r/docs/Tables/a.xlsx").as_deref(),
            Some("a.xlsx")
        );
        // A workbook elsewhere in the Documents folder is foreign to it.
        assert_eq!(
            key(Domain::Data, docs, data, "r/docs/b.xlsx").as_deref(),
            Some("/documents/b.xlsx")
        );
        // A document in the nested folder is native for the document family, relative to the Documents folder.
        assert_eq!(
            key(Domain::Documents, docs, data, "r/docs/Tables/c.pdf").as_deref(),
            Some("Tables/c.pdf")
        );
    }

    #[test]
    fn documents_inside_tables_key_each_file_once_and_by_its_own_family() {
        let (docs, data) = ("r/data/Docs", "r/data");
        assert_eq!(
            key(Domain::Documents, docs, data, "r/data/Docs/c.pdf").as_deref(),
            Some("c.pdf")
        );
        assert_eq!(
            key(Domain::Documents, docs, data, "r/data/d.pdf").as_deref(),
            Some("/data/d.pdf")
        );
        assert_eq!(
            key(Domain::Data, docs, data, "r/data/Docs/t.xlsx").as_deref(),
            Some("Docs/t.xlsx")
        );
    }

    #[test]
    fn a_folder_is_compared_by_components_never_by_prefix() {
        assert_eq!(
            key(Domain::Documents, "r/Docs", "r/Data", "r/Docs2/a.pdf"),
            None
        );
        // Neither the folder itself nor a way out of it is a file of it.
        assert_eq!(key(Domain::Documents, "r/Docs", "r/Data", "r/Docs"), None);
        assert_eq!(
            key(
                Domain::Documents,
                "r/Docs",
                "r/Data",
                "r/Docs/../Other/a.pdf"
            ),
            None
        );
    }

    #[cfg(windows)]
    #[test]
    fn a_folder_that_differs_only_by_case_is_the_same_folder_on_this_platform() {
        assert_eq!(
            key(Domain::Documents, "R/DOCS", "R/Data", "r/docs/Tables/a.pdf").as_deref(),
            Some("Tables/a.pdf")
        );
        assert_eq!(
            physical_identity(Path::new("R/Docs/A.pdf")),
            physical_identity(Path::new("r/docs/a.PDF"))
        );
    }

    #[test]
    fn a_key_is_given_by_the_family_roots_the_same_way() {
        let roots = FamilyRoots::new(
            Domain::Data,
            Path::new("r/docs/Tables"),
            Some(Path::new("r/docs")),
        );
        assert_eq!(
            roots.key_of(Path::new("r/docs/Tables/a.xlsx")).as_deref(),
            Some("a.xlsx")
        );
        assert_eq!(
            roots.key_of(Path::new("r/docs/b.xlsx")).as_deref(),
            Some("/documents/b.xlsx")
        );
    }

    #[test]
    fn the_viewer_and_the_file_manager_refuse_a_foreign_key() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("data")).unwrap();
        std::fs::write(root.path().join("data").join("a.pdf"), b"x").unwrap();
        // The same key, given to code that does not know about roots, opens nothing.
        let refused = crate::reveal::resolve_plain_file(root.path(), "/data/a.pdf");
        assert!(matches!(
            refused,
            Err(crate::reveal::PathRefusal::UnsafePath)
        ));
    }
}
