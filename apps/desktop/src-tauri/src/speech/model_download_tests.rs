//! Tests of the model download against a small local HTTP server: what is asked for, what is
//! written, what is refused, and that a failure or a cancel leaves no partial model. The tiny model
//! of `test_support` stands in for the real one; the server is on 127.0.0.1 and nothing leaves the
//! machine.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::model_download::{self, Phase, Progress, Sink};
use super::models::{self, ModelSpec};
use super::status::ModelState;
use super::stt::CancelToken;
use super::test_support::{digest, tiny_files, tiny_model, tiny_voice, tiny_voice_entries, zip_of};
use crate::error::AppError;

#[derive(Debug, Clone)]
struct Seen {
    method: String,
    url: String,
    headers: Vec<String>,
}

/// A server that answers every request with `(status, body)` and records what it was asked.
fn serve(status: u16, body: Vec<u8>) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
    let base = format!("http://{}/models/speech/", server.server_addr());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            log.lock().expect("log").push(Seen {
                method: request.method().to_string(),
                url: request.url().to_string(),
                headers: request
                    .headers()
                    .iter()
                    .map(|header| header.field.to_string().to_lowercase())
                    .collect(),
            });
            let response = tiny_http::Response::from_data(body.clone())
                .with_status_code(tiny_http::StatusCode(status));
            let _ = request.respond(response);
        }
    });
    (base, seen)
}

/// A server whose answer begins at once and whose body then arrives slowly, a few bytes at a time,
/// so a cancel can land in the middle. Written on a plain socket so that the headers are sent
/// immediately and the timing does not depend on the buffering of an HTTP library.
fn serve_trickle() -> String {
    use std::io::Write;

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("binds a free port");
    let base = format!(
        "http://{}/models/speech/",
        listener.local_addr().expect("address")
    );
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut request = [0_u8; 1024];
                let _ = stream.read(&mut request);
                let head = "HTTP/1.1 200 OK
Content-Length: 10000000

";
                if stream.write_all(head.as_bytes()).is_err() {
                    return;
                }
                while stream.write_all(&[7_u8; 64]).is_ok() {
                    std::thread::sleep(Duration::from_millis(10));
                }
            });
        }
    });
    base
}

/// Runs a future on a runtime of its own. Each test gets one, so the connections of a test never
/// share timers or worker threads with the other tests of the crate, which use the shared one.
fn run_async<T>(future: impl std::future::Future<Output = T>) -> T {
    // One download test at a time: on the owner's laptop a group of simultaneous loopback connections
    // freezes the whole test process for seconds.
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let _turn = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime builds")
        .block_on(future)
}

fn recorder() -> (Sink, Arc<Mutex<Vec<Progress>>>) {
    let reports = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&reports);
    let sink: Sink = Arc::new(move |progress| log.lock().expect("log").push(progress));
    (sink, reports)
}

fn install(
    spec: &ModelSpec,
    base: &str,
    root: &Path,
    cancel: &CancelToken,
) -> (Result<(), AppError>, Vec<Progress>) {
    let (sink, reports) = recorder();
    let outcome = run_async(model_download::install(spec, base, root, cancel, &sink));
    let seen = reports.lock().expect("log").clone();
    (outcome, seen)
}

fn entries(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .map(|listing| {
            listing
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn code(outcome: Result<(), AppError>) -> String {
    outcome
        .expect_err("an error was expected")
        .code()
        .to_string()
}

#[test]
fn a_good_download_installs_a_verified_model_and_asks_for_exactly_one_file() {
    let (spec, zip) = tiny_model();
    let (base, seen) = serve(200, zip);
    let dir = tempfile::tempdir().expect("temp dir");

    let (outcome, reports) = install(&spec, &base, dir.path(), &CancelToken::new());
    outcome.expect("installs");

    let folder = spec.folder(dir.path());
    assert_eq!(models::state(&spec, &folder), ModelState::Present);
    for (_, name, bytes) in tiny_files() {
        assert_eq!(fs::read(folder.join(name)).expect("file"), bytes);
    }
    assert_eq!(
        entries(dir.path()),
        vec!["tiny-model".to_string()],
        "no .part, no .staging left behind"
    );

    // Progress runs from the download to the unpack and ends complete.
    assert_eq!(
        reports.first().map(|report| report.phase),
        Some(Phase::Download)
    );
    let last = reports.last().expect("reports");
    assert_eq!(last.phase, Phase::Unpack);
    assert_eq!(last.done_bytes, last.total_bytes);
    assert!(reports
        .iter()
        .any(|report| report.phase == Phase::Download && report.done_bytes == spec.archive.bytes));

    // What was asked: one GET of the archive, with nothing in it that identifies a person or a machine.
    let seen = seen.lock().expect("log");
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, "GET");
    assert_eq!(seen[0].url, "/models/speech/tiny-model.zip");
    for header in &seen[0].headers {
        assert!(
            ["host", "accept"].contains(&header.as_str()),
            "unexpected header {header}"
        );
    }
}

#[test]
fn an_installed_model_replaces_a_damaged_one() {
    let (spec, zip) = tiny_model();
    let (base, _) = serve(200, zip);
    let dir = tempfile::tempdir().expect("temp dir");
    let folder = spec.folder(dir.path());
    fs::create_dir_all(&folder).expect("folder");
    fs::write(folder.join("enc.bin"), b"damaged").expect("damage");
    fs::write(folder.join("left-over.txt"), b"from an older version").expect("old");

    install(&spec, &base, dir.path(), &CancelToken::new())
        .0
        .expect("installs");
    assert_eq!(models::state(&spec, &folder), ModelState::Present);
    assert!(!folder.join("left-over.txt").exists());
}

#[test]
fn a_server_that_says_no_is_a_download_failure_that_names_the_folder() {
    let (spec, _) = tiny_model();
    let (base, _) = serve(404, b"not found".to_vec());
    let dir = tempfile::tempdir().expect("temp dir");

    let (outcome, _) = install(&spec, &base, dir.path(), &CancelToken::new());
    let error = outcome.expect_err("refused");
    assert_eq!(error.code(), "speech_model_download_failed");
    let expected = spec.folder(dir.path()).display().to_string();
    assert_eq!(
        error.data(),
        serde_json::json!({ "folder": expected }),
        "the side-load sentence needs the folder"
    );
    assert!(entries(dir.path()).is_empty(), "nothing was left behind");
}

#[test]
fn a_server_that_hangs_up_is_a_download_failure() {
    // A server that accepts the connection and closes it at once: the same failure as an unreachable
    // one for the user, without depending on how the system treats a connection to a closed port
    // (on Windows that can take many seconds and ties up the whole test run).
    let (spec, _) = tiny_model();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("binds");
    let base = format!("http://{}/", listener.local_addr().expect("address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            drop(stream);
        }
    });
    let dir = tempfile::tempdir().expect("temp dir");
    let (outcome, _) = install(&spec, &base, dir.path(), &CancelToken::new());
    assert_eq!(code(outcome), "speech_model_download_failed");
    assert!(entries(dir.path()).is_empty());
}

#[test]
fn an_archive_that_is_not_the_pinned_one_is_refused_before_anything_is_unpacked() {
    let (spec, zip) = tiny_model();
    let mut tampered = zip.clone();
    let middle = tampered.len() / 2;
    tampered[middle] ^= 0xff;
    let (base, _) = serve(200, tampered);
    let dir = tempfile::tempdir().expect("temp dir");

    let (outcome, _) = install(&spec, &base, dir.path(), &CancelToken::new());
    assert_eq!(code(outcome), "speech_model_corrupt");
    assert!(
        entries(dir.path()).is_empty(),
        "no part file, no staging, no model"
    );

    let (base, _) = serve(200, zip[..zip.len() - 10].to_vec());
    let (outcome, _) = install(&spec, &base, dir.path(), &CancelToken::new());
    assert_eq!(code(outcome), "speech_model_corrupt", "a short archive");
    assert!(entries(dir.path()).is_empty());
}

#[test]
fn a_file_inside_the_archive_that_does_not_match_its_digest_is_refused() {
    // The archive itself is the one the manifest pins, but a file in it is not the pinned file.
    let (mut spec, _) = tiny_model();
    let mut files = tiny_files();
    files[0].2 = vec![9_u8; 3_000];
    let zip = zip_of(
        &files
            .iter()
            .map(|(_, name, bytes)| (*name, bytes.as_slice()))
            .collect::<Vec<_>>(),
    );
    spec.archive.bytes = zip.len() as u64;
    spec.archive.sha256 = digest(&zip);
    let (base, _) = serve(200, zip);
    let dir = tempfile::tempdir().expect("temp dir");

    let (outcome, _) = install(&spec, &base, dir.path(), &CancelToken::new());
    assert_eq!(code(outcome), "speech_model_corrupt");
    assert!(entries(dir.path()).is_empty());
}

#[test]
fn an_archive_without_one_of_the_files_is_refused() {
    let (mut spec, _) = tiny_model();
    let files = tiny_files();
    let zip = zip_of(
        &files[..3]
            .iter()
            .map(|(_, name, bytes)| (*name, bytes.as_slice()))
            .collect::<Vec<_>>(),
    );
    spec.archive.bytes = zip.len() as u64;
    spec.archive.sha256 = digest(&zip);
    let (base, _) = serve(200, zip);
    let dir = tempfile::tempdir().expect("temp dir");

    let (outcome, _) = install(&spec, &base, dir.path(), &CancelToken::new());
    assert_eq!(code(outcome), "speech_model_corrupt");
    assert!(entries(dir.path()).is_empty());
}

#[test]
fn only_the_files_the_manifest_names_are_written_whatever_the_archive_holds() {
    let (mut spec, _) = tiny_model();
    let files = tiny_files();
    let mut entries_in_zip: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(_, name, bytes)| (*name, bytes.as_slice()))
        .collect();
    entries_in_zip.push(("../escaped.txt", b"outside"));
    entries_in_zip.push(("extra/inside.txt", b"extra"));
    let zip = zip_of(&entries_in_zip);
    spec.archive.bytes = zip.len() as u64;
    spec.archive.sha256 = digest(&zip);
    let (base, _) = serve(200, zip);
    let outer = tempfile::tempdir().expect("temp dir");
    let root = outer.path().join("models");

    install(&spec, &base, &root, &CancelToken::new())
        .0
        .expect("installs");
    assert!(
        !outer.path().join("escaped.txt").exists(),
        "nothing escapes the folder"
    );
    assert!(!root.join("escaped.txt").exists());
    let mut written = entries(&spec.folder(&root));
    written.sort();
    assert_eq!(
        written,
        vec![
            ".verified".to_string(),
            "dec.bin".to_string(),
            "enc.bin".to_string(),
            "join.bin".to_string(),
            "tok.txt".to_string()
        ]
    );
}

#[test]
fn a_cancel_that_is_already_set_downloads_nothing() {
    let (spec, zip) = tiny_model();
    let (base, _) = serve(200, zip);
    let dir = tempfile::tempdir().expect("temp dir");
    let token = CancelToken::new();
    token.cancel();

    let (outcome, _) = install(&spec, &base, dir.path(), &token);
    assert_eq!(code(outcome), "speech_cancelled");
    assert!(entries(dir.path()).is_empty());
}

#[test]
fn a_cancel_in_the_middle_of_a_download_removes_the_part_and_keeps_the_old_model() {
    let (mut spec, _) = tiny_model();
    spec.archive.bytes = 10_000_000;
    let base = serve_trickle();
    let dir = tempfile::tempdir().expect("temp dir");
    let folder = spec.folder(dir.path());
    crate::speech::test_support::place_tiny_files(&folder);
    models::write_marker(&spec, &folder).expect("marker");

    // The stop is asked from the first progress report, that is once the answer has begun to arrive,
    // not after a fixed delay: on a loaded machine a fixed delay can fall before the connection.
    let token = CancelToken::new();
    let stopper = token.clone();
    let reports = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&reports);
    let first_report = Arc::new(Mutex::new(None));
    let first = Arc::clone(&first_report);
    let sink: Sink = Arc::new(move |progress| {
        log.lock().expect("log").push(progress);
        first
            .lock()
            .expect("first")
            .get_or_insert_with(std::time::Instant::now);
        stopper.cancel();
    });
    let outcome = run_async(model_download::install(
        &spec,
        &base,
        dir.path(),
        &token,
        &sink,
    ));
    let reports = reports.lock().expect("log").clone();

    assert_eq!(code(outcome), "speech_cancelled");
    // The body would take about half an hour to arrive, so only a cancel can end this call. The
    // bound is generous on purpose: on the owner's laptop the whole test process sometimes freezes
    // for ten seconds at a time (seen with any group of loopback connections, antivirus suspected),
    // which a tight bound would report as a failure of the cancel.
    let waited = first_report
        .lock()
        .expect("first")
        .expect("the download began")
        .elapsed();
    assert!(
        waited < Duration::from_secs(120),
        "the cancel waited for the end"
    );
    assert!(!reports.is_empty());
    assert_eq!(
        entries(dir.path()),
        vec!["tiny-model".to_string()],
        "only the old model remains"
    );
    assert_eq!(models::state(&spec, &folder), ModelState::Present);
}

#[test]
fn the_address_is_the_setting_and_one_file_name() {
    let (spec, _) = tiny_model();
    assert_eq!(
        model_download::archive_url("https://example.org/m/s/", &spec),
        "https://example.org/m/s/tiny-model.zip"
    );
    assert_eq!(
        model_download::archive_url("https://example.org/m/s", &spec),
        "https://example.org/m/s/tiny-model.zip"
    );
}

#[test]
fn the_download_file_sends_nothing_but_a_get_for_the_archive() {
    let text = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("speech")
            .join("model_download.rs"),
    )
    .expect("source reads");
    for token in [
        ".post(",
        ".put(",
        ".body(",
        ".json(",
        ".form(",
        "multipart",
        "bearer_auth",
        ".header(",
    ] {
        assert!(!text.contains(token), "model_download.rs uses {token}");
    }
    assert!(text.contains(".get(archive_url("));
    assert_eq!(text.matches("reqwest::Client").count(), 1);
}

/// The whole chain with the real model: the archive the owner uploads is served from this machine,
/// downloaded, hashed, unpacked and verified against the manifest compiled into the application, and
/// the installed files then transcribe a real clip through the real worker. Run by hand:
/// `ACAI_STT_ARCHIVE=<zip> ACAI_STT_CLIP=<wav> cargo test --lib real_archive -- --ignored --nocapture`.
/// The worker is the one staged by `scripts/build-speech-worker.sh` (or `ACAI_STT_WORKER`).
#[test]
#[ignore = "needs the real archive, the real worker and takes about a minute"]
fn real_archive_installs_and_the_installed_model_transcribes() {
    use super::stt::{SpeechToTextProvider, SttRequest};
    use super::worker_stt::{self, Setup, WorkerLaunch, WorkerSttProvider};

    let spec = models::stt_model().expect("manifest").clone();
    let archive =
        std::path::PathBuf::from(std::env::var("ACAI_STT_ARCHIVE").expect("set ACAI_STT_ARCHIVE"));
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
    let base = format!("http://{}/models/speech/", server.server_addr());
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let file = fs::File::open(&archive).expect("archive opens");
            let _ = request.respond(tiny_http::Response::from_file(file));
        }
    });

    let dir = tempfile::tempdir().expect("temp dir");
    let root = worker_stt::models_root(dir.path());
    let started = std::time::Instant::now();
    let (outcome, reports) = install(&spec, &base, &root, &CancelToken::new());
    outcome.expect("the real archive installs");
    println!(
        "measured: install from a local server took {} ms, {} progress reports",
        started.elapsed().as_millis(),
        reports.len()
    );
    let folder = spec.folder(&root);
    assert_eq!(models::state(&spec, &folder), ModelState::Present);

    let worker = std::env::var("ACAI_STT_WORKER")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("binaries")
                .join("speech-stt-worker-x86_64-pc-windows-msvc.exe")
        });
    let provider = WorkerSttProvider::new(spec.clone());
    provider.configure(Setup {
        launch: WorkerLaunch::program(worker),
        model_folder: folder,
    });
    provider
        .availability("fr-FR")
        .expect("the installed model is ready");
    let clip = dir.path().join("clip.wav");
    fs::copy(
        std::env::var("ACAI_STT_CLIP").expect("set ACAI_STT_CLIP"),
        &clip,
    )
    .expect("clip copies");
    let heard = provider
        .transcribe(
            &SttRequest {
                wav_path: clip,
                language: "fr".into(),
                max_seconds: 120,
            },
            &CancelToken::new(),
        )
        .expect("the installed model transcribes");
    println!(
        "measured: {} words heard in {} ms",
        heard.text.split_whitespace().count(),
        heard.processing_ms
    );
    assert!(!heard.text.trim().is_empty());
    provider.shutdown();
}

// ---- a voice with a folder pinned as a whole -------------------------------------------------

/// The tiny voice with a zip made of `entries` instead, the manifest row pointing at that zip.
fn voice_with_entries(entries_in_zip: &[(&str, Vec<u8>)]) -> (ModelSpec, Vec<u8>) {
    let (mut spec, _) = tiny_voice();
    let zip = zip_of(
        &entries_in_zip
            .iter()
            .map(|(name, bytes)| (*name, bytes.as_slice()))
            .collect::<Vec<_>>(),
    );
    spec.archive.bytes = zip.len() as u64;
    spec.archive.sha256 = digest(&zip);
    (spec, zip)
}

#[test]
fn a_voice_installs_with_its_pinned_folder_and_is_present() {
    let (spec, zip) = tiny_voice();
    let (base, seen) = serve(200, zip);
    let dir = tempfile::tempdir().expect("temp dir");

    let (outcome, reports) = install(&spec, &base, dir.path(), &CancelToken::new());
    outcome.expect("installs");

    let folder = spec.folder(dir.path());
    assert_eq!(models::state(&spec, &folder), ModelState::Present);
    for (name, bytes) in tiny_voice_entries() {
        assert_eq!(fs::read(folder.join(name)).expect("file"), bytes, "{name}");
    }
    assert_eq!(entries(dir.path()), vec!["tiny-voice".to_string()]);
    let last = reports.last().expect("reports");
    assert_eq!(last.done_bytes, last.total_bytes);
    assert_eq!(
        seen.lock().expect("log")[0].url,
        "/models/speech/tiny-voice.zip"
    );

    // The installed folder passes the full check, which hashes every file of it.
    models::verify(&spec, &folder, &CancelToken::new()).expect("verifies");
}

#[test]
fn a_file_of_the_pinned_folder_that_is_not_the_pinned_one_is_refused() {
    // The archive is the one the row pins as a download, but a file of its folder is not the pinned one.
    let mut changed = tiny_voice_entries();
    changed[2].1 = vec![0_u8; 600];
    let (mut spec, zip) = voice_with_entries(&changed);
    spec.voice = tiny_voice().0.voice;
    let (base, _) = serve(200, zip);
    let dir = tempfile::tempdir().expect("temp dir");

    let (outcome, _) = install(&spec, &base, dir.path(), &CancelToken::new());
    assert_eq!(code(outcome), "speech_model_corrupt");
    assert!(entries(dir.path()).is_empty(), "nothing is left behind");
}

#[test]
fn a_missing_or_an_extra_file_in_the_pinned_folder_is_refused() {
    let (original, _) = tiny_voice();

    let mut fewer = tiny_voice_entries();
    fewer.pop();
    let mut more = tiny_voice_entries();
    more.push(("phon-data/lang/extra", b"unexpected".to_vec()));
    for (what, entries_in_zip) in [("fewer", fewer), ("more", more)] {
        let (mut spec, zip) = voice_with_entries(&entries_in_zip);
        spec.voice = original.voice.clone();
        let (base, _) = serve(200, zip);
        let dir = tempfile::tempdir().expect("temp dir");
        let (outcome, _) = install(&spec, &base, dir.path(), &CancelToken::new());
        assert_eq!(code(outcome), "speech_model_corrupt", "{what}");
        assert!(entries(dir.path()).is_empty(), "{what}");
    }
}

#[test]
fn only_the_pinned_folder_is_unpacked_and_no_name_climbs_out_of_it() {
    // An entry outside the pinned folder is ignored.
    let mut with_stranger = tiny_voice_entries();
    with_stranger.push(("elsewhere/stranger.txt", b"not pinned".to_vec()));
    let (mut spec, zip) = voice_with_entries(&with_stranger);
    spec.voice = tiny_voice().0.voice;
    let (base, _) = serve(200, zip);
    let outer = tempfile::tempdir().expect("temp dir");
    let root = outer.path().join("models");
    install(&spec, &base, &root, &CancelToken::new())
        .0
        .expect("installs");
    assert!(!spec.folder(&root).join("elsewhere").exists());

    // A name that climbs out of the pinned folder is refused, and nothing is written outside.
    let mut climbing = tiny_voice_entries();
    climbing.push(("phon-data/../../escaped.txt", b"outside".to_vec()));
    let (mut spec, zip) = voice_with_entries(&climbing);
    spec.voice = tiny_voice().0.voice;
    let (base, _) = serve(200, zip);
    let outer = tempfile::tempdir().expect("temp dir");
    let root = outer.path().join("models");
    let (outcome, _) = install(&spec, &base, &root, &CancelToken::new());
    assert_eq!(code(outcome), "speech_model_corrupt");
    assert!(!outer.path().join("escaped.txt").exists());
    assert!(!root.join("escaped.txt").exists());
    assert!(entries(&root).is_empty());
}

/// The four real voices: the archives the owner uploads are served from this machine, downloaded,
/// hashed, unpacked and checked against the manifest compiled into the application, then the folder
/// pinned as a whole is verified again from the disk. Run by hand:
/// `ACAI_VOICE_ZIPS=<folder of the zips> cargo test --lib real_voice_archives -- --ignored --nocapture`.
#[test]
#[ignore = "needs the four real voice archives, about 280 MB"]
fn real_voice_archives_install_and_verify() {
    let folder =
        std::path::PathBuf::from(std::env::var("ACAI_VOICE_ZIPS").expect("set ACAI_VOICE_ZIPS"));
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").expect("binds a free port"));
    let base = format!("http://{}/models/speech/", server.server_addr());
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let name = request
                .url()
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string();
            match fs::File::open(folder.join(name)) {
                Ok(file) => {
                    let _ = request.respond(tiny_http::Response::from_file(file));
                }
                Err(_) => {
                    let _ = request.respond(
                        tiny_http::Response::from_data(Vec::new())
                            .with_status_code(tiny_http::StatusCode(404)),
                    );
                }
            }
        }
    });
    let dir = tempfile::tempdir().expect("temp dir");
    for spec in models::voice_models() {
        let started = std::time::Instant::now();
        let (outcome, _) = install(spec, &base, dir.path(), &CancelToken::new());
        outcome.unwrap_or_else(|error| panic!("{} did not install: {}", spec.id, error.code()));
        let installed = spec.folder(dir.path());
        assert_eq!(
            models::state(spec, &installed),
            ModelState::Present,
            "{}",
            spec.id
        );
        models::verify(spec, &installed, &CancelToken::new()).expect("verifies from the disk");
        println!(
            "measured: {} installed and verified in {} ms",
            spec.id,
            started.elapsed().as_millis()
        );
    }
}
