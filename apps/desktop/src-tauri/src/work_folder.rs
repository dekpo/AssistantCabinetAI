//! The work folder allow-list.
//!
//! One folder is writable, like a project root. The rules live here, in code, so no prompt and no
//! view can widen them: a whole disk, a system tree and the whole of Documents are refused, while
//! a dedicated subfolder of Documents - the shape the pilot practice uses - is accepted.

use std::path::{Component, Path, PathBuf};

use crate::error::AppError;

/// Removed immediately. Windows only reports a protected or read-only folder when a write is
/// actually attempted, so remembering a folder we cannot write into would fail later instead.
const WRITE_PROBE_NAME: &str = ".assistant-cabinet-write-check";

pub struct WorkFolderPolicy {
    /// Refused together with everything inside them.
    forbidden_trees: Vec<PathBuf>,
    /// Refused only when chosen exactly: a subfolder of Documents is the intended shape.
    forbidden_exactly: Vec<PathBuf>,
}

impl WorkFolderPolicy {
    pub fn new(forbidden_trees: Vec<PathBuf>, forbidden_exactly: Vec<PathBuf>) -> Self {
        Self {
            forbidden_trees,
            forbidden_exactly,
        }
    }

    /// `personal` holds the folders the platform reports as the user's own - home, Documents,
    /// Desktop, Downloads - which may contain the work folder but must never be it.
    pub fn for_machine(home: Option<PathBuf>, personal: Vec<PathBuf>) -> Self {
        let mut forbidden_exactly = personal;
        if let Some(home) = home.clone() {
            forbidden_exactly.push(home);
        }
        Self::new(system_trees(home.as_deref()), forbidden_exactly)
    }

    /// The rules that need no disk access, so they can be tested on paths from any platform.
    pub fn check_path(&self, path: &Path) -> Result<(), AppError> {
        let shown = display(path);

        if is_unc(path) {
            return Err(AppError::WorkFolderUncNotSupported { path: shown });
        }
        if !path.is_absolute() {
            return Err(AppError::WorkFolderPathNotAbsolute { path: shown });
        }
        if is_filesystem_root(path) {
            return Err(AppError::WorkFolderIsDriveRoot { path: shown });
        }
        for tree in &self.forbidden_trees {
            if is_same_or_inside(path, tree) {
                return Err(AppError::WorkFolderIsProtected { path: shown });
            }
        }
        for folder in &self.forbidden_exactly {
            if components_for_comparison(path) == components_for_comparison(folder) {
                return Err(AppError::WorkFolderIsProtected { path: shown });
            }
        }
        Ok(())
    }

    /// The rules plus the disk: it exists, it is a real directory, and we can write in it.
    pub fn validate(&self, path: &Path) -> Result<PathBuf, AppError> {
        self.check_path(path)?;

        let metadata = std::fs::symlink_metadata(path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => AppError::WorkFolderNotFound {
                path: display(path),
            },
            _ => AppError::WorkFolderNotWritable {
                path: display(path),
            },
        })?;
        if metadata.file_type().is_symlink() {
            return Err(AppError::WorkFolderIsSymlink {
                path: display(path),
            });
        }
        if !metadata.is_dir() {
            return Err(AppError::WorkFolderNotADirectory {
                path: display(path),
            });
        }

        // Resolve first, then check again: a junction or a `..` could otherwise point the
        // allow-list at a tree the rules above just refused.
        let resolved = dunce::canonicalize(path).map_err(|_| AppError::WorkFolderNotFound {
            path: display(path),
        })?;
        self.check_path(&resolved)?;
        probe_writable(&resolved)?;
        Ok(resolved)
    }
}

pub fn display(path: &Path) -> String {
    dunce::simplified(path).display().to_string()
}

fn probe_writable(path: &Path) -> Result<(), AppError> {
    let probe = path.join(WRITE_PROBE_NAME);
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(_) => Err(AppError::WorkFolderNotWritable {
            path: display(path),
        }),
    }
}

fn is_unc(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Prefix(prefix))
            if matches!(
                prefix.kind(),
                std::path::Prefix::UNC(_, _) | std::path::Prefix::VerbatimUNC(_, _)
            )
    ) || path.to_string_lossy().starts_with("\\\\")
}

fn is_filesystem_root(path: &Path) -> bool {
    path.components()
        .all(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
}

/// Windows compares paths without regard to case, so the allow-list must too.
fn components_for_comparison(path: &Path) -> Vec<String> {
    path.components()
        .map(|component| {
            let text = component.as_os_str().to_string_lossy().to_string();
            if cfg!(windows) {
                text.to_lowercase()
            } else {
                text
            }
        })
        .collect()
}

fn is_same_or_inside(candidate: &Path, ancestor: &Path) -> bool {
    let ancestor = components_for_comparison(ancestor);
    let candidate = components_for_comparison(candidate);
    ancestor.len() <= candidate.len() && candidate[..ancestor.len()] == ancestor[..]
}

fn system_trees(home: Option<&Path>) -> Vec<PathBuf> {
    let mut trees: Vec<PathBuf> = Vec::new();

    #[cfg(windows)]
    {
        for variable in [
            "SystemRoot",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramData",
            "APPDATA",
            "LOCALAPPDATA",
        ] {
            if let Ok(value) = std::env::var(variable) {
                trees.push(PathBuf::from(value));
            }
        }
        if let Some(home) = home {
            trees.push(home.join("AppData"));
        }
    }

    #[cfg(not(windows))]
    {
        for path in [
            "/System",
            "/Library",
            "/Applications",
            "/usr",
            "/bin",
            "/sbin",
            "/etc",
            "/var",
            "/opt",
            "/private",
        ] {
            trees.push(PathBuf::from(path));
        }
        if let Some(home) = home {
            trees.push(home.join("Library"));
        }
    }

    trees
}

#[cfg(test)]
mod tests {
    use super::*;

    fn windows_policy() -> WorkFolderPolicy {
        WorkFolderPolicy::new(
            vec![
                PathBuf::from("C:\\Windows"),
                PathBuf::from("C:\\Users\\practice\\AppData"),
            ],
            vec![
                PathBuf::from("C:\\Users\\practice"),
                PathBuf::from("C:\\Users\\practice\\Documents"),
            ],
        )
    }

    fn code_of(result: Result<(), AppError>) -> String {
        result.expect_err("expected a refusal").code().to_string()
    }

    #[test]
    fn a_dedicated_subfolder_is_accepted() {
        let policy = windows_policy();
        let chosen = Path::new("C:\\Users\\practice\\Documents\\AssistantCabinet\\work");

        assert!(policy.check_path(chosen).is_ok());
    }

    #[test]
    fn the_whole_of_documents_is_refused() {
        let policy = windows_policy();

        assert_eq!(
            code_of(policy.check_path(Path::new("C:\\Users\\practice\\Documents"))),
            "work_folder_is_protected"
        );
    }

    #[test]
    fn the_home_folder_itself_is_refused() {
        let policy = windows_policy();

        assert_eq!(
            code_of(policy.check_path(Path::new("C:\\Users\\practice"))),
            "work_folder_is_protected"
        );
    }

    #[test]
    fn a_system_tree_is_refused_including_what_is_inside_it() {
        let policy = windows_policy();

        assert_eq!(
            code_of(policy.check_path(Path::new("C:\\Windows\\System32\\drivers"))),
            "work_folder_is_protected"
        );
        assert_eq!(
            code_of(policy.check_path(Path::new("C:\\Users\\practice\\AppData\\Local\\Temp"))),
            "work_folder_is_protected"
        );
    }

    #[test]
    fn case_does_not_open_a_way_around_the_list() {
        let policy = windows_policy();
        let same_folder_other_case = Path::new("c:\\users\\PRACTICE\\documents");

        // Only meaningful where the file system ignores case, which is where the risk is.
        if cfg!(windows) {
            assert_eq!(
                code_of(policy.check_path(same_folder_other_case)),
                "work_folder_is_protected"
            );
        }
    }

    #[test]
    fn a_drive_root_is_refused() {
        let policy = windows_policy();
        let root = if cfg!(windows) { "C:\\" } else { "/" };

        assert_eq!(
            code_of(policy.check_path(Path::new(root))),
            "work_folder_is_drive_root"
        );
    }

    #[test]
    fn a_relative_path_is_refused() {
        let policy = windows_policy();

        assert_eq!(
            code_of(policy.check_path(Path::new("work"))),
            "work_folder_path_not_absolute"
        );
    }

    #[test]
    fn a_network_folder_is_refused_for_now() {
        let policy = windows_policy();

        assert_eq!(
            code_of(policy.check_path(Path::new("\\\\server\\share\\work"))),
            "work_folder_unc_not_supported"
        );
    }

    #[test]
    fn a_folder_that_does_not_exist_is_refused_by_validate() {
        let policy = WorkFolderPolicy::new(Vec::new(), Vec::new());
        let directory = tempfile::tempdir().expect("temporary folder");
        let missing = directory.path().join("never-created");

        let error = policy.validate(&missing).expect_err("expected a refusal");

        assert_eq!(error.code(), "work_folder_not_found");
    }

    #[test]
    fn a_file_is_not_a_folder() {
        let policy = WorkFolderPolicy::new(Vec::new(), Vec::new());
        let directory = tempfile::tempdir().expect("temporary folder");
        let file = directory.path().join("letter.txt");
        std::fs::write(&file, b"fixture").expect("writes the fixture");

        let error = policy.validate(&file).expect_err("expected a refusal");

        assert_eq!(error.code(), "work_folder_not_a_directory");
    }

    #[test]
    fn a_real_writable_folder_is_accepted_and_returned_resolved() {
        let policy = WorkFolderPolicy::new(Vec::new(), Vec::new());
        let directory = tempfile::tempdir().expect("temporary folder");
        let chosen = directory.path().join("work");
        std::fs::create_dir(&chosen).expect("creates the folder");

        let accepted = policy.validate(&chosen).expect("accepted");

        assert!(accepted.is_absolute());
        // The probe file must not survive validation.
        assert!(!accepted.join(WRITE_PROBE_NAME).exists());
    }
}
