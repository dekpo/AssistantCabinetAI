# Speech phase 12: notes dictation (lot report)

Date: 18 October 2026. Branch: `feat/speech-notes-dictation`, from `feat/speech-integration` (phase 6 merged, PR #52; working tree clean when the work started; the changes below are uncommitted).

## What was done

The pill of the Notes section now records. She presses it, speaks, and the words are written into the active note while she goes on speaking; pressing it again finishes (the passages still waiting are written first). Spoken commands (punctuation, title, subtitle, new line, new paragraph, new note) are applied by Rust to each passage, and the name suggestions of phase 6 appear under the note.

- **Segmenter** (`audio/segmenter.ts`, pure). Energy only, no dependency. Named constants: cut on 1.2 s of silence once the passage has 1.5 s (0.7 s at first, changed after the human test), hard cap 25 s cut at the quietest block, 0.25 s pre-roll, a passage with under 0.2 s of speech is dropped, speech threshold = max(0.008, 3 x the room's noise, which falls at once and rises slowly). Every sample of a passage that holds speech is kept unchanged (test); only passages with no speech are dropped.
- **Streaming capture.** `AudioCapture.start` gained an `onBlock` option (the capture keeps nothing, `stop` resolves with an empty clip after the last block). `clipFromSamples` builds one passage's WAV (resampled by the browser if the context did not run at 16 kHz).
- **Controller** (`speech/longDictation.ts`, a state machine with no React and no Tauri): ordered serial queue, "N passages waiting", Stop finishes the queue, Cancel discards it and stops the engine, a failed passage is reported by position (a passage the engine finds too short or too quiet is silence, not a failure), the cap `maxNoteSeconds` stops one second before the length Rust accepts, Escape cancels (not under a dialog). The voice session owner is `notes`.
- **Hook** (`speech/useLongDictation.ts`), held in `App`, not in the pane: the pane can unmount without touching a recording. It returns `undefined` while the voice is off (no pill, D13). The input level has its own channel, so only the pill redraws.
- **Writing into the note** (`speech/noteText.ts`, `appendToNote`): blank, line-break and mark joining rules; a title gets exactly one blank line above it; the offset of the words is returned for the name suggestions. "New note" opens a new draft only after one that holds something.
- **Voice commands, Rust** (`speech/dictation_commands.rs`, pure; command `speech_apply_dictation_commands(text, pendingHeading)`). Words are data: `dictation_commands` in `fr-FR.json` and `en-US.json` (16 commands each; the format accepts `de` and `it` with no logic change). Folded matching, longest phrase first, whole words only; a mark replaces the engine's own punctuation (no doubling); a full stop, question mark or exclamation mark starts the next word with a capital; a title runs to the end of the sentence (its closing full stop dropped), a new-line command or the end of the passage; a title with nothing after it stays open and is handed back with the next passage; "new note" cuts the passage in two. The language and the setting `notesVoiceCommands` are read from the settings in Rust (the window names neither); setting off, or a pack without commands, gives the passage back untouched.
- **Leaving.** Every exit that unmounts the pane (hide the zone, choose another folder, window close, Notes off, Reset) asks while a dictation runs, and the answer ends it. A new dialog `RecordingDialog` is shown when there is no unsaved text. The red dot of the Files | Notes switch follows the recording. Save, New and Discard buttons are locked while a dictation runs.
- **Name suggestions**: `useRepairSuggestions` and `RepairChips` mounted for the active note, one `source` per passage, same selection as the chat.



## Files created


| File                                                                        | Content                                                                                                                         |
| --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `src/audio/segmenter.ts`, `segmenter.test.ts`                               | the segmenter, 15 tests on synthetic tones                                                                                      |
| `src/speech/longDictation.ts`, `longDictation.test.ts`                      | the state machine, 28 tests with a scripted recorder and engine                                                                 |
| `src/speech/useLongDictation.ts`                                            | the hook                                                                                                                        |
| `src/speech/noteText.ts`, `noteText.test.ts`                                | where a passage lands, 10 tests                                                                                                 |
| `src/speech/longDictationSources.test.ts`                                   | source scans (no network, no storage, no send, no save, one engine call, the exit rules in `App`, sentences in both catalogues) |
| `src/components/notes/LiveDictation.tsx`                                    | `NotesDictation`, `LivePill`, `DictationStatus`                                                                                 |
| `src-tauri/src/speech/dictation_commands.rs`, `dictation_commands_tests.rs` | commands, 6 in-crate tests (settings, language, unshipped language)                                                             |
| `src-tauri/tests/speech_dictation_commands.rs`                              | 19 tests in both languages (accents, case, heading rules, doubled punctuation, new note)                                        |




## Existing files edited (every edit)


| File                                                                                                                         | Edit                                                                                                                                                                                                                                                                                              |
| ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/audio/recorder.ts`                                                                                                      | `CaptureOptions.onBlock`, the worklet handler honours it, `clipFromSamples`                                                                                                                                                                                                                       |
| `src/App.tsx`                                                                                                                | `useLongDictation`, `useRepairSuggestions`, `NotesDictation` bundle (moved after `chatScope`), `guardExit` gets `{ recording }`, `guardLeaving` ends the recording on every real exit, window close asks while recording, `onWorkspaceCollapsed` ends it, `RecordingDialog`, red dot, `notesHold` |
| `src/components/notes/NotesPanel.tsx`                                                                                        | `dictation: NotesDictation                                                                                                                                                                                                                                                                        |
| `src/components/notes/NotesDialogs.tsx`                                                                                      | `RecordingDialog`                                                                                                                                                                                                                                                                                 |
| `src/lib/notesDrafts.ts`                                                                                                     | the four "lose the notes" exits also ask when `recording` is true                                                                                                                                                                                                                                 |
| `src/speech/ipc.ts`                                                                                                          | `AppliedCommands`, `speechApplyDictationCommands`                                                                                                                                                                                                                                                 |
| `src/locales/en-US.json`, `fr-FR.json`                                                                                       | `notes.dictation.*` (6) and `notes.recording.*` (3)                                                                                                                                                                                                                                               |
| `src/styles.css`                                                                                                             | `.notes__dictation`                                                                                                                                                                                                                                                                               |
| `src/speech/notesSources.test.ts`, `components/notes/notes.render.test.ts`, `lib/notesDrafts.test.ts`, `audio/audio.test.ts` | the two assertions the prompt named updated (`dictation: notesDictation`, `useLongDictation(`), the pill tests moved to the new bundle shape, new cases                                                                                                                                           |
| `src-tauri/src/speech/speakable_rules.rs`                                                                                    | `rules_for_feature(locale, Feature)` (`rules_for` calls it), `Rules.dictation_commands`, validated                                                                                                                                                                                                |
| `src-tauri/src/speech/mod.rs`, `src/lib.rs`, `src/speech_commands.rs`                                                        | module, handler, the thin command (placed last in the file)                                                                                                                                                                                                                                       |
| `src-tauri/resources/speech/rules/fr-FR.json`, `en-US.json`                                                                  | `dictation_commands`. The loader already ignored unknown keys; no change needed there                                                                                                                                                                                                             |
| `src-tauri/src/speech/worker_stt_tests.rs`                                                                                   | one `#[ignore]` real test: a clip of about 30 s in two passages, in order                                                                                                                                                                                                                         |
| `docs/SPEECH.md`                                                                                                             | section 7.3 "Built as follows" (two places), the exit rule, three invariant rows                                                                                                                                                                                                                  |


No helper of `commands.rs` was widened or duplicated.

## Decisions of this phase (the owner may overrule any of them)

1. **Dragging the separator closed ends the recording at once** (it cannot be intercepted; leaving the microphone open out of sight is worse). What was already written stays in its note.
2. **Save, New and Discard are locked while a dictation runs**, so the notes cannot change under a plan.
3. **"Review and save" in the leave dialog ends the recording first.**
4. **Marks are glued with no space before** `: ; ? !` in French too (the engine writes them that way).
5. **No bullet command** (not in the spec): "new line" gives plain lines, which Markdown preview merges into one paragraph; use "new paragraph" for separated lines.
6. **Chips only for the last piece of a passage that lands in the active note** (the hook takes one source at a time; a passage cut by "new note" loses the chips of its first part).
7. **A bare word such as "point" or "titre" is a command whenever the setting is on**; the setting is the way out.
8. **"New line" is a hard break** (two blanks before the newline), validated by the owner after the human test: visible in the preview, invisible in Brut.
9. **Hiding the zone always asks "Dictée en cours"** (the notes are kept); the exits that lose notes add a line saying the dictation stops too.
10. **Silence of 1.2 s before a cut** (was 0.7 s), validated by the owner.



## Tests

One targeted pre-check before the first edit: vitest on the speech, audio, notes and waveform files (385 passed) and `cargo test --lib speech::tests` (27 passed). Final pass, once, after the last edit of the first build: `cargo test -j 2 --lib --tests`; **I reported it green, but my filter cut the output at 60 lines and one test was not seen failing**: `a_pack_ignores_the_keys_it_does_not_know` (in `tests/speech_speakable.rs`) fed the new typed key `dictation_commands` a value of the old shape and failed. Found when the owner's fixes were tested; the test now only feeds a section the version does not know. Pass after the fixes, once, at the end: `cargo test -j 2 --lib --tests --no-fail-fast`: 1842 passed, 0 failed, 82 targets (the first count, 1568, was a partial output); `pnpm run build` ok; `pnpm run test`: 89 files, 2178 tests passed (the `say` helper of `longDictation.test.ts` now pauses `CUT_SILENCE_SECONDS + 0.3`, since its old default of 1 s was under the new silence). `cargo clippy --all-targets`: no warning in the files I wrote or edited; `pnpm run build` ok; `pnpm run test`: 89 files, 2175 tests passed (was 2058). `rustfmt --check` clean on the three new Rust files; on the modified leaf files (`speech_commands.rs`, `speakable_rules.rs`, `worker_stt_tests.rs`) 0 diff blocks before and after; `lib.rs` has one old diff block at line 19, not mine.

**Not proven by a test**: the React hook and the real microphone (no DOM environment), the thresholds of the segmenter on a real voice (**provisional: I had no recording of hers; the human test is where they are tuned, note what you see**), the `#[ignore]` real test (written, not run: it needs a 30 s clip of hers), the resampling path of a 48 kHz context (WKWebView, phase 14).

**Memory (D12 / minimum configuration):** Parakeet stays loaded between passages (idle unload about 120 s), so a 30 min note keeps 1.6 to 1.9 GB resident for its whole length.

## Human test (played by the owner, 19 October 2026)

Result: **globally conclusive** (steps 1 to 15 done; her laptop, her voice, Parakeet). The delay between the end of a pause and the text is about 2 s at most; a low voice is heard. Findings and what was done about each (all validated by the owner, fixed the same day):

| Finding | Cause | Fix |
|---|---|---|
| "Sous-titre" not recognised (a heading "Sous-titres Motif" was written as plain text); "titre" works | the engine writes the plural "Sous-titres"; the pack held the singular only | variants `sous-titres`, `sous titres` (fr), `subtitles`, `sub titles` (en) in the packs (data only) |
| Full stops added in the middle of a sentence after a pause of more than a second | the passage is cut after 0.7 s of silence and the engine ends every passage with a full stop; the script asked for one-second pauses | `CUT_SILENCE_SECONDS` 0.7 to 1.2 s (about half a second more delay) |
| Line breaks visible in Brut, not in Aperçu | Markdown draws a paragraph break, not a single newline | "new line" writes a hard break (two blanks before the newline), not after a title, not at the start of a text; "new paragraph" stays a blank line |
| Step 11 showed "2 notes ne sont pas enregistrées ... perdues" and not "Dictée en cours" (the notes were in fact kept) | with unsaved text the unsaved-notes dialog took the place of the recording one, although hiding the zone loses only the recording | hiding the zone always asks "Dictée en cours"; the dialogs of the exits that do lose notes add a line "La dictée en cours sera aussi arrêtée." |
| "Nouvelle ligne" did nothing, "à la ligne" worked; one "virgule" came out as a full stop | **not explained**: the logs hold no text, so what the engine wrote is unknown | nothing changed. To see it: switch the voice commands off and dictate the sentence again |

Worked as expected: name suggestion ("Dupont" -> "Jean Dupont", Replace changes only those words), a second note by "nouvelle note", the red dot on Notes, the question when hiding the zone, Save then Analyse, the question to the chat answered from the note, the restore.

Tuning note: the log shows passages of 13 to 25 s although she paused (24.7 s is the cap), so the speech threshold may be too low for her room (noise counted as speech). Not changed; to raise it only if the 1.2 s silence is not enough.

### The script that was played

About 12 minutes. A save writes a file in her Docs folder and runs an Analyse, so it starts with a backup and ends with one restore.

**You start with:** the application closed; Git Bash open; voice switched on in Settings (the Parakeet model installed, as in your last tests); no `Notes` folder inside `C:\Users\elise\AssistantCabinetAI-test\Docs` (check with the first command).

1. In Git Bash, check and back up (the first `ls` must print nothing):

```bash
ls -d /c/Users/elise/AssistantCabinetAI-test/Docs/Notes
mkdir -p /c/Users/elise/acai-backup-notes-dictation
cp /c/Users/elise/AppData/Local/com.assistantcabinetai.desktop/index.sqlite3 \
   /c/Users/elise/AppData/Local/com.assistantcabinetai.desktop/knowledge-identifier.key \
   /c/Users/elise/AppData/Local/com.assistantcabinetai.desktop/tabular.sqlite3 \
   /c/Users/elise/AppData/Local/com.assistantcabinetai.desktop/workbooks.sqlite3 \
   /c/Users/elise/acai-backup-notes-dictation/
```

1. Start the application:

```bash
cd /c/Users/elise/Documents/CURSOR/AssistantCabinetAI/apps/desktop
pnpm tauri dev
```

1. Documents Folder = `C:\Users\elise\AssistantCabinetAI-test\Docs` (already analysed). Tick `dupont-jean.pdf` in the Documents list. Click **Notes** in the title bar. Expected: the Notes section opens with a dark microphone pill at the bottom of the note.
2. Click the pill, allow the microphone if asked. Expected: the pill shows a moving waveform and a clock. Say, with a pause of about one second after each line: "Titre. Compte rendu de la visite." / "Sous-titre. Motif." / "Douleur au dos depuis deux jours, virgule sans fièvre, point." / "Nouvelle ligne. Contrôle prévu le mois prochain." Expected, while you speak: text appears in the note after each pause, in the right order, without waiting for the end; "N passages en attente d'écriture" may show briefly.
3. Say "J'ai croisé Du pont au cabinet ce matin", pause, then click the pill. Expected: the pill returns to the microphone, the last words are written, no waiting line stays.
4. Read the note in **Brut**: a line `# Compte rendu de la visite`, a line `## Motif`, then the sentence with a comma and a full stop, then a line break and "Contrôle prévu le mois prochain." If a command word was written as a word instead (or a word you said as plain text became a mark), note the exact text.
5. Click **Aperçu**: the two headings are drawn as headings. Click **Brut** again.
6. If a line like "Du pont -> " is under the note, click **Remplacer**: only those words change. If none, note it (the engine may have written the name as the base holds it).
7. Click the pill, say "Premier sujet, point. Nouvelle note. Deuxième sujet, point." and click the pill. Expected: a second note appears in the switcher with "Deuxième sujet." and the first one holds "Premier sujet." under the earlier text.
8. Click the pill again, then click **Fichiers** (the switch at the top of the zone). Expected: the red dot shows on **Notes**, the recording goes on. Click **Notes**, then the pill to finish.
9. Click the pill, then click the **Notes** button of the title bar. Expected: a question "Dictée en cours"; click **Annuler**: the recording goes on. Click the **Notes** button again and **Arrêter et abandonner**: the recording stops, the zone closes. Click **Notes** again: your notes are there.
10. Click **Enregistrer cette note** (first note), read the file list, click **Écrire le fichier**. Expected: the file is `Notes/<timestamp>.md`; the analysis runs and the Documents panel shows it; the report says the note is analysed.
11. In the chat, with the new note ticked, ask: "Qu'est-ce que le compte rendu de la visite dit du motif ?" Expected: the answer mentions the back pain from the note and cites the note.
12. Close the application (answer the dialog if one asks about the other note: **Abandonner**).

**You leave with:** the application closed, one file in `Docs/Notes`, the index analysed with it; the restore below puts everything back.

1. Restore (application closed):

```bash
rm -rf /c/Users/elise/AssistantCabinetAI-test/Docs/Notes
cp /c/Users/elise/acai-backup-notes-dictation/* \
   /c/Users/elise/AppData/Local/com.assistantcabinetai.desktop/
rm -rf /c/Users/elise/acai-backup-notes-dictation
```

Note for the report to me: how long after a pause the text appears, whether a pause in the middle of a sentence cut it badly, whether a quiet voice was heard (these are the thresholds to tune).

## For the next phases

- Phase 9 must insert its command above `speech_prepare_reading` in `speech_commands.rs` (three source scans read what follows it): see its alignment section.
- Phase 14 (macOS): the resampling path of `clipFromSamples` and the missing window-close event on Cmd+Q.



## Prompt alignment

Checked `11_speech-9_piper-voice-worker.md`: appended "Alignment with phase 12" (the command must be inserted above `speech_prepare_reading` or three scans fail; the `notes` voice-session owner; `rules_for` unchanged for callers). `STATUS.md` updated. No owner decision needed.