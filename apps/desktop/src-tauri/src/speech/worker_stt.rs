//! The speech-to-text provider that talks to the worker executable (decision D2): a separate
//! process, started on the first dictation and not before, kept while it is in use, ended after an
//! idle delay, killed when a job is cancelled.
//!
//! The protocol lives in [`super::worker_protocol`] and is shared by source with the worker crate.
//! This file is the client: it starts the process (through [`super::console_window`], which owns
//! the platform part), sends one line per request, reads one line per answer on a reader thread, and
//! polls the cancel token while it waits. Threads exist only while a worker does, and a worker
//! exists only after the user asked for a transcription.
//!
//! Cancelling is killing. A decode cannot be interrupted from inside the engine, so the process is
//! killed and started again on the next use, which costs the cold load (about two seconds). A worker
//! dies with the application because its standard input is a pipe from this process: when the
//! application ends, the pipe closes and the worker's loop ends (`serve`).

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, TryLockError};
use std::time::{Duration, Instant};

use super::console_window;
use super::languages::{self, Feature};
use super::models::{self, ModelSpec};
use super::status::ModelState;
use super::stt::{CancelToken, SpeechToTextProvider, SttRequest, SttResult};
use super::worker_protocol::{self as protocol, FailureKind, Request, Response};
use crate::error::AppError;

/// Threads asked of the engine. Measured on the owner's laptop (SpeechLab, 4 requested): the engine
/// busies 7 to 10 cores anyway, so the machine is loaded during a decode, an Analyse included. The
/// process split keeps the interface responsive, not the machine idle.
pub const STT_THREADS: u32 = 4;
/// How long a worker may sit unused before it is ended and its memory given back.
pub const IDLE_UNLOAD_SECONDS: u64 = 120;
/// Cold load of the model, in seconds (SpeechLab, Core 7 150U). The time allowed for it is a
/// multiple of this, because a slower disk or an antivirus scan must not read as a failure.
pub const COLD_LOAD_SECONDS: u64 = 2;
/// Resident memory of a loaded worker, in megabytes, lowest and highest (SpeechLab, Core 7 150U). The
/// worker process alone measured lower on 18 Oct 2026: 733 MB after the load, 857 MB peak.
pub const RESIDENT_MEMORY_MB: (u32, u32) = (1_600, 1_900);
/// Cores a decode keeps busy although `STT_THREADS` are requested, lowest and highest (SpeechLab).
pub const BUSY_CORES: (u32, u32) = (7, 10);

/// Time allowed for the first request of a worker, which loads the model.
const LOAD_TIMEOUT: Duration = Duration::from_secs(COLD_LOAD_SECONDS * 15);
/// How often a waiting job looks at the cancel token, in milliseconds.
const CANCEL_POLL: Duration = Duration::from_millis(20);
/// How long a worker is given to end by itself before it is killed.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// The names the worker may have on disk: as installed next to the application, and as staged under
/// the triple of each supported target. A list of names, not a platform test.
const WORKER_FILE_NAMES: [&str; 5] = [
    "speech-stt-worker.exe",
    "speech-stt-worker",
    "speech-stt-worker-x86_64-pc-windows-msvc.exe",
    "speech-stt-worker-aarch64-apple-darwin",
    "speech-stt-worker-x86_64-apple-darwin",
];

/// Every place the worker may be, in search order, for the roots the sidecar discovery returns.
pub fn worker_candidates(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .flat_map(|root| WORKER_FILE_NAMES.iter().map(move |name| root.join(name)))
        .collect()
}

/// The first worker executable that exists under `roots`.
pub fn find_worker(roots: &[PathBuf]) -> Option<PathBuf> {
    worker_candidates(roots)
        .into_iter()
        .find(|candidate| candidate.is_file())
}

/// How to start a worker: the program, its arguments and its environment. The tests start the test
/// executable itself as a scripted worker, which is why this is more than a path.
#[derive(Debug, Clone)]
pub struct WorkerLaunch {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub envs: Vec<(String, String)>,
}

impl WorkerLaunch {
    pub fn program(path: impl Into<PathBuf>) -> Self {
        Self {
            program: path.into(),
            args: Vec::new(),
            envs: Vec::new(),
        }
    }
}

/// Where the worker and the model are on this machine, found by a command that has an `AppHandle`
/// and handed to the provider, which never searches the disk itself.
#[derive(Debug, Clone)]
pub struct Setup {
    pub launch: WorkerLaunch,
    pub model_folder: PathBuf,
}

/// How a worker last ended, for the tests and for the measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// Told to shut down and ended by itself.
    Graceful,
    /// Killed: a cancel, a timeout, a failure or the provider being dropped.
    Killed,
}

struct Worker {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<String>,
    loaded: bool,
    generation: u64,
}

struct Slot {
    worker: Option<Worker>,
    last_used: Instant,
    generation: u64,
    last_load_ms: Option<u64>,
    last_ending: Option<Ending>,
    starts: u64,
}

/// Whether a worker unused since `last_used` has been idle long enough to end.
pub fn idle_expired(last_used: Instant, now: Instant, idle: Duration) -> bool {
    now.saturating_duration_since(last_used) >= idle
}

/// How often the idle watcher looks: often enough to end a worker soon after the delay, rarely
/// enough to cost nothing.
fn poll_step(idle: Duration) -> Duration {
    (idle / 4).clamp(Duration::from_millis(10), Duration::from_secs(1))
}

pub struct WorkerSttProvider {
    spec: ModelSpec,
    setup: Mutex<Option<Setup>>,
    slot: Arc<Mutex<Slot>>,
    idle: Duration,
    threads: u32,
    next_id: AtomicU64,
}

impl WorkerSttProvider {
    /// A provider for `spec`, with no worker found yet and nothing started.
    pub fn new(spec: ModelSpec) -> Self {
        Self::with_timing(spec, Duration::from_secs(IDLE_UNLOAD_SECONDS), STT_THREADS)
    }

    pub fn with_timing(spec: ModelSpec, idle: Duration, threads: u32) -> Self {
        Self {
            spec,
            setup: Mutex::new(None),
            slot: Arc::new(Mutex::new(Slot {
                worker: None,
                last_used: Instant::now(),
                generation: 0,
                last_load_ms: None,
                last_ending: None,
                starts: 0,
            })),
            idle,
            threads,
            next_id: AtomicU64::new(1),
        }
    }

    /// Records where the worker and the model are. Cheap: a command calls it before it reads the
    /// status or runs a job.
    pub fn configure(&self, setup: Setup) {
        if let Ok(mut current) = self.setup.lock() {
            *current = Some(setup);
        }
    }

    fn setup(&self) -> Result<Setup, AppError> {
        self.setup
            .lock()
            .map_err(|_| AppError::Internal)?
            .clone()
            .ok_or(AppError::SpeechEngineMissing)
    }

    /// Whether a worker executable was found.
    pub fn engine_found(&self) -> bool {
        self.setup()
            .map(|setup| setup.launch.program.is_file())
            .unwrap_or(false)
    }

    /// The state of the model files, without hashing.
    pub fn model_state(&self) -> ModelState {
        match self.setup() {
            Ok(setup) => models::state(&self.spec, &setup.model_folder),
            Err(_) => ModelState::Missing,
        }
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

    /// How long the last model load took, in milliseconds.
    pub fn last_load_ms(&self) -> Option<u64> {
        self.slot.lock().ok().and_then(|slot| slot.last_load_ms)
    }

    /// How many workers were started since this provider was built.
    pub fn starts(&self) -> u64 {
        self.slot.lock().map(|slot| slot.starts).unwrap_or(0)
    }

    /// Ends the worker, if any, and waits for it. Called before the model files are replaced: a
    /// loaded model keeps its files open, and Windows will not replace a file that is open.
    pub fn shutdown(&self) {
        if let Ok(mut slot) = self.slot.lock() {
            if let Some(worker) = slot.worker.take() {
                slot.last_ending = Some(stop_gracefully(worker));
            }
        }
    }

    fn fresh_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
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
                    guard.last_ending = Some(stop_gracefully(worker));
                }
                return;
            }
        });
    }

    fn run_job(
        &self,
        slot: &mut Slot,
        setup: &Setup,
        request: &SttRequest,
        cancel: &CancelToken,
    ) -> Result<SttResult, AppError> {
        let files = self
            .spec
            .worker_files(&setup.model_folder)
            .ok_or(AppError::SpeechEngineFailed)?;
        let load_id = self.fresh_id();
        let transcribe_id = self.fresh_id();
        let threads = self.threads;
        let worker = slot.worker.as_mut().ok_or(AppError::Internal)?;

        if !worker.loaded {
            send(
                worker,
                &Request::Load {
                    id: load_id,
                    threads,
                    files,
                },
            )?;
            match await_response(worker, load_id, cancel, Some(LOAD_TIMEOUT))? {
                Response::Loaded { load_ms, .. } => {
                    worker.loaded = true;
                    slot.last_load_ms = Some(load_ms);
                }
                Response::Failed { kind, .. } => return Err(map_failure(kind)),
                Response::Transcribed { .. } => return Err(AppError::SpeechEngineFailed),
            }
        }

        let worker = slot.worker.as_mut().ok_or(AppError::Internal)?;
        send(
            worker,
            &Request::Transcribe {
                id: transcribe_id,
                wav_path: request.wav_path.to_string_lossy().into_owned(),
                language: request.language.clone(),
            },
        )?;
        match await_response(worker, transcribe_id, cancel, None)? {
            Response::Transcribed {
                text,
                audio_ms,
                processing_ms,
                ..
            } => Ok(SttResult {
                text,
                audio_ms,
                processing_ms,
                engine_id: self.spec.id.clone(),
            }),
            Response::Failed { kind, .. } => Err(map_failure(kind)),
            Response::Loaded { .. } => Err(AppError::SpeechEngineFailed),
        }
    }
}

impl SpeechToTextProvider for WorkerSttProvider {
    fn engine_id(&self) -> &str {
        &self.spec.id
    }

    fn availability(&self, locale: &str) -> Result<(), AppError> {
        languages::resolve(locale, Feature::Dictation)?;
        let setup = self.setup()?;
        if !setup.launch.program.is_file() {
            return Err(AppError::SpeechEngineMissing);
        }
        match models::state(&self.spec, &setup.model_folder) {
            ModelState::Present => Ok(()),
            ModelState::Missing | ModelState::Unverified => Err(AppError::SpeechModelMissing),
            ModelState::Corrupt => Err(AppError::SpeechModelCorrupt),
        }
    }

    fn transcribe(
        &self,
        request: &SttRequest,
        cancel: &CancelToken,
    ) -> Result<SttResult, AppError> {
        let setup = self.setup()?;
        let mut slot = self.slot.lock().map_err(|_| AppError::Internal)?;
        if cancel.is_cancelled() {
            return Err(AppError::SpeechCancelled);
        }
        if slot.worker.is_none() {
            slot.generation += 1;
            let generation = slot.generation;
            slot.worker = Some(start(&setup.launch, generation)?);
            slot.starts += 1;
            slot.last_used = Instant::now();
            self.watch(generation);
        }
        let outcome = self.run_job(&mut slot, &setup, request, cancel);
        match &outcome {
            Ok(_) => slot.last_used = Instant::now(),
            // The clip was refused but the worker is fine and still loaded.
            Err(AppError::SpeechAudioInvalid) => slot.last_used = Instant::now(),
            // Anything else leaves the worker in a state nobody knows: end it, start fresh next time.
            Err(_) => {
                if let Some(worker) = slot.worker.take() {
                    slot.last_ending = Some(kill(worker));
                }
            }
        }
        outcome
    }
}

impl Drop for WorkerSttProvider {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.slot.lock() {
            if let Some(worker) = slot.worker.take() {
                kill(worker);
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // Nothing may outlive its owner: a worker that was not stopped on purpose is killed here.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn map_failure(kind: FailureKind) -> AppError {
    match kind {
        FailureKind::ModelMissing => AppError::SpeechModelMissing,
        FailureKind::ModelCorrupt => AppError::SpeechModelCorrupt,
        FailureKind::AudioInvalid => AppError::SpeechAudioInvalid,
        FailureKind::EngineFailed | FailureKind::BadRequest => AppError::SpeechEngineFailed,
    }
}

/// Starts the process and its reader thread. The worker's standard error goes nowhere: what a worker
/// prints must never reach a log, because it could be what was heard.
fn start(launch: &WorkerLaunch, generation: u64) -> Result<Worker, AppError> {
    let mut command = Command::new(&launch.program);
    command
        .args(&launch.args)
        .envs(launch.envs.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    console_window::hide(&mut command);
    let mut child = command.spawn().map_err(|_| AppError::SpeechEngineFailed)?;
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(AppError::SpeechEngineFailed);
    };
    let (sender, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { return };
            if sender.send(line).is_err() {
                return;
            }
        }
    });
    Ok(Worker {
        child,
        stdin: Some(stdin),
        lines,
        loaded: false,
        generation,
    })
}

fn send(worker: &mut Worker, request: &Request) -> Result<(), AppError> {
    let stdin = worker.stdin.as_mut().ok_or(AppError::SpeechEngineFailed)?;
    writeln!(stdin, "{}", protocol::encode_request(request))
        .and_then(|()| stdin.flush())
        .map_err(|_| AppError::SpeechEngineFailed)
}

/// Waits for the answer to request `id`. Lines that are not answers, and answers to other requests,
/// are skipped. Returns `speech_cancelled` as soon as the token is set, `speech_timeout` when a
/// deadline passes, `speech_engine_failed` if the process ended.
fn await_response(
    worker: &mut Worker,
    id: u64,
    cancel: &CancelToken,
    limit: Option<Duration>,
) -> Result<Response, AppError> {
    let started = Instant::now();
    loop {
        if cancel.is_cancelled() {
            return Err(AppError::SpeechCancelled);
        }
        if limit.is_some_and(|limit| started.elapsed() >= limit) {
            return Err(AppError::SpeechTimeout);
        }
        match worker.lines.recv_timeout(CANCEL_POLL) {
            Ok(line) => {
                if let Some(response) = protocol::decode_response(&line) {
                    if response.id() == id {
                        return Ok(response);
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err(AppError::SpeechEngineFailed),
        }
    }
}

/// Kills the process and waits for it. Returns how it ended.
fn kill(mut worker: Worker) -> Ending {
    worker.stdin = None;
    let _ = worker.child.kill();
    let _ = worker.child.wait();
    Ending::Killed
}

/// Asks the worker to end, gives it [`SHUTDOWN_GRACE`], then kills it.
fn stop_gracefully(mut worker: Worker) -> Ending {
    let asked = send(&mut worker, &Request::Shutdown { id: 0 }).is_ok();
    worker.stdin = None;
    if asked {
        let started = Instant::now();
        while started.elapsed() < SHUTDOWN_GRACE {
            if let Ok(Some(_)) = worker.child.try_wait() {
                return Ending::Graceful;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    kill(worker)
}

/// The path of the models folder under a local data directory.
pub fn models_root(local_data_dir: &Path) -> PathBuf {
    local_data_dir.join(models::MODELS_FOLDER_NAME)
}

/// What the provider needs on this machine: the worker found under `roots` (an empty path when
/// there is none, which reads as "not installed") and the model's folder under the local data
/// directory. Looks at the disk for the worker only, with a handful of `exists` calls.
pub fn setup_for(roots: &[PathBuf], local_data_dir: &Path, spec: &ModelSpec) -> Setup {
    Setup {
        launch: WorkerLaunch::program(find_worker(roots).unwrap_or_default()),
        model_folder: spec.folder(&models_root(local_data_dir)),
    }
}
