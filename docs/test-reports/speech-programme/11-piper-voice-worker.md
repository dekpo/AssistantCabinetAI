# Speech phase 9: Piper voice worker (lot report)

Date: 19 October 2026. Branch: `feat/speech-piper`, from `feat/speech-integration` (phase 12 merged, PR #53; working tree clean when the work started; the changes below are uncommitted).

## What was done

The quality voice, fully local, in a separate process. Read-aloud now has a second engine next to the system voices: a voice worker that speaks one sentence at a time, played by the window while the next sentence is prepared.

- **Voice worker** (`apps/speech-worker`): binary `speech-tts-worker` (feature `tts`, built with `--no-default-features`, the FULL sherpa-onnx libraries, its own `target-tts`). Both engine families of SpeechLab's `tts.rs`: Piper VITS (phonemizer data folder) and Coqui VITS (character based). Protocol: `load_voice`, `synthesize` (WAV written at a path the application reserved), `shutdown`; cancel is the kill. Its own notice, `NOTICE-TTS-WORKER.md` (espeak-ng, GPL-3.0).
- **Shared protocol** (`worker_protocol.rs`, still one file compiled by both sides): `LoadVoice`, `Synthesize`, `VoiceLoaded`, `Synthesized`, failure kind `nothing_to_say`; the `Engine` trait gained two methods with a default answer `bad_request`, so the speech-to-text worker is unchanged.
- **Voices in the manifest**: four rows (`siwis`, `css10`, `ljspeech`, `libritts_r`) with SHA-256 of the zip and of the two files, the licence rating, and for the Piper voices the `espeak-ng-data` folder pinned as a whole (count, size, one digest). The installer and Verify understand that folder; an entry that climbs out of it refuses the archive.
- **Rust** : `speech/tts.rs` (pure rules: which voice reads a language, speed, voice states), `speech/worker_tts.rs` (client; shares the process handling of `worker_stt.rs`), commands `tts_synthesize(text, locale)` (WAV as the raw body of the answer; voice and speed come from the settings) and `tts_cancel`, `speech_model_install` / `speech_model_verify` take an optional voice id, `speech_status` carries `voices` and a read-aloud probe that is true only when a voice of the reading language is installed and checked.
- **Window**: `piperWorker.ts` (the `ReadAloudEngine`; one sentence ahead; a stop ends the clip, the pause and the synthesis), returned by `engineFor` for `piper`; voices block in the settings (state, size, licence, Install / Verify / Cancel per voice); the voice list carries the rating; `speech/modelEvents.ts` makes every reader of the status refresh when a model job ends (a voice installed from the settings is usable at once).
- **System voice choice (open point 11, her go on 19 Oct 2026)**: setting `ttsSystemVoice` (own validation: up to 200 characters, trimmed, no control character), the list of local system voices when the engine is `system`, read by `pickLocalVoice` through the engine options.
- **Licence rows**: `docs/SPEECH.md` section 10 already holds the four voice rows (siwis flagged prototype); `models/LICENSES.md` is under `.claudeignore`, so the owner pastes them (unchanged since phase 1).

## Deviations and choices

- No `unload` message (the prompt listed one): a loaded voice is replaced by the next `load_voice` and the idle delay ends the process, which frees all the memory. Same choice as phase 3b.
- The window sends the text and the language only; Rust reads the voice and the speed from the settings (the prompt said `{text, voice, speed}`; this keeps "nothing the webview sends decides").
- A new synthesis request cancels the one in progress (latest wins); any failure ends the worker (a reload costs about a second).
- The error sentence `speech_model_download_failed` said "the four files"; it now says "the files" in both languages (a voice has two files and a folder).
- `SpeechOutputError.code` is now a `string` so that a code from Rust (`speech_model_corrupt`, `speech_busy`) keeps its own sentence.

## Files created

| File | Content |
|---|---|
| `apps/speech-worker/src/voice.rs`, `src/bin/tts.rs`, `NOTICE-TTS-WORKER.md` | the engine, the binary, the notice |
| `scripts/build-speech-tts-worker.sh` | build with the full libraries, check that `espeak` is in it, stage the executable and its notice |
| `src-tauri/src/speech/tts.rs`, `worker_tts.rs` | rules and client |
| `src-tauri/src/speech/tts_tests.rs`, `worker_tts_tests.rs` | 15 and 26 tests (one of them ignored: the real engines) |
| `src/speech/output/piperWorker.ts` (+ test, 16), `src/speech/voiceRows.ts` (+ test, 5), `src/speech/modelEvents.ts` (+ test) | the engine of the window, the rows of the settings, the refresh signal |

## Files edited (minimal)

`worker_protocol.rs` (messages), `worker_stt.rs` (`map_failure` arm; `Worker`, `start`, `send`, `await_response`, `kill`, `stop_gracefully`, `poll_step` widened to `pub(super)`; two match arms to `_`), `models.rs` (voice and folder rows, `voice_models`, `find`, `voice_files`, `TreeDigest`), `model_download.rs` (unpack the pinned folder), `status.rs` (`voices`), `temp_audio.rs` (`reserve`), `settings.rs` and `mod.rs` (`ttsSystemVoice`), `speech_commands.rs` (state, refresh, two commands above `speech_prepare_reading`, model id on install and verify), `lib.rs` (two registrations), `test_support.rs` (tiny voice), the manifest, the worker `Cargo.toml`, `lib.rs`, `wav.rs`; in the window `ipc.ts` (both), `engine.ts`, `ReadAloudContext.tsx`, `systemVoices.ts`, `useModelJob.ts`, `useSpeechStatus.ts`, `useSystemVoices.ts`, `SpeechSettings.tsx`, the two catalogues, and the tests that name the new status field or keys. Helpers widened: none of `commands.rs` beyond what phase 3b widened (`search_roots`); `worker_stt.rs` items above.

## Tests and measures (one final pass, after the last edit)

| Check | Result |
|---|---|
| `cargo test -j 2 --lib --tests` | 1888 passed, 0 failed (pre-check of `speech` before the first edit: 140 passed) |
| `cargo clippy` | exit 0; no warning in a file of this phase |
| `pnpm run build` / `pnpm run test` | built / 92 files, 2226 tests passed |
| Worker crate | `cargo check --no-default-features` ok; STT worker rebuilt, still no `espeak` in it; voice worker has `espeak` (11 matching lines) and a 19.0 MB executable |
| Real engines (`#[ignore]`, run once) | voice worker speaks French (siwis, css10) and English (ljspeech, libritts_r) paragraphs; speed 50 / 100 / 200 % gives longer / natural / shorter audio; one worker served the four voices; no audio file left; the four real zips install and verify against the compiled-in manifest |

Measured on the owner's laptop (release build, 4 threads): worker peak working set siwis 214 MB, css10 219 MB, libritts_r 227 MB, ljspeech 281 MB. A paragraph of about 45 words (11.7 to 15.7 s of audio) is spoken in 0.57 to 0.81 s by a bare worker and in 1.0 to 1.4 s through the application's client; first sentence of a reading, worker start and voice load included: siwis 1.7 s, libritts_r 2.5 s, css10 3.3 s, ljspeech 4.2 s. Now in `docs/SPEECH.md` section 11. rustfmt: new files clean; modified files did not increase their `Diff in` count (all were 0).

## Uploaded to `https://assistantcabinet.ai/models/speech/` (the owner, 19 Oct 2026; checked the same day)

Checked from this machine: the four URLs answer 200 `application/zip` with the sizes below, and the SHA-256 of each downloaded file equals the pinned one. Note: on the first attempts the server cut two of the four full downloads short (a plain `curl` got 41.8 MB of the 67.4 MB of `siwis.zip` twice); a resumed download (`curl -C -`) completed with the right hash. The application refuses a short archive and removes it, so an interrupted Install is simply pressed again.

Four files, made in `data/speech-models-upload/` (git-ignored). Each zip holds the model file, `tokens.txt` and, for Piper voices, the `espeak-ng-data` folder.

| File | Bytes | SHA-256 |
|---|---|---|
| `siwis.zip` | 67,428,923 | `f603cbc352c6394a4b65eb0f6f5ea4441eb5f6bcf015330c79ca9f0bd40e0b48` |
| `css10.zip` | 65,346,319 | `bb8abe0f6f4c8c153312755cb46d7a57efe7b09d46580bb456c0c5852768e0c6` |
| `ljspeech.zip` | 67,436,754 | `a2bb421b37479e1baaaf7528404146664cceae52c2e928c55858ba7ba4795474` |
| `libritts_r.zip` | 81,821,685 | `3756e846dd8762c45dbf0917fdc25cc149195a31c3101b25806cecefd5401ee4` |

The voices come from SpeechLab's already downloaded packages (SHA-256 of the source archives in its manifest). Licence status: siwis `review` (prototype, verify before distribution), css10 and libritts_r `attribution`, ljspeech `clear`.

## Human test (essentials, about 10 minutes; nothing is written to her documents, so no backup)

Needs the AI server on the tower. Real Install from the host (uploaded 19 Oct 2026); the voices are not installed yet.

**You start with:** the application closed; Git Bash open.

1. `cd /c/Users/elise/Documents/CURSOR/AssistantCabinetAI/apps/desktop && pnpm tauri dev`.
2. Settings, group **Voix**: **Utiliser la voix** and **Lire les réponses à voix haute** ticked, **Moteur de lecture** on **Automatique**. Under **Voix installées sur cet ordinateur**: two lines, both "Non installé.". Press **Installer** on siwis, then on css10: progress, then "Prêt." (if a download is cut, press it again).
3. Choose **Voix de lecture** = siwis. Ask a question, press **Lire à voix haute**: the voice starts within about 2 s; press **Arrêter la lecture** mid-sentence: it stops at once.
4. Same answer with css10. Opinion: is it a male voice? Which do you prefer?
5. Wi-Fi off, read an answer: unchanged. Wi-Fi back on.
6. **Moteur de lecture** = **Voix du système...**, choose Paul in **Voix du système**, read: you hear Paul. Back to **Automatique**.

**You leave with:** the application open, engine **Automatique**, both voices installed, Wi-Fi on. Nothing to restore.

## Human test result (owner, 19 Oct 2026)

Passed. Real Install from the host worked for the French voices; she also installed the English voices and tested that language: everything worked. Her opinions, recorded as such:

- **Start of a reading.** About 1 second before the first sound with Piper; practically instant with the system voices (French). Not a defect: it is the worker start, the voice load and the first sentence (measured 1.7 to 4.2 s on a cold start). She decided not to reduce it: the worker stays loaded for 120 s after the last reading, so the readings that follow start without that cost; only the first one after a pause pays it again.
- **css10.** "Correct, with a strong accent, but the pronunciation is right." She then confirmed it is a male voice, and an elderly one: open point 9 is closed, the label `male` of the language table stays as it is.
- **siwis versus css10.** No preference stated.
- **System voices.** Not available in English because her system is French: nothing to read with, and the reading answers `tts_no_local_voice`, as designed (local system voices only, no online voice). The French system voices (Paul included) work.
- No error sentence, no finding to fix. Nothing was changed in the code after the test.

## Known limits

- The installer wiring of both workers is phase 13. The host may cut a long download: press Install again (nothing partial is kept).
- The segment queue plays sentence by sentence: a very long sentence starts later than a short one. Rewriting of numbers is phase 7's (the rules apply to every voice).
- css10 gender and the siwis versus css10 preference are her opinion, recorded in the test above, not a measurement.

## Prompt alignment

Checked `12_speech-13_prove-it.md`. Misaligned: it knew one worker and no voice files. Appended "Alignment with phase 9" (two workers, the notice staged beside the voice worker, credits data from the manifest, voices hosted like the STT model, invariant test names). No owner decision needed.
