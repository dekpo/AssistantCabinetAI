//! `speech-tts-worker`: the text-to-speech worker. Reads JSON-lines requests on its standard input
//! and answers on its standard output (`protocol`). Piper and Coqui VITS voices through sherpa-onnx,
//! linked from the FULL libraries, which carry the GPL-3.0 espeak-ng phonemizer: this executable has
//! its own notice (`NOTICE-TTS-WORKER.md`) and shares no build with the speech-to-text worker.
//!
//! No network code and no log. The only file it writes is the audio it was asked to produce, at the
//! path it was given, and it keeps nothing. The process ends when its standard input closes, which
//! is how it dies with the application.

use std::io::{stdin, stdout, BufWriter};

use speech_worker::protocol;
use speech_worker::voice::VoiceEngine;

fn main() {
    let mut engine = VoiceEngine::default();
    let input = stdin().lock();
    let output = BufWriter::new(stdout().lock());
    if protocol::serve(input, output, &mut engine).is_err() {
        std::process::exit(1);
    }
}
