//! The work folder allow-list.
//!
//! One folder is writable, like a project root. The rules live here, in code, so no prompt and no
//! view can widen them. A whole disk, a system tree and the whole of Documents are refused, and so
//! is anything a cloud client mirrors.
//!
//! That last rule is the reason the intended shape is a dedicated folder **directly in the home**
//! rather than under Documents. Windows Known Folder Move redirects Documents into OneDrive, and
//! iCloud's "Desktop & Documents" does the same on macOS: a work folder under Documents can be
//! copied off the machine without anyone deciding that it should be. See docs/CLIENT.md.

use std::path::{Component, Path, PathBuf};

use crate::error::AppError;

/// Removed immediately. Windows only reports a protected or read-only folder when a write is
/// actually attempted, so remembering a folder we cannot write into would fail later instead.
const WRITE_PROBE_NAME: &str = ".assistant-cabinet-write-check";

/// What we propose at first launch, directly in the home: `~/AssistantCabinetAI`.
pub const WORK_FOLDER_NAME: &str = "AssistantCabinetAI";

/// Folder names meaning "a sync client mirrors this tree". Matched as a prefix on every path
/// component, without regard to case, so `OneDrive - Contoso` and `GoogleDrive-someone@example.com`
/// are caught too. The second item is a product name: it travels as data for the interface to
/// show, and it is a trademark rather than a sentence, so it is not translated.
const CLOUD_FOLDER_PREFIXES: &[(&str, &str)] = &[
    ("onedrive", "OneDrive"),
    ("dropbox", "Dropbox"),
    ("google drive", "Google Drive"),
    ("googledrive", "Google Drive"),
    ("icloud", "iCloud Drive"),
    ("mobile documents", "iCloud Drive"),
    ("nextcloud", "Nextcloud"),
    ("owncloud", "ownCloud"),
    ("pcloud", "pCloud"),
    ("creative cloud files", "Adobe Creative Cloud"),
    ("box sync", "Box"),
];

/// Names too short to match as a prefix without catching innocent folders such as `Boxes`.
const CLOUD_FOLDER_EXACT: &[(&str, &str)] = &[("box", "Box")];

pub struct WorkFolderPolicy {
    /// Refused together with everything inside them.
    forbidden_trees: Vec<PathBuf>,
    /// Refused only when chosen exactly: a dedicated subfolder of one of these is still fine.
    forbidden_exactly: Vec<PathBuf>,
    /// Refused together with everything inside them, and reported separately, because the reason
    /// is not "this folder is precious" but "this folder is already a copy in someone's cloud".
    cloud_trees: Vec<PathBuf>,
}

impl WorkFolderPolicy {
    pub fn new(
        forbidden_trees: Vec<PathBuf>,
        forbidden_exactly: Vec<PathBuf>,
        cloud_trees: Vec<PathBuf>,
    ) -> Self {
        Self {
            forbidden_trees,
            forbidden_exactly,
            cloud_trees,
        }
    }

    /// `personal` holds the folders the platform reports as the user's own - home, Documents,
    /// Desktop, Downloads - which may contain the work folder but must never be it.
    pub fn for_machine(home: Option<PathBuf>, personal: Vec<PathBuf>) -> Self {
        let mut forbidden_exactly = personal;
        if let Some(home) = home.clone() {
            forbidden_exactly.push(home);
        }
        Self::new(
            system_trees(home.as_deref()),
            forbidden_exactly,
            cloud_trees(home.as_deref()),
        )
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
        // Before the generic rules: when Known Folder Move is on, the redirected Documents is both
        // protected and synchronised, and only the second reason tells her what to do about it.
        if let Some(service) = self.cloud_service(path) {
            return Err(AppError::WorkFolderIsCloudSynced {
                path: shown,
                service,
            });
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

    /// The rules plus the disk: it exists, it is a real directory, no sync client has taken it
    /// over, and we can write in it.
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
        refuse_cloud_placeholder(&resolved)?;
        probe_writable(&resolved)?;
        Ok(resolved)
    }

    /// The product mirroring this path, if any. Named roots first, because the platform told us
    /// about those; then folder names, which catch a client that is not running right now.
    fn cloud_service(&self, path: &Path) -> Option<String> {
        for tree in &self.cloud_trees {
            if is_same_or_inside(path, tree) {
                return Some(service_name_of(tree));
            }
        }
        cloud_product_in_path(path).map(str::to_string)
    }
}

/// Where we suggest she puts the work folder. Outside Documents on purpose.
pub fn suggested_work_folder(home: Option<&Path>) -> Option<PathBuf> {
    home.map(|home| home.join(WORK_FOLDER_NAME))
}

/// Create `~/AssistantCabinetAI` if it is missing, then apply the same rules as a folder she
/// picked. Nothing is created until she asks: declining leaves the profile untouched.
pub fn ensure_suggested(
    policy: &WorkFolderPolicy,
    home: Option<&Path>,
) -> Result<PathBuf, AppError> {
    let path = suggested_work_folder(home).ok_or(AppError::Internal)?;
    policy.check_path(&path)?;

    match std::fs::symlink_metadata(&path) {
        Ok(_) => policy.validate(&path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(&path).map_err(|_| AppError::WorkFolderNotWritable {
                path: display(&path),
            })?;
            match policy.validate(&path) {
                Ok(accepted) => Ok(accepted),
                Err(failed) => {
                    let _ = std::fs::remove_dir(&path);
                    Err(failed)
                }
            }
        }
        Err(_) => Err(AppError::WorkFolderNotWritable {
            path: display(&path),
        }),
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

/// The product behind a folder name, falling back to the name itself so the interface can always
/// say *what* it refused rather than "a cloud service".
fn service_name_of(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| display(path));
    cloud_product_for_name(&name)
        .map(str::to_string)
        .unwrap_or(name)
}

fn cloud_product_for_name(name: &str) -> Option<&'static str> {
    let lowered = name.to_lowercase();
    for (exact, product) in CLOUD_FOLDER_EXACT {
        if lowered == *exact {
            return Some(product);
        }
    }
    for (prefix, product) in CLOUD_FOLDER_PREFIXES {
        if lowered.starts_with(prefix) {
            return Some(product);
        }
    }
    None
}

fn cloud_product_in_path(path: &Path) -> Option<&'static str> {
    path.components().find_map(|component| match component {
        Component::Normal(text) => cloud_product_for_name(&text.to_string_lossy()),
        _ => None,
    })
}

/// Windows: refuse a folder a sync client has taken over, whichever client it is.
///
/// The path is already canonical, so junctions and symbolic links have been resolved away. A
/// reparse point still sitting on it or on one of its parents is a cloud placeholder - OneDrive
/// tags its whole tree with `IO_REPARSE_TAG_CLOUD` - so the attribute is enough, and it holds for
/// a product we have never heard of.
#[cfg(windows)]
fn refuse_cloud_placeholder(path: &Path) -> Result<(), AppError> {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

    for ancestor in path.ancestors() {
        let Ok(metadata) = std::fs::symlink_metadata(ancestor) else {
            continue;
        };
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0 {
            continue;
        }
        return Err(AppError::WorkFolderIsCloudSynced {
            path: display(path),
            service: service_name_of(ancestor),
        });
    }
    Ok(())
}

#[cfg(not(windows))]
fn refuse_cloud_placeholder(_path: &Path) -> Result<(), AppError> {
    Ok(())
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

/// The sync roots the platform itself reports, which beats guessing a folder name: a business
/// OneDrive is named after the tenant, and the user can move any of these roots.
fn cloud_trees(home: Option<&Path>) -> Vec<PathBuf> {
    let _ = &home;
    let mut trees: Vec<PathBuf> = Vec::new();

    #[cfg(windows)]
    {
        for variable in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
            match std::env::var(variable) {
                Ok(value) if !value.trim().is_empty() => trees.push(PathBuf::from(value)),
                _ => {}
            }
        }
    }

    #[cfg(not(windows))]
    {
        if let Some(home) = home {
            // Every File Provider client since macOS 12.3 mounts under CloudStorage; iCloud Drive
            // keeps its own tree beside it. Both are inside Library, which is already refused, so
            // these entries exist to produce the message that explains why.
            trees.push(home.join("Library").join("CloudStorage"));
            trees.push(home.join("Library").join("Mobile Documents"));
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
            vec![PathBuf::from("C:\\Users\\practice\\OneDrive")],
        )
    }

    fn code_of(result: Result<(), AppError>) -> String {
        result.expect_err("expected a refusal").code().to_string()
    }

    /// The code and the product name, which is the pair a refused sync folder has to carry.
    fn refusal(result: Result<(), AppError>) -> (String, String) {
        let error = result.expect_err("expected a refusal");
        let service = error.data()["service"]
            .as_str()
            .expect("the service travels as data")
            .to_string();
        (error.code().to_string(), service)
    }

    #[test]
    fn the_suggested_folder_sits_in_the_home_and_is_accepted() {
        let policy = windows_policy();
        let home = PathBuf::from("C:\\Users\\practice");

        let suggested = suggested_work_folder(Some(&home)).expect("a home was given");

        assert_eq!(suggested, home.join("AssistantCabinetAI"));
        assert!(policy.check_path(&suggested).is_ok());
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
    fn the_reported_sync_root_is_refused_with_everything_inside_it() {
        let policy = windows_policy();

        let error = policy.check_path(Path::new("C:\\Users\\practice\\OneDrive\\work"));

        assert_eq!(code_of(error), "work_folder_is_cloud_synced");
    }

    #[test]
    fn the_redirected_documents_folder_is_refused_as_synchronised() {
        // Known Folder Move puts Documents inside OneDrive. The old rules refused that folder
        // exactly and accepted a subfolder of it, which is how a patient file reaches Microsoft.
        let policy = windows_policy();
        let redirected =
            Path::new("C:\\Users\\practice\\OneDrive\\Documents\\AssistantCabinet\\work");

        let (code, service) = refusal(policy.check_path(redirected));

        assert_eq!(code, "work_folder_is_cloud_synced");
        assert_eq!(service, "OneDrive");
    }

    #[test]
    fn a_sync_folder_is_refused_by_name_even_when_the_platform_reported_nothing() {
        let policy = WorkFolderPolicy::new(Vec::new(), Vec::new(), Vec::new());

        for (path, expected) in [
            ("D:\\Dropbox\\cabinet", "Dropbox"),
            ("C:\\Users\\p\\OneDrive - Contoso\\cabinet", "OneDrive"),
            ("C:\\Users\\p\\Google Drive\\cabinet", "Google Drive"),
            ("C:\\Users\\p\\iCloudDrive\\cabinet", "iCloud Drive"),
            ("C:\\Users\\p\\Box\\cabinet", "Box"),
        ] {
            let (code, service) = refusal(policy.check_path(Path::new(path)));

            assert_eq!(code, "work_folder_is_cloud_synced", "{path}");
            assert_eq!(service, expected, "{path}");
        }
    }

    #[test]
    fn a_folder_that_merely_looks_like_a_sync_name_is_accepted() {
        // `Box` is matched exactly, so ordinary folders keep working.
        let policy = WorkFolderPolicy::new(Vec::new(), Vec::new(), Vec::new());

        assert!(policy
            .check_path(Path::new("C:\\Users\\practice\\Boxes\\cabinet"))
            .is_ok());
        assert!(policy
            .check_path(Path::new("C:\\Users\\practice\\Cabinet\\courriers"))
            .is_ok());
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
        assert_eq!(
            code_of(policy.check_path(Path::new("C:\\Users\\practice\\onedrive\\work"))),
            "work_folder_is_cloud_synced"
        );
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
        let policy = WorkFolderPolicy::new(Vec::new(), Vec::new(), Vec::new());
        let directory = tempfile::tempdir().expect("temporary folder");
        let missing = directory.path().join("never-created");

        let error = policy.validate(&missing).expect_err("expected a refusal");

        assert_eq!(error.code(), "work_folder_not_found");
    }

    #[test]
    fn a_file_is_not_a_folder() {
        let policy = WorkFolderPolicy::new(Vec::new(), Vec::new(), Vec::new());
        let directory = tempfile::tempdir().expect("temporary folder");
        let file = directory.path().join("letter.txt");
        std::fs::write(&file, b"fixture").expect("writes the fixture");

        let error = policy.validate(&file).expect_err("expected a refusal");

        assert_eq!(error.code(), "work_folder_not_a_directory");
    }

    #[test]
    fn a_real_writable_folder_is_accepted_and_returned_resolved() {
        let policy = WorkFolderPolicy::new(Vec::new(), Vec::new(), Vec::new());
        let directory = tempfile::tempdir().expect("temporary folder");
        let chosen = directory.path().join("work");
        std::fs::create_dir(&chosen).expect("creates the folder");

        let accepted = policy.validate(&chosen).expect("accepted");

        assert!(accepted.is_absolute());
        // The probe file must not survive validation.
        assert!(!accepted.join(WRITE_PROBE_NAME).exists());
    }

    fn home_policy(home: &Path) -> WorkFolderPolicy {
        WorkFolderPolicy::new(Vec::new(), vec![home.to_path_buf()], Vec::new())
    }

    #[test]
    fn creating_the_suggested_folder_makes_it_and_accepts_it() {
        let home = tempfile::tempdir().expect("temporary home");
        let policy = home_policy(home.path());
        let expected = home.path().join(WORK_FOLDER_NAME);

        let accepted = ensure_suggested(&policy, Some(home.path())).expect("created");

        assert!(expected.is_dir());
        assert_eq!(accepted, dunce::canonicalize(&expected).expect("resolves"));
        assert!(!accepted.join(WRITE_PROBE_NAME).exists());
    }

    #[test]
    fn creating_the_suggested_folder_a_second_time_reuses_it() {
        let home = tempfile::tempdir().expect("temporary home");
        let policy = home_policy(home.path());

        let first = ensure_suggested(&policy, Some(home.path())).expect("created");
        let second = ensure_suggested(&policy, Some(home.path())).expect("reused");

        assert_eq!(first, second);
    }

    #[test]
    fn a_file_blocking_the_suggested_name_is_refused_and_left_alone() {
        let home = tempfile::tempdir().expect("temporary home");
        let policy = home_policy(home.path());
        let blocking = home.path().join(WORK_FOLDER_NAME);
        std::fs::write(&blocking, b"not a folder").expect("writes the fixture");

        let error = ensure_suggested(&policy, Some(home.path())).expect_err("expected a refusal");

        assert_eq!(error.code(), "work_folder_not_a_directory");
        assert!(blocking.is_file());
    }

    #[test]
    fn a_suggested_folder_inside_a_sync_tree_is_refused_without_creating_it() {
        let home = tempfile::tempdir().expect("temporary home");
        let policy = WorkFolderPolicy::new(Vec::new(), Vec::new(), vec![home.path().to_path_buf()]);

        let error = ensure_suggested(&policy, Some(home.path())).expect_err("expected a refusal");

        assert_eq!(error.code(), "work_folder_is_cloud_synced");
        assert!(!home.path().join(WORK_FOLDER_NAME).exists());
    }
}
