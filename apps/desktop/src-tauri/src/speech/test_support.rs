//! What the tests of the worker client, the model files and the download share: a tiny model (four
//! small files and the zip that holds them, with the manifest row that pins them), and a canonical
//! clip. Everything is generated inside the test; nothing is read from the repository.

use std::io::Write;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::models::{self, ArchiveSpec, FileSpec, ModelSpec};
use super::wav::{HEADER_BYTES, SAMPLE_RATE_HZ};

/// The four files of the tiny model: role, name and contents.
pub fn tiny_files() -> Vec<(&'static str, &'static str, Vec<u8>)> {
    vec![
        ("encoder", "enc.bin", vec![1_u8; 3_000]),
        ("decoder", "dec.bin", vec![2_u8; 500]),
        ("joiner", "join.bin", vec![3_u8; 700]),
        ("tokens", "tok.txt", b"a 0\nb 1\n".to_vec()),
    ]
}

pub fn digest(bytes: &[u8]) -> String {
    models::to_hex(&Sha256::digest(bytes))
}

/// A zip holding `files`, deflated, flat names.
pub fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in files {
            writer.start_file(*name, options).expect("zip entry starts");
            writer.write_all(bytes).expect("zip entry writes");
        }
        writer.finish().expect("zip finishes");
    }
    buffer.into_inner()
}

/// The manifest row of the tiny model and the zip it pins.
pub fn tiny_model() -> (ModelSpec, Vec<u8>) {
    let files = tiny_files();
    let zip = zip_of(
        &files
            .iter()
            .map(|(_, name, bytes)| (*name, bytes.as_slice()))
            .collect::<Vec<_>>(),
    );
    let spec = ModelSpec {
        id: "tiny-model".into(),
        kind: "stt".into(),
        licence: "test".into(),
        attribution: "test".into(),
        source: "test".into(),
        archive: ArchiveSpec {
            file: "tiny-model.zip".into(),
            bytes: zip.len() as u64,
            sha256: digest(&zip),
        },
        files: files
            .iter()
            .map(|(role, name, bytes)| FileSpec {
                role: (*role).into(),
                name: (*name).into(),
                bytes: bytes.len() as u64,
                sha256: digest(bytes),
            })
            .collect(),
        cased_output: false,
        voice: None,
    };
    (spec, zip)
}

/// Writes the tiny model's files into `folder` (no marker).
pub fn place_tiny_files(folder: &Path) {
    std::fs::create_dir_all(folder).expect("folder");
    for (_, name, bytes) in tiny_files() {
        std::fs::write(folder.join(name), bytes).expect("model file writes");
    }
}

/// A canonical clip: a 440 Hz tone at 30 % of full scale, `milliseconds` long.
pub fn canonical_clip(milliseconds: u64) -> Vec<u8> {
    let count = (milliseconds * u64::from(SAMPLE_RATE_HZ) / 1_000) as usize;
    let data_bytes = count * 2;
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
    for index in 0..count {
        let phase = index as f32 * 440.0 * std::f32::consts::TAU / SAMPLE_RATE_HZ as f32;
        let sample = (phase.sin() * 0.3 * 32_767.0) as i16;
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

/// Every entry of the tiny voice's zip: the two files of the manifest row and the three files of the
/// pinned folder, in the order the zip holds them.
pub fn tiny_voice_entries() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("voice.onnx", vec![9_u8; 2_000]),
        ("tokens.txt", b"a 0\nb 1\n".to_vec()),
        ("phon-data/phondata", vec![4_u8; 600]),
        ("phon-data/lang/xx", b"name xx\n".to_vec()),
        ("phon-data/voices/default", vec![5_u8; 90]),
    ]
}

/// The digest a manifest row pins for a folder: the lines `path:sha256` and a line feed, sorted by
/// path (inside the folder), hashed. Written out here on purpose, apart from the code under test.
pub fn tree_digest_of(entries: &[(&str, Vec<u8>)], dir: &str) -> String {
    let mut lines: Vec<String> = entries
        .iter()
        .filter_map(|(name, bytes)| {
            name.strip_prefix(&format!("{dir}/"))
                .map(|inside| format!("{inside}:{}\n", digest(bytes)))
        })
        .collect();
    lines.sort();
    digest(lines.concat().as_bytes())
}

/// The manifest row of the tiny voice (a Piper-style voice with a pinned folder) and the zip it pins.
pub fn tiny_voice() -> (ModelSpec, Vec<u8>) {
    let entries = tiny_voice_entries();
    let zip = zip_of(
        &entries
            .iter()
            .map(|(name, bytes)| (*name, bytes.as_slice()))
            .collect::<Vec<_>>(),
    );
    let file = |role: &str, name: &str| {
        let bytes = &entries
            .iter()
            .find(|(entry, _)| *entry == name)
            .expect("entry")
            .1;
        FileSpec {
            role: role.into(),
            name: name.into(),
            bytes: bytes.len() as u64,
            sha256: digest(bytes),
        }
    };
    let tree_files: Vec<_> = entries
        .iter()
        .filter(|(name, _)| name.starts_with("phon-data/"))
        .collect();
    let spec = ModelSpec {
        id: "tiny-voice".into(),
        kind: "tts".into(),
        licence: "test".into(),
        attribution: "test".into(),
        source: "test".into(),
        archive: ArchiveSpec {
            file: "tiny-voice.zip".into(),
            bytes: zip.len() as u64,
            sha256: digest(&zip),
        },
        files: vec![file("model", "voice.onnx"), file("tokens", "tokens.txt")],
        cased_output: false,
        voice: Some(models::VoiceSpec {
            family: super::worker_protocol::VoiceFamily::Piper,
            rating: models::LicenceRating::Clear,
            tree: Some(models::TreeSpec {
                dir: "phon-data".into(),
                files: tree_files.len() as u64,
                bytes: tree_files.iter().map(|(_, bytes)| bytes.len() as u64).sum(),
                sha256: tree_digest_of(&entries, "phon-data"),
            }),
        }),
    };
    (spec, zip)
}

/// Writes the tiny voice's files and folder into `folder` (no marker).
pub fn place_tiny_voice(folder: &Path) {
    for (name, bytes) in tiny_voice_entries() {
        let target = folder.join(name);
        std::fs::create_dir_all(target.parent().expect("parent")).expect("folder");
        std::fs::write(target, bytes).expect("voice file writes");
    }
}
