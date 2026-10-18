# Speech phase 3: STT port and fake provider (lot report)

Date: 18 October 2026. Branch: `feat/speech-stt-port`, from `feat/speech-integration` (phase 10 merged, PR #45; working tree clean when the work started;
the changes below are uncommitted).

## What was done

The speech-to-text contract, testable without any engine: a trait, a service that runs one job at a time under a time limit, a hand-written WAV parser, a private
temporary clip folder, an output guard, a scripted fake provider, and two thin commands (`speech_transcribe`, `speech_cancel`). In the application the service is built
**without a provider**, so `speech_transcribe` answers `speech_engine_missing`; `Probes` stays `Default` and `speech_status` still reports the engine as absent.
Phase 3b plugs the real worker behind the same trait and fills the probe.

## Files created

| File | Content |
|---|---|
| `src-tauri/src/speech/stt.rs` | `SpeechToTextProvider` (`engine_id`, `availability(locale)`, `transcribe(request, cancel)`), `SttRequest`, `SttResult` (camelCase), `CancelToken` (own `AtomicBool`) |
| `src-tauri/src/speech/service.rs` | `SpeechService` (provider, busy flag, cancel token, lazy sweep, `Limits`), `plan(settings, purpose)`, `Purpose`, `timing_line` |
| `src-tauri/src/speech/wav.rs` | strict parser of the canonical 44-byte file (`parse`, `check_usable`, `WavClip` with peak and RMS) |
| `src-tauri/src/speech/temp_audio.rs` | `TempAudioFile` (removed on drop), `write`, `sweep` |
| `src-tauri/src/speech/output_guard.rs` | `check(text, audio_ms, quiet)`, `word_count` |
| `src-tauri/src/speech/fake.rs` | `FakeProvider` (scripted text / failure / panic / delay, records what it saw); declared `#[cfg(test)]` |
| `src-tauri/src/speech/stt_tests.rs` | 26 tests |
| `src/speech/ipc.test.ts` | 3 tests of the bridge (raw body, header, cancel) |

## Existing files edited (every edit)

| File | Edit |
|---|---|
| `src-tauri/src/speech/mod.rs` | 5 `pub mod`, `#[cfg(test)] mod fake;`, `#[cfg(test)] mod stt_tests;`, doc lines |
| `src-tauri/src/speech_commands.rs` | `SpeechState` gains `service: SpeechService` (+ `pub(crate) fn service()`), the two commands, 4 `use` lines |
| `src-tauri/src/lib.rs` | the two handler names, next to `notes_save`; still no `.setup(` |
| `src/speech/ipc.ts` | `SpeechPurpose`, `SpeechTranscript`, `speechTranscribe(wav, purpose?)`, `speechCancel()` |

`error.rs`, the catalogues and `commands.rs` are **not** touched (no variant added; all 11 codes already existed). Nothing widened to `pub(crate)` in `commands.rs`;
nothing duplicated. `FOUNDATION_FILES` is not touched. `Cargo.toml` is not touched (no WAV crate, no new dependency).

## Behaviour and decisions

- **Spike S2 answered, as the prompt said**: the clip is the raw request body (`tauri::ipc::Request`, `InvokeBody::Raw`), the webview calls `invoke("speech_transcribe", wav, { headers })`.
  The body is the WAV and nothing else. Not exercised in the real window in this phase (no UI); the raw-body path is proven by SpeechLab's `save_clip` and by a scan of the command source.
- **The use travels as a header** `purpose` (`dictation` or `note`). Absent, or any other value, is `dictation`, the shorter cap (safe default). The cap is
  `settings.speech.max_dictation_seconds` or `max_note_seconds`, read in Rust; over it is `speech_audio_too_long`. The webview never sends a number of seconds or a language.
- **Order of a job**: voice switch and dictation switch (`speech_unavailable`) -> language table (`speech_locale_unsupported`) -> provider (`speech_engine_missing`) -> `availability`
  (`speech_model_missing` / `_corrupt`) -> busy flag (`speech_busy`) -> lazy sweep of the temp folder -> parse (`speech_audio_invalid`) -> too long, too short, too quiet -> clip written ->
  engine on `tauri::async_runtime::spawn_blocking` under `tokio::time::timeout` -> cancel check -> output guard (`speech_output_rejected`) -> clip removed.
  Both the notes use and the chat use require `enabled` and `dictationEnabled`; phase 12 may refine it for notes.
- **Busy**: taken before the temp file exists, released by a drop guard that moves into the blocking closure together with the clip. A panic, an error or a cancel release it. After a timeout the
  token is set and the caller gets `speech_timeout` at once, but the flag and the clip are released only when the engine really returns, so a hung engine keeps the service busy rather than running two jobs.
  The phase 3b worker must therefore honour the token (kill the process) promptly.
- **Cancel**: `speech_cancel` sets the token of the running job; returns whether there was one. A cancel that arrives after the engine answered still discards the text.
- **Timeout**: `TIMEOUT_FLOOR_SECONDS = 30` plus `TIMEOUT_PER_AUDIO_SECOND = 1.0` times the clip length (`Limits`, replaceable in tests).
- **Temp audio**: `<app local data dir>/speech-temp/clip-<pid>-<n>.wav`, created new, removed on drop (success, error, cancel, timeout, panic). The sweep lists that one folder, one level, files named like a clip; it runs once, at the first job, never at launch.
  An I/O failure while writing a clip is reported as `speech_engine_failed` (no dedicated code exists, none added).
- **Output guard**: rejects empty text, any text for audio flagged too quiet, words per second outside [0.1, 8.0], one word above 50 % of 8+ words, one group of up to 4 words repeated 4+ times in a row.
  **All thresholds are provisional constants** (`QUIET_PEAK_FLOOR = 0.01`, `QUIET_RMS_FLOOR = 0.002`, `MIN_CLIP_MS = 300`, and the guard's): phase 3b calibrates them on real clips.
- **Log**: one line per job in the command, `speech: engine=<id> audio_ms=<n> processing_ms=<n> words=<n>`, never the text (`timing_line`, tested).

## Tests

Pre-check before the first edit (targeted): `cargo test --lib speech -j 2` 50 passed.
Final pass, once, after the last edit: `cargo test --lib --tests --no-fail-fast -j 2` all green (981 passed, 4 ignored in the library, every other binary ok; 76 speech tests, 26 new);
`cargo clippy --all-targets`: no warning in a speech file (22 warnings elsewhere, pre-existing, not investigated); `pnpm run build` ok; `pnpm run test` 63 files, 1510 tests passed (the Rust language guard included);
`rustfmt --check --edition 2021` clean on the 7 new Rust files; `speech_commands.rs` 0 diff blocks before and after; `speech/mod.rs` 0 (without its children).

Covered: header byte for byte against `encodeWav`; every header field broken one at a time; truncated, padded and non-audio input; too long / too short / too quiet and their order; the clip file exists while the engine runs
and is gone after success, engine failure, rejected output, panic, cancel and timeout; a refused clip never reaches the engine or the disk; the lazy sweep (once, one level, clip names only, never at construction); one job at a time with the flag released;
cancel (running, idle, after the text); timeout; engine missing and engine not ready write nothing; the plan (switches, language table, German unsupported, caps by use); the header parse; synthetic silence, tone, noise and repeated text through the service;
the guard alone; timing line without text; no network, process or thread token in `stt.rs`, `output_guard.rs`, `wav.rs`; no HTTP client named anywhere in the module; no command constructs the fake; commands registered; raw body and local-data-dir scans; the default service answers `speech_engine_missing`.

**Not proven by a test**: the two commands themselves (they need an `AppHandle`), and the raw body through the real webview (phase 5's human test is its first real use).

## For the next phases

- Phase 3b: implement `SpeechToTextProvider` on the worker, build the service with `Some(provider)` in `SpeechState`, fill `Probes`, flip `INSTALL_AVAILABLE`; calibrate the guard and the quiet floors; honour the cancel token by killing the worker.
- Phases 5 and 12: call `speechTranscribe(wav, "dictation" | "note")` and show the machine codes; `speechCancel()` for the stop button.

## Human test

None proposed: nothing is visible (no UI, no engine).

## Prompt alignment

Checked `05a_speech-11_notes-mode-typed.md` (the next prompt): aligned; appended "Alignment with phase 3" (new ipc exports not used by typed notes, scans that still read `speech_commands.rs`, branch creation after merge).
