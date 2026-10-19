//! Tests of the voice protocol, of the voice worker client and of its idle, cancel and cleanup logic.
//!
//! Like the tests of the other worker, the fake voice worker is this very test executable started
//! again with an environment variable: `fake_voice_worker_entry` does nothing in a normal run and,
//! when `ACAI_FAKE_VOICE_WORKER` is set, runs the real worker loop (`worker_protocol::serve`) over a
//! scripted engine, then exits. So the protocol, the process handling, the kill on cancel, the audio
//! file and its removal are proven with a real child process, without a voice and without the real
//! engine. The real engines have one `#[ignore]` test at the end.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::models::{self, ModelSpec};
use super::status::ModelState;
use super::stt::CancelToken;
use super::test_support::{place_tiny_voice, tiny_voice};
use super::tts::MAX_TEXT_CHARS;
use super::worker_protocol::{
    self as protocol, Engine, FailureKind, Heard, ModelFiles, Request, Response, Spoken,
    VoiceFamily, VoiceFiles,
};
use super::worker_stt::{Ending, WorkerLaunch};
use super::worker_tts::{self, TtsSetup, WorkerTtsProvider};
use crate::error::AppError;

// ---- the scripted worker ----------------------------------------------------------------------

const FAKE_FLAG: &str = "ACAI_FAKE_VOICE_WORKER";
const FAKE_SYNTH_MS: &str = "ACAI_FAKE_SYNTH_MS";
const FAKE_LOAD_FAILS: &str = "ACAI_FAKE_VOICE_LOAD_FAILS";
const FAKE_CRASH: &str = "ACAI_FAKE_VOICE_CRASH";
const FAKE_STRAY: &str = "ACAI_FAKE_VOICE_STRAY";
const SAMPLE_RATE: u32 = 16_000;
/// How much audio the scripted voice makes for one character at the natural speed.
const MS_PER_CHARACTER: u64 = 20;

struct ScriptedVoice {
    loaded: Option<String>,
}

impl Engine for ScriptedVoice {
    fn load(&mut self, _threads: u32, _files: &ModelFiles) -> Result<u64, FailureKind> {
        Err(FailureKind::BadRequest)
    }

    fn transcribe(&mut self, _wav_path: &str, _language: &str) -> Result<Heard, FailureKind> {
        Err(FailureKind::BadRequest)
    }

    fn load_voice(&mut self, _threads: u32, voice: &VoiceFiles) -> Result<u64, FailureKind> {
        match std::env::var(FAKE_LOAD_FAILS).as_deref() {
            Ok("missing") => return Err(FailureKind::ModelMissing),
            Ok("corrupt") => return Err(FailureKind::ModelCorrupt),
            _ => {}
        }
        // The worker is given real paths: prove they are the voice's files and folder.
        let mut needed = vec![voice.model.as_str(), voice.tokens.as_str()];
        needed.extend(voice.data_dir.as_deref());
        for path in needed {
            if !Path::new(path).exists() {
                return Err(FailureKind::ModelMissing);
            }
        }
        self.loaded = Some(voice.model.clone());
        Ok(5)
    }

    fn synthesize(
        &mut self,
        text: &str,
        speed_percent: u32,
        wav_path: &str,
    ) -> Result<Spoken, FailureKind> {
        if std::env::var(FAKE_STRAY).is_ok() {
            // What a native library does: lines of its own on the standard output.
            println!("a stray line from a native library");
            println!("{{ not a response }}");
        }
        if std::env::var(FAKE_CRASH).as_deref() == Ok("synthesize") {
            std::process::exit(3);
        }
        if self.loaded.is_none() {
            return Err(FailureKind::EngineFailed);
        }
        if !text.chars().any(char::is_alphanumeric) {
            return Err(FailureKind::NothingToSay);
        }
        let delay: u64 = std::env::var(FAKE_SYNTH_MS)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        std::thread::sleep(Duration::from_millis(delay));
        let audio_ms =
            text.chars().count() as u64 * MS_PER_CHARACTER * 100 / u64::from(speed_percent.max(1));
        let count = (audio_ms * u64::from(SAMPLE_RATE) / 1_000) as usize;
        let samples: Vec<f32> = (0..count)
            .map(|index| (index as f32 * 0.05).sin() * 0.3)
            .collect();
        let bytes = encode(&samples);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(wav_path)
            .map_err(|_| FailureKind::EngineFailed)?;
        std::io::Write::write_all(&mut file, &bytes).map_err(|_| FailureKind::EngineFailed)?;
        Ok(Spoken {
            audio_ms,
            processing_ms: delay,
            sample_rate: SAMPLE_RATE,
        })
    }
}

/// A 16-bit mono WAV, written here on purpose apart from the worker crate's own encoder.
fn encode(samples: &[f32]) -> Vec<u8> {
    let data: Vec<u8> = samples
        .iter()
        .flat_map(|sample| ((sample * 32_767.0) as i16).to_le_bytes())
        .collect();
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    out
}

/// Not a test of anything: the entry point of the scripted voice worker. In a normal run it returns
/// at once. Started with the flag, it serves the protocol until shutdown or the end of its input.
#[test]
fn fake_voice_worker_entry() {
    if std::env::var(FAKE_FLAG).is_err() {
        return;
    }
    // The test harness has written "test ... " without a line break; start on a fresh line.
    println!();
    let input = std::io::stdin().lock();
    let output = std::io::stdout();
    let mut engine = ScriptedVoice { loaded: None };
    let outcome = protocol::serve(input, output, &mut engine);
    std::process::exit(if outcome.is_ok() { 0 } else { 1 });
}

fn fake_launch(envs: &[(&str, &str)]) -> WorkerLaunch {
    let mut launch = WorkerLaunch::program(std::env::current_exe().expect("test exe"));
    launch.args = [
        "--exact",
        "speech::worker_tts_tests::fake_voice_worker_entry",
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
    provider: Arc<WorkerTtsProvider>,
}

impl Rig {
    fn models_root(&self) -> PathBuf {
        self.dir.path().join("models")
    }

    fn temp_dir(&self) -> PathBuf {
        self.dir.path().join("speech-temp")
    }

    fn left_in_temp(&self) -> usize {
        fs::read_dir(self.temp_dir())
            .map(|entries| entries.flatten().count())
            .unwrap_or(0)
    }
}

fn rig_with(envs: &[(&str, &str)], idle: Duration) -> Rig {
    let dir = tempfile::tempdir().expect("temp dir");
    let (spec, _) = tiny_voice();
    let root = dir.path().join("models");
    let folder = spec.folder(&root);
    place_tiny_voice(&folder);
    models::verify(&spec, &folder, &CancelToken::new()).expect("tiny voice verifies");
    let provider = Arc::new(WorkerTtsProvider::with_timing(idle, 2));
    provider.configure(TtsSetup {
        launch: fake_launch(envs),
        models_root: root,
        temp_dir: dir.path().join("speech-temp"),
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

fn wav_duration_ms(wav: &[u8]) -> u64 {
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    ((wav.len() - 44) / 2) as u64 * 1_000 / u64::from(SAMPLE_RATE)
}

// ---- the protocol -----------------------------------------------------------------------------

fn sample_voice() -> VoiceFiles {
    VoiceFiles {
        family: VoiceFamily::Piper,
        model: "m.onnx".into(),
        tokens: "t.txt".into(),
        data_dir: Some("d".into()),
    }
}

#[test]
fn the_voice_requests_and_answers_survive_one_line_each() {
    let requests = [
        Request::LoadVoice {
            id: 1,
            threads: 4,
            voice: sample_voice(),
        },
        Request::LoadVoice {
            id: 2,
            threads: 1,
            voice: VoiceFiles {
                family: VoiceFamily::Coqui,
                data_dir: None,
                ..sample_voice()
            },
        },
        Request::Synthesize {
            id: 3,
            text: "Bonjour, \"monde\"\nsecond line".into(),
            speed_percent: 125,
            wav_path: "clip-1-0.wav".into(),
        },
    ];
    for request in requests {
        let line = protocol::encode_request(&request);
        assert!(!line.contains('\n'), "one request is one line");
        assert_eq!(
            serde_json::from_str::<Request>(&line).expect("parses"),
            request
        );
    }
    let answers = [
        Response::VoiceLoaded {
            id: 1,
            load_ms: 900,
        },
        Response::Synthesized {
            id: 3,
            audio_ms: 1_200,
            processing_ms: 70,
            sample_rate: 22_050,
        },
        Response::Failed {
            id: 3,
            kind: FailureKind::NothingToSay,
        },
    ];
    for answer in answers {
        let line = protocol::encode_response(&answer);
        assert!(!line.contains('\n'));
        assert_eq!(protocol::decode_response(&line), Some(answer.clone()));
        assert_eq!(
            answer.id(),
            protocol::decode_response(&line).expect("decodes").id()
        );
    }
}

#[test]
fn the_loop_serves_a_voice_in_order_and_a_speech_to_text_engine_refuses_it() {
    let input = [
        protocol::encode_request(&Request::Synthesize {
            id: 1,
            text: "hello".into(),
            speed_percent: 100,
            wav_path: "unused.wav".into(),
        }),
        protocol::encode_request(&Request::LoadVoice {
            id: 2,
            threads: 1,
            voice: sample_voice(),
        }),
    ]
    .join("\n");
    // A voice engine that has no voice yet cannot speak: the failure carries the id of the request.
    let mut engine = ScriptedVoice { loaded: None };
    let mut output = Vec::new();
    protocol::serve(Cursor::new(input.clone()), &mut output, &mut engine).expect("serves");
    let lines: Vec<Response> = String::from_utf8(output)
        .expect("utf8")
        .lines()
        .filter_map(protocol::decode_response)
        .collect();
    assert_eq!(
        lines[0],
        Response::Failed {
            id: 1,
            kind: FailureKind::EngineFailed
        }
    );
    assert_eq!(
        lines[1],
        Response::Failed {
            id: 2,
            kind: FailureKind::ModelMissing
        },
        "the files of the sample do not exist"
    );

    // An engine that only transcribes answers a voice request with a bad request, and goes on.
    struct SpeechToText;
    impl Engine for SpeechToText {
        fn load(&mut self, _: u32, _: &ModelFiles) -> Result<u64, FailureKind> {
            Ok(1)
        }
        fn transcribe(&mut self, _: &str, _: &str) -> Result<Heard, FailureKind> {
            Err(FailureKind::AudioInvalid)
        }
    }
    let mut output = Vec::new();
    protocol::serve(Cursor::new(input), &mut output, &mut SpeechToText).expect("serves");
    let answers: Vec<Response> = String::from_utf8(output)
        .expect("utf8")
        .lines()
        .filter_map(protocol::decode_response)
        .collect();
    assert_eq!(answers.len(), 2);
    assert!(answers.iter().all(|answer| matches!(
        answer,
        Response::Failed {
            kind: FailureKind::BadRequest,
            ..
        }
    )));
}

// ---- the client -----------------------------------------------------------------------------------

#[test]
fn a_sentence_comes_back_as_a_wav_and_the_audio_file_is_gone() {
    let rig = rig(&[]);
    let spoken = rig
        .provider
        .synthesize(&rig.spec, "Hello everyone here", 100)
        .expect("speaks");
    assert_eq!(spoken.voice_id, "tiny-voice");
    assert_eq!(spoken.audio_ms, 19 * MS_PER_CHARACTER);
    assert_eq!(wav_duration_ms(&spoken.wav), spoken.audio_ms);
    assert_eq!(
        rig.left_in_temp(),
        0,
        "the file is removed as soon as it is read"
    );
    assert!(rig.provider.is_running());
    assert_eq!(rig.provider.last_load_ms(), Some(5));
}

#[test]
fn the_speed_reaches_the_worker_as_a_percentage() {
    let rig = rig(&[]);
    let natural = rig
        .provider
        .synthesize(&rig.spec, "twenty characters!!", 100)
        .expect("natural");
    let fast = rig
        .provider
        .synthesize(&rig.spec, "twenty characters!!", 200)
        .expect("fast");
    let slow = rig
        .provider
        .synthesize(&rig.spec, "twenty characters!!", 50)
        .expect("slow");
    assert_eq!(fast.audio_ms * 2, natural.audio_ms);
    assert_eq!(slow.audio_ms, natural.audio_ms * 2);
}

#[test]
fn the_same_worker_serves_the_next_sentences_and_the_voice_is_loaded_once() {
    let rig = rig(&[]);
    for text in ["First sentence.", "Second one.", "And a third."] {
        rig.provider.synthesize(&rig.spec, text, 100).expect(text);
    }
    assert_eq!(
        rig.provider.starts(),
        1,
        "one process for the whole reading"
    );
    assert_eq!(rig.left_in_temp(), 0);
}

#[test]
fn another_voice_is_loaded_into_the_same_worker() {
    let rig = rig(&[]);
    rig.provider
        .synthesize(&rig.spec, "In the first voice.", 100)
        .expect("first");
    // A second voice with its own folder and manifest id.
    let mut second = rig.spec.clone();
    second.id = "tiny-voice-two".into();
    let folder = second.folder(&rig.models_root());
    place_tiny_voice(&folder);
    models::verify(&second, &folder, &CancelToken::new()).expect("verifies");
    let spoken = rig
        .provider
        .synthesize(&second, "In the second voice.", 100)
        .expect("second");
    assert_eq!(spoken.voice_id, "tiny-voice-two");
    assert_eq!(
        rig.provider.starts(),
        1,
        "the worker loads the new voice itself"
    );
}

#[test]
fn text_without_anything_to_say_is_a_failed_reading_not_a_panic() {
    let rig = rig(&[]);
    assert_eq!(
        code(rig.provider.synthesize(&rig.spec, " ... ?! ", 100)),
        "tts_failed"
    );
    assert_eq!(rig.left_in_temp(), 0);
    // The next sentence still works: a new worker starts if the old one was ended.
    rig.provider
        .synthesize(&rig.spec, "Fine again.", 100)
        .expect("speaks");
}

#[test]
fn a_text_over_the_limit_is_refused_without_starting_a_process() {
    let rig = rig(&[]);
    let long = "a".repeat(MAX_TEXT_CHARS + 1);
    assert_eq!(
        code(rig.provider.synthesize(&rig.spec, &long, 100)),
        "tts_failed"
    );
    assert_eq!(rig.provider.starts(), 0);
}

#[test]
fn stray_lines_from_a_native_library_do_not_disturb_the_answers() {
    let rig = rig(&[(FAKE_STRAY, "1")]);
    let spoken = rig
        .provider
        .synthesize(&rig.spec, "Still fine.", 100)
        .expect("speaks");
    assert_eq!(wav_duration_ms(&spoken.wav), spoken.audio_ms);
}

#[test]
fn a_voice_the_worker_cannot_load_is_reported_and_the_worker_is_ended() {
    for (kind, expected) in [
        ("missing", "tts_no_local_voice"),
        ("corrupt", "speech_model_corrupt"),
    ] {
        let rig = rig(&[(FAKE_LOAD_FAILS, kind)]);
        assert_eq!(
            code(rig.provider.synthesize(&rig.spec, "Hello.", 100)),
            expected
        );
        assert!(!rig.provider.is_running());
        assert_eq!(rig.provider.last_ending(), Some(Ending::Killed));
        assert_eq!(rig.left_in_temp(), 0);
    }
}

#[test]
fn a_worker_that_crashes_is_a_failed_reading_and_a_new_one_starts_next_time() {
    let rig = rig(&[(FAKE_CRASH, "synthesize")]);
    assert_eq!(
        code(rig.provider.synthesize(&rig.spec, "Hello.", 100)),
        "tts_failed"
    );
    assert!(!rig.provider.is_running());
    assert_eq!(rig.left_in_temp(), 0);
    let _ = rig.provider.synthesize(&rig.spec, "Hello again.", 100);
    assert_eq!(rig.provider.starts(), 2);
}

#[test]
fn a_worker_that_cannot_be_started_is_a_failed_reading_not_a_panic() {
    let rig = rig(&[]);
    let mut setup = TtsSetup {
        launch: WorkerLaunch::program(rig.dir.path().join("no-such-worker")),
        models_root: rig.models_root(),
        temp_dir: rig.temp_dir(),
    };
    // A path that does not exist reads as "not installed".
    rig.provider.configure(setup.clone());
    assert!(!rig.provider.engine_found());
    assert_eq!(
        code(rig.provider.synthesize(&rig.spec, "Hello.", 100)),
        "tts_no_local_voice"
    );
    // A file that exists but is not a program is a failure to start.
    let not_a_program = rig.dir.path().join("not-a-program");
    fs::write(&not_a_program, b"text").expect("file");
    setup.launch = WorkerLaunch::program(not_a_program);
    rig.provider.configure(setup);
    assert_eq!(
        code(rig.provider.synthesize(&rig.spec, "Hello.", 100)),
        "tts_failed"
    );
    assert!(!rig.provider.is_running());
}

#[test]
fn a_voice_that_is_not_installed_checked_and_whole_does_not_read() {
    let rig = rig(&[]);
    let folder = rig.spec.folder(&rig.models_root());

    fs::remove_file(folder.join(models::MARKER_FILE)).expect("marker");
    assert_eq!(models::state(&rig.spec, &folder), ModelState::Unverified);
    assert_eq!(
        code(rig.provider.synthesize(&rig.spec, "Hello.", 100)),
        "tts_no_local_voice",
        "a side-loaded voice waits for Verify"
    );

    fs::write(folder.join("voice.onnx"), b"short").expect("damage");
    assert_eq!(
        code(rig.provider.synthesize(&rig.spec, "Hello.", 100)),
        "speech_model_corrupt"
    );

    fs::remove_dir_all(&folder).expect("remove");
    assert_eq!(
        code(rig.provider.synthesize(&rig.spec, "Hello.", 100)),
        "tts_no_local_voice"
    );
    assert_eq!(
        rig.provider.starts(),
        0,
        "no process for a voice that is not there"
    );
}

#[test]
fn nothing_is_configured_nothing_is_started() {
    let provider = WorkerTtsProvider::default();
    let (spec, _) = tiny_voice();
    assert!(!provider.engine_found());
    assert_eq!(
        code(provider.synthesize(&spec, "Hello.", 100)),
        "tts_no_local_voice"
    );
    assert_eq!(provider.starts(), 0);
    assert!(!provider.cancel(), "nothing to cancel");
}

#[test]
fn cancel_kills_the_child_promptly_and_leaves_no_audio_file() {
    let rig = rig(&[(FAKE_SYNTH_MS, "10000")]);
    let provider = Arc::clone(&rig.provider);
    let spec = rig.spec.clone();
    let job = std::thread::spawn(move || provider.synthesize(&spec, "A long sentence.", 100));
    assert!(
        wait_until(Duration::from_secs(10), || rig.provider.is_busy()),
        "the job started"
    );
    // Long enough for the worker to start, read the voice and begin the synthesis.
    std::thread::sleep(Duration::from_millis(1_500));
    let started = Instant::now();
    assert!(rig.provider.cancel(), "a synthesis was running");
    let outcome = job.join().expect("job thread");
    assert_eq!(code(outcome), "speech_cancelled");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the cancel waited for the synthesis"
    );
    assert!(!rig.provider.is_running());
    assert_eq!(rig.provider.last_ending(), Some(Ending::Killed));
    assert_eq!(rig.left_in_temp(), 0);
    assert!(!rig.provider.cancel(), "nothing is left to cancel");
}

#[test]
fn a_new_request_ends_the_one_in_progress() {
    let rig = rig(&[(FAKE_SYNTH_MS, "10000")]);
    let provider = Arc::clone(&rig.provider);
    let spec = rig.spec.clone();
    let first = std::thread::spawn(move || provider.synthesize(&spec, "First, a long one.", 100));
    assert!(wait_until(Duration::from_secs(10), || rig
        .provider
        .is_busy()));
    std::thread::sleep(Duration::from_millis(1_500));
    // The second request cancels the first at once; it then waits its turn for the worker.
    let provider = Arc::clone(&rig.provider);
    let spec = rig.spec.clone();
    let second = std::thread::spawn(move || provider.synthesize(&spec, "Second.", 100));
    assert_eq!(code(first.join().expect("first")), "speech_cancelled");
    assert!(
        wait_until(Duration::from_secs(10), || rig.provider.cancel()),
        "the second one is the job now"
    );
    assert_eq!(code(second.join().expect("second")), "speech_cancelled");
    assert_eq!(rig.left_in_temp(), 0);
}

#[test]
fn an_idle_worker_is_ended_by_itself_and_a_new_one_starts_on_demand() {
    let rig = rig_with(&[], Duration::from_millis(200));
    rig.provider
        .synthesize(&rig.spec, "One.", 100)
        .expect("speaks");
    assert!(rig.provider.is_running());
    assert!(
        wait_until(Duration::from_secs(5), || !rig.provider.is_running()),
        "the idle worker stays"
    );
    assert_eq!(rig.provider.last_ending(), Some(Ending::Graceful));
    rig.provider
        .synthesize(&rig.spec, "Two.", 100)
        .expect("speaks");
    assert_eq!(rig.provider.starts(), 2);
}

#[test]
fn shutdown_ends_the_worker_before_voice_files_are_replaced() {
    let rig = rig(&[]);
    rig.provider
        .synthesize(&rig.spec, "One.", 100)
        .expect("speaks");
    rig.provider.shutdown();
    assert!(!rig.provider.is_running());
    assert_eq!(rig.provider.last_ending(), Some(Ending::Graceful));
}

#[test]
fn dropping_the_provider_leaves_no_process() {
    let rig = rig(&[]);
    rig.provider
        .synthesize(&rig.spec, "One.", 100)
        .expect("speaks");
    let weak = Arc::downgrade(&rig.provider);
    let Rig { provider, dir, .. } = rig;
    drop(provider);
    assert!(weak.upgrade().is_none(), "the provider is gone");
    drop(dir);
}

#[test]
fn the_worker_is_searched_under_the_sidecar_roots_by_its_triple_names() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = dir.path().join("a");
    let second = dir.path().join("b");
    fs::create_dir_all(&first).expect("a");
    fs::create_dir_all(&second).expect("b");
    assert_eq!(
        worker_tts::find_worker(&[first.clone(), second.clone()]),
        None
    );
    let staged = second.join("speech-tts-worker-x86_64-pc-windows-msvc.exe");
    fs::write(&staged, b"x").expect("staged");
    assert_eq!(
        worker_tts::find_worker(&[first.clone(), second.clone()]),
        Some(staged)
    );
    let installed = first.join("speech-tts-worker.exe");
    fs::write(&installed, b"x").expect("installed");
    assert_eq!(
        worker_tts::find_worker(&[first, second]),
        Some(installed),
        "the first root wins"
    );
    // The speech-to-text worker is never taken for the voice worker.
    let other = tempfile::tempdir().expect("temp dir");
    fs::write(other.path().join("speech-stt-worker.exe"), b"x").expect("stt");
    assert_eq!(worker_tts::find_worker(&[other.path().to_path_buf()]), None);
}

// ---- the real engines ---------------------------------------------------------------------------

/// A paragraph of about forty words, for the figure of "time to speak a paragraph".
const FRENCH_PARAGRAPH: &str = "Bonjour madame. Votre rendez-vous du douze mars reste confirm\u{e9} \
    \u{e0} neuf heures trente. Merci d'apporter votre carte vitale et votre derni\u{e8}re ordonnance. \
    En cas d'emp\u{ea}chement, appelez notre secr\u{e9}tariat au plus vite, afin de lib\u{e9}rer ce cr\u{e9}neau.";
const ENGLISH_PARAGRAPH: &str = "Good morning. Your appointment on the twelfth of March is confirmed \
    for half past nine. Please bring your insurance card and your latest prescription. If you cannot \
    come, call the front desk as soon as possible so that the slot can be given to someone else.";

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("folder");
    for entry in fs::read_dir(from).expect("reads").flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copies");
        }
    }
}

/// The four real voices, through the real worker. Set `ACAI_VOICE_SOURCES` to the folder of voice
/// packages (SpeechLab's models folder) and, if it is not the staged one, `ACAI_TTS_WORKER`. Each
/// voice is copied in, checked against the manifest compiled into the application, then speaks a
/// French or an English paragraph. Run by hand:
/// `cargo test --lib real_voices -- --ignored --nocapture`.
#[test]
#[ignore = "needs the real voices on disk and the real voice worker"]
fn real_voices_speak_a_french_and_an_english_paragraph() {
    let sources =
        PathBuf::from(std::env::var("ACAI_VOICE_SOURCES").expect("set ACAI_VOICE_SOURCES"));
    let worker = std::env::var("ACAI_TTS_WORKER")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("binaries")
                .join("speech-tts-worker-x86_64-pc-windows-msvc.exe")
        });
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("models");
    let provider = WorkerTtsProvider::default();
    provider.configure(TtsSetup {
        launch: WorkerLaunch::program(worker),
        models_root: root.clone(),
        temp_dir: dir.path().join("speech-temp"),
    });
    for (id, source, paragraph) in [
        ("siwis", "vits-piper-fr_FR-siwis-medium", FRENCH_PARAGRAPH),
        ("css10", "vits-coqui-fr-css10", FRENCH_PARAGRAPH),
        (
            "ljspeech",
            "vits-piper-en_US-ljspeech-medium",
            ENGLISH_PARAGRAPH,
        ),
        (
            "libritts_r",
            "vits-piper-en_US-libritts_r-medium",
            ENGLISH_PARAGRAPH,
        ),
    ] {
        let spec = models::find(id).expect("the manifest has the voice");
        let folder = spec.folder(&root);
        copy_tree(&sources.join(source), &folder);
        models::verify(spec, &folder, &CancelToken::new())
            .unwrap_or_else(|error| panic!("{id} does not match the manifest: {}", error.code()));

        let cold = Instant::now();
        let short = provider
            .synthesize(spec, "Bonjour.", 100)
            .unwrap_or_else(|error| panic!("{id}: {}", error.code()));
        let cold_ms = cold.elapsed().as_millis();
        assert!(short.audio_ms > 200, "{id}: {} ms of audio", short.audio_ms);

        let natural = provider.synthesize(spec, paragraph, 100).expect("natural");
        let fast = provider.synthesize(spec, paragraph, 200).expect("fast");
        let slow = provider.synthesize(spec, paragraph, 50).expect("slow");
        assert!(
            fast.audio_ms < natural.audio_ms && natural.audio_ms < slow.audio_ms,
            "{id}: the speed has no effect ({} {} {})",
            fast.audio_ms,
            natural.audio_ms,
            slow.audio_ms
        );
        assert_eq!(&natural.wav[0..4], b"RIFF");
        println!(
            "measured: {id}: first sentence (worker start and voice load included) {cold_ms} ms; \
             paragraph {} ms of audio spoken in {} ms (real-time factor {:.3}); voice load {:?} ms",
            natural.audio_ms,
            natural.processing_ms,
            natural.processing_ms as f64 / natural.audio_ms as f64,
            provider.last_load_ms()
        );
    }
    assert_eq!(provider.starts(), 1, "one worker served the four voices");
    assert_eq!(
        fs::read_dir(dir.path().join("speech-temp"))
            .map(|entries| entries.flatten().count())
            .unwrap_or(0),
        0,
        "no audio file is left behind"
    );
    provider.shutdown();
}

// ---- what the files may and may not contain ---------------------------------------------------

fn source(name: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name))
        .expect("source reads")
}

#[test]
fn the_voice_client_and_the_rules_open_no_connection_and_log_nothing() {
    for name in ["speech/worker_tts.rs", "speech/tts.rs"] {
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
            "eprintln!",
            "dbg!",
            "tracing::",
            "log::",
            "#[cfg(windows)]",
            "#[cfg(not(windows))]",
            "creation_flags",
        ] {
            assert!(!text.contains(token), "{name} uses {token}");
        }
    }
}

#[test]
fn the_voice_client_starts_its_process_only_through_the_shared_starter() {
    // The shared starter silences the standard error of the worker and hides its console window; a
    // second place that spawns a process would have to do both again.
    let client = source("speech/worker_tts.rs");
    assert!(!client.contains("Command::new"));
    assert!(!client.contains("Stdio::inherit"));
    let shared = source("speech/worker_stt.rs");
    assert!(shared.contains(".stderr(Stdio::null())"));
}

#[test]
fn the_voice_command_sits_above_the_reading_preparation_and_takes_only_the_text_and_the_language() {
    let commands = source("speech_commands.rs").replace("\r\n", "\n");
    let synthesize = commands
        .find("pub async fn tts_synthesize")
        .expect("the command exists");
    let prepare = commands
        .find("pub fn speech_prepare_reading")
        .expect("the preparation exists");
    assert!(
        synthesize < prepare,
        "three scans read everything after the preparation and forbid the worker there"
    );
    let signature = &commands[synthesize
        ..commands[synthesize..]
            .find("{\n")
            .map(|end| synthesize + end)
            .expect("signature ends")];
    for forbidden in ["voice", "rate", "speed", "path"] {
        assert!(
            !signature.contains(forbidden),
            "the window sends no {forbidden}: the settings decide"
        );
    }
    assert!(signature.contains("text: String") && signature.contains("locale: String"));
    let lib = source("lib.rs");
    assert!(lib.contains("speech_commands::tts_synthesize"));
    assert!(lib.contains("speech_commands::tts_cancel"));
}

#[test]
fn the_worker_crate_builds_the_voice_binary_from_the_full_libraries_on_their_own() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../speech-worker");
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("worker manifest");
    assert!(manifest.contains("name = \"speech-tts-worker\""));
    assert!(manifest.contains("required-features = [\"tts\"]"));
    // The default build is the speech-to-text one: the GPL phonemizer is never linked by accident.
    assert!(manifest.contains("default = [\"stt\"]"));
    for relative in ["src/voice.rs", "src/bin/tts.rs"] {
        let text = fs::read_to_string(root.join(relative)).expect("worker source");
        for token in [
            "std::net",
            "TcpStream",
            "UdpSocket",
            "reqwest",
            "ureq",
            "println!",
            "eprintln!",
            "dbg!",
        ] {
            assert!(!text.contains(token), "{relative} uses {token}");
        }
    }
    assert!(
        root.join("NOTICE-TTS-WORKER.md").is_file(),
        "the voice worker ships with its own notice"
    );
}
