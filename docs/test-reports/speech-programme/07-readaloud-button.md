# Speech phase 8: read-aloud button (lot report)

Date: 18 October 2026. Branch: `feat/speech-readaloud-button`, from `feat/speech-integration` (phase 7 merged, PR #49; working tree clean when the work started; the changes below are uncommitted).

## What was done

The read-aloud button under each finished answer, the engine interface behind it, and the first engine: the operating system's voices, restricted to local ones. TypeScript only: no Rust file was touched, so `cargo test`, `clippy` and `rustfmt` were not run (nothing to prove). Nothing starts at launch and nothing speaks without a click.

- `ReadAloudButton` sits in the idle row that holds Copy and Regenerate, after Regenerate (`MessageList.tsx`, one mount). It reads / stops, shows a speaking indicator (pulsing waves, still with reduced motion), and ends its reading when it leaves the screen (a new question, a regenerate and a clear all hide the row), on Escape (only when something is read and no dialog is open) and on window close.
- With the voice master switch off, the read-aloud switch off, or a language without read-aloud, the button draws nothing: `MessageList` renders byte for byte what it rendered before (the baseline of phase 2 passes unedited, and a new test compares the list inside a hidden provider with the list outside any provider).
- System voices go through `speechSynthesis` and only voices with `localService === true` are ever picked (exact language, then primary language, preferred voice honoured, `null` when none; never a default or an online voice).
- The reading is one object for the whole window (`readAloud.ts`): starting another answer ends the first; a recording refuses a reading with the recorder's sentence and a recording that starts ends the reading (the voice session of phase 4).

## Files created

| File | Content |
|---|---|
| `src/speech/output/engine.ts` | `ReadAloudEngine` (voices, speak with a segment queue and progress, stop), `SpeechOutputError`, `outputErrorCode` |
| `src/speech/output/systemVoices.ts` | `pickLocalVoice`, `loadVoices` (`voiceschanged` with a 1.5 s timeout), `summariseVoices`, the system engine, the two browser accessors |
| `src/speech/output/playback.ts` | The Web Audio helper for worker audio (`decodeAudioData` + buffer source; no blob, no `media-src`); used by phase 9, tested now |
| `src/speech/output/pauses.ts` | Rest after `clause` 120 ms, `sentence` 380 ms, `paragraph` 760 ms; `spokenReading`; `noteSentence` with the right plural |
| `src/speech/output/readingParts.ts` | The parts sent to `speech_prepare_reading`: shown answer (markdown), engine figure (markdown only if it holds a table or list), caution lines (plain) |
| `src/speech/output/engineChoice.ts` | `readAloudVisibility` (the matrix) and `chooseEngine` |
| `src/speech/output/readAloud.ts` | `createReadAloud` (the one reading), `leaveEntry`, `escapeStopsReading` |
| `src/speech/output/ReadAloudContext.tsx` | `ReadAloudProvider` (mounted once in `App`, draws nothing) and `useReadAloud` |
| `src/components/ReadAloudButton.tsx`, `SpeakerGlyph.tsx` | The button and its glyph |
| `src/state/useSystemVoices.ts` | Reads the system voice list for the settings line (only when the system engine is chosen) |
| Tests | `speech/output/{systemVoices,readAloud,readingParts,engineChoice,playback,readAloudSources}.test.ts`, `components/readAloudButton.test.ts` |

## Existing files edited (every edit)

| File | Edit |
|---|---|
| `src/lib/annotations.ts` | `annotationLines` and the new `cautionLines` are two calls of one private `noteLines(..., cautionOnly)`: one logic, no copy. `annotationLines` returns exactly what it returned (its 14 tests pass unchanged) |
| `src/speech/ipc.ts`, `ipc.test.ts` | `speechPrepareReading(parts, locale)` and the types `ReadingPart`, `ReadingItem`, `ReadingPause`, `ReadingNoteCode`; pinned like the other commands |
| `src/components/MessageList.tsx` | One import and one line: `<ReadAloudButton entry={entry} />` after Regenerate |
| `src/App.tsx` | One import and `<ReadAloudProvider speech={snapshot.settings.speech}>` around the `app` div (not re-indented, to keep the diff to three lines) |
| `src/components/SpeechSettings.tsx` | The engine, voice and speed controls are enabled; a line under the engine select lists the local system voices (and counts the online ones ignored) when the engine is "System voices" |
| `src/speech/availability.ts` | `READ_ALOUD_CONTROLS_AVAILABLE` removed (its job is done) |
| `src/components/speechSettings.render.test.ts` | One test inverted: the read-aloud choices are no longer disabled (that test asserted the "not yet" state of phase 4) |
| `src/locales/fr-FR.json`, `en-US.json` | `speech.readAloud` (3), `speech.readAloudNote` (6), `speech.systemVoices` (4); `speech.readAloudSoon` removed. `errors.tts_no_local_voice` and `errors.tts_failed` already existed |
| `src/styles.css` | `.message__actions--turn` may wrap; `.message__read`, `.glyph--speaking`, `.message__speech-note` |
| `docs/SPEECH.md` | An "As built (phase 8)" paragraph in 7.4 |

`commands.rs` and every Rust file are not touched. Helpers widened to `pub(crate)` or duplicated: none.

## Decisions of this phase (the owner may overrule any of them)

1. **The `not_asked_about` exception is mine** (from the prompt): `cautionLines` reads `mixedTableUnavailable` / `mixedDocumentsUnavailable` for `no_evidence`, `gateway_unavailable`, `not_linked`, and not for `not_asked_about`; `documentsNotNeeded` is not read.
2. **System voices are used only when the engine setting is "System voices".** Rust cannot probe the window's voices and a system voice may be an online one, so "Automatic" never falls back to them: choosing "System voices" is the step that follows the offline proof. With "Automatic" and no Piper voice the button is drawn and, when pressed, says `tts_no_local_voice`.
3. **Notes are spoken** (where the table or code stood, then a paragraph rest): a listener who does not see the screen would otherwise not know a table was left out. They are not shown on screen (the table is there).
4. **A reading with no `speech` item does not start**; it says `speech.readAloud.nothingToRead` (a catalogue key, not a new `AppError`).
5. **The offline proof is proposed with the network switched off, not with the SpeechLab firewall script.** That script blocks two named programs (`speechlab.exe`, `whisper-cli.exe`) and states that it does not block WebView2; the system voices run in the webview, so blocking the application would prove nothing. Cutting the network and still hearing the voice proves a local voice (an online "Natural" voice fails). If the owner wants the firewall variant (a rule on `msedgewebview2.exe`), it is a decision for her, and the agent would still not run it.
6. **One cheap query at launch**: the provider calls `speech_status` once (and again when the speech settings or the locale change), like the settings block. It reads settings and probes and starts nothing.
7. **Rate.** The `ttsRate` setting is applied to the system voice (`utterance.rate`). The buffer helper plays at natural speed (the synthesiser of phase 9 applies the rate), as `docs/SPEECH.md` says.
8. The Piper choice has no engine yet (phase 9 supplies it in `ReadAloudContext.tsx`, function `engineFor`); a Piper choice is refused with `tts_no_local_voice`, never replaced by a system voice.

## Tests

Pre-check before the first edit (targeted): the 11 test files of the areas touched, 134 passed, 0 failed.
Final pass, once, after the last edit: `pnpm run build` ok (`tsc --noEmit` and `vite build`); `pnpm run test`: 78 files, 1875 tests, all passed (the Rust language guard and the literal guard included). Rust: not run, no Rust file touched.

Named tests: `a_non_local_voice_is_never_selected`, `no_local_voice_refuses`, `one_voice_at_a_time` (engine and reading), `stop_on_unmount`, `speaking_never_starts_without_a_click`, the visibility matrix (`engineChoice.test.ts`), the parts include the caution notes and not the timings, model, sources or coverage (`readingParts.test.ts`), the voice-off markup unchanged (`readAloudButton.test.ts` and the untouched `voiceOffBaseline.test.ts`), plus: both catalogues have every sentence and every `NoteCode`, the plural of the notes, no network / blob / media element / storage in the new files, no `media-src` and no `blob:` for a script, worker or media in either policy, the button starts a reading only in its click handler, and the unmount cleanup calls `leaveEntry`.

**Not proven by a test**: the sound; which voices WebView2 lists on the owner's laptop (and whether one is local for French); the real `speech_prepare_reading` call through the bridge on a real answer; the React unmount, Escape and window-close paths in the real window (no DOM in the test environment: the logic is tested as pure functions and the wiring by source scans). The human test below is the first place all of these run. I did not play it myself: the app was not started.

## Human test (played by the owner on 18 October 2026)

Passed, every step. Step 3 line: "Voix du système utilisées pour cette langue : Microsoft Hortense - French (France), Microsoft Julie - French (France), Microsoft Paul - French (France)." Reading, stop, offline (Wi-Fi off: the reading works unchanged), Escape, regenerate while reading and the return to the starting state were all fine.

Findings and decisions that follow:
- **The system voices are proven local on her laptop and the owner wants them kept** as a supported option (recorded in `docs/DECISIONS.md`, "D3 amended", and `docs/SPEECH.md` 7.4). They stay opt-in; "Automatic" still does not fall back to them (not decided whether it should).
- **The voice heard was female (Hortense).** A male voice (Paul) is installed, but **it cannot be chosen yet**: the "Reading voice" list shows the Piper voices of the language table, and Rust drops a `ttsVoice` that is not ASCII letters, digits, `-` or `_`, which a Windows voice id is not. Recorded as open point 11 in `docs/SPEECH.md` with the recommended fix (a separate `ttsSystemVoice` setting, built with phase 9's voice choice). Nothing was changed in the code for this: it is a Rust settings change, and it waits for her go.

## Spike S4 (for the owner)

The human test is the procedure: it lists the voices in the settings line (name only, with a count of the online voices ignored), reads a French and an English answer, then reads again with the network switched off.

## For the next phases

- Phase 9 supplies the Piper engine: implement `ReadAloudEngine` over the TTS worker and `createBufferPlayer`, and return it from `engineFor("piper", ...)` in `ReadAloudContext.tsx`; `readAloudVisibility` already treats an available Rust probe as `piper`.
- Phase 5 (dictation) uses the same voice session: starting a recording ends a reading by itself.

## Prompt alignment

Checked `08_speech-5_dictation-chat.md`: appended "Alignment with phase 8" (the provider and its second `speech_status` consumer, the Escape listener, the claim/stop behaviour, the removed constant, the branch). `STATUS.md` updated. No owner decision needed; decision 5 above is the one point she may want to overrule.
