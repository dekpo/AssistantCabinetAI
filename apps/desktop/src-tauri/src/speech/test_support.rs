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
