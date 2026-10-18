//! Tests of the compiled-in manifest, of the state of a model folder, of Verify (the side-load
//! path) and of the one-job slot. Everything uses the tiny model of `test_support`, written to a
//! temporary folder; no real model is read.

use std::fs;

use super::models::{self, FileSpec, JobSlot, MARKER_FILE};
use super::status::ModelState;
use super::stt::CancelToken;
use super::test_support::{digest, place_tiny_files, tiny_model};

fn code<T: std::fmt::Debug>(outcome: Result<T, crate::error::AppError>) -> String {
    outcome
        .expect_err("an error was expected")
        .code()
        .to_string()
}

#[test]
fn the_compiled_in_manifest_parses_and_pins_the_parakeet_files() {
    let spec = models::stt_model().expect("the manifest parses");
    assert_eq!(spec.id, "parakeet-tdt-0.6b-v3-int8");
    assert_eq!(spec.licence, "CC-BY-4.0");
    assert!(
        spec.attribution.contains("CC BY 4.0"),
        "attribution is required"
    );
    assert_eq!(spec.archive.file, "parakeet-tdt-0.6b-v3-int8.zip");
    assert!(spec.archive.file.ends_with(".zip"), "the download is a zip");
    assert!(
        !spec.archive.file.contains('/'),
        "a file name, never a path"
    );

    let roles: Vec<&str> = spec.files.iter().map(|file| file.role.as_str()).collect();
    assert_eq!(roles, ["encoder", "decoder", "joiner", "tokens"]);
    for file in spec
        .files
        .iter()
        .map(|file| &file.sha256)
        .chain([&spec.archive.sha256])
    {
        assert_eq!(file.len(), 64);
        assert!(file
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
    for file in &spec.files {
        assert!(file.bytes > 0);
        assert!(
            !file.name.contains('/') && !file.name.contains('\\'),
            "{}",
            file.name
        );
    }
    assert!(spec.archive.bytes > 0);
    let files = spec
        .worker_files(std::path::Path::new("models"))
        .expect("all four roles exist");
    assert!(files.encoder.ends_with("encoder.int8.onnx"));
    assert!(files.tokens.ends_with("tokens.txt"));
}

#[test]
fn the_marker_follows_the_manifest_row() {
    let (spec, _) = tiny_model();
    let mut other = spec.clone();
    assert_eq!(spec.fingerprint(), other.fingerprint());
    other.files[1] = FileSpec {
        sha256: digest(b"another file"),
        ..other.files[1].clone()
    };
    assert_ne!(spec.fingerprint(), other.fingerprint());
}

#[test]
fn state_reads_the_folder_without_hashing() {
    let (spec, _) = tiny_model();
    let dir = tempfile::tempdir().expect("temp dir");
    let folder = dir.path().join("tiny-model");

    assert_eq!(
        models::state(&spec, &folder),
        ModelState::Missing,
        "no folder"
    );
    place_tiny_files(&folder);
    fs::remove_file(folder.join("join.bin")).expect("remove");
    assert_eq!(
        models::state(&spec, &folder),
        ModelState::Missing,
        "three files of four are not a model"
    );
    fs::write(folder.join("join.bin"), vec![3_u8; 700]).expect("restore");
    assert_eq!(
        models::state(&spec, &folder),
        ModelState::Unverified,
        "all there, never hashed"
    );

    models::write_marker(&spec, &folder).expect("marker");
    assert_eq!(models::state(&spec, &folder), ModelState::Present);

    fs::write(folder.join(MARKER_FILE), "stale").expect("stale marker");
    assert_eq!(models::state(&spec, &folder), ModelState::Unverified);

    models::write_marker(&spec, &folder).expect("marker");
    fs::write(folder.join("dec.bin"), b"cut").expect("damage");
    assert_eq!(models::state(&spec, &folder), ModelState::Corrupt);
}

#[test]
fn verify_trusts_a_hand_placed_model_only_after_hashing_it() {
    let (spec, _) = tiny_model();
    let dir = tempfile::tempdir().expect("temp dir");
    let folder = dir.path().join("tiny-model");
    place_tiny_files(&folder);
    assert_eq!(models::state(&spec, &folder), ModelState::Unverified);

    models::verify(&spec, &folder, &CancelToken::new()).expect("a correct model verifies");
    assert_eq!(models::state(&spec, &folder), ModelState::Present);
    assert_eq!(
        fs::read_to_string(folder.join(MARKER_FILE)).expect("marker"),
        spec.fingerprint()
    );
}

#[test]
fn verify_refuses_a_file_of_the_right_size_but_the_wrong_content() {
    let (spec, _) = tiny_model();
    let dir = tempfile::tempdir().expect("temp dir");
    let folder = dir.path().join("tiny-model");
    place_tiny_files(&folder);
    fs::write(folder.join("enc.bin"), vec![9_u8; 3_000]).expect("same size, other bytes");

    assert_eq!(
        code(models::verify(&spec, &folder, &CancelToken::new())),
        "speech_model_corrupt"
    );
    assert!(
        !folder.join(MARKER_FILE).exists(),
        "nothing is marked on a mismatch"
    );
    assert_eq!(models::state(&spec, &folder), ModelState::Unverified);
}

#[test]
fn verify_names_a_missing_file_and_obeys_a_cancel() {
    let (spec, _) = tiny_model();
    let dir = tempfile::tempdir().expect("temp dir");
    let folder = dir.path().join("tiny-model");
    assert_eq!(
        code(models::verify(&spec, &folder, &CancelToken::new())),
        "speech_model_missing"
    );
    place_tiny_files(&folder);
    let token = CancelToken::new();
    token.cancel();
    assert_eq!(
        code(models::verify(&spec, &folder, &token)),
        "speech_cancelled"
    );
    assert!(!folder.join(MARKER_FILE).exists());
}

#[test]
fn hashing_matches_the_standard_test_vectors() {
    let dir = tempfile::tempdir().expect("temp dir");
    let empty = dir.path().join("empty");
    fs::write(&empty, b"").expect("write");
    assert_eq!(
        models::hash_file(&empty, &CancelToken::new()).expect("hash"),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    let abc = dir.path().join("abc");
    fs::write(&abc, b"abc").expect("write");
    assert_eq!(
        models::hash_file(&abc, &CancelToken::new()).expect("hash"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn only_one_model_job_runs_at_a_time_and_a_dropped_guard_frees_the_slot() {
    let slot = JobSlot::default();
    assert!(!slot.is_running());
    assert!(!slot.cancel(), "nothing to cancel");

    let first = slot.begin().expect("first job");
    assert!(slot.is_running());
    assert_eq!(code(slot.begin()), "speech_busy");
    assert!(!first.cancel_token().is_cancelled());
    assert!(slot.cancel());
    assert!(first.cancel_token().is_cancelled());

    drop(first);
    assert!(!slot.is_running());
    let second = slot.begin().expect("the slot is free again");
    assert!(
        !second.cancel_token().is_cancelled(),
        "a new job does not inherit the old cancel"
    );
}
