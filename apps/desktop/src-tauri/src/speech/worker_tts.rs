//! The text-to-speech client of the voice worker (decision D3): a separate process, started on the
//! first reading and not before, kept while it is in use, ended after an idle delay, killed when a
//! reading is cancelled.
//!
//! The process handling is the one of [`super::worker_stt`] (same protocol file, same pipes, same
//! kill on cancel); this file adds what is specific to a voice: which voice is loaded, the audio
//! file the worker writes in the private temporary folder, and the cleanup of that file as soon as
//! its bytes have been read. The text travels on the worker's standard input and nowhere else; the
//! worker's standard error goes nowhere.
//!
//! One synthesis at a time. A new request cancels the one in progress (the reader of the earlier one
//! has already moved on), and [`WorkerTtsProvider::cancel`] stops the current one.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::{Duration, Instant};

use super::models::{self, ModelSpec};
use super::status::ModelState;
use super::stt::CancelToken;
use super::temp_audio;
use super::tts::{Synthesis, MAX_TEXT_CHARS};
use super::worker_protocol::{FailureKind, Request, Response, VoiceFiles};
use super::worker_stt::{
    await_response, idle_expired, kill, poll_step, send, start, stop_gracefully, Ending, Worker,
    WorkerLaunch, IDLE_UNLOAD_SECONDS,
};
use crate::error::AppError;

/// Threads asked of the engine. A voice runs at a real-time factor of 0.045 to 0.064 on the owner's
/// laptop (SpeechLab), so four threads keep a sentence well under a second.
pub const TTS_THREADS: u32 = 4;
/// Time allowed for the first request of a worker, which reads the voice (0.8 to 1.3 s measured).
const LOAD_TIMEOUT: Duration = Duration::from_secs(30);
/// Time allowed for one sentence.
const SYNTHESIS_TIMEOUT: Duration = Duration::from_secs(60);

/// The names the voice worker may have on disk: as installed next to the application, and as staged
/// under the triple of each supported target. A list of names, not a platform test.
const WORKER_FILE_NAMES: [&str; 5] = [
    "speech-tts-worker.exe",
    "speech-tts-worker",
    "speech-tts-worker-x86_64-pc-windows-msvc.exe",
    "speech-tts-worker-aarch64-apple-darwin",
    "speech-tts-worker-x86_64-apple-darwin",
];

/// The first voice worker executable that exists under `roots`.
pub fn find_worker(roots: &[PathBuf]) -> Option<PathBuf> {
    roots
        .iter()
        .flat_map(|root| WORKER_FILE_NAMES.iter().map(move |name| root.join(name)))
        .find(|candidate| candidate.is_file())
}

/// Where the worker, the voices and the private audio folder are on this machine, found by a
/// command that has an `AppHandle` and handed to the provider, which never searches the disk itself.
#[derive(Debug, Clone)]
pub struct TtsSetup {
    pub launch: WorkerLaunch,
    pub models_root: PathBuf,
    /// The private folder where the worker writes the audio of one sentence for as long as it takes
    /// to read it back.
    pub temp_dir: PathBuf,
}

/// What the application needs on this machine: the worker found under `roots` (an empty path when
/// there is none, which reads as "not installed").
pub fn setup_for(roots: &[PathBuf], models_root: PathBuf, temp_dir: PathBuf) -> TtsSetup {
    TtsSetup {
        launch: WorkerLaunch::program(find_worker(roots).unwrap_or_default()),
        models_root,
        temp_dir,
    }
}

struct Slot {
    worker: Option<Worker>,
    /// The id of the voice the running worker has loaded.
    voice: Option<String>,
    last_used: Instant,
    generation: u64,
    last_load_ms: Option<u64>,
    last_ending: Option<Ending>,
    starts: u64,
}

pub struct WorkerTtsProvider {
    setup: Mutex<Option<TtsSetup>>,
    slot: Arc<Mutex<Slot>>,
    /// The synthesis in progress, if any: its number and its token. The slot is held for the whole
    /// of a job, so this is the one thing a cancel can reach while a job runs.
    current: Mutex<Option<(u64, CancelToken)>>,
    idle: Duration,
    threads: u32,
    next_id: AtomicU64,
}

impl Default for WorkerTtsProvider {
    fn default() -> Self {
        Self::with_timing(Duration::from_secs(IDLE_UNLOAD_SECONDS), TTS_THREADS)
    }
}

impl WorkerTtsProvider {
    pub fn with_timing(idle: Duration, threads: u32) -> Self {
        Self {
            setup: Mutex::new(None),
            slot: Arc::new(Mutex::new(Slot {
                worker: None,
                voice: None,
                last_used: Instant::now(),
                generation: 0,
                last_load_ms: None,
                last_ending: None,
                starts: 0,
            })),
            current: Mutex::new(None),
            idle,
            threads,
            next_id: AtomicU64::new(1),
        }
    }

    /// Records where the worker and the voices are. Cheap: a command calls it before it reads the
    /// status or runs a job.
    pub fn configure(&self, setup: TtsSetup) {
        if let Ok(mut current) = self.setup.lock() {
            *current = Some(setup);
        }
    }

    fn setup(&self) -> Result<TtsSetup, AppError> {
        self.setup
            .lock()
            .map_err(|_| AppError::Internal)?
            .clone()
            .ok_or(AppError::TtsNoLocalVoice)
    }

    /// Whether a voice worker executable was found.
    pub fn engine_found(&self) -> bool {
        self.setup()
            .map(|setup| setup.launch.program.is_file())
            .unwrap_or(false)
    }

    /// Whether a worker process exists right now.
    pub fn is_running(&self) -> bool {
        self.slot
            .lock()
            .map(|slot| slot.worker.is_some())
            .unwrap_or(false)
    }

    /// How the last worker ended, if one has.
    pub fn last_ending(&self) -> Option<Ending> {
        self.slot.lock().ok().and_then(|slot| slot.last_ending)
    }

    /// How long the last voice load took, in milliseconds.
    pub fn last_load_ms(&self) -> Option<u64> {
        self.slot.lock().ok().and_then(|slot| slot.last_load_ms)
    }

    /// How many workers were started since this provider was built.
    pub fn starts(&self) -> u64 {
        self.slot.lock().map(|slot| slot.starts).unwrap_or(0)
    }

    /// Whether a synthesis is in progress (or waiting for the worker).
    pub fn is_busy(&self) -> bool {
        self.current
            .lock()
            .map(|current| current.is_some())
            .unwrap_or(false)
    }

    /// Stops the synthesis in progress, if any. Returns whether there was one. The job ends with
    /// `speech_cancelled` and its worker is killed.
    pub fn cancel(&self) -> bool {
        match self.current.lock() {
            Ok(current) => match current.as_ref() {
                Some((_, token)) if !token.is_cancelled() => {
                    token.cancel();
                    true
                }
                _ => false,
            },
            Err(_) => false,
        }
    }

    /// Ends the worker, if any, and waits for it. Called before voice files are replaced: a loaded
    /// voice keeps its files open, and Windows will not replace a file that is open.
    pub fn shutdown(&self) {
        if let Ok(mut slot) = self.slot.lock() {
            if let Some(worker) = slot.worker.take() {
                slot.voice = None;
                slot.last_ending = Some(stop_gracefully(worker));
            }
        }
    }

    fn fresh_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Makes the token of a new job. A job still in progress is cancelled: only the latest request
    /// matters, and the earlier reader has already moved on.
    fn begin(&self) -> Result<(u64, CancelToken), AppError> {
        let mut current = self.current.lock().map_err(|_| AppError::Internal)?;
        if let Some((_, earlier)) = current.as_ref() {
            earlier.cancel();
        }
        let number = self.fresh_id();
        let token = CancelToken::new();
        *current = Some((number, token.clone()));
        Ok((number, token))
    }

    /// Forgets job `number`, unless a newer one has taken its place.
    fn finish(&self, number: u64) {
        if let Ok(mut current) = self.current.lock() {
            if current.as_ref().is_some_and(|(held, _)| *held == number) {
                *current = None;
            }
        }
    }

    /// Starts the thread that ends the worker of `generation` once it has been idle too long. It
    /// exists only as long as that worker does.
    fn watch(&self, generation: u64) {
        let slot = Arc::clone(&self.slot);
        let idle = self.idle;
        let step = poll_step(idle);
        std::thread::spawn(move || loop {
            std::thread::sleep(step);
            let mut guard = match slot.try_lock() {
                Ok(guard) => guard,
                // A job is running: the worker is in use, not idle.
                Err(TryLockError::WouldBlock) => continue,
                Err(TryLockError::Poisoned(_)) => return,
            };
            match &guard.worker {
                Some(worker) if worker.generation == generation => {}
                _ => return,
            }
            if idle_expired(guard.last_used, Instant::now(), idle) {
                if let Some(worker) = guard.worker.take() {
                    guard.voice = None;
                    guard.last_ending = Some(stop_gracefully(worker));
                }
                return;
            }
        });
    }

    /// Speaks `text` with `voice` at `speed_percent` (100 is natural) and returns the audio. Blocking:
    /// the command runs it off the interface thread. The voice must be `Present`; the audio file the
    /// worker writes is read back and removed before this returns, whatever the outcome.
    pub fn synthesize(
        &self,
        voice: &ModelSpec,
        text: &str,
        speed: u32,
    ) -> Result<Synthesis, AppError> {
        let setup = self.setup()?;
        let (number, cancel) = self.begin()?;
        let outcome = self.synthesize_job(&setup, voice, text, speed, &cancel);
        self.finish(number);
        outcome
    }

    fn synthesize_job(
        &self,
        setup: &TtsSetup,
        voice: &ModelSpec,
        text: &str,
        speed: u32,
        cancel: &CancelToken,
    ) -> Result<Synthesis, AppError> {
        if text.chars().count() > MAX_TEXT_CHARS {
            return Err(AppError::TtsFailed);
        }
        if !setup.launch.program.is_file() {
            return Err(AppError::TtsNoLocalVoice);
        }
        let folder = voice.folder(&setup.models_root);
        match models::state(voice, &folder) {
            ModelState::Present => {}
            ModelState::Corrupt => return Err(AppError::SpeechModelCorrupt),
            ModelState::Missing | ModelState::Unverified => return Err(AppError::TtsNoLocalVoice),
        }
        let files = voice.voice_files(&folder).ok_or(AppError::TtsFailed)?;
        let mut slot = self.slot.lock().map_err(|_| AppError::Internal)?;
        if cancel.is_cancelled() {
            return Err(AppError::SpeechCancelled);
        }
        if slot.worker.is_none() {
            slot.generation += 1;
            let generation = slot.generation;
            slot.worker = Some(start(&setup.launch, generation).map_err(tts_error)?);
            slot.voice = None;
            slot.starts += 1;
            slot.last_used = Instant::now();
            self.watch(generation);
        }
        let job = Job {
            voice,
            files,
            text,
            speed,
            load_id: self.fresh_id(),
            synth_id: self.fresh_id(),
        };
        let outcome = self.run_job(&mut slot, setup, &job, cancel);
        match &outcome {
            Ok(_) => slot.last_used = Instant::now(),
            // Anything else leaves the worker in a state nobody knows: end it, start fresh next time.
            Err(_) => {
                if let Some(worker) = slot.worker.take() {
                    slot.voice = None;
                    slot.last_ending = Some(kill(worker));
                }
            }
        }
        outcome
    }

    fn run_job(
        &self,
        slot: &mut Slot,
        setup: &TtsSetup,
        job: &Job<'_>,
        cancel: &CancelToken,
    ) -> Result<Synthesis, AppError> {
        let worker = slot.worker.as_mut().ok_or(AppError::Internal)?;
        if slot.voice.as_deref() != Some(job.voice.id.as_str()) {
            send(
                worker,
                &Request::LoadVoice {
                    id: job.load_id,
                    threads: self.threads,
                    voice: job.files.clone(),
                },
            )
            .map_err(tts_error)?;
            match await_response(worker, job.load_id, cancel, Some(LOAD_TIMEOUT))
                .map_err(tts_error)?
            {
                Response::VoiceLoaded { load_ms, .. } => {
                    worker.loaded = true;
                    slot.voice = Some(job.voice.id.clone());
                    slot.last_load_ms = Some(load_ms);
                }
                Response::Failed { kind, .. } => return Err(map_failure(kind)),
                _ => return Err(AppError::TtsFailed),
            }
        }
        // The guard removes the audio file when this function ends, whatever the outcome.
        let audio = temp_audio::reserve(&setup.temp_dir)?;
        let worker = slot.worker.as_mut().ok_or(AppError::Internal)?;
        send(
            worker,
            &Request::Synthesize {
                id: job.synth_id,
                text: job.text.to_string(),
                speed_percent: job.speed,
                wav_path: audio.path().to_string_lossy().into_owned(),
            },
        )
        .map_err(tts_error)?;
        match await_response(worker, job.synth_id, cancel, Some(SYNTHESIS_TIMEOUT))
            .map_err(tts_error)?
        {
            Response::Synthesized {
                audio_ms,
                processing_ms,
                ..
            } => {
                let wav = std::fs::read(audio.path()).map_err(|_| AppError::TtsFailed)?;
                Ok(Synthesis {
                    wav,
                    voice_id: job.voice.id.clone(),
                    audio_ms,
                    processing_ms,
                })
            }
            Response::Failed { kind, .. } => Err(map_failure(kind)),
            _ => Err(AppError::TtsFailed),
        }
    }
}

/// What one synthesis needs, gathered so the worker loop reads in order.
struct Job<'a> {
    voice: &'a ModelSpec,
    files: VoiceFiles,
    text: &'a str,
    speed: u32,
    load_id: u64,
    synth_id: u64,
}

impl Drop for WorkerTtsProvider {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.slot.lock() {
            if let Some(worker) = slot.worker.take() {
                kill(worker);
            }
        }
    }
}

/// The worker process failing or taking too long is a failed reading, not a failed dictation.
fn tts_error(error: AppError) -> AppError {
    match error {
        AppError::SpeechEngineFailed | AppError::SpeechTimeout => AppError::TtsFailed,
        other => other,
    }
}

fn map_failure(kind: FailureKind) -> AppError {
    match kind {
        FailureKind::ModelMissing => AppError::TtsNoLocalVoice,
        FailureKind::ModelCorrupt => AppError::SpeechModelCorrupt,
        FailureKind::AudioInvalid
        | FailureKind::EngineFailed
        | FailureKind::BadRequest
        | FailureKind::NothingToSay => AppError::TtsFailed,
    }
}
