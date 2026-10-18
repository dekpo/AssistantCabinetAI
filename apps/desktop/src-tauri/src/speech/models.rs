//! The speech models this application knows, and what is on disk for them.
//!
//! The manifest (`resources/speech/models-manifest.json`) is compiled in, so the sizes and
//! SHA-256 digests it pins cannot be changed by a file on the disk. The files of a model live in
//! one folder per model under the models root, and a small marker file says "these files were
//! checked against the manifest". Looking at the state is cheap (a few `stat` calls); hashing the
//! files, which takes seconds, happens only when the user presses Install or Verify.
//!
//! A file placed by hand in the folder (a side-load, for a machine where the download cannot work)
//! is `Unverified` until Verify has hashed it.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::status::ModelState;
use super::stt::CancelToken;
use super::worker_protocol::ModelFiles;
use crate::error::AppError;

/// Name of the folder, under the application's local data directory, that holds every model.
pub const MODELS_FOLDER_NAME: &str = "speech-models";
/// The marker file written inside a model's folder once its files matched the manifest.
pub const MARKER_FILE: &str = ".verified";

const MANIFEST_JSON: &str = include_str!("../../resources/speech/models-manifest.json");

/// The file a model is distributed as: one zip of its files, hosted under the models address.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ArchiveSpec {
    /// The file name under the address of the `speechModelsUrl` setting.
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
}

/// One file of a model, as it must be on disk.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FileSpec {
    /// What the engine uses it as (`encoder`, `decoder`, `joiner`, `tokens`).
    pub role: String,
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModelSpec {
    pub id: String,
    pub kind: String,
    pub licence: String,
    pub attribution: String,
    pub source: String,
    pub archive: ArchiveSpec,
    pub files: Vec<FileSpec>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    models: Vec<ModelSpec>,
}

fn manifest() -> Option<&'static Manifest> {
    static PARSED: OnceLock<Option<Manifest>> = OnceLock::new();
    PARSED
        .get_or_init(|| serde_json::from_str(MANIFEST_JSON).ok())
        .as_ref()
}

/// The speech-to-text model the application ships a manifest row for.
pub fn stt_model() -> Option<&'static ModelSpec> {
    manifest()?.models.iter().find(|model| model.kind == "stt")
}

impl ModelSpec {
    /// The folder of this model under `models_root`.
    pub fn folder(&self, models_root: &Path) -> PathBuf {
        models_root.join(&self.id)
    }

    /// The four paths a transducer worker is given.
    pub fn worker_files(&self, folder: &Path) -> Option<ModelFiles> {
        let path = |role: &str| {
            self.files
                .iter()
                .find(|file| file.role == role)
                .map(|file| folder.join(&file.name).to_string_lossy().into_owned())
        };
        Some(ModelFiles {
            encoder: path("encoder")?,
            decoder: path("decoder")?,
            joiner: path("joiner")?,
            tokens: path("tokens")?,
        })
    }

    /// What the marker holds: a digest of the manifest row, so a new model version in a later
    /// release makes the old marker stale instead of silently trusted.
    pub fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.id.as_bytes());
        for file in &self.files {
            hasher.update(format!("\n{}:{}:{}", file.name, file.bytes, file.sha256));
        }
        to_hex(&hasher.finalize())
    }
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The state of a model's folder, without hashing anything.
///
/// No file at all, or only some of them: `Missing`. A file of the wrong size: `Corrupt`. Every
/// file there with its pinned size: `Present` if the marker matches the manifest, else
/// `Unverified`.
pub fn state(spec: &ModelSpec, folder: &Path) -> ModelState {
    let mut found = 0;
    let mut wrong_size = false;
    for file in &spec.files {
        match fs::metadata(folder.join(&file.name)) {
            Ok(meta) if meta.is_file() => {
                found += 1;
                wrong_size |= meta.len() != file.bytes;
            }
            _ => {}
        }
    }
    if found < spec.files.len() {
        return ModelState::Missing;
    }
    if wrong_size {
        return ModelState::Corrupt;
    }
    match fs::read_to_string(folder.join(MARKER_FILE)) {
        Ok(marker) if marker.trim() == spec.fingerprint() => ModelState::Present,
        _ => ModelState::Unverified,
    }
}

/// SHA-256 of a file, read in blocks. Stops with `speech_cancelled` when the token is set.
pub fn hash_file(path: &Path, cancel: &CancelToken) -> Result<String, AppError> {
    let mut file = fs::File::open(path).map_err(|_| AppError::SpeechModelMissing)?;
    let mut hasher = Sha256::new();
    let mut block = vec![0_u8; 1 << 20];
    loop {
        if cancel.is_cancelled() {
            return Err(AppError::SpeechCancelled);
        }
        let read = file
            .read(&mut block)
            .map_err(|_| AppError::SpeechModelCorrupt)?;
        if read == 0 {
            return Ok(to_hex(&hasher.finalize()));
        }
        hasher.update(&block[..read]);
    }
}

/// Hashes every file of the model's folder against the manifest and, if all match, writes the
/// marker. `speech_model_missing` for an absent file, `speech_model_corrupt` for any mismatch
/// (nothing is written then). This is the one place a hand-placed file is trusted.
pub fn verify(spec: &ModelSpec, folder: &Path, cancel: &CancelToken) -> Result<(), AppError> {
    for file in &spec.files {
        let path = folder.join(&file.name);
        let meta = fs::metadata(&path).map_err(|_| AppError::SpeechModelMissing)?;
        if !meta.is_file() {
            return Err(AppError::SpeechModelMissing);
        }
        if meta.len() != file.bytes || hash_file(&path, cancel)? != file.sha256 {
            return Err(AppError::SpeechModelCorrupt);
        }
    }
    write_marker(spec, folder)
}

/// Writes the marker. Only [`verify`] and the installer, which has just hashed every file, call it.
pub fn write_marker(spec: &ModelSpec, folder: &Path) -> Result<(), AppError> {
    fs::write(folder.join(MARKER_FILE), spec.fingerprint())
        .map_err(|_| AppError::SpeechModelCorrupt)
}

/// Holds the "a model job is running" flag and its cancel token: installing or verifying, one at a
/// time. Dropping the guard releases both, so an error, a cancel or a panic cannot leave it taken.
#[derive(Default)]
pub struct JobSlot {
    running: Arc<AtomicBool>,
    token: Arc<Mutex<Option<CancelToken>>>,
}

#[derive(Debug)]
pub struct JobGuard {
    running: Arc<AtomicBool>,
    token: Arc<Mutex<Option<CancelToken>>>,
    cancel: CancelToken,
}

impl JobGuard {
    /// The token the job polls.
    pub fn cancel_token(&self) -> &CancelToken {
        &self.cancel
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        if let Ok(mut token) = self.token.lock() {
            *token = None;
        }
        self.running.store(false, Ordering::SeqCst);
    }
}

impl JobSlot {
    /// Takes the slot, or answers `speech_busy`.
    pub fn begin(&self) -> Result<JobGuard, AppError> {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(AppError::SpeechBusy);
        }
        let cancel = CancelToken::new();
        let guard = JobGuard {
            running: Arc::clone(&self.running),
            token: Arc::clone(&self.token),
            cancel: cancel.clone(),
        };
        match self.token.lock() {
            Ok(mut token) => *token = Some(cancel),
            Err(_) => return Err(AppError::Internal),
        }
        Ok(guard)
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Stops the running job, if any. Returns whether there was one.
    pub fn cancel(&self) -> bool {
        match self.token.lock() {
            Ok(token) => match token.as_ref() {
                Some(token) => {
                    token.cancel();
                    true
                }
                None => false,
            },
            Err(_) => false,
        }
    }
}
