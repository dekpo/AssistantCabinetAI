# Notice for the voice worker (`speech-tts-worker`)

This file travels with the executable `speech-tts-worker` and with nothing else. It is the notice
of the text-to-speech worker of Assistant Cabinet AI, written in English like every file for
developers.

## Why this executable has its own notice

`speech-tts-worker` is linked against the **full** sherpa-onnx static libraries. They contain the
phonemizer **espeak-ng**, which is licensed under the **GNU General Public License, version 3
(GPL-3.0)**, and the voices in the Piper family read their text through it.

The speech-to-text worker (`speech-stt-worker`) and the application itself are built against the
libraries *without* the phonemizer. A build step refuses to stage them if the string `espeak`
appears in the executable. Nothing GPL-licensed is linked into the application or into the
speech-to-text worker.

The voice worker is a **separate executable**. The application starts it as a child process and
talks to it through its standard input and output, one JSON line per message
(`apps/desktop/src-tauri/src/speech/worker_protocol.rs`). They share no memory and no library.

## What this executable contains

| Component | Licence | Role |
|---|---|---|
| espeak-ng (inside the full sherpa-onnx libraries) | GPL-3.0 | Turns text into phonemes for the Piper voices |
| piper-phonemize (inside the same libraries) | MIT | Glue between Piper voices and espeak-ng |
| sherpa-onnx | Apache-2.0 | Runs the voice models |
| ONNX Runtime | MIT | Runs the neural network |
| `apps/speech-worker` (this crate) | the licence of the application | The worker loop and the WAV writer |

The text of the GPL-3.0 is at <https://www.gnu.org/licenses/gpl-3.0.txt>. The source of espeak-ng is
at <https://github.com/espeak-ng/espeak-ng>; the source of the libraries as built is the
sherpa-onnx release **v1.13.8** (<https://github.com/k2-fsa/sherpa-onnx>).

The folder `espeak-ng-data` that a Piper voice needs is **not** part of this executable. It is part
of each Piper voice package (it is one of the files the voice's download pins by digest) and is
placed next to the voice model, in the application's models folder.

## What this executable does and does not do

It reads the text on its standard input and writes one audio file at the path it is given. It opens
no network connection, writes no log, keeps no text and keeps no audio: the application reads the
audio file back and removes it at once. It ends when its standard input closes, which is how it
dies with the application.

## Voices

The voices are not part of this executable. Each is a separate download with its own licence,
recorded in `models/LICENSES.md` and shown next to the voice in the settings:

| Voice | Licence | Rating |
|---|---|---|
| Piper `fr_FR-siwis-medium` | CC BY 4.0 (SIWIS data); fine-tuned from a voice whose data is research-only | `review`: prototype, to verify before any distribution |
| Coqui VITS `fr` css10 | BSD-3-Clause (model); public-domain recordings | `attribution` |
| Piper `en_US-ljspeech-medium` | Public domain (LJ Speech) | `clear` |
| Piper `en_US-libritts_r-medium` | CC BY 4.0 (LibriTTS-R) | `attribution` |

This is a technical reading of public licence texts, not legal advice. A legal reading of the
"separate executable" position is open and must be settled before any distribution beyond the
controlled prototype.
