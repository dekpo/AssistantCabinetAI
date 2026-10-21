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

    /// The key of a file of this family found in `root`.
    pub fn key_in(&self, root: Root, path: &str) -> String {
        index_key(root, self.family, path)
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
