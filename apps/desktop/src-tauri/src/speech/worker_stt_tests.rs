//! Tests of the worker protocol, of the process client and of its idle and cancel logic.
//!
//! The fake worker is this very test executable started again with an environment variable: the
//! test `fake_worker_entry` does nothing in a normal run and, when `ACAI_FAKE_WORKER` is set, runs
//! the real worker loop (`worker_protocol::serve`) over a scripted engine, then exits. So the
//! protocol, the process handling, the cancel kill and the idle end are all proven with a real child
//! process, on both platforms, without a model and without the real engine. The real engine has one
//! `#[ignore]` test at the end.

use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::languages;
use super::models::{self, ModelSpec};
use super::service::{Limits, Plan, SpeechService};
use super::status::ModelState;
use super::stt::{CancelToken, SpeechToTextProvider, SttRequest};
use super::test_support::{canonical_clip, place_tiny_files, tiny_model};
use super::worker_protocol::{
    self as protocol, Engine, FailureKind, Heard, ModelFiles, Request, Response,
};
use super::worker_stt::{
    self, idle_expired, Ending, Setup, WorkerLaunch, WorkerSttProvider, BUSY_CORES,
    COLD_LOAD_SECONDS, IDLE_UNLOAD_SECONDS, RESIDENT_MEMORY_MB, STT_THREADS,
};
use crate::error::AppError;

// ---- the scripted worker ----------------------------------------------------------------------

const FAKE_FLAG: &str = "ACAI_FAKE_WORKER";
const FAKE_TEXT: &str = "ACAI_FAKE_TEXT";
const FAKE_DECODE_MS: &str = "ACAI_FAKE_DECODE_MS";
const FAKE_LOAD_FAILS: &str = "ACAI_FAKE_LOAD_FAILS";
const FAKE_CRASH: &str = "ACAI_FAKE_CRASH";
const FAKE_STRAY: &str = "ACAI_FAKE_STRAY";
const FAKE_BAD_AUDIO: &str = "ACAI_FAKE_BAD_AUDIO";
const SENTENCE: &str = "please send the report to the accountant before friday afternoon";

struct ScriptedEngine;

impl Engine for ScriptedEngine {
    fn load(&mut self, _threads: u32, files: &ModelFiles) -> Result<u64, FailureKind> {
        match std::env::var(FAKE_LOAD_FAILS).as_deref() {
            Ok("missing") => return Err(FailureKind::ModelMissing),
            Ok("corrupt") => return Err(FailureKind::ModelCorrupt),
            _ => {}
        }
        // The worker is given real paths: prove they are the model's files.
        for path in [&files.encoder, &files.decoder, &files.joiner, &files.tokens] {
            if !Path::new(path).is_file() {
                return Err(FailureKind::ModelMissing);
            }
        }
        Ok(7)
    }

    fn transcribe(&mut self, wav_path: &str, _language: &str) -> Result<Heard, FailureKind> {
        if std::env::var(FAKE_STRAY).is_ok() {
            // What a native library does: lines of its own on the standard output.
            println!("a stray line from a native library");
            println!("{{ not a response }}");
        }
        if std::env::var(FAKE_CRASH).as_deref() == Ok("transcribe") {
            std::process::exit(3);
        }
        if std::env::var(FAKE_BAD_AUDIO).is_ok() {
            return Err(FailureKind::AudioInvalid);
        }
        let bytes = fs::read(wav_path).map_err(|_| FailureKind::AudioInvalid)?;
        let delay: u64 = std::env::var(FAKE_DECODE_MS)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        std::thread::sleep(Duration::from_millis(delay));
        Ok(Heard {
            text: std::env::var(FAKE_TEXT).unwrap_or_else(|_| SENTENCE.to_string()),
            audio_ms: (bytes.len().saturating_sub(44) / 2) as u64 * 1_000 / 16_000,
            processing_ms: delay,
        })
    }
}

/// Not a test of anything: the entry point of the scripted worker. In a normal run it returns at
/// once. Started with the flag, it serves the protocol until shutdown or the end of its input.
#[test]
fn fake_worker_entry() {
    if std::env::var(FAKE_FLAG).is_err() {
        return;
    }
    // The test harness has written "test ... " without a line break; start on a fresh line.
    println!();
    let input = std::io::stdin().lock();
    let output = std::io::stdout();
    let outcome = protocol::serve(input, output, &mut ScriptedEngine);
    std::process::exit(if outcome.is_ok() { 0 } else { 1 });
}

fn fake_launch(envs: &[(&str, &str)]) -> WorkerLaunch {
    let mut launch = WorkerLaunch::program(std::env::current_exe().expect("test exe"));
    launch.args = [
        "--exact",
        "speech::worker_stt_tests::fake_worker_entry",
        "--nocapture",
        "--test-threads=1",
    ]
    .iter()
    .map(Into::into)
    .collect();
    launch.envs.push((FAKE_FLAG.into(), "1".into()));
    for (key, value) in envs {
        launch.envs.push(((*key).into(), (*value).into()));
    }
    launch
}

// ---- rig ----------------------------------------------------------------------------------------

struct Rig {
    dir: tempfile::TempDir,
    spec: ModelSpec,
    provider: Arc<WorkerSttProvider>,
}

fn rig_with(envs: &[(&str, &str)], idle: Duration) -> Rig {
    let dir = tempfile::tempdir().expect("temp dir");
    let (spec, _) = tiny_model();
    let folder = spec.folder(&worker_stt::models_root(dir.path()));
    place_tiny_files(&folder);
    models::verify(&spec, &folder, &CancelToken::new()).expect("tiny model verifies");
    let provider = Arc::new(WorkerSttProvider::with_timing(spec.clone(), idle, 2));
    provider.configure(Setup {
        launch: fake_launch(envs),
        model_folder: folder,
    });
    Rig {
        dir,
        spec,
        provider,
    }
}

fn rig(envs: &[(&str, &str)]) -> Rig {
    rig_with(envs, Duration::from_secs(60))
}

/// Writes a clip into the rig's folder and returns the request for it.
fn request_in(rig: &Rig, milliseconds: u64) -> (SttRequest, PathBuf) {
    let path = rig.dir.path().join(format!("clip-{milliseconds}.wav"));
    fs::write(&path, canonical_clip(milliseconds)).expect("clip writes");
    (
        SttRequest {
            wav_path: path.clone(),
            language: "fr".into(),
            max_seconds: 120,
        },
        path,
    )
}

fn code<T: std::fmt::Debug>(outcome: Result<T, AppError>) -> String {
    outcome
        .expect_err("an error was expected")
        .code()
        .to_string()
}

fn wait_until(limit: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < limit {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    condition()
}

fn plan() -> Plan {
    Plan {
        locale: "fr-FR".into(),
        language: "fr".into(),
        cap_seconds: 120,
    }
}

// ---- the protocol -----------------------------------------------------------------------------

fn sample_files() -> ModelFiles {
    ModelFiles {
        encoder: "e".into(),
        decoder: "d".into(),
        joiner: "j".into(),
        tokens: "t".into(),
    }
}

#[test]
fn every_message_survives_a_round_trip_as_one_line() {
    let requests = [
        Request::Load {
            id: 1,
            threads: 4,
            files: sample_files(),
        },
        Request::Transcribe {
            id: 2,
            wav_path: "clip.wav".into(),
            language: "fr".into(),
        },
        Request::Shutdown { id: 3 },
    ];
    for request in requests {
        let line = protocol::encode_request(&request);
        assert!(!line.contains('\n'));
        assert_eq!(
            serde_json::from_str::<Request>(&line).expect("parses"),
            request
        );
    }
    let responses = [
        Response::Loaded {
            id: 1,
            load_ms: 2_400,
        },
        Response::Transcribed {
            id: 2,
            text: "line one\nline two".into(),
            audio_ms: 4_000,
            processing_ms: 500,
        },
        Response::Failed {
            id: 3,
            kind: FailureKind::AudioInvalid,
        },
    ];
    for response in responses {
        let line = protocol::encode_response(&response);
        assert!(!line.contains('\n'), "a newline in the text is escaped");
        assert_eq!(protocol::decode_response(&line), Some(response));
    }
}

#[test]
fn a_line_that_is_not_a_response_is_skipped_never_parsed_loosely() {
    assert_eq!(protocol::decode_response("a stray line"), None);
    assert_eq!(protocol::decode_response(""), None);
    assert_eq!(protocol::decode_response("{ not a response }"), None);
    assert_eq!(
        protocol::decode_response("{\"status\":\"unknown\",\"id\":1}"),
        None
    );
}

struct CountingEngine {
    loads: u32,
    heard: Vec<String>,
}

impl Engine for CountingEngine {
    fn load(&mut self, _threads: u32, _files: &ModelFiles) -> Result<u64, FailureKind> {
        self.loads += 1;
        Ok(1)
    }

    fn transcribe(&mut self, wav_path: &str, language: &str) -> Result<Heard, FailureKind> {
        self.heard.push(format!("{wav_path}|{language}"));
        Ok(Heard {
            text: "ok".into(),
            audio_ms: 1,
            processing_ms: 1,
        })
    }
}

fn serve_lines(input: &str) -> (Vec<Response>, CountingEngine) {
    let mut engine = CountingEngine {
        loads: 0,
        heard: Vec::new(),
    };
    let mut output = Vec::new();
    protocol::serve(Cursor::new(input.to_string()), &mut output, &mut engine).expect("serves");
    let responses = String::from_utf8(output)
        .expect("utf8")
        .lines()
        .filter_map(protocol::decode_response)
        .collect();
    (responses, engine)
}

#[test]
fn the_loop_answers_in_order_and_stops_at_shutdown() {
    let input = format!(
        "{}\n\n{}\n{}\n{}\n",
        protocol::encode_request(&Request::Load {
            id: 1,
            threads: 4,
            files: sample_files()
        }),
        protocol::encode_request(&Request::Transcribe {
            id: 2,
            wav_path: "a.wav".into(),
            language: "en".into()
        }),
        protocol::encode_request(&Request::Shutdown { id: 3 }),
        protocol::encode_request(&Request::Transcribe {
            id: 4,
            wav_path: "never.wav".into(),
            language: "en".into()
        }),
    );
    let (responses, engine) = serve_lines(&input);
    assert_eq!(responses.len(), 2, "nothing is answered after shutdown");
    assert_eq!(responses[0].id(), 1);
    assert_eq!(responses[1].id(), 2);
    assert_eq!(engine.loads, 1);
    assert_eq!(engine.heard, vec!["a.wav|en".to_string()]);
}

#[test]
fn the_loop_ends_when_its_input_ends_and_answers_a_bad_line_without_stopping() {
    let (responses, _) = serve_lines("this is not json\n");
    assert_eq!(
        responses,
        vec![Response::Failed {
            id: 0,
            kind: FailureKind::BadRequest
        }]
    );
    let (responses, _) = serve_lines("");
    assert!(responses.is_empty());
}

#[test]
fn the_worker_ends_by_itself_when_the_application_that_started_it_is_gone() {
    // The pipe on its standard input is how a worker dies with the application: when the other end
    // closes, the loop ends. Nothing is sent: the stdin handle is simply dropped.
    let launch = fake_launch(&[]);
    let mut child = Command::new(&launch.program)
        .args(&launch.args)
        .envs(launch.envs.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("fake worker starts");
    drop(child.stdin.take());
    let ended = wait_until(Duration::from_secs(10), || {
        matches!(child.try_wait(), Ok(Some(_)))
    });
    if !ended {
        let _ = child.kill();
    }
    assert!(ended, "the worker outlived its input");
}

// ---- the provider with a real child process ---------------------------------------------------

#[test]
fn a_clip_is_transcribed_by_a_separate_process_and_the_model_is_loaded_once() {
    let rig = rig(&[]);
    assert!(
        !rig.provider.is_running(),
        "nothing runs before the first job"
    );
    let (request, _) = request_in(&rig, 2_000);

    let first = rig
        .provider
        .transcribe(&request, &CancelToken::new())
        .expect("first job");
    assert_eq!(first.text, SENTENCE);
    assert_eq!(first.audio_ms, 2_000);
    assert_eq!(first.engine_id, rig.spec.id);
    assert!(rig.provider.is_running());
    assert_eq!(rig.provider.last_load_ms(), Some(7));

    let second = rig
        .provider
        .transcribe(&request, &CancelToken::new())
        .expect("second job");
    assert_eq!(second.text, SENTENCE);
    assert_eq!(rig.provider.starts(), 1, "the same worker served both jobs");
}

#[test]
fn stray_lines_from_a_native_library_do_not_disturb_the_answers() {
    let rig = rig(&[(FAKE_STRAY, "1")]);
    let (request, _) = request_in(&rig, 1_000);
    let heard = rig
        .provider
        .transcribe(&request, &CancelToken::new())
        .expect("job");
    assert_eq!(heard.text, SENTENCE);
}

#[test]
fn a_model_the_worker_cannot_load_is_reported_and_the_worker_is_ended() {
    for (kind, expected) in [
        ("missing", "speech_model_missing"),
        ("corrupt", "speech_model_corrupt"),
    ] {
        let rig = rig(&[(FAKE_LOAD_FAILS, kind)]);
        let (request, _) = request_in(&rig, 1_000);
        assert_eq!(
            code(rig.provider.transcribe(&request, &CancelToken::new())),
            expected
        );
        assert!(!rig.provider.is_running());
        assert_eq!(rig.provider.last_ending(), Some(Ending::Killed));
    }
}

#[test]
fn a_worker_that_crashes_is_reported_and_a_new_one_starts_on_the_next_job() {
    let rig = rig(&[(FAKE_CRASH, "transcribe")]);
    let (request, _) = request_in(&rig, 1_000);
    assert_eq!(
        code(rig.provider.transcribe(&request, &CancelToken::new())),
        "speech_engine_failed"
    );
    assert!(!rig.provider.is_running());
    let _ = rig.provider.transcribe(&request, &CancelToken::new());
    assert_eq!(rig.provider.starts(), 2);
}

#[test]
fn a_clip_the_worker_cannot_read_keeps_the_worker_loaded() {
    let rig = rig(&[(FAKE_BAD_AUDIO, "1")]);
    let (request, _) = request_in(&rig, 1_000);
    assert_eq!(
        code(rig.provider.transcribe(&request, &CancelToken::new())),
        "speech_audio_invalid"
    );
    assert!(rig.provider.is_running(), "a bad clip is not a bad worker");
}

#[test]
fn a_worker_that_cannot_be_started_is_an_engine_failure_not_a_panic() {
    let rig = rig(&[]);
    rig.provider.configure(Setup {
        launch: WorkerLaunch::program(rig.dir.path().join("no-such-worker")),
        model_folder: rig.spec.folder(&worker_stt::models_root(rig.dir.path())),
    });
    let (request, _) = request_in(&rig, 1_000);
    assert_eq!(
        code(rig.provider.transcribe(&request, &CancelToken::new())),
        "speech_engine_failed"
    );
    assert!(!rig.provider.is_running());
}

#[test]
fn cancel_kills_the_child_promptly() {
    let rig = rig(&[(FAKE_DECODE_MS, "10000")]);
    let (request, _) = request_in(&rig, 1_000);
    let token = CancelToken::new();
    let stopper = token.clone();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        stopper.cancel();
    });
    let started = Instant::now();
    let outcome = rig.provider.transcribe(&request, &token);
    handle.join().expect("stopper");
    assert_eq!(code(outcome), "speech_cancelled");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the cancel waited for the decode"
    );
    assert!(!rig.provider.is_running());
    assert_eq!(rig.provider.last_ending(), Some(Ending::Killed));
}

#[test]
fn a_cancel_that_is_already_set_starts_nothing() {
    let rig = rig(&[]);
    let (request, _) = request_in(&rig, 1_000);
    let token = CancelToken::new();
    token.cancel();
    assert_eq!(
        code(rig.provider.transcribe(&request, &token)),
        "speech_cancelled"
    );
    assert_eq!(
        rig.provider.starts(),
        0,
        "no process for a job already stopped"
    );
}

#[test]
fn through_the_service_a_cancel_leaves_no_clip_and_no_process() {
    let rig = rig(&[(FAKE_DECODE_MS, "10000")]);
    let service = Arc::new(SpeechService::new(
        Some(rig.provider.clone()),
        Limits::default(),
    ));
    let temp_dir = rig.dir.path().join("speech-temp");
    let stopper = Arc::clone(&service);
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1_200));
        stopper.cancel()
    });
    let outcome = tauri::async_runtime::block_on(service.transcribe(
        &canonical_clip(2_000),
        &plan(),
        &temp_dir,
    ));
    assert!(
        handle.join().expect("stopper"),
        "a job was running to cancel"
    );
    assert_eq!(code(outcome), "speech_cancelled");

    assert!(
        wait_until(Duration::from_secs(5), || !service.is_busy()),
        "the service stays busy after a cancel"
    );
    let left: Vec<_> = fs::read_dir(&temp_dir)
        .map(|entries| entries.flatten().collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "a clip was left behind: {left:?}");
    assert!(!rig.provider.is_running());
    assert_eq!(rig.provider.last_ending(), Some(Ending::Killed));
}

#[test]
fn through_the_service_a_clip_is_transcribed_and_removed() {
    let rig = rig(&[]);
    let service = SpeechService::with_provider(rig.provider.clone());
    let temp_dir = rig.dir.path().join("speech-temp");
    let heard = tauri::async_runtime::block_on(service.transcribe(
        &canonical_clip(3_000),
        &plan(),
        &temp_dir,
    ))
    .expect("job");
    assert_eq!(heard.text, SENTENCE);
    assert_eq!(heard.engine_id, rig.spec.id);
    let left = fs::read_dir(&temp_dir)
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(left, 0);
}

#[test]
fn an_idle_worker_is_ended_gracefully_and_the_next_job_starts_a_new_one() {
    let rig = rig_with(&[], Duration::from_millis(200));
    let (request, _) = request_in(&rig, 1_000);
    rig.provider
        .transcribe(&request, &CancelToken::new())
        .expect("job");
    assert!(rig.provider.is_running());
    assert!(
        wait_until(Duration::from_secs(10), || !rig.provider.is_running()),
        "the idle worker was not ended"
    );
    assert_eq!(rig.provider.last_ending(), Some(Ending::Graceful));

    rig.provider
        .transcribe(&request, &CancelToken::new())
        .expect("job after the idle end");
    assert_eq!(rig.provider.starts(), 2);
}

#[test]
fn a_worker_is_never_ended_while_a_job_is_running() {
    // The delay is far longer than the idle limit: only a job in progress keeps the worker alive.
    let rig = rig_with(&[(FAKE_DECODE_MS, "900")], Duration::from_millis(100));
    let (request, _) = request_in(&rig, 1_000);
    let heard = rig
        .provider
        .transcribe(&request, &CancelToken::new())
        .expect("the job survives the idle limit");
    assert_eq!(heard.text, SENTENCE);
    assert_eq!(rig.provider.starts(), 1);
}

#[test]
fn shutdown_ends_the_worker_and_dropping_the_provider_kills_it() {
    let rig = rig(&[]);
    let (request, _) = request_in(&rig, 1_000);
    rig.provider
        .transcribe(&request, &CancelToken::new())
        .expect("job");
    rig.provider.shutdown();
    assert!(!rig.provider.is_running());
    assert_eq!(rig.provider.last_ending(), Some(Ending::Graceful));
    // Dropping a provider with a live worker must not leave the process behind: the drop kills it
    // and waits, which this test can only prove by not hanging.
    rig.provider
        .transcribe(&request, &CancelToken::new())
        .expect("job");
    let Rig { provider, .. } = rig;
    drop(provider);
}

#[test]
fn the_idle_rule_is_a_pure_comparison() {
    let start = Instant::now();
    let idle = Duration::from_secs(120);
    assert!(!idle_expired(start, start, idle));
    assert!(!idle_expired(start, start + Duration::from_secs(119), idle));
    assert!(idle_expired(start, start + idle, idle));
    assert!(idle_expired(start, start + Duration::from_secs(500), idle));
    // A clock that reads earlier than the last use never ends a worker.
    assert!(!idle_expired(start + Duration::from_secs(5), start, idle));
}

#[test]
fn the_measured_figures_are_the_ones_the_report_quotes() {
    assert_eq!(STT_THREADS, 4);
    assert_eq!(IDLE_UNLOAD_SECONDS, 120);
    assert_eq!(COLD_LOAD_SECONDS, 2);
    assert_eq!(RESIDENT_MEMORY_MB, (1_600, 1_900));
    assert_eq!(BUSY_CORES, (7, 10));
}

// ---- availability and discovery ---------------------------------------------------------------

#[test]
fn availability_names_the_first_obstacle() {
    let (spec, _) = tiny_model();
    let provider = WorkerSttProvider::new(spec.clone());
    assert_eq!(
        code(provider.availability("fr-FR")),
        "speech_engine_missing",
        "nothing is configured yet"
    );

    let dir = tempfile::tempdir().expect("temp dir");
    let folder = spec.folder(&worker_stt::models_root(dir.path()));
    let exe = std::env::current_exe().expect("exe");
    provider.configure(Setup {
        launch: WorkerLaunch::program(dir.path().join("absent-worker")),
        model_folder: folder.clone(),
    });
    assert_eq!(
        code(provider.availability("fr-FR")),
        "speech_engine_missing"
    );
    assert!(!provider.engine_found());

    provider.configure(Setup {
        launch: WorkerLaunch::program(exe),
        model_folder: folder.clone(),
    });
    assert!(provider.engine_found());
    assert_eq!(provider.model_state(), ModelState::Missing);
    assert_eq!(code(provider.availability("fr-FR")), "speech_model_missing");

    place_tiny_files(&folder);
    assert_eq!(provider.model_state(), ModelState::Unverified);
    assert_eq!(code(provider.availability("fr-FR")), "speech_model_missing");

    models::verify(&spec, &folder, &CancelToken::new()).expect("verifies");
    assert_eq!(provider.model_state(), ModelState::Present);
    provider.availability("fr-FR").expect("ready");
    provider
        .availability("en-US")
        .expect("english is ready too");
    assert_eq!(
        code(provider.availability("de-DE")),
        "speech_locale_unsupported"
    );

    fs::write(folder.join("enc.bin"), b"short").expect("damage");
    assert_eq!(provider.model_state(), ModelState::Corrupt);
    assert_eq!(code(provider.availability("fr-FR")), "speech_model_corrupt");
}

#[test]
fn the_worker_is_searched_under_the_sidecar_roots_by_its_triple_names() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("a");
    let second = dir.path().join("b").join("binaries");
    fs::create_dir_all(&first).expect("dir");
    fs::create_dir_all(&second).expect("dir");
    let roots = vec![first.clone(), second.clone()];

    let candidates = worker_stt::worker_candidates(&roots);
    assert!(candidates.starts_with(&[first.join("speech-stt-worker.exe")]));
    for triple in [
        "speech-stt-worker-x86_64-pc-windows-msvc.exe",
        "speech-stt-worker-aarch64-apple-darwin",
        "speech-stt-worker-x86_64-apple-darwin",
    ] {
        assert!(candidates.contains(&second.join(triple)), "{triple}");
    }
    assert_eq!(worker_stt::find_worker(&roots), None);

    let staged = second.join("speech-stt-worker-aarch64-apple-darwin");
    fs::write(&staged, b"x").expect("staged");
    assert_eq!(worker_stt::find_worker(&roots), Some(staged.clone()));

    let (spec, _) = tiny_model();
    let setup = worker_stt::setup_for(&roots, dir.path(), &spec);
    assert_eq!(setup.launch.program, staged);
    assert_eq!(
        setup.model_folder,
        dir.path().join("speech-models").join("tiny-model")
    );
    let none = worker_stt::setup_for(&[], dir.path(), &spec);
    assert!(!none.launch.program.is_file());
}

#[test]
fn the_language_the_worker_is_given_comes_from_the_language_table() {
    let request = {
        let rig = rig(&[]);
        request_in(&rig, 1_000).0
    };
    assert_eq!(request.language, "fr");
    let row = languages::resolve("fr-FR", languages::Feature::Dictation).expect("french");
    assert_eq!(row.engine_code, plan().language);
}

// ---- the platform detail ----------------------------------------------------------------------

#[test]
fn a_command_started_without_a_console_window_still_runs() {
    let mut command = Command::new(std::env::current_exe().expect("exe"));
    command
        .args(["--list", "--format=terse"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    super::console_window::hide(&mut command);
    assert!(command.status().expect("starts").success());
}

// ---- what the files may and may not contain ---------------------------------------------------

fn source(name: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name))
        .expect("source reads")
}

#[test]
fn the_client_the_protocol_and_the_models_open_no_connection() {
    for name in [
        "speech/worker_stt.rs",
        "speech/worker_protocol.rs",
        "speech/models.rs",
        "speech/console_window.rs",
    ] {
        let text = source(name);
        for token in [
            "reqwest",
            "ureq",
            "hyper",
            "std::net",
            "TcpStream",
            "UdpSocket",
            "TcpListener",
            "println!",
        ] {
            assert!(!text.contains(token), "{name} uses {token}");
        }
    }
}

#[test]
fn the_platform_flag_lives_only_in_the_console_window_module() {
    for name in [
        "speech/worker_stt.rs",
        "speech/worker_protocol.rs",
        "speech/models.rs",
    ] {
        let text = source(name);
        assert!(!text.contains("#[cfg(windows)]"), "{name}");
        assert!(!text.contains("#[cfg(not(windows))]"), "{name}");
        assert!(!text.contains("creation_flags"), "{name}");
    }
    let window = source("speech/console_window.rs");
    assert!(window.contains("#[cfg(windows)]"));
    assert!(window.contains("#[cfg(not(windows))]"));
}

#[test]
fn the_protocol_file_needs_no_other_module_of_the_application() {
    // The worker crate includes this file by path, so it may use serde and the standard library only.
    let text = source("speech/worker_protocol.rs");
    for token in ["crate::", "super::", "tauri", "AppError"] {
        assert!(
            !text.lines().any(|line| {
                let code = line.trim_start();
                !code.starts_with("//") && code.contains(token)
            }),
            "worker_protocol.rs uses {token}"
        );
    }
}

#[test]
fn nothing_logs_what_a_worker_said() {
    let client = source("speech/worker_stt.rs");
    assert!(!client.contains("eprintln!"));
    assert!(client.contains(".stderr(Stdio::null())"));
    assert!(!client.contains("Stdio::inherit"));
}

#[test]
fn the_worker_crate_contains_no_network_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../speech-worker");
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("worker manifest");
    for name in ["reqwest", "ureq", "hyper", "tokio", "tauri"] {
        assert!(
            !manifest
                .lines()
                .filter(|line| !line.trim_start().starts_with('#'))
                .any(|line| line.trim_start().starts_with(name)),
            "the worker crate depends on {name}"
        );
    }
    for relative in [
        "src/lib.rs",
        "src/wav.rs",
        "src/engine.rs",
        "src/bin/stt.rs",
    ] {
        let text = fs::read_to_string(root.join(relative)).expect("worker source");
        for token in ["std::net", "TcpStream", "UdpSocket", "reqwest", "ureq"] {
            assert!(!text.contains(token), "{relative} uses {token}");
        }
    }
}

// ---- the real engine --------------------------------------------------------------------------

/// Transcribes a real clip with the real worker and the real model. Run by hand on a machine that
/// has both: `cargo test --lib real_worker -- --ignored --nocapture`.
///
/// `ACAI_STT_WORKER` is the worker executable (default: the one staged by
/// `scripts/build-speech-worker.sh`), `ACAI_STT_MODEL_FOLDER` the folder of the model's files (their
/// names come from the manifest), `ACAI_STT_CLIP` a 16 kHz mono WAV (default: none, a tone is used,
/// which the engine may answer with nothing). The model need not carry the marker: the test only
/// reads the files.
#[test]
#[ignore = "needs the real worker and the real model on disk"]
fn real_worker_transcribes_a_clip_and_cancel_leaves_no_process() {
    let spec = models::stt_model().expect("manifest").clone();
    let staged = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join("speech-stt-worker-x86_64-pc-windows-msvc.exe");
    let worker = std::env::var("ACAI_STT_WORKER")
        .map(PathBuf::from)
        .unwrap_or(staged);
    let folder =
        PathBuf::from(std::env::var("ACAI_STT_MODEL_FOLDER").expect("set ACAI_STT_MODEL_FOLDER"));
    let provider = Arc::new(WorkerSttProvider::new(spec));
    provider.configure(Setup {
        launch: WorkerLaunch::program(worker),
        model_folder: folder,
    });

    let dir = tempfile::tempdir().expect("temp dir");
    let clip = match std::env::var("ACAI_STT_CLIP") {
        Ok(path) => fs::read(path).expect("clip reads"),
        Err(_) => canonical_clip(3_000),
    };
    let path = dir.path().join("clip.wav");
    fs::File::create(&path)
        .and_then(|mut file| file.write_all(&clip))
        .expect("clip writes");
    let request = SttRequest {
        wav_path: path,
        language: "fr".into(),
        max_seconds: 120,
    };

    let cold = Instant::now();
    let first = provider
        .transcribe(&request, &CancelToken::new())
        .expect("first job");
    let cold_ms = cold.elapsed().as_millis();
    let warm = Instant::now();
    let second = provider
        .transcribe(&request, &CancelToken::new())
        .expect("second job");
    let warm_ms = warm.elapsed().as_millis();
    println!(
        "measured: cold job {cold_ms} ms (load {:?} ms), warm job {warm_ms} ms, clip {} ms, words {}",
        provider.last_load_ms(),
        first.audio_ms,
        first.text.split_whitespace().count()
    );
    assert_eq!(first.text, second.text, "the same clip gives the same text");

    // A sixty-second clip keeps the engine busy for seconds; the cancel arrives while it decodes.
    let long = dir.path().join("long.wav");
    fs::write(&long, canonical_clip(60_000)).expect("long clip writes");
    let long_request = SttRequest {
        wav_path: long,
        language: "fr".into(),
        max_seconds: 120,
    };
    let token = CancelToken::new();
    let stopper = token.clone();
    let handle = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        stopper.cancel();
    });
    let started = Instant::now();
    let outcome = provider.transcribe(&long_request, &token);
    handle.join().expect("stopper");
    println!(
        "measured: cancel returned after {} ms",
        started.elapsed().as_millis()
    );
    assert_eq!(code(outcome), "speech_cancelled");
    assert!(!provider.is_running());
    assert_eq!(provider.last_ending(), Some(Ending::Killed));
}

/// A mono 16-bit PCM WAV at 16 kHz around `pcm` (little-endian samples).
fn wav_around(pcm: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(44 + pcm.len());
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&16_000u32.to_le_bytes());
    bytes.extend_from_slice(&32_000u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    bytes.extend_from_slice(pcm);
    bytes
}

/// A long recording of a note is sent to the engine as passages, one after the other: the warm
/// worker answers each in turn and the texts, put end to end, keep the order of the speech. Run by
/// hand with the variables of the test above and `ACAI_STT_CLIP` set to a 16 kHz mono 16-bit WAV of
/// about 30 seconds with the canonical 44-byte header:
/// `cargo test --lib real_worker_two_passages -- --ignored --nocapture`.
#[test]
#[ignore = "needs the real worker, the real model and a recorded clip of about 30 seconds"]
fn real_worker_two_passages_come_back_in_order() {
    let spec = models::stt_model().expect("manifest").clone();
    let staged = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join("speech-stt-worker-x86_64-pc-windows-msvc.exe");
    let worker = std::env::var("ACAI_STT_WORKER")
        .map(PathBuf::from)
        .unwrap_or(staged);
    let folder =
        PathBuf::from(std::env::var("ACAI_STT_MODEL_FOLDER").expect("set ACAI_STT_MODEL_FOLDER"));
    let clip = fs::read(std::env::var("ACAI_STT_CLIP").expect("set ACAI_STT_CLIP")).expect("clip");
    assert!(clip.len() > 44 + 2 * 16_000 * 2, "the clip is too short");
    let pcm = &clip[44..];
    let middle = (pcm.len() / 2) & !1;

    let provider = Arc::new(WorkerSttProvider::new(spec));
    provider.configure(Setup {
        launch: WorkerLaunch::program(worker),
        model_folder: folder,
    });
    let dir = tempfile::tempdir().expect("temp dir");
    let mut texts = Vec::new();
    for (index, part) in [&pcm[..middle], &pcm[middle..]].into_iter().enumerate() {
        let path = dir.path().join(format!("passage-{index}.wav"));
        fs::write(&path, wav_around(part)).expect("passage writes");
        let request = SttRequest {
            wav_path: path,
            language: "fr".into(),
            max_seconds: 120,
        };
        let heard = provider
            .transcribe(&request, &CancelToken::new())
            .expect("a passage is transcribed");
        println!(
            "measured: passage {index}: {} ms of audio in {} ms, {} words",
            heard.audio_ms,
            heard.processing_ms,
            heard.text.split_whitespace().count()
        );
        texts.push(heard.text);
    }
    assert_eq!(texts.len(), 2);
    assert!(
        texts.iter().all(|text| !text.trim().is_empty()),
        "both passages carry words"
    );
    println!("measured: the two texts, in order: {texts:?}");
}
