# Speech phase 4: voice settings and audio capture (lot report)

Date: 18 October 2026. Branch: `feat/speech-settings-capture`, from `feat/speech-integration` (phase 2 merged, PR #43; working tree clean when the work
started; the changes below are uncommitted).

## What was done

The "Voice" block of the settings dialog and the audio capture library, mounted nowhere else. No engine, no worker, no model, no network, no Tauri command
added (`speech_status` already existed). With no model the block says so, and nothing outside the settings dialog changes.

## Files created

| File | Content |
|---|---|
| `src/components/SpeechSettings.tsx` | `SpeechSettingsBlock`: one `<details>` (class `settings-voice`), master switch, dictation / read-aloud switches (disabled while the master is off), notes switch (independent), model line, Install button, one line per obstacle of a switched-on feature, language override, the two caps, read-aloud engine / voice / speed (shown, disabled), notes folder name, notes voice commands |
| `src/state/useSpeechStatus.ts` | One `speech_status` query when the block opens and again when the speech settings, the locale or the documents folder change; a reply that arrives after a newer request is dropped |
| `src/speech/ipc.ts` | Types of the `speech_status` answer (with `sttModel`) and `speechStatus()`, the only speech command called |
| `src/speech/availability.ts` | Pure matrix: reason code to `errors.<code>`; `speech_unavailable` is "off" (a choice, no sentence); dictation also needs the microphone; model line key; Install state; override languages and voices of the resolved language from the table |
| `src/speech/limits.ts` | The three bounds of the controls in ONE object (dictation 10-600 s, note 60-3600 s, rate 0.5-2) |
| `src/speech/voiceSession.ts`, `VoiceSessionContext.tsx` | The app-level voice session: `claim(owner, stop)` / `release(owner)` / `holder()` / `subscribe`; one provider |
| `src/audio/wav.ts` | `floatToPcm16`, `encodeWav` (mono 16-bit 16 kHz), `chunkLevel` (RMS), `createStatsAccumulator` (peak, clipped share, nothing of the audio kept), `concatPcm` |
| `src/audio/recorder.ts` | The `AudioCapture` interface (`start` / `stop` / `cancel`), `CaptureError` with four failures (denied, missing, unsupported, failed), `createClipBuilder` (pure), `createWebviewCapture` (getUserMedia with echo cancellation, noise suppression and auto gain off, `AudioContext({ sampleRate: 16000 })`, per-clip resampling by `OfflineAudioContext` as the fallback, 150 ms flush on stop) |
| `public/speech/capture-worklet.js` | Static worklet: blocks of 1024 frames posted to the page, a `flush` message for the last partial block. No address, no request, no storage |
| `src/lib/speechSave.ts` | `speechPatch(current, change)`: the function form the queue needs, `null` when nothing changes |
| Tests | `src/audio/audio.test.ts`, `src/speech/{availability,voiceSession,limits,speechSources}.test.ts`, `src/lib/speechSave.test.ts`, `src/components/speechSettings.render.test.ts` |

## Existing files edited (every edit)

| File | Edit |
|---|---|
| `src-tauri/src/speech/status.rs` | `ModelState` serialises (`missing` / `present` / `corrupt`); `SpeechStatus` gains `stt_model` (camelCase `sttModel`), copied from `Probes.stt_model`. Probes still default to "nothing found" |
| `src-tauri/src/speech/tests.rs` | One test beside the existing ones: the model state is reported with the master switch off, and crosses the bridge as `sttModel` |
| `src/state/useAppSettings.ts` | `saveSpeech(change)`: the queue's function form through `speechPatch`; a failure sets `saveError` like `update` |
| `src/components/SettingsDialog.tsx` | The prop `onSaveSpeech`, one mount of `SpeechSettingsBlock` just above the advanced `<details>` |
| `src/App.tsx` | `saveSpeech` handed to the dialog; the body of `App` renamed `AppContent`; a new `App` wraps it in `VoiceSessionProvider` (the provider renders nothing, so no indentation of the existing JSX changed) |
| `src/components/NumberSetting.tsx` | An optional `disabled` prop |
| `src/styles.css` | The three `.settings-advanced` spacing selectors also match `.settings-voice` |
| `src/locales/fr-FR.json`, `en-US.json` | The `speech` group (labels and sentences of the webview only; the 22 codes keep their `errors.<code>` sentences, no duplicate) |

`commands.rs` is not touched. Helpers widened to `pub(crate)` or duplicated: none. Rust edits: the two lines above only.

## Spike S1: the static worklet under the CSP

Measured in the real Tauri window (WebView2), reached through its remote debugging port, no microphone needed (an oscillator feeds the worklet).

| | `pnpm tauri dev` (`devCsp`, origin `http://localhost:1420`) | release build, `pnpm tauri build --no-bundle` (`csp`, origin `http://tauri.localhost`) |
|---|---|---|
| `audioWorklet.addModule("/speech/capture-worklet.js")` | loaded | loaded |
| CSP violation event | none | none |
| `new AudioContext({ sampleRate: 16000 })` | accepted, rate 16000 | accepted, rate 16000 |
| blocks received in 0.7 s | 8 of 1024 frames | 8 of 1024 frames |
| `flush` | the partial block arrives | the partial block arrives (896 frames in the run) |
| a `blob:` worklet | **loaded** (dev CSP has `'unsafe-inline'`) | blocked |
| a worklet from another origin | blocked | blocked |

Conclusion: nothing in the CSP was widened and `blob:` was never added. **The 16 kHz context is accepted by WebView2, so the per-clip resampling fallback does not run on Windows**; it stays in the code for WKWebView (phase 14), where it is the first thing to measure. Not measured here: `getUserMedia` itself (it would raise the permission prompt of the window), so the microphone permission flow and the real input are proven only by the unit tests of the classification and by phase 5's human test.

## Decisions taken where the prompt left a choice

- `INSTALL_AVAILABLE` and `READ_ALOUD_CONTROLS_AVAILABLE` are two constants in `speech/availability.ts`, both `false`; the phase that builds the piece flips its constant.
- **Install: progress and cancel are not drawn.** No command exists to drive them, so a progress bar would be dead code. The button is shown disabled with its reason (`speech.install.notYet`); phase 3b adds progress and cancel with the download.
- The block shows `settings.speech.speechModelsUrl` as Rust returns it ("downloaded only when you press Install, from ..."); no URL is in a webview source, and an empty value hides the line.
- The block's lines about obstacles appear only for a feature whose switch is on (a switched-off feature is a choice, not a problem).
- `microphoneSupport()` is checked at render time (API presence only): it never opens the microphone.
- Voice session rules: a recorder refuses a second recorder (`speech.session.microphoneBusy`), a playback refuses to start while recording (`speech.session.recording`), a recorder takes the place of a playback by calling its `stop`, a playback never replaces another playback.

## Tests

Pre-check before the first edit (targeted): `cargo test --lib speech::` 26 passed; vitest on `settingsQueue`, `speechSettings`, `voiceOffBaseline` and the guards: 692 passed.

New tests: Rust 1 (the model state); TypeScript 71 in 7 new files (WAV header, sizes and byte order; 16-bit conversion; statistics on synthetic signals; clip building with and without resampling; failure classification; availability matrix; voice session; limits read from `speech/mod.rs`; save queue with overlapping speech saves and the stale-copy case; sentence parity for every key the sources name, in both languages; the CSP gives `blob:` to no script, worker or media; the worklet reaches nothing; the block rendered in both languages). The existing guards add one check per new source file, which is why the suite grows by 115 tests (1374 to 1489) in total.

Final pass, once, after the last edit:

| Check | Result |
|---|---|
| `cargo test --lib --tests --no-fail-fast -j 2` | every test binary `ok`, 0 failed; library 932 passed, 4 ignored (27 in `speech::tests`) |
| `cargo clippy --lib --tests` | no warning in any file of this phase (the crate's existing warnings are elsewhere) |
| `rustfmt --check` | `status.rs` and `tests.rs`: 0 `Diff in` blocks |
| `pnpm run build` | passes |
| `pnpm run test` | 61 files, 1489 tests passed; `voiceOffBaseline.test.ts` passes without any edit |

`-j 2` was needed: the first run failed at link time with Windows error 1455 (paging file too small) when cargo linked many test binaries at once. Not a code problem.

## Things you should look at

1. `target/release/assistant-cabinet-ai.exe` was rebuilt by the spike (a normal build output; the previous binary is gone, `assistant-cabinet-ai.old.exe` is untouched). It predates the last cosmetic fix of the block, so run the human test with `pnpm tauri dev`.
2. The walk of the human test was done in the release window through the debugging port, on a copy of her `settings.json` taken first and put back byte for byte afterwards (checked with `cmp`). The one change made after the walk (the obstacle lines now sit on separate lines inside one `.field`) is covered by a render test, not by the walk.
3. A `blob:` worklet loads in dev (see the table). It does not matter here, because nothing uses one, but it means a dev run cannot prove a `blob:` is refused; the release run does.
4. `src/speech/ipc.ts` sends no audio yet. The raw `Uint8Array` body arrives with the engine (phase 3).

## Not done, on purpose

No microphone button, no dictation, no notes, no playback, no Install download, no progress or cancel for it, no change to the Composer or the answer row. Nothing is mounted outside the settings dialog except the empty voice-session provider.
