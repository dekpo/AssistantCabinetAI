//! `speech-stt-worker`: the speech-to-text worker. Reads JSON-lines requests on its standard input
//! and answers on its standard output (`protocol`). Parakeet through sherpa-onnx, linked from the
//! no-tts libraries so that no espeak-ng (GPL-3.0) code is in this executable.
//!
//! No network code, no log, no file written: the clip is read from the path it is given and the text
//! goes to the standard output and nowhere else. The process ends when its standard input closes,
//! which is how it dies with the application.

use std::io::{stdin, stdout, BufWriter};

use speech_worker::engine::SherpaEngine;
use speech_worker::protocol;

fn main() {
    let mut engine = SherpaEngine::default();
    let input = stdin().lock();
    let output = BufWriter::new(stdout().lock());
    if protocol::serve(input, output, &mut engine).is_err() {
        std::process::exit(1);
    }
}
