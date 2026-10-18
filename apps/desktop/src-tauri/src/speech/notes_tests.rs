//! Tests of the notes backend. They drive the pure functions of `notes.rs` in temporary folders:
//! nothing here builds a Tauri `State` or an `AppHandle`.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{NaiveDate, NaiveDateTime};

use super::notes::{
    preview, save, NoteDraft, NoteFile, NoteFormat, SaveLog, Target, LOG_FILE_NAME, MAX_NOTES,
    MAX_NOTE_BYTES,
};
use crate::discovery::DiscoveredFile;
use crate::error::AppError;
use crate::extraction::extract;
use crate::work_folder::WorkFolderPolicy;

fn clock() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 18)
        .expect("date")
        .and_hms_opt(14, 32, 5)
        .expect("time")
}

fn open_policy() -> WorkFolderPolicy {
    WorkFolderPolicy::new(Vec::new(), Vec::new(), Vec::new())
}

fn draft(name: &str, text: &str) -> NoteDraft {
    NoteDraft {
        name: name.to_string(),
        text: text.to_string(),
        format: NoteFormat::Md,
    }
}

fn save_in(
    policy: &WorkFolderPolicy,
    root: &Path,
    drafts: &[NoteDraft],
) -> Result<Vec<NoteFile>, AppError> {
    let target = Target {
        policy,
        work_folder: root,
        folder_name: "Notes",
    };
    save(&target, drafts, clock(), None)
}

fn names_in(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .expect("folder reads")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    names.sort();
    names
}

fn code_of<T: std::fmt::Debug>(result: Result<T, AppError>) -> String {
    result.expect_err("a refusal").code().to_string()
}

// ---- names -----------------------------------------------------------------------------------

#[test]
fn a_file_is_named_by_its_local_time_then_the_cleaned_user_name() {
    let root = tempfile::tempdir().expect("root");
    let saved = save_in(
        &open_policy(),
        root.path(),
        &[draft("  Compte-rendu caf\u{e9} / 2e visite  ", "text")],
    )
    .expect("saves");

    assert_eq!(saved.len(), 1);
    assert_eq!(
        saved[0].file_name,
        "2026-10-18_14-32-05-Compte-rendu-cafe-2e-visite.md"
    );
    assert_eq!(
        saved[0].relative_path,
        "Notes/2026-10-18_14-32-05-Compte-rendu-cafe-2e-visite.md"
    );
}

#[test]
fn an_empty_name_gives_the_timestamp_alone_and_txt_is_an_option() {
    let root = tempfile::tempdir().expect("root");
    let mut plain = draft("", "one");
    plain.format = NoteFormat::Txt;
    let saved = save_in(&open_policy(), root.path(), &[plain, draft("   ", "two")]).expect("saves");

    assert_eq!(saved[0].file_name, "2026-10-18_14-32-05.txt");
    assert_eq!(saved[1].file_name, "2026-10-18_14-32-05.md");
}

#[test]
fn a_dot_in_the_name_never_makes_a_second_extension() {
    let root = tempfile::tempdir().expect("root");
    let saved = save_in(
        &open_policy(),
        root.path(),
        &[draft("report.final.docx", "x")],
    )
    .expect("saves");

    assert_eq!(
        saved[0].file_name,
        "2026-10-18_14-32-05-report-final-docx.md"
    );
}

#[test]
fn a_name_with_nothing_left_once_cleaned_is_refused_and_nothing_is_written() {
    let root = tempfile::tempdir().expect("root");
    for bad in ["???", "..", "\u{7b14}\u{8bb0}", "/ \\ :"] {
        let result = save_in(
            &open_policy(),
            root.path(),
            &[draft("fine", "a"), draft(bad, "b")],
        );
        assert_eq!(code_of(result), "notes_name_invalid", "name {bad:?}");
    }
    assert!(
        !root.path().join("Notes").exists(),
        "a refusal writes nothing, not even the folder"
    );
}

#[test]
fn the_folder_name_is_cleaned_again_and_never_leaves_the_documents_folder() {
    let root = tempfile::tempdir().expect("root");
    let policy = open_policy();
    for (typed, expected) in [("../outside", "outside"), ("", "Notes"), ("a/b", "a-b")] {
        let target = Target {
            policy: &policy,
            work_folder: root.path(),
            folder_name: typed,
        };
        let saved = save(&target, &[draft("n", "x")], clock(), None).expect("saves");
        assert!(
            saved[0].relative_path.starts_with(&format!("{expected}/")),
            "{typed:?} gave {}",
            saved[0].relative_path
        );
        assert!(root.path().join(expected).is_dir());
    }
}

// ---- never overwritten -----------------------------------------------------------------------

#[test]
fn a_taken_name_gets_a_number_and_the_first_file_is_never_touched() {
    let root = tempfile::tempdir().expect("root");
    let policy = open_policy();
    save_in(&policy, root.path(), &[draft("visit", "first")]).expect("first save");
    let second = save_in(&policy, root.path(), &[draft("visit", "second")]).expect("second save");
    let third = save_in(&policy, root.path(), &[draft("visit", "third")]).expect("third save");

    assert_eq!(second[0].file_name, "2026-10-18_14-32-05-visit-2.md");
    assert_eq!(third[0].file_name, "2026-10-18_14-32-05-visit-3.md");
    let folder = root.path().join("Notes");
    assert_eq!(
        fs::read_to_string(folder.join("2026-10-18_14-32-05-visit.md")).expect("reads"),
        "first"
    );
    assert_eq!(names_in(&folder).len(), 3);
}

#[test]
fn two_notes_with_the_same_name_in_one_save_are_numbered_too() {
    let root = tempfile::tempdir().expect("root");
    let saved = save_in(
        &open_policy(),
        root.path(),
        &[draft("same", "a"), draft("same", "b"), draft("SAME", "c")],
    )
    .expect("saves");
    let names: Vec<&str> = saved.iter().map(|file| file.file_name.as_str()).collect();

    assert_eq!(names[0], "2026-10-18_14-32-05-same.md");
    assert_eq!(names[1], "2026-10-18_14-32-05-same-2.md");
    assert_eq!(names[2], "2026-10-18_14-32-05-SAME-3.md");
}

#[test]
fn the_preview_gives_the_names_a_save_would_give_and_writes_nothing() {
    let root = tempfile::tempdir().expect("root");
    let policy = open_policy();
    let drafts = [draft("visit", "a\nb"), draft("visit", "c")];
    let target = Target {
        policy: &policy,
        work_folder: root.path(),
        folder_name: "Notes",
    };

    let before = preview(&target, &drafts, clock()).expect("previews");
    assert!(
        !root.path().join("Notes").exists(),
        "a preview does not even create the folder"
    );
    assert_eq!(before[0].lines, 2);
    assert_eq!(before[0].chars, 3);

    let written = save(&target, &drafts, clock(), None).expect("saves");
    assert_eq!(before, written);

    let after = preview(&target, &drafts, clock()).expect("previews again");
    assert_eq!(after[0].file_name, "2026-10-18_14-32-05-visit-3.md");
    assert_eq!(names_in(&root.path().join("Notes")).len(), 2);
}

// ---- the destination is revalidated ----------------------------------------------------------

#[test]
fn a_missing_or_forbidden_documents_folder_is_notes_folder_unavailable() {
    let root = tempfile::tempdir().expect("root");
    let missing = root.path().join("never-created");
    assert_eq!(
        code_of(save_in(&open_policy(), &missing, &[draft("n", "x")])),
        "notes_folder_unavailable"
    );

    let forbidden = WorkFolderPolicy::new(Vec::new(), vec![root.path().to_path_buf()], Vec::new());
    assert_eq!(
        code_of(save_in(&forbidden, root.path(), &[draft("n", "x")])),
        "notes_folder_unavailable"
    );

    let target = Target {
        policy: &forbidden,
        work_folder: root.path(),
        folder_name: "Notes",
    };
    assert_eq!(
        code_of(preview(&target, &[draft("n", "x")], clock())),
        "notes_folder_unavailable"
    );
}

#[test]
fn a_file_in_place_of_the_notes_folder_is_refused() {
    let root = tempfile::tempdir().expect("root");
    fs::write(root.path().join("Notes"), b"not a folder").expect("writes");

    assert_eq!(
        code_of(save_in(&open_policy(), root.path(), &[draft("n", "x")])),
        "notes_folder_unavailable"
    );
}

/// A link at `link` pointing to `to`, or `false` where the platform or the account does not allow
/// one. A test that gets `false` says so and passes: it proves nothing then.
#[cfg(unix)]
fn make_link(link: &Path, to: &Path) -> bool {
    std::os::unix::fs::symlink(to, link).is_ok()
}

/// On Windows a symbolic link needs a privilege most accounts lack, but a junction does not.
#[cfg(windows)]
fn make_link(link: &Path, to: &Path) -> bool {
    if std::os::windows::fs::symlink_dir(to, link).is_ok() {
        return true;
    }
    std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(to)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(any(unix, windows)))]
fn make_link(_link: &Path, _to: &Path) -> bool {
    false
}

#[test]
fn a_link_in_place_of_the_notes_folder_is_refused_and_nothing_lands_behind_it() {
    let root = tempfile::tempdir().expect("root");
    let outside = tempfile::tempdir().expect("outside");
    if !make_link(&root.path().join("Notes"), outside.path()) {
        eprintln!("links cannot be created here: this test proves nothing");
        return;
    }

    let policy = open_policy();
    assert_eq!(
        code_of(save_in(&policy, root.path(), &[draft("n", "x")])),
        "notes_folder_unavailable"
    );
    assert!(
        names_in(outside.path()).is_empty(),
        "nothing was written behind the link"
    );
}

// ---- the bytes -------------------------------------------------------------------------------

#[test]
fn the_text_is_written_as_utf8_without_a_mark_with_unix_line_ends_and_nothing_else() {
    let root = tempfile::tempdir().expect("root");
    let saved = save_in(
        &open_policy(),
        root.path(),
        &[draft("bytes", "\u{feff}caf\u{e9}\r\nsecond\rthird\n")],
    )
    .expect("saves");
    let bytes = fs::read(root.path().join(&saved[0].relative_path)).expect("reads");

    assert_eq!(bytes, "caf\u{e9}\nsecond\nthird\n".as_bytes());
    assert!(
        !bytes.starts_with(&[0xEF, 0xBB, 0xBF]),
        "no byte order mark"
    );
    assert!(!bytes.contains(&b'\r'));
    assert_eq!(saved[0].bytes, bytes.len());
}

#[test]
fn a_saved_note_is_read_back_by_the_document_reader() {
    let root = tempfile::tempdir().expect("root");
    let text = "# Visit\n\nBlood pressure checked, caf\u{e9} noted.\n";
    let saved = save_in(&open_policy(), root.path(), &[draft("round", text)]).expect("saves");
    let absolute = root.path().join(&saved[0].relative_path);
    let file = DiscoveredFile {
        relative_path: saved[0].relative_path.clone(),
        absolute_path: absolute.to_string_lossy().to_string(),
        extension: "md".to_string(),
        size_bytes: saved[0].bytes as u64,
        modified_at: None,
    };

    let document = extract(&file, None, None, "en-US").expect("extracts");

    assert!(!document.empty);
    assert_eq!(document.pages.len(), 1);
    assert_eq!(document.pages[0].text, text);
}

// ---- limits, whole or nothing ----------------------------------------------------------------

#[test]
fn more_than_the_limit_of_notes_is_refused_before_anything_is_written() {
    let root = tempfile::tempdir().expect("root");
    let drafts: Vec<NoteDraft> = (0..=MAX_NOTES)
        .map(|n| draft(&format!("n{n}"), "x"))
        .collect();

    assert_eq!(
        code_of(save_in(&open_policy(), root.path(), &drafts)),
        "notes_too_many"
    );
    assert!(!root.path().join("Notes").exists());

    let allowed: Vec<NoteDraft> = (0..MAX_NOTES)
        .map(|n| draft(&format!("n{n}"), "x"))
        .collect();
    assert_eq!(
        save_in(&open_policy(), root.path(), &allowed)
            .expect("saves")
            .len(),
        MAX_NOTES
    );
}

#[test]
fn a_note_over_one_megabyte_is_refused_and_the_good_ones_of_the_same_save_are_not_written() {
    let root = tempfile::tempdir().expect("root");
    let exactly = "a".repeat(MAX_NOTE_BYTES);
    let over = "a".repeat(MAX_NOTE_BYTES + 1);

    let refused = save_in(
        &open_policy(),
        root.path(),
        &[draft("good", "x"), draft("huge", &over)],
    );
    assert_eq!(code_of(refused), "notes_too_large");
    assert!(!root.path().join("Notes").exists());

    let saved = save_in(&open_policy(), root.path(), &[draft("limit", &exactly)]).expect("saves");
    assert_eq!(saved[0].bytes, MAX_NOTE_BYTES);
}

#[test]
fn nothing_to_save_writes_nothing() {
    let root = tempfile::tempdir().expect("root");

    assert!(save_in(&open_policy(), root.path(), &[])
        .expect("saves")
        .is_empty());
    assert!(!root.path().join("Notes").exists());
}

#[test]
fn a_failed_save_takes_back_only_the_files_it_created() {
    // The write error itself cannot be provoked portably, so the pieces are driven directly: the
    // helper that rolls back removes exactly the paths it is given and leaves the rest.
    let root = tempfile::tempdir().expect("root");
    let policy = open_policy();
    let old = save_in(&policy, root.path(), &[draft("old", "keep")]).expect("saves");
    let folder = root.path().join("Notes");
    let created: Vec<PathBuf> = ["one.md", "two.md"]
        .iter()
        .map(|name| {
            let path = folder.join(name);
            fs::write(&path, b"x").expect("writes");
            path
        })
        .collect();

    super::notes::remove_created(&created);

    assert_eq!(names_in(&folder), vec![old[0].file_name.clone()]);
}

// ---- the log ---------------------------------------------------------------------------------

#[test]
fn the_log_has_one_line_per_file_with_counts_and_never_the_text() {
    let root = tempfile::tempdir().expect("root");
    let logs = tempfile::tempdir().expect("logs");
    let log_path = logs.path().join(LOG_FILE_NAME);
    let policy = open_policy();
    let target = Target {
        policy: &policy,
        work_folder: root.path(),
        folder_name: "Notes",
    };
    let secret = "Secret sentence about Mrs Example";
    let log = SaveLog {
        path: &log_path,
        at_seconds: 1_790_000_000,
    };

    let saved = save(
        &target,
        &[draft("one", secret), draft("two", "short\ntext")],
        clock(),
        Some(&log),
    )
    .expect("saves");
    let logged = fs::read_to_string(&log_path).expect("log reads");
    let lines: Vec<serde_json::Value> = logged
        .lines()
        .map(|line| serde_json::from_str(line).expect("json line"))
        .collect();

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["output"], saved[0].relative_path.as_str());
    assert_eq!(lines[0]["at"], 1_790_000_000_i64);
    assert_eq!(lines[1]["lines"], 2);
    assert_eq!(lines[1]["chars"], 10);
    assert!(!logged.contains("Secret"), "the text never reaches the log");
    assert!(!logged.contains("short"), "the text never reaches the log");
}

#[test]
fn a_log_that_cannot_be_written_never_undoes_a_note() {
    let root = tempfile::tempdir().expect("root");
    let blocker = tempfile::tempdir().expect("logs");
    // A directory where the log file should be: opening it for append fails.
    let log_path = blocker.path().to_path_buf();
    let policy = open_policy();
    let target = Target {
        policy: &policy,
        work_folder: root.path(),
        folder_name: "Notes",
    };
    let log = SaveLog {
        path: &log_path,
        at_seconds: 1,
    };

    let saved = save(&target, &[draft("n", "kept")], clock(), Some(&log)).expect("saves");

    assert_eq!(
        fs::read_to_string(root.path().join(&saved[0].relative_path)).expect("reads"),
        "kept"
    );
}

// ---- the bridge, the scan and launch ---------------------------------------------------------

#[test]
fn drafts_arrive_in_camel_case_with_md_as_the_default_and_files_leave_in_camel_case() {
    let drafts: Vec<NoteDraft> = serde_json::from_str(
        r#"[{"name":"a","text":"x"},{"name":"b","text":"y","format":"txt"},{"text":"z"}]"#,
    )
    .expect("drafts parse");

    assert_eq!(drafts[0].format, NoteFormat::Md);
    assert_eq!(drafts[1].format, NoteFormat::Txt);
    assert_eq!(drafts[2].name, "");

    let file = NoteFile {
        relative_path: "Notes/a.md".into(),
        file_name: "a.md".into(),
        bytes: 1,
        chars: 1,
        lines: 1,
    };
    let json = serde_json::to_value(&file).expect("serialises");
    assert_eq!(json["relativePath"], "Notes/a.md");
    assert_eq!(json["fileName"], "a.md");
    assert!(json.get("relative_path").is_none());
}

fn source(name: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name))
        .expect("source reads")
}

#[test]
fn the_notes_file_opens_no_connection_and_starts_no_process_or_thread() {
    let text = source("speech/notes.rs");
    for token in [
        "reqwest",
        "std::net",
        "TcpStream",
        "UdpSocket",
        "std::process",
        "Command::new",
        "std::thread",
        "tokio::spawn",
    ] {
        assert!(!text.contains(token), "notes.rs uses {token}");
    }
}

#[test]
fn notes_never_touch_the_knowledge_base_or_the_index_themselves() {
    // A note reaches the index only through the Analyse pass the user starts.
    for name in ["speech/notes.rs", "speech_commands.rs"] {
        let text = source(name);
        for token in ["kb_", "rusqlite", "index_store", "open_index", "indexing::"] {
            assert!(!text.contains(token), "{name} uses {token}");
        }
    }
}

#[test]
fn the_notes_commands_run_only_when_the_interface_asks() {
    let lib = source("lib.rs");
    assert!(!lib.contains(".setup("), "a setup hook would run at launch");
    assert!(lib.contains("speech_commands::notes_preview"));
    assert!(lib.contains("speech_commands::notes_save"));
}
