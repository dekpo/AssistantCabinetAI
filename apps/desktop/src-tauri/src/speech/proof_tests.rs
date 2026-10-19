//! Tests added by the "prove it" phase for the invariants of `docs/SPEECH.md`, section 12, that had a
//! name in the table and no test behind it. Source scans, like the other invariant tests of the module.

use std::fs;
use std::path::Path;

/// The non-test Rust files of the speech module, plus the file of its commands, as (name, text) with
/// the comment lines left out: a comment may name what the code must not touch.
fn speech_sources() -> Vec<(String, String)> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    for entry in fs::read_dir(src.join("speech"))
        .expect("folder reads")
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".rs")
            || name.ends_with("tests.rs")
            || name == "test_support.rs"
            || name == "fake.rs"
        {
            continue;
        }
        files.push((format!("speech/{name}"), entry.path()));
    }
    files.push((
        "speech_commands.rs".to_string(),
        src.join("speech_commands.rs"),
    ));
    files
        .into_iter()
        .map(|(name, path)| {
            let text = fs::read_to_string(path).expect("source reads");
            let code = text
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            (name, code)
        })
        .collect()
}

#[test]
fn no_speech_path_reaches_a_file_action() {
    // The file actions of the product are the plan that renames, fills or moves a document and the
    // Analyse pass. None of their modules is named by the speech module; the one clean-name function
    // it borrows is a pure function on a string.
    let actions = [
        "fill_plan",
        "template_fill",
        "data_folder",
        "index_store",
        "indexing::",
        "analyse_work_files",
        "apply_plan",
    ];
    // What it may do to files, and where: move its own staging folder to the model's folder, and remove
    // what it created itself (a clip, a download part, a note whose save failed).
    let may_rename = ["speech/model_download.rs"];
    let may_remove = [
        "speech/model_download.rs",
        "speech/notes.rs",
        "speech/temp_audio.rs",
    ];

    let sources = speech_sources();
    assert!(
        sources.len() >= 20,
        "the scan found {} files",
        sources.len()
    );
    for (name, code) in &sources {
        for token in actions {
            assert!(!code.contains(token), "{name} uses {token}");
        }
        for token in ["fs::rename", "fs::copy"] {
            assert!(
                !code.contains(token) || may_rename.contains(&name.as_str()),
                "{name} uses {token}"
            );
        }
        for token in ["remove_file", "remove_dir"] {
            assert!(
                !code.contains(token) || may_remove.contains(&name.as_str()),
                "{name} uses {token}"
            );
        }
    }
    // The only write that reaches a document folder is a saved note, in notes.rs, created new.
    let notes = &sources
        .iter()
        .find(|(name, _)| name == "speech/notes.rs")
        .expect("notes.rs is scanned")
        .1;
    assert!(notes.contains("create_new(true)"));
    assert!(!notes.contains("OpenOptions::new().write(true).truncate"));
}
