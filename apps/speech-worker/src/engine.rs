//! The sherpa-onnx speech-to-text engine: the only place of the worker crate that knows it.
//! A NeMo transducer (Parakeet TDT 0.6B v3, int8), greedy search, on the CPU.
//!
//! Parakeet v3 detects the language itself, so the language the application sends is not used.
//! The decode call is blocking and cannot be interrupted: the application cancels a job by killing
//! this process.

use std::path::Path;
use std::time::Instant;

use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};

use crate::protocol::{Engine, FailureKind, Heard, ModelFiles};
use crate::wav;

/// Holds the recognizer once `load` succeeded.
#[derive(Default)]
pub struct SherpaEngine {
    recognizer: Option<OfflineRecognizer>,
}

fn file_str(path: &str) -> Option<String> {
    Some(path.to_string())
}

impl Engine for SherpaEngine {
    fn load(&mut self, threads: u32, files: &ModelFiles) -> Result<u64, FailureKind> {
        for path in [&files.encoder, &files.decoder, &files.joiner, &files.tokens] {
            if !Path::new(path).is_file() {
                return Err(FailureKind::ModelMissing);
            }
        }
        let mut config = OfflineRecognizerConfig::default();
        config.model_config.tokens = file_str(&files.tokens);
        config.model_config.num_threads = i32::try_from(threads).unwrap_or(1).max(1);
        config.model_config.provider = Some("cpu".into());
        config.model_config.model_type = Some("nemo_transducer".into());
        config.model_config.transducer = OfflineTransducerModelConfig {
            encoder: file_str(&files.encoder),
            decoder: file_str(&files.decoder),
            joiner: file_str(&files.joiner),
        };
        config.decoding_method = Some("greedy_search".into());

        let started = Instant::now();
        let recognizer = OfflineRecognizer::create(&config).ok_or(FailureKind::ModelCorrupt)?;
        self.recognizer = Some(recognizer);
        Ok(started.elapsed().as_millis() as u64)
    }

    fn transcribe(&mut self, wav_path: &str, _language: &str) -> Result<Heard, FailureKind> {
        let recognizer = self.recognizer.as_ref().ok_or(FailureKind::EngineFailed)?;
        let bytes = std::fs::read(wav_path).map_err(|_| FailureKind::AudioInvalid)?;
        let clip = wav::parse(&bytes).ok_or(FailureKind::AudioInvalid)?;
        let started = Instant::now();
        let stream = recognizer.create_stream();
        stream.accept_waveform(clip.sample_rate as i32, &clip.samples);
        recognizer.decode(&stream);
        let result = stream.get_result().ok_or(FailureKind::EngineFailed)?;
        Ok(Heard {
            text: result.text.trim().to_string(),
            audio_ms: clip.duration_ms(),
            processing_ms: started.elapsed().as_millis() as u64,
        })
    }
}
