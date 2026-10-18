//! The one audio format of the speech module, read by hand.
//!
//! The webview records and writes a canonical WAV (`src/audio/wav.ts`, `encodeWav`): a 44-byte
//! header, mono, 16-bit PCM, 16 kHz, little-endian. This parser accepts exactly that file and
//! nothing else, and decides from the samples themselves whether a clip is long enough, short
//! enough and loud enough. Rust never trusts a number the webview computed about its own clip.
//!
//! Pure functions over bytes: no file, no thread, no network.

use crate::error::AppError;

/// The only sample rate accepted, in hertz: what the speech-to-text engine expects.
pub const SAMPLE_RATE_HZ: u32 = 16_000;
/// Size of the canonical header, in bytes.
pub const HEADER_BYTES: usize = 44;
/// Bytes per mono 16-bit sample.
const BYTES_PER_SAMPLE: usize = 2;

/// Shortest clip worth transcribing, in milliseconds. Below this it is a click, not a sentence.
pub const MIN_CLIP_MS: u64 = 300;
/// Peak below which a clip counts as too quiet, as a share of full scale (0 to 1). A microphone
/// that is muted or covered stays under 1 %. Five real clips (speech peaks 0.44 to 1.0) sit far above it;
/// kept in phase 3b.
pub const QUIET_PEAK_FLOOR: f32 = 0.01;
/// Root-mean-square level below which a clip counts as too quiet, as a share of full scale (0 to 1).
/// The same five clips have levels of 0.06 to 0.24, thirty times above it; kept in phase 3b.
pub const QUIET_RMS_FLOOR: f32 = 0.002;

/// What the samples of a valid clip say about it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WavClip {
    pub sample_count: usize,
    pub duration_ms: u64,
    /// Highest magnitude, as a share of full scale (0 to 1).
    pub peak: f32,
    /// Root-mean-square level, as a share of full scale (0 to 1).
    pub rms: f32,
}

impl WavClip {
    /// True when the clip carries no usable voice: nothing for an engine to hear, and the case where
    /// engines invent text.
    pub fn is_quiet(&self) -> bool {
        self.peak < QUIET_PEAK_FLOOR || self.rms < QUIET_RMS_FLOOR
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Reads and validates a clip. Anything that is not exactly the canonical file, including a header
/// whose lengths disagree with the bytes received, answers `speech_audio_invalid`.
pub fn parse(bytes: &[u8]) -> Result<WavClip, AppError> {
    if bytes.len() < HEADER_BYTES {
        return Err(AppError::SpeechAudioInvalid);
    }
    let sample_bytes = bytes.len() - HEADER_BYTES;
    let canonical = &bytes[0..4] == b"RIFF"
        && &bytes[8..12] == b"WAVE"
        && &bytes[12..16] == b"fmt "
        && &bytes[36..40] == b"data"
        && read_u32(bytes, 4) as usize == bytes.len() - 8
        && read_u32(bytes, 16) == 16
        && read_u16(bytes, 20) == 1
        && read_u16(bytes, 22) == 1
        && read_u32(bytes, 24) == SAMPLE_RATE_HZ
        && read_u32(bytes, 28) as usize == SAMPLE_RATE_HZ as usize * BYTES_PER_SAMPLE
        && read_u16(bytes, 32) as usize == BYTES_PER_SAMPLE
        && read_u16(bytes, 34) == 16
        && read_u32(bytes, 40) as usize == sample_bytes
        && sample_bytes % BYTES_PER_SAMPLE == 0;
    if !canonical {
        return Err(AppError::SpeechAudioInvalid);
    }

    let sample_count = sample_bytes / BYTES_PER_SAMPLE;
    let mut peak: u32 = 0;
    let mut sum_of_squares: f64 = 0.0;
    for pair in bytes[HEADER_BYTES..].chunks_exact(BYTES_PER_SAMPLE) {
        let value = i16::from_le_bytes([pair[0], pair[1]]) as i32;
        peak = peak.max(value.unsigned_abs());
        sum_of_squares += f64::from(value) * f64::from(value);
    }
    let full_scale = f64::from(i16::MAX) + 1.0;
    let rms = if sample_count == 0 {
        0.0
    } else {
        (sum_of_squares / sample_count as f64).sqrt() / full_scale
    };
    Ok(WavClip {
        sample_count,
        duration_ms: sample_count as u64 * 1_000 / u64::from(SAMPLE_RATE_HZ),
        peak: (f64::from(peak) / full_scale).min(1.0) as f32,
        rms: rms as f32,
    })
}

/// Whether a valid clip may be transcribed for this use: not over the cap, not a click, not
/// silence. The order is the order the user can act on: shorten it, speak longer, speak louder.
pub fn check_usable(clip: &WavClip, cap_seconds: u32) -> Result<(), AppError> {
    if clip.duration_ms > u64::from(cap_seconds) * 1_000 {
        return Err(AppError::SpeechAudioTooLong);
    }
    if clip.duration_ms < MIN_CLIP_MS {
        return Err(AppError::SpeechAudioTooShort);
    }
    if clip.is_quiet() {
        return Err(AppError::SpeechAudioTooQuiet);
    }
    Ok(())
}
