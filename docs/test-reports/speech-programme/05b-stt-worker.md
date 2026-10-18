# Speech phase 3b: STT worker (Parakeet) and model download (lot report)

Date: 18 October 2026. Branch: `feat/speech-stt-worker`, from `feat/speech-integration` (phase 11 merged, PR #47; working tree clean when the work started; the changes below are
uncommitted).

## What was done

The real dictation engine, out of process, and the way to get its model. A new crate `apps/speech-worker` builds `speech-stt-worker` (sherpa-onnx, Parakeet TDT 0.6B v3 int8, linked from
the no-tts libraries so that no espeak-ng is in it). The application starts it on the first dictation, keeps it while it is used, ends it after 120 s idle, kills it on a cancel. The
protocol, the client, the idle and cancel logic live in the main crate and are tested against a real child process. The settings block gained a working Install button with progress and
Cancel, and a Verify button (the side-load path). **Nothing in the interface starts a download or a process by itself.**

Proven end to end on the owner's laptop with the real files: the zip to upload was served from this machine, downloaded, hashed, unpacked, verified against the compiled-in manifest,
and the installed model then transcribed a real clip through the real worker (ignored test `real_archive_installs_and_the_installed_model_transcribes`).

## Files created

| File | Content |
|---|---|
| `apps/speech-worker/Cargo.toml`, `Cargo.lock` | the worker crate (feature `stt`, default; `sherpa-onnx =1.13.8` optional); its own lock |
| `apps/speech-worker/src/lib.rs`, `wav.rs`, `engine.rs`, `bin/stt.rs` | the protocol (included by path from the main crate), the clip reader, the sherpa engine, the `speech-stt-worker` executable |
| `src-tauri/src/speech/worker_protocol.rs` | requests, answers, the `Engine` trait and the `serve` loop; serde only, shared by source with the worker |
| `src-tauri/src/speech/worker_stt.rs` | `WorkerSttProvider` (the trait of phase 3), spawn, reader thread, cancel kill, idle watcher, discovery by name, `setup_for` |
| `src-tauri/src/speech/console_window.rs` | the one platform detail (`CREATE_NO_WINDOW`), paired `#[cfg(windows)]` / `#[cfg(not(windows))]` |
| `src-tauri/src/speech/models.rs` | the compiled-in manifest, the state of a model folder (no hashing), `verify`, the marker, `JobSlot` (one install or verify at a time) |
| `src-tauri/src/speech/model_download.rs` | the ONE network file: download, hash while writing, unpack the four manifest names, replace the folder, progress and cancel |
| `src-tauri/resources/speech/models-manifest.json` | id, licence, attribution, zip name, size and SHA-256 of the zip and of each file |
| `src-tauri/src/speech/test_support.rs`, `worker_stt_tests.rs`, `models_tests.rs`, `model_download_tests.rs` | 50 new tests plus 2 ignored tests with the real engine |
| `scripts/fetch-speech-resources.sh` and `.ps1` | checksum-pinned fetch of the sherpa-onnx libraries (no-tts and full) with curl, git-ignored target `apps/speech-worker/vendor/` |
| `scripts/build-speech-worker.sh` and `.ps1` | builds the STT worker into `apps/speech-worker/target-stt`, scans it for `espeak` (must find none), stages it under its triple name |
| `src/speech/modelJob.ts` (+ test), `src/state/useModelJob.ts` | the install / verify states, the buttons, the hook behind them |

## Existing files edited (every edit)

| File | Edit |
|---|---|
| `src-tauri/src/commands.rs` | `search_roots` widened to `pub(crate)` (one word). Nothing duplicated |
| `src-tauri/src/error.rs` | one variant `SpeechModelDownloadFailed { folder }`, its code and its data arm |
| `src-tauri/src/lib.rs` | three handler names (`speech_model_install`, `speech_model_verify`, `speech_model_cancel`); still no `.setup(` |
| `src-tauri/src/speech_commands.rs` | `SpeechState` holds the worker provider and the model job slot; `refresh(&app)`; `speech_status` takes an `AppHandle`; the three model commands; `speech_transcribe` refuses while a model job runs |
| `src-tauri/src/speech/mod.rs` | 5 `pub mod`, 4 test modules, doc lines |
| `src-tauri/src/speech/service.rs` | `SpeechService::with_provider` (4 lines), one doc line |
| `src-tauri/src/speech/status.rs` | `ModelState::Unverified`; dictation maps it to `speech_model_missing` |
| `src-tauri/src/speech/wav.rs`, `output_guard.rs` | three comments: the provisional thresholds were checked (see below), no value changed |
| `src-tauri/src/speech/stt_tests.rs` | the network scan allows `model_download.rs` only; the "no engine yet" test became "builds the worker engine"; the three new commands are asserted registered |
| `src/speech/ipc.ts`, `availability.ts` | `ModelState` gains `unverified`; three bridge functions; `INSTALL_AVAILABLE` is `true` |
| `src/components/SpeechSettings.tsx` | Install, Verify, Cancel, the progress line, the failure sentence; the status is read again when a job ends |
| `src/locales/en-US.json`, `fr-FR.json` | 7 keys under `speech`, one error sentence each |
| `src/lib/speechSettings.test.ts`, `speech/speechSources.test.ts`, `speech/availability.test.ts`, `speech/ipc.test.ts`, `components/speechSettings.render.test.ts` | the new code and keys, the new states, the bridge, the buttons |
| `.gitignore` | the worker's `vendor/`, `target*/` and staged executables |
| `docs/SPEECH.md` | protocol as built, codes, models, measured row, invariant test names, spikes S3 and S5, open point 7 |

`Cargo.toml` of the application is **not** touched (no new crate: `reqwest`, `zip`, `sha2`, `futures-util`, `tokio` were there). `.github/workflows/ci.yml` is **not** touched (see
"Questions for the owner").

## Decisions and deviations

- **No `unload` message.** After the idle delay the application sends `shutdown` and the process ends: that is the only way to give back all the memory of the runtime, and a
  respawn costs the same two-second load an unload would. The prompt listed `unload`; none is built because nothing would use it.
- **No `cancel` message** (as the prompt itself says: cancel is the kill). A kill returned in 0.6 s on the real engine mid-decode.
- **Line protocol and stray lines.** sherpa-onnx writes lines of its own to the worker's standard output (for example "Creating a resampler" for a 22 kHz clip). The client skips any
  line that is not an answer and never logs it (it could hold what was heard). The worker's standard error goes to the null device.
- **Fourth model state, `unverified`.** A model copied by hand has every file at its pinned size but has never been hashed. It reports `speech_model_missing` for dictation and the
  settings offer Verify; Install stays off for it. Only Install and Verify hash (about 1.1 GB for Install, 670 MB for Verify); looking at the state costs a few `stat` calls.
- **Provider configured lazily.** `SpeechState::new` builds nothing that runs; `refresh(&app)` hands the provider the worker path and the model folder at each `speech_status` and
  `speech_transcribe` (about a hundred `exists` calls at most). The worker is found through `commands::search_roots`, by name under every root: `speech-stt-worker.exe`,
  `speech-stt-worker`, and the three triple names.
- **Model folder**: `<app local data>/speech-models/<id>/`. Install shuts an idle worker down first (a loaded model keeps its files open and Windows would refuse the replacement),
  and a transcription is refused with `speech_busy` while a model job runs.
- **The model's folder is replaced, not merged**: Install deletes what was in `<id>/` (the app's own data) before moving the checked files in.
- **espeak**: the scan found 0 matching lines in the STT worker and 33 in SpeechLab's `speechlab.exe` (full libraries), so the check can tell them apart.

## Spikes

- **S3, answered.** A `tauri.windows.conf.json` overlay is read on Windows: with `"externalBin": ["binaries/speech-nope"]` the build stops with "resource path `binaries\speech-nope-x86_64-pc-windows-msvc.exe` doesn't exist";
  with `binaries/speech-stt-worker` (staged) it builds. The build script does not watch an overlay that did not exist before, so `build.rs` had to be touched once. **Not added**: the check
  also fails any run that has not staged the worker (CI, a fresh clone). That Tauri ignores the overlay on macOS is its documented behaviour; it waits for the Mac. Overlay removed after the trial.
- **S5, answered.** Measured with two tiny programs: a `windows_subsystem = "windows"` parent starting a console child gets, without a flag, a console window for the child
  (`GetConsoleWindow` non-null: the flash); with `CREATE_NO_WINDOW` it gets none. The flag is in `console_window.rs`. **Reported, not fixed here**: the Tesseract spawn in `ocr/tesseract.rs` sets no flag
  and has the same flash in a release build.

## Measured (the owner's laptop, Core 7 150U, 18 October 2026, release worker, 4 threads requested)

| Figure | Value |
|---|---|
| Worker executable | 18.6 MB; 0 matching lines for `espeak`; no network symbol |
| Cold load | 2.2 s (2.5 s once, 4.3 s once with a cold file cache) |
| Resident memory of the worker process alone | 733 MB after the load, 857 MB peak after decoding (commit 864 MB). SpeechLab's 1.6 to 1.9 GB is the figure the constants keep as the planning value |
| A 10 s clip | 0.9 to 1.1 s, about 7 busy cores (6.6 to 7.1) |
| Cancel of a decode (60 s clip, cancel after 0.5 s) | returned after 0.6 s; no `speech-stt` process left (`tasklist`) |
| Install from a local server, debug build | 50 s for 478 MB (hash of the zip, unpack, hash of the files); a release build is faster |
| Output guard and quiet floors | five real clips (fr, en, de, es, a 46 s voice message): peaks 0.44 to 1.0, levels 0.06 to 0.24, 2.3 to 4.4 words per second, no word above 22 % of a text. All pass with room, nothing changed; five clips of a few speakers are not a calibration of every voice |

`docs/SPEECH.md` section 11 has the measured row. The two "Minimum configuration" estimate rows are unchanged.

## To upload to `https://assistantcabinet.ai/models/speech/` (the owner)

One file, made on this machine, **not** in git: `data/speech-models-upload/parakeet-tdt-0.6b-v3-int8.zip`.

| | Bytes | SHA-256 |
|---|---|---|
| `parakeet-tdt-0.6b-v3-int8.zip` | 478,123,854 | `3781ca818590747c1ff244a8193b541ba8ea32506253832aa1a19ced0645883e` |
| inside: `encoder.int8.onnx` | 652,184,281 | `acfc2b4456377e15d04f0243af540b7fe7c992f8d898d751cf134c3a55fd2247` |
| inside: `decoder.int8.onnx` | 11,845,275 | `179e50c43d1a9de79c8a24149a2f9bac6eb5981823f2a2ed88d655b24248db4e` |
| inside: `joiner.int8.onnx` | 6,355,277 | `3164c13fc2821009440d20fcb5fdc78bff28b4db2f8d0f0b329101719c0948b3` |
| inside: `tokens.txt` | 93,939 | `d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d` |

The four files are flat in the zip (no folder). The address the application asks is `<speechModelsUrl>/parakeet-tdt-0.6b-v3-int8.zip`, that is
`https://assistantcabinet.ai/models/speech/parakeet-tdt-0.6b-v3-int8.zip` with the default setting; today it answers 404, which is what the failure sentence of the human test shows.
Licence: CC BY 4.0 (NVIDIA), attribution kept in the manifest. The row for `models/LICENSES.md` (under `.claudeignore`, not read) is in `docs/SPEECH.md` section 10.

## Tests

Pre-check before the first edit (targeted): `cargo test --lib speech -j 2` 76 passed.
Final pass, after the last edit:

`cargo test --lib -j 2` four consecutive runs green (1031 passed, 6 ignored: 126 speech tests, 50 of them new, plus the two ignored real-engine tests); the integration targets (`--tests`) were green on the
first full pass, before the last edits, which touched only `model_download.rs` (the request now honours a cancel while it waits) and test code, so they were not run again; `cargo clippy --all-targets`: no
warning in a speech file (the other warnings are pre-existing, not investigated), and none in the worker crate; `pnpm run build` ok; `pnpm run test` 71 files, 1725 tests passed (the Rust language guard included);
`rustfmt --check` clean on every new Rust file (9 in the application, 4 in the worker crate), `stt_tests.rs`, `speech_commands.rs`, `error.rs`, `service.rs`, `status.rs`, `commands.rs` at 0 diff blocks before and after;
`cargo check --no-default-features --offline` in `apps/speech-worker` passes, and its two unit tests pass.

**Unstable timing on this laptop, found and fixed.** The download tests opened loopback connections in groups, and the whole test process then froze for 10 to 70 s at a time (with or without the `SSLKEYLOGFILE` variable of the antivirus, so that variable is not the cause; the antivirus itself is suspected, not proven). It made three older phase 3 tests and one of mine fail. Fix: the download tests run one at a time and each on its own runtime. After it, the download tests take 0 to 1 s over six runs and the whole library passes in 13 to 15 s over three runs. A real weakness was fixed on the way: a cancel was not honoured while the request was still being sent.

Covered: the protocol (every message round trip as one line, a stray line skipped, the loop in order, shutdown, end of input, a bad line); the provider with a **real child process** (the test
executable started again as the worker): one process serves two jobs and loads once, stray lines, a load failure for a missing and a corrupt model, a crash, an unreadable clip keeps the
worker, a worker that cannot start, a cancel kills the child in under 5 s, a cancel already set starts nothing, through the service a cancel leaves no clip and no process, the idle end
(graceful, then a new process), no end during a job, shutdown and drop, the worker ends when its input closes; availability for every obstacle in order; discovery by name; the manifest;
the state of a model folder without hashing; Verify (a good side-load, the right size with the wrong bytes, missing, cancelled); hashing against the standard vectors; the one-job slot; the download
against a local server (what is asked: one GET, no identifying header; a good install; a damaged folder replaced; 404; unreachable; a tampered or short archive; a file that does not match;
a missing entry; an archive with `../escaped.txt` writes nothing outside; a cancel before and during; nothing left in any failure); source scans (no network outside `model_download.rs`, no
platform flag outside `console_window.rs`, the protocol file needs no other module, no log of what a worker said, the worker crate has no network dependency).

**Not proven by a test**: the three commands themselves (they need an `AppHandle`), the real window (Channel progress, the buttons), and the installer packaging of the worker. The two
ignored tests run by hand on this laptop: `real_worker_transcribes_a_clip_and_cancel_leaves_no_process` and `real_archive_installs_and_the_installed_model_transcribes`, both passed.
The `.ps1` scripts were parse-checked (PowerShell used once for that, by me) but not run: the `.sh` twins were, in Git Bash. The macOS branches of the scripts are pinned and untested.

## For the next phases

- Phase 5 and 12: call `speechTranscribe`; the provider, the busy flag and the cancel are in place. The worker's first call costs the cold load (about 2.2 s); the timeout floor is 30 s.
- Phase 9: a second worker with the FULL libraries, in its own `CARGO_TARGET_DIR`; `fetch-speech-resources` already downloads that variant.
- Packaging (phase 13 with the Tesseract track): `externalBin` through a Windows-only overlay works (S3), but needs the worker staged on every machine that builds.

## Questions for the owner

1. **CI step.** Add `cargo check --no-default-features` for `apps/speech-worker` to `.github/workflows/ci.yml`? It passes locally (about 6 s, no native library). Not edited: you asked me to ask.
2. **`unload` message** left out (above). Say so if you want it anyway.
3. **Upload of the zip**, then the optional part of the human test.

## Human test

PROPOSED, NOT RUN: it is in the hand-over message (it needs her eyes on the real window).

## Prompt alignment

Checked `06_speech-7_readaloud-text-prep.md`: misaligned on facts only (resources folder now exists with the manifest, `mod.rs` and the scans that read `speech_commands.rs` and every speech file changed,
`SpeechState` fields, branch). Appended "Alignment with phase 3b" with those eight points; no owner decision needed. `STATUS.md` table and notes updated (05a row also set to Merged, PR #47).
