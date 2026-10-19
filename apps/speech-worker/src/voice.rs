//! The sherpa-onnx text-to-speech engine: the only place of the worker crate that speaks. A VITS
//! voice (Piper, through the espeak-ng phonemizer, or Coqui, which reads characters), on the CPU.
//!
//! One voice is loaded at a time and `load_voice` replaces it. A synthesis cannot be interrupted
//! from inside, so the application cancels one by killing this process.

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsVitsModelConfig,
};

use crate::protocol::{Engine, FailureKind, Heard, ModelFiles, Spoken, VoiceFamily, VoiceFiles};
use crate::wav;

/// Slowest and fastest reading speed the engine is asked for, as a percentage of natural speed.
/// The application clamps first; this keeps a stray request from reaching the library.
const MIN_SPEED_PERCENT: u32 = 50;
const MAX_SPEED_PERCENT: u32 = 200;

/// Holds the voice once `load_voice` succeeded.
#[derive(Default)]
pub struct VoiceEngine {
    tts: Option<OfflineTts>,
}

fn file_str(path: &str) -> Option<String> {
    Some(path.to_string())
}

/// The text a voice receives: control characters become spaces (a NUL would abort the library's
/// string conversion), and the ends are trimmed. Empty when nothing can be said.
pub fn clean_text(text: &str) -> String {
    let spaced: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    spaced.trim().to_string()
}

/// Whether a text holds anything a voice can say: at least one letter or digit.
pub fn has_speech(text: &str) -> bool {
    text.chars().any(char::is_alphanumeric)
}

impl Engine for VoiceEngine {
    fn load(&mut self, _threads: u32, _files: &ModelFiles) -> Result<u64, FailureKind> {
        Err(FailureKind::BadRequest)
    }

    fn transcribe(&mut self, _wav_path: &str, _language: &str) -> Result<Heard, FailureKind> {
        Err(FailureKind::BadRequest)
    }

    fn load_voice(&mut self, threads: u32, voice: &VoiceFiles) -> Result<u64, FailureKind> {
        let mut needed = vec![&voice.model, &voice.tokens];
        needed.extend(voice.data_dir.as_ref());
        for path in needed {
            if !Path::new(path).exists() {
                return Err(FailureKind::ModelMissing);
            }
        }
        // A Piper voice needs its phonemizer's data folder; a Coqui voice has none.
        let vits = match voice.family {
            VoiceFamily::Piper => OfflineTtsVitsModelConfig {
                model: file_str(&voice.model),
                tokens: file_str(&voice.tokens),
                data_dir: Some(voice.data_dir.clone().ok_or(FailureKind::ModelMissing)?),
                ..Default::default()
            },
            VoiceFamily::Coqui => OfflineTtsVitsModelConfig {
                model: file_str(&voice.model),
                tokens: file_str(&voice.tokens),
                ..Default::default()
            },
        };
        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                vits,
                num_threads: i32::try_from(threads).unwrap_or(1).max(1),
                debug: false,
                provider: Some("cpu".into()),
                ..Default::default()
            },
            // One sentence per library call: the application sends a sentence at a time.
            max_num_sentences: 1,
            silence_scale: 0.2,
            ..Default::default()
        };
        // The previous voice is released before the next one is read.
        self.tts = None;
        let started = Instant::now();
        let tts = OfflineTts::create(&config).ok_or(FailureKind::ModelCorrupt)?;
        self.tts = Some(tts);
        Ok(started.elapsed().as_millis() as u64)
    }

    fn synthesize(
        &mut self,
        text: &str,
        speed_percent: u32,
        wav_path: &str,
    ) -> Result<Spoken, FailureKind> {
        let tts = self.tts.as_ref().ok_or(FailureKind::EngineFailed)?;
        let text = clean_text(text);
        if !has_speech(&text) {
            return Err(FailureKind::NothingToSay);
        }
        let config = GenerationConfig {
            speed: speed_percent.clamp(MIN_SPEED_PERCENT, MAX_SPEED_PERCENT) as f32 / 100.0,
            sid: 0,
            ..Default::default()
        };
        let started = Instant::now();
        let audio = tts
            .generate_with_config(&text, &config, None::<fn(&[f32], f32) -> bool>)
            .ok_or(FailureKind::EngineFailed)?;
        let sample_rate = u32::try_from(audio.sample_rate()).unwrap_or(0);
        if audio.samples().is_empty() || sample_rate == 0 {
            return Err(FailureKind::EngineFailed);
        }
        let processing_ms = started.elapsed().as_millis() as u64;
        let bytes = wav::encode_pcm16(audio.samples(), sample_rate);
        write_new(wav_path, &bytes).map_err(|_| FailureKind::EngineFailed)?;
        Ok(Spoken {
            audio_ms: audio.samples().len() as u64 * 1_000 / u64::from(sample_rate),
            processing_ms,
            sample_rate,
        })
    }
}

/// A new file, never over an existing one: the application names it inside its private folder.
fn write_new(path: &str, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_characters_never_reach_the_library() {
        assert_eq!(clean_text("  Bonjour\0 le monde\n"), "Bonjour  le monde");
        assert_eq!(clean_text("\0\0"), "");
    }

    #[test]
    fn a_text_without_letters_or_digits_has_nothing_to_say() {
        assert!(!has_speech(" ... !? "));
        assert!(has_speech("a"));
        assert!(has_speech("42"));
    }

    #[test]
    fn a_synthesis_before_a_voice_is_loaded_fails() {
        let mut engine = VoiceEngine::default();
        assert_eq!(
            engine.synthesize("Bonjour", 100, "unused.wav").unwrap_err(),
            FailureKind::EngineFailed
        );
    }

    #[test]
    fn a_missing_voice_file_is_reported_as_missing() {
        let mut engine = VoiceEngine::default();
        let voice = VoiceFiles {
            family: VoiceFamily::Coqui,
            model: "no-such-model.onnx".into(),
            tokens: "no-such-tokens.txt".into(),
            data_dir: None,
        };
        assert_eq!(
            engine.load_voice(1, &voice).unwrap_err(),
            FailureKind::ModelMissing
        );
    }

    /// One real synthesis per voice family, with the voices placed by hand: `ACAI_TEST_VOICES` is
    /// a folder that holds `fr/` (the siwis files and its `espeak-ng-data`) and `css10/`.
    fn speak_with(voice: VoiceFiles, text: &str) -> (Spoken, wav::Clip) {
        let out = std::env::temp_dir().join(format!(
            "acai-tts-test-{}-{}.wav",
            std::process::id(),
            text.len()
        ));
        let _ = std::fs::remove_file(&out);
        let mut engine = VoiceEngine::default();
        engine.load_voice(2, &voice).expect("voice loads");
        let spoken = engine
            .synthesize(text, 100, &out.to_string_lossy())
            .expect("speaks");
        let clip = wav::parse(&std::fs::read(&out).expect("wav written")).expect("canonical wav");
        let _ = std::fs::remove_file(&out);
        (spoken, clip)
    }

    #[test]
    #[ignore = "needs a voice model on disk (ACAI_TEST_VOICES)"]
    fn real_piper_voice_speaks_a_short_french_sentence() {
        let Some(root) = std::env::var_os("ACAI_TEST_VOICES") else {
            return;
        };
        let folder = Path::new(&root).join("fr");
        let voice = VoiceFiles {
            family: VoiceFamily::Piper,
            model: folder
                .join("fr_FR-siwis-medium.onnx")
                .to_string_lossy()
                .into(),
            tokens: folder.join("tokens.txt").to_string_lossy().into(),
            data_dir: Some(folder.join("espeak-ng-data").to_string_lossy().into()),
        };
        let (spoken, clip) = speak_with(voice, "Bonjour, le rendez-vous est confirm\u{e9}.");
        assert!(spoken.audio_ms > 1_000, "{spoken:?}");
        assert_eq!(clip.sample_rate, spoken.sample_rate);
    }

    #[test]
    #[ignore = "needs a voice model on disk (ACAI_TEST_VOICES)"]
    fn real_coqui_voice_speaks_a_short_french_sentence() {
        let Some(root) = std::env::var_os("ACAI_TEST_VOICES") else {
            return;
        };
        let folder = Path::new(&root).join("css10");
        let voice = VoiceFiles {
            family: VoiceFamily::Coqui,
            model: folder.join("model.onnx").to_string_lossy().into(),
            tokens: folder.join("tokens.txt").to_string_lossy().into(),
            data_dir: None,
        };
        let (spoken, clip) = speak_with(voice, "Bonjour, le rendez-vous est confirm\u{e9}.");
        assert!(spoken.audio_ms > 1_000, "{spoken:?}");
        assert_eq!(clip.sample_rate, spoken.sample_rate);
    }
}
