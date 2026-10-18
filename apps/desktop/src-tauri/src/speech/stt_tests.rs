//! Tests of the speech-to-text port: the WAV parser, the temporary clip lifecycle, the output guard
//! and the service, driven with the scripted fake provider. Nothing here builds a Tauri `State` or
//! an `AppHandle`, needs a model, or touches the network, so all of it runs on both platforms.
//! Every signal is generated inside the test.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::fake::{FakeProvider, Step, FAKE_ENGINE_ID};
use super::output_guard;
use super::service::{self, Limits, Plan, Purpose, SpeechService};
use super::settings::SpeechSettings;
use super::status::Probes;
use super::stt::{CancelToken, SpeechToTextProvider, SttRequest, SttResult};
use super::temp_audio::{self, TempAudioFile};
use super::wav::{self, HEADER_BYTES, MIN_CLIP_MS, SAMPLE_RATE_HZ};
use crate::error::AppError;
use crate::settings::Settings;

// ---- synthetic audio -------------------------------------------------------------------------

fn samples_for(milliseconds: u64) -> usize {
    (milliseconds * u64::from(SAMPLE_RATE_HZ) / 1_000) as usize
}

fn silence(milliseconds: u64) -> Vec<i16> {
    vec![0; samples_for(milliseconds)]
}

/// A 440 Hz sine at this share of full scale (0 to 1).
fn tone(milliseconds: u64, level: f32) -> Vec<i16> {
    (0..samples_for(milliseconds))
        .map(|index| {
            let phase = index as f32 * 440.0 * std::f32::consts::TAU / SAMPLE_RATE_HZ as f32;
            (phase.sin() * level * 32_767.0) as i16
        })
        .collect()
}

/// Deterministic white noise from a small linear congruential generator.
fn noise(milliseconds: u64, level: f32) -> Vec<i16> {
    let mut state: u32 = 12_345;
    (0..samples_for(milliseconds))
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let unit = (state >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
            (unit * level * 32_767.0) as i16
        })
        .collect()
}

/// The canonical file the webview writes (`src/audio/wav.ts`, `encodeWav`).
fn encode(samples: &[i16]) -> Vec<u8> {
    let data_bytes = samples.len() * 2;
    let mut bytes = Vec::with_capacity(HEADER_BYTES + data_bytes);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data_bytes) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&SAMPLE_RATE_HZ.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE_HZ * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

fn speech_clip(milliseconds: u64) -> Vec<u8> {
    encode(&tone(milliseconds, 0.3))
}

// ---- helpers ---------------------------------------------------------------------------------

const SENTENCE: &str = "please send the report to the accountant before friday afternoon";

fn plan() -> Plan {
    Plan {
        locale: "fr-FR".into(),
        language: "fr".into(),
        cap_seconds: 120,
    }
}

fn service_with(fake: &Arc<FakeProvider>) -> SpeechService {
    SpeechService::new(Some(fake.clone()), Limits::default())
}

fn run_job(service: &SpeechService, wav: &[u8], temp_dir: &Path) -> Result<SttResult, AppError> {
    tauri::async_runtime::block_on(service.transcribe(wav, &plan(), temp_dir))
}

fn code<T: std::fmt::Debug>(outcome: Result<T, AppError>) -> String {
    outcome
        .expect_err("an error was expected")
        .code()
        .to_string()
}

fn files_in(directory: &Path) -> Vec<String> {
    match fs::read_dir(directory) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn wait_until_idle(service: &SpeechService) {
    let started = Instant::now();
    while service.is_busy() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the service never became idle"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn enabled_settings() -> Settings {
    Settings {
        locale: Some("fr-FR".into()),
        speech: SpeechSettings {
            enabled: true,
            ..SpeechSettings::default()
        },
        ..Settings::default()
    }
}

// ---- the WAV parser --------------------------------------------------------------------------

#[test]
fn the_header_matches_byte_for_byte_what_the_webview_writes() {
    // Two samples, 1 and -1: the header of `encodeWav` for 4 data bytes.
    let expected: [u8; 48] = [
        b'R', b'I', b'F', b'F', 40, 0, 0, 0, b'W', b'A', b'V', b'E', b'f', b'm', b't', b' ', 16, 0,
        0, 0, 1, 0, 1, 0, 0x80, 0x3e, 0, 0, 0, 0x7d, 0, 0, 2, 0, 16, 0, b'd', b'a', b't', b'a', 4,
        0, 0, 0, 1, 0, 0xff, 0xff,
    ];
    assert_eq!(encode(&[1, -1]), expected);
    assert_eq!(HEADER_BYTES, 44);
    assert_eq!(SAMPLE_RATE_HZ, 16_000);
}

#[test]
fn a_canonical_clip_is_read_with_its_length_and_levels() {
    let clip = wav::parse(&speech_clip(2_000)).expect("canonical clip");
    assert_eq!(clip.duration_ms, 2_000);
    assert_eq!(clip.sample_count, 32_000);
    assert!((clip.peak - 0.3).abs() < 0.01, "peak {}", clip.peak);
    assert!(
        (clip.rms - 0.3 / 2f32.sqrt()).abs() < 0.01,
        "rms {}",
        clip.rms
    );
    assert!(!clip.is_quiet());

    let empty = wav::parse(&encode(&[])).expect("an empty clip is a valid file");
    assert_eq!(empty.duration_ms, 0);
    assert_eq!(empty.peak, 0.0);
}

#[test]
fn anything_but_the_canonical_file_is_invalid() {
    let good = speech_clip(500);
    assert!(wav::parse(&good).is_ok());

    // Each field of the header, broken one at a time.
    let breaks: [(&str, usize, u8); 12] = [
        ("riff tag", 0, b'X'),
        ("riff size", 4, 0xff),
        ("wave tag", 8, b'X'),
        ("fmt tag", 12, b'X'),
        ("fmt size", 16, 18),
        ("not pcm", 20, 3),
        ("stereo", 22, 2),
        ("sample rate", 24, 0x44),
        ("byte rate", 28, 1),
        ("bits", 34, 24),
        ("data tag", 36, b'X'),
        ("data size", 40, 0xff),
    ];
    for (what, offset, value) in breaks {
        let mut bytes = good.clone();
        bytes[offset] = value;
        assert_eq!(code(wav::parse(&bytes)), "speech_audio_invalid", "{what}");
    }

    assert_eq!(code(wav::parse(&[])), "speech_audio_invalid");
    assert_eq!(code(wav::parse(&good[..43])), "speech_audio_invalid");
    assert_eq!(
        code(wav::parse(&good[..good.len() - 1])),
        "speech_audio_invalid"
    );
    let mut padded = good.clone();
    padded.push(0);
    assert_eq!(code(wav::parse(&padded)), "speech_audio_invalid");
    assert_eq!(
        code(wav::parse(b"not audio at all, only text")),
        "speech_audio_invalid"
    );
}

#[test]
fn a_clip_is_judged_from_its_samples_in_the_order_the_user_can_act_on() {
    let too_long = wav::parse(&speech_clip(11_000)).expect("clip");
    assert_eq!(
        code(wav::check_usable(&too_long, 10)),
        "speech_audio_too_long"
    );
    assert!(
        wav::check_usable(&too_long, 11).is_ok(),
        "the cap itself is allowed"
    );

    let click = wav::parse(&speech_clip(MIN_CLIP_MS - 50)).expect("clip");
    assert_eq!(
        code(wav::check_usable(&click, 10)),
        "speech_audio_too_short"
    );

    let hush = wav::parse(&encode(&tone(1_000, 0.004))).expect("clip");
    assert_eq!(code(wav::check_usable(&hush, 10)), "speech_audio_too_quiet");

    let nothing = wav::parse(&encode(&silence(1_000))).expect("clip");
    assert!(nothing.is_quiet());

    // A short and silent clip is "too short" first; a long and silent one is "too long" first.
    let short_silence = wav::parse(&encode(&silence(100))).expect("clip");
    assert_eq!(
        code(wav::check_usable(&short_silence, 10)),
        "speech_audio_too_short"
    );
    let long_silence = wav::parse(&encode(&silence(11_000))).expect("clip");
    assert_eq!(
        code(wav::check_usable(&long_silence, 10)),
        "speech_audio_too_long"
    );

    let noisy = wav::parse(&encode(&noise(1_000, 0.3))).expect("clip");
    assert!(
        !noisy.is_quiet(),
        "noise is loud, the output guard judges what comes of it"
    );
}

// ---- the temporary clip ----------------------------------------------------------------------

#[test]
fn a_clip_file_is_new_private_and_removed_when_dropped() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join(temp_audio::TEMP_FOLDER_NAME);
    assert!(
        !directory.exists(),
        "nothing is created before a job needs it"
    );

    let first = temp_audio::write(&directory, b"one").expect("write");
    let second = temp_audio::write(&directory, b"two").expect("write");
    assert_ne!(first.path(), second.path(), "two clips never share a name");
    assert_eq!(fs::read(first.path()).expect("read"), b"one");
    assert!(first.path().starts_with(&directory));
    assert_eq!(files_in(&directory).len(), 2);

    let gone: PathBuf = first.path().to_path_buf();
    drop(first);
    assert!(!gone.exists());
    assert_eq!(files_in(&directory).len(), 1);
    drop::<TempAudioFile>(second);
    assert!(files_in(&directory).is_empty());
}

#[test]
fn the_sweep_removes_only_clips_of_this_one_folder() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join("speech-temp");
    fs::create_dir_all(directory.join("inner")).expect("folders");
    fs::write(directory.join("clip-1-0.wav"), b"stale").expect("write");
    fs::write(directory.join("clip-9-9.wav"), b"stale").expect("write");
    fs::write(directory.join("keep.txt"), b"mine").expect("write");
    fs::write(directory.join("clip-no-extension"), b"mine").expect("write");
    fs::write(directory.join("inner").join("clip-1-1.wav"), b"deeper").expect("write");
    let outside = root.path().join("clip-1-2.wav");
    fs::write(&outside, b"beside").expect("write");

    assert_eq!(temp_audio::sweep(&directory), 2);

    let mut left = files_in(&directory);
    left.sort();
    assert_eq!(left, ["clip-no-extension", "inner", "keep.txt"]);
    assert!(
        directory.join("inner").join("clip-1-1.wav").exists(),
        "one level only"
    );
    assert!(outside.exists(), "nothing outside the folder is listed");
    assert_eq!(temp_audio::sweep(&root.path().join("missing")), 0);
}

#[test]
fn the_sweep_runs_lazily_at_the_first_job_and_only_then() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join("speech-temp");
    fs::create_dir_all(&directory).expect("folder");
    fs::write(directory.join("clip-1-0.wav"), b"left by a crash").expect("write");

    let fake = Arc::new(FakeProvider::saying(SENTENCE));
    let service = service_with(&fake);
    assert_eq!(
        files_in(&directory),
        ["clip-1-0.wav"],
        "building the service sweeps nothing"
    );

    run_job(&service, &speech_clip(2_000), &directory).expect("job");
    assert!(
        files_in(&directory).is_empty(),
        "the first job swept the stale clip"
    );

    fs::write(directory.join("clip-1-1.wav"), b"later").expect("write");
    run_job(&service, &speech_clip(2_000), &directory).expect("job");
    assert_eq!(
        files_in(&directory),
        ["clip-1-1.wav"],
        "the sweep runs once per run"
    );
}

// ---- the service -----------------------------------------------------------------------------

#[test]
fn a_job_hands_the_engine_the_language_the_cap_and_a_clip_that_exists_then_removes_it() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join("speech-temp");
    let fake = Arc::new(FakeProvider::saying(SENTENCE));
    let service = service_with(&fake);

    let result = run_job(&service, &speech_clip(3_000), &directory).expect("job");

    assert_eq!(result.text, SENTENCE);
    assert_eq!(result.engine_id, FAKE_ENGINE_ID);
    assert_eq!(result.audio_ms, 3_000);
    let seen = fake.seen();
    assert_eq!(seen.len(), 1);
    assert!(
        seen[0].wav_existed,
        "the clip is on disk while the engine runs"
    );
    assert_eq!(seen[0].request.language, "fr");
    assert_eq!(seen[0].request.max_seconds, 120);
    assert!(seen[0].request.wav_path.starts_with(&directory));
    assert!(files_in(&directory).is_empty(), "audio is never kept");
    assert!(!service.is_busy());
}

#[test]
fn the_clip_is_removed_after_an_engine_failure_a_rejected_output_and_a_panic() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join("speech-temp");
    let fake = Arc::new(FakeProvider::saying(SENTENCE).then([
        Step::Fail,
        Step::Text("   ".into()),
        Step::Panic,
    ]));
    let service = service_with(&fake);
    let clip = speech_clip(2_000);

    assert_eq!(
        code(run_job(&service, &clip, &directory)),
        "speech_engine_failed"
    );
    assert!(files_in(&directory).is_empty());
    assert!(!service.is_busy());

    assert_eq!(
        code(run_job(&service, &clip, &directory)),
        "speech_output_rejected"
    );
    assert!(files_in(&directory).is_empty());
    assert!(!service.is_busy());

    assert_eq!(code(run_job(&service, &clip, &directory)), "internal_error");
    wait_until_idle(&service);
    assert!(files_in(&directory).is_empty(), "a panic leaves no clip");

    assert!(
        run_job(&service, &clip, &directory).is_ok(),
        "and the service still works"
    );
    assert_eq!(fake.calls(), 4);
}

#[test]
fn a_clip_the_samples_refuse_never_reaches_the_engine_or_the_disk() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join("speech-temp");
    let fake = Arc::new(FakeProvider::saying(SENTENCE));
    let service = service_with(&fake);

    let refused = [
        (b"garbage".to_vec(), "speech_audio_invalid"),
        (encode(&silence(2_000)), "speech_audio_too_quiet"),
        (encode(&tone(2_000, 0.003)), "speech_audio_too_quiet"),
        (speech_clip(100), "speech_audio_too_short"),
        (speech_clip(125_000), "speech_audio_too_long"),
    ];
    for (bytes, expected) in refused {
        assert_eq!(code(run_job(&service, &bytes, &directory)), expected);
        assert!(!service.is_busy());
    }

    assert_eq!(fake.calls(), 0, "the engine heard none of them");
    assert!(files_in(&directory).is_empty(), "and none was written");
}

#[test]
fn the_output_guard_judges_what_the_engine_heard_in_noise() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join("speech-temp");
    let fake = Arc::new(FakeProvider::saying("ok ok ok ok ok ok ok ok ok ok ok ok"));
    let service = service_with(&fake);

    // The engine loops on one word over noise: rejected, never shown.
    let looping = encode(&noise(4_000, 0.3));
    assert_eq!(
        code(run_job(&service, &looping, &directory)),
        "speech_output_rejected"
    );

    // The same noise with a plausible sentence passes: the guard judges the text, not the signal.
    let sensible = Arc::new(FakeProvider::saying(SENTENCE));
    let service = service_with(&sensible);
    assert_eq!(
        run_job(&service, &looping, &directory).expect("job").text,
        SENTENCE
    );
}

#[test]
fn the_guard_rejects_empty_runaway_too_fast_too_slow_and_quiet_outputs() {
    let ok = |text: &str, audio_ms: u64| output_guard::check(text, audio_ms, false);
    let rejected = |text: &str, audio_ms: u64| {
        assert_eq!(
            code(output_guard::check(text, audio_ms, false)),
            "speech_output_rejected",
            "{text:?} over {audio_ms} ms"
        );
    };

    assert!(ok(SENTENCE, 3_000).is_ok());
    assert!(ok("yes", 800).is_ok(), "one word is an answer");
    assert!(ok("no no", 1_500).is_ok(), "short repetition is not a loop");

    rejected("", 3_000);
    rejected("   \n\t ", 3_000);
    rejected("... --- !!!", 3_000);
    rejected(SENTENCE, 0);
    // One word is more than half of the output.
    rejected("the the the the the the the the the cat", 4_000);
    // One phrase repeated back to back.
    rejected("thank you for watching thank you for watching thank you for watching thank you for watching", 12_000);
    // More words per second than a person can say.
    rejected(SENTENCE, 500);
    // Almost no words over a long clip.
    rejected("yes", 60_000);

    assert_eq!(
        code(output_guard::check(SENTENCE, 3_000, true)),
        "speech_output_rejected",
        "any text for audio flagged too quiet is invented"
    );
}

#[test]
fn only_one_job_runs_at_a_time_and_the_flag_is_released_after_it() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join("speech-temp");
    let fake = Arc::new(FakeProvider::saying(SENTENCE).after(Duration::from_millis(300)));
    let service = service_with(&fake);
    let clip = speech_clip(2_000);
    let plan = plan();

    let (first, second) = tauri::async_runtime::block_on(async {
        tokio::join!(service.transcribe(&clip, &plan, &directory), async {
            tokio::time::sleep(Duration::from_millis(60)).await;
            service.transcribe(&clip, &plan, &directory).await
        })
    });

    assert_eq!(first.expect("the first job finishes").text, SENTENCE);
    assert_eq!(code(second), "speech_busy");
    assert_eq!(fake.calls(), 1, "the refused job never reached the engine");
    assert!(!service.is_busy());
    assert!(run_job(&service, &clip, &directory).is_ok());
}

#[test]
fn cancel_stops_the_running_job_and_does_nothing_when_idle() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join("speech-temp");
    let fake = Arc::new(FakeProvider::saying(SENTENCE).after(Duration::from_secs(10)));
    let service = service_with(&fake);
    let clip = speech_clip(2_000);

    assert!(!service.cancel(), "nothing to cancel at rest");
    assert!(!service.is_busy());

    let plan = plan();
    let started = Instant::now();
    let (outcome, cancelled) = tauri::async_runtime::block_on(async {
        tokio::join!(service.transcribe(&clip, &plan, &directory), async {
            tokio::time::sleep(Duration::from_millis(60)).await;
            service.cancel()
        })
    });

    assert!(cancelled, "there was a job to cancel");
    assert_eq!(code(outcome), "speech_cancelled");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "cancel does not wait for the delay"
    );
    wait_until_idle(&service);
    assert!(files_in(&directory).is_empty());
    assert!(!service.cancel(), "the token of a finished job is gone");
}

/// An engine that answers, but only after the user pressed stop.
struct AnswersAfterStop;

impl SpeechToTextProvider for AnswersAfterStop {
    fn engine_id(&self) -> &str {
        "late"
    }

    fn availability(&self, _locale: &str) -> Result<(), AppError> {
        Ok(())
    }

    fn transcribe(
        &self,
        _request: &SttRequest,
        cancel: &CancelToken,
    ) -> Result<SttResult, AppError> {
        cancel.cancel();
        Ok(SttResult {
            text: SENTENCE.into(),
            audio_ms: 2_000,
            processing_ms: 1,
            engine_id: "late".into(),
        })
    }
}

#[test]
fn a_cancel_that_arrives_after_the_text_still_discards_it() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join("speech-temp");
    let service = SpeechService::new(Some(Arc::new(AnswersAfterStop)), Limits::default());

    assert_eq!(
        code(run_job(&service, &speech_clip(2_000), &directory)),
        "speech_cancelled",
        "the text never reaches the box once the user stopped"
    );
    assert!(files_in(&directory).is_empty());
    assert!(!service.is_busy());
}

#[test]
fn a_job_over_its_time_limit_times_out_and_cleans_up() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join("speech-temp");
    let fake = Arc::new(FakeProvider::saying(SENTENCE).after(Duration::from_secs(10)));
    let limits = Limits {
        timeout_floor: Duration::from_millis(80),
        timeout_per_audio_second: 0.0,
    };
    let service = SpeechService::new(Some(fake.clone()), limits);

    let started = Instant::now();
    let outcome = run_job(&service, &speech_clip(2_000), &directory);

    assert_eq!(code(outcome), "speech_timeout");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the limit is enforced, not the delay"
    );
    // The engine is told to stop; the clip and the flag go when it really has.
    wait_until_idle(&service);
    assert!(files_in(&directory).is_empty());
    assert_eq!(fake.calls(), 1);
}

#[test]
fn the_time_limit_grows_with_the_clip_and_never_drops_under_the_floor() {
    let limits = Limits::default();
    let floor = Duration::from_secs(service::TIMEOUT_FLOOR_SECONDS);
    assert_eq!(limits.timeout_for(0), floor);
    assert_eq!(limits.timeout_for(1_000), floor + Duration::from_secs(1));
    assert!(limits.timeout_for(60_000) > limits.timeout_for(10_000));
    assert!(limits.timeout_for(10_000) > floor);
}

#[test]
fn without_a_provider_the_service_answers_engine_missing_and_writes_nothing() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join("speech-temp");
    let service = SpeechService::default();

    assert_eq!(
        code(run_job(&service, &speech_clip(2_000), &directory)),
        "speech_engine_missing"
    );
    assert!(!directory.exists(), "no clip was written");
    assert!(!service.is_busy());
}

#[test]
fn an_engine_that_is_not_ready_is_asked_before_anything_is_written() {
    let root = tempfile::tempdir().expect("temp");
    let directory = root.path().join("speech-temp");
    let fake =
        Arc::new(FakeProvider::saying(SENTENCE).unavailable(|| AppError::SpeechModelMissing));
    let service = service_with(&fake);

    assert_eq!(
        code(run_job(&service, &speech_clip(2_000), &directory)),
        "speech_model_missing"
    );
    assert_eq!(fake.calls(), 0);
    assert!(!directory.exists());
    assert!(!service.is_busy());
}

// ---- the plan: switches, language and cap ----------------------------------------------------

#[test]
fn the_plan_follows_the_switches_the_language_table_and_the_use() {
    // Voice is off by default.
    assert_eq!(
        code(service::plan(&Settings::default(), Purpose::Dictation)),
        "speech_unavailable"
    );
    let mut dictation_off = enabled_settings();
    dictation_off.speech.dictation_enabled = false;
    assert_eq!(
        code(service::plan(&dictation_off, Purpose::Dictation)),
        "speech_unavailable"
    );

    let settings = enabled_settings();
    let chat = service::plan(&settings, Purpose::Dictation).expect("plan");
    assert_eq!(chat.locale, "fr-FR");
    assert_eq!(chat.language, "fr");
    assert_eq!(chat.cap_seconds, settings.speech.max_dictation_seconds);
    let note = service::plan(&settings, Purpose::Note).expect("plan");
    assert_eq!(note.cap_seconds, settings.speech.max_note_seconds);
    assert!(note.cap_seconds > chat.cap_seconds);

    let mut english = enabled_settings();
    english.speech.language_override = Some("en-US".into());
    let plan = service::plan(&english, Purpose::Dictation).expect("plan");
    assert_eq!(
        (plan.locale.as_str(), plan.language.as_str()),
        ("en-US", "en")
    );

    let mut german = enabled_settings();
    german.speech.language_override = Some("de-DE".into());
    assert_eq!(
        code(service::plan(&german, Purpose::Dictation)),
        "speech_locale_unsupported"
    );
}

#[test]
fn the_use_travels_in_a_header_and_defaults_to_the_short_cap() {
    assert_eq!(Purpose::from_header(None), Purpose::Dictation);
    assert_eq!(Purpose::from_header(Some("dictation")), Purpose::Dictation);
    assert_eq!(Purpose::from_header(Some("note")), Purpose::Note);
    assert_eq!(Purpose::from_header(Some(" note ")), Purpose::Note);
    assert_eq!(Purpose::from_header(Some("120")), Purpose::Dictation);
    assert_eq!(service::PURPOSE_HEADER, "purpose");
}

// ---- what the module may do ------------------------------------------------------------------

fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(relative);
    fs::read_to_string(path).expect("source reads")
}

#[test]
fn a_timing_line_names_the_engine_and_the_numbers_but_never_the_text() {
    let result = SttResult {
        text: "a secret sentence of six words".into(),
        audio_ms: 4_200,
        processing_ms: 610,
        engine_id: "engine-x".into(),
    };
    let line = service::timing_line(&result);

    assert_eq!(
        line,
        "speech: engine=engine-x audio_ms=4200 processing_ms=610 words=6"
    );
    assert!(!line.contains("secret"));
    assert!(!line.contains("sentence"));
}

#[test]
fn the_port_the_guard_and_the_parser_open_no_connection_process_or_thread() {
    const FORBIDDEN: [&str; 12] = [
        "reqwest",
        "std::net",
        "TcpStream",
        "UdpSocket",
        "std::process",
        "Command::new",
        "std::fs",
        "std::thread",
        "tokio::spawn",
        "tauri::async_runtime",
        "tauri::",
        "println!",
    ];
    for name in ["speech/stt.rs", "speech/output_guard.rs", "speech/wav.rs"] {
        let text = source(name);
        for token in FORBIDDEN {
            assert!(!text.contains(token), "{name} uses {token}");
        }
    }
}

#[test]
fn no_http_client_exists_in_the_speech_module() {
    // The model download of phase 3b is the one network file of the module, and it is named by the
    // phase that adds it. Until then no file of the module may even name a client.
    let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("speech");
    let mut scanned = 0;
    for entry in fs::read_dir(&folder).expect("folder reads").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".rs") || name.ends_with("tests.rs") {
            continue;
        }
        let text = fs::read_to_string(entry.path()).expect("source reads");
        for token in [
            "reqwest",
            "ureq",
            "hyper",
            "isahc",
            "surf::",
            "std::net",
            "TcpStream",
            "UdpSocket",
            "TcpListener",
        ] {
            assert!(!text.contains(token), "{name} uses {token}");
        }
        scanned += 1;
    }
    assert!(scanned >= 10, "the scan found {scanned} files");
    assert!(!source("speech_commands.rs").contains("reqwest"));
}

#[test]
fn only_the_tests_build_the_fake_and_the_application_has_no_engine_yet() {
    let commands = source("speech_commands.rs");
    for token in ["fake", "Fake", "SpeechService::new", "Some(Arc"] {
        assert!(!commands.contains(token), "speech_commands.rs uses {token}");
    }
    let module = source("speech/mod.rs").replace("\r\n", "\n");
    assert!(module.contains("#[cfg(test)]\nmod fake;"));

    let state = crate::speech_commands::SpeechState::new();
    assert_eq!(state.probes().expect("lock"), Probes::default());
    assert!(!state.service().is_busy());
    let root = tempfile::tempdir().expect("temp");
    assert_eq!(
        code(tauri::async_runtime::block_on(state.service().transcribe(
            &speech_clip(2_000),
            &plan(),
            &root.path().join("speech-temp"),
        ))),
        "speech_engine_missing"
    );
}

#[test]
fn the_commands_are_registered_and_read_a_raw_body_without_a_setup_hook() {
    let lib = source("lib.rs");
    assert!(!lib.contains(".setup("), "a setup hook would run at launch");
    assert!(lib.contains("speech_commands::speech_transcribe"));
    assert!(lib.contains("speech_commands::speech_cancel"));
    assert!(lib.contains(".manage(speech_commands::SpeechState::new())"));

    let commands = source("speech_commands.rs");
    assert!(
        commands.contains("InvokeBody::Raw"),
        "the clip is a raw body, not JSON"
    );
    assert!(commands.contains("PURPOSE_HEADER"));
    assert!(commands.contains("temp_audio::TEMP_FOLDER_NAME"));
    assert!(
        commands.contains("app_local_data_dir"),
        "temporary audio lives under the local data directory, never in the work folder"
    );
}
