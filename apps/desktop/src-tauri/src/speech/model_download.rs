//! The one network file of the speech module (decision D5): it downloads a speech model, on an
//! explicit user action and nowhere else. Nothing else in `speech/` may name an HTTP client, and a
//! test scans the module to prove it.
//!
//! What it sends: a GET request for one zip file under the address of the `speechModelsUrl`
//! setting. No audio, no text, no name, no identifier of the user or of the machine travels with it.
//!
//! What it does: downloads the zip to a `.part` file while hashing it, checks its size and SHA-256
//! against the manifest, unpacks only the files the manifest names (never a path taken from the zip)
//! into a staging folder while hashing each, checks each one, writes the marker, and replaces the
//! model's folder with the staging folder. Any failure leaves no partial model behind. The user can
//! cancel at any moment; a cancel removes what was written.
//!
//! The shared gateway client is not used: this one is built here, with the same TLS stack, and it
//! cannot read the Windows certificate store, so behind an antivirus that intercepts TLS the download
//! can fail. The error says so and points to the side-load: the four files copied by hand into the
//! model's folder, then Verify.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::models::{self, ModelSpec, TreeDigest, TreeSpec};
use super::stt::CancelToken;
use crate::error::AppError;

/// Time allowed to reach the server.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// How often a waiting download looks at the cancel token.
const POLL: Duration = Duration::from_millis(250);
/// How long the connection may deliver nothing before the download is given up.
const STALL_LIMIT: Duration = Duration::from_secs(30);
/// Shortest time between two progress reports, so a fast link does not flood the interface.
const REPORT_EVERY: Duration = Duration::from_millis(200);
/// Size of the blocks read while unpacking.
const BLOCK_BYTES: usize = 1 << 20;

/// Which part of the installation is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Download,
    Unpack,
}

/// What the interface is told while a model installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: Phase,
    pub done_bytes: u64,
    pub total_bytes: u64,
}

/// Where progress goes: the interface's channel in the application, a recorder in the tests.
pub type Sink = Arc<dyn Fn(Progress) + Send + Sync>;

/// The address of the archive: the setting's address, one slash, the file name.
pub fn archive_url(base: &str, spec: &ModelSpec) -> String {
    format!("{}/{}", base.trim_end_matches('/'), spec.archive.file)
}

fn failed(folder: &Path) -> AppError {
    AppError::SpeechModelDownloadFailed {
        folder: folder.display().to_string(),
    }
}

/// Downloads, checks and installs `spec` under `models_root`. `speech_cancelled` when the user
/// stops it, `speech_model_corrupt` when a digest does not match, `speech_model_download_failed`
/// for everything the network or the disk did wrong.
pub async fn install(
    spec: &ModelSpec,
    base_url: &str,
    models_root: &Path,
    cancel: &CancelToken,
    sink: &Sink,
) -> Result<(), AppError> {
    let folder = spec.folder(models_root);
    fs::create_dir_all(models_root).map_err(|_| failed(&folder))?;
    let part = models_root.join(format!("{}.part", spec.archive.file));

    let downloaded = download(spec, base_url, &part, &folder, cancel, sink).await;
    if let Err(error) = downloaded {
        let _ = fs::remove_file(&part);
        return Err(error);
    }

    let unpack_spec = spec.clone();
    let unpack_root = models_root.to_path_buf();
    let unpack_part = part.clone();
    let unpack_cancel = cancel.clone();
    let unpack_sink = Arc::clone(sink);
    let unpacked = tauri::async_runtime::spawn_blocking(move || {
        unpack(
            &unpack_spec,
            &unpack_part,
            &unpack_root,
            &unpack_cancel,
            &unpack_sink,
        )
    })
    .await
    .map_err(|_| AppError::Internal)?;
    let _ = fs::remove_file(&part);
    unpacked
}

async fn download(
    spec: &ModelSpec,
    base_url: &str,
    part: &Path,
    folder: &Path,
    cancel: &CancelToken,
    sink: &Sink,
) -> Result<(), AppError> {
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(|_| failed(folder))?;
    // The request itself can take a while (a slow server, a proxy): a cancel is honoured meanwhile.
    let mut pending = Box::pin(client.get(archive_url(base_url, spec)).send());
    let response = loop {
        tokio::select! {
            outcome = &mut pending => break outcome.map_err(|_| failed(folder))?,
            _ = tokio::time::sleep(POLL) => {
                if cancel.is_cancelled() {
                    return Err(AppError::SpeechCancelled);
                }
            }
        }
    };
    if !response.status().is_success() {
        return Err(failed(folder));
    }
    let total = spec.archive.bytes;
    let mut file = fs::File::create(part).map_err(|_| failed(folder))?;
    let mut hasher = Sha256::new();
    let mut stream = response.bytes_stream();
    let mut done: u64 = 0;
    let mut last_report = Instant::now();
    let mut last_data = Instant::now();
    sink(Progress {
        phase: Phase::Download,
        done_bytes: 0,
        total_bytes: total,
    });
    loop {
        if cancel.is_cancelled() {
            return Err(AppError::SpeechCancelled);
        }
        let next = match tokio::time::timeout(POLL, stream.next()).await {
            Ok(next) => next,
            Err(_) => {
                if last_data.elapsed() >= STALL_LIMIT {
                    return Err(failed(folder));
                }
                continue;
            }
        };
        match next {
            None => break,
            Some(Err(_)) => return Err(failed(folder)),
            Some(Ok(chunk)) => {
                last_data = Instant::now();
                done += chunk.len() as u64;
                if done > total {
                    // More than the manifest says: not this file.
                    return Err(AppError::SpeechModelCorrupt);
                }
                hasher.update(&chunk);
                file.write_all(&chunk).map_err(|_| failed(folder))?;
                if last_report.elapsed() >= REPORT_EVERY {
                    last_report = Instant::now();
                    sink(Progress {
                        phase: Phase::Download,
                        done_bytes: done,
                        total_bytes: total,
                    });
                }
            }
        }
    }
    file.flush().map_err(|_| failed(folder))?;
    drop(file);
    sink(Progress {
        phase: Phase::Download,
        done_bytes: done,
        total_bytes: total,
    });
    if done != total || models::to_hex(&hasher.finalize()) != spec.archive.sha256 {
        return Err(AppError::SpeechModelCorrupt);
    }
    Ok(())
}

/// Removes the staging folder unless it was moved into place.
struct Staging {
    path: PathBuf,
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Where the unpacking of a pinned folder reports to, and how far the whole unpacking has got.
struct TreeProgress<'a> {
    done: u64,
    total: u64,
    last_report: Instant,
    sink: &'a Sink,
}

/// Unpacks the pinned folder of a model (the entries of the zip under `tree.dir`) into `staging`,
/// hashing each file, and checks the folder as a whole against the manifest. Paths come from
/// `enclosed_name`, which refuses a name that climbs out of the folder or is absolute; an entry
/// outside the pinned folder is never written.
fn unpack_tree(
    archive: &mut zip::ZipArchive<fs::File>,
    tree: &TreeSpec,
    staging: &Path,
    cancel: &CancelToken,
    progress: &mut TreeProgress<'_>,
    folder: &Path,
) -> Result<(), AppError> {
    let mut digest = TreeDigest::default();
    let mut count: u64 = 0;
    let mut written_total: u64 = 0;
    let mut block = vec![0_u8; BLOCK_BYTES];
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| AppError::SpeechModelCorrupt)?;
        let Some(name) = entry.enclosed_name() else {
            // A name that escapes the folder is a zip this application did not make.
            return Err(AppError::SpeechModelCorrupt);
        };
        let Ok(inside) = name.strip_prefix(&tree.dir) else {
            continue;
        };
        if inside.as_os_str().is_empty() {
            continue;
        }
        let target = staging.join(&tree.dir).join(inside);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|_| failed(folder))?;
            continue;
        }
        count += 1;
        if count > tree.files {
            return Err(AppError::SpeechModelCorrupt);
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|_| failed(folder))?;
        }
        let mut out = fs::File::create(&target).map_err(|_| failed(folder))?;
        let mut hasher = Sha256::new();
        let mut written: u64 = 0;
        loop {
            if cancel.is_cancelled() {
                return Err(AppError::SpeechCancelled);
            }
            let read = entry
                .read(&mut block)
                .map_err(|_| AppError::SpeechModelCorrupt)?;
            if read == 0 {
                break;
            }
            written += read as u64;
            written_total += read as u64;
            if written_total > tree.bytes {
                return Err(AppError::SpeechModelCorrupt);
            }
            hasher.update(&block[..read]);
            out.write_all(&block[..read]).map_err(|_| failed(folder))?;
        }
        out.flush().map_err(|_| failed(folder))?;
        drop(out);
        let relative = inside
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        digest.add(&relative, &models::to_hex(&hasher.finalize()), written);
        progress.done += written;
        if progress.last_report.elapsed() >= REPORT_EVERY {
            progress.last_report = Instant::now();
            (progress.sink)(Progress {
                phase: Phase::Unpack,
                done_bytes: progress.done,
                total_bytes: progress.total,
            });
        }
    }
    if !digest.matches(tree) {
        return Err(AppError::SpeechModelCorrupt);
    }
    Ok(())
}

fn unpack(
    spec: &ModelSpec,
    part: &Path,
    models_root: &Path,
    cancel: &CancelToken,
    sink: &Sink,
) -> Result<(), AppError> {
    let folder = spec.folder(models_root);
    let staging = Staging {
        path: models_root.join(format!("{}.staging", spec.id)),
    };
    let _ = fs::remove_dir_all(&staging.path);
    fs::create_dir_all(&staging.path).map_err(|_| failed(&folder))?;

    let archive_file = fs::File::open(part).map_err(|_| failed(&folder))?;
    let mut archive =
        zip::ZipArchive::new(archive_file).map_err(|_| AppError::SpeechModelCorrupt)?;
    let total: u64 = spec.files.iter().map(|file| file.bytes).sum::<u64>()
        + spec.tree().map_or(0, |tree| tree.bytes);
    let mut done: u64 = 0;
    let mut last_report = Instant::now();
    let mut block = vec![0_u8; BLOCK_BYTES];
    for wanted in &spec.files {
        // Only the names of the manifest are ever written, to paths built here: whatever else the
        // zip holds, and whatever path an entry claims, is never touched.
        let mut entry = archive
            .by_name(&wanted.name)
            .map_err(|_| AppError::SpeechModelCorrupt)?;
        let mut out =
            fs::File::create(staging.path.join(&wanted.name)).map_err(|_| failed(&folder))?;
        let mut hasher = Sha256::new();
        let mut written: u64 = 0;
        loop {
            if cancel.is_cancelled() {
                return Err(AppError::SpeechCancelled);
            }
            let read = entry
                .read(&mut block)
                .map_err(|_| AppError::SpeechModelCorrupt)?;
            if read == 0 {
                break;
            }
            written += read as u64;
            if written > wanted.bytes {
                return Err(AppError::SpeechModelCorrupt);
            }
            hasher.update(&block[..read]);
            out.write_all(&block[..read]).map_err(|_| failed(&folder))?;
            if last_report.elapsed() >= REPORT_EVERY {
                last_report = Instant::now();
                sink(Progress {
                    phase: Phase::Unpack,
                    done_bytes: done + written,
                    total_bytes: total,
                });
            }
        }
        out.flush().map_err(|_| failed(&folder))?;
        drop(out);
        done += written;
        if written != wanted.bytes || models::to_hex(&hasher.finalize()) != wanted.sha256 {
            return Err(AppError::SpeechModelCorrupt);
        }
    }
    if let Some(tree) = spec.tree() {
        let mut progress = TreeProgress {
            done,
            total,
            last_report,
            sink,
        };
        unpack_tree(
            &mut archive,
            tree,
            &staging.path,
            cancel,
            &mut progress,
            &folder,
        )?;
    }
    models::write_marker(spec, &staging.path)?;

    if folder.exists() {
        fs::remove_dir_all(&folder).map_err(|_| failed(&folder))?;
    }
    fs::rename(&staging.path, &folder).map_err(|_| failed(&folder))?;
    sink(Progress {
        phase: Phase::Unpack,
        done_bytes: total,
        total_bytes: total,
    });
    Ok(())
}
