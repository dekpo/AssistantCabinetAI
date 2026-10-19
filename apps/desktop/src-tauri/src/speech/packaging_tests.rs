//! Tests of how the helper programs reach the installer and how the application finds them there
//! (decision of 19 October 2026, `docs/DECISIONS.md`, "Where the helper programs and the models
//! live"): one list of programs, in `tauri.bundle.conf.json`; the default configuration names none,
//! so that `cargo test` and `tauri dev` need nothing staged; and every program is found by the names
//! and in the places an installed copy has them.
//!
//! The installer itself is built by `scripts/build-installer.sh` and checked by hand on the packaged
//! application; what a test can prove without a build is the contract between the two sides.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{worker_stt, worker_tts};
use crate::commands::sidecar_search_roots;

fn crate_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn json(relative: &str) -> Value {
    let text = fs::read_to_string(crate_file(relative)).expect("configuration reads");
    serde_json::from_str(&text).expect("configuration is JSON")
}

fn text(relative: &str) -> String {
    fs::read_to_string(crate_file(relative)).expect("file reads")
}

/// The programs the installer carries, as the bundle configuration lists them (without the folder).
fn bundled_programs() -> Vec<String> {
    json("tauri.bundle.conf.json")["bundle"]["externalBin"]
        .as_array()
        .expect("externalBin is a list")
        .iter()
        .map(|entry| {
            let path = entry.as_str().expect("an entry is a path");
            path.strip_prefix("binaries/")
                .expect("a program is staged in binaries/")
                .to_string()
        })
        .collect()
}

#[test]
fn the_bundle_configuration_lists_the_three_programs_and_the_default_one_lists_none() {
    assert_eq!(
        bundled_programs(),
        ["speech-stt-worker", "speech-tts-worker", "tesseract"]
    );

    // Tauri checks every externalBin and resource file at every build: declared in the default
    // configuration they would stop cargo test and tauri dev on any machine that has not staged them.
    let default = json("tauri.conf.json");
    let bundle = &default["bundle"];
    assert!(bundle.get("externalBin").is_none());
    assert!(bundle.get("resources").is_none());
}

#[test]
fn a_program_installed_next_to_the_application_is_found_by_its_finder() {
    // Tauri copies a program of externalBin next to the executable and drops the target triple from
    // its name; the installer's resource directory is that same folder.
    for (program, find) in [
        (
            "speech-stt-worker",
            worker_stt::find_worker as fn(&[PathBuf]) -> Option<PathBuf>,
        ),
        ("speech-tts-worker", worker_tts::find_worker),
    ] {
        assert!(bundled_programs().iter().any(|name| name == program));
        for installed_name in [format!("{program}.exe"), program.to_string()] {
            let install = tempfile::tempdir().expect("temp");
            let app = install.path().join("assistant-cabinet-ai.exe");
            let worker = install.path().join(&installed_name);
            fs::write(&worker, b"x").expect("worker written");

            let roots =
                sidecar_search_roots(Some(install.path().to_path_buf()), Some(app.clone()), None);
            assert_eq!(find(&roots), Some(worker.clone()), "{installed_name}");

            // The executable alone is enough: the resource directory may be somewhere else.
            let roots = sidecar_search_roots(None, Some(app), None);
            assert_eq!(
                find(&roots),
                Some(worker),
                "{installed_name}, no resource dir"
            );
        }
    }
}

#[test]
fn a_program_staged_under_its_triple_is_still_found_for_a_local_build() {
    let crate_root = tempfile::tempdir().expect("temp");
    let binaries = crate_root.path().join("binaries");
    fs::create_dir_all(&binaries).expect("folder");
    let stt = binaries.join("speech-stt-worker-x86_64-pc-windows-msvc.exe");
    let tts = binaries.join("speech-tts-worker-x86_64-pc-windows-msvc.exe");
    fs::write(&stt, b"x").expect("staged");
    fs::write(&tts, b"x").expect("staged");
    let app = crate_root
        .path()
        .join("target")
        .join("debug")
        .join("app.exe");

    let roots = sidecar_search_roots(None, Some(app), None);

    assert_eq!(worker_stt::find_worker(&roots), Some(stt));
    assert_eq!(worker_tts::find_worker(&roots), Some(tts));
}

#[test]
fn the_voice_notice_travels_with_the_voice_worker_under_the_name_the_build_gives_it() {
    let config = json("tauri.bundle.conf.json");
    let resources = config["bundle"]["resources"]
        .as_object()
        .expect("resources is a map");
    assert_eq!(
        resources["binaries/speech-tts-worker-NOTICE.md"].as_str(),
        Some("./")
    );

    let build = text("../../../scripts/build-speech-tts-worker.sh");
    assert!(build.contains("speech-tts-worker-NOTICE.md"));
    assert!(crate_file("../../speech-worker/NOTICE-TTS-WORKER.md").is_file());
    // The phonemizer data is not a file of the installer: each voice package carries its own copy.
    assert!(!resources.keys().any(|key| key.contains("espeak-ng-data")));
}

#[test]
fn the_ocr_resources_land_where_the_ocr_finders_look() {
    let config = json("tauri.bundle.conf.json");
    let resources = config["bundle"]["resources"]
        .as_object()
        .expect("resources is a map");
    // The model and the library keep their relative place; the Tesseract libraries sit beside it.
    assert_eq!(
        resources["resources/tessdata/fra.traineddata"].as_str(),
        Some("resources/tessdata/fra.traineddata")
    );
    assert_eq!(
        resources["resources/pdfium/pdfium.dll"].as_str(),
        Some("resources/pdfium/pdfium.dll")
    );
    assert_eq!(resources["resources/tesseract/*"].as_str(), Some("./"));

    let commands = text("src/commands.rs");
    assert!(commands.contains("\"tesseract.exe\""));
    assert!(commands.contains(".join(\"resources\").join(\"tessdata\")"));
    assert!(commands.contains(".join(\"resources\").join(\"pdfium\").join(name)"));
}

#[test]
fn the_installer_script_reads_the_list_of_programs_from_the_bundle_configuration() {
    let script = text("../../../scripts/build-installer.sh");
    assert!(script.contains("tauri.bundle.conf.json"));
    assert!(script.contains("--config"));
    // Every worker of the list has its builder named in the script, and the script names no other
    // worker: the list stays in one place.
    for program in bundled_programs() {
        if program.starts_with("speech-") {
            assert!(script.contains(&format!("{program})")), "{program}");
        }
    }
    assert!(script.contains("build-speech-worker.sh"));
    assert!(script.contains("build-speech-tts-worker.sh"));
    assert!(!script.contains("curl "));
    assert!(!script.contains("wget "));
}
